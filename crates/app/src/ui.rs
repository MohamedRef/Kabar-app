//! Shared UI primitives. Features compose these and own their state;
//! surface, spacing and shape come from the shared CSS tokens.

use dioxus::prelude::*;

/// Native root semantics of a shared card, including lists and reading items.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum CardElement {
    #[default]
    Section,
    Div,
    Article,
    Aside,
    Disclosure,
    List,
    ListItem,
    Paragraph,
}

/// One surface contract for all base cards. Feature classes describe layout;
/// shape comes from shared CSS tokens. Owned classes support dynamic choices.
/// Native global attributes, ARIA and explicitly typed events are forwarded.
#[component]
pub fn Card(
    #[props(default)] class: String,
    #[props(default)] element: CardElement,
    #[props(default)] onclick: Option<EventHandler<MouseEvent>>,
    #[props(default)] ondragover: Option<EventHandler<DragEvent>>,
    #[props(default)] ondragleave: Option<EventHandler<DragEvent>>,
    #[props(default)] ondrop: Option<EventHandler<DragEvent>>,
    #[props(extends = GlobalAttributes)] mut attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    if let Some(handler) = onclick {
        attributes.push(dioxus::events::onclick(move |event: MouseEvent| handler.call(event)));
    }
    if let Some(handler) = ondragover {
        attributes.push(dioxus::events::ondragover(move |event: DragEvent| handler.call(event)));
    }
    if let Some(handler) = ondragleave {
        attributes.push(dioxus::events::ondragleave(move |event: DragEvent| handler.call(event)));
    }
    if let Some(handler) = ondrop {
        attributes.push(dioxus::events::ondrop(move |event: DragEvent| handler.call(event)));
    }
    match element {
        CardElement::Section => rsx! { section { class: "card {class}", ..attributes, {children} } },
        CardElement::Div => rsx! { div { class: "card {class}", ..attributes, {children} } },
        CardElement::Article => rsx! { article { class: "card {class}", ..attributes, {children} } },
        CardElement::Aside => rsx! { aside { class: "card {class}", ..attributes, {children} } },
        CardElement::Disclosure => rsx! { details { class: "card {class}", ..attributes, {children} } },
        CardElement::List => rsx! { ul { class: "card {class}", ..attributes, {children} } },
        CardElement::ListItem => rsx! { li { class: "card {class}", ..attributes, {children} } },
        CardElement::Paragraph => rsx! { p { class: "card {class}", ..attributes, {children} } },
    }
}

/// Compatibility entry point for existing div surfaces; delegates to the same
/// contract as Card rather than maintaining another copy of card markup.
#[component]
pub fn DivCard(
    #[props(default)] class: String,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    rsx! { Card { element: CardElement::Div, class, attributes, children } }
}

/// A whole-card action stays a native button with keyboard, focus, disabled
/// and form semantics. Other interactive controls belong outside this button.
#[component]
pub fn CardButton(
    #[props(default)] class: String,
    #[props(default = "button".to_owned())] r#type: String,
    onclick: EventHandler<MouseEvent>,
    #[props(extends = button, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    rsx! { button { r#type, class: "card {class}", onclick: move |event| onclick.call(event), ..attributes, {children} } }
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

#[cfg(test)]
mod tests;
