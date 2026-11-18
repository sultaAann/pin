use eframe::egui::{self, Key, Modifiers};

use crate::app::{App, Command};

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
            self.query.clear();
            self.select(0);
            self.refresh();
            return None;
        }
        if pressed(Key::Q, Modifiers::COMMAND) {
            return Some(Command::Quit);
        }
        if let Some(index) = NUMBERS
            .iter()
            .position(|&key| pressed(key, Modifiers::COMMAND))
        {
            return self.entries.get(index).map(|entry| Command::Copy(entry.id));
        }
        let (enter, pin, delete) = (
            pressed(Key::Enter, Modifiers::NONE),
            pressed(Key::P, Modifiers::COMMAND),
            pressed(Key::Backspace, Modifiers::COMMAND),
        );
        let entry = self.entries.get(self.selected)?;
        if enter {
            Some(Command::Copy(entry.id))
        } else if pin {
            Some(Command::SetPinned(entry.id, !entry.pin.is_pinned()))
        } else if delete {
            Some(Command::Delete(entry.id))
        } else {
            None
        }
    }

    pub(super) fn select(&mut self, index: usize) {
        let index = index.min(self.entries.len().saturating_sub(1));
        if index != self.selected {
            self.selected = index;
            self.scroll_to_selected = true;
        }
    }
}
