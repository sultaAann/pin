use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Label, Layout, Margin, Rect, RichText, ScrollArea,
    Sense, Stroke, TextFormat, pos2, text::LayoutJob, vec2,
};
use pin_core::{FormatKind, Pin, Secret, Timestamp, VaultStatus};

use super::controls::{self, Pill};
use super::format;
use super::icons::{self, Icon};
use super::preview::{META_HEIGHT, meta_grid, well};
use super::row::{self, ENTRY_HEIGHT, index_hint, menu_button, two_lines};
use super::text;
use super::theme::{ALT, BACKSPACE, CMD_KEY, Palette};
use crate::app::{App, Command, Target, secret_items};

const STATUS_HEIGHT: f32 = 36.0;
const TICK: Duration = Duration::from_secs(1);

impl App {
    /// The strip under the search bar: whether the vault is open and when it locks itself.
    pub(super) fn vault_strip(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        let mut command = None;
        let (dot, state, detail) = match self.vault_status {
            VaultStatus::Unlocked => {
                let left = self.locks_in().map_or_else(String::new, |left| {
                    format!(" \u{00B7} locks in {} when idle", clock(left))
                });
                if self.is_shown() {
                    ui.ctx().request_repaint_after(TICK);
                }
                (Dot::Filled(palette.success), "Unlocked", left)
            }
            VaultStatus::Locked => (
                Dot::Filled(palette.tertiary),
                "Locked",
                " \u{00B7} enter your master password to continue".to_owned(),
            ),
            VaultStatus::Missing => (
                Dot::Hollow(palette.tertiary),
                "Not set up",
                " \u{00B7} secrets get their own password".to_owned(),
            ),
        };
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), STATUS_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add_space(20.0);
                let (rect, _) = ui.allocate_exact_size(vec2(7.0, 7.0), Sense::hover());
                match dot {
                    Dot::Filled(color) => {
                        ui.painter().circle_filled(rect.center(), 3.5, color);
                    }
                    Dot::Hollow(color) => {
                        ui.painter()
                            .circle_stroke(rect.center(), 2.8, Stroke::new(1.5, color));
                    }
                }
                ui.add_space(7.0);
                ui.label(RichText::new(state).size(12.0).color(palette.text));
                ui.label(RichText::new(detail).size(12.0).color(palette.tertiary));
                if self.vault_status != VaultStatus::Unlocked {
                    return;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(12.0);
                    let lock = Pill {
                        icon: Some(Icon::Lock),
                        label: "Lock Now",
                        keys: &[CMD_KEY, "L"],
                        strong: false,
                        active: false,
                    };
                    if lock.show(ui, palette).clicked() {
                        command = Some(Command::LockSecrets);
                    }
                });
            },
        );
        command
    }

    pub(super) fn secret_list(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        if self.secrets.is_empty() {
            self.hide_value();
            empty_vault(ui, palette, !self.secret_query().trim().is_empty());
            return None;
        }
        self.columns(ui, palette, Self::secret_rows, Self::secret_preview)
    }

    fn secret_rows(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        let mut command = None;
        let mut hovered = None;
        let pointer_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        let scroll = std::mem::take(&mut self.scroll_to_selected);
        let now = Timestamp::now();
        let query = self.secret_query().trim().to_owned();
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            Frame::new()
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    for (index, secret) in self.secrets.iter().enumerate() {
                        let selected = index == self.selected;
                        let (chosen, is_hovered) = SecretRow {
                            secret,
                            index,
                            selected,
                            scroll: scroll && selected,
                            now,
                            query: &query,
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
            && self.actions.is_none()
        {
            self.selected = index;
            ui.ctx().request_repaint();
        }
        command
    }

    fn secret_preview(&mut self, ui: &mut egui::Ui, palette: &Palette) {
        let Some(secret) = self.secrets.get(self.selected) else {
            return;
        };
        let (kind, name, note) = (
            kind_label(&secret.kind),
            secret.name.clone(),
            secret.note.clone(),
        );
        // Only these kinds have a text form; `Vault::reveal` gives nothing for the rest.
        let revealable = matches!(secret.kind, FormatKind::Text | FormatKind::Files);
        let now = Timestamp::now();
        // No size here: for a password it is its length.
        let cells = [
            ("Kind", kind.to_owned()),
            ("Created", format::relative(now, secret.created_at)),
            ("Updated", format::relative(now, secret.updated_at)),
        ];
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
                header(ui, palette, kind);
                ui.add_space(12.0);
                let height = (ui.available_height() - META_HEIGHT).max(40.0);
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                            ui.add(
                                Label::new(RichText::new(name).size(17.0).color(palette.text))
                                    .wrap(),
                            );
                            ui.add_space(12.0);
                            well(ui, palette, |ui| self.value(ui, palette, revealable));
                            if !note.is_empty() {
                                ui.add_space(12.0);
                                ui.label(RichText::new("Note").size(11.0).color(palette.tertiary));
                                ui.add_space(4.0);
                                ui.add(
                                    Label::new(RichText::new(note).size(13.0).color(palette.text))
                                        .wrap(),
                                );
                            }
                        });
                    },
                );
                ui.add_space(12.0);
                super::divider(ui, palette);
                ui.add_space(12.0);
                meta_grid(ui, palette, &cells);
            });
    }

    /// The hidden value, or the decrypted one after a click on Show or while Alt is held.
    fn value(&mut self, ui: &mut egui::Ui, palette: &Palette, revealable: bool) {
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            let held = revealable && {
                let shown = self.revealed.is_some();
                let button = reveal_button(ui, palette, shown)
                    .on_hover_text(format!("Shows it for 30 s. Or hold {ALT} to peek."));
                if button.clicked() {
                    self.toggle_reveal();
                }
                let (alt, others) = ui.input(|i| {
                    let m = i.modifiers;
                    (
                        m.alt,
                        m.command || m.ctrl || m.shift || !i.keys_down.is_empty(),
                    )
                });
                let (alt, wait) = self.alt_reveals(alt, others);
                if let Some(wait) = wait.or(self.shown_for()) {
                    ui.ctx().request_repaint_after(wait);
                }
                alt
            };
            self.hold_reveal(held);
            ui.add_space(10.0);
            let width = ui.available_width().max(0.0);
            ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(width);
                let revealed = self.revealed.as_ref().and_then(|r| r.text.as_deref());
                if let Some(value) = revealed {
                    // egui needs an owned copy and caches the laid-out text, so these copies
                    // can't be zeroized; they live only while the value is held on screen.
                    let mut job = LayoutJob::single_section(
                        value.to_owned(),
                        TextFormat::simple(FontId::monospace(12.5), palette.text),
                    );
                    job.wrap.break_anywhere = true;
                    ui.add(Label::new(job).wrap().selectable(false));
                } else {
                    let job = LayoutJob::single_section(
                        "\u{2022}".repeat(12),
                        TextFormat {
                            extra_letter_spacing: 2.0,
                            line_height: Some(22.0),
                            ..TextFormat::simple(FontId::proportional(15.0), palette.text)
                        },
                    );
                    ui.add(Label::new(job).truncate());
                    if !revealable {
                        ui.label(
                            RichText::new("Copy it to see it.")
                                .size(11.5)
                                .color(palette.tertiary),
                        );
                    }
                }
            });
        });
    }

    /// Left side of the footer in secrets mode: the clipboard countdown after a copy.
    pub(super) fn copied_note(&self, ui: &mut egui::Ui, palette: &Palette, footer: Rect) -> bool {
        let Some((left, share)) = self.clears_in() else {
            return false;
        };
        if self.is_shown() {
            ui.ctx().request_repaint_after(TICK);
        }
        let bar = Rect::from_min_size(
            pos2(footer.left(), footer.top() - 1.0),
            vec2(footer.width() * share, 2.0),
        );
        ui.painter().rect_filled(bar, 0.0, palette.success);
        controls::icon(ui, Icon::Check, 13.0, palette.success_text);
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "Copied \u{00B7} clipboard clears in {}",
                clock(left)
            ))
            .size(12.0)
            .color(palette.success_text),
        );
        true
    }
}

enum Dot {
    Filled(Color32),
    Hollow(Color32),
}

/// `m:ss`, rounded up so the countdown reaches 0:00 only when the time is up.
pub(super) fn clock(left: Duration) -> String {
    let secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub(super) fn kind_label(kind: &FormatKind) -> &'static str {
    match kind {
        FormatKind::Png => "Image",
        FormatKind::Files => "File",
        FormatKind::Text => "Plain Text",
        FormatKind::Html | FormatKind::Rtf => "Rich Text",
        FormatKind::Other(_) => "Clipboard Data",
    }
}

pub(super) fn icon(kind: &FormatKind) -> Icon {
    match kind {
        FormatKind::Png => Icon::Image,
        FormatKind::Files => Icon::File,
        _ => Icon::Key,
    }
}

fn header(ui: &mut egui::Ui, palette: &Palette, kind: &str) {
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), 20.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.label(RichText::new(kind).size(12.0).color(palette.secondary));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                Frame::new()
                    .fill(palette.icon)
                    .corner_radius(6)
                    .inner_margin(Margin::symmetric(7, 2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            controls::icon(ui, Icon::Shield, 12.0, palette.secondary);
                            ui.label(
                                RichText::new("Encrypted")
                                    .size(11.5)
                                    .color(palette.secondary),
                            );
                        });
                    });
            });
        },
    );
}

fn reveal_button(ui: &mut egui::Ui, palette: &Palette, shown: bool) -> egui::Response {
    let label = if shown { "Hide" } else { "Show" };
    let painter = ui.painter();
    let text = painter.layout_no_wrap(
        label.to_owned(),
        FontId::proportional(12.0),
        palette.secondary,
    );
    let cap = painter.layout_no_wrap(
        ALT.to_owned(),
        FontId::proportional(11.0),
        palette.secondary,
    );
    let cap_width = (cap.size().x + 8.0).max(18.0);
    let width = 8.0 + 13.0 + 6.0 + text.size().x + 6.0 + cap_width + 4.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let painter = ui.painter();
    let fill = if shown || response.hovered() {
        palette.hover
    } else {
        palette.field
    };
    painter.rect(
        rect,
        6.0,
        fill,
        Stroke::new(1.0, palette.border),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.left() + 8.0;
    icons::paint(
        painter,
        Rect::from_min_size(pos2(x, rect.center().y - 6.5), vec2(13.0, 13.0)),
        Icon::Eye,
        palette.secondary,
    );
    x += 19.0;
    let size = text.size();
    painter.galley(
        pos2(x, rect.center().y - size.y / 2.0),
        text,
        palette.secondary,
    );
    x += size.x + 6.0;
    controls::keycap(painter, palette, pos2(x, rect.center().y), cap);
    response
}

fn empty_vault(ui: &mut egui::Ui, palette: &Palette, filtered: bool) {
    if filtered {
        super::empty(
            ui,
            palette,
            Icon::Search,
            "No Matching Secrets",
            "Try a different name or note.",
            |_| {},
        );
        return;
    }
    super::empty(
        ui,
        palette,
        Icon::Key,
        "No Secrets Yet",
        "Secrets are added from history. Values never show up in history, search or logs.",
        |ui| {
            ui.add_space(20.0);
            let width = 380.0_f32.min(ui.available_width());
            ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                step(ui, palette, 1, |ui| {
                    say(ui, palette, "Leave Secrets with");
                    controls::keycaps(ui, palette, &[BACKSPACE]);
                    say(ui, palette, "and select a copied password");
                });
                step(ui, palette, 2, |ui| {
                    say(ui, palette, "Press");
                    controls::keycaps(ui, palette, &[CMD_KEY, "S"]);
                    say(ui, palette, "or choose Move to Secrets\u{2026} in Actions");
                });
                step(ui, palette, 3, |ui| {
                    say(
                        ui,
                        palette,
                        "Give it a name; it leaves history and is encrypted",
                    );
                });
            });
        },
    );
}

fn step(ui: &mut egui::Ui, palette: &Palette, number: u8, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), 10.0, palette.icon);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            number.to_string(),
            FontId::proportional(11.0),
            palette.secondary,
        );
        ui.add_space(4.0);
        add(ui);
    });
}

fn say(ui: &mut egui::Ui, palette: &Palette, words: &str) {
    ui.label(RichText::new(words).size(12.5).color(palette.text));
}

/// The pill in the search bar that marks secrets mode; returns whether its close button was hit.
/// The button is left out while secrets mode can't be left.
pub(super) fn mode_pill(ui: &mut egui::Ui, palette: &Palette, closable: bool) -> bool {
    let mut closed = false;
    Frame::new()
        .fill(palette.chip)
        .corner_radius(7)
        .inner_margin(Margin {
            left: 9,
            right: 4,
            top: 4,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                controls::icon(ui, Icon::Lock, 13.0, palette.accent_text);
                ui.label(
                    RichText::new("Secrets")
                        .size(13.0)
                        .color(palette.accent_text),
                );
                if !closable {
                    ui.add_space(4.0);
                    return;
                }
                let (rect, response) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, "Back to history")
                });
                if response.hovered() {
                    ui.painter().circle_filled(rect.center(), 9.0, palette.chip);
                }
                let arm = 3.3;
                let stroke = Stroke::new(1.6, palette.accent_text);
                let c = rect.center();
                ui.painter()
                    .line_segment([c - vec2(arm, arm), c + vec2(arm, arm)], stroke);
                ui.painter()
                    .line_segment([c + vec2(arm, -arm), c + vec2(-arm, arm)], stroke);
                closed = response.clicked();
            });
        });
    closed
}

/// The search hint at the right of an empty secrets search: Backspace goes back to history.
pub(super) fn history_hint(ui: &mut egui::Ui, palette: &Palette) {
    controls::keycaps(ui, palette, &[BACKSPACE]);
    ui.add_space(6.0);
    ui.label(RichText::new("History").size(12.0).color(palette.tertiary));
}

struct SecretRow<'a> {
    secret: &'a Secret,
    index: usize,
    selected: bool,
    scroll: bool,
    now: Timestamp,
    query: &'a str,
}

impl SecretRow<'_> {
    fn show(self, ui: &mut egui::Ui, palette: &Palette) -> (Option<Command>, bool) {
        let secret = self.secret;
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), ENTRY_HEIGHT), Sense::click());
        if self.scroll {
            response.scroll_to_me(None);
        }
        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            row::background(&painter, rect, self.selected, response.hovered(), palette);
            row::tile(&painter, rect, icon(&secret.kind), palette);
            let right = index_hint(&painter, rect, self.index, palette.secondary);
            let title = text::highlighted(
                &secret.name,
                self.query,
                FontId::proportional(13.0),
                palette.text,
                palette.hit,
            );
            let subtitle = if secret.note.is_empty() {
                format!(
                    "Updated {}",
                    format::relative(self.now, secret.updated_at).to_lowercase()
                )
            } else {
                secret.note.clone()
            };
            two_lines(&painter, rect, right, title, &subtitle, palette);
        }
        let hovered = response.hovered();
        let mut command = response.clicked().then_some(Command::CopySecret(secret.id));
        response.context_menu(|ui| {
            for item in secret_items() {
                if item.separated {
                    ui.separator();
                }
                if menu_button(ui, &item, Pin::None) {
                    command = item.action.command(Target::Secret(secret.id));
                    ui.close();
                }
            }
        });
        (command, hovered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_rounds_up_to_the_second() {
        assert_eq!(clock(Duration::from_millis(272_100)), "4:33");
        assert_eq!(clock(Duration::from_mins(1)), "1:00");
        assert_eq!(clock(Duration::ZERO), "0:00");
    }
}
