//! Stats, the Today preview and the monthly level check, rendered from real
//! stores and pressed like the learner would.

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use kabar_core::review::seed_bridge_deck;
use kabar_protocol::coach::{ActivityKind, LocalDate, Skill};
use kabar_protocol::level_check::{
    LEVEL_CHECK_KIND, LEVEL_CHECK_LESSON_ID, LEVEL_CHECK_VERSION, LevelCheckResult, LevelEta, LevelMove,
    RadarPoint,
};
use kabar_protocol::srs::{Rating, ReviewMode};

use super::demo::{Demo, seed};
use super::text::{S, st};
use super::*;
use crate::Screen;
use crate::store::{Store, now_ms};
use crate::test_screens::{self as td, Setup};

const DAY_MS: u64 = 86_400_000;

fn view(screen: Screen) -> Option<Element> {
    match screen {
        Screen::Stats => Some(rsx! { StatsScreen {} }),
        Screen::LevelCheck => Some(rsx! { check::LevelCheckScreen {} }),
        Screen::Today => Some(rsx! { TodayCards {} StatsButton {} }),
        _ => None,
    }
}

fn empty() -> Store {
    Store::open_in_memory().unwrap()
}

fn demo(d: Demo) -> Store {
    let mut s = empty();
    seed(&mut s, &d, now_ms());
    s
}

fn en(d: Demo) -> Demo {
    Demo { lang: "en".into(), ..d }
}

fn check_result(date: LocalDate, moved: LevelMove, score: Option<f32>) -> LevelCheckResult {
    LevelCheckResult {
        v: LEVEL_CHECK_VERSION,
        date,
        language: "id".into(),
        from_level: "bipa1".into(),
        level: if moved == LevelMove::Up { "bipa2".into() } else { "bipa1".into() },
        moved,
        score,
        radar: vec![
            RadarPoint { skill: Skill::Reading, value: 0.62 },
            RadarPoint { skill: Skill::Vocabulary, value: 0.7 },
        ],
        eta: Some(LevelEta { level: "bipa2".into(), date: date.add_days(170) }),
        missed: Vec::new(),
        duration_ms: 1000,
    }
}

// ------------------------------------------------------------- formatting

#[test]
fn durations_levels_and_isolation_read_naturally() {
    fn app() -> Element {
        let ui = use_context_provider(|| Ui {
            settings: Signal::new(kabar_core::settings::Settings::default()),
            width: Signal::new(kabar_core::layout::WidthClass::Compact),
            tablet: Signal::new(false),
            screen: Signal::new(Screen::Stats),
            lesson: Signal::new(None),
            lesson_from: Signal::new(Screen::Journey),
            focus: Signal::new(false),
        });
        let mut settings = ui.settings;
        let mut out = Vec::new();
        for lang in [UiLanguage::En, UiLanguage::ArEg] {
            settings.write().ui_language = lang;
            out.push(format!(
                "{}|{}|{}|{}|{}",
                duration(&ui, 45),
                duration(&ui, 150),
                duration(&ui, 300),
                duration(&ui, 840),
                pct(&ui, 0.874)
            ));
            out.push(format!("{}", rtl(&ui)));
        }
        rsx! { for line in out { p { "{line}" } } }
    }
    let dom = crate::test_dom::TestDom::unsettled(VirtualDom::new(app));
    let html = td::html(&dom);
    // Minutes under two hours; then hours, with Arabic count agreement:
    // 3–10 plural (ساعات), 11+ singular (ساعة).
    assert!(html.contains("45 min|3 hours|5 hours|14 hours|87%"), "{html}");
    assert!(html.contains("<p>false</p>") && html.contains("<p>true</p>"), "{html}");
    assert!(html.contains("45 دقيقة|3 ساعات|5 ساعات|14 ساعة|"), "{html}");

    assert_eq!(level_label("bipa2"), "BIPA 2");
    assert_eq!(level_label("bipa-3"), "BIPA 3");
    assert_eq!(level_label("j1"), "J1");
    assert_eq!(iso("BIPA 2"), "\u{2068}BIPA 2\u{2069}");
    assert_eq!(back_arrow(UiLanguage::ArEg), "→");
    assert_eq!(back_arrow(UiLanguage::En), "←");
    assert_eq!(language_name(UiLanguage::En, StudyLanguage::Jv), st(UiLanguage::En, S::LangJv));
    assert_eq!(stf(UiLanguage::En, S::Week, &[("n", "7")]), "Last 7 days");
}

#[test]
fn every_weak_skill_opens_where_it_is_practised() {
    assert_eq!(skill_action(Skill::Reading), (Screen::Reader, S::WeakReading));
    assert_eq!(skill_action(Skill::Listening), (Screen::Listen, S::WeakListening));
    assert_eq!(skill_action(Skill::Vocabulary), (Screen::Review, S::WeakVocabulary));
    assert_eq!(skill_action(Skill::Writing), (Screen::Play, S::WeakPlay));
    assert_eq!(skill_action(Skill::Speaking), (Screen::Play, S::WeakPlay));
}

// ------------------------------------------------------------- the strings

/// The `{name}` placeholders in a line (a plural form `{n|a|b}` counts as `n`).
fn placeholders(s: &str) -> std::collections::BTreeSet<String> {
    s.split('{')
        .skip(1)
        .filter_map(|p| p.split_once('}'))
        .map(|(p, _)| p.split('|').next().unwrap().to_string())
        .collect()
}

/// Every stats line, listed by an exhaustive match so a new line can't be
/// left out of the checks below.
fn all_lines() -> Vec<S> {
    use S::*;
    let all = [
        Title,
        Back,
        OpenStats,
        HeatTitle,
        HeatSummary,
        HeatLess,
        HeatMore,
        HeatCell,
        HeatCellEmpty,
        WordsTitle,
        Known,
        Learning,
        New,
        NoWords,
        LangId,
        LangJv,
        WordsSummary,
        RetentionTitle,
        Week,
        Month,
        Mature,
        Young,
        AllReviews,
        NoReviewsYet,
        RetentionRule,
        TimeTitle,
        AllTime,
        ForecastTitle,
        ForecastSummary,
        ForecastEmpty,
        ForecastBar,
        Today,
        In30Days,
        CheckTitle,
        CheckIntro,
        CheckNoTest,
        CheckStart,
        CheckSeeResult,
        CheckLater,
        CheckNext,
        CheckClose,
        CheckLast,
        CheckNever,
        CheckDoNow,
        CheckNotDue,
        ResultUp,
        ResultStay,
        ResultDown,
        StayBody,
        DownBody,
        FlightInvite,
        FlightButton,
        CheckScore,
        RadarTitle,
        RadarNow,
        RadarBefore,
        RadarNotMeasured,
        RadarSummary,
        RadarFirst,
        ForecastLevelTitle,
        ForecastNoPace,
        ForecastEstimate,
        Lede,
        PeriodGroup,
        PeriodWeek,
        PeriodMonth,
        RingTitle,
        RingExplain,
        RingEmpty,
        RingGood,
        RingLow,
        RingReview,
        RingStart,
        ActivityWeek,
        ActivityWeekEmpty,
        ActivityMonth,
        ActivityUp,
        ActivityDown,
        ActivitySame,
        ActivityEmpty,
        ActivityBar,
        WeekOf,
        TileDays,
        TileReviews,
        WeakTitle,
        WeakNone,
        WeakNoData,
        WeakLine,
        WeakReading,
        WeakListening,
        WeakPlay,
        WeakVocabulary,
        WeakJourney,
        MoreDetail,
        Done,
    ];
    // Compile-time completeness: adding a line to `S` breaks this match.
    for s in all {
        match s {
            Title | Back | OpenStats | HeatTitle | HeatSummary | HeatLess | HeatMore | HeatCell
            | HeatCellEmpty | WordsTitle | Known | Learning | New | NoWords | LangId | LangJv
            | WordsSummary | RetentionTitle | Week | Month | Mature | Young | AllReviews | NoReviewsYet
            | RetentionRule | TimeTitle | AllTime | ForecastTitle | ForecastSummary | ForecastEmpty
            | ForecastBar | Today | In30Days | CheckTitle | CheckIntro | CheckNoTest | CheckStart
            | CheckSeeResult | CheckLater | CheckNext | CheckClose | CheckLast | CheckNever | CheckDoNow
            | CheckNotDue | ResultUp | ResultStay | ResultDown | StayBody | DownBody | FlightInvite
            | FlightButton | CheckScore | RadarTitle | RadarNow | RadarBefore | RadarNotMeasured
            | RadarSummary | RadarFirst | ForecastLevelTitle | ForecastNoPace | ForecastEstimate | Lede
            | PeriodGroup | PeriodWeek | PeriodMonth | RingTitle | RingExplain | RingEmpty | RingGood
            | RingLow | RingReview | RingStart | ActivityWeek | ActivityWeekEmpty | ActivityMonth
            | ActivityUp | ActivityDown | ActivitySame | ActivityEmpty | ActivityBar | WeekOf | TileDays
            | TileReviews | WeakTitle | WeakNone | WeakNoData | WeakLine | WeakReading | WeakListening
            | WeakPlay | WeakVocabulary | WeakJourney | MoreDetail | Done => {}
        }
    }
    all.to_vec()
}

#[test]
fn every_line_is_translated_with_the_same_placeholders() {
    let lines = all_lines();
    assert_eq!(lines.len(), 95);
    for s in lines {
        let en = st(UiLanguage::En, s);
        let eg = st(UiLanguage::ArEg, s);
        let ar = st(UiLanguage::Ar, s);
        for line in [en, eg, ar] {
            assert!(!line.trim().is_empty(), "{s:?} is empty");
        }
        // Arabic lines are written in Arabic, not left in English.
        let words: String = en.split('{').map(|p| p.split_once('}').map_or(p, |(_, rest)| rest)).collect();
        if words.chars().any(|c| c.is_ascii_alphabetic()) {
            assert!(eg.chars().any(|c| ('\u{600}'..='\u{6ff}').contains(&c)), "{s:?}: {eg}");
            assert!(ar.chars().any(|c| ('\u{600}'..='\u{6ff}').contains(&c)), "{s:?}: {ar}");
        }
        // The same values fill every language.
        assert_eq!(placeholders(en), placeholders(eg), "{s:?}");
        assert_eq!(placeholders(en), placeholders(ar), "{s:?}");
    }
}

#[test]
fn counts_agree_with_their_nouns_in_every_language() {
    // 1 singular, 2 dual, 3–10 plural, 11+ singular.
    let days = |lang, n: &str| stf(lang, S::In30Days, &[("n", n)]);
    assert_eq!(days(UiLanguage::En, "1"), "In 1\u{a0}day");
    assert_eq!(days(UiLanguage::En, "30"), "In 30\u{a0}days");
    assert_eq!(days(UiLanguage::ArEg, "1"), "بعد يوم");
    assert_eq!(days(UiLanguage::ArEg, "2"), "بعد يومين");
    assert_eq!(days(UiLanguage::ArEg, "7"), "بعد 7\u{a0}أيام");
    assert_eq!(days(UiLanguage::ArEg, "30"), "بعد 30\u{a0}يوم");
    assert_eq!(days(UiLanguage::Ar, "30"), "بعد 30\u{a0}يومًا");
    // No unfilled placeholder is ever shown.
    let filled = stf(
        UiLanguage::ArEg,
        S::ForecastSummary,
        &[("n", "30"), ("total", "120"), ("avg", "4"), ("day", "الأحد"), ("peak", "9")],
    );
    assert!(!filled.contains('{'), "{filled}");
}

// ------------------------------------------------------------- Stats screen

#[test]
fn a_new_learner_sees_warm_empty_states_and_ways_on() {
    let mut dom = td::mount(Setup::new(empty(), Screen::Stats, view));
    let html = td::html(&dom);
    let en = |s| st(UiLanguage::En, s);
    for s in [S::Title, S::Lede, S::RingEmpty, S::ActivityEmpty, S::NoWords, S::CheckNever, S::MoreDetail] {
        assert!(td::shows(&html, en(s)), "missing {s:?}: {html}");
    }
    // Not measured yet: the weak list says how many answers it needs.
    assert!(td::shows(&html, &stf(UiLanguage::En, S::WeakNoData, &[("n", &MIN_SKILL_ATTEMPTS.to_string())])));
    // Retention has nothing to show: dashes, never 0%.
    assert_eq!(html.matches(en(S::NoReviewsYet)).count(), 6, "{html}");
    assert!(html.contains("<span class=\"st-ring-value\"><bdi>–</bdi>"), "{html}");
    assert!(!html.contains(">0%<"), "{html}");
    // The forecast has nothing due.
    assert!(td::shows(&html, &stf(UiLanguage::En, S::ForecastEmpty, &[("n", "30")])), "{html}");
    // The week is selected by default.
    let week = td::find(&dom, "click", |l| l.attr("role") == Some("radio") && l.text.contains("7"));
    assert_eq!(week.attr("aria-checked"), Some("true"));

    // The empty activity card offers the journey.
    td::click_where(&mut dom, |l| l.class().contains("st-activity-go"));
    assert_eq!(td::screen(&dom), Screen::Journey);
}

#[test]
fn the_empty_words_and_weak_cards_lead_to_the_journey() {
    let mut dom = td::mount(Setup::new(empty(), Screen::Stats, view));
    td::click_where(&mut dom, |l| l.class().contains("st-weak-empty"));
    assert_eq!(td::screen(&dom), Screen::Journey);

    let mut dom = td::mount(Setup::new(empty(), Screen::Stats, view));
    let words = td::listeners(&dom)
        .into_iter()
        .filter(|l| {
            l.event == "click" && l.class() == "tonal" && l.text == st(UiLanguage::En, S::WeakJourney)
        })
        .collect::<Vec<_>>();
    assert_eq!(words.len(), 1, "the words card's own button");
    td::click_on(&mut dom, &words[0]);
    assert_eq!(td::screen(&dom), Screen::Journey);
}

#[test]
fn a_month_of_study_fills_every_card() {
    let mut dom = td::mount(Setup::new(demo(en(Demo::default())), Screen::Stats, view));
    let html = td::html(&dom);
    // Five days studied this week (two rest days in the demo's last seven).
    let days_tile = td::find(&dom, "click", |l| {
        l.class() == "stat tap" && l.text.contains(st(UiLanguage::En, S::TileDays))
    });
    assert!(days_tile.text.starts_with('5'), "{}", days_tile.text);
    // 19 + 15 + 11 + 31 + 23 minutes of activity in those days.
    let time_tile = td::find(&dom, "click", |l| {
        l.class() == "stat tap" && l.text.contains(st(UiLanguage::En, S::TimeTitle))
    });
    assert!(time_tile.text.starts_with("99"), "{}", time_tile.text);
    // Seven day bars, today's in the strong colour.
    assert!(html.contains("data-n=\"7\""), "{html}");
    assert!(html.contains("st-bar-col now"), "{html}");
    // Twelve cards a day on the five days studied.
    let reviews = td::find(&dom, "click", |l| {
        l.class() == "stat tap" && l.text.contains(st(UiLanguage::En, S::TileReviews))
    });
    assert!(reviews.text.starts_with("60"), "{}", reviews.text);
    // Words known and learning from the reviewed deck, as a bar and in words.
    assert!(html.contains("aria-label=\"Indonesian: 6 known, 5 learning, 0 new.\""), "{html}");
    assert!(
        html.contains("st-seg known") && html.contains("st-seg learning") && !html.contains("st-seg new")
    );
    // Last month's check: its radar, forecast and when the next one is due.
    assert!(td::shows(&html, st(UiLanguage::En, S::RadarTitle)), "{html}");
    assert!(td::shows(&html, st(UiLanguage::En, S::ForecastLevelTitle)), "{html}");
    assert!(html.contains("Reading"), "{html}");
    // The heatmap and the forecast charts are drawn.
    assert!(html.contains("st-heat") && html.contains("<rect"), "{html}");
    assert!(!td::shows(&html, st(UiLanguage::En, S::NoWords)));

    // Four weeks: four bars, and the choice is remembered on this device.
    td::click_where(&mut dom, |l| l.attr("role") == Some("radio") && l.text.contains("4"));
    let html = td::html(&dom);
    assert!(html.contains("data-n=\"4\""), "{html}");
    assert!(td::shows(&html, st(UiLanguage::En, S::ActivityMonth)), "{html}");
    let saved = td::db(&dom).0.borrow().local_value("stats_period").unwrap();
    assert_eq!(saved.as_deref(), Some(Period::Month.id()));
    // Opening Stats again starts on four weeks.
    let mut store = demo(en(Demo::default()));
    store.set_local_value("stats_period", Period::Month.id()).unwrap();
    let dom = td::mount(Setup::new(store, Screen::Stats, view));
    assert!(td::html(&dom).contains("data-n=\"4\""));
}

#[test]
fn tiles_and_card_heads_open_where_their_numbers_come_from() {
    for (pick, to) in
        [(S::TileDays, Screen::Journal), (S::TimeTitle, Screen::Journal), (S::TileReviews, Screen::Review)]
    {
        let mut dom = td::mount(Setup::new(demo(en(Demo::default())), Screen::Stats, view));
        td::click_where(&mut dom, |l| l.class() == "stat tap" && l.text.contains(st(UiLanguage::En, pick)));
        assert_eq!(td::screen(&dom), to, "{pick:?}");
    }
    for (title, to) in [
        (S::HeatTitle, Screen::Journal),
        (S::WordsTitle, Screen::Review),
        (S::RetentionTitle, Screen::Review),
        (S::ForecastTitle, Screen::Review),
        (S::ActivityWeek, Screen::Journal),
    ] {
        let mut dom = td::mount(Setup::new(demo(en(Demo::default())), Screen::Stats, view));
        td::click_where(&mut dom, |l| l.class().contains("tap-head") && l.text == st(UiLanguage::En, title));
        assert_eq!(td::screen(&dom), to, "{title:?}");
    }
    // The ring card opens Review.
    let mut dom = td::mount(Setup::new(demo(en(Demo::default())), Screen::Stats, view));
    td::click_where(&mut dom, |l| l.class().contains("st-ring-card"));
    assert_eq!(td::screen(&dom), Screen::Review);
}

#[test]
fn back_returns_to_where_stats_was_opened_from() {
    let mut dom = td::mount(Setup::new(empty(), Screen::Today, view));
    // Today's quiet link and the week preview both open Stats.
    td::click_where(&mut dom, |l| l.class() == "st-open");
    assert_eq!(td::screen(&dom), Screen::Stats);
    td::click_where(&mut dom, |l| l.class().contains("st-back-page"));
    assert_eq!(td::screen(&dom), Screen::Today);
    td::click_where(&mut dom, |l| l.class().contains("st-today-week"));
    assert_eq!(td::screen(&dom), Screen::Stats);
    // The check opened from Stats keeps Today as where Back goes.
    td::click_where(&mut dom, |l| l.tag == "button" && l.text == st(UiLanguage::En, S::CheckDoNow));
    assert_eq!(td::screen(&dom), Screen::LevelCheck);
    td::click_where(&mut dom, |l| l.attr("data-back") == Some("true"));
    assert_eq!(td::screen(&dom), Screen::Today);
}

#[test]
fn weak_skills_are_listed_and_each_opens_its_practice() {
    let mut store = empty();
    let today = td::today_now();
    // Reading 2/8 and listening 6/7 this week: only reading is weak.
    store
        .add_activity(
            ActivityKind::Practice { skill: Skill::Reading, topic: "t".into(), correct: 2, total: 8 },
            today,
            300,
        )
        .unwrap();
    store
        .add_activity(
            ActivityKind::Practice { skill: Skill::Listening, topic: "t".into(), correct: 6, total: 7 },
            today,
            300,
        )
        .unwrap();
    let mut dom = td::mount(Setup::new(store, Screen::Stats, view));
    let html = td::html(&dom);
    assert!(html.contains("25%"), "{html}");
    assert!(
        td::shows(&html, &stf(UiLanguage::En, S::WeakLine, &[("correct", "2"), ("total", "8")])),
        "{html}"
    );
    assert!(!td::shows(&html, st(UiLanguage::En, S::WeakListening)), "listening is fine: {html}");
    td::click_where(&mut dom, |l| {
        l.class() == "activity-row" && l.text.contains(st(UiLanguage::En, S::WeakReading))
    });
    assert_eq!(td::screen(&dom), Screen::Reader);

    // Both measured and fine: "nothing weak" rather than "not enough data".
    let mut store = empty();
    store
        .add_activity(
            ActivityKind::Practice { skill: Skill::Reading, topic: "t".into(), correct: 8, total: 8 },
            today,
            60,
        )
        .unwrap();
    let dom = td::mount(Setup::new(store, Screen::Stats, view));
    let html = td::html(&dom);
    assert!(td::shows(&html, st(UiLanguage::En, S::WeakNone)), "{html}");
}

#[test]
fn retention_ring_reads_good_or_low_and_says_what_to_do() {
    // Reviews of learned cards today: 9 of 10 passed is good; 5 of 10 is low.
    for (passed, verdict, action) in [(9, S::RingGood, S::RingStart), (5, S::RingLow, S::RingReview)] {
        let mut store = empty();
        let cards = seed_bridge_deck();
        store.upsert_cards(&cards).unwrap();
        let now = now_ms();
        for (i, c) in cards.iter().take(10).enumerate() {
            // Learned two days ago, then reviewed today.
            store.record_review(&c.id, Rating::Good, now - 2 * DAY_MS, 3000, ReviewMode::Classic).unwrap();
            let rating = if i < passed { Rating::Good } else { Rating::Again };
            store.record_review(&c.id, rating, now - 60_000, 3000, ReviewMode::Classic).unwrap();
        }
        let dom = td::mount(Setup::new(store, Screen::Stats, view));
        let html = td::html(&dom);
        assert!(html.contains(&format!("{}0%", passed)), "{html}");
        assert!(td::shows(&html, st(UiLanguage::En, verdict)), "{verdict:?}: {html}");
        let ring = td::find(&dom, "click", |l| l.class().contains("st-ring-card"));
        assert!(ring.text.contains(st(UiLanguage::En, action)), "{}", ring.text);
        assert!(ring.text.contains(&stf(
            UiLanguage::En,
            S::RingExplain,
            &[("total", "10"), ("passed", &passed.to_string())]
        )));
    }
}

#[test]
fn arabic_stats_use_the_ui_digits_and_read_right_to_left() {
    let dom =
        td::mount(Setup::new(demo(Demo::default()), Screen::Stats, view).lang(UiLanguage::ArEg).eastern());
    let html = td::html(&dom);
    assert!(td::shows(&html, st(UiLanguage::ArEg, S::Title)), "{html}");
    assert!(html.contains("٥"), "eastern digits: {html}");
    // The back arrow points the way Arabic reads.
    assert!(html.contains("→"), "{html}");
    // Latin-script numbers inside Arabic sit in their own direction.
    assert!(html.contains("<bdi>"), "{html}");
}

// ------------------------------------------------------------- Today cards

#[test]
fn today_previews_the_week_and_invites_the_monthly_check() {
    // A month of study and no check for 31 days: the invitation shows.
    let mut dom = td::mount(Setup::new(demo(en(Demo::default())), Screen::Today, view));
    let html = td::html(&dom);
    assert!(html.contains("st-invite"), "{html}");
    assert!(html.contains("Kak Nur") && td::shows(&html, st(UiLanguage::En, S::CheckTitle)), "{html}");
    assert!(td::shows(&html, st(UiLanguage::En, S::Today)), "today's bar is named: {html}");
    assert!(html.contains("st-today-week-summary"), "{html}");
    // "Later" hides it for a few days, remembered on this device.
    td::click(&mut dom, st(UiLanguage::En, S::CheckLater));
    assert!(!td::html(&dom).contains("st-invite"));
    let today = td::today(&dom);
    let snooze = td::db(&dom).0.borrow().local_value("level_check_snooze").unwrap();
    assert_eq!(snooze, Some(today.to_string()));

    // Snoozed two days ago: still hidden; four days ago: back.
    for (ago, shows) in [(2, false), (4, true)] {
        let mut store = demo(en(Demo::default()));
        store.set_local_value("level_check_snooze", &td::today_now().add_days(-ago).to_string()).unwrap();
        let dom = td::mount(Setup::new(store, Screen::Today, view));
        assert_eq!(td::html(&dom).contains("st-invite"), shows, "snoozed {ago} days ago");
    }

    // "Do it now" opens the check, welcomed as the monthly invitation.
    let mut dom = td::mount(Setup::new(demo(en(Demo::default())), Screen::Today, view));
    td::click_where(&mut dom, |l| l.class() == "primary" && l.text == st(UiLanguage::En, S::CheckDoNow));
    assert_eq!(td::screen(&dom), Screen::LevelCheck);
    assert!(dom.peek_app(|| *CHECK_INVITED.peek()));
}

#[test]
fn a_quiet_week_says_so_instead_of_empty_bars() {
    let dom = td::mount(Setup::new(empty(), Screen::Today, view));
    let html = td::html(&dom);
    assert!(td::shows(&html, st(UiLanguage::En, S::ActivityWeekEmpty)), "{html}");
    assert!(!html.contains("st-today-bars"));
    assert!(!html.contains("st-invite"), "nothing studied yet, nothing to check: {html}");
}

// ------------------------------------------------------------- data

#[test]
fn logs_level_tests_and_journey_position_come_from_the_store() {
    let mut store = empty();
    let today = td::today_now();
    let prev = check_result(today.add_days(-31), LevelMove::Stay, Some(0.6));
    store.record_lesson_event(LEVEL_CHECK_LESSON_ID, LEVEL_CHECK_KIND, &prev).unwrap();
    // Another kind of event on the same id is not a check.
    store.record_lesson_event(LEVEL_CHECK_LESSON_ID, "opened", &serde_json::json!({})).unwrap();
    let finished = kabar_protocol::test::TestFinished {
        score: 0.8,
        passed: true,
        duration_ms: 1000,
        skills: Vec::new(),
        missed: Vec::new(),
    };
    store.record_lesson_event("bipa1.test", "test_finished", &finished).unwrap();
    store.record_lesson_event("bipa1.test", "completed", &serde_json::json!({})).unwrap();
    let db = Db(std::rc::Rc::new(std::cell::RefCell::new(store)));

    assert_eq!(level_checks(&db), vec![prev.clone()]);
    let logs = load_logs(&db);
    assert_eq!(logs.checks, vec![prev]);
    assert!(logs.cards.is_empty() && logs.log.is_empty());

    let (scores, last_pass) = level_tests(&db, "bipa1", 0);
    assert_eq!(scores.len(), 1);
    assert!((scores[0].0 - 0.8).abs() < 1e-6);
    assert!(last_pass.is_some(), "the passed level test resets the journey");
    assert_eq!(level_tests(&db, "bipa2", 0).0, Vec::new());

    // BIPA 1's level test passed: the journey stands at BIPA 2, none of it done.
    let (level, share) = journey_position(&db).unwrap();
    assert_eq!(level, "bipa2");
    assert!(share.abs() < 1e-6);
    assert!(level_share(&db, "bipa2").abs() < 1e-6);
    assert_eq!(level_share(&db, "no-such-level"), 0.0);
    // One lesson done moves the share.
    db.0.borrow_mut().record_lesson_event("bipa2.u01.l01", "completed", &serde_json::json!({})).unwrap();
    let share = level_share(&db, "bipa2");
    assert!(share > 0.0 && share < 0.1, "{share}");
    // Placed at BIPA 3: the journey stands there.
    let placed = kabar_protocol::test::PlacementResult {
        start_level: "bipa3".into(),
        start_unit: None,
        sections: Vec::new(),
    };
    db.0.borrow_mut().record_lesson_event("placement", "placed", &placed).unwrap();
    assert_eq!(journey_position(&db).unwrap().0, "bipa3");
}

#[test]
fn the_check_is_due_monthly_from_the_first_study_day() {
    use std::collections::BTreeMap;
    let today = LocalDate::from_ymd(2026, 11, 4).unwrap();
    let mut totals = BTreeMap::new();
    assert!(!check_is_due(&totals, &[], today), "nothing studied yet");
    totals
        .insert(today.add_days(-40), kabar_core::stats::DayTotal { minutes: 10, reviews: 3, studied: true });
    assert!(check_is_due(&totals, &[], today), "a month in, never checked");
    let recent = check_result(today.add_days(-3), LevelMove::Stay, None);
    assert!(!check_is_due(&totals, &[recent], today), "checked this month");
    let old = check_result(today.add_days(-35), LevelMove::Stay, None);
    assert!(check_is_due(&totals, &[old], today));
}

#[test]
fn checks_on_the_stats_screen_show_change_and_when_the_next_is_due() {
    // Two checks: the radar compares with last month; the newest says when
    // the next is due.
    let dom =
        td::mount(Setup::new(demo(en(Demo { two_checks: true, ..Demo::default() })), Screen::Stats, view));
    let html = td::html(&dom);
    assert!(td::shows(&html, st(UiLanguage::En, S::RadarBefore)), "{html}");
    assert!(html.contains("st-delta up"), "reading rose 12 points: {html}");
    assert!(html.contains("+12"), "{html}");
    assert!(td::shows(&html, td::lead(st(UiLanguage::En, S::CheckNotDue))), "{html}");
    assert!(!td::shows(&html, st(UiLanguage::En, S::RadarFirst)));
    // Not due: the button is the quiet one.
    let button = td::find(&dom, "click", |l| l.text == st(UiLanguage::En, S::CheckDoNow));
    assert_eq!(button.class(), "");
}

#[test]
fn the_demo_can_start_before_any_check_and_placed_further_on() {
    let store = demo(Demo { first_check: true, placed: Some("bipa3".into()), ..en(Demo::default()) });
    let db = Db(std::rc::Rc::new(std::cell::RefCell::new(store)));
    assert!(level_checks(&db).is_empty(), "the first check's welcome needs no earlier check");
    assert_eq!(journey_position(&db).map(|(l, _)| l).as_deref(), Some("bipa3"));
    assert_eq!(db.0.borrow().settings().unwrap().ui_language, UiLanguage::En);
}

#[test]
fn choices_that_cannot_be_saved_still_apply_for_now() {
    // A full or locked disk: "Later" still hides the invitation and the
    // period still switches; only remembering them fails.
    let mut dom = td::mount(Setup::new(demo(en(Demo::default())), Screen::Today, view));
    td::db(&dom).0.borrow().refuse_writes();
    td::click(&mut dom, st(UiLanguage::En, S::CheckLater));
    assert!(!td::html(&dom).contains("st-invite"));
    assert_eq!(td::db(&dom).0.borrow().local_value("level_check_snooze").unwrap(), None);

    let mut dom = td::mount(Setup::new(demo(en(Demo::default())), Screen::Stats, view));
    td::db(&dom).0.borrow().refuse_writes();
    td::click_where(&mut dom, |l| l.attr("role") == Some("radio") && l.text.contains("4"));
    assert!(td::html(&dom).contains("data-n=\"4\""));
    assert_eq!(td::db(&dom).0.borrow().local_value("stats_period").unwrap(), None);
}
