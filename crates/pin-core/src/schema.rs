use rusqlite::Connection;

use crate::error::{Error, Result};

const MIGRATIONS: &[&str] = &[r"
CREATE TABLE entries (
    id INTEGER PRIMARY KEY,
    hash BLOB NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    preview TEXT NOT NULL,
    source_app TEXT,
    size INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    use_seq INTEGER NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0,
    pin_slot INTEGER UNIQUE CHECK (pin_slot BETWEEN 1 AND 9)
);
CREATE INDEX entries_recent ON entries (pinned DESC, use_seq DESC);

CREATE TABLE formats (
    id INTEGER PRIMARY KEY,
    entry_id INTEGER NOT NULL REFERENCES entries (id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    data BLOB NOT NULL,
    UNIQUE (entry_id, kind)
);

CREATE VIRTUAL TABLE entries_fts USING fts5 (text, tokenize = 'trigram');

CREATE TRIGGER entries_delete_fts AFTER DELETE ON entries BEGIN
    DELETE FROM entries_fts WHERE rowid = old.id;
END;
"];

pub(crate) fn migrate(conn: &mut Connection) -> Result<()> {
    let supported = i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX);
    let found: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if found < 0 || found > supported {
        return Err(Error::UnsupportedSchema { found, supported });
    }
    for (version, sql) in (found..).zip(MIGRATIONS.iter().skip(usize::try_from(found).unwrap_or(0)))
    {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version + 1)?;
        tx.commit()?;
    }
    Ok(())
}
