mod format;
mod keys;
mod row;
mod theme;
mod vault;

use eframe::egui::{self, Align, Color32, FontId, Layout, RichText, ScrollArea, TextEdit, vec2};
use pin_core::Timestamp;

use self::row::Row;
use self::theme::{BACKSPACE, CMD, CORNER, Palette};
use crate::app::{App, Command};

pub use self::theme::{WINDOW_SIZE, install};

const SEARCH_HEIGHT: f32 = 52.0;
const FOOTER_HEIGHT: f32 = 32.0;

impl App {
    pub(crate) fn show_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if std::mem::take(&mut self.forget_text_state) {
            // egui keeps undo history of every text field; drop it so typed passwords don't linger.
            ctx.data_mut(egui::util::IdTypeMap::remove_by_type::<egui::text_edit::TextEditState>);
        }
        self.intercept_close(&ctx);
        self.track_focus(&ctx);
        let palette = Palette::of(&ctx);
        let mut command = self.keyboard(&ctx);
        egui::Frame::new()
            .fill(palette.background)
            .stroke(egui::Stroke::new(1.0, palette.border))
            .corner_radius(CORNER)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                self.search_bar(ui, &palette);
                divider(ui, &palette);
                let list_height = ui.available_height() - FOOTER_HEIGHT - 1.0;
                ui.allocate_ui(vec2(ui.available_width(), list_height), |ui| {
                    let chosen = if self.sheet.is_some() {
                        self.sheet_ui(ui, &palette)
                    } else if self.secrets_mode() {
                        self.secret_list(ui, &palette)
                    } else {
                        self.list(ui, &palette)
                    };
                    if chosen.is_some() {
                        command = chosen;
                    }
                });
                divider(ui, &palette);
                if let Some(chosen) = self.footer(ui, &palette) {
                    command = Some(chosen);
                }
            });
        if let Some(command) = command {
            self.run(&ctx, command);
        }
    }

    fn search_bar(&mut self, ui: &mut egui::Ui, palette: &Palette) {
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), SEARCH_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add_space(18.0);
                let glyph = if self.secrets_mode() {
                    "\u{1F511}"
                } else {
                    "\u{1F50D}"
                };
                ui.label(RichText::new(glyph).size(16.0).color(palette.secondary));
                ui.add_space(10.0);
                let search = ui.add(
                    TextEdit::singleline(&mut self.query)
                        .hint_text(
                            RichText::new("Search Clipboard \u{00B7} / for Secrets")
                                .color(palette.tertiary),
                        )
                        .font(FontId::proportional(19.0))
                        .text_color(palette.text)
                        .frame(egui::Frame::NONE)
                        .desired_width(ui.available_width() - 18.0),
                );
                if self.take_focus(crate::app::Focus::Search) {
                    search.request_focus();
                }
                if search.changed() {
                    self.selected = 0;
                    self.scroll_to_selected = true;
                    self.refresh();
                }
            },
        );
    }

    fn list(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        if self.entries.is_empty() {
            self.empty_state(ui, palette);
            return None;
        }
        let mut command = None;
        let pointer_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        let scroll = std::mem::take(&mut self.scroll_to_selected);
        let now = Timestamp::now();
        let sections =
            self.query.trim().is_empty() && self.entries.iter().any(|e| e.pin.is_pinned());
        let mut hovered = None;
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    for index in 0..self.entries.len() {
                        if sections && index == 0 {
                            section(ui, palette, "Pinned");
                        }
                        if sections
                            && index > 0
                            && !self.entries[index].pin.is_pinned()
                            && self.entries[index - 1].pin.is_pinned()
                        {
                            section(ui, palette, "Recent");
                        }
                        let next = egui::Rect::from_min_size(
                            ui.cursor().min,
                            vec2(ui.available_width(), row::height(&self.entries[index])),
                        );
                        let thumbnail = (self.entries[index].kind == pin_core::FormatKind::Png
                            && ui.is_rect_visible(next))
                        .then(|| self.thumbnail(self.entries[index].id))
                        .flatten();
                        let entry = &self.entries[index];
                        let selected = index == self.selected;
                        let output = Row {
                            entry,
                            index,
                            selected,
                            scroll: scroll && selected,
                            now,
                            thumbnail,
                        }
                        .show(ui, palette);
                        if output.hovered && pointer_moved {
                            hovered = Some(index);
                        }
                        if output.command.is_some() {
                            command = output.command;
                        }
                    }
                });
        });
        if let Some(index) = hovered
            && index != self.selected
        {
            self.selected = index;
            ui.ctx().request_repaint();
        }
        command
    }

    fn empty_state(&self, ui: &mut egui::Ui, palette: &Palette) {
        if self.query.trim().is_empty() {
            empty(
                ui,
                palette,
                "\u{1F4CB}",
                "No Clipboard History",
                "Copied text, images and files appear here.",
            );
        } else {
            let subtitle = format!("Nothing matches \u{201C}{}\u{201D}.", self.query.trim());
            empty(ui, palette, "\u{1F4CB}", "No Results", &subtitle);
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        let mut command = None;
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), FOOTER_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add_space(16.0);
                if let Some(error) = &self.error {
                    ui.add(
                        egui::Label::new(
                            RichText::new(error)
                                .size(11.5)
                                .color(ui.visuals().error_fg_color),
                        )
                        .truncate(),
                    );
                } else if self.sheet.is_none() {
                    let count = self.len();
                    let noun = match (self.secrets_mode(), count == 1) {
                        (true, true) => "secret",
                        (true, false) => "secrets",
                        (false, true) => "item",
                        (false, false) => "items",
                    };
                    ui.label(
                        RichText::new(format!("{count} {noun}"))
                            .size(11.5)
                            .color(palette.tertiary),
                    );
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(8.0);
                    if !self.recovery_key_open() {
                        ui.menu_button(
                            RichText::new("\u{22EF}")
                                .size(15.0)
                                .color(palette.secondary),
                            |ui| {
                                if ui
                                    .add(
                                        egui::Button::new("Quit Pin")
                                            .shortcut_text(format!("{CMD}Q")),
                                    )
                                    .clicked()
                                {
                                    command = Some(Command::Quit);
                                    ui.close();
                                }
                            },
                        );
                    }
                    ui.add_space(6.0);
                    let hints: &[(String, &str)] = if self.sheet.is_some() {
                        &[]
                    } else if self.secrets_mode() {
                        &[("\u{21A9}".to_owned(), "Copy \u{00B7} clears in 90 s")]
                    } else {
                        &[
                            (format!("{CMD}{BACKSPACE}"), "Delete"),
                            (format!("{CMD}S"), "Secret"),
                            (format!("{CMD}P"), "Pin"),
                            ("\u{21A9}".to_owned(), "Copy"),
                        ]
                    };
                    for (keys, action) in hints {
                        hint(ui, palette, keys, action);
                    }
                });
            },
        );
        command
    }
}

fn divider(ui: &mut egui::Ui, palette: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, palette.separator),
    );
}

fn empty(ui: &mut egui::Ui, palette: &Palette, glyph: &str, title: &str, subtitle: &str) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 2.0 - 48.0);
            ui.label(RichText::new(glyph).size(34.0).color(palette.tertiary));
            ui.add_space(10.0);
            ui.label(RichText::new(title).size(15.0).color(palette.text));
            ui.add_space(4.0);
            ui.label(RichText::new(subtitle).size(12.5).color(palette.secondary));
        });
    });
}

fn section(ui: &mut egui::Ui, palette: &Palette, title: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(title).size(11.0).color(palette.tertiary));
    });
    ui.add_space(4.0);
}

fn hint(ui: &mut egui::Ui, palette: &Palette, keys: &str, action: &str) {
    ui.add_space(12.0);
    ui.label(RichText::new(action).size(11.5).color(palette.tertiary));
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(palette.icon)
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(keys).size(11.0).color(palette.secondary));
        });
}

pub fn transparent() -> [f32; 4] {
    Color32::TRANSPARENT.to_normalized_gamma_f32()
}
