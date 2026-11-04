//! The Tablet Studio through its real screens: the ink canvas fed by a fake
//! `ink.js`, the mind map with a scripted engine, and every pen tool's
//! fallback when Kabar can't read ink on this device.

use std::cell::RefCell;
use std::rc::Rc;

use kabar_core::aksara::AksaraSet;
use kabar_protocol::ai::EngineId;
use kabar_protocol::coach::ActivityKind;
use kabar_protocol::ink::{COORD_SCALE, DocKind, InkStroke, InkTool, ItemKind, NotebookItem};
use kabar_protocol::reader::WordStatus;
use serde_json::json;

use super::ink::{InkCanvas, InkTools, Pen, PenTool, use_ink};
use super::*;
use crate::Db;
use crate::testkit::{App, FakeEval, Scripted, Setup, fixture, reply, visible};

/// Another image: answers are cached by input, so a new reply needs new ink.
const PNG2: &str = "data:image/png;base64,ZGlmZmVyZW50IGluaw==";

/// A 1×1 PNG, as the canvas hands one over.
const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

fn canvas(app: &App) -> Rc<FakeEval> {
    app.doc.find("Kabar ink canvas").expect("an ink canvas started")
}

fn canvases(app: &App) -> Vec<Rc<FakeEval>> {
    app.doc.evals.borrow().iter().filter(|e| e.script.contains("Kabar ink canvas")).cloned().collect()
}

fn stroke(id: &str, points: &[(f32, f32)]) -> InkStroke {
    InkStroke {
        id: id.into(),
        doc: String::new(),
        tool: InkTool::Pen,
        color: "ink".into(),
        width: 2.2,
        pointer: kabar_protocol::pen::PointerKind::Pen,
        xs: points.iter().map(|p| (p.0 * COORD_SCALE) as i32).collect(),
        ys: points.iter().map(|p| (p.1 * COORD_SCALE) as i32).collect(),
        ps: vec![],
        ts: (0..points.len() as u32).map(|i| i * 8).collect(),
        started_ms: 1_761_472_800_000,
    }
}

/// The pen lifts on the canvas: `ink.js` posts the stroke.
fn draw(app: &mut App, e: &FakeEval, s: InkStroke) {
    e.reply(json!({ "type": "stroke", "stroke": s }));
    app.settle();
}

/// The next snapshot request gets `png` back from the canvas.
fn answer_snapshot(app: &mut App, e: &FakeEval, png: Option<&str>) {
    assert!(
        app.doc.scripts().iter().any(|s| s.contains("kabar-ink") && s.contains("snapshot")),
        "a snapshot was asked for"
    );
    e.reply(json!({ "type": "png", "data": png }));
    app.settle();
}

fn line() -> Vec<(f32, f32)> {
    (0..12).map(|i| (100.0 + i as f32 * 20.0, 200.0)).collect()
}

// --------------------------------------------------------------- studio

fn studio() -> Element {
    rsx! { StudioScreen {} }
}

#[test]
fn the_studio_shows_only_the_mind_map_while_pen_writing_is_paused() {
    let app = App::new(studio, Setup::default());
    let html = app.html();
    assert!(html.contains("class=\"mindmap\""), "{html}");
    assert!(html.contains("role=\"tablist\"") && html.contains("hidden"), "one tool: the tab bar hides");
    for t in [T::Notebook, T::Gambar, T::Listening, T::WriteTab, T::Aksara] {
        assert!(t.is_writing_tab(), "{t:?}");
    }
}

impl T {
    fn is_writing_tab(self) -> bool {
        let tab = match self {
            T::Notebook => Tab::Notebook,
            T::Gambar => Tab::Gambar,
            T::Listening => Tab::Listening,
            T::WriteTab => Tab::Write,
            T::Aksara => Tab::Aksara,
            _ => Tab::MindMap,
        };
        tab.is_writing()
    }
}

#[test]
fn helpers_map_languages_and_problems() {
    assert_eq!(word_lang(Lang::Id), WordLang::Indonesian);
    assert_eq!(word_lang(Lang::Jv), WordLang::Javanese);
    assert_eq!(friendly(FriendlyReason::ResetsLater), T::ResetsLater);
    assert_eq!(friendly(FriendlyReason::Unreachable), T::Unreachable);
    assert_eq!(friendly(FriendlyReason::NotAvailable), T::NotConnected);
    assert_eq!(friendly(FriendlyReason::TryAgain), T::TryAgain);
    assert_eq!(tool_labels(UiLanguage::En), ["Pen", "Highlighter", "Brush", "Eraser"]);
    let (a, b) = (new_id(), new_id());
    assert!(a < b && a.len() == 36, "UUIDv7 ids sort by time");
}

#[test]
fn every_studio_line_is_written_in_all_three_languages() {
    use T::*;
    let all = [
        Notebook,
        Gambar,
        MindMap,
        Listening,
        Pen,
        Highlighter,
        Brush,
        Eraser,
        MyNotebook,
        NewNotebook,
        NewPage,
        Page,
        Pages,
        AddCard,
        CardPlaceholder,
        Search,
        NoResults,
        ReadWriting,
        Reading,
        Lines,
        NoLines,
        TypeWhatYouWrote,
        Save,
        Remove,
        Listen,
        NotConnected,
        ResetsLater,
        Unreachable,
        TryAgain,
        Start,
        DrawThis,
        SecondsLeft,
        Done,
        Guessing,
        Got,
        Close,
        Missed,
        Unclear,
        KabarSaw,
        SelfCheck,
        Yes,
        NotReally,
        NextRound,
        Score,
        Topic,
        Make,
        Making,
        Unverified,
        Maps,
        HeardIt,
        NoSuggestions,
        AddReview,
        TeachIt,
        Saved,
        ScribbleHere,
        WriteTab,
        DictationIntro,
        PlayAgain,
        WriteHere,
        Check,
        Clear,
        CorrectForm,
        Aksara,
        AksaraIntro,
        Letters,
        Signs,
        Finals,
        Words,
        Watch,
        Trace,
        WriteFree,
        Replay,
        OrderUnverified,
        CheckShape,
        AskTeacher,
        TeacherSays,
        TraceDone,
        WriteDone,
        TryWriting,
        NextLetter,
        TryAgainHere,
        NothingYet,
    ];
    for t in all {
        match t {
            Notebook | Gambar | MindMap | Listening | Pen | Highlighter | Brush | Eraser | MyNotebook
            | NewNotebook | NewPage | Page | Pages | AddCard | CardPlaceholder | Search | NoResults
            | ReadWriting | Reading | Lines | NoLines | TypeWhatYouWrote | Save | Remove | Listen
            | NotConnected | ResetsLater | Unreachable | TryAgain | Start | DrawThis | SecondsLeft | Done
            | Guessing | Got | Close | Missed | Unclear | KabarSaw | SelfCheck | Yes | NotReally
            | NextRound | Score | Topic | Make | Making | Unverified | Maps | HeardIt | NoSuggestions
            | AddReview | TeachIt | Saved | ScribbleHere | WriteTab | DictationIntro | PlayAgain
            | WriteHere | Check | Clear | CorrectForm | Aksara | AksaraIntro | Letters | Signs | Finals
            | Words | Watch | Trace | WriteFree | Replay | OrderUnverified | CheckShape | AskTeacher
            | TeacherSays | TraceDone | WriteDone | TryWriting | NextLetter | TryAgainHere | NothingYet => {}
        }
    }
    // The game's name and the example topics are Indonesian on purpose.
    crate::testkit::check_catalogue(&all, tr, &[Gambar, CardPlaceholder, Topic]);
    assert_eq!(tr(UiLanguage::ArEg, Gambar), "Gambar!");
}

// -------------------------------------------------------------- mind map

fn mind_map() -> Element {
    rsx! { mindmap::MindMapScreen {} }
}

fn engine() -> Scripted {
    Scripted { id: EngineId::WorkersAi, script: Default::default() }
}

#[test]
fn a_topic_becomes_a_saved_laid_out_map() {
    let e = engine();
    e.script.replies.borrow_mut().push_back(Ok(fixture("mind_map.di_pasar.workers_ai.json")));
    let mut app = App::new(mind_map, Setup { engines: vec![e.clone()], ..Setup::default() });
    let html = app.html();
    assert!(!html.contains("map-list-title"), "no maps yet");
    // Nothing to make from an empty topic.
    app.submit("map-form");
    assert!(e.script.calls.borrow().is_empty());

    app.input("di pasar, keluarga", "di pasar");
    app.submit("map-form");
    let call = e.script.calls.borrow()[0].clone();
    assert!(call.user.contains("di pasar"), "{}", call.user);

    let db = app.db();
    let maps = db.0.borrow().documents(DocKind::MindMap).unwrap();
    assert_eq!(maps.len(), 1);
    assert_eq!((maps[0].title.as_str(), maps[0].lang), ("di pasar", Lang::Id));
    let items = db.0.borrow().items(&maps[0].id).unwrap();
    let labels: Vec<String> = items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::MapNode { label, .. } => Some(label.clone()),
            _ => None,
        })
        .collect();
    for w in ["di pasar", "orang", "penjual", "pembeli", "sayur"] {
        assert!(labels.iter().any(|l| l == w), "{w}: {labels:?}");
    }
    assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Arrow { .. })), "branches are joined");

    let html = app.html();
    let text = visible(&html);
    assert!(text.contains("Your maps") && text.contains("penjual") && text.contains("seller"), "{text}");
    assert!(html.contains("map-node depth-0") && html.contains("<line"), "{html}");
    assert!(canvas(&app).sent.borrow()[0]["focus"].is_array(), "the canvas opens on the whole map");
    // The box empties for the next topic.
    assert!(app.targets("input").iter().any(|t| t.attr("value") == Some("")));
}

#[test]
fn a_map_problem_says_why_and_saved_maps_reopen() {
    // No engine: the friendly reason, nothing saved.
    let mut app = App::new(mind_map, Setup::default());
    app.input("di pasar, keluarga", "keluarga");
    app.click("jv");
    app.submit("map-form");
    assert!(app.html().contains("map-problem"));
    assert!(app.db().0.borrow().documents(DocKind::MindMap).unwrap().is_empty());

    // Two saved maps: the newest opens, the other one on a tap; Javanese
    // maps always carry the unverified badge.
    let mut store = crate::store::Store::open_in_memory().unwrap();
    for (title, lang) in [("keluarga", Lang::Jv), ("cuaca", Lang::Id)] {
        let doc = kabar_protocol::ink::InkDocument {
            id: format!("map-{title}"),
            kind: DocKind::MindMap,
            notebook: None,
            title: title.into(),
            lang,
            order: 0,
            anchor: None,
            size: None,
            created_ms: 0,
        };
        store.save_ink_document(&doc).unwrap();
    }
    let mut app = App::new(mind_map, Setup { store: Some(store), ..Setup::default() });
    let pressed = |app: &App, title: &str| {
        app.targets("click")
            .into_iter()
            .find(|t| t.text == title)
            .and_then(|t| t.attr("aria-pressed").map(str::to_string))
    };
    let newest = if pressed(&app, "cuaca").as_deref() == Some("true") { "cuaca" } else { "keluarga" };
    let other = if newest == "cuaca" { "keluarga" } else { "cuaca" };
    app.click(other);
    assert_eq!(pressed(&app, other).as_deref(), Some("true"));
    assert_eq!(visible(&app.html()).contains("unverified"), other == "keluarga");
}

// ------------------------------------------------------------- ink tools

thread_local! {
    static PEN: RefCell<Option<Signal<Pen>>> = const { RefCell::new(None) };
}

fn tools() -> Element {
    let pen = use_signal(Pen::default);
    PEN.with(|p| *p.borrow_mut() = Some(pen));
    rsx! { InkTools { pen, labels: tool_labels(UiLanguage::En), brush: true } }
}

#[test]
fn the_tool_bar_switches_tools_widths_and_colours() {
    let mut app = App::new(tools, Setup::default());
    let pen = || *PEN.with(|p| p.borrow().unwrap()).peek();
    assert_eq!(pen(), Pen::default());
    app.click("Highlighter");
    assert_eq!((pen().tool, pen().width), (PenTool::Ink(InkTool::Highlighter), 3.0));
    app.click("Brush");
    assert_eq!((pen().tool, pen().width), (PenTool::Ink(InkTool::Brush), 4.0));
    app.click("dawn");
    assert_eq!((pen().tool, pen().color), (PenTool::Ink(InkTool::Brush), "dawn"));
    app.click("Eraser");
    assert_eq!(pen().tool, PenTool::Eraser);
    // A colour while erasing picks the pen back up.
    app.click("indigo");
    assert_eq!((pen().tool, pen().color, pen().width), (PenTool::Ink(InkTool::Pen), "indigo", 2.2));
    let html = app.html();
    assert!(html.contains("aria-pressed=\"true\""), "{html}");
}

#[test]
fn without_brush_the_tool_bar_has_three_tools() {
    fn root() -> Element {
        let pen = use_signal(Pen::default);
        rsx! { InkTools { pen, labels: tool_labels(UiLanguage::En) } }
    }
    let app = App::new(root, Setup::default());
    assert!(!app.has("click", "Brush") && app.has("click", "Eraser"));
}

// ------------------------------------------------------------ the canvas

thread_local! {
    static SEEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn sheet() -> Element {
    let handle = use_ink();
    let pen = use_signal(Pen::default);
    let rev = *handle.revision.read();
    let view = *handle.view.read();
    rsx! {
        p { "revision {rev} view {view:?}" }
        InkCanvas {
            handle: handle.clone(),
            doc: "sheet-1",
            size: Some([720.0, 220.0]),
            pen,
            class: "ink-sheet",
            on_stroke: move |s: InkStroke| SEEN.with(|v| v.borrow_mut().push(format!("+{}", s.id))),
            on_erase: move |ids: Vec<String>| SEEN.with(|v| v.borrow_mut().extend(ids.into_iter().map(|i| format!("-{i}")))),
            underlay: rsx! { span { "guide" } },
            span { "a card" }
        }
    }
}

#[test]
fn strokes_from_the_canvas_are_saved_and_erased_in_the_store() {
    SEEN.with(|v| v.borrow_mut().clear());
    let mut app = App::new(sheet, Setup::default());
    let html = app.html();
    assert!(
        html.contains("aspect-ratio: 720 / 220") && html.contains("ink-under") && html.contains("a card"),
        "{html}"
    );
    let e = canvas(&app);
    let init = e.sent.borrow()[0].clone();
    assert_eq!((init["tool"]["tool"].as_str(), init["tool"]["color"].as_str()), (Some("pen"), Some("ink")));
    assert!((init["tool"]["width"].as_f64().unwrap() - 2.2).abs() < 1e-6);
    assert_eq!(init["size"], json!([720.0, 220.0]));

    draw(&mut app, &e, stroke("s1", &line()));
    let db = app.db();
    let saved = db.0.borrow().strokes("sheet-1").unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].doc, "sheet-1", "the stroke is filed under this canvas's document");
    assert!(visible(&app.html()).contains("revision 1"));

    e.reply(json!({ "type": "erase", "ids": ["s1"] }));
    app.settle();
    assert!(db.0.borrow().strokes("sheet-1").unwrap().is_empty());
    assert!(visible(&app.html()).contains("revision 2"));
    SEEN.with(|v| assert_eq!(*v.borrow(), ["+s1", "-s1"]));

    e.reply(json!({ "type": "view", "rect": [10.0, 20.0, 300.0, 200.0] }));
    app.settle();
    assert!(visible(&app.html()).contains("view [10.0, 20.0, 300.0, 200.0]"));
}

// --------------------------------------------------------------- notebook

fn notebook() -> Element {
    rsx! { notebook::NotebookScreen {} }
}

#[test]
fn a_first_visit_makes_a_notebook_with_one_page() {
    let mut app = App::new(notebook, Setup::default());
    let db = app.db();
    let books = db.0.borrow().notebooks().unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].title, "My notebook");
    let pages = db.0.borrow().pages(&books[0].id).unwrap();
    assert_eq!(pages.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(), ["Page 1"]);
    let text = visible(&app.html());
    assert!(text.contains("Write with the pen; your lines show up here"), "{text}");

    app.click("+ New page");
    let pages = db.0.borrow().pages(&books[0].id).unwrap();
    assert_eq!(pages.len(), 2);
    assert!(visible(&app.html()).contains("Page 2"));
    app.click("Page 1");

    app.click("+ New notebook");
    let books = db.0.borrow().notebooks().unwrap();
    assert_eq!(books.len(), 2);
    assert!(books.iter().any(|b| b.title == "My notebook 2"));
    // Each notebook opens on a page of its own.
    let second = books.iter().find(|b| b.title == "My notebook 2").unwrap();
    assert_eq!(db.0.borrow().pages(&second.id).unwrap().len(), 1);
    app.change("Notebook", &books[0].id);
    assert!(visible(&app.html()).contains("Page 2"), "back on the first notebook");
}

#[test]
fn word_cards_show_their_gloss_and_can_be_removed() {
    let mut app = App::new(notebook, Setup::default());
    app.submit("add-card");
    let db = app.db();
    let page = canvas_doc_of_page(&app);
    assert!(db.0.borrow().items(&page).unwrap().is_empty(), "an empty card isn't added");
    app.input("Add a word card", "makan");
    app.submit("add-card");
    let items = db.0.borrow().items(&page).unwrap();
    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0].kind, ItemKind::WordCard { word, lang: Lang::Id } if word == "makan"));
    let text = visible(&app.html());
    assert!(text.contains("makan") && text.contains("eat"), "the card shows the gloss: {text}");
    app.click("Listen");
    assert!(app.doc.scripts().iter().any(|s| s.contains("SpeechSynthesisUtterance") || s.contains("makan")));
    app.click("Remove");
    assert!(db.0.borrow().items(&page).unwrap().is_empty());
}

fn canvas_doc_of_page(app: &App) -> String {
    let db = app.db();
    let books = db.0.borrow().notebooks().unwrap();
    db.0.borrow().pages(&books[0].id).unwrap()[0].id.clone()
}

#[test]
fn written_lines_can_be_typed_searched_and_found() {
    let mut app = App::new(notebook, Setup::default());
    let page = canvas_doc_of_page(&app);
    let e = canvas(&app);
    draw(&mut app, &e, stroke("s1", &line()));
    // Recognition isn't on this device: the reason, and the typed way.
    app.click("Read my writing");
    answer_snapshot(&mut app, &e, Some(PNG));
    let text = visible(&app.html());
    assert!(text.contains("Kabar can't read ink on this device yet"), "{text}");
    app.input("Type what you wrote", "saya makan nasi");
    app.submit("line");
    let db = app.db();
    let ts = db.0.borrow().transcripts(&page).unwrap();
    assert_eq!(ts.len(), 1);
    assert_eq!(
        (ts[0].text.as_str(), ts[0].source),
        ("saya makan nasi", kabar_protocol::ink::TranscriptSource::Typed)
    );
    assert_eq!(ts[0].strokes, ["s1"]);

    app.input("Search your notebooks", "nasi");
    let text = visible(&app.html());
    assert!(text.contains("saya makan nasi") && text.contains("Page 1") && text.contains("✎"), "{text}");
    app.click("saya makan nasi · Page 1");
    app.input("Search your notebooks", "zzz");
    assert!(visible(&app.html()).contains("Nothing with that yet."));
}

#[test]
fn an_empty_snapshot_asks_to_try_again() {
    let mut app = App::new(notebook, Setup::default());
    let e = canvas(&app);
    draw(&mut app, &e, stroke("s1", &line()));
    app.click("Read my writing");
    assert!(visible(&app.html()).contains("Reading…"));
    answer_snapshot(&mut app, &e, None);
    assert!(visible(&app.html()).contains("Something went wrong. Try again."));
}

#[test]
fn items_on_a_page_draw_cards_notes_nodes_and_arrows() {
    fn root() -> Element {
        let item = |id: &str, kind, x| NotebookItem {
            id: id.into(),
            doc: "d".into(),
            kind,
            x,
            y: 10.0,
            w: 100.0,
            h: 40.0,
        };
        let items = vec![
            item("a", ItemKind::WordCard { lang: Lang::Jv, word: "mangan".into() }, 0.0),
            item("b", ItemKind::Text { text: "a note".into() }, 200.0),
            item(
                "c",
                ItemKind::MapNode {
                    label: "pasar".into(),
                    gloss: Some("market".into()),
                    depth: 1,
                    parent: None,
                },
                400.0,
            ),
            item(
                "d",
                ItemKind::Arrow { from: Some("a".into()), to: Some("c".into()), x2: 0.0, y2: 0.0 },
                0.0,
            ),
            item("e", ItemKind::Arrow { from: Some("b".into()), to: None, x2: 7.0, y2: 8.0 }, 0.0),
            item("f", ItemKind::Arrow { from: None, to: None, x2: 0.0, y2: 0.0 }, 0.0),
        ];
        rsx! { notebook::ItemsLayer { items, on_remove: None } }
    }
    let html = App::new(root, Setup::default()).html();
    assert!(html.contains("x1=\"50\"") && html.contains("x2=\"450\""), "a card-to-node arrow: {html}");
    assert!(html.contains("x2=\"7\"") && html.contains("y2=\"8\""), "an arrow to a free point");
    assert_eq!(html.matches("<line").count(), 2, "an arrow from nowhere isn't drawn");
    let text = visible(&html);
    assert!(
        text.contains("mangan")
            && text.contains("a note")
            && text.contains("pasar")
            && text.contains("market"),
        "{text}"
    );
    assert!(html.contains("map-node depth-1") && html.contains("inset-inline-start: 400px"), "{html}");
    assert!(!html.contains("item-remove"), "a read-only layer has no remove buttons");
}

// --------------------------------------------------------------- Gambar!

fn gambar() -> Element {
    rsx! { gambar::GambarScreen {} }
}

fn knows_kucing() -> crate::store::Store {
    let mut store = crate::store::Store::open_in_memory().unwrap();
    store.set_word_status("id:kucing", WordStatus::Learning).unwrap();
    store
}

#[test]
fn a_round_draws_a_known_word_against_the_clock_then_self_checks() {
    let mut app = App::new(gambar, Setup { store: Some(knows_kucing()), ..Setup::default() });
    assert!(visible(&app.html()).contains("Gambar!"));
    app.click("Start");
    let text = visible(&app.html());
    assert!(text.contains("Draw:") && text.contains("kucing") && text.contains("cat"), "{text}");
    assert!(text.contains("60 seconds left"), "{text}");
    let db = app.db();
    let sheets = db.0.borrow().documents(DocKind::Gambar).unwrap();
    assert_eq!(sheets.iter().map(|d| d.title.as_str()).collect::<Vec<_>>(), ["kucing"]);
    assert!(app.has("click", "Brush"), "the brush is offered while drawing");

    // The clock ticks once a second.
    let clock = app.doc.find("setInterval").unwrap();
    clock.reply(json!(1));
    clock.reply(json!(1));
    app.settle();
    assert!(visible(&app.html()).contains("58 seconds left"));

    app.click("Done");
    assert!(visible(&app.html()).contains("Kabar is looking at your drawing"));
    let e = canvas(&app);
    answer_snapshot(&mut app, &e, Some(PNG));
    // No vision engine here: the learner judges the sketch.
    let text = visible(&app.html());
    assert!(text.contains("Kabar can't see drawings on this device yet"), "{text}");
    let before = db.0.borrow().activity(None).unwrap().len();
    app.click("Yes");
    let log = db.0.borrow().activity(None).unwrap();
    assert_eq!(log.len(), before + 1);
    assert!(
        matches!(&log.last().unwrap().kind, ActivityKind::Practice { topic, correct: 1, total: 1, .. } if topic == "gambar:kucing")
    );
    let text = visible(&app.html());
    assert!(text.contains("1 / 1 guessed") && text.contains("seconds left"), "a new round starts: {text}");
}

#[test]
fn the_round_ends_by_itself_when_time_runs_out() {
    let mut app = App::new(gambar, Setup { store: Some(knows_kucing()), ..Setup::default() });
    app.click("Start");
    let clock = app.doc.find("setInterval").unwrap();
    for _ in 0..60 {
        clock.reply(json!(1));
    }
    app.settle();
    assert!(visible(&app.html()).contains("Kabar is looking at your drawing"));
    let e = canvas(&app);
    answer_snapshot(&mut app, &e, None);
    app.click("Not really");
    assert!(visible(&app.html()).contains("0 / 1 guessed"));
}

#[test]
fn known_drawables_are_the_learners_words_only() {
    fn root() -> Element {
        let db = use_context::<Db>();
        db.0.borrow_mut().set_word_status("id:rumah", WordStatus::Known).unwrap();
        db.0.borrow_mut().set_word_status("id:anjing", WordStatus::Ignored).unwrap();
        let known = gambar_known(&db);
        rsx! { p { "{known:?}" } }
    }
    let text = visible(&App::new(root, Setup::default()).html());
    assert_eq!(text, "[\"rumah\"]");
}

fn gambar_known(db: &Db) -> Vec<&'static str> {
    gambar::known_drawables_for_tests(db)
}

// -------------------------------------------------- dictation and the pad

thread_local! {
    static ANSWERS: RefCell<Vec<super::pad::InkAnswer>> = const { RefCell::new(Vec::new()) };
}

fn pad() -> Element {
    rsx! {
        super::pad::InkPad {
            lang: Lang::Id,
            expected: "makan",
            on_submit: move |a| ANSWERS.with(|v| v.borrow_mut().push(a)),
        }
    }
}

#[test]
fn the_pad_hands_over_typed_text_or_says_what_is_missing() {
    ANSWERS.with(|v| v.borrow_mut().clear());
    let mut app = App::new(pad, Setup::default());
    let db = app.db();
    let sheets = db.0.borrow().documents(DocKind::LessonAnswer).unwrap();
    assert_eq!(sheets.len(), 1, "the answer sheet is saved for its strokes");
    // Nothing written or typed.
    app.click("Check");
    assert!(visible(&app.html()).contains("Write your answer here"));
    // Written but not readable here: type it instead.
    let e = canvas(&app);
    draw(&mut app, &e, stroke("s1", &line()));
    app.click("Check");
    answer_snapshot(&mut app, &e, Some(PNG));
    assert!(visible(&app.html()).contains("Kabar can't read ink on this device yet"));
    app.input("Type what you wrote", " makan ");
    app.click("Check");
    ANSWERS.with(|v| {
        let a = &v.borrow()[0];
        assert_eq!(
            (a.text.as_deref(), a.strokes, a.doc_id.as_str()),
            (Some("makan"), 1, sheets[0].id.as_str())
        );
    });
    let ts = db.0.borrow().transcripts(&sheets[0].id).unwrap();
    assert_eq!(ts[0].text, "makan");
    // Clear wipes the ink and the typed text.
    app.click("Clear");
    assert!(db.0.borrow().strokes(&sheets[0].id).unwrap().is_empty());
    assert!(app.doc.scripts().iter().any(|s| s.contains("\"clear\"")));
}

#[test]
fn feedback_marks_wrong_missing_and_extra_letters() {
    fn root() -> Element {
        rsx! { super::pad::InkFeedback { written: "makn", expected: "makan" } }
    }
    let html = App::new(root, Setup::default()).html();
    assert!(html.contains("ink-missing") && html.contains("aria-label=\"a\""), "{html}");
    assert!(visible(&html).contains("Correct: makan"));
    fn extra() -> Element {
        rsx! { super::pad::InkFeedback { written: "mbkann", expected: "makan" } }
    }
    let html = App::new(extra, Setup::default()).html();
    assert!(html.contains("ink-wrong") || html.contains("ink-extra"), "{html}");
}

#[test]
fn dictation_says_a_word_and_shows_the_feedback_for_the_answer() {
    fn root() -> Element {
        rsx! { super::pad::DictationScreen {} }
    }
    let mut app = App::new(root, Setup::default());
    app.click("Play again");
    assert!(app.doc.scripts().iter().any(|s| s.contains("SpeechSynthesisUtterance")));
    app.input("Type what you wrote", "xyz");
    app.click("Check");
    let text = visible(&app.html());
    assert!(text.contains("Correct:") && text.contains("Next word"), "{text}");
    app.click("Next word");
    assert!(visible(&app.html()).contains("Write your answer here"));
}

// ---------------------------------------------------- listening companion

fn companion() -> Element {
    rsx! { listening::ListeningScreen {} }
}

#[test]
fn a_heard_word_gets_real_suggestions_to_save_or_teach() {
    let mut app = App::new(companion, Setup::default());
    let db = app.db();
    assert_eq!(db.0.borrow().documents(DocKind::Listening).unwrap().len(), 1, "a scribble sheet is ready");
    app.input("What you heard", "makan");
    let text = visible(&app.html());
    assert!(text.contains("makan") && text.contains("eat"), "{text}");
    app.click_nth("Add to review", 0);
    assert!(visible(&app.html()).contains("Saved"));
    let saved = db.0.borrow().saved_words(10).unwrap();
    assert!(saved[0].lexeme.starts_with("id:"), "{saved:?}");
    app.click("Teach Kabar");
    let ui = app.ui();
    assert_eq!(*ui.screen.peek(), crate::Screen::Teach);
    let seed = app.with(consume_context::<crate::teach::TeachInbox>).0.peek().clone().unwrap();
    assert_eq!((seed.lang, seed.word.as_str()), (Lang::Id, "makan"));
}

#[test]
fn a_nonsense_word_offers_teaching_and_clear_starts_a_new_sheet() {
    let mut app = App::new(companion, Setup::default());
    app.click("jv");
    app.input("What you heard", "pneumonoultramicroscopic");
    let text = visible(&app.html());
    assert!(text.contains("No close word yet"), "{text}");
    app.click("Clear");
    let db = app.db();
    assert_eq!(db.0.borrow().documents(DocKind::Listening).unwrap().len(), 2);
    assert!(app.targets("input").iter().any(|t| t.attr("value") == Some("")));
    // Reading the scribble needs Kabar Cloud.
    let e = canvas(&app);
    app.click("Read my writing");
    answer_snapshot(&mut app, &e, Some(PNG));
    assert!(visible(&app.html()).contains("Kabar can't read ink on this device yet"));
}

// ----------------------------------------------------------------- aksara

fn aksara() -> Element {
    rsx! { aksara::AksaraScreen {} }
}

fn set() -> AksaraSet {
    AksaraSet::parse(include_str!("../../../../content/aksara/strokes.json")).unwrap()
}

#[test]
fn a_letter_is_watched_traced_checked_and_written() {
    let mut app = App::new(aksara, Setup::default());
    let html = app.html();
    assert!(html.contains("@font-face") && html.contains("aksara-watch"), "the stroke order plays first");
    let text = visible(&html);
    assert!(
        text.contains("Watch the stroke order") && text.contains("Letters") && text.contains("Words"),
        "{text}"
    );

    app.click_nth("Trace", 0);
    let db = app.db();
    let sheets = db.0.borrow().documents(DocKind::Aksara).unwrap();
    assert_eq!(sheets.iter().map(|d| d.title.as_str()).collect::<Vec<_>>(), ["ha"]);
    assert!(
        app.html().contains("guide-path") && app.html().contains("start-num"),
        "numbered strokes to trace"
    );

    // Nothing drawn yet.
    app.click("Check shape");
    assert!(visible(&app.html()).contains("Nothing written yet."));

    // Trace the letter's own strokes: the shape is right.
    let ha = set().get("ha").unwrap().clone();
    let e = canvas(&app);
    for (i, s) in ha.strokes.iter().enumerate() {
        let pts: Vec<(f32, f32)> = s.iter().map(|p| (p[0] as f32, p[1] as f32)).collect();
        draw(&mut app, &e, stroke(&format!("h{i}"), &pts));
    }
    let before = db.0.borrow().activity(None).unwrap().len();
    app.click("Check shape");
    let text = visible(&app.html());
    assert!(text.contains("Lovely, the shape is right."), "{text}");
    assert_eq!(db.0.borrow().activity(None).unwrap().len(), before + 1, "the attempt is logged");

    app.click("Write it alone");
    assert!(!app.html().contains("guide-path"), "no guide when writing freehand");
    let e = canvases(&app).pop().unwrap();
    for (i, s) in ha.strokes.iter().enumerate() {
        let pts: Vec<(f32, f32)> = s.iter().map(|p| (p[0] as f32, p[1] as f32)).collect();
        draw(&mut app, &e, stroke(&format!("w{i}"), &pts));
    }
    app.click("Check shape");
    assert!(visible(&app.html()).contains("you wrote it on your own"));
    app.click("Next one");
    let next = &set().glyphs[1];
    assert!(visible(&app.html()).contains(&next.latin), "on to {}", next.id);
}

#[test]
fn a_wrong_shape_lists_what_to_fix_and_clear_wipes_it() {
    let mut app = App::new(aksara, Setup { lang: UiLanguage::ArEg, ..Setup::default() });
    app.click("اكتب لوحدك");
    let e = canvas(&app);
    draw(&mut app, &e, stroke("x", &line()));
    app.click("شوف الشكل");
    let text = visible(&app.html());
    assert!(text.contains("قربت!"), "{text}");
    app.click("امسح");
    let db = app.db();
    let doc = db.0.borrow().documents(DocKind::Aksara).unwrap()[0].id.clone();
    assert!(db.0.borrow().strokes(&doc).unwrap().is_empty());
    assert!(!visible(&app.html()).contains("قربت!"));
    // The teacher needs Kabar Cloud.
    app.click("اسأل المدرّس");
    answer_snapshot(&mut app, &e, Some(PNG));
    assert!(visible(&app.html()).contains("لسه مش شايف الخط"));
}

#[test]
fn picking_another_glyph_and_replaying_its_order() {
    let mut app = App::new(aksara, Setup::default());
    let words: Vec<_> =
        set().glyphs.into_iter().filter(|g| g.kind == kabar_core::aksara::GlyphKind::Word).collect();
    let w = &words[0];
    app.click(&w.latin);
    let html = app.html();
    assert!(html.contains(&format!("aria-label=\"{}\"", w.latin)), "{html}");
    let before = html.matches("anim-stroke").count();
    app.click("Replay");
    assert_eq!(app.html().matches("anim-stroke").count(), before);
    if !w.verified {
        assert!(visible(&app.html()).contains("Stroke order is a draft"));
    }
}

// ----------------------------------------------------------------- vision

#[test]
fn uploads_refuse_bad_or_huge_images_and_need_the_cloud() {
    let link = crate::cloud::link::CloudLink::default();
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    assert_eq!(
        rt.block_on(vision::upload(&link, "data:image/jpeg;base64,AAAA")),
        Err(FriendlyReason::TryAgain)
    );
    use base64::Engine as _;
    let huge = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(vec![0u8; vision::MAX_BLOB_BYTES + 1])
    );
    assert_eq!(rt.block_on(vision::upload(&link, &huge)), Err(FriendlyReason::TryAgain));
    assert_eq!(rt.block_on(vision::upload(&link, PNG)), Err(FriendlyReason::NotAvailable));
}

// ------------------------------------------- with Kabar Cloud signed in

fn cloud_with(replies: Vec<kabar_protocol::ai::EngineReply>) -> (Scripted, Setup) {
    let e = engine();
    e.script.replies.borrow_mut().extend(replies.into_iter().map(Ok));
    (e.clone(), Setup { engines: vec![e], cloud: true, ..Setup::default() })
}

#[test]
fn the_notebook_reads_each_new_line_once() {
    let (e, setup) = cloud_with(vec![reply(json!({ "text": "Saya makan nasi." }))]);
    let mut app = App::new(notebook, setup);
    let page = canvas_doc_of_page(&app);
    let c = canvas(&app);
    draw(&mut app, &c, stroke("s1", &line()));
    app.click("Read my writing");
    answer_snapshot(&mut app, &c, Some(PNG));
    let db = app.db();
    let ts = db.0.borrow().transcripts(&page).unwrap();
    assert_eq!(ts.len(), 1);
    assert_eq!(
        (ts[0].text.as_str(), ts[0].source),
        ("Saya makan nasi.", kabar_protocol::ink::TranscriptSource::Ai)
    );
    assert!(
        e.script.calls.borrow()[0].image_sha256.as_deref().is_some_and(|s| s.len() == 64),
        "the image goes by hash"
    );
    let html = app.html();
    assert!(html.contains("value=\"Saya makan nasi.\"") && visible(&html).contains("AI"), "{html}");
    // Read again: nothing changed, so nothing is asked.
    app.click("Read my writing");
    app.settle();
    assert_eq!(e.script.calls.borrow().len(), 1);
    // The line's pencil scrolls the canvas to it.
    app.click("↗");
    assert!(app.doc.scripts().iter().any(|s| s.contains("\"focus\"")));
}

#[test]
fn the_pad_reads_handwriting_or_asks_to_type_it() {
    ANSWERS.with(|v| v.borrow_mut().clear());
    let (_, setup) =
        cloud_with(vec![reply(json!({ "text": "  " })), fixture("read_handwriting.berkerja.local.json")]);
    let mut app = App::new(pad, setup);
    let c = canvas(&app);
    draw(&mut app, &c, stroke("s1", &line()));
    // An empty reading: type it instead.
    app.click("Check");
    answer_snapshot(&mut app, &c, Some(PNG));
    assert!(visible(&app.html()).contains("Type what you wrote"));
    assert!(ANSWERS.with(|v| v.borrow().is_empty()));
    app.click("Check");
    answer_snapshot(&mut app, &c, Some(PNG2));
    ANSWERS.with(|v| assert_eq!(v.borrow()[0].text.as_deref(), Some("Saya berkerja di Jakarta.")));
    let db = app.db();
    let sheet = &db.0.borrow().documents(DocKind::LessonAnswer).unwrap()[0].id.clone();
    let ts = db.0.borrow().transcripts(sheet).unwrap();
    assert_eq!(ts[0].source, kabar_protocol::ink::TranscriptSource::Ai);
}

#[test]
fn the_companion_reads_the_scribble_into_the_box() {
    let (_, setup) = cloud_with(vec![reply(json!({ "text": " makan " }))]);
    let mut app = App::new(companion, setup);
    let c = canvas(&app);
    draw(&mut app, &c, stroke("s1", &line()));
    app.click("Read my writing");
    answer_snapshot(&mut app, &c, Some(PNG));
    assert!(app.targets("input").iter().any(|t| t.attr("value") == Some("makan")));
    assert!(visible(&app.html()).contains("eat"));
    app.click_nth("🔊", 0);
    assert!(app.doc.scripts().iter().any(|s| s.contains("SpeechSynthesisUtterance")));
}

#[test]
fn gambar_scores_a_guess_kabar_got_and_one_it_missed() {
    let (_, setup) = cloud_with(vec![
        fixture("guess_sketch.kucing.workers_ai.json"),
        reply(json!({ "guess_id": "rumah", "alternatives": ["mobil"] })),
    ]);
    let mut app = App::new(gambar, Setup { store: Some(knows_kucing()), ..setup });
    app.click("Start");
    app.click("Done");
    let c = canvas(&app);
    answer_snapshot(&mut app, &c, Some(PNG));
    let text = visible(&app.html());
    assert!(text.contains("Kabar got it! Nice.") && text.contains("Kabar saw: kucing"), "{text}");
    assert!(text.contains("1 / 1 guessed"), "{text}");
    app.click("Next word");
    app.click("Done");
    let c = canvases(&app).pop().unwrap();
    answer_snapshot(&mut app, &c, Some(PNG2));
    let text = visible(&app.html());
    assert!(text.contains("Kabar saw: rumah") && text.contains("1 / 2 guessed"), "{text}");
}

#[test]
fn eleven_rounds_keep_only_the_last_ten_words_out_of_the_draw() {
    let mut app = App::new(gambar, Setup::default());
    for _ in 0..11 {
        if app.has("click", "Start") {
            app.click("Start");
        }
        app.click("Done");
        let c = canvases(&app).pop().unwrap();
        answer_snapshot(&mut app, &c, None);
        app.click("Not really");
    }
    let docs = app.db().0.borrow().documents(DocKind::Gambar).unwrap();
    assert_eq!(docs.len(), 12, "one sheet a round, plus the round now open");
    let last_ten: Vec<_> = docs.iter().rev().take(10).map(|d| d.title.clone()).collect();
    let mut unique = last_ten.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 10, "no word repeats within ten rounds: {last_ten:?}");
}

#[test]
fn the_teacher_answers_with_issues_to_read() {
    let (_, setup) =
        cloud_with(vec![reply(json!({ "correct": false, "issues": ["The loop is open at the top."] }))]);
    let mut app = App::new(aksara, setup);
    app.click_nth("Trace", 0);
    let c = canvas(&app);
    draw(&mut app, &c, stroke("x", &line()));
    app.click("Ask the teacher");
    answer_snapshot(&mut app, &c, Some(PNG));
    let text = visible(&app.html());
    assert!(text.contains("The teacher says:") && text.contains("The loop is open at the top."), "{text}");
    assert!(text.contains("unverified"), "the teacher's word stays unverified: {text}");
}

#[test]
fn a_javanese_map_from_the_engine_is_flagged() {
    let (_, setup) = cloud_with(vec![fixture("mind_map.di_pasar.workers_ai.json")]);
    let mut app = App::new(mind_map, setup);
    app.input("di pasar, keluarga", "di pasar");
    app.click("jv");
    app.submit("map-form");
    let maps = app.db().0.borrow().documents(DocKind::MindMap).unwrap();
    assert_eq!(maps[0].lang, Lang::Jv);
    assert!(visible(&app.html()).contains("unverified"));
}

#[test]
fn the_canvas_follows_the_pen_and_redraws_strokes_from_elsewhere() {
    let mut app = App::new(sheet, Setup::default());
    let e = canvas(&app);
    // The tool bar's pen reaches ink.js after the first paint.
    let tool = app.doc.scripts().into_iter().rev().find(|s| s.contains("\"cmd\":\"tool\"")).unwrap();
    assert!(tool.contains("\"tool\":\"pen\""), "{tool}");
    draw(&mut app, &e, stroke("s1", &line()));
    assert_eq!(app.db().0.borrow().strokes("sheet-1").unwrap().len(), 1);
}

fn desk() -> Element {
    let handle = use_ink();
    let pen = use_signal(Pen::default);
    rsx! {
        InkTools { pen, labels: tool_labels(UiLanguage::En), brush: true }
        InkCanvas { handle: handle.clone(), doc: "desk-1", pen }
    }
}

fn last_tool(app: &App) -> String {
    let cmd =
        app.doc.scripts().into_iter().rev().find(|s| s.contains("\"cmd\":\"tool\"")).expect("a tool command");
    cmd.split("\"tool\":\"").nth(1).unwrap().split('"').next().unwrap().to_string()
}

#[test]
fn each_tool_on_the_bar_reaches_the_canvas() {
    let mut app = App::new(desk, Setup::default());
    assert_eq!(last_tool(&app), "pen");
    for (button, tool) in
        [("Highlighter", "highlighter"), ("Brush", "brush"), ("Eraser", "eraser"), ("indigo", "pen")]
    {
        app.click(button);
        assert_eq!(last_tool(&app), tool, "{button}");
    }
}

#[test]
fn strokes_synced_from_another_device_are_redrawn() {
    let mut app = App::new(desk, Setup { sync: Some(crate::cloud::SyncState::Live), ..Setup::default() });
    let before = app.doc.scripts().len();
    // A stroke arrives from the tablet; the cloud bumps its revision.
    let mut s = stroke("tab-1", &line());
    s.doc = "desk-1".into();
    app.db().0.borrow_mut().add_stroke(&s).unwrap();
    app.with(|| {
        let mut rev = consume_context::<crate::account::Cloud>().revision;
        rev += 1;
    });
    app.settle();
    let redraw = app.doc.scripts()[before..]
        .iter()
        .find(|s| s.contains("\"cmd\":\"strokes\""))
        .cloned()
        .expect("the canvas was redrawn");
    assert!(redraw.contains("tab-1"), "{redraw}");
}

#[test]
fn a_full_disk_keeps_the_saved_ink_as_it_was() {
    let mut app = App::new(desk, Setup::default());
    let e = canvas(&app);
    draw(&mut app, &e, stroke("s1", &line()));
    app.db().0.borrow().refuse_writes();
    draw(&mut app, &e, stroke("s2", &line()));
    e.reply(json!({ "type": "erase", "ids": ["s1"] }));
    app.settle();
    let saved: Vec<String> =
        app.db().0.borrow().strokes("desk-1").unwrap().into_iter().map(|s| s.id).collect();
    assert_eq!(saved, ["s1"]);
}
