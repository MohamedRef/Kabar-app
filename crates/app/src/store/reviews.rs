//! Review cards (last-writer-wins by HLC) and the review log (append-only).
//! Card state is never stored: `kabar_core::review::Deck` replays the log.

use kabar_protocol::Hlc;
use kabar_protocol::srs::{Card, Rating, ReviewEvent, ReviewMode};
use rusqlite::params;

use super::{Result, Store, now_ms};

impl Store {
    /// Inserts or replaces a card with a fresh HLC.
    pub fn upsert_card(&mut self, card: &Card) -> Result<Hlc> {
        let hlc = self.clock.tick(now_ms());
        self.conn.execute(
            "INSERT INTO cards (id, payload, device_id, hlc_wall, hlc_counter, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)
             ON CONFLICT(id) DO UPDATE SET
                payload = excluded.payload, device_id = excluded.device_id,
                hlc_wall = excluded.hlc_wall, hlc_counter = excluded.hlc_counter, deleted = 0",
            params![card.id, serde_json::to_string(card)?, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
        Ok(hlc)
    }

    /// Upserts several cards in one transaction, keeping their order.
    pub fn upsert_cards(&mut self, cards: &[Card]) -> Result<()> {
        self.conn.execute_batch("BEGIN")?;
        let result = cards.iter().try_for_each(|c| self.upsert_card(c).map(drop));
        self.conn.execute_batch(if result.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        result
    }

    /// Live cards in the order they were added. A payload this build can't
    /// read is skipped and reported, so one bad row never blocks reviewing.
    pub fn cards(&self) -> Result<Vec<Card>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, payload FROM cards WHERE deleted = 0
             ORDER BY hlc_wall, hlc_counter, device_id, id",
        )?;
        let rows: Vec<(String, String)> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, payload)| match serde_json::from_str(&payload) {
                Ok(card) => Some(card),
                Err(e) => {
                    eprintln!("kabar: skipping unreadable card {id}: {e}");
                    None
                }
            })
            .collect())
    }

    /// A new review event for this device: uuid id, HLC from the store clock.
    pub fn new_review(
        &mut self,
        card_id: &str,
        rating: Rating,
        at_ms: u64,
        duration_ms: u32,
        mode: ReviewMode,
    ) -> ReviewEvent {
        ReviewEvent {
            id: uuid::Uuid::new_v4().to_string(),
            card_id: card_id.into(),
            hlc: self.clock.tick(at_ms),
            at_ms,
            rating,
            duration_ms,
            mode,
        }
    }

    /// Appends an event to the log. Re-appending the same id is a no-op, so
    /// replays and sync can deliver an event more than once.
    pub fn append_review(&mut self, event: &ReviewEvent) -> Result<()> {
        if event.hlc.device != self.device {
            self.clock.observe(&event.hlc, now_ms());
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO review_log (id, card_id, payload, device_id, hlc_wall, hlc_counter)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.id,
                event.card_id,
                serde_json::to_string(event)?,
                event.hlc.device.0,
                event.hlc.wall_ms as i64,
                event.hlc.counter
            ],
        )?;
        Ok(())
    }

    /// Creates and appends a review event in one step.
    pub fn record_review(
        &mut self,
        card_id: &str,
        rating: Rating,
        at_ms: u64,
        duration_ms: u32,
        mode: ReviewMode,
    ) -> Result<ReviewEvent> {
        let event = self.new_review(card_id, rating, at_ms, duration_ms, mode);
        self.append_review(&event)?;
        Ok(event)
    }

    /// The whole log in HLC order, the order every device replays it in.
    pub fn review_log(&self) -> Result<Vec<ReviewEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT payload FROM review_log WHERE deleted = 0
             ORDER BY hlc_wall, hlc_counter, device_id",
        )?;
        let rows: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        rows.iter().map(|p| Ok(serde_json::from_str(p)?)).collect()
    }

    /// In-session undo of an answer just given on this device. Until sync
    /// exists the event has never left the device, so deleting the local row
    /// is honest. Once M2 sync ships an event, undo must append a tombstone
    /// event instead (the log stays append-only across devices).
    pub fn undo_review(&mut self, event_id: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM review_log WHERE id = ?1 AND device_id = ?2",
            params![event_id, self.device.0],
        )?;
        Ok(n > 0)
    }
}

#[cfg(test)]
mod tests {
    use kabar_core::review::seed_bridge_deck;
    use kabar_protocol::DeviceId;

    use super::*;

    #[test]
    fn cards_upsert_keeps_order_and_replaces() {
        let mut store = Store::open_in_memory().unwrap();
        let deck = seed_bridge_deck();
        store.upsert_cards(&deck).unwrap();
        store.upsert_cards(&deck).unwrap();
        let cards = store.cards().unwrap();
        assert_eq!(cards.len(), deck.len(), "seeding twice never duplicates");
        assert_eq!(cards[0].id, deck[0].id);

        let mut kabar = deck[0].clone();
        kabar.verified = true;
        store.upsert_card(&kabar).unwrap();
        assert!(store.cards().unwrap().iter().any(|c| c.id == kabar.id && c.verified));
    }

    #[test]
    fn review_log_is_in_hlc_order_and_idempotent() {
        let mut store = Store::open_in_memory().unwrap();
        let a = store.record_review("kabar", Rating::Good, 2_000, 1500, ReviewMode::Classic).unwrap();
        let b = store.record_review("waktu", Rating::Again, 1_000, 900, ReviewMode::Burst).unwrap();
        assert!(b.hlc > a.hlc, "the clock never goes backwards");
        // A remote event from the past sorts before ours.
        let remote = ReviewEvent {
            id: "remote-1".into(),
            card_id: "kabar".into(),
            hlc: Hlc { wall_ms: 500, counter: 0, device: DeviceId("tab-s11".into()) },
            at_ms: 500,
            rating: Rating::Hard,
            duration_ms: 3000,
            mode: ReviewMode::CardWall,
        };
        store.append_review(&remote).unwrap();
        store.append_review(&remote).unwrap();
        store.append_review(&a).unwrap();
        let ids: Vec<String> = store.review_log().unwrap().into_iter().map(|e| e.id).collect();
        assert_eq!(ids, ["remote-1".to_string(), a.id, b.id]);
    }

    #[test]
    fn undo_removes_only_local_events_and_clock_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kabar.db");
        let mut store = Store::open(&path).unwrap();
        let e = store.record_review("kabar", Rating::Easy, now_ms(), 800, ReviewMode::Classic).unwrap();
        assert!(!store.undo_review("remote-1").unwrap());
        let kept = store.record_review("kabar", Rating::Good, now_ms(), 800, ReviewMode::Classic).unwrap();
        assert!(store.undo_review(&e.id).unwrap());
        assert_eq!(store.review_log().unwrap(), std::slice::from_ref(&kept));
        drop(store);

        let mut store = Store::open(&path).unwrap();
        let next = store.new_review("kabar", Rating::Good, 0, 1, ReviewMode::Classic);
        assert!(next.hlc > kept.hlc, "HLC resumes above review_log rows");
    }
}
