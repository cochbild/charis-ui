//! Damage tracking for the CPU renderer: which part of the window changed
//! since the last frame.
//!
//! Every drawing command of a [`Scene`] gets a fingerprint (its content
//! plus the clip and opacity it's drawn under) and conservative bounds.
//! Commands are matched against last frame's as a multiset; the damage is
//! the union of the bounds of commands that appeared, disappeared, or were
//! drawn in a different order (a z-order swap). Only that area is
//! rasterized again, on top of last frame's pixels.

use std::hash::{Hash, Hasher};

use crate::color::{Color, Fill};
use crate::fxhash::{FxHashMap, FxHasher};
use crate::geometry::Rect;
use crate::scene::{Cmd, Scene};
use crate::style::Corners;

/// A drawing command's fingerprint and bounds (logical px, clipped).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Item {
    hash: u64,
    pub bounds: Rect,
}

/// What the CPU renderer redrew in the last frame
/// ([`Runtime::last_damage`](crate::Runtime::last_damage)).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Damage {
    /// Everything (first frame, resize, too much changed).
    Full,
    /// Nothing changed.
    None,
    /// This area (logical px).
    Area(Rect),
}

fn f(h: &mut FxHasher, v: f32) {
    v.to_bits().hash(h);
}

fn rect(h: &mut FxHasher, r: Rect) {
    for v in [r.x, r.y, r.w, r.h] {
        f(h, v);
    }
}

fn corners(h: &mut FxHasher, c: Corners) {
    for v in [c.tl, c.tr, c.br, c.bl] {
        f(h, v);
    }
}

fn color(h: &mut FxHasher, c: Color) {
    for v in [c.r, c.g, c.b, c.a] {
        f(h, v);
    }
}

fn fill(h: &mut FxHasher, x: &Fill) {
    match x {
        Fill::Solid(c) => {
            0u8.hash(h);
            color(h, *c);
        }
        Fill::LinearGradient { angle, stops } => {
            1u8.hash(h);
            f(h, *angle);
            for (p, c) in stops.iter() {
                f(h, *p);
                color(h, *c);
            }
        }
    }
}

/// Fingerprints and bounds of `scene`'s drawing commands, one per command
/// (`None` for clip and layer commands).
pub(crate) fn items(scene: &Scene) -> Vec<Option<Item>> {
    let s = scene.scale;
    // The clip and opacity state, as a running hash and a clip rect.
    let mut clips: Vec<(u64, Rect)> = Vec::new();
    let mut layers: Vec<u64> = Vec::new();
    let state = |clips: &[(u64, Rect)], layers: &[u64]| {
        let mut h = FxHasher::default();
        clips.last().map(|c| c.0).hash(&mut h);
        layers.last().hash(&mut h);
        h.finish()
    };
    let clip_rect = |clips: &[(u64, Rect)]| clips.last().map(|c| c.1);
    let mut out = Vec::with_capacity(scene.cmds.len());
    for cmd in &scene.cmds {
        let mut h = FxHasher::default();
        let bounds: Rect = match cmd {
            Cmd::Fill { rect: r, radius, fill: x } => {
                0u8.hash(&mut h);
                rect(&mut h, *r);
                corners(&mut h, *radius);
                fill(&mut h, x);
                *r
            }
            Cmd::Border { rect: r, radius, widths, color: c } => {
                1u8.hash(&mut h);
                rect(&mut h, *r);
                corners(&mut h, *radius);
                for v in [widths.top, widths.right, widths.bottom, widths.left] {
                    f(&mut h, v);
                }
                color(&mut h, *c);
                *r
            }
            Cmd::Stroke { rect: r, radius, width, color: c } => {
                2u8.hash(&mut h);
                rect(&mut h, *r);
                corners(&mut h, *radius);
                f(&mut h, *width);
                color(&mut h, *c);
                r.outset(*width)
            }
            Cmd::Shadow { rect: r, radius, shadow } => {
                3u8.hash(&mut h);
                rect(&mut h, *r);
                corners(&mut h, *radius);
                for v in [shadow.x, shadow.y, shadow.blur, shadow.spread] {
                    f(&mut h, v);
                }
                color(&mut h, shadow.color);
                let reach = shadow.blur * 1.5 + shadow.spread.max(0.0) + 2.0;
                r.translate(shadow.x, shadow.y).outset(reach).union(r)
            }
            Cmd::Path { path, transform, color: c, stroke, bounds } => {
                4u8.hash(&mut h);
                (std::rc::Rc::as_ptr(path) as usize).hash(&mut h);
                for v in [transform.sx, transform.kx, transform.ky, transform.sy, transform.tx, transform.ty] {
                    f(&mut h, v);
                }
                color(&mut h, *c);
                stroke.map(f32::to_bits).hash(&mut h);
                rect(&mut h, *bounds);
                bounds.outset(2.0)
            }
            Cmd::Glyphs { glyphs, color: c } => {
                5u8.hash(&mut h);
                color(&mut h, *c);
                let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for g in glyphs {
                    g.key.hash(&mut h);
                    (g.x, g.y).hash(&mut h);
                    // Conservative glyph box around the pen position.
                    let fs = f32::from_bits(g.key.font_size_bits);
                    x0 = x0.min(g.x as f32 - fs);
                    x1 = x1.max(g.x as f32 + 2.0 * fs);
                    y0 = y0.min(g.y as f32 - 1.4 * fs);
                    y1 = y1.max(g.y as f32 + 0.7 * fs);
                }
                if glyphs.is_empty() {
                    Rect::default()
                } else {
                    Rect::new(x0 / s, y0 / s, (x1 - x0) / s, (y1 - y0) / s)
                }
            }
            Cmd::Image { source, crop, dest, radius, tint } => {
                6u8.hash(&mut h);
                crate::image::key(source, *crop, (dest.w * s) as u32, (dest.h * s) as u32, *tint).hash(&mut h);
                rect(&mut h, *dest);
                corners(&mut h, *radius);
                *dest
            }
            Cmd::PushClip { rect: r, radius } => {
                let mut ch = FxHasher::default();
                clips.last().map(|c| c.0).hash(&mut ch);
                rect(&mut ch, *r);
                corners(&mut ch, *radius);
                let cr = match clip_rect(&clips) {
                    Some(c) => c.intersect(r),
                    None => *r,
                };
                clips.push((ch.finish(), cr));
                out.push(None);
                continue;
            }
            Cmd::PopClip => {
                clips.pop();
                out.push(None);
                continue;
            }
            Cmd::SetClips(list) => {
                clips.clear();
                for (r, radius) in list {
                    let mut ch = FxHasher::default();
                    clips.last().map(|c| c.0).hash(&mut ch);
                    rect(&mut ch, *r);
                    corners(&mut ch, *radius);
                    let cr = match clip_rect(&clips) {
                        Some(c) => c.intersect(r),
                        None => *r,
                    };
                    clips.push((ch.finish(), cr));
                }
                out.push(None);
                continue;
            }
            Cmd::PushLayer { opacity, bounds } => {
                let mut lh = FxHasher::default();
                layers.last().hash(&mut lh);
                f(&mut lh, *opacity);
                bounds.map(|b| [b.x, b.y, b.w, b.h].map(f32::to_bits)).hash(&mut lh);
                layers.push(lh.finish());
                out.push(None);
                continue;
            }
            Cmd::PopLayer => {
                layers.pop();
                out.push(None);
                continue;
            }
        };
        state(&clips, &layers).hash(&mut h);
        let bounds = match clip_rect(&clips) {
            Some(c) => c.intersect(&bounds),
            None => bounds,
        };
        out.push(Some(Item { hash: h.finish(), bounds }));
    }
    out
}

/// What changed between last frame's items and this frame's (logical px,
/// within a `w`×`h` window).
pub(crate) fn diff(old: &[Option<Item>], new: &[Option<Item>], w: f32, h: f32) -> Damage {
    let mut by_hash: FxHashMap<u64, std::collections::VecDeque<(usize, Rect)>> = FxHashMap::default();
    for (i, it) in old.iter().flatten().enumerate() {
        by_hash.entry(it.hash).or_default().push_back((i, it.bounds));
    }
    let mut area: Option<Rect> = None;
    let mut add = |r: Rect| {
        if r.w > 0.0 && r.h > 0.0 {
            area = Some(match area {
                Some(a) => a.union(&r),
                None => r,
            });
        }
    };
    let mut last_old: Option<usize> = None;
    for it in new.iter().flatten() {
        match by_hash.get_mut(&it.hash).and_then(|q| q.pop_front()) {
            Some((i, _)) => {
                // Drawn before something it used to be drawn after.
                if last_old.is_some_and(|l| i < l) {
                    add(it.bounds);
                }
                last_old = Some(last_old.map_or(i, |l| l.max(i)));
            }
            None => add(it.bounds),
        }
    }
    for q in by_hash.values() {
        for (_, r) in q {
            add(*r);
        }
    }
    match area {
        None => Damage::None,
        Some(a) => {
            let a = a.intersect(&Rect::new(0.0, 0.0, w, h));
            if a.is_empty() {
                Damage::None
            } else if a.w * a.h > w * h * 0.6 {
                Damage::Full
            } else {
                Damage::Area(a)
            }
        }
    }
}
