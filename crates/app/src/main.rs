//! Batna: one Dioxus codebase for the Mac, the S24 Ultra and the Tab S11.
//!
//! Feature chunks (SPLIT.md §7) each own a module directory and register it
//! with one line in `Screen` / `screen_body` below.

mod calc;
mod case;
mod content;
mod i18n;
mod oral;
mod order;
mod platform;
mod prefs;
mod settings;
mod table;
mod today;

use dioxus::prelude::*;

const MAIN_CSS: &str = include_str!("../assets/main.css");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Today,
    Study,
    Bank,
    Cases,
    Oral,
    Stats,
    Settings,
}

impl Screen {
    /// Bottom bar on phones, side rail from 600dp. Settings is a sheet opened from the top bar.
    pub const NAV: [Screen; 6] = [Self::Today, Self::Study, Self::Bank, Self::Cases, Self::Oral, Self::Stats];

    pub fn label(self) -> &'static str {
        use i18n::nav;
        match self {
            Self::Today => nav::TODAY,
            Self::Study => nav::STUDY,
            Self::Bank => nav::BANK,
            Self::Cases => nav::CASES,
            Self::Oral => nav::ORAL,
            Self::Stats => nav::STATS,
            Self::Settings => nav::SETTINGS,
        }
    }

    /// A one-glyph icon for the nav (text, no image assets).
    fn glyph(self) -> &'static str {
        match self {
            Self::Today => "◐",
            Self::Study => "✎",
            Self::Bank => "?",
            Self::Cases => "✚",
            Self::Oral => "◉",
            Self::Stats => "▤",
            Self::Settings => "⚙",
        }
    }

    fn from_env() -> Option<Screen> {
        // BATNA_OPEN_SCREEN=study opens that screen at launch (previews and screenshots).
        let name = std::env::var("BATNA_OPEN_SCREEN").ok()?;
        [Self::NAV.as_slice(), &[Self::Settings]]
            .concat()
            .into_iter()
            .find(|s| format!("{s:?}").eq_ignore_ascii_case(&name))
    }
}

/// App-wide UI state, shared through context.
#[derive(Clone, Copy)]
pub struct Ui {
    pub screen: Signal<Screen>,
    /// This device's theme and text size (Settings › Look).
    pub prefs: Signal<prefs::DevicePrefs>,
}

fn main() {
    #[cfg(all(feature = "desktop", not(target_os = "android")))]
    {
        use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
        // BATNA_WINDOW_SIZE=412x900 previews the phone layout on the Mac.
        let (w, h) = std::env::var("BATNA_WINDOW_SIZE")
            .ok()
            .and_then(|v| {
                let (w, h) = v.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or((1100.0, 760.0));
        let window = WindowBuilder::new()
            .with_title("Batna")
            .with_inner_size(LogicalSize::new(w, h))
            .with_min_inner_size(LogicalSize::new(360.0, 560.0));
        dioxus::LaunchBuilder::desktop()
            .with_cfg(Config::new().with_window(window).with_menu(None).with_disable_context_menu(true))
            .launch(App);
    }
    #[cfg(not(all(feature = "desktop", not(target_os = "android"))))]
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let ui = use_context_provider(|| Ui {
        screen: Signal::new(Screen::from_env().unwrap_or(Screen::Today)),
        prefs: Signal::new(prefs::DevicePrefs::load()),
    });
    let settings = use_context_provider(settings::SettingsNav::new);
    let current = *ui.screen.read();
    let look = ui.prefs.read().css();
    rsx! {
        document::Title { "Batna" }
        style { {MAIN_CSS} }
        style { {look} }
        div { class: "app", lang: "ar", dir: "rtl",
            header { class: "topbar",
                div { class: "brand",
                    span { class: "brand-name", {i18n::app::NAME} }
                    span { class: "brand-tag", {i18n::app::TAGLINE} }
                }
                button {
                    class: "icon-button",
                    aria_label: Screen::Settings.label(),
                    title: Screen::Settings.label(),
                    onclick: move |_| settings.show(None),
                    span { class: "glyph", aria_hidden: "true", {Screen::Settings.glyph()} }
                }
            }
            nav { class: "nav", aria_label: "التنقل",
                for screen in Screen::NAV {
                    NavButton { key: "{screen:?}", screen, class: "nav-item" }
                }
            }
            main { class: "content", "data-screen": "{current:?}", {screen_body(current)} }
            settings::SettingsSheet {}
        }
    }
}

/// The body of each screen. A feature chunk swaps its `Soon` for its own component.
fn screen_body(screen: Screen) -> Element {
    match screen {
        Screen::Today => rsx! { today::Today {} },
        Screen::Study => rsx! { Soon { screen } },
        Screen::Bank => rsx! { Soon { screen } },
        Screen::Cases => rsx! { case::CasesScreen {} },
        Screen::Oral => rsx! { oral::Oral {} },
        Screen::Stats => rsx! { Soon { screen } },
        Screen::Settings => rsx! { Soon { screen } },
    }
}

#[component]
fn NavButton(screen: Screen, class: &'static str) -> Element {
    let mut ui = use_context::<Ui>();
    let active = *ui.screen.read() == screen;
    rsx! {
        button {
            class,
            "aria-current": if active { "page" } else { "false" },
            "aria-label": screen.label(),
            onclick: move |_| ui.screen.set(screen),
            span { class: "glyph", aria_hidden: "true", {screen.glyph()} }
            span { class: "label", {screen.label()} }
        }
    }
}

/// A placeholder for a screen whose chunk hasn't landed yet.
#[component]
fn Soon(screen: Screen) -> Element {
    rsx! {
        section { class: "card soon",
            h1 { {screen.label()} }
            h2 { {i18n::soon::TITLE} }
            p { {i18n::soon::BODY} }
        }
    }
}
