//! How an open text reads on this device (issue #180): text size, line
//! spacing and line width, kept per device like Settings › Text size, and a
//! focus mode on tablet and Mac that sets the sidebar and the Reader's bar
//! aside. On a phone the bar shrinks to one line so the text starts at the
//! top; the voice picker moves into the settings sheet.

use dioxus::prelude::*;
use kabar_core::layout::WidthClass;
use kabar_core::locale::UiLanguage;

use super::ReaderState;
use crate::Db;
use crate::Ui;
use crate::i18n::Msg;
use crate::settings::device::{DevicePrefs, ReadSize, ReadSpacing, ReadWidth, Reading};

/// The `data-*` attributes on `.reader` that main.css keys the text on.
pub struct Attrs {
    pub size: &'static str,
    pub spacing: &'static str,
    pub width: &'static str,
    /// Focus mode is on and makes sense here: a text is open on a window
    /// wider than a phone.
    pub focus: bool,
}

pub fn attrs(r: Reading, open: bool, width: WidthClass) -> Attrs {
    Attrs {
        size: r.size.as_str(),
        spacing: r.spacing.as_str(),
        width: r.width.as_str(),
        focus: r.focus && open && width != WidthClass::Compact,
    }
}

/// The Reader's bar while a text is open: one line with the reading
/// settings, focus mode (tablet and Mac) and a new text. `sheet` is whether
/// the settings are showing under it.
#[component]
pub fn OpenBar(sheet: Signal<bool>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let prefs = use_context::<DevicePrefs>();
    let st = use_context::<ReaderState>();
    let r = *prefs.reading.read();
    let roomy = *ui.width.read() != WidthClass::Compact;
    let shown = *sheet.read();

    rsx! {
        div { class: "card reader-bar one-line",
            h2 { "{ui.t(Msg::NavReader)}" }
            button {
                class: "read-aa",
                aria_expanded: if shown { "true" } else { "false" },
                aria_controls: "read-settings",
                aria_label: "{ui.t(Msg::ReadingSettings)}",
                title: "{ui.t(Msg::ReadingSettings)}",
                onclick: move |_| sheet.set(!shown),
                span { class: "latin", dir: "ltr", "Aa" }
            }
            if roomy {
                button {
                    aria_pressed: if r.focus { "true" } else { "false" },
                    title: if r.focus { ui.t(Msg::ReadFocusLeave) } else { ui.t(Msg::ReadFocus) },
                    onclick: {
                        let db = db.clone();
                        move |_| prefs.set_reading(&db, Reading { focus: !r.focus, ..r })
                    },
                    if r.focus { "{ui.t(Msg::ReadFocusLeave)}" } else { "{ui.t(Msg::ReadFocus)}" }
                }
            }
            button { onclick: move |_| st.close(), "{ui.t(Msg::ReaderNew)}" }
        }
        if shown {
            Sheet {}
        }
    }
}

/// The settings under the bar: voice, size, spacing and, where the window
/// has room for a choice, width.
#[component]
fn Sheet() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let prefs = use_context::<DevicePrefs>();
    let r = *prefs.reading.read();
    let roomy = *ui.width.read() != WidthClass::Compact;
    let set = move |next: Reading| prefs.set_reading(&db, next);

    rsx! {
        div { id: "read-settings", class: "card read-settings", role: "group",
            aria_label: "{ui.t(Msg::ReadingSettings)}",
            VoicePicker {}
            div { class: "field",
                span { class: "label", "{ui.t(Msg::ReadSize)}" }
                div { class: "row", role: "group", aria_label: "{ui.t(Msg::ReadSize)}",
                    for &size in ReadSize::ALL {
                        button {
                            key: "{size.as_str()}",
                            class: "read-size-{size.as_str()}",
                            aria_pressed: if size == r.size { "true" } else { "false" },
                            aria_label: "{ui.t(size_label(size))}",
                            title: "{ui.t(size_label(size))}",
                            onclick: {
                                let set = set.clone();
                                move |_| set(Reading { size, ..r })
                            },
                            span { class: "latin", dir: "ltr", "A" }
                        }
                    }
                }
            }
            div { class: "field",
                span { class: "label", "{ui.t(Msg::ReadSpacing)}" }
                div { class: "row", role: "group", aria_label: "{ui.t(Msg::ReadSpacing)}",
                    for &spacing in ReadSpacing::ALL {
                        button {
                            key: "{spacing.as_str()}",
                            aria_pressed: if spacing == r.spacing { "true" } else { "false" },
                            onclick: {
                                let set = set.clone();
                                move |_| set(Reading { spacing, ..r })
                            },
                            "{ui.t(spacing_label(spacing))}"
                        }
                    }
                }
            }
            if roomy {
                div { class: "field",
                    span { class: "label", "{ui.t(Msg::ReadWidth)}" }
                    div { class: "row", role: "group", aria_label: "{ui.t(Msg::ReadWidth)}",
                        for &width in ReadWidth::ALL {
                            button {
                                key: "{width.as_str()}",
                                aria_pressed: if width == r.width { "true" } else { "false" },
                                onclick: {
                                    let set = set.clone();
                                    move |_| set(Reading { width, ..r })
                                },
                                "{ui.t(width_label(width))}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Which language the explanations under the text speak.
#[component]
pub fn VoicePicker() -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let voice = *st.voice.read();
    rsx! {
        div { class: "row",
            span { class: "muted", "{ui.t(Msg::Voice)}" }
            div { class: "row", role: "group", aria_label: "{ui.t(Msg::Voice)}",
                for v in UiLanguage::ALL {
                    button {
                        key: "{v.tag()}",
                        lang: v.tag(),
                        aria_pressed: if voice == v { "true" } else { "false" },
                        onclick: move |_| {
                            let mut voice = st.voice;
                            voice.set(v);
                        },
                        "{v.native_name()}"
                    }
                }
            }
        }
    }
}

fn size_label(s: ReadSize) -> Msg {
    match s {
        ReadSize::Smaller => Msg::ReadSmaller,
        ReadSize::Normal => Msg::ReadNormal,
        ReadSize::Larger => Msg::ReadLarger,
        ReadSize::Largest => Msg::ReadLargest,
    }
}

fn spacing_label(s: ReadSpacing) -> Msg {
    match s {
        ReadSpacing::Snug => Msg::ReadSnug,
        ReadSpacing::Normal => Msg::ReadNormal,
        ReadSpacing::Airy => Msg::ReadAiry,
    }
}

fn width_label(w: ReadWidth) -> Msg {
    match w {
        ReadWidth::Narrow => Msg::ReadNarrow,
        ReadWidth::Book => Msg::ReadBook,
        ReadWidth::Wide => Msg::ReadWide,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_needs_an_open_text_and_room() {
        let on = Reading { focus: true, ..Reading::default() };
        assert!(attrs(on, true, WidthClass::Expanded).focus);
        assert!(attrs(on, true, WidthClass::Medium).focus);
        assert!(!attrs(on, false, WidthClass::Large).focus, "nothing to read yet");
        assert!(!attrs(on, true, WidthClass::Compact).focus, "phones have no sidebar");
        assert!(!attrs(Reading::default(), true, WidthClass::Large).focus);
    }

    #[test]
    fn defaults_are_the_book_layout() {
        let a = attrs(Reading::default(), true, WidthClass::Large);
        assert_eq!((a.size, a.spacing, a.width), ("m", "normal", "book"));
    }
}
