use eframe::egui::{
    self, Align2, Color32, FontId, Rect, Sense, Stroke, TextFormat, TextureHandle, Vec2, pos2,
    text::LayoutJob, vec2,
};
use pin_core::{Entry, FormatKind, Pin, PinSlot, Timestamp};

use super::format;
use super::theme::{BACKSPACE, CMD, Palette};
use crate::app::Command;
use crate::hotkeys::SLOT_LABEL;

pub(super) const HEIGHT: f32 = 36.0;
const IMAGE_HEIGHT: f32 = 60.0;
const ICON: f32 = 24.0;
const THUMB: Vec2 = Vec2::new(88.0, 48.0);

pub struct Row<'a> {
    pub entry: &'a Entry,
    pub index: usize,
    pub selected: bool,
    pub scroll: bool,
    pub now: Timestamp,
    pub thumbnail: Option<TextureHandle>,
}

pub fn height(entry: &Entry) -> f32 {
    if entry.kind == FormatKind::Png {
        IMAGE_HEIGHT
    } else {
        HEIGHT
    }
}

pub struct RowOutput {
    pub command: Option<Command>,
    pub hovered: bool,
}

impl Row<'_> {
    pub fn show(self, ui: &mut egui::Ui, palette: &Palette) -> RowOutput {
        let (rect, response) = ui.allocate_exact_size(
            vec2(ui.available_width(), height(self.entry)),
            Sense::click(),
        );
        if self.scroll {
            response.scroll_to_me(None);
        }
        if ui.is_rect_visible(rect) {
            self.paint(ui, rect, response.hovered(), palette);
        }
        let mut command = response.clicked().then_some(Command::Copy(self.entry.id));
        let hovered = response.hovered();
        let response = response.on_hover_ui(|ui| {
            ui.label(format::details(self.entry, self.now));
        });
        response.context_menu(|ui| {
            if let Some(chosen) = context_menu(ui, self.entry) {
                command = Some(chosen);
                ui.close();
            }
        });
        RowOutput { command, hovered }
    }

    fn paint(&self, ui: &egui::Ui, rect: Rect, hovered: bool, palette: &Palette) {
        let painter = ui.painter_at(rect);
        let look = match &self.thumbnail {
            Some(texture) => {
                Look::with_thumbnail(ui, rect, self.selected, hovered, palette, texture)
            }
            None => Look::paint(
                &painter,
                rect,
                self.selected,
                hovered,
                palette,
                format::icon(self.entry),
            ),
        };
        let (text, secondary) = (look.text, look.secondary);
        let mut right = index_hint(&painter, rect, self.index, secondary);
        match self.entry.pin {
            Pin::Slot(slot) => {
                right = badge(
                    &painter,
                    right,
                    rect.center().y,
                    &slot_label(slot),
                    secondary,
                );
            }
            Pin::Pinned => {
                let pin = painter.text(
                    pos2(right, rect.center().y),
                    Align2::RIGHT_CENTER,
                    "\u{1F4CC}",
                    FontId::proportional(11.0),
                    secondary,
                );
                right = pin.left() - 8.0;
            }
            Pin::None => {}
        }

        title(
            &painter,
            rect,
            look.left,
            right,
            format::title(self.entry),
            text,
        );
    }
}

pub(super) struct Look {
    pub text: Color32,
    pub secondary: Color32,
    pub left: f32,
}

impl Look {
    fn background(
        painter: &egui::Painter,
        rect: Rect,
        selected: bool,
        hovered: bool,
        palette: &Palette,
    ) -> (Color32, Color32) {
        if selected {
            painter.rect_filled(rect, 7.0, palette.accent);
            (Color32::WHITE, Color32::from_white_alpha(200))
        } else {
            if hovered {
                painter.rect_filled(rect, 7.0, palette.hover);
            }
            (palette.text, palette.secondary)
        }
    }

    fn with_thumbnail(
        ui: &egui::Ui,
        rect: Rect,
        selected: bool,
        hovered: bool,
        palette: &Palette,
        texture: &TextureHandle,
    ) -> Self {
        let painter = &ui.painter_at(rect);
        let (text, secondary) = Self::background(painter, rect, selected, hovered, palette);
        let slot = Rect::from_min_size(
            pos2(rect.left() + 8.0, rect.center().y - THUMB.y / 2.0),
            THUMB,
        );
        painter.rect_filled(slot, 5.0, palette.icon);
        let size = texture.size_vec2();
        let scale = (THUMB.x / size.x).min(THUMB.y / size.y).min(1.0);
        let image = Rect::from_center_size(slot.center(), size * scale);
        egui::Image::from_texture((texture.id(), image.size()))
            .corner_radius(4)
            .paint_at(ui, image);
        Self {
            text,
            secondary,
            left: slot.right() + 12.0,
        }
    }

    pub(super) fn paint(
        painter: &egui::Painter,
        rect: Rect,
        selected: bool,
        hovered: bool,
        palette: &Palette,
        glyph: &str,
    ) -> Self {
        let (text, secondary) = Self::background(painter, rect, selected, hovered, palette);
        let icon = Rect::from_center_size(
            pos2(rect.left() + 8.0 + ICON / 2.0, rect.center().y),
            vec2(ICON, ICON),
        );
        let icon_fill = if selected {
            Color32::from_white_alpha(46)
        } else {
            palette.icon
        };
        painter.rect_filled(icon, 6.0, icon_fill);
        painter.text(
            icon.center(),
            Align2::CENTER_CENTER,
            glyph,
            FontId::proportional(12.0),
            text,
        );
        Self {
            text,
            secondary,
            left: icon.right() + 10.0,
        }
    }
}

pub(super) fn index_hint(painter: &egui::Painter, rect: Rect, index: usize, color: Color32) -> f32 {
    let right = rect.right() - 10.0;
    if index >= 9 {
        return right;
    }
    let hint = painter.text(
        pos2(right, rect.center().y),
        Align2::RIGHT_CENTER,
        format!("{CMD}{}", index + 1),
        FontId::proportional(12.0),
        color,
    );
    hint.left() - 10.0
}

pub(super) fn title(
    painter: &egui::Painter,
    rect: Rect,
    left: f32,
    right: f32,
    text: &str,
    color: Color32,
) {
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat::simple(FontId::proportional(13.5), color),
    );
    job.wrap.max_width = (right - left).max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = painter.layout_job(job);
    painter.galley(
        pos2(left, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
}

fn badge(painter: &egui::Painter, right: f32, y: f32, label: &str, color: Color32) -> f32 {
    let galley = painter.layout_no_wrap(label.to_owned(), FontId::proportional(11.0), color);
    let rect = Rect::from_min_size(
        pos2(right - galley.size().x - 10.0, y - 9.0),
        vec2(galley.size().x + 10.0, 18.0),
    );
    painter.rect_stroke(
        rect,
        5.0,
        Stroke::new(1.0, color.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        pos2(rect.left() + 5.0, y - galley.size().y / 2.0),
        galley,
        color,
    );
    rect.left() - 8.0
}

fn slot_label(slot: PinSlot) -> String {
    format!("{SLOT_LABEL}{}", slot.get())
}

fn context_menu(ui: &mut egui::Ui, entry: &Entry) -> Option<Command> {
    let mut command = None;
    if ui
        .add(egui::Button::new("Copy").shortcut_text("\u{21A9}"))
        .clicked()
    {
        command = Some(Command::Copy(entry.id));
    }
    let pinned = entry.pin.is_pinned();
    let pin = if pinned { "Unpin" } else { "Pin" };
    if ui
        .add(egui::Button::new(pin).shortcut_text(format!("{CMD}P")))
        .clicked()
    {
        command = Some(Command::SetPinned(entry.id, !pinned));
    }
    ui.menu_button("Assign Shortcut", |ui| {
        for slot in PinSlot::all() {
            let current = entry.pin == Pin::Slot(slot);
            if ui
                .add(egui::Button::selectable(current, slot_label(slot)))
                .clicked()
            {
                command = Some(Command::SetSlot(entry.id, Some(slot)));
            }
        }
        if matches!(entry.pin, Pin::Slot(_)) {
            ui.separator();
            if ui.button("Remove Shortcut").clicked() {
                command = Some(Command::SetSlot(entry.id, None));
            }
        }
    });
    ui.separator();
    if ui
        .add(egui::Button::new("Move to Secrets\u{2026}").shortcut_text(format!("{CMD}S")))
        .clicked()
    {
        command = Some(Command::MoveToVault(entry.id));
    }
    if ui
        .add(egui::Button::new("Delete").shortcut_text(format!("{CMD}{BACKSPACE}")))
        .clicked()
    {
        command = Some(Command::Delete(entry.id));
    }
    command
}
