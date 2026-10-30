//! Settings: one sheet over the current screen, so opening it never loses a
//! lesson or a Reader page. The gear in the phone's top bar, the Settings item
//! pinned to the foot of the tablet and Mac sidebar, and Kabar › Settings… (⌘,)
//! on the Mac all open it.
//!
//! Sections follow the device: Android, Tablet and Mac show only where they
//! apply. Shared learning preferences sync to every device; the rest are
//! marked "this device only" (`device::DevicePrefs`).

#[cfg(test)]
mod browser;

pub mod device;
mod learning;
mod look;
#[cfg(all(feature = "desktop", not(target_os = "android")))]
pub mod menu;
mod platform_sections;
mod search;
mod text;

use dioxus::prelude::*;
use kabar_core::layout::WidthClass;

use crate::Ui;
use crate::ui::{ActionRow, Card};
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
    Updates,
    About,
}

impl Section {
    pub const ALL: [Self; 10] = [
        Self::Learning,
        Self::Look,
        Self::Coach,
        Self::Android,
        Self::Tablet,
        Self::Mac,
        Self::Storage,
        Self::Account,
        Self::Updates,
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
            Self::Updates => "updates",
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
            Self::Updates => S::Updates,
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
            Self::Updates => S::UpdatesSub,
            Self::About => S::AboutSub,
        }
    }
    fn matches(self, query: &str) -> bool {
        use kabar_core::locale::UiLanguage;
        let keywords = match self {
            Self::Learning => "pronunciation translation sounds vibration نطق ترجمة صوت اهتزاز",
            Self::Look => "theme font size language digits appearance calm glossy ثيم خط حجم لغة ارقام ألوان",
            Self::Coach => "notifications reminders quiet hours plan تنبيهات اشعارات تذكير مواعيد خطة",
            Self::Android => "notifications permission audio background pen اذونات إشعارات صوت خلفية قلم",
            Self::Tablet => "sidebar navigation layout pen قائمة تخطيط قلم",
            Self::Mac => "keyboard shortcuts mouse sidebar اختصارات كيبورد ماوس قائمة",
            Self::Storage => {
                "backup restore export import download offline diagnostics نسخة احتياطية استرجاع تصدير استيراد تحميل تشخيص"
            }
            Self::Account => "google sign in sync devices cost limits جوجل دخول مزامنة أجهزة تكلفة حدود",
            Self::Updates => "version download install changelog اصدار نسخة تنزيل تثبيت جديد",
            Self::About => "version welcome device اصدار نسخة ترحيب جهاز",
        };
        let mut text = format!("{} {keywords}", self.id());
        for lang in [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En] {
            text.push_str(&format!(" {} {}", st(lang, self.title()), st(lang, self.summary())));
        }
        search::matches(query, &text)
    }
}

/// Whether the sheet is open and on which section. `section: None` is the
/// section list on the phone; wider windows show Learning beside the list.
#[derive(Clone, Copy)]
pub struct SettingsNav {
    pub open: Signal<bool>,
    pub section: Signal<Option<Section>>,
    query: Signal<String>,
}

impl SettingsNav {
    pub fn new() -> Self {
        let start = std::env::var("KABAR_OPEN_SETTINGS").ok();
        Self {
            open: Signal::new(start.is_some()),
            section: Signal::new(start.as_deref().and_then(Section::from_id)),
            query: Signal::new(String::new()),
        }
    }

    pub fn show(mut self, section: Option<Section>) {
        self.query.set(String::new());
        self.section.set(section);
        self.open.set(true);
    }

    pub fn close(mut self) {
        self.open.set(false);
    }
}

const SETTINGS_CSS: crate::embedded::Css = crate::embedded::css!("assets/settings.css");

/// The Settings sheet; renders nothing while closed.
#[component]
pub fn SettingsSheet() -> Element {
    let ui = use_context::<Ui>();
    let nav = use_context::<SettingsNav>();
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
        style { {SETTINGS_CSS.text()} }
        div { class: "settings-backdrop", onclick: move |_| nav.close() }
        div {
            class: "settings-window",
            role: "dialog",
            aria_modal: "true",
            aria_labelledby: "settings-title",
            tabindex: "-1",
            "data-drilled": if drilled { "true" } else { "false" },
            onmounted: move |_| {
                document::eval(include_str!("../../assets/settings-dialog.js"));
            },
            header { class: "settings-head",
                if drilled {
                    button {
                        class: "icon-button settings-back",
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
            crate::persistence::SaveNotice {}
            div { class: "settings-body",
                if !drilled {
                    SectionList { sections: sections.clone(), current: if compact { None } else { current } }
                }
                if let Some(s) = current {
                    div { class: "settings-pane", key: "{s.id()}", role: "region", aria_labelledby: "settings-section-title",
                        header { class: "settings-intro",
                            h3 { id: "settings-section-title", "{st(lang, s.title())}" }
                            p { "{st(lang, s.summary())}" }
                        }
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
    (S::GroupData, &[Section::Account, Section::Storage, Section::Updates, Section::About]),
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
    let mut section = nav.section;
    let mut query = nav.query;
    let name = coach.profile.read().as_ref().map(|p| p.name.trim().to_string()).filter(|n| !n.is_empty());
    let sync = cloud.map(|c| c.status.read().sync);
    let updates = crate::update::view::use_update_status().read().clone();
    let filtered: Vec<Section> = sections.into_iter().filter(|s| s.matches(&query.read())).collect();
    let detail = move |s: Section| -> String {
        match s {
            Section::Account => sync
                .and_then(crate::account::sync_msg)
                .map(|m| ui.t(m))
                .unwrap_or(st(lang, S::AccountSub))
                .to_string(),
            Section::Updates => updates
                .as_ref()
                .map(|u| crate::update::view::summary(&ui, u))
                .unwrap_or_else(|| st(lang, S::UpdatesSub).to_string()),
            Section::About => format!("{} {}", st(lang, S::Version), env!("CARGO_PKG_VERSION")),
            s => st(lang, s.summary()).to_string(),
        }
    };
    rsx! {
        aside { class: "settings-index",
            div { class: "settings-tools",
                input {
                    class: "settings-search", r#type: "search",
                    aria_label: "{st(lang, S::Search)}", placeholder: "{st(lang, S::Search)}",
                    value: "{query}", oninput: move |event| query.set(event.value()),
                }
                p { class: "settings-save-hint", "{st(lang, S::Immediate)}" }
            }
            nav { class: "settings-sections", aria_label: "{st(lang, S::Sections)}",
                if query.read().trim().is_empty() {
                    if let Some(name) = name {
                        Card { class: "grouped settings-me",
                            ActionRow {
                                title: name.clone(), detail: st(lang, S::ProfileSub),
                                icon: rsx! { span { class: "settings-avatar", {name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()} } },
                                onclick: move |_| section.set(Some(Section::Coach)),
                            }
                        }
                    }
                }
                for (title, members) in GROUPS {
                    {
                        let shown: Vec<Section> = members.iter().copied().filter(|s| filtered.contains(s)).collect();
                        rsx! {
                            if !shown.is_empty() {
                                div { key: "{st(lang, title)}", class: "section-head", h2 { "{st(lang, title)}" } }
                                Card { class: "grouped",
                                    for s in shown {
                                        ActionRow {
                                            key: "{s.id()}", title: st(lang, s.title()), detail: detail(s),
                                            selected: current == Some(s), icon: crate::icons::settings_section(s),
                                            onclick: move |_| section.set(Some(s)),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if filtered.is_empty() {
                    div { class: "settings-empty", role: "status",
                        p { "{st(lang, S::NoResults)}" }
                        button { r#type: "button", onclick: move |_| query.set(String::new()), "{st(lang, S::ClearSearch)}" }
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
        Section::Updates => rsx! { crate::update::view::UpdatesCard {} },
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

    #[test]
    fn search_finds_preferences_inside_their_section() {
        assert!(Section::Look.matches("حجم الخط"));
        assert!(Section::Look.matches("FONT size"));
        assert!(Section::Coach.matches("إِشْعَارات"));
        assert!(Section::Storage.matches("نسخة احتياطية"));
        assert!(!Section::ALL.into_iter().any(|s| s.matches("zzzz-no-setting")));
        for section in Section::ALL {
            assert!(section.matches(section.id()));
        }
    }
}
