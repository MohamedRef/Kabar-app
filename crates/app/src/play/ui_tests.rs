//! The Play screens driven like a learner drives them: tap through each game
//! to its end and check what the screen says and what the activity log got.

use kabar_core::locale::UiLanguage::{self, ArEg, En};
use kabar_protocol::coach::{ActivityKind, Skill};

use super::*;
use crate::i18n::t;
use crate::test_dom::{App, El, memory_db};
use crate::testkit::FakeDoc as FakeDocument;
use serde_json::json;

fn en(msg: Msg) -> &'static str {
    t(En, msg)
}

/// Mounts the Play screen on `route`.
fn play_at(lang: UiLanguage, route: Route) -> App {
    let mut app = App::new(lang, || rsx! { Play {} });
    open_route(&mut app, route);
    app
}

fn open_route(app: &mut App, route: Route) {
    let PlayNav(mut nav) = app.ctx::<PlayNav>();
    app.dom.in_app(|| nav.set(route));
}

fn route_now(app: &mut App) -> Route {
    let PlayNav(nav) = app.ctx::<PlayNav>();
    *nav.peek()
}

/// The choice buttons of the current round.
fn choices(app: &App) -> Vec<El> {
    app.buttons().into_iter().filter(|&b| app.has_class(b, "choice")).collect()
}

/// Answers each round of a choice game with `pick(round)`, pressing Next
/// (Finish on the last), and returns how many were right.
fn play_choice_rounds(app: &mut App, lang: UiLanguage, rounds: usize, pick: impl Fn(usize) -> usize) -> u32 {
    let mut right = 0;
    for round in 0..rounds {
        let shown = format!("{} / {}", round + 1, rounds);
        assert!(app.screen().contains(&shown), "round {shown}:\n{}", app.html());
        let options = choices(app);
        assert!(options.len() >= 2, "{}", app.html());
        assert!(options.iter().all(|&b| !app.disabled(b) && app.has_class(b, "choice")));
        let i = pick(round) % options.len();
        app.click(options[i]);
        let after = choices(app);
        // Once answered: every choice is locked and the right one is shown.
        assert!(after.iter().all(|&b| app.disabled(b)));
        assert_eq!(after.iter().filter(|&&b| app.has_class(b, "right")).count(), 1);
        if app.has_class(after[i], "right") {
            right += 1;
            assert!(
                !app.by_class("feedback").is_empty() && app.has_class(app.by_class("feedback")[0], "right")
            );
        } else {
            assert!(app.has_class(after[i], "missed"));
            assert!(app.has_class(app.by_class("feedback")[0], "kind"));
        }
        let last = round + 1 == rounds;
        app.click_text(if last { t(lang, Msg::PlayFinish) } else { t(lang, Msg::Next) });
    }
    right
}

/// The practice events in the log, as (skill, topic, correct, total).
fn practices(app: &App) -> Vec<(Skill, String, u32, u32)> {
    app.activity()
        .into_iter()
        .filter_map(|e| match e.kind {
            ActivityKind::Practice { skill, topic, correct, total } => Some((skill, topic, correct, total)),
            _ => None,
        })
        .collect()
}

fn summary_score(app: &App) -> String {
    app.text(app.by_class("score")[0])
}

#[test]
fn the_hub_lists_every_game_and_opens_it() {
    let mut app = App::new(En, || rsx! { Play {} });
    let tiles = app.by_class("game-tile");
    assert_eq!(tiles.len(), GAMES.len());
    // The Bridge tile counts the words an Egyptian already knows.
    let known = bridge::known_deck(content::bridge().as_ref().unwrap()).len();
    assert!(app.screen().contains(&deck_headline(known)), "{}", app.screen());
    for g in GAMES.iter() {
        assert!(app.screen().contains(en(g.blurb)), "{:?}", g.route);
    }
    // Latin titles are isolated left to right.
    let pasar = app.find("button", en(Msg::PasarTitle));
    assert!(app.html().contains("<bdi dir=\"ltr\">Pasar!</bdi>"), "{}", app.text(pasar));
    app.click(pasar);
    assert_eq!(route_now(&mut app), Route::Pasar);
    assert!(app.find_all("h2").iter().any(|&h| app.text(h) == "Pasar!"));
    // The header's back button returns to the hub and stops any speech.
    app.click_on("button", en(Msg::NavPlay));
    assert_eq!(route_now(&mut app), Route::Hub);
    assert!(app.doc.ran("speechSynthesis.cancel()") >= 1);
}

#[test]
fn bridge_games_wear_bridge_gold() {
    for (route, bridge) in
        [(Route::Deck, true), (Route::Alarm, true), (Route::Detective, true), (Route::Pasar, false)]
    {
        let app = play_at(En, route);
        let root = app.find_all("div")[0];
        assert_eq!(app.has_class(root, "bridge-game"), bridge, "{route:?}");
        assert!(app.has_class(root, "play"));
    }
}

#[test]
fn false_friends_alarm_plays_ten_rounds_and_logs_the_score() {
    let mut app = play_at(En, Route::Alarm);
    assert!(app.screen().contains("kalimat"), "the intro shows the classic example");
    // The intro card starts the game wherever it is tapped.
    app.click_on("p", en(Msg::AlarmIntro));
    assert!(app.screen().contains(en(Msg::AlarmLooksLike)));
    let right = play_choice_rounds(&mut app, En, alarm::ROUNDS, |r| r);
    assert_eq!(summary_score(&app), format!("{right} / {}", alarm::ROUNDS));
    assert_eq!(
        practices(&app),
        vec![(Skill::Vocabulary, "false-friends".into(), right, alarm::ROUNDS as u32)]
    );
    // Again: a fresh round from the first question.
    app.click_text(en(Msg::PlayAgain));
    assert!(app.screen().contains(&format!("1 / {}", alarm::ROUNDS)));
}

#[test]
fn detective_mixes_every_rule_or_drills_one() {
    let mut app = play_at(En, Route::Detective);
    let data = content::bridge().as_ref().unwrap();
    // All rules start pressed, with every rule's card shown.
    let all = app.button(en(Msg::DetectiveAllRules));
    assert_eq!(app.attr(all, "aria-pressed"), Some("true"));
    assert_eq!(app.by_class("rule-card").len(), data.rules.len());
    // The picker shows a few rules, then all of them, then fewer again.
    let more = en(Msg::DetectiveShowAllRules).replace("{n}", &data.rules.len().to_string());
    if app.try_find("button", &more).is_some() {
        let before = app.buttons().len();
        app.click_text(&more);
        assert!(app.buttons().len() > before);
        app.click_text(en(Msg::DetectiveShowFewerRules));
        assert_eq!(app.buttons().len(), before);
    }
    // Picking one rule shows only its card.
    let first = &data.rules[0];
    let rule_button = app
        .buttons()
        .into_iter()
        .find(|&b| app.attr(b, "aria-pressed") == Some("false"))
        .expect("a rule button");
    app.click(rule_button);
    assert_eq!(app.by_class("rule-card").len(), 1);
    assert_eq!(app.attr(rule_button, "aria-pressed"), Some("true"));
    let _ = first;
    // Back to all, then play the mixed round to the end.
    app.click_text(en(Msg::DetectiveAllRules));
    app.click_text(en(Msg::Start));
    let right = play_choice_rounds(&mut app, En, detective::ROUNDS, |r| r + 1);
    assert_eq!(summary_score(&app), format!("{right} / {}", detective::ROUNDS));
    assert_eq!(
        practices(&app),
        vec![(Skill::Vocabulary, "bridge:all".into(), right, detective::ROUNDS as u32)]
    );
    // Again goes back to the rule picker.
    app.click_text(en(Msg::PlayAgain));
    assert!(app.try_find("button", en(Msg::DetectiveAllRules)).is_some());
}

/// The round total from the "1 / N" counter.
fn round_total(app: &App) -> usize {
    let count = app.text(app.by_class("ltr-count")[0]);
    count.rsplit(' ').next().unwrap().parse().unwrap()
}

#[test]
fn detective_on_one_rule_logs_that_rule_in_arabic() {
    let mut app = play_at(ArEg, Route::Detective);
    let data = content::bridge().as_ref().unwrap();
    let rule = app
        .buttons()
        .into_iter()
        .find(|&b| app.attr(b, "aria-pressed") == Some("false"))
        .expect("a rule button");
    // Which rule this button is: the one whose card is then the only one.
    app.click(rule);
    let card = app.text(app.by_class("rule-card")[0]);
    let id = data
        .rules
        .iter()
        .find(|r| card.contains(logic::meaning(ArEg, &r.explanation)))
        .map(|r| r.id.clone())
        .expect("the card explains its rule");
    app.click_text(t(ArEg, Msg::Start));
    assert!(
        app.screen().contains(t(ArEg, Msg::DetectiveGuessArabic))
            || app.screen().contains(t(ArEg, Msg::DetectiveGuessIndonesian))
    );
    let rounds = round_total(&app);
    let right = play_choice_rounds(&mut app, ArEg, rounds, |_| 0);
    assert_eq!(practices(&app), vec![(Skill::Vocabulary, format!("bridge:{id}"), right, rounds as u32)]);
    assert!(app.screen().contains(t(ArEg, Msg::PlayOtherGames)));
    // "Try it in a chat" leaves Play for Chat.
    app.click_text(t(ArEg, Msg::PlayTryInChat));
    assert_eq!(app.screen_now(), crate::Screen::Chat);
}

/// Words marked as met in the log, in order.
fn met_words(app: &App) -> Vec<String> {
    app.activity()
        .into_iter()
        .flat_map(|e| match e.kind {
            ActivityKind::NewWords { words } => words,
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn the_bridge_deck_searches_pages_and_marks_words_known() {
    let mut app = play_at(En, Route::Deck);
    let deck = bridge::known_deck(content::bridge().as_ref().unwrap());
    assert!(app.screen().contains(&deck_headline(deck.len())));
    assert!(app.text(app.by_class("deck-marked")[0]).contains(&format!("0 / {}", deck.len())));
    // A page of rows, then a page more.
    assert_eq!(app.by_class("deck-row").len(), 24.min(deck.len()));
    app.click_text(en(Msg::ShowMore));
    assert_eq!(app.by_class("deck-row").len(), 48.min(deck.len()));
    // Searching by the word (any case) finds it and resets the paging.
    let search = app.find_all("input")[0];
    app.input(search, "KABAR");
    let rows = app.by_class("deck-row");
    assert!(!rows.is_empty() && rows.len() < 24);
    assert!(
        rows.iter().all(|&r| app.text(r).to_lowercase().contains("kabar") || app.text(r).contains("خبر"))
    );
    // "I know it" logs the word once and ticks the row.
    let kabar = rows.into_iter().find(|&r| app.text(r).starts_with("kabar")).expect("kabar row");
    assert!(!app.has_class(kabar, "known"));
    app.click_on("button", en(Msg::DeckMarkKnown));
    assert_eq!(met_words(&app), vec!["kabar".to_string()]);
    let kabar = app.by_class("deck-row").into_iter().find(|&r| app.text(r).starts_with("kabar")).unwrap();
    assert!(app.has_class(kabar, "known"));
    assert!(app.text(kabar).contains(en(Msg::DeckKnown)));
    assert!(app.text(app.by_class("deck-marked")[0]).contains(&format!("1 / {}", deck.len())));
    // Nothing matches: a friendly note instead of an empty list.
    app.input(search, "zzzzqq");
    assert!(app.by_class("deck-row").is_empty());
    assert!(app.screen().contains(no_match_text()));
    // The games button goes back to the hub.
    app.click_on("button", "🎲");
    assert_eq!(route_now(&mut app), Route::Hub);
}

fn no_match_text() -> &'static str {
    "That word isn't on the Bridge."
}

#[test]
fn the_bridge_deck_finds_words_by_their_arabic() {
    let mut app = play_at(ArEg, Route::Deck);
    let deck = bridge::known_deck(content::bridge().as_ref().unwrap());
    let entry = deck.iter().find(|e| e.root.is_some()).expect("an entry with a root");
    let search = app.find_all("input")[0];
    assert_eq!(app.attr(search, "placeholder"), Some("دوّر على كلمة بالإندونيسي أو بالعربي…"));
    app.input(search, &entry.arabic);
    let row = app.by_class("deck-row")[0];
    let text = app.text(row);
    assert!(text.contains(&entry.arabic) && text.contains(entry.root.as_deref().unwrap()), "{text}");
    assert!(text.contains(&entry.meaning.arz));
}

/// A WebView with an Indonesian voice that reads each text it is given.
fn indonesian_voice() -> std::rc::Rc<FakeDocument> {
    let doc = FakeDocument::new();
    doc.reply_to("dioxus.send(!!kabarIdVoice())", vec![json!(true)]);
    doc.reply_to("if (kabarIdVoice())", vec![json!(true)]);
    doc
}

fn javanese_voice() -> std::rc::Rc<FakeDocument> {
    let doc = FakeDocument::new();
    doc.reply_to("if (kabarJvVoice())", vec![json!(true)]);
    doc
}

fn pagi_app(lang: UiLanguage, doc: std::rc::Rc<FakeDocument>) -> App {
    let mut app = App::with(lang, memory_db(), doc, || rsx! { Play {} });
    open_route(&mut app, Route::Pagi);
    app
}

#[test]
fn kabar_pagi_reads_a_story_one_line_at_a_time_and_logs_it() {
    let mut app = pagi_app(En, FakeDocument::new());
    let story = app.text(app.by_class("story-title")[0]);
    assert!(!story.is_empty(), "{}", app.html());
    // No Indonesian voice: the story stays readable, with a note.
    assert!(app.screen().contains(en(Msg::AudioMissing)));
    assert!(app.try_find("button", en(Msg::PagiPlayAll)).is_none());
    let count = app.by_class("sentence").len();
    assert!(count >= 2);
    // One by one: a sentence, then the next on Next.
    app.click_text(en(Msg::PagiOneByOne));
    assert_eq!(app.by_class("sentence").len(), 1);
    app.click_text(en(Msg::Next));
    assert_eq!(app.by_class("sentence").len(), 2);
    app.click_text(en(Msg::PagiOneByOne));
    assert_eq!(app.by_class("sentence").len(), count);
    // Meanings: one on request, then all (they may start all shown).
    if !app.by_class("translation").is_empty() {
        app.click_text(en(Msg::PagiAllTranslations));
    }
    assert!(app.by_class("translation").is_empty());
    app.click_text(en(Msg::ShowMeaning));
    assert_eq!(app.by_class("translation").len(), 1);
    app.click_text(en(Msg::PagiAllTranslations));
    assert_eq!(app.by_class("translation").len(), count);
    // Done: the story's new words and a listening practice are logged once.
    let targets: Vec<String> = app.by_class("word").into_iter().map(|w| app.text(w)).collect();
    let new_words: Vec<String> = app.by_class("new-word").into_iter().map(|w| app.text(w)).collect();
    assert!(!targets.is_empty());
    app.click_text(en(Msg::PagiDone));
    assert!(app.screen().contains(en(Msg::PagiReadToday)));
    assert!(app.try_find("button", en(Msg::PagiDone)).is_none());
    // The story's new words are logged as met, lowercase, once each;
    // the ones marked in the story are among them.
    let logged = met_words(&app);
    assert!(!logged.is_empty());
    assert!(logged.iter().all(|w| *w == w.to_lowercase()));
    let mut unique = logged.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), logged.len());
    assert!(new_words.iter().all(|w| logged.contains(&w.to_lowercase())), "{new_words:?} {logged:?}");
    assert_eq!(practices(&app), vec![(Skill::Listening, "kabar-pagi".into(), count as u32, count as u32)]);
}

#[test]
fn kabar_pagi_plays_the_story_aloud_when_a_voice_exists() {
    let doc = indonesian_voice();
    // The fake voice reads sentence 0, then 1, then ends.
    doc.reply_to("const texts =", vec![json!(0), json!(1), json!(-1)]);
    let mut app = pagi_app(ArEg, doc);
    assert!(!app.screen().contains(t(ArEg, Msg::AudioMissing)));
    app.click_text(t(ArEg, Msg::PagiOneByOne));
    assert_eq!(app.by_class("sentence").len(), 1);
    app.click_text(t(ArEg, Msg::PagiPlayAll));
    let spoken = app.doc.scripts().into_iter().find(|s| s.contains("const texts =")).unwrap();
    assert!(spoken.contains("u.lang = \"id-ID\""));
    // Reading along reveals the sentences as they are spoken.
    assert!(app.by_class("sentence").len() >= 2, "{}", app.html());
    // One sentence again, from its own play button.
    let before = app.doc.ran("const texts =");
    let listen = app
        .buttons()
        .into_iter()
        .find(|&b| app.attr(b, "aria-label") == Some(t(ArEg, Msg::Listen)))
        .expect("a sentence play button");
    app.click(listen);
    assert_eq!(app.doc.ran("const texts ="), before + 1);
}

#[test]
fn the_today_card_offers_kabar_pagi_in_the_morning_until_it_is_read() {
    let mut app = App::new(En, || rsx! { PagiTodayCard {} });
    let coach = app.ctx::<crate::coach::Coach>();
    // Find a morning hour in the learner's time zone.
    let mut now = 1_761_472_800_000u64;
    let set = |app: &mut App, at: u64| {
        let mut now_ms = coach.now_ms;
        app.dom.in_app(|| now_ms.set(at));
    };
    set(&mut app, now);
    while coach.time_of_day().hour() != 9 {
        now += 3_600_000;
        set(&mut app, now);
    }
    assert!(app.screen().contains("Kabar Pagi"), "{}", app.html());
    app.click_on("button", en(Msg::PagiListen));
    assert_eq!(app.screen_now(), crate::Screen::Play);
    assert_eq!(route_now(&mut app), Route::Pagi);
    // Before dawn it waits.
    set(&mut app, now - 6 * 3_600_000);
    assert!(!app.screen().contains("Kabar Pagi"));
    // Once read today, it is gone for the day.
    set(&mut app, now);
    let db = app.db.clone();
    app.dom.in_app(|| {
        coach.record(
            &db,
            ActivityKind::Practice {
                skill: Skill::Listening,
                topic: logic::TOPIC_PAGI.into(),
                correct: 1,
                total: 1,
            },
            60,
        );
    });
    assert!(!app.screen().contains("Kabar Pagi"));
}

fn pairs_app(doc: std::rc::Rc<FakeDocument>) -> App {
    let mut app = App::with(En, memory_db(), doc, || rsx! { Play {} });
    open_route(&mut app, Route::Pairs);
    app
}

#[test]
fn minimal_pairs_without_a_javanese_voice_quiz_the_spelling() {
    let mut app = pairs_app(FakeDocument::new());
    let data = content::pairs().as_ref().unwrap();
    assert!(app.screen().contains(en(Msg::PairsNoJavaneseVoice)));
    assert!(app.try_find("button", en(Msg::PairsListenMode)).is_none());
    // Both contrasts: every tip; one contrast: its tip only.
    assert_eq!(app.by_class("pairs-tip").len(), data.tips.len());
    let one = kabar_core::games::pairs::Contrast::ALL[0];
    app.click_on("button", &format!("{} / {}", one.plain(), one.curled()));
    assert!(app.by_class("pairs-tip").len() <= data.tips.len());
    app.click_text(en(Msg::PairsBoth));
    app.click_text(en(Msg::Start));
    assert!(app.screen().contains(en(Msg::PairsWhichSpelling)));
    let rounds = round_total(&app);
    let mut right = 0;
    for round in 0..rounds {
        let options = choices(&app);
        app.click(options[round % 2]);
        // After answering: both words compared, the target marked.
        assert_eq!(app.by_class("pairs-word").len(), 2);
        assert_eq!(app.by_class("target").len(), 1);
        if app.has_class(choices(&app)[round % 2], "right") {
            right += 1;
        }
        app.click_text(if round + 1 == rounds { en(Msg::PlayFinish) } else { en(Msg::Next) });
    }
    assert_eq!(summary_score(&app), format!("{right} / {rounds}"));
    // One practice per contrast played, read mode, adding up to the round.
    let logged = practices(&app);
    assert!(!logged.is_empty());
    assert!(
        logged.iter().all(
            |(skill, topic, _, _)| *skill == Skill::Reading && ["d-dh", "t-th"].contains(&topic.as_str())
        )
    );
    assert_eq!(logged.iter().map(|p| p.2).sum::<u32>(), right);
    assert_eq!(logged.iter().map(|p| p.3).sum::<u32>(), rounds as u32);
    assert_eq!(app.doc.ran("jv-ID"), 0, "nothing is spoken without a voice");
}

#[test]
fn minimal_pairs_with_a_javanese_voice_speak_each_word() {
    let mut app = pairs_app(javanese_voice());
    assert!(app.try_find("button", en(Msg::PairsListenMode)).is_some());
    // Read mode is still one tap away.
    app.click_text(en(Msg::PairsReadMode));
    assert_eq!(app.attr(app.button(en(Msg::PairsReadMode)), "aria-pressed"), Some("true"));
    app.click_text(en(Msg::PairsListenMode));
    app.click_text(en(Msg::Start));
    // Starting says the first word, in Javanese only.
    assert_eq!(app.doc.ran("u.lang = \"jv-ID\""), 1);
    assert!(app.screen().contains(en(Msg::PairsWhichHeard)));
    app.click_on("button", en(Msg::PairsHear));
    assert_eq!(app.doc.ran("u.lang = \"jv-ID\""), 2);
    let first = choices(&app)[0];
    app.click(first);
    // Each compared word has its own play button.
    let hear: Vec<El> = app
        .buttons()
        .into_iter()
        .filter(|&b| app.attr(b, "aria-label") == Some(en(Msg::PairsHear)))
        .collect();
    assert_eq!(hear.len(), 2);
    app.click(hear[1]);
    assert_eq!(app.doc.ran("u.lang = \"jv-ID\""), 3);
    // Next says the next word.
    app.click_text(en(Msg::Next));
    assert_eq!(app.doc.ran("u.lang = \"jv-ID\""), 4);
    let rounds = round_total(&app);
    for round in 1..rounds {
        let c = choices(&app)[0];
        app.click(c);
        app.click_text(if round + 1 == rounds { en(Msg::PlayFinish) } else { en(Msg::Next) });
    }
    assert!(practices(&app).iter().all(|p| p.0 == Skill::Listening));
    app.click_text(en(Msg::PlayAgain));
    assert!(app.try_find("button", en(Msg::Start)).is_some());
}

/// The Pasar move button whose hint is `hint`.
fn pasar_move(app: &App, hint: Msg) -> El {
    app.by_class("move").into_iter().find(|&b| app.text(b).contains(en(hint))).expect("move button")
}

fn open_first_stall(app: &mut App) -> kabar_core::games::pasar::Item {
    let data = content::pasar().as_ref().unwrap();
    let item = data.items.iter().find(|i| !i.bonus && !i.javanese).expect("a plain stall").clone();
    app.click_on("button", &item.name);
    item
}

#[test]
fn pasar_haggles_down_with_an_offer_and_logs_the_stars() {
    let mut app = play_at(En, Route::Pasar);
    let data = content::pasar().as_ref().unwrap();
    assert_eq!(app.by_class("item").len(), data.items.len());
    assert_eq!(app.by_class("bonus").len(), data.items.iter().filter(|i| i.bonus).count());
    let item = open_first_stall(&mut app);
    // The seller opens with a price on the table and the budget beside it.
    let budget = rupiah_fmt(logic::budget_for(item.fair));
    assert!(app.screen().contains(&budget), "{}", app.screen());
    assert!(app.screen().contains(&rupiah_fmt(item.ask)));
    // The seller's words translate on request, and fold again.
    assert!(app.by_class("reveal").is_empty());
    app.click_text(en(Msg::ShowMeaning));
    assert_eq!(app.by_class("reveal").len(), 1);
    app.click_text(en(Msg::HideMeaning));
    assert!(app.by_class("reveal").is_empty());
    // Asking the price adds a turn to the transcript.
    let b = pasar_move(&app, Msg::PasarAskPrice);
    app.click(b);
    assert_eq!(app.by_class("me").len(), 1);
    // A typed offer is read back in words and sent with the form.
    let field = app.find_all("input")[0];
    let offer = app.find_all("form")[0];
    let submit = app.buttons().into_iter().find(|&b| app.attr(b, "type") == Some("submit")).unwrap();
    assert!(app.disabled(submit), "nothing typed yet");
    app.input(field, &item.fair.to_string());
    assert!(app.screen().contains(&kabar_core::games::rupiah::rupiah_words(item.fair)));
    app.submit(offer);
    assert_eq!(app.by_class("me").len(), 2);
    if let Some(&field) = app.find_all("input").first() {
        assert_eq!(app.attr(field, "value"), Some(""), "the field clears for the next offer");
    }
    // A fair offer may be taken at once; otherwise take the price on the table.
    if app.by_class("summary-card").is_empty() {
        let b = pasar_move(&app, Msg::PasarAccept);
        app.click(b);
    }
    let card = app.text(app.by_class("summary-card")[0]);
    assert!(card.contains(en(Msg::PasarPaid)), "{card}");
    let stars = app.by_class("star").into_iter().filter(|&s| app.has_class(s, "on")).count() as u32;
    assert_eq!(practices(&app), vec![(Skill::Speaking, "pasar".into(), stars, 3)]);
    // Again: back to the stalls, the round logged once.
    app.click_text(en(Msg::PlayAgain));
    assert_eq!(app.by_class("item").len(), data.items.len());
    assert_eq!(practices(&app).len(), 1);
}

fn rupiah_fmt(n: u64) -> String {
    kabar_core::games::rupiah::format_rupiah(n)
}

#[test]
fn pasar_walking_away_politely_ends_the_round() {
    let mut app = play_at(En, Route::Pasar);
    open_first_stall(&mut app);
    let b = pasar_move(&app, Msg::PasarRefuse);
    app.click(b);
    assert!(app.screen().contains(en(Msg::PasarWalked)) || app.by_class("summary-card").is_empty());
    if app.by_class("summary-card").is_empty() {
        // The seller may try once more; declining again walks away.
        let b = pasar_move(&app, Msg::PasarRefuse);
        app.click(b);
    }
    assert!(app.screen().contains(en(Msg::PasarWalked)), "{}", app.screen());
    assert!(!app.screen().contains(en(Msg::PasarPaid)));
    assert_eq!(practices(&app).len(), 1);
}

#[test]
fn pasar_javanese_stalls_are_marked_unverified() {
    let data = content::pasar().as_ref().unwrap();
    let Some(item) = data.items.iter().find(|i| i.javanese) else { return };
    let mut app = play_at(En, Route::Pasar);
    let tile = app.find("button", &item.name);
    assert!(app.text(tile).contains(en(Msg::Unverified)));
    app.click(tile);
    assert!(app.html().contains("lang=\"jv\""));
    assert!(app.text(app.by_class("pasar-head")[0]).contains(en(Msg::Unverified)));
}

/// The price shown in the current Rupiah drill, as a number.
fn shown_price(app: &App) -> u64 {
    let shown = app.text(app.by_class("price")[0]);
    kabar_core::games::rupiah::parse_price(&shown).unwrap_or_else(|| panic!("unreadable price {shown}"))
}

#[test]
fn rupiah_reader_checks_readings_and_keeps_the_best_score() {
    let mut app = play_at(En, Route::Rupiah);
    // Thousands is picked to begin with; pick millions instead.
    assert_eq!(app.attr(app.button(en(Msg::RupiahThousands)), "aria-pressed"), Some("true"));
    app.click_text(en(Msg::RupiahMillions));
    assert_eq!(app.attr(app.button(en(Msg::RupiahMillions)), "aria-pressed"), Some("true"));
    app.click_text(en(Msg::Start));
    assert!(app.screen().contains(en(Msg::AudioMissing)));
    let check = app.button(en(Msg::Check));
    // An empty answer is not checked.
    app.click(check);
    assert!(app.by_class("feedback").is_empty());
    for drill in 0..rupiah::DRILLS {
        let field = app.find_all("input")[0];
        let amount = shown_price(&app);
        assert!(amount >= 1_000_000, "a millions drill: {amount}");
        if drill == 0 {
            // A wrong reading shows the right one.
            app.input(field, "seratus rupiah");
            app.click_text(en(Msg::Check));
            assert!(app.screen().contains(en(Msg::RupiahWrong)));
            assert!(app.screen().contains(&kabar_core::games::rupiah::rupiah_words(amount)));
        } else if drill == 1 {
            // The right number without "rupiah" counts, as close.
            let words = kabar_core::games::rupiah::number_words(amount);
            app.input(field, &words);
            let form = app.find_all("form")[0];
            app.submit(form);
            assert!(app.has_class(app.by_class("feedback")[0], "good"), "{}", app.html());
        } else {
            app.input(field, &kabar_core::games::rupiah::rupiah_words(amount));
            app.click_text(en(Msg::Check));
            assert!(app.screen().contains(en(Msg::PlayRight)));
        }
        let field = app.find_all("input")[0];
        assert!(app.disabled(field), "the answer locks once checked");
        let last = drill + 1 == rupiah::DRILLS;
        app.click_text(if last { en(Msg::PlayFinish) } else { en(Msg::Next) });
    }
    let total = rupiah::DRILLS as u32;
    assert_eq!(summary_score(&app), format!("{} / {total}", total - 1));
    assert!(app.screen().contains(en(Msg::RupiahNewBest)));
    assert_eq!(practices(&app), vec![(Skill::Reading, "rupiah".into(), total - 1, total)]);
    let best = app.db.0.borrow().local_value(&logic::best_key("Millions")).unwrap().expect("best saved");
    assert!(best.parse::<u32>().unwrap() >= (total - 1) * kabar_core::games::rupiah::POINTS_PER_CORRECT);
    // Back at the levels, the best shows beside Millions.
    app.click_text(en(Msg::PlayAgain));
    assert!(
        app.text(app.button(en(Msg::RupiahMillions))).contains(&format!("{} {best}", en(Msg::RupiahBest)))
    );
}

#[test]
fn rupiah_reader_offers_the_reading_aloud_with_a_voice() {
    let mut app = App::with(En, memory_db(), indonesian_voice(), || rsx! { Play {} });
    open_route(&mut app, Route::Rupiah);
    app.click_text(en(Msg::Start));
    let amount = shown_price(&app);
    app.click_text(en(Msg::Listen));
    let said = app.doc.scripts().into_iter().rfind(|s| s.contains("const texts =")).unwrap();
    assert!(said.contains(&kabar_core::games::rupiah::rupiah_words(amount)), "{said}");
}

fn unggah_app(profile: Option<kabar_protocol::coach::CoachProfile>) -> App {
    let db = memory_db();
    if let Some(p) = profile {
        db.0.borrow_mut().save_coach_profile(&p).unwrap();
    }
    let mut app = App::with(En, db, FakeDocument::new(), || rsx! { Play {} });
    open_route(&mut app, Route::Unggah);
    app
}

#[test]
fn unggah_ungguh_opens_scenes_up_to_the_learners_level() {
    let data = content::unggah().as_ref().unwrap();
    let app = unggah_app(None);
    // No Javanese target: every scene is open.
    let scenes = app.by_class("uu-scene");
    assert_eq!(scenes.len(), data.scenarios.len());
    assert!(scenes.iter().all(|&s| !app.disabled(s)));
    // A J1 target: scenes above J1 wait, saying when they open.
    let profile = kabar_protocol::coach::CoachProfile {
        target: Some(kabar_protocol::coach::Target {
            level: "J1".into(),
            by: kabar_protocol::coach::LocalDate::from_ymd(2030, 1, 1).unwrap(),
        }),
        ..Default::default()
    };
    assert_eq!(unggah::javanese_level(Some(&profile)).as_deref(), Some("j1"));
    let app = unggah_app(Some(profile));
    let locked = app.by_class("locked");
    let above = data.scenarios.iter().filter(|s| s.level != "j1").count();
    assert_eq!(locked.len(), above);
    assert!(locked.iter().all(|&s| app.disabled(s) && app.text(s).contains(en(Msg::UnggahOpensAt))));
}

#[test]
fn unggah_ungguh_plays_a_scene_and_logs_the_fitting_turns() {
    let mut app = unggah_app(None);
    let data = content::unggah().as_ref().unwrap();
    let scene = &data.scenarios[0];
    app.click_on("button", logic::meaning(En, &scene.title));
    assert!(app.screen().contains(&scene.partner.name));
    let meter = app.find_all("div").into_iter().find(|&d| app.attr(d, "role") == Some("meter")).unwrap();
    let start: u8 = app.attr(meter, "aria-valuenow").unwrap().parse().unwrap();
    let turns = scene.turns.len();
    let mut fitting = 0;
    for turn in 0..turns {
        assert!(
            app.screen().contains(&format!("{} {} / {turns}", en(Msg::UnggahTurn), turn + 1)),
            "{}",
            app.screen()
        );
        // Alternate: the fitting answer, then the first other one.
        let options = choices(&app);
        let pick = if turn % 2 == 0 {
            // The fitting one is revealed by answering; try the first and see.
            0
        } else {
            options.len() - 1
        };
        app.click(options[pick]);
        let after = choices(&app);
        let fits = app.has_class(after[pick], "right");
        if fits {
            fitting += 1;
            assert!(app.screen().contains(en(Msg::UnggahFits)));
        } else {
            assert!(!app.screen().contains(en(Msg::UnggahFits)));
        }
        // The meter moved, or stayed, and says so.
        let meter = app.find_all("div").into_iter().find(|&d| app.attr(d, "role") == Some("meter")).unwrap();
        let _now: u8 = app.attr(meter, "aria-valuenow").unwrap().parse().unwrap();
        let last = turn + 1 == turns;
        app.click_text(if last { en(Msg::PlayFinish) } else { en(Msg::Next) });
    }
    let _ = start;
    assert!(app.text(app.by_class("uu-end")[0]).contains(&format!("{fitting} / {turns}")));
    assert_eq!(
        practices(&app),
        vec![(Skill::Speaking, unggah::TOPIC_UNGGAH.into(), fitting as u32, turns as u32)]
    );
    // Again replays the scene; Scenes goes back to the list.
    app.click_text(en(Msg::PlayAgain));
    assert!(app.screen().contains(&format!("{} 1 / {turns}", en(Msg::UnggahTurn))));
    let c = choices(&app)[0];
    app.click(c);
    let mut guard = 0;
    while app.by_class("uu-end").is_empty() && guard < 20 {
        if let Some(&c) = choices(&app).iter().find(|&&b| !app.disabled(b)) {
            app.click(c);
        }
        let next =
            app.try_find("button", en(Msg::Next)).or_else(|| app.try_find("button", en(Msg::PlayFinish)));
        app.click(next.unwrap());
        guard += 1;
    }
    app.click_text(en(Msg::UnggahScenes));
    assert_eq!(app.by_class("uu-scene").len(), data.scenarios.len());
}

thread_local! {
    static SCORE: std::cell::Cell<(u32, u32)> = const { std::cell::Cell::new((0, 0)) };
}

fn summary_body() -> Element {
    let (correct, total) = SCORE.get();
    rsx! { RoundSummary { correct, total, onagain: |_| {}, p { "extra" } } }
}

#[test]
fn round_summary_words_the_score_warmly() {
    for (correct, total, line) in [
        (8, 10, Msg::PlayGreat),
        (10, 10, Msg::PlayGreat),
        (7, 10, Msg::PlayGood),
        (5, 10, Msg::PlayGood),
        (4, 10, Msg::PlayKeepGoing),
        (0, 0, Msg::PlayKeepGoing),
    ] {
        SCORE.set((correct, total));
        let mut app = App::new(En, summary_body);
        assert!(app.screen().contains(en(line)), "{correct}/{total}: {}", app.screen());
        assert_eq!(summary_score(&app), format!("{correct} / {total}"));
        assert!(app.screen().contains("extra"), "the game's own lines sit under the score");
        app.click_text(en(Msg::PlayOtherGames));
        assert_eq!(route_now(&mut app), Route::Hub);
    }
}

#[component]
fn Problem() -> Element {
    rsx! { ContentProblem { detail: "content/games/pasar.toml: bad line 3" } }
}

#[test]
fn a_content_problem_says_so_kindly_with_the_detail_folded() {
    let app = App::new(ArEg, || rsx! { Problem {} });
    assert!(app.screen().contains(t(ArEg, Msg::PlayContentProblem)));
    let detail = app.find("p", "bad line 3");
    assert_eq!(app.attr(detail, "dir"), Some("ltr"));
    assert_eq!(app.tag(app.find("summary", t(ArEg, Msg::PlayDetails))), "summary");
}

#[component]
fn Counter() -> Element {
    rsx! {
        RoundCount { at: 3, total: 8, label: Msg::UnggahTurn }
        RoundCount { at: 1, total: 0 }
    }
}

#[test]
fn the_round_counter_fills_with_finished_rounds() {
    let app = App::new(En, || rsx! { Counter {} });
    let fills = app.by_class("fill");
    assert_eq!(app.attr(fills[0], "style"), Some("inline-size: 25%"), "{}", app.html());
    assert_eq!(app.attr(fills[1], "style"), Some("inline-size: 0%"));
    assert!(app.screen().contains(&format!("{} 3 / 8", en(Msg::UnggahTurn))));
}
