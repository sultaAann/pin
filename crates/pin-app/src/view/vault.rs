use eframe::egui::{self, Align, Color32, FontId, Key, Layout, RichText, Sense, TextEdit, vec2};
use pin_core::{Secret, Timestamp};

use super::format;
use super::row::{HEIGHT, Look, index_hint, title};
use super::theme::Palette;
use crate::app::{App, Command, LabelTarget, Sheet, SheetAction};

const SHEET_WIDTH: f32 = 300.0;
const KEY_GLYPH: &str = "\u{1F511}";
const LOCK_GLYPH: &str = "\u{1F512}";

impl App {
    pub(super) fn secret_list(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        if self.secrets.is_empty() {
            let filtered = !self.query[crate::app::PREFIX.len_utf8()..]
                .trim()
                .is_empty();
            let (headline, subtitle) = if filtered {
                ("No Matching Secrets", "Try a different name or note.")
            } else {
                (
                    "No Secrets Yet",
                    "Right-click an item in history and choose Move to Secrets.",
                )
            };
            super::empty(ui, palette, KEY_GLYPH, headline, subtitle);
            return None;
        }
        let mut command = None;
        let mut hovered = None;
        let pointer_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        let scroll = std::mem::take(&mut self.scroll_to_selected);
        let now = Timestamp::now();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        super::section(ui, palette, "Secrets");
                        for (index, secret) in self.secrets.iter().enumerate() {
                            let selected = index == self.selected;
                            let (chosen, is_hovered) = SecretRow {
                                secret,
                                index,
                                selected,
                                scroll: scroll && selected,
                                now,
                            }
                            .show(ui, palette);
                            if is_hovered && pointer_moved {
                                hovered = Some(index);
                            }
                            if chosen.is_some() {
                                command = chosen;
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

    pub(super) fn sheet_ui(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        let focus = self.take_focus(crate::app::Focus::Sheet);
        let quiet = self.quiet_for();
        if let Some(wait) = quiet {
            ui.ctx().request_repaint_after(wait);
        }
        let error = self.sheet_error.as_deref();
        let sheet = self.sheet.as_mut()?;
        let mut command = None;
        ui.add_space(26.0);
        let (glyph, headline, subtitle) = copy_for(sheet);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(glyph).size(28.0).color(palette.secondary));
            ui.add_space(8.0);
            ui.label(RichText::new(headline).size(16.0).color(palette.text));
            ui.add_space(4.0);
            ui.set_max_width(SHEET_WIDTH + 60.0);
            ui.label(RichText::new(subtitle).size(12.5).color(palette.secondary));
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - SHEET_WIDTH) / 2.0).max(0.0));
            ui.vertical(|ui| {
                ui.set_width(SHEET_WIDTH);
                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                if let Some(chosen) = fields(ui, palette, sheet, focus) {
                    command = Some(chosen);
                }
                if let Some(error) = error {
                    ui.label(
                        RichText::new(error)
                            .size(12.0)
                            .color(ui.visuals().error_fg_color),
                    );
                }
                ui.add_space(6.0);
                if let Some(chosen) = buttons(ui, palette, sheet, quiet.is_none()) {
                    command = Some(chosen);
                }
            });
        });
        command
    }
}

fn copy_for(sheet: &Sheet) -> (&'static str, &'static str, &'static str) {
    match sheet {
        Sheet::Create { .. } => (
            LOCK_GLYPH,
            "Set Up Secrets",
            "Choose a master password. Secrets are encrypted with it and never appear in history.",
        ),
        Sheet::Unlock { .. } => (
            LOCK_GLYPH,
            "Secrets Are Locked",
            "Enter your master password to continue.",
        ),
        Sheet::Recover { .. } => (
            KEY_GLYPH,
            "Recover Secrets",
            "Enter your recovery key and choose a new master password.",
        ),
        Sheet::RecoveryKey { .. } => (
            KEY_GLYPH,
            "Save Your Recovery Key",
            "It is the only way back in if you forget the master password. It is shown once.",
        ),
        Sheet::Label {
            target: LabelTarget::Move(_),
            ..
        } => (
            KEY_GLYPH,
            "Move to Secrets",
            "The item leaves history and is stored encrypted.",
        ),
        Sheet::Label {
            target: LabelTarget::Edit(_),
            ..
        } => (KEY_GLYPH, "Edit Secret", "Change its name or note."),
        Sheet::ConfirmDelete { .. } => (
            KEY_GLYPH,
            "Delete This Secret?",
            "It is removed from the vault for good. This can't be undone.",
        ),
    }
}

fn fields(ui: &mut egui::Ui, palette: &Palette, sheet: &mut Sheet, focus: bool) -> Option<Command> {
    let kind = sheet.kind();
    let field = |ui: &mut egui::Ui, label: &str, text: &mut String, password: bool, focus: bool| {
        field(ui, palette, (kind, label), text, password, focus)
    };
    let mut entered = false;
    match sheet {
        Sheet::Create {
            password, confirm, ..
        } => {
            entered |= field(ui, "Master Password", password, true, focus);
            entered |= field(ui, "Confirm Password", confirm, true, false);
        }
        Sheet::Unlock { password, .. } => {
            entered |= field(ui, "Master Password", password, true, focus);
            let forgot = ui.add(
                egui::Label::new(
                    RichText::new("Forgot Password?")
                        .size(12.0)
                        .color(palette.accent),
                )
                .sense(Sense::click()),
            );
            if forgot
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                return Some(Command::Sheet(SheetAction::Forgot));
            }
        }
        Sheet::Recover {
            key,
            password,
            confirm,
            ..
        } => {
            entered |= field(ui, "Recovery Key", key, false, focus);
            entered |= field(ui, "New Master Password", password, true, false);
            entered |= field(ui, "Confirm Password", confirm, true, false);
        }
        Sheet::RecoveryKey { key, .. } => {
            egui::Frame::new()
                .fill(palette.icon)
                .corner_radius(8)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(
                        egui::Label::new(
                            RichText::new(key.as_str())
                                .font(FontId::monospace(13.0))
                                .color(palette.text),
                        )
                        .wrap(),
                    );
                });
        }
        Sheet::Label { name, note, .. } => {
            entered |= field(ui, "Name", name, false, focus);
            entered |= field(ui, "Note (Optional)", note, false, false);
        }
        Sheet::ConfirmDelete { .. } => {}
    }
    entered.then_some(Command::Sheet(SheetAction::Submit))
}

/// Returns whether Enter was pressed in this field.
fn field(
    ui: &mut egui::Ui,
    palette: &Palette,
    id: (&str, &str),
    text: &mut String,
    password: bool,
    focus: bool,
) -> bool {
    ui.label(RichText::new(id.1).size(11.5).color(palette.secondary));
    let response = ui.add(
        TextEdit::singleline(text)
            .id_salt(id)
            .password(password)
            .font(FontId::proportional(14.0))
            .margin(vec2(8.0, 6.0))
            .desired_width(f32::INFINITY),
    );
    if focus {
        response.request_focus();
    }
    response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
}

// Disabled briefly after a password check so clicks made while it blocked don't hit the next screen.
fn buttons(ui: &mut egui::Ui, palette: &Palette, sheet: &Sheet, enabled: bool) -> Option<Command> {
    let mut command = None;
    let primary = match sheet {
        Sheet::Create { .. } => "Create",
        Sheet::Unlock { .. } => "Unlock",
        Sheet::Recover { .. } => "Recover",
        Sheet::RecoveryKey { .. } => "I\u{2019}ve Saved It",
        Sheet::Label {
            target: LabelTarget::Move(_),
            ..
        } => "Move",
        Sheet::Label { .. } => "Save",
        Sheet::ConfirmDelete { .. } => "Delete",
    };
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        vec2(width, 28.0),
        Layout::right_to_left(Align::Center),
        |ui| {
            let main = egui::Button::new(RichText::new(primary).color(Color32::WHITE))
                .fill(palette.accent)
                .min_size(vec2(84.0, 26.0));
            if ui.add_enabled(enabled, main).clicked() {
                command = Some(Command::Sheet(SheetAction::Submit));
            }
            let secondary = if matches!(sheet, Sheet::RecoveryKey { .. }) {
                ("Copy", SheetAction::CopyRecoveryKey)
            } else {
                ("Cancel", SheetAction::Cancel)
            };
            if ui
                .add_enabled(
                    enabled,
                    egui::Button::new(secondary.0).min_size(vec2(72.0, 26.0)),
                )
                .clicked()
            {
                command = Some(Command::Sheet(secondary.1));
            }
        },
    );
    command
}

struct SecretRow<'a> {
    secret: &'a Secret,
    index: usize,
    selected: bool,
    scroll: bool,
    now: Timestamp,
}

impl SecretRow<'_> {
    fn show(self, ui: &mut egui::Ui, palette: &Palette) -> (Option<Command>, bool) {
        let SecretRow {
            secret,
            index,
            selected,
            scroll,
            now,
        } = self;
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::click());
        if scroll {
            response.scroll_to_me(None);
        }
        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            let look = Look::paint(
                &painter,
                rect,
                selected,
                response.hovered(),
                palette,
                KEY_GLYPH,
            );
            let right = index_hint(&painter, rect, index, look.secondary);
            title(&painter, rect, look.left, right, &secret.name, look.text);
        }
        let hovered = response.hovered();
        let mut command = response.clicked().then_some(Command::CopySecret(secret.id));
        let response = response.on_hover_ui(|ui| {
            if !secret.note.is_empty() {
                ui.label(&secret.note);
            }
            ui.label(format!(
                "Updated {}",
                format::relative(now, secret.updated_at).to_lowercase()
            ));
        });
        response.context_menu(|ui| {
            if ui
                .add(egui::Button::new("Copy").shortcut_text("\u{21A9}"))
                .clicked()
            {
                command = Some(Command::CopySecret(secret.id));
                ui.close();
            }
            if ui.button("Edit Name and Note\u{2026}").clicked() {
                command = Some(Command::EditSecret(secret.id));
                ui.close();
            }
            ui.separator();
            if ui.button("Delete").clicked() {
                command = Some(Command::DeleteSecret(secret.id));
                ui.close();
            }
        });
        (command, hovered)
    }
}
