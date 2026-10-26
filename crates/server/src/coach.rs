//! Kak Nur's reminder dispatcher: every [`CRON_INTERVAL_MIN`] minutes the
//! account's Durable Object alarm reads the synced coach rows, asks the coach
//! rules in kabar-core whether to remind (and whether the weekly report is
//! due), and the Worker glue pushes the result through FCM.
//!
//! The rules come in through [`CoachRules`], so this plumbing is tested with
//! a fake; production uses [`KakNur`], which is kabar-core itself. Devices run
//! the same rules on the same synced data, so once today's goal is met on any
//! device, every device and the server stay quiet.

use kabar_core::coach::reminders::{self, Decision, ReminderContext, ReminderKind, Silence};
use kabar_core::coach::report;
use kabar_core::coach::voice::Line;
use kabar_protocol::cloud::{PushKind, PushNotice};
use kabar_protocol::coach::{
    ActivityEvent, CoachProfile, DailyPlan, LocalDate, PROFILE_ROW_ID, REMINDER_ROW_ID, ReminderState,
};
use kabar_protocol::sync::SeqOp;
use serde::de::DeserializeOwned;

use crate::hub::Hub;
use crate::sql::{Sql, SqlError, meta_get, meta_set};

pub use kabar_core::coach::reminders::CRON_INTERVAL_MIN;

/// How far back activity is read for a decision (streaks, "welcome back",
/// when the learner actually studies).
pub const EVENT_WINDOW_DAYS: u64 = 21;
/// `meta` key: the `week_start` of the last weekly report pushed.
pub const REPORT_PUSHED_KEY: &str = "coach.report_pushed_week";

const DAY_MS: u64 = 86_400_000;
const INTERVAL_MS: u64 = CRON_INTERVAL_MIN as u64 * 60_000;

/// The coach decisions the dispatcher needs.
pub trait CoachRules {
    fn decide(&self, ctx: &ReminderContext<'_>) -> Decision;
    fn mark_sent(&self, state: &ReminderState, now_ms: u64, utc_offset_min: i16) -> ReminderState;
    fn report_due(
        &self,
        profile: &CoachProfile,
        now_ms: u64,
        last_report: Option<LocalDate>,
    ) -> Option<LocalDate>;
}

/// The real rules, from kabar-core (the same code the devices run).
pub struct KakNur;

impl CoachRules for KakNur {
    fn decide(&self, ctx: &ReminderContext<'_>) -> Decision {
        reminders::decide(ctx)
    }

    fn mark_sent(&self, state: &ReminderState, now_ms: u64, utc_offset_min: i16) -> ReminderState {
        reminders::mark_sent(state, now_ms, utc_offset_min)
    }

    fn report_due(
        &self,
        profile: &CoachProfile,
        now_ms: u64,
        last_report: Option<LocalDate>,
    ) -> Option<LocalDate> {
        report::report_due(profile, now_ms, last_report)
    }
}

/// The next alarm: the next quarter-hour boundary (UTC), so reminders land
/// on round local times in every whole- and quarter-hour time zone.
pub fn next_tick_ms(now_ms: u64) -> u64 {
    (now_ms / INTERVAL_MS + 1) * INTERVAL_MS
}

/// `RemindWelcomeBack` → `remind_welcome_back`: the key the app maps back to
/// its `Line`.
pub fn line_key(line: Line) -> String {
    let name = format!("{line:?}");
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn push_kind(kind: ReminderKind) -> PushKind {
    match kind {
        ReminderKind::Study => PushKind::Study,
        ReminderKind::WelcomeBack => PushKind::WelcomeBack,
        ReminderKind::WeeklyReport => PushKind::WeeklyReport,
    }
}

/// Something to push now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Due {
    Reminder(PushNotice),
    Report { notice: PushNotice, week: LocalDate },
}

impl Due {
    pub fn notice(&self) -> &PushNotice {
        match self {
            Due::Reminder(n) | Due::Report { notice: n, .. } => n,
        }
    }
}

/// One alarm's worth of work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tick {
    pub due: Vec<Due>,
    /// Why no reminder went out (for the logs).
    pub silence: Option<Silence>,
    /// Nothing to decide yet (e.g. onboarding not finished).
    pub idle: bool,
}

fn row<T: DeserializeOwned, S: Sql>(
    hub: &Hub<'_, S>,
    table: &str,
    row_id: &str,
) -> Result<Option<T>, SqlError> {
    // A row this build can't read is treated as missing (a newer app may
    // have written it); the next sync from that app fixes it.
    Ok(hub.current_payload(table, row_id)?.and_then(|v| serde_json::from_value(v).ok()))
}

/// Reads the coach rows and asks the rules what's due. Changes nothing.
pub fn tick<S: Sql>(db: &S, rules: &impl CoachRules, now_ms: u64) -> Result<Tick, SqlError> {
    let hub = Hub::open(db);
    let Some(profile) = row::<CoachProfile, _>(&hub, "goals", PROFILE_ROW_ID)? else {
        return Ok(Tick { idle: true, ..Default::default() });
    };
    let state: ReminderState = row(&hub, "reminders", REMINDER_ROW_ID)?.unwrap_or_default();
    let today = LocalDate::from_unix_ms(now_ms, profile.utc_offset_min);
    let plan: Option<DailyPlan> = row(&hub, "daily_plans", &today.to_string())?;
    let since = now_ms.saturating_sub(EVENT_WINDOW_DAYS * DAY_MS);
    let events: Vec<ActivityEvent> = hub
        .appended_since("xp_events", since)?
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    let mut out = Tick::default();
    let ctx =
        ReminderContext { profile: &profile, state: &state, events: &events, plan: plan.as_ref(), now_ms };
    match rules.decide(&ctx) {
        Decision::Send(r) => out.due.push(Due::Reminder(PushNotice {
            kind: push_kind(r.kind),
            line: line_key(r.line),
            seed: today.0,
        })),
        Decision::Silent(s) => out.silence = Some(s),
    }

    let made = hub.latest_row("reports")?.and_then(|(id, _)| id.parse::<LocalDate>().ok());
    let pushed = meta_get(db, REPORT_PUSHED_KEY)?.and_then(|v| v.parse::<LocalDate>().ok());
    if let Some(week) = rules.report_due(&profile, now_ms, made.max(pushed)) {
        let notice =
            PushNotice { kind: PushKind::WeeklyReport, line: line_key(Line::RemindReport), seed: today.0 };
        out.due.push(Due::Report { notice, week });
    }
    Ok(out)
}

/// Records that `due` went out to at least one device. For a reminder, the
/// new [`ReminderState`] is written as the server (built from the state as
/// it is *now*, so a snooze that synced while FCM was busy is kept) and
/// returned to broadcast.
pub fn commit<S: Sql>(
    db: &S,
    rules: &impl CoachRules,
    due: &Due,
    now_ms: u64,
) -> Result<Option<SeqOp>, SqlError> {
    match due {
        Due::Report { week, .. } => {
            meta_set(db, REPORT_PUSHED_KEY, &week.to_string())?;
            Ok(None)
        }
        Due::Reminder(_) => {
            let hub = Hub::open(db);
            let offset =
                row::<CoachProfile, _>(&hub, "goals", PROFILE_ROW_ID)?.unwrap_or_default().utc_offset_min;
            let state: ReminderState = row(&hub, "reminders", REMINDER_ROW_ID)?.unwrap_or_default();
            let next = rules.mark_sent(&state, now_ms, offset);
            let payload = serde_json::to_value(&next).map_err(|e| SqlError(e.to_string()))?;
            match hub.write_server("reminders", REMINDER_ROW_ID, payload, now_ms)? {
                Ok(op) => Ok(Some(op)),
                Err(reason) => Err(SqlError(format!("reminder state refused: {reason:?}"))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use kabar_core::coach::reminders::Reminder;
    use kabar_protocol::coach::{ActivityKind, PlanSize};
    use kabar_protocol::sync::{ClientMsg, Op, OpKind};
    use kabar_protocol::{DeviceId, Hlc, PROTOCOL_VERSION};
    use serde_json::{Value, json};

    use super::*;
    use crate::hub::Conn;
    use crate::sql::testing::MemSql;

    /// 2026-10-26T10:50:56Z, a Monday: 12:50 in Cairo.
    const NOW: u64 = 1_793_011_856_000;

    /// Records what the dispatcher passed in; answers what the test says.
    #[derive(Default)]
    struct Fake {
        decision: Option<Decision>,
        report: Option<LocalDate>,
        seen: RefCell<Vec<String>>,
    }

    impl CoachRules for Fake {
        fn decide(&self, ctx: &ReminderContext<'_>) -> Decision {
            self.seen.borrow_mut().push(format!(
                "decide name={} events={} plan={:?} state={:?}",
                ctx.profile.name,
                ctx.events.len(),
                ctx.plan.map(|p| p.date.to_string()),
                ctx.state.snoozed_until_ms,
            ));
            self.decision.clone().unwrap_or(Decision::Silent(Silence::NotYet))
        }

        fn mark_sent(&self, state: &ReminderState, now_ms: u64, utc_offset_min: i16) -> ReminderState {
            self.seen.borrow_mut().push(format!("mark_sent offset={utc_offset_min}"));
            ReminderState { last_sent_ms: Some(now_ms), sent_today: state.sent_today + 1, ..state.clone() }
        }

        fn report_due(&self, _: &CoachProfile, _: u64, last_report: Option<LocalDate>) -> Option<LocalDate> {
            self.seen.borrow_mut().push(format!("report_due last={:?}", last_report.map(|d| d.to_string())));
            self.report.filter(|w| last_report.is_none_or(|l| l < *w))
        }
    }

    fn date(s: &str) -> LocalDate {
        s.parse().unwrap()
    }

    struct Account {
        db: MemSql,
        n: u64,
    }

    impl Account {
        fn new() -> Self {
            let db = MemSql::new();
            Hub::new(&db).unwrap();
            crate::devices::migrate(&db).unwrap();
            Self { db, n: 0 }
        }

        fn push(&mut self, table: &str, row_id: &str, kind: OpKind, wall_ms: u64, payload: Value) {
            self.n += 1;
            let hub = Hub::open(&self.db);
            let mut conn = Conn { device: DeviceId("s24u".into()), conn_id: "c".into(), ready: false };
            let hello = ClientMsg::Hello { protocol: PROTOCOL_VERSION, last_seq: 0 };
            hub.handle(&mut conn, &serde_json::to_string(&hello).unwrap(), NOW).unwrap();
            let op = Op {
                id: format!("op{}", self.n),
                table: table.into(),
                row_id: row_id.into(),
                kind,
                hlc: Hlc { wall_ms, counter: 0, device: DeviceId("s24u".into()) },
                payload: Some(payload),
            };
            let out = hub.handle(
                &mut conn,
                &serde_json::to_string(&ClientMsg::Push { batch: 1, ops: vec![op] }).unwrap(),
                NOW,
            );
            assert!(out.unwrap().broadcast.len() == 1);
        }

        fn profile(&mut self) {
            let p = CoachProfile { name: "Mohamed".into(), utc_offset_min: 120, ..Default::default() };
            self.push("goals", PROFILE_ROW_ID, OpKind::Upsert, NOW - 1000, serde_json::to_value(p).unwrap());
        }

        fn event(&mut self, id: &str, wall_ms: u64) {
            let e = ActivityEvent {
                id: id.into(),
                hlc: Hlc { wall_ms, counter: 0, device: DeviceId("s24u".into()) },
                date: LocalDate::from_unix_ms(wall_ms, 120),
                at_ms: wall_ms,
                kind: ActivityKind::Reviews { cards: 5, correct: 4 },
                seconds: 120,
            };
            self.push("xp_events", id, OpKind::Append, wall_ms, serde_json::to_value(e).unwrap());
        }
    }

    #[test]
    fn quiet_until_onboarding_is_done() {
        let acc = Account::new();
        let fake = Fake::default();
        assert_eq!(tick(&acc.db, &fake, NOW).unwrap(), Tick { idle: true, ..Default::default() });
        assert!(fake.seen.borrow().is_empty());
    }

    #[test]
    fn hands_the_rules_the_synced_rows() {
        let mut acc = Account::new();
        acc.profile();
        acc.event("e-old", NOW - 30 * DAY_MS);
        acc.event("e-1", NOW - 2 * DAY_MS);
        acc.event("e-2", NOW - 3_600_000);
        acc.push(
            "reminders",
            REMINDER_ROW_ID,
            OpKind::Upsert,
            NOW - 500,
            json!({ "snoozed_until_ms": NOW + 1 }),
        );
        let plan =
            DailyPlan { date: date("2026-10-26"), size: PlanSize::Quick, comeback: false, items: vec![] };
        acc.push("daily_plans", "2026-10-26", OpKind::Upsert, NOW - 400, serde_json::to_value(plan).unwrap());
        acc.push("daily_plans", "2026-10-25", OpKind::Upsert, NOW - 400, json!({ "not": "today" }));
        acc.push("reports", "2026-10-12", OpKind::Upsert, NOW - 300, json!({}));

        let fake = Fake::default();
        let t = tick(&acc.db, &fake, NOW).unwrap();
        assert_eq!(t, Tick { due: vec![], silence: Some(Silence::NotYet), idle: false });
        assert_eq!(
            *fake.seen.borrow(),
            vec![
                format!("decide name=Mohamed events=2 plan=Some(\"2026-10-26\") state=Some({})", NOW + 1),
                "report_due last=Some(\"2026-10-12\")".to_owned(),
            ]
        );
    }

    #[test]
    fn a_reminder_goes_out_and_the_state_syncs_back() {
        let mut acc = Account::new();
        acc.profile();
        let fake = Fake {
            decision: Some(Decision::Send(Reminder {
                kind: ReminderKind::WelcomeBack,
                line: Line::RemindWelcomeBack,
            })),
            ..Default::default()
        };
        let t = tick(&acc.db, &fake, NOW).unwrap();
        let notice = PushNotice {
            kind: PushKind::WelcomeBack,
            line: "remind_welcome_back".into(),
            seed: date("2026-10-26").0,
        };
        assert_eq!(t.due, vec![Due::Reminder(notice)]);

        // A snooze from the tablet syncs while FCM is busy: kept.
        acc.push("reminders", REMINDER_ROW_ID, OpKind::Upsert, NOW, json!({ "snoozed_until_ms": NOW + 9 }));
        let op = commit(&acc.db, &fake, &t.due[0], NOW + 50).unwrap().unwrap();
        assert_eq!(op.op.hlc.device.0, "server");
        assert_eq!(op.op.table, "reminders");
        let state: ReminderState = serde_json::from_value(op.op.payload.clone().unwrap()).unwrap();
        assert_eq!(
            state,
            ReminderState { snoozed_until_ms: Some(NOW + 9), last_sent_ms: Some(NOW + 50), sent_today: 1 }
        );
        assert!(fake.seen.borrow().contains(&"mark_sent offset=120".to_owned()));
        let stored = Hub::open(&acc.db).current_payload("reminders", REMINDER_ROW_ID).unwrap();
        assert_eq!(stored, op.op.payload);
    }

    #[test]
    fn the_weekly_report_is_pushed_once() {
        let mut acc = Account::new();
        acc.profile();
        let fake = Fake { report: Some(date("2026-10-19")), ..Default::default() };
        let t = tick(&acc.db, &fake, NOW).unwrap();
        let [Due::Report { notice, week }] = t.due.as_slice() else { panic!("{t:?}") };
        assert_eq!(
            (notice.kind, notice.line.as_str(), *week),
            (PushKind::WeeklyReport, "remind_report", date("2026-10-19"))
        );
        assert_eq!(commit(&acc.db, &fake, &t.due[0], NOW).unwrap(), None);
        // The next alarm sees it was pushed, even before a device makes the report.
        assert!(tick(&acc.db, &fake, NOW + INTERVAL_MS).unwrap().due.is_empty());
        // A newer report made on a device also counts.
        acc.push("reports", "2026-10-26", OpKind::Upsert, NOW, json!({}));
        tick(&acc.db, &fake, NOW).unwrap();
        assert_eq!(fake.seen.borrow().last().unwrap(), "report_due last=Some(\"2026-10-26\")");
    }

    #[test]
    fn line_keys_are_snake_case() {
        assert_eq!(line_key(Line::RemindStudy), "remind_study");
        assert_eq!(line_key(Line::RemindWelcomeBack), "remind_welcome_back");
        assert_eq!(line_key(Line::PelitaLit), "pelita_lit");
    }

    #[test]
    fn alarms_land_on_quarter_hours() {
        let next = next_tick_ms(NOW);
        assert_eq!(next % INTERVAL_MS, 0);
        assert!(next > NOW && next - NOW <= INTERVAL_MS);
        assert_eq!(next_tick_ms(next), next + INTERVAL_MS);
        // At most 96 alarms a day.
        assert_eq!(DAY_MS / INTERVAL_MS, 96);
    }
}
