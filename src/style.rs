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
    /// Sized by content / layout (CSS `auto`).
    #[default]
    Auto,
    /// Absolute size in logical px.
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
///
/// ```
/// use charis_ui::prelude::*;
///
/// let p = Edges::xy(12.0, 8.0);
/// assert_eq!(p, Edges::new(8.0, 12.0, 8.0, 12.0));
/// assert!(Edges::all(4.0).is_uniform());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edges {
    /// Top edge (logical px).
    pub top: f32,
    /// Right edge (logical px).
    pub right: f32,
    /// Bottom edge (logical px).
    pub bottom: f32,
    /// Left edge (logical px).
    pub left: f32,
}

impl Edges {
    /// All sides zero.
    pub const ZERO: Edges = Edges { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };
    /// Same value on all four sides.
    pub const fn all(v: f32) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }
    /// `x` for left/right, `y` for top/bottom.
    pub const fn xy(x: f32, y: f32) -> Self {
        Self { top: y, right: x, bottom: y, left: x }
    }
    /// Explicit sides in CSS order: top, right, bottom, left.
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self { top, right, bottom, left }
    }
    /// True if all four sides are equal.
    pub fn is_uniform(&self) -> bool {
        self.top == self.right && self.right == self.bottom && self.bottom == self.left
    }
    /// True if every side is zero.
    pub fn is_zero(&self) -> bool {
        *self == Edges::ZERO
    }
}

/// Per-corner radii (top-left, top-right, bottom-right, bottom-left).
///
/// ```
/// use charis_ui::prelude::*;
///
/// let r = Corners::top(8.0);
/// assert_eq!(r.max(), 8.0);
/// assert_eq!(r.shrink(2.0), Corners { tl: 6.0, tr: 6.0, br: 0.0, bl: 0.0 });
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corners {
    /// Top-left radius (logical px).
    pub tl: f32,
    /// Top-right radius (logical px).
    pub tr: f32,
    /// Bottom-right radius (logical px).
    pub br: f32,
    /// Bottom-left radius (logical px).
    pub bl: f32,
}

impl Corners {
    /// All corners square.
    pub const ZERO: Corners = Corners { tl: 0.0, tr: 0.0, br: 0.0, bl: 0.0 };
    /// Same radius on all four corners.
    pub const fn all(r: f32) -> Self {
        Self { tl: r, tr: r, br: r, bl: r }
    }
    /// Round only the top two corners.
    pub const fn top(r: f32) -> Self {
        Self { tl: r, tr: r, br: 0.0, bl: 0.0 }
    }
    /// Round only the bottom two corners.
    pub const fn bottom(r: f32) -> Self {
        Self { tl: 0.0, tr: 0.0, br: r, bl: r }
    }
    /// True if no corner is rounded.
    pub fn is_zero(&self) -> bool {
        self.tl <= 0.0 && self.tr <= 0.0 && self.br <= 0.0 && self.bl <= 0.0
    }
    /// The largest of the four radii.
    pub fn max(&self) -> f32 {
        self.tl.max(self.tr).max(self.br).max(self.bl)
    }
    /// Radii reduced by `d`, clamped at zero (e.g. for an inner edge inside a border).
    pub fn shrink(&self, d: f32) -> Corners {
        Corners {
            tl: (self.tl - d).max(0.0),
            tr: (self.tr - d).max(0.0),
            br: (self.br - d).max(0.0),
            bl: (self.bl - d).max(0.0),
        }
    }
    /// Rounded corners enlarged by `d` (e.g. for an outer ring); square corners stay square.
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
///
/// ```
/// use charis_ui::prelude::*;
///
/// // CSS: box-shadow: 0 4px 12px 0 rgba(0, 0, 0, 0.2)
/// let s = Shadow::new(0.0, 4.0, 12.0, 0.0, Color::BLACK.with_alpha(0.2));
/// assert_eq!(s.blur, 12.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Horizontal offset (logical px).
    pub x: f32,
    /// Vertical offset (logical px).
    pub y: f32,
    /// Blur radius (logical px).
    pub blur: f32,
    /// Spread; positive grows, negative shrinks the shadow (logical px).
    pub spread: f32,
    /// Shadow color.
    pub color: Color,
}

impl Shadow {
    /// Shadow with offset `(x, y)`, `blur`, `spread` and `color`, like CSS `box-shadow: x y blur spread color`.
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
    /// Stroke width (logical px).
    pub width: f32,
    /// Gap between the border box and the ring (logical px).
    pub offset: f32,
    /// Ring color.
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// How an element lays out its children (CSS `display`).
pub enum Display {
    /// Flexbox (`display: flex`); the default.
    #[default]
    Flex,
    /// CSS grid; see [`Style::grid_columns`] and [`Style::grid_rows`].
    Grid,
    /// Block layout (`display: block`).
    Block,
    /// Hidden and removed from layout (`display: none`).
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Flex main axis (CSS `flex-direction`).
pub enum Direction {
    /// Left to right; the default.
    #[default]
    Row,
    /// Top to bottom.
    Column,
    /// Right to left.
    RowReverse,
    /// Bottom to top.
    ColumnReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Cross-axis alignment (CSS `align-items` / `align-self`).
pub enum Align {
    /// Pack at the start (`flex-start`).
    Start,
    /// Pack at the end (`flex-end`).
    End,
    /// Center.
    Center,
    /// Stretch to fill the cross axis.
    Stretch,
    /// Align text baselines.
    Baseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Main-axis distribution (CSS `justify-content` / `align-content`).
pub enum Justify {
    /// Pack at the start (`flex-start`).
    Start,
    /// Pack at the end (`flex-end`).
    End,
    /// Center.
    Center,
    /// First and last at the edges, equal space between.
    SpaceBetween,
    /// Equal space around each item (half-size at the edges).
    SpaceAround,
    /// Equal space between items and at the edges.
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Positioning scheme (CSS `position`).
pub enum Position {
    /// In normal flow, offset by `inset` (CSS `position: relative`); the default.
    #[default]
    Relative,
    /// Positioned relative to the parent, out of flow.
    Absolute,
    /// Positioned relative to the window (like CSS `position: fixed`).
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Handling of content larger than the element (CSS `overflow`).
pub enum Overflow {
    /// Content may paint outside; the default.
    #[default]
    Visible,
    /// Content is clipped.
    Hidden,
    /// Content is clipped and scrollable.
    Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Horizontal text alignment (CSS `text-align`).
pub enum TextAlign {
    /// Left-aligned; the default.
    #[default]
    Left,
    /// Centered.
    Center,
    /// Right-aligned.
    Right,
}

/// Font weight (100..=900), like CSS `font-weight`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Weight(pub u16);

impl Weight {
    /// Light (300).
    pub const LIGHT: Weight = Weight(300);
    /// Normal / regular (400).
    pub const NORMAL: Weight = Weight(400);
    /// Medium (500).
    pub const MEDIUM: Weight = Weight(500);
    /// Semibold (600).
    pub const SEMIBOLD: Weight = Weight(600);
    /// Bold (700).
    pub const BOLD: Weight = Weight(700);
}

/// Font family selection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
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
#[non_exhaustive]
pub enum Cursor {
    /// Platform default arrow.
    #[default]
    Default,
    /// Hand, for clickable items.
    Pointer,
    /// I-beam, for text.
    Text,
    /// Open hand, for draggable items.
    Grab,
    /// Closed hand, while dragging.
    Grabbing,
    /// Four-way move arrows.
    Move,
    /// Action not allowed.
    NotAllowed,
    /// Column resize (horizontal).
    ResizeCol,
    /// Row resize (vertical).
    ResizeRow,
    /// North-south resize.
    ResizeNs,
    /// East-west resize.
    ResizeEw,
    /// Northwest-southeast diagonal resize.
    ResizeNwse,
    /// Northeast-southwest diagonal resize.
    ResizeNesw,
}

/// Grid track sizing (used by `Display::Grid`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Track {
    /// Fixed size (logical px).
    Px(f32),
    /// Fraction of the remaining space (CSS `fr`).
    Fr(f32),
    /// Sized by content.
    Auto,
    /// Percentage of the container, 0..=100.
    Percent(f32),
}

/// The full style of an element. All fields have CSS-like defaults.
///
/// ```
/// use charis_ui::*;
///
/// let s = Style { direction: Direction::Column, gap: (0.0, 8.0), padding: Edges::all(16.0), ..Style::default() };
/// assert_eq!(s.width, Length::Auto);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    // ---- layout ----
    /// Layout mode (CSS `display`).
    pub display: Display,
    /// Flex main axis (CSS `flex-direction`).
    pub direction: Direction,
    /// Wrap children onto multiple lines (CSS `flex-wrap: wrap`).
    pub wrap: bool,
    /// Main-axis distribution (CSS `justify-content`); `None` = start.
    pub justify: Option<Justify>,
    /// Cross-axis alignment of children (CSS `align-items`); `None` = stretch.
    pub align_items: Option<Align>,
    /// Overrides the parent's `align_items` for this element (CSS `align-self`).
    pub align_self: Option<Align>,
    /// Distribution of wrapped lines (CSS `align-content`).
    pub align_content: Option<Justify>,
    /// Space between children as (column gap, row gap), i.e. (horizontal, vertical) (logical px).
    pub gap: (f32, f32),
    /// Flex grow factor (CSS `flex-grow`).
    pub grow: f32,
    /// Flex shrink factor (CSS `flex-shrink`); defaults to 1.
    pub shrink: f32,
    /// Initial main-axis size (CSS `flex-basis`).
    pub basis: Length,
    /// Width (CSS `width`).
    pub width: Length,
    /// Height (CSS `height`).
    pub height: Length,
    /// Minimum width (CSS `min-width`).
    pub min_width: Length,
    /// Minimum height (CSS `min-height`).
    pub min_height: Length,
    /// Maximum width (CSS `max-width`).
    pub max_width: Length,
    /// Maximum height (CSS `max-height`).
    pub max_height: Length,
    /// Width / height ratio (CSS `aspect-ratio`).
    pub aspect_ratio: Option<f32>,
    /// Inner spacing (logical px).
    pub padding: Edges,
    /// Outer spacing (logical px).
    pub margin: Edges,
    /// `margin: auto` per side (top, right, bottom, left).
    pub margin_auto: [bool; 4],
    /// Positioning scheme (CSS `position`).
    pub position: Position,
    /// Offsets for positioned elements: top, right, bottom, left (CSS `inset`).
    pub inset: [Length; 4],
    /// Overflow handling (CSS `overflow`), applied to both axes.
    pub overflow: Overflow,
    /// Grid column tracks (CSS `grid-template-columns`); used with [`Display::Grid`].
    pub grid_columns: Vec<Track>,
    /// Grid row tracks (CSS `grid-template-rows`); used with [`Display::Grid`].
    pub grid_rows: Vec<Track>,
    /// Number of grid columns this element spans (CSS `grid-column: span n`).
    pub grid_column_span: u16,
    /// Number of grid rows this element spans (CSS `grid-row: span n`).
    pub grid_row_span: u16,

    // ---- visuals ----
    /// Background fill (solid color or gradient); `None` = transparent.
    pub background: Option<Fill>,
    /// Border width per side (logical px).
    pub border_width: Edges,
    /// Border color.
    pub border_color: Color,
    /// Corner radii (logical px, CSS `border-radius`).
    pub radius: Corners,
    /// Box shadows, painted in order (CSS `box-shadow`).
    pub shadows: Vec<Shadow>,
    /// Opacity of the element and its subtree, 0..=1.
    pub opacity: f32,
    /// Focus ring / outline drawn outside the border box.
    pub outline: Option<Outline>,
    /// Stacking order; elements with `z_index > 0` paint above normal content, higher values on top.
    pub z_index: i32,
    /// Paint-time translation (like CSS `transform: translate`), does not affect layout.
    pub translate: (f32, f32),
    /// Mouse cursor over this element; `None` = inherit / default.
    pub cursor: Option<Cursor>,
    /// Duration (seconds) used to animate between interaction states.
    pub transition: f32,
    /// Timing function for transitions (defaults to the web's standard curve).
    pub easing: crate::anim::Easing,
    /// Animate changes of the element's laid-out position and size over this
    /// many seconds (FLIP-style). 0 disables.
    pub layout_transition: f32,

    // ---- text (inherited when `None`) ----
    /// Text color.
    pub color: Option<Color>,
    /// Font size (logical px).
    pub font_size: Option<f32>,
    /// Font weight.
    pub font_weight: Option<Weight>,
    /// Font family.
    pub font_family: Option<FontFamily>,
    /// Line height as a multiple of the font size.
    pub line_height: Option<f32>,
    /// Horizontal text alignment.
    pub text_align: Option<TextAlign>,
    /// Italic text.
    pub italic: Option<bool>,
    /// Extra space between characters (logical px).
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
            layout_transition: 0.0,
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
///
/// ```
/// use charis_ui::prelude::*;
///
/// let hover = StylePatch::default().bg(Color::WHITE).shadow(Shadow::new(0.0, 2.0, 8.0, 0.0, Color::BLACK.with_alpha(0.15)));
/// assert_eq!(hover.shadows.map(|s| s.len()), Some(1));
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StylePatch {
    /// Background fill override.
    pub background: Option<Fill>,
    /// Border color override.
    pub border_color: Option<Color>,
    /// Text color override.
    pub color: Option<Color>,
    /// Replacement shadow list (`Some(vec![])` removes all shadows).
    pub shadows: Option<Vec<Shadow>>,
    /// Opacity override, 0..=1.
    pub opacity: Option<f32>,
    /// Outline / focus ring override.
    pub outline: Option<Outline>,
    /// Corner radii override.
    pub radius: Option<Corners>,
    /// Paint-time translation override (logical px).
    pub translate: Option<(f32, f32)>,
    /// Cursor override.
    pub cursor: Option<Cursor>,
}

impl StylePatch {
    /// Set the background fill.
    pub fn bg(mut self, f: impl Into<Fill>) -> Self {
        self.background = Some(f.into());
        self
    }
    /// Set the border color.
    pub fn border_color(mut self, c: Color) -> Self {
        self.border_color = Some(c);
        self
    }
    /// Set the text color.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// Add a shadow (appended to any shadows already in the patch).
    pub fn shadow(mut self, s: Shadow) -> Self {
        self.shadows.get_or_insert_with(Vec::new).push(s);
        self
    }
    /// Remove all shadows.
    pub fn no_shadow(mut self) -> Self {
        self.shadows = Some(Vec::new());
        self
    }
    /// Set the opacity, 0..=1.
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = Some(o);
        self
    }
    /// Set an outline: `width` and `offset` from the border box (logical px) and `color`.
    pub fn outline(mut self, width: f32, offset: f32, color: Color) -> Self {
        self.outline = Some(Outline { width, offset, color });
        self
    }
    /// Set the same radius on all corners (logical px).
    pub fn rounded(mut self, r: f32) -> Self {
        self.radius = Some(Corners::all(r));
        self
    }
    /// Offset the element by `(x, y)` at paint time (logical px).
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.translate = Some((x, y));
        self
    }
    /// Set the cursor.
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
