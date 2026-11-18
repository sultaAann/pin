mod actions;
mod controls;
mod filetype;
mod filters;
mod format;
mod icons;
mod keys;
mod preview;
mod row;
mod secrets;
mod sheets;
mod text;
mod theme;

use eframe::egui::{
    self, Align, Color32, FontId, Layout, RichText, ScrollArea, Sense, Stroke, TextEdit, vec2,
};
use pin_core::{Filter, Timestamp};

use self::icons::Icon;
use self::row::Row;
use self::theme::{CMD, CMD_KEY, CORNER, Palette};
use crate::app::{App, Command, Focus, PREFIX};

pub use self::theme::{WINDOW_SIZE, install};

const SEARCH_HEIGHT: f32 = 52.0;
const FOOTER_HEIGHT: f32 = 40.0;
const LIST_WIDTH: f32 = 300.0;
const SEARCH_ID: &str = "pin-search";

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
        let panel_open = self.actions.is_some();
        let mut clicked = None;
        let window = egui::Frame::new()
            .fill(palette.background)
            .stroke(Stroke::new(1.0, palette.border))
            .corner_radius(CORNER)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                self.search_bar(ui, &palette);
                if self.secrets_mode() {
                    if let Some(chosen) = self.vault_strip(ui, &palette) {
                        clicked = Some(chosen);
                    }
                } else if self.sheet.is_none() {
                    self.filters(ui, &palette);
                }
                divider(ui, &palette);
                let body_height = (ui.available_height() - FOOTER_HEIGHT - 1.0).max(0.0);
                ui.allocate_ui(vec2(ui.available_width(), body_height), |ui| {
                    let chosen = if self.sheet.is_some() {
                        self.sheet_ui(ui, &palette)
                    } else if self.secrets_mode() {
                        self.secret_list(ui, &palette)
                    } else {
                        self.history(ui, &palette)
                    };
                    if chosen.is_some() {
                        clicked = chosen;
                    }
                });
                divider(ui, &palette);
                if let Some(chosen) = self.footer(ui, &palette) {
                    clicked = Some(chosen);
                }
            })
            .response
            .rect;
        if self.actions.is_some()
            && let Some(chosen) = self.actions_panel(&ctx, window, &palette)
        {
            command = Some(chosen);
        }
        // As with a macOS menu, a click outside the open panel only closes it.
        if !(panel_open && self.actions.is_none()) {
            command = clicked.or(command);
        }
        if let Some(command) = command {
            self.run(&ctx, command);
        }
    }

    fn search_bar(&mut self, ui: &mut egui::Ui, palette: &Palette) {
        let secrets = self.secrets_mode();
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), SEARCH_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add_space(18.0);
                controls::icon(ui, Icon::Search, 18.0, palette.secondary);
                ui.add_space(10.0);
                let id = egui::Id::unique(SEARCH_ID);
                let (mut text, mut edited) = self.search_prefix(ui, palette, id);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(12.0);
                    let prefix = PREFIX.to_string();
                    if !secrets
                        && self.sheet.is_none()
                        && (controls::Pill {
                            icon: Some(Icon::Lock),
                            label: "Secrets",
                            keys: &[prefix.as_str()],
                            strong: false,
                            active: false,
                        })
                        .show(ui, palette)
                        .clicked()
                    {
                        self.query.clone_from(&prefix);
                        edited = true;
                    }
                    if !text.is_empty() {
                        ui.add_space(6.0);
                        if controls::icon_button(ui, palette, Icon::Clear, "Clear search").clicked()
                        {
                            text.clear();
                            self.query = if secrets {
                                prefix.clone()
                            } else {
                                String::new()
                            };
                            edited = true;
                        }
                        ui.add_space(4.0);
                    } else if secrets && self.sheet.is_none() && !edited {
                        secrets::history_hint(ui, palette);
                        ui.add_space(8.0);
                    }
                    let hint = match (secrets, self.sheet.is_some()) {
                        (_, true) => "",
                        (true, false) => "Search secrets by name or note",
                        (false, false) => "Search Clipboard",
                    };
                    let search = ui.add(
                        TextEdit::singleline(&mut text)
                            .id(id)
                            .hint_text(RichText::new(hint).color(palette.tertiary))
                            .font(FontId::proportional(19.0))
                            .text_color(palette.text)
                            .frame(egui::Frame::NONE)
                            .lock_focus(true)
                            .desired_width(ui.available_width()),
                    );
                    if edited {
                        self.focus = Focus::Search;
                        cursor_to_end(ui.ctx(), id, self.search_text());
                    } else if search.changed() {
                        self.query = if secrets {
                            format!("{PREFIX}{text}")
                        } else {
                            text
                        };
                        edited = true;
                        if self.secrets_mode() != secrets {
                            cursor_to_end(ui.ctx(), id, self.search_text());
                        }
                    }
                    if self.take_focus(Focus::Search) {
                        search.request_focus();
                    }
                    if edited {
                        self.selected = 0;
                        self.scroll_to_selected = true;
                        self.refresh();
                    }
                });
            },
        );
    }

    /// The text the search field edits; in secrets mode the prefix is a pill before it, and
    /// Backspace in the empty field or the pill's close button go back to history.
    fn search_prefix(
        &mut self,
        ui: &mut egui::Ui,
        palette: &Palette,
        id: egui::Id,
    ) -> (String, bool) {
        if !self.secrets_mode() {
            return (self.query.clone(), false);
        }
        // A sheet about a secret must not outlive secrets mode, so leaving waits for it to close.
        let open = self.sheet.is_none();
        let mut back = secrets::mode_pill(ui, palette) && open;
        ui.add_space(10.0);
        let text = self.secret_query().to_owned();
        back |= open
            && text.is_empty()
            && ui.memory(|m| m.has_focus(id))
            && ui.input(|i| i.key_pressed(egui::Key::Backspace) && i.modifiers.is_none());
        if back {
            self.query.clear();
        }
        (text, back)
    }

    /// What the search field shows: the text after the prefix in secrets mode, else the query.
    fn search_text(&self) -> &str {
        if self.secrets_mode() {
            self.secret_query()
        } else {
            &self.query
        }
    }

    fn history(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        if self.entries.is_empty() {
            self.empty_history(ui, palette);
            return None;
        }
        self.columns(ui, palette, Self::list, Self::preview_pane)
    }

    /// The history and secrets body: a list column, a divider and a preview column.
    fn columns(
        &mut self,
        ui: &mut egui::Ui,
        palette: &Palette,
        list: fn(&mut Self, &mut egui::Ui, &Palette) -> Option<Command>,
        preview: fn(&mut Self, &mut egui::Ui, &Palette),
    ) -> Option<Command> {
        let height = ui.available_height();
        // Each column needs its own top-down layout: allocate_ui keeps the parent's horizontal one.
        let column = Layout::top_down(Align::Min);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let command = ui
                .allocate_ui_with_layout(vec2(LIST_WIDTH, height), column, |ui| {
                    ui.set_width(LIST_WIDTH);
                    list(self, ui, palette)
                })
                .inner;
            let (rect, _) = ui.allocate_exact_size(vec2(1.0, height), Sense::hover());
            ui.painter().vline(
                rect.center().x,
                rect.y_range(),
                Stroke::new(1.0, palette.separator),
            );
            let width = ui.available_width();
            ui.allocate_ui_with_layout(vec2(width, height), column, |ui| {
                ui.set_width(width);
                preview(self, ui, palette);
            });
            command
        })
        .inner
    }

    fn list(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        let mut command = None;
        let pointer_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        let scroll = std::mem::take(&mut self.scroll_to_selected);
        let now = Timestamp::now();
        let query = self.query.trim().to_owned();
        let sections = query.is_empty() && self.entries.iter().any(|e| e.pin.is_pinned());
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
                            vec2(ui.available_width(), row::ENTRY_HEIGHT),
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
                            query: &query,
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
            && self.actions.is_none()
        {
            self.selected = index;
            ui.ctx().request_repaint();
        }
        command
    }

    fn empty_history(&mut self, ui: &mut egui::Ui, palette: &Palette) {
        let query = self.query.trim().to_owned();
        if !query.is_empty() {
            let subtitle =
                format!("Nothing in your clipboard history matches \u{201C}{query}\u{201D}.");
            let mut clear = false;
            empty(ui, palette, Icon::Search, "No Results", &subtitle, |ui| {
                ui.add_space(14.0);
                clear = ui
                    .add(
                        egui::Button::new(RichText::new("Clear Search").size(12.5))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(1.0, palette.border))
                            .min_size(vec2(0.0, 28.0)),
                    )
                    .clicked();
                ui.add_space(16.0);
                ui.label(
                    RichText::new(format!(
                        "Secrets are not searched here. Start with {PREFIX} to search Secrets."
                    ))
                    .size(11.5)
                    .color(palette.tertiary),
                );
            });
            if clear {
                self.query.clear();
                self.focus = Focus::Search;
                self.refresh();
            }
        } else if self.filter == Filter::All {
            empty(
                ui,
                palette,
                Icon::Clipboard,
                "No Clipboard History",
                "Copied text, images and files appear here.",
                |_| {},
            );
        } else {
            let subtitle = format!(
                "No {} in your clipboard history.",
                format::filter_noun(self.filter)
            );
            empty(
                ui,
                palette,
                Icon::Clipboard,
                "Nothing Here Yet",
                &subtitle,
                |_| {},
            );
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        let mut command = None;
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), FOOTER_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                let footer = ui.max_rect();
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
                } else if self.sheet.is_none()
                    && !(self.secrets_mode() && self.copied_note(ui, palette, footer))
                {
                    ui.label(
                        RichText::new(self.count_label())
                            .size(12.0)
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
                    ui.add_space(4.0);
                    if let Some(sheet) = &self.sheet {
                        sheets::footer_hints(ui, palette, sheet);
                        return;
                    }
                    let copy = if self.secrets_mode() {
                        self.secrets
                            .get(self.selected)
                            .map(|s| Command::CopySecret(s.id))
                    } else {
                        self.entries.get(self.selected).map(|e| Command::Copy(e.id))
                    };
                    let Some(copy) = copy else {
                        return;
                    };
                    if (controls::Pill {
                        icon: None,
                        label: "Actions",
                        keys: &[CMD_KEY, "\u{21A9}"],
                        strong: false,
                        active: self.actions.is_some(),
                    })
                    .show(ui, palette)
                    .clicked()
                    {
                        self.toggle_actions();
                    }
                    let (rect, _) = ui.allocate_exact_size(vec2(5.0, 16.0), Sense::hover());
                    ui.painter().vline(
                        rect.center().x,
                        rect.y_range(),
                        Stroke::new(1.0, palette.separator),
                    );
                    if (controls::Pill {
                        icon: None,
                        label: "Copy",
                        keys: &["\u{21A9}"],
                        strong: true,
                        active: false,
                    })
                    .show(ui, palette)
                    .clicked()
                    {
                        command = Some(copy);
                    }
                });
            },
        );
        command
    }

    fn count_label(&self) -> String {
        let count = self.len();
        let noun = match (
            self.secrets_mode(),
            !self.query.trim().is_empty(),
            count == 1,
        ) {
            (true, _, true) => "secret",
            (true, _, false) => "secrets",
            (false, true, true) => "result",
            (false, true, false) => "results",
            (false, false, true) => "item",
            (false, false, false) => "items",
        };
        format!("{count} {noun}")
    }
}

fn cursor_to_end(ctx: &egui::Context, id: egui::Id, text: &str) {
    use egui::text::{CCursor, CCursorRange};
    let mut state = TextEdit::load_state(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(text.chars().count()))));
    state.store(ctx, id);
}

fn divider(ui: &mut egui::Ui, palette: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, palette.separator),
    );
}

fn empty(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Icon,
    title: &str,
    subtitle: &str,
    extra: impl FnOnce(&mut egui::Ui),
) {
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() / 2.0 - 64.0).max(0.0));
        let (rect, _) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::hover());
        ui.painter().rect_filled(rect, 12.0, palette.icon);
        icons::paint(
            ui.painter(),
            egui::Rect::from_center_size(rect.center(), vec2(22.0, 22.0)),
            icon,
            palette.secondary,
        );
        ui.add_space(14.0);
        ui.label(RichText::new(title).size(15.0).color(palette.text));
        ui.add_space(6.0);
        ui.label(RichText::new(subtitle).size(12.5).color(palette.secondary));
        extra(ui);
    });
}

fn section(ui: &mut egui::Ui, palette: &Palette, title: &str) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(title).size(11.0).color(palette.tertiary));
    });
    ui.add_space(6.0);
}

pub fn transparent() -> [f32; 4] {
    Color32::TRANSPARENT.to_normalized_gamma_f32()
}
