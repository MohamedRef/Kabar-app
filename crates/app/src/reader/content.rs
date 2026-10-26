//! The vocabulary content pack (top-2,000 Indonesian glosses and the Arabic
//! Bridge cards from `content/`), merged over the hand-made seed. Embedded in
//! the binary until downloadable content packs arrive, so lookups never wait
//! for the network.

use std::collections::HashMap;

use kabar_protocol::content::{self, BridgeEntry, GlossEntry, MeaningShift, ReviewStatus, Trilingual};

use super::glossary::{Bridge, Gloss, Glossary, SeedGlossary, Shift, WordInfo, WordLang};

const GLOSS_BATCHES: &[&str] = &[
    include_str!("../../../../content/glosses/id/batch-001.jsonl"),
    include_str!("../../../../content/glosses/id/batch-002.jsonl"),
    include_str!("../../../../content/glosses/id/batch-003.jsonl"),
    include_str!("../../../../content/glosses/id/batch-004.jsonl"),
    include_str!("../../../../content/glosses/id/batch-005.jsonl"),
    include_str!("../../../../content/glosses/id/batch-006.jsonl"),
    include_str!("../../../../content/glosses/id/batch-007.jsonl"),
    include_str!("../../../../content/glosses/id/batch-008.jsonl"),
    include_str!("../../../../content/glosses/id/batch-009.jsonl"),
    include_str!("../../../../content/glosses/id/batch-010.jsonl"),
    include_str!("../../../../content/glosses/id/batch-011.jsonl"),
    include_str!("../../../../content/glosses/id/batch-012.jsonl"),
    include_str!("../../../../content/glosses/id/batch-013.jsonl"),
    include_str!("../../../../content/glosses/id/batch-014.jsonl"),
    include_str!("../../../../content/glosses/id/batch-015.jsonl"),
    include_str!("../../../../content/glosses/id/batch-016.jsonl"),
    include_str!("../../../../content/glosses/id/batch-017.jsonl"),
    include_str!("../../../../content/glosses/id/batch-018.jsonl"),
    include_str!("../../../../content/glosses/id/batch-019.jsonl"),
    include_str!("../../../../content/glosses/id/batch-020.jsonl"),
];

const BRIDGE_ID: &str = include_str!("../../../../content/bridge/id.jsonl");

pub struct LocalGlossary {
    words: HashMap<String, WordInfo>,
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

fn bridge(b: &BridgeEntry) -> Bridge {
    Bridge {
        arabic: b.arabic.clone(),
        root: b.root.clone(),
        shift: shift(b.shift),
        note: b.shift_note.as_ref().map(gloss),
    }
}

fn jsonl<'a, T: serde::de::DeserializeOwned + 'a>(
    text: &'a str,
) -> impl Iterator<Item = Result<T, serde_json::Error>> + 'a {
    text.lines().filter(|l| !l.trim().is_empty()).map(serde_json::from_str)
}

impl LocalGlossary {
    pub fn load() -> Self {
        let seed = SeedGlossary::default();
        let mut words: HashMap<String, WordInfo> =
            seed.words.into_iter().map(|(k, v)| (k.to_string(), v)).collect();

        let bridges: HashMap<String, BridgeEntry> = jsonl::<BridgeEntry>(BRIDGE_ID)
            .filter_map(|r| r.map_err(|e| eprintln!("kabar: bad bridge entry: {e}")).ok())
            .map(|b| (b.word.to_lowercase(), b))
            .collect();

        for entry in GLOSS_BATCHES.iter().flat_map(|b| jsonl::<GlossEntry>(b)) {
            let e = match entry {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("kabar: bad gloss entry: {err}");
                    continue;
                }
            };
            let Some(first) = e.senses.first() else { continue };
            // Bridge card: by word, else the lemma, else the root (memikirkan → pikir).
            let card = [Some(&e.word), Some(&e.lemma), e.root.as_ref()]
                .into_iter()
                .flatten()
                .find_map(|k| bridges.get(&k.to_lowercase()));
            let mut senses: Vec<String> = Vec::new();
            for s in &e.senses {
                if !senses.contains(&s.gloss.en) {
                    senses.push(s.gloss.en.clone());
                }
            }
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
            let key = e.word.to_lowercase();
            let seed_levels = words.get(&key).and_then(|w| w.levels.clone());
            let info = WordInfo {
                lemma: e.lemma.clone(),
                lang: word_lang(e.lang),
                pos: serde_json::to_value(first.pos)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.replace('_', " "))),
                gloss: Gloss { arz: joined(|t| &t.arz), ar: joined(|t| &t.ar), en: joined(|t| &t.en) },
                bridge: card.map(bridge),
                levels: seed_levels,
                note: e.false_friend.as_ref().or(e.note.as_ref()).map(gloss),
                example: e.examples.first().map(|x| (x.text.clone(), gloss(&x.translation))),
                verified: e.status == ReviewStatus::Approved,
            };
            words.insert(key, info);
        }

        // Bridge words outside the top 2,000 still get a card.
        for (word, b) in &bridges {
            words.entry(word.clone()).or_insert_with(|| WordInfo {
                lemma: b.word.clone(),
                lang: word_lang(b.lang),
                pos: None,
                gloss: gloss(&b.meaning),
                bridge: Some(bridge(b)),
                levels: None,
                note: None,
                example: None,
                verified: b.status == ReviewStatus::Approved,
            });
        }
        // Seed words keep their Bridge card when the content pack has none.
        for (word, b) in SeedGlossary::default().words.into_iter().filter_map(|(k, v)| Some((k, v.bridge?))) {
            if let Some(w) = words.get_mut(word) {
                w.bridge.get_or_insert(b);
            }
        }
        Self { words }
    }
}

impl Glossary for LocalGlossary {
    fn lookup(&self, forms: &[&str]) -> Option<&WordInfo> {
        forms.iter().find_map(|f| self.words.get(*f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_every_gloss_batch() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/glosses/id");
        let on_disk = std::fs::read_dir(dir)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".jsonl"))
            .count();
        assert_eq!(on_disk, GLOSS_BATCHES.len(), "add new batches to GLOSS_BATCHES");
    }

    #[test]
    fn every_line_parses() {
        for b in GLOSS_BATCHES {
            for r in jsonl::<GlossEntry>(b) {
                r.unwrap();
            }
        }
        for r in jsonl::<BridgeEntry>(BRIDGE_ID) {
            r.unwrap();
        }
    }

    #[test]
    fn content_merges_over_the_seed() {
        let g = LocalGlossary::load();
        assert!(g.words.len() > 2000);
        // From the content pack.
        let yang = g.lookup(&["yang"]).unwrap();
        assert!(yang.gloss.arz.contains("اللي"));
        assert!(yang.example.is_some());
        // Bridge card by word, and the false-friend alarm.
        let kalimat = g.lookup(&["kalimat"]).unwrap();
        assert_eq!(kalimat.bridge.as_ref().unwrap().shift, Shift::FalseFriend);
        // Javanese and speech levels still come from the seed.
        let nedha = g.lookup(&["nedha"]).unwrap();
        assert_eq!(nedha.levels.as_ref().unwrap().ngoko, "mangan");
        assert!(g.lookup(&["kabar"]).unwrap().bridge.is_some());
    }
}
