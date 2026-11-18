use eframe::egui::{self, Color32, FontId, Rect, Response, Sense, Ui, WidgetInfo, pos2, vec2};

use super::icons::{self, Icon};
use super::theme::Palette;

const PILL_HEIGHT: f32 = 28.0;
const CAP: f32 = 18.0;

pub fn icon(ui: &mut Ui, icon: Icon, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    icons::paint(ui.painter(), rect, icon, color);
}

pub fn icon_button(ui: &mut Ui, palette: &Palette, icon: Icon, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, label));
    if response.hovered() {
        ui.painter().rect_filled(rect, 12.0, palette.hover);
    }
    icons::paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
        icon,
        palette.tertiary,
    );
    response
}

/// A borderless button: optional icon, label and key caps, e.g. "Copy ↩".
pub struct Pill<'a> {
    pub icon: Option<Icon>,
    pub label: &'a str,
    pub keys: &'a [&'a str],
    pub strong: bool,
    pub active: bool,
}

impl Pill<'_> {
    pub fn show(self, ui: &mut Ui, palette: &Palette) -> Response {
        pill(ui, palette, &self)
    }
}

fn pill(ui: &mut Ui, palette: &Palette, pill: &Pill<'_>) -> Response {
    let Pill {
        icon,
        label,
        keys,
        strong,
        active,
    } = *pill;
    let color = if strong {
        palette.text
    } else {
        palette.secondary
    };
    let painter = ui.painter();
    let text = painter.layout_no_wrap(label.to_owned(), FontId::proportional(12.5), color);
    let caps: Vec<_> = keys
        .iter()
        .map(|k| {
            painter.layout_no_wrap(
                (*k).to_owned(),
                FontId::proportional(11.0),
                palette.secondary,
            )
        })
        .collect();
    let caps_width: f32 = caps
        .iter()
        .enumerate()
        .map(|(i, g)| cap_gap(i) + (g.size().x + 8.0).max(CAP))
        .sum::<f32>();
    let icon_width = if icon.is_some() { 20.0 } else { 0.0 };
    let width = 8.0 + icon_width + text.size().x + caps_width + 8.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, PILL_HEIGHT), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, label));
    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, 6.0, palette.icon);
    } else if response.hovered() {
        painter.rect_filled(rect, 6.0, palette.hover);
    }
    let mut x = rect.left() + 8.0;
    if let Some(icon) = icon {
        let glyph = Rect::from_center_size(pos2(x + 7.0, rect.center().y), vec2(14.0, 14.0));
        icons::paint(painter, glyph, icon, color);
        x += icon_width;
    }
    let size = text.size();
    painter.galley(pos2(x, rect.center().y - size.y / 2.0), text, color);
    x += size.x;
    for (i, cap) in caps.into_iter().enumerate() {
        x = keycap(painter, palette, pos2(x + cap_gap(i), rect.center().y), cap);
    }
    response
}

fn cap_gap(index: usize) -> f32 {
    if index == 0 { 7.0 } else { 2.0 }
}

/// Paints a key cap whose left edge is at `at`; returns its right edge.
fn keycap(
    painter: &egui::Painter,
    palette: &Palette,
    at: egui::Pos2,
    galley: std::sync::Arc<egui::Galley>,
) -> f32 {
    let width = (galley.size().x + 8.0).max(CAP);
    let rect = Rect::from_min_size(pos2(at.x, at.y - CAP / 2.0), vec2(width, CAP));
    painter.rect_filled(rect, 4.0, palette.keycap);
    let size = galley.size();
    painter.galley(rect.center() - size / 2.0, galley, palette.secondary);
    rect.right()
}
