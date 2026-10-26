//! Plays every lesson in content/lessons through the real Lesson Player
//! (kabar_core::lesson::Player), answering each exercise with its own
//! reference answer in each explanation language. Every exercise must grade
//! as right and every lesson must pass the player's own `validate` and reach
//! its summary with a perfect score:
//! a lesson the learner can't finish by answering correctly is a content bug.

use std::path::Path;

use kabar_core::lesson::{Answer, Grade, Player, Step, normalize, parse_lesson, validate};
use kabar_core::locale::UiLanguage;
use kabar_protocol::lesson::{Direction, Exercise, ExerciseKind};

const LANGS: [UiLanguage; 3] = [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En];

/// The answer a learner who knows the lesson would give.
fn right_answer(ex: &Exercise, lang: UiLanguage) -> Result<Answer, String> {
    use ExerciseKind as K;
    Ok(match &ex.kind {
        K::MultipleChoice { answer, .. } => Answer::Choice(*answer),
        K::MatchPairs { .. } => Answer::Matched { missed: Vec::new() },
        K::Speaking { .. } => Answer::Practiced,
        K::Listen { options, answer, .. } if !options.is_empty() => {
            Answer::Choice(answer.ok_or("listen has options but no answer")?)
        }
        K::Listen { text, .. } | K::Dictation { text, .. } => Answer::Text(text.clone()),
        K::FillBlank { options, answers, .. } if !options.is_empty() => {
            let i = options
                .iter()
                .position(|o| answers.iter().any(|a| normalize(a) == normalize(o)))
                .ok_or("fill_blank: no option matches an accepted answer")?;
            Answer::Choice(i)
        }
        K::FillBlank { answers, .. } => {
            Answer::Text(answers.first().ok_or("fill_blank without answers")?.clone())
        }
        K::BuildSentence { answer, .. } => Answer::Text(answer.clone()),
        K::Translate { direction: Direction::ToTarget, answers, .. } => {
            Answer::Text(answers.first().ok_or("translate to_target without answers")?.clone())
        }
        K::Translate { answers_meaning, .. } => {
            let a = answers_meaning.as_ref().ok_or("translate to_meaning without answers_meaning")?;
            let list = match lang {
                UiLanguage::ArEg => &a.ar_eg,
                UiLanguage::Ar => &a.ar,
                UiLanguage::En => &a.en,
            };
            Answer::Text(list.first().ok_or("translate to_meaning: no answer in this language")?.clone())
        }
    })
}

fn play(yaml: &str, lang: UiLanguage) -> Result<(), String> {
    let lesson = parse_lesson(yaml).map_err(|e| format!("does not parse: {e:?}"))?;
    let issues = validate(&lesson);
    if !issues.is_empty() {
        return Err(format!("Lesson Player validation: {issues:?}"));
    }
    let mut player = Player::new(lesson);
    for _ in 0..200 {
        match player.step() {
            Step::Summary => {
                let s = player.summary();
                return if s.correct == s.graded {
                    Ok(())
                } else {
                    Err(format!("summary {}/{} with every answer right", s.correct, s.graded))
                };
            }
            Step::Exercise { index, .. } => {
                let ex = player.lesson().exercises[index].clone();
                let answer = right_answer(&ex, lang).map_err(|e| format!("{}: {e}", ex.id))?;
                let grade = player.check(&answer, lang);
                if !matches!(grade, Grade::Correct | Grade::Practiced) {
                    return Err(format!("{}: its own answer {answer:?} grades as {grade:?}", ex.id));
                }
            }
            Step::Slide(_) | Step::Dialogue => {}
        }
        player.advance();
    }
    Err("never reached the summary".into())
}

#[test]
fn every_lesson_plays_through_with_its_own_answers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/lessons");
    let mut problems = Vec::new();
    let mut played = 0;
    for lang_dir in std::fs::read_dir(&root).unwrap() {
        let dir = lang_dir.unwrap().path();
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
        files.sort();
        for f in files.iter().filter(|f| f.extension().is_some_and(|e| e == "yaml")) {
            let yaml = std::fs::read_to_string(f).unwrap();
            for lang in LANGS {
                if let Err(e) = play(&yaml, lang) {
                    problems.push(format!("{} ({lang:?}): {e}", f.file_name().unwrap().to_string_lossy()));
                }
            }
            played += 1;
        }
    }
    assert!(played > 300, "only {played} lessons found");
    assert!(problems.is_empty(), "{} problem(s):\n{}", problems.len(), problems.join("\n"));
}
