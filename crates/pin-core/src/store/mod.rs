mod entries;
mod pins;
mod search;

use std::path::Path;

use rusqlite::{Connection, ErrorCode, Row, Transaction, TransactionBehavior};

use crate::error::{Error, Result};
use crate::key::Key;
use crate::model::{Entry, EntryId, FormatKind, PinSlot, Timestamp};
use crate::schema;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub history_limit: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            history_limit: 1000,
        }
    }
}

pub struct Store {
    conn: Connection,
    options: Options,
}

impl Store {
    pub fn open(path: &Path, key: &Key, options: Options) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        conn.execute_batch(&key.sqlcipher_pragma())?;
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .map_err(|e| match e.sqlite_error_code() {
                Some(ErrorCode::NotADatabase) => Error::WrongKey,
                _ => Error::Sqlite(e),
            })?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;
             PRAGMA secure_delete = ON;",
        )?;
        search::register_functions(&conn)?;
        schema::migrate(&mut conn)?;
        Ok(Self { conn, options })
    }

    pub fn options(&self) -> Options {
        self.options
    }
}

const ENTRY_COLUMNS: &str = "e.id, e.kind, e.preview, e.source_app, e.size, e.created_at, \
                             e.last_used_at, e.pinned, e.pin_slot";

fn entry_from_row(row: &Row<'_>) -> rusqlite::Result<Entry> {
    let slot: Option<u8> = row.get(8)?;
    Ok(Entry {
        id: EntryId(row.get(0)?),
        kind: FormatKind::parse(&row.get::<_, String>(1)?),
        preview: row.get(2)?,
        source_app: row.get(3)?,
        size: u64::try_from(row.get::<_, i64>(4)?).unwrap_or_default(),
        created_at: Timestamp::from_millis(row.get(5)?),
        last_used_at: Timestamp::from_millis(row.get(6)?),
        pinned: row.get(7)?,
        slot: slot.and_then(|s| PinSlot::new(s).ok()),
    })
}

// A deferred transaction that reads before writing fails with SQLITE_BUSY_SNAPSHOT, not a
// retryable SQLITE_BUSY, when pin-app and pin-cli write the same WAL database concurrently.
fn write_tx(conn: &mut Connection) -> rusqlite::Result<Transaction<'_>> {
    conn.transaction_with_behavior(TransactionBehavior::Immediate)
}

fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn next_seq(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COALESCE(MAX(use_seq), 0) + 1 FROM entries",
        [],
        |row| row.get(0),
    )
}

fn prune(conn: &Connection, limit: usize) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM entries WHERE id IN (
             SELECT id FROM entries WHERE pinned = 0
             ORDER BY use_seq DESC LIMIT -1 OFFSET ?1
         )",
        [to_i64(limit.max(1))],
    )
}

fn expect_row(id: EntryId, changed: usize) -> Result<()> {
    if changed == 0 {
        Err(Error::NotFound(id))
    } else {
        Ok(())
    }
}
