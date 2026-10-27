//! Settings › Learning: how Indonesian and Javanese text is helped along.
//! These sync to every device.

use dioxus::prelude::*;
use kabar_core::settings::{PronMode, TranslationMode};

use super::text::{S, st};
use crate::i18n::Msg;
use crate::{Db, Ui};

#[component]
pub fn LearningSection() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let settings = ui.settings.read().clone();
    let lang = ui.lang();

    rsx! {
        div { class: "card",
            div { class: "field",
                span { class: "label", "{ui.t(Msg::SettingsPron)}" }
                div { class: "row", role: "group",
                    for mode in PronMode::ALL {
                        button {
                            key: "{mode:?}",
                            aria_pressed: if mode == settings.pronunciation { "true" } else { "false" },
                            onclick: {
                                let db = db.clone();
                                move |_| ui.update_settings(&db, |s| s.pronunciation = mode)
                            },
                            "{ui.t(pron_mode_label(mode))}"
                        }
                    }
                }
                details { class: "pron-details",
                    summary { "{ui.t(Msg::PronHowToRead)}" }
                    crate::pron::PronGuide {}
                }
            }

            div { class: "field",
                span { class: "label", "{st(lang, S::Translation)}" }
                div { class: "row", role: "group",
                    for mode in TranslationMode::ALL {
                        button {
                            key: "{mode:?}",
                            aria_pressed: if mode == settings.translation { "true" } else { "false" },
                            onclick: {
                                let db = db.clone();
                                move |_| ui.update_settings(&db, |s| s.translation = mode)
                            },
                            "{st(lang, translation_label(mode))}"
                        }
                    }
                }
            }

            div { class: "field",
                label {
                    input {
                        r#type: "checkbox",
                        checked: !settings.sounds_off,
                        onchange: {
                            let db = db.clone();
                            move |e: FormEvent| {
                                let on = e.checked();
                                ui.update_settings(&db, |s| s.sounds_off = !on)
                            }
                        },
                    }
                    " {ui.t(Msg::SettingsSounds)}"
                }
            }
        }
    }
}

fn pron_mode_label(mode: PronMode) -> Msg {
    match mode {
        PronMode::On => Msg::PronModeOn,
        PronMode::Tap => Msg::PronModeTap,
        PronMode::Off => Msg::PronModeOff,
    }
}

fn translation_label(mode: TranslationMode) -> S {
    match mode {
        TranslationMode::On => S::TranslationOn,
        TranslationMode::Tap => S::TranslationTap,
        TranslationMode::Off => S::TranslationOff,
    }
}
