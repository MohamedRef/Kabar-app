//! Settings › Look & language: the UI language, theme and digits (synced) and
//! the text size (this device only: the phone and the Mac want different ones).

use dioxus::prelude::*;

use crate::ui::Card;
use kabar_core::locale::format_number;
use kabar_core::settings::Theme;

use super::Scope;
use super::device::{Appearance, DevicePrefs, TextSize};
use super::text::{S, st};
use crate::i18n::Msg;
use crate::screens::LanguagePicker;
use crate::{Db, Ui};

#[component]
pub fn LookSection() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let prefs = use_context::<DevicePrefs>();
    let settings = ui.settings.read().clone();
    let lang = ui.lang();
    let sample = format_number(2026, settings.ui_language, settings.eastern_digits);
    let size = *prefs.text_size.read();
    let appearance = *prefs.appearance.read();

    rsx! {
        Card {
            div { class: "field",
                span { class: "label", "{ui.t(Msg::SettingsLanguage)}" }
                LanguagePicker {}
            }

            div { class: "field",
                span { class: "label", "{ui.t(Msg::SettingsTheme)}" }
                div { class: "row", role: "group",
                    for th in Theme::ALL {
                        button {
                            key: "{th.as_str()}",
                            aria_pressed: if th == settings.theme { "true" } else { "false" },
                            onclick: {
                                let db = db.clone();
                                move |_| ui.update_settings(&db, |s| s.theme = th)
                            },
                            span { class: "theme-swatch", aria_hidden: "true", style: "{swatch(th)}" }
                            "{ui.t(theme_label(th))}"
                        }
                    }
                }
                if matches!(settings.theme, Theme::Black | Theme::SystemBlack) {
                }
            }

            div { class: "field",
                span { class: "label", id: "appearance-label", "{st(lang, S::Appearance)}" Scope { device_only: true } }
                div { class: "segmented", role: "radiogroup", aria_labelledby: "appearance-label",
                    for &a in Appearance::ALL {
                        button {
                            key: "{a.as_str()}",
                            role: "radio",
                            aria_checked: if a == appearance { "true" } else { "false" },
                            onclick: {
                                let db = db.clone();
                                move |_| prefs.set_appearance(&db, a)
                            },
                            "{st(lang, appearance_label(a))}"
                        }
                    }
                }
            }

            div { class: "field",
                span { class: "label", "{st(lang, S::TextSize)}" Scope { device_only: true } }
                div { class: "row", role: "group",
                    for t in TextSize::ALL {
                        button {
                            key: "{t.as_str()}",
                            aria_pressed: if t == size { "true" } else { "false" },
                            onclick: {
                                let db = db.clone();
                                move |_| prefs.set_text_size(&db, t)
                            },
                            "{st(lang, text_size_label(t))}"
                        }
                    }
                }
                div { class: "text-sample",
                    p { lang: "id", dir: "ltr", class: "latin", "Apa kabar? Kabar baik." }
                    p { class: "muted", "{st(lang, S::TextSample)}" }
                }
            }

            div { class: "field",
                label {
                    input {
                        r#type: "checkbox",
                        checked: settings.eastern_digits,
                        onchange: {
                            let db = db.clone();
                            move |e: FormEvent| {
                                let on = e.checked();
                                ui.update_settings(&db, |s| s.eastern_digits = on)
                            }
                        },
                    }
                    " {ui.t(Msg::SettingsDigits)} · {sample}"
                }
            }
        }
    }
}

fn theme_label(th: Theme) -> Msg {
    match th {
        Theme::System => Msg::ThemeSystem,
        Theme::SystemBlack => Msg::ThemeSystemBlack,
        Theme::Light => Msg::ThemeLight,
        Theme::Dark => Msg::ThemeDark,
        Theme::Black => Msg::ThemeBlack,
        Theme::Night => Msg::ThemeNight,
        Theme::Batik => Msg::ThemeBatik,
        Theme::Sawah => Msg::ThemeSawah,
    }
}

/// A swatch previews the theme itself, not the current one: its surface,
/// accent and Bridge gold. A theme that follows the OS shows its day and
/// night surfaces side by side.
fn swatch(th: Theme) -> String {
    match (th.palette(), th.follows_os()) {
        (Some(p), _) => format!(
            "background: linear-gradient(135deg, {} 50%, {} 50%); border-color: {}",
            p.surface, p.indigo, p.gold
        ),
        (None, Some((day, night))) => {
            format!("background: linear-gradient(135deg, {} 50%, {} 50%)", day.surface, night.surface)
        }
        (None, None) => String::new(),
    }
}

fn appearance_label(a: Appearance) -> S {
    match a {
        Appearance::Calm => S::AppearanceCalm,
        Appearance::Glossy => S::AppearanceGlossy,
        Appearance::LiquidGlass => S::AppearanceLiquidGlass,
    }
}

fn text_size_label(t: TextSize) -> S {
    match t {
        TextSize::Small => S::TextSmall,
        TextSize::Normal => S::TextNormal,
        TextSize::Large => S::TextLarge,
        TextSize::Larger => S::TextLarger,
    }
}
