use eframe::egui::{self, Color32, FontId, TextStyle, Vec2};

pub const WINDOW_SIZE: Vec2 = Vec2::new(780.0, 480.0);
pub const CORNER: f32 = 14.0;

#[cfg(target_os = "macos")]
pub const CMD: &str = "\u{2318}";
#[cfg(target_os = "macos")]
pub const CMD_KEY: &str = "\u{2318}";
#[cfg(target_os = "macos")]
pub const BACKSPACE: &str = "\u{232B}";
#[cfg(target_os = "macos")]
pub const ALT: &str = "\u{2325}";
#[cfg(not(target_os = "macos"))]
pub const CMD: &str = "Ctrl+";
#[cfg(not(target_os = "macos"))]
pub const CMD_KEY: &str = "Ctrl";
#[cfg(not(target_os = "macos"))]
pub const BACKSPACE: &str = "Backspace";
#[cfg(not(target_os = "macos"))]
pub const ALT: &str = "Alt";

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
    pub danger_solid: Color32,
    pub danger_tile: Color32,
    pub success: Color32,
    pub success_text: Color32,
    pub field: Color32,
    pub focus_ring: Color32,
    pub dark: bool,
}

/// Colour families for icon tiles, one per kind of content or file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hue {
    Grey,
    Blue,
    Orange,
    Purple,
    Green,
    Red,
    Teal,
    Pink,
    Amber,
    Indigo,
    Slate,
}

impl Palette {
    /// Glyph colour of `hue`; each shade keeps 4.5:1 contrast against its own tile.
    pub fn hue(&self, hue: Hue) -> Color32 {
        let (light, dark) = match hue {
            Hue::Grey => ([0x5B, 0x61, 0x6B], [0xB4, 0xB8, 0xC0]),
            Hue::Blue => ([0x0A, 0x63, 0xC9], [0x62, 0xAE, 0xFF]),
            Hue::Orange => ([0xB4, 0x50, 0x0B], [0xFF, 0xA0, 0x5A]),
            Hue::Purple => ([0x73, 0x39, 0xC8], [0xBE, 0x93, 0xFF]),
            Hue::Green => ([0x1D, 0x7F, 0x38], [0x5F, 0xD6, 0x7E]),
            Hue::Red => ([0xBF, 0x2A, 0x1C], [0xFF, 0x7D, 0x73]),
            Hue::Teal => ([0x0B, 0x74, 0x80], [0x55, 0xD3, 0xDE]),
            Hue::Pink => ([0xB5, 0x22, 0x72], [0xFF, 0x7C, 0xC2]),
            Hue::Amber => ([0x8C, 0x5E, 0x00], [0xF5, 0xC4, 0x51]),
            Hue::Indigo => ([0x48, 0x40, 0xD6], [0x9C, 0x98, 0xFF]),
            Hue::Slate => ([0x3B, 0x57, 0xA3], [0x93, 0xAA, 0xF5]),
        };
        let [r, g, b] = if self.dark { dark } else { light };
        Color32::from_rgb(r, g, b)
    }

    /// Tile behind a glyph of `hue`: the same colour at low opacity.
    pub fn hue_tile(&self, hue: Hue) -> Color32 {
        let [r, g, b, _] = self.hue(hue).to_array();
        Color32::from_rgba_unmultiplied(r, g, b, if self.dark { 51 } else { 33 })
    }
    pub fn of(ctx: &egui::Context) -> Self {
        // Grey text shades keep 4.5:1 contrast against the background, even at 11 px.
        if ctx.global_style().visuals.dark_mode {
            Self {
                background: Color32::from_rgb(36, 36, 39),
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
                danger_solid: Color32::from_rgb(215, 55, 63),
                danger_tile: Color32::from_rgba_unmultiplied(255, 107, 107, 41),
                success: Color32::from_rgb(48, 209, 88),
                success_text: Color32::from_rgb(91, 212, 122),
                field: Color32::from_white_alpha(13),
                focus_ring: Color32::from_rgba_unmultiplied(10, 132, 255, 77),
                dark: true,
            }
        } else {
            Self {
                background: Color32::from_rgb(246, 246, 248),
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
                danger_solid: Color32::from_rgb(196, 0, 26),
                danger_tile: Color32::from_rgba_unmultiplied(196, 0, 26, 26),
                success: Color32::from_rgb(40, 167, 69),
                success_text: Color32::from_rgb(30, 123, 52),
                field: Color32::WHITE,
                focus_ring: Color32::from_rgba_unmultiplied(0, 100, 210, 46),
                dark: false,
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
