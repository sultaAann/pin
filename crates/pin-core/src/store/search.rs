use std::borrow::Cow;
use std::ops::Range;

use rusqlite::functions::FunctionFlags;
use rusqlite::types::FromSqlError;
use rusqlite::{Connection, params};

use super::{Store, entry_from_row, to_i64};
use crate::error::Result;
use crate::filter::Filter;
use crate::model::Entry;

macro_rules! contains_folded {
    () => {
        "pin_contains_folded"
    };
}

const MIN_TRIGRAM_CHARS: usize = 3;

impl Store {
    pub fn search(&self, query: &str, filter: Filter, limit: usize) -> Result<Vec<Entry>> {
        let query = query.trim();
        if query.is_empty() {
            return self.recent_in(filter, limit, 0);
        }
        // Trigram MATCH needs at least 3 characters. Shorter queries scan only previews: scanning
        // all indexed text per keystroke is too slow, and SQLite LIKE folds case for ASCII only.
        let (sql, pattern) = if query.chars().count() >= MIN_TRIGRAM_CHARS {
            (
                format!(
                    concat!(
                        "SELECT ",
                        entry_columns!(),
                        " FROM entries_fts f JOIN entries e ON e.id = f.rowid
                          WHERE f.text MATCH ?1 AND {}
                          ORDER BY e.pinned DESC, e.use_seq DESC LIMIT ?2"
                    ),
                    filter.sql()
                ),
                Cow::Owned(format!("\"{}\"", query.replace('"', "\"\""))),
            )
        } else {
            (
                format!(
                    concat!(
                        "SELECT ",
                        entry_columns!(),
                        " FROM entries e WHERE ",
                        contains_folded!(),
                        "(e.preview, ?1) AND {} ORDER BY e.pinned DESC, e.use_seq DESC LIMIT ?2"
                    ),
                    filter.sql()
                ),
                Cow::Borrowed(query),
            )
        };
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let entries = stmt
            .query_map(params![pattern, to_i64(limit)], entry_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(entries)
    }
}

struct FoldedNeedle {
    text: String,
    ascii: bool,
}

// Lowercasing per char maps Σ to σ, while str::to_lowercase would give ς at a word end; folding
// both sigmas to σ keeps "ος" matching "ΟΔΟΣ" on either side.
fn fold(c: char) -> impl Iterator<Item = char> {
    c.to_lowercase().map(|l| if l == 'ς' { 'σ' } else { l })
}

impl FoldedNeedle {
    fn new(text: &str) -> Self {
        let text: String = text.chars().flat_map(fold).collect();
        let ascii = text.is_ascii();
        Self { text, ascii }
    }
}

fn contains_folded(haystack: &str, needle: &FoldedNeedle) -> bool {
    if needle.text.is_empty() {
        return true;
    }
    if needle.ascii && haystack.is_ascii() {
        return haystack
            .as_bytes()
            .windows(needle.text.len())
            .any(|window| window.eq_ignore_ascii_case(needle.text.as_bytes()));
    }
    haystack.char_indices().any(|(start, _)| {
        let mut folded = haystack[start..].chars().flat_map(fold);
        needle.text.chars().all(|c| folded.next() == Some(c))
    })
}

/// Byte ranges of `haystack` that match `query` with the same case folding as search, for
/// highlighting results. Ranges do not overlap and always end on a char boundary.
pub fn match_ranges(haystack: &str, query: &str) -> Vec<Range<usize>> {
    let needle = FoldedNeedle::new(query.trim());
    let mut ranges = Vec::new();
    if needle.text.is_empty() {
        return ranges;
    }
    let mut from = 0;
    for (start, _) in haystack.char_indices() {
        if start < from {
            continue;
        }
        if let Some(len) = match_len(&haystack[start..], &needle.text) {
            ranges.push(start..start + len);
            from = start + len;
        }
    }
    ranges
}

fn match_len(haystack: &str, needle: &str) -> Option<usize> {
    let mut wanted = needle.chars().peekable();
    for (offset, c) in haystack.char_indices() {
        for folded in fold(c) {
            match wanted.next() {
                Some(w) if w == folded => {}
                Some(_) => return None,
                None => break,
            }
        }
        if wanted.peek().is_none() {
            return Some(offset + c.len_utf8());
        }
    }
    None
}

pub(super) fn register_functions(conn: &Connection) -> rusqlite::Result<()> {
    conn.create_scalar_function(
        contains_folded!(),
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let needle = ctx.get_or_create_aux(1, |value| {
                Ok::<_, FromSqlError>(FoldedNeedle::new(value.as_str()?))
            })?;
            let haystack = ctx.get_raw(0).as_str().unwrap_or_default();
            Ok(contains_folded(haystack, &needle))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(haystack: &str, needle: &str) -> bool {
        contains_folded(haystack, &FoldedNeedle::new(needle))
    }

    #[test]
    fn ascii_and_unicode_case_are_folded() {
        assert!(found("Hello World", "LO W"));
        assert!(!found("Hello", "xyz"));
        assert!(found("Привет мир", "ПРИ"));
        assert!(found("Привет мир", "ир"));
        assert!(found("ascii only", "ONLY"));
        assert!(!found("", "a"));
        assert!(!found("é", "e"));
        assert!(found("ΟΔΟΣ", "ος"));
        assert!(found("οδος", "ΟΣ"));
        assert!(found("İstanbul", "i"));
    }

    #[test]
    fn match_ranges_cover_every_folded_occurrence() {
        assert_eq!(match_ranges("Hello hello", " LL "), [2..4, 8..10]);
        assert_eq!(match_ranges("Привет мир", "при"), vec![0..6]);
        assert_eq!(match_ranges("İstanbul", "i"), vec![0..2]);
        assert_eq!(match_ranges("aaaa", "aa"), [0..2, 2..4]);
        assert!(match_ranges("abc", "").is_empty());
        assert!(match_ranges("abc", "abcd").is_empty());
    }
}
