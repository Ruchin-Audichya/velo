//! Instant file-name search across every document shared in every chat.
//!
//! A document's file name lives inside its stored protobuf, which no query
//! can look into, and the message index (`fts.rs`) holds text and captions
//! only, so "resume" found nothing unless someone had typed it. This keeps a
//! second FTS5 table of file names under the **trigram** tokenizer, which
//! indexes every three-character run: `LIKE '%resum%'` is answered from the
//! index rather than a scan, for any substring, in any position, across the
//! whole history — downloaded or not.
//!
//! The index catches up on demand rather than from the writer: every search
//! first indexes the documents stored since the last one (a cursor on
//! `messages.id`, which only grows), so it is always current and the hot
//! write path is untouched. The first search after an upgrade indexes the
//! whole history once. A row deleted or revoked since it was indexed drops
//! out through the join on `messages`, which also checks `msg_id`, so an id
//! reused after deleting the newest row never borrows a stale name.
//!
//! Created lazily, like the message index, so a build without the `search`
//! feature carries no FTS objects at all.

use diesel::prelude::*;
use diesel::sql_types::{BigInt, Binary, Integer, Nullable, Text};

use crate::error::{Result, db_err};
use crate::store::ChatStore;

/// One document whose name matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHit {
    pub chat_jid: String,
    pub msg_id: String,
    pub file_name: String,
    pub mime: Option<String>,
    pub size: Option<u64>,
    pub timestamp_ms: i64,
    pub from_me: bool,
}

/// How many stored documents one catch-up pass decodes at a time.
const BATCH: i64 = 500;

pub(crate) fn ensure_file_index(conn: &mut SqliteConnection) -> QueryResult<()> {
    diesel::sql_query(
        "CREATE VIRTUAL TABLE IF NOT EXISTS file_names_fts USING fts5(\
             file_name, msg_id UNINDEXED, mime UNINDEXED, size UNINDEXED, \
             tokenize = 'trigram')",
    )
    .execute(conn)?;
    diesel::sql_query(
        "CREATE TABLE IF NOT EXISTS file_names_cursor (\
             device_id INTEGER PRIMARY KEY, last_id BIGINT NOT NULL)",
    )
    .execute(conn)?;
    Ok(())
}

#[derive(QueryableByName)]
struct Cursor {
    #[diesel(sql_type = BigInt)]
    n: i64,
}

#[derive(QueryableByName)]
struct Pending {
    #[diesel(sql_type = BigInt)]
    id: i64,
    #[diesel(sql_type = Text)]
    msg_id: String,
    #[diesel(sql_type = Nullable<Binary>)]
    proto: Option<Vec<u8>>,
    #[diesel(sql_type = Integer)]
    proto_codec: i32,
}

/// The name, type and size a stored document carries, if it decodes.
fn document_facts(row: &Pending) -> Option<(String, Option<String>, Option<u64>)> {
    let message =
        crate::storage_proto::decode_storage_proto(row.proto.as_deref()?, row.proto_codec).ok()?;
    let base = crate::materialize::normalized_message(&message);
    let document = base.document_message.as_option()?;
    let name = document
        .file_name
        .clone()
        .or_else(|| document.title.clone())
        .filter(|name| !name.trim().is_empty())?;
    Some((name, document.mimetype.clone(), document.file_length))
}

/// Index every document stored since the last pass.
fn catch_up(conn: &mut SqliteConnection, device_id: i32) -> QueryResult<()> {
    let mut last = diesel::sql_query(
        "SELECT COALESCE((SELECT last_id FROM file_names_cursor WHERE device_id = ?), 0) AS n",
    )
    .bind::<Integer, _>(device_id)
    .get_result::<Cursor>(conn)?
    .n;
    loop {
        let rows: Vec<Pending> = diesel::sql_query(
            "SELECT id, msg_id, proto, proto_codec FROM messages \
             WHERE device_id = ? AND id > ? AND kind = 'document' \
             ORDER BY id LIMIT ?",
        )
        .bind::<Integer, _>(device_id)
        .bind::<BigInt, _>(last)
        .bind::<BigInt, _>(BATCH)
        .load(conn)?;
        let Some(tail) = rows.last().map(|row| row.id) else {
            break;
        };
        conn.transaction(|conn| {
            for row in &rows {
                let Some((name, mime, size)) = document_facts(row) else {
                    continue;
                };
                diesel::sql_query(
                    "INSERT INTO file_names_fts(rowid, file_name, msg_id, mime, size) \
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind::<BigInt, _>(row.id)
                .bind::<Text, _>(&name)
                .bind::<Text, _>(&row.msg_id)
                .bind::<Nullable<Text>, _>(mime.as_deref())
                .bind::<Nullable<BigInt>, _>(size.and_then(|s| i64::try_from(s).ok()))
                .execute(conn)?;
            }
            diesel::sql_query(
                "INSERT INTO file_names_cursor(device_id, last_id) VALUES (?, ?) \
                 ON CONFLICT(device_id) DO UPDATE SET last_id = excluded.last_id",
            )
            .bind::<Integer, _>(device_id)
            .bind::<BigInt, _>(tail)
            .execute(conn)
            .map(|_| ())
        })?;
        last = tail;
        if (rows.len() as i64) < BATCH {
            break;
        }
    }
    Ok(())
}

#[derive(QueryableByName)]
struct HitRow {
    #[diesel(sql_type = Text)]
    chat_jid: String,
    #[diesel(sql_type = Text)]
    msg_id: String,
    #[diesel(sql_type = Text)]
    file_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    mime: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = BigInt)]
    timestamp_ms: i64,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    from_me: bool,
}

/// `LIKE` with the user's text taken literally: `%` and `_` in a file name
/// are characters, not wildcards.
fn like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn search(
    conn: &mut SqliteConnection,
    device_id: i32,
    query: &str,
    limit: i64,
) -> QueryResult<Vec<FileHit>> {
    let query = query.trim();
    let rows: Vec<HitRow> = diesel::sql_query(
        "SELECT m.chat_jid, m.msg_id, f.file_name, f.mime, f.size, m.timestamp_ms, m.from_me \
         FROM file_names_fts f \
         JOIN messages m ON m.id = f.rowid AND m.msg_id = f.msg_id \
         WHERE m.device_id = ? AND m.revoked = 0 \
           AND (? = '' OR f.file_name LIKE ? ESCAPE '\\') \
         ORDER BY m.timestamp_ms DESC LIMIT ?",
    )
    .bind::<Integer, _>(device_id)
    .bind::<Text, _>(query)
    .bind::<Text, _>(like_pattern(query))
    .bind::<BigInt, _>(limit)
    .load(conn)?;
    Ok(rows
        .into_iter()
        .map(|row| FileHit {
            chat_jid: row.chat_jid,
            msg_id: row.msg_id,
            file_name: row.file_name,
            mime: row.mime,
            size: row.size.and_then(|s| u64::try_from(s).ok()),
            timestamp_ms: row.timestamp_ms,
            from_me: row.from_me,
        })
        .collect())
}

impl ChatStore {
    /// Documents whose file name contains `query` (any part of it, any
    /// case), newest first. An empty query lists the newest documents.
    pub async fn search_file_names(&self, query: &str, limit: i64) -> Result<Vec<FileHit>> {
        let device_id = self.device_id();
        let query = query.to_owned();
        let limit = limit.clamp(1, 500);
        Ok(self
            .db()
            .run(move |conn| {
                ensure_file_index(conn).map_err(db_err)?;
                catch_up(conn, device_id).map_err(db_err)?;
                search(conn, device_id, &query, limit).map_err(db_err)
            })
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
    use waproto::whatsapp as wa;

    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

    fn migrated_connection() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:").expect("in-memory sqlite");
        diesel::sql_query("CREATE TABLE device (id INTEGER PRIMARY KEY)")
            .execute(&mut conn)
            .expect("device parent");
        diesel::sql_query("INSERT INTO device (id) VALUES (1)")
            .execute(&mut conn)
            .expect("device row");
        diesel::sql_query("CREATE TABLE lid_pn_mapping (device_id INTEGER NOT NULL)")
            .execute(&mut conn)
            .expect("mapping parent");
        conn.run_pending_migrations(MIGRATIONS).expect("migrate");
        conn
    }

    fn store_document(conn: &mut SqliteConnection, id: i64, chat: &str, name: &str) {
        let mut message = wa::Message::default();
        message.document_message = buffa::MessageField::some({
            let mut document = wa::message::DocumentMessage::default();
            document.file_name = Some(name.to_owned());
            document.mimetype = Some("application/pdf".to_owned());
            document.file_length = Some(38_000);
            document
        });
        let (bytes, codec) = crate::storage_proto::encode_storage_proto(&message);
        diesel::sql_query(
            "INSERT INTO messages                  (id, device_id, chat_jid, msg_id, sender_jid, timestamp_ms, kind, proto, proto_codec)              VALUES (?, 1, ?, ?, 'someone', ?, 'document', ?, ?)",
        )
        .bind::<BigInt, _>(id)
        .bind::<Text, _>(chat)
        .bind::<Text, _>(format!("MSG-{id}"))
        .bind::<BigInt, _>(id * 1000)
        .bind::<Binary, _>(bytes)
        .bind::<Integer, _>(codec)
        .execute(conn)
        .expect("store a document");
    }

    fn names(conn: &mut SqliteConnection, query: &str) -> Vec<String> {
        ensure_file_index(conn).expect("index");
        catch_up(conn, 1).expect("catch up");
        search(conn, 1, query, 50)
            .expect("search")
            .into_iter()
            .map(|hit| hit.file_name)
            .collect()
    }

    #[test]
    fn any_part_of_a_file_name_finds_it_and_new_files_are_indexed_on_the_next_search() {
        let mut conn = migrated_connection();
        store_document(
            &mut conn,
            1,
            "a@s.whatsapp.net",
            "Quarterly Report FINAL.pdf",
        );
        store_document(&mut conn, 2, "b@g.us", "seating plan 18 nov.xlsx");

        // Substring, any case, mid-word: what the trigram index is for.
        assert_eq!(names(&mut conn, "report"), ["Quarterly Report FINAL.pdf"]);
        assert_eq!(names(&mut conn, "ATING"), ["seating plan 18 nov.xlsx"]);
        assert!(names(&mut conn, "invoice").is_empty());

        // A document that arrives later is found without any rebuild.
        store_document(&mut conn, 3, "a@s.whatsapp.net", "invoice-2026.pdf");
        assert_eq!(names(&mut conn, "invoice"), ["invoice-2026.pdf"]);

        // Newest first, and an empty query lists everything.
        assert_eq!(names(&mut conn, "").len(), 3);
        assert_eq!(
            names(&mut conn, "").first().map(String::as_str),
            Some("invoice-2026.pdf")
        );

        // A deleted message stops matching.
        diesel::sql_query("DELETE FROM messages WHERE id = 3")
            .execute(&mut conn)
            .expect("delete");
        assert!(names(&mut conn, "invoice").is_empty());
    }

    #[test]
    fn wildcards_in_a_file_name_are_taken_literally() {
        assert_eq!(like_pattern("resume"), "%resume%");
        assert_eq!(like_pattern("50%_off"), "%50\\%\\_off%");
    }
}
