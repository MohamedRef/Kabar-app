//! The ink canvas every Studio surface shares (and the Reader's margin
//! notes): `assets/ink.js` draws, this component stores. Strokes are saved
//! the moment the pen lifts and synced like every other row.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use futures_channel::oneshot;
use kabar_protocol::ink::{InkStroke, InkTool};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::Db;
use crate::account::Cloud;

const INK_JS: &str = include_str!("../../assets/ink.js");

/// What the pen does: draw with an ink tool, or erase whole strokes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PenTool {
    Ink(InkTool),
    Eraser,
}

/// The tool, colour (a theme token: `ink`, `dawn`, `sawah`, `indigo`, `gold`)
/// and base width in page pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pen {
    pub tool: PenTool,
    pub color: &'static str,
    pub width: f32,
}

impl Default for Pen {
    fn default() -> Self {
        Self { tool: PenTool::Ink(InkTool::Pen), color: "ink", width: 2.2 }
    }
}

impl Pen {
    fn to_js(self) -> Value {
        let tool = match self.tool {
            PenTool::Ink(InkTool::Pen) => "pen",
            PenTool::Ink(InkTool::Highlighter) => "highlighter",
            PenTool::Ink(InkTool::Brush) => "brush",
            PenTool::Eraser => "eraser",
        };
        json!({ "tool": tool, "color": self.color, "width": self.width })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum FromInk {
    Stroke { stroke: InkStroke },
    Erase { ids: Vec<String> },
    View { rect: [f32; 4] },
    Png { data: Option<String> },
}

type PngWaiter = Rc<RefCell<Option<oneshot::Sender<Option<String>>>>>;

/// Talks to one canvas: scroll to a spot, clear it, take a snapshot. Make it
/// with [`use_ink`] and pass it to [`InkCanvas`].
#[derive(Clone)]
pub struct InkHandle {
    id: Rc<str>,
    png: PngWaiter,
    /// The visible page rect `[x, y, w, h]`, for placing new items in view.
    pub view: Signal<[f32; 4]>,
    /// Bumps after every stroke saved or erased.
    pub revision: Signal<u64>,
}

impl PartialEq for InkHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub fn use_ink() -> InkHandle {
    use_hook(|| InkHandle {
        id: format!("ink-{}", uuid::Uuid::new_v4().simple()).into(),
        png: Rc::new(RefCell::new(None)),
        view: Signal::new([0.0, 0.0, 800.0, 600.0]),
        revision: Signal::new(0),
    })
}

impl InkHandle {
    fn cmd(&self, cmd: Value) {
        let id = serde_json::to_string(&*self.id).unwrap_or_default();
        document::eval(&format!(
            r#"document.getElementById({id})?.dispatchEvent(new CustomEvent("kabar-ink", {{ detail: {cmd} }}))"#
        ));
    }

    /// Scrolls and zooms an infinite page to show `rect` (page pixels).
    pub fn focus(&self, rect: [f32; 4]) {
        self.cmd(json!({ "cmd": "focus", "rect": rect }));
    }

    /// Wipes the canvas on screen (the strokes are erased by the caller).
    pub fn clear(&self) {
        self.cmd(json!({ "cmd": "clear" }));
    }

    pub fn reload(&self, strokes: &[InkStroke]) {
        self.cmd(json!({ "cmd": "strokes", "strokes": strokes }));
    }

    /// The ink as a PNG data URL (dark ink on white, cropped), for the vision
    /// tasks; only the strokes in `ids` when given. None when it's empty.
    pub async fn snapshot(&self, ids: Option<Vec<String>>) -> Option<String> {
        let (tx, rx) = oneshot::channel();
        *self.png.borrow_mut() = Some(tx);
        self.cmd(json!({ "cmd": "snapshot", "ids": ids }));
        rx.await.ok().flatten()
    }
}

/// An ink canvas for document `doc`: an infinite page (pan with two fingers
/// or the mouse wheel, pinch or ctrl+wheel to zoom) or, with `size`, a fixed
/// sheet scaled to the box. `children` are placed on the page in page
/// pixels (word cards, mind-map nodes) and move with it. `underlay` sits
/// under the ink and never takes input (a tracing guide on a fixed sheet).
#[component]
pub fn InkCanvas(
    handle: InkHandle,
    doc: String,
    #[props(default)] size: Option<[f32; 2]>,
    /// Infinite pages: open scrolled to this page rect.
    #[props(default)]
    focus: Option<[f32; 4]>,
    pen: ReadSignal<Pen>,
    #[props(default)] class: Option<String>,
    #[props(default)] on_stroke: Option<EventHandler<InkStroke>>,
    #[props(default)] on_erase: Option<EventHandler<Vec<String>>>,
    #[props(default)] underlay: Option<Element>,
    #[props(default)] children: Element,
) -> Element {
    let db = use_context::<Db>();
    let cloud = try_use_context::<Cloud>();
    let id = handle.id.to_string();

    {
        let db = db.clone();
        let handle = handle.clone();
        let doc = doc.clone();
        let id = id.clone();
        use_future(move || {
            let db = db.clone();
            let handle = handle.clone();
            let doc = doc.clone();
            let id = id.clone();
            async move {
                let strokes = db.0.borrow().strokes(&doc).unwrap_or_default();
                let mut eval = document::eval(INK_JS);
                let init = json!({
                    "id": id,
                    "size": size,
                    "focus": focus,
                    "tool": pen.peek().to_js(),
                    "strokes": strokes,
                });
                if eval.send(init).is_err() {
                    return;
                }
                let (mut view, mut revision) = (handle.view, handle.revision);
                while let Ok(msg) = eval.recv::<FromInk>().await {
                    match msg {
                        FromInk::Stroke { mut stroke } => {
                            stroke.doc = doc.clone();
                            match db.0.borrow_mut().add_stroke(&stroke) {
                                Ok(_) => revision += 1,
                                Err(e) => eprintln!("kabar: saving a stroke failed: {e}"),
                            }
                            if let Some(h) = on_stroke {
                                h.call(stroke);
                            }
                        }
                        FromInk::Erase { ids } => {
                            for id in &ids {
                                if let Err(e) = db.0.borrow_mut().erase_stroke(id) {
                                    eprintln!("kabar: erasing failed: {e}");
                                }
                            }
                            revision += 1;
                            if let Some(h) = on_erase {
                                h.call(ids);
                            }
                        }
                        FromInk::View { rect } => view.set(rect),
                        FromInk::Png { data } => {
                            if let Some(tx) = handle.png.borrow_mut().take() {
                                let _ = tx.send(data);
                            }
                        }
                    }
                }
            }
        });
    }

    // Tool changes after the first paint.
    {
        let handle = handle.clone();
        use_effect(move || {
            let p = pen.read().to_js();
            let mut cmd = json!({ "cmd": "tool" });
            if let (Some(c), Some(p)) = (cmd.as_object_mut(), p.as_object()) {
                c.extend(p.clone());
            }
            handle.cmd(cmd);
        });
    }

    // Strokes from another device: redraw from the store.
    {
        let handle = handle.clone();
        let doc = doc.clone();
        let mut seen = use_signal(|| None::<u64>);
        use_effect(move || {
            let Some(rev) = cloud.map(|c| *c.revision.read()) else { return };
            if seen.peek().is_some_and(|s| s != rev) {
                handle.reload(&db.0.borrow().strokes(&doc).unwrap_or_default());
            }
            seen.set(Some(rev));
        });
    }

    let ratio = size.map(|[w, h]| format!("aspect-ratio: {w} / {h}; block-size: auto;")).unwrap_or_default();
    let class = format!("ink-wrap {}", class.unwrap_or_default());
    rsx! {
        div { id: "{id}", class: "{class}", style: "{ratio}",
            if let Some(u) = underlay {
                div { class: "ink-under", aria_hidden: "true", {u} }
            }
            div { id: "{id}-surface", class: "ink-surface" }
            div { id: "{id}-items", class: "ink-items", dir: "ltr", {children} }
        }
    }
}

/// The pen, highlighter, brush and eraser, plus ink colours.
#[component]
pub fn InkTools(pen: Signal<Pen>, labels: [&'static str; 4], #[props(default)] brush: bool) -> Element {
    let current = pen();
    let tools = [
        (PenTool::Ink(InkTool::Pen), labels[0], "M4 20l4-1 10-10-3-3L5 16z"),
        (PenTool::Ink(InkTool::Highlighter), labels[1], "M6 15l6-9 5 3-6 9H6zM5 21h14"),
        (PenTool::Ink(InkTool::Brush), labels[2], "M14 4l6 6-8 8c-2 2-6 2-8 2 0-2 0-6 2-8z"),
        (PenTool::Eraser, labels[3], "M8 20h12M5 15l8-8 6 6-5 5H9z"),
    ];
    rsx! {
        div { class: "ink-tools", role: "toolbar",
            for (tool, label, path) in tools {
                if brush || tool != PenTool::Ink(InkTool::Brush) {
                    button {
                        key: "{label}",
                        class: "icon-button",
                        title: "{label}",
                        aria_label: "{label}",
                        aria_pressed: if current.tool == tool { "true" } else { "false" },
                        onclick: move |_| {
                            let width = match tool {
                                PenTool::Ink(InkTool::Highlighter) => 3.0,
                                PenTool::Ink(InkTool::Brush) => 4.0,
                                _ => 2.2,
                            };
                            pen.set(Pen { tool, width, ..pen() });
                        },
                        svg { view_box: "0 0 24 24", width: "20", height: "20", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "{path}" }
                        }
                    }
                }
            }
            span { class: "ink-colors",
                for color in ["ink", "indigo", "dawn", "sawah", "gold"] {
                    button {
                        key: "{color}",
                        class: "ink-swatch",
                        style: "--swatch: var(--{color})",
                        aria_label: "{color}",
                        aria_pressed: if current.color == color && current.tool != PenTool::Eraser { "true" } else { "false" },
                        onclick: move |_| {
                            let mut p = pen();
                            if p.tool == PenTool::Eraser {
                                p.tool = PenTool::Ink(InkTool::Pen);
                            }
                            pen.set(Pen { color, ..p });
                        },
                    }
                }
            }
        }
    }
}
