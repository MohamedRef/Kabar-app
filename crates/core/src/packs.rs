//! Offline packs (SPEC §20, PLAN §7–8): which target-language lines of a unit
//! get pre-generated audio, the content-addressed key of each clip, building
//! a unit's [`PackManifest`], the monthly TTS budget, and which units to
//! fetch just in time. Pure logic: the build tool and the app share it, so
//! both compute the same key for the same line.

use std::collections::{BTreeMap, BTreeSet};

use kabar_protocol::cloud::Voice;
use kabar_protocol::lesson::{Dialogue, ExerciseKind, LessonContent};
use kabar_protocol::pack::{
    AUDIO_KEY_VERSION, AUDIO_MIME, AUDIO_OUTPUT_FORMAT, AudioEntry, PACK_FORMAT, PackIndex, PackLesson,
    PackManifest, PackSummary, PackTotals, pack_file, pack_id,
};
use sha2::{Digest, Sha256};

/// Azure F0's free neural TTS characters per month (SPEC §20).
pub const FREE_MONTHLY_CHARS: u64 = 500_000;
/// What the build tool may spend per month by default. The TTS proxy at run
/// time (Kabar Radio, new lines) gets 190k; the last 30k are headroom, so a
/// counting slip can't run the month's voices dry.
pub const DEFAULT_MONTHLY_BUDGET: u64 = 280_000;

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Trims and collapses whitespace, so a line typed with a stray double space
/// or newline still finds its clip.
pub fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The content address of a clip: same voice, text and SSML parameters →
/// same key, in every unit and on every device.
pub fn audio_key(voice: Voice, text: &str) -> String {
    let input = format!(
        "{AUDIO_KEY_VERSION}\n{AUDIO_OUTPUT_FORMAT}\n{}\nrate=0\n{}",
        voice.azure_name(),
        normalize_text(text)
    );
    sha256_hex(input.as_bytes())
}

/// The SSML for one pre-generated clip: the same shape the TTS proxy builds
/// for a one-segment request at normal speed. Slow replays slow the clip
/// down on the device instead of costing a second clip.
pub fn clip_ssml(voice: Voice, text: &str) -> String {
    let mut escaped = String::new();
    for c in normalize_text(text).chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            c if c.is_control() => {}
            c => escaped.push(c),
        }
    }
    format!(
        r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="{}"><voice name="{}">{escaped}</voice></speak>"#,
        voice.locale(),
        voice.azure_name()
    )
}

/// Billable characters of a clip: what Azure counts for its SSML.
pub fn billable_chars(voice: Voice, text: &str) -> u32 {
    ssml_billable_chars(&clip_ssml(voice, text)) as u32
}

/// Characters Azure bills for an SSML request: everything inside the body
/// except the `<speak>` and `<voice>` tags themselves. Other markup
/// (`<prosody pitch="+10%">`) and XML escapes (`&apos;` is six) count as
/// written. Both TTS spenders (the packs tool and the Cloud proxy) count
/// with this, so their shares of the free quota hold.
pub fn ssml_billable_chars(ssml: &str) -> u64 {
    let mut count = 0;
    let mut rest = ssml;
    while let Some(open) = rest.find('<') {
        count += rest[..open].chars().count();
        let Some(close) = rest[open..].find('>') else {
            count += rest[open..].chars().count();
            return count as u64;
        };
        let tag = &rest[open..=open + close];
        let name = tag[1..].trim_start_matches('/');
        let name =
            &name[..name.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(name.len())];
        if name != "speak" && name != "voice" {
            count += tag.chars().count();
        }
        rest = &rest[open + close + 1..];
    }
    (count + rest.chars().count()) as u64
}

/// The narrator voice of a language: Gadis for Indonesian, Siti for Javanese.
pub fn default_voice(language: &str) -> Voice {
    if language == "jv" { Voice::JvSiti } else { Voice::IdGadis }
}

fn voices(language: &str) -> (Voice, Voice) {
    if language == "jv" { (Voice::JvSiti, Voice::JvDimas) } else { (Voice::IdGadis, Voice::IdArdi) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gender {
    Female,
    Male,
}

const FEMALE_TITLES: &[&str] =
    &["bu", "ibu", "mbak", "mba", "ning", "ni", "bude", "budhe", "tante", "nenek", "mama", "neng", "jeng"];
/// Kak (and Papuan Kaka) is said to an older sibling of either sex, so the
/// name decides; an unknown name after Kak keeps the female voice.
const NEUTRAL_TITLES: &[&str] = &["kak", "kaka"];
const MALE_TITLES: &[&str] =
    &["pak", "bapak", "pakdhe", "pakde", "mas", "cak", "bli", "kang", "om", "kakek", "satpam"];
const FEMALE_NAMES: &[&str] = &[
    "rina", "sari", "darmi", "ani", "siti", "gadis", "dewi", "wulan", "tari", "rara", "sinta", "lia", "yani",
    "lina", "ratna", "rahayu", "nur", "putri", "ayu", "luh", "yohana", "ruth", "ester", "debora", "frida",
    "grace", "selvi", "lince", "yanti", "tiara", "intan", "nirmala", "laras",
];
const MALE_NAMES: &[&str] = &[
    "mohamed", "muhammad", "dimas", "ardi", "bayu", "agus", "bagus", "budi", "joko", "andi", "heru", "harjo",
    "wiryo", "hasan", "yudi", "fikri", "darmo", "anton", "man", "mat", "yosua", "steven", "benny", "elias",
    "yance", "raka", "fadil", "reza", "bimo", "irwan",
];

/// Guesses a speaker's voice from Indonesian/Javanese titles (Bu, Mbak, Pak,
/// Mas, Cak, ...) and a short list of the curriculum's names.
fn gender_of(speaker: &str) -> Option<Gender> {
    let words: Vec<String> = speaker.split_whitespace().map(str::to_lowercase).collect();
    let first = words.first()?;
    let by_name = || {
        words.iter().rev().find_map(|w| {
            if FEMALE_NAMES.contains(&w.as_str()) {
                Some(Gender::Female)
            } else if MALE_NAMES.contains(&w.as_str()) {
                Some(Gender::Male)
            } else {
                None
            }
        })
    };
    if words.len() > 1 && words[0] == "mbah" {
        // Mbah Putri / Mbah Kakung, else the name decides.
        match words[1].as_str() {
            "putri" => return Some(Gender::Female),
            "kakung" => return Some(Gender::Male),
            _ => {}
        }
    } else if NEUTRAL_TITLES.contains(&first.as_str()) {
        return by_name().or((first == "kak").then_some(Gender::Female));
    } else if FEMALE_TITLES.contains(&first.as_str()) {
        return Some(Gender::Female);
    } else if MALE_TITLES.contains(&first.as_str()) {
        return Some(Gender::Male);
    }
    by_name()
}

/// The voice of `speaker` in `dialogue`: female speakers get Gadis/Siti,
/// male speakers Ardi/Dimas; a speaker we can't place (Penjual, Petugas)
/// gets the voice the other speakers don't use, so a dialogue never sounds
/// like one person talking to themself.
pub fn speaker_voice(language: &str, dialogue: &Dialogue, speaker: &str) -> Voice {
    let (female, male) = voices(language);
    let pick = |g| if g == Gender::Female { female } else { male };
    if let Some(g) = gender_of(speaker) {
        return pick(g);
    }
    let others: Vec<Gender> = dialogue
        .lines
        .iter()
        .filter(|l| l.speaker != speaker)
        .filter_map(|l| gender_of(&l.speaker))
        .collect();
    match others.first() {
        Some(Gender::Female) => male,
        Some(Gender::Male) => female,
        None => {
            // Nobody placed: alternate by order of appearance.
            let mut seen = Vec::new();
            for l in &dialogue.lines {
                if !seen.contains(&l.speaker.as_str()) {
                    seen.push(&l.speaker);
                }
            }
            if seen.iter().position(|s| *s == speaker).unwrap_or(0) % 2 == 0 { female } else { male }
        }
    }
}

/// One line that gets a pre-generated clip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioLine {
    pub text: String,
    pub voice: Voice,
}

/// Every target-language line the Lesson Player can play: slide examples,
/// target-language questions, sentences to translate, listening, dictation
/// and speaking texts, dialogue lines (in the speaker's voice) and new words.
/// Order of appearance; duplicates are kept (the manifest dedupes by key).
pub fn lesson_lines(lesson: &LessonContent) -> Vec<AudioLine> {
    let narrator = default_voice(&lesson.language);
    let mut out = Vec::new();
    let mut push = |text: &str, voice: Voice| {
        let text = normalize_text(text);
        if !text.is_empty() {
            out.push(AudioLine { text, voice });
        }
    };
    for slide in &lesson.slides {
        for ex in &slide.examples {
            push(&ex.text, narrator);
        }
    }
    for ex in &lesson.exercises {
        match &ex.kind {
            ExerciseKind::MultipleChoice { question, .. } => push(question, narrator),
            ExerciseKind::Translate { source_text: Some(t), .. } => push(t, narrator),
            ExerciseKind::Listen { text, .. }
            | ExerciseKind::Dictation { text, .. }
            | ExerciseKind::Speaking { text, .. } => push(text, narrator),
            _ => {}
        }
    }
    if let Some(d) = &lesson.dialogue {
        for line in &d.lines {
            push(&line.text, speaker_voice(&lesson.language, d, &line.speaker));
        }
    }
    for w in &lesson.new_words {
        push(&w.word, narrator);
    }
    out
}

/// The unit of a lesson id: `bipa1.u03.l02` → `bipa1.u03`.
pub fn unit_of(lesson_id: &str) -> &str {
    lesson_id.rsplit_once('.').map_or(lesson_id, |(unit, _)| unit)
}

/// One lesson going into a pack: its id, the SHA-256 of its YAML, and the
/// parsed content.
pub struct PackInput<'a> {
    pub id: &'a str,
    pub yaml_sha256: String,
    pub lesson: &'a LessonContent,
}

/// Builds a unit's manifest. Audio entries are deduped by key, sorted by
/// key; the synthesized hash and size of a clip carry over from `previous`
/// when its key is unchanged, and the version goes up only when the content
/// hash changes.
pub fn build_manifest(
    language: &str,
    unit: &str,
    inputs: &[PackInput<'_>],
    previous: Option<&PackManifest>,
) -> PackManifest {
    let mut lessons: Vec<PackLesson> =
        inputs.iter().map(|i| PackLesson { id: i.id.to_owned(), sha256: i.yaml_sha256.clone() }).collect();
    lessons.sort_by(|a, b| a.id.cmp(&b.id));

    let known: BTreeMap<&str, &AudioEntry> =
        previous.map(|p| p.audio.iter().map(|a| (a.key.as_str(), a)).collect()).unwrap_or_default();
    let mut audio: BTreeMap<String, AudioEntry> = BTreeMap::new();
    for input in inputs {
        for line in lesson_lines(input.lesson) {
            let key = audio_key(line.voice, &line.text);
            if audio.contains_key(&key) {
                continue;
            }
            let old = known.get(key.as_str());
            audio.insert(
                key.clone(),
                AudioEntry {
                    key,
                    chars: billable_chars(line.voice, &line.text),
                    text: line.text,
                    voice: line.voice,
                    sha256: old.and_then(|o| o.sha256.clone()),
                    size: old.and_then(|o| o.size),
                    mime: AUDIO_MIME.into(),
                },
            );
        }
    }
    let audio: Vec<AudioEntry> = audio.into_values().collect();

    let mut h = String::new();
    for l in &lessons {
        h.push_str(&format!("lesson {} {}\n", l.id, l.sha256));
    }
    for a in &audio {
        h.push_str(&format!("audio {}\n", a.key));
    }
    let content_hash = sha256_hex(h.as_bytes());
    let version = match previous {
        Some(p) if p.content_hash == content_hash => p.version,
        Some(p) => p.version + 1,
        None => 1,
    };
    let totals = totals(&lessons, &audio);
    PackManifest {
        format: PACK_FORMAT,
        id: pack_id(language, unit),
        language: language.to_owned(),
        unit: unit.to_owned(),
        version,
        content_hash,
        lessons,
        audio,
        totals,
    }
}

fn totals(lessons: &[PackLesson], audio: &[AudioEntry]) -> PackTotals {
    PackTotals {
        lessons: lessons.len() as u32,
        clips: audio.len() as u32,
        chars: audio.iter().map(|a| u64::from(a.chars)).sum(),
        audio_bytes: audio.iter().filter_map(|a| a.size).sum(),
        missing: audio.iter().filter(|a| a.sha256.is_none()).count() as u32,
    }
}

/// Recomputes a manifest's totals after clips were synthesized.
pub fn refresh_totals(m: &mut PackManifest) {
    m.totals = totals(&m.lessons, &m.audio);
}

/// The index row of a manifest. The tar's hash and size come from the last
/// bundle and stay only while the content is unchanged.
pub fn summary(m: &PackManifest, previous: Option<&PackSummary>) -> PackSummary {
    let same = previous.filter(|p| p.content_hash == m.content_hash && p.audio_bytes == m.totals.audio_bytes);
    PackSummary {
        id: m.id.clone(),
        language: m.language.clone(),
        unit: m.unit.clone(),
        stop: previous.and_then(|p| p.stop.clone()),
        version: m.version,
        content_hash: m.content_hash.clone(),
        lessons: m.totals.lessons,
        clips: m.totals.clips,
        chars: m.totals.chars,
        audio_bytes: m.totals.audio_bytes,
        missing: m.totals.missing,
        file: pack_file(&m.unit),
        file_sha256: same.and_then(|p| p.file_sha256.clone()),
        file_size: same.and_then(|p| p.file_size),
    }
}

// --- budget ------------------------------------------------------------------

/// Characters already spent this month and the cap for the month.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    pub used: u64,
    pub limit: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverBudget {
    pub needed: u64,
    pub left: u64,
}

impl Budget {
    pub fn left(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    /// Refuses a run that would go past the monthly cap.
    pub fn check(&self, chars: u64) -> Result<(), OverBudget> {
        if chars <= self.left() { Ok(()) } else { Err(OverBudget { needed: chars, left: self.left() }) }
    }
}

/// The clips of `manifests` (in order) that aren't synthesized or cached
/// yet, deduped across units, with their total characters.
pub fn to_synthesize<'a>(
    manifests: impl IntoIterator<Item = &'a PackManifest>,
    cached: &BTreeSet<String>,
) -> (Vec<&'a AudioEntry>, u64) {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for m in manifests {
        for a in &m.audio {
            if !cached.contains(&a.key) && seen.insert(a.key.as_str()) {
                out.push(a);
            }
        }
    }
    let chars = out.iter().map(|a| u64::from(a.chars)).sum();
    (out, chars)
}

/// `YYYY-MM` (UTC) of a Unix time in seconds: the budget's month.
pub fn month_of(unix_secs: u64) -> String {
    // Civil-from-days (Howard Hinnant), valid for every date after 1970.
    let days = (unix_secs / 86_400) as i64 + 719_468;
    let era = days / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}")
}

// --- devices -------------------------------------------------------------------

/// Packs a device should fetch when `lesson_id` opens: its unit and the
/// next one on the same route, just in time (SPEC §20), unless already
/// installed at the index's version or not downloadable yet.
pub fn prefetch(index: &PackIndex, lesson_id: &str, installed: &BTreeMap<String, u32>) -> Vec<String> {
    let unit = unit_of(lesson_id);
    let Some(at) = index.packs.iter().position(|p| p.unit == unit) else { return Vec::new() };
    let language = &index.packs[at].language;
    let next = index.packs[at + 1..].iter().find(|p| &p.language == language);
    std::iter::once(&index.packs[at])
        .chain(next)
        .filter(|p| p.downloadable() && installed.get(&p.id).is_none_or(|v| *v < p.version))
        .map(|p| p.id.clone())
        .collect()
}

/// Installed packs the index has a newer, downloadable version of.
pub fn outdated(index: &PackIndex, installed: &BTreeMap<String, u32>) -> Vec<String> {
    index
        .packs
        .iter()
        .filter(|p| p.downloadable() && installed.get(&p.id).is_some_and(|v| *v < p.version))
        .map(|p| p.id.clone())
        .collect()
}

/// Clips no installed pack needs any more (after removing a pack): the
/// cached keys minus every key of `keep`.
pub fn orphans<'a>(cached: impl IntoIterator<Item = &'a str>, keep: &[PackManifest]) -> Vec<String> {
    let needed: BTreeSet<&str> = keep.iter().flat_map(|m| m.audio.iter().map(|a| a.key.as_str())).collect();
    cached.into_iter().filter(|k| !needed.contains(k)).map(str::to_owned).collect()
}

/// Checks a downloaded pack's manifest against the index row it was
/// fetched for, before anything is written.
pub fn check_manifest(m: &PackManifest, expected: &PackSummary) -> Result<(), String> {
    if m.format > PACK_FORMAT {
        return Err(format!("pack format {} is newer than this app", m.format));
    }
    if m.id != expected.id || m.content_hash != expected.content_hash || m.version != expected.version {
        return Err("manifest doesn't match the index".into());
    }
    if m.audio.iter().any(|a| a.sha256.is_none()) {
        return Err("pack has clips without hashes".into());
    }
    if m.audio.iter().any(|a| !is_hex_key(&a.key)) {
        return Err("bad clip key".into());
    }
    Ok(())
}

/// A 64-character lowercase hex SHA-256 (safe as a file name).
pub fn is_hex_key(key: &str) -> bool {
    key.len() == 64 && key.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use kabar_protocol::lesson::{DialogueLine, Example, Exercise, Localized, NewWord, Slide, Souvenir};

    use super::*;

    fn loc(s: &str) -> Localized {
        Localized { ar_eg: s.into(), ar: s.into(), en: s.into() }
    }

    fn line(speaker: &str, text: &str) -> DialogueLine {
        DialogueLine { speaker: speaker.into(), text: text.into(), meaning: loc("") }
    }

    fn lesson(id: &str, language: &str) -> LessonContent {
        LessonContent {
            schema: "kabar.lesson/v1".into(),
            id: id.into(),
            language: language.into(),
            title: "Salam".into(),
            can_do: loc("greet"),
            slides: vec![Slide {
                title: Some("Selamat pagi!".into()),
                body: loc("..."),
                examples: vec![
                    Example { text: "Selamat pagi, Bu.".into(), meaning: loc("") },
                    Example { text: "Apa  kabar?".into(), meaning: loc("") },
                ],
                grammar: None,
            }],
            exercises: vec![
                Exercise {
                    id: "e01".into(),
                    kind: ExerciseKind::Listen {
                        text: "Apa kabar?".into(),
                        audio: None,
                        options: vec![],
                        answer: None,
                        meaning: None,
                        question: None,
                    },
                    explain: None,
                    words: vec![],
                },
                Exercise {
                    id: "e02".into(),
                    kind: ExerciseKind::MultipleChoice {
                        question: String::new(),
                        question_meaning: Some(loc("?")),
                        options: vec!["a".into()],
                        options_meaning: vec![],
                        answer: 0,
                    },
                    explain: None,
                    words: vec![],
                },
                Exercise {
                    id: "e03".into(),
                    kind: ExerciseKind::Speaking { text: "Kabar baik.".into(), meaning: loc("") },
                    explain: None,
                    words: vec![],
                },
            ],
            dialogue: Some(Dialogue {
                title: None,
                lines: vec![line("Mohamed", "Selamat pagi, Bu Sari."), line("Bu Sari", "Pagi! Apa kabar?")],
            }),
            new_words: vec![NewWord {
                word: "kabar".into(),
                pos: None,
                meaning: loc("news"),
                arabic: Some("خبر".into()),
                bridge: None,
                false_friend: None,
                same_as_id: false,
            }],
            souvenir: Some(Souvenir { id: "s".into(), title: "t".into(), note: loc("") }),
            review: Default::default(),
            game: None,
        }
    }

    #[test]
    fn keys_ignore_spacing_but_not_voice() {
        assert_eq!(audio_key(Voice::IdGadis, " Apa  kabar?\n"), audio_key(Voice::IdGadis, "Apa kabar?"));
        assert_ne!(audio_key(Voice::IdGadis, "Apa kabar?"), audio_key(Voice::IdArdi, "Apa kabar?"));
        assert_ne!(audio_key(Voice::IdGadis, "Apa kabar?"), audio_key(Voice::IdGadis, "apa kabar?"));
        assert!(is_hex_key(&audio_key(Voice::JvSiti, "Sugeng enjing")));
    }

    #[test]
    fn ssml_matches_the_proxy_shape() {
        assert_eq!(
            clip_ssml(Voice::IdGadis, "Apa kabar?"),
            r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="id-ID"><voice name="id-ID-GadisNeural">Apa kabar?</voice></speak>"#
        );
        assert!(clip_ssml(Voice::JvDimas, "a & <b>").contains("a &amp; &lt;b&gt;"));
        // Azure bills the escapes as written, not the speak/voice tags.
        assert_eq!(billable_chars(Voice::IdGadis, "Apa kabar?"), 10);
        assert_eq!(billable_chars(Voice::IdArdi, "Jum'at"), "Jum&apos;at".len() as u32);
        assert_eq!(
            ssml_billable_chars(r#"<speak a="1"><voice name="x"><break time="1s"/>ok</voice></speak>"#),
            20
        );
    }

    #[test]
    fn speakers_get_fitting_voices() {
        let d = Dialogue {
            title: None,
            lines: vec![line("Mohamed", "x"), line("Bu Siti", "y"), line("Penjual", "z")],
        };
        assert_eq!(speaker_voice("id", &d, "Mohamed"), Voice::IdArdi);
        assert_eq!(speaker_voice("jv", &d, "Bu Siti"), Voice::JvSiti);
        assert_eq!(speaker_voice("id", &d, "Mbak Rara"), Voice::IdGadis);
        assert_eq!(speaker_voice("jv", &d, "Mas Heru"), Voice::JvDimas);
        assert_eq!(speaker_voice("id", &d, "Ni Luh"), Voice::IdGadis);
        assert_eq!(speaker_voice("jv", &d, "Mbah Putri"), Voice::JvSiti);
        assert_eq!(speaker_voice("jv", &d, "Mbah Darmi"), Voice::JvSiti);
        assert_eq!(speaker_voice("id", &d, "Rina"), Voice::IdGadis);
        assert_eq!(speaker_voice("id", &d, "Kak Dewi"), Voice::IdGadis);
        assert_eq!(speaker_voice("id", &d, "Kak Mei"), Voice::IdGadis);
        assert_eq!(speaker_voice("id", &d, "Kak Yosua"), Voice::IdArdi);
        assert_eq!(speaker_voice("id", &d, "Kaka Benny"), Voice::IdArdi);
        assert_eq!(speaker_voice("id", &d, "Kaka Yohana"), Voice::IdGadis);
        assert_eq!(speaker_voice("id", &d, "Mama Ruth"), Voice::IdGadis);
        // Unknown: the voice the others don't use (Mohamed is first and male).
        assert_eq!(speaker_voice("id", &d, "Penjual"), Voice::IdGadis);
        let two = Dialogue { title: None, lines: vec![line("Petugas", "a"), line("Pengumuman", "b")] };
        assert_ne!(speaker_voice("id", &two, "Petugas"), speaker_voice("id", &two, "Pengumuman"));
    }

    #[test]
    fn lesson_lines_cover_what_the_player_plays() {
        let l = lesson("bipa1.u01.l01", "id");
        let lines = lesson_lines(&l);
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Selamat pagi, Bu.",
                "Apa kabar?",
                "Apa kabar?",
                "Kabar baik.",
                "Selamat pagi, Bu Sari.",
                "Pagi! Apa kabar?",
                "kabar"
            ]
        );
        assert_eq!(lines[4].voice, Voice::IdArdi);
        assert_eq!(lines[5].voice, Voice::IdGadis);
        assert_eq!(unit_of("bipa1.u01.l01"), "bipa1.u01");
    }

    #[test]
    fn manifest_dedupes_and_versions() {
        let a = lesson("bipa1.u01.l01", "id");
        let b = lesson("bipa1.u01.l02", "id");
        let inputs = [
            PackInput { id: "bipa1.u01.l02", yaml_sha256: "b".into(), lesson: &b },
            PackInput { id: "bipa1.u01.l01", yaml_sha256: "a".into(), lesson: &a },
        ];
        let m = build_manifest("id", "bipa1.u01", &inputs, None);
        assert_eq!(m.id, "id/bipa1.u01");
        assert_eq!(m.version, 1);
        assert_eq!(m.lessons[0].id, "bipa1.u01.l01");
        // Same lines in both lessons: one clip each.
        assert_eq!(m.totals.clips, 6);
        assert_eq!(m.totals.missing, 6);
        assert_eq!(m.totals.chars, m.audio.iter().map(|a| u64::from(a.chars)).sum::<u64>());

        // Rebuild unchanged: same version; synthesized hashes carry over.
        let mut synthesized = m.clone();
        synthesized.audio[0].sha256 = Some("f".repeat(64));
        synthesized.audio[0].size = Some(1234);
        refresh_totals(&mut synthesized);
        let again = build_manifest("id", "bipa1.u01", &inputs, Some(&synthesized));
        assert_eq!(again.version, 1);
        assert_eq!(again.audio[0].size, Some(1234));
        assert_eq!(again.totals.audio_bytes, 1234);
        assert_eq!(again.totals.missing, 5);

        // A changed lesson bumps the version.
        let inputs2 = [PackInput { id: "bipa1.u01.l01", yaml_sha256: "a2".into(), lesson: &a }];
        let changed = build_manifest("id", "bipa1.u01", &inputs2, Some(&again));
        assert_eq!(changed.version, 2);
        assert_ne!(changed.content_hash, again.content_hash);
    }

    #[test]
    fn budget_refuses_overspending() {
        let b = Budget { used: 270_000, limit: DEFAULT_MONTHLY_BUDGET };
        assert_eq!(b.left(), 10_000);
        assert!(b.check(10_000).is_ok());
        assert_eq!(b.check(10_001), Err(OverBudget { needed: 10_001, left: 10_000 }));
        assert_eq!(Budget { used: 600_000, limit: FREE_MONTHLY_CHARS }.left(), 0);
    }

    #[test]
    fn synthesis_skips_cached_and_shared_clips() {
        let a = lesson("bipa1.u01.l01", "id");
        let m1 = build_manifest(
            "id",
            "bipa1.u01",
            &[PackInput { id: "bipa1.u01.l01", yaml_sha256: "a".into(), lesson: &a }],
            None,
        );
        let m2 = build_manifest(
            "id",
            "bipa1.u02",
            &[PackInput { id: "bipa1.u02.l01", yaml_sha256: "c".into(), lesson: &a }],
            None,
        );
        let cached: BTreeSet<String> = [m1.audio[0].key.clone()].into();
        let (todo, chars) = to_synthesize([&m1, &m2], &cached);
        assert_eq!(todo.len(), m1.audio.len() - 1);
        assert_eq!(chars, m1.totals.chars - u64::from(m1.audio[0].chars));
    }

    #[test]
    fn months_are_utc_calendar_months() {
        assert_eq!(month_of(0), "1970-01");
        assert_eq!(month_of(1_793_000_000), "2026-10");
        assert_eq!(month_of(951_782_400), "2000-02"); // 2000-02-29, a leap day
    }

    fn summary_row(unit: &str, language: &str, version: u32, ready: bool) -> PackSummary {
        PackSummary {
            id: pack_id(language, unit),
            language: language.into(),
            unit: unit.into(),
            stop: None,
            version,
            content_hash: "h".into(),
            lessons: 4,
            clips: 10,
            chars: 100,
            audio_bytes: 1000,
            missing: 0,
            file: pack_file(unit),
            file_sha256: ready.then(|| "s".repeat(64)),
            file_size: ready.then_some(2000),
        }
    }

    #[test]
    fn prefetch_current_and_next_unit() {
        let index = PackIndex {
            format: PACK_FORMAT,
            packs: vec![
                summary_row("bipa1.u01", "id", 1, true),
                summary_row("bipa1.u02", "id", 2, true),
                summary_row("j1.u01", "jv", 1, true),
                summary_row("bipa2.u01", "id", 1, false),
            ],
        };
        let none = BTreeMap::new();
        assert_eq!(prefetch(&index, "bipa1.u01.l03", &none), ["id/bipa1.u01", "id/bipa1.u02"]);
        // The next unit skips the other language; not-yet-bundled packs wait.
        assert_eq!(prefetch(&index, "bipa1.u02.l01", &none), ["id/bipa1.u02"]);
        let installed: BTreeMap<String, u32> =
            [("id/bipa1.u01".to_owned(), 1), ("id/bipa1.u02".to_owned(), 1)].into();
        // Installed u02 is outdated (v1 < v2).
        assert_eq!(prefetch(&index, "bipa1.u01.l01", &installed), ["id/bipa1.u02"]);
        assert!(prefetch(&index, "j9.u01.l01", &none).is_empty());
        // Only installed packs with a newer version are refreshed.
        assert_eq!(outdated(&index, &installed), ["id/bipa1.u02"]);
        assert!(outdated(&index, &none).is_empty());
    }

    #[test]
    fn removing_a_pack_keeps_shared_clips() {
        let a = lesson("bipa1.u01.l01", "id");
        let m = build_manifest(
            "id",
            "bipa1.u01",
            &[PackInput { id: "bipa1.u01.l01", yaml_sha256: "a".into(), lesson: &a }],
            None,
        );
        let shared = m.audio[0].key.clone();
        let cached = [shared.as_str(), "old"];
        assert_eq!(orphans(cached, std::slice::from_ref(&m)), ["old"]);
        assert_eq!(orphans(cached, &[]).len(), 2);
    }

    #[test]
    fn downloaded_manifests_are_checked() {
        let a = lesson("bipa1.u01.l01", "id");
        let mut m = build_manifest(
            "id",
            "bipa1.u01",
            &[PackInput { id: "bipa1.u01.l01", yaml_sha256: "a".into(), lesson: &a }],
            None,
        );
        let row = summary(&m, None);
        assert!(check_manifest(&m, &row).unwrap_err().contains("without hashes"));
        for e in &mut m.audio {
            e.sha256 = Some("0".repeat(64));
            e.size = Some(10);
        }
        refresh_totals(&mut m);
        let row = summary(&m, None);
        assert_eq!(check_manifest(&m, &row), Ok(()));
        let mut newer = m.clone();
        newer.format = PACK_FORMAT + 1;
        assert!(check_manifest(&newer, &row).is_err());
        let mut other = row.clone();
        other.version = 9;
        assert!(check_manifest(&m, &other).is_err());
    }
}
