use eframe::egui::{self, Key, Modifiers, RichText, ScrollArea, TextEdit};
use pin_core::{Entry, Pin, PinSlot};

use crate::app::{App, Command};
use crate::hotkeys::SLOT_LABEL;

const PREVIEW_CHARS: usize = 80;

impl App {
    pub(crate) fn show_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.intercept_close(&ctx);
        self.track_focus(&ctx);
        let mut command = self.keyboard(&ctx);
        egui::CentralPanel::default().show(ui, |ui| {
            let search = ui.add(
                TextEdit::singleline(&mut self.query)
                    .hint_text("Search")
                    .desired_width(f32::INFINITY),
            );
            if std::mem::take(&mut self.focus_search) {
                search.request_focus();
            }
            if search.changed() {
                self.selected = 0;
                self.refresh();
            }
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.separator();
            ScrollArea::vertical()
                .auto_shrink(false)
                .max_height(ui.available_height() - 28.0)
                .show(ui, |ui| {
                    let scroll = std::mem::take(&mut self.scroll_to_selected);
                    for (index, entry) in self.entries.iter().enumerate() {
                        let selected = index == self.selected;
                        if let Some(row) = row(ui, entry, selected, scroll && selected) {
                            command = Some(row);
                        }
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                ui.weak(format!("{} items", self.entries.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        command = Some(Command::Quit);
                    }
                });
            });
        });
        if let Some(command) = command {
            self.run(&ctx, command);
        }
    }

    fn keyboard(&mut self, ctx: &egui::Context) -> Option<Command> {
        if egui::Popup::is_any_open(ctx) {
            return None;
        }
        let (up, down, enter, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if up {
            self.selected = self.selected.saturating_sub(1);
            self.scroll_to_selected = true;
        }
        if down && self.selected + 1 < self.entries.len() {
            self.selected += 1;
            self.scroll_to_selected = true;
        }
        if escape {
            return Some(Command::Hide);
        }
        if enter {
            return self.entries.get(self.selected).map(|e| Command::Copy(e.id));
        }
        None
    }
}

fn row(ui: &mut egui::Ui, entry: &Entry, selected: bool, scroll: bool) -> Option<Command> {
    let mut command = None;
    let response = ui
        .horizontal(|ui| {
            let label = ui.add_sized(
                [ui.available_width() - 64.0, 20.0],
                egui::Button::selectable(selected, preview(entry)).truncate(),
            );
            match entry.pin {
                Pin::Slot(slot) => ui.weak(format!("{SLOT_LABEL}{}", slot.get())),
                Pin::Pinned => ui.weak("pinned"),
                Pin::None => ui.weak(""),
            };
            label
        })
        .inner;
    if scroll {
        response.scroll_to_me(None);
    }
    if response.clicked() {
        command = Some(Command::Copy(entry.id));
    }
    response.context_menu(|ui| {
        let pinned = entry.pin.is_pinned();
        if ui.button(if pinned { "Unpin" } else { "Pin" }).clicked() {
            command = Some(Command::SetPinned(entry.id, !pinned));
            ui.close();
        }
        ui.menu_button("Hotkey slot", |ui| {
            for slot in PinSlot::all() {
                if ui.button(format!("{SLOT_LABEL}{}", slot.get())).clicked() {
                    command = Some(Command::SetSlot(entry.id, Some(slot)));
                    ui.close();
                }
            }
            if ui.button("None").clicked() {
                command = Some(Command::SetSlot(entry.id, None));
                ui.close();
            }
        });
        if ui.button("Delete").clicked() {
            command = Some(Command::Delete(entry.id));
            ui.close();
        }
    });
    command
}

fn preview(entry: &Entry) -> RichText {
    let line = entry
        .preview
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_else(|| entry.kind.as_str());
    RichText::new(line.chars().take(PREVIEW_CHARS).collect::<String>())
}
