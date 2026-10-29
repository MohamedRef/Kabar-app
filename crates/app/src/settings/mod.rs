//! Settings: one sheet over the current screen, so opening it never loses a
//! lesson or a Reader page. The gear in the phone's top bar, the Settings item
//! pinned to the foot of the tablet and Mac sidebar, and Kabar › Settings… (⌘,)
//! on the Mac all open it.
//!
//! Sections follow the device: Android, Tablet and Mac show only where they
//! apply. Shared learning preferences sync to every device; the rest are
//! marked "this device only" (`device::DevicePrefs`).

pub mod device;
mod learning;
mod look;
#[cfg(all(feature = "desktop", not(target_os = "android")))]
pub mod menu;
mod platform_sections;
mod text;

use dioxus::prelude::*;
use kabar_core::layout::WidthClass;

use crate::Ui;
use text::{S, st};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Learning,
    Look,
    Coach,
    Android,
    Tablet,
    Mac,
    Storage,
    Account,
    About,
}

impl Section {
    pub const ALL: [Self; 9] = [
        Self::Learning,
        Self::Look,
        Self::Coach,
        Self::Android,
        Self::Tablet,
        Self::Mac,
        Self::Storage,
        Self::Account,
        Self::About,
    ];

    /// Whether this section applies here. `tablet` is `Ui::tablet`.
    pub fn shown(self, tablet: bool) -> bool {
        match self {
            Self::Android => cfg!(target_os = "android"),
            Self::Tablet => cfg!(target_os = "android") && tablet,
            Self::Mac => !cfg!(target_os = "android"),
            _ => true,
        }
    }

    /// Stable id, for `KABAR_OPEN_SETTINGS=<id>` previews.
    pub fn id(self) -> &'static str {
        match self {
            Self::Learning => "learning",
            Self::Look => "look",
            Self::Coach => "coach",
            Self::Android => "android",
            Self::Tablet => "tablet",
            Self::Mac => "mac",
            Self::Storage => "storage",
            Self::Account => "account",
            Self::About => "about",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }

    fn title(self) -> S {
        match self {
            Self::Learning => S::Learning,
            Self::Look => S::Look,
            Self::Coach => S::Coach,
            Self::Android => S::Android,
            Self::Tablet => S::Tablet,
            Self::Mac => S::Mac,
            Self::Storage => S::Storage,
            Self::Account => S::Account,
            Self::About => S::About,
        }
    }

    fn summary(self) -> S {
        match self {
            Self::Learning => S::LearningSub,
            Self::Look => S::LookSub,
            Self::Coach => S::CoachSub,
            Self::Android => S::AndroidSub,
            Self::Tablet => S::TabletSub,
            Self::Mac => S::MacSub,
            Self::Storage => S::StorageSub,
            Self::Account => S::AccountSub,
            Self::About => S::AboutSub,
        }
    }
}

/// Whether the sheet is open and on which section. `section: None` is the
/// section list on the phone; wider windows show Learning beside the list.
#[derive(Clone, Copy)]
pub struct SettingsNav {
    pub open: Signal<bool>,
    pub section: Signal<Option<Section>>,
}

impl SettingsNav {
    pub fn new() -> Self {
        let start = std::env::var("KABAR_OPEN_SETTINGS").ok();
        Self {
            open: Signal::new(start.is_some()),
            section: Signal::new(start.as_deref().and_then(Section::from_id)),
        }
    }

    pub fn show(mut self, section: Option<Section>) {
        self.section.set(section);
        self.open.set(true);
    }

    pub fn close(mut self) {
        self.open.set(false);
    }
}

const SETTINGS_CSS: &str = r#"
.settings-backdrop { position: fixed; inset: 0; z-index: 40; background: rgb(0 0 0 / 0.35); }
.settings-window {
  position: fixed; z-index: 41; display: flex; flex-direction: column;
  background: var(--bg, var(--surface)); color: var(--text);
  inset: 0;
}
.app:not([data-width="compact"]) .settings-window {
  inset-block: 4vh; inset-inline: max(1.5rem, calc((100vw - 60rem) / 2));
  border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow); overflow: hidden;
}
.settings-head {
  display: flex; align-items: center; gap: 0.5rem; padding-block: 0.6rem; padding-inline: 1rem;
  padding-block-start: max(0.6rem, env(safe-area-inset-top));
  border-block-end: 1px solid var(--line); background: var(--surface);
}
.settings-head h2 { margin: 0; font-size: 1.2rem; flex: 1; }
.settings-body { flex: 1; min-block-size: 0; display: grid; grid-template-columns: 1fr; }
.app:not([data-width="compact"]) .settings-body { grid-template-columns: 20rem 1fr; }
.settings-sections { display: flex; flex-direction: column; padding: 0.75rem; overflow-y: auto; }
.app:not([data-width="compact"]) .settings-sections { border-inline-end: 1px solid var(--line); }
.settings-sections .section-head:first-child { margin-block-start: 0; }
.settings-sections .card.grouped { margin: 0; }
.settings-sections .activity-row[aria-current="true"] { background: var(--soft); }
.settings-sections .activity-row[aria-current="true"] strong { color: var(--primary); }
.settings-me { margin-block-end: var(--sp-2); }
.app:not([data-width="compact"]) .settings-sections .activity-row { padding-inline: var(--sp-3); }
.app:not([data-width="compact"]) .settings-sections .row-end { display: none; }
.settings-avatar { font-weight: 700; font-size: var(--fs-lg); background: var(--accent); color: var(--accent-ink); border-radius: 50%; }
.settings-pane { overflow-y: auto; padding: 1rem; padding-block-end: max(2rem, env(safe-area-inset-bottom)); }
.settings-pane > .card:last-child { margin-block-end: 0; }
/* Inside a section: one card, its fields as rows split by hairlines, the
   choices as segmented controls and on/off fields as switches at the end. */
.settings-pane .card > .field + .field { border-block-start: 1px solid var(--line-soft); padding-block-start: var(--sp-3); }
.settings-pane .card > .field { margin-block-end: var(--sp-3); }
.settings-pane .card > .field:last-child { margin-block-end: 0; }
.settings-pane .field > .label { display: block; font-weight: 600; margin-block-end: var(--sp-2); }
.settings-pane .segmented { border-radius: var(--radius-control); }
label.switch-row, .field > label.switch-row { display: flex; margin: 0; align-items: center; gap: var(--sp-2); min-block-size: var(--tap); font-weight: 600; cursor: pointer; }
.switch-row > input.switch { order: 2; margin-inline-start: auto; flex: none; }
.settings-scope { font-size: 0.75rem; margin-inline-start: 0.4rem; font-weight: 400; vertical-align: 0.1rem; }
.settings-hint { margin-block: 0.25rem 0; }
.shortcut-table { border-collapse: collapse; inline-size: 100%; }
.shortcut-table td { padding-block: 0.4rem; border-block-end: 1px solid var(--line); }
.shortcut-table td:last-child { text-align: end; white-space: nowrap; }
kbd { font-family: ui-monospace, monospace; font-size: 0.85rem; padding-inline: 0.4rem; padding-block: 0.1rem;
  border: 1px solid var(--line); border-block-end-width: 2px; border-radius: 6px; background: var(--surface-2); }
.text-sample { padding-block: 0.5rem; padding-inline: 0.75rem; border-radius: 10px; background: var(--surface-2); margin-block-start: 0.5rem; }
.text-sample p { margin-block: 0.2rem; }
.text-sample p[dir="ltr"] { text-align: start; font-size: 1.15rem; }
.about-name { font-size: 2rem; font-weight: 700; color: var(--indigo); margin-block: 0 0.25rem; }
.keys { display: inline-flex; gap: 0.25rem; align-items: center; }
"#;

/// The Settings sheet; renders nothing while closed.
#[component]
pub fn SettingsSheet() -> Element {
    let ui = use_context::<Ui>();
    let nav = use_context::<SettingsNav>();
    // Escape steps back (phone) or closes. Listened for on the document in
    // the capture phase: WebKit doesn't focus buttons on click, so the sheet
    // rarely has focus, and the Lesson Player underneath must not see it.
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            window.__kabarSettingsEsc = () => dioxus.send(true);
            if (!window.__kabarSettingsEscInit) {
              window.__kabarSettingsEscInit = true;
              document.addEventListener("keydown", (e) => {
                if (e.key === "Escape" && document.querySelector(".settings-window")) {
                  e.preventDefault();
                  e.stopPropagation();
                  window.__kabarSettingsEsc();
                }
              }, true);
            }
            await new Promise(() => {});
            "#,
        );
        let mut section = nav.section;
        while eval.recv::<bool>().await.is_ok() {
            let compact = *ui.width.peek() == WidthClass::Compact;
            if compact && section.peek().is_some() {
                section.set(None);
            } else {
                nav.close();
            }
        }
    });
    if !*nav.open.read() {
        return rsx! {};
    }
    let lang = ui.lang();
    let compact = *ui.width.read() == WidthClass::Compact;
    let tablet = *ui.tablet.read();
    let sections: Vec<Section> = Section::ALL.into_iter().filter(|s| s.shown(tablet)).collect();
    let chosen = nav.section.read().filter(|s| sections.contains(s));
    let current = chosen.or(if compact { None } else { Some(Section::Learning) });
    let drilled = compact && current.is_some();
    let title = match (drilled, current) {
        (true, Some(s)) => st(lang, s.title()),
        _ => ui.t(crate::i18n::Msg::NavSettings),
    };
    let mut section_sig = nav.section;

    rsx! {
        style { {SETTINGS_CSS} }
        div { class: "settings-backdrop", onclick: move |_| nav.close() }
        div {
            class: "settings-window",
            role: "dialog",
            aria_modal: "true",
            aria_labelledby: "settings-title",
            tabindex: "-1",
            onmounted: move |e: MountedEvent| async move {
                let _ = e.set_focus(true).await;
            },
            header { class: "settings-head",
                if drilled {
                    button {
                        class: "icon-button",
                        "data-back": "true",
                        aria_label: "{st(lang, S::Back)}",
                        onclick: move |_| section_sig.set(None),
                        span { aria_hidden: "true", if lang.dir() == kabar_core::locale::Direction::Rtl { "→" } else { "←" } }
                    }
                }
                h2 { id: "settings-title", "{title}" }
                button {
                    class: "icon-button",
                    // Back steps out of a section first (the button above).
                    "data-back": if drilled { "false" } else { "true" },
                    aria_label: "{st(lang, S::Close)}",
                    title: "{st(lang, S::Close)}",
                    onclick: move |_| nav.close(),
                    span { aria_hidden: "true", "✕" }
                }
            }
            div { class: "settings-body",
                if !drilled {
                    SectionList { sections: sections.clone(), current: if compact { None } else { current } }
                }
                if let Some(s) = current {
                    div { class: "settings-pane", key: "{s.id()}",
                        SectionView { section: s }
                    }
                }
            }
        }
    }
}

/// Settings' groups, in order: a heading and the sections under it (only
/// those shown here appear; an empty group is left out).
const GROUPS: [(S, &[Section]); 4] = [
    (S::GroupStudy, &[Section::Learning, Section::Coach]),
    (S::GroupLook, &[Section::Look]),
    (S::GroupDevice, &[Section::Android, Section::Tablet, Section::Mac]),
    (S::GroupData, &[Section::Account, Section::Storage, Section::About]),
];

/// The section list as grouped rows, headed by the learner (from Kak Nur's
/// profile, opening the study plan they set). `current` is marked on wide
/// windows, where the chosen section shows beside the list.
#[component]
fn SectionList(sections: Vec<Section>, current: Option<Section>) -> Element {
    let ui = use_context::<Ui>();
    let nav = use_context::<SettingsNav>();
    let coach = use_context::<crate::coach::Coach>();
    let cloud = try_use_context::<crate::Cloud>();
    let lang = ui.lang();
    let mut section_sig = nav.section;
    let name = coach.profile.read().as_ref().map(|p| p.name.trim().to_string()).filter(|n| !n.is_empty());
    let sync = cloud.map(|c| c.status.read().sync);
    // What a row says under its title: live state where there is one.
    let detail = move |s: Section| -> String {
        match s {
            Section::Account => match sync.and_then(crate::account::sync_msg) {
                Some(m) => ui.t(m).to_string(),
                None => st(lang, S::AccountSub).to_string(),
            },
            Section::About => format!("{} {}", st(lang, S::Version), env!("CARGO_PKG_VERSION")),
            s => st(lang, s.summary()).to_string(),
        }
    };
    rsx! {
        nav { class: "settings-sections", aria_label: "{st(lang, S::Sections)}",
            if let Some(name) = name {
                div { class: "card grouped settings-me",
                    button {
                        class: "activity-row",
                        onclick: move |_| section_sig.set(Some(Section::Coach)),
                        span { class: "row-icon settings-avatar", aria_hidden: "true",
                            {name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()}
                        }
                        span { class: "row-copy",
                            strong { bdi { "{name}" } }
                            small { "{st(lang, S::ProfileSub)}" }
                        }
                        span { class: "row-end", span { class: "chev", aria_hidden: "true" } }
                    }
                }
            }
            for (title, members) in GROUPS {
                {
                    let shown: Vec<Section> = members.iter().copied().filter(|s| sections.contains(s)).collect();
                    rsx! {
                        if !shown.is_empty() {
                            div { key: "{st(lang, title)}", class: "section-head", h2 { "{st(lang, title)}" } }
                            div { class: "card grouped",
                                for s in shown {
                                    button {
                                        key: "{s.id()}",
                                        class: "activity-row",
                                        aria_current: if current == Some(s) { "true" } else { "false" },
                                        onclick: move |_| section_sig.set(Some(s)),
                                        span { class: "row-icon", aria_hidden: "true", {crate::icons::settings_section(s)} }
                                        span { class: "row-copy",
                                            strong { "{st(lang, s.title())}" }
                                            small { "{detail(s)}" }
                                        }
                                        span { class: "row-end", span { class: "chev", aria_hidden: "true" } }
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

#[component]
fn SectionView(section: Section) -> Element {
    match section {
        Section::Learning => rsx! { learning::LearningSection {} },
        Section::Look => rsx! { look::LookSection {} },
        Section::Coach => rsx! { crate::coach::settings::CoachSettings {} },
        Section::Android => rsx! { platform_sections::AndroidSection {} },
        Section::Tablet => rsx! { platform_sections::TabletSection {} },
        Section::Mac => rsx! { platform_sections::MacSection {} },
        Section::Storage => rsx! { platform_sections::StorageSection {} },
        Section::Account => rsx! { platform_sections::AccountSection {} },
        Section::About => rsx! { platform_sections::AboutSection {} },
    }
}

/// A small pill after a field label: whether the choice syncs or stays here.
#[component]
pub fn Scope(device_only: bool) -> Element {
    let ui = use_context::<Ui>();
    let s = if device_only { S::ThisDevice } else { S::Synced };
    rsx! { span { class: "pill settings-scope", "{st(ui.lang(), s)}" } }
}

/// The gear that opens Settings (phone top bar).
#[component]
pub fn GearButton() -> Element {
    let ui = use_context::<Ui>();
    let nav = use_context::<SettingsNav>();
    let label = ui.t(crate::i18n::Msg::NavSettings);
    rsx! {
        button {
            class: "icon-button gear-button",
            aria_label: "{label}",
            title: "{label}",
            onclick: move |_| nav.show(None),
            {crate::icons::gear()}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_sections_show_only_where_they_apply() {
        let here: Vec<Section> = Section::ALL.into_iter().filter(|s| s.shown(false)).collect();
        assert!(here.contains(&Section::Learning) && here.contains(&Section::About));
        assert!(!here.contains(&Section::Tablet), "a phone or Mac never shows the tablet section");
        if cfg!(target_os = "android") {
            assert!(here.contains(&Section::Android) && !here.contains(&Section::Mac));
        } else {
            assert!(here.contains(&Section::Mac) && !here.contains(&Section::Android));
            assert!(!Section::Tablet.shown(true), "the tablet section is Android-only");
        }
    }

    #[test]
    fn every_section_sits_in_one_group() {
        for s in Section::ALL {
            let n = GROUPS.iter().filter(|(_, m)| m.contains(&s)).count();
            assert_eq!(n, 1, "{s:?} must be listed exactly once");
        }
    }

    #[test]
    fn section_ids_round_trip() {
        for s in Section::ALL {
            assert_eq!(Section::from_id(s.id()), Some(s));
        }
        assert_eq!(Section::from_id("nope"), None);
    }
}
