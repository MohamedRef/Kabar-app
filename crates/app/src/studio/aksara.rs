//! Aksara Jawa calligraphy (M13, spec §4, PLAN §18): pick a letter, sign or
//! word, watch its stroke order drawn, trace it over a faint guide with the
//! pressure brush, then write it freehand. Every attempt gets the offline
//! shape check from `kabar_core::aksara`; "Ask the teacher" sends the sheet
//! to the `check_strokes` vision task when Kabar Cloud is there.

use std::rc::Rc;

use base64::Engine as _;
use dioxus::prelude::*;
use kabar_core::ai::FriendlyReason;
use kabar_core::aksara::{
    AksaraSet, Glyph, GlyphKind, Issue, P, Part, ShapeReport, StrokeCheck, check, request,
};
use kabar_core::locale::UiLanguage;
use kabar_protocol::coach::{ActivityKind, Skill};
use kabar_protocol::content::Lang;
use kabar_protocol::ink::{COORD_SCALE, DocKind, InkDocument, InkStroke, InkTool};

use super::ink::{InkCanvas, InkTools, Pen, PenTool, use_ink};
use super::{T, friendly, new_id, tool_labels, tr, vision};
use crate::cloud::link::CloudLink;
use crate::coach::Coach;
use crate::store::now_ms;
use crate::teach::Ai;
use crate::{Db, Ui};

const STROKES_JSON: &str = include_str!("../../../../content/aksara/strokes.json");
const FONT: &[u8] = include_bytes!("../../../../content/fonts/NotoSansJavanese-Regular.ttf");

/// Page pixels per 1/1000 em on the sheet.
const PX_PER_UNIT: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Watch,
    Trace,
    Write,
}

#[derive(Clone, Debug, PartialEq)]
enum Outcome {
    Shape(ShapeReport),
    Teacher(StrokeCheck),
}

/// The part of the em square a glyph's sheet shows, `[x, y, w, h]` in
/// 1/1000 em (y down, baseline 0): the strokes plus a margin, never much
/// narrower than tall so single letters get room around them.
fn frame(g: &Glyph) -> [f32; 4] {
    let [x0, y0, x1, y1] = g.bbox();
    let pad = g.width as f32 / 2.0 + 140.0;
    let (mut x0, y0, mut x1, y1) = (x0 as f32 - pad, y0 as f32 - pad, x1 as f32 + pad, y1 as f32 + pad);
    let min_w = (y1 - y0) * 0.8;
    if x1 - x0 < min_w {
        let grow = (min_w - (x1 - x0)) / 2.0;
        x0 -= grow;
        x1 += grow;
    }
    [x0, y0, x1 - x0, y1 - y0]
}

fn sheet_size(g: &Glyph) -> [f32; 2] {
    let [_, _, w, h] = frame(g);
    [w * PX_PER_UNIT, h * PX_PER_UNIT]
}

fn path_d(stroke: &[[i32; 2]]) -> String {
    let mut d = String::new();
    for (i, [x, y]) in stroke.iter().enumerate() {
        d.push_str(if i == 0 { "M" } else { " L" });
        d.push_str(&format!("{x} {y}"));
    }
    d
}

/// The ink as points for the shape check (page units; the check normalizes).
fn ink_points(strokes: &[InkStroke]) -> Vec<Vec<P>> {
    let s = COORD_SCALE;
    strokes
        .iter()
        .map(|st| st.xs.iter().zip(&st.ys).map(|(&x, &y)| (x as f32 / s, y as f32 / s)).collect())
        .collect()
}

fn kind_label(k: GlyphKind) -> T {
    match k {
        GlyphKind::Nglegena => T::Letters,
        GlyphKind::Sandhangan => T::Signs,
        GlyphKind::Panyigeg => T::Finals,
        GlyphKind::Word => T::Words,
    }
}

/// A piece of an issue line: UI text, or aksara/Latin to isolate in `<bdi>`.
enum Piece {
    Text(String),
    Snippet(String),
}

fn part_name(l: UiLanguage, p: Part) -> &'static str {
    match (l, p) {
        (UiLanguage::ArEg, Part::Left) => "الجزء الشمال",
        (UiLanguage::ArEg, Part::Middle) => "النص",
        (UiLanguage::ArEg, Part::Right) => "الجزء اليمين",
        (UiLanguage::ArEg, Part::Above) => "العلامة اللي فوق",
        (UiLanguage::ArEg, Part::Below) => "الجزء اللي تحت السطر",
        (UiLanguage::Ar, Part::Left) => "الجزء الأيسر",
        (UiLanguage::Ar, Part::Middle) => "الوسط",
        (UiLanguage::Ar, Part::Right) => "الجزء الأيمن",
        (UiLanguage::Ar, Part::Above) => "العلامة العلوية",
        (UiLanguage::Ar, Part::Below) => "الجزء الذي تحت السطر",
        (UiLanguage::En, _) => "",
    }
}

fn issue_pieces(l: UiLanguage, issue: &Issue, g: &Glyph) -> Vec<Piece> {
    use Piece::*;
    if l == UiLanguage::En {
        return vec![Text(issue.note(g))];
    }
    let eg = l == UiLanguage::ArEg;
    match issue {
        Issue::Missing { stroke, part } => {
            let n = stroke + 1;
            let part = part_name(l, *part);
            vec![Text(format!("الخط رقم {n} ({part}) ناقص."))]
        }
        Issue::Extra { part } => {
            let part = part_name(l, *part);
            vec![Text(if eg {
                format!("فيه حبر زيادة عند {part} مش موجود في الحرف.")
            } else {
                format!("يوجد حبر زائد عند {part} ليس في الحرف.")
            })]
        }
        Issue::Mirrored => vec![Text(
            if eg {
                "الحرف مقلوب يمين في شمال، الفتحة المفروض تبص للناحية التانية."
            } else {
                "الحرف معكوس أفقيًا، والفتحة يجب أن تتجه إلى الجهة الأخرى."
            }
            .into(),
        )],
        Issue::ReadsAs { text, latin, .. } => vec![
            Text(if eg { "ده يتقري " } else { "هذا يُقرأ " }.into()),
            Snippet(format!("{latin} {text}")),
            Text(if eg { " مش " } else { " وليس " }.into()),
            Snippet(format!("{} {}", g.latin, g.text)),
            Text(".".into()),
        ],
        Issue::Empty => vec![Text(tr(l, T::NothingYet).into())],
    }
}

/// The faint letter, baseline and (when tracing) the numbered strokes, drawn
/// in em units under the ink.
#[component]
fn Guide(glyph: Glyph, mode: Mode) -> Element {
    let [x, y, w, h] = frame(&glyph);
    let numbers = mode == Mode::Trace;
    rsx! {
        svg { view_box: "{x} {y} {w} {h}", preserve_aspect_ratio: "none",
            line { class: "guide-line", x1: "{x}", y1: "0", x2: "{x + w}", y2: "0" }
            if mode != Mode::Write {
                text { class: "aksara-ghost", x: "0", y: "0", font_size: "1000", "{glyph.text}" }
            }
            if numbers {
                for (i, s) in glyph.strokes.iter().enumerate() {
                    path { key: "p{i}", class: "guide-path", d: path_d(s) }
                }
                for (i, s) in glyph.strokes.iter().enumerate() {
                    g { key: "n{i}",
                        circle { class: "start-dot", cx: "{s[0][0]}", cy: "{s[0][1]}", r: "26" }
                        text { class: "start-num", x: "{s[0][0] + 34}", y: "{s[0][1] - 30}", font_size: "90", "{i + 1}" }
                    }
                }
            }
        }
    }
}

/// The stroke order, drawn one stroke after another at a steady pace.
#[component]
fn StrokeOrder(glyph: Glyph) -> Element {
    let [x, y, w, h] = frame(&glyph);
    let mut delay = 0.3f32;
    let strokes: Vec<(String, f32, f32, [i32; 2])> = glyph
        .strokes
        .iter()
        .zip(glyph.stroke_lengths())
        .map(|(s, len)| {
            let secs = 0.35 + len / 1400.0;
            let item = (path_d(s), delay, secs, s[0]);
            delay += secs + 0.25;
            item
        })
        .collect();
    rsx! {
        svg { class: "aksara-watch", view_box: "{x} {y} {w} {h}", role: "img", "aria-label": "{glyph.latin}",
            line { class: "guide-line", x1: "{x}", y1: "0", x2: "{x + w}", y2: "0" }
            text { class: "aksara-ghost", x: "0", y: "0", font_size: "1000", "{glyph.text}" }
            for (i, (d, start, secs, p)) in strokes.into_iter().enumerate() {
                g { key: "{i}",
                    path {
                        class: "anim-stroke",
                        d: "{d}",
                        "pathLength": "1",
                        stroke_width: "{glyph.width}",
                        style: "animation-delay: {start}s; animation-duration: {secs}s",
                    }
                    text {
                        class: "start-num anim-num",
                        x: "{p[0] + 34}",
                        y: "{p[1] - 30}",
                        font_size: "90",
                        style: "animation-delay: {start}s",
                        "{i + 1}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn AksaraScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let ai = use_context::<Ai>();
    let link = use_context::<CloudLink>();
    let coach = try_use_context::<Coach>();
    let l = ui.lang();
    let set =
        use_hook(|| Rc::new(AksaraSet::parse(STROKES_JSON).expect("content/aksara/strokes.json is valid")));
    let font_css = use_hook(|| {
        let b64 = base64::engine::general_purpose::STANDARD.encode(FONT);
        format!(
            "@font-face {{ font-family: \"Kabar Javanese\"; src: url(data:font/ttf;base64,{b64}) format(\"truetype\"); }}"
        )
    });
    let handle = use_ink();
    let pen = use_signal(|| Pen { tool: PenTool::Ink(InkTool::Brush), color: "ink", width: 3.0 });
    let mut current = use_signal(|| "ha".to_string());
    let mut mode = use_signal(|| Mode::Watch);
    let mut sheet = use_signal(|| None::<String>);
    let mut started = use_signal(now_ms);
    let mut outcome = use_signal(|| None::<Outcome>);
    let mut problem = use_signal(|| None::<T>);
    let mut busy = use_signal(|| false);
    let mut replay = use_signal(|| 0u32);

    let glyph = {
        let set = set.clone();
        move || set.get(&current()).or(set.glyphs.first()).cloned().expect("the aksara set is not empty")
    };
    let g = glyph();

    // A fresh sheet for each trace or write attempt.
    let open = {
        let db = db.clone();
        let set = set.clone();
        move |id: String, m: Mode| {
            outcome.set(None);
            problem.set(None);
            current.set(id.clone());
            mode.set(m);
            if m == Mode::Watch {
                sheet.set(None);
                replay += 1;
                return;
            }
            let Some(g) = set.get(&id) else { return };
            let doc = InkDocument {
                id: new_id(),
                kind: DocKind::Aksara,
                notebook: None,
                title: g.id.clone(),
                lang: Lang::Jv,
                order: 0,
                anchor: None,
                size: Some(sheet_size(g)),
                created_ms: now_ms(),
            };
            match db.0.borrow_mut().save_ink_document(&doc) {
                Ok(()) => {
                    sheet.set(Some(doc.id));
                    started.set(now_ms());
                }
                Err(e) => crate::diag::report!("saving the aksara sheet failed: {e}"),
            }
        }
    };

    let clear = {
        let db = db.clone();
        let handle = handle.clone();
        move |_| {
            let Some(doc) = sheet() else { return };
            let strokes = db.0.borrow().strokes(&doc).unwrap_or_default();
            for s in strokes {
                let _ = db.0.borrow_mut().erase_stroke(&s.id);
            }
            handle.clear();
            outcome.set(None);
            problem.set(None);
        }
    };

    let check_shape = {
        let db = db.clone();
        let set = set.clone();
        let glyph = glyph.clone();
        move |_| {
            let Some(doc) = sheet() else { return };
            let g = glyph();
            let strokes = db.0.borrow().strokes(&doc).unwrap_or_default();
            let report = check(&g, &ink_points(&strokes), &set);
            if let Some(c) = coach
                && !strokes.is_empty()
            {
                let kind = ActivityKind::Practice {
                    skill: Skill::Writing,
                    topic: format!("aksara:{}", g.id),
                    correct: u32::from(report.correct),
                    total: 1,
                };
                let seconds = (now_ms().saturating_sub(started()) / 1000).clamp(5, 600) as u32;
                c.record(&db, kind, seconds);
            }
            problem.set(None);
            outcome.set(Some(Outcome::Shape(report)));
        }
    };

    let ask_teacher = {
        let handle = handle.clone();
        let glyph = glyph.clone();
        move |_| {
            let handle = handle.clone();
            let ai = ai.clone();
            let link = link.clone();
            let g = glyph();
            busy.set(true);
            problem.set(None);
            spawn(async move {
                let result = async {
                    let png = handle.snapshot(None).await.ok_or(FriendlyReason::TryAgain)?;
                    let sha = vision::upload(&link, &png).await?;
                    ai.0.run::<StrokeCheck>(&request(&g, &sha)).await.map_err(|e| e.friendly())
                }
                .await;
                busy.set(false);
                match result {
                    Ok(answer) => outcome.set(Some(Outcome::Teacher(answer.value))),
                    Err(f) => problem.set(Some(friendly(f))),
                }
            });
        }
    };

    let next = {
        let set = set.clone();
        move || {
            let i = set.glyphs.iter().position(|x| x.id == current()).unwrap_or(0);
            set.glyphs[(i + 1) % set.glyphs.len()].id.clone()
        }
    };

    let groups = [GlyphKind::Nglegena, GlyphKind::Sandhangan, GlyphKind::Panyigeg, GlyphKind::Word];
    let [_, _, fw, fh] = frame(&g);
    let stage_style = format!("max-inline-size: min(100%, calc(52dvh * {fw} / {fh}));");
    let done = match outcome() {
        Some(Outcome::Shape(r)) => r.correct,
        Some(Outcome::Teacher(c)) => c.correct,
        None => false,
    };

    rsx! {
        style { {font_css} }
        div { class: "aksara",
            p { class: "muted", "{tr(l, T::AksaraIntro)}" }
            details { class: "aksara-picker", open: true,
                summary {
                    bdi { class: "aksara-text", "{g.text}" }
                    " "
                    bdi { class: "latin", "{g.latin}" }
                }
                for k in groups {
                    div { key: "{k:?}", class: "aksara-group",
                        span { class: "muted small", "{tr(l, kind_label(k))}" }
                        for item in set.glyphs.iter().filter(|x| x.kind == k) {
                            button {
                                key: "{item.id}",
                                class: "aksara-pick",
                                aria_pressed: if item.id == g.id { "true" } else { "false" },
                                onclick: {
                                    let id = item.id.clone();
                                    let mut open = open.clone();
                                    move |_| open(id.clone(), Mode::Watch)
                                },
                                bdi { class: "aksara-text", "{item.text}" }
                                bdi { class: "latin small", "{item.latin}" }
                            }
                        }
                    }
                }
            }
            div { class: "row aksara-modes", role: "tablist",
                for (m, label) in [(Mode::Watch, T::Watch), (Mode::Trace, T::Trace), (Mode::Write, T::WriteFree)] {
                    button {
                        key: "{label:?}",
                        role: "tab",
                        aria_selected: if mode() == m { "true" } else { "false" },
                        aria_pressed: if mode() == m { "true" } else { "false" },
                        onclick: {
                            let mut open = open.clone();
                            let id = g.id.clone();
                            move |_| open(id.clone(), m)
                        },
                        "{tr(l, label)}"
                    }
                }
                if !g.verified {
                    span { class: "badge", "{tr(l, T::OrderUnverified)}" }
                }
            }
            div { class: "aksara-stage", style: "{stage_style}",
                match (mode(), sheet()) {
                    (Mode::Watch, _) | (_, None) => rsx! {
                        StrokeOrder { key: "{g.id}-{replay}", glyph: g.clone() }
                        div { class: "row",
                            button { onclick: move |_| replay += 1, "{tr(l, T::Replay)}" }
                            button {
                                class: "primary",
                                onclick: {
                                    let mut open = open.clone();
                                    let id = g.id.clone();
                                    move |_| open(id.clone(), Mode::Trace)
                                },
                                "{tr(l, T::Trace)}"
                            }
                        }
                    },
                    (m, Some(doc)) => rsx! {
                        InkCanvas {
                            key: "{doc}",
                            handle: handle.clone(),
                            doc: doc.clone(),
                            size: Some(sheet_size(&g)),
                            pen,
                            class: "ink-sheet aksara-sheet",
                            underlay: rsx! { Guide { glyph: g.clone(), mode: m } },
                        }
                        InkTools { pen, labels: tool_labels(l), brush: true }
                        div { class: "row",
                            button { onclick: clear.clone(), "{tr(l, T::Clear)}" }
                            button { class: "primary", onclick: check_shape.clone(), "{tr(l, T::CheckShape)}" }
                            button { disabled: busy(), onclick: ask_teacher.clone(),
                                if busy() { "{tr(l, T::Reading)}" } else { "{tr(l, T::AskTeacher)}" }
                            }
                        }
                    },
                }
            }
            if let Some(p) = problem() {
                p { class: "muted small", role: "status", "{tr(l, p)}" }
            }
            if let Some(o) = outcome() {
                crate::ui::Card { element: crate::ui::CardElement::Div, class: "aksara-result", role: "status",
                    if done {
                        p { class: "ok",
                            if mode() == Mode::Trace { "{tr(l, T::TraceDone)}" } else { "{tr(l, T::WriteDone)}" }
                        }
                    } else if matches!(o, Outcome::Shape(_)) {
                        p { "{tr(l, T::TryAgainHere)}" }
                    }
                    match o {
                        Outcome::Shape(r) => rsx! {
                            ul { class: "aksara-issues",
                                for (i, issue) in r.issues.iter().enumerate() {
                                    li { key: "{i}",
                                        for (j, piece) in issue_pieces(l, issue, &g).into_iter().enumerate() {
                                            match piece {
                                                Piece::Text(t) => rsx! { span { key: "{j}", "{t}" } },
                                                Piece::Snippet(t) => rsx! { bdi { key: "{j}", class: "latin", "{t}" } },
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Outcome::Teacher(c) => rsx! {
                            p { class: "small",
                                "{tr(l, T::TeacherSays)} "
                                span { class: "badge", "{tr(l, T::Unverified)}" }
                            }
                            ul { class: "aksara-issues",
                                for (i, line) in c.issues.iter().enumerate() {
                                    li { key: "{i}", bdi { class: "latin", dir: "ltr", "{line}" } }
                                }
                            }
                        },
                    }
                    if done {
                        div { class: "row",
                            if mode() == Mode::Trace {
                                button {
                                    class: "primary",
                                    onclick: {
                                        let mut open = open.clone();
                                        let id = g.id.clone();
                                        move |_| open(id.clone(), Mode::Write)
                                    },
                                    "{tr(l, T::TryWriting)}"
                                }
                            } else {
                                button {
                                    class: "primary",
                                    onclick: {
                                        let mut open = open.clone();
                                        let next = next.clone();
                                        move |_| open(next(), Mode::Watch)
                                    },
                                    "{tr(l, T::NextLetter)}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_set_parses_and_frames_fit_the_strokes() {
        let set = AksaraSet::parse(STROKES_JSON).unwrap();
        for g in &set.glyphs {
            let [x, y, w, h] = frame(g);
            let [x0, y0, x1, y1] = g.bbox();
            assert!(x < x0 as f32 && y < y0 as f32 && x + w > x1 as f32 && y + h > y1 as f32, "{}", g.id);
            assert!(w >= h * 0.79, "{}: room around narrow letters", g.id);
            let [sw, sh] = sheet_size(g);
            assert!((sw / sh - w / h).abs() < 1e-3, "sheet and guide share an aspect ratio");
        }
        assert_eq!(path_d(&[[1, 2], [3, -4]]), "M1 2 L3 -4");
    }

    #[test]
    fn issues_read_in_every_language() {
        let set = AksaraSet::parse(STROKES_JSON).unwrap();
        let ka = set.get("ka").unwrap();
        let ga = set.get("ga").unwrap();
        let reads_as = Issue::ReadsAs { id: ga.id.clone(), text: ga.text.clone(), latin: ga.latin.clone() };
        for l in [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En] {
            for issue in [
                Issue::Missing { stroke: 1, part: Part::Right },
                Issue::Extra { part: Part::Above },
                Issue::Mirrored,
                reads_as.clone(),
                Issue::Empty,
            ] {
                let pieces = issue_pieces(l, &issue, ka);
                assert!(!pieces.is_empty());
                assert!(pieces.iter().all(|p| match p {
                    Piece::Text(t) | Piece::Snippet(t) => !t.trim().is_empty() || t == " ",
                }));
            }
        }
        // Aksara and Latin stay in their own isolated pieces in Arabic.
        let ar = issue_pieces(UiLanguage::ArEg, &reads_as, ka);
        assert!(ar.iter().any(|p| matches!(p, Piece::Snippet(t) if t == "ga ꦒ")));
    }

    #[test]
    fn ink_points_undo_the_coordinate_scale() {
        let stroke = InkStroke {
            id: "a".into(),
            doc: "d".into(),
            tool: InkTool::Brush,
            color: "ink".into(),
            width: 3.0,
            pointer: kabar_protocol::pen::PointerKind::Pen,
            xs: vec![100, 200],
            ys: vec![50, 60],
            ps: vec![],
            ts: vec![0, 8],
            started_ms: 0,
        };
        assert_eq!(ink_points(&[stroke]), vec![vec![(10.0, 5.0), (20.0, 6.0)]]);
    }
}
