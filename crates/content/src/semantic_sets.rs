//! Tight semantic sets (#224, docs/PEDAGOGY.md §4.4): words are grouped by
//! situation, not by category, because near neighbours taught together
//! interfere ("all the colours or all the kinship terms in one lesson" is the
//! pattern to avoid). `content/semantic-sets.yaml` lists the open categories
//! that keep turning up (colours, fruit, body parts, kinship, jobs…) per
//! language; a learn lesson may teach at most `max_per_lesson` new words
//! from any one of them. `allow` names lessons that may go over, each with
//! the reason.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::model::Curriculum;
use crate::retrieval::forms;

pub const FILE: &str = "content/semantic-sets.yaml";

pub fn check(
    root: &Path,
    curricula: &[Curriculum],
    lessons: &BTreeMap<String, Value>,
    problems: &mut Vec<String>,
) {
    let path = root.join(FILE);
    if !path.exists() {
        return;
    }
    let data = match crate::yaml_value(&path) {
        Ok(v) => v,
        Err(e) => {
            problems.push(format!("{FILE}: {e}"));
            return;
        }
    };
    let Some(max) = data.get("max_per_lesson").and_then(Value::as_u64) else {
        problems.push(format!("{FILE}: max_per_lesson must be a number"));
        return;
    };
    let allowed = |id: &str| data.get("allow").and_then(|a| a.get(id)).is_some();
    let sets = data.get("sets").and_then(Value::as_object).cloned().unwrap_or_default();
    for c in curricula {
        let lang = c.language.as_str();
        let members: Vec<(&String, Vec<String>)> = sets
            .iter()
            .map(|(name, set)| {
                let words = set.get(lang).and_then(Value::as_array).into_iter().flatten();
                (name, words.filter_map(Value::as_str).map(str::to_lowercase).collect())
            })
            .collect();
        let learn =
            c.levels.iter().flat_map(|l| &l.units).flat_map(|u| &u.lessons).filter(|l| l.kind == "learn");
        for meta in learn {
            let Some(lesson) = lessons.get(&meta.id) else { continue };
            let words: Vec<Vec<String>> = lesson
                .get("new_words")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|w| w.get("word").and_then(Value::as_str))
                .map(forms)
                .collect();
            for (name, set) in &members {
                let hits: Vec<&str> = words
                    .iter()
                    .filter_map(|fs| fs.iter().find(|f| set.contains(f)))
                    .map(String::as_str)
                    .collect();
                if hits.len() as u64 > max && !allowed(&meta.id) {
                    problems.push(format!(
                        "{}: teaches {} {name} words ({}); keep it to {max} or fewer and move the rest into other situations (PEDAGOGY §4.4)",
                        meta.id,
                        hits.len(),
                        hits.join(", ")
                    ));
                }
            }
        }
    }
    for id in data.get("allow").and_then(Value::as_object).into_iter().flat_map(|a| a.keys()) {
        if !lessons.contains_key(id) {
            problems.push(format!("{FILE}: allow names {id}, which is not a lesson"));
        }
    }
}
