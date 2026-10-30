//! Regression coverage of the production card contract.
use super::*;

#[test]
fn native_root_and_accessibility_attributes_are_preserved() {
    for (element, tag) in [
        (CardElement::Section, "section"),
        (CardElement::Div, "div"),
        (CardElement::Article, "article"),
        (CardElement::Aside, "aside"),
        (CardElement::Disclosure, "details"),
        (CardElement::List, "ul"),
        (CardElement::ListItem, "li"),
        (CardElement::Paragraph, "p"),
    ] {
        let html = dioxus::ssr::render_element(rsx! {
            Card { element, class: "reading", id: "item", role: "region",
                aria_label: "A < B", "data-kind": "practice", dir: "rtl", "" }
        });
        assert!(html.starts_with(&format!("<{tag} ")) && html.ends_with(&format!("</{tag}>")), "{html}");
        for value in
            ["card reading", "id=\"item\"", "role=\"region\"", "data-kind=\"practice\"", "dir=\"rtl\""]
        {
            assert!(html.contains(value), "missing {value}: {html}");
        }
        assert!(
            !html.contains("A < B") && (html.contains("A &#60; B") || html.contains("A &lt; B")),
            "{html}"
        );
    }
}

#[test]
fn disabled_buttons_keep_native_button_semantics() {
    fn app() -> Element {
        rsx! { CardButton { class: "tap", disabled: true, onclick: move |_| {}, aria_label: "Open", "Continue" } }
    }
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let html = dioxus::ssr::render(&dom);
    for value in ["<button ", "type=\"button\"", "disabled", "aria-label=\"Open\"", "card tap", "Continue"] {
        assert!(html.contains(value), "missing {value}: {html}");
    }
}

#[test]
fn dynamic_classes_and_status_attributes_are_forwarded() {
    let class = format!("reading-{}", "expanded");
    let html = dioxus::ssr::render_element(rsx! {
        Card { class, role: "status", aria_live: "polite", tabindex: "-1", "Saved" }
    });
    for value in
        ["card reading-expanded", "role=\"status\"", "aria-live=\"polite\"", "tabindex=\"-1\"", "Saved"]
    {
        assert!(html.contains(value), "missing {value}: {html}");
    }
}

#[test]
fn static_surfaces_stay_passive_and_actions_keep_their_listeners() {
    fn app() -> Element {
        rsx! {
            Card { id: "content", "Read" }
            DivCard { id: "compatible", "Original div" }
            Card { id: "action", onclick: move |_| {}, "Open" }
            Card { ondragover: move |_| {}, ondragleave: move |_| {}, ondrop: move |_| {}, "Drop here" }
            CardButton { onclick: move |_| {}, "Continue" }
        }
    }
    let mut dom = VirtualDom::new(app);
    let edits = dom.rebuild_to_vec().edits;
    let events: Vec<&str> = edits
        .iter()
        .filter_map(|edit| match edit {
            dioxus::core::Mutation::NewEventListener { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(events.iter().filter(|name| **name == "click").count(), 2);
    for name in ["dragover", "dragleave", "drop"] {
        assert!(events.contains(&name));
    }
    assert_eq!(events.len(), 5);
}

#[test]
fn keyed_card_reordering_keeps_mounted_children() {
    use std::cell::{Cell, RefCell};
    thread_local! {
        static ORDER: RefCell<Option<Signal<Vec<usize>>>> = const { RefCell::new(None) };
        static MOUNTS: Cell<usize> = const { Cell::new(0) };
    }
    #[component]
    fn Item(id: usize) -> Element {
        use_hook(|| MOUNTS.set(MOUNTS.get() + 1));
        rsx! { Card { id: "item-{id}", input { r#type: "text", value: "draft-{id}" } } }
    }
    fn app() -> Element {
        let order = use_signal(|| vec![1usize, 2]);
        ORDER.with(|slot| *slot.borrow_mut() = Some(order));
        rsx! { for id in order() { Item { key: "{id}", id } } }
    }
    MOUNTS.set(0);
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    assert_eq!(MOUNTS.get(), 2);
    dom.in_scope(ScopeId::APP, || {
        ORDER.with(|slot| slot.borrow_mut().as_mut().unwrap().set(vec![2, 1]));
    });
    let edits = dom.render_immediate_to_vec().edits;
    assert_eq!(MOUNTS.get(), 2, "reordering a keyed card must retain component state");
    assert!(
        !edits.iter().any(|edit| matches!(
            edit,
            dioxus::core::Mutation::Remove { .. } | dioxus::core::Mutation::LoadTemplate { .. }
        )),
        "reordering must move existing DOM rather than recreate input nodes"
    );
}

#[test]
fn explicit_form_button_type_is_not_duplicated_or_overridden() {
    fn app() -> Element {
        rsx! { CardButton { r#type: "submit", onclick: move |_| {}, "Save" } }
    }
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let html = dioxus::ssr::render(&dom);
    assert!(html.contains("type=\"submit\""), "{html}");
    assert_eq!(html.matches("type=").count(), 1, "{html}");
}
