//! Who a sent message reached, and when: what "Message info" shows.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The receipts of one message this account sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageInfo {
    pub chat_jid: String,
    pub message_id: String,
    /// One entry per recipient, never per device: a person reading on their
    /// phone and on a computer read it once. Recipients nothing has come
    /// back from yet are absent.
    pub recipients: Vec<RecipientReceipt>,
}

/// How far a message got with one recipient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipientReceipt {
    pub jid: String,
    /// What to call them, or `None` when nobody has said; the renderer draws
    /// a stranger, as everywhere else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<DateTime<Utc>>,
    /// A voice note or video note played.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub played_at: Option<DateTime<Utc>>,
}

impl RecipientReceipt {
    /// Read implies delivered, and played implies read: a receipt for a
    /// later state can arrive without the earlier one ever being sent.
    #[must_use]
    pub fn has_read(&self) -> bool {
        self.read_at.is_some() || self.played_at.is_some()
    }

    #[must_use]
    pub fn read_time(&self) -> Option<DateTime<Utc>> {
        self.read_at.or(self.played_at)
    }

    #[must_use]
    pub fn delivered_time(&self) -> Option<DateTime<Utc>> {
        self.delivered_at.or(self.read_time())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_state_stands_in_for_an_earlier_one_never_sent() {
        let at = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let played = RecipientReceipt {
            jid: "1@s.whatsapp.net".into(),
            name: None,
            delivered_at: None,
            read_at: None,
            played_at: Some(at),
        };
        assert!(played.has_read());
        assert_eq!(played.delivered_time(), Some(at));
    }
}
