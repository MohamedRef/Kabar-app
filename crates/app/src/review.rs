//! Review screens (SPEC §16): classic cards, the Card Wall, and one-minute
//! bursts, on top of `kabar_core::review` (queues, stats) and the FSRS
//! scheduler. Card state is always derived from the review log.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use dioxus::prelude::*;
use kabar_core::layout::WidthClass;
use kabar_core::locale::UiLanguage;
use kabar_core::review::{
    AnswerMatch, BURST_MS, Deck, Session, Span, Summary, cloze, compare_answer, is_quick, seed_bridge_deck,
    seed_words, wall_rating,
};
use kabar_core::srs::{Pile, Scheduler};
use kabar_protocol::coach::ActivityKind;
use kabar_protocol::lesson::Localized;
use kabar_protocol::srs::{Card, CardKind, MeaningShift, Rating, ReviewMode, StudyLanguage};
use serde::Deserialize;

use crate::Screen;
use crate::annotated::Annotated;
use crate::coach::Coach;
use crate::embedded::{Css, css};
use crate::i18n::Msg;
use crate::platform::{self, SpeakError};
use crate::pron::{Pron, Say};
use crate::store::Store;
use crate::{Db, Ui};

const REVIEW_CSS: Css = css!("assets/review.css");
const WALL_JS: &str = include_str!("../assets/wall.js");

/// Classic-review keys, from anywhere on the page: Space reveals, 1–4 rate,
/// Ctrl/⌘+Z undoes. Never while typing an answer. Space is taken over on
/// buttons too, so it can't re-press a focused button by accident.
const KEYS_JS: &str = r#"
if (window.__kabarKeys) window.__kabarKeys();
const typing = (t) =>
  t && (t.tagName === "TEXTAREA" || (t.tagName === "INPUT" && !t.readOnly) || t.isContentEditable);
const down = (e) => {
  if (!document.getElementById("review-classic")) return off();
  if (e.altKey || e.repeat || typing(e.target)) return;
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
    e.preventDefault();
    return dioxus.send("undo");
  }
  if (e.ctrlKey || e.metaKey) return;
  if (e.key === " " || /^[1-4]$/.test(e.key)) {
    e.preventDefault();
    dioxus.send(e.key);
  }
};
const up = (e) => {
  if (e.key === " " && !typing(e.target) && document.getElementById("review-classic")) e.preventDefault();
};
document.addEventListener("keydown", down);
document.addEventListener("keyup", up);
function off() {
  document.removeEventListener("keydown", down);
  document.removeEventListener("keyup", up);
  if (window.__kabarKeys === off) window.__kabarKeys = null;
}
window.__kabarKeys = off;
await new Promise(() => {});
"#;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Home,
    Classic,
    Wall,
    Burst,
    Done,
}

/// Review state shared by the review views.
#[derive(Clone)]
struct Engine {
    db: Db,
    coach: Coach,
    sched: Rc<Scheduler>,
    deck: Signal<Deck>,
    session: Signal<Option<Session>>,
    view: Signal<View>,
    summary: Signal<Option<Summary>>,
    error: Signal<Option<Msg>>,
}

impl PartialEq for Engine {
    fn eq(&self, other: &Self) -> bool {
        self.db == other.db && Rc::ptr_eq(&self.sched, &other.sched)
    }
}

fn read_deck(db: &Db, sched: &Scheduler) -> Deck {
    let store = db.0.borrow();
    let cards = store.cards().unwrap_or_else(|e| {
        crate::diag::report!("reading cards failed: {e}");
        Vec::new()
    });
    let log = store.review_log().unwrap_or_else(|e| {
        crate::diag::report!("reading the review log failed: {e}");
        Vec::new()
    });
    Deck::new(cards, log, sched)
}

/// The last deck built, with what it was built from. The weak handle keeps
/// a dropped store's address from being mistaken for a new one.
struct CachedDeck {
    db: Weak<RefCell<Store>>,
    stamp: (u64, i64),
    /// Held so its address can't be reused by a different scheduler.
    sched: Rc<Scheduler>,
    deck: Rc<Deck>,
}

thread_local! {
    static DECK_CACHE: RefCell<Option<CachedDeck>> = const { RefCell::new(None) };
}

/// The deck, replayed from the store only when the store or the scheduler
/// changed since the last call (#116): Today, Kabar Pagi, the plan and
/// Review all ask for it, and parsing plus replaying a year of reviews is
/// not free on a phone. `sched` comes from [`crate::schedule`], which hands
/// out the same `Rc` while its inputs stay the same.
pub(crate) fn deck(db: &Db, sched: &Rc<Scheduler>) -> Rc<Deck> {
    let stamp = match db.0.borrow().change_stamp() {
        Ok(stamp) => stamp,
        Err(e) => {
            crate::diag::report!("reading the store's change stamp failed: {e}");
            return Rc::new(read_deck(db, sched));
        }
    };
    let hit = DECK_CACHE.with_borrow(|c| {
        c.as_ref()
            .filter(|c| {
                c.stamp == stamp
                    && Rc::ptr_eq(&c.sched, sched)
                    && c.db.upgrade().is_some_and(|s| Rc::ptr_eq(&s, &db.0))
            })
            .map(|c| c.deck.clone())
    });
    hit.unwrap_or_else(|| {
        let deck = Rc::new(read_deck(db, sched));
        DECK_CACHE.set(Some(CachedDeck {
            db: Rc::downgrade(&db.0),
            stamp,
            sched: sched.clone(),
            deck: deck.clone(),
        }));
        deck
    })
}

/// Turns what a lesson or the Reader showed about deck words into review
/// events (see `kabar_core::review::evidence` for the rules), so each word's
/// schedule follows how well the learner really knows it. Returns how many
/// events were recorded.
pub(crate) fn note_evidence(
    db: &Db,
    coach: &Coach,
    language: StudyLanguage,
    signals: &[(String, kabar_core::review::Signal)],
) -> usize {
    note_evidence_at(db, coach.offset(), language, signals, now_ms())
}

fn note_evidence_at(
    db: &Db,
    utc_offset_min: i16,
    language: StudyLanguage,
    signals: &[(String, kabar_core::review::Signal)],
    now: u64,
) -> usize {
    if signals.is_empty() {
        return 0;
    }
    let settings = db.0.borrow().settings().unwrap_or_default();
    let sched = crate::schedule::scheduler_at(utc_offset_min, &settings);
    let events = kabar_core::review::evidence_reviews(&sched, &deck(db, &sched), language, signals, now);
    let mut store = db.0.borrow_mut();
    let mut recorded = 0;
    for (card, rating, evidence) in events {
        match store.record_evidence(&card, rating, now, evidence) {
            Ok(_) => recorded += 1,
            Err(e) => crate::diag::report!("recording {evidence:?} for {card} failed: {e}"),
        }
    }
    recorded
}

/// An owned deck for a review session to update as answers come in.
pub(crate) fn load_deck(db: &Db, sched: &Rc<Scheduler>) -> Deck {
    Deck::clone(&deck(db, sched))
}

/// Words of the cards due now, least likely to be remembered first, without
/// duplicates (a word can have several card kinds). Feeds the daily plan and
/// Kabar Pagi.
pub(crate) fn due_words(db: &Db, sched: &Rc<Scheduler>, now_ms: u64) -> Vec<String> {
    let deck = deck(db, sched);
    let mut due: Vec<(f32, &Card)> = deck
        .due(sched, now_ms)
        .into_iter()
        .map(|c| (sched.retrievability(&deck.state(&c.id), now_ms).unwrap_or(0.0), c))
        .collect();
    due.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
    let mut words: Vec<String> = Vec::new();
    for (_, c) in due {
        let w = c.word.to_lowercase();
        if !words.contains(&w) {
            words.push(w);
        }
    }
    words
}

fn fits(mode: ReviewMode) -> fn(&Card) -> bool {
    match mode {
        ReviewMode::Classic => |_| true,
        ReviewMode::CardWall | ReviewMode::Burst => is_quick,
    }
}

impl Engine {
    fn queue_len(&self, mode: ReviewMode, now: u64) -> usize {
        self.deck.read().queue(&self.sched, now, fits(mode)).len()
    }

    fn start(&self, mode: ReviewMode) {
        let now = now_ms();
        let queue = self.deck.read().queue(&self.sched, now, fits(mode));
        let (mut session, mut view, mut error) = (self.session, self.view, self.error);
        session.set(Some(Session::new(mode, queue, now)));
        error.set(None);
        view.set(match mode {
            ReviewMode::Classic => View::Classic,
            ReviewMode::CardWall => View::Wall,
            ReviewMode::Burst => View::Burst,
        });
    }

    fn pull(&self) -> Option<String> {
        let mut session = self.session;
        session.write().as_mut().and_then(Session::pull)
    }

    fn put_back(&self, card_id: String) {
        let mut session = self.session;
        if let Some(s) = session.write().as_mut() {
            s.put_back(card_id);
        }
    }

    fn mode(&self) -> ReviewMode {
        self.session.read().as_ref().map_or(ReviewMode::Classic, |s| s.mode)
    }

    /// Writes the answer to the log, then updates the deck and the session.
    fn answer(&self, card_id: &str, rating: Rating, duration_ms: u64) -> bool {
        let mode = self.mode();
        let duration = duration_ms.min(u64::from(u32::MAX)) as u32;
        let recorded = self.db.0.borrow_mut().record_review(card_id, rating, now_ms(), duration, mode);
        let (mut deck, mut session, mut error) = (self.deck, self.session, self.error);
        match recorded {
            Ok(event) => {
                let id = event.id.clone();
                deck.write().record(event, &self.sched);
                if let Some(s) = session.write().as_mut() {
                    s.answer(id, card_id.to_string(), rating, duration);
                }
                error.set(None);
                true
            }
            Err(e) => {
                crate::diag::report!("saving a review failed: {e}");
                error.set(Some(Msg::RevSaveFailed));
                false
            }
        }
    }

    fn can_undo(&self) -> bool {
        self.session.read().as_ref().is_some_and(|s| !s.answers().is_empty())
    }

    /// Takes back the last answer; its card goes to the front of the queue.
    fn undo(&self) -> bool {
        let (mut session, mut deck) = (self.session, self.deck);
        let mut session = session.write();
        let Some(session) = session.as_mut() else { return false };
        let undone = take_back(&mut self.db.0.borrow_mut(), session, &mut deck.write(), &self.sched);
        undone.unwrap_or_else(|e| {
            crate::diag::report!("undo failed: {e}");
            false
        })
    }

    /// Ends the session (once: effects may ask twice before unmounting).
    fn finish(&self) {
        let (mut session, mut summary, mut view) = (self.session, self.summary, self.view);
        let Some(done) = session.write().take() else { return };
        if done.answers().is_empty() {
            // Nothing answered: no summary to show, just go back.
            view.set(View::Home);
            return;
        }
        let s = done.summary(now_ms());
        // Kak Nur counts reviews toward today's goal and the weekly report.
        let cards = s.answers as u32;
        let correct = (s.answers - s.again) as u32;
        self.coach.record(&self.db, ActivityKind::Reviews { cards, correct }, (s.duration_ms / 1000) as u32);
        summary.set(Some(s));
        view.set(View::Done);
    }

    fn home(&self) {
        let (mut session, mut view) = (self.session, self.view);
        session.set(None);
        view.set(View::Home);
    }

    fn seed(&self) {
        let mut deck = self.deck;
        if let Err(e) = self.db.0.borrow_mut().upsert_cards(&seed_bridge_deck()) {
            crate::diag::report!("seeding the starter deck failed: {e}");
            let mut error = self.error;
            error.set(Some(Msg::RevSaveFailed));
            return;
        }
        deck.set(load_deck(&self.db, &self.sched));
    }

    /// The learner checked a card's content: it loses the unverified flag
    /// (its memory schedule is untouched).
    fn verify_card(&self, card: &Card) {
        let checked = Card { verified: true, ..card.clone() };
        if let Err(e) = self.db.0.borrow_mut().upsert_card(&checked) {
            crate::diag::report!("saving the content check failed: {e}");
            let mut error = self.error;
            error.set(Some(Msg::RevSaveFailed));
            return;
        }
        let mut deck = self.deck;
        deck.set(load_deck(&self.db, &self.sched));
    }

    fn card(&self, id: &str) -> Option<Card> {
        self.deck.read().card(id).cloned()
    }
}

/// Undo of the session's last answer, in this order so the three never
/// disagree: the event gets its tombstone in the log (the review log is
/// append-only; a tombstone is its supported correction, and it syncs),
/// then the session hands the card back and the deck forgets the event, so
/// the card's schedule is exactly what it was before the answer. Nothing
/// changes if the log write fails. Returns whether an answer was undone.
fn take_back(
    store: &mut Store,
    session: &mut Session,
    deck: &mut Deck,
    sched: &Scheduler,
) -> Result<bool, crate::store::StoreError> {
    let Some(last) = session.answers().last().map(|a| a.event_id.clone()) else { return Ok(false) };
    store.undo_review(&last)?;
    session.undo();
    deck.forget(&last, sched);
    Ok(true)
}

// ---------------------------------------------------------------- text helpers

fn localized<'a>(ui: &Ui, l: &'a Localized) -> &'a str {
    match ui.settings.read().ui_language {
        UiLanguage::ArEg => &l.ar_eg,
        UiLanguage::Ar => &l.ar,
        UiLanguage::En => &l.en,
    }
}

/// Text whose Latin snippets are marked with FSI…PDI (U+2068…U+2069), with
/// each snippet rendered as a real `<bdi>`: not every WebView honors the
/// isolate characters themselves.
#[component]
pub(crate) fn Isolated(text: String) -> Element {
    let mut parts: Vec<(bool, String)> = Vec::new();
    for (i, chunk) in text.split('\u{2068}').enumerate() {
        match chunk.split_once('\u{2069}') {
            Some((latin, rest)) if i > 0 => {
                parts.push((true, latin.to_string()));
                parts.push((false, rest.to_string()));
            }
            _ => parts.push((false, chunk.to_string())),
        }
    }
    rsx! {
        for (i, (latin, part)) in parts.into_iter().enumerate() {
            if latin {
                bdi { key: "{i}", class: "latin", dir: "ltr", "{part}" }
            } else {
                span { key: "{i}", "{part}" }
            }
        }
    }
}

fn mm_ss(ui: &Ui, ms: u64) -> String {
    let secs = ms / 1000;
    ui.digits(&format!("{}:{:02}", secs / 60, secs % 60))
}

/// A span in words. Arabic counts: 1 and 2 have their own forms, 3–10 take
/// the plural, 11+ the singular. `after_prep`: MSA duals after «بعد».
fn span_text(ui: &Ui, span: Span, after_prep: bool) -> String {
    let lang = ui.lang();
    let (n, unit) = match span {
        Span::Minutes(n) => (n, 0),
        Span::Hours(n) => (n, 1),
        Span::Days(n) => (n, 2),
        Span::Months(n) => (n, 3),
        Span::Years(n) => (n, 4),
    };
    let d = ui.num(n);
    let arabic = |forms: [(&str, &str, &str, &str); 5]| {
        let (one, two, few, many) = forms[unit];
        match n {
            1 => one.to_string(),
            2 => two.to_string(),
            3..=10 => format!("{d} {few}"),
            _ => format!("{d} {many}"),
        }
    };
    match lang {
        UiLanguage::En => {
            let (one, many) = [
                ("minute", "minutes"),
                ("hour", "hours"),
                ("day", "days"),
                ("month", "months"),
                ("year", "years"),
            ][unit];
            format!("{d} {}", if n == 1 { one } else { many })
        }
        UiLanguage::ArEg => arabic([
            ("دقيقة", "دقيقتين", "دقايق", "دقيقة"),
            ("ساعة", "ساعتين", "ساعات", "ساعة"),
            ("يوم", "يومين", "أيام", "يوم"),
            ("شهر", "شهرين", "شهور", "شهر"),
            ("سنة", "سنتين", "سنين", "سنة"),
        ]),
        UiLanguage::Ar if after_prep => arabic([
            ("دقيقة", "دقيقتين", "دقائق", "دقيقة"),
            ("ساعة", "ساعتين", "ساعات", "ساعة"),
            ("يوم", "يومين", "أيام", "يومًا"),
            ("شهر", "شهرين", "أشهر", "شهرًا"),
            ("سنة", "سنتين", "سنوات", "سنة"),
        ]),
        UiLanguage::Ar => arabic([
            ("دقيقة", "دقيقتان", "دقائق", "دقيقة"),
            ("ساعة", "ساعتان", "ساعات", "ساعة"),
            ("يوم", "يومان", "أيام", "يومًا"),
            ("شهر", "شهران", "أشهر", "شهرًا"),
            ("سنة", "سنتان", "سنوات", "سنة"),
        ]),
    }
}

/// Compact interval for an answer button: "10m · 1d · 3d" in English.
fn span_short(ui: &Ui, span: Span) -> String {
    if ui.settings.read().ui_language != UiLanguage::En {
        return span_text(ui, span, false);
    }
    match span {
        Span::Minutes(n) => format!("{n}m"),
        Span::Hours(n) => format!("{n}h"),
        Span::Days(n) => format!("{n}d"),
        Span::Months(n) => format!("{n}mo"),
        Span::Years(n) => format!("{n}y"),
    }
}

/// "in 10 minutes", "tomorrow", "in 3 days".
fn when_text(ui: &Ui, sched: &Scheduler, now: u64, due: u64) -> String {
    match Span::until(sched, now, due) {
        Span::Days(1) => ui.t(Msg::RevTomorrow).to_string(),
        span => ui.t(Msg::RevIn).replace("{span}", &span_text(ui, span, true)),
    }
}

fn weekday(ui: &Ui, day: i64) -> &'static str {
    // Day 0 of the learner's calendar (1 Jan 1970) was a Thursday.
    let i = (day + 4).rem_euclid(7) as usize;
    ui.t([
        Msg::SunShort,
        Msg::MonShort,
        Msg::TueShort,
        Msg::WedShort,
        Msg::ThuShort,
        Msg::FriShort,
        Msg::SatShort,
    ][i])
}

fn rating_msg(r: Rating) -> Msg {
    match r {
        Rating::Again => Msg::RateAgain,
        Rating::Hard => Msg::RateHard,
        Rating::Good => Msg::RateGood,
        Rating::Easy => Msg::RateEasy,
    }
}

/// Speaks a word with the device's own voices: the free fallback until
/// cached Azure audio exists (SPEC §20). Android uses the system
/// TextToSpeech engine (its WebView has no `speechSynthesis`); elsewhere the
/// WebView speaks. Silent where unsupported.
fn speak(text: &str, language: StudyLanguage) {
    match platform::speak(text, language.speech_tag(), false) {
        Err(SpeakError::UseWebView) => {}
        Ok(()) => return,
        Err(e) => {
            crate::diag::report!("no review audio: {e:?}");
            return;
        }
    }
    let payload = serde_json::json!({ "text": text, "lang": language.speech_tag() });
    document::eval(&format!(
        r#"const p = {payload};
        if ("speechSynthesis" in window) {{
          const synth = window.speechSynthesis;
          synth.cancel();
          const u = new SpeechSynthesisUtterance(p.text);
          u.lang = p.lang;
          u.rate = 0.9;
          const voices = synth.getVoices();
          const norm = (l) => l.replace("_", "-").toLowerCase();
          const v = voices.find((v) => norm(v.lang) === p.lang.toLowerCase())
            || voices.find((v) => norm(v.lang).startsWith(p.lang.slice(0, 2)));
          if (v) u.voice = v;
          synth.speak(u);
        }}"#
    ));
}

// ---------------------------------------------------------------- screen

#[component]
pub fn ReviewScreen() -> Element {
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let sched = use_hook(|| crate::schedule::scheduler(&coach, &db.0.borrow()));
    let deck = use_signal({
        let (db, sched) = (db.clone(), sched.clone());
        move || load_deck(&db, &sched)
    });
    let engine = use_context_provider(|| Engine {
        db: db.clone(),
        coach,
        sched: sched.clone(),
        deck,
        session: Signal::new(None),
        view: Signal::new(View::Home),
        summary: Signal::new(None),
        error: Signal::new(None),
    });
    let ui = use_context::<Ui>();
    let view = *engine.view.read();
    let error = *engine.error.read();
    // A running session holds the phone's screen; Finish is its way out.
    crate::focus::use_focus_mode(matches!(view, View::Classic | View::Wall | View::Burst));

    rsx! {
        style { {REVIEW_CSS.text()} }
        div { class: "review",
            if let Some(msg) = error {
                p { class: "review-error", role: "alert", "{ui.t(msg)}" }
            }
            match view {
                View::Home => rsx! { ReviewHome {} },
                View::Classic => rsx! { Classic {} },
                View::Wall => rsx! { CardWall {} },
                View::Burst => rsx! { Burst {} },
                View::Done => rsx! { Done {} },
            }
        }
    }
}

// ---------------------------------------------------------------- home

/// The empty deck's invite: the starter words as chips, each opening its
/// Arabic Bridge card (word, meaning, the Arabic it comes from, and the
/// false-friend warning), so the words teach before the first round.
#[component]
fn SeedInvite() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let mut open = use_signal(|| None::<&'static str>);
    let count = ui.num(seed_words().count() as u64);
    let card = open()
        .and_then(|w| seed_bridge_deck().into_iter().find(|c| c.kind == CardKind::Bridge && c.word == w));
    rsx! {
        crate::ui::DivCard { class: "review-seed",
            h2 { "{ui.t(Msg::RevSeedTitle)}" }
            p { {ui.t(Msg::RevSeedBody).replace("{n}", &count)} }
            ul { class: "seed-words", lang: "id",
                for w in seed_words() {
                    li { key: "{w}",
                        button {
                            class: "seed-word",
                            aria_pressed: if open() == Some(w) { "true" } else { "false" },
                            aria_controls: "seed-card",
                            onclick: move |_| open.set(if open() == Some(w) { None } else { Some(w) }),
                            bdi { "{w}" }
                        }
                    }
                }
            }
            if let Some(card) = card {
                div { id: "seed-card", class: "seed-card", aria_live: "polite",
                    Back { key: "{card.id}", card }
                }
            }
            button { class: "primary", onclick: move |_| engine.seed(), "{ui.t(Msg::RevSeedButton)}" }
        }
    }
}

#[component]
fn ReviewHome() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let now = now_ms();
    let empty = engine.deck.read().is_empty();

    if empty {
        return rsx! { SeedInvite {} };
    }

    let sched = engine.sched.clone();
    let deck = engine.deck.read();
    let stats = deck.stats(&sched, now);
    let forecast = deck.forecast(&sched, now, 7);
    let today = sched.day.day(now);
    let peak = forecast.iter().copied().max().unwrap_or(0).max(1);
    let next_due = deck.next_due_ms(now).map(|d| when_text(&ui, &sched, now, d));
    drop(deck);
    let width = *ui.width.read();
    let modes = [
        (ReviewMode::Classic, Msg::RevClassic, Msg::RevClassicHint),
        // Say how this device moves cards: pen on the tablet, mouse on the Mac.
        (
            ReviewMode::CardWall,
            Msg::RevWall,
            if *ui.tablet.read() {
                Msg::RevWallHint
            } else if cfg!(target_os = "android") {
                Msg::RevWallHintTouch
            } else {
                Msg::RevWallHintMouse
            },
        ),
        (ReviewMode::Burst, Msg::RevBurst, Msg::RevBurstHint),
    ];
    let nothing_due = stats.due + stats.new_available == 0;

    rsx! {
        crate::ui::DivCard {
            h2 { "{ui.t(Msg::NavReview)}" }
            // Due and new start a classic round; the rest open Stats.
            div { class: "rev-stats",
                Stat {
                    label: Msg::RevDue,
                    value: ui.num(stats.due as u64),
                    accent: true,
                    start: stats.due > 0,
                }
                Stat {
                    label: Msg::RevNew,
                    value: ui.num(stats.new_available as u64),
                    accent: false,
                    start: stats.new_available > 0,
                }
                Stat { label: Msg::RevReviewedToday, value: ui.num(stats.reviews_today as u64), accent: false }
                Stat {
                    label: Msg::RevRetention,
                    value: stats.retention.map(|r| ui.percent(r)).unwrap_or_else(|| "–".into()),
                    accent: false,
                }
            }
            if nothing_due {
                crate::misc_art::Picture { art: crate::misc_art::Art::AllReviewed, class: "empty-art" }
                p { class: "rev-calm", "{ui.t(Msg::RevAllDone)}" }
                if let Some(when) = &next_due {
                    p { class: "muted", {ui.t(Msg::RevNextReview).replace("{when}", when)} }
                }
            }
        }
        crate::ui::DivCard {
            h3 { class: "rev-h3", "{ui.t(Msg::RevForecast)}" }
            // The week's bars open Stats, where the forecast runs further.
            button {
                class: "tap tap-plain forecast-tap",
                aria_label: "{ui.t(Msg::RevForecast)}",
                onclick: move |_| crate::stats::open(ui, Screen::Stats),
            ol { class: "forecast",
                for (i, n) in forecast.iter().copied().enumerate() {
                    li {
                        key: "{i}",
                        class: if i == 0 { "today" } else { "" },
                        aria_label: format!(
                            "{}: {}",
                            if i == 0 { ui.t(Msg::RevToday) } else { weekday(&ui, today + i as i64) },
                            ui.num(u64::from(n)),
                        ),
                        span { class: "fc-n", aria_hidden: "true", if n > 0 { "{ui.num(u64::from(n))}" } }
                        span { class: "fc-bar", aria_hidden: "true",
                            span { class: "fc-fill", style: "block-size: {n * 100 / peak}%" }
                        }
                        span { class: "fc-day", aria_hidden: "true",
                            if i == 0 { "{ui.t(Msg::RevToday)}" } else { "{weekday(&ui, today + i as i64)}" }
                        }
                    }
                }
            }
            }
            crate::stats::StatsButton {}
        }
        div { class: "rev-modes",
            for (mode, title, hint) in modes {
                {
                    let count = engine.queue_len(mode, now);
                    let engine = engine.clone();
                    let burst_secs = ui.num(BURST_MS / 1000);
                    rsx! {
                        // The whole card starts the mode when there is something to do;
                        // the Start button's click bubbles up to it.
                        div {
                            key: "{mode:?}",
                            class: match (mode == ReviewMode::Burst, count > 0) {
                                (true, true) => "card card-action rev-mode mode-burst",
                                (true, false) => "card rev-mode mode-burst",
                                (false, true) => "card card-action rev-mode",
                                (false, false) => "card rev-mode",
                            },
                            onclick: move |_| if count > 0 { engine.start(mode) },
                            h3 { class: "rev-h3", Isolated { text: ui.t(title).to_string() } }
                            p { class: "muted", Isolated { text: ui.t(hint).replace("{n}", &burst_secs) } }
                            if mode == ReviewMode::CardWall && width == WidthClass::Compact {
                                p { class: "muted small", "{ui.t(Msg::RevWallPhone)}" }
                            }
                            if count == 0 {
                                p { class: "muted small", "{ui.t(Msg::RevNothingHere)}" }
                            }
                            button {
                                class: "primary",
                                disabled: count == 0,
                                "{ui.t(Msg::RevStart)}"
                                if count > 0 {
                                    span { class: "count-badge", "{ui.num(count as u64)}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A number that opens what it counts: a classic round when `start` is
/// given, else Stats.
#[component]
fn Stat(label: Msg, value: String, accent: bool, #[props(default)] start: bool) -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    rsx! {
        button {
            class: if accent { "rev-stat accent tap" } else { "rev-stat tap" },
            onclick: move |_| {
                if start {
                    engine.start(ReviewMode::Classic);
                } else {
                    crate::stats::open(ui, Screen::Stats);
                }
            },
            span { class: "rev-stat-value", "{value}" }
            span { class: "rev-stat-label", "{ui.t(label)}" }
        }
    }
}

// ---------------------------------------------------------------- card faces

/// The word, isolated from the surrounding UI direction.
#[component]
fn Word(card: Card, big: bool) -> Element {
    let ui = use_context::<Ui>();
    let text = card.word.clone();
    let language = card.language;
    rsx! {
        div { class: "rev-word-row",
            bdi { class: if big { "rev-word big" } else { "rev-word" }, lang: language.tag(), dir: "ltr", "{card.word}" }
            button {
                class: "icon-btn",
                aria_label: ui.t(Msg::RevPlay),
                title: ui.t(Msg::RevPlay),
                onclick: move |e| {
                    e.stop_propagation();
                    speak(&text, language);
                },
                SpeakerIcon {}
            }
        }
    }
}

#[component]
fn SpeakerIcon() -> Element {
    rsx! {
        svg {
            view_box: "0 0 24 24",
            width: "22",
            height: "22",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            path { d: "M11 5 6 9H3v6h3l5 4zM15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13" }
        }
    }
}

#[component]
fn Badges(card: Card) -> Element {
    let ui = use_context::<Ui>();
    let unverified = card.language == StudyLanguage::Jv && !card.verified;
    rsx! {
        if card.arabic.is_some() || unverified {
            div { class: "rev-badges",
                if card.arabic.is_some() {
                    span { class: "badge gold", "{ui.t(Msg::RevFromArabic)}" }
                }
                if unverified {
                    span { class: "badge warn", "{ui.t(Msg::RevUnverified)}" }
                }
            }
        }
    }
}

/// Javanese words spelled like an Indonesian word with another meaning
/// (awan "midday" ≠ Indonesian awan "cloud"), from the lessons'
/// `new_words[].false_friend` (#293): lowercase word → the Indonesian twin.
/// Read once, the first time a card asks.
static JV_TWINS: std::sync::LazyLock<std::collections::HashMap<String, String>> =
    std::sync::LazyLock::new(|| {
        crate::embedded::all(crate::embedded::LESSONS)
            .filter(|(id, text)| id.starts_with('j') && text.contains("false_friend:"))
            .filter_map(|(id, text)| match kabar_core::lesson::parse_lesson(&text) {
                Ok(l) => Some(l),
                Err(e) => {
                    crate::diag::report!("reading {id} for false friends failed: {e}");
                    None
                }
            })
            .flat_map(|l| l.new_words)
            .filter_map(|w| Some((w.word.to_lowercase(), w.false_friend?)))
            .collect()
    });

/// The Indonesian word a Javanese card's word is a false friend of (#379).
fn indonesian_twin(card: &Card) -> Option<&'static str> {
    (card.language == StudyLanguage::Jv)
        .then(|| JV_TWINS.get(&card.word.trim().to_lowercase()).map(String::as_str))
        .flatten()
}

/// «≠ not like Indonesian awan»: the meaning memory will offer is the wrong one.
#[component]
fn TwinWarning(twin: &'static str) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        span { class: "badge twin",
            span { "≠ {ui.t(Msg::RevNotIndonesian)} " }
            bdi { lang: "id", dir: "ltr", class: "latin", "{twin}" }
        }
    }
}

/// What the learner sees before answering.
#[component]
fn Front(card: Card) -> Element {
    let ui = use_context::<Ui>();
    let prompt = match card.kind {
        CardKind::Recognition => Msg::RevPromptMeaning,
        CardKind::Production => Msg::RevPromptProduce,
        CardKind::AudioMeaning => Msg::RevPromptListen,
        CardKind::Cloze => Msg::RevPromptCloze,
        CardKind::Bridge => Msg::RevPromptBridge,
    };
    let gap = card.example.as_ref().and_then(|ex| {
        cloze(&ex.text, &card.word)
            .map(|(a, _, b)| (a.to_string(), b.to_string(), localized(&ui, &ex.meaning).to_string()))
    });
    let (text, language) = (card.word.clone(), card.language);
    rsx! {
        p { class: "rev-prompt", "{ui.t(prompt)}" }
        match card.kind {
            CardKind::Recognition => rsx! { Word { card: card.clone(), big: true } },
            CardKind::Production => rsx! {
                p { class: "rev-meaning big", "{localized(&ui, &card.meaning)}" }
            },
            CardKind::AudioMeaning => rsx! {
                button {
                    class: "rev-listen",
                    aria_label: ui.t(Msg::RevPlay),
                    onclick: move |e| {
                        e.stop_propagation();
                        speak(&text, language);
                    },
                    SpeakerIcon {}
                }
            },
            CardKind::Cloze => match gap {
                Some((before, after, meaning)) => rsx! {
                    p { class: "rev-cloze", lang: card.language.tag(), dir: "ltr",
                        "{before}"
                        span { class: "gap", aria_label: "…", "＿＿＿" }
                        "{after}"
                    }
                    p { class: "muted", "{meaning}" }
                },
                None => rsx! { Word { card: card.clone(), big: true } },
            },
            CardKind::Bridge => rsx! {
                p { class: "rev-arabic", lang: "ar",
                    {card.arabic.as_ref().map(|a| a.word.clone()).unwrap_or_default()}
                }
            },
        }
    }
}

/// The answer side: word, meaning, note, example and the Arabic Bridge.
#[component]
fn Back(card: Card) -> Element {
    let ui = use_context::<Ui>();
    // Arrow from the Arabic origin to the word, mirrored with the UI.
    let arrow = if ui.settings.read().ui_language == UiLanguage::En { "←" } else { "→" };
    rsx! {
        div { class: "rev-back",
            // A recognition card already shows the word on its front.
            if card.kind != CardKind::Recognition {
                Word { card: card.clone(), big: false }
            }
            // Only on the answer side: on the front it would hint the answer.
            Pron { text: card.word.clone(), tag: card.language.tag() }
            p { class: "rev-meaning", "{localized(&ui, &card.meaning)}" }
            if let Some(twin) = indonesian_twin(&card) {
                TwinWarning { twin }
            }
            if let Some(note) = &card.note {
                p { class: "muted rev-note", Isolated { text: localized(&ui, note).to_string() } }
            }
            if let Some(ex) = &card.example {
                div { class: "rev-example",
                    Annotated { text: ex.text.clone(), tag: card.language.tag(), meaning: localized(&ui, &ex.meaning).to_string(), class: "ex-text" }
                }
            }
            if let Some(a) = &card.arabic {
                div { class: if a.shift == MeaningShift::FalseFriend { "rev-bridge alarm" } else { "rev-bridge" },
                    div { class: "bridge-line",
                        bdi { class: "latin bridge-word", lang: card.language.tag(), dir: "ltr", "{card.word}" }
                        span { aria_hidden: "true", "{arrow}" }
                        span { class: "ar", lang: "ar", "{a.word}" }
                        if let Some(root) = &a.root {
                            span { class: "root", lang: "ar", "({root})" }
                        }
                    }
                    match a.shift {
                        MeaningShift::FalseFriend => rsx! { p { class: "shift", "{ui.t(Msg::RevFalseFriend)}" } },
                        MeaningShift::Shifted => rsx! { p { class: "shift", "{ui.t(Msg::RevShifted)}" } },
                        MeaningShift::Same => rsx! {},
                    }
                    if let Some(note) = &a.note {
                        p { class: "bridge-note", Isolated { text: localized(&ui, note).to_string() } }
                    }
                }
            }
            Badges { card: card.clone() }
        }
    }
}

// ---------------------------------------------------------------- classic

#[component]
fn Classic() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let mut hand = use_signal({
        let engine = engine.clone();
        move || engine.pull().map(|id| (id, now_ms()))
    });
    let mut revealed = use_signal(|| false);
    let mut typed = use_signal(String::new);
    let mut checked = use_signal(|| None::<AnswerMatch>);

    use_effect({
        let engine = engine.clone();
        move || {
            if hand.read().is_none() {
                engine.finish();
            }
        }
    });
    // Audio cards speak as soon as they are shown.
    use_effect({
        let engine = engine.clone();
        move || {
            if let Some((id, _)) = hand.read().as_ref()
                && let Some(card) = engine.deck.peek().card(id)
                && card.kind == CardKind::AudioMeaning
            {
                speak(&card.word, card.language);
            }
        }
    });

    // Hand the next card over with a clean slate.
    let mut advance = move |engine: &Engine| {
        hand.set(engine.pull().map(|id| (id, now_ms())));
        revealed.set(false);
        typed.set(String::new());
        checked.set(None);
    };
    let rate = use_callback({
        let engine = engine.clone();
        move |r: Rating| {
            // Only a revealed card is rated: a quick second tap must not
            // rate the card that just replaced it.
            if !revealed() {
                return;
            }
            let Some((card_id, shown_ms)) = hand() else { return };
            if engine.answer(&card_id, r, now_ms().saturating_sub(shown_ms)) {
                advance(&engine);
            }
        }
    });
    let undo = use_callback({
        let engine = engine.clone();
        move |()| {
            if let Some((card_id, _)) = hand() {
                engine.put_back(card_id);
            }
            engine.undo();
            advance(&engine);
        }
    });
    // Keys come from a document-level listener, so they work wherever focus
    // is (a button that just became disabled drops it to the body).
    let on_key = use_callback(move |key: String| match key.as_str() {
        " " if !revealed() => revealed.set(true),
        "1" | "2" | "3" | "4" if revealed() => {
            let i = key.parse::<usize>().unwrap_or(1) - 1;
            rate.call(Rating::ALL[i]);
        }
        "undo" => undo.call(()),
        _ => {}
    });
    use_future(move || async move {
        let mut eval = document::eval(KEYS_JS);
        while let Ok(key) = eval.recv::<String>().await {
            on_key.call(key);
        }
    });

    let Some((card_id, _)) = hand() else { return rsx! {} };
    let Some(card) = engine.card(&card_id) else { return rsx! {} };
    let now = now_ms();
    let previews = engine.sched.preview(&engine.deck.read().state(&card_id), now);
    let left = engine.session.read().as_ref().map_or(0, Session::remaining);
    let is_production = card.kind == CardKind::Production;
    let suggested = checked().map(AnswerMatch::suggested);
    // Expanded+ puts the session's progress beside the card (docs/DESIGN.md).
    let wide = *ui.width.read() >= WidthClass::Expanded;

    let check = {
        let word = card.word.clone();
        move || {
            checked.set(Some(compare_answer(&typed.read(), &word)));
            revealed.set(true);
        }
    };

    rsx! {
        div { id: "review-classic", class: if wide { "rev-session with-pane" } else { "rev-session" },
            SessionBar { left }
            crate::ui::Card { element: crate::ui::CardElement::Article,
                key: "{card_id}",
                class: "rev-card",
                aria_live: "polite",
                Front { card: card.clone() }
                if is_production {
                    form {
                        class: "rev-answer",
                        onsubmit: {
                            let mut check = check.clone();
                            move |e: FormEvent| {
                                e.prevent_default();
                                if !revealed() {
                                    check();
                                }
                            }
                        },
                        input {
                            r#type: "text",
                            lang: card.language.tag(),
                            dir: "ltr",
                            autocomplete: "off",
                            autocapitalize: "off",
                            spellcheck: "false",
                            readonly: revealed(),
                            placeholder: ui.t(Msg::RevAnswerPlaceholder),
                            aria_label: ui.t(Msg::RevPromptProduce),
                            value: "{typed}",
                            oninput: move |e: FormEvent| typed.set(e.value()),
                            onmounted: move |e: MountedEvent| async move {
                                let _ = e.set_focus(true).await;
                            },
                        }
                    }
                    if let Some(m) = checked() {
                        p { class: if m == AnswerMatch::Exact { "rev-verdict ok" } else { "rev-verdict" },
                            {
                                match m {
                                    AnswerMatch::Exact => ui.t(Msg::RevExact),
                                    AnswerMatch::Close => ui.t(Msg::RevClose),
                                    AnswerMatch::Wrong => ui.t(Msg::RevWrong),
                                }
                            }
                        }
                        if m != AnswerMatch::Exact && !typed.read().trim().is_empty() {
                            p { class: "muted",
                                "{ui.t(Msg::RevYouWrote)}: "
                                bdi { lang: card.language.tag(), dir: "ltr", class: "struck", "{typed}" }
                            }
                        }
                    }
                }
                if revealed() {
                    Back { card: card.clone() }
                }
            }
            // Whether the card's content is right is its own question, kept
            // apart from the memory rating below.
            if revealed() && card.language == StudyLanguage::Jv {
                ContentRow { card: card.clone() }
            }
            // The footer keeps its place: one primary action before the
            // answer shows, the four ratings (with their real next
            // intervals) after.
            div { class: "rev-footer",
            if revealed() {
                p { class: "rev-ask", id: "rev-ask",
                    strong { "{ui.t(Msg::RevHowWell)}" }
                    span { class: "muted small", " {ui.t(Msg::RevRateHint)}" }
                }
                div { class: "rate-row rating-row", role: "group", aria_labelledby: "rev-ask",
                    for (i, p) in previews.iter().enumerate() {
                        {
                            let r = p.rating;
                            let span = span_short(&ui, Span::from_days(p.interval_days));
                            let hint = if suggested == Some(r) { " suggested" } else { "" };
                            let class = format!("rating {}{hint}", rate_class(r));
                            rsx! {
                                button {
                                    key: "{i}",
                                    class,
                                    "aria-keyshortcuts": "{i + 1}",
                                    onclick: move |_| rate.call(r),
                                    span { class: "rate-label", "{ui.t(rating_msg(r))}" }
                                    small { class: "rate-span", "{span}" }
                                }
                            }
                        }
                    }
                }
            } else if is_production {
                div { class: "rate-row",
                    button {
                        class: "primary reveal-btn",
                        onclick: {
                            let mut check = check.clone();
                            move |_| check()
                        },
                        "{ui.t(Msg::RevCheck)}"
                    }
                }
            } else {
                div { class: "rate-row",
                    button {
                        class: "primary reveal-btn",
                        "aria-keyshortcuts": "Space",
                        onclick: move |_| revealed.set(true),
                        "{ui.t(Msg::RevShowAnswer)}"
                    }
                }
            }
            }
            div { class: "rev-foot",
                span { class: "muted small kbd-hint", {ui.digits(ui.t(Msg::RevKeysHint))} }
                SessionActions { on_undo: move |_| undo.call(()) }
            }
            if wide {
                SessionPane { left, on_undo: move |_| undo.call(()) }
            }
        }
    }
}

/// A Javanese card's content check, separate from how well it was
/// remembered: unchecked until the learner says the card is right.
#[component]
fn ContentRow(card: Card) -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    rsx! {
        div { class: "rev-content", role: "status",
            if card.verified {
                span { class: "muted small", "{ui.t(Msg::RevContentChecked)}" }
            } else {
                span { class: "muted small", "{ui.t(Msg::RevContentUnchecked)}" }
                button {
                    class: "ghost rev-content-ok",
                    onclick: move |_| engine.verify_card(&card),
                    "{ui.t(Msg::RevContentOk)}"
                }
            }
        }
    }
}

/// How many recent answers the side pane lists.
const RECENT: usize = 8;

/// The side pane on wide windows: answer counts per rating and the words just
/// reviewed, newest first, with undo on the latest. Fills the margin with the
/// session instead of leaving it empty.
#[component]
fn SessionPane(left: usize, on_undo: EventHandler<()>) -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let session = engine.session.read();
    let answers = session.as_ref().map_or(&[][..], Session::answers);
    let count = |r: Rating| answers.iter().filter(|a| a.rating == r).count() as u64;
    let recent: Vec<(String, Card, Rating)> = answers
        .iter()
        .rev()
        .take(RECENT)
        .filter_map(|a| engine.card(&a.card_id).map(|c| (a.event_id.clone(), c, a.rating)))
        .collect();
    let can_undo = engine.can_undo();
    rsx! {
        aside { class: "rev-pane", aria_label: ui.t(Msg::RevThisSession),
            h2 { class: "rev-pane-title", "{ui.t(Msg::RevThisSession)}" }
            div { class: "rev-pane-counts",
                div { class: "rev-pane-big",
                    span { class: "rev-stat-value", "{ui.num(answers.len() as u64)}" }
                    span { class: "rev-stat-label", "{ui.t(Msg::RevAnswered)}" }
                }
                div { class: "rev-pane-big",
                    span { class: "rev-stat-value", "{ui.num((left + 1) as u64)}" }
                    span { class: "rev-stat-label", "{ui.t(Msg::RevRemaining)}" }
                }
            }
            ul { class: "rev-pane-ratings",
                for r in Rating::ALL {
                    li { key: "{r:?}", class: rate_class(r),
                        span { class: "rev-pane-n", "{ui.num(count(r))}" }
                        span { "{ui.t(rating_msg(r))}" }
                    }
                }
            }
            h3 { class: "rev-h3", "{ui.t(Msg::RevJustReviewed)}" }
            if recent.is_empty() {
                p { class: "muted small", "{ui.t(Msg::RevNoneYet)}" }
            } else {
                ol { class: "rev-recent",
                    for (i, (key, card, r)) in recent.into_iter().enumerate() {
                        li { key: "{key}",
                            bdi { class: "latin rev-recent-word", lang: card.language.tag(), dir: "ltr", "{card.word}" }
                            span { class: "pill {rate_class(r)}", "{ui.t(rating_msg(r))}" }
                            if i == 0 && can_undo {
                                button {
                                    class: "ghost rev-recent-undo",
                                    title: ui.t(Msg::RevUndo),
                                    aria_label: ui.t(Msg::RevUndo),
                                    "aria-keyshortcuts": "Control+Z Meta+Z",
                                    onclick: move |_| on_undo.call(()),
                                    "↶"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The rating's colour class, shared by the answer buttons and the pane.
fn rate_class(r: Rating) -> &'static str {
    match r {
        Rating::Again => "again",
        Rating::Hard => "hard",
        Rating::Good => "good",
        Rating::Easy => "easy",
    }
}

#[component]
fn SessionBar(left: usize) -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let done = engine.session.read().as_ref().map_or(0, |s| s.answers().len());
    let total = done + left + 1;
    rsx! {
        div { class: "session-bar",
            div {
                class: "progress progress-line",
                role: "progressbar",
                aria_valuemin: "0",
                aria_valuemax: "{total}",
                aria_valuenow: "{done}",
                span { class: "progress-fill fill", style: "inline-size: {done * 100 / total.max(1)}%" }
            }
            // The card in hand out of the session, read left to right.
            span { class: "muted small ltr-count", dir: "ltr", "{ui.num((done + 1) as u64)} / {ui.num(total as u64)}" }
            span { class: "muted small", {ui.t(Msg::RevLeft).replace("{n}", &ui.num((left + 1) as u64))} }
        }
    }
}

#[component]
fn SessionActions(on_undo: EventHandler<()>) -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let can_undo = engine.can_undo();
    rsx! {
        div { class: "row",
            button { class: "rev-undo", disabled: !can_undo, onclick: move |_| on_undo.call(()), "{ui.t(Msg::RevUndo)}" }
            // Android Back ends the session here (its answers are already saved).
            button { "data-back": "true", onclick: move |_| engine.finish(), "{ui.t(Msg::RevFinish)}" }
        }
    }
}

// ---------------------------------------------------------------- card wall

#[derive(Clone, Debug, PartialEq)]
struct Slot {
    card_id: String,
    dealt_ms: u64,
    flipped: bool,
}

#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum FromWall {
    Flip { id: String },
    Drop { id: String, pile: Pile },
}

/// A small, stable tilt per card so the table looks dealt by hand.
fn tilt(card_id: &str) -> i32 {
    let h = card_id.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    (h % 7) as i32 - 3
}

#[component]
fn CardWall() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let seats = match *ui.width.peek() {
        WidthClass::Compact => 1,
        WidthClass::Medium => 4,
        _ => 6,
    };
    let mut slots = use_signal({
        let engine = engine.clone();
        move || {
            let now = now_ms();
            (0..seats)
                .map(|_| engine.pull().map(|card_id| Slot { card_id, dealt_ms: now, flipped: false }))
                .collect::<Vec<_>>()
        }
    });
    let mut last_sort = use_signal(now_ms);
    let mut freed = use_signal(Vec::<usize>::new);

    let sort = use_callback({
        let engine = engine.clone();
        move |(card_id, pile): (String, Pile)| {
            let Some(i) = slots.peek().iter().position(|s| s.as_ref().is_some_and(|s| s.card_id == card_id))
            else {
                return;
            };
            let slot = slots.peek()[i].clone().expect("slot is filled");
            let now = now_ms();
            // Sorting time: from when this card could get attention (dealt,
            // or the previous card left the table) to the drop.
            let since = slot.dealt_ms.max(*last_sort.peek());
            let sort_ms = now.saturating_sub(since).min(u64::from(u32::MAX)) as u32;
            let rating = wall_rating(pile, sort_ms, slot.flipped);
            if engine.answer(&card_id, rating, now.saturating_sub(since)) {
                last_sort.set(now);
                freed.write().push(i);
                slots.write()[i] =
                    engine.pull().map(|card_id| Slot { card_id, dealt_ms: now, flipped: false });
                if slots.peek().iter().all(Option::is_none) {
                    engine.finish();
                }
            }
        }
    });
    let flip = use_callback(move |card_id: String| {
        if let Some(s) = slots.write().iter_mut().flatten().find(|s| s.card_id == card_id) {
            s.flipped = !s.flipped;
        }
    });

    use_future(move || async move {
        let mut eval = document::eval(WALL_JS);
        while let Ok(msg) = eval.recv::<FromWall>().await {
            match msg {
                FromWall::Flip { id } => flip.call(id),
                FromWall::Drop { id, pile } => sort.call((id, pile)),
            }
        }
    });

    let undo = {
        let engine = engine.clone();
        move |_| {
            let Some(i) = freed.write().pop() else { return };
            if let Some(Some(current)) = slots.peek().get(i).cloned() {
                engine.put_back(current.card_id);
            }
            if engine.undo() {
                slots.write()[i] =
                    engine.pull().map(|card_id| Slot { card_id, dealt_ms: now_ms(), flipped: false });
            }
        }
    };

    let (tahu, belum) = engine.session.read().as_ref().map_or((0, 0), |s| {
        let belum = s.answers().iter().filter(|a| a.rating == Rating::Again).count();
        (s.answers().len() - belum, belum)
    });
    let left = engine.session.read().as_ref().map_or(0, Session::remaining);
    let seat_view: Vec<(String, Option<Slot>)> = slots
        .read()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            (
                s.as_ref().map_or_else(|| format!("empty-{i}"), |s| format!("{}@{}", s.card_id, s.dealt_ms)),
                s.clone(),
            )
        })
        .collect();

    rsx! {
        div { class: "wall",
            p { class: "muted small wall-help", "{ui.t(Msg::RevWallHelp)}" }
            div { id: "card-wall", class: "wall-table", "data-seats": "{seats}",
                // Seats are keyed by their card and deal time, so a new card,
                // or a "belum" card dealt again, gets a fresh element (no
                // leftover drag transform from the last throw).
                for (seat_key, slot) in seat_view {
                    div { key: "{seat_key}", class: "seat",
                        if let Some(slot) = slot {
                            if let Some(card) = engine.card(&slot.card_id) {
                                WallCard { card, flipped: slot.flipped, sort }
                            }
                        }
                    }
                }
            }
            div { class: "piles",
                div { class: "pile belum", "data-pile": "belum",
                    span { class: "pile-word latin", lang: "id", "belum" }
                    span { class: "pile-label", "{ui.t(Msg::RevBelum)}" }
                    span { class: "pile-count", "{ui.num(belum as u64)}" }
                }
                div { class: "pile tahu", "data-pile": "tahu",
                    span { class: "pile-word latin", lang: "id", "tahu" }
                    span { class: "pile-label", "{ui.t(Msg::RevTahu)}" }
                    span { class: "pile-count", "{ui.num(tahu as u64)}" }
                }
            }
            div { class: "rev-foot",
                span { class: "muted small", {ui.t(Msg::RevLeft).replace("{n}", &ui.num(left as u64))} }
                SessionActions { on_undo: undo }
            }
        }
    }
}

#[component]
fn WallCard(card: Card, flipped: bool, sort: Callback<(String, Pile)>) -> Element {
    let ui = use_context::<Ui>();
    let id = card.id.clone();
    let front = match card.kind {
        CardKind::Bridge => card.arabic.as_ref().map(|a| a.word.clone()).unwrap_or_default(),
        _ => card.word.clone(),
    };
    let front_lang = if card.kind == CardKind::Bridge { "ar" } else { card.language.tag() };
    let label = if flipped { localized(&ui, &card.meaning).to_string() } else { front.clone() };
    let false_friend = card.arabic.as_ref().is_some_and(|a| a.shift == MeaningShift::FalseFriend);
    rsx! {
        div {
            class: if flipped { "wall-card flipped" } else { "wall-card" },
            "data-id": "{id}",
            role: "group",
            aria_label: "{label}",
            style: "--tilt: {tilt(&id)}deg",
            tabindex: "0",
            div { class: "face front",
                if card.arabic.is_some() {
                    span { class: "dot-gold", aria_hidden: "true" }
                }
                span { class: if front_lang == "ar" { "wall-word ar" } else { "wall-word latin" }, lang: front_lang,
                    bdi { "{front}" }
                }
            }
            div { class: "face back",
                if card.kind == CardKind::Bridge {
                    bdi { class: "wall-word small latin", lang: card.language.tag(), dir: "ltr", "{card.word}" }
                    Say { text: card.word.clone(), tag: card.language.tag() }
                }
                Pron { text: card.word.clone(), tag: card.language.tag() }
                span { class: "wall-meaning", "{localized(&ui, &card.meaning)}" }
                if false_friend {
                    span { class: "badge alarm", "{ui.t(Msg::RevFalseFriend)}" }
                }
                if let Some(twin) = indonesian_twin(&card) {
                    TwinWarning { twin }
                }
                if card.language == StudyLanguage::Jv && !card.verified {
                    span { class: "badge warn", "{ui.t(Msg::RevUnverified)}" }
                }
            }
            // Keyboard and screen-reader path; shown on focus.
            div { class: "wall-keys",
                button {
                    onclick: {
                        let id = id.clone();
                        move |_| sort.call((id.clone(), Pile::Belum))
                    },
                    "{ui.t(Msg::RevBelum)}"
                }
                button {
                    onclick: {
                        let id = id.clone();
                        move |_| sort.call((id.clone(), Pile::Tahu))
                    },
                    "{ui.t(Msg::RevTahu)}"
                }
            }
        }
    }
}

// ---------------------------------------------------------------- burst

#[component]
fn Burst() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let started = engine.session.peek().as_ref().map_or_else(now_ms, |s| s.started_ms);
    let mut hand = use_signal({
        let engine = engine.clone();
        move || engine.pull().map(|id| (id, now_ms()))
    });
    let mut revealed = use_signal(|| false);
    let mut tick = use_signal(now_ms);

    use_future(move || async move {
        let mut eval = document::eval(
            r#"for (let i = 0; i < 400; i++) {
                 await new Promise((r) => setTimeout(r, 250));
                 dioxus.send(i);
               }"#,
        );
        while eval.recv::<u32>().await.is_ok() {
            tick.set(now_ms());
        }
    });
    use_effect({
        let engine = engine.clone();
        move || {
            if tick().saturating_sub(started) >= BURST_MS || hand.read().is_none() {
                engine.finish();
            }
        }
    });

    let Some((card_id, shown_ms)) = hand() else { return rsx! {} };
    let Some(card) = engine.card(&card_id) else { return rsx! {} };
    let left_ms = BURST_MS.saturating_sub(tick().saturating_sub(started));
    let secs = left_ms.div_ceil(1000);

    let answer = {
        let (engine, card_id) = (engine.clone(), card_id.clone());
        move |r: Rating| {
            if engine.answer(&card_id, r, now_ms().saturating_sub(shown_ms)) {
                hand.set(engine.pull().map(|id| (id, now_ms())));
                revealed.set(false);
            }
        }
    };
    let undo = {
        let (engine, card_id) = (engine.clone(), card_id.clone());
        move |_| {
            engine.put_back(card_id.clone());
            engine.undo();
            hand.set(engine.pull().map(|id| (id, now_ms())));
            revealed.set(false);
        }
    };
    let front = match card.kind {
        CardKind::Bridge => card.arabic.as_ref().map(|a| a.word.clone()).unwrap_or_default(),
        _ => card.word.clone(),
    };
    let front_ar = card.kind == CardKind::Bridge;

    rsx! {
        div { class: "burst",
            div { class: "burst-timer", role: "timer",
                span { class: "burst-secs", "{ui.num(secs)}" }
                span { class: "muted small", "{ui.t(Msg::RevSecondsLeft)}" }
                div { class: "progress",
                    span { class: "progress-fill", style: "inline-size: {left_ms * 100 / BURST_MS}%" }
                }
            }
            crate::ui::CardButton {
                key: "{card_id}",
                class: if revealed() { "burst-card revealed" } else { "burst-card" },
                aria_expanded: if revealed() { "true" } else { "false" },
                onclick: move |_| revealed.set(true),
                span { class: if front_ar { "burst-word ar" } else { "burst-word latin" },
                    lang: if front_ar { "ar" } else { card.language.tag() },
                    bdi { "{front}" }
                }
                if revealed() {
                    if front_ar {
                        bdi { class: "burst-sub latin", lang: card.language.tag(), dir: "ltr", "{card.word}" }
                    }
                    span { class: "burst-meaning", "{localized(&ui, &card.meaning)}" }
                    if card.language == StudyLanguage::Jv && !card.verified {
                        span { class: "badge warn", "{ui.t(Msg::RevUnverified)}" }
                    }
                } else {
                    span { class: "muted small", "{ui.t(Msg::RevTapToReveal)}" }
                }
            }
            div { class: "burst-answers",
                button {
                    class: "rate again",
                    onclick: {
                        let mut answer = answer.clone();
                        move |_| answer(Rating::Again)
                    },
                    "{ui.t(Msg::RevNotYet)}"
                }
                button {
                    class: "rate good",
                    onclick: {
                        let mut answer = answer.clone();
                        move |_| answer(Rating::Good)
                    },
                    "{ui.t(Msg::RevKnow)}"
                }
            }
            div { class: "rev-foot",
                span {}
                SessionActions { on_undo: undo }
            }
        }
    }
}

// ---------------------------------------------------------------- summary

#[component]
fn Done() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let summary = engine.summary.read().clone().unwrap_or_default();
    let now = now_ms();
    let deck = engine.deck.read();
    let word = |id: &String| deck.card(id).map(|c| (c.word.clone(), c.language.tag()));
    let strongest = summary.strongest.as_ref().and_then(word);
    let revisit: Vec<(String, &'static str)> = summary.revisit.iter().filter_map(word).collect();
    let next = deck.next_due_ms(now).map(|d| when_text(&ui, &engine.sched, now, d));
    drop(deck);

    if summary.answers == 0 {
        return rsx! {
            crate::ui::DivCard { class: "rev-done",
                p { "{ui.t(Msg::RevNothingHere)}" }
                if let Some(when) = &next {
                    p { class: "muted", {ui.t(Msg::RevNextReview).replace("{when}", when)} }
                }
                button { class: "primary", onclick: move |_| engine.home(), "{ui.t(Msg::RevBackHome)}" }
            }
        };
    }

    rsx! {
        crate::ui::DivCard { class: "rev-done",
            h2 { "{ui.t(Msg::RevDoneTitle)}" }
            div { class: "rev-stats",
                Stat { label: Msg::RevCards, value: ui.num(summary.answers as u64), accent: false }
                Stat {
                    label: Msg::RevRemembered,
                    value: ui.num((summary.answers - summary.again) as u64),
                    accent: true,
                }
                Stat {
                    label: Msg::RevRetention,
                    value: summary.retention.map(|r| ui.percent(r)).unwrap_or_else(|| "–".into()),
                    accent: false,
                }
                Stat { label: Msg::RevTime, value: mm_ss(&ui, summary.duration_ms), accent: false }
            }
            if summary.again == 0 {
                p { class: "rev-cheer", "{ui.t(Msg::RevAllRemembered)}" }
            }
            if let Some((w, lang)) = strongest {
                p {
                    "{ui.t(Msg::RevStrongest)}: "
                    bdi { class: "chip", lang, dir: "ltr", "{w}" }
                }
            }
            if !revisit.is_empty() {
                p { "{ui.t(Msg::RevRevisit)}:" }
                div { class: "row",
                    for (w, lang) in revisit {
                        bdi { key: "{w}", class: "chip soft", lang, dir: "ltr", "{w}" }
                    }
                }
            }
            if let Some(when) = &next {
                p { class: "muted", {ui.t(Msg::RevNextReview).replace("{when}", when)} }
            }
            button { class: "primary", onclick: move |_| engine.home(), "{ui.t(Msg::RevBackHome)}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::srs::{Rating, ReviewMode};

    use super::*;

    #[test]
    fn the_deck_is_rebuilt_only_after_the_store_changes() {
        let db = Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())));
        let sched = Rc::new(Scheduler::default());
        db.0.borrow_mut().upsert_cards(&seed_bridge_deck()).unwrap();
        let first = deck(&db, &sched);
        assert!(Rc::ptr_eq(&first, &deck(&db, &sched)), "nothing changed: the cached deck");
        let card = seed_bridge_deck()[0].id.clone();
        db.0.borrow_mut()
            .record_review(&card, Rating::Good, 1_761_472_800_000, 2000, ReviewMode::Classic)
            .unwrap();
        let second = deck(&db, &sched);
        assert!(!Rc::ptr_eq(&first, &second));
        assert_eq!(second.state(&card).reps, 1);
        // Another store never gets this one's deck.
        let other = Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())));
        assert!(deck(&other, &sched).is_empty());
        // Nor does a deck built with another scheduler.
        let travel = crate::schedule::scheduler_at(420, &Default::default());
        assert!(!Rc::ptr_eq(&deck(&db, &sched), &deck(&db, &travel)));
    }

    /// Undo puts everything back as it was: the card's schedule, the
    /// session (card in hand again), and the log (the event tombstoned, no
    /// second event). Undoing with nothing answered changes nothing.
    #[test]
    fn undo_restores_schedule_session_and_log() {
        let db = Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())));
        let sched = Scheduler::default();
        db.0.borrow_mut().upsert_cards(&seed_bridge_deck()).unwrap();
        let now = 1_761_472_800_000;
        let mut deck = read_deck(&db, &sched);
        let queue = deck.queue(&sched, now, |_| true);
        let mut session = Session::new(ReviewMode::Classic, queue.clone(), now);
        let card = session.pull().unwrap();
        let before = deck.state(&card);
        let preview = sched.preview(&before, now);

        assert!(!take_back(&mut db.0.borrow_mut(), &mut session, &mut deck, &sched).unwrap());

        let event =
            db.0.borrow_mut().record_review(&card, Rating::Easy, now, 900, ReviewMode::Classic).unwrap();
        deck.record(event.clone(), &sched);
        session.answer(event.id.clone(), card.clone(), Rating::Easy, 900);
        assert_ne!(deck.state(&card), before);

        assert!(take_back(&mut db.0.borrow_mut(), &mut session, &mut deck, &sched).unwrap());
        assert_eq!(deck.state(&card), before);
        assert_eq!(sched.preview(&deck.state(&card), now), preview, "the buttons show the same intervals");
        assert!(session.answers().is_empty());
        assert_eq!(session.pull(), Some(card.clone()), "the card is back in hand");
        assert!(db.0.borrow().review_log().unwrap().is_empty());
        // The log and the in-memory deck agree after a full replay.
        assert_eq!(read_deck(&db, &sched).state(&card), before);
        // A second undo has nothing left to take back.
        assert!(!take_back(&mut db.0.borrow_mut(), &mut session, &mut deck, &sched).unwrap());
    }

    #[test]
    fn javanese_false_friends_reach_review_cards() {
        let card = |word: &str, language| Card {
            v: 1,
            id: format!("{word}#r"),
            word: word.into(),
            language,
            kind: CardKind::Recognition,
            meaning: Default::default(),
            note: None,
            example: None,
            arabic: None,
            source: Default::default(),
            source_ref: None,
            verified: false,
        };
        assert_eq!(indonesian_twin(&card("awan", StudyLanguage::Jv)), Some("awan"));
        assert_eq!(indonesian_twin(&card("Mau", StudyLanguage::Jv)), Some("mau"));
        // Indonesian awan is just a cloud; ordinary Javanese words carry nothing.
        assert_eq!(indonesian_twin(&card("awan", StudyLanguage::Id)), None);
        assert_eq!(indonesian_twin(&card("enjing", StudyLanguage::Jv)), None);
        assert!(JV_TWINS.len() >= 10, "{:?}", JV_TWINS.keys());
    }

    #[test]
    fn lessons_and_lookups_move_deck_words() {
        use kabar_core::review::Signal as S;
        use kabar_protocol::srs::Evidence;
        const DAY: u64 = 86_400_000;
        let t0 = 1_761_472_800_000;
        let db = Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())));
        let cards = seed_bridge_deck();
        db.0.borrow_mut().upsert_cards(&cards).unwrap();
        let mut recognition = cards.iter().filter(|c| c.kind == CardKind::Recognition);
        let (a, b) = (recognition.next().unwrap(), recognition.next().unwrap());
        for c in [a, b] {
            db.0.borrow_mut().record_review(&c.id, Rating::Good, t0, 2000, ReviewMode::Classic).unwrap();
        }
        let signals = [(a.word.to_uppercase(), S::Recalled), (b.word.clone(), S::LookedUp)];
        let lang = a.language;
        // b's Bridge card was never reviewed, so only its recognition card moves.
        assert_eq!(note_evidence_at(&db, 180, lang, &signals, t0 + 30 * DAY), 2);
        // Once a day per card.
        assert_eq!(note_evidence_at(&db, 180, lang, &signals, t0 + 30 * DAY + 60_000), 0);
        let log = db.0.borrow().review_log().unwrap();
        let evidence: Vec<(&str, Rating, Evidence)> =
            log[2..].iter().map(|e| (e.card_id.as_str(), e.rating, e.evidence)).collect();
        assert!(evidence.contains(&(a.id.as_str(), Rating::Good, Evidence::Exercise)), "{evidence:?}");
        assert!(evidence.contains(&(b.id.as_str(), Rating::Again, Evidence::Lookup)), "{evidence:?}");
        assert!(log[2..].iter().all(|e| e.duration_ms == 0));
        let sched = Rc::new(Scheduler::default());
        let deck = deck(&db, &sched);
        assert_eq!(deck.state(&b.id).lapses, 1);
        assert_eq!(deck.stats(&sched, t0 + 30 * DAY).reviews_today, 0, "evidence isn't a review");
    }
}
