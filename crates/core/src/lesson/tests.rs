use kabar_protocol::lesson::{ExerciseKind, LessonStep, Outcome, Souvenir};

use super::*;

const L01: &str = include_str!("../../../../fixtures/lessons/bipa1.u01.l01.yaml");
const L02: &str = include_str!("../../../../fixtures/lessons/bipa1.u01.l02.yaml");

fn l01() -> LessonContent {
    parse_lesson(L01).unwrap()
}

fn l02() -> LessonContent {
    parse_lesson(L02).unwrap()
}

fn ex<'a>(l: &'a LessonContent, id: &str) -> &'a Exercise {
    l.exercises.iter().find(|e| e.id == id).unwrap()
}

const EG: UiLanguage = UiLanguage::ArEg;

fn text(s: &str) -> Answer {
    Answer::Text(s.into())
}

/// Most fixtures are Indonesian lessons.
fn grade(ex: &Exercise, answer: &Answer, lang: UiLanguage) -> Grade {
    super::grade(ex, answer, lang, "id")
}

#[test]
fn fixtures_parse_and_validate() {
    for (yaml, id) in [(L01, "bipa1.u01.l01"), (L02, "bipa1.u01.l02")] {
        let l = parse_lesson(yaml).unwrap();
        assert_eq!(l.id, id);
        let issues = validate(&l);
        assert!(issues.is_empty(), "{id}: {issues:#?}");
        assert!((3..=6).contains(&l.slides.len()));
        assert!(!l.review.verified, "fixtures stay unverified until the learner approves them");
    }
}

#[test]
fn fixtures_cover_every_exercise_kind() {
    let mut seen = HashSet::new();
    for l in [l01(), l02()] {
        for e in &l.exercises {
            let tag = match &e.kind {
                ExerciseKind::MultipleChoice { .. } => "multiple_choice",
                ExerciseKind::MatchPairs { .. } => "match_pairs",
                ExerciseKind::Translate { direction: Direction::ToTarget, .. } => "translate_to",
                ExerciseKind::Translate { direction: Direction::FromTarget, .. } => "translate_from",
                ExerciseKind::BuildSentence { .. } => "build_sentence",
                ExerciseKind::Listen { options, .. } if options.is_empty() => "listen_typed",
                ExerciseKind::Listen { .. } => "listen_pick",
                ExerciseKind::FillBlank { options, .. } if options.is_empty() => "fill_typed",
                ExerciseKind::FillBlank { .. } => "fill_pick",
                ExerciseKind::Dictation { .. } => "dictation",
                ExerciseKind::Speaking { .. } => "speaking",
            };
            seen.insert(tag);
        }
    }
    assert_eq!(seen.len(), 11, "{seen:?}");
}

#[test]
fn arabic_loans_are_marked() {
    let words: Vec<_> = [l01(), l02()].into_iter().flat_map(|l| l.new_words).collect();
    for (w, ar) in [("kabar", "خبر"), ("selamat", "سلامة"), ("salam", "سلام"), ("maaf", "معاف")]
    {
        let nw = words.iter().find(|n| n.word == w).unwrap_or_else(|| panic!("{w} missing"));
        assert_eq!(nw.arabic.as_deref(), Some(ar));
    }
}

#[test]
fn validate_catches_broken_content() {
    let mut l = l01();
    l.schema = "kabar.lesson/v0".into();
    l.exercises[1].id = "e01".into();
    if let ExerciseKind::MultipleChoice { answer, .. } = &mut l.exercises[0].kind {
        *answer = 7;
    }
    if let ExerciseKind::FillBlank { sentence, .. } = &mut l.exercises[4].kind {
        *sentence = "Selamat ___ di ___".into();
    }
    if let ExerciseKind::Translate { answers_meaning, .. } = &mut l.exercises[7].kind {
        *answers_meaning = None;
    }
    l.review.verified = true;
    let issues = validate(&l);
    let has = |needle: &str| issues.iter().any(|i| i.to_string().contains(needle));
    assert!(has("expected kabar.lesson/v1"), "{issues:#?}");
    assert!(has("duplicate id"));
    assert!(has("answer 7 is out of range"));
    assert!(has("exactly one ___"));
    assert!(has("from_target needs answers_meaning"));
    assert!(has("only approved content can be verified"));
}

#[test]
fn validate_checks_the_game_id() {
    let mut l = l01();
    let game_issues = |l: &LessonContent| validate(l).into_iter().filter(|i| i.path == "game").count();
    for ok in ["uu.grandmother", "mp.wedi-wedhi"] {
        l.game = Some(ok.into());
        assert_eq!(game_issues(&l), 0, "{ok}");
    }
    for bad in ["pasar", "xx.wedi-wedhi", "mp.", "uu", ""] {
        l.game = Some(bad.into());
        assert_eq!(game_issues(&l), 1, "{bad:?}");
    }
}

#[test]
fn normalize_folds_case_punctuation_and_arabic_variants() {
    assert_eq!(normalize("  Apa   kabar?! "), "apa kabar");
    assert_eq!(normalize("Saya sedang membaca buku."), "saya sedang membaca buku");
    assert_eq!(normalize("Anak-anak bermain di taman."), "anak-anak bermain di taman");
    assert_eq!(normalize("أهلاً وسهلاً!"), normalize("اهلا وسهلا"));
    assert_eq!(normalize("Kowé dèrèng mangan."), "kowe dereng mangan");
    assert_eq!(normalize("شكــراً"), "شكرا");
    assert_eq!(normalize("إلى اللقاء"), normalize("الي اللقاء"));
    assert_eq!(normalize("سلامة"), normalize("سلامه"));
    assert_eq!(normalize("I'm fine — thanks"), "im fine thanks");
}

#[test]
fn grading_text_answers() {
    let l = l01();
    // Dictation: exact, then a one-letter typo, then a real miss.
    let d = ex(&l, "e10");
    assert_eq!(grade(d, &text("selamat sore bu"), EG), Grade::Correct);
    assert_eq!(
        grade(d, &text("Selamat sor, Bu"), EG),
        Grade::Almost { expected: "Selamat sore, Bu.".into() }
    );
    assert_eq!(grade(d, &text("Selamat malam, Bu"), EG), Grade::Wrong);
    // Short words get no typo slack.
    assert_eq!(grade(d, &text("Selamat sore, Ba"), EG), Grade::Wrong);
    // Listen, typed.
    assert_eq!(grade(ex(&l, "e12"), &text("sampai jumpa"), EG), Grade::Correct);
    assert_eq!(
        grade(ex(&l, "e12"), &text("sampaijumpa"), EG),
        Grade::Almost { expected: "Sampai jumpa!".into() }
    );
    // Empty answers are never right.
    assert_eq!(grade(d, &text("  ?"), EG), Grade::Wrong);
}

#[test]
fn indonesian_answers_take_either_register() {
    let ex = |kind| Exercise { id: "x".into(), kind, explain: None, words: vec![] };
    let to_target = |answers: &[&str]| {
        ex(ExerciseKind::Translate {
            direction: Direction::ToTarget,
            source_meaning: Some(Localized::default()),
            source_text: None,
            answers: answers.iter().map(|a| a.to_string()).collect(),
            answers_meaning: None,
        })
    };
    let t = to_target(&["Saya sudah makan, tetapi saya tidak lapar."]);
    for given in [
        "Saya udah makan, tapi saya nggak lapar.",
        "saya udah makan tetapi saya tidak lapar",
        "Saya sudah makan, tapi saya gak lapar",
    ] {
        assert_eq!(grade(&t, &text(given), EG), Grade::Correct, "{given}");
    }
    // A typo elsewhere is still one small slip.
    assert!(matches!(grade(&t, &text("Saya udah makan, tapi saya nggak lapr"), EG), Grade::Almost { .. }));
    // Twins are the same word, not synonyms: a different word stays wrong.
    assert_eq!(grade(&t, &text("Saya belum makan, tapi saya nggak lapar"), EG), Grade::NeedsReview);

    // #298: bipa1.u02.l02.e03 lists only the formal answer.
    let l02_e03 = to_target(&["Saya bisa bahasa Indonesia sedikit."]);
    assert_eq!(
        grade(&l02_e03, &text("Aku bisa bahasa Indonesia dikit"), EG),
        Grade::NeedsReview,
        "aku is not a twin"
    );
    assert_eq!(grade(&l02_e03, &text("Saya bisa bahasa Indonesia dikit"), EG), Grade::Correct);

    // A casual reference accepts the formal answer too.
    let casual = to_target(&["Gimana kabarnya?"]);
    assert_eq!(grade(&casual, &text("Bagaimana kabarnya?"), EG), Grade::Correct);

    // Tiles: the sentence built with "nggak" and "aja" matches a formal answer.
    let build = ex(ExerciseKind::BuildSentence {
        source: Localized::default(),
        answer: "Saya tidak mau, saya di rumah saja.".into(),
        alternatives: vec![],
        distractors: vec!["nggak".into(), "aja".into()],
    });
    assert_eq!(grade(&build, &text("Saya nggak mau saya di rumah aja"), EG), Grade::Correct);

    // Fill-blank chips: either form of the word is the right chip.
    let blank = ex(ExerciseKind::FillBlank {
        sentence: "Saya ___ makan.".into(),
        meaning: Localized::default(),
        options: vec!["udah".into(), "belum".into()],
        answers: vec!["sudah".into()],
    });
    assert_eq!(grade(&blank, &Answer::Choice(0), EG), Grade::Correct);
    assert_eq!(grade(&blank, &Answer::Choice(1), EG), Grade::Wrong);

    // Dictation is writing down what was said: the register heard is the answer.
    let dict = ex(ExerciseKind::Dictation {
        text: "Saya udah makan.".into(),
        audio: None,
        meaning: Localized::default(),
    });
    assert_eq!(grade(&dict, &text("saya udah makan"), EG), Grade::Correct);
    assert_ne!(grade(&dict, &text("saya sudah makan"), EG), Grade::Correct);

    // Javanese keeps its own words: "aja" there is "don't", not "saja".
    let jv = to_target(&["Aja mangan."]);
    assert_ne!(super::grade(&jv, &text("Saja mangan."), EG, "jv"), Grade::Correct);
    assert_eq!(super::grade(&jv, &text("Aja mangan"), EG, "jv"), Grade::Correct);
}

#[test]
fn grading_spec_sentences() {
    let dict = |t: &str| Exercise {
        id: "x".into(),
        kind: ExerciseKind::Dictation { text: t.into(), audio: None, meaning: Localized::default() },
        explain: None,
        words: vec![],
    };
    let d = dict("Saya sedang membaca buku.");
    assert_eq!(grade(&d, &text("saya sedang membaca buku"), EG), Grade::Correct);
    assert!(matches!(grade(&d, &text("Saya sedang membca buku."), EG), Grade::Almost { .. }));
    // Two typos in four words is more than "one small slip".
    assert_eq!(grade(&d, &text("Saya sedeng membca buku."), EG), Grade::Wrong);
    let d = dict("Anak-anak bermain di taman.");
    assert_eq!(grade(&d, &text("anak-anak bermain di taman"), EG), Grade::Correct);
    assert!(matches!(grade(&d, &text("Anak-anak bermain di tamam"), EG), Grade::Almost { .. }));
    let d = dict("Rumahnya dibersihkan kemarin.");
    assert_eq!(grade(&d, &text("Rumahnya dibersihkan kemarin"), EG), Grade::Correct);
    assert_eq!(grade(&d, &text("Rumahnya membersihkan kemarin"), EG), Grade::Wrong);
}

#[test]
fn grading_choices_and_tiles() {
    let l = l01();
    assert_eq!(grade(ex(&l, "e01"), &Answer::Choice(0), EG), Grade::Correct);
    assert_eq!(grade(ex(&l, "e01"), &Answer::Choice(2), EG), Grade::Wrong);
    assert_eq!(grade(ex(&l, "e05"), &Answer::Choice(0), EG), Grade::Correct);
    assert_eq!(grade(ex(&l, "e05"), &Answer::Choice(1), EG), Grade::Wrong);
    let b = ex(&l, "e07");
    assert_eq!(grade(b, &text("Selamat jalan, sampai jumpa!"), EG), Grade::Correct);
    assert_eq!(grade(b, &text("Selamat tinggal, sampai jumpa!"), EG), Grade::Wrong);
    // Alternatives count.
    let l2 = l02();
    let b2 = ex(&l2, "e05");
    assert_eq!(grade(b2, &text("Baik, terima kasih."), EG), Grade::Correct);
}

#[test]
fn grading_translations() {
    let l = l01();
    let to = ex(&l, "e06");
    assert_eq!(grade(to, &text("selamat malam bu"), EG), Grade::Correct);
    assert_eq!(
        grade(to, &text("Selamat malm, Bu!"), EG),
        Grade::Almost { expected: "Selamat malam, Bu!".into() }
    );
    // An answer that misses every reference is for the learner (later the AI) to judge, not "wrong".
    assert_eq!(grade(to, &text("Malam, Ibu Rina!"), EG), Grade::NeedsReview);

    let from = ex(&l, "e08");
    assert_eq!(grade(from, &text("أهلاً وسهلاً"), EG), Grade::Correct);
    assert_eq!(grade(from, &text("اهلا بيك"), EG), Grade::Correct);
    // English while the UI is in Arabic is still right.
    assert_eq!(grade(from, &text("welcome"), EG), Grade::Correct);
    assert_eq!(grade(from, &text("نورت"), EG), Grade::NeedsReview);
    assert_eq!(references(from, UiLanguage::En), vec!["Welcome!".to_string()]);
}

#[test]
fn speaking_and_match_pairs_never_fail() {
    let l = l01();
    assert_eq!(grade(ex(&l, "e11"), &Answer::Practiced, EG), Grade::Practiced);
    assert_eq!(grade(ex(&l, "e02"), &Answer::Matched { missed: vec!["sore".into()] }, EG), Grade::Correct);
}

#[test]
fn shuffles_are_deterministic_and_never_identity() {
    for n in 2..10 {
        for seed in 0..50 {
            let s = shuffled(n, seed);
            assert_eq!(s, shuffled(n, seed));
            assert_ne!(s, (0..n).collect::<Vec<_>>());
            let mut sorted = s.clone();
            sorted.sort();
            assert_eq!(sorted, (0..n).collect::<Vec<_>>());
        }
    }
    let p = Player::new(l01());
    let b = ex(p.lesson(), "e07").clone();
    let tiles = p.tiles(&b);
    assert_eq!(tiles, Player::new(l01()).tiles(&b), "same order on every device");
    let mut sorted = tiles.clone();
    sorted.sort();
    let mut expected = vec!["Selamat", "jalan", "sampai", "jumpa", "tinggal", "pagi"];
    expected.sort();
    assert_eq!(sorted, expected);
}

/// Answers every exercise right, except the ids in `miss`.
fn answer(p: &mut Player, miss: &[&str]) -> Grade {
    let e = p.exercise().unwrap().clone();
    let wrong = miss.contains(&e.id.as_str());
    let a = match &e.kind {
        ExerciseKind::MultipleChoice { answer, .. } => {
            Answer::Choice(if wrong { answer + 1 } else { *answer })
        }
        ExerciseKind::Listen { options, answer: Some(a), .. } if !options.is_empty() => {
            Answer::Choice(if wrong { a + 1 } else { *a })
        }
        ExerciseKind::FillBlank { options, answers, .. } if !options.is_empty() => {
            let right = options.iter().position(|o| o == &answers[0]).unwrap();
            Answer::Choice(if wrong { (right + 1) % options.len() } else { right })
        }
        ExerciseKind::MatchPairs { .. } => Answer::Matched { missed: vec![] },
        ExerciseKind::Speaking { .. } => Answer::Practiced,
        _ if wrong => text("salah sekali"),
        ExerciseKind::FillBlank { answers, .. } => text(&answers[0]),
        _ => text(&references(&e, EG)[0]),
    };
    let g = p.check(&a, EG);
    if g == Grade::NeedsReview {
        p.self_assess(false);
    }
    g
}

#[test]
fn player_walks_slides_exercises_retries_dialogue_summary() {
    let mut p = Player::new(l01());
    assert_eq!(p.step(), Step::Slide(0));
    assert_eq!(p.position().step, LessonStep::Slide { index: 0 });
    assert!(p.back().is_none());
    p.advance();
    assert_eq!(p.back().unwrap().step, LessonStep::Slide { index: 0 });
    for _ in 0..4 {
        p.advance();
    }
    assert_eq!(p.step(), Step::Exercise { index: 0, retry: false });

    let mut positions = Vec::new();
    while let Step::Exercise { .. } = p.step() {
        // Miss e03 and e08 (a translation the learner says doesn't match) the first time only.
        let miss: &[&str] =
            if matches!(p.step(), Step::Exercise { retry: true, .. }) { &[] } else { &["e03", "e08"] };
        answer(&mut p, miss);
        // Checking twice returns the same grade and records nothing new.
        let again = p.check(&Answer::Choice(0), EG);
        assert_eq!(Some(&again), p.pending());
        positions.push(p.advance().step);
        assert!(p.fraction() <= 1.0);
    }
    let tail: Vec<_> = positions.iter().rev().take(3).rev().cloned().collect();
    assert_eq!(
        tail,
        vec![
            LessonStep::Exercise { id: "e03".into(), retry: true },
            LessonStep::Exercise { id: "e08".into(), retry: true },
            LessonStep::Dialogue,
        ]
    );
    assert_eq!(p.step(), Step::Dialogue);
    p.add_elapsed(90_000);
    p.advance();
    assert_eq!(p.step(), Step::Summary);
    assert_eq!(p.fraction(), 1.0);

    let s = p.summary();
    // 12 exercises, speaking isn't graded: 11 graded, 9 right first time.
    assert_eq!((s.correct, s.graded), (9, 11));
    assert_eq!(s.score, 81);
    assert_eq!(s.xp, XP_FINISH + 9);
    assert_eq!(s.duration_ms, 90_000);
    assert_eq!(s.mistakes, vec!["sore".to_string(), "selamat datang".to_string()]);
    let done = p.completed();
    let e03 = done.results.iter().find(|r| r.id == "e03").unwrap();
    assert_eq!((e03.first, e03.retry), (Outcome::Missed, Some(Outcome::Correct)));
}

#[test]
fn perfect_lesson_earns_the_bonus() {
    let mut p = Player::new(l02());
    while !matches!(p.step(), Step::Summary) {
        if p.exercise().is_some() {
            answer(&mut p, &[]);
        }
        p.advance();
    }
    let s = p.summary();
    assert_eq!(s.score, 100);
    assert_eq!(s.xp, XP_FINISH + s.correct + XP_PERFECT);
    assert!(s.mistakes.is_empty());
}

#[test]
fn resume_keeps_step_and_results() {
    let mut p = Player::new(l02());
    while p.exercise().is_none_or(|e| e.id != "e04") {
        if p.exercise().is_some() {
            answer(&mut p, &["e02", "e03"]);
        }
        p.advance();
    }
    let saved = p.position();
    assert_eq!(saved.step, LessonStep::Exercise { id: "e04".into(), retry: false });
    // Round-trips through JSON, as it does through the sessions table.
    let json = serde_json::to_string(&saved).unwrap();
    let mut q = Player::resume(l02(), &serde_json::from_str(&json).unwrap());
    assert_eq!(q.step(), p.step());
    assert_eq!(q.position(), saved);
    while !matches!(q.step(), Step::Summary) {
        if q.exercise().is_some() {
            answer(&mut q, &[]);
        }
        q.advance();
    }
    // e03 (listen) was missed before the handoff and still got its retry.
    let e03 = q.completed().results.into_iter().find(|r| r.id == "e03").unwrap();
    assert_eq!(e03.retry, Some(Outcome::Correct));
    // match_pairs is never "missed"; its result is a Correct.
    assert_eq!(q.summary().graded, 11);
}

/// The text of a choice item's options, in the order shown.
fn option_texts(ex: &Exercise) -> Vec<String> {
    match &ex.kind {
        ExerciseKind::MultipleChoice { options, options_meaning, .. } if options.is_empty() => {
            options_meaning.iter().map(|m| pick(m, EG).to_owned()).collect()
        }
        ExerciseKind::MultipleChoice { options, .. }
        | ExerciseKind::Listen { options, .. }
        | ExerciseKind::FillBlank { options, .. } => options.clone(),
        _ => Vec::new(),
    }
}

#[test]
fn a_retried_choice_item_reshuffles_its_options_and_grades_the_same() {
    // e03 is a multiple-choice item: miss it, and it comes back at the end.
    let mut p = Player::new(l01());
    while p.exercise().is_none_or(|e| e.id != "e03") {
        p.advance();
    }
    let first = p.exercise().unwrap().clone();
    answer(&mut p, &["e03"]);
    p.advance();
    while !matches!(p.step(), Step::Exercise { retry: true, .. }) {
        if p.exercise().is_some() {
            answer(&mut p, &[]);
        }
        p.advance();
    }
    let retry = p.exercise().unwrap().clone();
    assert_eq!(retry.id, "e03");
    let (before, after) = (option_texts(&first), option_texts(&retry));
    assert_ne!(before, after, "same order as the first time");
    let mut sorted = (before.clone(), after.clone());
    sorted.0.sort();
    sorted.1.sort();
    assert_eq!(sorted.0, sorted.1, "the same options, reordered");
    // The answer follows its option.
    assert_eq!(display_answer(&retry, EG), display_answer(&first, EG));
    let ExerciseKind::MultipleChoice { answer: right, .. } = retry.kind else { panic!("e03 is a choice") };
    let ExerciseKind::MultipleChoice { answer: old, .. } = first.kind else { panic!("e03 is a choice") };
    assert_eq!(after[right], before[old]);
    // A resumed lesson shows the same order on the retry.
    let q = Player::resume(l01(), &p.position());
    assert_eq!(q.exercise(), Some(&retry));
    // Grading follows the shown order: the remembered old spot is not the answer.
    let wrong = (0..after.len()).find(|&i| i != right).unwrap();
    let mut q2 = q.clone();
    assert_eq!(q2.check(&Answer::Choice(wrong), EG), Grade::Wrong);
    let mut q3 = q.clone();
    assert_eq!(q3.check(&Answer::Choice(right), EG), Grade::Correct);
    // The first time, an item keeps its written order.
    assert_eq!(&first, ex(&l01(), "e03"));
}

#[test]
fn retry_options_keep_grading_for_every_choice_item() {
    for l in [l01(), l02()] {
        for ex in &l.exercises {
            let Some(view) = retry_options(ex, 42) else {
                assert!(option_texts(ex).len() < 2, "{} has options but no retry order", ex.id);
                continue;
            };
            assert_eq!(view.id, ex.id);
            let (before, after) = (option_texts(ex), option_texts(&view));
            assert_ne!(before, after, "{}", ex.id);
            // Picking the same option text grades the same, wherever it moved.
            for (k, text) in after.iter().enumerate() {
                let j = before.iter().position(|t| t == text).unwrap();
                assert_eq!(
                    grade(&view, &Answer::Choice(k), EG),
                    grade(ex, &Answer::Choice(j), EG),
                    "{} option {text:?}",
                    ex.id
                );
            }
            // Deterministic.
            assert_eq!(retry_options(ex, 42), Some(view));
        }
    }
}

#[test]
fn resume_falls_back_to_the_start_for_unknown_positions() {
    let pos = LessonPosition {
        lesson_id: "bipa1.u01.l01".into(),
        step: LessonStep::Exercise { id: "e99".into(), retry: false },
        progress: None,
    };
    assert_eq!(Player::resume(l01(), &pos).step(), Step::Slide(0));
    let other = LessonPosition { lesson_id: "bipa9.u01.l01".into(), ..pos };
    assert_eq!(Player::resume(l01(), &other).step(), Step::Slide(0));
}

#[test]
fn skipping_listening_neither_scores_nor_retries() {
    let mut l = l01();
    l.slides.clear();
    l.dialogue = None;
    l.souvenir = Some(Souvenir {
        id: "bipa1.u01.souvenir".into(),
        title: "Fatahillah".into(),
        note: Localized { ar_eg: "x".into(), ar: "x".into(), en: "x".into() },
    });
    let mut p = Player::new(l);
    assert_eq!(p.step(), Step::Exercise { index: 0, retry: false });
    while !matches!(p.step(), Step::Summary) {
        let e = p.exercise().unwrap();
        if matches!(e.kind, ExerciseKind::Listen { .. } | ExerciseKind::Dictation { .. }) {
            p.skip();
        } else {
            answer(&mut p, &[]);
            p.advance();
        }
    }
    let s = p.summary();
    assert_eq!(s.graded, 8);
    assert_eq!(s.score, 100);
    assert!(p.lesson().souvenir.is_some());
}

#[test]
fn listening_comprehension_answers_a_question_about_the_text() {
    let mut l = l01();
    let i = l
        .exercises
        .iter()
        .position(|e| matches!(&e.kind, ExerciseKind::Listen { options, .. } if !options.is_empty()))
        .unwrap();
    let q = Localized { ar_eg: "إمتى؟".into(), ar: "متى؟".into(), en: "When?".into() };
    if let ExerciseKind::Listen { options, answer, question, .. } = &mut l.exercises[i].kind {
        *options = vec!["pagi".into(), "siang".into(), "malam".into()];
        *answer = Some(1);
        *question = Some(q.clone());
    }
    assert_eq!(validate(&l), vec![]);
    let ex = l.exercises[i].clone();
    assert_eq!(grade(&ex, &Answer::Choice(1), EG), Grade::Correct);
    assert_eq!(grade(&ex, &Answer::Choice(0), EG), Grade::Wrong);
    assert_eq!(display_answer(&ex, EG), "siang");

    // Without the question, the options must be transcripts again.
    if let ExerciseKind::Listen { question, .. } = &mut l.exercises[i].kind {
        *question = None;
    }
    assert!(validate(&l).iter().any(|i| i.message.contains("heard text")));
    // A question needs options to answer it.
    if let ExerciseKind::Listen { options, answer, question, .. } = &mut l.exercises[i].kind {
        options.clear();
        *answer = None;
        *question = Some(q);
    }
    assert!(validate(&l).iter().any(|i| i.message.contains("needs options")));
}

fn finished(score: u32, firsts: &[Outcome]) -> LessonCompleted {
    LessonCompleted {
        score,
        xp: 0,
        duration_ms: 0,
        mistakes: Vec::new(),
        results: firsts
            .iter()
            .enumerate()
            .map(|(i, o)| ExerciseResult {
                id: format!("e{i}"),
                first: *o,
                retry: None,
                mistakes: Vec::new(),
            })
            .collect(),
    }
}

#[test]
fn a_reasonable_score_approves_the_lesson() {
    use Outcome::*;
    assert!(approves(&finished(80, &[Correct, Correct, Correct, Almost, Missed])));
    assert!(approves(&finished(100, &[Correct, Correct, Skipped, Practiced])));
    assert!(!approves(&finished(60, &[Correct, Correct, Correct, Missed, Missed])), "too low");
    assert!(!approves(&finished(100, &[Correct, Skipped, Skipped, Practiced])), "mostly skipped");
    assert!(!approves(&finished(100, &[Practiced, Skipped])), "nothing graded");
    assert!(!approves(&finished(100, &[])), "old event without results");
}
