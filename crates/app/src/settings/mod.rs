//! Settings: one sheet over the current screen, so opening it never loses a
//! study session. The gear in the top bar opens it.
//!
//! A feature chunk that needs settings adds its `Section` variant (in
//! `ALL`, with a title, summary and id) and one line in `SectionView`, and
//! keeps the rows in its own module. Sections with nothing in them yet are
//! simply not listed.
// adapted from kabar-app main: crates/app/src/settings/mod.rs

mod about;
mod data;
mod look;

use dioxus::prelude::*;

use crate::i18n::settings as tx;

const SETTINGS_CSS: &str = include_str!("../../assets/settings.css");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Look,
    Data,
    About,
}

impl Section {
    pub const ALL: [Self; 3] = [Self::Look, Self::Data, Self::About];

    /// Stable id, for `BATNA_OPEN_SETTINGS=<id>` previews.
    pub fn id(self) -> &'static str {
        match self {
            Self::Look => "look",
            Self::Data => "data",
            Self::About => "about",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }

    fn title(self) -> &'static str {
        match self {
            Self::Look => tx::LOOK,
            Self::Data => tx::DATA,
            Self::About => tx::ABOUT,
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Self::Look => tx::LOOK_SUB,
            Self::Data => tx::DATA_SUB,
            Self::About => tx::ABOUT_SUB,
        }
    }
}

/// Whether the sheet is open and on which section. `section: None` is the
/// section list on the phone; wider windows show the first section beside it.
#[derive(Clone, Copy)]
pub struct SettingsNav {
    pub open: Signal<bool>,
    pub section: Signal<Option<Section>>,
}

impl SettingsNav {
    pub fn new() -> Self {
        // BATNA_OPEN_SETTINGS=look (or "" for the list) opens the sheet at launch.
        let start = std::env::var("BATNA_OPEN_SETTINGS").ok();
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

/// The Settings sheet; renders nothing while closed.
#[component]
pub fn SettingsSheet() -> Element {
    let nav = use_context::<SettingsNav>();
    // Escape steps back out of a section on the phone, else closes. Listened
    // for on the document: WebKit doesn't focus buttons on click.
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            if (!window.__batnaSettingsEsc) {
              window.__batnaSettingsEsc = true;
              document.addEventListener("keydown", (e) => {
                if (e.key === "Escape" && document.querySelector(".settings-window")) {
                  e.preventDefault();
                  e.stopPropagation();
                  dioxus.send(window.matchMedia("(max-width: 599.98px)").matches);
                }
              }, true);
            }
            await new Promise(() => {});
            "#,
        );
        let mut section = nav.section;
        while let Ok(compact) = eval.recv::<bool>().await {
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
    let chosen = *nav.section.read();
    // Phones show the list until a section is picked; wider windows show both.
    let current = chosen.unwrap_or(Section::ALL[0]);
    let drilled = chosen.is_some();
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
            "data-drilled": if drilled { "true" } else { "false" },
            onmounted: move |e: MountedEvent| async move {
                let _ = e.set_focus(true).await;
            },
            header { class: "settings-head",
                button {
                    class: "icon-button settings-back",
                    "data-back": "true",
                    aria_label: tx::BACK,
                    onclick: move |_| section_sig.set(None),
                    span { aria_hidden: "true", "→" }
                }
                h2 { id: "settings-title",
                    span { class: "settings-title-list", {tx::TITLE} }
                    span { class: "settings-title-section", {current.title()} }
                }
                button {
                    class: "icon-button",
                    "data-back": "true",
                    aria_label: tx::CLOSE,
                    title: tx::CLOSE,
                    onclick: move |_| nav.close(),
                    span { aria_hidden: "true", "✕" }
                }
            }
            div { class: "settings-body",
                nav { class: "settings-sections", aria_label: tx::SECTIONS,
                    for s in Section::ALL {
                        button {
                            key: "{s.id()}",
                            aria_current: if current == s { "true" } else { "false" },
                            onclick: move |_| section_sig.set(Some(s)),
                            span { class: "ss-title", {s.title()} }
                            span { class: "ss-sub", {s.summary()} }
                        }
                    }
                }
                div { class: "settings-pane", key: "{current.id()}",
                    SectionView { section: current }
                }
            }
        }
    }
}

#[component]
fn SectionView(section: Section) -> Element {
    match section {
        Section::Look => rsx! { look::LookSection {} },
        Section::Data => rsx! { data::DataSection {} },
        Section::About => rsx! { about::AboutSection {} },
    }
}

/// A small pill after a field label: this choice stays on this device.
#[component]
pub fn DeviceOnly() -> Element {
    rsx! { span { class: "chip settings-scope", {tx::THIS_DEVICE} } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_ids_round_trip() {
        for s in Section::ALL {
            assert_eq!(Section::from_id(s.id()), Some(s));
        }
        assert_eq!(Section::from_id("nope"), None);
    }
}
