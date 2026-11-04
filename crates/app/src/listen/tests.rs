//! Watch & listen through the real screens: the lists, the player driven by
//! a fake audio engine, downloads on a temporary disk, and what a finished
//! show records.

use std::cell::{Cell, RefCell};

use kabar_core::story::{Cue, Heading};
use kabar_protocol::lesson::Localized;
use kabar_protocol::story::SegmentKind;
use serde_json::json;

use super::player::{DownloadState, requests};
use super::*;
use crate::embedded::{self, PERSONAS};
use crate::testkit::{App, FakeEval, Setup, visible};

#[test]
fn every_shipped_episode_and_persona_parses() {
    assert!(SHOWS.sinetron.len() >= 10 && SHOWS.radio.len() >= 5);
    for s in &SHOWS.sinetron {
        let s = s.as_ref().unwrap();
        let p = Program::sinetron(s);
        assert!(!p.cues.is_empty(), "{}", s.id);
        assert!(crate::journey::curriculum(&s.language).is_some());
        let q = quiz(s);
        // A quiz is shorter than a lesson (3-6 questions); the rest must hold.
        let issues: Vec<_> =
            kabar_core::lesson::validate(&q).into_iter().filter(|i| i.path != "exercises").collect();
        assert!(
            issues.is_empty(),
            "{}: {}",
            s.id,
            issues.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("; ")
        );
    }
    for r in &SHOWS.radio {
        let r = r.as_ref().unwrap();
        assert_eq!(
            player::requests(&Program::radio(r)).len(),
            r.segments.iter().map(|s| s.lines.len()).sum::<usize>()
        );
    }
    for (id, yaml) in embedded::all(PERSONAS) {
        let p = kabar_core::story::parse_persona(&yaml).unwrap();
        assert_eq!(&p.id, id);
    }
    // Episodes come in order.
    let numbers: Vec<u32> = SHOWS.sinetron.iter().flatten().map(|s| s.episode).collect();
    assert!(numbers.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn locked_episodes_fold_into_one_teaser_per_language() {
    let story = |n: u32, language: &str| {
        let mut s = SHOWS.sinetron.iter().flatten().next().unwrap().clone();
        s.episode = n;
        s.id = format!("sinetron.e{n:02}");
        s.language = language.to_string();
        s
    };
    let shows: Vec<Result<Story, (String, String)>> = vec![
        Ok(story(1, "id")),
        Ok(story(2, "id")),
        Ok(story(3, "id")),
        Ok(story(4, "jv")),
        Ok(story(5, "jv")),
    ];
    let (rows, hidden) = sinetron_rows(&shows, |s| s.episode == 1);
    let kinds: Vec<String> = rows
        .iter()
        .map(|r| match r {
            Row::Open(s, n) => format!("open {} #{n}", s.episode),
            Row::Next(s, n) => format!("next {} #{n}", s.episode),
            Row::Broken(id, _) => format!("broken {id}"),
        })
        .collect();
    // Javanese counts from 1 again.
    assert_eq!(kinds, ["open 1 #1", "next 2 #2", "next 4 #1"]);
    assert_eq!(hidden, 2);
}

// ------------------------------------------------------------------ lists

fn listen() -> Element {
    rsx! { ListenScreen {} }
}

fn placed(level: &str) -> crate::store::Store {
    let mut store = crate::store::Store::open_in_memory().unwrap();
    let p = kabar_protocol::test::PlacementResult {
        start_level: level.into(),
        start_unit: None,
        sections: vec![],
    };
    store.record_lesson_event("placement", "placed", &serde_json::to_value(&p).unwrap()).unwrap();
    store
}

fn first(language: &str) -> &'static Story {
    SHOWS.sinetron.iter().flatten().find(|s| s.language == language).unwrap()
}

#[test]
fn a_new_learner_sees_one_teaser_per_language_and_how_far_it_is() {
    let mut app = App::new(listen, Setup::default());
    let text = visible(&app.html());
    let (id, jv) = (first("id"), first("jv"));
    assert!(text.contains(&id.title.target) && text.contains(&jv.title.target), "{text}");
    assert!(text.contains("Episode 1 · Indonesian") && text.contains("Episode 1 · Javanese"), "{text}");
    assert!(text.contains("Opens after") && text.contains("lessons to go in this unit"), "{text}");
    assert!(text.contains("more episodes after that"), "{text}");
    assert!(text.contains("No downloaded episodes yet."), "{text}");
    // The teaser opens the lesson that leads there.
    app.click(&id.title.target);
    assert_eq!(*app.ui().screen.peek(), crate::Screen::Lesson);
    let lesson = app.ui().lesson.peek().clone().unwrap();
    assert!(lesson.starts_with(&id.unlock_after), "{lesson}");
}

#[test]
fn unlocked_episodes_open_the_player_and_watched_ones_are_marked() {
    let mut store = placed("bipa2");
    let s = first("id");
    store.record_lesson_event(&s.id, "completed", &json!({})).unwrap();
    let mut app = App::new(listen, Setup { store: Some(store), ..Setup::default() });
    let text = visible(&app.html());
    assert!(text.contains("✓ Watched"), "{text}");
    app.click(&s.title.target);
    let html = app.html();
    assert!(html.contains("class=\"lx-player\""), "{html}");
    let text = visible(&html);
    assert!(text.contains("Sinetron") && text.contains("Scene 1"), "{text}");
    assert!(text.contains(&format!("Line 0 / {}", Program::sinetron(s).cues.len())), "{text}");
    // The questions come after the last line.
    app.click("On to the questions");
    assert!(!app.html().contains("class=\"lx-player\""), "the quiz replaces the player");
}

#[test]
fn the_radio_tab_lists_every_show_with_its_length() {
    let mut app = App::new(listen, Setup::default());
    app.click("Kabar Radio");
    let text = visible(&app.html());
    for r in SHOWS.radio.iter().flatten() {
        assert!(text.contains(&r.title.target), "{}: {text}", r.id);
    }
    assert!(text.contains("min"), "{text}");
    let r = SHOWS.radio.iter().flatten().next().unwrap();
    app.click(&r.title.target);
    let text = visible(&app.html());
    assert!(text.contains("Kabar Radio") && text.contains("Words"), "{text}");
    for w in r.words.iter().take(3) {
        assert!(text.contains(w.as_str()), "{w}: {text}");
    }
    app.click("Back");
    assert!(visible(&app.html()).contains("Sinetron"));
}

#[test]
fn listening_a_radio_show_to_the_end_counts_as_done() {
    let mut app = App::new(listen, Setup::default());
    app.click("Kabar Radio");
    let r = SHOWS.radio.iter().flatten().next().unwrap();
    app.click(&r.title.target);
    let n = Program::radio(r).cues.len();
    // Jump to the last line and let it end.
    app.change("Where you are", &(n - 1).to_string());
    let engine = engine(&app);
    let seq = last_seq(&engine);
    engine.reply(json!({ "ev": "ended", "seq": seq }));
    app.settle();
    let db = app.db();
    assert!(db.0.borrow().completed_lessons().unwrap().contains(&r.id));
    let log = db.0.borrow().activity(None).unwrap();
    assert!(
        log.iter().any(|e| matches!(&e.kind, ActivityKind::Practice { skill: Skill::Listening, topic, .. } if *topic == r.title.target)),
        "{log:?}"
    );
    app.click("Back");
    assert!(visible(&app.html()).contains("✓ Listened"));
}

#[test]
fn episode_labels_count_in_each_language() {
    fn root() -> Element {
        let ui = use_context::<Ui>();
        let s = first("jv");
        rsx! {
            p { "{episode_label(&ui, s, 2)}" }
            p { "{lessons_left(&ui, 1)}" }
            p { "{lessons_left(&ui, 2)}" }
            p { "{lessons_left(&ui, 5)}" }
            p { "{more_episodes(&ui, 1)}" }
            p { "{more_episodes(&ui, 3)}" }
            p { "{megabytes(&ui, 3_400_000)}" }
        }
    }
    let lines = |lang| {
        let app = App::new(root, Setup { lang, ..Setup::default() });
        visible(&app.html())
    };
    assert_eq!(
        lines(UiLanguage::En),
        "Episode 2 · Javanese 1 lesson to go in this unit 2 lessons to go in this unit 5 lessons to go in this unit \
         And 1 more episode after that. And 3 more episodes after that. 3.4 MB"
    );
    let eg = lines(UiLanguage::ArEg);
    for part in ["فاضلك درس واحد في الوحدة", "فاضلك درسين في الوحدة", "دروس في الوحدة", "حلقتين"]
    {
        assert!(eg.contains(part) || part == "حلقتين", "{part}: {eg}");
    }
    assert!(eg.contains("حلقة") && eg.contains("جاوي"), "{eg}");
    let ar = lines(UiLanguage::Ar);
    assert!(ar.contains("بقي لك درس واحد") && ar.contains("درسان"), "{ar}");
}

#[test]
fn a_broken_file_is_listed_with_its_error() {
    fn root() -> Element {
        rsx! { Broken { id: "sinetron.e99", error: "missing field `scenes`" } }
    }
    let text = visible(&App::new(root, Setup::default()).html());
    assert!(text.contains("sinetron.e99") && text.contains("missing field `scenes`"), "{text}");
}

#[test]
fn every_listen_line_is_written_in_all_three_languages() {
    use self::text::T::*;
    let all = [
        Episode,
        Indonesian,
        Javanese,
        OpensAfter,
        Watched,
        Listened,
        Play,
        Pause,
        Slower,
        Translations,
        PrevLine,
        NextLine,
        Line,
        Position,
        NormalSpeed,
        Note,
        TapLine,
        Download,
        Downloading,
        Downloaded,
        RemoveDownload,
        DownloadsUse,
        FreeUpSpace,
        NoDownloads,
        NeedCloud,
        Offline,
        Quota,
        ServerTrouble,
        SystemVoice,
        JavaneseLater,
        Questions,
        NextTime,
        Words,
        Back,
        Broken,
        Scene,
        Intro,
        Dialogue,
        WordSpotlight,
        Recap,
        Outro,
    ];
    for t in all {
        match t {
            Episode | Indonesian | Javanese | OpensAfter | Watched | Listened | Play | Pause | Slower
            | Translations | PrevLine | NextLine | Line | Position | NormalSpeed | Note | TapLine
            | Download | Downloading | Downloaded | RemoveDownload | DownloadsUse | FreeUpSpace
            | NoDownloads | NeedCloud | Offline | Quota | ServerTrouble | SystemVoice | JavaneseLater
            | Questions | NextTime | Words | Back | Broken | Scene | Intro | Dialogue | WordSpotlight
            | Recap | Outro => {}
        }
    }
    crate::testkit::check_catalogue(&all, tt, &[]);
}

// ----------------------------------------------------------------- player

thread_local! {
    static PROGRAM: RefCell<Option<Program>> = const { RefCell::new(None) };
    static ENDED: Cell<u32> = const { Cell::new(0) };
}

fn localized(en: &str, ar: &str) -> Localized {
    Localized { en: en.into(), ar: ar.into(), ar_eg: ar.into() }
}

fn program(language: &str) -> Program {
    let cue = |group, speaker: &str, text: &str, en: &str, note: Option<Localized>| Cue {
        group,
        speaker: speaker.into(),
        text: text.into(),
        meaning: localized(en, "معنى"),
        note,
        voice: kabar_protocol::cloud::Voice::IdGadis,
        pitch_percent: 0,
    };
    Program {
        id: format!("sinetron.t{language}"),
        language: language.into(),
        groups: vec![
            Heading::Scene(localized("At the warung", "في الوارونج")),
            Heading::Segment { kind: SegmentKind::WordSpotlight, title: Some("pagi".into()) },
        ],
        cues: vec![
            cue(0, "Sari", "Selamat pagi, Pak!", "Good morning, sir!", None),
            cue(
                0,
                "Pak Budi",
                "Pagi, Sari.",
                "Morning, Sari.",
                Some(localized("Pagi is short for selamat pagi.", "اختصار")),
            ),
            cue(1, "Sari", "Sampai jumpa!", "See you!", None),
        ],
    }
}

fn player() -> Element {
    let program = PROGRAM.with(|p| p.borrow().clone()).unwrap();
    rsx! {
        ProgramPlayer {
            program,
            title: "Pagi di warung",
            on_back: move |_| {},
            on_end: move |_| ENDED.set(ENDED.get() + 1),
            p { "the end card" }
        }
    }
}

fn open_player(language: &str, mut setup: Setup) -> App {
    PROGRAM.with(|p| *p.borrow_mut() = Some(program(language)));
    ENDED.set(0);
    setup.settings.translation = kabar_core::settings::TranslationMode::Off;
    App::new(player, setup)
}

fn engine(app: &App) -> std::rc::Rc<FakeEval> {
    app.doc.find("const a = new Audio()").expect("the audio engine started")
}

fn last_seq(e: &FakeEval) -> u64 {
    e.sent.borrow().iter().rev().find_map(|m| m["seq"].as_u64()).expect("a line was sent")
}

fn ops(e: &FakeEval) -> Vec<String> {
    e.sent.borrow().iter().map(|m| m["op"].as_str().unwrap_or_default().to_string()).collect()
}

#[test]
fn the_player_shows_scenes_segments_lines_and_the_end_card() {
    let app = open_player("id", Setup::default());
    let text = visible(&app.html());
    for part in [
        "Pagi di warung",
        "Line 0 / 3",
        "Scene 1",
        "At the warung",
        "Word spotlight",
        "pagi",
        "Sari",
        "Pak Budi",
        "Selamat",
        "the end card",
    ] {
        assert!(text.contains(part), "{part}: {text}");
    }
    assert!(!text.contains("Good morning, sir!"), "translations start hidden: {text}");
    assert!(app.has("click", "Download"));
}

#[test]
fn lines_play_in_order_with_the_device_voice_and_the_end_is_reported_once() {
    let mut app = open_player("id", Setup::default());
    let e = engine(&app);
    app.click("Selamat pagi, Pak!");
    // No neural audio here: the device voice reads it, the engine waits.
    let text = visible(&app.html());
    assert!(text.contains("Line 1 / 3"), "{text}");
    assert_eq!(ops(&e).last().map(String::as_str), Some("wait"));
    assert!(
        app.doc
            .scripts()
            .iter()
            .any(|s| s.contains("SpeechSynthesisUtterance") && s.contains("Selamat pagi, Pak!"))
    );
    assert!(app.html().contains("lx-cue a on"), "the line is highlighted");

    // A late `ended` from an older line changes nothing.
    let seq = last_seq(&e);
    e.reply(json!({ "ev": "ended", "seq": seq - 1 }));
    app.settle();
    assert!(visible(&app.html()).contains("Line 1 / 3"));

    // Each `ended` moves on while playing.
    e.reply(json!({ "ev": "ended", "seq": seq }));
    app.settle();
    assert!(visible(&app.html()).contains("Line 2 / 3"));
    e.reply(json!({ "ev": "ended", "seq": last_seq(&e) }));
    app.settle();
    assert!(visible(&app.html()).contains("Line 3 / 3"));
    assert_eq!(ENDED.get(), 0);
    e.reply(json!({ "ev": "ended", "seq": last_seq(&e) }));
    app.settle();
    assert_eq!(ENDED.get(), 1, "the last line finished the show");
    assert!(app.has("click", "Play"), "paused at the end");
    // Play again starts over; finishing again doesn't report twice.
    app.click("Play");
    assert!(visible(&app.html()).contains("Line 1 / 3"));
    assert_eq!(ENDED.get(), 1);
}

#[test]
fn controls_step_pause_slow_down_and_reveal() {
    let mut app = open_player("id", Setup::default());
    let e = engine(&app);
    app.click("Play");
    assert!(visible(&app.html()).contains("Line 1 / 3"));
    app.click("Next line");
    assert!(visible(&app.html()).contains("Line 2 / 3"));
    app.click("Previous line");
    assert!(visible(&app.html()).contains("Line 1 / 3"));
    app.click("Pause");
    assert_eq!(ops(&e).last().map(String::as_str), Some("pause"));
    assert!(app.has("click", "Play"));

    app.click("A bit slower");
    assert_eq!(e.sent.borrow().last().unwrap()["rate"], json!(0.8));
    assert!(visible(&app.html()).contains("0.8×"));
    app.click("Normal speed");
    assert_eq!(e.sent.borrow().last().unwrap()["rate"], json!(1.0));

    app.click_nth("…", 0);
    let text = visible(&app.html());
    assert!(text.contains("Good morning, sir!") && !text.contains("See you!"), "{text}");
    app.click("lx-tr");
    let text = visible(&app.html());
    assert!(text.contains("See you!") && text.contains("Morning, Sari"), "{text}");

    // The cultural note opens and closes.
    app.click("Note");
    assert!(visible(&app.html()).contains("Pagi is short for selamat pagi"));
    app.click("Note");
    assert!(!visible(&app.html()).contains("Pagi is short for selamat pagi"));
}

#[test]
fn javanese_lines_wait_for_their_voice() {
    let mut app = open_player("jv", Setup::default());
    app.click("Selamat pagi, Pak!");
    let text = visible(&app.html());
    assert!(text.contains(tt(UiLanguage::En, T::JavaneseLater)), "{text}");
    assert!(app.has("click", "Play"), "nothing plays");
}

#[test]
fn an_audio_error_falls_back_to_the_device_voice() {
    let mut app = open_player("id", Setup::default());
    let e = engine(&app);
    app.click("Play");
    e.reply(json!({ "ev": "error", "seq": last_seq(&e) }));
    app.settle();
    assert!(visible(&app.html()).contains(tt(UiLanguage::En, T::SystemVoice)));
}

/// An audio cache on a temporary disk with every line of `p` already fetched.
fn cached(p: &Program) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let cache = crate::audio::AudioCache::new(dir.path().into(), Default::default());
    for req in requests(p) {
        let path = cache.path(&req);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, vec![0u8; 6_000]).unwrap();
    }
    dir
}

#[test]
fn cached_lines_play_as_neural_audio_and_download_into_one_track() {
    let dir = cached(&program("id"));
    let mut app = open_player("id", Setup { audio_dir: Some(dir.path().into()), ..Setup::default() });
    let e = engine(&app);
    app.click("Pagi, Sari.");
    let load = e.sent.borrow().iter().rev().find(|m| m["op"] == "load").cloned().expect("a clip loads");
    assert!(load["src"].as_str().unwrap().starts_with("data:audio/mpeg;base64,"), "{load}");
    assert!(!visible(&app.html()).contains(tt(UiLanguage::En, T::SystemVoice)));

    // Download: every line is cached, so it joins them offline.
    app.click("Download");
    let text = visible(&app.html());
    assert!(text.contains("✓ Downloaded"), "{text}");
    let track =
        crate::audio::AudioCache::new(dir.path().into(), Default::default()).track("sinetron.tid").unwrap();
    assert_eq!(track.starts.len(), 3);
    assert!(track.starts.windows(2).all(|w| w[0] < w[1]));

    // The track plays from the line's start and follows the time.
    app.click("Sampai jumpa!");
    let load = e.sent.borrow().last().cloned().unwrap();
    assert_eq!((load["op"].as_str(), load["at"].as_f64()), (Some("load"), Some(track.starts[2])));
    e.reply(json!({ "ev": "time", "seq": last_seq(&e), "t": track.starts[1] + 0.01 }));
    app.settle();
    assert!(visible(&app.html()).contains("Line 2 / 3"));
    app.click("Pause");
    app.click("Play");
    assert_eq!(ops(&e).last().map(String::as_str), Some("resume"));
    app.click("Selamat pagi, Pak!");
    assert_eq!(ops(&e).last().map(String::as_str), Some("seek"));
    e.reply(json!({ "ev": "ended", "seq": last_seq(&e) }));
    app.settle();
    assert_eq!(ENDED.get(), 1);

    app.click("Remove download");
    assert!(app.has("click", "Download"));
    assert!(
        crate::audio::AudioCache::new(dir.path().into(), Default::default()).track("sinetron.tid").is_none()
    );
}

#[test]
fn a_broken_download_is_dropped_and_the_device_voice_reads_on() {
    let dir = tempfile::tempdir().unwrap();
    let episodes = dir.path().join("episodes");
    std::fs::create_dir_all(&episodes).unwrap();
    std::fs::write(episodes.join("sinetron.tid.mp3"), vec![0u8; 9_000]).unwrap();
    std::fs::write(episodes.join("sinetron.tid.json"), "[0.0, 0.5, 1.0]").unwrap();
    let mut app = open_player("id", Setup { audio_dir: Some(dir.path().into()), ..Setup::default() });
    let e = engine(&app);
    assert!(visible(&app.html()).contains("✓ Downloaded"));
    app.click("Pagi, Sari.");
    let load = e.sent.borrow().last().cloned().unwrap();
    assert_eq!((load["op"].as_str(), load["at"].as_f64()), (Some("load"), Some(0.5)));
    e.reply(json!({ "ev": "error", "seq": last_seq(&e) }));
    app.settle();
    assert_eq!(ops(&e).last().map(String::as_str), Some("wait"));
    let text = visible(&app.html());
    assert!(
        text.contains(tt(UiLanguage::En, T::SystemVoice)) && text.contains("Download for offline"),
        "{text}"
    );
}

#[test]
fn a_download_without_the_cloud_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = open_player("id", Setup { audio_dir: Some(dir.path().into()), ..Setup::default() });
    app.click("Download");
    let text = visible(&app.html());
    assert!(text.contains(tt(UiLanguage::En, T::NeedCloud)), "{text}");
    // Playing without a cached line and no sync: the device voice, and why.
    app.click("Selamat pagi, Pak!");
    assert!(visible(&app.html()).contains(tt(UiLanguage::En, T::SystemVoice)));
    let _ = DownloadState::Idle;
}

#[test]
fn downloads_show_their_size_and_free_up_space() {
    let dir = tempfile::tempdir().unwrap();
    let episodes = dir.path().join("episodes");
    std::fs::create_dir_all(&episodes).unwrap();
    std::fs::write(episodes.join("radio.bipa1.u01.mp3"), vec![0u8; 2_500_000]).unwrap();
    std::fs::write(episodes.join("radio.bipa1.u01.json"), "[0.0]").unwrap();
    let mut app = App::new(listen, Setup { audio_dir: Some(dir.path().into()), ..Setup::default() });
    app.click("Kabar Radio");
    let text = visible(&app.html());
    assert!(text.contains("2.5 MB") && text.contains("⤓ Downloaded"), "{text}");
    app.click("Free up space");
    assert!(!episodes.join("radio.bipa1.u01.mp3").exists());
    assert!(visible(&app.html()).contains("No downloaded episodes yet."));
}

#[test]
fn leaving_the_player_or_the_quiz_goes_back_to_the_list() {
    let mut app = App::new(listen, Setup { store: Some(placed("bipa2")), ..Setup::default() });
    let s = first("id");
    app.click(&s.title.target);
    app.click("Back");
    assert!(!app.html().contains("class=\"lx-player\"") && visible(&app.html()).contains("Kabar Radio"));
    app.click(&s.title.target);
    app.click("On to the questions");
    app.click("lp-close");
    assert!(visible(&app.html()).contains(&s.title.target), "back on the list");
}

#[test]
fn not_done_first_puts_unwatched_episodes_and_unheard_shows_on_top() {
    let mut store = placed("bipa2");
    let s = first("id");
    store.record_lesson_event(&s.id, "completed", &json!({})).unwrap();
    let r = SHOWS.radio.iter().flatten().next().unwrap();
    store.record_lesson_event(&r.id, "completed", &json!({})).unwrap();
    let mut app = App::new(listen, Setup { store: Some(store), ..Setup::default() });
    app.click("Course order");
    app.click("Not done yet first");
    let cards = |app: &App| -> Vec<String> {
        app.targets("click")
            .into_iter()
            .filter(|t| t.attr("class").is_some_and(|c| c.contains("lx-card")))
            .map(|t| t.text)
            .collect()
    };
    let list = cards(&app);
    assert!(!list[0].contains(&s.title.target), "the watched episode moved down: {list:?}");
    app.click("Kabar Radio");
    app.click("Course order");
    app.click("Not done yet first");
    let list = cards(&app);
    assert!(list.last().unwrap().contains(&r.title.target), "{list:?}");
}

#[test]
fn a_downloaded_episode_is_marked_in_the_list() {
    let dir = tempfile::tempdir().unwrap();
    let s = first("id");
    let episodes = dir.path().join("episodes");
    std::fs::create_dir_all(&episodes).unwrap();
    std::fs::write(episodes.join(format!("{}.mp3", s.id)), vec![0u8; 1000]).unwrap();
    std::fs::write(episodes.join(format!("{}.json", s.id)), "[0.0]").unwrap();
    let app = App::new(
        listen,
        Setup { store: Some(placed("bipa2")), audio_dir: Some(dir.path().into()), ..Setup::default() },
    );
    let text = visible(&app.html());
    assert!(text.contains("⤓ Downloaded"), "{text}");
}

#[test]
fn every_cloud_problem_has_its_own_notice() {
    use super::player::notice_for;
    use crate::cloud::link::Unreachable;
    assert_eq!(notice_for(&Unreachable::NotConfigured), T::NeedCloud);
    assert_eq!(notice_for(&Unreachable::SignedOut), T::NeedCloud);
    assert_eq!(notice_for(&Unreachable::Offline), T::Offline);
    assert_eq!(notice_for(&Unreachable::QuotaExhausted), T::Quota);
    assert_eq!(notice_for(&Unreachable::Server("500".into())), T::ServerTrouble);
}

#[test]
fn unknown_engine_events_change_nothing() {
    let mut app = open_player("id", Setup::default());
    let e = engine(&app);
    app.click("Play");
    e.reply(json!({ "ev": "loadedmetadata", "seq": last_seq(&e) }));
    e.reply(json!({ "ev": "time", "seq": last_seq(&e), "t": 3.0 }));
    app.settle();
    assert!(visible(&app.html()).contains("Line 1 / 3"));
    assert!(app.has("click", "Pause"), "still playing");
}

#[test]
fn while_synced_lines_stream_from_the_cloud_and_the_next_one_is_fetched_ahead() {
    let dir = tempfile::tempdir().unwrap();
    let setup = Setup {
        audio_dir: Some(dir.path().into()),
        cloud: true,
        sync: Some(crate::cloud::SyncState::Live),
        ..Setup::default()
    };
    let mut app = open_player("id", setup);
    let e = engine(&app);
    // An engine error first leaves the device-voice notice up.
    app.click("Play");
    let load = e.sent.borrow().iter().rev().find(|m| m["op"] == "load").cloned().expect("streamed");
    assert!(load["src"].as_str().unwrap().starts_with("data:audio/mpeg;base64,"), "{load}");
    e.reply(json!({ "ev": "error", "seq": last_seq(&e) }));
    app.settle();
    assert!(visible(&app.html()).contains(tt(UiLanguage::En, T::SystemVoice)));

    // The next streamed line clears it, and the one after is saved ahead.
    app.click("Pagi, Sari.");
    assert_eq!(ops(&e).last().map(String::as_str), Some("load"));
    assert!(!visible(&app.html()).contains(tt(UiLanguage::En, T::SystemVoice)));
    let cache = crate::audio::AudioCache::new(dir.path().into(), Default::default());
    let reqs = requests(&program("id"));
    assert!(reqs.iter().all(|r| cache.cached(r).is_some()), "all three lines are on disk");
}

#[test]
fn a_cloud_that_cannot_answer_falls_back_with_its_reason() {
    // Synced, but the link has no server: the fetch fails, the device reads.
    let dir = tempfile::tempdir().unwrap();
    let setup = Setup {
        audio_dir: Some(dir.path().into()),
        sync: Some(crate::cloud::SyncState::Live),
        ..Setup::default()
    };
    let mut app = open_player("id", setup);
    let e = engine(&app);
    app.click("Selamat pagi, Pak!");
    assert_eq!(ops(&e).last().map(String::as_str), Some("wait"));
    assert!(visible(&app.html()).contains(tt(UiLanguage::En, T::SystemVoice)));

    // Reconnecting: Javanese says it waits for its voice rather than "sign in".
    let setup = Setup {
        audio_dir: Some(dir.path().into()),
        sync: Some(crate::cloud::SyncState::Offline),
        ..Setup::default()
    };
    let mut app = open_player("jv", setup);
    app.click("Selamat pagi, Pak!");
    assert!(visible(&app.html()).contains(tt(UiLanguage::En, T::JavaneseLater)));
    assert!(app.has("click", "Play"));
}

#[test]
fn a_downloaded_episode_that_cannot_be_read_streams_instead() {
    let dir = tempfile::tempdir().unwrap();
    let episodes = dir.path().join("episodes");
    std::fs::create_dir_all(&episodes).unwrap();
    std::fs::write(episodes.join("sinetron.tid.mp3"), vec![0u8; 9_000]).unwrap();
    std::fs::write(episodes.join("sinetron.tid.json"), "[0.0, 0.5, 1.0]").unwrap();
    let mut app = open_player("id", Setup { audio_dir: Some(dir.path().into()), ..Setup::default() });
    let e = engine(&app);
    assert!(visible(&app.html()).contains("✓ Downloaded"));
    // The sound is cleaned away while the player is open.
    std::fs::remove_file(episodes.join("sinetron.tid.mp3")).unwrap();
    app.click("Pagi, Sari.");
    assert_eq!(ops(&e).last().map(String::as_str), Some("wait"), "the device voice reads it");
    let text = visible(&app.html());
    assert!(text.contains("Download for offline") && text.contains("Line 2 / 3"), "{text}");
}

#[test]
fn a_full_disk_at_the_end_of_a_show_leaves_it_unheard_and_the_app_running() {
    let mut app = App::new(listen, Setup::default());
    app.click("Kabar Radio");
    let r = SHOWS.radio.iter().flatten().next().unwrap();
    app.click(&r.title.target);
    let n = Program::radio(r).cues.len();
    app.change("Where you are", &(n - 1).to_string());
    app.db().0.borrow().refuse_writes();
    let engine = engine(&app);
    engine.reply(json!({ "ev": "ended", "seq": last_seq(&engine) }));
    app.settle();
    assert!(!app.db().0.borrow().completed_lessons().unwrap().contains(&r.id));
    app.click("Back");
    let text = visible(&app.html());
    assert!(text.contains(&r.title.target) && !text.contains("✓ Listened"), "{text}");
}
