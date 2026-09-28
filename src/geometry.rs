//! Basic 2D geometry in logical (device-independent) pixels.

/// A point in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// Horizontal position.
    pub x: f32,
    /// Vertical position.
    pub y: f32,
}

impl Point {
    /// The origin `(0, 0)`.
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };
    /// A point at `(x, y)`.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    /// Euclidean distance to `o`.
    pub fn distance(self, o: Point) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
}

impl std::ops::Sub for Point {
    type Output = Point;
    fn sub(self, o: Point) -> Point {
        Point::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Add for Point {
    type Output = Point;
    fn add(self, o: Point) -> Point {
        Point::new(self.x + o.x, self.y + o.y)
    }
}

/// A size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Size {
    /// A size of `w` × `h`.
    pub const fn new(w: f32, h: f32) -> Self {
        Self { w, h }
    }
}

/// An axis-aligned rectangle in logical pixels.
///
/// ```
/// use charis_ui::prelude::*;
///
/// let r = Rect::new(10.0, 10.0, 100.0, 50.0);
/// assert_eq!(r.right(), 110.0);
/// assert_eq!(r.center(), Point::new(60.0, 35.0));
/// assert!(r.contains(Point::new(20.0, 20.0)));
///
/// let other = Rect::new(80.0, 40.0, 100.0, 100.0);
/// assert_eq!(r.intersect(&other), Rect::new(80.0, 40.0, 30.0, 20.0));
/// assert_eq!(r.union(&other), Rect::new(10.0, 10.0, 170.0, 130.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// A rect with top-left corner `(x, y)` and size `w` × `h`.
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    /// The right edge (`x + w`).
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    /// The bottom edge (`y + h`).
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    /// The center point.
    pub fn center(&self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    /// The top-left corner.
    pub fn origin(&self) -> Point {
        Point::new(self.x, self.y)
    }
    /// Whether `p` is inside (left/top edges inclusive, right/bottom exclusive).
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }
    /// Grow (positive) or shrink (negative) the rect on every side.
    pub fn outset(&self, d: f32) -> Rect {
        Rect::new(self.x - d, self.y - d, self.w + 2.0 * d, self.h + 2.0 * d)
    }
    /// Moved by `(dx, dy)`.
    pub fn translate(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
    /// The overlap with `o` (zero-sized if they don't overlap).
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        Rect::new(x, y, (r - x).max(0.0), (b - y).max(0.0))
    }
    /// The smallest rect containing both.
    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(x, y, self.right().max(o.right()) - x, self.bottom().max(o.bottom()) - y)
    }
    /// Whether the width or height is zero or negative.
    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
    /// Whether `o` lies entirely inside.
    pub fn contains_rect(&self, o: &Rect) -> bool {
        o.x >= self.x && o.y >= self.y && o.right() <= self.right() && o.bottom() <= self.bottom()
    }
}

/// Layout axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Axis {
    /// Children laid out left to right.
    Horizontal,
    /// Children laid out top to bottom.
    Vertical,
}

impl Axis {
    /// The coordinate of `p` along this axis.
    pub fn main(&self, p: Point) -> f32 {
        match self {
            Axis::Horizontal => p.x,
            Axis::Vertical => p.y,
        }
    }
    /// The size of `r` along this axis.
    pub fn main_len(&self, r: &Rect) -> f32 {
        match self {
            Axis::Horizontal => r.w,
            Axis::Vertical => r.h,
        }
    }
}
