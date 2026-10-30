//! Curriculum journal metadata and souvenir lookup.

use super::*;

// ------------------------------------------------------- learning journal ----

/// Lesson titles, level tests and unit souvenirs of both routes, for the
/// learning journal. Empty when the curricula didn't load (the journal then
/// shows lesson ids instead of titles).
pub fn journal_catalog() -> crate::coach::logic::Catalog {
    use crate::coach::logic::{Catalog, UnitInfo};
    let mut catalog = Catalog::default();
    let Ok(routes) = &*ROUTES else { return catalog };
    for level in routes.id.levels.iter().chain(&routes.jv.levels) {
        if let Some(test) = &level.level_test {
            catalog.titles.insert(test.id.clone(), level.code.clone());
            catalog.tests.insert(test.id.clone());
        }
        for unit in &level.units {
            for lesson in &unit.lessons {
                catalog.titles.insert(lesson.id.clone(), lesson.title.clone());
            }
            if let Some(c) = &unit.culture {
                catalog.units.push(UnitInfo {
                    id: unit.id.clone(),
                    souvenir: c.souvenir.clone(),
                    lessons: unit.lessons.iter().map(|l| l.id.clone()).collect(),
                });
            }
        }
    }
    catalog
}
// Daily learning route helpers live in their own module.
// ------------------------------------------------------------- suitcase ----

/// The souvenir a unit's lesson content ships (its target-language name and
/// the note in the UI language). The curriculum's card is English only, so
/// it's the fallback for units whose lessons aren't written yet.
/// Each unit's lessons are unpacked and parsed once, not on every render.
pub(super) fn local_souvenir(unit_id: &str, lang: UiLanguage) -> Option<(String, String)> {
    static FOUND: LazyLock<Mutex<HashMap<String, Option<Souvenir>>>> = LazyLock::new(Default::default);
    let mut found = FOUND.lock().unwrap_or_else(|e| e.into_inner());
    let s = found.entry(unit_id.to_string()).or_insert_with(|| {
        find_unit(unit_id)?.lessons.iter().rev().find_map(|l| {
            kabar_core::lesson::parse_lesson(&crate::lesson::lesson_content(&l.id)?).ok()?.souvenir
        })
    });
    s.as_ref().map(|s| (s.title.clone(), kabar_core::lesson::pick(&s.note, lang).to_string()))
}
/// The lesson that hands out a unit's souvenir: the last one carrying it,
/// else the unit's last lesson.
pub(super) fn souvenir_lesson(unit_id: &str) -> Option<String> {
    let unit = find_unit(unit_id)?;
    unit.lessons
        .iter()
        .rev()
        .find(|l| {
            crate::lesson::lesson_content(&l.id)
                .and_then(|c| kabar_core::lesson::parse_lesson(&c).ok())
                .is_some_and(|p| p.souvenir.is_some())
        })
        .or(unit.lessons.last())
        .map(|l| l.id.clone())
}
