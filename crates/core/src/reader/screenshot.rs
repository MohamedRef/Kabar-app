//! Deterministic first pass over an OCR'd screenshot (spec §11): decide
//! whether it is a chat, and split it into bubbles, timestamps, UI labels
//! and plain text in reading order. The AI pass (`structure_screenshot`)
//! refines this later; this works offline and instantly.
//!
//! Heuristics, all relative to the image size (W × H), kept deliberately
//! modest:
//!
//! - **Reading order:** blocks sorted top to bottom; blocks whose tops fall
//!   above the middle of the row's first block join that row, read left to
//!   right.
//! - **Zones:** the status bar (bottom edge in the top 5% of H: clock,
//!   battery %) and, for chats, the header (top edge in the top 11%: back
//!   arrow, contact name, "online") and the input bar (top edge in the
//!   bottom 7%: "Ketik pesan") are UI, not content.
//! - **Chat detection:** a body block *hugs the left edge* when it starts
//!   within 15% of W and ends before 80%; it *hugs the right edge* when it
//!   ends past 85% and starts after 20%. It looks like a chat (WhatsApp,
//!   Telegram, Instagram DMs) when at least two body blocks hug an edge, they
//!   are at least half of the body, and either two of them end in a time or
//!   both sides are present (with a time, or with four or more hugging
//!   blocks). A marketplace listing is left-aligned too, but has no times
//!   and no right-hand column.
//! - **In a chat:** a centered block (center within 5% of W of the middle,
//!   margins over 8% of W) is a Label ("Hari ini", the end-to-end
//!   encryption notice); a block that is only a time is a Timestamp; any
//!   other block is a Bubble on the side whose margin is closer. A trailing
//!   time on a bubble's last line ("12:41", "12.41", "08:05 PM", optionally
//!   followed by read ticks) moves into `time`. If the header's first line
//!   looks like a contact name, it becomes the `speaker` of left bubbles;
//!   right bubbles are mine (`speaker: None`).
//! - **Not a chat:** every block is Text in reading order, except status-bar
//!   blocks that look like status (times, percentages, one or two short
//!   words), which are Labels.

use kabar_protocol::reader::{OcrBlock, OcrResult, Rect};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
    Center,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ItemKind {
    Bubble { side: Side },
    Timestamp,
    Label,
    Text,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenItem {
    pub kind: ItemKind,
    pub text: String,
    /// The bubble's time, or the Timestamp itself.
    pub time: Option<String>,
    /// Who wrote a left bubble, when the chat header names them. `None` on a
    /// right bubble means "me".
    pub speaker: Option<String>,
    pub bbox: Rect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenDoc {
    pub items: Vec<ScreenItem>,
    pub looks_like_chat: bool,
}

impl ScreenDoc {
    /// The learnable text: bubbles and Text items, one per line, in reading
    /// order. Timestamps and labels are dropped.
    pub fn reading_text(&self) -> String {
        self.items
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::Bubble { .. } | ItemKind::Text))
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

const STATUS_BOTTOM: f32 = 0.05;
const HEADER_TOP: f32 = 0.11;
const INPUT_TOP: f32 = 0.93;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Zone {
    Status,
    Header,
    Body,
    Input,
}

/// One OCR block, pre-digested.
struct Block {
    bbox: Rect,
    lines: Vec<String>,
    /// All lines joined with spaces, time included.
    full: String,
    /// The text with a trailing time removed.
    text: String,
    time: Option<String>,
    zone: Zone,
}

pub fn structure(ocr: &OcrResult) -> ScreenDoc {
    let (w, h) = (ocr.width.max(1.0), ocr.height.max(1.0));
    let blocks: Vec<Block> = reading_order(&ocr.blocks).into_iter().filter_map(|b| digest(b, h)).collect();

    let body: Vec<&Block> = blocks.iter().filter(|b| b.zone == Zone::Body && !b.text.is_empty()).collect();
    let left: Vec<&&Block> = body.iter().filter(|b| hugs_left(&b.bbox, w)).collect();
    let right: Vec<&&Block> = body.iter().filter(|b| hugs_right(&b.bbox, w)).collect();
    let hugging = left.len() + right.len();
    let timed = left.iter().chain(&right).filter(|b| b.time.is_some()).count();
    let both_sides = !left.is_empty() && !right.is_empty();
    let looks_like_chat = hugging >= 2
        && hugging * 2 >= body.len()
        && (timed >= 2 || (both_sides && (timed >= 1 || hugging >= 4)));

    let items = if looks_like_chat { chat_items(&blocks, w) } else { plain_items(&blocks) };
    ScreenDoc { items, looks_like_chat }
}

fn chat_items(blocks: &[Block], w: f32) -> Vec<ScreenItem> {
    // A header line that ends in a time or hugs the right edge is really the
    // first bubble of a cropped screenshot, not UI.
    let is_header = |b: &Block| {
        b.zone == Zone::Header && b.time.is_none() && !hugs_right(&b.bbox, w) && !centered(&b.bbox, w)
    };
    let speaker = blocks.iter().filter(|b| is_header(b)).find_map(|b| b.lines.first().filter(|l| is_name(l)));

    blocks
        .iter()
        .map(|b| {
            let item = |kind, text: &str, time: Option<String>, speaker: Option<String>| ScreenItem {
                kind,
                text: text.to_string(),
                time,
                speaker,
                bbox: b.bbox,
            };
            match b.zone {
                Zone::Status | Zone::Input => item(ItemKind::Label, &b.full, None, None),
                Zone::Header if is_header(b) => item(ItemKind::Label, &b.full, None, None),
                _ if b.text.is_empty() => item(ItemKind::Timestamp, &b.full, b.time.clone(), None),
                _ if centered(&b.bbox, w) => item(ItemKind::Label, &b.full, None, None),
                _ => {
                    let side = if b.bbox.x <= w - b.bbox.right() { Side::Left } else { Side::Right };
                    let who = if side == Side::Left { speaker.cloned() } else { None };
                    item(ItemKind::Bubble { side }, &b.text, b.time.clone(), who)
                }
            }
        })
        .collect()
}

fn plain_items(blocks: &[Block]) -> Vec<ScreenItem> {
    blocks
        .iter()
        .map(|b| {
            let status = b.zone == Zone::Status && b.lines.iter().all(|l| looks_like_status(l));
            ScreenItem {
                kind: if status { ItemKind::Label } else { ItemKind::Text },
                text: b.full.clone(),
                time: None,
                speaker: None,
                bbox: b.bbox,
            }
        })
        .collect()
}

/// Sort blocks top to bottom; a block that starts above the middle of the
/// current row's first block joins that row, and rows read left to right.
pub(crate) fn reading_order(blocks: &[OcrBlock]) -> Vec<&OcrBlock> {
    let mut sorted: Vec<&OcrBlock> = blocks.iter().collect();
    sorted.sort_by(|a, b| a.bbox.y.total_cmp(&b.bbox.y).then(a.bbox.x.total_cmp(&b.bbox.x)));
    let mut out = Vec::with_capacity(sorted.len());
    let mut row: Vec<&OcrBlock> = Vec::new();
    for b in sorted {
        if let Some(first) = row.first()
            && b.bbox.y >= first.bbox.center_y()
        {
            row.sort_by(|a, b| a.bbox.x.total_cmp(&b.bbox.x));
            out.append(&mut row);
        }
        row.push(b);
    }
    row.sort_by(|a, b| a.bbox.x.total_cmp(&b.bbox.x));
    out.append(&mut row);
    out
}

fn digest(b: &OcrBlock, h: f32) -> Option<Block> {
    let mut lines: Vec<String> = b
        .lines
        .iter()
        .map(|l| l.text.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }
    let full = lines.join(" ");
    // A time on its own last line (ML Kit often splits it off), or at the
    // end of the last line.
    let mut time = None;
    if let Some(last) = lines.last()
        && let Some((rest, t)) = split_trailing_time(last)
    {
        time = Some(t);
        if rest.is_empty() {
            lines.pop();
        } else {
            *lines.last_mut().expect("non-empty") = rest;
        }
    }
    let zone = if b.bbox.bottom() <= STATUS_BOTTOM * h {
        Zone::Status
    } else if b.bbox.y < HEADER_TOP * h {
        Zone::Header
    } else if b.bbox.y >= INPUT_TOP * h {
        Zone::Input
    } else {
        Zone::Body
    };
    Some(Block { bbox: b.bbox, text: lines.join(" "), lines, full, time, zone })
}

fn hugs_left(r: &Rect, w: f32) -> bool {
    r.x < 0.15 * w && r.right() < 0.80 * w
}

fn hugs_right(r: &Rect, w: f32) -> bool {
    r.right() > 0.85 * w && r.x > 0.20 * w
}

fn centered(r: &Rect, w: f32) -> bool {
    (r.center_x() - w / 2.0).abs() < 0.05 * w && r.x > 0.08 * w && r.right() < 0.92 * w
}

/// "12:41", "12.41", "8:05", "08:05 PM", "8.05pm". Hours 0–23, minutes 00–59.
pub(crate) fn is_time(s: &str) -> bool {
    let s = s.trim();
    let lower = s.to_ascii_lowercase();
    let (core, ampm) = match lower.strip_suffix("am").or_else(|| lower.strip_suffix("pm")) {
        Some(c) => (c.trim_end(), true),
        None => (lower.as_str(), false),
    };
    let Some((hh, mm)) = core.split_once([':', '.']) else { return false };
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    if !(digits(hh) && digits(mm) && hh.len() <= 2 && mm.len() == 2) {
        return false;
    }
    let (h, m): (u32, u32) = (hh.parse().unwrap_or(99), mm.parse().unwrap_or(99));
    m < 60 && if ampm { (1..=12).contains(&h) } else { h < 24 }
}

/// Split a trailing time (and read ticks after it) off a line:
/// `"Sek tak lapor pusat 12:41"` → `("Sek tak lapor pusat", "12:41")`.
/// The rest is empty when the whole line is a time.
pub(crate) fn split_trailing_time(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '✓' | '✔' | '√'));
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    let n = words.len();
    for k in [2, 1] {
        if n >= k {
            let t = words[n - k..].join(" ");
            if is_time(&t) {
                return Some((words[..n - k].join(" "), t));
            }
        }
    }
    None
}

/// Status-bar text: a time, a battery percentage, or one or two short words
/// (carrier, "LTE", "5G").
fn looks_like_status(line: &str) -> bool {
    is_time(line)
        || line.contains('%')
        || (line.chars().count() <= 12 && line.split_whitespace().count() <= 2)
}

/// Whether a header line reads like a contact or group name rather than UI.
fn is_name(line: &str) -> bool {
    let l = line.trim();
    let lower = l.to_lowercase();
    const UI: [&str; 10] = [
        "online",
        "daring",
        "mengetik",
        "typing",
        "terakhir dilihat",
        "last seen",
        "whatsapp",
        "telegram",
        "instagram",
        "messages",
    ];
    (2..=40).contains(&l.chars().count())
        && l.chars().any(char::is_alphabetic)
        && !l.contains('%')
        && !is_time(l)
        && !UI.iter().any(|u| lower.starts_with(u))
}

#[cfg(test)]
mod tests {
    use kabar_protocol::reader::{OcrEngine, OcrLine};

    use super::*;

    const W: f32 = 1080.0;
    const H: f32 = 2340.0;

    fn block(x: f32, y: f32, w: f32, lines: &[&str]) -> OcrBlock {
        let lh = 48.0;
        OcrBlock {
            bbox: Rect { x, y, w, h: lh * lines.len() as f32 },
            lines: lines
                .iter()
                .enumerate()
                .map(|(i, t)| OcrLine {
                    text: (*t).into(),
                    bbox: Rect { x, y: y + lh * i as f32, w, h: lh },
                    confidence: Some(0.9),
                })
                .collect(),
        }
    }

    fn ocr(blocks: Vec<OcrBlock>) -> OcrResult {
        OcrResult { engine: OcrEngine::MlKit, width: W, height: H, blocks }
    }

    fn chat_fixture() -> OcrResult {
        serde_json::from_str(include_str!("../../../../fixtures/ocr/whatsapp_jv_chat.mlkit.json")).unwrap()
    }

    fn bubbles(doc: &ScreenDoc) -> Vec<(Side, &str, Option<&str>, Option<&str>)> {
        doc.items
            .iter()
            .filter_map(|i| match i.kind {
                ItemKind::Bubble { side } => {
                    Some((side, i.text.as_str(), i.time.as_deref(), i.speaker.as_deref()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn whatsapp_fixture_becomes_bubbles() {
        let doc = structure(&chat_fixture());
        assert!(doc.looks_like_chat);
        assert_eq!(
            bubbles(&doc),
            [
                (Side::Left, "Sek tak lapor pusat", Some("12:41"), Some("Pakdhe Joko")),
                (Side::Right, "ng prabowo?", Some("12:42"), None),
                (Side::Left, "piye dhe", Some("12:43"), Some("Pakdhe Joko")),
                (Side::Right, "arp dicul piro", Some("12.44"), None),
            ]
        );
        assert_eq!(doc.reading_text(), "Sek tak lapor pusat\nng prabowo?\npiye dhe\narp dicul piro");
    }

    #[test]
    fn whatsapp_fixture_ui_is_labelled() {
        let doc = structure(&chat_fixture());
        let labels: Vec<&str> =
            doc.items.iter().filter(|i| i.kind == ItemKind::Label).map(|i| i.text.as_str()).collect();
        assert!(labels.contains(&"12:45"), "status-bar clock: {labels:?}");
        assert!(labels.contains(&"87%"));
        assert!(labels.contains(&"Pakdhe Joko online"));
        assert!(labels.contains(&"Hari ini"));
        assert!(labels.iter().any(|l| l.starts_with("Pesan dan panggilan terenkripsi end-to-end")));
        assert!(labels.contains(&"Ketik pesan"));
        assert!(doc.items.iter().all(|i| i.kind != ItemKind::Text));
    }

    #[test]
    fn fixture_round_trips_through_serde() {
        let doc = structure(&chat_fixture());
        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains(r#""kind":"bubble","side":"left""#), "{json}");
        assert!(json.contains(r#""kind":"label""#));
        assert_eq!(serde_json::from_str::<ScreenDoc>(&json).unwrap(), doc);
    }

    #[test]
    fn standalone_time_is_a_timestamp() {
        let doc = structure(&ocr(vec![
            block(60.0, 400.0, 420.0, &["Sek tak lapor pusat 12:41"]),
            block(640.0, 500.0, 380.0, &["ng prabowo? 12:42"]),
            block(60.0, 600.0, 260.0, &["piye dhe"]),
            block(200.0, 660.0, 120.0, &["12:43"]),
        ]));
        assert!(doc.looks_like_chat);
        let ts = doc.items.iter().find(|i| i.kind == ItemKind::Timestamp).expect("a timestamp");
        assert_eq!((ts.text.as_str(), ts.time.as_deref()), ("12:43", Some("12:43")));
        assert_eq!(bubbles(&doc)[2], (Side::Left, "piye dhe", None, None));
    }

    #[test]
    fn cropped_chat_without_header_has_no_speaker() {
        // Cropped to the bubbles: the first one sits where a header would be.
        let doc = structure(&ocr(vec![
            block(60.0, 150.0, 420.0, &["Sek tak lapor pusat 12:41"]),
            block(640.0, 260.0, 380.0, &["ng prabowo? 12:42"]),
            block(60.0, 360.0, 260.0, &["piye dhe 12:43"]),
        ]));
        assert!(doc.looks_like_chat);
        let b = bubbles(&doc);
        assert_eq!(b.len(), 3);
        assert_eq!(b[0], (Side::Left, "Sek tak lapor pusat", Some("12:41"), None));
    }

    #[test]
    fn marketplace_listing_is_plain_text() {
        let doc = structure(&ocr(vec![
            block(40.0, 20.0, 110.0, &["09:12"]),
            block(940.0, 20.0, 100.0, &["64%"]),
            block(40.0, 1300.0, 420.0, &["Rp26.000.000"]),
            block(
                40.0,
                1380.0,
                1000.0,
                &["iphone 17 pm 512gb resmi ibox mulus nomin", "garansi jalan minat wa"],
            ),
            block(40.0, 1520.0, 600.0, &["Jakarta Selatan · 2 hari lalu"]),
            block(700.0, 1600.0, 340.0, &["Chat penjual"]),
        ]));
        assert!(!doc.looks_like_chat);
        assert_eq!(doc.items[0].kind, ItemKind::Label);
        assert_eq!(doc.items[1].kind, ItemKind::Label);
        assert!(doc.items[2..].iter().all(|i| i.kind == ItemKind::Text && i.speaker.is_none()));
        assert_eq!(
            doc.reading_text(),
            "Rp26.000.000\niphone 17 pm 512gb resmi ibox mulus nomin garansi jalan minat wa\n\
             Jakarta Selatan · 2 hari lalu\nChat penjual"
        );
    }

    #[test]
    fn same_row_reads_left_to_right() {
        let doc = structure(&ocr(vec![
            block(600.0, 1000.0, 400.0, &["garansi jalan"]),
            block(40.0, 1010.0, 400.0, &["resmi ibox"]),
            block(40.0, 1200.0, 900.0, &["minat wa"]),
        ]));
        assert_eq!(doc.reading_text(), "resmi ibox\ngaransi jalan\nminat wa");
    }

    #[test]
    fn times() {
        for t in ["12:41", "12.41", "8:05", "08:05 PM", "8.05pm", "00:00", "23:59"] {
            assert!(is_time(t), "{t}");
        }
        for t in ["26.000.000", "12.5", "24:00", "12:60", "13:00 PM", "piro", "512gb", "3,5"] {
            assert!(!is_time(t), "{t}");
        }
        assert_eq!(
            split_trailing_time("arp dicul piro 08:05 PM ✓✓"),
            Some(("arp dicul piro".into(), "08:05 PM".into()))
        );
        assert_eq!(split_trailing_time("12:41"), Some((String::new(), "12:41".into())));
        assert_eq!(split_trailing_time("Rp26.000.000"), None);
    }

    #[test]
    fn empty_ocr_is_empty_doc() {
        let doc = structure(&ocr(vec![]));
        assert!(!doc.looks_like_chat && doc.items.is_empty());
        assert_eq!(doc.reading_text(), "");
    }
}
