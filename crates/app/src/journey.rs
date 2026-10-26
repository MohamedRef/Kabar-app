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
use kabar_protocol::coach::CoachProfile;

const CURRICULUM_ID: &str = include_str!("../../../curriculum/indonesian.yaml");
const CURRICULUM_JV: &str = include_str!("../../../curriculum/javanese.yaml");

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
        let p = Progress::new(self.curriculum(), &self.completed);
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
    let coach = use_context::<crate::coach::Coach>();
    let start = use_hook(|| start_level(&db, coach.profile.peek().as_ref()));
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
    let j = Journey { route: route(), completed: completed.clone(), start: start.clone() };
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
            p { class: "all-done card", crate::review::Isolated { text: ui.t(Msg::JourneyAllDone).to_string() } }
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
                        bdi { lang: j.route.target_lang(), dir: "ltr", "{level.place()}" }
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
                            bdi { lang: j.route.target_lang(), dir: "ltr", "{t}" }
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
                            bdi { lang: tl, dir: "ltr", "{unit.stop}" }
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
                                    bdi { class: "lesson-title", lang: tl, dir: "ltr", "{lesson.title}" }
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
    let mut selected = use_signal(|| initial.clone());
    let stage_id = format!("jm-stage-{}", j.route.target_lang());
    // Start centred on the place the learner is at.
    let focus = initial
        .as_deref()
        .and_then(|id| map.pin(id))
        .map_or((map.view.x + map.view.w / 2.0, map.view.y + map.view.h / 2.0), |p| (p.x, p.y));
    let script = format!("{PAN_ZOOM_JS}({stage_id:?}, {:.2}, {:.2});", focus.0, focus.1);
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
    // Pins are drawn in a small design space, scaled so they keep the same
    // on-screen size on both maps (the JS counter-scales them while zooming).
    let k = map.view.w / 200.0;
    let dot = map.radius * 0.22;
    let current_level = selected.read().clone();
    let panel_level = current_level.as_deref().and_then(|id| cur.level(id));
    let spots = label_spots(
        &cur.levels
            .iter()
            .zip(map.pins.iter())
            .map(|(level, pin)| (pin.x / k, pin.y / k, short_place(level.place()).chars().count()))
            .collect::<Vec<_>>(),
    );
    let rail = map.style == RouteStyle::Rail;

    rsx! {
        div { class: "jm", "data-panel": if panel_level.is_some() { "open" } else { "closed" },
            div { class: "jm-map",
                div {
                    id: "{stage_id}",
                    class: "jm-stage",
                    style: "aspect-ratio: {map.view.w:.1} / {map.view.h:.1}",
                    tabindex: "0",
                    aria_label: "{ui.t(map_label)}",
                    onmounted: move |_| {
                        document::eval(&script);
                    },
                    svg {
                        view_box: "{map.view.attr()}",
                        preserve_aspect_ratio: "xMidYMid slice",
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
                                        path { class: "rail-bed", d: "{seg.path_d()}" }
                                    }
                                    path {
                                        class: match seg.kind {
                                            SegmentKind::Flight => "seg flight",
                                            SegmentKind::Rail => "seg rail",
                                            SegmentKind::Branch => "seg branch",
                                        },
                                        d: "{seg.path_d()}",
                                    }
                                    }
                                }
                            }
                            for ((level, pin), spot) in cur.levels.iter().zip(map.pins.iter()).zip(spots.iter().copied()) {
                                {
                                    let (lx, ly, anchor) = spot.attrs();
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
                                    let short = short_place(level.place());
                                    rsx! {
                                        g { key: "{level.id}", class: "jm-level", "data-state": state,
                                            path { class: "unit-loop", d: "{pin.units_d()}" }
                                            for (u, (ux, uy)) in level.units.iter().zip(pin.units.iter()) {
                                                circle {
                                                    key: "{u.id}",
                                                    class: if progress.unit_done(u) { "unit-dot done" } else { "unit-dot" },
                                                    cx: "{ux}",
                                                    cy: "{uy}",
                                                    r: "{dot}",
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
                                                    circle { class: "pin-hit", r: "8" }
                                                    circle { class: "pin-disc", r: "3.6" }
                                                    if pct > 0 {
                                                        circle {
                                                            class: "pin-ring",
                                                            r: "4.6",
                                                            "pathLength": "100",
                                                            "stroke-dasharray": "{pct} 100",
                                                            transform: "rotate(-90)",
                                                        }
                                                    }
                                                    text {
                                                        class: "pin-label",
                                                        x: "{lx}",
                                                        y: "{ly}",
                                                        "text-anchor": anchor,
                                                        direction: "ltr",
                                                        lang: j.route.target_lang(),
                                                        "{short}"
                                                    }
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

// ------------------------------------------------------------ for Today ----

fn find_unit(unit_id: &str) -> Option<&'static Unit> {
    let routes = ROUTES.as_ref().ok()?;
    routes.id.levels.iter().chain(&routes.jv.levels).flat_map(|l| &l.units).find(|u| u.id == unit_id)
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

/// The lesson a plan's "Start" opens: the unit's first unfinished lesson.
/// `None` for an unknown unit or one already finished.
pub fn next_lesson_in(unit_id: &str, completed: &BTreeSet<String>) -> Option<String> {
    find_unit(unit_id)?.lessons.iter().find(|l| !completed.contains(&l.id)).map(|l| l.id.clone())
}

/// Where the Indonesian journey starts: the placement result, else the level
/// the onboarding answer points at (`None`: from the very beginning).
pub fn start_level(db: &Db, profile: Option<&CoachProfile>) -> Option<String> {
    let placed = db.0.borrow().placement().unwrap_or_else(|e| {
        eprintln!("kabar: reading the placement result failed: {e}");
        None
    });
    placed.map(|p| p.start_level).or_else(|| {
        let unit = kabar_core::coach::onboarding::starting_unit(profile?.start);
        unit.split_once('.').map(|(level, _)| level.to_string())
    })
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

// ------------------------------------------------------------- suitcase ----

/// The souvenir a unit's lesson content ships (its target-language name and
/// the note in the UI language). The curriculum's card is English only, so
/// it's the fallback for units whose lessons aren't written yet.
fn local_souvenir(unit_id: &str, lang: UiLanguage) -> Option<(String, String)> {
    find_unit(unit_id)?.lessons.iter().rev().find_map(|l| {
        let lesson = kabar_core::lesson::parse_lesson(crate::lesson::lesson_content(&l.id)?).ok()?;
        let s = lesson.souvenir?;
        Some((s.title, kabar_core::lesson::pick(&s.note, lang).to_string()))
    })
}

#[component]
fn Suitcase(completed: Rc<BTreeSet<String>>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
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
                        h3 { bdi { lang: tl, dir: "ltr", "{local_souvenir(&s.unit_id, lang).map(|(t, _)| t).unwrap_or(s.name.clone())}" } }
                        p { class: "muted souvenir-where",
                            bdi { lang: tl, dir: "ltr", "{s.stop}" }
                            if s.unverified {
                                " "
                                span { class: "badge-unverified", "{ui.t(Msg::JourneyUnverified)}" }
                            }
                        }
                        if let Some((_, note)) = local_souvenir(&s.unit_id, lang) {
                            p { class: "souvenir-note", crate::lesson::Rich { text: note, target: tl } }
                        } else {
                            p { class: "souvenir-note", lang: "en", dir: "ltr", "{s.note}" }
                        }
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

/// "Bali (Ubud)" → "Bali": the name drawn next to a map pin.
fn short_place(place: &str) -> String {
    place.split([',', '(']).next().unwrap_or_default().trim().to_string()
}

/// Where a pin's name goes. Above by default, clear of the pin's selection
/// halo; beside or below it when that would cover a neighbouring pin or its
/// name (Bali and Lombok sit almost on top of each other).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LabelSpot {
    Above,
    Below,
    East,
    West,
}

/// Pin design units (see `k` in the map): the selection halo's radius, the
/// pin's own radius with its progress ring, and the label's font metrics.
const HALO: f64 = 8.0;
const PIN: f64 = 5.0;
const CHAR_W: f64 = 2.4;
const ASCENT: f64 = 3.2;
const DESCENT: f64 = 1.0;

impl LabelSpot {
    /// (x, y, text-anchor) relative to the pin, in pin design units.
    fn attrs(self) -> (f64, f64, &'static str) {
        match self {
            Self::Above => (0.0, -(HALO + 1.6), "middle"),
            Self::Below => (0.0, HALO + 1.4 + ASCENT, "middle"),
            Self::East => (HALO + 1.4, 1.2, "start"),
            Self::West => (-(HALO + 1.4), 1.2, "end"),
        }
    }

    /// The label's box (x0, y0, x1, y1) for a pin at (x, y).
    fn rect(self, x: f64, y: f64, chars: usize) -> (f64, f64, f64, f64) {
        let (dx, dy, anchor) = self.attrs();
        let w = chars as f64 * CHAR_W;
        let x0 = match anchor {
            "middle" => x + dx - w / 2.0,
            "start" => x + dx,
            _ => x + dx - w,
        };
        (x0, y + dy - ASCENT, x0 + w, y + dy + DESCENT)
    }
}

fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// Picks a spot per pin, given (x, y, label length) in pin design units:
/// the first of above, below, east, west that covers no other pin and no
/// label already placed. Falls back to above.
fn label_spots(pins: &[(f64, f64, usize)]) -> Vec<LabelSpot> {
    let discs: Vec<_> = pins.iter().map(|&(x, y, _)| (x - PIN, y - PIN, x + PIN, y + PIN)).collect();
    let mut placed: Vec<(f64, f64, f64, f64)> = Vec::new();
    let mut out = Vec::with_capacity(pins.len());
    for (i, &(x, y, chars)) in pins.iter().enumerate() {
        let free = |spot: LabelSpot| {
            let r = spot.rect(x, y, chars);
            !discs.iter().enumerate().any(|(j, d)| j != i && overlaps(r, *d))
                && !placed.iter().any(|p| overlaps(r, *p))
        };
        let spot = [LabelSpot::Above, LabelSpot::Below, LabelSpot::East, LabelSpot::West]
            .into_iter()
            .find(|s| free(*s))
            .unwrap_or(LabelSpot::Above);
        placed.push(spot.rect(x, y, chars));
        out.push(spot);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curricula_parse_and_lay_out() {
        let r = ROUTES.as_ref().unwrap();
        assert_eq!(r.id.levels.len(), 7);
        assert_eq!(r.jv.levels.len(), 6);
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
    fn map_labels_stay_off_other_pins_and_labels() {
        // Two pins almost touching, like Bali and Lombok.
        let spots = label_spots(&[(0.0, 0.0, 4), (7.0, 1.0, 6), (60.0, 0.0, 7)]);
        assert_eq!(spots[0], LabelSpot::Above);
        assert_ne!(spots[1], LabelSpot::Above, "would cover Bali's name");
        assert_eq!(spots[2], LabelSpot::Above, "a lone pin keeps the default");
        // Above clears the selection halo.
        let (_, _, _, bottom) = LabelSpot::Above.rect(0.0, 0.0, 7);
        assert!(bottom <= -HALO + 0.01, "{bottom}");
        // The real maps: no label covers another pin or label.
        let r = ROUTES.as_ref().unwrap();
        for (cur, map) in [(&r.id, &r.id_map), (&r.jv, &r.jv_map)] {
            let k = map.view.w / 200.0;
            let pins: Vec<_> = cur
                .levels
                .iter()
                .zip(map.pins.iter())
                .map(|(l, p)| (p.x / k, p.y / k, short_place(l.place()).chars().count()))
                .collect();
            let spots = label_spots(&pins);
            let rects: Vec<_> = pins.iter().zip(&spots).map(|(&(x, y, n), s)| s.rect(x, y, n)).collect();
            for (i, a) in rects.iter().enumerate() {
                for (j, &(x, y, _)) in pins.iter().enumerate() {
                    if i != j {
                        assert!(
                            !overlaps(*a, (x - PIN, y - PIN, x + PIN, y + PIN)),
                            "label {i} covers pin {j}"
                        );
                    }
                }
                for b in &rects[i + 1..] {
                    assert!(!overlaps(*a, *b), "label {i} overlaps another");
                }
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

    #[test]
    fn today_names_units_and_opens_the_next_lesson() {
        assert_eq!(
            unit_title(UiLanguage::ArEg, "bipa1.u01").as_deref(),
            Some("التحية · \u{2068}Salam\u{2069}")
        );
        assert_eq!(
            unit_title(UiLanguage::En, "bipa1.u01").as_deref(),
            Some("Greetings · \u{2068}Salam\u{2069}")
        );
        assert_eq!(unit_title(UiLanguage::En, "nope"), None);

        let mut done = BTreeSet::new();
        assert_eq!(next_lesson_in("bipa1.u01", &done).as_deref(), Some("bipa1.u01.l01"));
        assert_eq!(current_unit(&done, None, false).as_deref(), Some("bipa1.u01"));
        done.insert("bipa1.u01.l01".to_string());
        assert_eq!(next_lesson_in("bipa1.u01", &done).as_deref(), Some("bipa1.u01.l02"));
        assert_eq!(current_unit(&done, None, false).as_deref(), Some("bipa1.u01"));

        // A finished unit has no next lesson; the plan moves on to the next unit.
        let unit = find_unit("bipa1.u01").unwrap();
        done.extend(unit.lessons.iter().map(|l| l.id.clone()));
        assert_eq!(next_lesson_in("bipa1.u01", &done), None);
        assert_eq!(current_unit(&done, None, false).as_deref(), Some("bipa1.u02"));
        assert_eq!(next_lesson_in("nope", &done), None);

        // "I know the basics" starts at BIPA 2; Javanese has its own route.
        assert_eq!(current_unit(&BTreeSet::new(), Some("bipa2"), false).as_deref(), Some("bipa2.u01"));
        assert_eq!(current_unit(&BTreeSet::new(), Some("bipa2"), true).as_deref(), Some("j1.u01"));
    }

    #[test]
    fn suitcase_souvenirs_speak_the_ui_language() {
        // The lesson content's souvenir, not the English curriculum card.
        let (title, note) = local_souvenir("bipa1.u01", UiLanguage::ArEg).unwrap();
        assert_eq!(title, "Taman Fatahillah");
        assert!(note.contains("جاكرتا"), "{note}");
        let (_, en) = local_souvenir("bipa1.u01", UiLanguage::En).unwrap();
        assert!(en.starts_with("Jakarta began"), "{en}");
        assert_eq!(local_souvenir("no.such.unit", UiLanguage::ArEg), None);
    }
}
