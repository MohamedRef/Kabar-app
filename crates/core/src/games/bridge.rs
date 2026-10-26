//! Arabic Bridge games (spec §12): the "You already know N words" deck,
//! Sound-Shift Detective and the False Friends Alarm. Built on
//! `content/bridge/id.jsonl` and `content/bridge/sound-rules.json`.

use std::collections::{BTreeSet, HashMap, HashSet};

use kabar_protocol::content::{BridgeEntry, Confidence, MeaningShift, ReviewStatus, SoundRule, Trilingual};

use super::Rng;

/// The Bridge dataset, parsed once.
#[derive(Clone, Debug, Default)]
pub struct BridgeData {
    pub entries: Vec<BridgeEntry>,
    pub rules: Vec<SoundRule>,
    /// Word → frequency rank (1 = most frequent).
    ranks: HashMap<String, u32>,
    /// Word → index into `entries`.
    by_word: HashMap<String, usize>,
}

impl BridgeData {
    /// `entries_jsonl`: `content/bridge/id.jsonl`; `rules_json`:
    /// `content/bridge/sound-rules.json`; `freq_tsv`: `content/freq/id.tsv`
    /// (optional) to order decks by frequency.
    ///
    /// Cards marked `needs_fix` are left out. A duplicate word or a card naming an
    /// unknown sound rule is an error: the dataset is reviewed at build time, so a
    /// bad file should fail loudly rather than show a half-right game.
    pub fn parse(entries_jsonl: &str, rules_json: &str, freq_tsv: Option<&str>) -> Result<Self, String> {
        let rules: Vec<SoundRule> =
            serde_json::from_str(rules_json).map_err(|e| format!("sound-rules.json: {e}"))?;
        let mut rule_ids = HashSet::new();
        for r in &rules {
            if !rule_ids.insert(r.id.as_str()) {
                return Err(format!("sound-rules.json: duplicate rule id {}", r.id));
            }
        }

        let mut entries = Vec::new();
        let mut by_word = HashMap::new();
        let mut first_line: HashMap<String, usize> = HashMap::new();
        for (i, line) in entries_jsonl.lines().enumerate() {
            let line_no = i + 1;
            if line.trim().is_empty() {
                continue;
            }
            let entry: BridgeEntry =
                serde_json::from_str(line).map_err(|e| format!("bridge line {line_no}: {e}"))?;
            if let Some(prev) = first_line.get(&entry.word) {
                return Err(format!(
                    "bridge line {line_no}: duplicate word {} (first on line {prev})",
                    entry.word
                ));
            }
            first_line.insert(entry.word.clone(), line_no);
            if entry.status == ReviewStatus::NeedsFix {
                continue;
            }
            if let Some(bad) = entry.sound_rules.iter().find(|r| !rule_ids.contains(r.as_str())) {
                return Err(format!("bridge line {line_no}: {} names unknown sound rule {bad}", entry.word));
            }
            by_word.insert(entry.word.clone(), entries.len());
            entries.push(entry);
        }

        let ranks = match freq_tsv {
            Some(tsv) => parse_ranks(tsv)?,
            None => HashMap::new(),
        };
        Ok(Self { entries, rules, ranks, by_word })
    }

    pub fn entry(&self, word: &str) -> Option<&BridgeEntry> {
        match self.by_word.get(word).and_then(|&i| self.entries.get(i)) {
            Some(e) if e.word == word => Some(e),
            // The index is stale if `entries` was edited after parsing.
            _ => self.entries.iter().find(|e| e.word == word),
        }
    }

    pub fn rule(&self, id: &str) -> Option<&SoundRule> {
        self.rules.iter().find(|r| r.id == id)
    }

    /// Frequency rank of a word (1 = most frequent), if known.
    pub fn rank(&self, word: &str) -> Option<u32> {
        self.ranks.get(word).copied()
    }

    /// Sort key: ranked words first by rank, then the rest alphabetically.
    fn freq_key<'a>(&self, e: &'a BridgeEntry) -> (bool, u32, &'a str) {
        let r = self.rank(&e.word);
        (r.is_none(), r.unwrap_or(0), e.word.as_str())
    }

    /// Coarse frequency band, so shuffled games still favour common words.
    fn band(&self, word: &str) -> u8 {
        match self.rank(word) {
            Some(r) if r <= 3_000 => 0,
            Some(r) if r <= 10_000 => 1,
            _ => 2,
        }
    }
}

/// `content/freq/id.tsv`: `rank<TAB>word<TAB>zipf<TAB>sources`, `#` comments.
fn parse_ranks(tsv: &str) -> Result<HashMap<String, u32>, String> {
    let mut ranks = HashMap::new();
    for (i, line) in tsv.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split('\t');
        let (Some(rank), Some(word)) = (cols.next(), cols.next()) else {
            return Err(format!("freq line {}: expected rank and word", i + 1));
        };
        let rank: u32 = rank.trim().parse().map_err(|_| format!("freq line {}: bad rank {rank:?}", i + 1))?;
        ranks.entry(word.trim().to_string()).or_insert(rank);
    }
    Ok(ranks)
}

/// Review-engine card id for a Bridge card (`CardKind::Bridge`): `bridge:pikir`.
pub fn card_id(word: &str) -> String {
    format!("bridge:{word}")
}

fn trusted(e: &BridgeEntry) -> bool {
    e.confidence >= Confidence::Medium
}

/// "You already know N words": loans whose meaning an Egyptian already knows
/// (same, narrowed or broadened; medium or high confidence), most frequent first.
pub fn known_deck(data: &BridgeData) -> Vec<&BridgeEntry> {
    let mut deck: Vec<&BridgeEntry> = data
        .entries
        .iter()
        .filter(|e| {
            trusted(e)
                && matches!(e.shift, MeaningShift::Same | MeaningShift::Narrowed | MeaningShift::Broadened)
        })
        .collect();
    deck.sort_by(|a, b| data.freq_key(a).cmp(&data.freq_key(b)));
    deck
}

/// The N in "You already know N words".
pub fn known_count(data: &BridgeData) -> usize {
    known_deck(data).len()
}

/// A sound rule with the cards that show it, for the lesson before the game.
#[derive(Clone, Debug)]
pub struct RuleLesson<'a> {
    pub rule: &'a SoundRule,
    /// The rule's own examples first (in the order the rule lists them), then
    /// every other card with the rule, most frequent first.
    pub entries: Vec<&'a BridgeEntry>,
}

pub fn rule_lesson<'a>(data: &'a BridgeData, rule_id: &str) -> Option<RuleLesson<'a>> {
    let rule = data.rule(rule_id)?;
    let mut entries: Vec<&BridgeEntry> = rule.examples.iter().filter_map(|w| data.entry(w)).collect();
    let mut rest: Vec<&BridgeEntry> = data
        .entries
        .iter()
        .filter(|e| e.sound_rules.iter().any(|r| r == rule_id) && !rule.examples.contains(&e.word))
        .collect();
    rest.sort_by(|a, b| data.freq_key(a).cmp(&data.freq_key(b)));
    entries.extend(rest);
    Some(RuleLesson { rule, entries })
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

/// Arabic without vowel marks, shadda or tatweel, for comparing words.
fn bare_arabic(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, '\u{064B}'..='\u{065F}' | '\u{0670}' | '\u{0640}')).collect()
}

/// Root letters of a card: the `root` field, or the consonants of the bare word.
fn root_letters(e: &BridgeEntry) -> BTreeSet<char> {
    let fold = |c: char| match c {
        'أ' | 'إ' | 'آ' | 'ؤ' | 'ئ' => 'ء',
        c => c,
    };
    match &e.root {
        Some(root) => root.chars().filter(|c| !c.is_whitespace()).map(fold).collect(),
        None => bare_arabic(&e.arabic)
            .chars()
            .filter(|c| !c.is_whitespace() && !matches!(c, 'ا' | 'ة' | 'ى' | 'و' | 'ي'))
            .map(fold)
            .collect(),
    }
}

fn shares_rule(e: &BridgeEntry, rules: &[String]) -> bool {
    e.sound_rules.iter().any(|r| rules.contains(r))
}

/// Picks `k` items from scored candidates: shuffled, best scores first, then a
/// random `k` from the top window so sessions vary without losing quality.
fn pick_best<T>(mut scored: Vec<(i32, T)>, k: usize, rng: &mut Rng) -> Vec<T> {
    rng.shuffle(&mut scored);
    scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    scored.truncate(k * 2);
    rng.shuffle(&mut scored);
    scored.into_iter().take(k).map(|(_, t)| t).collect()
}

/// Where the rule's Arabic letter sits, to pick which Latin letter to undo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spot {
    First,
    Last,
}

/// A wrong-but-plausible spelling that undoes one sound rule: `pikir` → `fikir`
/// (fa_p), `kalbu` → `qalbu` (qaf_k), `selamat` → `selamah`. `None` when the
/// rule has no clean reversal (vowel rules) or the word doesn't show it.
pub fn near_miss(word: &str, rule_id: &str) -> Option<String> {
    undo_rule(word, rule_id, Spot::First)
}

/// `near_miss` that uses the Arabic word to find the right letter:
/// `lafal` ← لفظ undoes the last l (`lafazh`), `lalim` ← ظالم the first (`zhalim`).
fn near_miss_for(e: &BridgeEntry, rule: &SoundRule) -> Option<String> {
    let bare = bare_arabic(&e.arabic);
    let letters: Vec<char> = bare.strip_prefix("ال").unwrap_or(&bare).chars().filter(|c| *c != 'ة').collect();
    let from: Vec<char> = rule.from.chars().collect();
    let spot = match (from.as_slice(), letters.last()) {
        ([c], Some(l)) if l == c && letters.first() != Some(c) => Spot::Last,
        _ => Spot::First,
    };
    undo_rule(&e.word, &rule.id, spot)
}

fn undo_rule(word: &str, rule_id: &str, spot: Spot) -> Option<String> {
    let b = word.as_bytes();
    // An occurrence of `from` that is not part of a digraph (kh, sy, sh, dh, zh).
    let free = |i: usize, from: &str| -> bool {
        let next = b.get(i + from.len()).copied();
        let prev = i.checked_sub(1).map(|p| b[p]);
        match from {
            "k" | "d" | "z" => next != Some(b'h'),
            "s" => !matches!(next, Some(b'h' | b'y')),
            "h" => !matches!(prev, Some(b'k' | b's' | b'd' | b'z' | b't' | b'g')),
            _ => true,
        }
    };
    let swap = |from: &str, to: &str, extra: &dyn Fn(usize) -> bool| -> Option<String> {
        let mut hits = word.match_indices(from).map(|(i, _)| i).filter(|&i| free(i, from) && extra(i));
        let i = match spot {
            Spot::First => hits.next(),
            Spot::Last => hits.last(),
        }?;
        Some(format!("{}{to}{}", &word[..i], &word[i + from.len()..]))
    };
    let any = |_: usize| true;
    let suffix = |from: &str, to: &str| word.strip_suffix(from).map(|stem| format!("{stem}{to}"));
    let out = match rule_id {
        "fa_p" => swap("p", "f", &any),
        // Already spelled kh: nothing to undo.
        "kha_k" if word.contains("kh") => None,
        "kha_k" => swap("k", "kh", &any),
        "qaf_k" => swap("k", "q", &any),
        "ta_marbuta_at" => suffix("at", "ah"),
        "ta_marbuta_ah" => suffix("ah", "at"),
        // The k that stands for ع closes a syllable: maklum → ma'lum, jamak → jama'.
        "ayn_k" => swap("k", "'", &|i| b.get(i + 1).is_none_or(|c| !b"aeiou".contains(c))),
        "tha_s" => swap("s", "ts", &any),
        "dhal_z" => swap("z", "dz", &any),
        "dad_d" => swap("d", "dh", &any),
        "zha_l" => swap("z", "zh", &any).or_else(|| swap("l", "zh", &any)),
        "sad_s" => swap("s", "sh", &any),
        "shin_sy" => swap("sy", "sh", &any),
        "shin_s" => swap("s", "sy", &any),
        // ح vs خ: the classic mix-up.
        "ha_h" => swap("h", "kh", &any),
        "qaf_g" => swap("g", "q", &any),
        "final_vowel" => {
            word.strip_suffix(['a', 'i', 'u']).filter(|stem| stem.len() >= 2).map(str::to_string)
        }
        _ => None,
    };
    out.filter(|w| w != word && !w.contains("hh"))
}

/// `n` rounds, optionally practising one rule, with 4 choices each.
///
/// Modes alternate (the seed picks which comes first). Each word and each Arabic
/// source appears at most once per session; fewer rounds come back when the
/// data runs short.
pub fn detective_rounds(data: &BridgeData, rule: Option<&str>, n: usize, seed: u64) -> Vec<DetectiveRound> {
    let mut rng = Rng::new(seed);
    let mut pool: Vec<&BridgeEntry> = data
        .entries
        .iter()
        .filter(|e| trusted(e) && !e.sound_rules.is_empty())
        .filter(|e| rule.is_none_or(|r| e.sound_rules.iter().any(|x| x == r)))
        .collect();
    rng.shuffle(&mut pool);
    pool.sort_by_key(|e| data.band(&e.word));

    let mut rounds = Vec::new();
    let mut used_arabic = HashSet::new();
    for e in pool {
        if rounds.len() == n {
            break;
        }
        if !used_arabic.insert(bare_arabic(&e.arabic)) {
            continue;
        }
        let rule_ids = match rule {
            Some(r) => vec![r.to_string()],
            None => e.sound_rules.clone(),
        };
        let mode = if (rounds.len() as u64).wrapping_add(seed).is_multiple_of(2) {
            DetectiveMode::GuessArabic
        } else {
            DetectiveMode::GuessIndonesian
        };
        let built = match mode {
            DetectiveMode::GuessArabic => guess_arabic(data, e, &rule_ids, &mut rng),
            DetectiveMode::GuessIndonesian => guess_indonesian(data, e, &rule_ids, &mut rng),
        };
        let Some((prompt, right, others)) = built else { continue };
        let mut choices = others;
        choices.push(right.clone());
        rng.shuffle(&mut choices);
        let answer = choices.iter().position(|c| *c == right).expect("right answer is among the choices");
        rounds.push(DetectiveRound { mode, word: e.word.clone(), rule_ids, prompt, choices, answer });
    }
    rounds
}

/// Prompt, right Arabic, 3 Arabic distractors that share a root letter or the rule.
fn guess_arabic(
    data: &BridgeData,
    e: &BridgeEntry,
    rules: &[String],
    rng: &mut Rng,
) -> Option<(String, String, Vec<String>)> {
    let target = bare_arabic(&e.arabic);
    let roots = root_letters(e);
    let mut seen = HashSet::from([target]);
    let mut scored = Vec::new();
    for o in data.entries.iter().filter(|o| trusted(o)) {
        let bare = bare_arabic(&o.arabic);
        let o_roots = root_letters(o);
        // Same root (عِلْم vs عُلُوم) would be too close to call.
        if o_roots == roots || !seen.insert(bare) {
            continue;
        }
        let shared = roots.intersection(&o_roots).count().min(2) as i32;
        let score = shared + if shares_rule(o, rules) { 2 } else { 0 };
        scored.push((score, o.arabic.clone()));
    }
    let others = pick_best(scored, 3, rng);
    (others.len() == 3).then(|| (e.word.clone(), e.arabic.clone(), others))
}

/// Prompt (Arabic), right word, 3 distractors: up to two near-misses that undo a
/// rule, then real words with the same rule or a similar shape.
fn guess_indonesian(
    data: &BridgeData,
    e: &BridgeEntry,
    rules: &[String],
    rng: &mut Rng,
) -> Option<(String, String, Vec<String>)> {
    let target = bare_arabic(&e.arabic);
    let mut seen = HashSet::from([e.word.clone()]);
    let mut others = Vec::new();
    for r in rules.iter().chain(e.sound_rules.iter()) {
        if others.len() == 2 {
            break;
        }
        // A near-miss that is itself a real card (surat/surah) is not "wrong".
        let miss = data.rule(r).and_then(|rule| near_miss_for(e, rule));
        if let Some(w) = miss.filter(|w| data.entry(w).is_none() && seen.insert(w.clone())) {
            others.push(w);
        }
    }
    let first = e.word.chars().next();
    let mut scored = Vec::new();
    for o in data.entries.iter().filter(|o| trusted(o)) {
        // Another word from the same Arabic would also be right.
        if bare_arabic(&o.arabic) == target || seen.contains(&o.word) {
            continue;
        }
        let mut score = if shares_rule(o, rules) { 2 } else { 0 };
        score += i32::from(o.word.chars().next() == first);
        score += i32::from(o.word.len().abs_diff(e.word.len()) <= 1);
        scored.push((score, o.word.clone()));
    }
    let need = 3 - others.len();
    others.extend(pick_best(scored, need, rng));
    (others.len() == 3).then(|| (e.arabic.clone(), e.word.clone(), others))
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

/// The spec's own alarm examples (§12); always in a session when in the data.
const ALARM_MUST: [&str; 2] = ["kalimat", "mungkin"];

fn same_meaning(a: &Trilingual, b: &Trilingual) -> bool {
    a.en.trim().eq_ignore_ascii_case(b.en.trim())
}

/// `n` rounds over false friends and shifted meanings, worst traps first.
///
/// Choices: the Indonesian meaning, the Arabic meaning (the trap) and other
/// cards' meanings, 4 in all.
pub fn alarm_rounds(data: &BridgeData, n: usize, seed: u64) -> Vec<AlarmRound> {
    let mut rng = Rng::new(seed);
    let mut ordered = Vec::new();
    for shift in [MeaningShift::FalseFriend, MeaningShift::Shifted] {
        let mut group: Vec<&BridgeEntry> =
            data.entries.iter().filter(|e| e.shift == shift && trusted(e)).collect();
        rng.shuffle(&mut group);
        group.sort_by_key(|e| data.band(&e.word));
        ordered.extend(group);
    }

    // Take the first n, swapping in the spec's examples if they fell outside.
    let mut chosen: Vec<usize> = (0..ordered.len().min(n)).collect();
    for must in ALARM_MUST {
        let Some(pos) = ordered.iter().position(|e| e.word == must) else { continue };
        if chosen.contains(&pos) {
            continue;
        }
        let swap = chosen.iter().rposition(|&i| !ALARM_MUST.contains(&ordered[i].word.as_str()));
        if let Some(slot) = swap {
            chosen[slot] = pos;
        }
    }
    chosen.sort_unstable();

    let mut rounds = Vec::new();
    for e in chosen.into_iter().map(|i| ordered[i]) {
        let mut choices = vec![e.meaning.clone()];
        if !same_meaning(&e.arabic_meaning, &e.meaning) {
            choices.push(e.arabic_meaning.clone());
        }
        let mut pool: Vec<&BridgeEntry> = data.entries.iter().filter(|o| o.word != e.word).collect();
        rng.shuffle(&mut pool);
        for o in pool {
            if choices.len() == 4 {
                break;
            }
            if !choices.iter().any(|c| same_meaning(c, &o.meaning)) {
                choices.push(o.meaning.clone());
            }
        }
        if choices.len() < 4 {
            continue;
        }
        let right = e.meaning.clone();
        rng.shuffle(&mut choices);
        let answer = choices.iter().position(|c| *c == right).expect("right meaning is among the choices");
        rounds.push(AlarmRound {
            word: e.word.clone(),
            arabic: e.arabic.clone(),
            choices,
            answer,
            note: e.shift_note.clone(),
        });
    }
    rounds
}

/// Whether the Reader should sound the alarm on this word.
pub fn is_false_friend(data: &BridgeData, word: &str) -> bool {
    data.entry(word).is_some_and(|e| e.shift == MeaningShift::FalseFriend && trusted(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRIES: &str = include_str!("../../../../content/bridge/id.jsonl");
    const RULES: &str = include_str!("../../../../content/bridge/sound-rules.json");
    const FREQ: &str = include_str!("../../../../content/freq/id.tsv");

    fn data() -> BridgeData {
        BridgeData::parse(ENTRIES, RULES, Some(FREQ)).expect("the Bridge dataset parses")
    }

    fn line(word: &str, extra: &str) -> String {
        format!(
            r#"{{"lang":"id","word":"{word}","arabic":"ا","arabic_meaning":{{"arz":"ا","ar":"ا","en":"a"}},"meaning":{{"arz":"ا","ar":"ا","en":"m-{word}"}},"shift":"same","confidence":"high","sources":["x"]{extra}}}"#
        )
    }

    #[test]
    fn parses_the_real_dataset() {
        let d = data();
        // The card set only grows (content/bridge/id.jsonl is append-only).
        assert!(d.entries.len() >= 608);
        assert!(d.rules.len() >= 19);
        assert_eq!(d.entry("kabar").unwrap().arabic, "خَبَر");
        assert!(d.entry("nope").is_none());
        assert_eq!(d.rule("fa_p").unwrap().to, "p");
        assert!(d.rule("nope").is_none());
        assert_eq!(d.rank("yang"), Some(1));
        assert!(d.rank("waktu").is_some());
        assert_eq!(card_id("pikir"), "bridge:pikir");
    }

    #[test]
    fn parse_skips_blanks_drops_needs_fix_and_reports_lines() {
        let text = format!("{}\n\n{}\n", line("satu", ""), line("dua", r#","status":"needs_fix""#));
        let d = BridgeData::parse(&text, "[]", None).unwrap();
        assert_eq!(d.entries.len(), 1);
        assert!(d.entry("dua").is_none());
        assert!(d.rank("satu").is_none());

        let bad = format!("{}\n{{\"word\":1}}\n", line("satu", ""));
        assert!(BridgeData::parse(&bad, "[]", None).unwrap_err().contains("line 2"));
        let unknown = line("satu", r#","extra":1"#);
        assert!(BridgeData::parse(&unknown, "[]", None).unwrap_err().contains("line 1"));
        let dup = format!("{}\n{}\n", line("satu", ""), line("satu", ""));
        let err = BridgeData::parse(&dup, "[]", None).unwrap_err();
        assert!(err.contains("duplicate word satu") && err.contains("line 1"), "{err}");
        let rule = line("satu", r#","sound_rules":["nope"]"#);
        assert!(BridgeData::parse(&rule, "[]", None).unwrap_err().contains("unknown sound rule nope"));
        assert!(BridgeData::parse("", "{", None).unwrap_err().contains("sound-rules"));
        assert!(BridgeData::parse("", "[]", Some("# c\n1\tyang\n")).is_ok());
        assert!(BridgeData::parse("", "[]", Some("x\tyang\n")).unwrap_err().contains("freq line 1"));
    }

    #[test]
    fn stale_index_still_finds_words() {
        let mut d = data();
        d.entries.remove(0);
        assert_eq!(d.entry("kabar").unwrap().word, "kabar");
        assert!(d.entry("abad").is_none());
    }

    #[test]
    fn known_deck_is_trusted_known_meanings_most_frequent_first() {
        let d = data();
        let deck = known_deck(&d);
        assert_eq!(deck.len(), known_count(&d));
        assert!(deck.len() > 400, "{}", deck.len());
        for e in &deck {
            assert!(matches!(e.shift, MeaningShift::Same | MeaningShift::Narrowed | MeaningShift::Broadened));
            assert!(e.confidence >= Confidence::Medium);
        }
        for w in deck.windows(2) {
            assert!(d.freq_key(w[0]) <= d.freq_key(w[1]), "{} before {}", w[0].word, w[1].word);
        }
        assert!(d.rank(&deck[0].word).is_some());
        assert!(d.rank(&deck.last().unwrap().word).is_none());
        // Spec §12 examples that keep their meaning (selamat shifted, so it is an alarm card).
        for w in ["kabar", "waktu", "lezat", "kursi", "ilmu", "pikir", "paham", "kalbu"] {
            assert!(deck.iter().any(|e| e.word == w), "{w} missing from the known deck");
        }
        assert!(!deck.iter().any(|e| e.word == "kalimat" || e.word == "selamat"));
        // Without a frequency list the deck is alphabetical.
        let plain = BridgeData::parse(ENTRIES, RULES, None).unwrap();
        let deck = known_deck(&plain);
        assert!(deck.windows(2).all(|w| w[0].word < w[1].word));
    }

    #[test]
    fn rule_lessons_start_with_the_rule_examples() {
        let d = data();
        let l = rule_lesson(&d, "fa_p").unwrap();
        assert_eq!(l.rule.from, "ف");
        let words: Vec<&str> = l.entries.iter().map(|e| e.word.as_str()).collect();
        assert_eq!(&words[..3], ["pikir", "paham", "pasal"]);
        assert!(l.entries.iter().all(|e| e.sound_rules.iter().any(|r| r == "fa_p")));
        assert!(rule_lesson(&d, "nope").is_none());
        for r in &d.rules {
            let l = rule_lesson(&d, &r.id).unwrap();
            assert_eq!(l.entries.len(), l.entries.iter().map(|e| &e.word).collect::<HashSet<_>>().len());
            assert!(!l.entries.is_empty(), "{}", r.id);
        }
    }

    #[test]
    fn near_misses_undo_the_rule() {
        assert_eq!(near_miss("pikir", "fa_p").as_deref(), Some("fikir"));
        assert_eq!(near_miss("kabar", "kha_k").as_deref(), Some("khabar"));
        assert_eq!(near_miss("kalbu", "qaf_k").as_deref(), Some("qalbu"));
        assert_eq!(near_miss("kalbu", "final_vowel").as_deref(), Some("kalb"));
        assert_eq!(near_miss("selamat", "ta_marbuta_at").as_deref(), Some("selamah"));
        assert_eq!(near_miss("maklum", "ayn_k").as_deref(), Some("ma'lum"));
        assert_eq!(near_miss("syukur", "shin_sy").as_deref(), Some("shukur"));
        assert_eq!(near_miss("hadis", "tha_s").as_deref(), Some("hadits"));
        assert_eq!(near_miss("jamak", "ayn_k").as_deref(), Some("jama'"));
        assert_eq!(near_miss("khusyuk", "ayn_k").as_deref(), Some("khusyu'"));
        assert_eq!(near_miss("akhir", "kha_k"), None);
        assert_eq!(near_miss("akhlak", "qaf_k").as_deref(), Some("akhlaq"));
        assert_eq!(near_miss("mushaf", "sad_s"), None);
        assert_eq!(near_miss("abdi", "ayn_vowel"), None);
        assert_eq!(near_miss("waktu", "fa_p"), None);
    }

    #[test]
    fn near_misses_follow_the_arabic_letter() {
        let d = data();
        let miss = |w: &str, r: &str| near_miss_for(d.entry(w).unwrap(), d.rule(r).unwrap());
        assert_eq!(miss("lafal", "zha_l").as_deref(), Some("lafazh"));
        assert_eq!(miss("lalim", "zha_l").as_deref(), Some("zhalim"));
        assert_eq!(miss("takluk", "qaf_k").as_deref(), Some("takluq"));
        assert_eq!(miss("pikir", "fa_p").as_deref(), Some("fikir"));
        for e in &d.entries {
            for r in &e.sound_rules {
                if let Some(w) = near_miss_for(e, d.rule(r).unwrap()) {
                    assert_ne!(w, e.word);
                    assert!(!w.contains("hh") && !w.contains("khh"), "{} → {w}", e.word);
                }
            }
        }
    }

    fn check_detective(d: &BridgeData, rounds: &[DetectiveRound]) {
        let mut words = HashSet::new();
        for (i, r) in rounds.iter().enumerate() {
            assert!(words.insert(r.word.clone()), "{} twice", r.word);
            let e = d.entry(&r.word).unwrap();
            assert_eq!(r.choices.len(), 4);
            assert_eq!(r.choices.iter().collect::<HashSet<_>>().len(), 4, "{r:?}");
            let right = &r.choices[r.answer];
            match r.mode {
                DetectiveMode::GuessArabic => {
                    assert_eq!(r.prompt, r.word);
                    assert_eq!(right, &e.arabic);
                    let bare: HashSet<_> = r.choices.iter().map(|c| bare_arabic(c)).collect();
                    assert_eq!(bare.len(), 4, "{r:?}");
                }
                DetectiveMode::GuessIndonesian => {
                    assert_eq!(r.prompt, e.arabic);
                    assert_eq!(right, &r.word);
                    // No other choice is a card from the same Arabic word.
                    for c in r.choices.iter().filter(|c| *c != right) {
                        if let Some(o) = d.entry(c) {
                            assert_ne!(bare_arabic(&o.arabic), bare_arabic(&e.arabic), "{r:?}");
                        }
                    }
                }
            }
            if i > 0 {
                assert_ne!(r.mode, rounds[i - 1].mode);
            }
        }
    }

    #[test]
    fn detective_rounds_are_deterministic_and_correct() {
        let d = data();
        let a = detective_rounds(&d, None, 20, 42);
        assert_eq!(a.len(), 20);
        assert_eq!(a, detective_rounds(&d, None, 20, 42));
        assert_ne!(a, detective_rounds(&d, None, 20, 43));
        assert_eq!(a[0].mode, DetectiveMode::GuessArabic);
        assert_eq!(detective_rounds(&d, None, 4, 1)[0].mode, DetectiveMode::GuessIndonesian);
        check_detective(&d, &a);
        for seed in 0..30 {
            check_detective(&d, &detective_rounds(&d, None, 12, seed));
        }
        assert!(detective_rounds(&d, Some("nope"), 10, 1).is_empty());
        assert!(detective_rounds(&d, None, 0, 1).is_empty());
    }

    #[test]
    fn every_sound_rule_has_detective_rounds() {
        let d = data();
        for r in &d.rules {
            let rounds = detective_rounds(&d, Some(&r.id), 10, 7);
            assert!(!rounds.is_empty(), "no rounds for {}", r.id);
            check_detective(&d, &rounds);
            for round in &rounds {
                assert_eq!(round.rule_ids, std::slice::from_ref(&r.id));
                assert!(d.entry(&round.word).unwrap().sound_rules.contains(&r.id));
            }
        }
        // qaf_g has a single card: one round, not ten.
        assert_eq!(detective_rounds(&d, Some("qaf_g"), 10, 7).len(), 1);
    }

    #[test]
    fn fa_p_rounds_use_the_wrong_rule_as_a_trap() {
        let d = data();
        let rounds = detective_rounds(&d, Some("fa_p"), 10, 3);
        let gi: Vec<_> = (0..20)
            .flat_map(|s| detective_rounds(&d, Some("fa_p"), 10, s))
            .filter(|r| r.mode == DetectiveMode::GuessIndonesian)
            .collect();
        // The f-for-p near miss is offered as a trap (paham → faham is not, since
        // faham is a real variant spelling and would be a second right answer).
        assert!(
            gi.iter().any(|r| near_miss(&r.word, "fa_p").is_some_and(|n| r.choices.contains(&n))),
            "no round offers the f-for-p trap"
        );
        for r in &gi {
            let wrong = r.choices.iter().enumerate().filter(|(i, _)| *i != r.answer);
            assert!(
                wrong
                    .clone()
                    .all(|(_, c)| d.entry(c).is_none_or(|e| e.arabic != d.entry(&r.word).unwrap().arabic)),
                "{r:?}"
            );
        }
        // Arabic distractors share the rule's letter or a root letter, so it's a real puzzle.
        let ga = rounds.iter().find(|r| r.mode == DetectiveMode::GuessArabic).unwrap();
        let with_fa = ga.choices.iter().filter(|c| c.contains('ف')).count();
        assert!(with_fa >= 2, "{ga:?}");
    }

    #[test]
    fn alarm_rounds_put_false_friends_first_and_teach_the_trap() {
        let d = data();
        let rounds = alarm_rounds(&d, 12, 5);
        assert_eq!(rounds.len(), 12);
        assert_eq!(rounds, alarm_rounds(&d, 12, 5));
        let words: HashSet<_> = rounds.iter().map(|r| r.word.as_str()).collect();
        assert_eq!(words.len(), 12);
        assert!(words.contains("kalimat") && words.contains("mungkin"));
        let mut seen_shifted = false;
        for r in &rounds {
            let e = d.entry(&r.word).unwrap();
            assert_eq!(r.arabic, e.arabic);
            assert_eq!(r.choices.len(), 4);
            assert_eq!(r.choices.iter().map(|c| c.en.to_lowercase()).collect::<HashSet<_>>().len(), 4);
            assert_eq!(r.choices[r.answer], e.meaning);
            assert_eq!(r.note, e.shift_note);
            if !same_meaning(&e.meaning, &e.arabic_meaning) {
                assert!(r.choices.contains(&e.arabic_meaning), "{} lacks its trap", r.word);
            }
            match e.shift {
                MeaningShift::FalseFriend => assert!(!seen_shifted, "false friends come first"),
                MeaningShift::Shifted => seen_shifted = true,
                s => panic!("{s:?} in the alarm"),
            }
        }
        let kalimat = rounds.iter().find(|r| r.word == "kalimat").unwrap();
        assert_eq!(kalimat.choices[kalimat.answer].en, "sentence");
        // Even a tiny session keeps the spec's two examples.
        for seed in 0..10 {
            let two = alarm_rounds(&d, 2, seed);
            let w: HashSet<_> = two.iter().map(|r| r.word.as_str()).collect();
            assert_eq!(w, HashSet::from(["kalimat", "mungkin"]));
        }
        let all = alarm_rounds(&d, 1000, 1);
        let expected = d
            .entries
            .iter()
            .filter(|e| matches!(e.shift, MeaningShift::FalseFriend | MeaningShift::Shifted) && trusted(e))
            .count();
        assert_eq!(all.len(), expected);
        assert!(alarm_rounds(&d, 0, 1).is_empty());
    }

    #[test]
    fn false_friend_lookup() {
        let d = data();
        assert!(is_false_friend(&d, "kalimat"));
        assert!(is_false_friend(&d, "abad"));
        assert!(!is_false_friend(&d, "mungkin"));
        assert!(!is_false_friend(&d, "kabar"));
        assert!(!is_false_friend(&d, "nope"));
        let count = d.entries.iter().filter(|e| is_false_friend(&d, &e.word)).count();
        assert!(count >= 30);
    }
}
