//! Stats (SPEC §16) and the monthly level check (SPEC §6). The numbers come
//! from `kabar_core::stats`; this module reads the synced logs, draws the
//! charts and runs the check. Reached from the Review home, Today and the
//! check's reminder card; not in the nav bar.

pub mod charts;
pub mod check;
pub mod text;

#[cfg(test)]
mod demo;

use dioxus::prelude::*;
use kabar_core::journey::{Curriculum, Progress};
use kabar_core::locale::{Direction, UiLanguage};
use kabar_core::review::Deck;
use kabar_core::stats::level_check::{self as lc, check_due};
use kabar_core::stats::{
    self as core_stats, DayTotal, FORECAST_DAYS, HEATMAP_WEEKS, KNOWN_STABILITY_DAYS, MATURE_INTERVAL_DAYS,
    Retention, WordCounts,
};
use kabar_protocol::coach::LocalDate;
use kabar_protocol::level_check::{LEVEL_CHECK_KIND, LEVEL_CHECK_LESSON_ID, LevelCheckResult};
use kabar_protocol::srs::{ReviewEvent, StudyLanguage};
use kabar_protocol::test::TestFinished;

use self::text::{S, st};
use crate::coach::{Coach, labels};
use crate::{Db, Screen, Ui};

const STATS_CSS: &str = include_str!("../../assets/stats.css");

/// The Indonesian route, parsed once (shared with the Journey Map).
fn curriculum() -> Option<&'static Curriculum> {
    crate::journey::curriculum("id")
}

/// Where "Back" on Stats and the level check returns to.
static RETURN_TO: GlobalSignal<Screen> = Signal::global(|| Screen::Today);

/// Whether the open level check came from the monthly invitation, so its
/// welcome fits how it was reached (see `check::welcome_line`).
static CHECK_INVITED: GlobalSignal<bool> = Signal::global(|| false);

/// Opens a stats screen, remembering where the learner came from.
pub fn open(ui: Ui, screen: Screen) {
    let from = *ui.screen.peek();
    if !matches!(from, Screen::Stats | Screen::LevelCheck) {
        *RETURN_TO.write() = from;
    }
    if screen == Screen::LevelCheck {
        *CHECK_INVITED.write() = false;
    }
    let mut s = ui.screen;
    s.set(screen);
}

pub fn back(ui: Ui) {
    let mut s = ui.screen;
    s.set(*RETURN_TO.peek());
}

// ------------------------------------------------------------- formatting

pub fn rtl(ui: &Ui) -> bool {
    ui.lang().dir() == Direction::Rtl
}

/// "87%" in the UI's digits (and the Arabic percent sign with Eastern digits).
pub fn pct(ui: &Ui, share: f32) -> String {
    ui.percent(share)
}

/// Minutes, or hours once there are a couple of them.
pub fn duration(ui: &Ui, minutes: u32) -> String {
    if minutes < 120 {
        return labels::minutes(ui, minutes);
    }
    let h = (minutes as f32 / 60.0).round() as u32;
    let unit = match (ui.lang(), h) {
        (UiLanguage::En, _) => "hours",
        (_, 3..=10) => "ساعات",
        _ => "ساعة",
    };
    format!("{} {unit}", ui.num(h))
}

/// `bipa2` / `bipa-2` → `BIPA 2`, `j1` → `J1`.
pub fn level_label(level: &str) -> String {
    match level.strip_prefix("bipa") {
        Some(n) => format!("BIPA {}", n.trim_start_matches('-')),
        None => level.to_uppercase(),
    }
}

/// Wraps an inserted Latin value so it keeps its order inside Arabic text.
pub fn iso(s: &str) -> String {
    format!("\u{2068}{s}\u{2069}")
}

pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    args.iter().fold(template.to_string(), |acc, (k, v)| acc.replace(&format!("{{{k}}}"), v))
}

// ------------------------------------------------------------- data

/// Everything the Stats screen and the check read from the store.
pub struct Logs {
    pub cards: Vec<kabar_protocol::srs::Card>,
    pub log: Vec<ReviewEvent>,
    pub checks: Vec<LevelCheckResult>,
}

pub fn load_logs(db: &Db) -> Logs {
    let store = db.0.borrow();
    let warn = |what: &str, e: &dyn std::fmt::Display| crate::diag::report!("reading {what} failed: {e}");
    let cards = store.cards().unwrap_or_else(|e| {
        warn("cards", &e);
        Vec::new()
    });
    let log = store.review_log().unwrap_or_else(|e| {
        warn("the review log", &e);
        Vec::new()
    });
    Logs { cards, log, checks: level_checks(db) }
}

/// Every finished level check, oldest first (from any device).
pub fn level_checks(db: &Db) -> Vec<LevelCheckResult> {
    let events = db.0.borrow().lesson_events(LEVEL_CHECK_LESSON_ID).unwrap_or_default();
    events
        .into_iter()
        .filter(|e| e.kind == LEVEL_CHECK_KIND)
        .filter_map(|e| serde_json::from_str(&e.payload).ok())
        .collect()
}

/// Where the learner is on the Indonesian route: (level, share of it done).
pub fn journey_position(db: &Db) -> Option<(String, f32)> {
    let cur = curriculum()?;
    let store = db.0.borrow();
    let completed = store.completed_lessons().unwrap_or_default();
    let mut progress = Progress::new(cur, &completed);
    if let Ok(Some(p)) = store.placement() {
        progress = progress.start_at(cur, p.start_unit.as_deref().unwrap_or(&p.start_level));
    }
    lc::journey_level(cur, &progress)
}

/// Share of `level`'s lessons done.
pub fn level_share(db: &Db, level: &str) -> f32 {
    let Some(cur) = curriculum() else { return 0.0 };
    let Some(l) = cur.level(level) else { return 0.0 };
    let completed = db.0.borrow().completed_lessons().unwrap_or_default();
    let (done, total) = Progress::new(cur, &completed).level_counts(l);
    if total == 0 { 0.0 } else { done as f32 / total as f32 }
}

/// (score, local day) of every finished full test of `level`, and the day of
/// the last passed level test on the route or placement taken (either
/// resets where the journey stands).
pub fn level_tests(db: &Db, level: &str, offset: i16) -> (Vec<(f32, LocalDate)>, Option<LocalDate>) {
    let store = db.0.borrow();
    let day = |hlc: &kabar_protocol::Hlc| LocalDate::from_unix_ms(hlc.wall_ms, offset);
    let results = store
        .lesson_events(&format!("{level}.test"))
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.kind == "test_finished")
        .filter_map(|e| serde_json::from_str::<TestFinished>(&e.payload).ok().map(|f| (f.score, day(&e.hlc))))
        .collect();
    let last_pass = curriculum()
        .into_iter()
        .flat_map(|c| c.levels.iter().filter_map(|l| l.level_test.as_ref()))
        .flat_map(|t| {
            store.lesson_events(&t.id).unwrap_or_default().into_iter().filter(|e| e.kind == "completed")
        })
        .chain(
            store.lesson_events("placement").unwrap_or_default().into_iter().filter(|e| e.kind == "placed"),
        )
        .map(|e| day(&e.hlc))
        .max();
    (results, last_pass)
}

/// Whether the monthly check should be offered on Today.
pub fn check_is_due(
    totals: &std::collections::BTreeMap<LocalDate, DayTotal>,
    checks: &[LevelCheckResult],
    today: LocalDate,
) -> bool {
    let first = totals.iter().find(|(_, t)| core_stats::heat_level(t) > 0).map(|(d, _)| *d);
    check_due(today, first, checks.last().map(|c| c.date))
}

// ------------------------------------------------------------- entry points

/// "All stats" on the Review home.
#[component]
pub fn StatsButton() -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        button { class: "st-open", onclick: move |_| open(ui, Screen::Stats), "{st(ui.lang(), S::OpenStats)}" }
    }
}

const SNOOZE_KEY: &str = "level_check_snooze";
const SNOOZE_DAYS: i32 = 3;

/// Today: the monthly check invitation when it's due (once a month; "Later"
/// hides it for a few days on this device), and a quiet link to Stats.
#[component]
pub fn TodayCards() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let lang = ui.lang();
    let today = coach.today();
    let mut snoozed = use_signal(|| {
        db.0.borrow()
            .local_value(SNOOZE_KEY)
            .ok()
            .flatten()
            .and_then(|v| v.parse::<LocalDate>().ok())
            .is_some_and(|d| today.days_since(d) < SNOOZE_DAYS)
    });
    let due = use_memo(move || {
        let events = coach.events.read();
        let log = db.0.borrow().review_log().unwrap_or_default();
        let totals = core_stats::day_totals(&events, &log, coach.offset());
        check_is_due(&totals, &level_checks(&db), coach.today())
    });
    let name = coach.profile.read().as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let invite =
        crate::coach::say(&ui, kabar_core::coach::voice::Line::LevelCheckInvite, today, &[("name", &name)]);
    let db_later = use_context::<Db>();
    rsx! {
        style { {STATS_CSS} }
        if due() && !snoozed() {
            div { class: "card st-invite", style: "margin-block-start: 1rem",
                h3 { "{st(lang, S::CheckTitle)}" }
                crate::coach::widgets::Nur { text: invite }
                div { class: "row",
                    button {
                        class: "primary",
                        onclick: move |_| {
                            open(ui, Screen::LevelCheck);
                            *CHECK_INVITED.write() = true;
                        },
                        "{st(lang, S::CheckDoNow)}"
                    }
                    button {
                        onclick: move |_| {
                            if let Err(e) = db_later.0.borrow_mut().set_local_value(SNOOZE_KEY, &today.to_string()) {
                                crate::diag::report!("saving the snooze failed: {e}");
                            }
                            snoozed.set(true);
                        },
                        "{st(lang, S::CheckLater)}"
                    }
                }
            }
        }
        // The whole card opens Stats; the button's click bubbles up to it.
        div { class: "card card-action st-link", style: "margin-block-start: 1rem",
            onclick: move |_| open(ui, Screen::Stats),
            p { class: "muted", "{st(lang, S::OpenStatsHint)}" }
            button { "{st(lang, S::OpenStats)}" }
        }
    }
}

/// A card's title that opens where its numbers come from (the journal's
/// days, or the words in Review).
#[component]
fn CardHead(title: &'static str, to: Screen) -> Element {
    let ui = use_context::<Ui>();
    let mut screen = ui.screen;
    rsx! {
        h3 {
            button { class: "tap tap-plain tap-head", onclick: move |_| screen.set(to),
                "{title}"
                span { class: "chev", aria_hidden: "true" }
            }
        }
    }
}

// ------------------------------------------------------------- the screen

#[derive(Clone, PartialEq)]
struct Numbers {
    today: LocalDate,
    heat: core_stats::Heatmap,
    time: core_stats::TimeStudied,
    words: Vec<WordCounts>,
    week: Retention,
    month: Retention,
    forecast: Vec<u32>,
    checks: Vec<LevelCheckResult>,
    due: bool,
}

fn numbers(db: &Db, coach: &Coach) -> Numbers {
    let today = coach.today();
    let now = coach.now();
    let Logs { cards, log, checks } = load_logs(db);
    let events = coach.events.read();
    let totals = core_stats::day_totals(&events, &log, coach.offset());
    let sched = crate::schedule::scheduler(coach, &db.0.borrow());
    // One pass over the log for both windows and every card's state (#116).
    let (states, windows) = core_stats::replay_with_retention(&sched, &log, now, &[7, 30]);
    let (week, month) = (windows[0], windows[1]);
    let deck = Deck::with_states(cards, log, states);
    let words = core_stats::word_counts(deck.cards(), |id| deck.state(id));
    let forecast = deck.forecast(&sched, now, FORECAST_DAYS);
    let due = check_is_due(&totals, &checks, today);
    Numbers {
        today,
        heat: core_stats::heatmap(&totals, today, HEATMAP_WEEKS),
        time: core_stats::time_studied(&totals, today),
        words,
        week,
        month,
        forecast,
        checks,
        due,
    }
}

#[component]
pub fn StatsScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let lang = ui.lang();
    // Recomputed when the coach's events, clock or offset change, or the
    // review settings do (a new FSRS fit), not on every render.
    let n = use_memo(move || {
        let _ = (ui.settings.read().desired_retention, ui.settings.read().fsrs.clone());
        numbers(&db, &coach)
    });
    let n = n.read();
    rsx! {
        style { {STATS_CSS} }
        div { class: "st",
            div { class: "st-head",
                button { class: "st-back", onclick: move |_| back(ui), "{st(lang, S::Back)}" }
                h2 { "{st(lang, S::Title)}" }
            }
            div { class: "st-grid",
                HeatCard { heat: n.heat.clone(), today: n.today }
                TimeCard { time: n.time }
                WordsCard { words: n.words.clone() }
                RetentionCard { week: n.week, month: n.month }
                ForecastCard { days: n.forecast.clone(), today: n.today }
                CheckCard { checks: n.checks.clone(), due: n.due, today: n.today }
            }
        }
    }
}

#[component]
fn HeatCard(heat: core_stats::Heatmap, today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let summary = fill(
        st(lang, S::HeatSummary),
        &[
            ("weeks", &ui.num(HEATMAP_WEEKS)),
            ("days", &ui.num(heat.active_days)),
            ("minutes", &duration(&ui, heat.minutes)),
        ],
    );
    let svg = charts::heatmap(
        &heat,
        rtl(&ui),
        st(lang, S::HeatTitle),
        |d| {
            let date = labels::date(&ui, d.date, today);
            if d.level == 0 {
                fill(st(lang, S::HeatCellEmpty), &[("date", &date)])
            } else {
                fill(
                    st(lang, S::HeatCell),
                    &[
                        ("date", &date),
                        ("minutes", &labels::minutes(&ui, d.minutes)),
                        ("reviews", &ui.num(d.reviews)),
                    ],
                )
            }
        },
        |m| labels::month_short(&ui, m.ymd().1).to_string(),
    );
    rsx! {
        section { class: "card st-card st-wide",
            CardHead { title: st(lang, S::HeatTitle), to: Screen::Journal }
            div { class: "st-chart st-heat", dangerous_inner_html: svg }
            div { class: "st-legend", aria_hidden: "true",
                span { "{st(lang, S::HeatLess)}" }
                span { class: "st-swatches", dangerous_inner_html: charts::heat_legend(rtl(&ui)) }
                span { "{st(lang, S::HeatMore)}" }
            }
            p { class: "st-summary", "{summary}" }
        }
    }
}

#[component]
fn TimeCard(time: core_stats::TimeStudied) -> Element {
    let ui = use_context::<Ui>();
    let mut screen = ui.screen;
    let lang = ui.lang();
    let span = |s: core_stats::TimeSpan| {
        (duration(&ui, s.minutes), fill(st(lang, S::StudyDays), &[("n", &ui.num(s.days))]))
    };
    let (wm, wd) = span(time.week);
    let (mm, md) = span(time.month);
    rsx! {
        section { class: "card st-card",
            CardHead { title: st(lang, S::TimeTitle), to: Screen::Journal }
            // Each span opens the journal, day by day.
            div { class: "stats-row",
                button { class: "stat tap", onclick: move |_| screen.set(Screen::Journal),
                    span { class: "stat-value", "{wm}" }
                    span { class: "stat-label", {fill(st(lang, S::Week), &[("n", &ui.num(7u32))])} }
                    span { class: "stat-label", "{wd}" }
                }
                button { class: "stat tap", onclick: move |_| screen.set(Screen::Journal),
                    span { class: "stat-value", "{mm}" }
                    span { class: "stat-label", {fill(st(lang, S::Month), &[("n", &ui.num(30u32))])} }
                    span { class: "stat-label", "{md}" }
                }
                button { class: "stat tap", onclick: move |_| screen.set(Screen::Journal),
                    span { class: "stat-value", "{duration(&ui, time.total_minutes)}" }
                    span { class: "stat-label", "{st(lang, S::AllTime)}" }
                }
            }
        }
    }
}

fn language_name(lang: UiLanguage, l: StudyLanguage) -> &'static str {
    match l {
        StudyLanguage::Id => st(lang, S::LangId),
        StudyLanguage::Jv => st(lang, S::LangJv),
    }
}

#[component]
fn WordsCard(words: Vec<WordCounts>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let _ = KNOWN_STABILITY_DAYS; // the rule the note below explains
    rsx! {
        section { class: "card st-card",
            CardHead { title: st(lang, S::WordsTitle), to: Screen::Review }
            if words.is_empty() {
                p { class: "muted", "{st(lang, S::NoWords)}" }
            }
            for w in words.iter().copied() {
                {
                    let total = w.total().max(1) as f32;
                    let summary = fill(
                        st(lang, S::WordsSummary),
                        &[
                            ("lang", language_name(lang, w.language)),
                            ("known", &ui.num(w.known)),
                            ("learning", &ui.num(w.learning)),
                            ("new", &ui.num(w.new)),
                        ],
                    );
                    rsx! {
                        div { key: "{w.language:?}", class: "st-words",
                            div { class: "st-words-head",
                                strong { "{language_name(lang, w.language)}" }
                                span { class: "st-big", "{ui.num(w.known)}" }
                            }
                            div { class: "st-stack", role: "img", aria_label: "{summary}",
                                for (class, n) in [("known", w.known), ("learning", w.learning), ("new", w.new)] {
                                    if n > 0 {
                                        span { key: "{class}", class: "st-seg {class}", style: "flex-grow: {n as f32 / total}", title: "{n}" }
                                    }
                                }
                            }
                            ul { class: "st-keys",
                                li { span { class: "st-key known", aria_hidden: "true" } "{st(lang, S::Known)} " bdi { "{ui.num(w.known)}" } }
                                li { span { class: "st-key learning", aria_hidden: "true" } "{st(lang, S::Learning)} " bdi { "{ui.num(w.learning)}" } }
                                li { span { class: "st-key new", aria_hidden: "true" } "{st(lang, S::New)} " bdi { "{ui.num(w.new)}" } }
                            }
                        }
                    }
                }
            }
            p { class: "muted small", "{st(lang, S::KnownRule)}" }
        }
    }
}

#[component]
fn RetentionCard(week: Retention, month: Retention) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let cell = |t: core_stats::Tally| match t.share() {
        Some(s) => (pct(&ui, s), format!("{}/{}", ui.num(t.passed), ui.num(t.total))),
        None => ("–".to_string(), st(lang, S::NoReviewsYet).to_string()),
    };
    let rows = [
        (st(lang, S::Mature), cell(week.mature), cell(month.mature)),
        (st(lang, S::Young), cell(week.young), cell(month.young)),
        (st(lang, S::AllReviews), cell(week.all()), cell(month.all())),
    ];
    let rule = fill(
        st(lang, S::RetentionRule),
        &[("days", &ui.num(MATURE_INTERVAL_DAYS as u32)), ("pct", &pct(&ui, 0.9))],
    );
    rsx! {
        section { class: "card st-card",
            CardHead { title: st(lang, S::RetentionTitle), to: Screen::Review }
            table { class: "st-table",
                thead {
                    tr {
                        th { scope: "col" }
                        th { scope: "col", {fill(st(lang, S::Week), &[("n", &ui.num(7u32))])} }
                        th { scope: "col", {fill(st(lang, S::Month), &[("n", &ui.num(30u32))])} }
                    }
                }
                tbody {
                    for (label, w, m) in rows {
                        tr { key: "{label}",
                            th { scope: "row", "{label}" }
                            td { span { class: "st-big", bdi { "{w.0}" } } span { class: "muted small", bdi { "{w.1}" } } }
                            td { span { class: "st-big", bdi { "{m.0}" } } span { class: "muted small", bdi { "{m.1}" } } }
                        }
                    }
                }
            }
            p { class: "muted small", "{rule}" }
        }
    }
}

#[component]
fn ForecastCard(days: Vec<u32>, today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let s = core_stats::forecast_summary(&days);
    let day_name = |i: usize| {
        if i == 0 {
            st(lang, S::Today).to_string()
        } else {
            labels::date(&ui, today.add_days(i as i32), today)
        }
    };
    let n_days = ui.num(days.len() as u64);
    let summary = if s.total == 0 {
        fill(st(lang, S::ForecastEmpty), &[("n", &n_days)])
    } else {
        fill(
            st(lang, S::ForecastSummary),
            &[
                ("n", &n_days),
                ("total", &ui.num(s.total)),
                ("avg", &ui.num(s.per_day)),
                ("day", &day_name(s.peak_day)),
                ("peak", &ui.num(s.peak)),
            ],
        )
    };
    let svg = charts::forecast(&days, rtl(&ui), st(lang, S::ForecastTitle), |i, n| {
        fill(st(lang, S::ForecastBar), &[("day", &day_name(i)), ("n", &ui.num(n))])
    });
    rsx! {
        section { class: "card st-card st-wide",
            CardHead { title: st(lang, S::ForecastTitle), to: Screen::Review }
            div { class: "st-chart st-forecast", dangerous_inner_html: svg }
            div { class: "st-axis", aria_hidden: "true",
                span { "{st(lang, S::Today)}" }
                span { {fill(st(lang, S::In30Days), &[("n", &n_days)])} }
            }
            p { class: "st-summary", "{summary}" }
        }
    }
}

/// The last check's radar and forecast, and the way to the next check.
#[component]
fn CheckCard(checks: Vec<LevelCheckResult>, due: bool, today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let last = checks.last().cloned();
    let before = checks.len().checked_sub(2).and_then(|i| checks.get(i)).map(|c| c.radar.clone());
    rsx! {
        section { class: "card st-card st-wide",
            h3 { "{st(lang, S::CheckTitle)}" }
            match &last {
                None => rsx! { p { class: "muted", "{st(lang, S::CheckNever)}" } },
                Some(c) => rsx! {
                    p { class: "muted", {fill(st(lang, S::CheckLast), &[("date", &labels::date(&ui, c.date, today))])} }
                    check::RadarView { now: c.radar.clone(), before }
                    if let Some(eta) = &c.eta {
                        check::EtaLine { level: eta.level.clone(), date: eta.date, today }
                    }
                    if !due {
                        p { class: "muted small",
                            {fill(st(lang, S::CheckNotDue), &[("date", &labels::date(&ui, c.date.add_days(lc::CHECK_EVERY_DAYS), today))])}
                        }
                    }
                },
            }
            button {
                class: if due { "primary" } else { "" },
                onclick: move |_| open(ui, Screen::LevelCheck),
                "{st(lang, S::CheckDoNow)}"
            }
        }
    }
}
