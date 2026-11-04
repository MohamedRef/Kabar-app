//! Each content check names the mistake it catches: a committed file passes,
//! and one hand-made slip at a time is reported with its message.

use std::path::Path;
use std::sync::OnceLock;

use kabar_content::lessons::{Index, check_lesson, check_persona, check_radio, check_story, check_test};
use kabar_content::{Report, check_repo, yaml_value};
use serde_json::{Value, json};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap()
}

fn report() -> &'static Report {
    static REPORT: OnceLock<Report> = OnceLock::new();
    REPORT.get_or_init(|| check_repo(root()).expect("content files load"))
}

fn index() -> Index<'static> {
    let r = report();
    Index::new(&r.curricula, r.grammar.iter().map(|g| g.id.as_str()))
}

fn load(rel: &str) -> Value {
    yaml_value(&root().join(rel)).unwrap()
}

type Mutation = fn(&mut Value);
type Checker<'a> = dyn Fn(&Value) -> Vec<String> + 'a;

/// The committed file passes; each mutation is caught with its message.
fn expect_each(good: &Value, check: &Checker<'_>, broken: &[(&str, Mutation)]) {
    assert_eq!(check(good), Vec::<String>::new(), "the committed file passes");
    for (expected, mutate) in broken {
        let mut v = good.clone();
        mutate(&mut v);
        let found = check(&v);
        assert!(found.iter().any(|p| p.contains(expected)), "expected '{expected}', got {found:#?}");
    }
}

#[test]
fn lesson_mistakes_are_named() {
    let index = index();
    let check = |v: &Value| {
        let mut out = Vec::new();
        check_lesson("t", "bipa1.u01.l01", "id", v, &index, &mut out);
        out
    };
    let good = load("content/lessons/id/bipa1.u01.l01.yaml");
    expect_each(
        &good,
        &check,
        &[
            ("schema must be kabar.lesson/v1", |v| v["schema"] = "kabar.lesson/v0".into()),
            ("id must equal the file name", |v| v["id"] = "bipa1.u01.l02".into()),
            ("language must be 'id'", |v| v["language"] = "jv".into()),
            ("exercises[0].id must be e01", |v| v["exercises"][0] = "e01".into()),
            ("unknown exercise type 'dance'", |v| v["exercises"][0]["type"] = "dance".into()),
            ("pairs: need at least 3", |v| {
                let pairs = v["exercises"][0]["pairs"].as_array_mut().unwrap();
                pairs.truncate(2);
            }),
            ("duplicate entries", |v| {
                v["exercises"][1]["options"][1] = v["exercises"][1]["options"][0].clone()
            }),
            ("needs question or question_meaning", |v| {
                v["exercises"][1].as_object_mut().unwrap().remove("question_meaning");
            }),
            ("expected a list of at least 2 non-empty strings", |v| {
                v["exercises"][1]["options"] = json!(["x"])
            }),
            ("exercises[1].id must be e02", |v| v["exercises"][1]["id"] = "e09".into()),
            ("options[answer] must equal text", |v| v["exercises"][3]["text"] = "Selamat pagi".into()),
            ("a comprehension question needs options", |v| {
                v["exercises"][3]["question"] = json!({"ar_eg": "إيه؟", "ar": "ماذا؟", "en": "What?"});
                v["exercises"][3].as_object_mut().unwrap().remove("options");
            }),
            ("answer without options", |v| {
                v["exercises"][3].as_object_mut().unwrap().remove("options");
            }),
            ("answers must be among options", |v| v["exercises"][2]["answers"] = json!(["siang hari"])),
            ("answer should have at least 3 words", |v| v["exercises"][4]["answer"] = "Selamat pagi".into()),
            ("distractor 'pagi' is part of the answer", |v| {
                v["exercises"][4]["distractors"] = json!(["pagi"])
            }),
            ("alternative 'Bu, selamat malam!' must use the answer's tiles", |v| {
                v["exercises"][4]["alternatives"] = json!(["Bu, selamat malam!"])
            }),
            ("expected non-empty text", |v| v["exercises"][5]["text"] = " ".into()),
            ("direction must be to_target or from_target", |v| {
                v["exercises"][5] = json!({"id": "e06", "type": "translate", "direction": "sideways"})
            }),
            ("to_target must not have source_text", |v| {
                v["exercises"][5] = json!({"id": "e06", "type": "translate", "direction": "to_target",
                    "source_meaning": {"ar_eg": "صباح الخير", "ar": "صباح الخير", "en": "Good morning"},
                    "answers": ["Selamat pagi"], "source_text": "x"})
            }),
            ("answers_meaning.ar: answers must be Arabic", |v| {
                v["exercises"][5] = json!({"id": "e06", "type": "translate", "direction": "from_target",
                    "source_text": "Selamat pagi", "answers": ["x"],
                    "answers_meaning": {"ar_eg": ["صباح الخير"], "ar": ["good morning"], "en": ["Good morning"]}})
            }),
            ("from_target must not have answers", |v| {
                v["exercises"][5] = json!({"id": "e06", "type": "translate", "direction": "from_target",
                    "source_text": "Selamat pagi", "answers": ["x"],
                    "answers_meaning": {"ar_eg": ["صباح الخير"], "ar": ["صباح الخير"], "en": ["Good morning"]}})
            }),
            ("review.status must be draft, approved or needs-changes", |v| {
                v["review"]["status"] = "done".into()
            }),
            ("review.verified must be true or false", |v| v["review"]["verified"] = "yes".into()),
            ("at most", |v| {
                let ex = v["slides"][0]["examples"][0].clone();
                v["slides"][0]["examples"] = Value::Array(vec![ex; 12]);
            }),
            ("grammar: id.no.such.point is not in the grammar library", |v| {
                v["slides"][0]["grammar"] = "id.no.such.point".into()
            }),
            ("missing dialogue", |v| {
                v.as_object_mut().unwrap().remove("dialogue");
            }),
            ("use at least", |v| {
                let first = v["exercises"][1].clone();
                let all = v["exercises"].as_array_mut().unwrap();
                for (i, e) in all.iter_mut().enumerate() {
                    let mut same = first.clone();
                    same["id"] = format!("e{:02}", i + 1).into();
                    *e = same;
                }
            }),
            ("game must look like uu.<scene> or mp.<pair>", |v| v["game"] = "pasar".into()),
        ],
    );
    // A lesson the curriculum doesn't list stops the checks early.
    let mut out = Vec::new();
    check_lesson(
        "t",
        "bipa1.u01.l77",
        "id",
        &json!({"schema": "kabar.lesson/v1", "id": "bipa1.u01.l77"}),
        &index,
        &mut out,
    );
    assert!(out.iter().any(|p| p.contains("lesson id is not in the curriculum")), "{out:#?}");
    let mut out = Vec::new();
    check_lesson("t", "bipa1.u01.l01", "id", &json!("not a mapping"), &index, &mut out);
    assert_eq!(out, ["t: lesson: expected a mapping"]);
}

#[test]
fn level_and_placement_test_mistakes_are_named() {
    let index = index();
    let level = |v: &Value| {
        let mut out = Vec::new();
        check_test("t", "bipa1.test", v, &index, &mut out);
        out
    };
    let good = load("content/tests/id/bipa1.test.yaml");
    expect_each(
        &good,
        &level,
        &[
            ("schema must be kabar.test/v1", |v| v["schema"] = "x".into()),
            ("id must equal the file name", |v| v["id"] = "bipa2.test".into()),
            ("language must be id or jv", |v| v["language"] = "ms".into()),
            ("kind must be level or placement", |v| v["kind"] = "quiz".into()),
            ("minutes must be 5-90", |v| v["minutes"] = 200.into()),
            ("a test needs sections", |v| v["sections"] = json!([])),
            ("skill must be one of", |v| v["sections"][0]["skill"] = "juggling".into()),
            ("item ids must be present and unique", |v| {
                v["sections"][0]["items"][1]["id"] = v["sections"][0]["items"][0]["id"].clone()
            }),
            ("needs the unit it tests", |v| {
                v["sections"][0]["items"][0].as_object_mut().unwrap().remove("unit");
            }),
            ("unit: bipa9.u01 is not a curriculum unit", |v| {
                v["sections"][0]["items"][0]["unit"] = "bipa9.u01".into()
            }),
            ("is outside the item's level", |v| v["sections"][0]["items"][0]["unit"] = "bipa2.u01".into()),
            ("rubric: expected a list of at least 2", |v| {
                let item = v["sections"][0]["items"][0].as_object_mut().unwrap();
                item.insert("type".into(), "writing_task".into());
                item.insert("prompt".into(), json!({"ar_eg": "اكتب", "ar": "اكتب", "en": "Write"}));
                item.insert("rubric".into(), json!(["one"]));
                item.insert("sample".into(), "Halo.".into());
            }),
            ("grammar: id.no.such is not in the grammar library", |v| {
                v["sections"][0]["items"][0]["grammar"] = json!(["id.no.such"])
            }),
        ],
    );
    let mut out = Vec::new();
    check_test("t", "bipa1.quiz", &good, &index, &mut out);
    assert!(out.iter().any(|p| p.contains("a level test must be named <level>.test")), "{out:#?}");

    let placement = load("content/tests/id/placement.yaml");
    let check = |v: &Value| {
        let mut out = Vec::new();
        check_test("t", "placement", v, &index, &mut out);
        out
    };
    expect_each(
        &placement,
        &check,
        &[("placement items need a level", |v| {
            for s in v["sections"].as_array_mut().unwrap() {
                s.as_object_mut().unwrap().remove("level");
                for i in s["items"].as_array_mut().unwrap() {
                    i.as_object_mut().unwrap().remove("level");
                }
            }
        })],
    );
}

#[test]
fn story_mistakes_are_named() {
    let index = index();
    let check = |v: &Value| {
        let mut out = Vec::new();
        check_story("t", "sinetron.e01", v, &index, &mut out);
        out
    };
    let good = load("content/stories/sinetron/sinetron.e01.yaml");
    expect_each(
        &good,
        &check,
        &[
            ("series must be sinetron", |v| v["series"] = "drama".into()),
            ("id must be sinetron.eNN", |v| v["episode"] = 2.into()),
            ("unlock_after must be a curriculum unit id", |v| v["unlock_after"] = "bipa9.u01".into()),
            ("level: must be a curriculum level id", |v| v["level"] = "bipa9".into()),
            ("an episode has 1-4 scenes", |v| v["scenes"] = json!([])),
            ("an episode needs at least 8 lines", |v| {
                v["scenes"][0]["lines"].as_array_mut().unwrap().truncate(1);
                v["scenes"].as_array_mut().unwrap().truncate(1);
            }),
            ("must be one of", |v| v["scenes"][0]["lines"][0]["speaker"] = "Zorro".into()),
            ("is read aloud: put Javanese or Arabic script in the note", |v| {
                v["scenes"][0]["lines"][0]["text"] = "مرحبا".into()
            }),
            ("needs an arabic source too", |v| {
                let w = v["new_words"][0].as_object_mut().unwrap();
                w.remove("arabic");
                w.insert("bridge".into(), "selamat".into());
            }),
            ("an episode has 3-6 questions", |v| v["questions"].as_array_mut().unwrap().truncate(1)),
            ("questions[1].id must be q02", |v| v["questions"][1]["id"] = "q09".into()),
        ],
    );
    let mut out = Vec::new();
    check_story("t", "sinetron.e01", &json!([]), &index, &mut out);
    assert_eq!(out, ["t: story: expected a mapping"]);
}

#[test]
fn radio_mistakes_are_named() {
    let index = index();
    let check = |v: &Value| {
        let mut out = Vec::new();
        check_radio("t", "radio.bipa1.u01", v, &index, &mut out);
        out
    };
    let good = load("content/radio/id/radio.bipa1.u01.yaml");
    expect_each(
        &good,
        &check,
        &[
            ("unit must be a curriculum unit id", |v| v["unit"] = "bipa9.u01".into()),
            ("id must be radio.<unit> and equal the file name", |v| v["unit"] = "bipa1.u02".into()),
            ("a radio episode has exactly 2 hosts", |v| v["hosts"].as_array_mut().unwrap().truncate(1)),
            ("hosts[0].voice must be one of", |v| v["hosts"][0]["voice"] = "robot".into()),
            ("minutes must be 3-8", |v| v["minutes"] = 20.into()),
            ("kind 'ads' is not a segment kind", |v| v["segments"][0]["kind"] = "ads".into()),
            ("missing a intro segment", |v| v["segments"][0]["kind"] = "outro".into()),
            ("spoken words is too short for an episode", |v| {
                for s in v["segments"].as_array_mut().unwrap() {
                    s["lines"].as_array_mut().unwrap().truncate(1);
                }
            }),
        ],
    );
    let mut out = Vec::new();
    check_radio("t", "radio.bipa1.u01", &json!(1), &index, &mut out);
    assert_eq!(out, ["t: radio: expected a mapping"]);
}

#[test]
fn persona_mistakes_are_named() {
    let index = index();
    let check = |v: &Value| {
        let mut out = Vec::new();
        check_persona("t", "bu-siti", v, &index, &mut out);
        out
    };
    let good = load("content/personas/bu-siti.yaml");
    expect_each(
        &good,
        &check,
        &[
            ("units: 'bipa9.u01' is not a curriculum unit id", |v| v["units"] = json!(["bipa9.u01"])),
            ("id must equal the file name", |v| v["id"] = "bu-sita".into()),
            ("language must be id or jv", |v| v["language"] = "ms".into()),
            ("voice must be one of", |v| v["voice"] = "robot".into()),
            ("style.levels needs at least 2 bands", |v| {
                v["style"]["levels"].as_array_mut().unwrap().truncate(1)
            }),
            ("openers needs at least 3", |v| v["openers"].as_array_mut().unwrap().truncate(1)),
            ("example_dialogues needs at least 4", |v| {
                v["example_dialogues"].as_array_mut().unwrap().truncate(1)
            }),
            ("example_dialogues[0].id must be d01", |v| v["example_dialogues"][0]["id"] = "d07".into()),
            ("needs at least 6 lines", |v| {
                v["example_dialogues"][0]["lines"].as_array_mut().unwrap().truncate(2)
            }),
            ("the first two dialogues must show an inline correction", |v| {
                for l in v["example_dialogues"][0]["lines"].as_array_mut().unwrap() {
                    l.as_object_mut().unwrap().remove("correction");
                }
            }),
            ("Javanese stays unverified until approved", |v| {
                v["language"] = "jv".into();
                v["review"]["verified"] = true.into();
            }),
        ],
    );
    let mut out = Vec::new();
    check_persona("t", "bu-siti", &json!(null), &index, &mut out);
    assert_eq!(out, ["t: persona: expected a mapping"]);
}

/// The first unit of the Indonesian track, and its lessons by id.
fn first_unit() -> (Vec<kabar_content::model::Curriculum>, std::collections::BTreeMap<String, Value>) {
    let mut c = report().curricula.iter().find(|c| c.language == "id").unwrap().clone();
    c.levels.truncate(1);
    c.levels[0].units.truncate(1);
    let lessons = c.levels[0].units[0]
        .lessons
        .iter()
        .map(|l| (l.id.clone(), load(&format!("content/lessons/id/{}.yaml", l.id))))
        .collect();
    (vec![c], lessons)
}

#[test]
fn a_unit_runs_learn_then_practice_then_review() {
    use kabar_content::unit_shape::check;
    let (curricula, lessons) = first_unit();
    let problems = |c: &[kabar_content::model::Curriculum], l: &std::collections::BTreeMap<String, Value>| {
        let mut out = Vec::new();
        check(c, l, &mut out);
        out
    };
    assert_eq!(problems(&curricula, &lessons), Vec::<String>::new());

    let mut late = curricula.clone();
    let unit = &mut late[0].levels[0].units[0];
    let learn = unit.lessons.remove(0);
    unit.lessons.insert(unit.lessons.len() - 1, learn);
    let found = problems(&late, &lessons);
    assert!(
        found.iter().any(|p| p.contains("learn lesson bipa1.u01.l01 comes after the unit's practice lesson")),
        "{found:#?}"
    );

    let mut early = curricula.clone();
    early[0].levels[0].units[0].lessons.rotate_right(1);
    let found = problems(&early, &lessons);
    assert!(
        found.iter().any(|p| p.contains("the review lesson bipa1.u01.l07 must be the unit's last")),
        "{found:#?}"
    );

    let mut forgetful = lessons.clone();
    forgetful.get_mut("bipa1.u01.l07").unwrap()["exercises"] = json!([]);
    let found = problems(&curricula, &forgetful);
    assert!(found.iter().any(|p| p.contains("brings back none of bipa1.u01.l01's new words")), "{found:#?}");
}

#[test]
fn a_lesson_keeps_one_semantic_set_small() {
    use kabar_content::semantic_sets::{FILE, check};
    let (curricula, lessons) = first_unit();
    let dir = tempfile::tempdir().unwrap();
    let problems = |yaml: &str| {
        std::fs::create_dir_all(dir.path().join("content")).unwrap();
        std::fs::write(dir.path().join(FILE), yaml).unwrap();
        let mut out = Vec::new();
        check(dir.path(), &curricula, &lessons, &mut out);
        out
    };
    let mut none = Vec::new();
    check(Path::new("/nowhere"), &curricula, &lessons, &mut none);
    assert!(none.is_empty(), "no list, nothing to check");

    let sets = "sets:\n  times:\n    id: [pagi, siang, sore, malam]\n";
    let found = problems(&format!("max_per_lesson: 2\n{sets}"));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("bipa1.u01.l01: teaches 4 times words (pagi, siang, sore, malam)"),
        "{found:#?}"
    );
    assert!(problems(&format!("max_per_lesson: 4\n{sets}")).is_empty());
    assert!(
        problems(&format!("max_per_lesson: 2\n{sets}allow:\n  bipa1.u01.l01: one greeting set\n")).is_empty()
    );
    let found = problems(&format!("max_per_lesson: 4\n{sets}allow:\n  bipa9.u01.l01: gone\n"));
    assert_eq!(found, [format!("{FILE}: allow names bipa9.u01.l01, which is not a lesson")]);
    assert_eq!(
        problems(&format!("max_per_lesson: lots\n{sets}")),
        [format!("{FILE}: max_per_lesson must be a number")]
    );
    assert!(problems("sets: [").iter().any(|p| p.starts_with(FILE)), "bad YAML is named");
}

/// What the cast check adds for a change, on top of what it says anyway
/// for the (empty) appearances these tests start from.
fn cast_problems(
    cast: Option<&Value>,
    pending: Option<&Value>,
    personas: &[(String, Value)],
    seen: &kabar_content::cast::Appearances,
) -> Vec<String> {
    let index = index();
    let mut out = Vec::new();
    kabar_content::cast::check(cast, pending, personas, seen, &index, &mut out);
    out
}

#[test]
fn cast_registry_mistakes_are_named() {
    use kabar_content::cast::Appearances;
    let cast = load("content/cast.yaml");
    let pending = load("content/pending.yaml");
    let seen = Appearances::default();
    let base = cast_problems(Some(&cast), Some(&pending), &[], &seen);
    let added = |mutate: &dyn Fn(&mut Value)| {
        let mut v = cast.clone();
        mutate(&mut v);
        let found = cast_problems(Some(&v), Some(&pending), &[], &seen);
        found.into_iter().filter(|p| !base.contains(p)).collect::<Vec<_>>()
    };
    let has = |found: &[String], expected: &str| {
        assert!(found.iter().any(|p| p.contains(expected)), "expected '{expected}', got {found:#?}");
    };
    has(
        &added(&|v| v["characters"][1]["family"] = json!(["nobody"])),
        "family 'nobody' is not a character id",
    );
    has(&added(&|v| v["characters"][1]["id"] = "Bad Id".into()), "id must be lower-case kebab-case");
    has(&added(&|v| v["characters"][1]["in"] = json!(["podcast"])), "in: 'podcast' is not one of");
    has(
        &added(&|v| {
            let ch = v["characters"][1].as_object_mut().unwrap();
            ch.remove("units");
            ch.remove("levels");
        }),
        "needs `units` or `levels`",
    );
    has(
        &added(&|v| v["characters"][1]["units"] = json!(["bipa9.u01"])),
        "units: 'bipa9.u01' is not a curriculum unit id",
    );
    has(
        &added(&|v| v["characters"][1]["levels"] = json!(["bipa9"])),
        "levels: 'bipa9' is not a curriculum level id",
    );
    let twin = added(&|v| {
        let copy = v["characters"][1].clone();
        v["characters"].as_array_mut().unwrap().push(copy);
    });
    has(&twin, "duplicate id");
    has(&twin, "names both");
    has(
        &added(&|v| {
            let mut other = v["characters"][1].clone();
            other["id"] = "someone-else".into();
            other.as_object_mut().unwrap().remove("aliases");
            other.as_object_mut().unwrap().remove("family");
            let name = other["name"].as_str().unwrap().to_owned();
            other["name"] = format!("Pak {name}").into();
            v["characters"].as_array_mut().unwrap().push(other);
        }),
        "share the name",
    );
    let missing = cast_problems(None, Some(&pending), &[], &seen);
    assert_eq!(
        missing,
        ["content/cast.yaml: missing: every recurring character must be listed (docs/CAST.md)"]
    );
    let found = cast_problems(Some(&cast), None, &[], &seen);
    assert!(found.iter().any(|p| p.contains("missing (list the units still waiting")), "{found:#?}");
}

#[test]
fn cast_appearances_are_checked_against_the_registry() {
    use kabar_content::cast::Appearances;
    let cast = load("content/cast.yaml");
    let pending = load("content/pending.yaml");
    let base = cast_problems(Some(&cast), Some(&pending), &[], &Appearances::default());
    let character = &cast["characters"][1];
    let (id, name) = (character["id"].as_str().unwrap(), character["name"].as_str().unwrap());

    let mut seen = Appearances::default();
    // A registered character speaking in a program its entry doesn't list.
    seen.unggah_ungguh("uu.toml", &format!("[[scenes]]\nid = \"uu.pasar\"\npartner.name = \"{name}\"\n"));
    // An unlisted extra who keeps coming back.
    for u in 1..=4 {
        let line = json!({"lines": [{"speaker": "Pak Zorro", "text": "Halo."}]});
        seen.lesson("l.yaml", &format!("bipa2.u{u:02}.l01"), &line);
    }
    seen.test("t.yaml", &json!({"items": [{"speaker": "Pak Zorro"}]}));
    let personas = [
        ("ghost.yaml".to_string(), json!({"id": "ghost", "name": "Ghost"})),
        ("p.yaml".to_string(), json!({"id": id, "name": "Somebody Else", "units": ["bipa9.u99"]})),
    ];
    let found: Vec<String> = cast_problems(Some(&cast), Some(&pending), &personas, &seen)
        .into_iter()
        .filter(|p| !base.contains(p))
        .collect();
    for expected in [
        format!("{id}: add 'games' to `in`"),
        "'Pak Zorro' recurs (4 units, 0 Sinetron episodes, e.g. l.yaml)".into(),
        "ghost.yaml: persona 'ghost' must have a registry entry with the same id".into(),
        format!("p.yaml: name 'Somebody Else' differs from the registry's '{name}'"),
        format!("p.yaml: unit bipa9.u99 is missing from {id}'s registry units"),
        "game scene uu.pasar is not linked from any lesson".into(),
    ] {
        assert!(found.iter().any(|p| p.contains(&expected)), "expected '{expected}', got {found:#?}");
    }

    // Pending lists name real units, and are emptied as episodes arrive.
    let mut seen = Appearances::default();
    seen.radio("r.yaml", &json!({"unit": "bipa1.u01", "hosts": []}));
    seen.persona("p.yaml", &json!({"name": "X", "units": ["bipa1.u01"]}));
    let pending = json!({"schema": "kabar.pending/v1", "radio": ["bipa1.u01", "bipa9.u01"], "sinetron": [], "personas": ["bipa1"]});
    let found = cast_problems(Some(&cast), Some(&pending), &[], &seen);
    for expected in [
        "bipa1.u01 has its radio episode now; remove it from radio",
        "radio: 'bipa9.u01' is not a curriculum unit id",
        "bipa1 has a chat partner now; remove it from personas",
        "no persona practises a bipa2 unit",
    ] {
        assert!(found.iter().any(|p| p.contains(expected)), "expected '{expected}', got {found:#?}");
    }
}
