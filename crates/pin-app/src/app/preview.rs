use std::ops::Range;

use pin_core::{Details, EntryId};

use super::App;

/// Details of the selected history entry; only that entry is decrypted beyond its preview.
pub(crate) struct Preview {
    id: EntryId,
    shown: Option<Shown>,
}

pub(crate) struct Shown {
    pub details: Details,
    /// Per path in `details.files`: whether the file is gone. Checked once per selection, not
    /// per frame, since the popup repaints on every pointer move.
    pub missing: Vec<bool>,
    /// Search matches in `details.text` and the query they are for; up to 16 KiB is too much
    /// to scan again on every repaint.
    hits: (String, Vec<Range<usize>>),
}

impl Shown {
    pub fn hits(&self) -> &[Range<usize>] {
        &self.hits.1
    }
}

impl App {
    /// Details of the selected entry, with search matches in its text for `query`.
    pub(crate) fn selected_preview(&mut self, query: &str) -> Option<&Shown> {
        let id = self.entries.get(self.selected)?.id;
        if self.preview.as_ref().is_none_or(|p| p.id != id) {
            let shown = self
                .store
                .details(id)
                .inspect_err(|err| tracing::warn!(%err, "preview unavailable"))
                .ok()
                .map(|details| Shown {
                    missing: details
                        .files
                        .iter()
                        .map(|path| std::fs::symlink_metadata(path).is_err())
                        .collect(),
                    details,
                    hits: (String::new(), Vec::new()),
                });
            self.preview = Some(Preview { id, shown });
        }
        let shown = self.preview.as_mut()?.shown.as_mut()?;
        if shown.hits.0 != query {
            let ranges = shown
                .details
                .text
                .as_deref()
                .map(|text| pin_core::match_ranges(text, query))
                .unwrap_or_default();
            shown.hits = (query.to_owned(), ranges);
        }
        Some(shown)
    }

    pub(super) fn drop_preview(&mut self) {
        self.preview = None;
    }

    /// Ids are reused after a delete, so a cached preview must go with its entry: otherwise the
    /// next copy would show the old, possibly secret, text.
    pub(super) fn prune_preview(&mut self) {
        if let Some(preview) = &self.preview
            && !self.entries.iter().any(|entry| entry.id == preview.id)
        {
            self.preview = None;
        }
    }
}
