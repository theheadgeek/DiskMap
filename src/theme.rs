//! The app's themes: Inactive Node, taken from the design system's tokens.json,
//! and plain Mac-style Light and Dark. Every colour the app draws comes from this file.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke, TextStyle, Visuals, vec2};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

/// The look the user picked. Inactive Node is the brand theme; Light and Dark
/// are plain Mac-style themes, and System switches between those two with macOS.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Choice {
    #[default]
    InactiveNode,
    System,
    Light,
    Dark,
}

impl Choice {
    pub const ALL: [Choice; 4] = [Choice::InactiveNode, Choice::System, Choice::Light, Choice::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Choice::InactiveNode => "Inactive Node",
            Choice::System => "Match system",
            Choice::Light => "Light",
            Choice::Dark => "Dark",
        }
    }

    /// The palette to draw with, given whether macOS is in dark mode right now.
    pub fn palette(self, system: Option<egui::Theme>) -> &'static Palette {
        match self {
            Choice::InactiveNode => &INACTIVE_NODE,
            Choice::Light => &LIGHT,
            Choice::Dark => &DARK,
            Choice::System if system == Some(egui::Theme::Light) => &LIGHT,
            Choice::System => &DARK,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Choice::InactiveNode => "inactive-node",
            Choice::System => "system",
            Choice::Light => "light",
            Choice::Dark => "dark",
        }
    }

    /// The choice saved last time, or Inactive Node if there isn't one.
    pub fn load() -> Choice {
        let saved = std::fs::read_to_string(choice_file()).unwrap_or_default();
        Choice::ALL.into_iter().find(|c| c.key() == saved.trim()).unwrap_or_default()
    }

    pub fn save(self) {
        let file = choice_file();
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, self.key());
    }
}

/// ~/Library/Application Support/DiskMap/appearance
fn choice_file() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    Path::new(&home).join("Library/Application Support/DiskMap/appearance")
}

/// Every colour the app draws, by the Inactive Node design system's token names.
/// The Light and Dark palettes fill the same slots with macOS's system colours.
pub struct Palette {
    pub paper: Color32,
    pub surface: Color32,
    pub surface_2: Color32,
    pub ink: Color32,
    pub ink_soft: Color32,
    pub line: Color32,
    /// Share bars and spinners.
    pub tone: Color32,
    /// Interaction: links, the primary button, the active control's border.
    pub accent: Color32,
    pub ok: Color32,
    /// The middle step between ok and bad, for "delete with care".
    pub caution: Color32,
    pub bad: Color32,
    /// Text on a `bad` fill.
    pub bad_ink: Color32,
    /// A selected row's fill and text.
    pub selection: Color32,
    pub on_selection: Color32,
    /// The outline round the selected tile on the map.
    pub map_selection: Color32,
    /// Treemap colours for the largest file types, biggest first.
    pub type_colors: [Color32; 12],
    /// Every file type past the first twelve.
    pub type_other: Color32,
    /// Folders too small on screen to split into their files.
    pub folder: Color32,
    pub shadow: Shadow,
    pub dark: bool,
    /// Fraunces, Archivo and Spline Sans Mono, rather than the system faces alone.
    pub brand_fonts: bool,
    pub window_radius: u8,
    pub control_radius: u8,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Tokens from the design system's tokens.json.
pub static INACTIVE_NODE: Palette = Palette {
    paper: rgb(0x0b0a0f),
    surface: rgb(0x141019),
    surface_2: rgb(0x1e1826),
    ink: rgb(0xdcdbe3),
    ink_soft: rgb(0x8c8898),
    line: rgb(0x352b43),
    tone: rgb(0x8e7eb6),
    accent: rgb(0xe56338),
    ok: rgb(0x63c083),
    caution: rgb(0xc9a54a), // new to DiskMap
    bad: rgb(0xf0655f),
    bad_ink: rgb(0x1a0505),
    // Selection inverts: an ink block with paper text.
    selection: rgb(0xdcdbe3),
    on_selection: rgb(0x0b0a0f),
    map_selection: rgb(0xe56338),
    // The design system has no data palette, so these are new to DiskMap: muted,
    // mid-lightness hues that sit beside tone and related without borrowing root
    // red or accent orange, which keep their meanings (identity and interaction).
    type_colors: [
        rgb(0x8e7eb6), // tone
        rgb(0x6e8fc0), // related
        rgb(0x63c083), // ok
        rgb(0xc9a54a), // gold
        rgb(0x4fa3a5), // teal
        rgb(0xc0668f), // rose
        rgb(0x8fa35a), // olive
        rgb(0xb3a2dc), // lilac
        rgb(0x5fb0d8), // sky
        rgb(0xb08a6e), // sand
        rgb(0x9a5c9e), // plum
        rgb(0x7d8aa3), // slate
    ],
    type_other: rgb(0x5c566a),
    folder: rgb(0x52466e), // fretwire
    // The `shadow` token: a tight black drop plus a faint red glow from root (#d81f29 at 13%).
    shadow: Shadow {
        offset: [0, 1],
        blur: 28,
        spread: 0,
        color: Color32::from_rgba_premultiplied(28, 4, 5, 33),
    },
    dark: true,
    brand_fonts: true,
    window_radius: 16,
    control_radius: 9,
};

/// macOS's light appearance: system greys, with the accessible variants of the
/// system colours wherever they're used for text.
pub static LIGHT: Palette = Palette {
    paper: rgb(0xffffff),
    surface: rgb(0xf5f5f7),
    surface_2: rgb(0xe5e5ea),
    ink: rgb(0x1d1d1f),
    ink_soft: rgb(0x6e6e73),
    line: rgb(0xd2d2d7),
    tone: rgb(0x007aff),
    accent: rgb(0x007aff),
    ok: rgb(0x248a3d),
    caution: rgb(0xc46a00),
    bad: rgb(0xd70015),
    bad_ink: rgb(0xffffff),
    selection: rgb(0x007aff),
    on_selection: rgb(0xffffff),
    // Blue is a file type colour here, so the selected tile is outlined in ink.
    map_selection: rgb(0x000000),
    type_colors: [
        rgb(0x007aff), // blue
        rgb(0x34c759), // green
        rgb(0xff9500), // orange
        rgb(0xaf52de), // purple
        rgb(0x30b0c7), // teal
        rgb(0xff2d55), // pink
        rgb(0xffcc00), // yellow
        rgb(0x5856d6), // indigo
        rgb(0x00c7be), // mint
        rgb(0xa2845e), // brown
        rgb(0x32ade6), // cyan
        rgb(0x8e8e93), // grey
    ],
    type_other: rgb(0xc7c7cc),
    folder: rgb(0xd1d1d6),
    shadow: Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 0,
        color: Color32::from_rgba_premultiplied(0, 0, 0, 40),
    },
    dark: false,
    brand_fonts: false,
    window_radius: 10,
    control_radius: 6,
};

/// macOS's dark appearance.
pub static DARK: Palette = Palette {
    paper: rgb(0x1c1c1e),
    surface: rgb(0x2c2c2e),
    surface_2: rgb(0x3a3a3c),
    ink: rgb(0xf5f5f7),
    ink_soft: rgb(0x98989d),
    line: rgb(0x48484a),
    tone: rgb(0x0a84ff),
    accent: rgb(0x0a84ff),
    ok: rgb(0x30d158),
    caution: rgb(0xff9f0a),
    bad: rgb(0xff453a),
    bad_ink: rgb(0xffffff),
    selection: rgb(0x0a84ff),
    on_selection: rgb(0xffffff),
    map_selection: rgb(0xffffff),
    type_colors: [
        rgb(0x0a84ff), // blue
        rgb(0x30d158), // green
        rgb(0xff9f0a), // orange
        rgb(0xbf5af2), // purple
        rgb(0x40c8e0), // teal
        rgb(0xff375f), // pink
        rgb(0xffd60a), // yellow
        rgb(0x5e5ce6), // indigo
        rgb(0x66d4cf), // mint
        rgb(0xac8e68), // brown
        rgb(0x64d2ff), // cyan
        rgb(0x8e8e93), // grey
    ],
    type_other: rgb(0x636366),
    folder: rgb(0x48484a),
    shadow: Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 0,
        color: Color32::from_rgba_premultiplied(0, 0, 0, 110),
    },
    dark: true,
    brand_fonts: false,
    window_radius: 10,
    control_radius: 6,
};

/// The palette in use, as an index into `PALETTES`.
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PALETTES: [&Palette; 3] = [&INACTIVE_NODE, &LIGHT, &DARK];
/// The faces loaded so far: none yet, the brand fonts, or the system ones.
static FACES: AtomicU8 = AtomicU8::new(0);
const FACES_BRAND: u8 = 1;
const FACES_SYSTEM: u8 = 2;

/// The palette in use.
pub fn palette() -> &'static Palette {
    PALETTES[CURRENT.load(Ordering::Relaxed)]
}

/// Fades a colour most of the way into the paper, for map tiles that aren't
/// the file type being highlighted.
pub fn dim(c: Color32) -> Color32 {
    let paper = palette().paper;
    let mix = |v: u8, p: u8| (v as f32 * 0.18 + p as f32 * 0.82) as u8;
    Color32::from_rgb(mix(c.r(), paper.r()), mix(c.g(), paper.g()), mix(c.b(), paper.b()))
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

/// Switches to a palette, reloading the fonts only when it uses different faces.
pub fn apply(ctx: &egui::Context, p: &'static Palette) {
    let index = PALETTES.iter().position(|&candidate| std::ptr::eq(candidate, p)).unwrap_or(0);
    CURRENT.store(index, Ordering::Relaxed);
    let faces = if p.brand_fonts { FACES_BRAND } else { FACES_SYSTEM };
    if FACES.swap(faces, Ordering::Relaxed) != faces {
        ctx.set_fonts(fonts(p.brand_fonts));
    }

    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.surface;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.paper;
    v.faint_bg_color = p.surface_2;
    v.code_bg_color = p.surface_2;
    v.weak_text_color = Some(p.ink_soft);
    v.hyperlink_color = p.accent;
    v.error_fg_color = p.bad;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.window_corner_radius = CornerRadius::same(p.window_radius);
    v.menu_corner_radius = CornerRadius::same(p.window_radius - 4);
    v.window_shadow = p.shadow;
    v.popup_shadow = p.shadow;
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0, p.on_selection);

    let control = CornerRadius::same(p.control_radius);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.ink);
    for (state, border) in [
        (&mut w.inactive, p.line),
        (&mut w.hovered, p.ink_soft),
        (&mut w.active, p.accent),
        (&mut w.open, p.accent),
    ] {
        state.bg_fill = p.surface_2;
        state.weak_bg_fill = p.surface_2;
        state.bg_stroke = Stroke::new(1.0, border);
        state.fg_stroke = Stroke::new(1.0, p.ink);
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
/// fallbacks otherwise. The plain themes use the system faces alone, with
/// Helvetica Neue for titles too. egui's built-in fonts stay last, for symbols.
fn fonts(brand_fonts: bool) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let families = [
        (
            FontFamily::Name("display".into()),
            "Fraunces",
            if brand_fonts {
                "/System/Library/Fonts/Supplemental/Iowan Old Style.ttc"
            } else {
                "/System/Library/Fonts/HelveticaNeue.ttc"
            },
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
        let path = installed_font(brand)
            .filter(|_| brand_fonts)
            .or_else(|| Some(PathBuf::from(system)).filter(|p| p.exists()));
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
