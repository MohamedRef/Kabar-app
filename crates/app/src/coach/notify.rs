//! Kak Nur's local reminders (issue #127, phase 1: Android). The device
//! works out the next few with `coach::reminders::upcoming` and words them in
//! the UI language, and the platform schedules them as notifications, so they
//! work offline and without Kabar Cloud. Re-planned whenever the activity
//! log, the reminder settings or the language change, which drops any that
//! no longer apply (today's goal met, snoozed, style off).
//!
//! Issue #187: once this Android device's Firebase token is registered with
//! Kabar Cloud (and the Cloud has its Firebase key), the Cloud's quarter-hour
//! alarm decides instead, from every device's synced activity, and pushes a
//! data-only `{kind, line, seed}` message. The device then schedules nothing
//! locally, so a reminder never arrives twice, and words the push from the
//! [`phrasebook`] it keeps up to date here. Without Firebase, or signed out,
//! the local schedule stays.

use std::collections::BTreeMap;

use dioxus::prelude::*;
use kabar_core::coach::reminders::{self, ReminderContext};
use kabar_core::coach::voice::{self, Line};
use kabar_core::locale::{UiLanguage, format_number};
use kabar_protocol::coach::{ActivityEvent, CoachProfile, DailyPlan, LocalDate, LocalTime, ReminderState};
use serde::Serialize;

use super::{Coach, clock};
use crate::account::Cloud;
use crate::cloud::{Command, SyncState};
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
    /// Kabar Cloud pushes this device's reminders (FCM), so none are
    /// scheduled locally.
    pub pushed: bool,
    pub profile: Option<&'a CoachProfile>,
    pub state: &'a ReminderState,
    pub events: &'a [ActivityEvent],
    pub plan: Option<&'a DailyPlan>,
    pub now_ms: u64,
}

/// What to schedule now, soonest first: empty on a device that doesn't
/// ring, whose reminders Kabar Cloud pushes, with no profile, or with
/// reminders off.
pub fn schedule(i: Inputs) -> Vec<Scheduled> {
    let Some(profile) = i.profile.filter(|_| i.here && !i.pushed) else { return Vec::new() };
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

/// The lines Kabar Cloud may push (`coach::reminders::decide` and the weekly
/// report): the server sends only the line's key and a seed.
pub const PUSH_LINES: [Line; 4] =
    [Line::RemindStudy, Line::RemindGentle, Line::RemindWelcomeBack, Line::RemindReport];

/// Every wording of the pushable lines, already in the UI language with the
/// learner's name and minutes filled in, as `PushBridge.setPhrasebook` keeps
/// it. A push `{line, seed}` shows `lines[line][seed mod n]`, the very text
/// `voice::say(line, seed)` gives (see [`push_text`]), with no Rust running.
/// It stays on the device: the server never sees the name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Phrasebook {
    pub title: String,
    pub lines: BTreeMap<&'static str, Vec<String>>,
}

pub fn phrasebook(lang: UiLanguage, eastern_digits: bool, profile: &CoachProfile) -> Phrasebook {
    let minutes = format_number(u64::from(profile.daily_size.minutes()), lang, eastern_digits);
    let args = [("name", profile.name.as_str()), ("minutes", minutes.as_str())];
    let lines = PUSH_LINES
        .iter()
        .map(|&line| {
            let n = voice::variant_count(lang, line) as u64;
            (line.key(), (0..n).map(|seed| voice::say(lang, line, seed, &args)).collect())
        })
        .collect();
    Phrasebook { title: TITLE.into(), lines }
}

/// What `PushService.kt` shows for a push: `lines[line][seed mod n]`. Kept
/// here so the rule the Kotlin side follows is tested.
#[cfg_attr(not(test), allow(dead_code))]
pub fn push_text<'a>(book: &'a Phrasebook, line: &str, seed: i64) -> Option<&'a str> {
    let variants = book.lines.get(line)?;
    let n = i64::try_from(variants.len()).ok().filter(|&n| n > 0)?;
    variants.get(usize::try_from(seed.rem_euclid(n)).ok()?).map(String::as_str)
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

/// Keeps this device's scheduled reminders in step (Android and the Mac),
/// and the push phrasebook where Firebase is set up.
pub fn use_local_reminders(db: &Db, coach: Coach, ui: Ui, prefs: DevicePrefs) {
    let db = db.clone();
    let hook_db = db.clone();
    let cloud = try_use_context::<Cloud>();
    use_effect(move || {
        if !platform::push::available() {
            return;
        }
        let profile = coach.profile.read();
        let Some(profile) = profile.as_ref() else { return };
        let (lang, eastern_digits) = {
            let s = ui.settings.read();
            (s.ui_language, s.eastern_digits)
        };
        let json = serde_json::to_string(&phrasebook(lang, eastern_digits, profile)).unwrap_or_default();
        if let Err(e) = platform::push::set_phrasebook(&json) {
            crate::diag::report!("push phrasebook: {e}");
        }
    });
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
        let pushed = cloud.is_some_and(|c| *c.push.read());
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
            pushed,
            profile: profile.as_ref(),
            state: &state,
            events: &events,
            plan: plan.as_ref(),
            now_ms,
        });
        let json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".into());
        if let Err(e) = platform::reminders::schedule(&json) {
            crate::diag::report!("scheduling reminders: {e}");
        }
    });
}

/// How long to wait for Firebase's first token: it usually comes within a
/// few seconds of the first launch, later ones have it at once.
const TOKEN_TRIES: u32 = 12;
const TOKEN_POLL: std::time::Duration = std::time::Duration::from_secs(5);

/// Tells Kabar Cloud where to push Kak Nur's reminders (#187): this device's
/// FCM token when it rings, `""` when it doesn't, once per launch and again
/// after a rotated token, a changed "Remind me on this device" or a failed
/// try. Forgets the push state on sign-out, so local reminders come back.
pub fn use_push_token(ui: Ui, prefs: DevicePrefs) {
    let Some(cloud) = try_use_context::<Cloud>() else { return };
    // Only these matter: the status also ticks with every sync.
    let link = use_memo(move || {
        let s = cloud.status.read();
        (s.configured && s.email.is_none(), matches!(s.sync, SyncState::Live | SyncState::CatchingUp))
    });
    use_effect(move || {
        if !platform::push::available() {
            return;
        }
        let (mut push, mut sent) = (cloud.push, cloud.push_sent);
        let (signed_out, connected) = *link.read();
        let here = prefs.reminds_here(*ui.tablet.read());
        if signed_out {
            if *push.peek() {
                platform::push::set_pushed("", false);
                push.set(false);
            }
            if sent.peek().is_some() {
                sent.set(None);
            }
            return;
        }
        if !connected {
            return;
        }
        spawn(async move {
            let mut token = here.then(platform::push::token).flatten();
            for _ in 1..TOKEN_TRIES {
                if token.is_some() || !here {
                    break;
                }
                crate::platform::time::sleep(TOKEN_POLL).await;
                token = platform::push::token();
            }
            let token = if here {
                let Some(t) = token else {
                    crate::diag::report!("no Firebase token yet: reminders stay local");
                    return;
                };
                t
            } else {
                String::new()
            };
            if sent.peek().as_deref() == Some(token.as_str()) {
                return;
            }
            sent.set(Some(token.clone()));
            cloud.send(Command::PushToken(token));
        });
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
            pushed: false,
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
        assert!(
            schedule(Inputs { pushed: true, ..inputs(Some(&on), &state) }).is_empty(),
            "Kabar Cloud pushes them instead, so none arrives twice"
        );
    }

    #[test]
    fn the_phrasebook_words_a_push_like_the_app_would() {
        let p = profile(ReminderStyle::Steady);
        for lang in [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En] {
            let book = phrasebook(lang, true, &p);
            assert_eq!(book.title, TITLE);
            assert_eq!(book.lines.len(), PUSH_LINES.len());
            let minutes = format_number(u64::from(p.daily_size.minutes()), lang, true);
            let args = [("name", p.name.as_str()), ("minutes", minutes.as_str())];
            for line in PUSH_LINES {
                assert!(!book.lines[line.key()].is_empty(), "{lang:?} {line:?}");
                // The server's seed is the learner's local day number.
                for seed in [0, 1, 7, 20_752, 20_753, 20_754] {
                    let want = voice::say(lang, line, seed as u64, &args);
                    assert_eq!(
                        push_text(&book, line.key(), seed),
                        Some(want.as_str()),
                        "{lang:?} {line:?} {seed}"
                    );
                    assert!(!want.contains('{'));
                }
            }
        }
        let book = phrasebook(UiLanguage::En, false, &p);
        assert!(push_text(&book, "remind_report", 3).is_some());
        assert_eq!(push_text(&book, "level_up", 3), None, "not a pushable line");
        assert!(push_text(&book, "remind_study", -1).is_some(), "never panics on odd seeds");
        let json = serde_json::to_string(&book).unwrap();
        assert!(json.starts_with(r#"{"title":"Kak Nur","lines":{"remind_gentle":["#), "{json}");
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
