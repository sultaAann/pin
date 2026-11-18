use eframe::egui::{
    self, Align2, Color32, FontId, Rect, Sense, Stroke, TextFormat, TextureHandle, Vec2, pos2,
    text::LayoutJob, vec2,
};
use pin_core::{Entry, Pin, Timestamp};

use super::format;
use super::icons::{self, Icon};
use super::text;
use super::theme::{CMD, Palette};
use crate::app::{Command, EntryAction, Item, Page, entry_items};
use crate::hotkeys::slot_label;

pub(super) const HEIGHT: f32 = 36.0;
pub(super) const ENTRY_HEIGHT: f32 = 46.0;
const ICON: f32 = 24.0;
const TILE: Vec2 = Vec2::new(40.0, 30.0);

/// One history entry in the list: icon or thumbnail, first line, source app and time.
pub struct Row<'a> {
    pub entry: &'a Entry,
    pub index: usize,
    pub selected: bool,
    pub scroll: bool,
    pub now: Timestamp,
    pub thumbnail: Option<TextureHandle>,
    pub query: &'a str,
}

pub struct RowOutput {
    pub command: Option<Command>,
    pub hovered: bool,
}

impl Row<'_> {
    pub fn show(self, ui: &mut egui::Ui, palette: &Palette) -> RowOutput {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), ENTRY_HEIGHT), Sense::click());
        if self.scroll {
            response.scroll_to_me(None);
        }
        if ui.is_rect_visible(rect) {
            self.paint(ui, rect, response.hovered(), palette);
        }
        let mut command = response.clicked().then_some(Command::Copy(self.entry.id));
        let hovered = response.hovered();
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
        Look::background(&painter, rect, self.selected, hovered, palette);
        let thumb = Rect::from_min_size(
            pos2(rect.left() + 6.0, rect.center().y - TILE.y / 2.0),
            TILE,
        );
        if let Some(texture) = &self.thumbnail {
            painter.rect_filled(thumb, 5.0, palette.icon);
            let size = texture.size_vec2();
            let scale = (TILE.x / size.x).min(TILE.y / size.y);
            let image = Rect::from_center_size(thumb.center(), size * scale);
            egui::Image::from_texture((texture.id(), image.size()))
                .corner_radius(4)
                .paint_at(ui, image);
        } else {
            let square = Rect::from_center_size(thumb.center(), vec2(30.0, 30.0));
            painter.rect_filled(square, 7.0, palette.icon);
            icons::paint(
                &painter,
                Rect::from_center_size(square.center(), vec2(16.0, 16.0)),
                format::icon(self.entry),
                palette.secondary,
            );
        }
        let left = thumb.right() + 10.0;
        let right = match self.entry.pin {
            Pin::Slot(slot) => badge(
                &painter,
                rect.right() - 10.0,
                rect.center().y,
                &slot_label(slot),
                palette,
            ),
            _ => index_hint(&painter, rect, self.index, palette.secondary),
        };
        let width = right - left;
        let title = painter.layout_job(text::one_line(
            text::highlighted(
                format::title(self.entry),
                self.query,
                FontId::proportional(13.0),
                palette.text,
                palette.hit,
            ),
            width,
        ));
        painter.galley(
            pos2(left, rect.center().y - 1.0 - title.size().y),
            title,
            palette.text,
        );
        let subtitle = painter.layout_job(text::one_line(
            text::plain(
                &format::subtitle(self.entry, self.now),
                11.5,
                palette.secondary,
            ),
            width,
        ));
        painter.galley(
            pos2(left, rect.center().y + 2.0),
            subtitle,
            palette.secondary,
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
    ) {
        if selected {
            painter.rect_filled(rect, 8.0, palette.selection);
        } else if hovered {
            painter.rect_filled(rect, 8.0, palette.hover);
        }
    }

    /// Background and icon tile of a single-line row, as used for secrets.
    pub(super) fn paint(
        painter: &egui::Painter,
        rect: Rect,
        selected: bool,
        hovered: bool,
        palette: &Palette,
        icon: Icon,
    ) -> Self {
        Self::background(painter, rect, selected, hovered, palette);
        let tile = Rect::from_center_size(
            pos2(rect.left() + 8.0 + ICON / 2.0, rect.center().y),
            vec2(ICON, ICON),
        );
        painter.rect_filled(tile, 6.0, palette.icon);
        icons::paint(
            painter,
            Rect::from_center_size(tile.center(), vec2(14.0, 14.0)),
            icon,
            palette.secondary,
        );
        Self {
            text: palette.text,
            secondary: palette.secondary,
            left: tile.right() + 10.0,
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
        FontId::proportional(11.5),
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

fn badge(painter: &egui::Painter, right: f32, y: f32, label: &str, palette: &Palette) -> f32 {
    let galley = painter.layout_no_wrap(
        label.to_owned(),
        FontId::proportional(11.0),
        palette.secondary,
    );
    let rect = Rect::from_min_size(
        pos2(right - galley.size().x - 10.0, y - 9.0),
        vec2(galley.size().x + 10.0, 18.0),
    );
    painter.rect_stroke(
        rect,
        5.0,
        Stroke::new(1.0, palette.border),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        pos2(rect.left() + 5.0, y - galley.size().y / 2.0),
        galley,
        palette.secondary,
    );
    rect.left() - 10.0
}

/// The same actions as the Cmd+Enter panel, as a native-style menu.
fn context_menu(ui: &mut egui::Ui, entry: &Entry) -> Option<Command> {
    let mut chosen = None;
    for item in entry_items(entry, Page::Main) {
        if item.separated {
            ui.separator();
        }
        if item.action == EntryAction::AssignShortcut {
            ui.menu_button(item.label.as_str(), |ui| {
                for slot in entry_items(entry, Page::Slots) {
                    if slot.separated {
                        ui.separator();
                    }
                    if menu_button(ui, &slot, entry.pin) {
                        chosen = Some(slot.action);
                    }
                }
            });
        } else if menu_button(ui, &item, entry.pin) {
            chosen = Some(item.action);
        }
    }
    chosen.and_then(|action| action.command(entry.id))
}

fn menu_button(ui: &mut egui::Ui, item: &Item, pin: Pin) -> bool {
    let button = if matches!(item.action, EntryAction::Slot(_)) {
        egui::Button::selectable(item.current, item.label.as_str())
    } else {
        egui::Button::new(item.label.as_str()).shortcut_text(super::actions::shortcut(item, pin))
    };
    ui.add(button).clicked()
}
