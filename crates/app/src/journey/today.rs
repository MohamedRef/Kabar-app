//! The Journey's daily-plan helpers, kept apart from map rendering.

use super::*;

// ------------------------------------------------------------ for Today ----

pub(super) fn find_unit(unit_id: &str) -> Option<&'static Unit> {
    let routes = ROUTES.as_ref().ok()?;
    routes.id.levels.iter().chain(&routes.jv.levels).flat_map(|l| &l.units).find(|u| u.id == unit_id)
}

/// The lesson that goes back over a unit: its review lesson, else its
/// first. `None` for an unknown unit.
pub fn unit_refresher(unit_id: &str) -> Option<String> {
    let unit = find_unit(unit_id)?;
    unit.lessons
        .iter()
        .find(|l| l.kind == kabar_core::journey::LessonKind::Review)
        .or(unit.lessons.first())
        .map(|l| l.id.clone())
}

/// A unit's name for the daily plan: "التحية · Salam" (the UI language's
/// title, then the target-language one, isolated so it reads right inside
/// Arabic). `None` for an unknown id.
pub fn unit_title(lang: UiLanguage, unit_id: &str) -> Option<String> {
    let unit = find_unit(unit_id)?;
    let local = local(lang, &unit.title);
    Some(match unit.title.target.as_deref() {
        Some(target) if target != local => format!("{local} · \u{2068}{target}\u{2069}"),
        _ => local.to_string(),
    })
}

/// A lesson's title in its own language (`None` for an unknown id).
pub fn lesson_title(lesson_id: &str) -> Option<String> {
    let routes = ROUTES.as_ref().ok()?;
    routes.id.lessons().chain(routes.jv.lessons()).find(|l| l.id == lesson_id).map(|l| l.title.clone())
}

/// The lesson a plan's "Start" opens: the unit's first unfinished lesson.
/// `None` for an unknown unit or one already finished.
pub fn next_lesson_in(unit_id: &str, completed: &BTreeSet<String>) -> Option<String> {
    find_unit(unit_id)?.lessons.iter().find(|l| !completed.contains(&l.id)).map(|l| l.id.clone())
}

/// Where the Indonesian journey starts: the placement result (its unit when
/// the test could tell, else its level), else the level the onboarding answer
/// points at (`None`: from the very beginning). Feed it to
/// [`Progress::start_at`]; it is not always a level id.
pub fn start_level(db: &Db, profile: Option<&CoachProfile>) -> Option<String> {
    let placed = db.0.borrow().placement().unwrap_or_else(|e| {
        crate::diag::report!("reading the placement result failed: {e}");
        None
    });
    placed.map(|p| p.start_unit.unwrap_or(p.start_level)).or_else(|| {
        let unit = kabar_core::coach::onboarding::starting_unit(profile?.start);
        unit.split_once('.').map(|(level, _)| level.to_string())
    })
}

/// The level of `id`'s unit (`bipa2.u03` → `bipa2`).
pub fn level_of(unit: &str) -> String {
    unit.split_once('.').map_or(unit, |(l, _)| l).to_string()
}

/// (Indonesian level, Javanese level once that journey has started).
pub fn learner_levels(db: &Db, profile: Option<&CoachProfile>) -> (String, Option<String>) {
    let completed = db.0.borrow().completed_lessons().unwrap_or_default();
    let start = start_level(db, profile);
    let id = current_unit(&completed, start.as_deref(), false)
        .map(|u| level_of(&u))
        // Placement may start inside a level (a unit id).
        .or(start.as_deref().map(level_of))
        .unwrap_or_else(|| "bipa1".into());
    let jv = completed
        .iter()
        .any(|l| l.starts_with('j'))
        .then(|| current_unit(&completed, None, true).map(|u| level_of(&u)))
        .flatten();
    (id, jv)
}

/// The unit the learner is on: the one holding the Journey's next lesson
/// (Javanese when that is the learner's only focus).
pub fn current_unit(completed: &BTreeSet<String>, start: Option<&str>, javanese: bool) -> Option<String> {
    let routes = ROUTES.as_ref().ok()?;
    let curriculum = if javanese { &routes.jv } else { &routes.id };
    let progress = Progress::new(curriculum, completed);
    let progress = match start {
        Some(level) if !javanese => progress.start_at(curriculum, level),
        _ => progress,
    };
    let lesson = progress.current()?;
    curriculum
        .levels
        .iter()
        .flat_map(|l| &l.units)
        .find(|u| u.lessons.iter().any(|l| l.id == lesson))
        .map(|u| u.id.clone())
}

