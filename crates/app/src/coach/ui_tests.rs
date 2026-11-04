//! Kak Nur's screens driven like a learner drives them: onboarding to the
//! saved profile, Today's plan ticked off, the Warung, the journal and the
//! coach settings. Each test checks the screen and what was stored.

use kabar_core::coach::rewards;
use kabar_core::locale::UiLanguage::{self, ArEg, En};
use kabar_protocol::coach::{
    ActivityKind, CoachProfile, LanguageFocus, LocalDate, LocalTime, PlanItemKind, PlanSize, ReminderStyle,
    StartingPoint, Weekday,
};

use super::text::{T, tt};
use super::*;
use crate::Screen;
use crate::i18n::{Msg, t};
use crate::test_dom::{App, El, memory_db};
use crate::testkit::FakeDoc as FakeDocument;

fn en(msg: Msg) -> &'static str {
    t(En, msg)
}

/// 2026-11-04 09:00 in Cairo (UTC+2, the profile default).
const MORNING: u64 = 1_793_775_600_000;
const HOUR: u64 = 3_600_000;

fn profile() -> CoachProfile {
    CoachProfile {
        name: "Mohamed".into(),
        reminder_style: ReminderStyle::Gentle,
        start: StartingPoint::BrandNew,
        ..Default::default()
    }
}

fn db_with(profile: Option<CoachProfile>) -> crate::Db {
    let db = memory_db();
    if let Some(p) = profile {
        db.0.borrow_mut().save_coach_profile(&p).unwrap();
    }
    db
}

/// Mounts `body` with the profile saved, the clock at `now`.
fn mount(lang: UiLanguage, db: crate::Db, now: u64, body: fn() -> Element) -> App {
    let mut app = App::with(lang, db, FakeDocument::new(), body);
    set_clock(&mut app, now);
    app
}

fn set_clock(app: &mut App, now: u64) {
    let coach = app.ctx::<Coach>();
    let mut now_ms = coach.now_ms;
    app.dom.in_app(|| now_ms.set(now));
}

fn today_of(now: u64) -> LocalDate {
    clock::today(now, profile().utc_offset_min)
}

fn coach_of(app: &mut App) -> Coach {
    app.ctx::<Coach>()
}

fn stored_profile(app: &App) -> CoachProfile {
    app.db.0.borrow().coach_profile().unwrap().expect("a saved profile")
}

/// Logs `kind` on `date` straight into the store (another device, earlier).
fn seed(db: &crate::Db, kind: ActivityKind, date: LocalDate, seconds: u32) {
    db.0.borrow_mut().add_activity(kind, date, seconds).unwrap();
}

fn reviews(cards: u32) -> ActivityKind {
    ActivityKind::Reviews { cards, correct: cards }
}

// --- onboarding ---------------------------------------------------------------

fn onboarding_app(lang: UiLanguage) -> App {
    mount(lang, memory_db(), MORNING, || rsx! { onboarding::Onboarding {} })
}

fn next(app: &mut App, lang: UiLanguage) {
    app.click_text(t(lang, Msg::Next));
}

fn pressed(app: &App) -> Vec<String> {
    app.buttons()
        .into_iter()
        .filter(|&b| app.attr(b, "aria-pressed") == Some("true"))
        .map(|b| app.text(b))
        .collect()
}

#[test]
fn onboarding_walks_every_step_and_saves_the_profile() {
    let mut app = onboarding_app(En);
    let dots = app.by_class("dot");
    assert_eq!(dots.len(), kabar_core::coach::onboarding::Step::ALL.len());
    assert!(app.has_class(dots[0], "current"));
    // Name: Next waits for a name; there is no Back on the first step.
    assert!(app.disabled(app.button(en(Msg::Next))));
    assert!(app.try_find("button", en(Msg::Back)).is_none());
    let name = app.find_all("input")[0];
    app.input(name, "  Mohamed ");
    next(&mut app, En);
    // Why: chips toggle; a note is optional.
    assert!(app.has_class(app.by_class("dot")[0], "done"));
    app.click_text(en(Msg::WhyTravel));
    app.click_text(en(Msg::WhyWork));
    app.click_text(en(Msg::WhyTravel));
    assert_eq!(pressed(&app), vec![en(Msg::WhyWork).to_string()]);
    let note = app.find_all("textarea")[0];
    app.input(note, "For my trip to Yogya");
    next(&mut app, En);
    // Focus: Javanese.
    app.click_on("button", "Basa Jawa");
    assert_eq!(pressed(&app), vec!["Basa Jawa".to_string()]);
    // Back and forward keeps the answer.
    app.click_text(en(Msg::Back));
    app.click_text(en(Msg::Back));
    next(&mut app, En);
    next(&mut app, En);
    assert_eq!(pressed(&app), vec!["Basa Jawa".to_string()]);
    next(&mut app, En);
    // Start: knows the basics.
    app.click_on("button", en(Msg::StartBasics));
    next(&mut app, En);
    // Minutes: the deep plan.
    let deep =
        labels::minutes_text(En, u32::from(PlanSize::Deep.minutes()), &PlanSize::Deep.minutes().to_string());
    let radio = app.find("button", &deep);
    app.click(radio);
    assert_eq!(app.attr(app.find("button", &deep), "aria-checked"), Some("true"));
    next(&mut app, En);
    // Target: a suggested date, explained as an estimate.
    assert!(app.screen().contains(tt(En, T::TargetEstimateHint)));
    let level = app.find_all("select")[0];
    app.change(level, "j2");
    // A chosen target is the learner's own: no estimate note.
    assert!(!app.screen().contains(tt(En, T::TargetEstimateHint)));
    let year = app.find_all("select")[2];
    app.change(year, "2029");
    let month = app.find_all("select")[1];
    app.change(month, "6");
    next(&mut app, En);
    // Times: add one, change it, remove the first.
    app.click_on("button", en(Msg::AddTime));
    assert_eq!(app.by_class("times")[0..1].len(), 1);
    assert_eq!(app.find_all("li").iter().filter(|&&l| app.text(l).contains(":")).count(), 2);
    let hours = app.find_all("select")[2];
    app.change(hours, "7");
    app.click_on("button", en(Msg::RemoveTime));
    next(&mut app, En);
    // Reminders: steady.
    app.click_on("button", en(Msg::StyleSteady));
    next(&mut app, En);
    // Done: the summary reads back the answers.
    let summary = app.text(app.find_all("dl")[0]);
    for part in [
        "Mohamed",
        en(Msg::WhyWork),
        "Basa Jawa",
        "June 2029",
        "J2",
        en(Msg::StyleSteady),
        en(Msg::StartBasics),
    ] {
        assert!(summary.contains(part), "missing {part}: {summary}");
    }
    app.click_text(en(Msg::OnbFinish));
    let p = stored_profile(&app);
    assert_eq!(p.name, "Mohamed");
    assert_eq!(p.motivations, vec![kabar_protocol::coach::Motivation::Work]);
    assert_eq!(p.why_note.as_deref(), Some("For my trip to Yogya"));
    assert_eq!(p.focus, LanguageFocus::Javanese);
    assert_eq!(p.daily_size, PlanSize::Deep);
    let target = p.target.expect("a target");
    assert_eq!((target.level.as_str(), target.by.ymd()), ("j2", (2029, 6, 1)));
    assert_eq!(p.study_times.len(), 1);
    assert_eq!(p.reminder_style, ReminderStyle::Steady);
    assert_eq!(p.start, StartingPoint::KnowsBasics);
    let coach = coach_of(&mut app);
    assert_eq!(coach.profile.peek().as_ref().map(|p| p.name.clone()), Some("Mohamed".into()));
}

#[test]
fn onboarding_can_start_early_with_reminders_off() {
    let mut app = onboarding_app(ArEg);
    let name = app.find_all("input")[0];
    app.input(name, "محمد");
    for _ in 0..3 {
        next(&mut app, ArEg);
        // Not before the starting point is chosen.
        assert!(app.try_find("button", tt(ArEg, T::StartNow)).is_none());
    }
    next(&mut app, ArEg);
    assert!(app.screen().contains(tt(ArEg, T::StartNowHint)));
    app.click_text(tt(ArEg, T::StartNow));
    let p = stored_profile(&app);
    assert_eq!(p.name, "محمد");
    assert_eq!(p.reminder_style, ReminderStyle::Off);
}

#[test]
fn onboarding_without_a_target_and_back_to_an_unknown_level() {
    let mut app = onboarding_app(En);
    let name = app.find_all("input")[0];
    app.input(name, "Sam");
    for _ in 0..5 {
        next(&mut app, En);
    }
    // Both languages: the order note shows on the target step.
    // No target: say so, then offer one back.
    app.click_text(en(Msg::OnbNoTarget));
    assert!(app.screen().contains(en(Msg::OnbNoTargetChosen)));
    app.click_text(en(Msg::OnbSetTarget));
    assert!(app.try_find("button", en(Msg::OnbNoTarget)).is_some());
    // A level the core doesn't know sends the learner back with a message.
    let level = app.find_all("select")[0];
    app.change(level, "zz9");
    for _ in 0..3 {
        next(&mut app, En);
    }
    app.click_text(en(Msg::OnbFinish));
    assert!(app.db.0.borrow().coach_profile().unwrap().is_none());
    let alert =
        app.find_all("p").into_iter().find(|&p| app.attr(p, "role") == Some("alert")).expect("an error");
    assert_eq!(app.text(alert), en(Msg::ErrUnknownLevel));
    assert!(app.try_find("button", en(Msg::OnbNoTarget)).is_some(), "back on the target step");
    // Going back clears the message.
    app.click_text(en(Msg::Back));
    assert!(app.find_all("p").into_iter().all(|p| app.attr(p, "role") != Some("alert")));
}

#[test]
fn onboarding_for_both_languages_explains_the_order() {
    let mut app = onboarding_app(En);
    let name = app.find_all("input")[0];
    app.input(name, "Sam");
    next(&mut app, En);
    next(&mut app, En);
    app.click_on("button", en(Msg::FocusBoth));
    for _ in 0..3 {
        next(&mut app, En);
    }
    assert!(app.screen().contains(en(Msg::OnbBothOrder)));
    // A month in the past is pushed to next month.
    let today = today_of(MORNING);
    let year = app.find_all("select")[2];
    app.change(year, &today.ymd().0.to_string());
    let month = app.find_all("select")[1];
    app.change(month, "1");
    for _ in 0..3 {
        next(&mut app, En);
    }
    let summary = app.text(app.find_all("dl")[0]);
    assert!(summary.contains("December 2026"), "{summary}");
    assert!(summary.contains(en(Msg::OnbBothOrder)));
}

// --- Today --------------------------------------------------------------------

fn today_app(lang: UiLanguage, db: crate::Db, now: u64) -> App {
    mount(lang, db, now, || rsx! { today::Today {} })
}

fn stored_plan(app: &App, date: LocalDate) -> kabar_protocol::coach::DailyPlan {
    app.db.0.borrow().plan_for(date).unwrap().expect("a stored plan")
}

#[test]
fn today_shows_the_plan_and_changes_its_size() {
    let db = db_with(Some(profile()));
    let mut app = today_app(En, db, MORNING);
    let date = today_of(MORNING);
    let plan = stored_plan(&app, date);
    assert_eq!(plan.size, profile().daily_size);
    // The hero names the first step and the whole plan's minutes.
    let hero = app.text(app.by_class("today-hero")[0]);
    assert!(hero.contains(tt(En, T::PlanEyebrow)), "{hero}");
    assert!(hero.contains(tt(En, T::StartPlan)));
    // Kak Nur greets by name and her line opens her page.
    let nur = app.by_class("nur-card")[0];
    assert!(app.text(nur).contains("Mohamed"));
    app.click(nur);
    assert_eq!(app.screen_now(), Screen::Coach);
    // A bigger size rebuilds today's plan, saved for every device.
    let deep =
        labels::minutes_text(En, u32::from(PlanSize::Deep.minutes()), &PlanSize::Deep.minutes().to_string());
    app.click_on("button", &deep);
    let plan = stored_plan(&app, date);
    assert_eq!(plan.size, PlanSize::Deep);
    assert_eq!(app.attr(app.find("button", &deep), "aria-checked"), Some("true"));
    assert!(app.text(app.by_class("today-hero")[0]).contains(&deep));
}

#[test]
fn today_steps_tick_off_and_untick() {
    let db = db_with(Some(CoachProfile { daily_size: PlanSize::Deep, ..profile() }));
    let mut app = today_app(En, db, MORNING);
    let date = today_of(MORNING);
    let plan = stored_plan(&app, date);
    assert!(plan.items.len() > 1, "{plan:?}");
    // The rest of the plan, each row with its tick box.
    let checks: Vec<El> = app.by_class("check");
    assert_eq!(checks.len(), plan.items.len() - 1);
    assert!(checks.iter().all(|&c| app.attr(c, "aria-checked") == Some("false")));
    app.click(checks[0]);
    let done: Vec<_> =
        app.activity().into_iter().filter(|e| matches!(e.kind, ActivityKind::PlanItemDone { .. })).collect();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].date, date);
    let checked = app.by_class("check").into_iter().find(|&c| app.attr(c, "aria-checked") == Some("true"));
    let checked = checked.expect("the row is ticked");
    assert_eq!(app.text(checked), "✓");
    // Unticking takes it back (a tombstone, not a second event).
    app.click(checked);
    assert!(app.activity().iter().all(|e| !matches!(e.kind, ActivityKind::PlanItemDone { .. })));
    // Started: the hero continues, counting what's left.
    seed_minutes(&mut app, 120);
    let hero = app.text(app.by_class("today-hero")[0]);
    assert!(hero.contains(tt(En, T::ContinuePlan)), "{hero}");
}

/// Logs `seconds` of reviewing today through the coach (the screen updates).
fn seed_minutes(app: &mut App, seconds: u32) {
    let coach = coach_of(app);
    let db = app.db.clone();
    app.dom.in_app(|| {
        coach.record(&db, reviews(3), seconds);
    });
}

#[test]
fn today_rows_open_what_they_say() {
    let db = db_with(Some(CoachProfile { daily_size: PlanSize::Deep, ..profile() }));
    let mut app = today_app(En, db, MORNING);
    let plan = stored_plan(&app, today_of(MORNING));
    for (i, item) in plan.items.iter().enumerate().skip(1) {
        let ui = app.ctx::<crate::Ui>();
        let mut screen = ui.screen;
        app.dom.in_app(|| screen.set(Screen::Today));
        let rows = app.by_class("plan-text");
        app.click(rows[i - 1]);
        let now = app.screen_now();
        let expected = match &item.kind {
            PlanItemKind::Review { .. } => vec![Screen::Review],
            PlanItemKind::Lesson { .. } => vec![Screen::Lesson, Screen::Journey],
            PlanItemKind::WeakSpot { .. } => vec![Screen::Journey],
            PlanItemKind::Reading => vec![Screen::Reader],
        };
        assert!(expected.contains(&now), "{:?} opened {now:?}", item.kind);
    }
    // The hero's Start opens the first step.
    let ui = app.ctx::<crate::Ui>();
    let mut screen = ui.screen;
    app.dom.in_app(|| screen.set(Screen::Today));
    app.click_on("button", tt(En, T::StartPlan));
    assert_ne!(app.screen_now(), Screen::Today);
}

#[test]
fn today_rewards_bar_opens_stats_and_the_warung() {
    let date = today_of(MORNING);
    let db = db_with(Some(profile()));
    seed(&db, reviews(40), date.add_days(-1), 900);
    let mut app = today_app(En, db, MORNING);
    let bar = app.text(app.by_class("rewards-bar")[0]);
    assert!(bar.contains("Pelita") && bar.contains("Koin"), "{bar}");
    let pelita = app.by_class("reward")[0];
    app.click(pelita);
    assert_eq!(app.screen_now(), Screen::Stats);
    let koin = app.by_class("reward")[1];
    app.click(koin);
    assert_eq!(app.screen_now(), Screen::Coach);
}

#[test]
fn today_snooze_waits_an_hour_and_says_until_when() {
    let db = db_with(Some(profile()));
    let mut app = today_app(En, db, MORNING);
    app.click_text(en(Msg::Snooze));
    let state = app.db.0.borrow().reminder_state().unwrap();
    let until = state.snoozed_until_ms.expect("snoozed");
    assert!(until > MORNING);
    assert!(app.screen().contains(en(Msg::SnoozedUntil)));
    assert!(app.try_find("button", en(Msg::Snooze)).is_none());
    // Reminders off: no snooze at all.
    let db = db_with(Some(CoachProfile { reminder_style: ReminderStyle::Off, ..profile() }));
    let app = today_app(En, db, MORNING);
    assert!(app.try_find("button", en(Msg::Snooze)).is_none());
}

#[test]
fn today_after_a_break_opens_on_a_welcome_back() {
    let date = today_of(MORNING);
    let db = db_with(Some(profile()));
    seed(&db, reviews(10), date.add_days(-20), 600);
    let mut app = today_app(ArEg, db, MORNING);
    let card = app.by_class("welcome-back");
    assert_eq!(card.len(), 1, "{}", app.screen());
    let text = app.text(card[0]);
    assert!(text.contains(t(ArEg, Msg::WelcomeBackTitle)));
    // No streak talk: the lamp is ready to light, not a count.
    assert!(app.screen().contains(t(ArEg, Msg::PelitaReady)));
    app.click_text(t(ArEg, Msg::WelcomeBackStart));
    assert_ne!(app.screen_now(), Screen::Today);
    // A bigger plan is one tap away, and then the usual Today.
    let deep = labels::minutes_text(
        ArEg,
        u32::from(PlanSize::Deep.minutes()),
        &PlanSize::Deep.minutes().to_string(),
    );
    app.click_on("button", &deep);
    assert_eq!(stored_plan(&app, date).size, PlanSize::Deep);
    assert!(app.by_class("welcome-back").is_empty());
}

#[test]
fn today_done_offers_one_more_lesson_and_an_evening_recap() {
    let date = today_of(MORNING);
    let db = db_with(Some(CoachProfile { daily_size: PlanSize::Quick, ..profile() }));
    let mut app = today_app(En, db, MORNING);
    let plan = stored_plan(&app, date);
    // Every step done, and the time studied.
    let coach = coach_of(&mut app);
    let db = app.db.clone();
    app.dom.in_app(|| {
        for item in &plan.items {
            coach.record(
                &db,
                ActivityKind::PlanItemDone { item_id: item.id.clone() },
                u32::from(item.minutes) * 60,
            );
        }
        coach.record(&db, ActivityKind::NewWords { words: vec!["kabar".into(), "pagi".into()] }, 60);
    });
    let hero = app.text(app.by_class("today-hero")[0]);
    assert!(hero.contains(tt(En, T::AllDoneTitle)), "{hero}");
    assert!(app.try_find("button", en(Msg::Snooze)).is_none(), "no reminder once the goal is met");
    // After the last study time: the day's recap with the new words.
    set_clock(&mut app, MORNING + 13 * HOUR);
    let recap = app.by_class("recap");
    assert_eq!(recap.len(), 1, "{}", app.screen());
    let text = app.text(recap[0]);
    assert!(text.contains(en(Msg::RecapTitle)) && text.contains("kabar") && text.contains("pagi"), "{text}");
    // One more lesson opens a lesson (or the Journey when there is none).
    app.click_on("button", tt(En, T::OneMoreLesson));
    assert!(matches!(app.screen_now(), Screen::Lesson | Screen::Journey));
}

// --- Kak Nur's page -----------------------------------------------------------

fn coach_page(lang: UiLanguage, db: crate::Db, now: u64) -> App {
    mount(lang, db, now, || rsx! { coach_screen::CoachScreen {} })
}

#[test]
fn the_coach_page_shows_the_next_step_the_report_and_achievements() {
    let date = today_of(MORNING);
    let db = db_with(Some(profile()));
    for back in 1..=3 {
        seed(&db, reviews(20), date.add_days(-back), 900);
    }
    let mut app = coach_page(En, db, MORNING);
    assert!(app.screen().contains(tt(En, T::CoachNextTitle)));
    assert!(app.screen().contains(en(Msg::ReportTitle)));
    assert!(app.screen().contains("Liga Pribadi"));
    assert_eq!(app.by_class("achievement").len(), rewards::achievements().len());
    // The next step starts from here too.
    app.click_on("button", &labels::minutes_text(En, 5, "5").replace("5 min", "min"));
    assert_ne!(app.screen_now(), Screen::Today);
    // Change the plan / reminders open Settings › Kak Nur.
    app.click_on("button", tt(En, T::EditPlan));
    let nav = app.ctx::<crate::settings::SettingsNav>();
    assert!(*nav.open.peek());
    assert_eq!(*nav.section.peek(), Some(crate::settings::Section::Coach));
    assert!(app.screen().contains(tt(En, T::RemindersOn)));
    // The journal card opens the journal.
    app.click_on("button", en(Msg::JournalOpen));
    assert_eq!(app.screen_now(), Screen::Journal);
}

#[test]
fn the_weekly_report_switches_between_last_week_and_this_one() {
    let date = today_of(MORNING);
    let db = db_with(Some(profile()));
    let last = date.week_start().add_days(-7);
    for d in 0..5 {
        seed(&db, reviews(30), last.add_days(d), 1200);
    }
    // Last week's report was made (on its day) and saved.
    let saved = kabar_core::coach::report::weekly_report(
        &db.0.borrow().activity(None).unwrap(),
        last,
        3,
        rewards::league(&db.0.borrow().activity(None).unwrap(), last, 8),
    );
    db.0.borrow_mut().save_report(&saved).unwrap();
    let mut app = coach_page(En, db, MORNING);
    let report = app.find_all("button").into_iter().find(|&b| app.text(b) == en(Msg::ReportLast)).unwrap();
    assert_eq!(app.attr(report, "aria-pressed"), Some("true"));
    assert!(!app.screen().contains(en(Msg::SoFar)));
    app.click_text(en(Msg::ReportThisWeek));
    assert!(app.screen().contains(en(Msg::SoFar)));
    app.click_text(en(Msg::ReportLast));
    assert!(!app.screen().contains(en(Msg::SoFar)));
    // Focus suggestions open where that skill is practised.
    if let Some(&focus) = app.by_class("tap-plain").first() {
        app.click(focus);
        assert_ne!(app.screen_now(), Screen::Today);
    }
}

#[test]
fn the_warung_sells_a_batik_uses_it_and_takes_it_back() {
    let date = today_of(MORNING);
    let db = db_with(Some(profile()));
    for back in 0..10 {
        seed(&db, reviews(400), date.add_days(-back), 3600);
    }
    let balance = rewards::koin_balance(&db.0.borrow().activity(None).unwrap());
    let mut app = coach_page(En, db, MORNING);
    let item = *rewards::warung().iter().find(|i| i.koin <= balance).expect("an affordable batik");
    assert!(app.screen().contains(&format!("Koin {balance}")), "{}", app.screen());
    // Too dear items show how far off they are, not a dead button.
    for dear in rewards::warung().iter().filter(|i| i.koin > balance) {
        let _ = dear;
        assert!(!app.by_class("koin-to-go").is_empty());
    }
    let ware = app.by_class("ware").into_iter().find(|&w| app.text(w).contains(item.name)).unwrap();
    let buy =
        app.find_all("button").into_iter().find(|&b| app.text(b) == en(Msg::Buy) && is_inside(&app, b, ware));
    app.click(buy.expect("a buy button"));
    let coach = coach_of(&mut app);
    assert_eq!(coach.batik.peek().as_deref(), Some(item.id));
    assert_eq!(coach.batik_color(), item.color);
    assert!(
        app.activity()
            .iter()
            .any(|e| matches!(&e.kind, ActivityKind::Purchase { item_id, .. } if item_id == item.id))
    );
    assert!(app.screen().contains(&format!("Koin {}", balance - item.koin)));
    // The toast offers Undo: the koin come back and the classic batik returns.
    app.click_text(en(Msg::Undo));
    assert!(app.activity().iter().all(|e| !matches!(e.kind, ActivityKind::Purchase { .. })));
    assert_eq!(coach.batik.peek().as_deref(), None);
    assert_eq!(coach.batik_color(), DEFAULT_BATIK);
    assert!(app.screen().contains(&format!("Koin {balance}")));
    // Buy again, dismiss, switch to the classic and back.
    let ware = app.by_class("ware").into_iter().find(|&w| app.text(w).contains(item.name)).unwrap();
    let buy =
        app.find_all("button").into_iter().find(|&b| app.text(b) == en(Msg::Buy) && is_inside(&app, b, ware));
    app.click(buy.unwrap());
    app.click_text(en(Msg::Dismiss));
    assert!(app.by_class("toast").is_empty());
    let classic = app.by_class("ware")[0];
    let use_classic = app.find_all("button").into_iter().find(|&b| is_inside(&app, b, classic)).unwrap();
    app.click(use_classic);
    assert_eq!(coach.batik.peek().as_deref(), None);
    assert_eq!(app.db.0.borrow().local_value("batik").unwrap().as_deref(), Some(""));
    let ware = app.by_class("ware").into_iter().find(|&w| app.text(w).contains(item.name)).unwrap();
    let use_it =
        app.find_all("button").into_iter().find(|&b| app.text(b) == en(Msg::Use) && is_inside(&app, b, ware));
    app.click(use_it.expect("owned: a Use button"));
    assert_eq!(coach.batik.peek().as_deref(), Some(item.id));
}

fn is_inside(app: &App, el: El, container: El) -> bool {
    app.contains(container, el)
}

// --- the journal ----------------------------------------------------------------

fn journal_app(db: crate::Db) -> App {
    mount(En, db, MORNING, || rsx! { journal::JournalScreen {} })
}

#[test]
fn the_journal_keeps_reflections_written_edited_and_deleted() {
    let mut app = journal_app(db_with(Some(profile())));
    assert!(app.screen().contains(en(Msg::JournalEmpty)));
    app.click_text(en(Msg::JournalReflectionAdd));
    let save = app.button(en(Msg::JournalReflectionSave));
    assert!(app.disabled(save), "nothing to save yet");
    let field = app.find_all("textarea")[0];
    app.input(field, "  Hari ini saya belajar angka.  ");
    app.click_text(en(Msg::JournalReflectionSave));
    let stored = app.db.0.borrow().reflections().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].text, "Hari ini saya belajar angka.");
    assert_eq!(stored[0].date, today_of(MORNING));
    assert!(app.screen().contains("Hari ini saya belajar angka."));
    // Edit it.
    app.click_text(en(Msg::JournalReflectionEdit));
    let field = app.find_all("textarea")[0];
    app.input(field, "Hari ini saya belajar angka dan warna.");
    app.click_text(en(Msg::JournalReflectionSave));
    let stored = app.db.0.borrow().reflections().unwrap();
    assert_eq!(stored[0].text, "Hari ini saya belajar angka dan warna.");
    assert!(stored[0].updated_ms >= stored[0].created_ms);
    // Cancel an edit: nothing changes.
    app.click_text(en(Msg::JournalReflectionEdit));
    app.click_text(en(Msg::JournalReflectionCancel));
    assert!(app.find_all("textarea").is_empty());
    // Delete asks first; Cancel keeps it.
    app.click_text(en(Msg::JournalReflectionDelete));
    app.click_text(en(Msg::JournalReflectionCancel));
    assert_eq!(app.db.0.borrow().reflections().unwrap().len(), 1);
    app.click_text(en(Msg::JournalReflectionDelete));
    app.click_text(en(Msg::JournalReflectionDeleteSure));
    assert!(app.db.0.borrow().reflections().unwrap().is_empty());
    assert!(!app.screen().contains("warna"));
}

#[test]
fn the_journal_lists_the_days_activity_and_filters_it() {
    let date = today_of(MORNING);
    let db = db_with(Some(profile()));
    seed(&db, reviews(12), date, 600);
    seed(&db, ActivityKind::NewWords { words: vec!["rumah".into(), "makan".into()] }, date, 60);
    seed(
        &db,
        ActivityKind::Practice {
            skill: kabar_protocol::coach::Skill::Listening,
            topic: "kabar-pagi".into(),
            correct: 4,
            total: 4,
        },
        date.add_days(-1),
        300,
    );
    seed(&db, ActivityKind::Purchase { item_id: "kawung-senja".into(), koin: 100 }, date.add_days(-1), 0);
    let mut app = journal_app(db);
    let screen = app.screen();
    for part in
        [en(Msg::EvReviews), "12 / 12", "rumah, makan", "kabar-pagi", "Kawung Senja", en(Msg::Yesterday)]
    {
        assert!(screen.contains(part), "missing {part}: {screen}");
    }
    // Words only.
    app.click_text(en(Msg::JournalFilterWords));
    assert!(app.screen().contains("rumah"));
    assert!(!app.screen().contains("kabar-pagi"));
    // Reflections only, and there are none: the way back to everything.
    app.click_text(en(Msg::JournalFilterReflections));
    assert!(app.screen().contains(en(Msg::JournalNothingHere)));
    app.click_text(en(Msg::JournalShowAll));
    assert!(app.screen().contains("kabar-pagi"));
    // Each line opens what it was.
    for (text, screen) in
        [("12 / 12", Screen::Review), ("kabar-pagi", Screen::Listen), ("Kawung Senja", Screen::Coach)]
    {
        app.click_on("button", text);
        assert_eq!(app.screen_now(), screen, "{text}");
    }
}

#[test]
fn the_journal_pages_older_days() {
    let date = today_of(MORNING);
    let db = db_with(Some(profile()));
    for back in 0..20 {
        seed(&db, reviews(1), date.add_days(-back), 60);
    }
    let mut app = journal_app(db);
    let days = app.by_class("journal-day").len();
    assert!(days < 20, "{days}");
    app.click_text(en(Msg::JournalShowOlder));
    assert!(app.by_class("journal-day").len() > days);
}

// --- coach settings -------------------------------------------------------------

#[test]
fn coach_settings_edit_one_field_at_a_time() {
    let db = db_with(Some(profile()));
    let mut app = mount(En, db, MORNING, || rsx! { settings::CoachSettings {} });
    let quick = labels::minutes_text(
        En,
        u32::from(PlanSize::Quick.minutes()),
        &PlanSize::Quick.minutes().to_string(),
    );
    app.click_on("button", &quick);
    assert_eq!(stored_profile(&app).daily_size, PlanSize::Quick);
    // Study times: add one, then remove one.
    app.click_on("button", en(Msg::AddTime));
    assert_eq!(stored_profile(&app).study_times.len(), 2);
    app.click_on("button", en(Msg::RemoveTime));
    assert_eq!(stored_profile(&app).study_times.len(), 1);
    // Reminder style.
    app.click_on("button", en(Msg::StyleSteady));
    assert_eq!(stored_profile(&app).reminder_style, ReminderStyle::Steady);
    // The report's day and hour.
    let selects = app.find_all("select");
    let day = selects
        .iter()
        .copied()
        .find(|&s| {
            app.attr(s, "aria-label") == Some(en(Msg::ReportWhen)) && app.text(s).contains(en(Msg::Mon))
        })
        .unwrap();
    app.change(day, "Sun");
    assert_eq!(stored_profile(&app).report_day, Weekday::Sun);
    app.change(day, "Someday");
    assert_eq!(stored_profile(&app).report_day, Weekday::Sun, "an unknown day changes nothing");
    let selects = app.find_all("select");
    let hour = selects[selects.len() - 2];
    app.change(hour, "8");
    assert_eq!(stored_profile(&app).report_time.hour(), 8);
    let minute = selects[selects.len() - 1];
    app.change(minute, "35");
    assert_eq!(stored_profile(&app).report_time, LocalTime::hm(8, 35));
    // Quiet hours: the start hour.
    let quiet_from = app
        .find_all("select")
        .into_iter()
        .find(|&s| app.attr(s, "aria-label") == Some(en(Msg::QuietFrom)))
        .unwrap();
    app.change(quiet_from, "23");
    assert_eq!(stored_profile(&app).quiet_hours.start.hour(), 23);
    // Not a number: nothing changes.
    app.change(quiet_from, "x");
    assert_eq!(stored_profile(&app).quiet_hours.start.hour(), 23);
}

#[test]
fn coach_settings_need_a_profile() {
    let app = mount(En, memory_db(), MORNING, || rsx! { settings::CoachSettings {} });
    assert!(app.screen().is_empty());
    let app = mount(En, memory_db(), MORNING, || rsx! { today::Today {} });
    assert!(app.screen().is_empty());
    let app = mount(En, memory_db(), MORNING, || rsx! { coach_screen::CoachScreen {} });
    assert!(app.screen().is_empty());
}

// --- the wording helpers ------------------------------------------------------

const ALL_T: [T; 20] = [
    T::PlanEyebrow,
    T::HowLong,
    T::StartPlan,
    T::ContinuePlan,
    T::MarkStepDone,
    T::ThenRows,
    T::AllDoneTitle,
    T::OneMoreLesson,
    T::LessonIn,
    T::NurOpens,
    T::CoachNextTitle,
    T::CoachDoneTitle,
    T::EditPlan,
    T::EditPlanHint,
    T::Reminders,
    T::RemindersOn,
    T::RemindersOff,
    T::StartNow,
    T::StartNowHint,
    T::TargetEstimateHint,
];

fn has_arabic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

#[test]
fn every_coach_string_reads_in_each_language() {
    for msg in ALL_T {
        let (eg, ar, en) = (tt(ArEg, msg), tt(UiLanguage::Ar, msg), tt(En, msg));
        assert!(has_arabic(eg) && has_arabic(ar), "{msg:?}");
        assert!(!has_arabic(en) && !en.is_empty(), "{msg:?}");
        // Kak Nur's name stays Latin inside the Arabic lines.
        if en.contains("Kak Nur") {
            assert!(eg.contains("Kak Nur") && ar.contains("Kak Nur"), "{msg:?}");
        }
    }
    // Egyptian and standard Arabic differ where the register does.
    assert_ne!(tt(ArEg, T::HowLong), tt(UiLanguage::Ar, T::HowLong));
    assert_eq!(super::text::left(UiLanguage::Ar, "٧ دقائق"), "متبقٍّ ٧ دقائق");
}

/// Runs `f` with the app's `Ui` in `lang`, Eastern digits or not.
fn with_ui<R>(lang: UiLanguage, eastern: bool, f: impl FnOnce(&crate::Ui) -> R) -> R {
    let mut app = App::new(lang, || rsx! {});
    let ui = app.ctx::<crate::Ui>();
    let mut settings = ui.settings;
    app.dom.in_app(|| settings.write().eastern_digits = eastern);
    app.dom.in_app(|| f(&ui))
}

#[test]
fn coach_labels_read_dates_times_and_plan_steps() {
    let today = LocalDate::from_ymd(2026, 11, 4).unwrap();
    with_ui(En, false, |ui| {
        assert_eq!(labels::date(ui, LocalDate::from_ymd(2026, 3, 9).unwrap(), today), "9/3");
        assert_eq!(labels::date(ui, LocalDate::from_ymd(2025, 3, 9).unwrap(), today), "9/3/2025");
        assert_eq!(labels::month(ui, 2), "February");
        assert_eq!(labels::month(ui, 0), "January", "out of range clamps");
        assert_eq!(labels::month(ui, 13), "December");
        assert_eq!(labels::month_short(ui, 9), "Sep");
        assert_eq!(labels::long_date(ui, today), "4 November 2026");
        assert_eq!(labels::time(ui, LocalTime::hm(7, 5)), "07:05");
        assert_eq!(labels::time(ui, LocalTime::hm(19, 30)), "19:30");
        assert_eq!(labels::day_heading(ui, today, today), en(Msg::TodayWord));
        assert_eq!(labels::day_heading(ui, today.add_days(-1), today), en(Msg::Yesterday));
        assert_eq!(labels::day_heading(ui, today.add_days(-2), today), format!("{} 2/11", en(Msg::Mon)));
        assert_eq!(labels::minutes(ui, 15), "15 min");
        // Plan steps: a title and, where it has one, a detail.
        let review = PlanItemKind::Review { cards: 12 };
        assert_eq!(labels::plan_item(ui, &review), (Msg::ItemReview, Some(format!("12 {}", en(Msg::Cards)))));
        assert_eq!(labels::plan_item(ui, &PlanItemKind::Review { cards: 0 }), (Msg::ItemReview, None));
        assert_eq!(labels::plan_item_text(ui, &PlanItemKind::Reading), en(Msg::ItemReading));
        let weak =
            PlanItemKind::WeakSpot { skill: kabar_protocol::coach::Skill::Listening, topic: "angka".into() };
        assert_eq!(
            labels::plan_item_text(ui, &weak),
            format!("{} (\u{2068}{} · angka\u{2069})", en(Msg::ItemWeakSpot), en(Msg::SkillListening))
        );
        let lesson = PlanItemKind::Lesson { unit_id: "bipa1.u01".into() };
        let (msg, detail) = labels::plan_item(ui, &lesson);
        assert_eq!(msg, Msg::ItemLesson);
        assert_ne!(detail.as_deref(), Some("bipa1.u01"), "a known unit shows its title");
        let unknown = PlanItemKind::Lesson { unit_id: "zz.u99".into() };
        assert_eq!(labels::plan_item(ui, &unknown).1.as_deref(), Some("zz.u99"));
        // Kak Nur's {count} and {topic}: reviews due and the first other step.
        let items: Vec<_> = [review.clone(), PlanItemKind::Reading]
            .into_iter()
            .enumerate()
            .map(|(i, kind)| kabar_protocol::coach::PlanItem { id: format!("i{i}"), kind, minutes: 5 })
            .collect();
        assert_eq!(labels::plan_voice_args(ui, &items), ("12".to_string(), en(Msg::ItemReading).to_string()));
        assert_eq!(labels::plan_voice_args(ui, &items[..1]), ("12".to_string(), String::new()));
    });
    with_ui(ArEg, true, |ui| {
        assert_eq!(labels::time(ui, LocalTime::hm(7, 5)), "٠٧:٠٥");
        assert_eq!(labels::month(ui, 2), "فبراير");
        assert_eq!(labels::month_short(ui, 2), "فبراير", "Arabic months stay whole");
        assert_eq!(labels::minutes(ui, 2), "دقيقتين");
        assert_eq!(labels::minutes(ui, 5), "٥ دقايق");
        assert_eq!(labels::long_date(ui, today), "٤ نوفمبر ٢٠٢٦");
    });
    // Every enum the pickers show has its own words.
    for s in [
        kabar_protocol::coach::Skill::Reading,
        kabar_protocol::coach::Skill::Listening,
        kabar_protocol::coach::Skill::Writing,
        kabar_protocol::coach::Skill::Speaking,
        kabar_protocol::coach::Skill::Vocabulary,
    ] {
        assert!(!en(labels::skill(s)).is_empty());
    }
    let weekdays: std::collections::BTreeSet<&str> =
        Weekday::ALL.iter().map(|d| en(labels::weekday(*d))).collect();
    assert_eq!(weekdays.len(), 7);
    let whys: std::collections::BTreeSet<&str> =
        labels::MOTIVATIONS.iter().map(|m| en(labels::motivation(*m))).collect();
    assert_eq!(whys.len(), labels::MOTIVATIONS.len());
    assert_eq!(labels::focus(LanguageFocus::Both), None);
    assert_eq!(labels::focus(LanguageFocus::Indonesian), Some("Bahasa Indonesia"));
}

// --- the coach's state ----------------------------------------------------------

#[test]
fn plan_steps_open_their_screen() {
    let mut app = mount(En, db_with(Some(profile())), MORNING, || rsx! {});
    let (ui, coach, db) = (app.ctx::<crate::Ui>(), coach_of(&mut app), app.db.clone());
    for (kind, expected) in [
        (PlanItemKind::Review { cards: 3 }, Screen::Review),
        (PlanItemKind::Reading, Screen::Reader),
        (
            PlanItemKind::WeakSpot { skill: kabar_protocol::coach::Skill::Reading, topic: "x".into() },
            Screen::Journey,
        ),
        (PlanItemKind::Lesson { unit_id: "bipa1.u01".into() }, Screen::Lesson),
        (PlanItemKind::Lesson { unit_id: "zz.u99".into() }, Screen::Lesson),
    ] {
        let mut screen = ui.screen;
        app.dom.in_app(|| {
            screen.set(Screen::Today);
            today::start_item(ui, &db, coach, &kind);
        });
        assert_eq!(app.screen_now(), expected, "{kind:?}");
    }
    // An unknown unit falls back to the Journey's next lesson.
    assert!(app.ctx::<crate::Ui>().lesson.peek().as_deref().is_some_and(|id| id.starts_with("bipa1.")));
    // Without a profile there is no Journey lesson to fall back on.
    let mut bare = mount(En, memory_db(), MORNING, || rsx! {});
    let (ui, coach, db) = (bare.ctx::<crate::Ui>(), coach_of(&mut bare), bare.db.clone());
    bare.dom.in_app(|| today::start_item(ui, &db, coach, &PlanItemKind::Lesson { unit_id: "zz.u99".into() }));
    assert_eq!(bare.screen_now(), Screen::Journey);
}

#[test]
fn a_failed_save_leaves_the_coach_as_it_was() {
    let mut app = mount(En, db_with(Some(profile())), MORNING, || rsx! {});
    let coach = coach_of(&mut app);
    let db = app.db.clone();
    let event = app.dom.in_app(|| coach.record(&db, reviews(2), 60)).expect("recorded");
    db.0.borrow().refuse_writes();
    app.dom.in_app(|| {
        // Each write fails: nothing changes in memory either.
        assert_eq!(coach.record(&db, reviews(2), 60), None);
        coach.save_profile(&db, CoachProfile { name: "Other".into(), ..profile() });
        coach.retract(&db, std::slice::from_ref(&event.id));
        coach.save_reminders(
            &db,
            kabar_protocol::coach::ReminderState { snoozed_until_ms: Some(1), ..Default::default() },
        );
    });
    assert_eq!(coach.profile.peek().as_ref().unwrap().name, "Mohamed");
    assert_eq!(coach.events.peek().len(), 1, "the undo failed, so the event stays");
    assert_eq!(coach.reminders.peek().snoozed_until_ms, None);
    // The batik still changes on screen; only saving it failed.
    app.dom.in_app(|| coach.use_batik(&db, Some("kawung-senja")));
    assert_eq!(coach.batik_color(), "c8573a");
    db.0.borrow().allow_writes();
    app.dom.in_app(|| coach.retract(&db, std::slice::from_ref(&event.id)));
    assert!(coach.events.peek().is_empty());
    // An unknown batik id falls back to the classic colour.
    app.dom.in_app(|| coach.use_batik(&db, Some("gone")));
    assert_eq!(coach.batik_color(), DEFAULT_BATIK);
}

#[test]
fn todays_plan_is_shared_once_made() {
    let mut app = mount(En, db_with(Some(profile())), MORNING, || rsx! {});
    let db = app.db.clone();
    let date = today_of(MORNING);
    let first = app.dom.in_app(|| plan_for(&db, &profile(), &[], date));
    // Another call (another screen, another device) reads the same plan.
    let again = app
        .dom
        .in_app(|| plan_for(&db, &CoachProfile { daily_size: PlanSize::Deep, ..profile() }, &[], date));
    assert_eq!(first, again);
    // Saving can fail: the plan still shows.
    let other = date.add_days(1);
    db.0.borrow().refuse_writes();
    let fresh = app.dom.in_app(|| plan_for(&db, &profile(), &[], other));
    db.0.borrow().allow_writes();
    assert_eq!(fresh.date, other);
    assert!(db.0.borrow().plan_for(other).unwrap().is_none());
    // Kak Nur says the same line all day.
    let ui = app.ctx::<crate::Ui>();
    let a = app.dom.in_app(|| say(&ui, kabar_core::coach::voice::Line::PlanMorning, date, &[("name", "Mo")]));
    let b = app.dom.in_app(|| say(&ui, kabar_core::coach::voice::Line::PlanMorning, date, &[("name", "Mo")]));
    assert_eq!(a, b);
}

fn clock_body() -> Element {
    let coach = use_context::<Coach>();
    let db = use_context::<crate::Db>();
    use_clock(coach, db);
    rsx! { p { "offset {coach.offset()}" } }
}

#[test]
fn the_clock_follows_the_devices_time_zone() {
    let doc = FakeDocument::new();
    // Jakarta (UTC+7): getTimezoneOffset() is -420; a junk reading is skipped.
    doc.reply_to(
        "getTimezoneOffset",
        vec![serde_json::json!(-420.0), serde_json::json!(99999.0), serde_json::json!(-420.0)],
    );
    let app = App::with(En, db_with(Some(profile())), doc, clock_body);
    assert!(app.screen().contains("offset 420"), "{}", app.screen());
    assert_eq!(stored_profile(&app).utc_offset_min, 420, "the profile follows the trip");
    // Without a profile, the device's own offset is used.
    let doc = FakeDocument::new();
    doc.reply_to("getTimezoneOffset", vec![serde_json::json!(-60.0)]);
    let app = App::with(En, memory_db(), doc, clock_body);
    assert!(app.screen().contains("offset 60"), "{}", app.screen());
}

#[test]
fn the_weekly_report_is_made_when_its_time_has_come() {
    // The report day is Monday 00:00, so this week's report is due now.
    let p = CoachProfile { report_day: Weekday::Mon, report_time: LocalTime::hm(0, 0), ..profile() };
    let app = App::with(En, db_with(Some(p)), FakeDocument::new(), || rsx! { coach_screen::CoachScreen {} });
    let week = clock::today(crate::store::now_ms(), profile().utc_offset_min).week_start();
    assert_eq!(app.db.0.borrow().latest_report_week().unwrap(), Some(week));
}

#[test]
fn the_journal_shows_finished_lessons_and_lesson_steps() {
    let now = crate::store::now_ms();
    let date = today_of(now);
    let db = db_with(Some(profile()));
    db.0.borrow_mut()
        .record_lesson_event("bipa1.u01.l01", "completed", &serde_json::json!({ "score": 81 }))
        .unwrap();
    seed(&db, ActivityKind::LessonStep { unit_id: "bipa1.u01".into(), done: false }, date, 300);
    seed(&db, ActivityKind::PlanItemDone { item_id: "x".into() }, date, 300);
    let mut app = mount(En, db, now, || rsx! { journal::JournalScreen {} });
    let screen = app.screen();
    assert!(screen.contains(en(Msg::EvLessonDone)) && screen.contains("81%"), "{screen}");
    assert!(screen.contains(en(Msg::EvLessonStep)) && screen.contains("bipa1.u01"));
    assert!(screen.contains(en(Msg::EvPlanItem)));
    // A finished lesson opens that lesson; a plan step opens Today.
    app.click_on("button", "81%");
    assert_eq!(app.screen_now(), Screen::Lesson);
    assert_eq!(app.ctx::<crate::Ui>().lesson.peek().as_deref(), Some("bipa1.u01.l01"));
    app.click_on("button", en(Msg::EvPlanItem));
    assert_eq!(app.screen_now(), Screen::Today);
    app.click_on("button", en(Msg::EvLessonStep));
    assert!(matches!(app.screen_now(), Screen::Lesson | Screen::Journey));
    // Learning only hides the words and keeps the lessons.
    app.click_text(en(Msg::JournalFilterLearning));
    assert!(app.screen().contains("81%"));
}
