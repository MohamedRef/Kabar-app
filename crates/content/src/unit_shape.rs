//! Unit shape (#224, docs/PEDAGOGY.md §3.1, §5.1): a unit runs learn… →
//! practice… → review, so every learn lesson gets the practice lesson's lag,
//! and the unit review brings back at least one word from every learn lesson
//! of the unit. Words added after the practice lesson, or left out of the
//! review, are only met again through FSRS.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::model::Curriculum;
use crate::retrieval::{contains_word, forms, tags, target_text};

pub fn check(curricula: &[Curriculum], lessons: &BTreeMap<String, Value>, problems: &mut Vec<String>) {
    for unit in curricula.iter().flat_map(|c| &c.levels).flat_map(|l| &l.units) {
        let kinds: Vec<&str> = unit.lessons.iter().map(|l| l.kind.as_str()).collect();
        if let Some(first_practice) = kinds.iter().position(|k| *k == "practice") {
            for l in unit.lessons[first_practice..].iter().filter(|l| l.kind == "learn") {
                problems.push(format!(
                    "{}: learn lesson {} comes after the unit's practice lesson; move it before {} (learn → practice → review)",
                    unit.id, l.id, unit.lessons[first_practice].id
                ));
            }
        }
        if let Some(r) = unit.lessons.iter().position(|l| l.kind == "review")
            && r + 1 != unit.lessons.len()
        {
            problems.push(format!(
                "{}: the review lesson {} must be the unit's last",
                unit.id, unit.lessons[r].id
            ));
        }
        let Some(review) =
            unit.lessons.iter().rev().find(|l| l.kind == "review").and_then(|l| lessons.get(&l.id))
        else {
            continue;
        };
        let exercises: Vec<(String, Vec<String>)> = review
            .get("exercises")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|e| (target_text(e, true), tags(e)))
            .collect();
        for learn in unit.lessons.iter().filter(|l| l.kind == "learn") {
            let Some(lesson) = lessons.get(&learn.id) else { continue };
            let words: Vec<&str> = lesson
                .get("new_words")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|w| w.get("word").and_then(Value::as_str))
                .collect();
            if words.is_empty() {
                continue;
            }
            let met = words
                .iter()
                .flat_map(|w| forms(w))
                .any(|f| exercises.iter().any(|(text, tags)| tags.contains(&f) || contains_word(text, &f)));
            if !met {
                problems.push(format!(
                    "{}: the unit review brings back none of {}'s new words ({}); add at least one to a review exercise",
                    unit.id,
                    learn.id,
                    words.join(", ")
                ));
            }
        }
    }
}
