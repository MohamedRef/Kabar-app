//! The device side of live sync, as a state machine with no I/O.
//!
//! The app's transport feeds it socket events and does what it returns:
//! send a message, store remote ops, drop acked ops from the outbox. Keeping
//! it pure means every ordering of messages can be tested without a socket.

use kabar_protocol::PROTOCOL_VERSION;
use kabar_protocol::sync::{ClientMsg, MAX_OPS_PER_PUSH, Op, Rejected, SeqOp, ServerMsg, SyncErrorCode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// No socket.
    Offline,
    /// Hello sent; receiving the ops this device missed.
    CatchingUp,
    /// Has the whole log; pushes the outbox and receives live ops.
    Live,
    /// Signed out from another device. Nothing more until sign-in.
    Revoked,
    /// The server speaks a newer protocol. Nothing more until an update.
    Outdated,
}

/// What the transport must do, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Send(ClientMsg),
    /// Merge these ops into the store and save `last_seq`, in one transaction.
    Apply {
        ops: Vec<SeqOp>,
        last_seq: u64,
    },
    /// Save `last_seq` (nothing to merge).
    SaveLastSeq(u64),
    /// Remove these ops from the outbox. Rejected ops will never be accepted,
    /// so they leave too; the reasons are for the log.
    Acked {
        done: Vec<String>,
        rejected: Vec<Rejected>,
    },
    /// Close the socket and reconnect after a backoff.
    Reconnect {
        reason: String,
    },
    /// Close the socket and stop: the user has to act (sign in, update).
    Stop,
}

#[derive(Clone, Debug)]
struct InFlight {
    batch: u64,
    op_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SyncClient {
    phase: Phase,
    last_seq: u64,
    next_batch: u64,
    in_flight: Option<InFlight>,
}

impl SyncClient {
    /// `last_seq` is the highest server seq this device has stored.
    pub fn new(last_seq: u64) -> Self {
        Self { phase: Phase::Offline, last_seq, next_batch: 1, in_flight: None }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// The socket is open.
    pub fn on_open(&mut self) -> Vec<Effect> {
        self.phase = Phase::CatchingUp;
        self.in_flight = None;
        vec![self.hello()]
    }

    /// The socket closed or failed. A push in flight is resent next time
    /// (the hub ignores ops it already has).
    pub fn on_close(&mut self) {
        if matches!(self.phase, Phase::CatchingUp | Phase::Live) {
            self.phase = Phase::Offline;
        }
        self.in_flight = None;
    }

    /// One message from the hub.
    pub fn on_message(&mut self, msg: ServerMsg) -> Vec<Effect> {
        match msg {
            ServerMsg::Ops { ops, more } => {
                let mut out = Vec::new();
                if let Some(top) = ops.iter().map(|o| o.seq).max() {
                    self.last_seq = self.last_seq.max(top);
                    out.push(Effect::Apply { ops, last_seq: self.last_seq });
                }
                if more {
                    out.push(self.hello());
                }
                out
            }
            ServerMsg::CaughtUp { head_seq } => {
                self.phase = Phase::Live;
                if head_seq > self.last_seq {
                    // Only seqs without a stored op (none today) could be missing.
                    self.last_seq = head_seq;
                    vec![Effect::SaveLastSeq(head_seq)]
                } else {
                    vec![]
                }
            }
            ServerMsg::Ack { batch, done, rejected, head_seq } => {
                if self.in_flight.as_ref().is_some_and(|f| f.batch == batch) {
                    self.in_flight = None;
                }
                let mut out = vec![Effect::Acked { done, rejected }];
                // Live sockets get every other op before the ack of a later
                // push, so everything up to head_seq is here now.
                if self.phase == Phase::Live && head_seq > self.last_seq {
                    self.last_seq = head_seq;
                    out.push(Effect::SaveLastSeq(head_seq));
                }
                out
            }
            ServerMsg::Pong => vec![],
            ServerMsg::Revoked => {
                self.phase = Phase::Revoked;
                self.in_flight = None;
                vec![Effect::Stop]
            }
            ServerMsg::Error { code, message } => {
                self.in_flight = None;
                if code == SyncErrorCode::UnsupportedProtocol {
                    self.phase = Phase::Outdated;
                    vec![Effect::Stop]
                } else {
                    vec![Effect::Reconnect { reason: format!("{code:?}: {message}") }]
                }
            }
        }
    }

    /// The next push, if the socket is live and nothing is in flight.
    /// `outbox` is the oldest pending ops first; at most
    /// [`MAX_OPS_PER_PUSH`] are taken.
    pub fn next_push(&mut self, outbox: &[Op]) -> Option<ClientMsg> {
        if self.phase != Phase::Live || self.in_flight.is_some() || outbox.is_empty() {
            return None;
        }
        let ops: Vec<Op> = outbox.iter().take(MAX_OPS_PER_PUSH).cloned().collect();
        let batch = self.next_batch;
        self.next_batch += 1;
        self.in_flight = Some(InFlight { batch, op_ids: ops.iter().map(|o| o.id.clone()).collect() });
        Some(ClientMsg::Push { batch, ops })
    }

    /// Op ids of the push waiting for its ack.
    pub fn in_flight(&self) -> &[String] {
        self.in_flight.as_ref().map(|f| f.op_ids.as_slice()).unwrap_or_default()
    }

    fn hello(&self) -> Effect {
        Effect::Send(ClientMsg::Hello { protocol: PROTOCOL_VERSION, last_seq: self.last_seq })
    }
}

/// Reconnect delays: 1 s, 2 s, 4 s … up to 5 minutes, with jitter so two
/// devices don't retry in lockstep.
#[derive(Clone, Debug, Default)]
pub struct Backoff {
    attempt: u32,
}

impl Backoff {
    pub const MAX_MS: u64 = 5 * 60 * 1000;

    /// Delay before the next attempt. `jitter` is uniform in `0.0..1.0`.
    pub fn next_delay_ms(&mut self, jitter: f64) -> u64 {
        let base = 1000u64.saturating_mul(1 << self.attempt.min(20)).min(Self::MAX_MS);
        self.attempt += 1;
        // 50–100 % of the base.
        (base as f64 * (0.5 + jitter.clamp(0.0, 1.0) / 2.0)) as u64
    }

    /// A connection reached `Live`.
    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::sync::{OpKind, RejectReason};
    use kabar_protocol::{DeviceId, Hlc};
    use serde_json::json;

    use super::*;

    fn op(id: &str) -> Op {
        Op {
            id: id.into(),
            table: "word_status".into(),
            row_id: "id:rumah".into(),
            kind: OpKind::Upsert,
            hlc: Hlc { wall_ms: 1, counter: 0, device: DeviceId("s24u".into()) },
            payload: Some(json!({ "lexeme": "rumah", "lang": "id", "status": "learning" })),
        }
    }

    fn seq(n: u64) -> SeqOp {
        SeqOp { seq: n, op: op(&format!("o{n}")) }
    }

    fn hello(last_seq: u64) -> Effect {
        Effect::Send(ClientMsg::Hello { protocol: PROTOCOL_VERSION, last_seq })
    }

    fn live(last_seq: u64) -> SyncClient {
        let mut c = SyncClient::new(last_seq);
        c.on_open();
        c.on_message(ServerMsg::CaughtUp { head_seq: last_seq });
        c
    }

    #[test]
    fn hello_catch_up_then_live() {
        let mut c = SyncClient::new(4);
        assert_eq!(c.on_open(), vec![hello(4)]);
        assert_eq!(c.phase(), Phase::CatchingUp);
        // Nothing is pushed before the device has the whole log.
        assert_eq!(c.next_push(&[op("a")]), None);

        let fx = c.on_message(ServerMsg::Ops { ops: vec![seq(5), seq(6)], more: false });
        assert_eq!(fx, vec![Effect::Apply { ops: vec![seq(5), seq(6)], last_seq: 6 }]);
        assert_eq!(c.on_message(ServerMsg::CaughtUp { head_seq: 6 }), vec![]);
        assert_eq!(c.phase(), Phase::Live);
        assert!(c.next_push(&[op("a")]).is_some());
    }

    #[test]
    fn a_capped_burst_says_hello_again() {
        let mut c = SyncClient::new(0);
        c.on_open();
        let fx = c.on_message(ServerMsg::Ops { ops: vec![seq(1), seq(2)], more: true });
        assert_eq!(fx[1], hello(2));
        assert_eq!(c.phase(), Phase::CatchingUp);
    }

    #[test]
    fn one_push_in_flight_at_a_time() {
        let mut c = live(0);
        let outbox: Vec<Op> = (0..MAX_OPS_PER_PUSH + 3).map(|i| op(&format!("p{i}"))).collect();
        let Some(ClientMsg::Push { batch: 1, ops }) = c.next_push(&outbox) else { panic!() };
        assert_eq!(ops.len(), MAX_OPS_PER_PUSH);
        assert_eq!(c.next_push(&outbox), None);

        let rejected = vec![Rejected { op_id: "p1".into(), reason: RejectReason::ClockSkew }];
        let done: Vec<String> = ops.iter().skip(2).map(|o| o.id.clone()).collect();
        let fx = c.on_message(ServerMsg::Ack {
            batch: 1,
            done: done.clone(),
            rejected: rejected.clone(),
            head_seq: 9,
        });
        assert_eq!(fx, vec![Effect::Acked { done, rejected }, Effect::SaveLastSeq(9)]);
        let Some(ClientMsg::Push { batch: 2, .. }) = c.next_push(&outbox[MAX_OPS_PER_PUSH..]) else {
            panic!()
        };
    }

    #[test]
    fn a_dropped_socket_resends_the_push() {
        let mut c = live(3);
        c.next_push(&[op("a")]).unwrap();
        assert_eq!(c.in_flight(), ["a".to_string()]);
        c.on_close();
        assert_eq!(c.phase(), Phase::Offline);
        assert_eq!(c.on_open(), vec![hello(3)]);
        c.on_message(ServerMsg::CaughtUp { head_seq: 3 });
        assert!(c.next_push(&[op("a")]).is_some());
    }

    #[test]
    fn live_ops_move_last_seq() {
        let mut c = live(2);
        let fx = c.on_message(ServerMsg::Ops { ops: vec![seq(3)], more: false });
        assert_eq!(fx, vec![Effect::Apply { ops: vec![seq(3)], last_seq: 3 }]);
        // An old page never moves it back.
        c.on_message(ServerMsg::Ops { ops: vec![seq(1)], more: false });
        assert_eq!(c.last_seq(), 3);
    }

    #[test]
    fn revoked_and_outdated_stop() {
        let mut c = live(0);
        assert_eq!(c.on_message(ServerMsg::Revoked), vec![Effect::Stop]);
        assert_eq!(c.phase(), Phase::Revoked);
        c.on_close();
        assert_eq!(c.phase(), Phase::Revoked);

        let mut c = SyncClient::new(0);
        c.on_open();
        let fx = c.on_message(ServerMsg::Error {
            code: SyncErrorCode::UnsupportedProtocol,
            message: "server speaks 2".into(),
        });
        assert_eq!(fx, vec![Effect::Stop]);
        assert_eq!(c.phase(), Phase::Outdated);

        let mut c = live(0);
        let fx = c.on_message(ServerMsg::Error { code: SyncErrorCode::Internal, message: "db".into() });
        assert!(matches!(fx[0], Effect::Reconnect { .. }));
    }

    #[test]
    fn backoff_doubles_up_to_five_minutes() {
        let mut b = Backoff::default();
        assert_eq!(b.next_delay_ms(1.0), 1000);
        assert_eq!(b.next_delay_ms(1.0), 2000);
        assert_eq!(b.next_delay_ms(0.0), 2000);
        for _ in 0..30 {
            b.next_delay_ms(1.0);
        }
        assert_eq!(b.next_delay_ms(1.0), Backoff::MAX_MS);
        b.reset();
        assert_eq!(b.next_delay_ms(1.0), 1000);
    }
}
