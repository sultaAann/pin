use pin_core::{Entry, EntryId, Pin, PinSlot, Secret, SecretId};

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
    Edit,
    Delete,
}

/// What the actions panel acts on: a history entry or a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Entry(EntryId),
    Secret(SecretId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub action: EntryAction,
    pub label: String,
    pub separated: bool,
    pub current: bool,
}

/// The actions panel opened with Cmd+Enter for one history entry or secret.
#[derive(Debug)]
pub struct Actions {
    pub target: Target,
    pub page: Page,
    pub query: String,
    pub highlighted: usize,
    /// Set on the frame the panel opens: it takes focus and ignores that frame's click.
    pub opened: bool,
}

impl EntryAction {
    /// What choosing this does to `target`; `AssignShortcut` only opens the slot list.
    pub fn command(self, target: Target) -> Option<Command> {
        Some(match (self, target) {
            (Self::Copy, Target::Entry(id)) => Command::Copy(id),
            (Self::Copy, Target::Secret(id)) => Command::CopySecret(id),
            (Self::Pin(pinned), Target::Entry(id)) => Command::SetPinned(id, pinned),
            (Self::Slot(slot), Target::Entry(id)) => Command::SetSlot(id, Some(slot)),
            (Self::RemoveShortcut, Target::Entry(id)) => Command::SetSlot(id, None),
            (Self::MoveToSecrets, Target::Entry(id)) => Command::MoveToVault(id),
            (Self::Edit, Target::Secret(id)) => Command::EditSecret(id),
            (Self::Delete, Target::Entry(id)) => Command::Delete(id),
            (Self::Delete, Target::Secret(id)) => Command::DeleteSecret(id),
            _ => return None,
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

    fn filtered(&self, items: Vec<Item>) -> Vec<Item> {
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

fn item(action: EntryAction, label: &str, separated: bool) -> Item {
    Item {
        action,
        label: label.to_owned(),
        separated,
        current: false,
    }
}

/// Every action on `entry` for one page, shared by the actions panel and the context menu.
pub fn entry_items(entry: &Entry, page: Page) -> Vec<Item> {
    match page {
        Page::Main => vec![
            item(EntryAction::Copy, "Copy", false),
            item(
                EntryAction::Pin(!entry.pin.is_pinned()),
                if entry.pin.is_pinned() {
                    "Unpin"
                } else {
                    "Pin"
                },
                false,
            ),
            item(EntryAction::AssignShortcut, "Assign Shortcut", false),
            item(EntryAction::MoveToSecrets, "Move to Secrets\u{2026}", true),
            item(EntryAction::Delete, "Delete", false),
        ],
        Page::Slots => {
            let mut slots: Vec<Item> = PinSlot::all()
                .map(|slot| Item {
                    current: entry.pin == Pin::Slot(slot),
                    label: slot_label(slot),
                    ..item(EntryAction::Slot(slot), "", false)
                })
                .collect();
            if matches!(entry.pin, Pin::Slot(_)) {
                slots.push(item(EntryAction::RemoveShortcut, "Remove Shortcut", true));
            }
            slots
        }
    }
}

/// Every action on a secret, shared by the actions panel and the context menu.
pub fn secret_items() -> Vec<Item> {
    vec![
        item(EntryAction::Copy, "Copy", false),
        item(EntryAction::Edit, "Edit Name and Note\u{2026}", false),
        item(EntryAction::Delete, "Delete", true),
    ]
}

impl App {
    pub(crate) fn toggle_actions(&mut self) {
        if self.actions.is_some() {
            self.close_actions();
            return;
        }
        if self.sheet.is_some() {
            return;
        }
        let target = if self.secrets_mode() {
            self.secrets
                .get(self.selected)
                .map(|s| Target::Secret(s.id))
        } else {
            self.entries.get(self.selected).map(|e| Target::Entry(e.id))
        };
        if let Some(target) = target {
            self.actions = Some(Actions {
                target,
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
        let Target::Entry(id) = self.actions.as_ref()?.target else {
            return None;
        };
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub(crate) fn actions_secret(&self) -> Option<&Secret> {
        let Target::Secret(id) = self.actions.as_ref()?.target else {
            return None;
        };
        self.secrets.iter().find(|secret| secret.id == id)
    }

    pub(crate) fn action_items(&self) -> Vec<Item> {
        let Some(actions) = &self.actions else {
            return Vec::new();
        };
        let items = match actions.target {
            Target::Entry(_) => match self.actions_entry() {
                Some(entry) => entry_items(entry, actions.page),
                None => Vec::new(),
            },
            Target::Secret(_) => secret_items(),
        };
        actions.filtered(items)
    }

    pub(crate) fn choose(&mut self, action: EntryAction) -> Option<Command> {
        let actions = self.actions.as_mut()?;
        let Some(command) = action.command(actions.target) else {
            if action == EntryAction::AssignShortcut {
                actions.show(Page::Slots);
            }
            return None;
        };
        self.close_actions();
        Some(command)
    }

    /// Keeps the selection on the panel's target when the list changes under it, e.g. after a
    /// background copy, and closes the panel once that target is gone.
    pub(super) fn prune_actions(&mut self) {
        let Some(target) = self.actions.as_ref().map(|a| a.target) else {
            return;
        };
        let index = match target {
            Target::Entry(id) if !self.secrets_mode() => {
                self.entries.iter().position(|entry| entry.id == id)
            }
            Target::Secret(id) if self.secrets_mode() => {
                self.secrets.iter().position(|secret| secret.id == id)
            }
            _ => None,
        };
        match index {
            Some(index) => self.selected = index,
            None => self.close_actions(),
        }
    }
}
