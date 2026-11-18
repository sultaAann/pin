use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, params};

use super::{ENTRY_COLUMNS, Store, entry_from_row, to_i64};
use crate::error::Result;
use crate::model::Entry;

const TRIGRAM: usize = 3;
const CONTAINS_FOLDED: &str = "pin_contains_folded";

impl Store {
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Entry>> {
        let query = query.trim();
        if query.is_empty() {
            return self.recent(limit, 0);
        }
        // Trigram MATCH needs at least 3 characters. Shorter queries scan only previews: scanning
        // all indexed text per keystroke is too slow, and SQLite LIKE folds case for ASCII only.
        let (sql, pattern) = if query.chars().count() >= TRIGRAM {
            (
                format!(
                    "SELECT {ENTRY_COLUMNS} FROM entries_fts f JOIN entries e ON e.id = f.rowid
                     WHERE f.text MATCH ?1
                     ORDER BY e.pinned DESC, e.use_seq DESC LIMIT ?2"
                ),
                format!("\"{}\"", query.replace('"', "\"\"")),
            )
        } else {
            (
                format!(
                    "SELECT {ENTRY_COLUMNS} FROM entries e
                     WHERE {CONTAINS_FOLDED}(e.preview, ?1)
                     ORDER BY e.pinned DESC, e.use_seq DESC LIMIT ?2"
                ),
                query.to_lowercase(),
            )
        };
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let entries = stmt
            .query_map(params![pattern, to_i64(limit)], entry_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(entries)
    }
}

pub(super) fn register_functions(conn: &Connection) -> rusqlite::Result<()> {
    conn.create_scalar_function(
        CONTAINS_FOLDED,
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let haystack = ctx.get_raw(0).as_str().unwrap_or_default();
            let needle = ctx.get_raw(1).as_str().unwrap_or_default();
            Ok(haystack.to_lowercase().contains(needle))
        },
    )
}
