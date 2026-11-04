//! Interaction tests without a browser: render a component in a real
//! `VirtualDom`, find a button or field by its text, fire the event at it,
//! and read the HTML again. SSR alone never runs `onclick`/`oninput`, so
//! this is how a test checks what a tap actually changes.

use std::any::Any;
use std::rc::Rc;

use dioxus::core::{AttributeValue, DynamicNode, ElementId, NoOpMutations, TemplateAttribute, TemplateNode};
use dioxus::html::{
    PlatformEventData, SerializedFormData, SerializedHtmlEventConverter, SerializedMouseData,
};
use dioxus::prelude::*;

/// An element listening for an event.
#[derive(Clone, Debug)]
pub struct Target {
    /// `click`, `input`, `change`, `submit`…
    pub event: String,
    pub id: ElementId,
    pub tag: String,
    /// Its text, components included, with whitespace collapsed.
    pub text: String,
    /// Its attributes as written (`aria-label`, `class`, `data-*`…).
    pub attrs: Vec<(String, String)>,
}

impl Target {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

pub struct Harness {
    pub dom: VirtualDom,
}

impl Harness {
    pub fn new(app: fn() -> Element) -> Self {
        set_event_converter(Box::new(SerializedHtmlEventConverter));
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        Self { dom }
    }

    pub fn html(&self) -> String {
        dioxus::ssr::render(&self.dom)
    }

    /// Runs `f` inside the app's runtime, e.g. to read a signal.
    pub fn read<T>(&self, f: impl FnOnce() -> T) -> T {
        self.dom.in_scope(ScopeId::APP, f)
    }

    /// Every element listening for something, in document order.
    pub fn targets(&self) -> Vec<Target> {
        let mut out = Vec::new();
        walk(&self.dom, self.dom.base_scope().root_node(), &mut out);
        out
    }

    /// The one element listening for `event` that `pick` accepts; panics
    /// with the candidates when there are none or several.
    pub fn find(&self, event: &str, pick: impl Fn(&Target) -> bool) -> Target {
        let all = self.targets();
        let hits: Vec<&Target> = all.iter().filter(|t| t.event == event && pick(t)).collect();
        match hits.as_slice() {
            [one] => (*one).clone(),
            _ => panic!(
                "{} {event} targets match; listening: {:#?}",
                hits.len(),
                all.iter()
                    .filter(|t| t.event == event)
                    .map(|t| (&t.tag, &t.text, &t.attrs))
                    .collect::<Vec<_>>()
            ),
        }
    }

    /// Clicks the clickable element whose text is exactly `text`.
    pub fn click(&mut self, text: &str) {
        let t = self.find("click", |t| t.text == text || t.attr("aria-label") == Some(text));
        self.fire_click(&t);
    }

    /// Every element listening for `event` whose text is exactly `text`.
    pub fn all(&self, event: &str, text: &str) -> Vec<Target> {
        self.targets().into_iter().filter(|t| t.event == event && t.text == text).collect()
    }

    /// Sends `value` to the one field listening for `event` that `pick` accepts.
    pub fn type_into(&mut self, event: &str, pick: impl Fn(&Target) -> bool, value: &str) {
        let t = self.find(event, pick);
        self.form(&t, value);
    }

    pub fn fire_click(&mut self, t: &Target) {
        let data = PlatformEventData::new(Box::<SerializedMouseData>::default());
        self.fire(&t.event, t.id, data);
    }

    /// Sends `value` to a form event (`input`, `change`, `submit`). A
    /// checkbox's `change` carries "true" or "false".
    pub fn form(&mut self, t: &Target, value: &str) {
        let data = PlatformEventData::new(Box::new(SerializedFormData::new(value.to_string(), Vec::new())));
        self.fire(&t.event, t.id, data);
    }

    fn fire(&mut self, event: &str, id: ElementId, data: PlatformEventData) {
        let data = Event::new(Rc::new(data) as Rc<dyn Any>, true);
        self.dom.runtime().handle_event(event, data, id);
        self.settle();
    }

    /// Re-renders whatever the last event changed.
    pub fn settle(&mut self) {
        self.dom.process_events();
        self.dom.render_immediate(&mut NoOpMutations);
    }
}

fn walk(dom: &VirtualDom, vnode: &VNode, out: &mut Vec<Target>) {
    for root in vnode.template.roots {
        walk_node(dom, vnode, root, out);
    }
}

fn walk_node(dom: &VirtualDom, vnode: &VNode, node: &TemplateNode, out: &mut Vec<Target>) {
    match node {
        TemplateNode::Element { tag, attrs, children, .. } => {
            let mut values = Vec::new();
            let mut listeners = Vec::new();
            for attr in attrs.iter() {
                match attr {
                    TemplateAttribute::Static { name, value, .. } => {
                        values.push((name.to_string(), value.to_string()))
                    }
                    TemplateAttribute::Dynamic { id } => {
                        for a in vnode.dynamic_attrs[*id].iter() {
                            match &a.value {
                                AttributeValue::Listener(_) => {
                                    if let Some(eid) = vnode.mounted_dynamic_attribute(*id, dom) {
                                        listeners.push((a.name.trim_start_matches("on").to_string(), eid));
                                    }
                                }
                                AttributeValue::Text(v) => values.push((a.name.to_string(), v.clone())),
                                AttributeValue::Bool(b) => values.push((a.name.to_string(), b.to_string())),
                                AttributeValue::Int(i) => values.push((a.name.to_string(), i.to_string())),
                                AttributeValue::Float(f) => values.push((a.name.to_string(), f.to_string())),
                                _ => {}
                            }
                        }
                    }
                }
            }
            if !listeners.is_empty() {
                let mut text = String::new();
                for c in children.iter() {
                    text_node(dom, vnode, c, &mut text);
                }
                let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                let values: Vec<(String, String)> =
                    values.into_iter().map(|(n, v)| (n.replace('_', "-"), v)).collect();
                for (event, id) in listeners {
                    out.push(Target {
                        event,
                        id,
                        tag: tag.to_string(),
                        text: text.clone(),
                        attrs: values.clone(),
                    });
                }
            }
            for c in children.iter() {
                walk_node(dom, vnode, c, out);
            }
        }
        TemplateNode::Text { .. } => {}
        TemplateNode::Dynamic { id } => match &vnode.dynamic_nodes[*id] {
            DynamicNode::Component(c) => {
                if let Some(root) = c.mounted_scope(*id, vnode, dom).and_then(|s| s.try_root_node()) {
                    walk(dom, root, out);
                }
            }
            DynamicNode::Fragment(nodes) => {
                for n in nodes {
                    walk(dom, n, out);
                }
            }
            _ => {}
        },
    }
}

fn text_of(dom: &VirtualDom, vnode: &VNode, out: &mut String) {
    for root in vnode.template.roots {
        text_node(dom, vnode, root, out);
    }
}

fn text_node(dom: &VirtualDom, vnode: &VNode, node: &TemplateNode, out: &mut String) {
    match node {
        TemplateNode::Element { children, .. } => {
            for c in children.iter() {
                text_node(dom, vnode, c, out);
            }
        }
        TemplateNode::Text { text } => {
            out.push_str(text);
            out.push(' ');
        }
        TemplateNode::Dynamic { id } => match &vnode.dynamic_nodes[*id] {
            DynamicNode::Text(t) => {
                out.push_str(&t.value);
                out.push(' ');
            }
            DynamicNode::Component(c) => {
                if let Some(root) = c.mounted_scope(*id, vnode, dom).and_then(|s| s.try_root_node()) {
                    text_of(dom, root, out);
                }
            }
            DynamicNode::Fragment(nodes) => {
                for n in nodes {
                    text_of(dom, n, out);
                }
            }
            DynamicNode::Placeholder(_) => {}
        },
    }
}
