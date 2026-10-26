//! Live sync wire types: ops, and the WebSocket messages between a device and
//! its account's sync hub (a Durable Object in Kabar Cloud).
//!
//! Flow: the device opens `GET /v1/sync` (WebSocket), sends [`ClientMsg::Hello`]
//! with the last server `seq` it has seen, receives the missing ops in
//! [`ServerMsg::Ops`] pages and a [`ServerMsg::CaughtUp`]. From then on it
//! pushes its outbox with [`ClientMsg::Push`] and receives other devices' ops
//! as they arrive.

use serde::{Deserialize, Serialize};

use crate::Hlc;

/// Most ops a device may send in one [`ClientMsg::Push`].
pub const MAX_OPS_PER_PUSH: usize = 50;
/// Largest serialized payload of a single op, in bytes.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;
/// Most ops the hub sends in one [`ServerMsg::Ops`] page.
pub const OPS_PAGE_SIZE: usize = 200;
/// How far ahead of the hub's clock an op's HLC may be. Anything further
/// would win every last-writer-wins race until the real clock catches up.
pub const MAX_CLOCK_SKEW_MS: u64 = 5 * 60 * 1000;

/// How a table merges concurrent writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncKind {
    /// Last writer wins by `(hlc)`: statuses, settings, documents.
    Lww,
    /// Events that are only ever added: reviews, attempts, XP.
    AppendOnly,
    /// Written once, deleted only by a tombstone: ink strokes.
    Immutable,
}

/// Every synced table and how it merges. A table missing here is rejected by
/// the hub, so an old server never silently drops a new client's data.
pub const TABLES: &[(&str, SyncKind)] = &[
    // Reading
    ("texts", SyncKind::Lww),
    ("highlights", SyncKind::Lww),
    ("word_status", SyncKind::Lww),
    // Learning
    ("review_log", SyncKind::AppendOnly),
    ("attempts", SyncKind::AppendOnly),
    // Activity log: append-only, but a tombstone lets the learner undo a tick or a purchase.
    ("xp_events", SyncKind::Immutable),
    ("conversations", SyncKind::Lww),
    ("stories", SyncKind::Lww),
    ("souvenirs", SyncKind::Lww),
    // Ink
    ("notebooks", SyncKind::Lww),
    ("ink_documents", SyncKind::Lww),
    ("ink_strokes", SyncKind::Immutable),
    // Coach
    ("goals", SyncKind::Lww),
    ("daily_plans", SyncKind::Lww),
    ("reminders", SyncKind::Lww),
    ("reports", SyncKind::Lww),
    ("journal_entries", SyncKind::AppendOnly),
    ("coach_memory", SyncKind::Lww),
    ("skill_scores", SyncKind::Lww),
    // Account
    ("sessions", SyncKind::Lww),
    // Quality
    ("engine_runs", SyncKind::AppendOnly),
    ("quality_feedback", SyncKind::AppendOnly),
    // System
    ("settings", SyncKind::Lww),
    ("content_packs", SyncKind::Lww),
];

pub fn table_kind(table: &str) -> Option<SyncKind> {
    TABLES.iter().find(|(name, _)| *name == table).map(|(_, kind)| *kind)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    /// Create or replace a row (LWW tables).
    Upsert,
    /// Add a new row that is never changed afterwards.
    Append,
    /// Tombstone a row.
    Delete,
}

/// One local write, as it travels from a device's outbox to the hub and on to
/// the other devices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Op {
    /// Unique per op (UUIDv7); makes replays idempotent.
    pub id: String,
    pub table: String,
    pub row_id: String,
    pub kind: OpKind,
    /// When the write happened; `hlc.device` is the writing device.
    pub hlc: Hlc,
    /// The row's columns. `None` for deletes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// An op as stored by the hub, with its position in the account's log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeqOp {
    pub seq: u64,
    #[serde(flatten)]
    pub op: Op,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// First message on every connection.
    Hello {
        protocol: u32,
        /// Highest `seq` this device has applied; 0 for a fresh install.
        last_seq: u64,
    },
    /// Ops from the device's outbox, at most [`MAX_OPS_PER_PUSH`].
    Push {
        batch: u64,
        ops: Vec<Op>,
    },
    Ping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    UnknownTable,
    /// e.g. an `upsert` on an append-only table.
    KindNotAllowed,
    /// `hlc.device` is not the device that is connected.
    WrongDevice,
    MissingPayload,
    PayloadTooLarge,
    ClockSkew,
    InvalidId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rejected {
    pub op_id: String,
    pub reason: RejectReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncErrorCode {
    BadMessage,
    UnsupportedProtocol,
    TooManyOps,
    HelloFirst,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// A page of the account's log, oldest first. Live updates use the same
    /// message.
    Ops {
        ops: Vec<SeqOp>,
        /// The hub stopped sending before the head (one hello sends at most a
        /// few thousand ops): send [`ClientMsg::Hello`] again with the last
        /// `seq` applied. No [`ServerMsg::CaughtUp`] follows this page.
        more: bool,
    },
    /// Everything up to `head_seq` has been sent; live updates follow.
    CaughtUp {
        head_seq: u64,
    },
    /// Reply to a [`ClientMsg::Push`]. Ops in `done` can leave the outbox:
    /// they were stored, or were already known, or lost a last-writer-wins
    /// race to a newer write (which the device has or will receive).
    Ack {
        batch: u64,
        done: Vec<String>,
        rejected: Vec<Rejected>,
        head_seq: u64,
    },
    Pong,
    /// This device was signed out from another device. The socket closes next.
    Revoked,
    Error {
        code: SyncErrorCode,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeviceId;

    #[test]
    fn every_table_is_listed_once() {
        for (i, (name, _)) in TABLES.iter().enumerate() {
            assert!(TABLES[i + 1..].iter().all(|(other, _)| other != name), "{name} twice");
        }
        assert_eq!(table_kind("word_status"), Some(SyncKind::Lww));
        assert_eq!(table_kind("ink_strokes"), Some(SyncKind::Immutable));
        assert_eq!(table_kind("nope"), None);
    }

    #[test]
    fn messages_use_a_type_tag() {
        let op = Op {
            id: "op1".into(),
            table: "word_status".into(),
            row_id: "rumah".into(),
            kind: OpKind::Upsert,
            hlc: Hlc { wall_ms: 5, counter: 0, device: DeviceId("s24u".into()) },
            payload: Some(serde_json::json!({ "status": "learning" })),
        };
        let json =
            serde_json::to_value(ServerMsg::Ops { ops: vec![SeqOp { seq: 7, op }], more: false }).unwrap();
        assert_eq!(json["type"], "ops");
        assert_eq!(json["ops"][0]["seq"], 7);
        assert_eq!(json["ops"][0]["table"], "word_status");

        let hello: ClientMsg = serde_json::from_str(r#"{"type":"hello","protocol":1,"last_seq":0}"#).unwrap();
        assert_eq!(hello, ClientMsg::Hello { protocol: 1, last_seq: 0 });
    }
}
