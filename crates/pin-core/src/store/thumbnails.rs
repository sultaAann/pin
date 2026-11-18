use rusqlite::{OptionalExtension, params};

use super::{Store, write_tx};
use crate::error::{Error, Result};
use crate::model::{EntryId, FormatKind};
use crate::thumbnail::{self, Thumbnail};

impl Store {
    /// Small preview of an image entry, made from its PNG on first request and kept in the database.
    /// The first call decodes the full image, so callers should keep it off latency-sensitive threads.
    pub fn thumbnail(&mut self, id: EntryId) -> Result<Option<Thumbnail>> {
        let stored: Option<Option<Vec<u8>>> = self
            .conn
            .prepare_cached("SELECT thumbnail FROM entries WHERE id = ?1")?
            .query_row([id], |row| row.get(0))
            .optional()?;
        let png = match stored.ok_or(Error::NotFound(id))? {
            Some(png) => png,
            None => self.generate(id)?,
        };
        Ok((!png.is_empty()).then(|| thumbnail::decode(&png)).flatten())
    }

    // Decoding happens before the write lock so pin-cli is not blocked for the length of a decode.
    fn generate(&mut self, id: EntryId) -> Result<Vec<u8>> {
        let source: Option<Vec<u8>> = self
            .conn
            .prepare_cached("SELECT data FROM formats WHERE entry_id = ?1 AND kind = ?2")?
            .query_row(params![id, FormatKind::Png], |row| row.get(0))
            .optional()?;
        let png = source
            .and_then(|data| thumbnail::encode(&data))
            .unwrap_or_default();
        let tx = write_tx(&mut self.conn)?;
        tx.prepare_cached("UPDATE entries SET thumbnail = ?2 WHERE id = ?1 AND thumbnail IS NULL")?
            .execute(params![id, png])?;
        tx.commit()?;
        Ok(png)
    }
}
