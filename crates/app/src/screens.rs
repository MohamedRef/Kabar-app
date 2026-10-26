use dioxus::prelude::*;
use kabar_core::locale::{UiLanguage, format_number};
use kabar_core::settings::Theme;

use crate::i18n::Msg;
use crate::{Db, Screen, Ui};

/// First launch: "Apa kabar?" and the reveal that kabar comes from خبر.
#[component]
pub fn Welcome() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let lang = ui.settings.read().ui_language;

    rsx! {
        div { class: "welcome",
            div { class: "card",
                p { class: "greeting reveal", bdi { lang: "id", "Apa kabar?" } }
                div { class: "reveal d2",
                    p { class: "muted", "{ui.t(Msg::WelcomeKabarFrom)}" }
                    div { class: "bridge",
                        span { class: "bridge-word latin", lang: "id", "kabar" }
                        span { aria_hidden: "true", if lang == UiLanguage::En { "←" } else { "→" } }
                        span { class: "ar", lang: "ar", "خبر" }
                    }
                    p { "{ui.t(Msg::WelcomeExplain)}" }
                }
                div { class: "reveal d3",
                    p { class: "muted", "{ui.t(Msg::WelcomeFirstWord)}" }
                    LanguagePicker {}
                    p {
                        button {
                            class: "primary",
                            onclick: move |_| ui.update_settings(&db, |s| s.welcomed = true),
                            "{ui.t(Msg::WelcomeStart)}"
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn Today() -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "card",
            h2 { lang: "id", bdi { "{ui.t(Msg::TodayTitle)}" } }
            p { "{ui.t(Msg::TodayBody)}" }
        }
        div { class: "card",
            p { class: "greeting", style: "font-size: 2rem", bdi { lang: "id", "Apa kabar?" } }
            div { class: "bridge",
                span { class: "bridge-word latin", lang: "id", "kabar" }
                span { class: "ar", lang: "ar", "خبر" }
            }
        }
    }
}

#[component]
pub fn ComingSoon(title: Msg, milestone: &'static str) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "card",
            h2 { "{ui.t(title)}" }
            p { class: "muted",
                "{ui.t(Msg::ComingIn)} "
                span { class: "pill latin", bdi { "{milestone}" } }
            }
        }
    }
}

#[component]
fn LanguagePicker() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let current = ui.settings.read().ui_language;
    rsx! {
        div { class: "row", role: "group",
            for lang in UiLanguage::ALL {
                button {
                    key: "{lang.tag()}",
                    lang: lang.tag(),
                    aria_pressed: if lang == current { "true" } else { "false" },
                    onclick: {
                        let db = db.clone();
                        move |_| ui.update_settings(&db, |s| s.ui_language = lang)
                    },
                    "{lang.native_name()}"
                }
            }
        }
    }
}

#[component]
pub fn SettingsScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let settings = ui.settings.read().clone();
    let device = db.0.borrow().device_id().to_string();
    let width_class = *ui.width.read();
    let sample = format_number(2026, settings.ui_language, settings.eastern_digits);

    let theme_label = |th: Theme| match th {
        Theme::System => Msg::ThemeSystem,
        Theme::Light => Msg::ThemeLight,
        Theme::Dark => Msg::ThemeDark,
    };

    rsx! {
        div { class: "card",
            h2 { "{ui.t(Msg::NavSettings)}" }

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
                            "{ui.t(theme_label(th))}"
                        }
                    }
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

            div { class: "field",
                span { class: "label", "{ui.t(Msg::Width)}" }
                span { class: "pill latin", bdi { "{width_class.as_str()}" } }
            }

            div { class: "field",
                span { class: "label", "{ui.t(Msg::SettingsDevice)}" }
                code { bdi { "{device}" } }
            }

            div { class: "row",
                button {
                    onclick: {
                        let db = db.clone();
                        move |_| ui.update_settings(&db, |s| s.welcomed = false)
                    },
                    "{ui.t(Msg::SettingsShowWelcome)}"
                }
                for screen in [Screen::Studio, Screen::PenLab] {
                    button {
                        key: "{screen:?}",
                        onclick: move |_| {
                            let mut s = ui.screen;
                            s.set(screen);
                        },
                        "{ui.t(screen.label())}"
                    }
                }
            }
        }
    }
}
