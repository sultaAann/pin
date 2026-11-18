use rusqlite::{OptionalExtension, params};

use super::{ENTRY_COLUMNS, Store, entry_from_row, expect_row, next_seq, prune, to_i64, write_tx};
use crate::content;
use crate::error::{Error, Result};
use crate::model::{Entry, EntryId, Format, FormatKind, NewEntry, Timestamp};

impl Store {
    pub fn insert(&mut self, new: NewEntry) -> Result<EntryId> {
        let formats = content::normalize(new.formats);
        if formats.is_empty() {
            return Err(Error::Empty);
        }
        let hash = content::hash(&formats);
        let now = Timestamp::now().as_millis();
        let tx = write_tx(&mut self.conn)?;
        let seq = next_seq(&tx)?;

        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM entries WHERE hash = ?1",
                [&hash.as_bytes()[..]],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            tx.execute(
                "UPDATE entries SET last_used_at = ?2, use_seq = ?3 WHERE id = ?1",
                params![id, now, seq],
            )?;
            tx.commit()?;
            return Ok(EntryId(id));
        }

        let kind = content::primary_kind(&formats);
        tx.execute(
            "INSERT INTO entries (hash, kind, preview, source_app, size, created_at, last_used_at, use_seq)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
            params![
                &hash.as_bytes()[..],
                kind.as_str(),
                content::preview(&formats, &kind),
                new.source_app,
                to_i64(content::size(&formats)),
                now,
                seq,
            ],
        )?;
        let id = tx.last_insert_rowid();
        {
            let mut stmt =
                tx.prepare("INSERT INTO formats (entry_id, kind, data) VALUES (?1, ?2, ?3)")?;
            for format in &formats {
                stmt.execute(params![id, format.kind.as_str(), format.data])?;
            }
        }
        if let Some(text) = content::search_text(&formats) {
            tx.execute(
                "INSERT INTO entries_fts (rowid, text) VALUES (?1, ?2)",
                params![id, text],
            )?;
        }
        prune(&tx, self.options.history_limit)?;
        tx.commit()?;
        Ok(EntryId(id))
    }

    pub fn get(&self, id: EntryId) -> Result<Entry> {
        self.conn
            .query_row(
                &format!("SELECT {ENTRY_COLUMNS} FROM entries e WHERE e.id = ?1"),
                [id.0],
                entry_from_row,
            )
            .optional()?
            .ok_or(Error::NotFound(id))
    }

    pub fn formats(&self, id: EntryId) -> Result<Vec<Format>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT kind, data FROM formats WHERE entry_id = ?1 ORDER BY kind")?;
        let formats = stmt
            .query_map([id.0], |row| {
                Ok(Format {
                    kind: FormatKind::parse(&row.get::<_, String>(0)?),
                    data: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if formats.is_empty() {
            self.get(id)?;
        }
        Ok(formats)
    }

    pub fn recent(&self, limit: usize, offset: usize) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {ENTRY_COLUMNS} FROM entries e
             ORDER BY e.pinned DESC, e.use_seq DESC LIMIT ?1 OFFSET ?2"
        ))?;
        let entries = stmt
            .query_map([to_i64(limit), to_i64(offset)], entry_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(entries)
    }

    pub fn touch(&mut self, id: EntryId) -> Result<()> {
        let tx = write_tx(&mut self.conn)?;
        let seq = next_seq(&tx)?;
        let changed = tx.execute(
            "UPDATE entries SET last_used_at = ?2, use_seq = ?3 WHERE id = ?1",
            params![id.0, Timestamp::now().as_millis(), seq],
        )?;
        expect_row(id, changed)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete(&mut self, id: EntryId) -> Result<()> {
        let changed = self
            .conn
            .execute("DELETE FROM entries WHERE id = ?1", [id.0])?;
        expect_row(id, changed)
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
