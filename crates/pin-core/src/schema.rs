use rusqlite::{Connection, TransactionBehavior};

use crate::error::{Error, Result};

const MIGRATIONS: &[&str] = &[
    r"
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
",
    r"
CREATE TABLE vault (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    uid BLOB NOT NULL,
    salt BLOB NOT NULL,
    kdf_memory_kib INTEGER NOT NULL,
    kdf_iterations INTEGER NOT NULL,
    kdf_parallelism INTEGER NOT NULL,
    password_wrap BLOB NOT NULL,
    recovery_wrap BLOB NOT NULL
);

CREATE TABLE secrets (
    id INTEGER PRIMARY KEY,
    uid BLOB NOT NULL UNIQUE,
    meta BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE secret_formats (
    id INTEGER PRIMARY KEY,
    secret_id INTEGER NOT NULL REFERENCES secrets (id) ON DELETE CASCADE,
    data BLOB NOT NULL
);
CREATE INDEX secret_formats_secret ON secret_formats (secret_id);

-- Without it FTS5 keeps deleted text in index segments until a merge; moving an entry to the vault
-- must not leave it readable with the database key alone.
INSERT INTO entries_fts (entries_fts, rank) VALUES ('secure-delete', 1);
INSERT INTO entries_fts (entries_fts) VALUES ('optimize');

CREATE INDEX entries_seq ON entries (use_seq);
",
    r"
-- NULL: not generated yet; empty blob: the image could not be decoded.
ALTER TABLE entries ADD COLUMN thumbnail BLOB;
",
];

// pin-app and pin-cli may open the same database right after an upgrade, so the version is
// re-read under a write lock before any migration runs.
pub(crate) fn migrate(conn: &mut Connection) -> Result<()> {
    let supported = i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX);
    if user_version(conn)? == supported {
        return Ok(());
    }
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let found = user_version(&tx)?;
    if found < 0 || found > supported {
        return Err(Error::UnsupportedSchema { found, supported });
    }
    for sql in MIGRATIONS.iter().skip(usize::try_from(found).unwrap_or(0)) {
        tx.execute_batch(sql)?;
    }
    tx.pragma_update(None, "user_version", supported)?;
    tx.commit()?;
    tracing::info!(from = found, to = supported, "applied schema migrations");
    Ok(())
}

fn user_version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
}
