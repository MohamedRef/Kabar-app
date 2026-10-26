//! Persona chats (M12, PLAN §16 C): one LWW row per conversation and one per
//! message in `conversations`. No typed table: rows live in `sync_rows`,
//! found by the `sync_rows_doc` index on `payload.doc`.

use kabar_protocol::chat::{CONVERSATIONS_TABLE, ChatMessage, ChatRow, Conversation};
use kabar_protocol::sync::OpKind;

use super::{Result, Store};

impl Store {
    pub fn save_chat_row(&mut self, row: &ChatRow) -> Result<()> {
        let payload = serde_json::to_value(row)?;
        self.write_synced(CONVERSATIONS_TABLE, row.id(), OpKind::Upsert, Some(payload)).map(drop)
    }

    /// Conversations, newest activity first. Rows a newer build wrote in a
    /// shape this one can't read are skipped.
    pub fn conversations(&self) -> Result<Vec<Conversation>> {
        let mut all: Vec<Conversation> = self
            .synced_rows(CONVERSATIONS_TABLE)?
            .into_iter()
            .filter_map(|(_, p)| match serde_json::from_value(p) {
                Ok(ChatRow::Conversation(c)) => Some(c),
                _ => None,
            })
            .collect();
        all.sort_by(|a, b| b.updated_ms.cmp(&a.updated_ms).then_with(|| b.id.cmp(&a.id)));
        Ok(all)
    }

    /// A conversation's messages, oldest first.
    pub fn chat_messages(&self, conversation: &str) -> Result<Vec<ChatMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT payload FROM sync_rows
             WHERE tbl = ?1 AND json_extract(payload, '$.doc') = ?2 AND deleted = 0 AND payload IS NOT NULL",
        )?;
        let rows: Vec<String> = stmt
            .query_map([CONVERSATIONS_TABLE, conversation], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut messages: Vec<ChatMessage> = rows
            .iter()
            .filter_map(|p| match serde_json::from_str(p) {
                Ok(ChatRow::Message(m)) => Some(m),
                _ => None,
            })
            .collect();
        messages.sort_by(|a, b| (a.created_ms, &a.id).cmp(&(b.created_ms, &b.id)));
        Ok(messages)
    }

    /// Deletes a conversation and its messages (tombstones, so it syncs).
    pub fn delete_conversation(&mut self, id: &str) -> Result<()> {
        for m in self.chat_messages(id)? {
            self.write_synced(CONVERSATIONS_TABLE, &m.id, OpKind::Delete, None)?;
        }
        self.write_synced(CONVERSATIONS_TABLE, id, OpKind::Delete, None).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::chat::ChatRole;

    use super::*;

    fn conversation(id: &str, updated_ms: u64) -> Conversation {
        Conversation {
            id: id.into(),
            doc: id.into(),
            persona: "mas-dimas".into(),
            scenario: Some("d01".into()),
            level: "j1".into(),
            register: "ngoko".into(),
            created_ms: 1,
            updated_ms,
        }
    }

    fn message(id: &str, doc: &str, at: u64) -> ChatMessage {
        ChatMessage {
            id: id.into(),
            doc: doc.into(),
            role: ChatRole::Learner,
            text: "Aku wis mangan.".into(),
            corrections: vec![],
            register: None,
            unverified: true,
            created_ms: at,
        }
    }

    #[test]
    fn conversations_and_messages_round_trip_and_delete() {
        let mut store = Store::open_in_memory().unwrap();
        store.save_chat_row(&ChatRow::Conversation(conversation("c1", 5))).unwrap();
        store.save_chat_row(&ChatRow::Conversation(conversation("c2", 9))).unwrap();
        store.save_chat_row(&ChatRow::Message(message("m2", "c1", 3))).unwrap();
        store.save_chat_row(&ChatRow::Message(message("m1", "c1", 2))).unwrap();
        store.save_chat_row(&ChatRow::Message(message("m3", "c2", 1))).unwrap();

        let ids: Vec<_> = store.conversations().unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["c2", "c1"], "newest activity first");
        let ids: Vec<_> = store.chat_messages("c1").unwrap().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, ["m1", "m2"]);

        store.delete_conversation("c1").unwrap();
        assert_eq!(store.conversations().unwrap().len(), 1);
        assert!(store.chat_messages("c1").unwrap().is_empty());
        assert_eq!(store.chat_messages("c2").unwrap().len(), 1);
    }
}
