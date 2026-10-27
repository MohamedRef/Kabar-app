//! Kak Nur's local reminders (issue #127, phase 1: Android). The device
//! works out the next few with `coach::reminders::upcoming` and words them in
//! the UI language, and the platform schedules them as notifications, so they
//! work offline and without Kabar Cloud. Re-planned whenever the activity
//! log, the reminder settings or the language change, which drops any that
//! no longer apply (today's goal met, snoozed, style off).

use dioxus::prelude::*;
use kabar_core::coach::reminders::{self, ReminderContext};
use kabar_core::coach::voice;
use kabar_core::locale::{UiLanguage, format_number};
use kabar_protocol::coach::{ActivityEvent, CoachProfile, DailyPlan, LocalDate, LocalTime, ReminderState};
use serde::Serialize;

use super::{Coach, clock};
use crate::settings::device::DevicePrefs;
use crate::{Db, Ui, platform};

/// Reminders scheduled ahead: enough for two or three days of Steady, so a
/// learner who doesn't open the app still hears from Kak Nur.
pub const UPCOMING: usize = 6;

/// The notification title.
const TITLE: &str = "Kak Nur";

/// One scheduled notification, as `ReminderBridge.schedule` reads it.
/// `date` and `time` are the learner's local wall clock: Android arms the
/// alarm from them in the device's own time zone, so a daylight-saving
/// change (Egypt's ends the last Thursday of October) keeps 19:00 at 19:00.
/// `at` is the same moment by the profile's offset, the fallback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Scheduled {
    pub at: u64,
    pub date: String,
    pub time: String,
    pub title: String,
    pub text: String,
}

/// Everything [`schedule`] reads.
#[derive(Clone, Copy)]
pub struct Inputs<'a> {
    pub lang: UiLanguage,
    pub eastern_digits: bool,
    /// This device rings (Settings → "Remind me on this device").
    pub here: bool,
    pub profile: Option<&'a CoachProfile>,
    pub state: &'a ReminderState,
    pub events: &'a [ActivityEvent],
    pub plan: Option<&'a DailyPlan>,
    pub now_ms: u64,
}

/// What to schedule now, soonest first: empty on a device that doesn't
/// ring, with no profile, or with reminders off.
pub fn schedule(i: Inputs) -> Vec<Scheduled> {
    let Some(profile) = i.profile.filter(|_| i.here) else { return Vec::new() };
    let ctx = ReminderContext { profile, state: i.state, events: i.events, plan: i.plan, now_ms: i.now_ms };
    let minutes = format_number(u64::from(profile.daily_size.minutes()), i.lang, i.eastern_digits);
    let off = profile.utc_offset_min;
    reminders::upcoming(&ctx, UPCOMING)
        .into_iter()
        .map(|(at, r)| {
            let day = LocalDate::from_unix_ms(at, off);
            let args = [("name", profile.name.as_str()), ("minutes", minutes.as_str())];
            Scheduled {
                at,
                date: day.to_string(),
                time: LocalTime::from_unix_ms(at, off).to_string(),
                title: TITLE.into(),
                text: voice::say(i.lang, r.line, clock::seed(day), &args),
            }
        })
        .collect()
}

/// Counts the reminders Android showed while the app was closed toward the
/// synced daily limit, so every device (and Kabar Cloud) sees them.
pub fn record_delivered(
    state: &ReminderState,
    delivered: &[u64],
    utc_offset_min: i16,
) -> Option<ReminderState> {
    let mut shown: Vec<u64> = delivered
        .iter()
        .copied()
        .filter(|&at| at > 0 && state.last_sent_ms.is_none_or(|last| at > last))
        .collect();
    shown.sort_unstable();
    shown.dedup();
    let next = shown.iter().fold(state.clone(), |s, &at| reminders::mark_sent(&s, at, utc_offset_min));
    (next != *state).then_some(next)
}

/// Keeps this device's scheduled reminders in step (Android only).
pub fn use_local_reminders(db: &Db, coach: Coach, ui: Ui, prefs: DevicePrefs) {
    let db = db.clone();
    let hook_db = db.clone();
    use_hook(move || {
        let delivered = platform::reminders::take_delivered();
        let offset = coach.offset();
        if let Some(next) = record_delivered(&coach.reminders.peek(), &delivered, offset) {
            coach.save_reminders(&hook_db, next);
        }
    });
    use_effect(move || {
        if !platform::LOCAL_REMINDERS {
            return;
        }
        let here = prefs.reminds_here(*ui.tablet.read());
        let profile = coach.profile.read();
        let state = coach.reminders.read();
        let events = coach.events.read();
        let (lang, eastern_digits) = {
            let s = ui.settings.read();
            (s.ui_language, s.eastern_digits)
        };
        let now_ms = *coach.now_ms.peek();
        let offset = profile.as_ref().map_or(CoachProfile::default().utc_offset_min, |p| p.utc_offset_min);
        let plan = db.0.borrow().plan_for(clock::today(now_ms, offset)).ok().flatten();
        let items = schedule(Inputs {
            lang,
            eastern_digits,
            here,
            profile: profile.as_ref(),
            state: &state,
            events: &events,
            plan: plan.as_ref(),
            now_ms,
        });
        let json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".into());
        if let Err(e) = platform::reminders::schedule(&json) {
            eprintln!("kabar: scheduling reminders: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::ReminderStyle;

    use super::*;

    fn profile(style: ReminderStyle) -> CoachProfile {
        CoachProfile {
            name: "Mohamed".into(),
            study_times: vec![LocalTime::hm(9, 0), LocalTime::hm(19, 0)],
            reminder_style: style,
            ..CoachProfile::default()
        }
    }

    fn at(h: u16) -> u64 {
        let p = CoachProfile::default();
        kabar_protocol::coach::to_unix_ms(
            LocalDate::from_ymd(2026, 10, 27).unwrap(),
            LocalTime::hm(h, 0),
            p.utc_offset_min,
        )
    }

    fn inputs<'a>(profile: Option<&'a CoachProfile>, state: &'a ReminderState) -> Inputs<'a> {
        Inputs {
            lang: UiLanguage::ArEg,
            eastern_digits: false,
            here: true,
            profile,
            state,
            events: &[],
            plan: None,
            now_ms: at(8),
        }
    }

    #[test]
    fn reminders_are_worded_in_the_ui_language() {
        let p = profile(ReminderStyle::Steady);
        let state = ReminderState::default();
        let items = schedule(inputs(Some(&p), &state));
        assert_eq!(items.len(), UPCOMING);
        assert_eq!(items[0].at, at(9));
        assert_eq!((items[0].date.as_str(), items[0].time.as_str()), ("2026-10-27", "09:00"));
        assert_eq!(items[1].at, at(19));
        assert_eq!(items[1].time, "19:00");
        assert!(items.iter().all(|s| s.title == TITLE && !s.text.is_empty() && !s.text.contains('{')));
        let json = serde_json::to_string(&items[..1]).unwrap();
        assert!(json.starts_with(r#"[{"at":"#) && json.contains(r#""title":"Kak Nur""#), "{json}");
    }

    #[test]
    fn nothing_is_scheduled_without_a_profile_with_reminders_off_or_on_a_quiet_device() {
        let state = ReminderState::default();
        assert!(schedule(inputs(None, &state)).is_empty());
        let off = profile(ReminderStyle::Off);
        assert!(schedule(inputs(Some(&off), &state)).is_empty());
        let on = profile(ReminderStyle::Steady);
        assert!(
            schedule(Inputs { here: false, ..inputs(Some(&on), &state) }).is_empty(),
            "the tablet stays quiet"
        );
    }

    #[test]
    fn delivered_reminders_count_toward_the_synced_limit() {
        let p = profile(ReminderStyle::Steady);
        let off = p.utc_offset_min;
        let state = ReminderState::default();
        let next = record_delivered(&state, &[at(19), at(9), at(9)], off).unwrap();
        assert_eq!((next.last_sent_ms, next.sent_today), (Some(at(19)), 2));
        assert_eq!(record_delivered(&next, &[at(9)], off), None, "already counted");
        // The phone rang at 09:00, so the next plan starts at 19:00.
        let items = schedule(Inputs {
            now_ms: at(9) + 60_000,
            ..inputs(Some(&p), &record_delivered(&state, &[at(9)], off).unwrap())
        });
        assert_eq!(items[0].at, at(19));
        // Steady's limit for two times is two a day: none left after both rang.
        let later = schedule(Inputs { now_ms: at(19) + 60_000, ..inputs(Some(&p), &next) });
        assert!(later.iter().all(|s| s.date != "2026-10-27"));
    }
}
