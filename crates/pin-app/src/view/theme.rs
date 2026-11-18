use eframe::egui::{self, Color32, FontId, TextStyle, Vec2};

pub const WINDOW_SIZE: Vec2 = Vec2::new(560.0, 440.0);
pub const CORNER: f32 = 12.0;

#[cfg(target_os = "macos")]
pub const CMD: &str = "\u{2318}";
#[cfg(target_os = "macos")]
pub const BACKSPACE: &str = "\u{232B}";
#[cfg(not(target_os = "macos"))]
pub const CMD: &str = "Ctrl+";
#[cfg(not(target_os = "macos"))]
pub const BACKSPACE: &str = "Backspace";

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub border: Color32,
    pub separator: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub tertiary: Color32,
    pub accent: Color32,
    pub hover: Color32,
    pub icon: Color32,
}

impl Palette {
    pub fn of(ctx: &egui::Context) -> Self {
        if ctx.global_style().visuals.dark_mode {
            Self {
                background: Color32::from_rgba_unmultiplied(30, 30, 32, 250),
                border: Color32::from_white_alpha(30),
                separator: Color32::from_white_alpha(18),
                text: Color32::from_rgb(236, 236, 240),
                secondary: Color32::from_gray(160),
                tertiary: Color32::from_gray(112),
                accent: Color32::from_rgb(10, 132, 255),
                hover: Color32::from_white_alpha(12),
                icon: Color32::from_white_alpha(20),
            }
        } else {
            Self {
                background: Color32::from_rgba_unmultiplied(248, 248, 250, 250),
                border: Color32::from_black_alpha(36),
                separator: Color32::from_black_alpha(20),
                text: Color32::from_rgb(28, 28, 30),
                secondary: Color32::from_gray(108),
                tertiary: Color32::from_gray(150),
                accent: Color32::from_rgb(0, 122, 255),
                hover: Color32::from_black_alpha(10),
                icon: Color32::from_black_alpha(14),
            }
        }
    }
}

pub fn install(ctx: &egui::Context) {
    install_system_font(ctx);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(15.0)),
            (TextStyle::Body, FontId::proportional(13.5)),
            (TextStyle::Button, FontId::proportional(13.0)),
            (TextStyle::Small, FontId::proportional(11.0)),
            (TextStyle::Monospace, FontId::monospace(12.5)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 2.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.menu_margin = egui::Margin::same(6);
        style.spacing.scroll = egui::style::ScrollStyle::floating();
        style.visuals.menu_corner_radius = egui::CornerRadius::same(8);
        style.visuals.window_corner_radius = egui::CornerRadius::same(10);
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(6);
        }
    });
}

#[cfg(target_os = "macos")]
fn install_system_font(ctx: &egui::Context) {
    use egui::FontFamily;
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
    // egui needs the font bytes in memory; ~8 MB is the price of matching the native UI font.
    const SF: &str = "/System/Library/Fonts/SFNS.ttf";
    match std::fs::read(SF) {
        Ok(bytes) => ctx.add_font(FontInsert::new(
            "sf",
            egui::FontData::from_owned(bytes),
            vec![InsertFontFamily {
                family: FontFamily::Proportional,
                priority: FontPriority::Highest,
            }],
        )),
        Err(err) => tracing::debug!(%err, "system font unavailable"),
    }
}

#[cfg(not(target_os = "macos"))]
fn install_system_font(_ctx: &egui::Context) {}
