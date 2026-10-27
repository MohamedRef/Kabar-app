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

#[test]
fn bipa_1_and_2_lessons_and_tests_exist() {
    let report = check_repo(root()).expect("content files load");
    assert_eq!(report.lessons.iter().filter(|l| l.starts_with("bipa1.")).count(), 74);
    assert_eq!(report.lessons.iter().filter(|l| l.starts_with("bipa2.")).count(), 62);
    for test in ["bipa1.test", "bipa2.test", "placement"] {
        assert!(report.tests.iter().any(|t| t == test), "missing content/tests/id/{test}.yaml");
    }
}

/// The lesson checks catch the mistakes that are easy to make by hand.
#[test]
fn lesson_checks_reject_broken_lessons() {
    use kabar_content::lessons::{Index, check_lesson};
    use kabar_content::yaml_value;

    let report = check_repo(root()).expect("content files load");
    let index = Index::new(&report.curricula, report.grammar.iter().map(|g| g.id.as_str()));
    let file = root().join("content/lessons/id/bipa1.u01.l01.yaml");
    let good = yaml_value(&file).expect("reference lesson");
    let problems = |v: &serde_json::Value| {
        let mut out = Vec::new();
        check_lesson("t", "bipa1.u01.l01", "id", v, &index, &mut out);
        out
    };
    assert_eq!(problems(&good), Vec::<String>::new());

    type Mutation = fn(&mut serde_json::Value);
    let broken: [(&str, Mutation); 6] = [
        ("answer must be an index", |v| v["exercises"][1]["answer"] = 7.into()),
        ("exactly one ___", |v| v["exercises"][2]["sentence"] = "Selamat malam.".into()),
        ("unknown field 'answr'", |v| v["exercises"][2]["answr"] = 1.into()),
        ("must contain Arabic script", |v| v["can_do"]["ar_eg"] = "Greet people".into()),
        ("only the unit's last lesson", |v| {
            v["souvenir"] = serde_json::json!({"id": "x", "title": "x", "note": {}})
        }),
        ("title must equal", |v| v["title"] = "Selamat siang!".into()),
    ];
    for (expected, mutate) in broken {
        let mut v = good.clone();
        mutate(&mut v);
        let found = problems(&v);
        assert!(found.iter().any(|p| p.contains(expected)), "expected '{expected}', got {found:?}");
    }
}

#[test]
fn javanese_lessons_and_story_assets_exist() {
    let report = check_repo(root()).expect("content files load");
    for (level, n) in [("j1.", 36), ("j2.", 36), ("j3.", 42)] {
        assert_eq!(report.lessons.iter().filter(|l| l.starts_with(level)).count(), n, "{level} lessons");
    }
    for id in ["mas-dimas", "bu-siti", "mbak-gadis", "mas-ardi", "sinetron.e01", "radio.bipa1.u01"] {
        assert!(report.stories.iter().any(|s| s == id), "missing {id}");
    }
}

/// The cast checks catch a reused name, an unlisted host and a unit that
/// lost its episode.
#[test]
fn cast_checks_reject_orphans_and_name_clashes() {
    use kabar_content::cast::{Appearances, check};
    use kabar_content::lessons::Index;
    use kabar_content::yaml_value;
    use serde_json::json;

    let report = check_repo(root()).expect("content files load");
    let index = Index::new(&report.curricula, report.grammar.iter().map(|g| g.id.as_str()));
    let cast = yaml_value(&root().join("content/cast.yaml")).expect("cast.yaml");
    let pending = yaml_value(&root().join("content/pending.yaml")).expect("pending.yaml");
    let mut seen = Appearances::default();
    let line = |speaker: &str| json!({"lines": [{"speaker": speaker, "text": "Halo."}]});
    seen.lesson("a.yaml", "bipa3.u02.l01", &line("Mbak Siti"));
    seen.lesson("b.yaml", "bipa3.u02.l02", &line("Bu Siti"));
    seen.radio("c.yaml", &json!({"unit": "bipa2.u06", "hosts": [{"name": "Kak Bima"}]}));
    seen.story("d.yaml", &json!({"id": "sinetron.e01", "unlock_after": "bipa1.u02", "cast": ["Sari"]}));
    let mut out = Vec::new();
    check(Some(&cast), Some(&pending), &[], &seen, &index, &mut out);
    for expected in [
        "'Mbak Siti' (a.yaml) shares a name with bu-siti",
        "'Kak Bima' (radio, c.yaml) must be listed",
        "bipa2.u06 has its radio episode now",
        "bipa1.u01 has no radio episode",
        "bipa1.u01 has no sinetron episode",
    ] {
        assert!(out.iter().any(|p| p.contains(expected)), "expected '{expected}' in {out:#?}");
    }
    // The real Bu Siti is fine.
    assert!(!out.iter().any(|p| p.contains("'Bu Siti'")), "{out:#?}");
}
