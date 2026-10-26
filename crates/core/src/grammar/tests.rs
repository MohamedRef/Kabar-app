use std::path::PathBuf;

use super::*;
use crate::lang::analyze::Engine;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn library() -> Library {
    let mut files: Vec<(String, String)> = Vec::new();
    for lang in ["id", "jv"] {
        for e in std::fs::read_dir(repo().join("content/grammar").join(lang)).unwrap() {
            let path = e.unwrap().path();
            if path.extension().is_some_and(|x| x == "yaml") {
                files.push((path.display().to_string(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let (lib, problems) = Library::from_yaml(files.iter().map(|(p, t)| (p.as_str(), t.as_str())));
    assert!(problems.is_empty(), "{problems:#?}");
    lib
}

#[test]
fn every_grammar_file_loads() {
    let lib = library();
    assert!(lib.len() > 100);
    let ada = &lib.cues["id.ada"];
    assert!(ada.words.contains("ada"));
    assert!(ada.phrases.contains(&vec!["tidak".to_string(), "ada".to_string()]));
}

#[test]
fn affix_forms() {
    assert_eq!(affix_set("meN-…-kan").unwrap(), ["men-", "-kan"]);
    assert_eq!(affix_set("-nya").unwrap(), ["-nya"]);
    assert_eq!(affix_set("ke-an").unwrap(), ["ke-", "-an"]);
    assert_eq!(canonical_affix("mem-"), "men-");
    assert!(affix_set("anak-anak").is_none());
}

fn ids(lib: &Library, engine: &Engine, text: &str, lang: Lang) -> Vec<String> {
    lib.candidates(&engine.analyze(text), lang, MAX_CANDIDATES).into_iter().map(|c| c.id).collect()
}

#[test]
fn spec_24_sentences_find_their_grammar() {
    let lib = library();
    let engine = Engine::default();
    let c = ids(&lib, &engine, "Saya sedang membaca buku.", Lang::Indonesian);
    assert!(c.contains(&"id.aspect.markers".to_string()), "{c:?}");
    assert!(c.iter().any(|id| id.starts_with("id.prefix.men")), "{c:?}");
    let c = ids(&lib, &engine, "Rumahnya dibersihkan kemarin.", Lang::Indonesian);
    assert!(c.contains(&"id.clitic.nya".to_string()), "{c:?}");
}

#[derive(Deserialize)]
struct Golden {
    items: Vec<GoldenItem>,
}

#[derive(Deserialize)]
struct GoldenItem {
    id: String,
    input: GoldenInput,
    expect: GoldenExpect,
}

#[derive(Deserialize)]
struct GoldenInput {
    text: String,
    lang: String,
}

#[derive(Deserialize)]
struct GoldenExpect {
    grammar_ids: Vec<String>,
}

/// The on-device pick should already contain most of what a good answer
/// uses, so the model chooses among the right entries.
#[test]
fn golden_set_recall() {
    let lib = library();
    let engine = Engine::default();
    let text = std::fs::read_to_string(repo().join("content/golden/explain_sentence.yaml")).unwrap();
    let golden: Golden = serde_yaml_ng::from_str(&text).unwrap();
    let (mut hit, mut total) = (0, 0);
    let mut misses = Vec::new();
    for item in &golden.items {
        let lang = if item.input.lang == "jv" { Lang::Javanese } else { Lang::Indonesian };
        let got = ids(&lib, &engine, &item.input.text, lang);
        for want in item.expect.grammar_ids.iter().filter(|id| lib.get(id).is_some()) {
            total += 1;
            if got.contains(want) {
                hit += 1;
            } else {
                misses.push(format!("{}: {want} (got {got:?}) — {}", item.id, item.input.text));
            }
        }
    }
    let recall = hit as f64 / total.max(1) as f64;
    eprintln!("explain_sentence recall {hit}/{total} = {recall:.2}\n{}", misses.join("\n"));
    assert!(total > 20);
    assert!(recall >= 0.7, "recall {recall:.2}");
}

#[test]
fn explain_answer_checks() {
    let cands = vec!["id.aspect.markers".to_string(), "id.prefix.men.basic".to_string()];
    let ok: ExplainSentence = serde_json::from_str(include_str!(
        "../../../../fixtures/ai/explain_sentence.saya_sedang_membaca_buku.workers_ai.json"
    ))
    .unwrap();
    ok.validate().unwrap();
    ok.check(&cands, Voice::Arz).unwrap();
    let mut bad = ok.clone();
    bad.grammar_ids.push("id.made.up".into());
    assert!(bad.check(&cands, Voice::Arz).is_err());
    bad.keep_known(&cands);
    assert_eq!(bad.grammar_ids, ok.grammar_ids);
    assert!(ok.check(&cands, Voice::En).is_ok());
    let mut latin = ok.clone();
    latin.paraphrase = "Ana ba2ra ktab".into();
    assert!(latin.check(&cands, Voice::Arz).is_err());
    let mut reg = ok;
    reg.register = "casual".into();
    assert!(reg.validate().is_err());
}

#[test]
fn input_matches_the_golden_shape() {
    let c = vec![Candidate { id: "id.ada".into(), score: 3, matched: vec!["ada".into()] }];
    let v = explain_input("Ada apa?", Lang::Indonesian, Voice::Arz, &c);
    assert_eq!(
        v,
        json!({"text": "Ada apa?", "lang": "id", "voice": "arz", "grammar_candidates": ["id.ada"]})
    );
}
