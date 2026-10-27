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
    // Only bipa2.u06 is pending, and the fake radio episode below covers it.
    let pending =
        json!({"schema": "kabar.pending/v1", "radio": ["bipa2.u06"], "sinetron": [], "personas": []});
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

/// Every character in the cast has a portrait entry (`assets/portraits.yaml`,
/// keyed by cast id), no entry names someone outside the cast, and every
/// portrait master (`assets/src/personas/*.webp`) belongs to an entry. The learner
/// has no portrait.
#[test]
fn every_character_has_a_portrait_entry() {
    use kabar_content::yaml_value;
    use std::collections::BTreeSet;

    const LEARNER: &str = "mohamed";
    const EXPRESSIONS: [&str; 4] = ["listening", "laughing", "thinking", "explaining"];
    let ids = |v: &serde_json::Value, key: &str| -> Vec<(String, bool)> {
        v[key]
            .as_array()
            .unwrap_or_else(|| panic!("{key} is a list"))
            .iter()
            .map(|c| (c["id"].as_str().expect("id").to_string(), c["expressions"].as_bool().unwrap_or(false)))
            .collect()
    };
    let cast: BTreeSet<String> =
        ids(&yaml_value(&root().join("content/cast.yaml")).expect("cast.yaml"), "characters")
            .into_iter()
            .map(|(id, _)| id)
            .collect();
    let portraits = yaml_value(&root().join("assets/portraits.yaml")).expect("portraits.yaml");
    assert_eq!(portraits["schema"], "kabar.portraits/v1");
    let entries = ids(&portraits, "portraits");
    let mut problems = Vec::new();
    let listed: BTreeSet<&str> = entries.iter().map(|(id, _)| id.as_str()).collect();
    if listed.len() != entries.len() {
        problems.push("an id is listed twice".to_string());
    }
    for id in &cast {
        if id != LEARNER && !listed.contains(id.as_str()) {
            problems.push(format!("{id}: in content/cast.yaml but has no portrait entry"));
        }
    }
    for id in &listed {
        if !cast.contains(*id) || *id == LEARNER {
            problems.push(format!("{id}: portrait entry for someone who isn't in the cast"));
        }
    }
    if let Ok(dir) = std::fs::read_dir(root().join("assets/src/personas")) {
        for file in dir.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".webp") else { continue };
            if stem == "cast-sheet" {
                continue;
            }
            let (id, expression) = match EXPRESSIONS.iter().find_map(|e| stem.strip_suffix(&format!("-{e}")))
            {
                Some(id) => (id, true),
                None => (stem, false),
            };
            match entries.iter().find(|(e, _)| e == id) {
                None => problems.push(format!("{name}: no portrait entry for {id}")),
                Some((_, false)) if expression => problems.push(format!("{name}: {id} has no expressions")),
                _ => {}
            }
        }
    }
    assert!(problems.is_empty(), "assets/portraits.yaml:\n{}", problems.join("\n"));
}
