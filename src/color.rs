//! Colors and fills (solid colors and CSS-like linear gradients).

/// An sRGB color with straight (non-premultiplied) alpha. Components are 0..=1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

    pub const fn rgba_f(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 1.0)
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a }
    }

    /// Parse `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`. Panics on malformed input;
    /// use [`Color::try_hex`] for fallible parsing.
    pub fn hex(s: &str) -> Self {
        Self::try_hex(s).unwrap_or_else(|| panic!("invalid hex color: {s:?}"))
    }

    pub fn try_hex(s: &str) -> Option<Self> {
        let s = s.trim_start_matches('#');
        let v = |i: usize, n: usize| u8::from_str_radix(&s[i..i + n], 16).ok();
        let (r, g, b, a) = match s.len() {
            3 | 4 => {
                let d = |i| v(i, 1).map(|x| x * 17);
                (d(0)?, d(1)?, d(2)?, if s.len() == 4 { d(3)? } else { 255 })
            }
            6 | 8 => (v(0, 2)?, v(2, 2)?, v(4, 2)?, if s.len() == 8 { v(6, 2)? } else { 255 }),
            _ => return None,
        };
        Some(Self::rgba(r, g, b, a as f32 / 255.0))
    }

    /// Build from hue (degrees), saturation and lightness (0..=1).
    pub fn hsl(h: f32, s: f32, l: f32) -> Self {
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let hp = (h.rem_euclid(360.0)) / 60.0;
        let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
        let (r, g, b) = match hp as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = l - c / 2.0;
        Self { r: r + m, g: g + m, b: b + m, a: 1.0 }
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// Multiply the alpha channel.
    pub fn fade(self, f: f32) -> Self {
        Self { a: self.a * f, ..self }
    }

    pub fn lerp(self, o: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let l = |a: f32, b: f32| a + (b - a) * t;
        Color { r: l(self.r, o.r), g: l(self.g, o.g), b: l(self.b, o.b), a: l(self.a, o.a) }
    }

    /// Mix toward white by `amount` (0..=1).
    pub fn lighten(self, amount: f32) -> Color {
        let a = self.a;
        self.lerp(Color::WHITE, amount).with_alpha(a)
    }

    /// Mix toward black by `amount` (0..=1).
    pub fn darken(self, amount: f32) -> Color {
        let a = self.a;
        self.lerp(Color::BLACK, amount).with_alpha(a)
    }

    /// Composite `top` over `self` (both straight alpha).
    pub fn blend(self, top: Color) -> Color {
        let a = top.a + self.a * (1.0 - top.a);
        if a <= 0.0 {
            return Color::TRANSPARENT;
        }
        let c = |b: f32, t: f32| (t * top.a + b * self.a * (1.0 - top.a)) / a;
        Color { r: c(self.r, top.r), g: c(self.g, top.g), b: c(self.b, top.b), a }
    }

    /// Relative luminance, useful for picking readable foreground colors.
    pub fn luminance(&self) -> f32 {
        0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
    }

    pub(crate) fn to_skia(self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba(
            self.r.clamp(0.0, 1.0),
            self.g.clamp(0.0, 1.0),
            self.b.clamp(0.0, 1.0),
            self.a.clamp(0.0, 1.0),
        )
        .unwrap_or(tiny_skia::Color::TRANSPARENT)
    }
}

/// Shorthand for [`Color::hex`].
pub fn hex(s: &str) -> Color {
    Color::hex(s)
}

/// Shorthand for [`Color::rgb`].
pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::rgb(r, g, b)
}

/// Shorthand for [`Color::rgba`].
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::rgba(r, g, b, a)
}

/// A background fill, similar to CSS `background`.
#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    Solid(Color),
    /// A linear gradient. `angle` is in degrees using CSS conventions
    /// (0 = bottom to top, 90 = left to right, 180 = top to bottom).
    LinearGradient {
        angle: f32,
        stops: Vec<(f32, Color)>,
    },
}

impl Fill {
    pub fn linear(angle: f32, stops: impl IntoIterator<Item = (f32, Color)>) -> Self {
        Fill::LinearGradient { angle, stops: stops.into_iter().collect() }
    }

    pub fn lerp(&self, o: &Fill, t: f32) -> Fill {
        match (self, o) {
            (Fill::Solid(a), Fill::Solid(b)) => Fill::Solid(a.lerp(*b, t)),
            _ => {
                if t < 0.5 {
                    self.clone()
                } else {
                    o.clone()
                }
            }
        }
    }

    pub fn is_transparent(&self) -> bool {
        match self {
            Fill::Solid(c) => c.a <= 0.0,
            Fill::LinearGradient { stops, .. } => stops.iter().all(|s| s.1.a <= 0.0),
        }
    }

    pub fn fade(&self, f: f32) -> Fill {
        match self {
            Fill::Solid(c) => Fill::Solid(c.fade(f)),
            Fill::LinearGradient { angle, stops } => {
                Fill::LinearGradient { angle: *angle, stops: stops.iter().map(|(p, c)| (*p, c.fade(f))).collect() }
            }
        }
    }
}

impl From<Color> for Fill {
    fn from(c: Color) -> Self {
        Fill::Solid(c)
    }
}
