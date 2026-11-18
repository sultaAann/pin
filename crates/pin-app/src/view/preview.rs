use eframe::egui::{
    self, Align, FontId, Frame, Label, Layout, Margin, Rect, RichText, ScrollArea, Sense,
    TextureHandle, pos2, vec2,
};
use pin_core::{Details, Entry, Filter, FormatKind, Pin, Timestamp};

use super::format;
use super::icons::{self, Icon};
use super::text;
use super::theme::Palette;
use crate::app::App;
use crate::hotkeys::slot_label;

const META_HEIGHT: f32 = 100.0;
const IMAGE_HEIGHT: f32 = 186.0;
const MAX_FILES: usize = 100;

impl App {
    pub(super) fn preview_pane(&mut self, ui: &mut egui::Ui, palette: &Palette) {
        let Some(entry) = self.entries.get(self.selected).cloned() else {
            return;
        };
        let thumbnail = (entry.kind == FormatKind::Png)
            .then(|| self.thumbnail(entry.id))
            .flatten();
        let query = self.query.trim().to_owned();
        let details = self.selected_details();
        Pane {
            entry: &entry,
            details,
            thumbnail,
            query: &query,
            now: Timestamp::now(),
        }
        .show(ui, palette);
    }
}

struct Pane<'a> {
    entry: &'a Entry,
    details: Option<&'a Details>,
    thumbnail: Option<TextureHandle>,
    query: &'a str,
    now: Timestamp,
}

impl Pane<'_> {
    fn show(&self, ui: &mut egui::Ui, palette: &Palette) {
        Frame::new()
            .inner_margin(Margin {
                left: 18,
                right: 18,
                top: 14,
                bottom: 16,
            })
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                self.header(ui, palette);
                ui.add_space(12.0);
                let height = (ui.available_height() - META_HEIGHT).max(40.0);
                ui.allocate_ui(vec2(ui.available_width(), height), |ui| {
                    ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| self.content(ui, palette));
                });
                ui.add_space(12.0);
                super::divider(ui, palette);
                ui.add_space(12.0);
                self.meta(ui, palette);
            });
    }

    fn header(&self, ui: &mut egui::Ui, palette: &Palette) {
        let files = self.details.map_or(0, |d| d.files.len());
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 20.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(
                    RichText::new(format::kind_label(self.entry, files))
                        .size(12.0)
                        .color(palette.secondary),
                );
                let pinned = match self.entry.pin {
                    Pin::None => return,
                    Pin::Pinned => "Pinned".to_owned(),
                    Pin::Slot(slot) => format!("Pinned \u{00B7} {}", slot_label(slot)),
                };
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    Frame::new()
                        .fill(palette.chip)
                        .corner_radius(6)
                        .inner_margin(Margin::symmetric(7, 2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                super::controls::icon(ui, Icon::Pin, 12.0, palette.accent_text);
                                ui.label(
                                    RichText::new(pinned).size(11.5).color(palette.accent_text),
                                );
                            });
                        });
                });
            },
        );
    }

    fn content(&self, ui: &mut egui::Ui, palette: &Palette) {
        match self.entry.kind {
            FormatKind::Png => self.image(ui, palette),
            FormatKind::Files => self.files(ui, palette),
            FormatKind::Text if Filter::Links.matches(self.entry) => self.link(ui, palette),
            _ => self.text(ui, palette),
        }
    }

    fn well(ui: &mut egui::Ui, palette: &Palette, add: impl FnOnce(&mut egui::Ui)) {
        Frame::new()
            .fill(palette.well)
            .corner_radius(8)
            .inner_margin(Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                add(ui);
            });
    }

    fn text(&self, ui: &mut egui::Ui, palette: &Palette) {
        let shown = self.details.and_then(|d| d.text.as_deref());
        Self::well(ui, palette, |ui| match shown {
            Some(content) => {
                let mut job = text::highlighted(
                    content,
                    self.query,
                    FontId::proportional(13.0),
                    palette.text,
                    palette.hit,
                );
                job.wrap.break_anywhere = false;
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add(Label::new(job).wrap());
            }
            None => {
                ui.label(
                    RichText::new("No plain text to preview.")
                        .size(12.5)
                        .color(palette.tertiary),
                );
            }
        });
    }

    fn link(&self, ui: &mut egui::Ui, palette: &Palette) {
        let url = self
            .details
            .and_then(|d| d.text.as_deref())
            .unwrap_or(&self.entry.preview)
            .split_whitespace()
            .next()
            .unwrap_or_default();
        Self::well(ui, palette, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = vec2(12.0, 4.0);
                tile(ui, palette, Icon::Link, 36.0, palette.accent_text);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format::domain(url))
                            .size(14.0)
                            .color(palette.text),
                    );
                    let mut job = text::highlighted(
                        url,
                        self.query,
                        FontId::proportional(12.5),
                        palette.secondary,
                        palette.hit,
                    );
                    job.wrap.break_anywhere = true;
                    ui.add(Label::new(job).wrap());
                });
            });
        });
    }

    fn image(&self, ui: &mut egui::Ui, palette: &Palette) {
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), IMAGE_HEIGHT), Sense::hover());
        ui.painter().rect_filled(rect, 8.0, palette.well);
        let inner = rect.shrink(8.0);
        match &self.thumbnail {
            Some(texture) => {
                let size = texture.size_vec2();
                let scale = (inner.width() / size.x)
                    .min(inner.height() / size.y)
                    .min(1.0);
                let image = Rect::from_center_size(inner.center(), size * scale);
                egui::Image::from_texture((texture.id(), image.size()))
                    .corner_radius(6)
                    .paint_at(ui, image);
            }
            None => icons::paint(
                ui.painter(),
                Rect::from_center_size(rect.center(), vec2(28.0, 28.0)),
                Icon::Image,
                palette.tertiary,
            ),
        }
    }

    fn files(&self, ui: &mut egui::Ui, palette: &Palette) {
        let paths = self.details.map_or(&[][..], |d| d.files.as_slice());
        Frame::new()
            .fill(palette.well)
            .corner_radius(8)
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                for path in paths.iter().take(MAX_FILES) {
                    file_row(ui, palette, path);
                }
                if paths.len() > MAX_FILES {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("and {} more", paths.len() - MAX_FILES))
                            .size(11.5)
                            .color(palette.tertiary),
                    );
                }
            });
    }

    fn meta(&self, ui: &mut egui::Ui, palette: &Palette) {
        let formats = self.details.map_or_else(
            || "\u{2014}".to_owned(),
            |d| {
                d.formats
                    .iter()
                    .map(format::format_name)
                    .collect::<Vec<_>>()
                    .join(" \u{00B7} ")
            },
        );
        let cells = [
            (
                "Application",
                self.entry
                    .source_app
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_owned()),
            ),
            (
                "Last Used",
                format::relative(self.now, self.entry.last_used_at),
            ),
            ("Size", format::size(self.entry.size)),
            ("Formats", formats),
        ];
        let width = ((ui.available_width() - 16.0) / 2.0).max(0.0);
        for (index, pair) in cells.chunks(2).enumerate() {
            if index > 0 {
                ui.add_space(10.0);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for (label, value) in pair {
                    ui.allocate_ui(vec2(width, 32.0), |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(RichText::new(*label).size(11.0).color(palette.tertiary));
                            ui.add(
                                Label::new(
                                    RichText::new(value.as_str())
                                        .size(12.5)
                                        .color(palette.secondary),
                                )
                                .truncate(),
                            );
                        });
                    });
                }
            });
        }
    }
}

fn tile(ui: &mut egui::Ui, palette: &Palette, icon: Icon, size: f32, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui.painter().rect_filled(rect, size / 4.5, palette.icon);
    icons::paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), vec2(size / 2.0, size / 2.0)),
        icon,
        color,
    );
}

fn file_row(ui: &mut egui::Ui, palette: &Palette, path: &str) {
    let folder = path.ends_with('/');
    let trimmed = path.trim_end_matches('/');
    let (dir, name) = trimmed
        .rsplit_once('/')
        .map_or(("", trimmed), |(dir, name)| (dir, name));
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let square = Rect::from_min_size(
        pos2(rect.left() + 8.0, rect.center().y - 14.0),
        vec2(28.0, 28.0),
    );
    painter.rect_filled(square, 6.0, palette.icon);
    icons::paint(
        &painter,
        Rect::from_center_size(square.center(), vec2(15.0, 15.0)),
        if folder { Icon::Folder } else { Icon::File },
        palette.secondary,
    );
    let left = square.right() + 10.0;
    let width = rect.right() - 8.0 - left;
    let name = painter.layout_job(text::one_line(text::plain(name, 13.0, palette.text), width));
    painter.galley(
        pos2(left, rect.center().y - 1.0 - name.size().y),
        name,
        palette.text,
    );
    let dir = if dir.is_empty() { "/" } else { dir };
    let dir = painter.layout_job(text::one_line(
        text::plain(dir, 11.5, palette.tertiary),
        width,
    ));
    painter.galley(pos2(left, rect.center().y + 2.0), dir, palette.tertiary);
}
