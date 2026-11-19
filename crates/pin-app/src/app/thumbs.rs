use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::thread;

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use pin_core::{EntryId, Thumbnail};

use super::App;
use crate::events::{Event, Events};

enum Slot {
    Pending,
    Ready(TextureHandle),
    Missing,
}

/// Thumbnails are made on a worker with its own database connection: the first one for an entry
/// decodes the full image, which must not stall the popup.
#[derive(Default)]
pub(crate) struct Thumbnails {
    cache: HashMap<EntryId, Slot>,
    jobs: Option<Sender<EntryId>>,
}

impl Thumbnails {
    pub(super) fn spawn(events: Events) -> Self {
        let (tx, rx) = mpsc::channel::<EntryId>();
        let worker = thread::Builder::new()
            .name("pin-thumbnails".into())
            .spawn(move || {
                let mut store = match super::open_store() {
                    Ok(store) => store,
                    Err(err) => {
                        tracing::warn!(%err, "thumbnails disabled");
                        return;
                    }
                };
                for id in rx {
                    let thumbnail = store
                        .thumbnail(id)
                        .inspect_err(|err| tracing::warn!(%err, "thumbnail unavailable"))
                        .ok()
                        .flatten();
                    events.send(Event::Thumbnail(id, thumbnail));
                }
            });
        match worker {
            Ok(_) => Self {
                cache: HashMap::new(),
                jobs: Some(tx),
            },
            Err(err) => {
                tracing::warn!(%err, "thumbnails disabled");
                Self::default()
            }
        }
    }
}

impl App {
    pub(crate) fn thumbnail(&mut self, id: EntryId) -> Option<TextureHandle> {
        match self.thumbs.cache.get(&id) {
            Some(Slot::Ready(texture)) => return Some(texture.clone()),
            Some(Slot::Pending | Slot::Missing) => return None,
            None => {}
        }
        if let Some(jobs) = &self.thumbs.jobs
            && jobs.send(id).is_ok()
        {
            self.thumbs.cache.insert(id, Slot::Pending);
        }
        None
    }

    pub(super) fn thumbnail_ready(
        &mut self,
        ctx: &egui::Context,
        id: EntryId,
        thumb: Option<Thumbnail>,
    ) {
        if !self.thumbs.cache.contains_key(&id) {
            return;
        }
        let slot = thumb.map_or(Slot::Missing, |thumb| {
            let size = [
                usize::try_from(thumb.width).unwrap_or(0),
                usize::try_from(thumb.height).unwrap_or(0),
            ];
            let image = ColorImage::from_rgba_unmultiplied(size, &thumb.rgba);
            Slot::Ready(ctx.load_texture(format!("thumb-{id}"), image, TextureOptions::LINEAR))
        });
        self.thumbs.cache.insert(id, slot);
    }

    /// Drops textures of entries that left the list and lets failed ones be retried.
    pub(super) fn prune_thumbnails(&mut self) {
        let entries = &self.entries;
        self.thumbs.cache.retain(|id, slot| {
            !matches!(slot, Slot::Missing) && entries.iter().any(|entry| entry.id == *id)
        });
    }

    pub(super) fn drop_thumbnails(&mut self) {
        self.thumbs.cache.clear();
    }
}
