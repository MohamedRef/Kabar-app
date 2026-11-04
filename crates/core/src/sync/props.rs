//! Property tests for the merge rules: two devices write to the same rows,
//! and every replica that sees the same ops, in any order and with any
//! duplicates, ends in the same state. Tombstones never come back.

use std::collections::BTreeMap;

use kabar_protocol::sync::{Op, OpKind, SyncKind, table_kind};
use kabar_protocol::{DeviceId, Hlc};
use proptest::prelude::*;
use serde_json::{Value, json};

use super::{Merge, RowMeta, merge, validate};
use crate::clock::HlcClock;

/// One table of each kind.
const TABLES: [&str; 3] = ["word_status", "lesson_events", "ink_strokes"];
const DEVICES: [&str; 2] = ["s24u", "tab-s11"];

/// What one replica holds for a row, like the app's `sync_rows`.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    meta: RowMeta,
    payload: Option<Value>,
}

type Replica = BTreeMap<(String, String), Row>;

/// Applies `op` the way the store and the hub do: validate, merge, store.
/// An immutable tombstone keeps the row's last payload.
fn apply(replica: &mut Replica, op: &Op) -> Merge {
    let sync = validate(op, 1024).expect("generated ops are valid");
    let key = (op.table.clone(), op.row_id.clone());
    let existing = replica.get(&key);
    let decision = merge(sync, op, existing.map(|r| &r.meta));
    if decision == Merge::Apply {
        let deleted = op.kind == OpKind::Delete;
        let payload = match (&op.payload, existing) {
            (None, Some(row)) if sync == SyncKind::Immutable => row.payload.clone(),
            (p, _) => p.clone(),
        };
        replica.insert(key, Row { meta: RowMeta { hlc: op.hlc.clone(), deleted }, payload });
    }
    decision
}

/// What a reader can see: live rows and their payloads, plus which rows are
/// tombstoned. A tombstone's own clock may differ between replicas for
/// immutable rows (the first delete to arrive wins), so it is left out.
fn visible(replica: &Replica) -> BTreeMap<(String, String), Option<Value>> {
    replica
        .iter()
        .map(|(key, row)| (key.clone(), (!row.meta.deleted).then(|| row.payload.clone()).flatten()))
        .collect()
}

/// A device action: which device, which table and row, what it does, and how
/// far its wall clock moved (it may also jump back).
#[derive(Clone, Debug)]
struct Action {
    device: usize,
    table: usize,
    row: usize,
    delete: bool,
    clock_step: i64,
    /// Whether the device first saw everything written so far (a sync).
    synced: bool,
}

fn action() -> impl Strategy<Value = Action> {
    (0..2usize, 0..3usize, 0..3usize, any::<bool>(), -50i64..200, any::<bool>()).prop_map(
        |(device, table, row, delete, clock_step, synced)| Action {
            device,
            table,
            row,
            delete,
            clock_step,
            synced,
        },
    )
}

/// Turns actions into the ops two real devices would write. Append-only and
/// immutable rows are appended once (their ids are UUIDs), then only deleted.
fn ops_from(actions: &[Action]) -> Vec<Op> {
    let mut clocks: Vec<HlcClock> = DEVICES.iter().map(|d| HlcClock::new(DeviceId((*d).into()))).collect();
    let mut walls = [1_000_000i64, 1_000_000];
    let mut ops: Vec<Op> = Vec::new();
    for (n, a) in actions.iter().enumerate() {
        walls[a.device] = (walls[a.device] + a.clock_step).max(0);
        let now = walls[a.device] as u64;
        let clock = &mut clocks[a.device];
        if a.synced {
            for op in &ops {
                clock.observe(&op.hlc, now);
            }
        }
        let table = TABLES[a.table];
        let row_id = format!("row-{}", a.row);
        let sync = table_kind(table).unwrap();
        let appended = ops.iter().any(|o| o.table == table && o.row_id == row_id && o.kind == OpKind::Append);
        let kind = match sync {
            SyncKind::Lww if a.delete => OpKind::Delete,
            SyncKind::Lww => OpKind::Upsert,
            SyncKind::AppendOnly if appended => continue,
            SyncKind::AppendOnly => OpKind::Append,
            SyncKind::Immutable if !appended => OpKind::Append,
            SyncKind::Immutable => OpKind::Delete,
        };
        let payload = match kind {
            OpKind::Delete => None,
            // Each append carries the event's own content; upserts differ per write.
            OpKind::Append => Some(json!({ "row": row_id, "by": DEVICES[a.device] })),
            OpKind::Upsert => Some(json!({ "status": n, "by": DEVICES[a.device] })),
        };
        ops.push(Op {
            id: format!("op-{n}"),
            table: table.into(),
            row_id,
            kind,
            hlc: clock.tick(now),
            payload,
        });
    }
    ops
}

/// A delivery order: every op at least once, some twice, shuffled.
fn delivery(len: usize) -> impl Strategy<Value = Vec<usize>> {
    proptest::collection::vec(0..len.max(1), 0..len + 1).prop_flat_map(move |dupes| {
        let mut order: Vec<usize> = (0..len).chain(dupes.into_iter().filter(|_| len > 0)).collect();
        order.sort_unstable();
        Just(order).prop_shuffle()
    })
}

fn scenario() -> impl Strategy<Value = (Vec<Op>, Vec<usize>, Vec<usize>)> {
    proptest::collection::vec(action(), 1..40).prop_flat_map(|actions| {
        let ops = ops_from(&actions);
        let len = ops.len();
        (Just(ops), delivery(len), delivery(len))
    })
}

fn replay(ops: &[Op], order: &[usize]) -> Replica {
    let mut replica = Replica::new();
    for &i in order {
        apply(&mut replica, &ops[i]);
    }
    replica
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn any_delivery_order_converges((ops, phone, tablet) in scenario()) {
        let a = replay(&ops, &phone);
        let b = replay(&ops, &tablet);
        prop_assert_eq!(visible(&a), visible(&b));
        // LWW rows agree on the winning write too, so a later edit on either
        // device compares against the same clock.
        for (key, row) in &a {
            if table_kind(&key.0) == Some(SyncKind::Lww) {
                prop_assert_eq!(&row.meta, &b[key].meta);
            }
        }
    }

    #[test]
    fn lww_rows_hold_the_write_with_the_highest_clock((ops, order, _) in scenario()) {
        let replica = replay(&ops, &order);
        let mut newest: BTreeMap<(String, String), &Op> = BTreeMap::new();
        for op in ops.iter().filter(|o| o.table == "word_status") {
            let key = (op.table.clone(), op.row_id.clone());
            if newest.get(&key).is_none_or(|n| op.hlc > n.hlc) {
                newest.insert(key, op);
            }
        }
        for (key, op) in newest {
            let row = &replica[&key];
            prop_assert_eq!(&row.meta.hlc, &op.hlc);
            prop_assert_eq!(row.meta.deleted, op.kind == OpKind::Delete);
            prop_assert_eq!(&row.payload, &op.payload);
        }
    }

    #[test]
    fn immutable_tombstones_never_resurrect((ops, order, _) in scenario()) {
        let mut replica = Replica::new();
        for &i in &order {
            let op = &ops[i];
            let key = (op.table.clone(), op.row_id.clone());
            let was_deleted = replica.get(&key).is_some_and(|r| r.meta.deleted);
            apply(&mut replica, op);
            if op.table != "word_status" && was_deleted {
                let row = &replica[&key];
                prop_assert!(row.meta.deleted, "{key:?} came back after its tombstone");
            }
        }
        // And a deleted stroke always ends deleted, whatever arrived first.
        for op in ops.iter().filter(|o| o.table == "ink_strokes" && o.kind == OpKind::Delete) {
            prop_assert!(replica[&(op.table.clone(), op.row_id.clone())].meta.deleted);
        }
    }

    #[test]
    fn replaying_the_same_ops_changes_nothing((ops, order, _) in scenario()) {
        let mut replica = replay(&ops, &order);
        let before = replica.clone();
        for op in &ops {
            prop_assert_eq!(apply(&mut replica, op), Merge::Ignore);
        }
        prop_assert_eq!(replica, before);
    }

    #[test]
    fn an_lww_delete_outranks_only_older_writes(wall in 0u64..1_000, counter in 0u32..5, later in 1u64..1_000) {
        let at = |wall_ms, device: &str| Hlc { wall_ms, counter, device: DeviceId(device.into()) };
        let up = |hlc| Op { id: "u".into(), table: "settings".into(), row_id: "theme".into(), kind: OpKind::Upsert, hlc, payload: Some(json!({ "value": "night" })) };
        let del = Op { id: "d".into(), table: "settings".into(), row_id: "theme".into(), kind: OpKind::Delete, hlc: at(wall + later, "a"), payload: None };
        let tomb = RowMeta { hlc: del.hlc.clone(), deleted: true };
        // An older upsert can't bring the setting back...
        prop_assert_eq!(merge(SyncKind::Lww, &up(at(wall, "z")), Some(&tomb)), Merge::Ignore);
        // ...a newer one is a fresh write and does.
        prop_assert_eq!(merge(SyncKind::Lww, &up(at(wall + later + 1, "a")), Some(&tomb)), Merge::Apply);
    }
}
