use eframe::egui::{self, Key, Modifiers};
use pin_core::Filter;

use crate::app::{App, Command, EntryAction, SheetAction};

const NUMBERS: [Key; 9] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];

impl App {
    pub(super) fn keyboard(&mut self, ctx: &egui::Context) -> Option<Command> {
        if egui::Popup::is_any_open(ctx) {
            return None;
        }
        let pressed = |key, modifiers| ctx.input_mut(|i| i.consume_key(modifiers, key));
        if self.input_quiet() {
            pressed(Key::Enter, Modifiers::NONE);
            return None;
        }
        if !self.recovery_key_open() && pressed(Key::Q, Modifiers::COMMAND) {
            return Some(Command::Quit);
        }
        if self.sheet.is_some() {
            if pressed(Key::Escape, Modifiers::NONE) {
                return Some(Command::Sheet(SheetAction::Cancel));
            }
            // Fields submit on Enter themselves; this covers sheets with no field in focus. A held
            // Enter that opened the sheet must not confirm it by key repeat.
            let typing = ctx.egui_wants_keyboard_input();
            return (!typing && fresh_enter(ctx)).then_some(Command::Sheet(SheetAction::Submit));
        }
        if pressed(Key::Enter, Modifiers::COMMAND) {
            self.toggle_actions();
            return None;
        }
        if self.actions.is_some() {
            return self.actions_keyboard(&pressed);
        }
        if !self.secrets_mode() {
            let step = if pressed(Key::Tab, Modifiers::SHIFT) {
                Some(Filter::ALL.len() - 1)
            } else {
                pressed(Key::Tab, Modifiers::NONE).then_some(1)
            };
            if let Some(step) = step {
                let current = Filter::ALL
                    .iter()
                    .position(|&f| f == self.filter)
                    .unwrap_or(0);
                self.set_filter(Filter::ALL[(current + step) % Filter::ALL.len()]);
                return None;
            }
        }
        if pressed(Key::ArrowUp, Modifiers::NONE) {
            self.select(self.selected.saturating_sub(1));
        }
        if pressed(Key::ArrowDown, Modifiers::NONE) {
            self.select(self.selected + 1);
        }
        if pressed(Key::Escape, Modifiers::NONE) {
            if self.query.is_empty() {
                return Some(Command::Hide);
            }
            if self.secrets_mode() && !self.secret_query().is_empty() {
                self.query = crate::app::PREFIX.to_string();
            } else {
                self.query.clear();
            }
            self.select(0);
            self.refresh();
            return None;
        }
        let number = NUMBERS
            .iter()
            .position(|&key| pressed(key, Modifiers::COMMAND));
        let (enter, pin, delete, secret) = (
            pressed(Key::Enter, Modifiers::NONE),
            pressed(Key::P, Modifiers::COMMAND),
            pressed(Key::Backspace, Modifiers::COMMAND),
            pressed(Key::S, Modifiers::COMMAND),
        );
        if self.secrets_mode() {
            return self.secret_keys(&pressed, number, enter, delete);
        }
        if let Some(index) = number {
            return self.entries.get(index).map(|entry| Command::Copy(entry.id));
        }
        let entry = self.entries.get(self.selected)?;
        if enter {
            Some(Command::Copy(entry.id))
        } else if pin {
            Some(Command::SetPinned(entry.id, !entry.pin.is_pinned()))
        } else if delete {
            Some(Command::Delete(entry.id))
        } else if secret {
            Some(Command::MoveToVault(entry.id))
        } else {
            None
        }
    }

    fn secret_keys(
        &self,
        pressed: &impl Fn(Key, Modifiers) -> bool,
        number: Option<usize>,
        enter: bool,
        delete: bool,
    ) -> Option<Command> {
        if pressed(Key::L, Modifiers::COMMAND) {
            return Some(Command::LockSecrets);
        }
        let edit = pressed(Key::E, Modifiers::COMMAND);
        if let Some(index) = number {
            return self.secrets.get(index).map(|s| Command::CopySecret(s.id));
        }
        let secret = self.secrets.get(self.selected)?;
        if enter {
            Some(Command::CopySecret(secret.id))
        } else if delete {
            Some(Command::DeleteSecret(secret.id))
        } else if edit {
            Some(Command::EditSecret(secret.id))
        } else {
            None
        }
    }

    fn actions_keyboard(&mut self, pressed: &impl Fn(Key, Modifiers) -> bool) -> Option<Command> {
        if pressed(Key::Escape, Modifiers::NONE) {
            self.actions_back();
            return None;
        }
        // Tab would move focus out of the panel's search field into the main search.
        pressed(Key::Tab, Modifiers::SHIFT);
        pressed(Key::Tab, Modifiers::NONE);
        let pinned = self.actions_entry().map(|e| e.pin.is_pinned());
        let shortcut = if pressed(Key::Backspace, Modifiers::COMMAND) {
            Some(EntryAction::Delete)
        } else if let Some(pinned) = pinned {
            if pressed(Key::P, Modifiers::COMMAND) {
                Some(EntryAction::Pin(!pinned))
            } else {
                pressed(Key::S, Modifiers::COMMAND).then_some(EntryAction::MoveToSecrets)
            }
        } else {
            pressed(Key::E, Modifiers::COMMAND).then_some(EntryAction::Edit)
        };
        if let Some(action) = shortcut {
            return self.choose(action);
        }
        let items = self.action_items();
        let actions = self.actions.as_mut()?;
        let last = items.len().saturating_sub(1);
        actions.highlighted = actions.highlighted.min(last);
        if pressed(Key::ArrowUp, Modifiers::NONE) {
            actions.highlighted = actions.highlighted.saturating_sub(1);
        }
        if pressed(Key::ArrowDown, Modifiers::NONE) {
            actions.highlighted = (actions.highlighted + 1).min(last);
        }
        if pressed(Key::Enter, Modifiers::NONE) {
            let action = items.get(actions.highlighted)?.action;
            return self.choose(action);
        }
        None
    }

    pub(super) fn select(&mut self, index: usize) {
        let index = index.min(self.len().saturating_sub(1));
        if index != self.selected {
            self.selected = index;
            self.scroll_to_selected = true;
        }
    }
}

/// Consumes a plain Enter press, ignoring auto-repeat.
fn fresh_enter(ctx: &egui::Context) -> bool {
    ctx.input_mut(|i| {
        let at = i.events.iter().position(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: Key::Enter,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } if modifiers.is_none()
            )
        });
        at.map(|at| i.events.remove(at)).is_some()
    })
}
