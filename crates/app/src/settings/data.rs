//! Settings › Your data: where it lives. Export, restore and sync (chunk 21)
//! and the Anki export (chunk 9) add their rows here.

use dioxus::prelude::*;

use crate::i18n::settings as tx;

#[component]
pub fn DataSection() -> Element {
    let folder = crate::prefs::data_dir().map(|d| d.display().to_string());
    rsx! {
        section { class: "card",
            h2 { {tx::DATA_FOLDER} }
            match folder {
                Some(f) => rsx! { bdi { class: "path", dir: "ltr", "{f}" } },
                None => rsx! { p { class: "muted", {tx::DATA_NOWHERE} } },
            }
            p { class: "muted small", {tx::DATA_LOCAL} }
        }
    }
}
