//! Search and filters for the changelog (Settings › Updates › history):
//! words (Arabic marks and letter variants ignored), a level or the app,
//! and a kind. While any is set, the matching lines of every build kept on
//! this device replace the timeline, newest build first, each line still
//! opening its lesson.
//!
//! Same design as Batna-app's `crates/app/src/update/filter.rs` (courses
//! there, levels here).

use dioxus::prelude::*;
use kabar_protocol::update::{BuildChanges, Change, ChangeKind};

use super::changelog::label;
use super::groups::{group_name, groups, line_item};
use super::sheet::{digits, kind_id, kind_label};
use super::text::{U, ut};
use crate::Ui;

/// What the changelog is narrowed to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub words: String,
    /// A level id, `app`, or "" for lines nothing places.
    pub group: Option<String>,
    pub kind: Option<ChangeKind>,
}

impl Filter {
    pub fn is_on(&self) -> bool {
        !self.words.trim().is_empty() || self.group.is_some() || self.kind.is_some()
    }

    fn keeps(&self, c: &Change) -> bool {
        self.kind.is_none_or(|k| c.kind == k)
            && self.group.as_deref().is_none_or(|g| c.group().unwrap_or("") == g)
            && (self.words.trim().is_empty()
                || crate::settings::search::matches(
                    &self.words,
                    &format!("{} {} {}", c.text.ar_eg, c.text.ar, c.text.en),
                ))
    }
}

static FILTER: GlobalSignal<Filter> = Signal::global(Filter::default);

/// The changelog is narrowed: the sheet shows [`HistoryFilter`]'s results
/// instead of its timeline.
pub fn filtering() -> bool {
    FILTER.read().is_on()
}

/// Forgets the search (the sheet closed).
pub fn clear() {
    if FILTER.peek().is_on() {
        *FILTER.write() = Filter::default();
    }
}

/// The builds with a line `f` keeps, newest first, with those lines.
pub fn matching<'a>(history: &'a [BuildChanges], f: &Filter) -> Vec<(&'a BuildChanges, Vec<&'a Change>)> {
    history
        .iter()
        .map(|b| (b, b.changes.iter().filter(|c| f.keeps(c)).collect::<Vec<_>>()))
        .filter(|(_, lines)| !lines.is_empty())
        .collect()
}

/// The groups any kept build has, in the sheet's order (app, levels, rest).
fn group_choices(history: &[BuildChanges]) -> Vec<String> {
    let all = BuildChanges {
        version: String::new(),
        build: 0,
        published_at: String::new(),
        changes: history.iter().flat_map(|b| b.changes.iter().cloned()).collect(),
        content_only: false,
    };
    groups(&all).into_iter().map(|g| g.id).collect()
}

/// The search box, the level and kind pickers, and (while narrowed) the
/// lines that match.
#[component]
pub(super) fn HistoryFilter(history: Vec<BuildChanges>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let f = FILTER.read().clone();
    let choices = group_choices(&history);
    let found = matching(&history, &f);
    let n: usize = found.iter().map(|(_, l)| l.len()).sum();
    let count = crate::i18n::fill(
        lang,
        ut(lang, U::Found),
        &[("n", &digits(&ui, n.to_string())), ("b", &digits(&ui, found.len().to_string()))],
    );
    let group_value = f.group.clone().map_or("*".to_owned(), |g| if g.is_empty() { "-".into() } else { g });
    rsx! {
        div { class: "us-filter",
            input {
                r#type: "search",
                class: "us-search",
                placeholder: "{ut(lang, U::Search)}",
                aria_label: "{ut(lang, U::Search)}",
                value: "{f.words}",
                oninput: move |e| FILTER.write().words = e.value(),
            }
            div { class: "us-filter-row",
                select {
                    class: "us-select",
                    aria_label: "{ut(lang, U::AllLevels)}",
                    value: "{group_value}",
                    onchange: move |e| {
                        FILTER.write().group = match e.value().as_str() {
                            "*" => None,
                            "-" => Some(String::new()),
                            g => Some(g.to_owned()),
                        };
                    },
                    option { value: "*", selected: f.group.is_none(), "{ut(lang, U::AllLevels)}" }
                    for g in choices {
                        {
                            let value = if g.is_empty() { "-".to_owned() } else { g.clone() };
                            let selected = f.group.as_deref() == Some(g.as_str());
                            rsx! {
                                option { key: "{value}", value: "{value}", selected, "{group_name(lang, &g)}" }
                            }
                        }
                    }
                }
                div { class: "us-kind-chips", role: "group", aria_label: "{ut(lang, U::AllKinds)}",
                    for k in ChangeKind::ALL {
                        button {
                            key: "{kind_id(k)}",
                            class: "us-kind-chip",
                            "data-kind": "{kind_id(k)}",
                            aria_pressed: if f.kind == Some(k) { "true" } else { "false" },
                            onclick: move |_| {
                                let mut w = FILTER.write();
                                w.kind = if w.kind == Some(k) { None } else { Some(k) };
                            },
                            "{ut(lang, kind_label(k))}"
                        }
                    }
                }
            }
        }
        if f.is_on() {
            div { class: "us-found",
                p { class: "us-found-count",
                    if n == 0 { "{ut(lang, U::NoMatch)}" } else { "{count}" }
                    button { class: "us-more", onclick: move |_| *FILTER.write() = Filter::default(), "{ut(lang, U::ClearSearch)}" }
                }
                for (b, lines) in found {
                    section { key: "{b.key()}", class: "us-found-build",
                        h3 { bdi { dir: "ltr", class: "latin", "v{label(&b.version, b.build)}" } }
                        ul { class: "us-items",
                            for (j, c) in lines.into_iter().enumerate() {
                                {line_item(lang, c, j)}
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
    use kabar_core::locale::UiLanguage;
    use kabar_protocol::update::UpdateNotes;

    use super::super::sheet::{SheetMode, UpdateSheetNav};
    use super::*;
    use crate::Screen;

    fn c(kind: ChangeKind, en: &str, ar_eg: &str, scope: Option<&str>) -> Change {
        Change {
            scope: scope.map(str::to_owned),
            ..Change::new(kind, UpdateNotes { en: en.into(), ar_eg: ar_eg.into(), ar: String::new() })
        }
    }

    fn b(build: u64, changes: Vec<Change>) -> BuildChanges {
        BuildChanges {
            version: "1.0.0".into(),
            build,
            published_at: String::new(),
            changes,
            content_only: false,
        }
    }

    fn history() -> Vec<BuildChanges> {
        use ChangeKind::*;
        vec![
            b(
                9,
                vec![
                    c(New, "New pictures in BIPA 3", "صور جديدة في المستوى", Some("bipa3")),
                    c(Fixed, "Reader scrolls", "القارئ بقى أسرع", Some("reader")),
                ],
            ),
            b(8, vec![c(Improved, "Better pictures in J2", "إصلاح صور الجاوي", Some("j2"))]),
        ]
    }

    #[test]
    fn words_level_and_kind_narrow_every_build() {
        use ChangeKind::*;
        let h = history();
        let f = |words: &str, group: Option<&str>, kind: Option<ChangeKind>| Filter {
            words: words.into(),
            group: group.map(str::to_owned),
            kind,
        };
        let builds = |f: Filter| matching(&h, &f).iter().map(|(b, l)| (b.build, l.len())).collect::<Vec<_>>();
        assert_eq!(builds(f("pictures", None, None)), [(9, 1), (8, 1)]);
        assert_eq!(builds(f("اصلاح", None, None)), [(8, 1)], "إ and ا match");
        assert_eq!(builds(f("", Some("app"), None)), [(9, 1)]);
        assert_eq!(builds(f("", None, Some(Improved))), [(8, 1)]);
        assert_eq!(builds(f("pictures", Some("bipa3"), Some(Fixed))), []);
        assert_eq!(group_choices(&h), ["app", "bipa3", "j2"]);
        assert!(!Filter::default().is_on() && !f(" ", None, None).is_on());
    }

    #[test]
    fn the_changelog_search_narrows_and_clears() {
        use super::super::sheet::SheetFor;
        use super::super::tests::status;
        use super::super::{Mode, Phase};

        fn app() -> Element {
            crate::testkit::provide(|| crate::testkit::Setup {
                lang: UiLanguage::En,
                screen: Screen::Today,
                ..Default::default()
            });
            let nav = use_context_provider(|| UpdateSheetNav(Signal::new(Some(SheetMode::History))));
            let mut s = status(Phase::UpToDate, Mode::Auto);
            s.history = history();
            rsx! {
                if let Some(m) = *nav.0.read() {
                    SheetFor { s, m }
                }
            }
        }
        let mut t = crate::test_dom::TestDom::new(app);
        assert!(
            t.html().contains("class=\"us-search") && !t.html().contains("class=\"us-found\""),
            "{}",
            t.html()
        );
        let search = t.by_class("us-search")[0];
        t.input(search, "pictures");
        let page = t.html();
        assert!(
            page.contains("2\u{a0}changes in 2\u{a0}builds") && page.contains("us-timeline\" hidden"),
            "{page}"
        );
        let improved = t.find("button", "Improved");
        t.click(improved);
        assert!(t.html().contains("1\u{a0}change in 1\u{a0}build"), "{}", t.html());
        t.click_on("button", "Clear search");
        assert!(!t.html().contains("class=\"us-found\""), "{}", t.html());
        let select = t.by_class("us-select")[0];
        t.change(select, "app");
        assert!(
            t.html().contains("1\u{a0}change in 1\u{a0}build") && t.html().contains("Reader scrolls"),
            "{}",
            t.html()
        );
    }
}
