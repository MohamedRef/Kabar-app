//! Sync merge rules, shared by the app and the hub in Kabar Cloud so both
//! sides reach the same state from the same ops in any order.
//!
//! - LWW tables keep the write with the highest HLC (wall, counter, device).
//! - Append-only tables insert a row once; later ops for the same row are duplicates.
//! - Immutable tables insert once; a delete tombstones the row for good.

use kabar_protocol::Hlc;
use kabar_protocol::sync::{Op, OpKind, RejectReason, SyncKind};

/// What a store already holds for `(table, row_id)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowMeta {
    pub hlc: Hlc,
    pub deleted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Merge {
    /// Store the op and make it the row's current state.
    Apply,
    /// Nothing changes: a duplicate, or an older write that lost.
    Ignore,
}

/// Whether `kind` is a valid operation on a table of `sync`.
pub fn kind_allowed(sync: SyncKind, kind: OpKind) -> bool {
    match sync {
        SyncKind::Lww => matches!(kind, OpKind::Upsert | OpKind::Delete),
        SyncKind::AppendOnly => kind == OpKind::Append,
        SyncKind::Immutable => matches!(kind, OpKind::Append | OpKind::Delete),
    }
}

/// Decides how an op merges into the current row. Call after [`kind_allowed`].
pub fn merge(sync: SyncKind, op: &Op, existing: Option<&RowMeta>) -> Merge {
    let Some(row) = existing else { return Merge::Apply };
    match sync {
        SyncKind::Lww => {
            if op.hlc > row.hlc {
                Merge::Apply
            } else {
                Merge::Ignore
            }
        }
        SyncKind::AppendOnly => Merge::Ignore,
        SyncKind::Immutable => {
            if op.kind == OpKind::Delete && !row.deleted {
                Merge::Apply
            } else {
                Merge::Ignore
            }
        }
    }
}

/// Checks that don't need storage: table, kind, payload. Ids are validated
/// separately because the allowed id shape is a store concern.
pub fn validate(op: &Op, max_payload_bytes: usize) -> Result<SyncKind, RejectReason> {
    let sync = kabar_protocol::sync::table_kind(&op.table).ok_or(RejectReason::UnknownTable)?;
    if !kind_allowed(sync, op.kind) {
        return Err(RejectReason::KindNotAllowed);
    }
    match (&op.payload, op.kind) {
        (None, OpKind::Upsert | OpKind::Append) => return Err(RejectReason::MissingPayload),
        (Some(payload), _) => {
            // serde_json::Value always serializes.
            let size = serde_json::to_string(payload).map(|s| s.len()).unwrap_or(usize::MAX);
            if size > max_payload_bytes {
                return Err(RejectReason::PayloadTooLarge);
            }
        }
        (None, OpKind::Delete) => {}
    }
    Ok(sync)
}

#[cfg(test)]
mod tests {
    use kabar_protocol::DeviceId;
    use serde_json::json;

    use super::*;

    fn op(table: &str, kind: OpKind, wall_ms: u64, device: &str) -> Op {
        Op {
            id: format!("{device}-{wall_ms}"),
            table: table.into(),
            row_id: "rumah".into(),
            kind,
            hlc: Hlc { wall_ms, counter: 0, device: DeviceId(device.into()) },
            payload: (kind != OpKind::Delete).then(|| json!({ "status": "known" })),
        }
    }

    fn meta(o: &Op, deleted: bool) -> RowMeta {
        RowMeta { hlc: o.hlc.clone(), deleted }
    }

    #[test]
    fn lww_keeps_the_newest_write_in_any_order() {
        let phone = op("word_status", OpKind::Upsert, 10, "s24u");
        let tablet = op("word_status", OpKind::Upsert, 11, "tab-s11");
        // Arrival order phone → tablet
        assert_eq!(merge(SyncKind::Lww, &phone, None), Merge::Apply);
        assert_eq!(merge(SyncKind::Lww, &tablet, Some(&meta(&phone, false))), Merge::Apply);
        // Arrival order tablet → phone
        assert_eq!(merge(SyncKind::Lww, &phone, Some(&meta(&tablet, false))), Merge::Ignore);
    }

    #[test]
    fn lww_ties_break_on_device_id() {
        let a = op("settings", OpKind::Upsert, 10, "a");
        let b = op("settings", OpKind::Upsert, 10, "b");
        assert_eq!(merge(SyncKind::Lww, &b, Some(&meta(&a, false))), Merge::Apply);
        assert_eq!(merge(SyncKind::Lww, &a, Some(&meta(&b, false))), Merge::Ignore);
    }

    #[test]
    fn lww_delete_is_a_newer_write() {
        let up = op("highlights", OpKind::Upsert, 10, "a");
        let del = op("highlights", OpKind::Delete, 12, "b");
        assert_eq!(merge(SyncKind::Lww, &del, Some(&meta(&up, false))), Merge::Apply);
        let late_up = op("highlights", OpKind::Upsert, 11, "a");
        assert_eq!(merge(SyncKind::Lww, &late_up, Some(&meta(&del, true))), Merge::Ignore);
    }

    #[test]
    fn append_only_inserts_once() {
        let r = op("review_log", OpKind::Append, 10, "a");
        assert_eq!(merge(SyncKind::AppendOnly, &r, None), Merge::Apply);
        assert_eq!(merge(SyncKind::AppendOnly, &r, Some(&meta(&r, false))), Merge::Ignore);
    }

    #[test]
    fn immutable_strokes_can_only_be_tombstoned() {
        let s = op("ink_strokes", OpKind::Append, 10, "tab");
        let d = op("ink_strokes", OpKind::Delete, 5, "phone");
        assert_eq!(merge(SyncKind::Immutable, &s, None), Merge::Apply);
        // A delete wins even with an older clock: strokes never change otherwise.
        assert_eq!(merge(SyncKind::Immutable, &d, Some(&meta(&s, false))), Merge::Apply);
        assert_eq!(merge(SyncKind::Immutable, &s, Some(&meta(&d, true))), Merge::Ignore);
        assert_eq!(merge(SyncKind::Immutable, &d, Some(&meta(&d, true))), Merge::Ignore);
    }

    #[test]
    fn validation() {
        assert_eq!(validate(&op("word_status", OpKind::Upsert, 1, "a"), 1024), Ok(SyncKind::Lww));
        assert_eq!(validate(&op("nope", OpKind::Upsert, 1, "a"), 1024), Err(RejectReason::UnknownTable));
        assert_eq!(
            validate(&op("review_log", OpKind::Upsert, 1, "a"), 1024),
            Err(RejectReason::KindNotAllowed)
        );
        assert_eq!(
            validate(&op("word_status", OpKind::Append, 1, "a"), 1024),
            Err(RejectReason::KindNotAllowed)
        );
        let mut empty = op("word_status", OpKind::Upsert, 1, "a");
        empty.payload = None;
        assert_eq!(validate(&empty, 1024), Err(RejectReason::MissingPayload));
        assert_eq!(
            validate(&op("word_status", OpKind::Upsert, 1, "a"), 5),
            Err(RejectReason::PayloadTooLarge)
        );
    }
}
