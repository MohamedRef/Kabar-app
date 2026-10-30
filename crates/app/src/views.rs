//! List screens' view switcher, Files-app style (Mohamed, 2026-10-29):
//! cards in a grid, a list of rows with a second line, or a compact list of
//! one line each, plus the orders a screen offers. The markup is the same
//! in all three; `data-layout` on the `ul.coll` picks the look
//! (components.css § List screens). Each screen remembers its choice on
//! this device (local value `view.<key>`, never synced).
// Same design as batna-app's crates/app/src/parts.rs ViewBar.

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use serde::{Deserialize, Serialize};

use crate::{Db, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Cards,
    List,
    Compact,
}

impl Layout {
    pub const ALL: [Self; 3] = [Self::Cards, Self::List, Self::Compact];
    /// Starting layout for collections; saved choices still win.
    pub const DEFAULT: Self = Self::List;

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cards => "cards",
            Self::List => "list",
            Self::Compact => "compact",
        }
    }

    fn label(self, lang: UiLanguage) -> &'static str {
        let en = lang == UiLanguage::En;
        match self {
            Self::Cards => {
                if en {
                    "Cards"
                } else {
                    "كروت"
                }
            }
            Self::List => {
                if en {
                    "List"
                } else {
                    "قائمة"
                }
            }
            Self::Compact => {
                if en {
                    "Compact list"
                } else {
                    "قائمة مضغوطة"
                }
            }
        }
    }

    fn icon(self) -> Element {
        crate::icons::layout(self)
    }
}

/// The order a list shows its items in. Each screen offers the ones that
/// mean something for it ([`Collection::sorts`]); the first is its default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    /// The course's (or the list's own) order.
    Course,
    /// Newest first.
    Recent,
    /// What still has work left first, the rest after, each part in course order.
    Left,
    /// Alphabetical.
    Name,
}

impl Sort {
    fn label(self, lang: UiLanguage) -> &'static str {
        use UiLanguage::*;
        match (self, lang) {
            (Self::Course, En) => "Course order",
            (Self::Course, _) => "ترتيب المنهج",
            (Self::Recent, En) => "Newest first",
            (Self::Recent, ArEg) => "الأحدث الأول",
            (Self::Recent, Ar) => "الأحدث أولًا",
            (Self::Left, En) => "Not done yet first",
            (Self::Left, ArEg) => "اللي لسه الأول",
            (Self::Left, Ar) => "غير المنتهي أولًا",
            (Self::Name, En) => "A to Z",
            (Self::Name, _) => "أبجديًا (A–Z)",
        }
    }
}

/// One list's view choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewPick {
    pub layout: Layout,
    pub sort: Sort,
}

/// A list screen that offers view choices: its key, its default layout
/// and the orders it offers (first = default; one or none hides the menu).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Collection {
    pub key: &'static str,
    pub layout: Layout,
    pub sorts: &'static [Sort],
}

/// Choices made while the app runs, so every list re-renders at once.
static PICKS: GlobalSignal<std::collections::BTreeMap<&'static str, ViewPick>> =
    Signal::global(std::collections::BTreeMap::new);

impl Collection {
    fn default_pick(&self) -> ViewPick {
        ViewPick { layout: self.layout, sort: self.sorts.first().copied().unwrap_or(Sort::Course) }
    }

    fn store_key(&self) -> String {
        format!("view.{}", self.key)
    }

    /// The current choice: this run's, else the saved one, else the
    /// default. A saved order this list no longer offers falls back.
    /// `KABAR_VIEW=cards|list|compact` forces a layout (screenshots).
    pub fn pick(&self, db: &Db) -> ViewPick {
        let saved = PICKS.read().get(self.key).copied().or_else(|| {
            db.0.borrow()
                .local_value(&self.store_key())
                .ok()
                .flatten()
                .and_then(|v| serde_json::from_str(&v).ok())
        });
        let mut p = saved.unwrap_or_else(|| self.default_pick());
        if !self.sorts.contains(&p.sort) {
            p.sort = self.default_pick().sort;
        }
        if let Ok(v) = std::env::var("KABAR_VIEW")
            && let Some(l) = Layout::ALL.into_iter().find(|l| l.as_str() == v)
        {
            p.layout = l;
        }
        p
    }

    pub fn set(&self, db: &Db, p: ViewPick) {
        let db = db.clone();
        let key = self.store_key();
        crate::persistence::save(format!("local.{key}"), move || {
            let value = serde_json::to_string(&p).map_err(|e| e.to_string())?;
            db.0.borrow_mut().set_local_value(&key, &value).map_err(|e| e.to_string())
        });
        PICKS.write().insert(self.key, p);
    }
}

/// Sorts `items` by `sort`, stably, so each part keeps its order. `left`
/// says whether an item still has work, `name` gives its title. Recent
/// assumes the list arrives newest first.
pub fn sort_by<T>(items: &mut [T], sort: Sort, left: impl Fn(&T) -> bool, name: impl Fn(&T) -> String) {
    match sort {
        Sort::Course | Sort::Recent => {}
        Sort::Left => items.sort_by_key(|i| !left(i)),
        Sort::Name => items.sort_by_cached_key(|i| name(i).to_lowercase()),
    }
}

/// The view switcher above a list: the order (a small menu, when the
/// list offers more than one) and the three layouts as icon buttons.
#[component]
pub fn ViewBar(of: Collection) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let lang = ui.lang();
    let pick = of.pick(&db);
    let mut menu = use_signal(|| false);
    let en = lang == UiLanguage::En;
    let sort_title = if en { "Sort by" } else { "رتّب حسب" };
    rsx! {
        div { class: "view-bar",
            if of.sorts.len() > 1 {
                div { class: "view-sort",
                    button {
                        class: "view-sort-btn",
                        "aria-haspopup": "menu",
                        "aria-expanded": if menu() { "true" } else { "false" },
                        title: sort_title,
                        onclick: move |_| menu.toggle(),
                        {crate::icons::sort()}
                        span { "{pick.sort.label(lang)}" }
                    }
                    if menu() {
                        // A tap outside closes the menu.
                        button {
                            class: "view-scrim",
                            aria_label: if en { "Close the menu" } else { "اقفل القائمة" },
                            tabindex: "-1",
                            onclick: move |_| menu.set(false),
                        }
                        div { class: "view-menu", role: "menu", aria_label: sort_title,
                            for s in of.sorts.iter().copied() {
                                button {
                                    key: "{s:?}",
                                    class: "view-menu-item",
                                    role: "menuitemradio",
                                    "aria-checked": if pick.sort == s { "true" } else { "false" },
                                    onclick: {
                                        let db = db.clone();
                                        move |_| {
                                            of.set(&db, ViewPick { sort: s, ..of.pick(&db) });
                                            menu.set(false);
                                        }
                                    },
                                    span { class: "view-tick", aria_hidden: "true", if pick.sort == s { "✓" } }
                                    "{s.label(lang)}"
                                }
                            }
                        }
                    }
                }
            }
            div { class: "view-seg", role: "radiogroup", aria_label: if en { "Show as" } else { "طريقة العرض" },
                for l in Layout::ALL {
                    button {
                        key: "{l:?}",
                        role: "radio",
                        "aria-checked": if pick.layout == l { "true" } else { "false" },
                        aria_label: "{l.label(lang)}",
                        title: "{l.label(lang)}",
                        onclick: {
                            let db = db.clone();
                            move |_| of.set(&db, ViewPick { layout: l, ..of.pick(&db) })
                        },
                        {l.icon()}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pick_round_trips() {
        let p = ViewPick { layout: Layout::Compact, sort: Sort::Left };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<ViewPick>(&json).unwrap(), p);
        let c = Collection { key: "test", layout: Layout::Cards, sorts: &[] };
        assert_eq!(c.default_pick(), ViewPick { layout: Layout::Cards, sort: Sort::Course });
    }

    #[test]
    fn sorting_keeps_each_part_in_order() {
        let mut v = vec![("b", true), ("a", false), ("c", true), ("d", false)];
        sort_by(&mut v, Sort::Left, |i| i.1, |i| i.0.to_string());
        assert_eq!(v.iter().map(|i| i.0).collect::<String>(), "bcad");
        sort_by(&mut v, Sort::Name, |i| i.1, |i| i.0.to_string());
        assert_eq!(v.iter().map(|i| i.0).collect::<String>(), "abcd");
        sort_by(&mut v, Sort::Recent, |i| i.1, |i| i.0.to_string());
        assert_eq!(v.iter().map(|i| i.0).collect::<String>(), "abcd");
    }

    #[test]
    fn every_label_is_written_in_every_language() {
        for lang in [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En] {
            for l in Layout::ALL {
                assert!(!l.label(lang).is_empty());
            }
            for s in [Sort::Course, Sort::Recent, Sort::Left, Sort::Name] {
                assert!(!s.label(lang).is_empty());
            }
        }
    }
}
