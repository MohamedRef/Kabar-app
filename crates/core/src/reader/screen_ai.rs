//! The AI pass over an OCR'd screenshot (`structure_screenshot`, spec §11,
//! §19): the model reads the on-device OCR blocks and says what kind of
//! screen it is and which messages it holds, who wrote each and when. The
//! deterministic pass (`screenshot::structure`) stays the instant, offline
//! fallback; both end in a [`ScreenDoc`], so the Reader takes either.
//!
//! - **Input** ([`input`], [`request`]): the golden-set shape
//!   `{ocr_blocks[{text, box[x, y, w, h]}]}`, blocks in reading order, boxes
//!   in whole image pixels. No `image` yet: there is no blob upload, so the
//!   model works from the OCR text alone.
//! - **Privacy:** a screenshot is someone's chat. The task is personal
//!   (`Task::is_personal`) and every request is flagged personal too, so it
//!   never reaches an engine that trains on inputs or a shared cache.
//! - **Validation** ([`StructureScreenshot`]): beyond serde, sides fit the
//!   kind, texts are non-empty, times look like times, and every message and
//!   time must be traceable to the OCR text (normalized, typo-tolerant word
//!   containment), so an invented message is rejected and retried.
//! - **Back to a [`ScreenDoc`]** ([`to_screen_doc`]): each message keeps the
//!   union of the OCR lines it came from as its box.

use std::collections::HashSet;

use kabar_protocol::ai::Task;
use kabar_protocol::reader::{OcrResult, Rect};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::screenshot::{ItemKind, ScreenDoc, ScreenItem, Side, is_time, reading_order};
use crate::ai::{AiOutput, AiRequest};

/// The `structure_screenshot` input for one OCR result.
pub fn input(ocr: &OcrResult) -> Value {
    let blocks: Vec<Value> = reading_order(&ocr.blocks)
        .into_iter()
        .filter_map(|b| {
            let text = b
                .lines
                .iter()
                .map(|l| l.text.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let r = b.bbox;
            let px = |v: f32| v.round().max(0.0) as i64;
            (!text.is_empty()).then(|| json!({"text": text, "box": [px(r.x), px(r.y), px(r.w), px(r.h)]}))
        })
        .collect();
    json!({ "ocr_blocks": blocks })
}

/// The router request: always personal (see the module docs).
pub fn request(ocr: &OcrResult) -> AiRequest {
    AiRequest { input: input(ocr), personal: true, image_sha256: None }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScreenKind {
    Chat,
    Listing,
    Post,
    Ui,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenMessage {
    /// A name, `me` for the screenshot owner's bubbles, `app` for UI text;
    /// absent when the screen doesn't say.
    #[serde(default)]
    pub speaker: Option<String>,
    pub side: Side,
    pub text: String,
    #[serde(default)]
    pub time: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructureScreenshot {
    pub kind: ScreenKind,
    pub messages: Vec<ScreenMessage>,
    /// The model was unsure (unreadable OCR, unclear speakers).
    #[serde(default = "yes")]
    pub confident: bool,
}

fn yes() -> bool {
    true
}

impl AiOutput for StructureScreenshot {
    const TASK: Task = Task::StructureScreenshot;

    fn validate(&self) -> Result<(), String> {
        if self.messages.is_empty() {
            return Err("messages must list the screen's content (at least one message)".into());
        }
        for (i, m) in self.messages.iter().enumerate() {
            let n = i + 1;
            if m.text.trim().is_empty() {
                return Err(format!("message {n} has empty text"));
            }
            match (self.kind, m.side) {
                (ScreenKind::Chat, _) | (_, Side::Center) => {}
                _ => {
                    return Err(format!(
                        "message {n} has side left/right, but only chat bubbles do; in a listing, post or ui every message is center"
                    ));
                }
            }
            if m.side == Side::Right
                && let Some(s) = m.speaker.as_deref()
                && !s.trim().eq_ignore_ascii_case("me")
            {
                return Err(format!(
                    "message {n} is a right-side bubble, which is the screenshot owner: speaker must be \"me\", not \"{s}\""
                ));
            }
            if let Some(t) = &m.time
                && !looks_like_time(t)
            {
                return Err(format!(
                    "message {n}: time \"{t}\" is not a time; use the time printed on the screen or omit it"
                ));
            }
        }
        Ok(())
    }

    fn validate_against(&self, input: &Value) -> Result<(), String> {
        let source = Source::new(
            input["ocr_blocks"].as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()),
        );
        for (i, m) in self.messages.iter().enumerate() {
            let missing = source.untraced(&m.text);
            if !missing.is_empty() {
                return Err(format!(
                    "message {} (\"{}\") is not in the OCR text: {} not found. Copy messages from the OCR blocks only; never invent, translate or merge in other text",
                    i + 1,
                    m.text.trim(),
                    missing.join(", "),
                ));
            }
            if let Some(t) = &m.time
                && !source.untraced(t).is_empty()
            {
                return Err(format!("message {}: time \"{t}\" is not in the OCR text", i + 1));
            }
        }
        Ok(())
    }

    fn needs_review(&self) -> bool {
        !self.confident
    }
}

/// The validated answer as a Reader layout. Chats become bubbles (right =
/// mine, so no speaker), everything else plain text; chrome is already gone.
pub fn to_screen_doc(answer: &StructureScreenshot, ocr: &OcrResult) -> ScreenDoc {
    let lines: Vec<(Rect, Vec<String>)> =
        ocr.blocks.iter().flat_map(|b| &b.lines).map(|l| (l.bbox, tokens(&l.text))).collect();
    let mut claimed = vec![false; lines.len()];
    let chat = answer.kind == ScreenKind::Chat;

    let items = answer
        .messages
        .iter()
        .map(|m| {
            let words: HashSet<String> = tokens(&m.text).into_iter().collect();
            let time_words: HashSet<String> = m.time.iter().flat_map(|t| tokens(t)).collect();
            let mut bbox: Option<Rect> = None;
            for (i, (r, line)) in lines.iter().enumerate() {
                if claimed[i] || line.is_empty() {
                    continue;
                }
                let hits = line.iter().filter(|w| words.contains(*w)).count();
                let covered = line.iter().filter(|w| words.contains(*w) || time_words.contains(*w)).count();
                // Mostly this message's words (a time glued on is fine), and
                // at least one word of its text, so a lone "10.34" line isn't
                // claimed by every message sent at 10.34.
                if hits > 0 && covered * 5 >= line.len() * 3 {
                    claimed[i] = true;
                    bbox = Some(bbox.map_or(*r, |b| union(b, *r)));
                }
            }
            let speaker = m
                .speaker
                .as_deref()
                .map(str::trim)
                .filter(|s| {
                    !s.is_empty() && !s.eq_ignore_ascii_case("me") && !(chat && m.side == Side::Right)
                })
                .map(String::from);
            ScreenItem {
                kind: if chat { ItemKind::Bubble { side: m.side } } else { ItemKind::Text },
                text: m.text.trim().to_string(),
                time: m.time.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(String::from),
                speaker,
                bbox: bbox.unwrap_or_default(),
            }
        })
        .collect();
    ScreenDoc { items, looks_like_chat: chat }
}

fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    Rect { x, y, w: a.right().max(b.right()) - x, h: a.bottom().max(b.bottom()) - y }
}

/// Lowercased runs of letters and digits; punctuation and emoji drop out.
fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

/// The OCR text, for tracing a message back to it.
struct Source {
    words: HashSet<String>,
    /// Every word run together: catches a model splitting a word OCR glued
    /// ("piroSek" → "piro", "Sek").
    stream: String,
}

impl Source {
    fn new<'a>(texts: impl IntoIterator<Item = &'a str>) -> Self {
        let all: Vec<String> = texts.into_iter().flat_map(tokens).collect();
        Self { stream: all.concat(), words: all.into_iter().collect() }
    }

    fn has(&self, w: &str) -> bool {
        let n = w.chars().count();
        self.words.contains(w)
            || (n >= 3 && self.stream.contains(w))
            // An obvious OCR slip fixed by the model ("pusst" → "pusat").
            || (n >= 4 && self.words.iter().any(|o| within_one_edit(o, w)))
    }

    /// The words of `text` not found in the OCR text, when more are missing
    /// than OCR fixes explain (none up to five words, then one in six).
    fn untraced(&self, text: &str) -> Vec<String> {
        let words = tokens(text);
        let missing: Vec<String> = words.iter().filter(|w| !self.has(w)).cloned().collect();
        if missing.len() <= words.len() / 6 { Vec::new() } else { missing }
    }
}

/// Levenshtein distance ≤ 1.
fn within_one_edit(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let (short, long) = if a.len() <= b.len() { (&a, &b) } else { (&b, &a) };
    if long.len() - short.len() > 1 {
        return false;
    }
    let prefix = short.iter().zip(long.iter()).take_while(|(x, y)| x == y).count();
    if short.len() == long.len() {
        short[prefix..].iter().skip(1).eq(long[prefix..].iter().skip(1))
    } else {
        short[prefix..] == long[prefix + 1..]
    }
}

const RELATIVE: [&str; 12] = [
    "kemarin",
    "kemaren",
    "baru saja",
    "barusan",
    "sekarang",
    "hari ini",
    "wingi",
    "saiki",
    "yesterday",
    "today",
    "just now",
    "now",
];
const WEEKDAYS: [&str; 14] = [
    "senin", "selasa", "rabu", "kamis", "jumat", "sabtu", "minggu", "mon", "tue", "wed", "thu", "fri", "sat",
    "sun",
];
const UNITS: [&str; 34] = [
    "s", "d", "dtk", "detik", "m", "mnt", "menit", "min", "mins", "j", "jam", "h", "hr", "hrs", "hari", "mg",
    "minggu", "w", "wk", "bln", "bulan", "mo", "th", "thn", "tahun", "y", "yr", "sec", "minute", "minutes",
    "hour", "hours", "day", "days",
];
const MONTHS: [&str; 16] = [
    "jan", "feb", "mar", "apr", "mei", "may", "jun", "jul", "agu", "agt", "aug", "sep", "okt", "oct", "nov",
    "des",
];

/// What a chat, post or notification prints as its time: a clock
/// ("10.42", "08:05 PM"), a date with or without one ("12 Okt 2026, 14.22",
/// "1-6", "Kemarin 21.40"), or an age ("2 jam", "3j", "5m ago").
pub fn looks_like_time(t: &str) -> bool {
    let lower = t.trim().to_lowercase();
    if lower.is_empty() || lower.chars().count() > 32 {
        return false;
    }
    if is_time(&lower) {
        return true;
    }
    let parts: Vec<&str> =
        lower.split(|c: char| c.is_whitespace() || c == ',').filter(|p| !p.is_empty()).collect();
    if parts.windows(2).any(|w| is_time(&w.join(" "))) || parts.iter().any(|p| is_time(p)) {
        return true;
    }
    if RELATIVE.contains(&lower.as_str()) || WEEKDAYS.iter().any(|d| lower.starts_with(d)) {
        return true;
    }
    let age = lower
        .strip_suffix("yang lalu")
        .or_else(|| lower.strip_suffix("lalu"))
        .or_else(|| lower.strip_suffix("ago"))
        .unwrap_or(&lower)
        .trim();
    let digits = age.find(|c: char| !c.is_ascii_digit()).unwrap_or(age.len());
    if digits > 0 {
        let rest = age[digits..].trim();
        if UNITS.contains(&rest) {
            return true;
        }
        // "12 Okt", "12 Okt 2026"
        let words: Vec<&str> = rest.split_whitespace().collect();
        if let [month, year @ ..] = words.as_slice()
            && month.get(..3).is_some_and(|m| MONTHS.contains(&m))
            && year.len() <= 1
            && year.iter().all(|y| y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()))
        {
            return true;
        }
    }
    numeric_date(age)
}

/// "1-6", "12/10", "12-10-2026", "12.10.26": day and month of one or two
/// digits, then an optional two- or four-digit year.
fn numeric_date(s: &str) -> bool {
    let groups: Vec<&str> = s.split(['-', '/', '.']).collect();
    let digits = |g: &str| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit());
    match groups.as_slice() {
        [d, m, rest @ ..] if rest.len() <= 1 => {
            digits(d)
                && digits(m)
                && d.len() <= 2
                && m.len() <= 2
                && rest.iter().all(|y| digits(y) && matches!(y.len(), 2 | 4))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
