//! Existing behavior regressions for the feature facade.

use super::*;

const FIXTURE: &str = include_str!("../../../../fixtures/lessons/bipa1.u01.l01.yaml");

use kabar_core::lesson::level_test::{SelfGrade, TestRunner, TestStep, parse_test, validate_test};

/// Graded choices carry a glyph and a spoken label, not just a colour (#149).
#[test]
fn graded_choices_are_marked_beyond_colour() {
    assert_eq!(choice_mark(false, true, true), None);
    assert_eq!(choice_mark(true, false, false), None);
    assert_eq!(choice_mark(true, true, false), Some(ChoiceMark::Right));
    assert_eq!(choice_mark(true, true, true), Some(ChoiceMark::YoursRight));
    assert_eq!(choice_mark(true, false, true), Some(ChoiceMark::YoursWrong));
    assert_eq!(ChoiceMark::Right.glyph(), "✓");
    assert_eq!(ChoiceMark::YoursWrong.glyph(), "✗");
    for lang in [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En] {
        for m in [ChoiceMark::Right, ChoiceMark::YoursRight, ChoiceMark::YoursWrong] {
            assert!(!lt(lang, m.says()).is_empty());
        }
    }
}

/// The "no voice" note shows once: on the first step after the first run
/// of skipped listening items, and in the summary (#156).
#[test]
fn the_no_voice_note_shows_once_per_lesson() {
    let ex = |index| Step::Exercise { index, retry: false };
    // e2 is skipped, landing on e3: the note goes on e3.
    let note = voice_note_step(None, ex(2), ex(3));
    assert_eq!(note, Some(ex(3)));
    // e3 is a listening item too: the note moves on with the skip.
    let note = voice_note_step(note, ex(3), ex(4));
    assert_eq!(note, Some(ex(4)));
    // A later skip (e7 -> e8) leaves it where the learner saw it.
    let note = voice_note_step(note, ex(7), ex(8));
    assert_eq!(note, Some(ex(4)));
    assert!(show_voice_note(note, 3, ex(4)));
    assert!(!show_voice_note(note, 3, ex(5)));
    assert!(!show_voice_note(note, 3, ex(8)));
    assert!(show_voice_note(note, 3, Step::Summary));
    assert!(!show_voice_note(None, 0, Step::Summary));
}

/// A filled blank sits right against the punctuation after it (#156).
#[test]
fn a_blank_hugs_following_punctuation() {
    let sentence = "Selamat ___, Pak!";
    let (_, after) = sentence.split_once("___").unwrap();
    assert!(blank_hugs_next(after));
    assert!(blank_hugs_next("."));
    assert!(!blank_hugs_next(" pagi."));
    assert!(!blank_hugs_next(""));
}

/// Both halves of a matched couple share a tint; unmatched ones keep their state.
#[test]
fn matched_pairs_share_a_couple_tint() {
    let matched = [2, 0, 4, 1, 3];
    assert_eq!(pair_class(&matched, None, 2, None), "lp-pair done couple-0");
    assert_eq!(pair_class(&matched, None, 0, None), "lp-pair done couple-1");
    assert_eq!(pair_class(&matched, None, 3, None), "lp-pair done couple-4");
    // Six couples, six tints, and the number repeats nothing.
    let six = [5, 4, 3, 2, 1, 0];
    let tints: std::collections::BTreeSet<String> = (0..6).map(|i| pair_class(&six, None, i, None)).collect();
    assert_eq!(tints.len(), 6);
    assert_eq!((couple_number(&six, 5), couple_number(&six, 0)), (Some(1), Some(6)));
    assert_eq!(couple_number(&six[..2], 0), None);
    assert_eq!(pair_class(&[], Some(1), 1, None), "lp-pair selected");
    assert_eq!(pair_class(&[], Some(1), 1, Some(1)), "lp-pair wrong");
    assert_eq!(pair_class(&[], None, 1, None), "lp-pair");
}

/// The reference answer for a closed exercise.
fn right_answer(ex: &Exercise) -> Answer {
    use ExerciseKind as K;
    match &ex.kind {
        K::MultipleChoice { answer, .. } => Answer::Choice(*answer),
        K::Listen { answer: Some(i), options, .. } if !options.is_empty() => Answer::Choice(*i),
        K::Listen { text, .. } | K::Dictation { text, .. } => Answer::Text(text.clone()),
        K::FillBlank { options, answers, .. } if !options.is_empty() => {
            Answer::Choice(options.iter().position(|o| answers.contains(o)).unwrap())
        }
        K::FillBlank { answers, .. } => Answer::Text(answers[0].clone()),
        K::Translate { answers, answers_meaning, .. } => Answer::Text(
            answers.first().cloned().unwrap_or_else(|| answers_meaning.as_ref().unwrap().ar_eg[0].clone()),
        ),
        K::BuildSentence { answer, .. } => Answer::Text(answer.clone()),
        K::MatchPairs { .. } => Answer::Matched { missed: vec![] },
        K::Speaking { .. } => Answer::Practiced,
    }
}

/// Takes a test answering everything right (open tasks self-rated fully).
fn take_perfectly(r: &mut TestRunner) {
    r.start();
    while let TestStep::Item { .. } = r.step() {
        match r.exercise() {
            Some(ex) => r.answer(&right_answer(&ex), UiLanguage::ArEg),
            None => r.answer_open("Hai!"),
        }
    }
    let ids: Vec<String> = r.open_answers().map(|o| o.id.clone()).collect();
    for id in ids {
        r.self_assess(&id, SelfGrade::Fully);
    }
    r.finish_self_check();
    assert_eq!(r.step(), TestStep::Results);
}

#[test]
fn every_embedded_test_parses_validates_and_can_be_passed() {
    assert!(TESTS.rows.len() >= 3, "only {} tests embedded", TESTS.rows.len());
    for (id, yaml) in embedded::all(TESTS) {
        let test = parse_test(&yaml).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(&test.id, id);
        let issues: Vec<String> = validate_test(&test).iter().map(|i| i.to_string()).collect();
        assert!(issues.is_empty(), "{id}:\n{}", issues.join("\n"));
        let mut r = TestRunner::new(test.clone());
        take_perfectly(&mut r);
        let res = r.results();
        assert!(res.missed.is_empty(), "{id}: {:?}", res.missed);
        match test.kind {
            kabar_protocol::test::TestKind::Level => assert_eq!(res.passed, Some(true), "{id}"),
            kabar_protocol::test::TestKind::Placement => {
                assert_eq!(res.start_level.as_deref(), Some("bipa5"), "{id}")
            }
        }
    }
    assert!(test_content("bipa1.test").is_some());
    assert!(test_content("placement").is_some());
    assert!(test_content("bipa7.test").is_some());
    assert!(lesson_content("bipa1.test").is_none());
}

#[test]
fn every_embedded_lesson_parses_and_validates() {
    assert!(LESSONS.rows.len() >= 100, "only {} lessons embedded", LESSONS.rows.len());
    let mut problems = Vec::new();
    for (id, yaml) in embedded::all(LESSONS) {
        match parse_lesson(&yaml) {
            Ok(lesson) => {
                if lesson.id != *id {
                    problems.push(format!("{id}: id is {:?}", lesson.id));
                }
                problems.extend(validate(&lesson).iter().map(|i| format!("{id}: {i}")));
            }
            Err(e) => problems.push(format!("{id}: {e}")),
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(lesson_content("bipa1.u01.l01").is_some());
    assert!(lesson_content("bipa2.u10.l05").is_some());
    assert!(lesson_content("bipa7.u10.l14").is_some());
    assert!(lesson_content("bipa8.u01.l01").is_none());
}

#[test]
fn a_lesson_game_opens_its_play_screen() {
    use crate::play::Route;
    assert_eq!(game_route("uu.grandmother"), Some(Route::Unggah));
    assert_eq!(game_route("mp.wedi-wedhi"), Some(Route::Pairs));
    assert_eq!(game_route("pasar"), None);
}

#[test]
fn bold_markup_is_split_out() {
    let parts = bold_parts("فيه كلمات **مشتركة** بين اللغتين");
    assert_eq!(
        parts,
        vec![(false, "فيه كلمات ".into()), (true, "مشتركة".into()), (false, " بين اللغتين".into())]
    );
    assert_eq!(bold_parts("**Aku**"), vec![(true, "Aku".into())]);
    assert_eq!(bold_parts("no markup"), vec![(false, "no markup".into())]);
    assert_eq!(bold_parts("a ** b"), vec![(false, "a ".into()), (false, "** b".into())]);
}

#[test]
fn arabic_in_an_english_line_gets_its_own_isolate() {
    // #403: j1.u01.l01, English UI.
    let text = "ngoko between friends (like «إنت»), krama when being polite (like «حضرتك»).";
    let plain: Vec<String> = pron_units(latin_runs(text), "jv", false)
        .into_iter()
        .filter_map(|u| if let Unit::Plain(t) = u { Some(t) } else { None })
        .collect();
    let arabic: Vec<String> =
        plain.iter().flat_map(|t| arabic_runs(t)).filter(|(a, _)| *a).map(|(_, r)| r).collect();
    assert_eq!(arabic, ["إنت", "حضرتك"]);
    // Every character survives the split, quotes and commas outside.
    for t in &plain {
        assert_eq!(arabic_runs(t).into_iter().map(|(_, r)| r).collect::<String>(), *t);
    }
    assert_eq!(
        arabic_runs("«إنت»), "),
        [(false, "«".to_string()), (true, "إنت".to_string()), (false, "»), ".to_string())]
    );
    assert_eq!(arabic_runs("like «مع السلامة» here")[1], (true, "مع السلامة".to_string()));
    assert_eq!(arabic_runs("only English"), [(false, "only English".to_string())]);
}

#[test]
fn latin_runs_isolate_target_text() {
    let runs = latin_runs("«Apa kabar?» يعني «إزيك؟»");
    assert_eq!(
        runs,
        vec![(false, "«".into()), (true, "Apa kabar?".into()), (false, "» يعني «إزيك؟»".into())]
    );
    let runs = latin_runs("selamat + وقت");
    assert_eq!(runs, vec![(true, "selamat".into()), (false, " + وقت".into())]);
    assert_eq!(latin_runs("سلام"), vec![(false, "سلام".into())]);
    let runs = latin_runs("بنقول sore. malam بتبدأ");
    assert_eq!(runs[1], (true, "sore".into()));
    assert_eq!(runs[3], (true, "malam".into()));
    assert_eq!(
        latin_runs("قول Selamat jalan, sampai jumpa!")[1],
        (true, "Selamat jalan, sampai jumpa!".into())
    );
    let runs = latin_runs("- Menurut X, … = على حسب X …");
    assert_eq!(runs[1], (true, "Menurut X, …".into()));
    assert_eq!(runs[3], (true, "X …".into()));
}

#[test]
fn suffixes_keep_their_hyphen_and_arrows_follow_the_line() {
    let runs = latin_runs("aku و-ku = saya");
    assert_eq!(runs, vec![(true, "aku".into()), (false, " و".into()), (true, "-ku = saya".into()),]);
    // A hyphen inside a Latin word is still a hyphen, not a new run.
    assert_eq!(latin_runs("anak-anak"), vec![(true, "anak-anak".into())]);
    let runs = latin_runs("besar = كبير → lebih besar = أكبر");
    let plain: String =
        runs.iter().filter(|(l, _)| !l).map(|(_, t)| in_direction(t, true).into_owned()).collect();
    assert!(plain.contains('←') && !plain.contains('→'), "{plain}");
    assert_eq!(in_direction("a → b", false), "a → b");
}

#[test]
fn pronunciation_follows_the_closing_quote() {
    let units = pron_units(latin_runs("«selamat jalan» لما تمشي"), "id", true);
    assert_eq!(units[0], Unit::Plain("«".into()));
    let Unit::Latin { phrases, close, short } = &units[1] else { panic!("{units:?}") };
    assert_eq!((phrases[0].text.as_str(), close.as_str(), *short), ("selamat jalan", "»", true));
    assert!(phrases[0].say);
    assert_eq!(units[2], Unit::Plain(" لما تمشي".into()));
}

#[test]
fn mapping_lines_say_each_side_and_skip_indonesian_on_javanese_slides() {
    let units = pron_units(latin_runs("- Selamat pagi → Sugeng enjing (صباح الخير)"), "jv", true);
    let latin = units.iter().find_map(|u| match u {
        Unit::Latin { phrases, .. } => Some(phrases),
        Unit::Plain(_) => None,
    });
    let phrases = latin.unwrap();
    let said: Vec<(&str, bool)> = phrases.iter().map(|p| (p.text.as_str(), p.say)).collect();
    assert_eq!(said, [("Selamat pagi", false), ("Sugeng enjing", true)]);
    assert_eq!(phrases[0].sep, " → ");
    // On Indonesian slides both sides are said.
    let units = pron_units(vec![(true, "ora ↔ tidak".into())], "id", true);
    let Unit::Latin { phrases, .. } = &units[0] else { panic!() };
    assert!(phrases.iter().all(|p| p.say));
}

#[test]
fn pronunciation_is_drawn_after_its_word_and_quote_outside_the_latin_isolate() {
    let units = pron_units(latin_runs("«selamat jalan» لما تمشي"), "id", true);
    let Unit::Latin { phrases, close, .. } = &units[1] else { panic!("{units:?}") };
    assert_eq!(
        layout(phrases, close, true),
        [Bit::Latin("selamat jalan".into()), Bit::Close("»".into()), Bit::Say("selamat jalan".into())]
    );
    let units = pron_units(latin_runs("- Selamat pagi → Sugeng enjing (صباح الخير)"), "jv", true);
    let Some(Unit::Latin { phrases, close, .. }) = units.iter().find(|u| matches!(u, Unit::Latin { .. }))
    else {
        panic!("{units:?}")
    };
    assert_eq!(
        layout(phrases, close, true),
        [
            Bit::Latin("Selamat pagi".into()),
            Bit::Sep(" ← ".into()),
            Bit::Latin("Sugeng enjing".into()),
            Bit::Say("Sugeng enjing".into()),
        ]
    );
    assert_eq!(layout(phrases, close, false)[1], Bit::Sep(" → ".into()));
}

#[test]
fn brackets_around_mixed_text_form_one_group() {
    let pieces = bracket_groups(latin_runs("كلامك إنت للكبير (هو ngendika)"));
    assert_eq!(
        pieces,
        vec![
            Piece::Run(false, "كلامك إنت للكبير ".into()),
            Piece::Group(vec![(false, "(هو ".into()), (true, "ngendika".into()), (false, ")".into())]),
        ]
    );
    // Text after the group, and brackets that close inside one run, stay plain.
    let pieces = bracket_groups(latin_runs("(شنطة) tas (يعني bag) وخلاص"));
    assert_eq!(pieces[0], Piece::Run(false, "(شنطة) ".into()));
    assert_eq!(pieces[1], Piece::Run(true, "tas".into()));
    assert!(matches!(&pieces[3], Piece::Group(g) if g.len() == 3));
    assert_eq!(pieces[4], Piece::Run(false, " وخلاص".into()));
    // An unclosed bracket changes nothing.
    let pieces = bracket_groups(latin_runs("قول (apa kabar"));
    assert!(pieces.iter().all(|p| matches!(p, Piece::Run(..))));
    let text: String =
        pieces.iter().map(|p| if let Piece::Run(_, t) = p { t.as_str() } else { "" }).collect();
    assert_eq!(text, "قول (apa kabar");
}

#[test]
fn latin_runs_keep_phrases_whole() {
    let latin =
        |s: &str| -> Vec<String> { latin_runs(s).into_iter().filter(|(l, _)| *l).map(|(_, r)| r).collect() };
    // bipa3.u02.l01: two Latin sentences inside an Arabic line.
    assert_eq!(
        latin("Ni Luh membawa tas. Tasnya dari kain = الشنطة (اللي اتكلمنا عنها) من القماش."),
        ["Ni Luh membawa tas. Tasnya dari kain"]
    );
    assert_eq!(
        latin("قول Ni Luh membawa tas. Tasnya dari kain. يعني"),
        ["Ni Luh membawa tas. Tasnya dari kain."]
    );
    // Affix hyphens stay with the word.
    assert_eq!(latin("بتاعه / الـ = -nya"), ["-nya"]);
    assert_eq!(latin("البادئة ber- بتعمل فعل"), ["ber-"]);
    assert_eq!(latin("- Menurut X"), ["Menurut X"]);
    // Javanese ↔ Indonesian chains read left to right as one run.
    assert_eq!(latin("قارن: ora ↔ tidak · wis ↔ sudah"), ["ora ↔ tidak · wis ↔ sudah"]);
    // A parenthesis opened inside the run closes inside it.
    assert_eq!(latin("pantainya (pantai + nya) يعني"), ["pantainya (pantai + nya)"]);
    // The Arabic sentence's own full stop stays Arabic.
    assert_eq!(latin("بنقول sore. malam بتبدأ"), ["sore", "malam"]);
}

#[test]
fn draft_answers_follow_the_exercise_kind() {
    let lesson = parse_lesson(FIXTURE).unwrap();
    let ex = |id: &str| lesson.exercises.iter().find(|e| e.id == id).unwrap().clone();
    let d = Draft::default();
    assert_eq!(draft_answer(&ex("e01"), &d, &[]), None);
    assert_eq!(
        draft_answer(&ex("e01"), &Draft { choice: Some(1), ..d.clone() }, &[]),
        Some(Answer::Choice(1))
    );
    assert_eq!(draft_answer(&ex("e06"), &Draft { text: "  ".into(), ..d.clone() }, &[]), None);
    let tiles = vec!["jalan,".to_string(), "Selamat".to_string()];
    assert_eq!(
        draft_answer(&ex("e07"), &Draft { picked: vec![1, 0], ..d.clone() }, &tiles),
        Some(Answer::Text("Selamat jalan,".into()))
    );
    assert_eq!(draft_answer(&ex("e11"), &d, &[]), Some(Answer::Practiced));
    assert_eq!(draft_answer(&ex("e02"), &d, &[]), None, "match pairs check themselves");
}

/// Every kind's answer survives a save and a reopen, by content: the
/// same option, tiles (repeats included), pairs and text come back.
#[test]
fn drafts_survive_saving_for_every_kind() {
    let lesson = parse_lesson(FIXTURE).unwrap();
    let player = Player::new(lesson.clone());
    let mut kinds = std::collections::HashSet::new();
    for ex in &lesson.exercises {
        let tiles = player.tiles(ex);
        let d = match &ex.kind {
            ExerciseKind::MatchPairs { .. } => {
                Draft { matched: vec![1, 0], missed: vec!["pagi".into()], ..Draft::default() }
            }
            ExerciseKind::BuildSentence { .. } => {
                Draft { picked: vec![tiles.len() - 1, 0], ..Draft::default() }
            }
            _ if option_count(ex) > 0 => Draft { choice: Some(option_count(ex) - 1), ..Draft::default() },
            _ => Draft { text: "Selamat sore".into(), ..Draft::default() },
        };
        kinds.insert(std::mem::discriminant(&ex.kind));
        let saved = save_draft(ex, &d, &tiles);
        let json = serde_json::to_string(&saved).unwrap();
        let back = restore_draft(ex, &serde_json::from_str(&json).unwrap(), &tiles);
        assert_eq!(back, d, "{}", ex.id);
    }
    assert!(kinds.len() >= 6, "the fixture covers {} kinds", kinds.len());
    // Repeated tiles each find their own slot; unknown pieces drop out.
    let ex = lesson.exercises.iter().find(|e| matches!(e.kind, ExerciseKind::BuildSentence { .. })).unwrap();
    let tiles = vec!["a".to_string(), "b".into(), "a".into()];
    let r = SavedResponse { tiles: vec!["a".into(), "a".into(), "zzz".into()], ..Default::default() };
    assert_eq!(restore_draft(ex, &r, &tiles).picked, vec![0, 2]);
}

/// A choice is kept by its text, so a reshuffled retry restores the
/// same option, not the same slot.
#[test]
fn a_choice_is_kept_by_its_text() {
    let lesson = parse_lesson(FIXTURE).unwrap();
    let ex = lesson.exercises.iter().find(|e| matches!(e.kind, ExerciseKind::MultipleChoice { .. })).unwrap();
    let saved = save_draft(ex, &Draft { choice: Some(0), ..Draft::default() }, &[]);
    let view = kabar_core::lesson::retry_options(ex, 7).unwrap();
    let back = restore_draft(&view, &saved, &[]).choice.unwrap();
    assert_eq!(option_key(&view, back), option_key(ex, 0));
}

#[test]
fn levels_are_named_like_the_journey() {
    assert_eq!(level_label("bipa1.u01.l01"), "BIPA 1");
    assert_eq!(level_label("j3.u02.l04"), "J3");
}

#[test]
fn a_disabled_check_says_what_it_waits_for() {
    let lesson = parse_lesson(FIXTURE).unwrap();
    for ex in &lesson.exercises {
        for lang in [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En] {
            assert!(!lt(lang, check_hint(ex)).is_empty());
        }
    }
}
