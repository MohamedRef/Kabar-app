//! Validates every curriculum and grammar file in the repository.

use std::path::Path;

use kabar_content::{check_repo, planned_grammar};

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

#[test]
fn all_content_is_valid() {
    let report = check_repo(root()).expect("content files load");
    assert!(
        report.problems.is_empty(),
        "{} content problem(s):\n{}",
        report.problems.len(),
        report.problems.join("\n")
    );
}

#[test]
fn both_curricula_are_present_and_complete() {
    let report = check_repo(root()).expect("content files load");
    let id = report.curricula.iter().find(|c| c.language == "id");
    let jv = report.curricula.iter().find(|c| c.language == "jv");
    let (id, jv) = (id.expect("indonesian.yaml"), jv.expect("javanese.yaml"));

    let codes: Vec<&str> = id.levels.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(codes, ["bipa1", "bipa2", "bipa3", "bipa4", "bipa5", "bipa6", "bipa7"]);
    let codes: Vec<&str> = jv.levels.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(codes, ["j1", "j2", "j3", "j4", "j5", "j6"]);

    // Javanese unlocks once BIPA 1 is done (SPEC §21).
    assert_eq!(jv.levels[0].requires, ["bipa1"]);
}

#[test]
fn bipa_1_and_2_grammar_is_fully_written() {
    let report = check_repo(root()).expect("content files load");
    let planned = planned_grammar(&report);
    assert!(!planned.contains_key("bipa1"), "{:?}", planned.get("bipa1"));
    assert!(!planned.contains_key("bipa2"), "{:?}", planned.get("bipa2"));
}

#[test]
fn grammar_examples_use_spec_test_texts() {
    // SPEC §24 morphology sentences must appear somewhere in the library.
    let report = check_repo(root()).expect("content files load");
    let texts: Vec<&str> =
        report.grammar.iter().flat_map(|g| g.examples.iter().map(|e| e.text.as_str())).collect();
    for wanted in ["Saya sedang membaca buku.", "Anak-anak bermain di taman.", "Apa kabar?"] {
        assert!(texts.contains(&wanted), "missing example: {wanted}");
    }
}
