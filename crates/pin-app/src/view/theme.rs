use eframe::egui::{self, Color32, FontId, TextStyle, Vec2};

pub const WINDOW_SIZE: Vec2 = Vec2::new(780.0, 480.0);
pub const CORNER: f32 = 14.0;

#[cfg(target_os = "macos")]
pub const CMD: &str = "\u{2318}";
#[cfg(target_os = "macos")]
pub const CMD_KEY: &str = "\u{2318}";
#[cfg(target_os = "macos")]
pub const BACKSPACE: &str = "\u{232B}";
#[cfg(not(target_os = "macos"))]
pub const CMD: &str = "Ctrl+";
#[cfg(not(target_os = "macos"))]
pub const CMD_KEY: &str = "Ctrl";
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
    pub accent_text: Color32,
    pub selection: Color32,
    pub chip: Color32,
    pub chip_border: Color32,
    pub hit: Color32,
    pub hover: Color32,
    pub icon: Color32,
    pub keycap: Color32,
    pub well: Color32,
    pub menu: Color32,
    pub danger: Color32,
}

impl Palette {
    pub fn of(ctx: &egui::Context) -> Self {
        // Grey text shades keep 4.5:1 contrast against the background, even at 11 px.
        if ctx.global_style().visuals.dark_mode {
            Self {
                background: Color32::from_rgba_unmultiplied(36, 36, 39, 250),
                border: Color32::from_white_alpha(30),
                separator: Color32::from_white_alpha(20),
                text: Color32::from_rgb(236, 236, 240),
                secondary: Color32::from_rgb(180, 180, 186),
                tertiary: Color32::from_rgb(142, 142, 147),
                accent: Color32::from_rgb(10, 111, 224),
                accent_text: Color32::from_rgb(90, 169, 255),
                selection: Color32::from_rgba_unmultiplied(10, 132, 255, 66),
                chip: Color32::from_rgba_unmultiplied(10, 132, 255, 51),
                chip_border: Color32::from_rgba_unmultiplied(10, 132, 255, 115),
                hit: Color32::from_rgba_unmultiplied(255, 214, 10, 87),
                hover: Color32::from_white_alpha(12),
                icon: Color32::from_white_alpha(20),
                keycap: Color32::from_white_alpha(26),
                well: Color32::from_white_alpha(10),
                menu: Color32::from_rgb(46, 46, 49),
                danger: Color32::from_rgb(255, 107, 107),
            }
        } else {
            Self {
                background: Color32::from_rgba_unmultiplied(246, 246, 248, 250),
                border: Color32::from_black_alpha(36),
                separator: Color32::from_black_alpha(20),
                text: Color32::from_rgb(28, 28, 30),
                secondary: Color32::from_rgb(79, 79, 85),
                tertiary: Color32::from_rgb(110, 110, 115),
                accent: Color32::from_rgb(0, 100, 210),
                accent_text: Color32::from_rgb(0, 88, 184),
                selection: Color32::from_rgba_unmultiplied(0, 100, 210, 33),
                chip: Color32::from_rgba_unmultiplied(0, 100, 210, 31),
                chip_border: Color32::from_rgba_unmultiplied(0, 100, 210, 71),
                hit: Color32::from_rgba_unmultiplied(255, 196, 0, 115),
                hover: Color32::from_black_alpha(10),
                icon: Color32::from_black_alpha(15),
                keycap: Color32::from_black_alpha(18),
                well: Color32::from_black_alpha(9),
                menu: Color32::WHITE,
                danger: Color32::from_rgb(196, 0, 26),
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
