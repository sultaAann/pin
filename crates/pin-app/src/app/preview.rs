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
}

impl App {
    pub(crate) fn selected_preview(&mut self) -> Option<&Shown> {
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
                });
            self.preview = Some(Preview { id, shown });
        }
        self.preview.as_ref()?.shown.as_ref()
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
