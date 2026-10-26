//! The per-account sync hub: an op log with a monotonically increasing `seq`,
//! plus the current version of every row so merges are O(1). Runs inside the
//! account's Durable Object; the WebSocket plumbing lives in `wasm::hub_do`.

use kabar_core::sync::{Merge, RowMeta, merge, validate};
use kabar_protocol::sync::{
    ClientMsg, MAX_CLOCK_SKEW_MS, MAX_OPS_PER_PUSH, MAX_PAYLOAD_BYTES, OPS_PAGE_SIZE, Op, OpKind,
    RejectReason, Rejected, SeqOp, ServerMsg, SyncErrorCode,
};
use kabar_protocol::{DeviceId, Hlc, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};

use crate::sql::{Row, Sql, SqlError, Value, col};

/// Pages sent per `hello` before the device has to ask again. Keeps one
/// Durable Object event small even for a fresh install after a long time.
pub const MAX_PAGES_PER_HELLO: usize = 25;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ops (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    op_id TEXT NOT NULL UNIQUE,
    tbl TEXT NOT NULL,
    row_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    wall_ms INTEGER NOT NULL,
    counter INTEGER NOT NULL,
    device TEXT NOT NULL,
    payload TEXT,
    received_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS row_state (
    tbl TEXT NOT NULL,
    row_id TEXT NOT NULL,
    wall_ms INTEGER NOT NULL,
    counter INTEGER NOT NULL,
    device TEXT NOT NULL,
    deleted INTEGER NOT NULL,
    seq INTEGER NOT NULL,
    PRIMARY KEY (tbl, row_id)
)";

/// Per-connection state, kept as the WebSocket's attachment so it survives
/// hibernation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conn {
    pub device: DeviceId,
    /// Random per connection, to tell two sockets of one device apart.
    pub conn_id: String,
    pub ready: bool,
}

/// What to do after one client message.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    /// Send to the connection that sent the message, in order.
    pub replies: Vec<ServerMsg>,
    /// Send to every other ready connection of the account.
    pub broadcast: Vec<SeqOp>,
    /// Ops moved (for the Devices screen's "last sync").
    pub synced: bool,
}

fn kind_str(k: OpKind) -> &'static str {
    match k {
        OpKind::Upsert => "upsert",
        OpKind::Append => "append",
        OpKind::Delete => "delete",
    }
}

fn kind_from(s: &str) -> Result<OpKind, SqlError> {
    match s {
        "upsert" => Ok(OpKind::Upsert),
        "append" => Ok(OpKind::Append),
        "delete" => Ok(OpKind::Delete),
        other => Err(SqlError(format!("bad kind {other}"))),
    }
}

fn valid_op_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn valid_row_id(id: &str) -> bool {
    (1..=200).contains(&id.len()) && !id.chars().any(char::is_control)
}

fn error(code: SyncErrorCode, message: impl Into<String>) -> ServerMsg {
    ServerMsg::Error { code, message: message.into() }
}

pub struct Hub<'a, S: Sql> {
    db: &'a S,
}

impl<'a, S: Sql> Hub<'a, S> {
    /// Creates the tables if needed, then opens.
    pub fn new(db: &'a S) -> Result<Self, SqlError> {
        db.exec_batch(SCHEMA)?;
        Ok(Self::open(db))
    }

    /// Opens without touching the schema (the Durable Object migrates once at start).
    pub fn open(db: &'a S) -> Self {
        Self { db }
    }

    pub fn head_seq(&self) -> Result<u64, SqlError> {
        let rows = self.db.exec("SELECT COALESCE(MAX(seq), 0) FROM ops", &[])?;
        Ok(rows.first().and_then(|r| r.first()).and_then(Value::as_u64).unwrap_or(0))
    }

    /// Handles one text frame from a device.
    pub fn handle(&self, conn: &mut Conn, text: &str, now_ms: u64) -> Result<Outcome, SqlError> {
        let msg: ClientMsg = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                return Ok(Outcome {
                    replies: vec![error(SyncErrorCode::BadMessage, e.to_string())],
                    ..Default::default()
                });
            }
        };
        match msg {
            ClientMsg::Ping => Ok(Outcome { replies: vec![ServerMsg::Pong], ..Default::default() }),
            ClientMsg::Hello { protocol, last_seq } => {
                if protocol != PROTOCOL_VERSION {
                    let msg = format!("server speaks protocol {PROTOCOL_VERSION}, device sent {protocol}");
                    return Ok(Outcome {
                        replies: vec![error(SyncErrorCode::UnsupportedProtocol, msg)],
                        ..Default::default()
                    });
                }
                let replies = self.catch_up(last_seq)?;
                // Live ops only after the device has the whole log, so it can
                // keep `last_seq` as the highest seq it has seen.
                conn.ready = matches!(replies.last(), Some(ServerMsg::CaughtUp { .. }));
                let synced =
                    replies.iter().any(|m| matches!(m, ServerMsg::Ops { ops, .. } if !ops.is_empty()));
                Ok(Outcome { replies, broadcast: vec![], synced })
            }
            ClientMsg::Push { batch, ops } => {
                if !conn.ready {
                    return Ok(Outcome {
                        replies: vec![error(SyncErrorCode::HelloFirst, "send hello first")],
                        ..Default::default()
                    });
                }
                if ops.len() > MAX_OPS_PER_PUSH {
                    let msg = format!("at most {MAX_OPS_PER_PUSH} ops per push");
                    return Ok(Outcome {
                        replies: vec![error(SyncErrorCode::TooManyOps, msg)],
                        ..Default::default()
                    });
                }
                let (ack, applied) = self.push(&conn.device, batch, ops, now_ms)?;
                Ok(Outcome { replies: vec![ack], synced: true, broadcast: applied })
            }
        }
    }

    /// Ops after `after_seq`, then `caught_up` (or `more: true` when the
    /// device should send another `hello` to continue).
    pub fn catch_up(&self, after_seq: u64) -> Result<Vec<ServerMsg>, SqlError> {
        let mut out = Vec::new();
        let mut cursor = after_seq;
        for page in 0..MAX_PAGES_PER_HELLO {
            let (ops, more) = self.page(cursor, OPS_PAGE_SIZE)?;
            if let Some(last) = ops.last() {
                cursor = last.seq;
            }
            if !more {
                if !ops.is_empty() {
                    out.push(ServerMsg::Ops { ops, more: false });
                }
                out.push(ServerMsg::CaughtUp { head_seq: cursor.max(self.head_seq()?) });
                return Ok(out);
            }
            // `more` only on the last page of a capped burst: the device sends
            // hello again from there.
            out.push(ServerMsg::Ops { ops, more: page + 1 == MAX_PAGES_PER_HELLO });
        }
        Ok(out)
    }

    fn page(&self, after_seq: u64, limit: usize) -> Result<(Vec<SeqOp>, bool), SqlError> {
        let rows = self.db.exec(
            "SELECT seq, op_id, tbl, row_id, kind, wall_ms, counter, device, payload
             FROM ops WHERE seq > ? ORDER BY seq LIMIT ?",
            &[after_seq.into(), (limit as u64 + 1).into()],
        )?;
        let more = rows.len() > limit;
        let ops = rows.iter().take(limit).map(row_to_op).collect::<Result<_, _>>()?;
        Ok((ops, more))
    }

    /// Applies a batch from `device`. Returns the ack and the ops that changed
    /// state (to broadcast).
    pub fn push(
        &self,
        device: &DeviceId,
        batch: u64,
        ops: Vec<Op>,
        now_ms: u64,
    ) -> Result<(ServerMsg, Vec<SeqOp>), SqlError> {
        let mut done = Vec::new();
        let mut rejected = Vec::new();
        let mut applied = Vec::new();
        for op in ops {
            match self.apply(device, op, now_ms)? {
                Ok((id, Some(seq_op))) => {
                    done.push(id);
                    applied.push(seq_op);
                }
                Ok((id, None)) => done.push(id),
                Err(r) => rejected.push(r),
            }
        }
        let ack = ServerMsg::Ack { batch, done, rejected, head_seq: self.head_seq()? };
        Ok((ack, applied))
    }

    #[allow(clippy::type_complexity)]
    fn apply(
        &self,
        device: &DeviceId,
        op: Op,
        now_ms: u64,
    ) -> Result<Result<(String, Option<SeqOp>), Rejected>, SqlError> {
        let reject = |op: &Op, reason| Ok(Err(Rejected { op_id: op.id.clone(), reason }));
        if !valid_op_id(&op.id) || !valid_row_id(&op.row_id) {
            return reject(&op, RejectReason::InvalidId);
        }
        if op.hlc.device != *device {
            return reject(&op, RejectReason::WrongDevice);
        }
        if op.hlc.wall_ms > now_ms + MAX_CLOCK_SKEW_MS {
            return reject(&op, RejectReason::ClockSkew);
        }
        let sync = match validate(&op, MAX_PAYLOAD_BYTES) {
            Ok(s) => s,
            Err(reason) => return reject(&op, reason),
        };
        if !self.db.exec("SELECT 1 FROM ops WHERE op_id = ?", &[op.id.as_str().into()])?.is_empty() {
            return Ok(Ok((op.id, None)));
        }
        let existing = self
            .db
            .exec(
                "SELECT wall_ms, counter, device, deleted FROM row_state WHERE tbl = ? AND row_id = ?",
                &[op.table.as_str().into(), op.row_id.as_str().into()],
            )?
            .first()
            .map(|r| -> Result<RowMeta, SqlError> {
                Ok(RowMeta {
                    hlc: Hlc {
                        wall_ms: col(r, 0, "wall_ms")?.as_u64().unwrap_or(0),
                        counter: col(r, 1, "counter")?.as_u64().unwrap_or(0) as u32,
                        device: DeviceId(col(r, 2, "device")?.as_str().unwrap_or_default().to_owned()),
                    },
                    deleted: col(r, 3, "deleted")?.as_i64() == Some(1),
                })
            })
            .transpose()?;
        if merge(sync, &op, existing.as_ref()) == Merge::Ignore {
            return Ok(Ok((op.id, None)));
        }

        let payload = op.payload.as_ref().map(|p| p.to_string());
        let seq = self
            .db
            .exec(
                "INSERT INTO ops (op_id, tbl, row_id, kind, wall_ms, counter, device, payload, received_ms)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING seq",
                &[
                    op.id.as_str().into(),
                    op.table.as_str().into(),
                    op.row_id.as_str().into(),
                    kind_str(op.kind).into(),
                    op.hlc.wall_ms.into(),
                    u64::from(op.hlc.counter).into(),
                    op.hlc.device.0.as_str().into(),
                    payload.into(),
                    now_ms.into(),
                ],
            )?
            .first()
            .and_then(|r| r.first())
            .and_then(Value::as_u64)
            .ok_or_else(|| SqlError("insert returned no seq".into()))?;
        self.db.exec(
            "INSERT INTO row_state (tbl, row_id, wall_ms, counter, device, deleted, seq)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (tbl, row_id) DO UPDATE SET wall_ms = excluded.wall_ms, counter = excluded.counter,
                device = excluded.device, deleted = excluded.deleted, seq = excluded.seq",
            &[
                op.table.as_str().into(),
                op.row_id.as_str().into(),
                op.hlc.wall_ms.into(),
                u64::from(op.hlc.counter).into(),
                op.hlc.device.0.as_str().into(),
                (op.kind == OpKind::Delete).into(),
                seq.into(),
            ],
        )?;
        let id = op.id.clone();
        Ok(Ok((id, Some(SeqOp { seq, op }))))
    }
}

fn row_to_op(r: &Row) -> Result<SeqOp, SqlError> {
    let text = |i, name| col(r, i, name).map(|v| v.as_str().unwrap_or_default().to_owned());
    let payload = match col(r, 8, "payload")? {
        Value::Text(s) => Some(serde_json::from_str(s).map_err(|e| SqlError(e.to_string()))?),
        _ => None,
    };
    Ok(SeqOp {
        seq: col(r, 0, "seq")?.as_u64().unwrap_or(0),
        op: Op {
            id: text(1, "op_id")?,
            table: text(2, "tbl")?,
            row_id: text(3, "row_id")?,
            kind: kind_from(col(r, 4, "kind")?.as_str().unwrap_or_default())?,
            hlc: Hlc {
                wall_ms: col(r, 5, "wall_ms")?.as_u64().unwrap_or(0),
                counter: col(r, 6, "counter")?.as_u64().unwrap_or(0) as u32,
                device: DeviceId(text(7, "device")?),
            },
            payload,
        },
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::sql::testing::MemSql;

    const NOW: u64 = 1_793_011_856_000;

    fn conn(device: &str) -> Conn {
        Conn { device: DeviceId(device.into()), conn_id: format!("c-{device}"), ready: false }
    }

    fn word(id: &str, device: &str, wall_ms: u64, word: &str, status: &str) -> Op {
        Op {
            id: id.into(),
            table: "word_status".into(),
            row_id: word.into(),
            kind: OpKind::Upsert,
            hlc: Hlc { wall_ms, counter: 0, device: DeviceId(device.into()) },
            payload: Some(json!({ "lexeme": word, "status": status })),
        }
    }

    fn send(hub: &Hub<'_, MemSql>, c: &mut Conn, msg: &ClientMsg) -> Outcome {
        hub.handle(c, &serde_json::to_string(msg).unwrap(), NOW).unwrap()
    }

    fn hello(last_seq: u64) -> ClientMsg {
        ClientMsg::Hello { protocol: PROTOCOL_VERSION, last_seq }
    }

    /// M2's acceptance test: a word saved on the phone reaches the tablet.
    #[test]
    fn word_saved_on_phone_appears_on_tablet() {
        let db = MemSql::new();
        let hub = Hub::new(&db).unwrap();
        let mut phone = conn("s24u");
        let mut tablet = conn("tab-s11");
        assert_eq!(send(&hub, &mut phone, &hello(0)).replies, vec![ServerMsg::CaughtUp { head_seq: 0 }]);
        send(&hub, &mut tablet, &hello(0));

        let op = word("op-1", "s24u", NOW, "rumah", "learning");
        let out = send(&hub, &mut phone, &ClientMsg::Push { batch: 1, ops: vec![op.clone()] });
        assert_eq!(
            out.replies,
            vec![ServerMsg::Ack { batch: 1, done: vec!["op-1".into()], rejected: vec![], head_seq: 1 }]
        );
        // The Durable Object forwards this to the tablet's socket right away.
        assert_eq!(out.broadcast, vec![SeqOp { seq: 1, op: op.clone() }]);

        // A third device that was offline catches up on connect.
        let mut mac = conn("mac");
        let out = send(&hub, &mut mac, &hello(0));
        assert_eq!(
            out.replies,
            vec![
                ServerMsg::Ops { ops: vec![SeqOp { seq: 1, op }], more: false },
                ServerMsg::CaughtUp { head_seq: 1 }
            ]
        );
        assert!(out.synced);
    }

    #[test]
    fn replays_are_idempotent() {
        let db = MemSql::new();
        let hub = Hub::new(&db).unwrap();
        let mut phone = conn("s24u");
        send(&hub, &mut phone, &hello(0));
        let op = word("op-1", "s24u", NOW, "rumah", "learning");
        send(&hub, &mut phone, &ClientMsg::Push { batch: 1, ops: vec![op.clone()] });
        // Lost ack → the outbox sends it again.
        let out = send(&hub, &mut phone, &ClientMsg::Push { batch: 2, ops: vec![op.clone(), op] });
        assert_eq!(
            out.replies,
            vec![ServerMsg::Ack {
                batch: 2,
                done: vec!["op-1".into(), "op-1".into()],
                rejected: vec![],
                head_seq: 1
            }]
        );
        assert!(out.broadcast.is_empty());
    }

    #[test]
    fn concurrent_edits_converge_on_the_newest() {
        let db = MemSql::new();
        let hub = Hub::new(&db).unwrap();
        let mut phone = conn("s24u");
        let mut tablet = conn("tab-s11");
        send(&hub, &mut phone, &hello(0));
        send(&hub, &mut tablet, &hello(0));
        // The tablet's later edit arrives first (the phone was offline).
        let newer = word("t-1", "tab-s11", NOW + 5, "sega", "known");
        let older = word("p-1", "s24u", NOW, "sega", "learning");
        assert_eq!(
            send(&hub, &mut tablet, &ClientMsg::Push { batch: 1, ops: vec![newer] }).broadcast.len(),
            1
        );
        let out = send(&hub, &mut phone, &ClientMsg::Push { batch: 1, ops: vec![older] });
        // Done (drop it from the outbox) but not stored: it lost.
        assert!(matches!(&out.replies[0], ServerMsg::Ack { done, .. } if done == &["p-1".to_owned()]));
        assert!(out.broadcast.is_empty());
        assert_eq!(hub.head_seq().unwrap(), 1);
    }

    #[test]
    fn bad_ops_are_rejected_with_a_reason() {
        let db = MemSql::new();
        let hub = Hub::new(&db).unwrap();
        let mut phone = conn("s24u");
        send(&hub, &mut phone, &hello(0));

        let spoofed = word("a", "tab-s11", NOW, "rumah", "known");
        let future = word("b", "s24u", NOW + MAX_CLOCK_SKEW_MS + 1, "rumah", "known");
        let mut unknown = word("c", "s24u", NOW, "rumah", "known");
        unknown.table = "secrets".into();
        let mut review_edit = word("d", "s24u", NOW, "r1", "known");
        review_edit.table = "review_log".into();
        let bad_id = word("e f", "s24u", NOW, "rumah", "known");
        let out = send(
            &hub,
            &mut phone,
            &ClientMsg::Push { batch: 9, ops: vec![spoofed, future, unknown, review_edit, bad_id] },
        );
        let ServerMsg::Ack { done, rejected, .. } = &out.replies[0] else { panic!("{out:?}") };
        assert!(done.is_empty());
        let reasons: Vec<_> = rejected.iter().map(|r| r.reason).collect();
        assert_eq!(
            reasons,
            vec![
                RejectReason::WrongDevice,
                RejectReason::ClockSkew,
                RejectReason::UnknownTable,
                RejectReason::KindNotAllowed,
                RejectReason::InvalidId
            ]
        );
    }

    #[test]
    fn protocol_errors() {
        let db = MemSql::new();
        let hub = Hub::new(&db).unwrap();
        let mut phone = conn("s24u");
        let push = ClientMsg::Push { batch: 1, ops: vec![] };
        assert!(matches!(
            send(&hub, &mut phone, &push).replies[0],
            ServerMsg::Error { code: SyncErrorCode::HelloFirst, .. }
        ));
        assert!(matches!(
            send(&hub, &mut phone, &ClientMsg::Hello { protocol: 99, last_seq: 0 }).replies[0],
            ServerMsg::Error { code: SyncErrorCode::UnsupportedProtocol, .. }
        ));
        assert!(matches!(
            hub.handle(&mut phone, "{not json", NOW).unwrap().replies[0],
            ServerMsg::Error { code: SyncErrorCode::BadMessage, .. }
        ));
        send(&hub, &mut phone, &hello(0));
        let many =
            (0..=MAX_OPS_PER_PUSH).map(|i| word(&format!("o{i}"), "s24u", NOW, "x", "known")).collect();
        assert!(matches!(
            send(&hub, &mut phone, &ClientMsg::Push { batch: 1, ops: many }).replies[0],
            ServerMsg::Error { code: SyncErrorCode::TooManyOps, .. }
        ));
        assert_eq!(send(&hub, &mut phone, &ClientMsg::Ping).replies, vec![ServerMsg::Pong]);
    }

    #[test]
    fn catch_up_pages_and_resumes() {
        let db = MemSql::new();
        let hub = Hub::new(&db).unwrap();
        let mut phone = conn("s24u");
        send(&hub, &mut phone, &hello(0));
        let total = OPS_PAGE_SIZE * MAX_PAGES_PER_HELLO + 10;
        for chunk in (0..total).collect::<Vec<_>>().chunks(MAX_OPS_PER_PUSH) {
            let ops = chunk
                .iter()
                .map(|i| word(&format!("o{i}"), "s24u", NOW, &format!("w{i}"), "known"))
                .collect();
            send(&hub, &mut phone, &ClientMsg::Push { batch: 1, ops });
        }
        let replies = hub.catch_up(0).unwrap();
        assert_eq!(replies.len(), MAX_PAGES_PER_HELLO);
        assert!(matches!(&replies[0], ServerMsg::Ops { more: false, .. }));
        let Some(ServerMsg::Ops { ops, more: true }) = replies.last() else { panic!() };
        let resume = ops.last().unwrap().seq;
        // Not live until it has everything.
        let mut tablet = conn("tab");
        send(&hub, &mut tablet, &hello(0));
        assert!(!tablet.ready);
        send(&hub, &mut tablet, &hello(resume));
        assert!(tablet.ready);
        let rest = hub.catch_up(resume).unwrap();
        assert!(matches!(&rest[0], ServerMsg::Ops { ops, more: false } if ops.len() == 10));
        assert_eq!(rest[1], ServerMsg::CaughtUp { head_seq: total as u64 });
    }

    #[test]
    fn strokes_and_tombstones_round_trip() {
        let db = MemSql::new();
        let hub = Hub::new(&db).unwrap();
        let mut tab = conn("tab");
        send(&hub, &mut tab, &hello(0));
        let stroke = Op {
            id: "s1".into(),
            table: "ink_strokes".into(),
            row_id: "stroke-1".into(),
            kind: OpKind::Append,
            hlc: Hlc { wall_ms: NOW, counter: 0, device: DeviceId("tab".into()) },
            payload: Some(json!({ "points": [[0.0, 0.0, 0.5, 0]] })),
        };
        let erase = Op { id: "s2".into(), kind: OpKind::Delete, payload: None, ..stroke.clone() };
        let out = send(&hub, &mut tab, &ClientMsg::Push { batch: 1, ops: vec![stroke, erase] });
        assert_eq!(out.broadcast.len(), 2);
        let ServerMsg::Ops { ops, .. } = &hub.catch_up(0).unwrap()[0] else { panic!() };
        assert_eq!(ops[1].op.kind, OpKind::Delete);
        assert_eq!(ops[1].op.payload, None);
        assert_eq!(ops[0].op.payload.as_ref().unwrap()["points"][0][2], 0.5);
    }
}
