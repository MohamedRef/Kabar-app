//! The Reader as the learner sees it: each screen state rendered with SSR
//! (`fixture`), and the actions behind its buttons run against the store.
//! The §24 test texts throughout; AI answers from `fixtures/ai/`.

use std::io::{Read as _, Write as _};

use kabar_core::layout::WidthClass;
use kabar_core::locale::UiLanguage;
use kabar_core::settings::TranslationMode;
use kabar_protocol::ai::{EngineError, EngineId};

use super::fixture::{Case, Screenful, Script, msg, open_text, render, text_of};
use super::*;

const EN: UiLanguage = UiLanguage::En;
const SAYA: &str = "Saya sedang membaca buku.";
/// The §24 Javanese chat, one line per bubble.
const CHAT: &str = "Sek tak lapor pusat\n\narp dicul piro";

fn reader() -> Case {
    Case::new(|| rsx! { Reader {} })
}

fn with_open(body: fn() -> Element, prep: fn(&Db, ReaderState)) -> Case {
    let mut case = Case::new(body);
    case.prep = prep;
    case
}

fn token(st: &ReaderState, surface: &str) -> usize {
    let open = st.open.peek();
    let doc = &open.as_ref().unwrap().doc;
    doc.tokens.iter().position(|t| doc.text(t.start, t.end) == surface).unwrap()
}

fn ocr_fixture() -> OcrResult {
    serde_json::from_str(include_str!("../../../../fixtures/ocr/whatsapp_jv_chat.mlkit.json")).unwrap()
}

// ---- Home: nothing open -------------------------------------------------

#[test]
fn an_empty_reader_offers_paste_and_an_empty_library() {
    let html = render(reader());
    let text = text_of(&html);
    assert!(text.contains(reading::tagline(EN)), "{text}");
    assert!(html.contains("reader-textarea") && html.contains(msg(EN, Msg::ReaderPlaceholder)), "{html}");
    assert!(text.contains(msg(EN, Msg::ReaderRead)) && text.contains(msg(EN, Msg::ReaderScreenshot)));
    assert!(text.contains("No texts yet."), "{text}");
    // Nothing typed yet: "Read" can't be pressed.
    assert!(html.contains("class=\"primary\" disabled"), "{html}");
}

#[test]
fn the_arabic_reader_lays_the_empty_box_out_right_to_left() {
    let mut case = reader();
    case.lang = UiLanguage::ArEg;
    let html = render(case);
    assert!(html.contains(reading::tagline(UiLanguage::ArEg)), "{html}");
    assert!(html.contains("dir=\"rtl\" lang=\"id\""), "the placeholder reads right to left: {html}");
    assert!(html.contains("مكتبتك"), "{html}");
}

#[test]
fn the_library_lists_saved_texts_newest_first_with_their_source() {
    let mut case = reader();
    case.prep = |db, _| {
        let mut s = db.0.borrow_mut();
        s.save_text("Halo! Selamat pagi.", TextKind::Pasted, None, None).unwrap();
        s.save_text(CHAT, TextKind::Shared, Some("WhatsApp"), None).unwrap();
        s.save_web_text(Some("Berita"), "Saya sedang membaca berita.", "https://www.kompas.com/read/1")
            .unwrap();
    };
    let text = text_of(&render(case));
    let (web, shared, pasted) =
        (text.find("Berita").unwrap(), text.find("Sek tak").unwrap(), text.find("Halo!").unwrap());
    assert!(web < shared && shared < pasted, "newest first: {text}");
    assert!(text.contains("Web page · kompas.com"), "{text}");
    assert!(text.contains("Shared to Kabar · WhatsApp"), "{text}");
    assert!(text.contains("Pasted text"), "{text}");
}

#[test]
fn deleting_a_text_takes_it_off_the_list_only_once_the_store_did() {
    let mut s = Screenful::new(reader());
    let recent = s.act(|db, st| {
        let text = db.0.borrow_mut().save_text(SAYA, TextKind::Pasted, None, None).unwrap();
        let recent = Signal::new(db.0.borrow().recent_texts(10).unwrap());
        db.0.borrow().refuse_writes();
        library::delete_text(db, st, recent, &text.id);
        assert_eq!(recent.peek().len(), 1, "a refused delete keeps the row");
        assert_eq!(*st.toast.peek(), Some(Msg::ReaderWriteFailed));
        db.0.borrow().allow_writes();
        library::delete_text(db, st, recent, &text.id);
        recent.peek().clone()
    });
    assert!(recent.is_empty());
}

#[test]
fn a_saved_word_opens_in_its_text_with_its_card() {
    let mut s = Screenful::new(reader());
    s.act(|db, st| {
        let text = db.0.borrow_mut().save_text(SAYA, TextKind::Pasted, None, None).unwrap();
        library::open_at(db, st, text.clone(), "id:membaca");
        let i = token(&st, "membaca");
        assert_eq!(*st.panel.peek(), Some(Panel::Word(i)));
        // A saved phrase opens on its highlight.
        st.save_phrase(db, &[i, i + 1]);
        library::open_at(db, st, text, "phrase:membaca buku");
        assert_eq!(*st.panel.peek(), Some(Panel::Phrase { tokens: vec![i, i + 1], save_first: false }));
    });
}

// ---- An open text --------------------------------------------------------

#[test]
fn an_open_text_shows_every_word_its_meaning_and_its_grammar() {
    let html = render(with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, SAYA)));
    let text = text_of(&html);
    for (i, w) in ["Saya", "sedang", "membaca", "buku"].iter().enumerate() {
        assert!(html.contains(&format!("data-i=\"{i}\">{w}</span>")), "{w}: {html}");
    }
    assert!(html.contains("class=\"w st-new wc-ink wc-verb\" data-i=\"2\""), "{html}");
    assert!(text.contains("to read") && text.contains("book"), "glosses under the words: {text}");
    // No engine: the line under it is word by word, and says so.
    assert!(text.contains(msg(EN, Msg::TranslationNeedsCloud)), "{text}");
    assert!(text.contains(msg(EN, Msg::TranslationWordByWordShort)), "{text}");
    assert!(!text.contains(msg(EN, Msg::TranslationRetry)), "nothing to retry without Kabar Cloud");
    // The grammar the sentence uses, as chips and in the text's list.
    assert!(text.contains("Time without tenses"), "{text}");
    assert!(text.contains(msg(EN, Msg::GrammarInText)), "{text}");
    // The bar replaces the Reader's title, and the pen hint is the mouse's on a desktop.
    assert!(text.contains(msg(EN, Msg::ReaderNew)) && text.contains(msg(EN, Msg::ReadFocus)), "{text}");
    assert!(text.contains("Rest the mouse on any word"), "{text}");
}

#[test]
fn the_tablet_hint_is_the_pen_sentence() {
    let mut case = with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, SAYA));
    case.tablet = true;
    let text = text_of(&render(case));
    assert!(text.contains(msg(EN, Msg::PenHint)), "{text}");
}

#[test]
fn input_hints_speak_the_learners_language() {
    for lang in UiLanguage::ALL {
        let hint = input_hint(lang, false);
        assert!(!hint.is_empty());
        assert_eq!(input_hint(lang, true), msg(lang, Msg::PenHint));
        let arabic = hint.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c));
        assert_eq!(arabic, lang != UiLanguage::En, "{hint}");
    }
}

#[test]
fn statuses_and_highlights_colour_the_words() {
    let html = render(with_open(
        || rsx! { Reader {} },
        |db, st| {
            db.0.borrow_mut().set_word_status("id:membaca", WordStatus::Known).unwrap();
            open_text(db, st, SAYA);
            st.save_phrase(db, &[0, 2]);
        },
    ));
    assert!(html.contains("class=\"w st-known wc-ink wc-verb hl\" data-i=\"2\""), "{html}");
    assert!(html.contains("class=\"w st-new wc-ink wc-func hl\" data-i=\"0\""), "{html}");
    assert!(!html.contains("wc-noun hl"), "buku is outside the phrase: {html}");
    assert!(html.contains(msg(EN, Msg::PhraseSaved)), "the toast says it was saved: {html}");
}

#[test]
fn bridge_words_and_false_friends_are_marked() {
    let html =
        render(with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, "Selamat pagi. Kalimat ini.")));
    assert!(html.contains("from-arabic"), "{html}");
    assert!(html.contains("false-friend"), "kalimat is a sentence, not a word: {html}");
}

#[test]
fn a_chat_screenshot_shows_bubbles_speakers_and_times() {
    let mut s = Screenful::new(reader());
    s.act(|db, st| {
        let recent = Signal::new(Vec::new());
        open_ocr(db, st, recent, Ok(ocr_fixture()), Some("WhatsApp".into()), None);
        assert_eq!(recent.peek().len(), 1);
        assert_eq!(recent.peek()[0].kind, TextKind::Screenshot);
    });
    let html = s.settle();
    assert!(html.contains("seg bubble left") && html.contains("seg bubble right"), "{html}");
    assert!(html.contains("<div class=\"speaker\"><bdi>Pakdhe Joko</bdi></div>"), "{html}");
    assert!(html.contains(&format!("<bdi>{}</bdi>", msg(EN, Msg::Me))), "{html}");
    assert!(html.contains("<span class=\"time\">12:41</span>"), "{html}");
    assert!(html.contains("seg chrome"), "the input bar stays chrome: {html}");
    assert!(text_of(&html).contains(&format!("{} WhatsApp", msg(EN, Msg::SharedFrom))), "{html}");
}

#[test]
fn an_empty_or_failed_screenshot_says_so_and_opens_nothing() {
    let mut s = Screenful::new(reader());
    s.act(|db, st| {
        let recent = Signal::new(Vec::new());
        let empty = OcrResult { blocks: Vec::new(), ..ocr_fixture() };
        open_ocr(db, st, recent, Ok(empty), None, None);
        assert_eq!(*st.toast.peek(), Some(Msg::ScreenshotEmpty));
        open_ocr(db, st, recent, Err("camera roll locked".into()), None, None);
        assert_eq!(*st.toast.peek(), Some(Msg::OcrFailed));
        assert!(st.open.peek().is_none() && recent.peek().is_empty());
    });
    assert!(text_of(&s.settle()).contains(msg(EN, Msg::OcrFailed)));
}

#[test]
fn the_ai_layout_replaces_the_on_device_one_unless_a_phrase_is_highlighted() {
    let reply = include_str!("../../../../fixtures/ai/structure_screenshot.whatsapp_jv_chat.local.json");
    let reply: kabar_protocol::ai::EngineReply = serde_json::from_str(reply).unwrap();
    let mut case = reader();
    case.engine = EngineId::Local;
    case.ai = Some(Script::answering(&[&reply.text]));
    let mut s = Screenful::new(case);
    s.act(|db, st| {
        let ai = try_consume_context::<Ai>();
        open_ocr(db, st, Signal::new(Vec::new()), Ok(ocr_fixture()), None, ai);
    });
    s.settle();
    let layout = s.act(|db, st| {
        let id = st.open.peek().as_ref().unwrap().text.id.clone();
        db.0.borrow().text(&id).unwrap().unwrap().layout.unwrap()
    });
    let layout: serde_json::Value = serde_json::from_str(&layout).unwrap();
    assert_eq!(layout["structured_by"]["task"], "structure_screenshot");
    assert_eq!(layout["structured_by"]["engine"], "local");

    // A highlight is a byte range in the old body: the AI pass keeps away.
    let mut case = reader();
    case.engine = EngineId::Local;
    case.ai = Some(Script::answering(&[&reply.text]));
    let mut s = Screenful::new(case);
    s.act(|db, st| {
        open_ocr(db, st, Signal::new(Vec::new()), Ok(ocr_fixture()), None, None);
        st.save_phrase(db, &[0, 1]);
        let text = st.open.peek().as_ref().unwrap().text.clone();
        let ai = try_consume_context::<Ai>().unwrap();
        refine_with_ai(ai, db.clone(), st, Signal::new(Vec::new()), ocr_fixture(), text);
    });
    s.settle();
    let layout = s.act(|db, st| {
        let id = st.open.peek().as_ref().unwrap().text.id.clone();
        db.0.borrow().text(&id).unwrap().unwrap().layout.unwrap()
    });
    assert!(!layout.contains("structured_by"), "{layout}");
}

#[test]
fn a_failed_ai_layout_keeps_the_on_device_one() {
    let mut case = reader();
    case.engine = EngineId::Local;
    case.ai = Some(Script::failing(EngineError::Unavailable { reason: "Kabar Home asleep".into() }));
    let mut s = Screenful::new(case);
    s.act(|db, st| {
        let ai = try_consume_context::<Ai>();
        open_ocr(db, st, Signal::new(Vec::new()), Ok(ocr_fixture()), None, ai);
    });
    let html = s.settle();
    assert!(html.contains("seg bubble left"), "{html}");
    let layout = s.act(|db, st| {
        let id = st.open.peek().as_ref().unwrap().text.id.clone();
        db.0.borrow().text(&id).unwrap().unwrap().layout.unwrap()
    });
    assert!(!layout.contains("structured_by"));
}

// ---- Shared, typed and dropped input --------------------------------------

#[test]
fn shared_text_opens_with_its_source_and_typed_text_as_pasted() {
    let mut s = Screenful::new(reader());
    s.act(|db, st| {
        let busy = Signal::new(None);
        let recent = Signal::new(Vec::new());
        take_shared(
            db,
            st,
            busy,
            recent,
            SharedInput::Text { text: CHAT.into(), source: Some("WhatsApp".into()) },
        );
        let open = st.open.peek().clone().unwrap();
        assert_eq!((open.text.kind, open.text.source.as_deref()), (TextKind::Shared, Some("WhatsApp")));
        read_input(db, st, busy, recent, SAYA);
        let open = st.open.peek().clone().unwrap();
        assert_eq!((open.text.kind, open.text.body.as_str()), (TextKind::Pasted, SAYA));
        assert_eq!(recent.peek().len(), 2);
        // Blank input opens nothing.
        read_input(db, st, busy, recent, "  \n");
        assert_eq!(recent.peek().len(), 2);
    });
}

#[test]
fn a_shared_image_says_only_the_first_is_read() {
    let mut s = Screenful::new(reader());
    s.act(|db, st| {
        let image = SharedInput::Image {
            path: "/nonexistent/shot.png".into(),
            mime: Some("image/png".into()),
            source: None,
            more: 2,
        };
        take_shared(db, st, Signal::new(None), Signal::new(Vec::new()), image);
        let expected = if platform::ocr_available() { Msg::ShareFirstImageOnly } else { Msg::OcrUnavailable };
        assert_eq!(*st.toast.peek(), Some(expected));
    });
}

/// A one-request HTTP server on localhost: the Reader fetches pages
/// natively, and this keeps the test off the network.
fn serve_once(status: &str, content_type: &str, body: &str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    std::thread::spawn(move || {
        if let Ok((mut conn, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = conn.read(&mut buf);
            let _ = conn.write_all(reply.as_bytes());
        }
    });
    format!("http://127.0.0.1:{port}/berita")
}

#[test]
fn a_shared_link_is_fetched_and_its_article_opens() {
    let story = "Saya sedang membaca buku tentang Yogyakarta. ".repeat(8);
    let url = serve_once(
        "200 OK",
        "text/html; charset=utf-8",
        &format!(
            "<html><head><title>Berita Hari Ini</title></head><body><nav>Beranda</nav><article>{story}</article></body></html>"
        ),
    );
    let mut s = Screenful::new(reader());
    let busy = s.act(|db, st| {
        let busy = Signal::new(None);
        take_shared(
            db,
            st,
            busy,
            Signal::new(Vec::new()),
            SharedInput::Text { text: url.clone(), source: None },
        );
        assert_eq!(*busy.peek(), Some(Msg::UrlFetching));
        busy
    });
    for _ in 0..40 {
        s.settle();
        if s.act(|_, st| st.open.peek().is_some()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let open = s.act(|_, st| st.open.peek().clone()).expect("the page opened");
    assert_eq!(open.text.kind, TextKind::Web);
    assert_eq!(open.text.title, "Berita Hari Ini");
    assert!(open.text.body.contains("Yogyakarta") && !open.text.body.contains("Beranda"));
    assert_eq!(open.text.source.as_deref(), Some(url.as_str()));
    assert_eq!(*busy.peek(), None);
}

#[test]
fn a_page_that_cannot_be_read_asks_for_the_text() {
    for (status, kind, body) in [
        ("404 Not Found", "text/html", "<p>gone</p>"),
        ("200 OK", "image/png", "PNG"),
        ("200 OK", "text/html", "<html><body><script>x()</script></body></html>"),
    ] {
        let url = serve_once(status, kind, body);
        assert!(web::fetch_page(&url).is_err(), "{status} {kind}");
    }
    let url = serve_once("200 OK", "text/plain", "Kabar baik.");
    assert_eq!(web::fetch_page(&url).unwrap().text, "Kabar baik.");

    let url = serve_once("500 Internal Server Error", "text/html", "");
    let mut s = Screenful::new(reader());
    s.act(|db, st| read_input(db, st, Signal::new(None), Signal::new(Vec::new()), &url));
    for _ in 0..40 {
        s.settle();
        if s.act(|_, st| st.toast.peek().is_some()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(s.act(|_, st| *st.toast.peek()), Some(Msg::UrlImportFailed));
    assert!(s.act(|_, st| st.open.peek().is_none()));
}

/// A picked or dropped file, as the WebView hands it over.
struct Picked(&'static str, Option<&'static str>);

impl dioxus::html::NativeFileData for Picked {
    fn name(&self) -> String {
        self.0.into()
    }
    fn size(&self) -> u64 {
        3
    }
    fn last_modified(&self) -> u64 {
        0
    }
    fn path(&self) -> std::path::PathBuf {
        self.0.into()
    }
    fn content_type(&self) -> Option<String> {
        self.1.map(str::to_string)
    }
    fn read_bytes(
        &self,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = Result<dioxus::html::bytes::Bytes, dioxus::CapturedError>> + 'static>,
    > {
        Box::pin(async { Ok(dioxus::html::bytes::Bytes::from_static(b"png")) })
    }
    fn byte_stream(
        &self,
    ) -> std::pin::Pin<
        Box<
            dyn futures_util::Stream<Item = Result<dioxus::html::bytes::Bytes, dioxus::CapturedError>>
                + 'static
                + Send,
        >,
    > {
        Box::pin(futures_util::stream::empty())
    }
    fn read_string(
        &self,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<String, dioxus::CapturedError>> + 'static>> {
        Box::pin(async { Ok("png".to_string()) })
    }
    fn inner(&self) -> &dyn std::any::Any {
        self
    }
}

fn picked(name: &'static str, kind: Option<&'static str>) -> dioxus::html::FileData {
    dioxus::html::FileData::new(Picked(name, kind))
}

#[test]
fn images_are_known_by_type_or_extension() {
    assert!(is_image(&picked("shot.PNG", None)));
    assert!(is_image(&picked("photo.heic", None)));
    assert!(is_image(&picked("clipboard", Some("image/webp"))));
    assert!(!is_image(&picked("notes.txt", Some("text/plain"))));
    assert!(!is_image(&picked("README", None)));
}

#[test]
fn a_picked_image_is_copied_into_the_inbox_for_the_ocr() {
    let dir = tempfile::tempdir().unwrap();
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let path = rt.block_on(stash_image(&picked("Shot.JPG", None), Ok(dir.path().join("inbox")))).unwrap();
    assert_eq!(path.parent().unwrap(), dir.path().join("inbox"));
    assert_eq!(path.extension().unwrap(), "jpg");
    assert_eq!(std::fs::read(&path).unwrap(), b"png");
    let png =
        rt.block_on(stash_image(&picked("clipboard", Some("image/png")), Ok(dir.path().into()))).unwrap();
    assert_eq!(png.extension().unwrap(), "png", "no extension: a png");
    assert!(rt.block_on(stash_image(&picked("a.png", None), Err("no home directory".into()))).is_err());
}

#[test]
fn the_photo_picker_says_so_when_it_cannot_open() {
    if platform::NATIVE_IMAGE_PICKER {
        return;
    }
    let mut s = Screenful::new(reader());
    s.act(|_, st| {
        pick_native(st, consume_context::<Inbox>());
        assert_eq!(*st.toast.peek(), Some(Msg::PickFailed));
    });
}

#[test]
fn something_shared_while_the_reader_is_open_opens_by_itself() {
    let mut s = Screenful::new(reader());
    s.settle();
    let mut inbox = s.ctx::<Inbox>().0;
    inbox.set(Some(SharedInput::Text { text: SAYA.into(), source: Some("Telegram".into()) }));
    let html = s.settle();
    assert!(inbox.peek().is_none(), "taken once");
    assert!(html.contains("data-i=\"2\">membaca</span>"), "{html}");
    assert!(text_of(&html).contains("Telegram"), "{html}");
}

// ---- Messages from the WebView --------------------------------------------

#[test]
fn webview_messages_open_peek_and_close_the_right_layer() {
    let mut s = Screenful::new(reader());
    s.act(|db, st| {
        open_text(db, st, SAYA);
        let mut sheet = Signal::new(false);
        let peek = Peek { i: 2, start: 10.0, below: true, y: 40.0 };
        on_js(db, st, sheet, FromJs::Peek(peek));
        assert_eq!(*st.peek.peek(), Some(peek));
        on_js(db, st, sheet, FromJs::Unpeek);
        assert_eq!(*st.peek.peek(), None);
        on_js(db, st, sheet, FromJs::Open { i: 2 });
        assert_eq!(*st.panel.peek(), Some(Panel::Word(2)));
        // The card is open: hovering the same word doesn't repeat it.
        on_js(db, st, sheet, FromJs::Peek(peek));
        assert_eq!(*st.peek.peek(), None);
        on_js(db, st, sheet, FromJs::Phrase { tokens: vec![2, 3] });
        assert_eq!(*st.panel.peek(), Some(Panel::Phrase { tokens: vec![2, 3], save_first: false }));
        on_js(db, st, sheet, FromJs::Menu { tokens: vec![0] });
        assert_eq!(*st.panel.peek(), Some(Panel::Phrase { tokens: vec![0], save_first: true }));
        on_js(db, st, sheet, FromJs::Highlight { tokens: vec![2, 3] });
        assert_eq!(st.highlights.peek()[0].phrase, "membaca buku");
        // Escape closes the reading settings first, then the card.
        sheet.set(true);
        on_js(db, st, sheet, FromJs::Escape);
        assert!(!*sheet.peek());
        assert!(st.panel.peek().is_some());
        on_js(db, st, sheet, FromJs::Escape);
        assert!(st.panel.peek().is_none());
    });
}

#[test]
fn js_messages_parse_as_reader_js_sends_them() {
    let m: FromJs =
        serde_json::from_str(r#"{"type":"peek","i":3,"start":1.5,"below":false,"y":20}"#).unwrap();
    assert!(matches!(m, FromJs::Peek(Peek { i: 3, below: false, .. })));
    let m: FromJs = serde_json::from_str(r#"{"type":"menu","tokens":[1,2]}"#).unwrap();
    assert!(matches!(m, FromJs::Menu { tokens } if tokens == [1, 2]));
    assert!(matches!(serde_json::from_str(r#"{"type":"escape"}"#).unwrap(), FromJs::Escape));
}

#[test]
fn the_peek_floats_by_the_word_with_its_meaning() {
    let mut s =
        Screenful::new(with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, "Kalimat ini panjang.")));
    s.settle();
    s.act(|_, st| {
        let mut peek = st.peek;
        peek.set(Some(Peek { i: 0, start: 12.0, below: false, y: 30.0 }));
    });
    let html = s.settle();
    assert!(
        html.contains(
            "class=\"peek\" role=\"tooltip\" style=\"inset-inline-start: 12px; inset-block-end: 30px\""
        ),
        "{html}"
    );
    let peek = &html[html.find("class=\"peek\"").unwrap()..];
    let text = text_of(peek);
    assert!(text.contains("sentence"), "{text}");
    assert!(text.contains(msg(EN, Msg::FalseFriend)), "{text}");
}

#[test]
fn a_peek_on_an_unknown_word_says_there_is_no_local_meaning() {
    let html = render(with_open(
        || rsx! { card::PeekView { peek: Peek { i: 0, start: 0.0, below: true, y: 0.0 } } },
        |db, st| open_text(db, st, "Zxqvbn."),
    ));
    assert!(text_of(&html).contains(msg(EN, Msg::NoLocalGloss)), "{html}");
    assert!(html.contains("inset-block-start"), "{html}");
}

// ---- Toasts, busy, sync ----------------------------------------------------

#[test]
fn a_busy_reader_says_what_it_is_doing_and_offers_a_saved_word() {
    let html = render(with_open(
        || {
            let busy = use_signal(|| Some(Msg::UrlFetching));
            let recent = use_signal(Vec::new);
            rsx! {
                if let Some(m) = *busy.read() {
                    p { "{msg(EN, m)}" }
                    wait_word::WaitWord {}
                }
                InputCard { busy, recent }
            }
        },
        |db, _| {
            db.0.borrow_mut().save_word("id:baca", "membaca", SAYA, None).unwrap();
        },
    ));
    let text = text_of(&html);
    assert!(text.contains(msg(EN, Msg::UrlFetching)), "{text}");
    assert!(text.contains(msg(EN, Msg::WaitWordPrompt)) && text.contains("membaca"), "{text}");
    assert!(text.contains(SAYA), "{text}");
}

#[test]
fn a_toast_shows_until_closed() {
    let html = render(with_open(
        || rsx! { Reader {} },
        |_, st| {
            let mut t = st.toast;
            t.set(Some(Msg::ReaderWriteFailed));
        },
    ));
    assert!(html.contains("class=\"toast\" role=\"status\""), "{html}");
    assert!(text_of(&html).contains(msg(EN, Msg::ReaderWriteFailed)), "{html}");
    assert!(html.contains(&format!("aria-label=\"{}\"", msg(EN, Msg::Close))), "{html}");
}

#[test]
fn marks_synced_from_another_device_recolour_the_open_text() {
    let mut s = Screenful::new(with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, SAYA)));
    s.settle();
    s.act(|db, _| db.0.borrow_mut().set_word_status("id:buku", WordStatus::Learning).unwrap());
    let mut cloud = s.ctx::<Cloud>();
    cloud.revision.set(1);
    let html = s.settle();
    assert!(html.contains("class=\"w st-learning wc-ink wc-noun\" data-i=\"3\""), "{html}");
}

#[test]
fn closing_the_text_returns_to_the_library() {
    let mut s = Screenful::new(with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, SAYA)));
    s.settle();
    s.act(|_, st| st.close());
    let html = s.settle();
    assert!(html.contains("reader-textarea"), "{html}");
    assert!(s.act(|_, st| st.grammar.peek().is_none() && st.panel.peek().is_none()));
}

// ---- The word card ---------------------------------------------------------

fn open_word(word: &'static str) -> Case {
    thread_local! { static WORD: std::cell::Cell<&'static str> = const { std::cell::Cell::new("") }; }
    WORD.with(|w| w.set(word));
    with_open(
        || rsx! { Reader {} },
        |db, st| {
            let word = WORD.with(std::cell::Cell::get);
            open_text(db, st, &format!("{SAYA} Rumahnya dibersihkan. Kalimat itu {word}."));
            let i = token(&st, word);
            let mut p = st.panel;
            p.set(Some(Panel::Word(i)));
        },
    )
}

#[test]
fn the_word_card_explains_the_word_in_its_sentence() {
    let html = render(open_word("membaca"));
    let text = text_of(&html);
    assert!(html.contains("read-layout with-panel"), "{html}");
    assert!(html.contains("class=\"w st-new wc-ink wc-verb selected\""), "{html}");
    assert!(html.contains(&format!("aria-label=\"{}\"", msg(EN, Msg::WordSheet))), "{html}");
    for m in [
        Msg::Breakdown,
        Msg::MorphMore,
        Msg::InThisSentence,
        Msg::ExplainWord,
        Msg::AddToReview,
        Msg::TeachKabar,
        Msg::Listen,
        Msg::StatusNew,
        Msg::StatusKnown,
        Msg::LangIndonesian,
    ] {
        assert!(text.contains(msg(EN, m)), "{m:?}: {text}");
    }
    assert!(text.contains("to read"), "{text}");
    assert!(html.contains("aria-pressed=\"true\">New</button>"), "the current status is pressed: {html}");
    assert!(html.contains(&format!("aria-label=\"{}\"", msg(EN, Msg::Helpful))), "{html}");
}

#[test]
fn the_word_card_names_the_rules_the_word_is_part_of() {
    let text = text_of(&render(open_word("itu")));
    let card = &text[text.find(msg(EN, Msg::GrammarForWord)).expect("a rule for itu")..];
    assert!(card.contains("this and that"), "{card}");
}

#[test]
fn the_word_card_of_a_false_friend_shows_the_bridge_and_the_teach_draft() {
    let html = render(open_word("Kalimat"));
    let text = text_of(&html);
    assert!(html.contains("bridge-card false"), "{html}");
    assert!(text.contains("كلمة") || text.contains("كَلِمَة"), "{text}");
    assert!(text.contains(msg(EN, Msg::BridgeTitle)), "{text}");
}

#[test]
fn an_unknown_word_offers_teaching_kabar_first() {
    let html = render(open_word("zxqvbn"));
    let text = text_of(&html);
    assert!(text.contains(msg(EN, Msg::NoLocalGloss)), "{text}");
    assert!(
        html.contains(
            &format!("class=\"primary\" title=\"{}\"", msg(EN, Msg::TeachKabarHint)).replace('\'', "&#39;")
        ),
        "{html}"
    );
}

#[test]
fn a_word_taught_to_kabar_is_badged_and_shown_first() {
    let mut case = with_open(
        || rsx! { Reader {} },
        |db, st| {
            let mut t: kabar_protocol::learn::TaughtEntry =
                serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json")).unwrap();
            t.entry.status = kabar_protocol::content::ReviewStatus::Approved;
            db.0.borrow_mut().save_taught(&t).unwrap();
            open_text(db, st, "Kalimat itu panjang.");
            let mut p = st.panel;
            p.set(Some(Panel::Word(0)));
        },
    );
    case.lang = UiLanguage::ArEg;
    let html = render(case);
    let text = text_of(&html);
    assert!(text.contains(msg(UiLanguage::ArEg, Msg::TaughtByYou)), "{text}");
    assert!(text.contains(msg(UiLanguage::ArEg, Msg::KabarDictionary)), "{text}");
    assert!(html.contains("taught-block"), "{html}");
}

#[test]
fn a_draft_waiting_for_review_links_to_teach_kabar() {
    let html = render(with_open(
        || rsx! { Reader {} },
        |db, st| {
            let t: kabar_protocol::learn::TaughtEntry =
                serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json")).unwrap();
            db.0.borrow_mut().save_taught(&t).unwrap();
            open_text(db, st, "Kalimat itu panjang.");
            let mut p = st.panel;
            p.set(Some(Panel::Word(0)));
        },
    ));
    let text = text_of(&html);
    assert!(text.contains(msg(EN, Msg::DraftWaiting)), "{text}");
    assert!(!text.contains(msg(EN, Msg::TeachKabar)), "a draft is already waiting: {text}");
}

#[test]
fn a_word_saved_before_says_so_on_the_card() {
    let mut case = open_word("buku");
    case.prep = |db, st| {
        db.0.borrow_mut().save_word("id:buku", "buku", SAYA, None).unwrap();
        open_text(db, st, SAYA);
        let mut p = st.panel;
        p.set(Some(Panel::Word(3)));
    };
    let html = render(case);
    let added = msg(EN, Msg::AddedToReview).replace('\'', "&#39;");
    assert!(html.contains(&format!("class=\"primary\" disabled=true>{added}")), "{html}");
}

#[test]
fn adding_a_word_to_review_turns_it_learning_and_survives_a_refused_write() {
    let mut s = Screenful::new(with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, SAYA)));
    s.act(|db, st| {
        let info = st.open.peek().as_ref().unwrap().doc.tokens[2].word.as_ref().unwrap().shown();
        let id = st.open.peek().as_ref().unwrap().text.id.clone();
        db.0.borrow().refuse_writes();
        assert!(!card::add_word(db, st, "id:membaca", "membaca", SAYA, &id, info.as_ref()));
        assert_eq!(st.status("id:membaca"), WordStatus::New);
        assert_eq!(*st.toast.peek(), Some(Msg::ReaderWriteFailed));
        db.0.borrow().allow_writes();
        assert!(card::add_word(db, st, "id:membaca", "membaca", SAYA, &id, info.as_ref()));
        assert_eq!(st.status("id:membaca"), WordStatus::Learning);
        assert_eq!(db.0.borrow().saved_word_count("id:membaca").unwrap(), 1);
    });
    let html = s.settle();
    assert!(html.contains("class=\"w st-learning wc-ink wc-verb\" data-i=\"2\""), "{html}");
}

#[test]
fn status_and_feedback_are_stored_and_a_refusal_changes_nothing() {
    let mut s = Screenful::new(with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, SAYA)));
    s.act(|db, st| {
        st.set_status(db, "id:buku", WordStatus::Known);
        assert_eq!(db.0.borrow().word_statuses(["id:buku"]).unwrap()["id:buku"], WordStatus::Known);
        assert!(card::send_feedback(db, st, "id:buku", true));
        db.0.borrow().refuse_writes();
        st.set_status(db, "id:buku", WordStatus::Ignored);
        assert_eq!(st.status("id:buku"), WordStatus::Known);
        assert!(!card::send_feedback(db, st, "id:buku", false));
        assert_eq!(*st.toast.peek(), Some(Msg::ReaderWriteFailed));
    });
}

#[test]
fn teaching_a_word_opens_teach_kabar_with_the_sentence() {
    let mut s = Screenful::new(reader());
    s.act(|_, _| {
        let ui = consume_context::<Ui>();
        let inbox = consume_context::<crate::teach::TeachInbox>();
        let seed = crate::teach::TeachSeed {
            lang: kabar_protocol::content::Lang::Id,
            word: "mantul".into(),
            context: Some("Mantul banget!".into()),
        };
        card::teach_word(ui, inbox, seed.clone());
        assert_eq!(*ui.screen.peek(), crate::Screen::Teach);
        assert_eq!(inbox.0.peek().as_ref(), Some(&seed));
    });
}

// ---- Phrases and "Explain this sentence" -----------------------------------

#[test]
fn a_selected_phrase_can_be_saved_and_then_removed() {
    let mut s = Screenful::new(with_open(
        || rsx! { Reader {} },
        |db, st| {
            open_text(db, st, SAYA);
            let mut p = st.panel;
            p.set(Some(Panel::Phrase { tokens: vec![2, 3], save_first: true }));
        },
    ));
    let html = s.settle();
    let text = text_of(&html);
    assert!(html.contains(&format!("class=\"primary\">{}", msg(EN, Msg::SavePhrase))), "{html}");
    assert!(text.contains("membaca buku") && text.contains(msg(EN, Msg::ExplainSentence)), "{text}");
    assert!(html.contains("selected\" data-i=\"2\"") && html.contains("selected\" data-i=\"3\""), "{html}");
    let id = s.act(|db, st| {
        st.save_phrase(db, &[2, 3]);
        st.highlights.peek()[0].id.clone()
    });
    let html = s.settle();
    assert!(text_of(&html).contains(msg(EN, Msg::RemoveHighlight)), "{html}");
    s.act(|db, st| {
        db.0.borrow().refuse_writes();
        card::remove_highlight(db, st, &id);
        assert_eq!(st.highlights.peek().len(), 1, "still saved");
        db.0.borrow().allow_writes();
        card::remove_highlight(db, st, &id);
        assert!(st.highlights.peek().is_empty());
        assert!(db.0.borrow().highlights(&st.open.peek().as_ref().unwrap().text.id).unwrap().is_empty());
    });
    assert!(text_of(&s.settle()).contains(msg(EN, Msg::SavePhrase)));
}

fn explain(ai: Option<std::rc::Rc<Script>>, lang: UiLanguage) -> String {
    let mut case = with_open(
        || rsx! { Reader {} },
        |db, st| {
            open_text(db, st, SAYA);
            let mut p = st.panel;
            p.set(Some(Panel::Explain(2)));
        },
    );
    case.ai = ai;
    case.lang = lang;
    render(case)
}

#[test]
fn explain_this_sentence_without_an_engine_lists_the_local_grammar() {
    let html = explain(None, EN);
    let text = text_of(&html);
    assert!(text.contains(msg(EN, Msg::ExplainNeedsCloud)), "{text}");
    assert!(text.contains(msg(EN, Msg::GrammarInSentence)), "{text}");
    assert!(text.contains("Time without tenses"), "{text}");
    // The whole sentence is selected.
    assert!(html.contains("selected\" data-i=\"0\"") && html.contains("selected\" data-i=\"3\""), "{html}");
}

#[test]
fn explain_this_sentence_puts_the_engines_picks_first() {
    // The recorded answer speaks Egyptian Arabic, the learner's voice here.
    const AR: UiLanguage = UiLanguage::ArEg;
    let answer =
        include_str!("../../../../fixtures/ai/explain_sentence.saya_sedang_membaca_buku.workers_ai.json");
    let html = explain(Some(Script::answering(&[answer])), AR);
    let text = text_of(&html);
    assert!(text.contains("أنا بقرا كتاب دلوقتي."), "the paraphrase: {text}");
    assert!(
        text.contains(msg(AR, Msg::InOtherWords)) && text.contains(msg(AR, Msg::RegisterNeutral)),
        "{text}"
    );
    assert!(text.contains(msg(AR, Msg::Unverified)), "a provisional route stays unverified: {text}");
    let card = &text[text.find("أنا بقرا كتاب").unwrap()..];
    let (aspect, men) = (card.find("sedang").unwrap(), card.find("meN-").unwrap());
    assert!(aspect < men, "the engine's order: {card}");
}

#[test]
fn an_explanation_in_the_wrong_voice_is_refused_kindly() {
    let answer =
        include_str!("../../../../fixtures/ai/explain_sentence.saya_sedang_membaca_buku.workers_ai.json");
    let text = text_of(&explain(Some(Script::answering(&[answer])), EN));
    assert!(!text.contains("أنا بقرا كتاب"), "{text}");
    assert!(text.contains(msg(EN, Msg::GrammarInSentence)), "the offline grammar still shows: {text}");
}

#[test]
fn a_failed_explanation_says_why_kindly() {
    let html = explain(Some(Script::failing(EngineError::QuotaExhausted { retry_after_s: Some(60) })), EN);
    assert!(text_of(&html).contains(msg(EN, Msg::ExplainResetsLater)), "{html}");
}

// ---- Translation under each line -------------------------------------------

fn translated(
    settings: fn(&mut kabar_core::settings::Settings),
    ai: Option<std::rc::Rc<Script>>,
    signed_in: bool,
) -> String {
    let mut case =
        with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, "arp dicul piro\n\nSek tak lapor pusat"));
    case.settings = settings;
    case.ai = ai;
    case.signed_in = signed_in;
    render(case)
}

#[test]
fn lines_get_their_translation_in_the_learners_voice() {
    let answer = include_str!("../../../../fixtures/ai/translate_text.arp_dicul_piro.workers_ai.json");
    let html = translated(|_| {}, Some(Script::answering(&[answer])), true);
    let text = text_of(&html);
    assert!(text.contains("How much will it be sold for?"), "{text}");
    assert!(text.contains("Hold on, I'll report to HQ."), "{text}");
    assert!(text.contains(msg(EN, Msg::TranslationByAi)), "{text}");
    assert!(text.contains(msg(EN, Msg::Unverified)), "Javanese stays unverified: {text}");
}

#[test]
fn signed_out_lines_fall_back_to_word_by_word() {
    let answer = include_str!("../../../../fixtures/ai/translate_text.arp_dicul_piro.workers_ai.json");
    let text = text_of(&translated(|_| {}, Some(Script::answering(&[answer])), false));
    assert!(text.contains(msg(EN, Msg::TranslationNeedsCloud)), "{text}");
    assert!(text.contains("let go how much"), "{text}");
}

#[test]
fn a_failed_translation_can_be_tried_again() {
    let html =
        translated(|_| {}, Some(Script::failing(EngineError::RateLimited { retry_after_s: None })), true);
    let text = text_of(&html);
    assert!(text.contains(msg(EN, Msg::TranslationRetry)), "{text}");
    assert!(text.contains(msg(EN, Msg::TranslationWordByWordShort)), "{text}");
}

#[test]
fn translation_off_or_on_tap_hides_the_lines() {
    let text = text_of(&translated(|s| s.translation = TranslationMode::Off, None, false));
    assert!(!text.contains(msg(EN, Msg::TranslationTitle)), "{text}");
    assert!(!text.contains(msg(EN, Msg::TranslationWordByWordShort)), "{text}");
    let text = text_of(&translated(|s| s.translation = TranslationMode::Tap, None, false));
    assert!(text.contains("What does it mean?"), "{text}");
    assert!(!text.contains(msg(EN, Msg::TranslationWordByWordShort)), "{text}");
}

#[test]
fn grammar_hints_off_hides_the_rule_chips() {
    let text = text_of(&render({
        let mut case = with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, SAYA));
        case.settings = |s| s.grammar_hints_off = true;
        case
    }));
    assert!(text.contains(msg(EN, Msg::GrammarInText)), "the text's list stays: {text}");
    let html = render({
        let mut case = open_word("membaca");
        case.settings = |s| s.grammar_hints_off = true;
        case
    });
    assert!(!html.contains("line-grammar") && !html.contains("word-grammar"), "{html}");
}

#[test]
fn friendly_failures_name_their_reason() {
    use kabar_core::ai::FriendlyReason::*;
    let pairs = [
        (ResetsLater, Msg::TranslationResetsLater, Msg::ExplainResetsLater),
        (Unreachable, Msg::TranslationUnreachable, Msg::ExplainUnreachable),
        (NotAvailable, Msg::TranslationNeedsCloud, Msg::ExplainNeedsCloud),
        (TryAgain, Msg::TranslationTryAgain, Msg::ExplainTryAgain),
    ];
    for (r, tr, ex) in pairs {
        assert_eq!(translate::failure(r), tr);
        assert_eq!(explain::friendly(r), ex);
    }
}

// ---- Reading settings ------------------------------------------------------

fn bar(width: WidthClass, open: bool) -> String {
    thread_local! { static OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    OPEN.with(|o| o.set(open));
    let mut case = Case::new(|| {
        let sheet = use_signal(|| OPEN.with(std::cell::Cell::get));
        rsx! { reading::OpenBar { sheet } }
    });
    case.width = width;
    render(case)
}

#[test]
fn the_reading_settings_offer_size_spacing_and_width_where_there_is_room() {
    let html = bar(WidthClass::Expanded, true);
    let text = text_of(&html);
    assert!(html.contains("reader-bar-wrap open") && html.contains("read-settings-scrim"), "{html}");
    for m in [
        Msg::ReadSize,
        Msg::ReadSpacing,
        Msg::ReadWidth,
        Msg::ReadSnug,
        Msg::ReadAiry,
        Msg::ReadBook,
        Msg::Voice,
    ] {
        assert!(text.contains(msg(EN, m)), "{m:?}: {text}");
    }
    assert!(
        html.contains(&format!("aria-pressed=\"true\" aria-label=\"{}\"", msg(EN, Msg::ReadNormal))),
        "{html}"
    );
    assert!(text.contains("Apa kabar? Kabar baik, terima kasih."), "{text}");
    // A phone: no width choice and no focus mode.
    let html = bar(WidthClass::Compact, true);
    assert!(
        !text_of(&html).contains(msg(EN, Msg::ReadWidth)) && !html.contains(msg(EN, Msg::ReadFocus)),
        "{html}"
    );
    let html = bar(WidthClass::Compact, false);
    assert!(!html.contains("id=\"read-settings\"") && html.contains("aria-expanded=\"false\""), "{html}");
}

#[test]
fn focus_mode_closes_the_settings_and_is_kept_per_device() {
    let mut s = Screenful::new(reader());
    s.act(|db, _| {
        let prefs = consume_context::<crate::settings::device::DevicePrefs>();
        let sheet = Signal::new(true);
        let r = *prefs.reading.peek();
        reading::toggle_focus(db, prefs, sheet, r);
        assert!(!*sheet.peek());
        assert!(prefs.reading.peek().focus);
        assert!(crate::settings::device::DevicePrefs::load(db).reading.peek().focus, "stored");
    });
    s.act(|db, st| open_text(db, st, SAYA));
    let html = s.settle();
    assert!(html.contains("class=\"reader focus\""), "{html}");
    assert!(html.contains(msg(EN, Msg::ReadFocusLeave)), "{html}");
}

#[test]
fn the_voice_picker_marks_the_voice_in_use() {
    let mut case = Case::new(|| rsx! { reading::VoicePicker {} });
    case.lang = UiLanguage::Ar;
    let html = render(case);
    assert!(html.contains("lang=\"ar\" aria-pressed=\"true\""), "{html}");
    assert!(html.contains("lang=\"en\" aria-pressed=\"false\""), "{html}");
}

// ---- More of the word card, the library and the store rules ----------------

fn gloss(s: &str) -> glossary::Gloss {
    glossary::Gloss { arz: format!("{s} (arz)"), ar: format!("{s} (ar)"), en: s.into() }
}

#[test]
fn a_javanese_entry_shows_its_speech_levels_and_its_indonesian() {
    thread_local! { static INFO: std::cell::RefCell<Option<WordInfo>> = const { std::cell::RefCell::new(None) }; }
    INFO.with(|i| {
        *i.borrow_mut() = Some(WordInfo {
            lemma: "mangan".into(),
            lang: glossary::WordLang::Javanese,
            pos: Some("verb".into()),
            gloss: gloss("to eat"),
            indonesian: Some("makan".into()),
            bridge: None,
            levels: Some(glossary::Levels {
                level: Some(glossary::SpeechLevel::Ngoko),
                ngoko: vec!["mangan".into()],
                krama: vec!["nedha".into()],
                krama_inggil: vec!["dhahar".into(), "nedha".into()],
                note: Some(gloss("dhahar for elders")),
                ..Default::default()
            }),
            note: Some(gloss("everyday word")),
            example: Some(("Aku wis mangan.".into(), gloss("I've eaten."))),
            verified: false,
        })
    });
    let mut case = Case::new(|| {
        let info = INFO.with(|i| i.borrow().clone().unwrap());
        rsx! { card::InfoView { info, voice: UiLanguage::ArEg } }
    });
    case.lang = UiLanguage::ArEg;
    let html = render(case);
    let text = text_of(&html);
    const AR: UiLanguage = UiLanguage::ArEg;
    assert!(text.contains("to eat (arz)") && text.contains("to eat"), "the voice, then English: {text}");
    assert!(text.contains(msg(AR, Msg::InIndonesian)) && text.contains("makan"), "{text}");
    assert!(text.contains("everyday word (arz)"), "{text}");
    assert!(text.contains("Aku wis mangan.") && text.contains("I've eaten. (arz)"), "{text}");
    assert!(
        text.contains(&format!("{} · {}", msg(AR, Msg::SpeechLevels), msg(AR, Msg::LevelNgoko))),
        "{text}"
    );
    assert!(html.contains(&format!("<dt class=\"current\">{}</dt>", msg(AR, Msg::LevelNgoko))), "{html}");
    assert!(text.contains("dhahar / nedha"), "{text}");
    assert!(!text.contains(msg(AR, Msg::LevelMadya)), "no madya forms, no row: {text}");
    assert!(text.contains("dhahar for elders (arz)"), "{text}");
    assert!(html.contains("lang=\"jv\""), "{html}");
}

#[test]
fn the_part_of_speech_is_not_repeated_under_its_chip() {
    // #603: «تفصيل الكلمة [اسم]» then «اسم» again over the meaning.
    fn noun() -> WordInfo {
        WordInfo {
            lemma: "perpustakaan".into(),
            lang: glossary::WordLang::Indonesian,
            pos: Some("noun".into()),
            gloss: gloss("library"),
            indonesian: None,
            bridge: None,
            levels: None,
            note: None,
            example: None,
            verified: false,
        }
    }
    let ar = |chip: Option<&'static str>| {
        thread_local! { static CHIP: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) }; }
        CHIP.with(|c| c.set(chip));
        let mut case = Case::new(|| {
            let chip = CHIP.with(|c| c.get()).map(str::to_string);
            rsx! { card::InfoView { info: noun(), voice: UiLanguage::ArEg, chip } }
        });
        case.lang = UiLanguage::ArEg;
        text_of(&render(case))
    };
    let label = crate::i18n::part_of_speech(UiLanguage::ArEg, "noun");
    assert!(ar(None).contains(label), "no chip: the gloss names its part of speech");
    assert!(!ar(Some("اسم")).contains(label), "the chip already says it");
    assert!(ar(Some("فعل")).contains(label), "a different chip keeps it");
}

#[test]
fn every_label_has_its_own_words() {
    use std::collections::HashSet;
    let tags: HashSet<_> =
        [doc::Tag::Indonesian, doc::Tag::Javanese, doc::Tag::English, doc::Tag::Proper, doc::Tag::Unknown]
            .map(|x| msg(EN, card::tag_msg(x)))
            .into();
    assert_eq!(tags.len(), 5);
    let statuses: HashSet<_> = WordStatus::ALL.map(|x| msg(EN, card::status_msg(x))).into();
    assert_eq!(statuses.len(), WordStatus::ALL.len());
    let shifts: HashSet<_> =
        [Shift::Same, Shift::Narrowed, Shift::Broadened, Shift::Shifted, Shift::FalseFriend]
            .map(|x| msg(EN, card::shift_msg(x)))
            .into();
    assert_eq!(shifts.len(), 5);
    let levels: HashSet<_> = glossary::SpeechLevel::ALL.map(|x| msg(EN, card::level_msg(x))).into();
    assert_eq!(levels.len(), 5);
    assert_eq!(card::shift_msg(Shift::FalseFriend), Msg::FalseFriend);
}

#[test]
fn a_review_card_keeps_the_bridge_and_how_far_the_meaning_moved() {
    let (_, text) = {
        let mut store = Store::open_in_memory().unwrap();
        let t = store.save_text(SAYA, TextKind::Pasted, None, None).unwrap();
        (store, t)
    };
    for (shift, expect) in [
        (Shift::Narrowed, MeaningShift::Same),
        (Shift::Shifted, MeaningShift::Shifted),
        (Shift::FalseFriend, MeaningShift::FalseFriend),
    ] {
        let info = WordInfo {
            lemma: "kabar".into(),
            lang: glossary::WordLang::Indonesian,
            pos: None,
            gloss: gloss("news"),
            indonesian: None,
            bridge: Some(glossary::Bridge {
                arabic: "خبر".into(),
                root: Some("خ ب ر".into()),
                shift,
                note: Some(gloss("same word")),
            }),
            levels: None,
            note: Some(gloss("a greeting too")),
            example: None,
            verified: true,
        };
        let c = reader_card("id:kabar", " Kabar ", Some(&info), " Apa kabar? ", &text.id, &[]).unwrap();
        let a = c.arabic.unwrap();
        assert_eq!((a.word.as_str(), a.root.as_deref(), a.shift), ("خبر", Some("خ ب ر"), expect));
        assert_eq!(a.note.unwrap().en, "same word");
        assert_eq!(c.note.unwrap().ar_eg, "a greeting too (arz)");
        assert_eq!((c.word.as_str(), c.example.unwrap().text.as_str()), ("kabar", "Apa kabar?"));
    }
    let blank = WordInfo {
        gloss: glossary::Gloss { arz: String::new(), ar: String::new(), en: String::new() },
        ..WordInfo {
            lemma: "x".into(),
            lang: glossary::WordLang::Indonesian,
            pos: None,
            gloss: gloss("x"),
            indonesian: None,
            bridge: None,
            levels: None,
            note: None,
            example: None,
            verified: true,
        }
    };
    assert!(reader_card("id:x", "x", Some(&blank), "x", &text.id, &[]).is_none(), "nothing to recall");
    assert!(reader_card("id:x", "  ", Some(&blank), "x", &text.id, &[]).is_none());
}

#[test]
fn a_phrase_that_cannot_be_saved_leaves_no_highlight() {
    let mut s = Screenful::new(with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, SAYA)));
    s.act(|db, st| {
        db.0.borrow().refuse_writes();
        st.save_phrase(db, &[2, 3]);
        assert!(st.highlights.peek().is_empty());
        assert_eq!(*st.toast.peek(), Some(Msg::ReaderWriteFailed));
        assert_eq!(st.highlight_at(2), None);
        db.0.borrow().allow_writes();
        st.save_phrase(db, &[2, 3]);
        assert_eq!(st.highlight_at(3).map(|h| h.phrase), Some("membaca buku".to_string()));
        assert_eq!(st.highlight_at(0), None);
    });
}

#[test]
fn english_in_a_chat_needs_no_gloss_chip() {
    let html = render(with_open(
        || rsx! { Reader {} },
        |db, st| open_text(db, st, "Which is, gue literally capek banget."),
    ));
    let explain = &html[html.find("class=\"explain\"").unwrap()..];
    let explain = &explain[..explain.find("</div>").unwrap()];
    assert!(!explain.contains(">literally<"), "{explain}");
    assert!(explain.contains(">capek<") && explain.contains("tired"), "{explain}");
}

#[test]
fn the_library_strings_exist_in_all_three_voices() {
    use library::L;
    let all = [
        L::Title,
        L::Intro,
        L::Search,
        L::Texts,
        L::Words,
        L::Downloads,
        L::NoTexts,
        L::NoTextsPasteOnly,
        L::NoMatch,
        L::ClearSearch,
        L::NoWords,
        L::AllSaved,
        L::Phrases,
        L::FilterSaved,
        L::BridgeTag,
        L::BridgeTitle,
        L::BridgeCount,
        L::Taught,
        L::TaughtNote,
        L::OpenInText,
        L::TextGone,
        L::Pasted,
        L::Shared,
        L::Screenshot,
        L::Web,
        L::Phrase,
        L::InReview,
    ];
    for l in all {
        for lang in UiLanguage::ALL {
            let s = library::lt(lang, l);
            let arabic = s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c));
            assert_eq!(arabic, lang != UiLanguage::En, "{l:?} {lang:?}: {s}");
        }
    }
    for kind in [TextKind::Pasted, TextKind::Shared, TextKind::Screenshot, TextKind::Web] {
        assert!(!kind_icon(kind).is_empty());
    }
}

fn words_tab(lang: UiLanguage, prep: fn(&Db, ReaderState)) -> String {
    let mut case = with_open(
        || {
            let db = use_context::<Db>();
            let words = use_hook(|| db.0.borrow().saved_words(50).unwrap());
            let taught = use_hook(|| db.0.borrow().taught_entries().unwrap().len());
            let query = use_signal(String::new);
            rsx! { library::WordList { words, taught, q: String::new(), query } }
        },
        prep,
    );
    case.lang = lang;
    render(case)
}

#[test]
fn saved_words_show_their_meaning_their_sentence_and_where_they_came_from() {
    let html = words_tab(EN, |db, _| {
        let mut s = db.0.borrow_mut();
        let text = s.save_text(SAYA, TextKind::Pasted, None, None).unwrap();
        s.save_word("id:membaca", "membaca", SAYA, Some(&text.id)).unwrap();
        s.set_word_status("id:membaca", WordStatus::Learning).unwrap();
        s.save_word("phrase:membaca buku", "membaca buku", SAYA, Some("gone")).unwrap();
        let t: kabar_protocol::learn::TaughtEntry =
            serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json")).unwrap();
        s.save_taught(&t).unwrap();
    });
    let text = text_of(&html);
    assert!(text.contains("to read"), "the meaning: {text}");
    assert!(text.contains(library::lt(EN, library::L::Phrase)), "a phrase says so: {text}");
    assert!(text.contains(library::lt(EN, library::L::OpenInText)), "{text}");
    assert!(text.contains(library::lt(EN, library::L::TextGone)), "{text}");
    assert!(text.contains(library::lt(EN, library::L::Taught)), "{text}");
    assert!(
        html.contains("lib-kind-filter") && text.contains(library::lt(EN, library::L::Phrases)),
        "{html}"
    );
    assert!(
        html.contains("status-chip accent") && text.contains(library::lt(EN, library::L::InReview)),
        "{html}"
    );
}

#[test]
fn an_empty_word_list_says_how_words_get_there() {
    let text = text_of(&words_tab(UiLanguage::ArEg, |_, _| {}));
    assert!(text.contains(library::lt(UiLanguage::ArEg, library::L::NoWords)), "{text}");
}

#[test]
fn a_search_with_no_match_offers_to_clear_it() {
    let mut case = Case::new(|| {
        let db = use_context::<Db>();
        let recent = use_signal(|| db.0.borrow().recent_texts(10).unwrap());
        let query = use_signal(|| "zzz".to_string());
        let words = use_hook(|| db.0.borrow().saved_words(50).unwrap());
        rsx! {
            library::TextList { recent, q: "zzz", query }
            library::WordList { words, taught: 0, q: "zzz", query }
        }
    });
    case.prep = |db, _| {
        let mut s = db.0.borrow_mut();
        s.save_text(SAYA, TextKind::Pasted, None, None).unwrap();
        s.save_word("id:buku", "buku", SAYA, None).unwrap();
    };
    let text = text_of(&render(case));
    assert_eq!(text.matches(library::lt(EN, library::L::NoMatch)).count(), 2, "{text}");
    assert_eq!(text.matches(library::lt(EN, library::L::ClearSearch)).count(), 2, "{text}");
}

#[test]
fn the_bridge_row_counts_the_words_an_arabic_speaker_knows() {
    let text = text_of(&render(Case::new(|| rsx! { library::BridgeEntry {} })));
    assert!(text.contains(library::lt(EN, library::L::BridgeTitle)), "{text}");
    assert!(text.contains(library::lt(EN, library::L::BridgeCount)), "{text}");
    assert!(text.chars().any(|c| c.is_ascii_digit()), "a count: {text}");
}

#[test]
fn the_word_list_reads_the_kabar_dictionary_once() {
    assert!(!lexicon().is_empty(), "every word of the content pack, for spelling suggestions");
}

// ---- The word breakdown ----------------------------------------------------

fn breakdown_of(text: &str, word: &str, voice: UiLanguage) -> morph::Shown {
    let mut s = Screenful::new(reader());
    let text = text.to_string();
    let word = word.to_string();
    s.act(move |db, st| {
        open_text(db, st, &text);
        morph::shown(&st, token(&st, &word), voice).expect("a breakdown")
    })
}

#[test]
fn the_breakdown_names_the_root_and_what_each_affix_does() {
    let s = breakdown_of("Rumahnya dibersihkan kemarin.", "dibersihkan", EN);
    let labels: Vec<_> = s.detail.parts.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["di-", "bersih", "-kan"]);
    assert_eq!(s.root_gloss.as_deref(), Some("clean"));
    assert_eq!(s.notes.len(), 2, "{:?}", s.notes);
    assert!(s.notes.iter().all(|(_, note, verified)| !note.is_empty() && *verified));
    assert_eq!((s.say.as_str(), s.tag), ("dibersihkan", "id"));
}

#[test]
fn the_breakdown_of_a_doubled_word_and_a_javanese_one() {
    let s = breakdown_of("Anak-anak bermain di taman.", "Anak-anak", EN);
    assert_eq!(s.redup.as_ref().map(|(_, d)| d.as_str()), Some("anak-anak"));
    let s = breakdown_of("Bukune ning meja.", "Bukune", UiLanguage::ArEg);
    assert_eq!(s.tag, "jv");
    assert!(
        s.notes.iter().any(|(_, _, verified)| !verified),
        "Javanese notes aren't reviewed: {:?}",
        s.notes
    );
    // A chat spelling is said in full.
    let s = breakdown_of("arp dicul piro", "arp", EN);
    assert_eq!(s.say, "arep");
}

#[test]
fn the_opened_breakdown_draws_parts_notes_time_words_and_grammar() {
    thread_local! { static SHOWN: std::cell::RefCell<Option<morph::Shown>> = const { std::cell::RefCell::new(None) }; }
    let shown = breakdown_of("Saya sedang membaca buku.", "membaca", EN);
    assert!(!shown.detail.aspect.is_empty(), "sedang is a time word around the verb");
    assert!(!shown.entries.is_empty(), "meN- has a grammar entry");
    SHOWN.with(|s| *s.borrow_mut() = Some(shown));
    let html = render(Case::new(|| {
        let s = SHOWN.with(|s| s.borrow().clone().unwrap());
        rsx! { morph::MorphBody { s, class: Some(kabar_core::grammar::word::WordClass::Verb), voice: UiLanguage::En } }
    }));
    let text = text_of(&html);
    assert!(html.contains("parts-line wc-verb"), "{html}");
    assert!(html.contains("piece wp-root") && text.contains("baca"), "{html}");
    assert!(text.contains(msg(EN, Msg::MorphTimeWords)) && text.contains("sedang"), "{text}");
    assert!(text.contains(msg(EN, Msg::MorphGrammar)) && html.contains("morph-entry"), "{text}");
    assert!(html.contains("colour-legend"), "{html}");
}

#[test]
fn only_words_with_something_to_break_down_get_the_toggle() {
    let html = render(open_word("buku"));
    assert!(html.contains("morph-detail wc-noun"), "{html}");
    let html = render(open_word("zxqvbn"));
    assert!(!html.contains("morph-detail"), "nothing known about it: {html}");
}

// ---- Odds and ends the screens rely on --------------------------------------

#[test]
fn grammar_standing_has_a_label_and_a_class_for_each_state() {
    use kabar_core::grammar::detect::Standing;
    let all = [Standing::Learned, Standing::AtLevel, Standing::Ahead, Standing::Unknown];
    let classes: std::collections::HashSet<_> = all.map(grammar::standing_class).into();
    assert_eq!(classes.len(), 4);
    assert_eq!(grammar::standing_msg(Standing::Ahead), Some(Msg::GrammarNotYet));
    assert_eq!(grammar::standing_msg(Standing::Learned), Some(Msg::GrammarLearned));
    assert_eq!(grammar::standing_msg(Standing::Unknown), None, "nothing to say without progress");
}

#[test]
fn registers_read_as_the_learner_knows_them() {
    use kabar_core::ai::tasks::Register::*;
    assert_eq!(explain::register_msg(Krama), Msg::RegisterKrama);
    assert_eq!(explain::register_msg(KramaInggil), Msg::RegisterKrama);
    assert_eq!(explain::register_msg(Casual), Msg::RegisterSlang);
    assert_eq!(explain::register_msg(Standard), Msg::RegisterNeutral);
    let all = [Chat, Formal, Neutral, Slang, Mixed, Ngoko, Krama];
    let shown: std::collections::HashSet<_> = all.map(|r| msg(EN, explain::register_msg(r))).into();
    assert_eq!(shown.len(), all.len());
    assert_eq!(explain::voice_of(UiLanguage::Ar), kabar_core::grammar::Voice::Ar);
}

#[test]
fn grammar_pieces_stay_away_while_no_text_is_open() {
    let html = render(Case::new(|| {
        rsx! {
            grammar::TextGrammar {}
            grammar::WordGrammar { i: 0 }
            grammar::LineGrammar { seg: 0 }
            explain::ExplainCard { token: 0 }
            card::PanelView {}
        }
    }));
    assert!(
        !html.contains("text-grammar") && !html.contains("line-grammar") && !html.contains("word-card"),
        "{html}"
    );
    assert!(html.contains("class=\"panel\""), "only the empty sheet: {html}");
}

#[test]
fn a_text_with_no_grammar_shows_no_grammar_list() {
    let html = render(with_open(|| rsx! { Reader {} }, |db, st| open_text(db, st, "Zxqvbn qwrtp.")));
    assert!(!html.contains("text-grammar") && !html.contains("line-grammar"), "{html}");
}

#[test]
fn an_empty_phrase_selection_shows_nothing() {
    let html = render(with_open(
        || rsx! { card::PanelView {} },
        |db, st| {
            open_text(db, st, SAYA);
            st.show(Some(Panel::Phrase { tokens: vec![], save_first: false }));
        },
    ));
    assert!(!html.contains("word-card"), "{html}");
}

#[test]
fn a_chat_spelling_shows_its_full_form_in_the_headline() {
    let html = render(with_open(
        || rsx! { card::PanelView {} },
        |db, st| {
            open_text(db, st, "arp dicul piro");
            st.show(Some(Panel::Word(0)));
        },
    ));
    let head = &html[html.find("class=\"headline\"").unwrap()..];
    let head = text_of(&head[..head.find("</div>").unwrap()]);
    assert!(head.contains("arp") && head.contains("→ arep"), "{head}");
    assert!(head.contains(msg(EN, Msg::LangJavanese)), "{head}");
}

#[test]
fn a_culture_note_shows_when_the_engine_gives_one() {
    let answer =
        include_str!("../../../../fixtures/ai/explain_sentence.saya_sedang_membaca_buku.workers_ai.json");
    let mut v: serde_json::Value = serde_json::from_str(answer).unwrap();
    v["culture"] = "في إندونيسيا بيقولوا sedang في الكلام الرسمي أكتر.".into();
    let answer = v.to_string();
    let html = explain(Some(Script::answering(&[&answer])), UiLanguage::ArEg);
    let text = text_of(&html);
    assert!(text.contains(msg(UiLanguage::ArEg, Msg::CultureNote)), "{text}");
    assert!(text.contains("في الكلام الرسمي"), "{text}");
}

#[test]
fn reading_aloud_falls_back_to_the_webview_voice() {
    // Off Android there is no native engine: the WebView speaks, and the call
    // must not fail without one (no document in a test).
    let mut s = Screenful::new(reader());
    s.act(|_, _| {
        card::speak("Saya sedang membaca buku.", "id-ID");
        card::speak("Aku wis mangan.", "jv-ID");
    });
    assert_eq!(platform::NATIVE_TTS, cfg!(target_os = "android"));
}

#[test]
fn the_reader_tagline_reads_in_every_voice() {
    let lines: std::collections::HashSet<_> = UiLanguage::ALL.map(reading::tagline).into();
    assert_eq!(lines.len(), 3);
}

#[test]
fn the_drop_hint_only_promises_what_works() {
    // A phone has nothing to drag; the web reads no pictures yet.
    assert_eq!(drop_hint(true, true), None);
    assert_eq!(drop_hint(true, false), Some(Msg::ReaderDrop));
    assert_eq!(drop_hint(false, false), Some(Msg::ReaderDropText));
    assert!(!msg(EN, Msg::ReaderDropText).contains("image"));
}
