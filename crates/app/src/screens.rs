use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;

use crate::i18n::Msg;
use crate::{Db, Ui};

/// First launch: "Apa kabar?" and the reveal that kabar comes from خبر.
#[component]
pub fn Welcome() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let lang = ui.settings.read().ui_language;

    rsx! {
        div { class: "welcome",
            div { class: "card",
                // Dawn over the terraces (prompt 7); the greeting sits in its empty sky.
                div { class: "hero",
                    crate::misc_art::Picture { art: crate::misc_art::Art::HeroLandscape, class: "wide" }
                    crate::misc_art::Picture { art: crate::misc_art::Art::HeroPortrait, class: "tall" }
                    p { class: "greeting reveal", bdi { lang: "id", dir: "ltr", "Apa kabar?" } }
                }
                div { class: "reveal d2",
                    p { class: "muted", crate::review::Isolated { text: ui.t(Msg::WelcomeKabarFrom).to_string() } }
                    div { class: "bridge",
                        span { class: "bridge-word latin", lang: "id", "kabar" }
                        span { aria_hidden: "true", if lang == UiLanguage::En { "←" } else { "→" } }
                        span { class: "ar", lang: "ar", "خبر" }
                    }
                    p { crate::review::Isolated { text: ui.t(Msg::WelcomeExplain).to_string() } }
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
pub fn LanguagePicker() -> Element {
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
