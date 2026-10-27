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
use kabar_protocol::coach::{ActivityEvent, CoachProfile, DailyPlan, LocalDate, ReminderState};
use serde::Serialize;

use super::{Coach, clock};
use crate::{Db, Ui, platform};

/// Reminders scheduled ahead: enough for two or three days of Steady, so a
/// learner who doesn't open the app still hears from Kak Nur.
pub const UPCOMING: usize = 6;

/// The notification title.
const TITLE: &str = "Kak Nur";

/// One scheduled notification, as `ReminderBridge.schedule` reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Scheduled {
    pub at: u64,
    pub title: String,
    pub text: String,
}

/// What to schedule now, soonest first (empty with no profile or style off).
pub fn schedule(
    lang: UiLanguage,
    eastern_digits: bool,
    profile: Option<&CoachProfile>,
    state: &ReminderState,
    events: &[ActivityEvent],
    plan: Option<&DailyPlan>,
    now_ms: u64,
) -> Vec<Scheduled> {
    let Some(profile) = profile else { return Vec::new() };
    let ctx = ReminderContext { profile, state, events, plan, now_ms };
    let minutes = format_number(u64::from(profile.daily_size.minutes()), lang, eastern_digits);
    reminders::upcoming(&ctx, UPCOMING)
        .into_iter()
        .map(|(at, r)| {
            let day = LocalDate::from_unix_ms(at, profile.utc_offset_min);
            let args = [("name", profile.name.as_str()), ("minutes", minutes.as_str())];
            Scheduled { at, title: TITLE.into(), text: voice::say(lang, r.line, clock::seed(day), &args) }
        })
        .collect()
}

/// Keeps this device's scheduled reminders in step (Android only).
pub fn use_local_reminders(db: &Db, coach: Coach, ui: Ui) {
    let db = db.clone();
    use_effect(move || {
        if !platform::LOCAL_REMINDERS {
            return;
        }
        let profile = coach.profile.read();
        let state = coach.reminders.read();
        let events = coach.events.read();
        let (lang, eastern) = {
            let s = ui.settings.read();
            (s.ui_language, s.eastern_digits)
        };
        let now = *coach.now_ms.peek();
        let offset = profile.as_ref().map_or(CoachProfile::default().utc_offset_min, |p| p.utc_offset_min);
        let plan = db.0.borrow().plan_for(clock::today(now, offset)).ok().flatten();
        let items = schedule(lang, eastern, profile.as_ref(), &state, &events, plan.as_ref(), now);
        let json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".into());
        if let Err(e) = platform::reminders::schedule(&json) {
            crate::diag::report!("scheduling reminders: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::{LocalTime, ReminderStyle};

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

    #[test]
    fn reminders_are_worded_in_the_ui_language() {
        let p = profile(ReminderStyle::Steady);
        let state = ReminderState::default();
        let items = schedule(UiLanguage::ArEg, false, Some(&p), &state, &[], None, at(8));
        assert_eq!(items.len(), UPCOMING);
        assert_eq!(items[0].at, at(9));
        assert_eq!(items[1].at, at(19));
        assert!(items.iter().all(|s| s.title == TITLE && !s.text.is_empty() && !s.text.contains('{')));
        let json = serde_json::to_string(&items[..1]).unwrap();
        assert!(json.starts_with(r#"[{"at":"#) && json.contains(r#""title":"Kak Nur""#), "{json}");
    }

    #[test]
    fn nothing_is_scheduled_without_a_profile_or_with_reminders_off() {
        let state = ReminderState::default();
        assert!(schedule(UiLanguage::En, false, None, &state, &[], None, at(8)).is_empty());
        let off = profile(ReminderStyle::Off);
        assert!(schedule(UiLanguage::En, false, Some(&off), &state, &[], None, at(8)).is_empty());
    }
}
