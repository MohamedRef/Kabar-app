//! Today: the first screen. For now it lists the chapter's lectures and how
//! many items each has; chunk 3 adds due reviews and "start a session".

use dioxus::prelude::*;

use crate::content::Library;
use crate::i18n::today as tx;

#[component]
pub fn Today() -> Element {
    let lib = Library::get();
    let rows = lib.index.lectures.iter().map(|entry| {
        let items = lib.lectures.get(&entry.id).map(|l| l.items.len());
        let number = entry.id.rsplit('.').next().unwrap_or(&entry.id).trim_start_matches('L').to_string();
        let [first, last] = entry.pages;
        (entry.id.clone(), number, entry.title.clone(), items, format!("{first}–{last}"))
    });
    rsx! {
        section { class: "card",
            h1 { {tx::LIBRARY} }
            p { class: "muted", bdi { {tx::CHAPTER} } }
            p { class: "note", {tx::UNVERIFIED_NOTE} }
        }
        ol { class: "lectures",
            for (id, number, title, items, pages) in rows {
                li { key: "{id}", class: "lecture", "data-ready": if items.is_some() { "yes" } else { "no" },
                    span { class: "num", "{number}" }
                    div { class: "lecture-main",
                        bdi { class: "lecture-title", "{title}" }
                        span { class: "muted small", "{tx::PAGES} " bdi { dir: "ltr", "{pages}" } }
                    }
                    match items {
                        Some(n) => rsx! { span { class: "chip ready", "{tx::READY} · {n} {tx::ITEMS}" } },
                        None => rsx! { span { class: "chip", {tx::WRITING} } },
                    }
                }
            }
        }
    }
}
