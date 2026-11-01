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
use kabar_core::coach::report::MIN_SKILL_ATTEMPTS;
use kabar_core::journey::{Curriculum, Progress};
use kabar_core::locale::{Direction, UiLanguage};
use kabar_core::review::Deck;
use kabar_core::stats::level_check::{self as lc, check_due};
use kabar_core::stats::overview::{self, Overview, Period, WeakSpot};
use kabar_core::stats::{
    self as core_stats, DayTotal, FORECAST_DAYS, HEATMAP_WEEKS, KNOWN_STABILITY_DAYS, MATURE_INTERVAL_DAYS,
    Retention, Tally, WordCounts,
};
use kabar_protocol::coach::{LocalDate, Skill};
use kabar_protocol::level_check::{LEVEL_CHECK_KIND, LEVEL_CHECK_LESSON_ID, LevelCheckResult};
use kabar_protocol::srs::{ReviewEvent, StudyLanguage};
use kabar_protocol::test::TestFinished;

use self::text::{S, st};
use crate::coach::{Coach, labels};
use crate::embedded::{Css, css};
use crate::{Db, Screen, Ui};

const STATS_CSS: Css = css!("assets/stats.css");

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
    // Reuse the same synced-log totals for both the monthly check and
    // Today's seven-day preview; never count a second, synthetic streak.
    let snapshot = use_memo(move || {
        let events = coach.events.read();
        let log = db.0.borrow().review_log().unwrap_or_default();
        let totals = core_stats::day_totals(&events, &log, coach.offset());
        let today = coach.today();
        (check_is_due(&totals, &level_checks(&db), today), overview::overview(&totals, today, Period::Week))
    });
    let (due, week) = snapshot.read().clone();
    let peak = week.buckets.iter().map(|b| b.minutes).max().unwrap_or(0).max(1);
    let bars: Vec<(String, String, f32, bool)> = week
        .buckets
        .iter()
        .map(|b| {
            let day = if b.start == today {
                st(lang, S::Today).to_string()
            } else {
                ui.t(labels::weekday(b.start.weekday())).to_string()
            };
            let detail = fill(
                st(lang, S::ActivityBar),
                &[
                    ("label", &day),
                    ("minutes", &labels::minutes(&ui, b.minutes)),
                    ("reviews", &ui.num(b.reviews)),
                ],
            );
            let height = if b.minutes == 0 { 0.0 } else { (b.minutes as f32 / peak as f32 * 100.0).max(4.0) };
            (day, detail, height, b.start == today)
        })
        .collect();
    let daily_summary = bars.iter().map(|b| b.1.as_str()).collect::<Vec<_>>().join(" · ");
    let week_label = fill(st(lang, S::PeriodWeek), &[("n", &ui.num(Period::Week.days() as u64))]);
    let activity_summary = format!(
        "{}: {} · {}: {}",
        st(lang, S::TileDays),
        ui.num(week.active_days),
        st(lang, S::TimeTitle),
        labels::minutes(&ui, week.minutes),
    );
    let name = coach.profile.read().as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let invite =
        crate::coach::say(&ui, kabar_core::coach::voice::Line::LevelCheckInvite, today, &[("name", &name)]);
    let db_later = use_context::<Db>();
    rsx! {
        style { {STATS_CSS.text()} }
        if due && !snoozed() {
            crate::ui::Card { element: crate::ui::CardElement::Div, class: "st-invite", style: "margin-block-start: 1rem",
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
        // The existing Stats shortcut now shows a lightweight, real-data
        // preview. The full chart and period controls still live in Stats.
        crate::ui::Card {
            element: crate::ui::CardElement::Div,
            class: "card-action st-today-week",
            style: "margin-block-start: 1rem",
            onclick: move |_| open(ui, Screen::Stats),
            div { class: "st-today-week-head",
                h3 { "{st(lang, S::ActivityWeek)}" }
                span { class: "muted small", "{week_label}" }
            }
            div { class: "st-bars st-today-bars", role: "img", aria_label: "{daily_summary}",
                for (i, (name, detail, height, is_today)) in bars.into_iter().enumerate() {
                    div {
                        key: "{i}",
                        class: if is_today { "st-bar-col now" } else { "st-bar-col" },
                        title: "{detail}",
                        div { class: "st-bar-track",
                            span { class: "st-bar", style: "block-size: {height}%" }
                        }
                        span { class: "st-bar-label", "{name}" }
                    }
                }
            }
            p { class: "st-today-week-summary", "{activity_summary}" }
            button { class: "tonal", "{st(lang, S::OpenStats)}" }
        }
    }
}

/// The back arrow, pointing the way the page reads.
pub(crate) fn back_arrow(lang: UiLanguage) -> &'static str {
    if lang.dir() == Direction::Rtl { "→" } else { "←" }
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

/// One period's headline numbers (the top of the screen).
#[derive(Clone, PartialEq)]
struct PeriodNumbers {
    overview: Overview,
    /// True retention of every learned-card review in the period.
    retention: Tally,
    weak: Vec<WeakSpot>,
    /// Whether any skill had enough answers to judge.
    measured: bool,
}

#[derive(Clone, PartialEq)]
struct Numbers {
    today: LocalDate,
    periods: Vec<(Period, PeriodNumbers)>,
    heat: core_stats::Heatmap,
    total_minutes: u32,
    words: Vec<WordCounts>,
    week: Retention,
    month: Retention,
    forecast: Vec<u32>,
    checks: Vec<LevelCheckResult>,
    due: bool,
}

impl Numbers {
    fn period(&self, p: Period) -> &PeriodNumbers {
        &self.periods.iter().find(|(q, _)| *q == p).unwrap_or(&self.periods[0]).1
    }
}

fn numbers(db: &Db, coach: &Coach) -> Numbers {
    let today = coach.today();
    let now = coach.now();
    let Logs { cards, log, checks } = load_logs(db);
    let events = coach.events.read();
    let totals = core_stats::day_totals(&events, &log, coach.offset());
    let sched = crate::schedule::scheduler(coach, &db.0.borrow());
    // One pass over the log for every window and every card's state (#116):
    // the detail table's 7 and 30 days, then each period's own span.
    let mut spans = vec![7, 30];
    spans.extend(Period::ALL.iter().map(|p| p.days() as u64));
    let (states, windows) = core_stats::replay_with_retention(&sched, &log, now, &spans);
    let (week, month) = (windows[0], windows[1]);
    let periods = Period::ALL
        .iter()
        .zip(&windows[2..])
        .map(|(&p, r)| {
            let skills = lc::recent_skills(&events, today, p.days());
            let numbers = PeriodNumbers {
                overview: overview::overview(&totals, today, p),
                retention: r.all(),
                weak: overview::weak_spots(&skills),
                measured: skills.iter().any(|&(_, total)| total >= MIN_SKILL_ATTEMPTS),
            };
            (p, numbers)
        })
        .collect();
    let deck = Deck::with_states(cards, log, states);
    let words = core_stats::word_counts(deck.cards(), |id| deck.state(id));
    let forecast = deck.forecast(&sched, now, FORECAST_DAYS);
    let due = check_is_due(&totals, &checks, today);
    Numbers {
        today,
        periods,
        heat: core_stats::heatmap(&totals, today, HEATMAP_WEEKS),
        total_minutes: totals.values().map(|t| t.minutes).sum(),
        words,
        week,
        month,
        forecast,
        checks,
        due,
    }
}

/// The period the learner last picked on this device.
const PERIOD_KEY: &str = "stats_period";

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
    let db_period = use_context::<Db>();
    let period = use_signal(move || {
        db_period
            .0
            .borrow()
            .local_value(PERIOD_KEY)
            .ok()
            .flatten()
            .and_then(|v| Period::from_id(&v))
            .unwrap_or_default()
    });
    let n = n.read();
    rsx! {
        style { {STATS_CSS.text()} }
        div { class: "st",
            div { class: "page-head st-head",
                div {
                    h1 { "{st(lang, S::Title)}" }
                    p { class: "sub", "{st(lang, S::Lede)}" }
                }
                // The header's round arrow (#442, #448); a phone's top bar
                // already has one, so it shows from the rail up.
                button {
                    class: "icon-button st-back st-back-page round-back",
                    aria_label: st(lang, S::Back),
                    title: st(lang, S::Back),
                    onclick: move |_| back(ui),
                    span { aria_hidden: "true", {back_arrow(lang)} }
                }
            }
            PeriodSwitch { period }
            PeriodView { p: n.period(period()).clone(), today: n.today, total_minutes: n.total_minutes }
            div { class: "section-head", h2 { "{st(lang, S::MoreDetail)}" } }
            div { class: "st-grid",
                HeatCard { heat: n.heat.clone(), today: n.today }
                WordsCard { words: n.words.clone() }
                RetentionCard { week: n.week, month: n.month }
                ForecastCard { days: n.forecast.clone(), today: n.today }
                CheckCard { checks: n.checks.clone(), due: n.due, today: n.today }
            }
        }
    }
}

fn period_name(ui: &Ui, p: Period) -> String {
    let (s, n) = match p {
        Period::Week => (S::PeriodWeek, p.days() as u32),
        Period::Month => (S::PeriodMonth, (p.days() / 7) as u32),
    };
    fill(st(ui.lang(), s), &[("n", &ui.num(n))])
}

/// Week / four weeks. Changes every number above the detail cards, and is
/// remembered on this device.
#[component]
fn PeriodSwitch(period: Signal<Period>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    rsx! {
        div { class: "segmented st-period", role: "radiogroup", aria_label: st(ui.lang(), S::PeriodGroup),
            for p in Period::ALL {
                button {
                    key: "{p.id()}",
                    role: "radio",
                    aria_checked: if period() == p { "true" } else { "false" },
                    onclick: {
                        let db = db.clone();
                        move |_| {
                        period.set(p);
                        if let Err(e) = db.0.borrow_mut().set_local_value(PERIOD_KEY, p.id()) {
                            crate::diag::report!("saving the stats period failed: {e}");
                        }
                    }},
                    "{period_name(&ui, p)}"
                }
            }
        }
    }
}

/// Where a skill is practised, and the button that says so.
fn skill_action(skill: Skill) -> (Screen, S) {
    match skill {
        Skill::Reading => (Screen::Reader, S::WeakReading),
        Skill::Listening => (Screen::Listen, S::WeakListening),
        Skill::Vocabulary => (Screen::Review, S::WeakVocabulary),
        Skill::Writing | Skill::Speaking => (Screen::Play, S::WeakPlay),
    }
}

#[component]
fn PeriodView(p: PeriodNumbers, today: LocalDate, total_minutes: u32) -> Element {
    let ui = use_context::<Ui>();
    let mut screen = ui.screen;
    let lang = ui.lang();
    let o = &p.overview;
    rsx! {
        div { class: "st-top",
            RingCard { tally: p.retention }
            ActivityCard { overview: o.clone(), today, total_minutes }
            div { class: "stats-row st-tiles",
                button { class: "stat tap", onclick: move |_| screen.set(Screen::Journal),
                    span { class: "stat-value", bdi { "{ui.num(o.active_days)}" } }
                    span { class: "stat-label", "{st(lang, S::TileDays)}" }
                }
                button { class: "stat tap", onclick: move |_| screen.set(Screen::Journal),
                    span { class: "stat-value", "{duration(&ui, o.minutes)}" }
                    span { class: "stat-label", "{st(lang, S::TimeTitle)}" }
                }
                button { class: "stat tap", onclick: move |_| screen.set(Screen::Review),
                    span { class: "stat-value", bdi { "{ui.num(o.reviews)}" } }
                    span { class: "stat-label", "{st(lang, S::TileReviews)}" }
                }
            }
            WeakCard { weak: p.weak.clone(), measured: p.measured }
        }
    }
}

/// True retention for the period as a ring, what it means, and one action.
/// The whole card opens Review; its button's click bubbles up to it.
#[component]
fn RingCard(tally: Tally) -> Element {
    let ui = use_context::<Ui>();
    let mut screen = ui.screen;
    let lang = ui.lang();
    let share = tally.share();
    let (line, action) = match share {
        None => (st(lang, S::RingEmpty).to_string(), S::RingStart),
        Some(s) => (
            fill(
                st(lang, S::RingExplain),
                &[("total", &ui.num(tally.total)), ("passed", &ui.num(tally.passed))],
            ),
            if s >= 0.85 { S::RingStart } else { S::RingReview },
        ),
    };
    let verdict = share.map(|s| st(lang, if s >= 0.85 { S::RingGood } else { S::RingLow }));
    let value = share.map_or("–".to_string(), |s| pct(&ui, s));
    let svg = charts::ring(share.unwrap_or(0.0), rtl(&ui), &format!("{}: {value}", st(lang, S::RingTitle)));
    rsx! {
        crate::ui::Card { class: "card-action st-card st-ring-card", onclick: move |_| screen.set(Screen::Review),
            div { class: "st-ring", aria_hidden: "true",
                div { class: "st-ring-svg", dangerous_inner_html: svg }
                span { class: "st-ring-value", bdi { "{value}" } }
            }
            div { class: "st-ring-text",
                h3 { "{st(lang, S::RingTitle)}" }
                p { class: "muted", "{line}" }
                if let Some(v) = verdict {
                    p { class: "st-verdict", "{v}" }
                }
                button { class: "tonal st-ring-go", "{st(lang, action)}" }
            }
        }
    }
}

/// Minutes per bar (a day, or a week on the four-week view), today's bar
/// in the strong colour, and how the period compares with the one before.
#[component]
fn ActivityCard(overview: Overview, today: LocalDate, total_minutes: u32) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let o = overview;
    let peak = o.buckets.iter().map(|b| b.minutes).max().unwrap_or(0).max(1);
    let label = |b: &overview::Bucket| -> String {
        if b.days == 1 {
            if b.start == today {
                st(lang, S::Today).to_string()
            } else {
                ui.t(labels::weekday(b.start.weekday())).to_string()
            }
        } else {
            fill(st(lang, S::WeekOf), &[("date", &labels::date(&ui, b.start, today))])
        }
    };
    let title = st(lang, if o.period == Period::Week { S::ActivityWeek } else { S::ActivityMonth });
    let diff = o.minutes.abs_diff(o.previous_minutes);
    let compare = if o.minutes == 0 {
        st(lang, S::ActivityEmpty).to_string()
    } else if o.minutes > o.previous_minutes {
        fill(st(lang, S::ActivityUp), &[("n", &duration(&ui, diff))])
    } else if o.minutes < o.previous_minutes {
        fill(st(lang, S::ActivityDown), &[("n", &duration(&ui, diff))])
    } else {
        st(lang, S::ActivitySame).to_string()
    };
    let bars: Vec<(String, String, f32, bool)> = o
        .buckets
        .iter()
        .map(|b| {
            let name = label(b);
            let tip = fill(
                st(lang, S::ActivityBar),
                &[
                    ("label", &name),
                    ("minutes", &labels::minutes(&ui, b.minutes)),
                    ("reviews", &ui.num(b.reviews)),
                ],
            );
            let h = if b.minutes == 0 { 0.0 } else { (b.minutes as f32 / peak as f32 * 100.0).max(4.0) };
            let last = b.start.add_days(b.days - 1) == today;
            (name, tip, h, last)
        })
        .collect();
    let summary = bars.iter().map(|b| b.1.as_str()).collect::<Vec<_>>().join(" · ");
    let n_bars = bars.len();
    rsx! {
        crate::ui::Card { class: "st-card st-activity",
            CardHead { title, to: Screen::Journal }
            div { class: "st-bars", role: "img", aria_label: "{summary}", "data-n": "{n_bars}",
                for (i, (name, tip, h, now)) in bars.into_iter().enumerate() {
                    div { key: "{i}", class: if now { "st-bar-col now" } else { "st-bar-col" }, title: "{tip}",
                        div { class: "st-bar-track",
                            span { class: "st-bar", style: "block-size: {h}%" }
                        }
                        span { class: "st-bar-label", "{name}" }
                    }
                }
            }
            p { class: "st-summary", "{compare}" }
            // An empty period says one short lesson is enough; give the way to it.
            if o.minutes == 0 {
                button {
                    class: "tonal st-activity-go",
                    onclick: move |_| {
                        let mut screen = ui.screen;
                        screen.set(Screen::Journey);
                    },
                    "{st(lang, S::WeakJourney)}"
                }
            }
            p { class: "muted small", "{st(lang, S::AllTime)}: " bdi { "{duration(&ui, total_minutes)}" } }
        }
    }
}

/// Skills under the bar in the period, each a row that opens the place to
/// practise it; or, with none, what fills this list and the way on.
#[component]
fn WeakCard(weak: Vec<WeakSpot>, measured: bool) -> Element {
    let ui = use_context::<Ui>();
    let mut screen = ui.screen;
    let lang = ui.lang();
    rsx! {
        section { class: "st-weak",
            div { class: "section-head", h2 { "{st(lang, S::WeakTitle)}" } }
            if weak.is_empty() {
                // The whole card goes on to the journey; the button's click bubbles up.
                crate::ui::Card { element: crate::ui::CardElement::Div,
                    class: "card-action empty-state st-weak-empty",
                    onclick: move |_| screen.set(Screen::Journey),
                    span { class: "row-icon", aria_hidden: "true", {crate::icons::nav(Screen::Journey)} }
                    p {
                        if measured {
                            "{st(lang, S::WeakNone)}"
                        } else {
                            {fill(st(lang, S::WeakNoData), &[("n", &ui.num(MIN_SKILL_ATTEMPTS))])}
                        }
                    }
                    button { class: "tonal", "{st(lang, S::WeakJourney)}" }
                }
            } else {
                crate::ui::DivCard { class: "grouped",
                    for w in weak {
                        {
                            let (to, action) = skill_action(w.skill);
                            let line = fill(
                                st(lang, S::WeakLine),
                                &[("correct", &ui.num(w.correct)), ("total", &ui.num(w.total))],
                            );
                            rsx! {
                                button { key: "{w.skill:?}", class: "activity-row", onclick: move |_| screen.set(to),
                                    span { class: "row-icon", aria_hidden: "true", {crate::icons::nav(to)} }
                                    span { class: "row-copy",
                                        strong { "{ui.t(labels::skill(w.skill))}" }
                                        small { bdi { "{line}" } " · " bdi { "{pct(&ui, w.share())}" } }
                                    }
                                    span { class: "row-end",
                                        span { class: "st-weak-go", "{st(lang, action)}" }
                                        span { class: "chev", aria_hidden: "true" }
                                    }
                                }
                            }
                        }
                    }
                }
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
        crate::ui::Card { class: "st-card st-wide",
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
        crate::ui::Card { class: "st-card",
            CardHead { title: st(lang, S::WordsTitle), to: Screen::Review }
            if words.is_empty() {
                div { class: "empty-state st-words-empty",
                    span { class: "row-icon", aria_hidden: "true", {crate::icons::nav(Screen::Journey)} }
                    p { "{st(lang, S::NoWords)}" }
                    button {
                        class: "tonal",
                        onclick: move |_| {
                            let mut screen = ui.screen;
                            screen.set(Screen::Journey);
                        },
                        "{st(lang, S::WeakJourney)}"
                    }
                }
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
        crate::ui::Card { class: "st-card",
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
        crate::ui::Card { class: "st-card st-wide",
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
        crate::ui::Card { class: "st-card st-wide",
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
