use std::path::PathBuf;

use serde::Deserialize;

use super::*;
use crate::grammar::tests::library;
use crate::grammar::word::WordClass;
use crate::journey::parse_curriculum;
use crate::lang::analyze::Engine;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `(grammar id, matched words)` for every hit in `text`.
fn spot(lib: &Library, engine: &Engine, text: &str, lang: Lang) -> Vec<(String, String)> {
    let tokens = engine.analyze(text);
    let words: Vec<(usize, &AnalyzedToken)> =
        tokens.iter().enumerate().filter(|(_, t)| t.token.is_wordlike()).collect();
    lib.spot(&words, lang)
        .into_iter()
        .map(|h| {
            let label = h.label(|i| tokens[i].token.text.clone());
            (h.grammar, label)
        })
        .collect()
}

#[test]
fn every_cue_compiles_and_names_a_real_entry() {
    let lib = library();
    let mut cues = 0;
    for (id, patterns) in &lib.patterns {
        for (i, p) in patterns.iter().enumerate() {
            assert!(p.is_some(), "{id} cue {i}: {:?}", lib.points[id].cues[i]);
            cues += 1;
        }
    }
    assert!(cues > 150, "{cues}");
}

#[test]
fn pattern_language() {
    let cue = |m: &str| GrammarCue { pattern: m.into(), class: None, note: None };
    let p = compile(&cue("lebih … daripada|dari")).unwrap();
    assert_eq!(p.slots.len(), 3);
    assert_eq!(p.role, Role::Phrase);
    assert_eq!(compile(&cue("sudah|udah")).unwrap().role, Role::Class(WordClass::Function));
    assert_eq!(
        compile(&cue("{meN-|-kan}")).unwrap().slots,
        [Slot::Affix(vec!["men-".into(), "-kan".into()])]
    );
    for bad in ["… sudah", "sudah …", "a … … b", "{sudah}", "", "sudah 2"] {
        assert!(compile(&cue(bad)).is_err(), "{bad}");
    }
    let mut verb = cue("mau");
    verb.class = Some("verb".into());
    assert_eq!(compile(&verb).unwrap().role, Role::Class(WordClass::Verb));
    verb.class = Some("adverb".into());
    assert!(compile(&verb).is_err());
}

#[test]
fn frames_skip_words_but_not_sentences() {
    let lib = library();
    let engine = Engine::default();
    let got = spot(&lib, &engine, "Rumah ini lebih besar daripada rumah itu.", Lang::Indonesian);
    assert!(got.contains(&("id.comparison.lebih".into(), "lebih … daripada".into())), "{got:?}");
    // The short cue `lebih` is covered by the frame.
    assert!(!got.contains(&("id.comparison.lebih".into(), "lebih".into())), "{got:?}");
    let got = spot(&lib, &engine, "Dia lebih suka teh.", Lang::Indonesian);
    assert!(got.contains(&("id.comparison.lebih".into(), "lebih".into())), "{got:?}");
}

#[test]
fn chat_spellings_are_hits_with_their_full_form() {
    let lib = library();
    let engine = Engine::default();
    let tokens = engine.analyze("udh makan blm?");
    let words: Vec<_> = tokens.iter().enumerate().filter(|(_, t)| t.token.is_wordlike()).collect();
    let hits = lib.spot(&words, Lang::Indonesian);
    let chat: Vec<_> = hits.iter().filter(|h| h.grammar == "id.chat.abbreviations").collect();
    assert!(chat.iter().any(|h| h.expands_to.as_deref() == Some("belum")), "{hits:?}");
    // The expanded form also meets its grammar: blm → belum.
    assert!(hits.iter().any(|h| h.grammar == "id.aspect.markers" && h.cue.is_some()), "{hits:?}");
    assert!(lib.cue_note(chat[0]).is_none());
}

#[test]
fn notes_come_from_the_entry() {
    let lib = library();
    let engine = Engine::default();
    let tokens = engine.analyze("Saya sudah makan.");
    let words: Vec<_> = tokens.iter().enumerate().filter(|(_, t)| t.token.is_wordlike()).collect();
    let hits = lib.spot(&words, Lang::Indonesian);
    let sudah = hits.iter().find(|h| h.grammar == "id.aspect.markers").unwrap();
    assert_eq!(sudah.tokens, [1]);
    assert!(lib.cue_note(sudah).unwrap().ar_eg.contains("خلاص"));
}

const CURRICULUM: &str = include_str!("../../../../../curriculum/indonesian.yaml");

#[test]
fn standing_prefers_what_the_learner_has_done() {
    let lib = library();
    let engine = Engine::default();
    let cur = parse_curriculum(CURRICULUM).unwrap();
    let text = "Kami membicarakan masalah itu, tetapi rumahnya lebih besar daripada rumah kami.";
    let tokens = engine.analyze(text);
    let words: Vec<_> = tokens.iter().enumerate().filter(|(_, t)| t.token.is_wordlike()).collect();
    let hits = lib.spot(&words, Lang::Indonesian);

    // Nothing to compare with: every point is Unknown, easiest level first.
    let found = lib.found(&hits, &Syllabus::default());
    assert!(found.iter().all(|f| f.standing == Standing::Unknown));
    assert_eq!(found[0].level, "bipa1");

    // A learner who finished the first unit of BIPA 1: personal pronouns are
    // learned, other BIPA 1 points are at their level, BIPA 2 comparison is ahead.
    let unit1: std::collections::BTreeSet<String> =
        cur.levels[0].units[0].lessons.iter().map(|l| l.id.clone()).collect();
    let mut syl = Syllabus::default();
    syl.add(&cur, &Progress::new(&cur, &unit1));
    assert_eq!(syl.level("id"), Some("bipa1"));
    let found = lib.found(&hits, &syl);
    let get = |id: &str| found.iter().find(|f| f.grammar == id).unwrap_or_else(|| panic!("{id}: {found:#?}"));
    let pron = get("id.pronoun.personal");
    assert_eq!(pron.standing, Standing::Learned);
    assert_eq!(pron.place.as_ref().unwrap().lesson, "bipa1.u01.l04");
    assert_eq!(get("id.conjunction.basic").standing, Standing::AtLevel);
    let lebih = get("id.comparison.lebih");
    assert_eq!(lebih.standing, Standing::Ahead);
    assert_eq!(lebih.place.as_ref().unwrap().level, "bipa2");
    assert_eq!(found[0].standing, Standing::Learned);
    assert!(found.windows(2).all(|w| w[0].standing <= w[1].standing), "{found:#?}");
}

#[derive(Deserialize)]
struct Golden {
    text: String,
    lang: String,
    /// Every hit, as `grammar id: matched words`.
    hits: Vec<String>,
}

/// Spec §24 texts and a few more, each with every hit it should give: a new
/// hit or a lost one fails until the file is updated on purpose.
#[test]
fn golden_spots() {
    let lib = library();
    let engine = Engine::default();
    let text = std::fs::read_to_string(repo().join("fixtures/grammar/spot.yaml")).unwrap();
    let golden: Vec<Golden> = serde_yaml_ng::from_str(&text).unwrap();
    assert!(golden.len() >= 20);
    let mut wrong = Vec::new();
    for g in &golden {
        let lang = if g.lang == "jv" { Lang::Javanese } else { Lang::Indonesian };
        let got: Vec<String> =
            spot(&lib, &engine, &g.text, lang).into_iter().map(|(id, w)| format!("{id}: {w}")).collect();
        let mut want = g.hits.clone();
        let mut have = got.clone();
        want.sort();
        have.sort();
        if want != have {
            wrong.push(format!("{}\n  want {want:?}\n  got  {have:?}", g.text));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// An entry's cues should find it in most of its own examples.
#[test]
fn entries_spot_their_own_examples() {
    let lib = library();
    let engine = Engine::default();
    let (mut hit, mut total) = (0, 0);
    let mut misses = Vec::new();
    for (id, point) in &lib.points {
        if point.cues.is_empty() {
            continue;
        }
        let lang = if point.language == "jv" { Lang::Javanese } else { Lang::Indonesian };
        for ex in &point.examples {
            total += 1;
            if spot(&lib, &engine, &ex.text, lang).iter().any(|(g, _)| g == id) {
                hit += 1;
            } else {
                misses.push(format!("{id}: {}", ex.text));
            }
        }
    }
    let recall = hit as f64 / total as f64;
    eprintln!("own-example recall {hit}/{total} = {recall:.2}\n{}", misses.join("\n"));
    assert!(recall >= 0.7, "recall {recall:.2}");
}
