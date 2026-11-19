use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, vec2};
use pin_core::Filter;

use super::format;
use super::theme::Palette;
use crate::app::{App, Focus};

pub(super) const HEIGHT: f32 = 40.0;

impl App {
    pub(super) fn filters(&mut self, ui: &mut egui::Ui, palette: &Palette) {
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add_space(14.0);
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.spacing_mut().button_padding = vec2(11.0, 0.0);
                for filter in Filter::ALL {
                    let active = filter == self.filter;
                    let (fill, border, color) = if active {
                        (palette.chip, palette.chip_border, palette.accent_text)
                    } else {
                        (palette.icon, Color32::TRANSPARENT, palette.secondary)
                    };
                    let chip = egui::Button::new(
                        RichText::new(format::filter_label(filter))
                            .size(12.0)
                            .color(color),
                    )
                    .fill(fill)
                    .stroke(Stroke::new(1.0, border))
                    .corner_radius(13)
                    .min_size(vec2(0.0, 26.0))
                    .selected(active);
                    if ui.add(chip).clicked() {
                        self.set_filter(filter);
                        self.focus = Focus::Search;
                    }
                }
            },
        );
    }
}
