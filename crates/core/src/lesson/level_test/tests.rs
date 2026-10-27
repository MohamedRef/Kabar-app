use super::*;

const EG: UiLanguage = UiLanguage::ArEg;

const LEVEL: &str = r#"
schema: kabar.test/v1
id: bipa1.test
language: id
kind: level
title: {ar_eg: اختبار, ar: اختبار, en: Test}
minutes: 30
pass_mark: 0.75
sections:
  - id: listening
    skill: listening
    title: {ar_eg: الاستماع, ar: الاستماع, en: Listening}
    items:
      - id: t01
        type: listen
        text: Saya sedang membaca buku.
        options: ["Saya sedang membaca buku.", "Saya sedang menulis buku."]
        answer: 0
      - id: t02
        type: listen
        text: Nama saya Rina. Saya tinggal di Jakarta.
        question: {ar_eg: رينا ساكنة فين؟, ar: أين تسكن رينا؟, en: Where does Rina live?}
        options: ["Bali", "Jakarta", "Kairo"]
        answer: 1
      - id: t03
        type: dictation
        text: Apa kabar?
        meaning: {ar_eg: إزيك؟, ar: كيف حالك؟, en: How are you?}
  - id: reading
    skill: reading
    title: {ar_eg: القراية, ar: القراءة, en: Reading}
    items:
      - id: t04
        skill: vocabulary
        type: match_pairs
        pairs:
          - {text: buku, ar_eg: كتاب, ar: كتاب, en: book}
          - {text: rumah, ar_eg: بيت, ar: بيت, en: house}
          - {text: taman, ar_eg: جنينة, ar: حديقة, en: park}
          - {text: anak, ar_eg: عيل, ar: طفل, en: child}
      - id: t05
        skill: grammar
        type: fill_blank
        sentence: Rumahnya ___ kemarin.
        meaning: {ar_eg: البيت اتنضف امبارح., ar: نُظّف البيت أمس., en: The house was cleaned yesterday.}
        options: [dibersihkan, membersihkan]
        answers: [dibersihkan]
      - id: t06
        type: translate
        direction: from_target
        source_text: Kesehatan itu penting.
        answers_meaning: {ar_eg: [الصحة مهمة], ar: [الصحة مهمة], en: [Health is important]}
  - id: writing
    skill: writing
    title: {ar_eg: الكتابة, ar: الكتابة, en: Writing}
    items:
      - id: t07
        points: 4
        type: writing_task
        prompt: {ar_eg: اكتب رسالة, ar: اكتب رسالة, en: Write a message}
        rubric: [Greets, Says where, Signs off]
        sample: Hai Dimas! Saya di Jakarta. Salam, Mohamed
      - id: t08
        type: speaking
        text: Anak-anak bermain di taman.
        meaning: {ar_eg: العيال بيلعبوا في الجنينة., ar: الأطفال يلعبون في الحديقة., en: The children play in the park.}
review: {status: draft, verified: false}
"#;

const PLACEMENT: &str = r#"
schema: kabar.test/v1
id: placement
language: id
kind: placement
title: {ar_eg: نبدأ منين؟, ar: من أين نبدأ؟, en: Where do we start?}
minutes: 10
sections:
  - id: bipa1
    level: bipa1
    skill: reading
    title: {ar_eg: ١, ar: ١, en: "1"}
    items:
      - {id: p1, type: multiple_choice, question: Apa kabar?, options: ["Baik.", "Sampai jumpa."], answer: 0}
      - {id: p2, type: multiple_choice, question: Selamat pagi!, options: ["Pagi!", "Malam!"], answer: 0}
  - id: bipa2
    level: bipa2
    skill: reading
    title: {ar_eg: ٢, ar: ٢, en: "2"}
    items:
      - {id: p3, type: multiple_choice, question: Rumahnya dibersihkan kemarin., options: ["past", "future"], answer: 0}
      - {id: p4, type: multiple_choice, question: Kesehatan itu penting., options: ["health", "wealth"], answer: 0}
  - id: bipa3
    level: bipa3
    skill: reading
    title: {ar_eg: ٣, ar: ٣, en: "3"}
    items:
      - {id: p5, type: multiple_choice, question: Sek tak lapor pusat, options: ["jv", "id"], answer: 0}
review: {status: draft, verified: false}
"#;

fn level() -> TestRunner {
    let t = parse_test(LEVEL).unwrap();
    assert_eq!(validate_test(&t), vec![]);
    TestRunner::new(t)
}

fn placement() -> TestRunner {
    let t = parse_test(PLACEMENT).unwrap();
    assert_eq!(validate_test(&t), vec![]);
    TestRunner::new(t)
}

fn text(s: &str) -> Answer {
    Answer::Text(s.into())
}

#[test]
fn validation_catches_broken_tests() {
    let mut t = parse_test(LEVEL).unwrap();
    t.pass_mark = None;
    t.sections[0].items[1].id = "t01".into();
    if let TestItemKind::WritingTask(task) = &mut t.sections[2].items[0].kind {
        task.rubric.clear();
    }
    t.sections[1].items[1].points = Some(0.0);
    let issues: Vec<String> = validate_test(&t).iter().map(|i| i.to_string()).collect();
    assert_eq!(issues.len(), 4, "{issues:#?}");
    assert!(issues.iter().any(|i| i.contains("pass mark")));
    assert!(issues.iter().any(|i| i.contains("duplicate id")));
    assert!(issues.iter().any(|i| i.contains("rubric")));
    assert!(issues.iter().any(|i| i.contains("points")));

    let mut p = parse_test(PLACEMENT).unwrap();
    p.sections[2].level = None;
    assert!(validate_test(&p).iter().any(|i| i.message.contains("level")));
}

#[test]
fn a_level_test_scores_per_skill_and_passes_at_the_mark() {
    let mut r = level();
    assert_eq!(r.step(), TestStep::Intro);
    r.start();
    r.answer(&Answer::Choice(0), EG); // t01 right
    r.answer(&Answer::Choice(1), EG); // t02 comprehension, right
    r.answer(&text("apa kabar"), EG); // t03 right (punctuation and case fold)
    r.answer(&Answer::Matched { missed: vec!["rumah".into()] }, EG); // t04: 3 of 4
    r.answer(&Answer::Choice(0), EG); // t05 right
    r.answer(&text("الصحة شي مهم"), EG); // t06: no reference matches → self-check
    assert!(r.item().is_some_and(|i| i.open_task().is_some()));
    r.answer_open("Hai Dimas! Saya di Kairo.");
    r.answer(&Answer::Practiced, EG); // t08 speaking practice: unscored
    assert_eq!(r.step(), TestStep::SelfCheck);
    assert_eq!(r.open_answers().count(), 2);
    assert_eq!(r.pending_answer("t07"), Some("Hai Dimas! Saya di Kairo."));

    // Results wait for every self-grade.
    r.finish_self_check();
    assert_eq!(r.step(), TestStep::SelfCheck);
    r.self_assess("t06", SelfGrade::Fully);
    r.self_assess("t07", SelfGrade::Partly);
    assert!(r.self_check_done());
    r.finish_self_check();
    assert_eq!(r.step(), TestStep::Results);

    let res = r.results();
    // 1 + 1 + 1 + 0.75 + 1 + 1 + 2 of 1+1+1+1+1+1+4 = 7.75 / 10.
    assert!((res.total.earned - 7.75).abs() < 1e-4, "{res:?}");
    assert_eq!(res.total.available, 10.0);
    assert_eq!(res.passed, Some(true));
    let skill = |s| res.skills.iter().find(|(k, _)| *k == s).and_then(|(_, t)| t.share());
    assert_eq!(skill(TestSkill::Listening), Some(1.0));
    assert_eq!(skill(TestSkill::Vocabulary), Some(0.75));
    assert_eq!(skill(TestSkill::Writing), Some(0.5));
    assert_eq!(skill(TestSkill::Speaking), None, "practice is not scored");
    assert_eq!(res.missed.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["t04", "t07"]);
    assert_eq!(res.skipped, 0);
    assert_eq!(res.start_level, None);
    let f = r.finished();
    assert!(f.passed && (f.score - 0.775).abs() < 1e-4);
}

#[test]
fn a_level_test_below_the_mark_fails_gently_and_skips_do_not_count() {
    let mut r = level();
    r.start();
    r.skip(); // t01: can't listen now
    r.answer(&Answer::Choice(0), EG); // t02 wrong
    r.answer(&text("apa kabr"), EG); // t03 a typo still counts
    r.answer(&Answer::Matched { missed: vec![] }, EG);
    r.answer(&Answer::Choice(1), EG); // t05 wrong
    r.answer(&text("Health is important"), EG); // t06 reference in another language
    r.answer_open("");
    r.answer(&Answer::Practiced, EG);
    r.self_assess("t07", SelfGrade::NotYet);
    r.finish_self_check();
    let res = r.results();
    assert_eq!(res.total.available, 9.0);
    assert_eq!(res.total.earned, 3.0);
    assert_eq!(res.passed, Some(false));
    assert_eq!(res.skipped, 1);
    assert_eq!(res.missed.len(), 3);
}

#[test]
fn placement_stops_at_the_first_weak_section() {
    let mut r = placement();
    r.start();
    r.answer(&Answer::Choice(0), EG);
    r.answer(&Answer::Choice(0), EG); // bipa1: 100%
    assert_eq!(r.section().map(|s| s.id.as_str()), Some("bipa2"));
    r.answer(&Answer::Choice(0), EG);
    r.answer(&Answer::Choice(1), EG); // bipa2: 50% → start here
    assert_eq!(r.step(), TestStep::Results);
    let res = r.results();
    assert_eq!(res.start_level.as_deref(), Some("bipa2"));
    assert_eq!(res.sections.len(), 2, "bipa3 is never shown");
    assert_eq!(res.passed, None);
    let p = r.placement().unwrap();
    assert_eq!(p.start_level, "bipa2");
    assert_eq!(p.sections.iter().map(|s| s.score).collect::<Vec<_>>(), [1.0, 0.5]);
}

#[test]
fn placement_past_every_section_starts_at_the_next_level() {
    let mut r = placement();
    r.start();
    for _ in 0..5 {
        r.answer(&Answer::Choice(0), EG);
    }
    assert_eq!(r.results().start_level.as_deref(), Some("bipa4"));
    let mut r = placement();
    r.start();
    r.answer(&Answer::Choice(1), EG);
    r.answer(&Answer::Choice(1), EG);
    assert_eq!(r.results().start_level.as_deref(), Some("bipa1"));
}

#[test]
fn next_level_counts_up() {
    assert_eq!(next_level("bipa4"), "bipa5");
    assert_eq!(next_level("j2"), "j3");
    assert_eq!(next_level("bipa9"), "bipa10");
    assert_eq!(next_level("odd"), "odd");
}

#[test]
fn the_progress_bar_walks_the_items() {
    let mut r = level();
    assert_eq!(r.fraction(), 0.0);
    r.start();
    r.answer(&Answer::Choice(0), EG);
    r.answer(&Answer::Choice(1), EG);
    assert!((r.fraction() - 2.0 / 8.0).abs() < 1e-4);
}

#[test]
fn tagged_items_point_back_to_their_units() {
    let tagged = PLACEMENT
        .replace("{id: p3,", "{id: p3, unit: bipa2.u10,")
        .replace("{id: p4,", "{id: p4, unit: bipa2.u03,")
        .replace("{id: p1,", "{id: p1, unit: bipa1.u01,");
    let mut r = TestRunner::new(parse_test(&tagged).unwrap());
    r.start();
    r.answer(&Answer::Choice(0), EG);
    r.answer(&Answer::Choice(0), EG); // bipa1: 100%
    r.answer(&Answer::Choice(1), EG); // p3 (u10) wrong
    r.answer(&Answer::Choice(1), EG); // p4 (u03) wrong → start in bipa2
    let res = r.results();
    assert_eq!(res.start_level.as_deref(), Some("bipa2"));
    // Route order, not string order: u03 comes before u10.
    assert_eq!(res.start_unit.as_deref(), Some("bipa2.u03"));
    assert_eq!(res.weak_units, [("bipa2.u03".to_string(), 1.0), ("bipa2.u10".to_string(), 1.0)]);
    assert_eq!(r.placement().unwrap().start_unit.as_deref(), Some("bipa2.u03"));
}

#[test]
fn untagged_tests_have_no_weak_units() {
    let mut r = placement();
    r.start();
    for _ in 0..4 {
        r.answer(&Answer::Choice(1), EG);
    }
    let res = r.results();
    assert!(res.weak_units.is_empty());
    assert_eq!(res.start_unit, None);
}
