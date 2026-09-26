//! Design tokens, organized like modern web design systems (Radix/shadcn):
//!
//! 1. **Primitive scales**: 12-step OKLCH color [`Scale`]s generated from a
//!    seed color (gray, accent, red, green, amber).
//! 2. **Semantic roles**: the [`Palette`] (background, surface, border, text,
//!    accent, focus ring...) derived from the scales. Components only use
//!    semantic roles, so changing a knob restyles everything consistently.
//! 3. **A few global knobs**: [`ThemeConfig`] — dark/light, accent, gray tint,
//!    radius, scaling and density.
//!
//! ```
//! use rust_ui::{Theme, ThemeConfig, GrayTint, Density, hex};
//! let t = Theme::from_config(ThemeConfig {
//!     accent: hex("#a371f7"),
//!     gray: GrayTint::Mauve,
//!     radius: 8.0,
//!     density: Density::Compact,
//!     ..ThemeConfig::dark()
//! });
//! assert!(t.dark);
//! ```
//!
//! Every field of the resulting [`Theme`] is public, so individual tokens can
//! still be overridden after generation.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::anim::Easing;
use crate::color::{hex, Color};
use crate::element::Element;
use crate::style::{FontFamily, Shadow};

/// A 12-step color scale (Radix convention). Steps are 1-based:
///
/// | Steps | Use |
/// |---|---|
/// | 1–2 | App / subtle backgrounds |
/// | 3–5 | Component backgrounds: normal, hover, pressed |
/// | 6–8 | Borders: subtle, interactive, strong/focus |
/// | 9–10 | Solid fills: normal, hover |
/// | 11–12 | Text: low contrast, high contrast |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale(pub [Color; 12]);

impl Scale {
    /// Step 1..=12.
    pub fn step(&self, n: usize) -> Color {
        self.0[n.clamp(1, 12) - 1]
    }

    /// Generate a scale around `seed` (which becomes step 9) for a dark or light scheme.
    pub fn generate(seed: Color, dark: bool) -> Scale {
        let (sl, sc, sh) = seed.to_oklch();
        // Lightness and relative chroma per step, tuned after Radix Colors.
        let (ls, cs): ([f32; 12], [f32; 12]) = if dark {
            (
                [0.178, 0.198, 0.238, 0.268, 0.298, 0.335, 0.39, 0.47, sl, 0.0, 0.8, 0.94],
                [0.12, 0.14, 0.26, 0.34, 0.4, 0.46, 0.55, 0.7, 1.0, 1.0, 0.75, 0.25],
            )
        } else {
            (
                [0.994, 0.982, 0.958, 0.934, 0.908, 0.876, 0.83, 0.76, sl, 0.0, 0.5, 0.25],
                [0.05, 0.12, 0.2, 0.28, 0.36, 0.44, 0.55, 0.72, 1.0, 1.0, 0.9, 0.45],
            )
        };
        let mut steps = [Color::BLACK; 12];
        for i in 0..12 {
            let l = match i {
                9 => {
                    // Hover: a bit lighter in dark mode, darker in light mode.
                    if dark {
                        (sl + 0.05).min(0.97)
                    } else {
                        (sl - 0.045).max(0.05)
                    }
                }
                _ => ls[i],
            };
            steps[i] = if i == 8 { seed.with_alpha(1.0) } else { Color::oklch(l, sc * cs[i], sh) };
        }
        Scale(steps)
    }
}

/// Neutral color families (Radix-style grays with a subtle tint).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum GrayTint {
    /// Pure neutral gray.
    Gray,
    /// Cool, blue-tinted (Tailwind slate).
    Slate,
    /// Very slightly cool (Tailwind zinc) — the default.
    Zinc,
    /// Purple-tinted.
    Mauve,
    /// Warm, yellow-tinted.
    Sand,
    /// Green-tinted.
    Sage,
    /// Tinted with the accent hue.
    Accent,
    /// Any hue (degrees) and chroma.
    Custom {
        /// OKLCH hue in degrees.
        hue: f32,
        /// OKLCH chroma (neutral grays use about 0.01–0.03).
        chroma: f32,
    },
}

impl GrayTint {
    fn seed(&self, accent: Color, dark: bool) -> Color {
        let l = if dark { 0.56 } else { 0.62 };
        let (h, c) = match *self {
            GrayTint::Gray => (0.0, 0.0),
            GrayTint::Slate => (257.0, 0.03),
            GrayTint::Zinc => (286.0, 0.014),
            GrayTint::Mauve => (300.0, 0.022),
            GrayTint::Sand => (80.0, 0.018),
            GrayTint::Sage => (160.0, 0.018),
            GrayTint::Accent => (accent.to_oklch().2, 0.02),
            GrayTint::Custom { hue, chroma } => (hue, chroma),
        };
        Color::oklch(l, c, h)
    }
}

/// Named accent presets (Radix/Tailwind-inspired hues tuned to work as step 9
/// of a generated scale in both light and dark mode). Any `Color` works as an
/// accent too; these are just good starting points for pickers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Accent {
    /// Blue (`#3b82f6`).
    Blue,
    /// Indigo (`#6366f1`).
    Indigo,
    /// Violet (`#8b5cf6`).
    Violet,
    /// Purple (`#a855f7`).
    Purple,
    /// Pink (`#ec4899`).
    Pink,
    /// Rose (`#f43f5e`).
    Rose,
    /// Red (`#e5484d`).
    Red,
    /// Orange (`#f76b15`).
    Orange,
    /// Amber (`#ffb000`).
    Amber,
    /// Lime (`#84cc16`).
    Lime,
    /// Green (`#22c55e`).
    Green,
    /// Emerald (`#10b981`).
    Emerald,
    /// Teal (`#14b8a6`).
    Teal,
    /// Cyan (`#06b6d4`).
    Cyan,
    /// Sky (`#0ea5e9`).
    Sky,
    /// Neutral (gray-on-gray, like Vercel/Linear monochrome UIs).
    Mono,
}

impl Accent {
    /// Every preset, in color-wheel order (for swatch pickers).
    pub const ALL: [Accent; 16] = [
        Accent::Blue,
        Accent::Indigo,
        Accent::Violet,
        Accent::Purple,
        Accent::Pink,
        Accent::Rose,
        Accent::Red,
        Accent::Orange,
        Accent::Amber,
        Accent::Lime,
        Accent::Green,
        Accent::Emerald,
        Accent::Teal,
        Accent::Cyan,
        Accent::Sky,
        Accent::Mono,
    ];

    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            Accent::Blue => "Blue",
            Accent::Indigo => "Indigo",
            Accent::Violet => "Violet",
            Accent::Purple => "Purple",
            Accent::Pink => "Pink",
            Accent::Rose => "Rose",
            Accent::Red => "Red",
            Accent::Orange => "Orange",
            Accent::Amber => "Amber",
            Accent::Lime => "Lime",
            Accent::Green => "Green",
            Accent::Emerald => "Emerald",
            Accent::Teal => "Teal",
            Accent::Cyan => "Cyan",
            Accent::Sky => "Sky",
            Accent::Mono => "Mono",
        }
    }

    /// The seed color for this preset.
    pub fn color(self, dark: bool) -> Color {
        hex(match self {
            Accent::Blue => "#3b82f6",
            Accent::Indigo => "#6366f1",
            Accent::Violet => "#8b5cf6",
            Accent::Purple => "#a855f7",
            Accent::Pink => "#ec4899",
            Accent::Rose => "#f43f5e",
            Accent::Red => "#e5484d",
            Accent::Orange => "#f76b15",
            Accent::Amber => "#ffb000",
            Accent::Lime => "#84cc16",
            Accent::Green => "#22c55e",
            Accent::Emerald => "#10b981",
            Accent::Teal => "#14b8a6",
            Accent::Cyan => "#06b6d4",
            Accent::Sky => "#0ea5e9",
            Accent::Mono if dark => "#e4e4e7",
            Accent::Mono => "#27272a",
        })
    }
}

/// How much space controls take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    /// Smaller controls and rows (26px controls at 1× scaling).
    Compact,
    /// Standard sizing (30px controls at 1× scaling).
    #[default]
    Default,
    /// Roomier controls and rows (34px controls at 1× scaling).
    Comfortable,
}

/// How strongly colors are separated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Contrast {
    /// The standard palette.
    #[default]
    Normal,
    /// For low vision and bright environments (like Windows contrast
    /// themes): near-black or white surfaces, opaque borders, no shadows, and
    /// text at WCAG AAA (7:1) contrast, secondary text, accents and state
    /// colors at 4.5:1 or more, and borders at 3:1 or more.
    High,
}

/// The global knobs a theme is generated from.
///
/// ```
/// use rust_ui::prelude::*;
/// let t = ThemeConfig { accent: Accent::Teal.color(false), radius: 10.0, ..ThemeConfig::light() }.build();
/// assert!(!t.dark);
/// assert_eq!(t.radius_lg, 16.0);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeConfig {
    /// Dark mode if true, light mode otherwise.
    pub dark: bool,
    /// Brand/accent color (becomes step 9 of the accent scale).
    pub accent: Color,
    /// Tint of the neutral gray scale.
    pub gray: GrayTint,
    /// Base corner radius in px (small = ×0.6, large = ×1.6, like shadcn).
    pub radius: f32,
    /// Overall size multiplier for text and controls (1.0 = 13px UI text).
    pub scaling: f32,
    /// Control and row sizing.
    pub density: Density,
    /// UI text font.
    pub font: FontFamily,
    /// Monospace font (code, editors).
    pub mono_font: FontFamily,
    /// Normal or high contrast.
    pub contrast: Contrast,
}

impl ThemeConfig {
    /// Default dark-mode knobs (blue accent, zinc grays, 6px radius).
    pub fn dark() -> Self {
        Self {
            dark: true,
            accent: hex("#5b8cff"),
            gray: GrayTint::Zinc,
            radius: 6.0,
            scaling: 1.0,
            density: Density::Default,
            font: FontFamily::Ui,
            mono_font: FontFamily::Mono,
            contrast: Contrast::Normal,
        }
    }
    /// Default light-mode knobs; like [`ThemeConfig::dark`] with a slightly deeper accent.
    pub fn light() -> Self {
        Self { dark: false, accent: hex("#3867f5"), ..Self::dark() }
    }
    /// Generate the theme; same as [`Theme::from_config`].
    pub fn build(self) -> Theme {
        Theme::from_config(self)
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self::dark()
    }
}

/// The primitive color scales a theme was generated from.
#[derive(Debug, Clone, PartialEq)]
pub struct Scales {
    /// Neutral scale, seeded from [`ThemeConfig::gray`].
    pub gray: Scale,
    /// Accent scale, seeded from [`ThemeConfig::accent`].
    pub accent: Scale,
    /// Red scale (source of `danger`).
    pub red: Scale,
    /// Green scale (source of `success`).
    pub green: Scale,
    /// Amber scale (source of `warning`).
    pub amber: Scale,
}

/// Semantic color roles. Components use these, never raw scale steps.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    /// Window background (behind everything).
    pub background: Color,
    /// Primary content surface (editor area, main page).
    pub surface: Color,
    /// Side panels / sidebars.
    pub panel: Color,
    /// Title bar, activity bar and status-bar-like chrome.
    pub chrome: Color,
    /// Raised surfaces: menus, popovers, cards.
    pub elevated: Color,
    /// Input field backgrounds.
    pub input: Color,
    /// Default border and divider color (also splitters).
    pub border: Color,
    /// Higher-contrast border, e.g. for checkboxes.
    pub border_strong: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text (descriptions, labels).
    pub text_muted: Color,
    /// Tertiary text: placeholders, hints, disabled labels.
    pub text_faint: Color,
    /// Accent fill, e.g. primary buttons and active indicators.
    pub accent: Color,
    /// Accent fill on hover.
    pub accent_hover: Color,
    /// Text and icons on `accent` backgrounds.
    pub accent_text: Color,
    /// Translucent accent background for selected or highlighted rows.
    pub accent_soft: Color,
    /// Destructive actions and errors.
    pub danger: Color,
    /// Success states.
    pub success: Color,
    /// Warnings.
    pub warning: Color,
    /// Translucent overlay used for hover states on neutral elements.
    pub hover: Color,
    /// Translucent overlay used for pressed states on neutral elements.
    pub pressed: Color,
    /// Text selection highlight.
    pub selection: Color,
    /// Keyboard focus ring.
    pub focus_ring: Color,
    /// Scrollbar thumb.
    pub scrollbar: Color,
    /// Scrollbar thumb while hovered or dragged.
    pub scrollbar_hover: Color,
    /// Status bar background.
    pub status_bar: Color,
    /// Status bar text.
    pub status_text: Color,
    /// Inline code and other "literal" text (accent-tinted, readable on surfaces).
    pub code_text: Color,
    /// Text on `danger` backgrounds.
    pub danger_text: Color,
}

/// Derive the high-contrast palette: flat near-black (or white) surfaces,
/// opaque borders, and every foreground pushed to its contrast target.
fn high_contrast(base: &Palette, dark: bool, gray: &Scale, accent: &Scale) -> Palette {
    let (ink, paper) = if dark { (Color::WHITE, Color::BLACK) } else { (Color::BLACK, Color::WHITE) };
    let bg = gray.step(1).lerp(paper, 0.85);
    // Menus and dialogs stand apart by a border, plus a slight lift.
    let elevated = gray.step(2).lerp(paper, 0.6);
    // Foregrounds must hold up on every surface they sit on.
    let fg = |c: Color, ratio: f32| c.with_contrast(bg, ratio).with_contrast(elevated, ratio);
    let text_muted = fg(gray.step(11), 7.0);
    let text_faint = fg(gray.step(10), 4.5);
    let accent_c = fg(accent.step(9), 4.5);
    let danger = fg(base.danger, 4.5);
    let readable = |c: Color| c.most_readable(ink, paper);
    Palette {
        background: bg,
        surface: bg,
        panel: bg,
        chrome: bg,
        elevated,
        input: bg,
        border: fg(gray.step(8), 3.0),
        border_strong: fg(gray.step(10), 4.5),
        text: ink,
        text_muted,
        text_faint,
        accent: accent_c,
        accent_hover: accent_c.lerp(ink, 0.25),
        accent_text: readable(accent_c),
        accent_soft: accent_c.with_alpha(0.28),
        danger,
        success: fg(base.success, 4.5),
        warning: fg(base.warning, 4.5),
        hover: ink.with_alpha(0.14),
        pressed: ink.with_alpha(0.24),
        selection: accent_c.with_alpha(0.45),
        focus_ring: accent_c,
        scrollbar: text_faint,
        scrollbar_hover: text_muted,
        status_bar: bg,
        status_text: text_muted,
        code_text: fg(accent.step(11), 7.0),
        danger_text: readable(danger),
    }
}

/// A complete theme: palette plus shape, typography and motion tokens.
///
/// ```
/// use rust_ui::prelude::*;
/// let t = Theme::light().with_accent_preset(Accent::Violet).with_density(Density::Compact);
/// assert_eq!(t.config.density, Density::Compact);
/// assert!(t.colors.text.contrast(t.colors.background) >= 7.0);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Display name, e.g. "Dark" or "High Contrast Light".
    pub name: String,
    /// Whether this is a dark theme.
    pub dark: bool,
    /// The knobs this theme was generated from.
    pub config: ThemeConfig,
    /// Primitive color scales.
    pub scales: Scales,
    /// Semantic colors used by widgets.
    pub colors: Palette,
    /// Small corner radius (0.6× base), for small elements like checkboxes and list rows.
    pub radius_sm: f32,
    /// Base corner radius, used by most controls.
    pub radius: f32,
    /// Large corner radius (1.6× base), for cards, dialogs and popovers.
    pub radius_lg: f32,
    /// UI text font.
    pub font: FontFamily,
    /// Monospace font.
    pub mono_font: FontFamily,
    /// Default UI text size in px.
    pub font_size: f32,
    /// Small text size (captions, hints).
    pub font_size_sm: f32,
    /// Large text size (dialog titles).
    pub font_size_lg: f32,
    /// Default line height, as a multiple of font size.
    pub line_height: f32,
    /// Standard control height (buttons, inputs).
    pub control_height: f32,
    /// List / tree / menu row height.
    pub row_height: f32,
    /// Tab strip height.
    pub tab_height: f32,
    /// Standard interaction transition, in seconds.
    pub transition: f32,
    /// Easing curve for standard transitions.
    pub easing: Easing,
    /// Width of keyboard focus rings.
    pub focus_ring_width: f32,
    /// Shadow used for popovers, menus and dialogs.
    pub shadow_popover: Vec<Shadow>,
    /// Shadow used for small raised controls.
    pub shadow_sm: Vec<Shadow>,
    /// Height of the custom window title bar.
    pub titlebar_height: f32,
    /// Extra hit area in px on each side of a pane splitter.
    pub splitter_hit: f32,
    /// Delay before a hovered splitter highlights (VS Code uses 300ms).
    pub splitter_hover_delay: f32,
    /// Named style classes (see [`Theme::style_class`]).
    pub classes: StyleClasses,
}

/// A style class: restyles an element with the regular builder methods.
pub type ClassFn = Rc<dyn Fn(Element<()>) -> Element<()>>;

/// Named style classes of a theme, like CSS classes. Every built-in widget
/// tags itself with classes (`"button"`, `"button-primary"`, `"input"`,
/// `"tab-active"`, …), so a theme can restyle all of them at once; apps can
/// define their own and apply them with [`Element::class`].
#[derive(Clone, Default)]
pub struct StyleClasses(Rc<HashMap<String, Vec<ClassFn>>>);

impl StyleClasses {
    /// The class functions registered under `name`, in registration order.
    pub fn get(&self, name: &str) -> Option<&[ClassFn]> {
        self.0.get(name).map(Vec::as_slice)
    }
    /// Whether no classes are registered.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Names of all registered classes, in no particular order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

impl PartialEq for StyleClasses {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for StyleClasses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::dark()
    }
}

impl Theme {
    /// The default dark theme.
    pub fn dark() -> Self {
        Self::from_config(ThemeConfig::dark())
    }

    /// The default light theme.
    pub fn light() -> Self {
        Self::from_config(ThemeConfig::light())
    }

    /// Generate a complete theme from a handful of knobs.
    pub fn from_config(cfg: ThemeConfig) -> Self {
        let dark = cfg.dark;
        let gray = Scale::generate(cfg.gray.seed(cfg.accent, dark), dark);
        let accent = Scale::generate(cfg.accent, dark);
        let red = Scale::generate(if dark { hex("#ef5f6b") } else { hex("#dc3e4c") }, dark);
        let green = Scale::generate(if dark { hex("#3ecf8e") } else { hex("#1a9e64") }, dark);
        let amber = Scale::generate(if dark { hex("#f5b94a") } else { hex("#c98a12") }, dark);
        let g = |n| gray.step(n);
        let a = |n| accent.step(n);
        // White on the accent unless that drops below WCAG's 3:1 for UI components
        // (e.g. orange, amber, lime, near-white "mono"); then near-black.
        let on = |bg: Color| {
            if bg.contrast(Color::WHITE) >= 3.0 {
                Color::WHITE
            } else {
                bg.most_readable(Color::WHITE, hex("#111113"))
            }
        };
        let on_accent = on(accent.step(9));
        let on_danger = on(red.step(9));
        let colors = if dark {
            Palette {
                background: g(1),
                surface: g(2).lerp(g(1), 0.35),
                panel: g(1).lerp(g(2), 0.35),
                chrome: g(1).darken(0.12),
                elevated: g(3).lerp(g(2), 0.4),
                input: g(1).darken(0.2),
                // Translucent borders read well on every dark surface (shadcn).
                border: Color::WHITE.with_alpha(0.085),
                border_strong: Color::WHITE.with_alpha(0.15),
                text: g(12),
                text_muted: g(11).lerp(g(12), 0.1),
                text_faint: g(9).lerp(g(11), 0.25),
                accent: a(9),
                accent_hover: a(10),
                accent_text: on_accent,
                accent_soft: a(9).with_alpha(0.16),
                danger: red.step(9),
                success: green.step(9),
                warning: amber.step(9),
                hover: Color::WHITE.with_alpha(0.06),
                pressed: Color::WHITE.with_alpha(0.1),
                selection: a(9).with_alpha(0.35),
                focus_ring: a(8).lerp(a(9), 0.5).with_alpha(0.55),
                scrollbar: Color::WHITE.with_alpha(0.16),
                scrollbar_hover: Color::WHITE.with_alpha(0.28),
                status_bar: g(1).darken(0.12),
                status_text: g(11),
                code_text: a(11),
                danger_text: on_danger,
            }
        } else {
            Palette {
                background: g(2),
                surface: Color::WHITE,
                panel: g(1).lerp(g(2), 0.5),
                chrome: g(2).lerp(g(3), 0.5),
                elevated: Color::WHITE,
                input: Color::WHITE,
                border: g(6).lerp(g(5), 0.3),
                border_strong: g(7),
                text: g(12),
                text_muted: g(11),
                text_faint: g(9),
                accent: a(9),
                accent_hover: a(10),
                accent_text: on_accent,
                accent_soft: a(9).with_alpha(0.12),
                danger: red.step(9),
                success: green.step(9),
                warning: amber.step(9),
                hover: Color::BLACK.with_alpha(0.05),
                pressed: Color::BLACK.with_alpha(0.09),
                selection: a(9).with_alpha(0.25),
                focus_ring: a(8).lerp(a(9), 0.5).with_alpha(0.5),
                scrollbar: Color::BLACK.with_alpha(0.18),
                scrollbar_hover: Color::BLACK.with_alpha(0.32),
                status_bar: g(2).lerp(g(3), 0.5),
                status_text: g(11),
                code_text: a(11),
                danger_text: on_danger,
            }
        };
        let high = cfg.contrast == Contrast::High;
        let colors = if high { high_contrast(&colors, dark, &gray, &accent) } else { colors };
        let k = cfg.scaling.max(0.5);
        let (control, row, tab) = match cfg.density {
            Density::Compact => (26.0, 22.0, 30.0),
            Density::Default => (30.0, 26.0, 36.0),
            Density::Comfortable => (34.0, 30.0, 40.0),
        };
        // Tailwind-style layered shadows; stronger in dark mode where the
        // background is already dark.
        let sh = if dark { 0.35 } else { 0.1 };
        let shadows = |v: Vec<Shadow>| if high { Vec::new() } else { v };
        Theme {
            name: match (dark, high) {
                (true, false) => "Dark".into(),
                (false, false) => "Light".into(),
                (true, true) => "High Contrast Dark".into(),
                (false, true) => "High Contrast Light".into(),
            },
            dark,
            scales: Scales { gray, accent, red, green, amber },
            colors,
            radius_sm: cfg.radius * 0.6,
            radius: cfg.radius,
            radius_lg: cfg.radius * 1.6,
            font: cfg.font.clone(),
            mono_font: cfg.mono_font.clone(),
            font_size: (13.0 * k).round(),
            font_size_sm: (12.0 * k).round(),
            font_size_lg: (15.0 * k).round(),
            line_height: 1.45,
            control_height: (control * k).round(),
            row_height: (row * k).round(),
            tab_height: (tab * k).round(),
            transition: 0.15,
            easing: Easing::Standard,
            focus_ring_width: 3.0,
            shadow_popover: shadows(vec![
                Shadow::new(0.0, 10.0, 15.0, -3.0, Color::BLACK.with_alpha(sh * 1.3)),
                Shadow::new(0.0, 4.0, 6.0, -4.0, Color::BLACK.with_alpha(sh * 1.3)),
                Shadow::new(0.0, 20.0, 40.0, -8.0, Color::BLACK.with_alpha(sh)),
            ]),
            shadow_sm: shadows(vec![
                Shadow::new(0.0, 1.0, 3.0, 0.0, Color::BLACK.with_alpha(sh)),
                Shadow::new(0.0, 1.0, 2.0, -1.0, Color::BLACK.with_alpha(sh)),
            ]),
            titlebar_height: (38.0 * k).round(),
            splitter_hit: 4.0,
            splitter_hover_delay: 0.3,
            classes: StyleClasses::default(),
            config: cfg,
        }
    }

    /// Add a style class. Built-in widgets use these names (see
    /// `docs/CUSTOMIZING.md` for the full list): `button`, `button-primary`,
    /// `button-secondary`, `button-ghost`, `button-danger`, `icon-button`,
    /// `input`, `text-area`, `checkbox`, `switch`, `slider`, `progress`,
    /// `badge`, `tag`, `card`, `tab`, `tab-active`, `menu`, `menu-item`,
    /// `modal`, `table-row`, … Registering the same name again adds to it.
    ///
    /// ```
    /// # use rust_ui::prelude::*;
    /// let t = Theme::dark()
    ///     .style_class("button", |e| e.pill().px(18.0))
    ///     .style_class("card", |e| e.rounded(16.0).no_shadow());
    /// ```
    ///
    /// Class styles apply after the widget's defaults and before anything the
    /// app chains on the element itself, like CSS specificity. Only style
    /// and state styles (`hover`, `active`, `focus_style`) are taken; children
    /// added by a class function are ignored.
    pub fn style_class(mut self, name: impl Into<String>, f: impl Fn(Element<()>) -> Element<()> + 'static) -> Self {
        Rc::make_mut(&mut self.classes.0).entry(name.into()).or_default().push(Rc::new(f));
        self
    }

    /// Rebuild from new knobs, keeping the style classes.
    fn rebuilt(&self, cfg: ThemeConfig) -> Self {
        Self { classes: self.classes.clone(), ..Self::from_config(cfg) }
    }

    /// Same theme with a different accent color.
    pub fn with_accent(self, accent: Color) -> Self {
        self.rebuilt(ThemeConfig { accent, ..self.config.clone() })
    }

    /// Same theme with a different density.
    pub fn with_density(self, density: Density) -> Self {
        self.rebuilt(ThemeConfig { density, ..self.config.clone() })
    }

    /// Same theme with an accent preset (resolved for this theme's mode).
    pub fn with_accent_preset(self, accent: Accent) -> Self {
        let color = accent.color(self.config.dark);
        self.with_accent(color)
    }

    /// Same knobs, switched between dark and light mode.
    pub fn with_dark(self, dark: bool) -> Self {
        self.rebuilt(ThemeConfig { dark, ..self.config.clone() })
    }

    /// Same theme with a different base corner radius.
    pub fn with_radius(self, radius: f32) -> Self {
        self.rebuilt(ThemeConfig { radius, ..self.config.clone() })
    }

    /// Same theme with a different size multiplier.
    pub fn with_scaling(self, scaling: f32) -> Self {
        self.rebuilt(ThemeConfig { scaling, ..self.config.clone() })
    }

    /// Same theme with a different gray tint.
    pub fn with_gray(self, gray: GrayTint) -> Self {
        self.rebuilt(ThemeConfig { gray, ..self.config.clone() })
    }

    /// Same theme with a different contrast level.
    pub fn with_contrast(self, contrast: Contrast) -> Self {
        self.rebuilt(ThemeConfig { contrast, ..self.config.clone() })
    }
}

thread_local! {
    static CURRENT: RefCell<Rc<Theme>> = RefCell::new(Rc::new(Theme::dark()));
}

/// The theme active for the view currently being built. Widgets call this to
/// pick up design tokens.
///
/// ```
/// use rust_ui::prelude::*;
/// let th = theme();
/// let label: Element<()> = text("Saved").color(th.colors.success).font_size(th.font_size_sm);
/// ```
pub fn theme() -> Rc<Theme> {
    CURRENT.with(|t| t.borrow().clone())
}

pub(crate) fn set_theme(t: Rc<Theme>) {
    CURRENT.with(|c| *c.borrow_mut() = t);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast(a: Color, b: Color) -> f32 {
        a.contrast(b)
    }

    #[test]
    fn every_accent_preset_is_readable_in_both_modes() {
        for dark in [true, false] {
            for a in Accent::ALL {
                for gray in [GrayTint::Zinc, GrayTint::Accent, GrayTint::Sand] {
                    let t =
                        Theme::from_config(ThemeConfig { dark, accent: a.color(dark), gray, ..ThemeConfig::dark() });
                    let c = &t.colors;
                    let on = contrast(c.accent_text, c.accent);
                    assert!(on >= 3.0, "{} button text contrast {on:.2} (dark={dark})", a.name());
                    let code = contrast(c.code_text, c.surface);
                    assert!(code >= 4.5, "{} code text contrast {code:.2} (dark={dark})", a.name());
                    let body = contrast(c.text, c.background);
                    assert!(body >= 7.0, "{} body text contrast {body:.2} (dark={dark})", a.name());
                }
            }
        }
    }

    #[test]
    fn scales_are_monotonic_and_readable() {
        for dark in [true, false] {
            let t = Theme::from_config(ThemeConfig { dark, ..ThemeConfig::dark() });
            let l: Vec<f32> = t.scales.gray.0.iter().map(|c| c.to_oklch().0).collect();
            for w in l.windows(2).take(7) {
                assert!(if dark { w[1] > w[0] } else { w[1] < w[0] }, "dark={dark} {l:?}");
            }
            let c = &t.colors;
            assert!(contrast(c.text, c.surface) >= 12.0, "text contrast dark={dark}");
            assert!(contrast(c.text_muted, c.surface) >= 6.0, "muted contrast dark={dark}");
            assert!(contrast(c.text_faint, c.surface) >= 3.0, "faint contrast dark={dark}");
            assert!(contrast(c.accent_text, c.accent) >= 3.0, "on-accent contrast dark={dark}");
        }
    }

    #[test]
    fn knobs_apply() {
        let t = Theme::from_config(ThemeConfig {
            radius: 10.0,
            scaling: 1.2,
            density: Density::Compact,
            ..ThemeConfig::light()
        });
        assert_eq!(t.radius_lg, 16.0);
        assert_eq!(t.font_size, 16.0);
        assert_eq!(t.control_height, 31.0);
        // A light accent gets dark text.
        let y = Theme::light().with_accent(hex("#ffe066"));
        assert!(y.colors.accent_text.luminance() < 0.2);
    }
}
