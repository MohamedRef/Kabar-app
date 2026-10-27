//! Settings › About: version, what content this build carries, the source.

use dioxus::prelude::*;

use crate::content::Library;
use crate::i18n::{app, settings as tx};

#[component]
pub fn AboutSection() -> Element {
    let lib = Library::get();
    let written = lib.lectures.len();
    let total = lib.index.lectures.len();
    let items: usize = lib.lectures.values().map(|l| l.items.len()).sum();
    rsx! {
        section { class: "card about",
            p { class: "about-name", {app::NAME} }
            p { class: "muted", {app::TAGLINE} }
            p { {tx::VERSION} " " bdi { dir: "ltr", {env!("CARGO_PKG_VERSION")} } }
            p { class: "muted small", {tx::PERSONAL} }
        }
        section { class: "card",
            h2 { {tx::CONTENT} }
            p { "{written} {tx::LECTURES_WRITTEN} {total} · {items} {tx::ITEMS}" }
            p { class: "muted small", {tx::SOURCE} ": " bdi { {tx::BOOK} } }
        }
    }
}
