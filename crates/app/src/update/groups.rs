//! A release's lines in the "what's new" sheet, grouped by what they are
//! about: the app first, then one row per level (BIPA 1…7, then Javanese
//! J1…J6) with its count. A line that names a lesson, a unit or a screen
//! opens it; closing a lesson opened that way brings the sheet back.
//!
//! A release whose lines all sit in one group (or that predates `scope`)
//! keeps the kind cards: new, improved, fixed.
//!
//! Same design as Batna-app's `crates/app/src/update/groups.rs` (courses
//! there, levels here).

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use kabar_protocol::update::{APP, BuildChanges, Change, ChangeKind, Open};

use super::changelog::{line, of_kind};
use super::sheet::{SheetMode, UpdateSheetNav, chevron, digits, kind_icon, kind_id, kind_label, with_n};
use super::text::{U, ut};
use crate::{Screen, Ui};

const CSS: &str = include_str!("groups.css");

/// Lines a card or group shows before "Show all"; it folds only past `FOLD_AT`.
const PREVIEW: usize = 5;
const FOLD_AT: usize = 6;
/// A release this short opens every group; a longer one starts folded.
const OPEN_UPTO: usize = 8;

/// One row of a release: the lines of one level, of the app, or of
/// nothing known.
#[derive(Debug, PartialEq)]
pub struct Group<'a> {
    /// A level id, [`APP`], or "" for lines nothing places.
    pub id: String,
    /// New things first, then improvements, then fixes; written order inside.
    pub lines: Vec<&'a Change>,
}

impl Group<'_> {
    /// How many of each kind, zeros left out.
    pub fn tally(&self) -> Vec<(ChangeKind, usize)> {
        ChangeKind::ALL
            .into_iter()
            .map(|k| (k, self.lines.iter().filter(|c| c.kind == k).count()))
            .filter(|(_, n)| *n > 0)
            .collect()
    }
}

/// Indonesian levels before Javanese ones, each by number.
fn level_rank(id: &str) -> (u8, u32) {
    match id.strip_prefix("bipa") {
        Some(n) => (0, n.parse().unwrap_or(u32::MAX)),
        None => (1, id.trim_start_matches('j').parse().unwrap_or(u32::MAX)),
    }
}

/// The groups of `b`: the app, then levels in order, then lines nothing places.
pub fn groups(b: &BuildChanges) -> Vec<Group<'_>> {
    let mut out: Vec<Group<'_>> = Vec::new();
    for c in &b.changes {
        let id = c.group().unwrap_or("");
        match out.iter_mut().find(|g| g.id == id) {
            Some(g) => g.lines.push(c),
            None => out.push(Group { id: id.to_owned(), lines: vec![c] }),
        }
    }
    for g in &mut out {
        g.lines.sort_by_key(|c| ChangeKind::ALL.iter().position(|k| *k == c.kind));
    }
    out.sort_by_key(|g| match g.id.as_str() {
        APP => (0, (0, 0)),
        "" => (2, (0, 0)),
        id => (1, level_rank(id)),
    });
    out
}

/// A group's name: "BIPA 3", "J2" as the Journey names levels.
pub(super) fn group_name(lang: UiLanguage, id: &str) -> String {
    let ar = lang != UiLanguage::En;
    match id {
        APP if ar => "التطبيق".to_owned(),
        APP => "The app".to_owned(),
        "" if lang == UiLanguage::ArEg => "حاجات تانية".to_owned(),
        "" if ar => "أخرى".to_owned(),
        "" => "Other".to_owned(),
        id => match id.strip_prefix("bipa") {
            Some(n) => format!("BIPA {n}"),
            None => id.to_uppercase(),
        },
    }
}

/// "Open" before a linked line, for screen readers.
fn open_word(lang: UiLanguage) -> &'static str {
    match lang {
        UiLanguage::ArEg => "افتح",
        UiLanguage::Ar => "افتح",
        UiLanguage::En => "Open",
    }
}

/// Where a line's tap goes, when this build knows it.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Lesson(String),
    Unit(String),
    Screen(Screen),
}

pub fn target(c: &Change) -> Option<Target> {
    match Open::parse(c.open.as_deref()?)? {
        Open::Lesson(id) => crate::lesson::lesson_content(id).map(|_| Target::Lesson(id.to_owned())),
        Open::Unit(id) => Some(Target::Unit(id.to_owned())),
        Open::Screen(name) => screen_named(name).map(Target::Screen),
    }
}

fn screen_named(name: &str) -> Option<Screen> {
    Some(match name {
        "today" => Screen::Today,
        "journey" => Screen::Journey,
        "reader" => Screen::Reader,
        "review" => Screen::Review,
        "coach" => Screen::Coach,
        "play" => Screen::Play,
        "studio" => Screen::Studio,
        "listen" => Screen::Listen,
        "chat" => Screen::Chat,
        "teach" => Screen::Teach,
        "journal" => Screen::Journal,
        "stats" => Screen::Stats,
        _ => return None,
    })
}

/// The sheet to bring back once a lesson a line opened is closed.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Parked {
    mode: SheetMode,
    /// The screen the sheet sat over: closing the lesson comes back to it.
    screen: Screen,
}

static PARKED: GlobalSignal<Option<Parked>> = Signal::global(|| None);

/// Opens `t` and puts the sheet away (until a lesson is closed).
fn follow(t: &Target) {
    let (Some(ui), Some(mut nav)) = (try_consume_context::<Ui>(), try_consume_context::<UpdateSheetNav>())
    else {
        return;
    };
    let Some(mode) = *nav.0.peek() else { return };
    let screen = *ui.screen.peek();
    nav.0.set(None);
    match t {
        Target::Lesson(id) => {
            ui.open_lesson(id.clone());
            *PARKED.write() = Some(Parked { mode, screen });
        }
        Target::Unit(id) => {
            if mode == SheetMode::Updated
                && let Some(u) = super::view::updater()
            {
                u.close_whats_new();
            }
            crate::journey::open_suitcase(ui, id);
        }
        Target::Screen(s) => {
            if mode == SheetMode::Updated
                && let Some(u) = super::view::updater()
            {
                u.close_whats_new();
            }
            let mut screen = ui.screen;
            screen.set(*s);
        }
    }
}

/// Brings the sheet back once the lesson a line opened is closed; called
/// by the sheet, so it lives as long as the shell.
pub(super) fn use_return(mut mode: Signal<Option<SheetMode>>) {
    let ui = try_use_context::<Ui>();
    use_effect(move || {
        let Some(ui) = ui else { return };
        let open = ui.lesson.read().is_some() && *ui.screen.read() == Screen::Lesson;
        let screen = *ui.screen.read();
        let Some(p) = *PARKED.peek() else {
            if mode.read().is_none() {
                super::filter::clear();
            }
            return;
        };
        if open {
            return;
        }
        *PARKED.write() = None;
        if screen == p.screen && mode.peek().is_none() {
            mode.set(Some(p.mode));
        }
    });
}

fn go_icon() -> Element {
    rsx! {
        svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2",
            stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
            path { d: "M9 6l6 6-6 6" }
        }
    }
}

/// One line: plain, or a button when it opens something.
pub(super) fn line_item(lang: UiLanguage, c: &Change, key: usize) -> Element {
    let (text, latin) = line(lang, c);
    let kind = kind_id(c.kind);
    let words = rsx! {
        if latin {
            bdi { dir: "ltr", class: "latin", "{text}" }
        } else {
            "{text}"
        }
    };
    match target(c) {
        Some(t) => rsx! {
            li { key: "{key}", "data-kind": "{kind}", class: "us-linked",
                button { class: "us-link", aria_label: "{open_word(lang)}: {text}", onclick: move |_| follow(&t),
                    span { class: "us-link-text", {words} }
                    span { class: "us-go-icon", {go_icon()} }
                }
            }
        },
        None => rsx! {
            li { key: "{key}", "data-kind": "{kind}", {words} }
        },
    }
}

/// One line over a release that spans two or more levels: how many
/// changes, over how many levels (and the app), and the busiest level.
pub fn digest(ui: &Ui, total: usize, groups: &[Group<'_>]) -> Option<String> {
    let lang = ui.lang();
    let levels: Vec<&Group<'_>> = groups.iter().filter(|g| g.id != APP && !g.id.is_empty()).collect();
    if levels.len() < 2 {
        return None;
    }
    let app = groups.iter().any(|g| g.id == APP);
    let top = levels.iter().max_by_key(|g| (g.lines.len(), std::cmp::Reverse(level_rank(&g.id))))?;
    let d = |n: usize| digits(ui, n.to_string());
    Some(crate::i18n::fill(
        lang,
        ut(lang, U::Digest),
        &[
            ("changes", &d(total)),
            ("levels", &d(levels.len())),
            ("app", if app { ut(lang, U::AndApp) } else { "" }),
            ("top", &group_name(lang, &top.id)),
            ("n", &d(top.lines.len())),
        ],
    ))
}

/// Lessons finished before the day `b` came out that its lines open (a
/// lesson, or every finished lesson of a unit), in order, once each.
/// `done` is when a lesson was first finished (ms), if it was.
pub fn finished_changed(b: &BuildChanges, done: impl Fn(&str) -> Option<i64>) -> Vec<String> {
    let Some(day) = super::changelog::day_of(&b.published_at) else { return Vec::new() };
    let before = |id: &str| done(id).is_some_and(|ms| ms.div_euclid(86_400_000) < day);
    let mut out: Vec<String> = Vec::new();
    for c in &b.changes {
        let ids = match target(c) {
            Some(Target::Lesson(id)) => vec![id],
            Some(Target::Unit(id)) => crate::journey::unit_lessons(&id),
            _ => Vec::new(),
        };
        for id in ids {
            if before(&id) && !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// What leads an open release: the finished lessons it changed, and the
/// digest of a release over several levels.
fn release_head(ui: &Ui, b: &BuildChanges, groups: &[Group<'_>]) -> Element {
    let lang = ui.lang();
    let firsts = try_consume_context::<crate::Db>()
        .and_then(|db| db.0.borrow().first_completions().ok())
        .unwrap_or_default();
    let changed = finished_changed(b, |id| firsts.get(id).copied());
    let digest = digest(ui, b.changes.len(), groups);
    let head = crate::i18n::fill(
        lang,
        ut(lang, U::FinishedChanged),
        &[("n", &digits(ui, changed.len().to_string()))],
    );
    rsx! {
        if let Some(d) = digest {
            p { class: "us-digest", "{d}" }
        }
        if !changed.is_empty() {
            section { class: "us-changed",
                h3 { "{head}" }
                ul { class: "us-items",
                    for id in changed {
                        {
                            let title = crate::journey::lesson_title(&id).unwrap_or_else(|| id.clone());
                            let level = group_name(lang, id.split('.').next().unwrap_or_default());
                            let t = Target::Lesson(id.clone());
                            rsx! {
                                li { key: "{id}", class: "us-linked",
                                    button { class: "us-link", aria_label: "{open_word(lang)}: {title}", onclick: move |_| follow(&t),
                                        span { class: "us-link-text",
                                            span { class: "us-crumb", bdi { "{level}" } " · " }
                                            bdi { "{title}" }
                                        }
                                        span { class: "us-go-icon", {go_icon()} }
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

/// The open part of a release on the timeline: kind cards, or level rows
/// when its lines span more than one group.
#[component]
pub(super) fn ReleaseBody(release: BuildChanges) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let key = release.key();
    let flipped = use_signal(Vec::<String>::new);
    let all = use_signal(Vec::<String>::new);
    let toggle = move |mut list: Signal<Vec<String>>, k: String| {
        let mut v = list.write();
        match v.iter().position(|x| *x == k) {
            Some(i) => {
                v.remove(i);
            }
            None => v.push(k),
        }
    };
    let groups = groups(&release);
    let start_open = release.changes.len() <= OPEN_UPTO;
    let head = release_head(&ui, &release, &groups);

    if groups.len() < 2 {
        return rsx! {
            style { {CSS} }
            {head}
            div { class: "us-kinds",
                for k in ChangeKind::ALL {
                    {
                        let items = of_kind(&release, k);
                        let kkey = format!("{key}:{}", kind_id(k));
                        let shows_all = all.read().contains(&kkey);
                        let folds = items.len() > FOLD_AT;
                        let n = items.len();
                        let show: Vec<&Change> =
                            items.into_iter().take(if folds && !shows_all { PREVIEW } else { usize::MAX }).collect();
                        rsx! {
                            if n > 0 {
                                section { key: "{kkey}", class: "us-kind", "data-kind": "{kind_id(k)}",
                                    div { class: "us-kind-head",
                                        span { class: "us-kind-icon", {kind_icon(k)} }
                                        h3 { "{ut(lang, kind_label(k))}" }
                                        span { class: "us-chip", "{digits(&ui, n.to_string())}" }
                                    }
                                    ul { class: "us-items",
                                        for (j, c) in show.into_iter().enumerate() {
                                            {line_item(lang, c, j)}
                                        }
                                    }
                                    if folds {
                                        button { class: "us-more", onclick: move |_| toggle(all, kkey.clone()),
                                            if shows_all {
                                                "{ut(lang, U::ShowLess)}"
                                            } else {
                                                "{with_n(&ui, ut(lang, U::ShowAll), n)}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };
    }

    rsx! {
        style { {CSS} }
        {head}
        div { class: "us-groups",
            for g in groups {
                {
                    let gkey = format!("{key}:{}", g.id);
                    let open = start_open != flipped.read().contains(&gkey);
                    let shows_all = all.read().contains(&gkey);
                    let n = g.lines.len();
                    let folds = n > FOLD_AT;
                    let tally = g.tally();
                    let name = group_name(lang, &g.id);
                    let show: Vec<&Change> =
                        g.lines.iter().copied().take(if folds && !shows_all { PREVIEW } else { usize::MAX }).collect();
                    let (head_key, more_key) = (gkey.clone(), gkey.clone());
                    let group_id = if g.id.is_empty() { "other".to_owned() } else { g.id.clone() };
                    rsx! {
                        section { key: "{gkey}", class: "us-group", "data-group": "{group_id}",
                            button {
                                class: "us-group-head",
                                aria_expanded: if open { "true" } else { "false" },
                                onclick: move |_| toggle(flipped, head_key.clone()),
                                h3 { class: "us-group-name",
                                    bdi { "{name}" }
                                }
                                span { class: "us-group-kinds",
                                    for (k, c) in tally {
                                        span { key: "{kind_id(k)}", class: "us-mini", "data-kind": "{kind_id(k)}",
                                            title: "{ut(lang, kind_label(k))}",
                                            {kind_icon(k)}
                                            "{digits(&ui, c.to_string())}"
                                        }
                                    }
                                }
                                span { class: "us-chip", "{digits(&ui, n.to_string())}" }
                                span { class: "us-chevron", {chevron()} }
                            }
                            if open {
                                ul { class: "us-items",
                                    for (j, c) in show.into_iter().enumerate() {
                                        {line_item(lang, c, j)}
                                    }
                                }
                                if folds {
                                    button { class: "us-more", onclick: move |_| toggle(all, more_key.clone()),
                                        if shows_all {
                                            "{ut(lang, U::ShowLess)}"
                                        } else {
                                            "{with_n(&ui, ut(lang, U::ShowAll), n)}"
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

#[cfg(test)]
mod tests {
    use kabar_protocol::update::UpdateNotes;

    use super::*;

    fn c(kind: ChangeKind, en: &str, scope: Option<&str>, open: Option<&str>) -> Change {
        Change {
            scope: scope.map(str::to_owned),
            open: open.map(str::to_owned),
            ..Change::new(kind, UpdateNotes { en: en.into(), ar_eg: format!("مصري {en}"), ar: String::new() })
        }
    }

    fn build(changes: Vec<Change>) -> BuildChanges {
        BuildChanges {
            version: "1.0.0".into(),
            build: 9,
            published_at: String::new(),
            changes,
            content_only: false,
        }
    }

    #[test]
    fn the_app_comes_first_then_levels_in_order_then_the_rest() {
        use ChangeKind::*;
        let b = build(vec![
            c(Fixed, "j2", Some("j2"), None),
            c(New, "old", None, None),
            c(New, "b10", None, Some("bipa10.u01.l01")),
            c(New, "b3", Some("bipa3"), None),
            c(Improved, "reader", Some("reader"), None),
            c(Fixed, "b3 fix", Some("bipa3"), None),
        ]);
        let got: Vec<(String, Vec<&str>)> = groups(&b)
            .into_iter()
            .map(|g| (g.id, g.lines.iter().map(|l| l.text.en.as_str()).collect()))
            .collect();
        assert_eq!(
            got,
            [
                ("app".to_owned(), vec!["reader"]),
                ("bipa3".to_owned(), vec!["b3", "b3 fix"]),
                ("bipa10".to_owned(), vec!["b10"]),
                ("j2".to_owned(), vec!["j2"]),
                (String::new(), vec!["old"]),
            ]
        );
        assert_eq!(groups(&b)[1].tally(), [(New, 1), (Fixed, 1)]);
        assert_eq!(group_name(UiLanguage::En, "bipa3"), "BIPA 3");
        assert_eq!(group_name(UiLanguage::ArEg, "j2"), "J2");
    }

    #[test]
    fn a_release_over_several_levels_gets_one_summary_line() {
        use ChangeKind::*;
        fn app() -> Element {
            crate::testkit::provide(|| crate::testkit::Setup { lang: UiLanguage::En, ..Default::default() });
            let ui = use_context::<Ui>();
            let b = build(vec![
                c(New, "a", Some("bipa3"), None),
                c(New, "b", Some("bipa3"), None),
                c(Fixed, "c", Some("j2"), None),
                c(Improved, "d", Some("reader"), None),
            ]);
            let one = build(vec![c(New, "a", Some("bipa3"), None), c(New, "d", Some("reader"), None)]);
            let d = digest(&ui, b.changes.len(), &groups(&b)).unwrap_or_default();
            let none = digest(&ui, one.changes.len(), &groups(&one)).is_none();
            rsx! { p { "[{d}] [{none}]" } }
        }
        let t = crate::test_dom::TestDom::new(app);
        assert!(
            t.html().contains("[4\u{a0}changes in 2\u{a0}levels and the app; most: BIPA 3 (2)] [true]"),
            "{}",
            t.html()
        );
    }

    #[test]
    fn finished_lessons_a_release_changed() {
        let unit = crate::journey::unit_lessons("bipa1.u01");
        let (first, second) = (unit[0].clone(), unit[1].clone());
        let mut b = build(vec![
            c(ChangeKind::New, "unit", None, Some("bipa1.u01")),
            c(ChangeKind::Fixed, "lesson", None, Some(&first)),
            c(ChangeKind::Fixed, "screen", None, Some("reader")),
        ]);
        b.published_at = "2026-11-15T05:00:00Z".into();
        let day = super::super::changelog::day_of(&b.published_at).unwrap() * 86_400_000;
        let done = |id: &str| (id == first || id == second).then_some(day - 1);
        assert_eq!(finished_changed(&b, done), [first.clone(), second.clone()]);
        assert_eq!(finished_changed(&b, |id| (id == first).then_some(day + 1)), Vec::<String>::new());
        b.published_at.clear();
        assert_eq!(finished_changed(&b, done), Vec::<String>::new());
    }

    #[test]
    fn a_line_opens_only_what_this_build_knows() {
        let t = |open: &str| target(&c(ChangeKind::New, "x", None, Some(open)));
        assert_eq!(t("bipa1.u01.l01"), Some(Target::Lesson("bipa1.u01.l01".into())));
        assert_eq!(t("bipa1.u01.l99"), None);
        assert_eq!(t("j1.u02"), Some(Target::Unit("j1.u02".into())));
        assert_eq!(t("reader"), Some(Target::Screen(Screen::Reader)));
        assert_eq!(target(&c(ChangeKind::New, "x", Some("bipa1"), None)), None);
    }

    #[test]
    fn a_line_opens_its_lesson_and_closing_it_brings_the_sheet_back() {
        use super::super::sheet::SheetFor;
        use super::super::tests::status;
        use super::super::{Mode, Phase};

        fn app() -> Element {
            let ui = crate::testkit::provide(|| crate::testkit::Setup {
                lang: UiLanguage::En,
                screen: Screen::Today,
                ..Default::default()
            })
            .0;
            let nav = use_context_provider(|| UpdateSheetNav(Signal::new(Some(SheetMode::History))));
            use_return(nav.0);
            let mut s = status(Phase::UpToDate, Mode::Auto);
            s.history = vec![build(vec![
                c(ChangeKind::New, "New pictures in the first lesson", None, Some("bipa1.u01.l01")),
                c(ChangeKind::Fixed, "Reader scrolls", Some("reader"), None),
            ])];
            let lesson = ui.lesson.read().clone().unwrap_or_default();
            rsx! {
                p { "[lesson: {lesson}]" }
                if let Some(m) = *nav.0.read() {
                    SheetFor { s, m }
                }
            }
        }
        let mut t = crate::test_dom::TestDom::new(app);
        assert!(t.html().contains("us-group") && t.html().contains("The app"), "{}", t.html());
        t.click_on("button", "New pictures in the first lesson");
        assert!(
            t.html().contains("[lesson: bipa1.u01.l01]") && !t.html().contains("us-group"),
            "{}",
            t.html()
        );
        t.in_app(|| consume_context::<Ui>().close_lesson());
        t.settle();
        assert!(t.html().contains("us-group"), "the sheet is back: {}", t.html());
    }
}
