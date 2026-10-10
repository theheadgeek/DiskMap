//! The Inactive Node theme, taken from the design system's tokens.json.
//! Every colour the app draws comes from this file.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke, TextStyle, Visuals, vec2};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// Tokens from the design system, by their token names.
pub const PAPER: Color32 = Color32::from_rgb(0x0b, 0x0a, 0x0f);
pub const SURFACE: Color32 = Color32::from_rgb(0x14, 0x10, 0x19);
pub const SURFACE_2: Color32 = Color32::from_rgb(0x1e, 0x18, 0x26);
pub const INK: Color32 = Color32::from_rgb(0xdc, 0xdb, 0xe3);
pub const INK_SOFT: Color32 = Color32::from_rgb(0x8c, 0x88, 0x98);
pub const LINE: Color32 = Color32::from_rgb(0x35, 0x2b, 0x43);
pub const FRETWIRE: Color32 = Color32::from_rgb(0x52, 0x46, 0x6e);
pub const TONE: Color32 = Color32::from_rgb(0x8e, 0x7e, 0xb6);
pub const ROOT: Color32 = Color32::from_rgb(0xd8, 0x1f, 0x29);
pub const ACCENT: Color32 = Color32::from_rgb(0xe5, 0x63, 0x38);
pub const RELATED: Color32 = Color32::from_rgb(0x6e, 0x8f, 0xc0);
pub const OK: Color32 = Color32::from_rgb(0x63, 0xc0, 0x83);
pub const BAD: Color32 = Color32::from_rgb(0xf0, 0x65, 0x5f);
pub const BAD_INK: Color32 = Color32::from_rgb(0x1a, 0x05, 0x05);
/// New to DiskMap: the middle step between ok and bad, for "delete with care".
pub const CAUTION: Color32 = Color32::from_rgb(0xc9, 0xa5, 0x4a);

/// Treemap colours for the largest file types, biggest first. The design system
/// has no data palette, so these are new to DiskMap: muted, mid-lightness hues
/// that sit beside tone and related without borrowing root red or accent orange,
/// which keep their meanings (identity and interaction).
pub const TYPE_COLORS: [Color32; 12] = [
    TONE,
    RELATED,
    OK,
    Color32::from_rgb(0xc9, 0xa5, 0x4a), // gold
    Color32::from_rgb(0x4f, 0xa3, 0xa5), // teal
    Color32::from_rgb(0xc0, 0x66, 0x8f), // rose
    Color32::from_rgb(0x8f, 0xa3, 0x5a), // olive
    Color32::from_rgb(0xb3, 0xa2, 0xdc), // lilac
    Color32::from_rgb(0x5f, 0xb0, 0xd8), // sky
    Color32::from_rgb(0xb0, 0x8a, 0x6e), // sand
    Color32::from_rgb(0x9a, 0x5c, 0x9e), // plum
    Color32::from_rgb(0x7d, 0x8a, 0xa3), // slate
];
/// Every file type past the first twelve.
pub const TYPE_OTHER: Color32 = Color32::from_rgb(0x5c, 0x56, 0x6a);
/// Folders too small on screen to split into their files.
pub const FOLDER: Color32 = FRETWIRE;

/// Fades a colour most of the way into the paper, for map tiles that aren't
/// the file type being highlighted.
pub fn dim(c: Color32) -> Color32 {
    let mix = |v: u8, p: u8| (v as f32 * 0.18 + p as f32 * 0.82) as u8;
    Color32::from_rgb(mix(c.r(), PAPER.r()), mix(c.g(), PAPER.g()), mix(c.b(), PAPER.b()))
}

/// Multiplies a colour's brightness, for the treemap's cushion shading.
pub fn shade(c: Color32, factor: f32) -> Color32 {
    let f = |v: u8| (v as f32 * factor).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// The display face, used for the app's title.
pub fn display_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("display".into()))
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());

    let mut v = Visuals::dark();
    v.panel_fill = SURFACE;
    v.window_fill = SURFACE;
    v.extreme_bg_color = PAPER;
    v.faint_bg_color = SURFACE_2;
    v.code_bg_color = SURFACE_2;
    v.weak_text_color = Some(INK_SOFT);
    v.hyperlink_color = ACCENT;
    v.error_fg_color = BAD;
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(12);
    // The `shadow` token: a tight black drop plus a faint red glow from root.
    let glow = Shadow {
        offset: [0, 1],
        blur: 28,
        spread: 0,
        color: ROOT.gamma_multiply(0.13),
    };
    v.window_shadow = glow;
    v.popup_shadow = glow;
    // Selection inverts: an ink block with paper text.
    v.selection.bg_fill = INK;
    v.selection.stroke = Stroke::new(1.0, PAPER);

    let control = CornerRadius::same(9);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = SURFACE;
    w.noninteractive.weak_bg_fill = SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    w.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    for (state, border) in [
        (&mut w.inactive, LINE),
        (&mut w.hovered, INK_SOFT),
        (&mut w.active, ACCENT),
        (&mut w.open, ACCENT),
    ] {
        state.bg_fill = SURFACE_2;
        state.weak_bg_fill = SURFACE_2;
        state.bg_stroke = Stroke::new(1.0, border);
        state.fg_stroke = Stroke::new(1.0, INK);
        state.corner_radius = control;
        state.expansion = 0.0;
    }

    ctx.all_styles_mut(|style| {
        style.visuals = v.clone();
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(10.0, 4.0);
        // Sizes from the type scale: control, button, caption, readout and card-title.
        style.text_styles = [
            (TextStyle::Body, FontId::proportional(14.5)),
            (TextStyle::Button, FontId::proportional(13.0)),
            (TextStyle::Small, FontId::proportional(12.5)),
            (TextStyle::Monospace, FontId::monospace(12.0)),
            (TextStyle::Heading, FontId::proportional(19.0)),
        ]
        .into();
    });
}

/// The design system names Fraunces, Archivo and Spline Sans Mono first, then
/// system faces. Use the brand fonts when they're installed, and the system
/// fallbacks otherwise. egui's built-in fonts stay last, for symbols.
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let families = [
        (
            FontFamily::Name("display".into()),
            "Fraunces",
            "/System/Library/Fonts/Supplemental/Iowan Old Style.ttc",
            true,
        ),
        (
            FontFamily::Proportional,
            "Archivo",
            "/System/Library/Fonts/HelveticaNeue.ttc",
            false,
        ),
        (FontFamily::Monospace, "SplineSansMono", "/System/Library/Fonts/Menlo.ttc", false),
    ];
    for (family, brand, system, is_new_family) in families {
        let fallbacks = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
        let list = fonts.families.entry(family).or_default();
        if is_new_family {
            *list = fallbacks;
        }
        let path = installed_font(brand).or_else(|| Some(PathBuf::from(system)).filter(|p| p.exists()));
        if let Some(bytes) = path.and_then(|p| std::fs::read(p).ok()) {
            fonts.font_data.insert(brand.to_owned(), Arc::new(FontData::from_owned(bytes)));
            list.insert(0, brand.to_owned());
        }
    }
    fonts
}

/// Looks for an installed regular-weight brand font, e.g. "Archivo-Regular.ttf".
fn installed_font(name: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = [Path::new(&home).join("Library/Fonts"), PathBuf::from("/Library/Fonts")];
    let wanted = [format!("{name}-Regular.ttf"), format!("{name}-Regular.otf")];
    dirs.iter()
        .flat_map(|dir| wanted.iter().map(move |file| dir.join(file)))
        .find(|path| path.exists())
}
