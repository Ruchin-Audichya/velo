use serde::{Deserialize, Serialize};

/// A finalized WhatsApp call. Unknown fields stay absent when the phone omits them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallHistoryEntry {
    pub id: String,
    pub chat_jid: Option<String>,
    pub caller_jid: Option<String>,
    pub started_ms: Option<i64>,
    pub from_me: Option<bool>,
    pub video: Option<bool>,
    pub group: bool,
    pub duration_seconds: Option<u32>,
    pub outcome: Option<String>,
    pub synced: bool,
}
