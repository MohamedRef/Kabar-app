//! Settings › Look: colours and text size, per device.

use batna_core::theme::Theme;
use dioxus::prelude::*;

use super::DeviceOnly;
use crate::Ui;
use crate::i18n::settings as tx;
use crate::prefs::TextSize;

fn theme_label(t: Theme) -> (&'static str, &'static str) {
    match t {
        Theme::System => (tx::THEME_SYSTEM, tx::THEME_SYSTEM_SUB),
        Theme::Paper => (tx::THEME_PAPER, tx::THEME_PAPER_SUB),
        Theme::Night => (tx::THEME_NIGHT, tx::THEME_NIGHT_SUB),
        Theme::NightShift => (tx::THEME_SHIFT, tx::THEME_SHIFT_SUB),
    }
}

fn size_label(s: TextSize) -> &'static str {
    match s {
        TextSize::Small => tx::TEXT_SMALL,
        TextSize::Normal => tx::TEXT_NORMAL,
        TextSize::Large => tx::TEXT_LARGE,
        TextSize::Larger => tx::TEXT_LARGER,
    }
}

/// Two halves of the swatch: the page and the accent (System: Paper and Night).
fn swatch_style(t: Theme) -> String {
    use batna_core::theme::{NIGHT, PAPER};
    let (a, b) = match t.palette() {
        Some(p) => (p.bg, p.accent),
        None => (PAPER.bg, NIGHT.bg),
    };
    format!("--sw-a:{a};--sw-b:{b};")
}

#[component]
pub fn LookSection() -> Element {
    let ui = use_context::<Ui>();
    let mut prefs = ui.prefs;
    let now = *prefs.read();
    rsx! {
        section { class: "card",
            h2 { {tx::THEME} DeviceOnly {} }
            div { class: "choice-list", role: "radiogroup", aria_label: tx::THEME,
                for t in Theme::ALL {
                    button {
                        key: "{t.as_str()}",
                        class: "choice",
                        role: "radio",
                        aria_checked: if now.theme == t { "true" } else { "false" },
                        onclick: move |_| {
                            prefs.write().theme = t;
                            prefs.peek().save();
                        },
                        span { class: "swatch", aria_hidden: "true", style: swatch_style(t) }
                        span { class: "choice-text",
                            span { class: "choice-title", {theme_label(t).0} }
                            span { class: "muted small", {theme_label(t).1} }
                        }
                    }
                }
            }
        }
        section { class: "card",
            h2 { {tx::TEXT_SIZE} DeviceOnly {} }
            div { class: "segmented", role: "radiogroup", aria_label: tx::TEXT_SIZE,
                for s in TextSize::ALL {
                    button {
                        key: "{s.as_str()}",
                        role: "radio",
                        aria_checked: if now.text_size == s { "true" } else { "false" },
                        onclick: move |_| {
                            prefs.write().text_size = s;
                            prefs.peek().save();
                        },
                        {size_label(s)}
                    }
                }
            }
            p { class: "text-sample", {tx::TEXT_SAMPLE_AR} " " bdi { {tx::TEXT_SAMPLE_TERM} } "؟" }
        }
    }
}
