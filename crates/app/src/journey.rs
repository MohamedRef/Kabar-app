//! The Journey Map (SPEC §10): BIPA islands and the Javanese train line.
//!
//! Phones get a vertical scrolling route; tablets and the Mac get the full
//! stylized map (generated from Natural Earth by `kabar-tools journey-map`)
//! with pan and zoom, and a side panel for the chosen place. Nothing is
//! locked: every lesson opens, and "Take a flight" opens the level test.

mod suitcase;
use suitcase::*;

mod catalog;
pub use catalog::journal_catalog;
use catalog::*;

mod map_view;
use map_view::*;

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::{LazyLock, Mutex};

use dioxus::prelude::*;
use kabar_core::journey::layout::{LevelPin, MapLayout, RouteStyle, SegmentKind};
use kabar_core::journey::{
    Curriculum, LessonKind, LessonState, Level, Progress, Text, Unit, parse_curriculum, souvenirs,
};
use kabar_core::layout::WidthClass;
use kabar_core::locale::UiLanguage;

use crate::embedded::{Css, css};
use crate::i18n::Msg;
use crate::map_art::{self, Size as ArtSize};
use crate::views::Layout;
use crate::{Db, Ui};
use kabar_protocol::coach::CoachProfile;
use kabar_protocol::lesson::Souvenir;

mod text;
mod today;
use today::find_unit;
pub use today::{
    current_unit, learner_levels, lesson_title, next_lesson_in, start_level, unit_refresher, unit_title,
};

use text::{T, tt};

const JOURNEY_CSS: Css = css!("assets/journey.css");

struct Routes {
    id: Curriculum,
    jv: Curriculum,
    id_map: MapLayout,
    jv_map: MapLayout,
}

static ROUTES: LazyLock<Result<Routes, String>> = LazyLock::new(|| {
    use crate::embedded::{CURRICULUM_ID, CURRICULUM_JV, text};
    let id = parse_curriculum(&text(CURRICULUM_ID)).map_err(|e| e.to_string())?;
    let jv = parse_curriculum(&text(CURRICULUM_JV)).map_err(|e| e.to_string())?;
    let (mut id_map, mut jv_map) = (MapLayout::new(&id), MapLayout::new(&jv));
    for map in [&mut id_map, &mut jv_map] {
        map.spread_pins(PIN_GAP * pin_scale(map));
    }
    Ok(Routes { id, jv, id_map, jv_map })
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Indonesian,
    Javanese,
}

impl Route {
    fn parts(self, r: &Routes) -> (&Curriculum, &MapLayout) {
        match self {
            Self::Indonesian => (&r.id, &r.id_map),
            Self::Javanese => (&r.jv, &r.jv_map),
        }
    }

    fn base_map(self) -> &'static str {
        match self {
            Self::Indonesian => INDONESIA_SVG,
            Self::Javanese => JAVA_SVG,
        }
    }

    fn target_lang(self) -> &'static str {
        match self {
            Self::Indonesian => "id",
            Self::Javanese => "jv",
        }
    }
}

/// Everything the Journey components share for one render.
#[derive(Clone, PartialEq)]
struct Journey {
    route: Route,
    completed: Rc<BTreeSet<String>>,
    /// Lessons passed with a reasonable score: their Javanese counts as approved.
    approved: Rc<BTreeSet<String>>,
    /// Where the placement test started the journey (e.g. `bipa2`).
    start: Option<String>,
}

impl Journey {
    fn routes(&self) -> &'static Routes {
        ROUTES.as_ref().expect("checked by JourneyScreen")
    }

    fn curriculum(&self) -> &'static Curriculum {
        self.route.parts(self.routes()).0
    }

    fn map(&self) -> &'static MapLayout {
        self.route.parts(self.routes()).1
    }

    fn progress(&self) -> Progress {
        let p = Progress::new(self.curriculum(), &self.completed).with_approved(&self.approved);
        match &self.start {
            Some(level) => p.start_at(self.curriculum(), level),
            None => p,
        }
    }

    /// Levels done in the other route (J1 waits, gently, for BIPA 1).
    fn done_elsewhere(&self) -> BTreeSet<String> {
        let r = self.routes();
        let other = match self.route {
            Route::Indonesian => &r.jv,
            Route::Javanese => &r.id,
        };
        Progress::new(other, &self.completed).done_levels().clone()
    }
}

/// A curriculum built into the app, by language (`id`, `jv`).
pub fn curriculum(language: &str) -> Option<&'static Curriculum> {
    let r = ROUTES.as_ref().ok()?;
    match language {
        "id" => Some(&r.id),
        "jv" => Some(&r.jv),
        _ => None,
    }
}

pub fn local(lang: UiLanguage, text: &Text) -> &str {
    match lang {
        UiLanguage::ArEg | UiLanguage::Ar => &text.ar,
        UiLanguage::En => &text.en,
    }
}

fn kind_label(kind: LessonKind) -> Msg {
    match kind {
        LessonKind::Practice => Msg::LessonPractice,
        LessonKind::Review => Msg::LessonReview,
        LessonKind::Learn | LessonKind::Other => Msg::LessonLearn,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Route,
    /// The map on a phone, which otherwise shows the route as a list (#380).
    Map,
    Suitcase,
}

/// A souvenir to show in the Suitcase when Journey next opens (the
/// Journal's "new souvenir" line).
static SUITCASE_AT: GlobalSignal<Option<String>> = Signal::global(|| None);

/// Opens Journey on the Suitcase, scrolled to the souvenir of `unit_id`.
pub fn open_suitcase(ui: Ui, unit_id: &str) {
    *SUITCASE_AT.write() = Some(unit_id.to_string());
    let mut screen = ui.screen;
    screen.set(crate::Screen::Journey);
}

#[component]
pub fn JourneyScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    // Re-read on every visit: the screen remounts when the player closes.
    let completed = use_hook(|| {
        Rc::new(db.0.borrow().completed_lessons().unwrap_or_else(|e| {
            crate::diag::report!("reading finished lessons failed: {e}");
            BTreeSet::new()
        }))
    });
    let approved = use_hook(|| {
        Rc::new(db.0.borrow().approved_lessons().unwrap_or_else(|e| {
            crate::diag::report!("reading approved lessons failed: {e}");
            BTreeSet::new()
        }))
    });
    let coach = use_context::<crate::coach::Coach>();
    let start = use_hook(|| start_level(&db, coach.profile.peek().as_ref()));
    // Opens on the learner's own route (Javanese when that is their only focus).
    let mut route = use_signal(|| match coach.profile.peek().as_ref().map(|p| p.focus) {
        Some(kabar_protocol::coach::LanguageFocus::Javanese) => Route::Javanese,
        _ => Route::Indonesian,
    });
    let mut pane = use_signal(|| if SUITCASE_AT.peek().is_some() { Pane::Suitcase } else { Pane::Route });
    // The stop picked on each route lives here, above the phone list and the
    // map, so it survives a window resize or a trip to the suitcase
    // (list-detail rule, docs/DESIGN.md). `None` = not picked yet.
    let pick_id = use_signal(|| None::<Option<String>>);
    let pick_jv = use_signal(|| None::<Option<String>>);

    let routes = match &*ROUTES {
        Ok(r) => r,
        Err(e) => {
            return rsx! {
                style { {JOURNEY_CSS.text()} }
                crate::ui::DivCard {
                    h2 { "{ui.t(Msg::NavJourney)}" }
                    p { "{ui.t(Msg::JourneyLoadError)}" }
                    p { class: "mono", dir: "ltr", "{e}" }
                }
            };
        }
    };
    let j = Journey {
        route: route(),
        completed: completed.clone(),
        approved: approved.clone(),
        start: start.clone(),
    };
    let suitcase_count = [&routes.id, &routes.jv]
        .into_iter()
        .map(|c| souvenirs(c, &Progress::new(c, &completed)).len())
        .sum::<usize>();
    let compact = *ui.width.read() == WidthClass::Compact;
    let picked = move || if route() == Route::Indonesian { pick_id } else { pick_jv };

    rsx! {
        style { {JOURNEY_CSS.text()} }
        style { {crate::map_art::paper_css()} }
        div { class: "journey",
            header { class: "journey-bar",
                div { class: "seg", role: "group", aria_label: "{ui.t(Msg::NavJourney)}",
                    for (r, label) in [(Route::Indonesian, Msg::JourneyIndonesian), (Route::Javanese, Msg::JourneyJavanese)] {
                        button {
                            key: "{r:?}",
                            aria_pressed: if route() == r && pane() != Pane::Suitcase { "true" } else { "false" },
                            onclick: move |_| {
                                route.set(r);
                                // A phone on the map stays on the map.
                                if pane() == Pane::Suitcase {
                                    pane.set(Pane::Route);
                                }
                            },
                            span { class: "seg-icon", aria_hidden: "true",
                                if r == Route::Indonesian { {icon_plane()} } else { {icon_train()} }
                            }
                            "{ui.t(label)}"
                        }
                    }
                }
                div { class: "journey-tools",
                    if compact {
                        button {
                            class: "map-toggle",
                            title: "{ui.t(Msg::JourneyMap)}",
                            aria_pressed: if pane() == Pane::Map { "true" } else { "false" },
                            onclick: move |_| {
                                let open = pane() != Pane::Map;
                                pane.set(if open { Pane::Map } else { Pane::Route });
                                if open {
                                    // The list may have scrolled far down: bring the map up.
                                    document::eval(
                                        "requestAnimationFrame(() => document.querySelector('.journey')?.scrollIntoView({ block: 'start' }));",
                                    );
                                }
                            },
                            span { class: "seg-icon", aria_hidden: "true", {icon_map()} }
                            span { class: "tool-label", "{ui.t(Msg::JourneyMap)}" }
                        }
                    }
                    button {
                        class: "suitcase-toggle",
                        title: "{ui.t(Msg::JourneySuitcase)}",
                        aria_pressed: if pane() == Pane::Suitcase { "true" } else { "false" },
                        onclick: move |_| pane.set(if pane() == Pane::Suitcase { Pane::Route } else { Pane::Suitcase }),
                        span { class: "seg-icon", aria_hidden: "true", {icon_suitcase()} }
                        span { class: "tool-label", "{ui.t(Msg::JourneySuitcase)} " }
                        span { class: "count", "{ui.num(suitcase_count as u64)}" }
                    }
                }
            }
            if pane() != Pane::Suitcase {
                {now_card(&ui, &j, &j.progress())}
            }
            if pane() == Pane::Suitcase {
                Suitcase { completed: completed.clone(), approved: approved.clone() }
            } else if compact && pane() == Pane::Route {
                RouteList { key: "{j.route:?}", j: j.clone(), picked: picked() }
            } else {
                MapView { key: "{j.route:?}", j: j.clone(), picked: picked() }
            }
        }
    }
}

// ---------------------------------------------------------------- phone ----

/// Each stop's lessons: rows (the path's own look), tiles two across, or
/// one line each with the state left to the dot. The course order always:
/// the path is the order.
const LESSONS_VIEW: crate::views::Collection =
    crate::views::Collection { key: "journey.lessons", layout: Layout::List, sorts: &[] };

#[component]
fn RouteList(j: Journey, picked: Signal<Option<Option<String>>>) -> Element {
    let ui = use_context::<Ui>();
    let cur = j.curriculum();
    let progress = j.progress();
    let elsewhere = j.done_elsewhere();
    // Bring the stop picked on the map (before a resize) into view once,
    // gently. Otherwise the list opens at the top, under the current-unit
    // card, which already leads to the next lesson.
    let target = picked.peek().clone().flatten().map(|level| format!("rl-{level}"));
    let block = "start";
    use_effect(move || {
        let Some(target) = target.clone() else { return };
        document::eval(&format!(
            r#"const el = document.getElementById({target:?});
            if (el) {{
              const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
              el.scrollIntoView({{ block: "{block}", behavior: still ? "auto" : "smooth" }});
            }}"#
        ));
    });

    let layout = LESSONS_VIEW.pick(&use_context::<crate::Db>()).layout;
    // Mark the active stop's level, without changing curriculum order or
    // locking any completed/unstarted lesson.
    let active_level = progress.current().and_then(|id| {
        cur.levels
            .iter()
            .find(|level| level.units.iter().any(|unit| unit.lessons.iter().any(|lesson| lesson.id == id)))
            .map(|level| level.id.as_str())
    });
    rsx! {
        div { class: "route-list",
            crate::views::ViewBar { of: LESSONS_VIEW }
            for level in cur.levels.iter() {
                section {
                    key: "{level.id}",
                    id: "rl-{level.id}",
                    class: "rl-level",
                    "data-current": if active_level == Some(level.id.as_str()) { "true" } else { "false" },
                    {level_head(&ui, &j, level, &progress, &elsewhere)}
                    {unit_stops(&ui, &j, level, &progress, true, layout)}
                }
            }
            {continue_bar(&ui, &j, &progress)}
        }
    }
}

fn continue_bar(ui: &Ui, j: &Journey, progress: &Progress) -> Element {
    let ui = *ui;
    let Some(id) = progress.current() else {
        return rsx! {
            crate::ui::Card { element: crate::ui::CardElement::Paragraph, class: "all-done", crate::review::Isolated { text: ui.t(Msg::JourneyAllDone).to_string() } }
        };
    };
    let title = j.curriculum().lessons().find(|l| l.id == id).map(|l| l.title.clone()).unwrap_or_default();
    let id = id.to_string();
    rsx! {
        div { class: "continue-bar",
            button { class: "primary continue", onclick: move |_| ui.open_lesson(id.clone()),
                span { class: "continue-word", bdi { lang: "id", dir: "ltr", "Lanjut" } }
                span { class: "continue-sub",
                    "{ui.t(Msg::JourneyContinue)}: "
                    bdi { lang: j.route.target_lang(), dir: "ltr", "{title}" }
                }
            }
        }
    }
}

/// Where the learner is: the unit holding the next lesson, its city, what
/// the next lesson lets them do, how far through the unit they are, and the
/// nearest milestone (the stop's souvenir, or the level test once every
/// stop is done). The card opens the next lesson; the milestone row opens
/// the lesson that gives the souvenir, or the test.
fn now_card(ui: &Ui, j: &Journey, progress: &Progress) -> Element {
    let ui = *ui;
    let lang = ui.lang();
    let tl = j.route.target_lang();
    let Some(current) = progress.current().map(str::to_string) else { return rsx! {} };
    let cur = j.curriculum();
    let Some((level, unit)) = cur
        .levels
        .iter()
        .find_map(|l| l.units.iter().find(|u| u.lessons.iter().any(|x| x.id == current)).map(|u| (l, u)))
    else {
        return rsx! {};
    };
    let (done, total) = progress.unit_counts(unit);
    let pct = (done * 100).checked_div(total).unwrap_or(0);
    let lesson = unit.lessons.iter().find(|l| l.id == current);
    let can_do = crate::lesson::lesson_content(&current)
        .and_then(|c| kabar_core::lesson::parse_lesson(&c).ok())
        .map(|l| kabar_core::lesson::pick(&l.can_do, lang).to_string())
        .filter(|c| !c.is_empty());
    let (done_s, total_s) = (ui.num(done as u64), ui.num(total as u64));
    // The nearest milestone: this stop's souvenir, else the level test when
    // this is the level's last unfinished stop.
    let last_stop = level.units.iter().filter(|u| !progress.unit_done(u)).count() <= 1;
    let milestone = match (&unit.culture, &level.level_test) {
        (Some(c), _) => souvenir_lesson(&unit.id).map(|id| {
            let name = local_souvenir(&unit.id, lang).map_or(c.souvenir.clone(), |(t, _)| t);
            (tt(lang, T::SouvenirHere), name, id)
        }),
        (None, Some(test)) if last_stop => {
            Some((tt(lang, T::LevelTestNext), level.code.clone(), test.id.clone()))
        }
        _ => None,
    };
    let open = current.clone();
    rsx! {
        section { class: "journey-now", aria_label: "{tt(lang, T::NowStop)}",
            button {
                class: "now-card tap",
                onclick: move |_| ui.open_lesson(open.clone()),
                span { class: "now-eyebrow",
                    span { class: "status-chip latin", bdi { "{level.code}" } }
                    " {tt(lang, T::NowStop)}"
                }
                span { class: "now-city", bdi { lang: tl, dir: "ltr", "{unit.stop}" } }
                span { class: "now-unit", "{local(lang, &unit.title)}" }
                if let Some(l) = lesson {
                    span { class: "now-lesson",
                        "{tt(lang, T::Continue)}: "
                        bdi { lang: tl, dir: "ltr", "{l.title}" }
                    }
                }
                if let Some(c) = can_do {
                    span { class: "now-goal muted", "{tt(lang, T::Goal)} " "{c}" }
                }
                span { class: "now-progress",
                    span {
                        class: "progress-line",
                        role: "progressbar",
                        aria_valuemin: "0",
                        aria_valuemax: "{total}",
                        aria_valuenow: "{done}",
                        aria_label: "{unit.stop}",
                        span { class: "fill", style: "inline-size: {pct}%" }
                    }
                    span { class: "count", "{text::lessons_of(lang, &done_s, &total_s, total)}" }
                }
            }
            if let Some((label, name, id)) = milestone {
                button {
                    class: "activity-row now-milestone",
                    onclick: move |_| ui.open_lesson(id.clone()),
                    span { class: "row-icon gold", aria_hidden: "true", {icon_suitcase()} }
                    span { class: "row-copy",
                        small { "{label}" }
                        strong { bdi { lang: tl, dir: "ltr", "{name}" } }
                    }
                    span { class: "row-end", span { class: "chev", aria_hidden: "true" } }
                }
            }
        }
    }
}

// ------------------------------------------------------- shared blocks ----

fn level_head(
    ui: &Ui,
    j: &Journey,
    level: &Level,
    progress: &Progress,
    elsewhere: &BTreeSet<String>,
) -> Element {
    let ui = *ui;
    let lang = ui.settings.read().ui_language;
    let (done, total) = progress.level_counts(level);
    let finished = progress.level_done(&level.id);
    let cur = j.curriculum();
    let waiting: Vec<String> = cur
        .external_requires(level)
        .filter(|r| !elsewhere.contains(*r))
        .map(|r| j.routes().id.level(r).map_or(r.to_string(), |l| l.code.clone()))
        .collect();
    let flight = match j.map().style {
        RouteStyle::Islands => Msg::JourneyTakeFlight,
        RouteStyle::Rail => Msg::JourneyTakeExpress,
    };
    let test = level.level_test.as_ref().map(|t| t.id.clone());
    let pct = (done * 100).checked_div(total).unwrap_or(0);
    let size = if *ui.width.read() == WidthClass::Compact { ArtSize::Small } else { ArtSize::Large };
    let art = map_art::vignette(&level.id, size);
    rsx! {
        header { class: "level-head", "data-done": if finished { "true" } else { "false" },
            // The level's name opens its next lesson (its first once it's done).
            button {
                class: "level-title tap tap-plain",
                onclick: {
                    let next = level.lessons().find(|l| progress.lesson_state(&l.id) != LessonState::Done)
                        .or_else(|| level.lessons().next())
                        .map(|l| l.id.clone());
                    move |_| if let Some(id) = &next { ui.open_lesson(id.clone()) }
                },
                if let Some(src) = art {
                    img { class: "level-art", src: "{src}", alt: "", aria_hidden: "true", draggable: "false" }
                } else {
                    span { class: "level-icon", aria_hidden: "true",
                        if j.route == Route::Indonesian { {icon_island()} } else { {icon_train()} }
                    }
                }
                div {
                    h2 {
                        bdi { lang: j.route.target_lang(), dir: "ltr", "{level.place()}" }
                        " "
                        span { class: "pill latin", bdi { "{level.code}" } }
                        if cur.unverified() && !progress.level_approved(level) {
                            " "
                            span { class: "badge-unverified", "{ui.t(Msg::JourneyUnverified)}" }
                        }
                    }
                    p { class: "muted level-sub",
                        "{local(lang, &level.title)}"
                        if let Some(t) = &level.title.target {
                            " · "
                            bdi { lang: j.route.target_lang(), dir: "ltr", "{t}" }
                        }
                    }
                }
                span { class: "chev", aria_hidden: "true" }
            }
            div { class: "level-progress",
                div {
                    class: "bar",
                    role: "progressbar",
                    aria_valuemin: "0",
                    aria_valuemax: "{total}",
                    aria_valuenow: "{done}",
                    aria_label: "{level.code}",
                    div { class: "fill", style: "inline-size: {pct}%" }
                }
                span { class: "count", if finished { "{ui.t(Msg::JourneyDone)} · " } "{ui.num(done as u64)}/{ui.num(total as u64)}" }
            }
            if !waiting.is_empty() {
                p { class: "gentle-note",
                    "{ui.t(Msg::JourneyBestAfter)} "
                    for w in waiting {
                        bdi { key: "{w}", class: "latin", "{w}" }
                    }
                    ". {ui.t(Msg::JourneyOpenAnyway)}"
                }
            }
            if let Some(test) = test {
                if !finished {
                    button { class: "flight", onclick: move |_| ui.open_lesson(test.clone()),
                        span { class: "seg-icon", aria_hidden: "true",
                            if flight == Msg::JourneyTakeFlight { {icon_plane()} } else { {icon_train()} }
                        }
                        "{ui.t(flight)}"
                    }
                }
            }
        }
    }
}

/// The units of a level as stops on a winding path, lessons inside each.
fn unit_stops(
    ui: &Ui,
    j: &Journey,
    level: &Level,
    progress: &Progress,
    mark_current: bool,
    layout: Layout,
) -> Element {
    let ui = *ui;
    let lang = ui.settings.read().ui_language;
    let tl = j.route.target_lang();
    rsx! {
        ol { class: "stops",
            for (i, unit) in level.units.iter().enumerate() {
                li {
                    key: "{unit.id}",
                    class: "stop",
                    "data-side": if i % 2 == 0 { "a" } else { "b" },
                    "data-done": if progress.unit_done(unit) { "true" } else { "false" },
                    "data-current": if progress.holds_current(unit) { "true" } else { "false" },
                    div { class: "stop-rail", aria_hidden: "true",
                        if i + 1 < level.units.len() {
                            svg {
                                class: "stop-curve",
                                view_box: "0 0 52 100",
                                preserve_aspect_ratio: "none",
                                path { d: if i % 2 == 0 { "M16 0C16 55 36 45 36 100" } else { "M36 0C36 55 16 45 16 100" } }
                            }
                        }
                        span { class: "stop-node" }
                    }
                    div { class: "stop-body",
                        // The stop's name opens the unit's next lesson.
                        h3 {
                            button {
                                class: "stop-head tap tap-plain",
                                onclick: {
                                    let next = unit.lessons.iter().find(|l| progress.lesson_state(&l.id) != LessonState::Done)
                                        .or(unit.lessons.first())
                                        .map(|l| l.id.clone());
                                    move |_| if let Some(id) = &next { ui.open_lesson(id.clone()) }
                                },
                                bdi { lang: tl, dir: "ltr", "{unit.stop}" }
                                span { class: "stop-unit muted",
                                    " · {local(lang, &unit.title)}"
                                }
                            }
                        }
                        {lessons(&ui, tl, unit, progress, mark_current, layout)}
                    }
                }
            }
        }
    }
}

fn lessons(
    ui: &Ui,
    tl: &'static str,
    unit: &Unit,
    progress: &Progress,
    mark_current: bool,
    layout: Layout,
) -> Element {
    let ui = *ui;
    rsx! {
        ol { class: "lessons", "data-layout": layout.as_str(),
            for lesson in unit.lessons.iter() {
                {
                    let state = progress.lesson_state(&lesson.id);
                    let state_attr = match state {
                        LessonState::Done => "done",
                        LessonState::Current => "current",
                        LessonState::Open => "open",
                    };
                    // Done / Continue / Upcoming, in words (nothing is locked).
                    let state_text = match state {
                        LessonState::Done => ui.t(Msg::JourneyDone),
                        LessonState::Current => tt(ui.lang(), T::Continue),
                        LessonState::Open => tt(ui.lang(), T::Upcoming),
                    };
                    let id = lesson.id.clone();
                    let kind = ui.t(kind_label(lesson.kind));
                    let minutes = crate::coach::labels::minutes(&ui, lesson.minutes);
                    let label = format!("{}, {kind}, {minutes}, {state_text}", lesson.title);
                    rsx! {
                        li { key: "{lesson.id}",
                            button {
                                class: "lesson",
                                id: if mark_current && state == LessonState::Current { "journey-current" },
                                "data-state": state_attr,
                                "data-kind": kind_attr(lesson.kind),
                                aria_label: "{label}",
                                aria_current: if state == LessonState::Current { "step" },
                                onclick: move |_| ui.open_lesson(id.clone()),
                                span { class: "lesson-dot", aria_hidden: "true",
                                    if state == LessonState::Done { {icon_check()} }
                                }
                                span { class: "lesson-text",
                                    bdi { class: "lesson-title", lang: tl, dir: "ltr", "{lesson.title}" }
                                    span { class: "lesson-meta muted",
                                        "{kind} · {minutes} · "
                                        span { class: "lesson-state", "{state_text}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn kind_attr(kind: LessonKind) -> &'static str {
    match kind {
        LessonKind::Learn => "learn",
        LessonKind::Practice => "practice",
        LessonKind::Review => "review",
        LessonKind::Other => "other",
    }
}

// ---------------------------------------------------------------- icons ----

fn stroke_icon(d: &'static str) -> Element {
    rsx! {
        svg {
            width: "20",
            height: "20",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d }
        }
    }
}

fn icon_plane() -> Element {
    stroke_icon("M3 13l7-2 4-7h2l-2 7 6 1 1 2-7 1-3 5h-2l1-5-6-1z")
}
fn icon_train() -> Element {
    stroke_icon(
        "M7 3h10a2 2 0 0 1 2 2v9a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3V5a2 2 0 0 1 2-2zM5 10h14M8 21l2-4M16 21l-2-4M9 14h.01M15 14h.01",
    )
}
fn icon_island() -> Element {
    stroke_icon(
        "M3 18c3-2 6-2 9 0s6 2 9 0M7 16c1-4 4-6 8-6M14 10c-1-3 0-5 2-6M14 10c2-1 4 0 5 2M14 10c-2-2-5-2-6 0",
    )
}
fn icon_map() -> Element {
    stroke_icon("M3 6l6-2 6 2 6-2v14l-6 2-6-2-6 2zM9 4v14M15 6v14")
}
fn icon_suitcase() -> Element {
    stroke_icon("M4 8h16v11H4zM9 8V5h6v3M4 13h16M9 13v2M15 13v2")
}
fn icon_check() -> Element {
    stroke_icon("M5 12l5 5 9-10")
}
fn icon_reset() -> Element {
    stroke_icon("M4 12a8 8 0 1 0 3-6.2M4 4v4h4")
}

#[cfg(test)]
mod tests;
