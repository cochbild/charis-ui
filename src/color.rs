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

    /// Build from OKLCH: perceptual lightness `l` (0..=1), chroma `c`
    /// (0..≈0.37) and hue `h` in degrees — the color space used by modern CSS
    /// design systems (Tailwind v4, shadcn). Out-of-gamut colors are brought
    /// into sRGB by reducing chroma, which preserves lightness and hue.
    pub fn oklch(l: f32, c: f32, h: f32) -> Self {
        let to_rgb = |c: f32| {
            let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
            oklab_to_linear_srgb(l, a, b)
        };
        let in_gamut = |(r, g, b): (f32, f32, f32)| {
            let e = 1e-4;
            (-e..=1.0 + e).contains(&r) && (-e..=1.0 + e).contains(&g) && (-e..=1.0 + e).contains(&b)
        };
        let mut rgb = to_rgb(c);
        if !in_gamut(rgb) {
            let (mut lo, mut hi) = (0.0f32, c);
            for _ in 0..24 {
                let mid = (lo + hi) / 2.0;
                if in_gamut(to_rgb(mid)) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            rgb = to_rgb(lo);
        }
        let enc = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            if x <= 0.003_130_8 {
                12.92 * x
            } else {
                1.055 * x.powf(1.0 / 2.4) - 0.055
            }
        };
        Color { r: enc(rgb.0), g: enc(rgb.1), b: enc(rgb.2), a: 1.0 }
    }

    /// Convert to OKLCH `(lightness, chroma, hue_degrees)`.
    pub fn to_oklch(&self) -> (f32, f32, f32) {
        let lin = |x: f32| if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
        let (r, g, b) = (lin(self.r), lin(self.g), lin(self.b));
        let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
        let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
        let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
        let ll = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
        let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
        let bb = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
        let c = (a * a + bb * bb).sqrt();
        let h = bb.atan2(a).to_degrees().rem_euclid(360.0);
        (ll, c, h)
    }

    /// Relative luminance, useful for picking readable foreground colors.
    pub fn luminance(&self) -> f32 {
        0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
    }

    /// WCAG 2 contrast ratio against another opaque color (1.0 ..= 21.0).
    pub fn contrast(&self, other: Color) -> f32 {
        let lum = |c: &Color| {
            let f = |x: f32| if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
            0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
        };
        let (x, y) = (lum(self), lum(&other));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// The same hue, lightened or darkened (in OKLCH, keeping chroma where
    /// possible) until it has at least `ratio` contrast against `bg`.
    /// Opaque colors only; returns `self` if it already qualifies.
    pub fn with_contrast(self, bg: Color, ratio: f32) -> Color {
        if self.contrast(bg) >= ratio {
            return self;
        }
        let (l, c, h) = self.to_oklch();
        let up = bg.contrast(Color::WHITE) > bg.contrast(Color::BLACK);
        let mut best = if up { Color::WHITE } else { Color::BLACK };
        // Binary search the lightness closest to the original that passes.
        let (mut lo, mut hi) = if up { (l, 1.0) } else { (0.0, l) };
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            let cand = Color::oklch(mid, c, h).with_alpha(self.a);
            if cand.contrast(bg) >= ratio {
                best = cand;
                if up {
                    hi = mid;
                } else {
                    lo = mid;
                }
            } else if up {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        best
    }

    /// Whichever of `a` / `b` reads better on top of this color.
    pub fn most_readable(&self, a: Color, b: Color) -> Color {
        if self.contrast(a) >= self.contrast(b) {
            a
        } else {
            b
        }
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

fn oklab_to_linear_srgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    (
        4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3,
        -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3,
        -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3,
    )
}

/// Shorthand for [`Color::oklch`].
pub fn oklch(l: f32, c: f32, h: f32) -> Color {
    Color::oklch(l, c, h)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklch_roundtrip() {
        for hexs in ["#5b8cff", "#f0616d", "#3ecf8e", "#121316", "#ffffff", "#808080"] {
            let c = Color::hex(hexs);
            let (l, ch, h) = c.to_oklch();
            let back = Color::oklch(l, ch, h);
            for (x, y) in [(c.r, back.r), (c.g, back.g), (c.b, back.b)] {
                assert!((x - y).abs() < 0.004, "{hexs}: {c:?} vs {back:?}");
            }
        }
    }

    #[test]
    fn oklch_known_value() {
        // Tailwind v4 zinc-900 = oklch(21% 0.006 285.885) ≈ #18181b
        let c = Color::oklch(0.21, 0.006, 285.885);
        let e = Color::hex("#18181b");
        assert!((c.r - e.r).abs() < 0.01 && (c.g - e.g).abs() < 0.01 && (c.b - e.b).abs() < 0.01, "{c:?}");
        // Out of gamut chroma is clamped, not garbage.
        let v = Color::oklch(0.7, 0.5, 150.0);
        assert!(v.r >= 0.0 && v.g <= 1.0);
    }
}
