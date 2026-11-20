//! Trazo's visual identity: a white and light-blue light theme (there is no
//! dark mode), the system UI font when available, and the colours shared by
//! the panels and the drawing canvas.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, TextStyle, Vec2,
};

const fn rgb(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

// ---------- interface surfaces ----------

/// Panels, menus, dialogs.
pub const PANEL: Color32 = rgb(0xFFFFFF);
/// The ribbon body and section headers: a breath of blue over white.
pub const PANEL_ALT: Color32 = rgb(0xF5F9FE);
/// Bands that frame the work area: tab strip, view header, status bar.
pub const BAND: Color32 = rgb(0xEDF4FC);
/// Lines between panels.
pub const BORDER: Color32 = rgb(0xDCE6F1);
/// Outlines of controls (fields, buttons), stronger than panel lines.
pub const BORDER_STRONG: Color32 = rgb(0xBFD0E3);

pub const TEXT: Color32 = rgb(0x1C2B3C);
pub const TEXT_2: Color32 = rgb(0x3B4E63);
pub const MUTED: Color32 = rgb(0x627489);
pub const FAINT: Color32 = rgb(0xA3B2C2);

pub const ACCENT: Color32 = rgb(0x1E6FD9);
pub const ACCENT_STRONG: Color32 = rgb(0x175CB8);
pub const ACCENT_TEXT: Color32 = rgb(0x1659B0);
/// Selected rows and tools.
pub const ACCENT_SOFT: Color32 = rgb(0xDDEBFC);
/// Hovered rows and tools.
pub const ACCENT_SOFTER: Color32 = rgb(0xEEF5FE);
pub const ACCENT_RING: Color32 = rgb(0x93BDF0);

pub const SUCCESS: Color32 = rgb(0x22925F);
pub const WARNING: Color32 = rgb(0xD5841B);
pub const DANGER: Color32 = rgb(0xCC3F33);

// ---------- drawing canvas ----------

pub const CANVAS: Color32 = rgb(0xFFFFFF);
pub const GRID_MINOR: Color32 = rgb(0xF0F5FB);
pub const GRID_MAJOR: Color32 = rgb(0xDDE8F5);
pub const WALL: Color32 = rgb(0x1D2836);
pub const WALL_FILL: Color32 = rgb(0xFFFFFF);
pub const WALL_FILL_HOVER: Color32 = ACCENT_SOFTER;
pub const WALL_FILL_SELECTED: Color32 = ACCENT_SOFT;
pub const ROOM_FILL: Color32 = rgb(0xEEF5FD);
pub const ROOM_FILL_HOVER: Color32 = rgb(0xE4EFFC);
pub const ROOM_FILL_SELECTED: Color32 = rgb(0xD6E7FB);
pub const TERRAIN_FILL: Color32 = rgb(0xF8FBFE);
pub const TERRAIN_FILL_SELECTED: Color32 = rgb(0xE9F2FD);
pub const TERRAIN_LINE: Color32 = rgb(0x5E8BBF);
pub const DOOR_ARC: Color32 = rgb(0x8EA2B8);
pub const WINDOW: Color32 = rgb(0x2878D2);
pub const SELECTION: Color32 = ACCENT;
pub const HOVER: Color32 = rgb(0x5B9CE8);
/// What the connected AI assistant is doing on the plan.
pub const AI: Color32 = rgb(0x7C3AED);
/// Electrical devices and their tags, as on electrical drawings.
pub const ELECTRICAL: Color32 = rgb(0xC62828);

/// The semibold family, used for titles and strong labels.
pub fn strong_family() -> FontFamily {
    FontFamily::Name("semibold".into())
}

pub fn strong(size: f32) -> FontId {
    FontId::new(size, strong_family())
}

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn apply(context: &egui::Context) {
    install_fonts(context);
    context.set_theme(egui::ThemePreference::Light);
    context.set_style_of(egui::Theme::Light, style());
}

/// Uses the operating system's interface font when it can be found (Segoe UI
/// on Windows), keeping egui's bundled fonts as fallbacks for other glyphs.
fn install_fonts(context: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut load = |name: &str, candidates: &[&str]| -> bool {
        for path in candidates {
            if let Some(bytes) = std::fs::read(path)
                .ok()
                .filter(|bytes| is_usable_font(bytes))
            {
                fonts
                    .font_data
                    .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
                return true;
            }
        }
        false
    };
    let regular = load(
        "ui-regular",
        &[
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/System/Library/Fonts/SFNS.ttf",
        ],
    );
    let semibold = load(
        "ui-semibold",
        &[
            "C:\\Windows\\Fonts\\seguisb.ttf",
            "C:\\Windows\\Fonts\\segoeuib.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
            "/usr/share/fonts/noto/NotoSans-SemiBold.ttf",
        ],
    );
    let monospace = load(
        "ui-mono",
        &[
            "C:\\Windows\\Fonts\\consola.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        ],
    );
    // Arrows and shapes the interface font lacks (▾ ↻ ↺ ↕ ⌀ …), which would
    // otherwise show as empty boxes in labels such as "Girar 90° ↻".
    let symbols = load(
        "ui-symbols",
        &[
            "C:\\Windows\\Fonts\\seguisym.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/System/Library/Fonts/Apple Symbols.ttf",
        ],
    );

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    if regular {
        proportional.insert(0, "ui-regular".to_owned());
    }
    if symbols {
        // Right after the interface font, before egui's bundled fallbacks.
        let at = usize::from(regular);
        proportional.insert(at, "ui-symbols".to_owned());
    }
    let mut strong_list = proportional.clone();
    if semibold {
        strong_list.insert(0, "ui-semibold".to_owned());
    }
    fonts.families.insert(strong_family(), strong_list);
    if monospace {
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "ui-mono".to_owned());
    }
    if symbols {
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push("ui-symbols".to_owned());
    }
    context.set_fonts(fonts);
}

/// Whether `bytes` hold a font egui can use. A damaged system font would
/// otherwise make egui panic (and the app not start), and a truncated one
/// would draw nothing; either is skipped for the bundled fonts.
fn is_usable_font(bytes: &[u8]) -> bool {
    use skrifa::raw::TableProvider;
    let Ok(font) = skrifa::FontRef::from_index(bytes, 0) else {
        return false;
    };
    font.head().is_ok()
        && font.cmap().is_ok()
        && font.maxp().is_ok()
        && font.hhea().is_ok()
        && font.hmtx().is_ok()
}

fn widget(
    fill: Color32,
    stroke: Stroke,
    text: Color32,
    expansion: f32,
) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: stroke,
        corner_radius: CornerRadius::same(5),
        fg_stroke: Stroke::new(1.0, text),
        expansion,
    }
}

fn style() -> egui::Style {
    let mut style = egui::Style {
        visuals: egui::Visuals::light(),
        ..Default::default()
    };

    style.text_styles = [
        (TextStyle::Heading, strong(16.0)),
        (TextStyle::Body, regular(13.0)),
        (TextStyle::Button, regular(13.0)),
        (TextStyle::Small, regular(11.0)),
        (TextStyle::Monospace, mono(12.5)),
    ]
    .into();

    let spacing = &mut style.spacing;
    spacing.item_spacing = Vec2::new(8.0, 6.0);
    spacing.button_padding = Vec2::new(10.0, 4.0);
    spacing.interact_size = Vec2::new(36.0, 24.0);
    spacing.menu_margin = Margin::same(6);
    spacing.window_margin = Margin::same(18);
    spacing.indent = 16.0;
    spacing.menu_width = 230.0;
    spacing.tooltip_width = 320.0;
    spacing.icon_width = 15.0;
    spacing.icon_width_inner = 9.0;

    let visuals = &mut style.visuals;
    visuals.dark_mode = false;
    visuals.override_text_color = None;
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.faint_bg_color = PANEL_ALT;
    visuals.extreme_bg_color = PANEL;
    visuals.text_edit_bg_color = Some(PANEL);
    visuals.code_bg_color = BAND;
    visuals.hyperlink_color = ACCENT_TEXT;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = DANGER;
    visuals.window_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.window_shadow = Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(22, 50, 92, 46),
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(22, 50, 92, 38),
    };
    visuals.selection.bg_fill = rgb(0xC9DFFA);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT_TEXT);
    visuals.text_cursor.stroke = Stroke::new(1.5, ACCENT);
    visuals.slider_trailing_fill = true;
    visuals.striped = false;
    visuals.indent_has_left_vline = false;

    let widgets = &mut visuals.widgets;
    widgets.noninteractive = widget(PANEL, Stroke::new(1.0, BORDER), TEXT, 0.0);
    widgets.inactive = widget(rgb(0xF7FAFE), Stroke::new(1.0, BORDER_STRONG), TEXT, 0.0);
    widgets.hovered = widget(ACCENT_SOFTER, Stroke::new(1.0, ACCENT_RING), TEXT, 0.0);
    widgets.active = widget(ACCENT_SOFT, Stroke::new(1.0, ACCENT), ACCENT_TEXT, 0.0);
    widgets.open = widget(ACCENT_SOFTER, Stroke::new(1.0, ACCENT_RING), TEXT, 0.0);

    style
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color32) -> f32 {
        let channel = |value: u8| {
            let value = value as f32 / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn text_colours_are_legible_on_every_surface() {
        for surface in [PANEL, PANEL_ALT, BAND, ACCENT_SOFT, ACCENT_SOFTER] {
            assert!(contrast(TEXT, surface) >= 7.0);
            assert!(contrast(TEXT_2, surface) >= 4.5);
            assert!(contrast(ACCENT_TEXT, surface) >= 4.5);
            assert!(contrast(MUTED, surface) >= 3.5);
        }
    }

    #[test]
    fn damaged_font_files_are_rejected() {
        assert!(!is_usable_font(b""));
        assert!(!is_usable_font(b"not a font at all"));
        assert!(!is_usable_font(&[0x00, 0x01, 0x00, 0x00, 0x00]));
        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
            assert!(is_usable_font(&bytes));
            assert!(!is_usable_font(&bytes[..bytes.len().min(64)]));
        }
    }

    #[test]
    fn the_theme_stays_light() {
        assert!(!style().visuals.dark_mode);
        assert!(luminance(PANEL) > 0.9 && luminance(BAND) > 0.85);
    }
}
