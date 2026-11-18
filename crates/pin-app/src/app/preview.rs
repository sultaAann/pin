use pin_core::{Details, EntryId};

use super::App;

/// Details of the selected history entry; only that entry is decrypted beyond its preview.
pub(crate) struct Preview {
    id: EntryId,
    details: Option<Details>,
}

impl App {
    pub(crate) fn selected_details(&mut self) -> Option<&Details> {
        let id = self.entries.get(self.selected)?.id;
        if self.preview.as_ref().is_none_or(|p| p.id != id) {
            let details = self
                .store
                .details(id)
                .inspect_err(|err| tracing::warn!(%err, "preview unavailable"))
                .ok();
            self.preview = Some(Preview { id, details });
        }
        self.preview.as_ref()?.details.as_ref()
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
