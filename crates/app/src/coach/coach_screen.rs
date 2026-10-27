//! Kak Nur's page: the weekly report with Liga Pribadi, achievements, the
//! Warung and the learning journal.

use dioxus::prelude::*;
use kabar_core::coach::voice::Line;
use kabar_core::coach::{report, rewards};
use kabar_protocol::coach::{ActivityEvent, ActivityKind, CoachProfile, LocalDate, WeeklyReport};

use super::widgets::{Nur, Stat};
use super::{Coach, labels, logic, say};
use crate::i18n::Msg;
use crate::portraits::Mood;
use crate::{Db, Screen, Ui, icons, theme};

/// How many past weeks Liga Pribadi compares against.
const LEAGUE_WEEKS: u32 = 8;

fn report_for_week(events: &[ActivityEvent], week: LocalDate, today: LocalDate) -> WeeklyReport {
    let streak = rewards::pelita(events, week.add_days(6).min(today)).streak;
    report::weekly_report(events, week, streak, rewards::league(events, week, LEAGUE_WEEKS))
}

/// Writes the weekly report once it's due (Kabar Cloud does the same from
/// its cron; the row is LWW, so both writing it is harmless).
fn make_due_report(db: &Db, coach: Coach, profile: &CoachProfile) {
    let latest = db.0.borrow().latest_report_week().ok().flatten();
    let Some(week) = report::report_due(profile, coach.now(), latest) else { return };
    let events = coach.events.peek().clone();
    let r = report_for_week(&events, week, coach.today());
    if let Err(e) = db.0.borrow_mut().save_report(&r) {
        eprintln!("kabar: saving the weekly report failed: {e}");
    }
}

#[component]
pub fn CoachScreen() -> Element {
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let profile = coach.profile.read().clone();
    use_hook(|| {
        if let Some(p) = &profile {
            make_due_report(&db, coach, p);
        }
    });
    let Some(profile) = profile else {
        return rsx! {};
    };
    let today = coach.today();
    rsx! {
        div { class: "coach-grid",
            div {
                ReportCard { profile, today }
                Achievements { today }
            }
            div {
                Warung { today }
                JournalLink {}
            }
        }
    }
}

#[component]
fn ReportCard(profile: CoachProfile, today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let mut show_saved = use_signal(|| true);
    let events = coach.events.read().clone();
    let this_week = today.week_start();
    let saved =
        db.0.borrow()
            .latest_report_week()
            .ok()
            .flatten()
            .filter(|w| *w != this_week)
            .and_then(|w| db.0.borrow().report_for(w).ok().flatten());
    let (r, so_far) = match (&saved, show_saved()) {
        (Some(s), true) => (s.clone(), false),
        _ => (report_for_week(&events, this_week, today), true),
    };
    let note = logic::report_note(&r, &events);
    // Voice placeholders: numbers in the UI's digits, skill names localized.
    // A great week names the strongest skill; a steady one next week's focus.
    let (minutes, days, words) = (ui.num(r.minutes), ui.num(r.days_studied), ui.num(r.new_words));
    let skill = match note {
        Line::ReportGreatWeek => r.strongest,
        _ => r.next_focus.first().map(|f| f.skill).or(r.weakest),
    }
    .map(|s| ui.t(labels::skill(s)))
    .unwrap_or_default();
    let args = [
        ("name", profile.name.as_str()),
        ("minutes", minutes.as_str()),
        ("days", days.as_str()),
        ("words", words.as_str()),
        ("skill", skill),
    ];
    let retention =
        r.retention.map_or("–".to_string(), |x| format!("{}%", ui.num((x * 100.0).round() as u32)));

    rsx! {
        div { class: "card",
            h2 { "{ui.t(Msg::ReportTitle)}" }
            if saved.is_some() {
                div { class: "row", role: "group",
                    button {
                        aria_pressed: if !so_far { "true" } else { "false" },
                        onclick: move |_| show_saved.set(true),
                        "{ui.t(Msg::ReportLast)}"
                    }
                    button {
                        aria_pressed: if so_far { "true" } else { "false" },
                        onclick: move |_| show_saved.set(false),
                        "{ui.t(Msg::ReportThisWeek)}"
                    }
                }
            }
            p { class: "muted small",
                "{ui.t(Msg::WeekOf)} {labels::date(&ui, r.week_start, today)}"
                if so_far { " · {ui.t(Msg::SoFar)}" }
            }
            Nur { mood: Mood::Explaining, text: say(&ui, note, r.week_start, &args) }
            div { class: "stats-row",
                Stat { value: labels::minutes(&ui, r.minutes), label: Msg::StatTime }
                Stat { value: ui.num(r.days_studied), label: Msg::StatDays }
                Stat { value: ui.num(r.new_words), label: Msg::StatNewWords }
                Stat { value: retention, label: Msg::StatRetention }
                Stat { value: ui.num(r.streak), label: Msg::StatPelita }
            }
            dl { class: "summary",
                if let Some(s) = r.strongest {
                    dt { "{ui.t(Msg::Strongest)}" }
                    dd { "{ui.t(labels::skill(s))}" }
                }
                if let Some(s) = r.weakest {
                    dt { "{ui.t(Msg::Weakest)}" }
                    dd { "{ui.t(labels::skill(s))}" }
                }
            }
            if !r.next_focus.is_empty() {
                h3 { "{ui.t(Msg::NextFocus)}" }
                ol { class: "focus-list",
                    for (i , f) in r.next_focus.iter().enumerate() {
                        li { key: "{i}",
                            "{ui.t(labels::skill(f.skill))} · "
                            bdi { lang: "id", "{f.topic}" }
                        }
                    }
                }
            }
            div { class: "league",
                h3 { bdi { lang: "id", "Liga Pribadi" } }
                p { class: "muted small", "{ui.t(Msg::LeagueExplain)}" }
                div { class: "stats-row",
                    Stat {
                        value: format!("{} / {}", ui.num(r.league.rank), ui.num(r.league.of)),
                        label: Msg::LeagueRank,
                    }
                    Stat { value: ui.num(r.league.xp), label: Msg::LeagueXp }
                }
            }
        }
    }
}

#[component]
fn Achievements(today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let coach = use_context::<Coach>();
    let events = coach.events.read();
    let unlocked = rewards::unlocked(&events, today);
    rsx! {
        div { class: "card",
            h2 { "{ui.t(Msg::AchievementsTitle)}" }
            ul { class: "achievements",
                for a in rewards::achievements() {
                    {
                        let on = unlocked.iter().find(|u| u.achievement.id == a.id).map(|u| u.on);
                        rsx! {
                            li { key: "{a.id}", class: if on.is_some() { "achievement" } else { "achievement locked" },
                                span { class: "badge", aria_hidden: "true", {icons::star()} }
                                div {
                                    bdi { lang: "id", class: "proverb", "{a.proverb}" }
                                    div { class: "muted small",
                                        match on {
                                            Some(d) => labels::date(&ui, d, today),
                                            None => ui.t(Msg::Locked).to_string(),
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
}

/// What the "bought" toast can undo.
#[derive(Clone, PartialEq)]
struct Bought {
    event_id: String,
    item_id: &'static str,
    name: &'static str,
}

#[component]
fn Warung(today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let mut toast = use_signal(|| None::<Bought>);
    let events = coach.events.read();
    let balance = rewards::koin_balance(&events);
    let owned = rewards::owned(&events);
    let active = coach.batik.read().clone().filter(|s| !s.is_empty());
    let name = coach.profile.read().as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let koin_left = ui.num(balance);

    rsx! {
        div { class: "card",
            h2 { bdi { lang: "id", "Warung" } }
            p { class: "muted small",
                "{ui.t(Msg::WarungExplain)} · "
                bdi { class: "latin", "Koin" }
                " {ui.num(balance)}"
            }
            p { class: "muted small", "{ui.t(Msg::KoinEarnedFrom)}" }
            if let Some(b) = toast() {
                div { class: "toast", role: "status",
                    Nur { mood: Mood::Laughing, text: say(&ui, Line::WarungBought, today, &[("name", name.as_str()), ("item", b.name), ("koin", &koin_left)]) }
                    div { class: "row",
                        button {
                            onclick: {
                                let db = db.clone();
                                let b = b.clone();
                                move |_| {
                                    coach.retract(&db, std::slice::from_ref(&b.event_id));
                                    if coach.batik.peek().as_deref() == Some(b.item_id) {
                                        coach.use_batik(&db, None);
                                    }
                                    toast.set(None);
                                }
                            },
                            "{ui.t(Msg::Undo)}"
                        }
                        button { class: "quiet", onclick: move |_| toast.set(None), "{ui.t(Msg::Dismiss)}" }
                    }
                }
            }
            ul { class: "warung",
                li { class: "ware",
                    div { class: "swatch", style: "background-image: {theme::kawung_tile(super::DEFAULT_BATIK, 0.6, 40)}" }
                    span { "{ui.t(Msg::BatikClassic)}" }
                    if active.is_none() {
                        span { class: "pill", "{ui.t(Msg::InUse)}" }
                    } else {
                        button {
                            onclick: {
                                let db = db.clone();
                                move |_| coach.use_batik(&db, None)
                            },
                            "{ui.t(Msg::Use)}"
                        }
                    }
                }
                for item in rewards::warung().iter().copied() {
                    li { key: "{item.id}", class: "ware",
                        div { class: "swatch", style: "background-image: {theme::kawung_tile(item.color, 0.6, 40)}" }
                        div {
                            bdi { lang: "id", "{item.name}" }
                            div { class: "muted small", "{ui.num(item.koin)} " bdi { class: "latin", "Koin" } }
                        }
                        if active.as_deref() == Some(item.id) {
                            span { class: "pill", "{ui.t(Msg::InUse)}" }
                        } else if owned.iter().any(|o| o == item.id) {
                            button {
                                onclick: {
                                    let db = db.clone();
                                    move |_| coach.use_batik(&db, Some(item.id))
                                },
                                "{ui.t(Msg::Use)}"
                            }
                        } else if balance < item.koin {
                            // Not a dead button: how far off it is, at a glance.
                            div {
                                class: "koin-to-go",
                                role: "img",
                                aria_label: "{ui.t(Msg::NotEnoughKoin)}",
                                span { class: "small",
                                    {ui.t(Msg::KoinToGo).replace("{n}", &ui.num(item.koin - balance))}
                                    " "
                                    bdi { class: "latin", "Koin" }
                                }
                                div { class: "progress",
                                    div {
                                        class: "progress-fill",
                                        style: "inline-size: {u64::from(balance) * 100 / u64::from(item.koin.max(1))}%",
                                    }
                                }
                            }
                        } else {
                            button {
                                class: "primary",
                                onclick: {
                                    let db = db.clone();
                                    move |_| {
                                        let kind = ActivityKind::Purchase { item_id: item.id.into(), koin: item.koin };
                                        if let Some(e) = coach.record(&db, kind, 0) {
                                            coach.use_batik(&db, Some(item.id));
                                            toast.set(Some(Bought { event_id: e.id, item_id: item.id, name: item.name }));
                                        }
                                    }
                                },
                                "{ui.t(Msg::Buy)}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The way into the learning journal (its own screen).
#[component]
fn JournalLink() -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "card journal-link",
            h2 {
                span { class: "nav-icon", aria_hidden: "true", {icons::nav(Screen::Journal)} }
                " {ui.t(Msg::JournalTitle)}"
            }
            p { class: "muted", "{ui.t(Msg::JournalIntro)}" }
            button {
                class: "primary",
                onclick: move |_| {
                    let mut screen = ui.screen;
                    screen.set(Screen::Journal);
                },
                "{ui.t(Msg::JournalOpen)}" }
        }
    }
}
