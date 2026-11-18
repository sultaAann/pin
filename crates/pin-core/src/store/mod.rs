macro_rules! entry_columns {
    () => {
        "e.id, e.kind, e.preview, e.source_app, e.size, e.created_at, e.last_used_at, e.pinned, \
         e.pin_slot"
    };
}

mod entries;
mod pins;
mod search;
mod sql;

pub(crate) use entries::{delete_entry, load_formats};

use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, ErrorCode, Row, Transaction, TransactionBehavior};

use crate::error::{Error, Result};
use crate::key::Key;
use crate::keychain::{self, KeyStore};
use crate::model::{Entry, EntryId, Pin, PinSlot};
use crate::schema;
use crate::vault::{Session, Vault, VaultOptions};

const DEFAULT_HISTORY_LIMIT: NonZeroUsize = match NonZeroUsize::new(200) {
    Some(limit) => limit,
    None => panic!("default history limit must be non-zero"),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub history_limit: NonZeroUsize,
    pub vault: VaultOptions,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            history_limit: DEFAULT_HISTORY_LIMIT,
            vault: VaultOptions::default(),
        }
    }
}

pub struct Store {
    conn: Connection,
    options: Options,
    vault: Option<Session>,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl Store {
    // The lock file serialises first runs of pin-app and pin-cli: without it both could see no key,
    // each save its own, and leave the database encrypted with a key the keychain no longer holds.
    pub fn open_with_keys(
        path: impl AsRef<Path>,
        keys: &impl KeyStore,
        options: Options,
    ) -> Result<Self> {
        let path = path.as_ref();
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path(path))
            .map_err(Error::Lock)?;
        lock.lock().map_err(Error::Lock)?;
        let key = keychain::load_or_create(keys, path.try_exists()?)?;
        Self::open(path, &key, options)
    }

    pub fn open(path: impl AsRef<Path>, key: &Key, options: Options) -> Result<Self> {
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
        Ok(Self {
            conn,
            options,
            vault: None,
        })
    }

    pub fn options(&self) -> Options {
        self.options
    }

    pub fn vault(&mut self) -> Vault<'_> {
        Vault::new(&mut self.conn, &mut self.vault, self.options.vault)
    }
}

fn entry_from_row(row: &Row<'_>) -> rusqlite::Result<Entry> {
    let pinned: bool = row.get(7)?;
    let pin = match row.get::<_, Option<PinSlot>>(8)? {
        Some(slot) => Pin::Slot(slot),
        None if pinned => Pin::Pinned,
        None => Pin::None,
    };
    let size: i64 = row.get(4)?;
    Ok(Entry {
        id: row.get(0)?,
        kind: row.get(1)?,
        preview: row.get(2)?,
        source_app: row.get(3)?,
        size: u64::try_from(size).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(4, size))?,
        created_at: row.get(5)?,
        last_used_at: row.get(6)?,
        pin,
    })
}

// A deferred transaction that reads before writing fails with SQLITE_BUSY_SNAPSHOT, not a
// retryable SQLITE_BUSY, when pin-app and pin-cli write the same WAL database concurrently.
pub(crate) fn write_tx(conn: &mut Connection) -> rusqlite::Result<Transaction<'_>> {
    conn.transaction_with_behavior(TransactionBehavior::Immediate)
}

fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn next_seq(conn: &Connection) -> rusqlite::Result<i64> {
    conn.prepare_cached("SELECT COALESCE(MAX(use_seq), 0) + 1 FROM entries")?
        .query_row([], |row| row.get(0))
}

fn prune(conn: &Connection, limit: NonZeroUsize) -> rusqlite::Result<usize> {
    let pruned = conn.execute(
        "DELETE FROM entries WHERE id IN (
             SELECT id FROM entries WHERE pinned = 0
             ORDER BY use_seq DESC LIMIT -1 OFFSET ?1
         )",
        [to_i64(limit.get())],
    )?;
    tracing::debug!(pruned, "pruned history");
    Ok(pruned)
}

fn expect_row(id: EntryId, changed: usize) -> Result<()> {
    if changed == 0 {
        Err(Error::NotFound(id))
    } else {
        Ok(())
    }
}

// Best effort: copies scrubbed pages into the database and truncates the WAL so deleted plaintext
// does not linger in old frames. The change is already committed; a busy reader only delays this.
pub(crate) fn checkpoint(conn: &Connection) {
    if let Err(e) = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())) {
        tracing::warn!(code = ?e.sqlite_error_code(), "wal checkpoint failed");
    }
}

fn lock_path(path: &Path) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(".lock");
    PathBuf::from(name)
}
