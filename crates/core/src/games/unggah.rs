//! Unggah-Ungguh Simulator (spec §13): short Javanese scenes (a friend, a
//! market seller, a grandmother, a teacher, an elder neighbour) where each
//! turn offers the same thing said at different speech levels. The Respect
//! Meter rises when the choice fits the person and dips when it doesn't,
//! with a kind explanation every time.
//!
//! Scenes are build-time content in `content/games/unggah-ungguh.toml`. Each
//! choice names the speech-level forms it uses (`"eat:krama_inggil"`), and
//! parsing checks them against `content/javanese/speech-levels.jsonl`, the
//! reviewed table, so the game never teaches a form the table doesn't have.
//! Everything stays unverified until the learner approves it.

use std::collections::{HashMap, HashSet};

use kabar_protocol::content::{SpeechLevel, SpeechLevelEntry, Trilingual};
use serde::Deserialize;

use super::Rng;

/// Where the meter starts: neutral, a little above the middle.
pub const METER_START: u8 = 60;
/// Meter at or above this at the end of a scene earns "Sopan!".
pub const METER_GREAT: u8 = 80;

/// How a choice lands with this person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    /// The right level for this person.
    Fits,
    /// Too plain for this person (ngoko to a grandmother).
    TooPlain,
    /// Stiffer than the moment needs (krama inggil to a close friend): odd, not rude.
    TooStiff,
    /// An honorific used about yourself ("kula dhahar"): the classic slip.
    SelfRaised,
}

impl Fit {
    /// The meter change for this fit.
    pub fn delta(self) -> i8 {
        match self {
            Self::Fits => 15,
            // Odd, not rude: a small dip.
            Self::TooStiff => -5,
            Self::TooPlain => -20,
            Self::SelfRaised => -25,
        }
    }
}

/// The 0..=100 Respect Meter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RespectMeter(pub u8);

impl Default for RespectMeter {
    fn default() -> Self {
        Self(METER_START)
    }
}

impl RespectMeter {
    /// Applies a fit, clamped to 0..=100; returns the change actually made.
    pub fn apply(&mut self, fit: Fit) -> i8 {
        let before = i16::from(self.0.min(100));
        let after = (before + i16::from(fit.delta())).clamp(0, 100);
        self.0 = after as u8;
        (after - before) as i8
    }

    /// Whether the meter is in the "Sopan!" band.
    pub fn is_great(self) -> bool {
        self.0 >= METER_GREAT
    }
}

/// A line in the target language with its meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said {
    pub text: String,
    pub meaning: Trilingual,
}

/// One way to answer at one turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub said: Said,
    /// The overall level of the sentence.
    pub level: SpeechLevel,
    pub fit: Fit,
    /// Speech-level forms used, as `row_id:level` (checked against the table).
    pub uses: Vec<String>,
    /// Why it lands the way it does (always shown after choosing).
    pub why: Trilingual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub id: String,
    /// What is happening, in the learner's language.
    pub situation: Trilingual,
    /// What the other person says first, if anything.
    pub partner_says: Option<Said>,
    pub choices: Vec<Choice>,
}

impl Turn {
    /// The choice that fits this person (parsing guarantees exactly one).
    pub fn fitting(&self) -> Option<&Choice> {
        self.choices.iter().find(|c| c.fit == Fit::Fits)
    }
}

/// Who the learner is talking to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Partner {
    /// Javanese name or form of address (Mbah Sri, Pak Guru).
    pub name: String,
    pub role: Trilingual,
    /// The level they are owed: ngoko for a friend, krama for a seller or a
    /// stranger, krama (with krama inggil for their actions) for an elder.
    pub owed: SpeechLevel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    /// `uu.friend`, `uu.market`, `uu.grandmother`, `uu.teacher`, `uu.elder`, …
    pub id: String,
    /// Javanese track level it unlocks at: `j1`, `j2` or `j3`.
    pub level: String,
    pub title: Trilingual,
    pub setting: Trilingual,
    pub partner: Partner,
    pub turns: Vec<Turn>,
}

/// Parsed scenes plus the speech-level table they reference.
#[derive(Clone, Debug, Default)]
pub struct UnggahData {
    pub scenarios: Vec<Scenario>,
    /// The table, by row id.
    pub table: HashMap<String, SpeechLevelEntry>,
    /// Whether the learner has approved the scenes. Until then they show as unverified.
    pub reviewed: bool,
    pub version: u32,
}

// --- File format (see content/games/README.md) --------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileToml {
    reviewed: bool,
    version: u32,
    #[serde(default)]
    scenes: Vec<SceneToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneToml {
    id: String,
    level: String,
    title: Trilingual,
    setting: Trilingual,
    partner: PartnerToml,
    turns: Vec<TurnToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PartnerToml {
    name: String,
    role: Trilingual,
    owed: SpeechLevel,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TurnToml {
    id: String,
    situation: Trilingual,
    #[serde(default)]
    partner_says: Option<LineToml>,
    choices: Vec<ChoiceToml>,
}

/// What the partner says. Its `uses` are checked like a choice's.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LineToml {
    text: String,
    uses: Vec<String>,
    meaning: Trilingual,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChoiceToml {
    text: String,
    level: SpeechLevel,
    fit: Fit,
    uses: Vec<String>,
    meaning: Trilingual,
    why: Trilingual,
}

/// The rank of a Javanese track level id: `j2` → 2. `None` for anything else.
pub fn level_rank(level: &str) -> Option<u8> {
    let n = level.trim().strip_prefix(['j', 'J'])?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    n.parse().ok()
}

/// Lower-case words, punctuation dropped; hyphens inside a word are kept
/// (`ati-ati`, `mugi-mugi`).
pub fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .map(|w| w.trim_matches('-'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether `form` (one or more words) appears in `text` as whole words,
/// ignoring case and punctuation.
pub fn contains_form(text: &str, form: &str) -> bool {
    let (hay, needle) = (words(text), words(form));
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle.as_slice())
}

/// The forms a row lists at exactly this level (no fallback).
pub fn forms(entry: &SpeechLevelEntry, level: SpeechLevel) -> &[String] {
    match level {
        SpeechLevel::Ngoko => &entry.ngoko,
        SpeechLevel::Madya => &entry.madya,
        SpeechLevel::Krama => &entry.krama,
        SpeechLevel::KramaInggil => &entry.krama_inggil,
        SpeechLevel::KramaAndhap => &entry.krama_andhap,
    }
}

fn parse_level(s: &str) -> Option<SpeechLevel> {
    Some(match s {
        "ngoko" => SpeechLevel::Ngoko,
        "madya" => SpeechLevel::Madya,
        "krama" => SpeechLevel::Krama,
        "krama_inggil" => SpeechLevel::KramaInggil,
        "krama_andhap" => SpeechLevel::KramaAndhap,
        _ => return None,
    })
}

fn check_trilingual(what: &str, t: &Trilingual) -> Result<(), String> {
    for (lang, s) in [("arz", &t.arz), ("ar", &t.ar), ("en", &t.en)] {
        if s.trim().is_empty() {
            return Err(format!("{what}: empty {lang}"));
        }
    }
    Ok(())
}

/// Checks `uses` against the table and the text they claim to be in.
fn check_uses(
    what: &str,
    text: &str,
    uses: &[String],
    table: &HashMap<String, SpeechLevelEntry>,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err(format!("{what}: empty text"));
    }
    if uses.is_empty() {
        return Err(format!("{what}: {text:?} lists no speech-level forms in `uses`"));
    }
    for u in uses {
        let (row, level) =
            u.split_once(':').ok_or_else(|| format!("{what}: `uses` entry {u:?} is not row_id:level"))?;
        let entry =
            table.get(row).ok_or_else(|| format!("{what}: {u:?}: no row {row:?} in speech-levels.jsonl"))?;
        let level = parse_level(level).ok_or_else(|| format!("{what}: {u:?}: unknown level {level:?}"))?;
        let at = forms(entry, level);
        if at.is_empty() {
            return Err(format!("{what}: {u:?}: row {row:?} has no form at that level"));
        }
        if !at.iter().any(|f| contains_form(text, f)) {
            return Err(format!("{what}: {u:?}: none of {at:?} appears in {text:?}"));
        }
    }
    Ok(())
}

fn parse_table(levels_jsonl: &str) -> Result<HashMap<String, SpeechLevelEntry>, String> {
    let mut table = HashMap::new();
    for (i, line) in levels_jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: SpeechLevelEntry =
            serde_json::from_str(line).map_err(|e| format!("speech-levels.jsonl line {}: {e}", i + 1))?;
        let id = entry.id.clone();
        if table.insert(id.clone(), entry).is_some() {
            return Err(format!("speech-levels.jsonl line {}: duplicate row {id}", i + 1));
        }
    }
    if table.is_empty() {
        return Err("speech-levels.jsonl: no rows".into());
    }
    Ok(table)
}

impl UnggahData {
    /// Parses the scenes and the table; errors name the scene, turn and form.
    /// Rejects: duplicate ids, a turn without exactly one `fits` choice, a
    /// `uses` entry whose row or level isn't in the table, or whose form
    /// doesn't appear in the choice's text.
    pub fn parse(scenarios_toml: &str, levels_jsonl: &str) -> Result<Self, String> {
        let table = parse_table(levels_jsonl)?;
        let file: FileToml =
            toml::from_str(scenarios_toml).map_err(|e| format!("unggah-ungguh.toml: {e}"))?;

        let mut scenarios: Vec<Scenario> = Vec::with_capacity(file.scenes.len());
        for sc in file.scenes {
            let sid = sc.id.clone();
            if sid.trim().is_empty() {
                return Err("unggah-ungguh.toml: a scene has an empty id".into());
            }
            if scenarios.iter().any(|s| s.id == sid) {
                return Err(format!("unggah-ungguh.toml: duplicate scene {sid}"));
            }
            if level_rank(&sc.level).is_none() {
                return Err(format!("{sid}: level {:?} is not a Javanese level like j1", sc.level));
            }
            if sc.partner.name.trim().is_empty() {
                return Err(format!("{sid}: the partner has no name"));
            }
            check_trilingual(&format!("{sid} title"), &sc.title)?;
            check_trilingual(&format!("{sid} setting"), &sc.setting)?;
            check_trilingual(&format!("{sid} partner role"), &sc.partner.role)?;
            if sc.turns.is_empty() {
                return Err(format!("{sid}: no turns"));
            }

            let mut turn_ids = HashSet::new();
            let mut turns = Vec::with_capacity(sc.turns.len());
            for t in sc.turns {
                let tid = format!("{sid}/{}", t.id);
                if t.id.trim().is_empty() {
                    return Err(format!("{sid}: a turn has an empty id"));
                }
                if !turn_ids.insert(t.id.clone()) {
                    return Err(format!("{sid}: duplicate turn {}", t.id));
                }
                check_trilingual(&format!("{tid} situation"), &t.situation)?;
                let partner_says = match t.partner_says {
                    Some(l) => {
                        check_uses(&format!("{tid} partner"), &l.text, &l.uses, &table)?;
                        check_trilingual(&format!("{tid} partner meaning"), &l.meaning)?;
                        Some(Said { text: l.text, meaning: l.meaning })
                    }
                    None => None,
                };
                if t.choices.len() < 2 {
                    return Err(format!("{tid}: needs at least two choices"));
                }
                let fitting = t.choices.iter().filter(|c| c.fit == Fit::Fits).count();
                if fitting != 1 {
                    return Err(format!("{tid}: needs exactly one `fits` choice, has {fitting}"));
                }
                let mut choices = Vec::with_capacity(t.choices.len());
                for (i, c) in t.choices.into_iter().enumerate() {
                    let what = format!("{tid} choice {}", i + 1);
                    if choices.iter().any(|p: &Choice| words(&p.said.text) == words(&c.text)) {
                        return Err(format!("{what}: same text as another choice: {:?}", c.text));
                    }
                    check_uses(&what, &c.text, &c.uses, &table)?;
                    check_trilingual(&format!("{what} meaning"), &c.meaning)?;
                    check_trilingual(&format!("{what} why"), &c.why)?;
                    choices.push(Choice {
                        said: Said { text: c.text, meaning: c.meaning },
                        level: c.level,
                        fit: c.fit,
                        uses: c.uses,
                        why: c.why,
                    });
                }
                turns.push(Turn { id: t.id, situation: t.situation, partner_says, choices });
            }
            scenarios.push(Scenario {
                id: sid,
                level: sc.level.trim().to_lowercase(),
                title: sc.title,
                setting: sc.setting,
                partner: Partner { name: sc.partner.name, role: sc.partner.role, owed: sc.partner.owed },
                turns,
            });
        }
        Ok(Self { scenarios, table, reviewed: file.reviewed, version: file.version })
    }

    pub fn scenario(&self, id: &str) -> Option<&Scenario> {
        self.scenarios.iter().find(|s| s.id == id)
    }

    /// Scenes open at a Javanese level (`j2` opens j1 and j2 scenes); all
    /// scenes when the learner has no Javanese level yet but opens the game.
    pub fn open_at(&self, level: Option<&str>) -> Vec<&Scenario> {
        match level.and_then(level_rank) {
            None => self.scenarios.iter().collect(),
            Some(max) => {
                self.scenarios.iter().filter(|s| level_rank(&s.level).is_some_and(|r| r <= max)).collect()
            }
        }
    }
}

/// One play-through of a scene. Choice order is shuffled per turn by the seed.
#[derive(Clone, Debug)]
pub struct Run {
    scenario: Scenario,
    /// Per turn, the order choices are shown in (indices into `turn.choices`).
    order: Vec<Vec<usize>>,
    turn: usize,
    meter: RespectMeter,
    /// Per answered turn, the choice index picked (into `turn.choices`).
    picks: Vec<usize>,
}

/// What happened after a pick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answered {
    pub fit: Fit,
    pub delta: i8,
    pub meter: RespectMeter,
    /// The choice that fits, shown when the pick didn't.
    pub better: Option<Choice>,
    pub why: Trilingual,
}

/// The end of a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunResult {
    pub meter: RespectMeter,
    pub fitting: u32,
    pub turns: u32,
}

impl Run {
    pub fn new(scenario: &Scenario, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let order = scenario
            .turns
            .iter()
            .map(|t| {
                let mut o: Vec<usize> = (0..t.choices.len()).collect();
                rng.shuffle(&mut o);
                o
            })
            .collect();
        Self { scenario: scenario.clone(), order, turn: 0, meter: RespectMeter::default(), picks: Vec::new() }
    }

    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    pub fn meter(&self) -> RespectMeter {
        self.meter
    }

    /// Zero-based index of the current turn (equals the number of turns when over).
    pub fn turn_index(&self) -> usize {
        self.turn
    }

    /// The current turn, `None` when the scene is over.
    pub fn current(&self) -> Option<&Turn> {
        self.scenario.turns.get(self.turn)
    }

    /// The current turn's choices in display order.
    pub fn shown(&self) -> Vec<&Choice> {
        match (self.current(), self.order.get(self.turn)) {
            (Some(t), Some(order)) => order.iter().filter_map(|&i| t.choices.get(i)).collect(),
            _ => Vec::new(),
        }
    }

    /// Picks the `shown()[i]` choice and moves to the next turn. `None` when
    /// the scene is over or `i` is out of range.
    pub fn pick(&mut self, shown_index: usize) -> Option<Answered> {
        let turn = self.scenario.turns.get(self.turn)?;
        let index = *self.order.get(self.turn)?.get(shown_index)?;
        let choice = turn.choices.get(index)?;
        let fit = choice.fit;
        let better = if fit == Fit::Fits { None } else { turn.fitting().cloned() };
        let why = choice.why.clone();
        let delta = self.meter.apply(fit);
        self.picks.push(index);
        self.turn += 1;
        Some(Answered { fit, delta, meter: self.meter, better, why })
    }

    pub fn result(&self) -> Option<RunResult> {
        if self.turn < self.scenario.turns.len() {
            return None;
        }
        let fitting = self
            .picks
            .iter()
            .zip(&self.scenario.turns)
            .filter(|&(&i, t)| t.choices.get(i).is_some_and(|c| c.fit == Fit::Fits))
            .count();
        Some(RunResult {
            meter: self.meter,
            fitting: fitting as u32,
            turns: self.scenario.turns.len() as u32,
        })
    }
}

/// The form of a table row at a level, falling back as the table says
/// (krama inggil → krama → madya → ngoko; krama andhap → krama → …).
pub fn form_at(entry: &SpeechLevelEntry, level: SpeechLevel) -> Option<&str> {
    use SpeechLevel::*;
    let chain: &[SpeechLevel] = match level {
        Ngoko => &[Ngoko],
        Madya => &[Madya, Ngoko],
        Krama => &[Krama, Madya, Ngoko],
        KramaInggil => &[KramaInggil, Krama, Madya, Ngoko],
        KramaAndhap => &[KramaAndhap, Krama, Madya, Ngoko],
    };
    chain.iter().find_map(|&l| forms(entry, l).first()).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCENES: &str = include_str!("../../../../content/games/unggah-ungguh.toml");
    const LEVELS: &str = include_str!("../../../../content/javanese/speech-levels.jsonl");

    fn data() -> UnggahData {
        UnggahData::parse(SCENES, LEVELS).expect("content/games/unggah-ungguh.toml parses")
    }

    /// A tiny hand-built scene over the real table.
    const MINI: &str = r#"
reviewed = false
version = 1

[[scenes]]
id = "uu.test"
level = "j3"
title = { arz = "تيتا", ar = "الجدة", en = "Grandmother" }
setting = { arz = "البيت", ar = "البيت", en = "At home" }
partner = { name = "Mbah Sri", owed = "krama", role = { arz = "تيتا", ar = "الجدة", en = "grandmother" } }

[[scenes.turns]]
id = "eat"
situation = { arz = "الضهر", ar = "الظهر", en = "Noon" }
partner_says = { text = "Kowe wis mangan?", uses = ["you_singular:ngoko", "already:ngoko", "eat:ngoko"], meaning = { arz = "كلت؟", ar = "هل أكلت؟", en = "Have you eaten?" } }

[[scenes.turns.choices]]
text = "Sampun, Mbah. Kula sampun nedha."
level = "krama"
fit = "fits"
uses = ["already:krama", "i_me:krama", "eat:krama"]
meaning = { arz = "أيوه", ar = "نعم", en = "Yes, I have." }
why = { arz = "تمام", ar = "صحيح", en = "Right." }

[[scenes.turns.choices]]
text = "Sampun, Mbah. Kula sampun dhahar."
level = "krama_inggil"
fit = "self_raised"
uses = ["already:krama", "i_me:krama", "eat:krama_inggil"]
meaning = { arz = "أيوه", ar = "نعم", en = "Yes, I have." }
why = { arz = "لنفسك لا", ar = "ليس لنفسك", en = "Not for yourself." }

[[scenes.turns.choices]]
text = "Wis. Aku wis mangan."
level = "ngoko"
fit = "too_plain"
uses = ["already:ngoko", "i_me:ngoko", "eat:ngoko"]
meaning = { arz = "أيوه", ar = "نعم", en = "Yes, I have." }
why = { arz = "سادة", ar = "عادية", en = "Plain." }

[[scenes.turns]]
id = "leave"
situation = { arz = "مروّح", ar = "تغادر", en = "Leaving" }

[[scenes.turns.choices]]
text = "Kula nyuwun pamit."
level = "krama_andhap"
fit = "fits"
uses = ["i_me:krama", "take_leave:krama_andhap"]
meaning = { arz = "أستأذن", ar = "أستأذن", en = "May I take my leave." }
why = { arz = "تمام", ar = "صحيح", en = "Right." }

[[scenes.turns.choices]]
text = "Aku pamit."
level = "ngoko"
fit = "too_plain"
uses = ["i_me:ngoko", "take_leave:ngoko"]
meaning = { arz = "ماشي", ar = "سأذهب", en = "I'm off." }
why = { arz = "سادة", ar = "عادية", en = "Plain." }

[[scenes]]
id = "uu.pal"
level = "j1"
title = { arz = "صاحب", ar = "صديق", en = "Friend" }
setting = { arz = "الشارع", ar = "الشارع", en = "Street" }
partner = { name = "Bayu", owed = "ngoko", role = { arz = "صاحب", ar = "صديق", en = "friend" } }

[[scenes.turns]]
id = "hi"
situation = { arz = "سلام", ar = "تحية", en = "Hello" }

[[scenes.turns.choices]]
text = "Piye kabare?"
level = "ngoko"
fit = "fits"
uses = ["how_are_you:ngoko"]
meaning = { arz = "إزيك؟", ar = "كيف حالك؟", en = "How are you?" }
why = { arz = "تمام", ar = "صحيح", en = "Right." }

[[scenes.turns.choices]]
text = "Pripun kabaripun?"
level = "krama"
fit = "too_stiff"
uses = ["how_are_you:krama"]
meaning = { arz = "إزي حضرتك؟", ar = "كيف حالك؟", en = "How are you? (polite)" }
why = { arz = "رسمي", ar = "رسمي", en = "Stiff." }
"#;

    fn mini() -> UnggahData {
        UnggahData::parse(MINI, LEVELS).expect("mini scene parses")
    }

    fn err(toml: &str) -> String {
        UnggahData::parse(toml, LEVELS).expect_err("should be rejected")
    }

    #[test]
    fn deltas_and_meter_clamp() {
        assert!(Fit::Fits.delta() > 0);
        assert!(Fit::TooStiff.delta() < 0 && Fit::TooStiff.delta() > Fit::TooPlain.delta());
        assert!(Fit::SelfRaised.delta() < Fit::TooPlain.delta());
        let mut m = RespectMeter::default();
        assert_eq!(m.0, METER_START);
        assert_eq!(m.apply(Fit::Fits), 15);
        assert_eq!(m.apply(Fit::Fits), 15);
        assert_eq!(m.apply(Fit::Fits), 10, "clamped at the top");
        assert_eq!(m, RespectMeter(100));
        assert_eq!(m.apply(Fit::Fits), 0);
        assert!(m.is_great());
        let mut low = RespectMeter(10);
        assert_eq!(low.apply(Fit::SelfRaised), -10, "clamped at the bottom");
        assert_eq!(low, RespectMeter(0));
        assert_eq!(low.apply(Fit::TooPlain), 0);
        assert_eq!(low.apply(Fit::TooStiff), 0);
        assert!(!low.is_great());
        let mut odd = RespectMeter(250);
        assert_eq!(odd.apply(Fit::TooStiff), -5, "an out-of-range meter is clamped first");
        assert_eq!(odd, RespectMeter(95));
    }

    #[test]
    fn words_and_forms() {
        assert_eq!(words("Sampun, Mbah. Kula sampun nedha!"), ["sampun", "mbah", "kula", "sampun", "nedha"]);
        assert_eq!(words("Iya, ati-ati."), ["iya", "ati-ati"]);
        assert!(contains_form("Kula nyuwun pamit, Mbah.", "nyuwun pamit"));
        assert!(contains_form("KOWE wis mangan?", "kowe"));
        assert!(!contains_form("Kowe wis mangan?", "kowe mangan"));
        assert!(!contains_form("Panjenengan sampun dhahar?", "dhahar sampun"));
        assert!(!contains_form("dhaharipun", "dhahar"), "whole words only");
        assert!(!contains_form("ati-ati", "ati"));
        assert!(!contains_form("anything", ""));
    }

    #[test]
    fn levels_rank_and_open() {
        assert_eq!(level_rank("j1"), Some(1));
        assert_eq!(level_rank("J3"), Some(3));
        assert_eq!(level_rank("bipa-2"), None);
        assert_eq!(level_rank("j"), None);
        let d = mini();
        let ids = |v: Vec<&Scenario>| v.into_iter().map(|s| s.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(d.open_at(None)), ["uu.test", "uu.pal"]);
        assert_eq!(ids(d.open_at(Some("j1"))), ["uu.pal"]);
        assert_eq!(ids(d.open_at(Some("j2"))), ["uu.pal"]);
        assert_eq!(ids(d.open_at(Some("j3"))), ["uu.test", "uu.pal"]);
        assert_eq!(ids(d.open_at(Some("bipa-2"))), ["uu.test", "uu.pal"], "no Javanese level: all open");
        assert!(d.scenario("uu.pal").is_some());
        assert!(d.scenario("nope").is_none());
    }

    #[test]
    fn form_at_falls_back() {
        let d = data();
        let row = |id: &str| d.table.get(id).unwrap_or_else(|| panic!("row {id}"));
        let eat = row("eat");
        assert_eq!(form_at(eat, SpeechLevel::Ngoko), Some("mangan"));
        assert_eq!(form_at(eat, SpeechLevel::Madya), Some("mangan"));
        assert_eq!(form_at(eat, SpeechLevel::Krama), Some("nedha"));
        assert_eq!(form_at(eat, SpeechLevel::KramaInggil), Some("dhahar"));
        assert_eq!(form_at(eat, SpeechLevel::KramaAndhap), Some("nedha"));
        let you = row("you_singular");
        assert_eq!(form_at(you, SpeechLevel::Madya), Some("samang"));
        assert_eq!(form_at(you, SpeechLevel::KramaAndhap), Some("sampeyan"));
        let i = row("i_me");
        assert_eq!(form_at(i, SpeechLevel::KramaAndhap), Some("dalem"));
        assert_eq!(form_at(i, SpeechLevel::KramaInggil), Some("kula"));
        let bathe = row("bathe");
        assert_eq!(form_at(bathe, SpeechLevel::Krama), Some("adus"), "krama → madya → ngoko");
        assert_eq!(form_at(bathe, SpeechLevel::KramaInggil), Some("siram"));
    }

    #[test]
    fn a_full_run() {
        let d = mini();
        let sc = d.scenario("uu.test").unwrap();
        let mut run = Run::new(sc, 7);
        assert_eq!(run.scenario().id, "uu.test");
        assert_eq!(run.meter(), RespectMeter(METER_START));
        assert!(run.result().is_none());
        assert_eq!(run.current().unwrap().id, "eat");
        let shown = run.shown();
        assert_eq!(shown.len(), 3);
        // The slip: honorific about yourself.
        let slip = shown.iter().position(|c| c.fit == Fit::SelfRaised).unwrap();
        assert!(run.pick(99).is_none(), "out of range");
        let a = run.pick(slip).unwrap();
        assert_eq!(a.fit, Fit::SelfRaised);
        assert_eq!(a.delta, Fit::SelfRaised.delta());
        assert_eq!(a.meter, RespectMeter(35));
        assert_eq!(a.better.as_ref().unwrap().said.text, "Sampun, Mbah. Kula sampun nedha.");
        assert_eq!(a.why.en, "Not for yourself.");
        assert_eq!(run.turn_index(), 1);
        // The fitting one: no "better".
        let fit = run.shown().iter().position(|c| c.fit == Fit::Fits).unwrap();
        let b = run.pick(fit).unwrap();
        assert_eq!((b.fit, b.delta, b.better), (Fit::Fits, 15, None));
        assert!(run.current().is_none());
        assert!(run.shown().is_empty());
        assert!(run.pick(0).is_none(), "over");
        let r = run.result().unwrap();
        assert_eq!(r, RunResult { meter: RespectMeter(50), fitting: 1, turns: 2 });
    }

    #[test]
    fn same_seed_same_order() {
        let d = data();
        for sc in &d.scenarios {
            let texts = |seed: u64| {
                let mut run = Run::new(sc, seed);
                let mut all = Vec::new();
                while run.current().is_some() {
                    all.push(run.shown().iter().map(|c| c.said.text.clone()).collect::<Vec<_>>());
                    run.pick(0).unwrap();
                }
                all
            };
            assert_eq!(texts(42), texts(42), "{}", sc.id);
        }
        // Over many seeds the fitting choice lands in every position.
        let sc = d.scenario("uu.grandmother").unwrap();
        let mut seen = HashSet::new();
        for seed in 0..64 {
            let run = Run::new(sc, seed);
            seen.insert(run.shown().iter().position(|c| c.fit == Fit::Fits).unwrap());
        }
        assert_eq!(seen.len(), 3, "the fitting choice isn't always in the same place");
    }

    #[test]
    fn parse_rejects_bad_content() {
        assert!(err("nope = 1").contains("unggah-ungguh.toml"));
        assert!(UnggahData::parse(MINI, "").is_err(), "empty table");
        assert!(UnggahData::parse(MINI, "{not json}").unwrap_err().contains("line 1"));
        let dup_row = format!("{}\n{}", LEVELS.lines().next().unwrap(), LEVELS.lines().next().unwrap());
        assert!(
            UnggahData::parse("reviewed = false\nversion = 1\n", &dup_row).unwrap_err().contains("duplicate")
        );
        let swap = |from: &str, to: &str| {
            assert!(MINI.contains(from), "{from}");
            MINI.replacen(from, to, 1)
        };
        // Unknown fields.
        assert!(err(&swap("level = \"j3\"", "level = \"j3\"\nmood = \"happy\"")).contains("mood"));
        // Duplicate scene and turn ids.
        assert!(err(&swap("id = \"uu.pal\"", "id = \"uu.test\"")).contains("duplicate scene uu.test"));
        assert!(err(&swap("id = \"leave\"", "id = \"eat\"")).contains("duplicate turn eat"));
        // Exactly one fits.
        assert!(err(&swap("fit = \"self_raised\"", "fit = \"fits\"")).contains("exactly one"));
        assert!(err(&swap("fit = \"too_stiff\"", "fit = \"fits\"")).contains("uu.pal/hi"));
        let none = MINI.replacen("fit = \"fits\"", "fit = \"too_plain\"", 1);
        assert!(err(&none).contains("uu.test/eat: needs exactly one"));
        // Row, level and form checks.
        let e = err(&swap("\"eat:krama\"", "\"munch:krama\""));
        assert!(e.contains("uu.test/eat choice 1") && e.contains("munch"), "{e}");
        assert!(err(&swap("\"eat:krama\"", "\"eat:kromo\"")).contains("unknown level"));
        assert!(err(&swap("\"eat:krama\"", "\"eat:madya\"")).contains("no form at that level"));
        assert!(err(&swap("\"eat:krama\"", "\"eat\"")).contains("row_id:level"));
        let e = err(&swap("\"eat:krama\"", "\"eat:krama_inggil\""));
        assert!(e.contains("dhahar") && e.contains("Kula sampun nedha"), "{e}");
        assert!(err(&swap("uses = [\"how_are_you:krama\"]", "uses = []")).contains("no speech-level forms"));
        // Partner lines are checked too.
        let e = err(&swap("\"you_singular:ngoko\", ", "\"you_singular:krama\", "));
        assert!(e.contains("uu.test/eat partner"), "{e}");
        // Structure.
        assert!(err(&swap("level = \"j1\"", "level = \"bipa-1\"")).contains("Javanese level"));
        assert!(err(&swap("en = \"Leaving\"", "en = \" \"")).contains("empty en"));
        let one_choice = MINI.split("[[scenes.turns.choices]]\ntext = \"Pripun").next().unwrap();
        assert!(err(one_choice).contains("at least two choices"));
        let dup_text = swap("text = \"Aku pamit.\"", "text = \"Kula nyuwun pamit!\"");
        assert!(err(&dup_text).contains("same text"));
        assert!(err(&swap("name = \"Bayu\"", "name = \"\"")).contains("no name"));
        let no_turns = "reviewed = false\nversion = 1\n[[scenes]]\nid = \"x\"\nlevel = \"j1\"\n\
            title = { arz = \"a\", ar = \"a\", en = \"a\" }\nsetting = { arz = \"a\", ar = \"a\", en = \"a\" }\n\
            partner = { name = \"A\", owed = \"ngoko\", role = { arz = \"a\", ar = \"a\", en = \"a\" } }\nturns = []\n";
        assert!(err(no_turns).contains("no turns"));
        // An empty scene list parses (for a build without scenes yet).
        let empty = UnggahData::parse("reviewed = false\nversion = 2\n", LEVELS).unwrap();
        assert!(empty.scenarios.is_empty() && empty.version == 2 && !empty.table.is_empty());
    }

    // --- The shipped content -------------------------------------------------

    fn is_arabic_letter(c: char) -> bool {
        ('\u{0600}'..='\u{06FF}').contains(&c)
    }

    /// Latin letters in Arabic text sit inside FSI … PDI.
    fn isolation_errors(v: &str) -> Vec<String> {
        let mut errors = Vec::new();
        let mut depth = 0i32;
        for c in v.chars() {
            match c {
                '\u{2068}' => depth += 1,
                '\u{2069}' => {
                    depth -= 1;
                    if depth < 0 {
                        errors.push("PDI without FSI".into());
                    }
                }
                '\u{202A}'..='\u{202E}' | '\u{2066}' | '\u{2067}' => {
                    errors.push(format!("use FSI/PDI, not U+{:04X}", c as u32))
                }
                c if c.is_ascii_alphabetic() && depth == 0 => {
                    errors.push(format!("Latin {c:?} outside FSI…PDI"))
                }
                _ => {}
            }
        }
        if depth != 0 {
            errors.push("unbalanced FSI/PDI".into());
        }
        errors
    }

    fn check_meaning(what: &str, m: &Trilingual) {
        for (lang, s) in [("arz", &m.arz), ("ar", &m.ar)] {
            assert!(s.chars().any(is_arabic_letter), "{what} {lang}: no Arabic in {s:?}");
            let errors = isolation_errors(s);
            assert!(errors.is_empty(), "{what} {lang}: {errors:?} in {s:?}");
        }
        assert!(!m.en.chars().any(is_arabic_letter), "{what} en: Arabic in {:?}", m.en);
        assert!(!m.en.contains(['\u{2068}', '\u{2069}']), "{what} en: isolates belong to Arabic");
        for s in [&m.arz, &m.ar, &m.en] {
            assert!(s.matches('!').count() <= 1, "{what}: too many exclamation marks in {s:?}");
        }
    }

    /// Every word of a line is a table form it lists, or part of the partner's name.
    fn assert_covered(what: &str, text: &str, uses: &[String], d: &UnggahData, name: &str) {
        let mut allowed: HashSet<String> = words(name).into_iter().collect();
        for u in uses {
            let (row, level) = u.split_once(':').unwrap();
            for f in forms(&d.table[row], parse_level(level).unwrap()) {
                if contains_form(text, f) {
                    allowed.extend(words(f));
                }
            }
        }
        for w in words(text) {
            assert!(allowed.contains(&w), "{what}: {w:?} in {text:?} is not a listed table form");
        }
        assert!(!text.chars().any(is_arabic_letter), "{what}: Arabic in {text:?}");
        assert!(text.starts_with(|c: char| c.is_uppercase()), "{what}: sentence case in {text:?}");
    }

    #[test]
    fn content_parses_and_awaits_review() {
        let d = data();
        assert!(!d.reviewed, "set reviewed = true only after the learner approves every scene");
        assert_eq!(d.version, 1);
        let ids: Vec<&str> = d.scenarios.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["uu.friend", "uu.market", "uu.grandmother", "uu.teacher", "uu.elder"]);
        let level = |id: &str| d.scenario(id).unwrap().level.as_str();
        assert_eq!(level("uu.friend"), "j1");
        assert_eq!(level("uu.market"), "j2");
        for id in ["uu.grandmother", "uu.teacher", "uu.elder"] {
            assert_eq!(level(id), "j3");
        }
        assert_eq!(d.scenario("uu.friend").unwrap().partner.owed, SpeechLevel::Ngoko);
        for id in ["uu.grandmother", "uu.teacher", "uu.elder"] {
            assert_eq!(d.scenario(id).unwrap().partner.owed, SpeechLevel::Krama, "{id}");
        }
        assert_eq!(d.open_at(Some("j1")).len(), 1);
        assert_eq!(d.open_at(Some("j2")).len(), 2);
        assert_eq!(d.open_at(Some("j3")).len(), 5);
    }

    #[test]
    fn every_scene_is_complete_and_kind() {
        let d = data();
        for sc in &d.scenarios {
            assert!((4..=6).contains(&sc.turns.len()), "{}: 4–6 turns", sc.id);
            for (what, t) in [("title", &sc.title), ("setting", &sc.setting), ("role", &sc.partner.role)] {
                check_meaning(&format!("{} {what}", sc.id), t);
            }
            for turn in &sc.turns {
                let tid = format!("{}/{}", sc.id, turn.id);
                check_meaning(&format!("{tid} situation"), &turn.situation);
                assert_eq!(turn.choices.len(), 3, "{tid}: three choices");
                if let Some(p) = &turn.partner_says {
                    check_meaning(&format!("{tid} partner"), &p.meaning);
                }
                for (i, c) in turn.choices.iter().enumerate() {
                    let what = format!("{tid} choice {}", i + 1);
                    check_meaning(&what, &c.said.meaning);
                    check_meaning(&format!("{what} why"), &c.why);
                    assert_covered(&what, &c.said.text, &c.uses, &d, &sc.partner.name);
                    // Kind explanations: no shaming words.
                    let en = words(&c.why.en);
                    for w in
                        ["rude", "wrong", "stupid", "shame", "shameful", "bad", "mistake", "embarrassing"]
                    {
                        assert!(!en.iter().any(|x| x == w), "{what}: {w:?} in {:?}", c.why.en);
                    }
                    for w in ["عيب", "غلط", "خطأ", "قلة أدب", "وقاحة"] {
                        assert!(!c.why.arz.contains(w) && !c.why.ar.contains(w), "{what}: {w:?}");
                    }
                    if c.fit == Fit::TooPlain {
                        assert_ne!(
                            c.level,
                            SpeechLevel::KramaInggil,
                            "{what}: krama inggil is never too plain"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn partner_lines_use_only_table_forms() {
        // Partner lines are checked by parse; here every word must be covered too.
        let raw: FileToml = toml::from_str(SCENES).unwrap();
        let d = data();
        for sc in &raw.scenes {
            for t in &sc.turns {
                if let Some(p) = &t.partner_says {
                    assert_covered(
                        &format!("{}/{} partner", sc.id, t.id),
                        &p.text,
                        &p.uses,
                        &d,
                        &sc.partner.name,
                    );
                }
            }
        }
    }

    #[test]
    fn the_classic_slip_is_taught() {
        let d = data();
        let gm = d.scenario("uu.grandmother").unwrap();
        // J3 lesson j3.u01.l03 practises "Have you eaten?" at three levels:
        // the grandmother scene opens with the same sentence.
        let first = &gm.turns[0];
        let texts: Vec<&str> = first.choices.iter().map(|c| c.said.text.as_str()).collect();
        for t in ["Kowe wis mangan?", "Sampeyan sampun nedha?", "Panjenengan sampun dhahar?"] {
            assert!(texts.contains(&t), "{t:?} missing from {texts:?}");
        }
        assert_eq!(first.fitting().unwrap().said.text, "Panjenengan sampun dhahar?");
        for c in &first.choices {
            assert!(c.uses.iter().any(|u| u.starts_with("eat:")));
            assert!(c.uses.iter().any(|u| u.starts_with("you_singular:")));
        }
        // "Kula dhahar": an honorific about yourself.
        let slip = gm.turns.iter().flat_map(|t| &t.choices).find(|c| {
            let w = words(&c.said.text);
            w.contains(&"kula".to_string()) && w.contains(&"dhahar".to_string())
        });
        assert_eq!(slip.map(|c| c.fit), Some(Fit::SelfRaised));
        // Every scene but the friend has at least one self-raised option.
        for sc in &d.scenarios {
            let n = sc.turns.iter().flat_map(|t| &t.choices).filter(|c| c.fit == Fit::SelfRaised).count();
            assert!(n >= 1, "{}: no self-raised slip", sc.id);
        }
        // Friends get ngoko; anything higher is too stiff, never "too plain".
        let friend = d.scenario("uu.friend").unwrap();
        for t in &friend.turns {
            assert_eq!(t.fitting().unwrap().level, SpeechLevel::Ngoko, "{}", t.id);
            assert!(t.choices.iter().all(|c| c.fit != Fit::TooPlain), "{}", t.id);
        }
    }

    #[test]
    fn a_perfect_run_is_great_and_a_plain_one_is_not() {
        let d = data();
        for sc in &d.scenarios {
            let mut best = Run::new(sc, 3);
            while best.current().is_some() {
                let i = best.shown().iter().position(|c| c.fit == Fit::Fits).unwrap();
                best.pick(i).unwrap();
            }
            let r = best.result().unwrap();
            assert!(r.meter.is_great(), "{}: {r:?}", sc.id);
            assert_eq!(r.fitting, r.turns);

            let mut worst = Run::new(sc, 3);
            while worst.current().is_some() {
                let i = worst.shown().iter().position(|c| c.fit != Fit::Fits).unwrap();
                worst.pick(i).unwrap();
            }
            let r = worst.result().unwrap();
            assert!(!r.meter.is_great() && r.fitting == 0, "{}: {r:?}", sc.id);
        }
    }
}
