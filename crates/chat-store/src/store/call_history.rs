use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use velo_core::CallHistoryEntry;
use wacore::types::events::Event;
use wacore_binary::{Jid, JidExt as _};
use waproto::whatsapp::{CallLogRecord, call_log_record::CallResult};

use super::ChatStore;
use crate::error::{Result, db_err};

const LIMIT: i64 = 5000;

pub(super) fn apply(conn: &mut SqliteConnection, device: i32, event: &Event) -> QueryResult<()> {
    let (record, id, creator, from_me, revision_ms) = match event {
        Event::CallLogSync(call) => (
            &*call.record,
            Some(call.call_id.as_str()),
            Some(&call.call_creator_jid),
            Some(call.from_me),
            call.timestamp.timestamp_millis(),
        ),
        Event::CallLogHistory(call) => (
            &*call.record,
            call.record.call_id.as_deref(),
            call.call_creator_jid.as_ref(),
            call.from_me,
            0,
        ),
        _ => return Ok(()),
    };
    let Some(id) = id.filter(|id| !id.trim().is_empty() && id.len() <= 256) else {
        return Ok(());
    };
    let Some(entry) = entry(record, id, creator, from_me) else {
        return Ok(());
    };
    let payload = serde_json::to_string(&entry)
        .map_err(|error| diesel::result::Error::SerializationError(Box::new(error)))?;
    diesel::sql_query(
        "INSERT INTO companion_calls (device_id, call_id, started_ms, record, revision_ms) VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(device_id, call_id) DO UPDATE SET started_ms = excluded.started_ms, record = excluded.record,
         revision_ms = excluded.revision_ms WHERE excluded.revision_ms >= companion_calls.revision_ms",
    )
    .bind::<Integer, _>(device)
    .bind::<Text, _>(id)
    .bind::<Nullable<BigInt>, _>(entry.started_ms)
    .bind::<Text, _>(payload)
    .bind::<BigInt, _>(revision_ms)
    .execute(conn)?;
    diesel::sql_query("DELETE FROM companion_calls WHERE device_id = ? AND call_id IN
        (SELECT call_id FROM companion_calls WHERE device_id = ? ORDER BY started_ms DESC, call_id DESC LIMIT -1 OFFSET ?)")
        .bind::<Integer, _>(device).bind::<Integer, _>(device).bind::<BigInt, _>(LIMIT).execute(conn)?;
    Ok(())
}

fn entry(
    record: &CallLogRecord,
    id: &str,
    creator: Option<&Jid>,
    from_me: Option<bool>,
) -> Option<CallHistoryEntry> {
    let outcome = match record.call_result {
        Some(CallResult::Connected) => Some("connected"),
        Some(CallResult::Acceptedelsewhere) => Some("answered_elsewhere"),
        Some(CallResult::Missed) => Some("missed"),
        Some(CallResult::Rejected) => Some("declined"),
        Some(CallResult::Cancelled) => Some("cancelled"),
        Some(CallResult::Unavailable) => Some("unavailable"),
        Some(CallResult::Failed) => Some("failed"),
        Some(CallResult::Abandoned) => Some("abandoned"),
        Some(CallResult::Upcoming | CallResult::Ongoing | CallResult::Invalid) => return None,
        _ => None,
    };
    let participants: std::collections::BTreeSet<_> = record
        .participants
        .iter()
        .filter_map(|person| person.user_jid.as_deref()?.parse::<Jid>().ok())
        .map(|jid| jid.to_non_ad_string())
        .collect();
    let group_jid = record
        .group_jid
        .as_deref()
        .and_then(|jid| jid.parse::<Jid>().ok())
        .filter(|jid| jid.is_group());
    let caller_jid = creator.map(|jid| jid.to_non_ad_string());
    // Direction comes from the record. The event's own `from_me` says which
    // device wrote the call-log entry — the account's phone, for every
    // synced call — so reading it as the call's direction made every call
    // in the history "outgoing".
    let from_me = record.is_incoming.map(|incoming| !incoming).or(from_me);
    let peer = match from_me {
        Some(true) => participants
            .iter()
            .find(|jid| Some(jid.as_str()) != caller_jid.as_deref())
            .cloned(),
        Some(false) => caller_jid.clone(),
        None => None,
    };
    Some(CallHistoryEntry {
        id: id.to_owned(),
        group: group_jid.is_some() || participants.len() > 2,
        chat_jid: group_jid
            .as_ref()
            .map(|jid| jid.to_non_ad_string())
            .or(peer),
        caller_jid,
        started_ms: record
            .start_time
            .filter(|seconds| *seconds > 0)
            .and_then(|seconds| seconds.checked_mul(1000)),
        from_me,
        video: record.is_video,
        duration_seconds: record
            .duration
            .and_then(|seconds| u32::try_from(seconds).ok()),
        outcome: outcome.map(str::to_owned),
        synced: true,
    })
}

#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type = Text)]
    record: String,
}

impl ChatStore {
    pub async fn call_history(&self, limit: usize) -> Result<Vec<CallHistoryEntry>> {
        let device = self.device_id();
        self.flush().await?;
        Ok(self
            .db()
            .read(move |conn| {
                let rows = diesel::sql_query(
                    "SELECT record FROM companion_calls WHERE device_id = ?
                ORDER BY started_ms DESC, call_id DESC LIMIT ?",
                )
                .bind::<Integer, _>(device)
                .bind::<BigInt, _>(limit.clamp(1, LIMIT as usize) as i64)
                .load::<Row>(conn)
                .map_err(db_err)?;
                rows.into_iter()
                    .map(|row| {
                        serde_json::from_str(&row.record).map_err(|error| {
                            db_err(diesel::result::Error::DeserializationError(Box::new(error)))
                        })
                    })
                    .collect()
            })
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::types::events::{CallLogHistory, CallLogSync};

    #[test]
    fn phone_revision_survives_older_replays_and_keeps_accounts_separate() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        diesel::sql_query("CREATE TABLE device (id INTEGER PRIMARY KEY)")
            .execute(&mut conn)
            .unwrap();
        diesel::sql_query("INSERT INTO device (id) VALUES (1), (2)")
            .execute(&mut conn)
            .unwrap();
        diesel::connection::SimpleConnection::batch_execute(
            &mut conn,
            include_str!("../../migrations/2026-11-16-000001_call_history/up.sql"),
        )
        .unwrap();
        let creator: Jid = "55500000001@s.whatsapp.net".parse().unwrap();
        let mut remote = waproto::whatsapp::call_log_record::ParticipantInfo::default();
        remote.user_jid = Some("55500000002@s.whatsapp.net".into());
        let mut call = CallLogRecord::default();
        call.call_id = Some("call-a".into());
        call.start_time = Some(1_700_000_000);
        // Placed by the account (the creator), to the remote participant.
        call.is_incoming = Some(false);
        call.is_video = Some(true);
        call.duration = Some(45);
        call.call_result = Some(CallResult::Connected);
        call.participants.push(remote);
        let mut old = call.clone();
        old.call_result = Some(CallResult::Missed);
        let revised = Event::CallLogSync(
            CallLogSync::builder()
                .call_creator_jid(creator.clone())
                .call_id("call-a".into())
                .from_me(true)
                .timestamp(wacore::time::from_millis_or_now(1_700_000_001_000))
                .record(Box::new(call))
                .from_full_sync(false)
                .build(),
        );
        let replay = Event::CallLogHistory(
            CallLogHistory::builder()
                .call_creator_jid(creator)
                .from_me(true)
                .record(Box::new(old))
                .build(),
        );
        apply(&mut conn, 1, &revised).unwrap();
        apply(&mut conn, 1, &replay).unwrap();
        apply(&mut conn, 2, &replay).unwrap();
        let rows = diesel::sql_query("SELECT record FROM companion_calls ORDER BY device_id")
            .load::<Row>(&mut conn)
            .unwrap();
        let first: CallHistoryEntry = serde_json::from_str(&rows[0].record).unwrap();
        let second: CallHistoryEntry = serde_json::from_str(&rows[1].record).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(first.from_me, Some(true));
        assert_eq!(first.started_ms, Some(1_700_000_000_000));
        assert_eq!(
            first.chat_jid.as_deref(),
            Some("55500000002@s.whatsapp.net")
        );
        assert_eq!(first.outcome.as_deref(), Some("connected"));
        assert_eq!(first.duration_seconds, Some(45));
        assert_eq!(second.outcome.as_deref(), Some("missed"));
    }

    /// The direction is the record's, not the event's: every synced entry
    /// is written by the account's own phone, so the event's `from_me` is
    /// true even for a call that rang in.
    #[test]
    fn an_incoming_call_is_incoming_whoever_synced_it() {
        let caller: Jid = "55500000003@s.whatsapp.net".parse().unwrap();
        let mut record = CallLogRecord::default();
        record.is_incoming = Some(true);
        record.call_result = Some(CallResult::Missed);
        let entry = entry(&record, "call-b", Some(&caller), Some(true)).unwrap();
        assert_eq!(entry.from_me, Some(false));
        assert_eq!(
            entry.chat_jid.as_deref(),
            Some("55500000003@s.whatsapp.net")
        );
        assert_eq!(entry.outcome.as_deref(), Some("missed"));
    }
}
