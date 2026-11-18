use eframe::egui::{
    self, Align, FontId, Frame, Key, Label, Layout, Margin, Rect, RichText, ScrollArea, Sense,
    Stroke, TextEdit, pos2, vec2,
};
use pin_core::Timestamp;

use super::controls;
use super::format;
use super::icons::{self, Icon};
use super::secrets;
use super::theme::Palette;
use crate::app::{App, Command, Focus, LabelTarget, Sheet, SheetAction};

const WIDTH: f32 = 320.0;

/// Everything a sheet says, apart from its fields.
pub(super) struct Wording {
    icon: Icon,
    danger: bool,
    title: &'static str,
    subtitle: &'static str,
    pub primary: &'static str,
    cancel: bool,
    link: Option<(&'static str, SheetAction)>,
    pub esc: &'static str,
}

pub(super) fn wording(sheet: &Sheet) -> Wording {
    let base = Wording {
        icon: Icon::Lock,
        danger: false,
        title: "",
        subtitle: "",
        primary: "",
        cancel: true,
        link: None,
        esc: "Cancel",
    };
    match sheet {
        Sheet::Create { .. } => Wording {
            icon: Icon::Shield,
            title: "Set Up Secrets",
            subtitle: "Choose a master password. It encrypts every secret and is never stored; \
                       a recovery key comes next.",
            primary: "Continue",
            ..base
        },
        Sheet::Unlock { then, .. } => Wording {
            title: "Secrets Are Locked",
            subtitle: "Enter your master password. Secrets lock again when left unused for a while.",
            primary: "Unlock",
            cancel: false,
            link: Some(("Forgot Password?", SheetAction::Forgot)),
            esc: if then.is_some() {
                "Cancel"
            } else {
                "Back to History"
            },
            ..base
        },
        Sheet::Recover { .. } => Wording {
            icon: Icon::Key,
            title: "Recover Secrets",
            subtitle: "Enter your recovery key and choose a new master password. You will get a \
                       new recovery key after this.",
            primary: "Recover",
            link: Some(("Back to Unlock", SheetAction::Back)),
            ..base
        },
        Sheet::RecoveryKey { saved, .. } => Wording {
            icon: Icon::Key,
            title: "Save Your Recovery Key",
            subtitle: "It is the only way back in if you forget the master password. pin shows \
                       it once and replaces it each time it is used.",
            primary: "Done",
            cancel: false,
            esc: if *saved {
                "Done"
            } else {
                "Unavailable until saved"
            },
            ..base
        },
        Sheet::Label {
            target: LabelTarget::Move(_),
            ..
        } => Wording {
            title: "Move to Secrets",
            subtitle: "The item leaves history and is stored encrypted. Name it so you can find \
                       it again.",
            primary: "Move to Secrets",
            esc: "Keep in History",
            ..base
        },
        Sheet::Label {
            target: LabelTarget::Edit(_),
            ..
        } => Wording {
            icon: Icon::Key,
            title: "Edit Secret",
            subtitle: "Change its name or note. The value stays as it is.",
            primary: "Save",
            ..base
        },
        Sheet::ConfirmDelete { .. } => Wording {
            icon: Icon::Trash,
            danger: true,
            title: "Delete This Secret?",
            subtitle: "It is removed from the vault for good. This can\u{2019}t be undone.",
            primary: "Delete",
            ..base
        },
    }
}

impl App {
    pub(super) fn sheet_ui(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<Command> {
        self.hide_value();
        let focus = self.take_focus(Focus::Sheet);
        if focus {
            // Sheets without fields would leave focus in the search, which then eats Enter.
            ui.ctx().memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
        let quiet = self.quiet_for();
        if let Some(wait) = quiet {
            ui.ctx().request_repaint_after(wait);
        }
        let summary = self.sheet_summary();
        let error = self.sheet_error.clone();
        let sheet = self.sheet.as_mut()?;
        let words = wording(sheet);
        let mut command = None;
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(22.0);
                heading(ui, palette, &words);
                ui.add_space(18.0);
                ui.allocate_ui_with_layout(vec2(WIDTH, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(WIDTH);
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    if let Some(summary) = &summary {
                        summary.show(ui, palette);
                        ui.add_space(12.0);
                    }
                    let form = Form {
                        palette,
                        kind: sheet.kind(),
                        focus,
                        wrong: error.is_some() && matches!(sheet, Sheet::Unlock { .. }),
                        ready: quiet.is_none(),
                    };
                    if let Some(chosen) = form.fields(ui, sheet) {
                        command = Some(chosen);
                    }
                    if let Some(error) = &error {
                        ui.add_space(8.0);
                        problem(ui, palette, error);
                    }
                    ui.add_space(14.0);
                    let ready = quiet.is_none()
                        && !matches!(sheet, Sheet::RecoveryKey { saved: false, .. });
                    if let Some(chosen) = buttons(ui, palette, &words, ready) {
                        command = Some(chosen);
                    }
                    ui.add_space(16.0);
                });
            });
        });
        command
    }

    /// The item a sheet is about, shown above its fields; never its value.
    fn sheet_summary(&self) -> Option<Summary> {
        let now = Timestamp::now();
        match self.sheet.as_ref()? {
            Sheet::Label {
                target: LabelTarget::Move(id),
                ..
            } => {
                let entry = self.entries.iter().find(|e| e.id == *id)?;
                Some(Summary {
                    icon: format::icon(entry),
                    title: None,
                    detail: format!(
                        "{} \u{00B7} {}",
                        format::kind_label(entry, 1),
                        format::subtitle(entry, now)
                    ),
                })
            }
            Sheet::Label {
                target: LabelTarget::Edit(id),
                ..
            }
            | Sheet::ConfirmDelete { id } => {
                let secret = self.secrets.iter().find(|s| s.id == *id)?;
                Some(Summary {
                    icon: secrets::icon(&secret.kind),
                    title: Some(secret.name.clone()),
                    detail: format!(
                        "{} \u{00B7} updated {}",
                        secrets::kind_label(&secret.kind),
                        format::relative(now, secret.updated_at).to_lowercase()
                    ),
                })
            }
            _ => None,
        }
    }
}

/// A one-line card for the entry or secret a sheet acts on; `title: None` hides the content.
struct Summary {
    icon: Icon,
    title: Option<String>,
    detail: String,
}

impl Summary {
    fn show(&self, ui: &mut egui::Ui, palette: &Palette) {
        Frame::new()
            .fill(palette.well)
            .corner_radius(8)
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let (rect, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                    icons::tile(ui.painter(), rect, self.icon, palette);
                    if self.title.is_none() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new("hidden").size(11.0).color(palette.tertiary));
                            self.lines(ui, palette);
                        });
                    } else {
                        self.lines(ui, palette);
                    }
                });
            });
    }

    fn lines(&self, ui: &mut egui::Ui, palette: &Palette) {
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            let title = match &self.title {
                Some(title) => RichText::new(title).size(13.0),
                None => RichText::new("\u{2022}".repeat(16))
                    .size(13.0)
                    .extra_letter_spacing(2.0),
            };
            ui.add(Label::new(title.color(palette.text)).truncate());
            ui.add(
                Label::new(
                    RichText::new(&self.detail)
                        .size(11.5)
                        .color(palette.secondary),
                )
                .truncate(),
            );
        });
    }
}

fn heading(ui: &mut egui::Ui, palette: &Palette, words: &Wording) {
    let (fill, color) = if words.danger {
        (palette.danger_tile, palette.danger)
    } else {
        (palette.chip, palette.accent_text)
    };
    let (rect, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
    ui.painter().rect_filled(rect, 11.0, fill);
    icons::paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), vec2(20.0, 20.0)),
        words.icon,
        color,
    );
    ui.add_space(12.0);
    ui.label(RichText::new(words.title).size(16.0).color(palette.text));
    ui.add_space(5.0);
    ui.allocate_ui_with_layout(
        vec2(380.0_f32.min(ui.available_width()), 0.0),
        Layout::top_down(Align::Center),
        |ui| {
            ui.add(
                Label::new(
                    RichText::new(words.subtitle)
                        .size(12.5)
                        .color(palette.secondary),
                )
                .wrap(),
            );
        },
    );
}

fn problem(ui: &mut egui::Ui, palette: &Palette, message: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        controls::icon(ui, Icon::Alert, 13.0, palette.danger);
        ui.add(Label::new(RichText::new(message).size(12.0).color(palette.danger)).wrap());
    });
}

struct Form<'a> {
    palette: &'a Palette,
    kind: &'static str,
    ready: bool,
    focus: bool,
    wrong: bool,
}

impl Form<'_> {
    fn fields(&self, ui: &mut egui::Ui, sheet: &mut Sheet) -> Option<Command> {
        let mut entered = false;
        match sheet {
            Sheet::Create {
                password, confirm, ..
            } => {
                entered |= self.field(ui, "Master Password", password, Input::Password(self.focus));
                if !password.is_empty() {
                    ui.add_space(8.0);
                    meter(ui, self.palette, strength(password));
                }
                ui.add_space(12.0);
                entered |= self.field(ui, "Confirm Password", confirm, Input::Password(false));
            }
            Sheet::Unlock { password, .. } => {
                entered |= self.field(ui, "Master Password", password, Input::Password(self.focus));
            }
            Sheet::Recover {
                key,
                password,
                confirm,
                ..
            } => {
                entered |= self.field(ui, "Recovery Key", key, Input::Code(self.focus));
                ui.add_space(12.0);
                entered |= self.field(ui, "New Master Password", password, Input::Password(false));
                ui.add_space(12.0);
                entered |= self.field(ui, "Confirm Password", confirm, Input::Password(false));
            }
            Sheet::RecoveryKey { key, saved, .. } => {
                return recovery_key(ui, self.palette, key, saved, self.ready);
            }
            Sheet::Label { name, note, .. } => {
                entered |= self.field(
                    ui,
                    "Name",
                    name,
                    Input::Text(self.focus, "e.g. Staging database"),
                );
                ui.add_space(12.0);
                entered |= self.field(
                    ui,
                    "Note (optional)",
                    note,
                    Input::Text(false, "Where it is used"),
                );
            }
            Sheet::ConfirmDelete { .. } => {}
        }
        entered.then_some(Command::Sheet(SheetAction::Submit))
    }

    /// Returns whether Enter was pressed in this field.
    fn field(&self, ui: &mut egui::Ui, label: &str, text: &mut String, input: Input) -> bool {
        let palette = self.palette;
        ui.label(RichText::new(label).size(11.5).color(palette.secondary));
        ui.add_space(5.0);
        let (focus, password, mono, hint) = match input {
            Input::Password(focus) => (focus, true, false, ""),
            Input::Code(focus) => (focus, false, true, ""),
            Input::Text(focus, hint) => (focus, false, false, hint),
        };
        let border = if self.wrong {
            palette.danger
        } else {
            palette.border
        };
        let response = ui
            .scope(|ui| {
                let visuals = ui.visuals_mut();
                visuals.extreme_bg_color = palette.field;
                visuals.selection.stroke = Stroke::new(1.0, palette.accent);
                for widget in [
                    &mut visuals.widgets.inactive,
                    &mut visuals.widgets.hovered,
                    &mut visuals.widgets.active,
                ] {
                    widget.bg_stroke = Stroke::new(1.0, border);
                    widget.corner_radius = egui::CornerRadius::same(7);
                }
                let font = if mono {
                    FontId::monospace(12.5)
                } else {
                    FontId::proportional(14.0)
                };
                ui.add(
                    TextEdit::singleline(text)
                        .id_salt((self.kind, label))
                        .password(password)
                        .hint_text(RichText::new(hint).color(palette.tertiary))
                        .font(font)
                        .text_color(palette.text)
                        .margin(vec2(10.0, 8.0))
                        .desired_width(f32::INFINITY),
                )
            })
            .inner;
        if response.has_focus() {
            ui.painter().rect_stroke(
                response.rect.expand(1.0),
                8.0,
                Stroke::new(3.0, palette.focus_ring),
                egui::StrokeKind::Outside,
            );
        }
        if focus {
            response.request_focus();
        }
        response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
    }
}

#[derive(Clone, Copy)]
enum Input {
    Password(bool),
    Code(bool),
    Text(bool, &'static str),
}

fn recovery_key(
    ui: &mut egui::Ui,
    palette: &Palette,
    key: &str,
    saved: &mut bool,
    ready: bool,
) -> Option<Command> {
    const GROUP: egui::Vec2 = egui::Vec2::new(50.0, 26.0);
    const GAP: f32 = 6.0;
    const PER_ROW: usize = 5;
    let mut command = None;
    Frame::new()
        .fill(palette.well)
        .stroke(Stroke::new(1.0, palette.separator))
        .corner_radius(10)
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            let width = ui.available_width();
            ui.set_width(width);
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
            let groups: Vec<&str> = key.split('-').collect();
            for row in groups.chunks(PER_ROW) {
                ui.horizontal(|ui| {
                    let used = row.iter().map(|_| GROUP.x + GAP).sum::<f32>() - GAP;
                    ui.add_space(((width - used) / 2.0).max(0.0) - GAP);
                    for group in row {
                        let (rect, _) = ui.allocate_exact_size(GROUP, Sense::hover());
                        ui.painter().rect(
                            rect,
                            5.0,
                            palette.background,
                            Stroke::new(1.0, palette.separator),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            *group,
                            FontId::monospace(13.0),
                            palette.text,
                        );
                    }
                });
            }
        });
    ui.add_space(12.0);
    // Clicks queued while Argon2 ran must not copy the key or tick the box.
    let copy = controls::bordered(ui, palette, Some(Icon::Copy), "Copy Key", Some(WIDTH));
    if copy.clicked() && ready {
        command = Some(Command::Sheet(SheetAction::CopyRecoveryKey));
    }
    ui.add_space(6.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("Hidden from other clipboard apps and cleared after 90 s.")
                .size(11.5)
                .color(palette.tertiary),
        );
    });
    ui.add_space(12.0);
    ui.add_enabled(
        ready,
        egui::Checkbox::new(
            saved,
            RichText::new("I\u{2019}ve stored this key somewhere safe")
                .size(12.5)
                .color(palette.text),
        ),
    );
    command
}

fn buttons(ui: &mut egui::Ui, palette: &Palette, words: &Wording, ready: bool) -> Option<Command> {
    let mut command = None;
    ui.allocate_ui_with_layout(
        vec2(WIDTH, 32.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            if let Some((label, action)) = words.link {
                let link = ui.add(
                    Label::new(RichText::new(label).size(12.0).color(palette.accent_text))
                        .sense(Sense::click()),
                );
                if link
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    command = Some(Command::Sheet(action));
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let fill = if words.danger {
                    palette.danger_solid
                } else {
                    palette.accent
                };
                if controls::primary(ui, words.primary, fill, ready).clicked() {
                    command = Some(Command::Sheet(SheetAction::Submit));
                }
                if words.cancel {
                    ui.add_space(8.0);
                    if controls::bordered(ui, palette, None, "Cancel", None).clicked() && ready {
                        command = Some(Command::Sheet(SheetAction::Cancel));
                    }
                }
            });
        },
    );
    command
}

/// A rough 0–4 score from length and the kinds of characters used; 0 for an empty password.
fn strength(password: &str) -> u8 {
    let length = password.chars().count();
    let by_length = match length {
        0 => return 0,
        1..8 => 1,
        8..12 => 2,
        12..16 => 3,
        _ => 4,
    };
    let classes = [
        password.chars().any(char::is_lowercase),
        password.chars().any(char::is_uppercase),
        password.chars().any(|c| c.is_ascii_digit()),
        password.chars().any(|c| !c.is_alphanumeric()),
    ]
    .into_iter()
    .filter(|&has| has)
    .count();
    let bonus = u8::from(classes >= 3 && by_length >= 2);
    (by_length + bonus).min(4)
}

fn meter(ui: &mut egui::Ui, palette: &Palette, score: u8) {
    let (label, color, text) = match score {
        0 | 1 => ("Weak", palette.danger, palette.danger),
        2 => (
            "Fair",
            palette.hue(super::theme::Hue::Amber),
            palette.hue(super::theme::Hue::Amber),
        ),
        3 => ("Good", palette.success, palette.success_text),
        _ => ("Strong", palette.success, palette.success_text),
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let label_width = 44.0;
        let bars = (ui.available_width() - label_width - 8.0).max(0.0);
        let (rect, _) = ui.allocate_exact_size(vec2(bars, 4.0), Sense::hover());
        let step = (rect.width() - 12.0) / 4.0;
        for index in 0..4_u8 {
            let left = rect.left() + f32::from(index) * (step + 4.0);
            let bar = Rect::from_min_size(pos2(left, rect.top()), vec2(step, 4.0));
            let fill = if index < score { color } else { palette.icon };
            ui.painter().rect_filled(bar, 2.0, fill);
        }
        ui.label(RichText::new(label).size(11.5).color(text));
    });
}

/// The footer of a sheet: what Enter and Esc do there.
pub(super) fn footer_hints(ui: &mut egui::Ui, palette: &Palette, sheet: &Sheet) {
    let words = wording(sheet);
    controls::hint(ui, palette, "esc", words.esc);
    ui.add_space(10.0);
    controls::hint(ui, palette, "\u{21A9}", words.primary);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_grows_with_length_and_variety() {
        assert_eq!(strength(""), 0);
        assert_eq!(strength("abc"), 1);
        assert_eq!(strength("abcdefgh"), 2);
        assert_eq!(strength("Abcdefg1"), 3);
        assert_eq!(strength("paper lantern orbit 9"), 4);
    }
}
