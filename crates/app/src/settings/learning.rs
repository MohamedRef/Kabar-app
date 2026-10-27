//! Settings › Learning: how Indonesian and Javanese text is helped along.
//! These sync to every device.

use dioxus::prelude::*;
use kabar_core::settings::{PronMode, RetentionPct, TranslationMode};

use super::text::{S, st};
use crate::i18n::Msg;
use crate::{Db, Ui};

#[component]
pub fn LearningSection() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let settings = ui.settings.read().clone();
    let lang = ui.lang();
    let tuning = match &settings.fsrs {
        Some(p) => ui.t(Msg::RetentionTuned).replace("{n}", &ui.num(u64::from(p.reviews))),
        None => ui
            .t(Msg::RetentionLearning)
            .replace("{n}", &ui.num(kabar_core::srs::MIN_REVIEWS_TO_OPTIMIZE as u64)),
    };

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
                        checked: !settings.word_colours_off,
                        onchange: {
                            let db = db.clone();
                            move |e: FormEvent| {
                                let on = e.checked();
                                ui.update_settings(&db, |s| s.word_colours_off = !on)
                            }
                        },
                    }
                    " {st(lang, S::WordColours)}"
                }
            }

            div { class: "field",
                span { class: "label", "{ui.t(Msg::SettingsRetention)}" }
                div { class: "row", role: "group",
                    for (pct, label) in [(85u8, Msg::RetentionLight), (90, Msg::RetentionBalanced), (95, Msg::RetentionFirm)] {
                        button {
                            key: "{pct}",
                            aria_pressed: if settings.desired_retention.0 == pct { "true" } else { "false" },
                            onclick: {
                                let db = db.clone();
                                move |_| ui.update_settings(&db, |s| s.desired_retention = RetentionPct(pct))
                            },
                            "{ui.t(label)} · {ui.whole_percent(pct)}"
                        }
                    }
                }
                p { class: "muted small", "{ui.t(Msg::RetentionHint)}" }
                p { class: "muted small", "{tuning}" }
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
