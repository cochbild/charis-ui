//! Design tokens. Every built-in widget reads its colors, radii and sizes from
//! the active [`Theme`], so an app can restyle everything by changing a single
//! struct — or override any individual element with its own style.

use std::cell::RefCell;
use std::rc::Rc;

use crate::color::{hex, Color};
use crate::style::{FontFamily, Shadow};

/// Semantic color palette.
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
    /// Standard interaction transition, in seconds.
    pub transition: f32,
    /// Shadow used for popovers, menus and dialogs.
    pub shadow_popover: Vec<Shadow>,
    /// Shadow used for small raised controls.
    pub shadow_sm: Vec<Shadow>,
    pub titlebar_height: f32,
    pub splitter_hit: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::dark()
    }
}

impl Theme {
    /// A modern dark theme in the spirit of VS Code / Linear / Discord.
    pub fn dark() -> Self {
        Theme {
            name: "Dark".into(),
            dark: true,
            colors: Palette {
                background: hex("#121316"),
                surface: hex("#1a1b1f"),
                panel: hex("#16171b"),
                chrome: hex("#111215"),
                elevated: hex("#232429"),
                input: hex("#0f1013"),
                border: hex("#2a2c32"),
                border_strong: hex("#3a3d45"),
                text: hex("#e6e7ea"),
                text_muted: hex("#a0a3ab"),
                text_faint: hex("#6b6f78"),
                accent: hex("#5b8cff"),
                accent_hover: hex("#76a0ff"),
                accent_text: hex("#ffffff"),
                accent_soft: hex("#5b8cff").with_alpha(0.16),
                danger: hex("#f0616d"),
                success: hex("#3ecf8e"),
                warning: hex("#f5b94a"),
                hover: Color::WHITE.with_alpha(0.06),
                pressed: Color::WHITE.with_alpha(0.10),
                selection: hex("#5b8cff").with_alpha(0.35),
                focus_ring: hex("#5b8cff").with_alpha(0.65),
                scrollbar: Color::WHITE.with_alpha(0.16),
                scrollbar_hover: Color::WHITE.with_alpha(0.28),
                status_bar: hex("#111215"),
                status_text: hex("#a0a3ab"),
            },
            radius_sm: 4.0,
            radius: 6.0,
            radius_lg: 10.0,
            font: FontFamily::Ui,
            mono_font: FontFamily::Mono,
            font_size: 13.0,
            font_size_sm: 12.0,
            font_size_lg: 15.0,
            line_height: 1.45,
            control_height: 30.0,
            transition: 0.12,
            shadow_popover: vec![
                Shadow::new(0.0, 12.0, 32.0, 0.0, Color::BLACK.with_alpha(0.45)),
                Shadow::new(0.0, 2.0, 6.0, 0.0, Color::BLACK.with_alpha(0.3)),
            ],
            shadow_sm: vec![Shadow::new(0.0, 1.0, 2.0, 0.0, Color::BLACK.with_alpha(0.35))],
            titlebar_height: 38.0,
            splitter_hit: 5.0,
        }
    }

    /// A clean light theme in the spirit of modern web apps.
    pub fn light() -> Self {
        Theme {
            name: "Light".into(),
            dark: false,
            colors: Palette {
                background: hex("#eef0f3"),
                surface: hex("#ffffff"),
                panel: hex("#f7f8fa"),
                chrome: hex("#eceef2"),
                elevated: hex("#ffffff"),
                input: hex("#ffffff"),
                border: hex("#dfe2e8"),
                border_strong: hex("#c7ccd5"),
                text: hex("#1b1d22"),
                text_muted: hex("#5d6370"),
                text_faint: hex("#8f95a1"),
                accent: hex("#3867f5"),
                accent_hover: hex("#2d58dc"),
                accent_text: hex("#ffffff"),
                accent_soft: hex("#3867f5").with_alpha(0.12),
                danger: hex("#d93f4c"),
                success: hex("#1a9e64"),
                warning: hex("#c98a12"),
                hover: Color::BLACK.with_alpha(0.05),
                pressed: Color::BLACK.with_alpha(0.09),
                selection: hex("#3867f5").with_alpha(0.25),
                focus_ring: hex("#3867f5").with_alpha(0.55),
                scrollbar: Color::BLACK.with_alpha(0.18),
                scrollbar_hover: Color::BLACK.with_alpha(0.32),
                status_bar: hex("#e4e7ec"),
                status_text: hex("#5d6370"),
            },
            shadow_popover: vec![
                Shadow::new(0.0, 12.0, 32.0, 0.0, hex("#0b1020").with_alpha(0.16)),
                Shadow::new(0.0, 2.0, 6.0, 0.0, hex("#0b1020").with_alpha(0.08)),
            ],
            shadow_sm: vec![Shadow::new(0.0, 1.0, 2.0, 0.0, hex("#0b1020").with_alpha(0.08))],
            ..Theme::dark()
        }
    }

    /// Same as [`Theme::dark`] with a different accent color.
    pub fn with_accent(mut self, accent: Color) -> Self {
        let c = &mut self.colors;
        c.accent = accent;
        c.accent_hover = if self.dark { accent.lighten(0.15) } else { accent.darken(0.1) };
        c.accent_soft = accent.with_alpha(if self.dark { 0.16 } else { 0.12 });
        c.selection = accent.with_alpha(if self.dark { 0.35 } else { 0.25 });
        c.focus_ring = accent.with_alpha(0.6);
        c.accent_text = if accent.luminance() > 0.6 { hex("#111111") } else { Color::WHITE };
        self
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
