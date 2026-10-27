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
.app:not([data-width="compact"]) .settings-body { grid-template-columns: 17rem 1fr; }
.settings-sections {
  display: flex; flex-direction: column; gap: 0.2rem; padding: 0.75rem; overflow-y: auto;
  background: var(--surface);
}
.app:not([data-width="compact"]) .settings-sections { border-inline-end: 1px solid var(--line); }
.settings-sections button {
  display: grid; grid-template-columns: 1.6rem 1fr; column-gap: 0.6rem; align-items: center;
  text-align: start; border: 0; background: transparent; border-radius: 12px; padding: 0.55rem 0.7rem;
}
.settings-sections button .ss-title { font-weight: 600; }
.settings-sections button .ss-sub { grid-column: 2; font-size: 0.82rem; color: var(--muted); }
.settings-sections button[aria-current="true"] { background: var(--surface-2); color: var(--indigo); }
.app[data-width="compact"] .settings-sections button { padding-block: 0.8rem; border-block-end: 1px solid var(--line); border-radius: 0; }
.settings-pane { overflow-y: auto; padding: 1rem; padding-block-end: max(2rem, env(safe-area-inset-bottom)); }
.settings-pane > .card:last-child { margin-block-end: 0; }
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
                    nav { class: "settings-sections", aria_label: "{st(lang, S::Sections)}",
                        for s in sections {
                            button {
                                key: "{s.id()}",
                                aria_current: if !compact && current == Some(s) { "true" } else { "false" },
                                onclick: move |_| section_sig.set(Some(s)),
                                span { class: "nav-icon", aria_hidden: "true", {crate::icons::settings_section(s)} }
                                span { class: "ss-title", "{st(lang, s.title())}" }
                                span { class: "ss-sub", "{st(lang, s.summary())}" }
                            }
                        }
                    }
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
    fn section_ids_round_trip() {
        for s in Section::ALL {
            assert_eq!(Section::from_id(s.id()), Some(s));
        }
        assert_eq!(Section::from_id("nope"), None);
    }
}
