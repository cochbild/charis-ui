//! Time-based animation helpers.

/// Easing curves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    Linear,
    #[default]
    EaseOutCubic,
    EaseInOutCubic,
}

impl Easing {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
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
