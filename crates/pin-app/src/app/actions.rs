use pin_core::{Entry, EntryId, Pin, PinSlot};

use super::{App, Command, Focus};
use crate::hotkeys::slot_label;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Main,
    Slots,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryAction {
    Copy,
    Pin(bool),
    AssignShortcut,
    Slot(PinSlot),
    RemoveShortcut,
    MoveToSecrets,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub action: EntryAction,
    pub label: String,
    pub separated: bool,
    pub current: bool,
}

/// The actions panel opened with Cmd+Enter for one history entry.
#[derive(Debug)]
pub struct Actions {
    pub entry: EntryId,
    pub page: Page,
    pub query: String,
    pub highlighted: usize,
    /// Set on the frame the panel opens: it takes focus and ignores that frame's click.
    pub opened: bool,
}

impl EntryAction {
    /// What choosing this does to entry `id`; `AssignShortcut` only opens the slot list.
    pub fn command(self, id: EntryId) -> Option<Command> {
        Some(match self {
            Self::AssignShortcut => return None,
            Self::Copy => Command::Copy(id),
            Self::Pin(pinned) => Command::SetPinned(id, pinned),
            Self::Slot(slot) => Command::SetSlot(id, Some(slot)),
            Self::RemoveShortcut => Command::SetSlot(id, None),
            Self::MoveToSecrets => Command::MoveToVault(id),
            Self::Delete => Command::Delete(id),
        })
    }
}

impl Actions {
    /// Switches to `page`; egui drops text focus on Esc and clicks, so the search field asks for
    /// it again on the next frame.
    fn show(&mut self, page: Page) {
        self.page = page;
        self.query.clear();
        self.highlighted = 0;
        self.opened = true;
    }

    pub fn items(&self, entry: &Entry) -> Vec<Item> {
        let items = entry_items(entry, self.page);
        let query = self.query.trim().to_lowercase();
        if query.is_empty() {
            return items;
        }
        items
            .into_iter()
            .filter(|item| item.label.to_lowercase().contains(&query))
            .map(|item| Item {
                separated: false,
                ..item
            })
            .collect()
    }
}

/// Every action on `entry` for one page, shared by the actions panel and the context menu.
pub fn entry_items(entry: &Entry, page: Page) -> Vec<Item> {
    let item = |action, label: String, separated| Item {
        action,
        label,
        separated,
        current: false,
    };
    match page {
        Page::Main => vec![
            item(EntryAction::Copy, "Copy".to_owned(), false),
            item(
                EntryAction::Pin(!entry.pin.is_pinned()),
                if entry.pin.is_pinned() {
                    "Unpin"
                } else {
                    "Pin"
                }
                .to_owned(),
                false,
            ),
            item(
                EntryAction::AssignShortcut,
                "Assign Shortcut".to_owned(),
                false,
            ),
            item(
                EntryAction::MoveToSecrets,
                "Move to Secrets\u{2026}".to_owned(),
                true,
            ),
            item(EntryAction::Delete, "Delete".to_owned(), false),
        ],
        Page::Slots => {
            let mut slots: Vec<Item> = PinSlot::all()
                .map(|slot| Item {
                    current: entry.pin == Pin::Slot(slot),
                    ..item(EntryAction::Slot(slot), slot_label(slot), false)
                })
                .collect();
            if matches!(entry.pin, Pin::Slot(_)) {
                slots.push(item(
                    EntryAction::RemoveShortcut,
                    "Remove Shortcut".to_owned(),
                    true,
                ));
            }
            slots
        }
    }
}

impl App {
    pub(crate) fn toggle_actions(&mut self) {
        if self.actions.is_some() {
            self.close_actions();
            return;
        }
        if self.secrets_mode() || self.sheet.is_some() {
            return;
        }
        if let Some(entry) = self.entries.get(self.selected) {
            self.actions = Some(Actions {
                entry: entry.id,
                page: Page::Main,
                query: String::new(),
                highlighted: 0,
                opened: true,
            });
        }
    }

    pub(crate) fn close_actions(&mut self) {
        if self.actions.take().is_some() {
            self.focus = Focus::Search;
        }
    }

    /// Esc leaves the shortcut page first, then closes the panel.
    pub(crate) fn actions_back(&mut self) {
        match &mut self.actions {
            Some(actions) if actions.page == Page::Slots => actions.show(Page::Main),
            _ => self.close_actions(),
        }
    }

    pub(crate) fn actions_entry(&self) -> Option<&Entry> {
        let id = self.actions.as_ref()?.entry;
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub(crate) fn action_items(&self) -> Vec<Item> {
        match (&self.actions, self.actions_entry()) {
            (Some(actions), Some(entry)) => actions.items(entry),
            _ => Vec::new(),
        }
    }

    pub(crate) fn choose(&mut self, action: EntryAction) -> Option<Command> {
        let actions = self.actions.as_mut()?;
        let Some(command) = action.command(actions.entry) else {
            actions.show(Page::Slots);
            return None;
        };
        self.close_actions();
        Some(command)
    }

    /// Keeps the selection on the panel's entry when the list changes under it, e.g. after a
    /// background copy, and closes the panel once that entry is gone.
    pub(super) fn prune_actions(&mut self) {
        let Some(id) = self.actions.as_ref().map(|a| a.entry) else {
            return;
        };
        match self.entries.iter().position(|entry| entry.id == id) {
            Some(index) => self.selected = index,
            None => self.close_actions(),
        }
    }
}
