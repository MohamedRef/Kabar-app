//! Arabic Bridge games (spec §12): the "You already know N words" deck,
//! Sound-Shift Detective and the False Friends Alarm. Built on
//! `content/bridge/id.jsonl` and `content/bridge/sound-rules.json`.

use kabar_protocol::content::{BridgeEntry, SoundRule, Trilingual};

/// The Bridge dataset, parsed once.
#[derive(Clone, Debug, Default)]
pub struct BridgeData {
    pub entries: Vec<BridgeEntry>,
    pub rules: Vec<SoundRule>,
}

impl BridgeData {
    /// `entries_jsonl`: `content/bridge/id.jsonl`; `rules_json`:
    /// `content/bridge/sound-rules.json`; `freq_tsv`: `content/freq/id.tsv`
    /// (optional) to order decks by frequency.
    pub fn parse(entries_jsonl: &str, rules_json: &str, freq_tsv: Option<&str>) -> Result<Self, String> {
        let _ = (entries_jsonl, rules_json, freq_tsv);
        todo!("m10 agent B")
    }

    pub fn entry(&self, word: &str) -> Option<&BridgeEntry> {
        let _ = word;
        todo!("m10 agent B")
    }

    pub fn rule(&self, id: &str) -> Option<&SoundRule> {
        let _ = id;
        todo!("m10 agent B")
    }

    /// Frequency rank of a word (1 = most frequent), if known.
    pub fn rank(&self, word: &str) -> Option<u32> {
        let _ = word;
        todo!("m10 agent B")
    }
}

/// "You already know N words": loans whose meaning an Egyptian already knows
/// (same, narrowed or broadened; medium or high confidence), most frequent first.
pub fn known_deck(data: &BridgeData) -> Vec<&BridgeEntry> {
    let _ = data;
    todo!("m10 agent B")
}

/// One Sound-Shift Detective question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectiveRound {
    pub mode: DetectiveMode,
    /// The Indonesian word the round is about.
    pub word: String,
    /// The rule(s) the word shows.
    pub rule_ids: Vec<String>,
    /// What the learner sees: the Indonesian word (GuessArabic) or the Arabic word (GuessIndonesian).
    pub prompt: String,
    pub choices: Vec<String>,
    /// Index of the right choice.
    pub answer: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectiveMode {
    /// See `pikir`, pick «فكر».
    GuessArabic,
    /// See «فهم» and the rule, pick `paham`.
    GuessIndonesian,
}

/// `n` rounds, optionally practising one rule, with 4 choices each.
pub fn detective_rounds(data: &BridgeData, rule: Option<&str>, n: usize, seed: u64) -> Vec<DetectiveRound> {
    let _ = (data, rule, n, seed);
    todo!("m10 agent B")
}

/// One False Friends Alarm question: what does this word mean in Indonesian?
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlarmRound {
    pub word: String,
    /// The Arabic word it looks like.
    pub arabic: String,
    pub choices: Vec<Trilingual>,
    pub answer: usize,
    /// Why the meaning moved; shown after answering.
    pub note: Option<Trilingual>,
}

/// `n` rounds over false friends and shifted meanings, worst traps first.
pub fn alarm_rounds(data: &BridgeData, n: usize, seed: u64) -> Vec<AlarmRound> {
    let _ = (data, n, seed);
    todo!("m10 agent B")
}

/// Whether the Reader should sound the alarm on this word.
pub fn is_false_friend(data: &BridgeData, word: &str) -> bool {
    let _ = (data, word);
    todo!("m10 agent B")
}
