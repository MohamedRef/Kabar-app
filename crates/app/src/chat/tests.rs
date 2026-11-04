//! Persona chats driven through the real screen: pick a partner and a scene,
//! talk with a scripted engine, and check what is stored and shown.

use kabar_protocol::ai::{EngineError, EngineId};

use super::*;
use crate::testkit::{App, Scripted, Setup, fixture, reply, visible};

fn languages(focus: LanguageFocus) -> Vec<bool> {
    personas_for(focus).into_iter().flatten().map(is_javanese).collect()
}

#[test]
fn the_focus_language_leads_the_partner_list() {
    let id = languages(LanguageFocus::Indonesian);
    assert!(id.contains(&true) && !id[0], "Indonesian partners first");
    assert!(id.windows(2).all(|w| w[0] <= w[1]), "then every Javanese one");
    let jv = languages(LanguageFocus::Javanese);
    assert!(jv[0] && jv.windows(2).all(|w| w[0] >= w[1]));
    assert_eq!(languages(LanguageFocus::Both), id);
}

fn chat() -> Element {
    rsx! { ChatScreen {} }
}

fn engine() -> Scripted {
    Scripted { id: EngineId::WorkersAi, script: Default::default() }
}

fn with_engine(e: &Scripted) -> Setup {
    Setup { engines: vec![e.clone()], ..Setup::default() }
}

/// Opens Mas Dimas's first scene (meeting at the angkringan).
fn open_dimas(app: &mut App) -> String {
    app.click("Mas Dimas");
    app.click("first time at an angkringan");
    let convs = app.db().0.borrow().conversations().unwrap();
    assert_eq!(convs.len(), 1);
    convs[0].id.clone()
}

#[test]
fn every_partner_is_listed_with_its_language_and_levels() {
    let app = App::new(chat, Setup::default());
    let html = visible(&app.html());
    for p in PERSONA_LIST.iter().flatten() {
        assert!(html.contains(&p.name), "{} missing: {html}", p.name);
    }
    assert!(html.contains("Who to talk to") && html.contains("Javanese") && html.contains("Indonesian"));
    assert!(!html.contains("Your chats"), "no history before the first chat");
    assert!(!html.contains("broken"), "every persona file parses");
}

#[test]
fn a_scene_starts_a_saved_chat_with_the_partners_opener() {
    let mut app = App::new(chat, Setup::default());
    app.click("Mas Dimas");
    let scenes = visible(&app.html());
    assert!(scenes.contains("Start with") && scenes.contains("Free chat"), "{scenes}");
    assert!(scenes.contains("“Halo, Mas! Piye kabare?”"), "the opener previews: {scenes}");
    app.click("Back");
    let id = open_dimas(&mut app);

    let db = app.db();
    let c = db.0.borrow().conversations().unwrap().remove(0);
    assert_eq!(
        (c.persona.as_str(), c.scenario.as_deref(), c.register.as_str()),
        ("mas-dimas", Some("d01"), "ngoko")
    );
    let msgs = db.0.borrow().chat_messages(&id).unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].role, ChatRole::Persona);
    assert_eq!(msgs[0].text, "Halo, Mas! Piye kabare?");
    assert!(msgs[0].unverified, "Javanese stays unverified until reviewed");

    let html = app.html();
    assert!(html.contains("class=\"ch-chat\"") && html.contains("aria-pressed=\"true\""), "{html}");
    let text = visible(&html);
    assert!(text.contains("Halo") && text.contains("Not checked yet"), "{text}");
    // One message: nothing to replay yet.
    assert!(html.contains("disabled") && text.contains("Replay"));
}

#[test]
fn a_turn_stores_the_fixes_and_the_reply_and_shows_both() {
    let e = engine();
    let mut app = App::new(chat, with_engine(&e));
    let id = open_dimas(&mut app);
    e.script.replies.borrow_mut().clear();
    e.script.replies.borrow_mut().push_back(Ok(fixture("roleplay_turn.angkringan_dimas.workers_ai.json")));

    // An empty box sends nothing.
    app.click("Send");
    assert_eq!(app.db().0.borrow().chat_messages(&id).unwrap().len(), 1);

    app.input("Write your reply", "Aku belum makan, lapar banget");
    app.click("Send");

    let msgs = app.db().0.borrow().chat_messages(&id).unwrap();
    assert_eq!(msgs.len(), 3, "{msgs:#?}");
    let mine = &msgs[1];
    assert_eq!(mine.role, ChatRole::Learner);
    assert_eq!(mine.text, "Aku belum makan, lapar banget");
    let fixes: Vec<_> = mine.corrections.iter().map(|c| (c.text.as_str(), c.fix.as_str())).collect();
    assert_eq!(fixes, [("belum makan", "durung mangan"), ("lapar banget", "ngelih banget")]);
    assert!(mine.unverified, "an engine's fixes stay unverified");
    let reply = &msgs[2];
    assert_eq!(reply.role, ChatRole::Persona);
    assert!(reply.text.starts_with("Wah, durung mangan to?"));
    assert_eq!(reply.register.as_deref(), Some("ngoko"));
    assert!(reply.created_ms > mine.created_ms);

    let call = e.script.calls.borrow().iter().find(|c| c.user.contains("lapar banget")).cloned().unwrap();
    assert!(call.user.contains("Halo, Mas! Piye kabare?"), "the history goes with the turn");

    let text = visible(&app.html());
    for shown in [
        "belum makan",
        "durung mangan",
        "More natural",
        "ngelih banget",
        "Hungry in ngoko Javanese is ngelih.",
    ] {
        assert!(text.contains(shown), "{shown}: {text}");
    }
    assert!(!app.html().contains("ch-bubble ch-typing"), "the typing bubble is gone");
    // Two messages or more: the replay opens.
    app.click("Replay");
    let player = visible(&app.html());
    assert!(player.contains("Mas Dimas") && player.contains("Line"), "{player}");
    app.click("Back");
    assert!(app.html().contains("class=\"ch-chat\""));
}

#[test]
fn a_clean_message_gets_all_good_and_arabic_reasons_show_in_arabic() {
    let e = engine();
    let mut app = App::new(chat, Setup { lang: UiLanguage::ArEg, ..with_engine(&e) });
    app.click("Mas Dimas");
    app.click("أول مرة تقابل ديماس");
    e.script.replies.borrow_mut().clear();
    e.script.replies.borrow_mut().push_back(Ok(fixture("roleplay_turn.angkringan_dimas.workers_ai.json")));
    app.input("اكتب", "Aku belum makan, lapar banget");
    app.key("اكتب", Key::Enter, true);
    assert_eq!(app.db().0.borrow().conversations().unwrap().len(), 1);
    let id = app.db().0.borrow().conversations().unwrap()[0].id.clone();
    assert_eq!(
        app.db().0.borrow().chat_messages(&id).unwrap().len(),
        1,
        "Shift+Enter is a new line, not send"
    );
    app.key("اكتب", Key::Enter, false);
    let text = visible(&app.html());
    assert!(text.contains("«جعان» بالـ ngoko الجاوي هي ngelih."), "{text}");
    assert!(!text.contains("Hungry in ngoko"), "{text}");
}

#[test]
fn no_engine_points_to_devices_and_a_failed_turn_can_be_retried() {
    // No engine at all: the message stays, the problem says how to connect.
    let mut app = App::new(chat, Setup::default());
    let id = open_dimas(&mut app);
    app.input("Write your reply", "Apik, Mas");
    app.click("Send");
    let html = app.html();
    assert!(html.contains("role=\"alert\""), "{html}");
    assert_eq!(app.db().0.borrow().chat_messages(&id).unwrap().len(), 2, "the learner's line is kept");
    app.click("Open Devices");
    assert_eq!(*app.ui().screen.peek(), Screen::Devices);

    // An engine that fails once: Retry asks again with the same line.
    let e = engine();
    let mut app = App::new(chat, with_engine(&e));
    let id = open_dimas(&mut app);
    e.script.replies.borrow_mut().clear();
    e.script.replies.borrow_mut().push_back(Err(EngineError::Other { message: "boom".into() }));
    app.input("Write your reply", "Aku belum makan, lapar banget");
    app.click("Send");
    let text = visible(&app.html());
    assert!(app.has("click", "Try again"), "{text}");
    let calls = e.script.calls.borrow().len();
    e.script.replies.borrow_mut().push_back(Ok(fixture("roleplay_turn.angkringan_dimas.workers_ai.json")));
    app.click("Try again");
    assert!(e.script.calls.borrow().len() > calls);
    let msgs = app.db().0.borrow().chat_messages(&id).unwrap();
    assert_eq!(msgs.len(), 3, "one learner line, answered once");
    assert!(!app.html().contains("role=\"alert\""));
}

#[test]
fn the_register_chip_is_saved_with_the_conversation() {
    let mut app = App::new(chat, Setup::default());
    let id = open_dimas(&mut app);
    app.click("krama (politely)");
    let c = app.db().0.borrow().conversations().unwrap().into_iter().find(|c| c.id == id).unwrap();
    assert_eq!(c.register, "krama");
    // Reopening the chat keeps it.
    app.click("Back");
    app.click("Mas Dimas");
    assert!(app.html().contains("Mas Dimas"));
}

#[test]
fn past_chats_list_newest_with_their_preview_and_delete() {
    let mut app = App::new(chat, Setup::default());
    open_dimas(&mut app);
    app.click("Back");
    let text = visible(&app.html());
    assert!(text.contains("Your chats") && text.contains("Halo, Mas! Piye kabare?"), "{text}");
    assert!(text.contains("1 messages") || text.contains("1 message"), "{text}");
    // Opening it goes back into the chat.
    app.click("Halo, Mas! Piye kabare?");
    assert!(app.html().contains("class=\"ch-chat\""));
    app.click("Back");
    app.click("Delete");
    assert!(app.db().0.borrow().conversations().unwrap().is_empty());
    assert!(!visible(&app.html()).contains("Your chats"));
}

#[test]
fn the_partner_card_opens_their_scenes() {
    let mut app = App::new(chat, Setup::default());
    open_dimas(&mut app);
    app.click("About them");
    let text = visible(&app.html());
    assert!(text.contains("Start with") && text.contains("Free chat"), "{text}");
    app.click("Back");
    assert!(visible(&app.html()).contains("Who to talk to"));
}

#[test]
fn a_finished_visit_is_logged_for_the_coach() {
    let e = engine();
    let mut app = App::new(chat, with_engine(&e));
    open_dimas(&mut app);
    e.script.replies.borrow_mut().clear();
    e.script.replies.borrow_mut().push_back(Ok(fixture("roleplay_turn.angkringan_dimas.workers_ai.json")));
    app.input("Write your reply", "Aku belum makan, lapar banget");
    app.click("Send");
    let db = app.db();
    let before = db.0.borrow().activity(None).unwrap().len();
    // Leaving the chat writes one practice event: 1 sent, 0 without a fix.
    app.click("Back");
    let log = db.0.borrow().activity(None).unwrap();
    assert_eq!(log.len(), before + 1);
    match &log.last().unwrap().kind {
        ActivityKind::Practice { skill, topic, correct, total } => {
            assert_eq!((skill, topic.as_str(), correct, total), (&Skill::Writing, "Mas Dimas", &0, &1));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn moods_follow_the_conversation() {
    let m = |role, fixes: usize| ChatMessage {
        id: "m".into(),
        doc: "c".into(),
        role,
        text: "x".into(),
        corrections: (0..fixes)
            .map(|_| ChatCorrection {
                text: "a".into(),
                fix: "b".into(),
                why_en: "c".into(),
                why_ar: "d".into(),
            })
            .collect(),
        register: None,
        unverified: false,
        created_ms: 0,
    };
    assert_eq!(mood_of(true, &[]), Mood::Thinking);
    assert_eq!(mood_of(false, &[]), Mood::Listening);
    assert_eq!(mood_of(false, &[m(ChatRole::Learner, 0)]), Mood::Listening);
    assert_eq!(mood_of(false, &[m(ChatRole::Learner, 1), m(ChatRole::Persona, 0)]), Mood::Explaining);
    assert_eq!(mood_of(false, &[m(ChatRole::Learner, 0), m(ChatRole::Persona, 0)]), Mood::Laughing);
    assert_eq!(register_label("ngoko"), T::Ngoko);
    assert_eq!(register_label("krama"), T::Krama);
    assert_eq!(register_label("casual"), T::Casual);
    assert_eq!(register_label("formal"), T::Standard);
    let fix = &m(ChatRole::Learner, 1).corrections[0];
    assert_eq!(
        (why(fix, UiLanguage::En), why(fix, UiLanguage::ArEg), why(fix, UiLanguage::Ar)),
        ("c", "d", "d")
    );
    assert_eq!(friendly(FriendlyReason::ResetsLater), T::ResetsLater);
    assert_eq!(friendly(FriendlyReason::Unreachable), T::Unreachable);
    assert_eq!(friendly(FriendlyReason::NotAvailable), T::NotConnected);
    assert_eq!(friendly(FriendlyReason::TryAgain), T::TryAgain);
}

#[test]
fn a_conversation_whose_scene_is_gone_falls_back() {
    let p = persona("mas-dimas").unwrap();
    let c = |scenario: Option<&str>, level: &str| Conversation {
        id: "c".into(),
        doc: "c".into(),
        persona: p.id.clone(),
        scenario: scenario.map(str::to_string),
        level: level.into(),
        register: "ngoko".into(),
        created_ms: 0,
        updated_ms: 0,
    };
    assert_eq!(scene_of(p, &c(Some("d01"), "j1")).id.as_deref(), Some("d01"));
    let free = scene_of(p, &c(None, "j1"));
    assert!(free.id.is_none() && free.level == "j1");
    // A dialogue removed since: the first scene stands in.
    assert_eq!(scene_of(p, &c(Some("gone"), "zz")).id, scenarios(p)[0].id);
}

#[test]
fn every_chat_line_is_written_in_all_three_languages() {
    use self::text::T::*;
    let all = [
        YourChats,
        StartWith,
        FreeChat,
        Javanese,
        Indonesian,
        TalkToMe,
        Ngoko,
        Krama,
        Casual,
        Standard,
        Placeholder,
        Send,
        Write,
        Type,
        PreparingReply,
        Privacy,
        Unverified,
        MoreNatural,
        AllGood,
        Listen,
        Replay,
        Back,
        Delete,
        TapWord,
        ResetsLater,
        Unreachable,
        NotConnected,
        TryAgain,
        Retry,
        Broken,
        Messages,
        You,
        Partners,
        Idea,
        UseIdea,
        OpenDevices,
        AboutPartner,
    ];
    // A new line must join the list above (this match has no catch-all).
    for t in all {
        match t {
            YourChats | StartWith | FreeChat | Javanese | Indonesian | TalkToMe | Ngoko | Krama | Casual
            | Standard | Placeholder | Send | Write | Type | PreparingReply | Privacy | Unverified
            | MoreNatural | AllGood | Listen | Replay | Back | Delete | TapWord | ResetsLater
            | Unreachable | NotConnected | TryAgain | Retry | Broken | Messages | You | Partners | Idea
            | UseIdea | OpenDevices | AboutPartner => {}
        }
    }
    crate::testkit::check_catalogue(&all, tt, &[]);
}

#[test]
fn the_reply_idea_fills_the_box_once_the_partner_has_answered() {
    let p = persona("mas-dimas").unwrap();
    // The scene's next learner line after one turn (unless it is a planted mistake).
    let line = reply_idea(p, Some("d01"), 1).expect("d01 has a second learner line");
    let e = engine();
    let mut app = App::new(chat, with_engine(&e));
    open_dimas(&mut app);
    assert!(!app.has("click", "Reply idea"), "no idea before the learner's first turn is answered");
    e.script.replies.borrow_mut().clear();
    e.script.replies.borrow_mut().push_back(Ok(fixture("roleplay_turn.angkringan_dimas.workers_ai.json")));
    app.input("Write your reply", "Aku belum makan, lapar banget");
    app.click("Send");
    app.click("Reply idea");
    let text = visible(&app.html());
    assert!(text.contains(&line.text), "{text}");
    app.click("Use it");
    assert!(
        app.html().contains(&format!("value=\"{}\"", line.text)) || {
            let ta =
                app.targets("input").into_iter().find(|t| t.attr("placeholder") == Some("Write your reply…"));
            ta.is_some_and(|t| t.attr("value") == Some(line.text.as_str()))
        }
    );
    assert!(!visible(&app.html()).contains("Use it"), "the idea closes once used");
}

#[test]
fn a_reply_shows_its_translation_and_stays_flagged_in_javanese() {
    let e = engine();
    e.script.replies.borrow_mut().push_back(Ok(reply(serde_json::json!({
        "lines": [{ "i": 0, "arz": "أهلا يا باشا! عامل إيه؟", "ar": "مرحبًا يا أخي! كيف حالك؟", "en": "Hi, Mas! How are you doing?" }],
        "register": "ngoko",
    }))));
    let mut app = App::new(chat, with_engine(&e));
    open_dimas(&mut app);
    let text = visible(&app.html());
    assert!(text.contains("Hi, Mas! How are you doing?"), "{text}");
    assert!(text.contains("Not checked yet"), "{text}");
    // Its speaker button reads the line aloud.
    app.click("Listen");
    assert!(
        app.doc.scripts().iter().any(|s| s.contains("SpeechSynthesisUtterance") && s.contains("Piye kabare"))
    );
}

#[test]
fn partners_sort_a_to_z() {
    let mut app = App::new(chat, Setup::default());
    app.click_nth("Course order", 0);
    app.click("A to Z");
    let names: Vec<String> = app
        .targets("click")
        .into_iter()
        .filter(|t| t.attr("class") == Some("coll-btn ch-partner"))
        .map(|t| t.text)
        .collect();
    let mut sorted = names.clone();
    sorted.sort_by_key(|n| n.to_lowercase());
    assert!(names.len() >= 10 && names == sorted, "{names:?}");
}

#[test]
fn a_full_disk_starts_and_deletes_nothing() {
    let mut app = App::new(chat, Setup::default());
    open_dimas(&mut app);
    app.click("Back");
    app.db().0.borrow().refuse_writes();
    app.click("Delete");
    assert_eq!(app.db().0.borrow().conversations().unwrap().len(), 1, "the chat is still there");
    app.click("ch-partner");
    app.click("ch-scene");
    assert_eq!(app.db().0.borrow().conversations().unwrap().len(), 1);
    // The unsaved chat has nothing to show; Back still works.
    assert!(!app.html().contains("class=\"ch-chat\""));
}

#[test]
fn a_partner_without_a_portrait_shows_their_initial() {
    fn root() -> Element {
        rsx! {
            Avatar { name: "Mbak Nobody", javanese: true }
            Chat { id: "no-such-chat", on_back: |_| {}, on_replay: |_| {}, on_persona: |_| {} }
        }
    }
    let html = App::new(root, Setup::default()).html();
    assert!(html.contains("ch-avatar jv") && visible(&html) == "N", "{html}");
}

#[test]
fn a_persona_with_no_scenes_still_gets_a_free_chat_scene() {
    let mut p = persona("mas-dimas").unwrap().clone();
    p.example_dialogues.clear();
    p.openers.clear();
    let c = Conversation {
        id: "c".into(),
        doc: "c".into(),
        persona: p.id.clone(),
        scenario: None,
        level: "j2".into(),
        register: "ngoko".into(),
        created_ms: 0,
        updated_ms: 0,
    };
    let s = scene_of(&p, &c);
    assert_eq!((s.id, s.level.as_str(), s.opener.as_str()), (None, "j2", ""));
    assert_eq!(s.situation, p.role);
}
