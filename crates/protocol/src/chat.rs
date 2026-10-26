//! Persona chats (`docs/SPEC.md` §15, PLAN §16 C), synced in the LWW table
//! `conversations`: one row per conversation and one per message, so every
//! op stays small. Every row carries `doc` (the conversation id), which the
//! app's `sync_rows_doc` index finds a chat's messages by.

use serde::{Deserialize, Serialize};

/// The sync table holding [`ChatRow`]s.
pub const CONVERSATIONS_TABLE: &str = "conversations";
/// Bumped when a chat row changes incompatibly.
pub const CHAT_SCHEMA_VERSION: u32 = 1;
/// Most characters of one learner message (sync payloads stay small, and a
/// roleplay turn is a sentence or three).
pub const MAX_MESSAGE_CHARS: usize = 600;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "row", rename_all = "snake_case")]
pub enum ChatRow {
    Conversation(Conversation),
    Message(ChatMessage),
}

impl ChatRow {
    pub fn id(&self) -> &str {
        match self {
            Self::Conversation(c) => &c.id,
            Self::Message(m) => &m.id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    /// UUIDv7.
    pub id: String,
    /// The same as `id`, so the conversation row is found with its messages.
    pub doc: String,
    /// Canonical persona id (`mas-dimas`).
    pub persona: String,
    /// The persona's example dialogue the scene comes from (`d01`), or
    /// `None` for a free chat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    /// The learner's level the chat was pitched at (`j1`, `bipa2`).
    pub level: String,
    /// `ngoko`, `krama`, `casual` or `standard`: what the learner asked the
    /// persona to use with them.
    pub register: String,
    pub created_ms: u64,
    /// When the newest message was added (the list sorts by it).
    pub updated_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    Persona,
    Learner,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// UUIDv7, so a chat's messages sort by time.
    pub id: String,
    /// The conversation id.
    pub doc: String,
    pub role: ChatRole,
    pub text: String,
    /// On a learner message: the persona's gentle fixes of it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corrections: Vec<ChatCorrection>,
    /// On a persona message: the register the reply used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub register: Option<String>,
    /// AI-written Javanese (or a correction of it) not yet approved.
    #[serde(default)]
    pub unverified: bool,
    pub created_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatCorrection {
    /// The learner's words as written.
    pub text: String,
    pub fix: String,
    pub why_en: String,
    pub why_ar: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_tagged_and_carry_their_doc() {
        let m = ChatRow::Message(ChatMessage {
            id: "m1".into(),
            doc: "c1".into(),
            role: ChatRole::Learner,
            text: "Aku wis mangan.".into(),
            corrections: vec![],
            register: None,
            unverified: false,
            created_ms: 1,
        });
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["row"], "message");
        assert_eq!(json["doc"], "c1");
        assert_eq!(json["role"], "learner");
        assert!(json.get("corrections").is_none());
        assert_eq!(serde_json::from_value::<ChatRow>(json).unwrap(), m);
        assert_eq!(m.id(), "m1");
    }
}
