//! The vocabulary content pack (every Indonesian and Javanese gloss batch,
//! the Arabic Bridge cards and the Javanese speech-level table from
//! `content/`), merged over the hand-made seed. Embedded (compressed) in
//! the binary until downloadable content packs arrive, so lookups never wait
//! for the network.

use std::collections::{HashMap, HashSet};

use kabar_protocol::content::{
    self, BridgeEntry, GlossEntry, MeaningShift, ReviewStatus, SpeechLevelEntry, Trilingual,
};
use kabar_protocol::learn::TaughtEntry;

use super::glossary::{
    Bridge, Gloss, Glossary, Levels, SeedGlossary, Shift, SpeechLevel, WordInfo, WordLang,
};

// `GLOSS_BATCHES` (Indonesian) and `GLOSS_BATCHES_JV`: every
// `content/glosses/<lang>/*.jsonl`, listed by build.rs.
use crate::embedded::{BRIDGE_ID, GLOSS_BATCHES, GLOSS_BATCHES_JV, SPEECH_LEVELS, unpack};

/// `prepare` applied to every gloss entry, Indonesian batches first, in file
/// order. The batches are unpacked, parsed and prepared on all cores: the
/// Reader's first popover waits for this.
fn gloss_entries<T: Send>(
    prepare: impl Fn(GlossEntry) -> T + Sync,
) -> impl Iterator<Item = Result<T, serde_json::Error>> {
    let batches: Vec<&[u8]> = GLOSS_BATCHES.iter().chain(GLOSS_BATCHES_JV).copied().collect();
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8);
    let prepare = &prepare;
    let parts: Vec<Vec<_>> = std::thread::scope(|s| {
        let workers: Vec<_> = batches
            .chunks(batches.len().div_ceil(cores).max(1))
            .map(|chunk| {
                s.spawn(move || {
                    let mut out = Vec::new();
                    for b in chunk {
                        out.extend(jsonl::<GlossEntry>(&unpack(b)).map(|r| r.map(prepare)));
                    }
                    out
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().expect("gloss parser panicked")).collect()
    });
    parts.into_iter().flatten()
}

/// Words by lowercase spelling; one entry per language (Indonesian and
/// Javanese share ~350 spellings).
pub struct LocalGlossary {
    words: HashMap<String, Vec<WordInfo>>,
}

fn gloss(t: &Trilingual) -> Gloss {
    Gloss { arz: t.arz.clone(), ar: t.ar.clone(), en: t.en.clone() }
}

fn word_lang(l: content::Lang) -> WordLang {
    match l {
        content::Lang::Id => WordLang::Indonesian,
        content::Lang::Jv => WordLang::Javanese,
    }
}

fn shift(s: MeaningShift) -> Shift {
    match s {
        MeaningShift::Same => Shift::Same,
        MeaningShift::Narrowed => Shift::Narrowed,
        MeaningShift::Broadened => Shift::Broadened,
        MeaningShift::Shifted => Shift::Shifted,
        MeaningShift::FalseFriend => Shift::FalseFriend,
    }
}

fn level(l: content::SpeechLevel) -> SpeechLevel {
    match l {
        content::SpeechLevel::Ngoko => SpeechLevel::Ngoko,
        content::SpeechLevel::Madya => SpeechLevel::Madya,
        content::SpeechLevel::Krama => SpeechLevel::Krama,
        content::SpeechLevel::KramaInggil => SpeechLevel::KramaInggil,
        content::SpeechLevel::KramaAndhap => SpeechLevel::KramaAndhap,
    }
}

fn bridge(b: &BridgeEntry) -> Bridge {
    Bridge {
        arabic: b.arabic.clone(),
        root: b.root.clone(),
        shift: shift(b.shift),
        note: b.shift_note.as_ref().map(gloss),
    }
}

/// "proper_noun" → "proper noun".
fn pos_name<T: serde::Serialize>(pos: &T) -> Option<String> {
    serde_json::to_value(pos).ok().and_then(|v| v.as_str().map(|s| s.replace('_', " ")))
}

fn jsonl<'a, T: serde::de::DeserializeOwned + 'a>(
    text: &'a str,
) -> impl Iterator<Item = Result<T, serde_json::Error>> + 'a {
    text.lines().filter(|l| !l.trim().is_empty()).map(serde_json::from_str)
}

/// The popover's view of a gloss entry: up to three senses joined, the
/// first example, the false-friend warning before any other note.
pub fn word_info(e: &GlossEntry) -> Option<WordInfo> {
    let first = e.senses.first()?;
    let joined = |f: fn(&Trilingual) -> &String| {
        let mut out: Vec<&str> = Vec::new();
        for s in e.senses.iter().take(3) {
            let g = f(&s.gloss).as_str();
            if !out.contains(&g) {
                out.push(g);
            }
        }
        out.join(" · ")
    };
    Some(WordInfo {
        lemma: e.lemma.clone(),
        lang: word_lang(e.lang),
        pos: pos_name(&first.pos),
        gloss: Gloss { arz: joined(|t| &t.arz), ar: joined(|t| &t.ar), en: joined(|t| &t.en) },
        indonesian: e.indonesian.clone(),
        bridge: None,
        levels: e.speech_level.map(|l| Levels { level: Some(level(l)), ..Levels::default() }),
        note: e.false_friend.as_ref().or(e.note.as_ref()).map(gloss),
        example: e.examples.first().map(|x| (x.text.clone(), gloss(&x.translation))),
        verified: e.status == ReviewStatus::Approved,
    })
}

/// An entry the learner taught Kabar, shown like any other word.
pub fn taught_info(t: &TaughtEntry) -> Option<WordInfo> {
    let mut info = word_info(&t.entry)?;
    info.bridge = t.bridge.as_ref().map(|b| Bridge {
        arabic: b.arabic.clone(),
        root: b.root.clone(),
        shift: shift(b.shift),
        note: b.shift_note.as_ref().map(gloss),
    });
    if let Some(l) = &t.levels {
        let one = |f: &Option<String>| f.iter().cloned().collect::<Vec<_>>();
        info.levels = Some(Levels {
            level: match l.level.as_str() {
                "ngoko" => Some(SpeechLevel::Ngoko),
                "madya" => Some(SpeechLevel::Madya),
                "krama" => Some(SpeechLevel::Krama),
                "krama_inggil" => Some(SpeechLevel::KramaInggil),
                "krama_andhap" => Some(SpeechLevel::KramaAndhap),
                _ => None,
            },
            ngoko: one(&l.ngoko),
            krama: one(&l.krama),
            krama_inggil: one(&l.krama_inggil),
            ..Levels::default()
        });
    }
    info.verified = t.is_approved();
    Some(info)
}

/// The content pack's words and ranks (Indonesian and Javanese).
pub fn known_words() -> Vec<kabar_core::ink::spell::Known> {
    gloss_entries(|e| e)
        .filter_map(Result::ok)
        .map(|e| kabar_core::ink::spell::Known { lang: e.lang, word: e.word.to_lowercase(), rank: e.rank })
        .collect()
}

impl LocalGlossary {
    pub fn load() -> Self {
        let mut g = Self { words: HashMap::new() };
        for (k, v) in SeedGlossary::default().words {
            g.upsert(k.to_string(), v);
        }

        let bridges: HashMap<String, BridgeEntry> = jsonl::<BridgeEntry>(&unpack(BRIDGE_ID))
            .filter_map(|r| r.map_err(|e| crate::diag::report!("bad bridge entry: {e}")).ok())
            .map(|b| (b.word.to_lowercase(), b))
            .collect();

        // Indonesian word → its lemma and root, for Javanese words whose card is
        // their Indonesian equivalent's (ngurmati → menghormati → hormat).
        let mut id_stems: HashMap<String, Vec<String>> = HashMap::new();
        // On the parser threads: the popover view and its Bridge card, by word,
        // else the lemma, else the root (memikirkan → pikir).
        let prepared = gloss_entries(|e| {
            let mut info = word_info(&e)?;
            info.bridge = [Some(&e.word), Some(&e.lemma), e.root.as_ref()]
                .into_iter()
                .flatten()
                .filter_map(|k| bridges.get(&k.to_lowercase()))
                .find(|b| b.lang == e.lang)
                .map(bridge);
            Some((e, info))
        });
        for entry in prepared {
            let (e, mut info) = match entry {
                Ok(Some(prepared)) => prepared,
                Ok(None) => continue,
                Err(err) => {
                    crate::diag::report!("bad gloss entry: {err}");
                    continue;
                }
            };
            match e.lang {
                content::Lang::Id => {
                    let stems = [Some(e.lemma.clone()), e.root.clone()].into_iter().flatten().collect();
                    id_stems.insert(e.word.to_lowercase(), stems);
                }
                // The Bridge is Indonesian: an Arabic-origin Javanese word shows the
                // card of its own spelling, or of its Indonesian equivalent.
                content::Lang::Jv if info.bridge.is_none() && e.arabic_origin.is_some() => {
                    let first = e
                        .indonesian
                        .as_deref()
                        .and_then(|i| i.split(',').next())
                        .map(|f| f.trim().to_lowercase());
                    let via = first
                        .iter()
                        .chain(first.as_ref().and_then(|f| id_stems.get(f)).into_iter().flatten());
                    info.bridge = [Some(&e.word), Some(&e.lemma), e.root.as_ref()]
                        .into_iter()
                        .flatten()
                        .chain(via)
                        .find_map(|k| bridges.get(&k.to_lowercase()))
                        .map(bridge);
                }
                _ => {}
            }
            let key = e.word.to_lowercase();
            // The seed's hand-checked level table stays until the real one below.
            if info.levels.is_none() {
                info.levels = g.get(&key, info.lang).and_then(|w| w.levels.clone());
            }
            g.upsert(key, info);
        }

        // Bridge words outside the top 2,000 still get a card.
        for (word, b) in &bridges {
            if g.get(word, word_lang(b.lang)).is_none() {
                g.upsert(
                    word.clone(),
                    WordInfo {
                        lemma: b.word.clone(),
                        lang: word_lang(b.lang),
                        pos: None,
                        gloss: gloss(&b.meaning),
                        indonesian: None,
                        bridge: Some(bridge(b)),
                        levels: None,
                        note: None,
                        example: None,
                        verified: b.status == ReviewStatus::Approved,
                    },
                );
            }
        }
        // Seed words keep their Bridge card when the content pack has none.
        for (word, seed) in SeedGlossary::default().words {
            if let (Some(b), Some(w)) = (seed.bridge, g.get_mut(word, seed.lang)) {
                w.bridge.get_or_insert(b);
            }
        }
        g.add_speech_levels();
        // A second push gives a Vec room for four; most spellings have one or two.
        for list in g.words.values_mut() {
            list.shrink_to_fit();
        }
        g.words.shrink_to_fit();
        g
    }

    /// The speech-level table (§13): every form of a meaning, at every level,
    /// points at the whole row.
    fn add_speech_levels(&mut self) {
        let mut done: HashSet<String> = HashSet::new();
        for row in jsonl::<SpeechLevelEntry>(&unpack(SPEECH_LEVELS)) {
            let row = match row {
                Ok(r) => r,
                Err(err) => {
                    crate::diag::report!("bad speech-level entry: {err}");
                    continue;
                }
            };
            let base = Levels {
                level: None,
                ngoko: row.ngoko.clone(),
                madya: row.madya.clone(),
                krama: row.krama.clone(),
                krama_inggil: row.krama_inggil.clone(),
                krama_andhap: row.krama_andhap.clone(),
                note: row.note.as_ref().map(gloss),
            };
            let mut seen: HashSet<String> = HashSet::new();
            for (lvl, forms) in base.rows() {
                for form in forms {
                    let key = form.to_lowercase();
                    // A word on two rows (sampun: "already", "finished") keeps the first.
                    if !seen.insert(key.clone()) || done.contains(&key) {
                        continue;
                    }
                    let levels = Levels { level: Some(lvl), ..base.clone() };
                    match self.get_mut(&key, WordLang::Javanese) {
                        Some(w) => {
                            w.levels = Some(levels);
                            w.indonesian.get_or_insert_with(|| row.indonesian.clone());
                        }
                        None => self.upsert(
                            key.clone(),
                            WordInfo {
                                lemma: form.clone(),
                                lang: WordLang::Javanese,
                                pos: pos_name(&row.pos),
                                gloss: gloss(&row.meaning),
                                indonesian: Some(row.indonesian.clone()),
                                bridge: None,
                                levels: Some(levels),
                                note: None,
                                example: None,
                                verified: row.status == ReviewStatus::Approved,
                            },
                        ),
                    }
                }
            }
            done.extend(seen);
        }
    }

    fn get(&self, key: &str, lang: WordLang) -> Option<&WordInfo> {
        self.words.get(key)?.iter().find(|w| w.lang == lang)
    }

    fn get_mut(&mut self, key: &str, lang: WordLang) -> Option<&mut WordInfo> {
        self.words.get_mut(key)?.iter_mut().find(|w| w.lang == lang)
    }

    /// Adds a word, replacing the same spelling in the same language.
    fn upsert(&mut self, key: String, info: WordInfo) {
        let list = self.words.entry(key).or_insert_with(|| Vec::with_capacity(1));
        match list.iter_mut().find(|w| w.lang == info.lang) {
            Some(w) => *w = info,
            None => list.push(info),
        }
    }
}

impl Glossary for LocalGlossary {
    fn lookup(&self, prefer: Option<WordLang>, forms: &[&str]) -> Option<&WordInfo> {
        let find = |forms: &[&str]| {
            prefer
                .and_then(|p| forms.iter().find_map(|f| self.get(f, p)))
                .or_else(|| forms.iter().find_map(|f| self.words.get(*f).and_then(|l| l.first())))
        };
        find(forms).or_else(|| {
            // Glosses spell Javanese without accents (kowe); kowé and kowè
            // are accepted variants.
            let plain: Vec<String> = forms.iter().map(|f| unaccented(f)).collect();
            let plain: Vec<&str> = plain.iter().map(String::as_str).collect();
            if plain != forms { find(&plain) } else { None }
        })
    }
}

/// Javanese é/è written as plain e, the spelling the glosses use.
fn unaccented(word: &str) -> String {
    word.chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' => 'e',
            'É' | 'È' | 'Ê' => 'E',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_every_gloss_batch() {
        for (lang, embedded) in [("id", GLOSS_BATCHES), ("jv", GLOSS_BATCHES_JV)] {
            let dir = format!("{}/../../content/glosses/{lang}", env!("CARGO_MANIFEST_DIR"));
            let on_disk = std::fs::read_dir(dir)
                .unwrap()
                .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".jsonl"))
                .count();
            assert_eq!(on_disk, embedded.len(), "build.rs missed a {lang} gloss batch");
        }
    }

    #[test]
    fn javanese_arabic_loans_show_the_indonesian_card() {
        let g = LocalGlossary::load();
        let card = |w: &str| {
            g.lookup(Some(WordLang::Javanese), &[w]).and_then(|i| i.bridge.as_ref()).map(|b| b.arabic.clone())
        };
        // Through the Indonesian equivalent's root: ngurmati → menghormati → hormat.
        assert!(card("ngurmati").is_some());
        // Not every Javanese word that shares a spelling gets one: only Arabic loans.
        assert!(card("kowe").is_none());
    }

    #[test]
    fn accented_javanese_finds_the_plain_gloss() {
        assert_eq!(unaccented("kowé"), "kowe");
        assert_eq!(unaccented("Sampèyan"), "Sampeyan");
        let g = LocalGlossary::load();
        let plain = g.lookup(Some(WordLang::Javanese), &["kowe"]).map(|w| w.lemma.clone());
        assert!(plain.is_some());
        assert_eq!(g.lookup(Some(WordLang::Javanese), &["kowé"]).map(|w| w.lemma.clone()), plain);
    }

    #[test]
    fn every_line_parses() {
        for r in gloss_entries(|e| e) {
            r.unwrap();
        }
        for r in jsonl::<BridgeEntry>(&unpack(BRIDGE_ID)) {
            r.unwrap();
        }
        for r in jsonl::<SpeechLevelEntry>(&unpack(SPEECH_LEVELS)) {
            r.unwrap();
        }
    }

    #[test]
    fn content_merges_over_the_seed() {
        let g = LocalGlossary::load();
        assert!(g.words.len() > 2000);
        // From the content pack.
        let yang = g.lookup(None, &["yang"]).unwrap();
        assert!(yang.gloss.arz.contains("اللي"));
        assert!(yang.example.is_some());
        // Bridge card by word, and the false-friend alarm.
        let kalimat = g.lookup(None, &["kalimat"]).unwrap();
        assert_eq!(kalimat.bridge.as_ref().unwrap().shift, Shift::FalseFriend);
        assert!(g.lookup(None, &["kabar"]).unwrap().bridge.is_some());
    }

    #[test]
    fn javanese_and_indonesian_share_spellings() {
        let g = LocalGlossary::load();
        let jv = WordLang::Javanese;
        let id = WordLang::Indonesian;
        // "aku" is in both packs: the preferred language wins.
        assert_eq!(g.lookup(Some(jv), &["aku"]).unwrap().lang, jv);
        assert_eq!(g.lookup(Some(id), &["aku"]).unwrap().lang, id);
        assert_eq!(g.lookup(None, &["aku"]).unwrap().lang, id);
        // A Javanese word shows its Indonesian equivalent.
        let sing = g.lookup(Some(jv), &["sing"]).unwrap();
        assert!(sing.indonesian.is_some());
    }

    #[test]
    fn speech_levels_come_from_the_table() {
        let g = LocalGlossary::load();
        let jv = Some(WordLang::Javanese);
        // "Have you eaten?" across the levels (§24).
        for (word, lvl) in [
            ("mangan", SpeechLevel::Ngoko),
            ("nedha", SpeechLevel::Krama),
            ("dhahar", SpeechLevel::KramaInggil),
        ] {
            let l = g.lookup(jv, &[word]).unwrap().levels.clone().unwrap();
            assert_eq!(l.level, Some(lvl), "{word}");
            assert!(l.ngoko.iter().any(|f| f == "mangan"), "{word}");
            assert!(l.krama_inggil.iter().any(|f| f == "dhahar"), "{word}");
        }
        let you = g.lookup(jv, &["panjenengan"]).unwrap().levels.clone().unwrap();
        assert_eq!(you.level, Some(SpeechLevel::KramaInggil));
        assert!(you.note.is_some());
        assert!(!you.madya.is_empty());
    }
}
