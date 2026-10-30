//! Settings › Learning: how Indonesian and Javanese text is helped along.
//! These sync to every device.

use dioxus::prelude::*;

use crate::ui::Card;
use kabar_core::settings::{CorrectSound, EndSound, PronMode, RetentionPct, TranslationMode};

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
        Card {
            div { class: "field",
                span { class: "label", "{ui.t(Msg::SettingsPron)}" }
                div { class: "segmented", role: "radiogroup",
                    for mode in PronMode::ALL {
                        button {
                            key: "{mode:?}",
                            role: "radio",
                            aria_checked: if mode == settings.pronunciation { "true" } else { "false" },
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
                div { class: "segmented", role: "radiogroup",
                    for mode in TranslationMode::ALL {
                        button {
                            key: "{mode:?}",
                            role: "radio",
                            aria_checked: if mode == settings.translation { "true" } else { "false" },
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
                label { class: "switch-row",
                    input {
                        class: "switch",
                        role: "switch",
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
                label { class: "switch-row",
                    input {
                        class: "switch",
                        role: "switch",
                        r#type: "checkbox",
                        checked: !settings.grammar_hints_off,
                        onchange: {
                            let db = db.clone();
                            move |e: FormEvent| {
                                let on = e.checked();
                                ui.update_settings(&db, |s| s.grammar_hints_off = !on)
                            }
                        },
                    }
                    " {st(lang, S::GrammarHints)}"
                }
            }

            div { class: "field",
                span { class: "label", "{ui.t(Msg::SettingsRetention)}" }
                div { class: "segmented", role: "radiogroup",
                    for (pct, label) in [(85u8, Msg::RetentionLight), (90, Msg::RetentionBalanced), (95, Msg::RetentionFirm)] {
                        button {
                            key: "{pct}",
                            role: "radio",
                            aria_checked: if settings.desired_retention.0 == pct { "true" } else { "false" },
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
                label { class: "switch-row",
                    input {
                        class: "switch",
                        role: "switch",
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

            if !settings.sounds_off {
                div { class: "field",
                    span { class: "label", "{st(lang, S::SoundCorrect)}" }
                    div { class: "segmented", role: "radiogroup",
                        for choice in CorrectSound::ALL {
                            button {
                                key: "{choice.id()}",
                                role: "radio",
                            aria_checked: if settings.correct_sound == choice { "true" } else { "false" },
                                onclick: {
                                    let db = db.clone();
                                    move |_| {
                                        crate::sounds::preview_correct(choice);
                                        ui.update_settings(&db, |s| s.correct_sound = choice)
                                    }
                                },
                                "{st(lang, correct_label(choice))}"
                            }
                        }
                    }
                }
                div { class: "field",
                    span { class: "label", "{st(lang, S::SoundEnd)}" }
                    div { class: "segmented", role: "radiogroup",
                        for choice in EndSound::ALL {
                            button {
                                key: "{choice.id()}",
                                role: "radio",
                            aria_checked: if settings.end_sound == choice { "true" } else { "false" },
                                onclick: {
                                    let db = db.clone();
                                    move |_| {
                                        crate::sounds::preview_end(choice);
                                        ui.update_settings(&db, |s| s.end_sound = choice)
                                    }
                                },
                                "{st(lang, end_label(choice))}"
                            }
                        }
                    }
                    p { class: "muted small", "{st(lang, S::SoundHint)}" }
                }
            }

            if crate::platform::can_vibrate() {
                div { class: "field",
                    label { class: "switch-row",
                        input {
                            class: "switch",
                            role: "switch",
                            r#type: "checkbox",
                            checked: !settings.haptics_off,
                            onchange: {
                                let db = db.clone();
                                move |e: FormEvent| {
                                    let on = e.checked();
                                    ui.update_settings(&db, |s| s.haptics_off = !on);
                                    if on {
                                        crate::platform::vibrate(crate::platform::Haptic::Correct);
                                    }
                                }
                            },
                        }
                        " {st(lang, S::Vibration)}"
                    }
                }
            }
        }
    }
}

fn correct_label(choice: CorrectSound) -> S {
    match choice {
        CorrectSound::Ding1 => S::DingSoft,
        CorrectSound::Ding2 => S::DingBell,
        CorrectSound::Ding3 => S::DingSaron,
        CorrectSound::Kendang1 => S::KendangRound,
        CorrectSound::Kendang2 => S::KendangLow,
        CorrectSound::Kendang3 => S::KendangTight,
    }
}

fn end_label(choice: EndSound) -> S {
    match choice {
        EndSound::Gong1 => S::GongWarm,
        EndSound::Gong2 => S::GongDeep,
        EndSound::Gong3 => S::GongBright,
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
