use std::fmt;

use super::Store;
use crate::content;
use crate::error::Result;
use crate::model::{EntryId, FormatKind};

const MAX_DETAIL_TEXT_BYTES: usize = 16 * 1024;

/// What a preview of one entry shows: its format kinds, the start of its plain text and its file
/// paths. Image data is never loaded.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Details {
    pub formats: Vec<FormatKind>,
    pub text: Option<String>,
    pub files: Vec<String>,
}

impl fmt::Debug for Details {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Details")
            .field("formats", &self.formats)
            .field("text_len", &self.text.as_ref().map(String::len))
            .field("files", &self.files.len())
            .finish()
    }
}

impl Store {
    pub fn details(&self, id: EntryId) -> Result<Details> {
        let rows = self
            .conn
            .prepare_cached("SELECT id, kind FROM formats WHERE entry_id = ?1 ORDER BY kind")?
            .query_map([id], |row| Ok((row.get::<_, i64>(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<(i64, FormatKind)>>>()?;
        if rows.is_empty() {
            self.get(id)?;
        }
        let mut details = Details::default();
        for (row, kind) in rows {
            // One byte past the limit lets the cut land on a char or line boundary.
            match kind {
                FormatKind::Text => {
                    let data = self.read_prefix(row, MAX_DETAIL_TEXT_BYTES + 1)?;
                    details.text = Some(content::display_text(&data, MAX_DETAIL_TEXT_BYTES));
                }
                FormatKind::Files => {
                    let data = self.read_prefix(row, content::MAX_INDEXED_BYTES + 1)?;
                    details.files = content::file_paths(&data);
                }
                _ => {}
            }
            details.formats.push(kind);
        }
        Ok(details)
    }

    // Incremental blob I/O reads only the pages it needs; substr() would load and decrypt the
    // whole value first, which for a large copy stalls the UI thread on every selection change.
    fn read_prefix(&self, row: i64, max: usize) -> Result<Vec<u8>> {
        let blob = self.conn.blob_open("main", "formats", "data", row, true)?;
        let mut data = vec![0; blob.len().min(max)];
        blob.read_at_exact(&mut data, 0)?;
        Ok(data)
    }
}
