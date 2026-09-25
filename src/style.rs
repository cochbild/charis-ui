//! CSS-inspired styling.
//!
//! [`Style`] mirrors the subset of CSS that matters for application UIs:
//! flexbox/grid layout, box model, borders, per-corner radii, layered box
//! shadows, gradients, opacity, typography and cursors. Interaction states
//! (`:hover`, `:active`, `:focus`) are expressed with [`StylePatch`]es that are
//! smoothly interpolated when a `transition` is set.

use crate::color::{Color, Fill};

/// A CSS-like length.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    #[default]
    Auto,
    Px(f32),
    /// Percentage of the parent, 0..=100.
    Percent(f32),
}

impl From<f32> for Length {
    fn from(v: f32) -> Self {
        Length::Px(v)
    }
}

impl From<i32> for Length {
    fn from(v: i32) -> Self {
        Length::Px(v as f32)
    }
}

/// Shorthand for `Length::Percent`.
pub fn pct(v: f32) -> Length {
    Length::Percent(v)
}

/// Per-side values (padding, margin, border widths).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub const ZERO: Edges = Edges { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };
    pub const fn all(v: f32) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }
    pub const fn xy(x: f32, y: f32) -> Self {
        Self { top: y, right: x, bottom: y, left: x }
    }
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self { top, right, bottom, left }
    }
    pub fn is_uniform(&self) -> bool {
        self.top == self.right && self.right == self.bottom && self.bottom == self.left
    }
    pub fn is_zero(&self) -> bool {
        *self == Edges::ZERO
    }
}

/// Per-corner radii (top-left, top-right, bottom-right, bottom-left).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corners {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Corners {
    pub const ZERO: Corners = Corners { tl: 0.0, tr: 0.0, br: 0.0, bl: 0.0 };
    pub const fn all(r: f32) -> Self {
        Self { tl: r, tr: r, br: r, bl: r }
    }
    pub const fn top(r: f32) -> Self {
        Self { tl: r, tr: r, br: 0.0, bl: 0.0 }
    }
    pub const fn bottom(r: f32) -> Self {
        Self { tl: 0.0, tr: 0.0, br: r, bl: r }
    }
    pub fn is_zero(&self) -> bool {
        self.tl <= 0.0 && self.tr <= 0.0 && self.br <= 0.0 && self.bl <= 0.0
    }
    pub fn max(&self) -> f32 {
        self.tl.max(self.tr).max(self.br).max(self.bl)
    }
    pub fn shrink(&self, d: f32) -> Corners {
        Corners {
            tl: (self.tl - d).max(0.0),
            tr: (self.tr - d).max(0.0),
            br: (self.br - d).max(0.0),
            bl: (self.bl - d).max(0.0),
        }
    }
    pub fn grow(&self, d: f32) -> Corners {
        let g = |r: f32| if r > 0.0 { (r + d).max(0.0) } else { 0.0 };
        Corners { tl: g(self.tl), tr: g(self.tr), br: g(self.br), bl: g(self.bl) }
    }
    fn lerp(&self, o: &Corners, t: f32) -> Corners {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Corners { tl: l(self.tl, o.tl), tr: l(self.tr, o.tr), br: l(self.br, o.br), bl: l(self.bl, o.bl) }
    }
}

/// A CSS `box-shadow`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

impl Shadow {
    pub fn new(x: f32, y: f32, blur: f32, spread: f32, color: Color) -> Self {
        Self { x, y, blur, spread, color }
    }
    fn lerp(&self, o: &Shadow, t: f32) -> Shadow {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Shadow {
            x: l(self.x, o.x),
            y: l(self.y, o.y),
            blur: l(self.blur, o.blur),
            spread: l(self.spread, o.spread),
            color: self.color.lerp(o.color, t),
        }
    }
}

/// A focus ring / CSS `outline`, drawn outside the border box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outline {
    pub width: f32,
    pub offset: f32,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Flex,
    Grid,
    Block,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    End,
    Center,
    Stretch,
    Baseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Relative,
    /// Positioned relative to the parent, out of flow.
    Absolute,
    /// Positioned relative to the window (like CSS `position: fixed`).
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Font weight (100..=900), like CSS `font-weight`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Weight(pub u16);

impl Weight {
    pub const LIGHT: Weight = Weight(300);
    pub const NORMAL: Weight = Weight(400);
    pub const MEDIUM: Weight = Weight(500);
    pub const SEMIBOLD: Weight = Weight(600);
    pub const BOLD: Weight = Weight(700);
}

/// Font family selection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FontFamily {
    /// The theme's UI font (Inter when the `bundled-fonts` feature is enabled).
    Ui,
    /// A monospace font.
    Mono,
    /// Any installed or registered family, by name.
    Named(String),
}

/// Mouse cursor shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Cursor {
    #[default]
    Default,
    Pointer,
    Text,
    Grab,
    Grabbing,
    Move,
    NotAllowed,
    ResizeCol,
    ResizeRow,
    ResizeNs,
    ResizeEw,
    ResizeNwse,
    ResizeNesw,
}

/// Grid track sizing (used by `Display::Grid`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Track {
    Px(f32),
    Fr(f32),
    Auto,
    Percent(f32),
}

/// The full style of an element. All fields have CSS-like defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    // ---- layout ----
    pub display: Display,
    pub direction: Direction,
    pub wrap: bool,
    pub justify: Option<Justify>,
    pub align_items: Option<Align>,
    pub align_self: Option<Align>,
    pub align_content: Option<Justify>,
    pub gap: (f32, f32),
    pub grow: f32,
    pub shrink: f32,
    pub basis: Length,
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub max_width: Length,
    pub max_height: Length,
    pub aspect_ratio: Option<f32>,
    pub padding: Edges,
    pub margin: Edges,
    /// `margin: auto` per side (top, right, bottom, left).
    pub margin_auto: [bool; 4],
    pub position: Position,
    pub inset: [Length; 4],
    pub overflow: Overflow,
    pub grid_columns: Vec<Track>,
    pub grid_rows: Vec<Track>,
    pub grid_column_span: u16,
    pub grid_row_span: u16,

    // ---- visuals ----
    pub background: Option<Fill>,
    pub border_width: Edges,
    pub border_color: Color,
    pub radius: Corners,
    pub shadows: Vec<Shadow>,
    pub opacity: f32,
    pub outline: Option<Outline>,
    pub z_index: i32,
    /// Paint-time translation (like CSS `transform: translate`), does not affect layout.
    pub translate: (f32, f32),
    pub cursor: Option<Cursor>,
    /// Duration (seconds) used to animate between interaction states.
    pub transition: f32,
    /// Timing function for transitions (defaults to the web's standard curve).
    pub easing: crate::anim::Easing,

    // ---- text (inherited when `None`) ----
    pub color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_weight: Option<Weight>,
    pub font_family: Option<FontFamily>,
    pub line_height: Option<f32>,
    pub text_align: Option<TextAlign>,
    pub italic: Option<bool>,
    pub letter_spacing: Option<f32>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            direction: Direction::Row,
            wrap: false,
            justify: None,
            align_items: None,
            align_self: None,
            align_content: None,
            gap: (0.0, 0.0),
            grow: 0.0,
            shrink: 1.0,
            basis: Length::Auto,
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            aspect_ratio: None,
            padding: Edges::ZERO,
            margin: Edges::ZERO,
            margin_auto: [false; 4],
            position: Position::Relative,
            inset: [Length::Auto; 4],
            overflow: Overflow::Visible,
            grid_columns: Vec::new(),
            grid_rows: Vec::new(),
            grid_column_span: 1,
            grid_row_span: 1,
            background: None,
            border_width: Edges::ZERO,
            border_color: Color::TRANSPARENT,
            radius: Corners::ZERO,
            shadows: Vec::new(),
            opacity: 1.0,
            outline: None,
            z_index: 0,
            translate: (0.0, 0.0),
            cursor: None,
            transition: 0.0,
            easing: crate::anim::Easing::Standard,
            color: None,
            font_size: None,
            font_weight: None,
            font_family: None,
            line_height: None,
            text_align: None,
            italic: None,
            letter_spacing: None,
        }
    }
}

/// Overrides applied on top of a [`Style`] for an interaction state
/// (hover, active/pressed, focus, selected, disabled).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StylePatch {
    pub background: Option<Fill>,
    pub border_color: Option<Color>,
    pub color: Option<Color>,
    pub shadows: Option<Vec<Shadow>>,
    pub opacity: Option<f32>,
    pub outline: Option<Outline>,
    pub radius: Option<Corners>,
    pub translate: Option<(f32, f32)>,
    pub cursor: Option<Cursor>,
}

impl StylePatch {
    pub fn bg(mut self, f: impl Into<Fill>) -> Self {
        self.background = Some(f.into());
        self
    }
    pub fn border_color(mut self, c: Color) -> Self {
        self.border_color = Some(c);
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    pub fn shadow(mut self, s: Shadow) -> Self {
        self.shadows.get_or_insert_with(Vec::new).push(s);
        self
    }
    pub fn no_shadow(mut self) -> Self {
        self.shadows = Some(Vec::new());
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = Some(o);
        self
    }
    pub fn outline(mut self, width: f32, offset: f32, color: Color) -> Self {
        self.outline = Some(Outline { width, offset, color });
        self
    }
    pub fn rounded(mut self, r: f32) -> Self {
        self.radius = Some(Corners::all(r));
        self
    }
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.translate = Some((x, y));
        self
    }
    pub fn cursor(mut self, c: Cursor) -> Self {
        self.cursor = Some(c);
        self
    }
}

impl Style {
    /// Blend the patch into this style by factor `t` (0 = unchanged, 1 = fully patched).
    pub(crate) fn apply_patch(&mut self, p: &StylePatch, t: f32) {
        if t <= 0.0 {
            return;
        }
        let t = t.min(1.0);
        if let Some(bg) = &p.background {
            let from = self.background.clone().unwrap_or_else(|| {
                // Fade in from a transparent version of the target.
                match bg {
                    Fill::Solid(c) => Fill::Solid(c.with_alpha(0.0)),
                    other => other.fade(0.0),
                }
            });
            self.background = Some(from.lerp(bg, t));
        }
        if let Some(c) = p.border_color {
            self.border_color = self.border_color.lerp(c, t);
        }
        if let Some(c) = p.color {
            self.color = Some(self.color.map_or(c, |from| from.lerp(c, t)));
        }
        if let Some(sh) = &p.shadows {
            let n = self.shadows.len().max(sh.len());
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let a = self.shadows.get(i).copied();
                let b = sh.get(i).copied();
                match (a, b) {
                    (Some(a), Some(b)) => out.push(a.lerp(&b, t)),
                    (Some(a), None) => out.push(Shadow { color: a.color.fade(1.0 - t), ..a }),
                    (None, Some(b)) => out.push(Shadow { color: b.color.fade(t), ..b }),
                    _ => {}
                }
            }
            self.shadows = out;
        }
        if let Some(o) = p.opacity {
            self.opacity += (o - self.opacity) * t;
        }
        if let Some(o) = p.outline {
            let from = self.outline.unwrap_or(Outline { color: o.color.with_alpha(0.0), ..o });
            self.outline = Some(Outline {
                width: from.width + (o.width - from.width) * t,
                offset: from.offset + (o.offset - from.offset) * t,
                color: from.color.lerp(o.color, t),
            });
        }
        if let Some(r) = p.radius {
            self.radius = self.radius.lerp(&r, t);
        }
        if let Some((x, y)) = p.translate {
            self.translate.0 += (x - self.translate.0) * t;
            self.translate.1 += (y - self.translate.1) * t;
        }
        if let Some(c) = p.cursor {
            if t >= 0.5 {
                self.cursor = Some(c);
            }
        }
    }
}
