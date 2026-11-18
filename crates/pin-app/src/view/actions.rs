use eframe::egui::{
    self, Align2, Color32, FontId, Frame, Margin, Rect, Sense, Shadow, Stroke, TextEdit, pos2, vec2,
};
use pin_core::Pin;

use super::format;
use super::icons::{self, Icon};
use super::text;
use super::theme::{BACKSPACE, CMD, Palette};
use crate::app::{App, Command, EntryAction, Item, Page};
use crate::hotkeys::slot_label;

const WIDTH: f32 = 252.0;
const ITEM_HEIGHT: f32 = 28.0;

impl App {
    /// Draws the Cmd+Enter actions panel above the footer, anchored to the window's bottom right.
    pub(super) fn actions_panel(
        &mut self,
        ctx: &egui::Context,
        window: Rect,
        palette: &Palette,
    ) -> Option<Command> {
        let (title, pin) = match (self.actions_entry(), self.actions_secret()) {
            (Some(entry), _) => (format::title(entry).to_owned(), entry.pin),
            (None, Some(secret)) => (secret.name.clone(), Pin::None),
            (None, None) => return None,
        };
        let items = self.action_items();
        let actions = self.actions.as_ref()?;
        let (page, opened) = (actions.page, actions.opened);
        let highlighted = actions.highlighted.min(items.len().saturating_sub(1));
        let pointer_moved = ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        let mut chosen = None;
        let mut hovered = None;
        let mut query_changed = false;
        let anchor = pos2(
            window.right() - 10.0,
            window.bottom() - super::FOOTER_HEIGHT - 6.0,
        );
        let area = egui::Area::new(egui::Id::unique("pin-actions"))
            .order(egui::Order::Foreground)
            .pivot(Align2::RIGHT_BOTTOM)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                panel_frame(palette).show(ui, |ui| {
                    let width = WIDTH - 10.0;
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    let heading = match page {
                        Page::Main => title.as_str(),
                        Page::Slots => "Assign Shortcut",
                    };
                    caption(ui, palette, width, heading);
                    let Some(actions) = self.actions.as_mut() else {
                        return;
                    };
                    let search = ui.add(
                        TextEdit::singleline(&mut actions.query)
                            .hint_text("Search for actions\u{2026}")
                            .font(FontId::proportional(12.5))
                            .text_color(palette.text)
                            .margin(Margin::symmetric(8, 5))
                            .lock_focus(true)
                            .desired_width(width),
                    );
                    if opened {
                        search.request_focus();
                    }
                    query_changed = search.changed();
                    ui.add_space(5.0);
                    separator(ui, palette, width);
                    ui.add_space(4.0);
                    if items.is_empty() {
                        caption(ui, palette, width, "No matching actions");
                    }
                    for (index, item) in items.iter().enumerate() {
                        if item.separated {
                            ui.add_space(4.0);
                            separator(ui, palette, width);
                            ui.add_space(4.0);
                        }
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(width, ITEM_HEIGHT), Sense::click());
                        if response.hovered() && pointer_moved {
                            hovered = Some(index);
                        }
                        paint_item(ui, rect, item, pin, index == highlighted, palette);
                        if response.clicked() {
                            chosen = Some(item.action);
                        }
                    }
                });
            });
        let clicked_outside = !opened && area.response.clicked_elsewhere();
        if let Some(actions) = self.actions.as_mut() {
            actions.opened = false;
            if query_changed {
                actions.highlighted = 0;
            } else if let Some(index) = hovered {
                actions.highlighted = index;
            }
        }
        if let Some(action) = chosen {
            return self.choose(action);
        }
        if clicked_outside {
            self.close_actions();
        }
        None
    }
}

fn panel_frame(palette: &Palette) -> Frame {
    Frame::new()
        .fill(palette.menu)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(10)
        .inner_margin(Margin::same(5))
        .shadow(Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 0,
            color: Color32::from_black_alpha(60),
        })
}

fn caption(ui: &mut egui::Ui, palette: &Palette, width: f32, caption: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
    let galley = ui.painter().layout_job(text::one_line(
        text::plain(caption, 11.0, palette.tertiary),
        width - 16.0,
    ));
    ui.painter().galley(
        pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0),
        galley,
        palette.tertiary,
    );
}

fn separator(ui: &mut egui::Ui, palette: &Palette, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().hline(
        rect.left() + 8.0..=rect.right() - 8.0,
        rect.center().y,
        Stroke::new(1.0, palette.separator),
    );
}

fn paint_item(ui: &egui::Ui, rect: Rect, item: &Item, pin: Pin, hot: bool, palette: &Palette) {
    let painter = ui.painter_at(rect);
    let (color, keys_color) = if hot {
        painter.rect_filled(rect, 6.0, palette.accent);
        (Color32::WHITE, Color32::from_white_alpha(220))
    } else if item.action == EntryAction::Delete {
        (palette.danger, palette.tertiary)
    } else {
        (palette.text, palette.tertiary)
    };
    icons::paint(
        &painter,
        Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(15.0, 15.0)),
        icon(item.action),
        color,
    );
    let mut right = rect.right() - 8.0;
    if item.action == EntryAction::AssignShortcut {
        icons::paint(
            &painter,
            Rect::from_center_size(pos2(right - 6.0, rect.center().y), vec2(12.0, 12.0)),
            Icon::Chevron,
            keys_color,
        );
        right -= 18.0;
    }
    let keys = shortcut(item, pin);
    if !keys.is_empty() {
        let shown = painter.text(
            pos2(right, rect.center().y),
            Align2::RIGHT_CENTER,
            keys,
            FontId::proportional(12.0),
            keys_color,
        );
        right = shown.left() - 8.0;
    }
    let left = rect.left() + 33.0;
    let label = painter.layout_job(text::one_line(
        text::plain(&item.label, 13.0, color),
        right - left,
    ));
    painter.galley(
        pos2(left, rect.center().y - label.size().y / 2.0),
        label,
        color,
    );
}

fn icon(action: EntryAction) -> Icon {
    match action {
        EntryAction::Copy => Icon::Copy,
        EntryAction::Pin(_) => Icon::Pin,
        EntryAction::AssignShortcut | EntryAction::Slot(_) => Icon::Keyboard,
        EntryAction::RemoveShortcut => Icon::Clear,
        EntryAction::MoveToSecrets => Icon::Lock,
        EntryAction::Edit => Icon::Doc,
        EntryAction::Delete => Icon::Trash,
    }
}

pub(super) fn shortcut(item: &Item, pin: Pin) -> String {
    match item.action {
        EntryAction::Copy => "\u{21A9}".to_owned(),
        EntryAction::Pin(_) => format!("{CMD}P"),
        EntryAction::AssignShortcut => match pin {
            Pin::Slot(slot) => slot_label(slot),
            _ => String::new(),
        },
        EntryAction::Slot(_) if item.current => "Current".to_owned(),
        EntryAction::MoveToSecrets => format!("{CMD}S"),
        EntryAction::Edit => format!("{CMD}E"),
        EntryAction::Delete => format!("{CMD}{BACKSPACE}"),
        EntryAction::Slot(_) | EntryAction::RemoveShortcut => String::new(),
    }
}
