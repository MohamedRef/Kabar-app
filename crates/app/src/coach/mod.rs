//! Kak Nur, the coach (spec §6): onboarding, today's plan, the recap, the
//! weekly report, rewards and the learning journal. The decisions live in
//! `kabar_core::coach`; this module keeps the synced data in signals and
//! draws it.

pub mod clock;
pub mod coach_screen;
pub mod journal;
pub mod labels;
pub mod logic;
pub mod notify;
pub mod onboarding;
pub mod settings;
pub mod today;
pub mod widgets;

use dioxus::prelude::*;
use kabar_core::coach::plan::{self, PlanInputs};
use kabar_core::coach::voice::{self, Line};
use kabar_core::coach::{history, onboarding as core_onboarding, rewards};
use kabar_protocol::coach::{
    ActivityEvent, ActivityKind, CoachProfile, DailyPlan, LanguageFocus, LocalDate, LocalTime, PlanSize,
    ReminderState,
};

use crate::{Db, Ui};

/// How far back weak spots are looked for.
pub const WEAK_SPOT_WINDOW_DAYS: i32 = 14;
/// The background batik when no Warung theme is in use.
pub const DEFAULT_BATIK: &str = "2b3a70";
const BATIK_KEY: &str = "batik";

/// The coach's synced data, shared through context.
#[derive(Clone, Copy)]
pub struct Coach {
    pub profile: Signal<Option<CoachProfile>>,
    /// The whole activity log, oldest first.
    pub events: Signal<Vec<ActivityEvent>>,
    pub reminders: Signal<ReminderState>,
    /// Wall clock, refreshed every minute so "today" rolls over at midnight.
    pub now_ms: Signal<u64>,
    /// This device's UTC offset, as its WebView reports it.
    pub device_offset: Signal<Option<i16>>,
    /// The Warung theme in use on this device.
    pub batik: Signal<Option<String>>,
    /// Bumped when a plan is rebuilt, so the Today screen re-reads it.
    pub plan_rev: Signal<u32>,
}

impl Coach {
    /// Call inside `use_context_provider`.
    pub fn load(db: &Db) -> Self {
        let store = db.0.borrow();
        let log = |what: &str, e: &dyn std::fmt::Display| crate::diag::report!("loading {what} failed: {e}");
        let profile = store.coach_profile().unwrap_or_else(|e| {
            log("coach profile", &e);
            None
        });
        let events = store.activity(None).unwrap_or_else(|e| {
            log("activity", &e);
            Vec::new()
        });
        let reminders = store.reminder_state().unwrap_or_default();
        let batik = store.local_value(BATIK_KEY).ok().flatten();
        Self {
            profile: Signal::new(profile),
            events: Signal::new(events),
            reminders: Signal::new(reminders),
            now_ms: Signal::new(crate::store::now_ms()),
            device_offset: Signal::new(None),
            batik: Signal::new(batik),
            plan_rev: Signal::new(0),
        }
    }

    /// The learner's UTC offset: the profile's, else this device's.
    pub fn offset(&self) -> i16 {
        self.profile
            .read()
            .as_ref()
            .map(|p| p.utc_offset_min)
            .or(*self.device_offset.read())
            .unwrap_or(CoachProfile::default().utc_offset_min)
    }

    pub fn now(&self) -> u64 {
        *self.now_ms.read()
    }

    pub fn today(&self) -> LocalDate {
        clock::today(self.now(), self.offset())
    }

    pub fn time_of_day(&self) -> LocalTime {
        clock::time_of_day(self.now(), self.offset())
    }

    pub fn save_profile(mut self, db: &Db, profile: CoachProfile) {
        match db.0.borrow_mut().save_coach_profile(&profile) {
            Ok(()) => self.profile.set(Some(profile)),
            Err(e) => crate::diag::report!("saving the coach profile failed: {e}"),
        }
    }

    /// Appends to the activity log, dated in the learner's local day.
    pub fn record(mut self, db: &Db, kind: ActivityKind, seconds: u32) -> Option<ActivityEvent> {
        let date = self.today();
        match db.0.borrow_mut().add_activity(kind, date, seconds) {
            Ok(e) => {
                self.events.write().push(e.clone());
                Some(e)
            }
            Err(e) => {
                crate::diag::report!("recording activity failed: {e}");
                None
            }
        }
    }

    /// Undo: tombstones the events (see `Store::retract_activity`).
    pub fn retract(mut self, db: &Db, ids: &[String]) {
        for id in ids {
            if let Err(e) = db.0.borrow_mut().retract_activity(id) {
                crate::diag::report!("undo failed: {e}");
                return;
            }
        }
        self.events.write().retain(|e| !ids.contains(&e.id));
    }

    pub fn save_reminders(mut self, db: &Db, state: ReminderState) {
        match db.0.borrow_mut().save_reminder_state(&state) {
            Ok(()) => self.reminders.set(state),
            Err(e) => crate::diag::report!("saving reminders failed: {e}"),
        }
    }

    pub fn use_batik(mut self, db: &Db, item_id: Option<&str>) {
        if let Err(e) = db.0.borrow_mut().set_local_value(BATIK_KEY, item_id.unwrap_or_default()) {
            crate::diag::report!("saving the batik failed: {e}");
        }
        self.batik.set(item_id.map(str::to_string));
    }

    /// The colour of the background batik (CSS hex without `#`).
    pub fn batik_color(&self) -> &'static str {
        match self.batik.read().as_deref() {
            None | Some("") => DEFAULT_BATIK,
            Some(id) => rewards::warung().iter().find(|i| i.id == id).map_or(DEFAULT_BATIK, |i| i.color),
        }
    }
}

/// Builds the plan for `date` (not saved). `due_reviews` comes from the
/// review deck; the lesson is the unit the Journey is on (its start follows
/// the placement test, else the onboarding answer). `picked`: the learner
/// chose `size` just now (a bigger size then wins over a welcome-back plan).
pub fn build_plan(
    db: &Db,
    due_reviews: u32,
    profile: &CoachProfile,
    events: &[ActivityEvent],
    date: LocalDate,
    size: PlanSize,
    picked: bool,
) -> DailyPlan {
    let weak = plan::weak_spots(events, date, WEAK_SPOT_WINDOW_DAYS);
    let completed = db.0.borrow().completed_lessons().unwrap_or_default();
    let start = crate::journey::start_level(db, Some(profile));
    let javanese = profile.focus == LanguageFocus::Javanese;
    let unit = crate::journey::current_unit(&completed, start.as_deref(), javanese)
        .unwrap_or_else(|| core_onboarding::starting_unit(profile.start).to_string());
    plan::build_plan(&PlanInputs {
        date,
        size,
        picked,
        due_reviews,
        current_unit: Some(&unit),
        weak_spots: &weak,
        last_study_day: history::last_study_day(events, date),
    })
}

/// Today's plan: the stored one (so every device shows the same plan), or a
/// fresh one at the learner's usual size, saved for the others.
pub fn plan_for(db: &Db, profile: &CoachProfile, events: &[ActivityEvent], date: LocalDate) -> DailyPlan {
    if let Ok(Some(plan)) = db.0.borrow().plan_for(date) {
        return plan;
    }
    let due = crate::review::due_words(db, crate::store::now_ms()).len() as u32;
    let plan = build_plan(db, due, profile, events, date, profile.daily_size, false);
    if let Err(e) = db.0.borrow_mut().save_plan(&plan) {
        crate::diag::report!("saving today's plan failed: {e}");
    }
    plan
}

/// Kak Nur's line in the UI language, worded the same all day on every device.
pub fn say(ui: &Ui, line: Line, date: LocalDate, args: &[(&str, &str)]) -> String {
    voice::say(ui.lang(), line, clock::seed(date), args)
}

/// Keeps `now_ms` ticking and the profile's UTC offset in step with this
/// device's time zone (checked at launch and every minute, so travel and
/// daylight saving are picked up).
pub fn use_clock(coach: Coach, db: Db) {
    use_future(move || {
        let db = db.clone();
        async move {
            let mut eval = document::eval(
                r#"
                const send = () => dioxus.send(new Date().getTimezoneOffset());
                setInterval(send, 60000);
                send();
                await new Promise(() => {});
                "#,
            );
            let (mut now_ms, mut device_offset) = (coach.now_ms, coach.device_offset);
            while let Ok(minutes_west) = eval.recv::<f64>().await {
                now_ms.set(crate::store::now_ms());
                let Some(offset) = clock::offset_from_js(minutes_west) else { continue };
                if *device_offset.peek() != Some(offset) {
                    device_offset.set(Some(offset));
                }
                let changed =
                    coach.profile.peek().as_ref().and_then(|p| clock::with_device_offset(p, offset));
                if let Some(profile) = changed {
                    coach.save_profile(&db, profile);
                }
            }
        }
    });
}
