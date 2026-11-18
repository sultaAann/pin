use rusqlite::{Connection, OptionalExtension, params};

use super::{Store, entry_from_row, expect_row, next_seq, prune, to_i64, write_tx};
use crate::content;
use crate::error::{Error, Result};
use crate::model::{Entry, EntryId, Format, NewEntry, Timestamp};

const GET_SQL: &str = concat!(
    "SELECT ",
    entry_columns!(),
    " FROM entries e WHERE e.id = ?1"
);
const RECENT_SQL: &str = concat!(
    "SELECT ",
    entry_columns!(),
    " FROM entries e ORDER BY e.pinned DESC, e.use_seq DESC LIMIT ?1 OFFSET ?2"
);

impl Store {
    pub fn insert(&mut self, new: NewEntry) -> Result<EntryId> {
        let formats = content::normalize(new.formats);
        if formats.is_empty() {
            return Err(Error::Empty);
        }
        let hash = content::hash(&formats);
        let now = Timestamp::now();
        let tx = write_tx(&mut self.conn)?;
        let seq = next_seq(&tx)?;

        let existing: Option<EntryId> = tx
            .prepare_cached("SELECT id FROM entries WHERE hash = ?1")?
            .query_row([&hash.as_bytes()[..]], |row| row.get(0))
            .optional()?;
        if let Some(id) = existing {
            tx.prepare_cached("UPDATE entries SET last_used_at = ?2, use_seq = ?3 WHERE id = ?1")?
                .execute(params![id, now, seq])?;
            tx.commit()?;
            return Ok(id);
        }

        let kind = content::primary_kind(&formats);
        tx.prepare_cached(
            "INSERT INTO entries (hash, kind, preview, source_app, size, created_at, last_used_at, use_seq)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
        )?
        .execute(params![
            &hash.as_bytes()[..],
            kind,
            content::preview(&formats, &kind),
            new.source_app,
            to_i64(content::size(&formats)),
            now,
            seq,
        ])?;
        let id = EntryId(tx.last_insert_rowid());
        {
            let mut stmt = tx
                .prepare_cached("INSERT INTO formats (entry_id, kind, data) VALUES (?1, ?2, ?3)")?;
            for format in &formats {
                stmt.execute(params![id, format.kind, format.data])?;
            }
        }
        if let Some(text) = content::search_text(&formats) {
            tx.prepare_cached("INSERT INTO entries_fts (rowid, text) VALUES (?1, ?2)")?
                .execute(params![id, text])?;
        }
        prune(&tx, self.options.history_limit)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn get(&self, id: EntryId) -> Result<Entry> {
        self.conn
            .prepare_cached(GET_SQL)?
            .query_row([id], entry_from_row)
            .optional()?
            .ok_or(Error::NotFound(id))
    }

    pub fn formats(&self, id: EntryId) -> Result<Vec<Format>> {
        load_formats(&self.conn, id)
    }

    pub fn recent(&self, limit: usize, offset: usize) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare_cached(RECENT_SQL)?;
        let entries = stmt
            .query_map([to_i64(limit), to_i64(offset)], entry_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(entries)
    }

    pub fn touch(&mut self, id: EntryId) -> Result<()> {
        let tx = write_tx(&mut self.conn)?;
        let seq = next_seq(&tx)?;
        let changed = tx
            .prepare_cached("UPDATE entries SET last_used_at = ?2, use_seq = ?3 WHERE id = ?1")?
            .execute(params![id, Timestamp::now(), seq])?;
        expect_row(id, changed)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete(&mut self, id: EntryId) -> Result<()> {
        delete_entry(&self.conn, id)
    }

    pub fn clear_history(&mut self) -> Result<usize> {
        let tx = write_tx(&mut self.conn)?;
        let removed = tx.execute("DELETE FROM entries WHERE pinned = 0", [])?;
        tx.execute(
            "INSERT INTO entries_fts (entries_fts) VALUES ('optimize')",
            [],
        )?;
        tx.commit()?;
        Ok(removed)
    }
}

pub(crate) fn load_formats(conn: &Connection, id: EntryId) -> Result<Vec<Format>> {
    let mut stmt =
        conn.prepare_cached("SELECT kind, data FROM formats WHERE entry_id = ?1 ORDER BY kind")?;
    let formats = stmt
        .query_map([id], |row| {
            Ok(Format {
                kind: row.get(0)?,
                data: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if formats.is_empty() {
        let exists: bool = conn
            .prepare_cached("SELECT EXISTS (SELECT 1 FROM entries WHERE id = ?1)")?
            .query_row([id], |row| row.get(0))?;
        if !exists {
            return Err(Error::NotFound(id));
        }
    }
    Ok(formats)
}

pub(crate) fn delete_entry(conn: &Connection, id: EntryId) -> Result<()> {
    let changed = conn.execute("DELETE FROM entries WHERE id = ?1", [id])?;
    expect_row(id, changed)
}
