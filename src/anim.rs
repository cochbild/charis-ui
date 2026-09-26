//! Time-based animation helpers.

/// Easing curves.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Easing {
    Linear,
    EaseOutCubic,
    EaseInOutCubic,
    /// CSS `cubic-bezier(x1, y1, x2, y2)`.
    CubicBezier(f32, f32, f32, f32),
    /// The web's standard curve, `cubic-bezier(0.4, 0, 0.2, 1)` (Tailwind/Material).
    #[default]
    Standard,
    /// Decelerate curve for things entering the screen, `cubic-bezier(0, 0, 0.2, 1)`.
    Decelerate,
}

impl Easing {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier(*x1, *y1, *x2, *y2, t),
            Easing::Standard => cubic_bezier(0.4, 0.0, 0.2, 1.0, t),
            Easing::Decelerate => cubic_bezier(0.0, 0.0, 0.2, 1.0, t),
            Easing::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
        }
    }
}

/// Evaluate a CSS cubic-bezier timing function at progress `x`.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    if x <= 0.0 || x >= 1.0 {
        return x.clamp(0.0, 1.0);
    }
    let bez = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
    };
    let dbez = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
    };
    // Newton-Raphson, falling back to bisection.
    let mut t = x;
    for _ in 0..8 {
        let err = bez(x1, x2, t) - x;
        if err.abs() < 1e-5 {
            return bez(y1, y2, t);
        }
        let d = dbez(x1, x2, t);
        if d.abs() < 1e-6 {
            break;
        }
        t = (t - err / d).clamp(0.0, 1.0);
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    t = x;
    for _ in 0..30 {
        let v = bez(x1, x2, t);
        if (v - x).abs() < 1e-5 {
            break;
        }
        if v < x {
            lo = t;
        } else {
            hi = t;
        }
        t = (lo + hi) / 2.0;
    }
    bez(y1, y2, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_curve_shape() {
        let e = Easing::Standard;
        assert_eq!(e.apply(0.0), 0.0);
        assert_eq!(e.apply(1.0), 1.0);
        let mid = e.apply(0.5);
        assert!(mid > 0.7 && mid < 0.85, "standard(0.5) = {mid}");
        assert!((Easing::CubicBezier(0.0, 0.0, 1.0, 1.0).apply(0.3) - 0.3).abs() < 1e-3);
    }
}

/// A scalar that animates toward a target over a fixed duration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anim {
    from: f32,
    to: f32,
    start: f64,
    dur: f32,
    pub easing: Easing,
}

impl Anim {
    pub fn new(v: f32) -> Self {
        Self { from: v, to: v, start: 0.0, dur: 0.0, easing: Easing::EaseOutCubic }
    }

    pub fn value(&self, now: f64) -> f32 {
        if self.dur <= 0.0 {
            return self.to;
        }
        let t = ((now - self.start) as f32 / self.dur).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * self.easing.apply(t)
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    /// Retarget, starting from the current animated value.
    pub fn set(&mut self, target: f32, now: f64, dur: f32) {
        if (target - self.to).abs() > f32::EPSILON {
            self.from = self.value(now);
            self.to = target;
            self.start = now;
            self.dur = dur;
        }
    }

    /// Move both the current value and the target by `d` (keeps any running
    /// animation's shape; used to keep scroll content anchored).
    pub fn shift(&mut self, d: f32) {
        self.from += d;
        self.to += d;
    }

    /// Jump immediately.
    pub fn snap(&mut self, v: f32) {
        self.from = v;
        self.to = v;
        self.dur = 0.0;
    }

    pub fn is_animating(&self, now: f64) -> bool {
        self.dur > 0.0 && ((now - self.start) as f32) < self.dur
    }
}
