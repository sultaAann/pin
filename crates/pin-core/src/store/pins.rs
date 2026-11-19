use rusqlite::{OptionalExtension, params};

use super::{Store, entry_from_row, expect_row, write_tx};
use crate::error::Result;
use crate::model::{Entry, EntryId, PinSlot};

const BY_SLOT_SQL: &str = concat!(
    "SELECT ",
    entry_columns!(),
    " FROM entries e WHERE e.pin_slot = ?1"
);

impl Store {
    pub fn set_pinned(&mut self, id: EntryId, pinned: bool) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE entries SET pinned = ?2, pin_slot = CASE WHEN ?2 THEN pin_slot END
             WHERE id = ?1",
            params![id, pinned],
        )?;
        expect_row(id, changed)
    }

    pub fn set_slot(&mut self, id: EntryId, slot: Option<PinSlot>) -> Result<()> {
        let tx = write_tx(&mut self.conn)?;
        if let Some(slot) = slot {
            tx.execute(
                "UPDATE entries SET pin_slot = NULL WHERE pin_slot = ?1 AND id != ?2",
                params![slot, id],
            )?;
        }
        let changed = tx.execute(
            "UPDATE entries SET pin_slot = ?2, pinned = CASE WHEN ?2 IS NULL THEN pinned ELSE 1 END
             WHERE id = ?1",
            params![id, slot],
        )?;
        expect_row(id, changed)?;
        tx.commit()?;
        Ok(())
    }

    pub fn by_slot(&self, slot: PinSlot) -> Result<Option<Entry>> {
        Ok(self
            .conn
            .prepare_cached(BY_SLOT_SQL)?
            .query_row([slot], entry_from_row)
            .optional()?)
    }
}
