//! The Journey Map (SPEC §10): BIPA islands and the Javanese train line.
//!
//! Phones get a vertical scrolling route; tablets and the Mac get the full
//! stylized map (generated from Natural Earth by `kabar-tools journey-map`)
//! with pan and zoom, and a side panel for the chosen place. Nothing is
//! locked: every lesson opens, and "Take a flight" opens the level test.

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use dioxus::prelude::*;
use kabar_core::journey::layout::{MapLayout, RouteStyle, SegmentKind};
use kabar_core::journey::{
    Curriculum, LessonKind, LessonState, Level, Progress, Text, Unit, parse_curriculum, souvenirs,
};
use kabar_core::layout::WidthClass;
use kabar_core::locale::{UiLanguage, format_number};

use crate::i18n::Msg;
use crate::{Db, Ui};

// TODO: switch to curriculum/*.yaml once the curriculum PR lands.
const CURRICULUM_ID: &str = include_str!("../assets/fixtures/curriculum-id.yaml");
// TODO: switch to curriculum/*.yaml once the curriculum PR lands.
const CURRICULUM_JV: &str = include_str!("../assets/fixtures/curriculum-jv.yaml");

const JOURNEY_CSS: &str = include_str!("../assets/journey.css");
const PAN_ZOOM_JS: &str = include_str!("../assets/journey.js");
const INDONESIA_SVG: &str = include_str!("../assets/journey/indonesia.svg");
const JAVA_SVG: &str = include_str!("../assets/journey/java.svg");

struct Routes {
    id: Curriculum,
    jv: Curriculum,
    id_map: MapLayout,
    jv_map: MapLayout,
}

static ROUTES: LazyLock<Result<Routes, String>> = LazyLock::new(|| {
    let id = parse_curriculum(CURRICULUM_ID).map_err(|e| e.to_string())?;
    let jv = parse_curriculum(CURRICULUM_JV).map_err(|e| e.to_string())?;
    let (id_map, jv_map) = (MapLayout::new(&id), MapLayout::new(&jv));
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

/// The `d` of the generated `<path class="{class}">`.
fn land_path(svg: &'static str, class: &str) -> &'static str {
    let Some(at) = svg.find(&format!("class=\"{class}\"")) else { return "" };
    let rest = &svg[at..];
    let Some(start) = rest.find(" d=\"").map(|i| i + 4) else { return "" };
    let end = rest[start..].find('"').map_or(start, |i| start + i);
    &rest[start..end]
}

/// Everything the Journey components share for one render.
#[derive(Clone, PartialEq)]
struct Journey {
    route: Route,
    completed: Rc<BTreeSet<String>>,
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
        Progress::new(self.curriculum(), &self.completed)
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

fn local(lang: UiLanguage, text: &Text) -> &str {
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

fn num(ui: &Ui, n: usize) -> String {
    let s = ui.settings.read();
    format_number(n as u64, s.ui_language, s.eastern_digits)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Route,
    Suitcase,
}

#[component]
pub fn JourneyScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    // Re-read on every visit: the screen remounts when the player closes.
    let completed = use_hook(|| {
        Rc::new(db.0.borrow().completed_lessons().unwrap_or_else(|e| {
            eprintln!("kabar: reading finished lessons failed: {e}");
            BTreeSet::new()
        }))
    });
    let mut route = use_signal(|| Route::Indonesian);
    let mut pane = use_signal(|| Pane::Route);

    let routes = match &*ROUTES {
        Ok(r) => r,
        Err(e) => {
            return rsx! {
                style { {JOURNEY_CSS} }
                div { class: "card",
                    h2 { "{ui.t(Msg::NavJourney)}" }
                    p { "{ui.t(Msg::JourneyLoadError)}" }
                    p { class: "mono", dir: "ltr", "{e}" }
                }
            };
        }
    };
    let j = Journey { route: route(), completed: completed.clone() };
    let suitcase_count = [&routes.id, &routes.jv]
        .into_iter()
        .map(|c| souvenirs(c, &Progress::new(c, &completed)).len())
        .sum::<usize>();
    let compact = *ui.width.read() == WidthClass::Compact;

    rsx! {
        style { {JOURNEY_CSS} }
        div { class: "journey",
            header { class: "journey-bar",
                div { class: "seg", role: "group", aria_label: "{ui.t(Msg::NavJourney)}",
                    for (r, label) in [(Route::Indonesian, Msg::JourneyIndonesian), (Route::Javanese, Msg::JourneyJavanese)] {
                        button {
                            key: "{r:?}",
                            aria_pressed: if route() == r && pane() == Pane::Route { "true" } else { "false" },
                            onclick: move |_| {
                                route.set(r);
                                pane.set(Pane::Route);
                            },
                            span { class: "seg-icon", aria_hidden: "true",
                                if r == Route::Indonesian { {icon_plane()} } else { {icon_train()} }
                            }
                            "{ui.t(label)}"
                        }
                    }
                }
                button {
                    class: "suitcase-toggle",
                    aria_pressed: if pane() == Pane::Suitcase { "true" } else { "false" },
                    onclick: move |_| pane.set(if pane() == Pane::Suitcase { Pane::Route } else { Pane::Suitcase }),
                    span { class: "seg-icon", aria_hidden: "true", {icon_suitcase()} }
                    "{ui.t(Msg::JourneySuitcase)} "
                    span { class: "count", "{num(&ui, suitcase_count)}" }
                }
            }
            if pane() == Pane::Suitcase {
                Suitcase { completed: completed.clone() }
            } else if compact {
                RouteList { key: "{j.route:?}", j: j.clone() }
            } else {
                MapView { key: "{j.route:?}", j: j.clone() }
            }
        }
    }
}

// ---------------------------------------------------------------- phone ----

#[component]
fn RouteList(j: Journey) -> Element {
    let ui = use_context::<Ui>();
    let cur = j.curriculum();
    let progress = j.progress();
    let elsewhere = j.done_elsewhere();
    // Bring the current lesson into view once, gently.
    use_effect(|| {
        document::eval(
            r#"const el = document.getElementById("journey-current");
            if (el) {
              const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
              el.scrollIntoView({ block: "center", behavior: still ? "auto" : "smooth" });
            }"#,
        );
    });

    rsx! {
        div { class: "route-list",
            for level in cur.levels.iter() {
                section { key: "{level.id}", class: "rl-level",
                    {level_head(&ui, &j, level, &progress, &elsewhere)}
                    {unit_stops(&ui, &j, level, &progress, true)}
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
            p { class: "all-done card", "{ui.t(Msg::JourneyAllDone)}" }
        };
    };
    let title = j.curriculum().lessons().find(|l| l.id == id).map(|l| l.title.clone()).unwrap_or_default();
    let id = id.to_string();
    rsx! {
        div { class: "continue-bar",
            button { class: "primary continue", onclick: move |_| ui.open_lesson(id.clone()),
                span { class: "continue-word", bdi { lang: "id", "Lanjut" } }
                span { class: "continue-sub",
                    "{ui.t(Msg::JourneyContinue)}: "
                    bdi { lang: j.route.target_lang(), "{title}" }
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
    rsx! {
        header { class: "level-head", "data-done": if finished { "true" } else { "false" },
            div { class: "level-title",
                span { class: "level-icon", aria_hidden: "true",
                    if j.route == Route::Indonesian { {icon_island()} } else { {icon_train()} }
                }
                div {
                    h2 {
                        bdi { lang: j.route.target_lang(), "{level.place()}" }
                        " "
                        span { class: "pill latin", bdi { "{level.code}" } }
                        if cur.unverified() {
                            " "
                            span { class: "badge-unverified", "{ui.t(Msg::JourneyUnverified)}" }
                        }
                    }
                    p { class: "muted level-sub",
                        "{local(lang, &level.title)}"
                        if let Some(t) = &level.title.target {
                            " · "
                            bdi { lang: j.route.target_lang(), "{t}" }
                        }
                    }
                }
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
                span { class: "count", if finished { "{ui.t(Msg::JourneyDone)} · " } "{num(&ui, done)}/{num(&ui, total)}" }
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
fn unit_stops(ui: &Ui, j: &Journey, level: &Level, progress: &Progress, mark_current: bool) -> Element {
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
                        h3 {
                            bdi { lang: tl, "{unit.stop}" }
                            span { class: "stop-unit muted",
                                " · {local(lang, &unit.title)}"
                            }
                        }
                        {lessons(&ui, tl, unit, progress, mark_current)}
                    }
                }
            }
        }
    }
}

fn lessons(ui: &Ui, tl: &'static str, unit: &Unit, progress: &Progress, mark_current: bool) -> Element {
    let ui = *ui;
    rsx! {
        ol { class: "lessons",
            for lesson in unit.lessons.iter() {
                {
                    let state = progress.lesson_state(&lesson.id);
                    let (state_attr, state_msg) = match state {
                        LessonState::Done => ("done", Some(Msg::JourneyDone)),
                        LessonState::Current => ("current", Some(Msg::JourneyUpNext)),
                        LessonState::Open => ("open", None),
                    };
                    let id = lesson.id.clone();
                    let kind = ui.t(kind_label(lesson.kind));
                    let minutes = num(&ui, lesson.minutes as usize);
                    let label = format!(
                        "{}, {kind}, {minutes} {}{}",
                        lesson.title,
                        ui.t(Msg::JourneyMinutes),
                        state_msg.map(|m| format!(", {}", ui.t(m))).unwrap_or_default()
                    );
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
                                    bdi { class: "lesson-title", lang: tl, "{lesson.title}" }
                                    span { class: "lesson-meta muted",
                                        "{kind} · {minutes} {ui.t(Msg::JourneyMinutes)}"
                                        if let Some(m) = state_msg {
                                            " · "
                                            span { class: "lesson-state", "{ui.t(m)}" }
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
}

fn kind_attr(kind: LessonKind) -> &'static str {
    match kind {
        LessonKind::Learn => "learn",
        LessonKind::Practice => "practice",
        LessonKind::Review => "review",
        LessonKind::Other => "other",
    }
}

// ------------------------------------------------------- tablet & Mac ----

#[component]
fn MapView(j: Journey) -> Element {
    let ui = use_context::<Ui>();
    let cur = j.curriculum();
    let map = j.map();
    let progress = j.progress();
    let elsewhere = j.done_elsewhere();
    let initial = progress
        .current()
        .and_then(|c| cur.levels.iter().find(|l| l.lessons().any(|x| x.id == c)))
        .or(cur.levels.first())
        .map(|l| l.id.clone());
    let mut selected = use_signal(|| initial);
    let stage_id = format!("jm-stage-{}", j.route.target_lang());
    let script = format!("{PAN_ZOOM_JS}({stage_id:?});");
    let zoom = {
        let stage_id = stage_id.clone();
        move |how: &str| {
            document::eval(&format!("document.getElementById({stage_id:?})?.__kz?.{how};"));
        }
    };
    let (zoom_in, zoom_out, zoom_reset) = (zoom.clone(), zoom.clone(), zoom);
    let map_label = match j.route {
        Route::Indonesian => Msg::JourneyMapOfIndonesia,
        Route::Javanese => Msg::JourneyMapOfJava,
    };
    let base = j.route.base_map();
    let (land_id, land_other) = (land_path(base, "land-id"), land_path(base, "land-other"));
    // Pins are drawn in a 10-unit design space, scaled to the map.
    let k = map.radius / 5.0;
    let current_level = selected.read().clone();
    let panel_level = current_level.as_deref().and_then(|id| cur.level(id));
    let rail = map.style == RouteStyle::Rail;

    rsx! {
        div { class: "jm", "data-panel": if panel_level.is_some() { "open" } else { "closed" },
            div { class: "jm-map",
                div {
                    id: "{stage_id}",
                    class: "jm-stage",
                    tabindex: "0",
                    aria_label: "{ui.t(map_label)}",
                    onmounted: move |_| {
                        document::eval(&script);
                    },
                    svg {
                        view_box: "{map.view.attr()}",
                        preserve_aspect_ratio: "xMidYMid meet",
                        role: "group",
                        "aria-label": "{ui.t(map_label)}",
                        rect {
                            class: "sea",
                            x: "{map.view.x - map.view.w}",
                            y: "{map.view.y - map.view.h}",
                            width: "{map.view.w * 3.0}",
                            height: "{map.view.h * 3.0}",
                        }
                        g { class: "jm-viewport",
                            path { class: "land-other", "fill-rule": "evenodd", d: "{land_other}" }
                            path { class: "land-id", "fill-rule": "evenodd", d: "{land_id}" }
                            g { class: if rail { "jm-route rail" } else { "jm-route islands" }, "aria-hidden": "true",
                                for (i, seg) in map.segments.iter().enumerate() {
                                    g { key: "{i}",
                                    if seg.kind != SegmentKind::Flight {
                                        path { class: "rail-bed", d: "{seg.path_d()}", "stroke-width": "{0.55 * k}" }
                                    }
                                    path {
                                        class: match seg.kind {
                                            SegmentKind::Flight => "seg flight",
                                            SegmentKind::Rail => "seg rail",
                                            SegmentKind::Branch => "seg branch",
                                        },
                                        d: "{seg.path_d()}",
                                        "stroke-width": "{0.35 * k}",
                                        "stroke-dasharray": if seg.kind == SegmentKind::Flight { "{1.6 * k} {1.4 * k}" } else if seg.kind == SegmentKind::Branch { "{1.0 * k} {0.6 * k}" },
                                    }
                                    }
                                }
                            }
                            for (level, pin) in cur.levels.iter().zip(map.pins.iter()) {
                                {
                                    let (done, total) = progress.level_counts(level);
                                    let pct = (done * 100).checked_div(total).unwrap_or(0);
                                    let state = if progress.level_done(&level.id) {
                                        "done"
                                    } else if level.units.iter().any(|u| progress.holds_current(u)) {
                                        "current"
                                    } else {
                                        "open"
                                    };
                                    let is_sel = current_level.as_deref() == Some(level.id.as_str());
                                    let id = level.id.clone();
                                    let id2 = level.id.clone();
                                    let label = format!("{}, {}: {done}/{total}", level.place(), level.code);
                                    let short = level.place().split([',', '(']).next().unwrap_or_default().trim().to_string();
                                    rsx! {
                                        g { key: "{level.id}", class: "jm-level", "data-state": state,
                                            path { class: "unit-loop", d: "{pin.units_d()}", "stroke-width": "{0.25 * k}" }
                                            for (u, (ux, uy)) in level.units.iter().zip(pin.units.iter()) {
                                                circle {
                                                    key: "{u.id}",
                                                    class: if progress.unit_done(u) { "unit-dot done" } else { "unit-dot" },
                                                    cx: "{ux}",
                                                    cy: "{uy}",
                                                    r: "{0.55 * k}",
                                                }
                                            }
                                            g {
                                                class: "jm-pin-at",
                                                transform: "translate({pin.x} {pin.y}) scale({k})",
                                                g {
                                                    class: "jm-pin",
                                                    tabindex: "0",
                                                    role: "button",
                                                    "aria-label": "{label}",
                                                    "aria-pressed": if is_sel { "true" } else { "false" },
                                                    onclick: move |_| selected.set(Some(id.clone())),
                                                    onkeydown: move |e: KeyboardEvent| {
                                                        let key = e.key();
                                                        if key == Key::Enter || key == Key::Character(" ".into()) {
                                                            e.prevent_default();
                                                            selected.set(Some(id2.clone()));
                                                        }
                                                    },
                                                    circle { class: "pin-hit", r: "7" }
                                                    circle { class: "pin-disc", r: "3.6" }
                                                    circle {
                                                        class: "pin-ring",
                                                        r: "4.6",
                                                        "pathLength": "100",
                                                        "stroke-dasharray": "{pct} 100",
                                                        transform: "rotate(-90)",
                                                    }
                                                    text { class: "pin-label", y: "-6.6", "text-anchor": "middle", lang: j.route.target_lang(), "{short}" }
                                                    text { class: "pin-code", y: "1.25", "text-anchor": "middle", "{level.code.replace(\"BIPA \", \"\")}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "jm-controls",
                        button { aria_label: "{ui.t(Msg::JourneyZoomIn)}", title: "{ui.t(Msg::JourneyZoomIn)}", onclick: move |_| zoom_in("zoom(1.5)"), "+" }
                        button { aria_label: "{ui.t(Msg::JourneyZoomOut)}", title: "{ui.t(Msg::JourneyZoomOut)}", onclick: move |_| zoom_out("zoom(1 / 1.5)"), "−" }
                        button { aria_label: "{ui.t(Msg::JourneyZoomReset)}", title: "{ui.t(Msg::JourneyZoomReset)}", onclick: move |_| zoom_reset("reset()"), {icon_reset()} }
                    }
                }
                p { class: "muted jm-hint", "{ui.t(Msg::JourneyMapHint)}" }
                {continue_bar(&ui, &j, &progress)}
            }
            if let Some(level) = panel_level {
                aside { class: "jm-panel", aria_label: "{level.place()}",
                    div { class: "jm-panel-close",
                        button { aria_label: "{ui.t(Msg::JourneyClose)}", title: "{ui.t(Msg::JourneyClose)}", onclick: move |_| selected.set(None), "×" }
                    }
                    {level_head(&ui, &j, level, &progress, &elsewhere)}
                    {unit_stops(&ui, &j, level, &progress, false)}
                }
            }
        }
    }
}

// ------------------------------------------------------------- suitcase ----

#[component]
fn Suitcase(completed: Rc<BTreeSet<String>>) -> Element {
    let ui = use_context::<Ui>();
    let routes = ROUTES.as_ref().expect("checked by JourneyScreen");
    let cards: Vec<_> = [(&routes.id, "id"), (&routes.jv, "jv")]
        .into_iter()
        .flat_map(|(c, tl)| souvenirs(c, &Progress::new(c, &completed)).into_iter().map(move |s| (s, tl)))
        .collect();
    rsx! {
        section { class: "suitcase", aria_label: "{ui.t(Msg::JourneySuitcase)}",
            if cards.is_empty() {
                div { class: "card suitcase-empty",
                    span { class: "suitcase-art", aria_hidden: "true", {icon_suitcase()} }
                    p { "{ui.t(Msg::JourneySuitcaseEmpty)}" }
                }
            }
            ul { class: "souvenirs",
                for (s, tl) in cards {
                    li { key: "{s.unit_id}", class: "souvenir card",
                        div { class: "souvenir-stamp", aria_hidden: "true", bdi { "{s.level_code}" } }
                        h3 { bdi { lang: tl, "{s.name}" } }
                        p { class: "muted souvenir-where",
                            bdi { lang: tl, "{s.stop}" }
                            " · "
                            bdi { class: "latin", "{s.level_code}" }
                            if s.unverified {
                                " "
                                span { class: "badge-unverified", "{ui.t(Msg::JourneyUnverified)}" }
                            }
                        }
                        p { class: "souvenir-note", lang: "en", dir: "ltr", "{s.note}" }
                    }
                }
            }
        }
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
mod tests {
    use super::*;

    #[test]
    fn fixtures_parse_and_lay_out() {
        let r = ROUTES.as_ref().unwrap();
        assert_eq!(r.id.levels.len(), 2);
        assert_eq!(r.id_map.style, RouteStyle::Islands);
        assert_eq!(r.jv_map.style, RouteStyle::Rail);
        assert!(r.jv.unverified());
        // J5 Semarang rides the branch line.
        assert!(r.jv_map.segments.iter().any(|s| s.kind == SegmentKind::Branch));
        // Every level and its test have an id the player can open.
        for c in [&r.id, &r.jv] {
            for l in &c.levels {
                assert!(l.level_test.is_some(), "{} has no level test", l.id);
                assert!(l.lessons().count() > 0, "{} has no lessons", l.id);
            }
        }
    }

    #[test]
    fn generated_maps_have_land() {
        for svg in [INDONESIA_SVG, JAVA_SVG] {
            let d = land_path(svg, "land-id");
            assert!(d.starts_with('M') && d.len() > 1000, "{}", &d[..d.len().min(40)]);
        }
        assert!(land_path(INDONESIA_SVG, "land-other").starts_with('M'));
        assert_eq!(land_path(INDONESIA_SVG, "nope"), "");
    }
}
