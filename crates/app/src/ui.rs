//! Shared UI primitives. Features compose these and own their state;
//! surface, spacing and shape come from the shared CSS tokens.

use dioxus::prelude::*;

/// A content surface. Keep actions on real buttons inside it; a Card is
/// never itself clickable. `class` adds feature layout, not another skin.
#[component]
pub fn Card(#[props(default)] class: &'static str, children: Element) -> Element {
    rsx! { section { class: "card {class}", {children} } }
}

/// A navigation row: one button owns the whole hit area and accessible
/// name. Settings and other grouped lists share its markup and CSS.
#[component]
pub fn ActionRow(
    title: String,
    detail: String,
    icon: Element,
    #[props(default)] selected: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "activity-row",
            aria_current: if selected { "page" } else { "false" },
            onclick: move |event| onclick.call(event),
            span { class: "row-icon", aria_hidden: "true", {icon} }
            span { class: "row-copy",
                strong { bdi { class: "row-title", "{title}" } }
                if !detail.is_empty() { small { "{detail}" } }
            }
            span { class: "row-end", span { class: "chev", aria_hidden: "true" } }
        }
    }
}
