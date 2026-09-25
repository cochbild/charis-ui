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
use std::rc::Rc;

use crate::anim::Easing;
use crate::color::{hex, Color};
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
    Custom { hue: f32, chroma: f32 },
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

/// How much space controls take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    Compact,
    #[default]
    Default,
    Comfortable,
}

/// The global knobs a theme is generated from.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeConfig {
    pub dark: bool,
    /// Brand/accent color (becomes step 9 of the accent scale).
    pub accent: Color,
    pub gray: GrayTint,
    /// Base corner radius in px (small = ×0.6, large = ×1.6, like shadcn).
    pub radius: f32,
    /// Overall size multiplier for text and controls (1.0 = 13px UI text).
    pub scaling: f32,
    pub density: Density,
    pub font: FontFamily,
    pub mono_font: FontFamily,
}

impl ThemeConfig {
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
        }
    }
    pub fn light() -> Self {
        Self { dark: false, accent: hex("#3867f5"), ..Self::dark() }
    }
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
    pub gray: Scale,
    pub accent: Scale,
    pub red: Scale,
    pub green: Scale,
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
    pub border: Color,
    pub border_strong: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_faint: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_text: Color,
    pub accent_soft: Color,
    pub danger: Color,
    pub success: Color,
    pub warning: Color,
    /// Translucent overlay used for hover states on neutral elements.
    pub hover: Color,
    /// Translucent overlay used for pressed states on neutral elements.
    pub pressed: Color,
    pub selection: Color,
    pub focus_ring: Color,
    pub scrollbar: Color,
    pub scrollbar_hover: Color,
    pub status_bar: Color,
    pub status_text: Color,
}

/// A complete theme: palette plus shape, typography and motion tokens.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    pub dark: bool,
    /// The knobs this theme was generated from.
    pub config: ThemeConfig,
    pub scales: Scales,
    pub colors: Palette,
    pub radius_sm: f32,
    pub radius: f32,
    pub radius_lg: f32,
    pub font: FontFamily,
    pub mono_font: FontFamily,
    pub font_size: f32,
    pub font_size_sm: f32,
    pub font_size_lg: f32,
    pub line_height: f32,
    /// Standard control height (buttons, inputs).
    pub control_height: f32,
    /// List / tree / menu row height.
    pub row_height: f32,
    /// Tab strip height.
    pub tab_height: f32,
    /// Standard interaction transition, in seconds.
    pub transition: f32,
    pub easing: Easing,
    /// Width of keyboard focus rings.
    pub focus_ring_width: f32,
    /// Shadow used for popovers, menus and dialogs.
    pub shadow_popover: Vec<Shadow>,
    /// Shadow used for small raised controls.
    pub shadow_sm: Vec<Shadow>,
    pub titlebar_height: f32,
    pub splitter_hit: f32,
    /// Delay before a hovered splitter highlights (VS Code uses 300ms).
    pub splitter_hover_delay: f32,
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
        let on_accent = if accent.step(9).to_oklch().0 > 0.72 { hex("#111113") } else { Color::WHITE };
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
            }
        };
        let k = cfg.scaling.max(0.5);
        let (control, row, tab) = match cfg.density {
            Density::Compact => (26.0, 22.0, 30.0),
            Density::Default => (30.0, 26.0, 36.0),
            Density::Comfortable => (34.0, 30.0, 40.0),
        };
        // Tailwind-style layered shadows; stronger in dark mode where the
        // background is already dark.
        let sh = if dark { 0.35 } else { 0.1 };
        Theme {
            name: if dark { "Dark".into() } else { "Light".into() },
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
            shadow_popover: vec![
                Shadow::new(0.0, 10.0, 15.0, -3.0, Color::BLACK.with_alpha(sh * 1.3)),
                Shadow::new(0.0, 4.0, 6.0, -4.0, Color::BLACK.with_alpha(sh * 1.3)),
                Shadow::new(0.0, 20.0, 40.0, -8.0, Color::BLACK.with_alpha(sh)),
            ],
            shadow_sm: vec![
                Shadow::new(0.0, 1.0, 3.0, 0.0, Color::BLACK.with_alpha(sh)),
                Shadow::new(0.0, 1.0, 2.0, -1.0, Color::BLACK.with_alpha(sh)),
            ],
            titlebar_height: (38.0 * k).round(),
            splitter_hit: 4.0,
            splitter_hover_delay: 0.3,
            config: cfg,
        }
    }

    /// Same theme with a different accent color.
    pub fn with_accent(self, accent: Color) -> Self {
        Self::from_config(ThemeConfig { accent, ..self.config })
    }

    /// Same theme with a different density.
    pub fn with_density(self, density: Density) -> Self {
        Self::from_config(ThemeConfig { density, ..self.config })
    }

    /// Same theme with a different gray tint.
    pub fn with_gray(self, gray: GrayTint) -> Self {
        Self::from_config(ThemeConfig { gray, ..self.config })
    }
}

thread_local! {
    static CURRENT: RefCell<Rc<Theme>> = RefCell::new(Rc::new(Theme::dark()));
}

/// The theme active for the view currently being built. Widgets call this to
/// pick up design tokens.
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
        let lum = |c: Color| {
            let f = |x: f32| if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
            0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
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
