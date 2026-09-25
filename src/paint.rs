//! The 2D painter: anti-aliased rounded rectangles, borders, gradients,
//! blurred box shadows, clipping, opacity layers, icons and text.

use std::collections::HashMap;

use tiny_skia::{FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform};

use crate::color::{Color, Fill};
use crate::geometry::{Point, Rect};
use crate::icons::Icon;
use crate::style::{Corners, Edges, Shadow};
use crate::text::{TextStyle, TextSystem};

#[derive(Clone, Copy, PartialEq)]
struct Clip {
    rect: Rect,
    radius: Corners,
}

/// Build a rounded-rectangle path (coordinates in physical pixels).
pub(crate) fn rrect_path(r: Rect, c: Corners) -> Option<Path> {
    if r.w <= 0.0 || r.h <= 0.0 {
        return None;
    }
    let max = (r.w.min(r.h)) / 2.0;
    let c = Corners { tl: c.tl.min(max), tr: c.tr.min(max), br: c.br.min(max), bl: c.bl.min(max) };
    if c.is_zero() {
        return Some(PathBuilder::from_rect(tiny_skia::Rect::from_xywh(r.x, r.y, r.w, r.h)?));
    }
    // Cubic approximation constant for quarter circles.
    const K: f32 = 0.552_284_8;
    let mut pb = PathBuilder::new();
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    pb.move_to(x0 + c.tl, y0);
    pb.line_to(x1 - c.tr, y0);
    if c.tr > 0.0 {
        pb.cubic_to(x1 - c.tr + c.tr * K, y0, x1, y0 + c.tr - c.tr * K, x1, y0 + c.tr);
    }
    pb.line_to(x1, y1 - c.br);
    if c.br > 0.0 {
        pb.cubic_to(x1, y1 - c.br + c.br * K, x1 - c.br + c.br * K, y1, x1 - c.br, y1);
    }
    pb.line_to(x0 + c.bl, y1);
    if c.bl > 0.0 {
        pb.cubic_to(x0 + c.bl - c.bl * K, y1, x0, y1 - c.bl + c.bl * K, x0, y1 - c.bl);
    }
    pb.line_to(x0, y0 + c.tl);
    if c.tl > 0.0 {
        pb.cubic_to(x0, y0 + c.tl - c.tl * K, x0 + c.tl - c.tl * K, y0, x0 + c.tl, y0);
    }
    pb.close();
    pb.finish()
}

fn scale_rect(r: Rect, s: f32) -> Rect {
    Rect::new(r.x * s, r.y * s, r.w * s, r.h * s)
}

fn scale_corners(c: Corners, s: f32) -> Corners {
    Corners { tl: c.tl * s, tr: c.tr * s, br: c.br * s, bl: c.bl * s }
}

#[derive(Hash, PartialEq, Eq)]
struct ShadowKey {
    w: u32,
    h: u32,
    radius: [u32; 4],
    blur: u32,
}

struct ShadowMask {
    w: usize,
    h: usize,
    pad: f32,
    alpha: Vec<u8>,
}

/// Caches that live across frames.
#[derive(Default)]
pub(crate) struct PaintCache {
    shadows: HashMap<ShadowKey, (ShadowMask, u64)>,
    frame: u64,
}

impl PaintCache {
    pub(crate) fn begin_frame(&mut self) {
        self.frame += 1;
        if self.frame.is_multiple_of(120) {
            let f = self.frame;
            self.shadows.retain(|_, v| f - v.1 < 240);
        }
    }
}

/// A drawing surface. Coordinates are logical pixels; the canvas applies the
/// display scale factor. Custom widgets draw with it via [`canvas`](crate::canvas).
pub struct Canvas<'a> {
    pub(crate) pixmap: Pixmap,
    /// Physical position of `pixmap`'s top-left corner in window space
    /// (non-zero while drawing into a bounded layer).
    origin: (f32, f32),
    layers: Vec<Layer>,
    pub(crate) scale: f32,
    clips: Vec<Clip>,
    pub(crate) text: &'a mut TextSystem,
    cache: &'a mut PaintCache,
}

struct Layer {
    prev: Pixmap,
    prev_origin: (f32, f32),
    opacity: f32,
}

impl<'a> Canvas<'a> {
    pub(crate) fn new(pixmap: Pixmap, scale: f32, text: &'a mut TextSystem, cache: &'a mut PaintCache) -> Self {
        Self { pixmap, origin: (0.0, 0.0), layers: Vec::new(), scale, clips: Vec::new(), text, cache }
    }

    pub(crate) fn finish(self) -> Pixmap {
        self.pixmap
    }

    /// Display scale factor (physical pixels per logical pixel).
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Logical rect → physical rect in the current target's coordinates.
    fn phys(&self, r: Rect) -> Rect {
        let s = self.scale;
        Rect::new(r.x * s - self.origin.0, r.y * s - self.origin.1, r.w * s, r.h * s)
    }

    fn to_target(&self) -> Transform {
        Transform::from_translate(-self.origin.0, -self.origin.1)
    }

    // ------------------------------------------------------------- clipping

    /// Restrict drawing to `rect` (intersected with the current clip).
    pub fn push_clip(&mut self, rect: Rect, radius: Corners) {
        let rect = match self.clips.last() {
            Some(c) => c.rect.intersect(&rect),
            None => rect,
        };
        self.clips.push(Clip { rect, radius });
    }

    pub fn pop_clip(&mut self) {
        self.clips.pop();
    }

    pub(crate) fn clip_rect(&self) -> Option<Rect> {
        self.clips.last().map(|c| c.rect)
    }

    /// Temporarily disable clipping (used for overlays/portals).
    pub(crate) fn take_clips(&mut self) -> Vec<Clip2> {
        std::mem::take(&mut self.clips).into_iter().map(|c| Clip2(c.rect, c.radius)).collect()
    }

    pub(crate) fn restore_clips(&mut self, c: Vec<Clip2>) {
        self.clips = c.into_iter().map(|c| Clip { rect: c.0, radius: c.1 }).collect();
    }

    /// Run a path-drawing closure with clipping applied.
    ///
    /// The closure receives the target pixmap, a transform mapping window-space
    /// physical coordinates into it, and an optional mask. Items entirely inside
    /// the clip draw directly; partially clipped items are drawn into a small
    /// scratch pixmap covering only their visible region, so no full-window
    /// masks are ever allocated.
    fn clipped(&mut self, bounds: Rect, f: impl FnOnce(&mut Pixmap, Transform, Option<&Mask>)) {
        let base = self.to_target();
        let Some(clip) = self.clips.last().copied() else {
            f(&mut self.pixmap, base, None);
            return;
        };
        let vis = clip.rect.intersect(&bounds);
        if vis.is_empty() {
            return;
        }
        let inner = if clip.radius.is_zero() {
            clip.rect
        } else {
            let m = clip.radius.max();
            Rect::new(clip.rect.x, clip.rect.y + m, clip.rect.w, (clip.rect.h - 2.0 * m).max(0.0))
        };
        if inner.contains_rect(&bounds) {
            f(&mut self.pixmap, base, None);
            return;
        }
        let pv = self.phys(vis);
        let (pw, ph) = (self.pixmap.width() as i32, self.pixmap.height() as i32);
        let x0 = (pv.x.floor() as i32).clamp(0, pw);
        let y0 = (pv.y.floor() as i32).clamp(0, ph);
        let x1 = (pv.right().ceil() as i32).clamp(0, pw);
        let y1 = (pv.bottom().ceil() as i32).clamp(0, ph);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
        let Some(mut tmp) = Pixmap::new(w, h) else { return };
        let t = base.post_translate(-(x0 as f32), -(y0 as f32));
        // Mask for the (possibly rounded, possibly fractional) clip shape, sized to the scratch area.
        let mask = {
            let s = self.scale;
            let cr = Rect::new(clip.rect.x * s, clip.rect.y * s, clip.rect.w * s, clip.rect.h * s);
            let needs = !clip.radius.is_zero()
                || cr.x.fract() != 0.0
                || cr.y.fract() != 0.0
                || cr.right().fract() != 0.0
                || cr.bottom().fract() != 0.0;
            if needs {
                let mut m = Mask::new(w, h).expect("mask");
                if let Some(p) = rrect_path(cr, scale_corners(clip.radius, s)) {
                    m.fill_path(&p, FillRule::Winding, true, t);
                }
                Some(m)
            } else {
                None
            }
        };
        f(&mut tmp, t, mask.as_ref());
        self.pixmap.draw_pixmap(x0, y0, tmp.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }

    // --------------------------------------------------------------- layers

    /// Begin a group composited with `opacity` when popped. `bounds` (logical)
    /// limits the layer's size; `None` covers the whole window.
    pub fn push_layer(&mut self, opacity: f32, bounds: Option<Rect>) {
        let (ox, oy, w, h) = match bounds {
            Some(b) => {
                let p = self.phys(b);
                let (pw, ph) = (self.pixmap.width() as f32, self.pixmap.height() as f32);
                let x0 = p.x.floor().clamp(0.0, pw);
                let y0 = p.y.floor().clamp(0.0, ph);
                let x1 = p.right().ceil().clamp(0.0, pw);
                let y1 = p.bottom().ceil().clamp(0.0, ph);
                (x0 + self.origin.0, y0 + self.origin.1, (x1 - x0).max(1.0) as u32, (y1 - y0).max(1.0) as u32)
            }
            None => (self.origin.0, self.origin.1, self.pixmap.width(), self.pixmap.height()),
        };
        let fresh = Pixmap::new(w, h).expect("layer");
        let prev = std::mem::replace(&mut self.pixmap, fresh);
        let prev_origin = std::mem::replace(&mut self.origin, (ox, oy));
        self.layers.push(Layer { prev, prev_origin, opacity });
    }

    pub fn pop_layer(&mut self) {
        if let Some(Layer { mut prev, prev_origin, opacity }) = self.layers.pop() {
            let paint = PixmapPaint { opacity, ..Default::default() };
            let x = (self.origin.0 - prev_origin.0) as i32;
            let y = (self.origin.1 - prev_origin.1) as i32;
            prev.draw_pixmap(x, y, self.pixmap.as_ref(), &paint, Transform::identity(), None);
            self.pixmap = prev;
            self.origin = prev_origin;
        }
    }

    // ------------------------------------------------------------ primitives

    fn paint_for(&self, fill: &Fill, r: Rect) -> Paint<'static> {
        let mut paint = Paint { anti_alias: true, ..Default::default() };
        match fill {
            Fill::Solid(c) => paint.set_color(c.to_skia()),
            Fill::LinearGradient { angle, stops } => {
                // CSS angle: 0deg points up, 90deg points right.
                let a = angle.to_radians();
                let (dx, dy) = (a.sin(), -a.cos());
                let half = (r.w * dx.abs() + r.h * dy.abs()) / 2.0;
                let c = r.center();
                let p0 = tiny_skia::Point::from_xy(c.x - dx * half, c.y - dy * half);
                let p1 = tiny_skia::Point::from_xy(c.x + dx * half, c.y + dy * half);
                let stops: Vec<_> = stops.iter().map(|(p, c)| tiny_skia::GradientStop::new(*p, c.to_skia())).collect();
                match tiny_skia::LinearGradient::new(p0, p1, stops, tiny_skia::SpreadMode::Pad, Transform::identity()) {
                    Some(sh) => paint.shader = sh,
                    None => {
                        if let Fill::LinearGradient { stops, .. } = fill {
                            if let Some((_, c)) = stops.first() {
                                paint.set_color(c.to_skia());
                            }
                        }
                    }
                }
            }
        }
        paint
    }

    /// Fill a (rounded) rectangle.
    pub fn fill_rrect(&mut self, rect: Rect, radius: Corners, fill: &Fill) {
        if fill.is_transparent() || rect.is_empty() {
            return;
        }
        let s = self.scale;
        // Fast path: axis-aligned rect with rectangular clip → intersect directly.
        if radius.is_zero() {
            let r = match self.clips.last() {
                Some(c) if c.radius.is_zero() => c.rect.intersect(&rect),
                Some(_) => Rect::default(),
                None => rect,
            };
            if !r.is_empty() || self.clips.is_empty() {
                let paint = self.paint_for(fill, scale_rect(rect, s));
                let pr = self.phys(r);
                if let Some(sr) = tiny_skia::Rect::from_xywh(pr.x, pr.y, pr.w, pr.h) {
                    // Gradients are defined in window space; shift the shader into target space.
                    let mut paint = paint;
                    paint.shader.transform(self.to_target());
                    self.pixmap.fill_rect(sr, &paint, Transform::identity(), None);
                }
                return;
            }
            if self.clips.last().is_some_and(|c| c.radius.is_zero()) {
                return;
            }
        }
        let Some(path) = rrect_path(scale_rect(rect, s), scale_corners(radius, s)) else { return };
        let paint = self.paint_for(fill, scale_rect(rect, s));
        self.clipped(rect, |pm, t, m| pm.fill_path(&path, &paint, FillRule::Winding, t, m));
    }

    /// Convenience: fill a rect with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.fill_rrect(rect, Corners::ZERO, &Fill::Solid(color));
    }

    /// Convenience: fill a rounded rect with a solid color.
    pub fn fill_rounded(&mut self, rect: Rect, radius: f32, color: Color) {
        self.fill_rrect(rect, Corners::all(radius), &Fill::Solid(color));
    }

    /// Draw a border inside `rect` with per-side widths.
    pub fn border(&mut self, rect: Rect, radius: Corners, widths: Edges, color: Color) {
        if widths.is_zero() || color.a <= 0.0 || rect.is_empty() {
            return;
        }
        let s = self.scale;
        // Straight rect: draw each side as a rect (crisp lines).
        if radius.is_zero() {
            let sides = [
                Rect::new(rect.x, rect.y, rect.w, widths.top),
                Rect::new(rect.x, rect.bottom() - widths.bottom, rect.w, widths.bottom),
                Rect::new(rect.x, rect.y + widths.top, widths.left, rect.h - widths.top - widths.bottom),
                Rect::new(
                    rect.right() - widths.right,
                    rect.y + widths.top,
                    widths.right,
                    rect.h - widths.top - widths.bottom,
                ),
            ];
            for side in sides {
                if !side.is_empty() {
                    self.fill_rrect(side, Corners::ZERO, &Fill::Solid(color));
                }
            }
            return;
        }
        let outer = rrect_path(scale_rect(rect, s), scale_corners(radius, s));
        let inner_rect = Rect::new(
            rect.x + widths.left,
            rect.y + widths.top,
            rect.w - widths.left - widths.right,
            rect.h - widths.top - widths.bottom,
        );
        let inner_radius = radius.shrink(widths.top.max(widths.left));
        let mut pb = PathBuilder::new();
        if let Some(o) = outer {
            pb.push_path(&o);
        }
        if let Some(i) = rrect_path(scale_rect(inner_rect, s), scale_corners(inner_radius, s)) {
            pb.push_path(&i);
        }
        let Some(path) = pb.finish() else { return };
        let mut paint = Paint { anti_alias: true, ..Default::default() };
        paint.set_color(color.to_skia());
        self.clipped(rect, |pm, t, m| pm.fill_path(&path, &paint, FillRule::EvenOdd, t, m));
    }

    /// Stroke a rounded rectangle outline centered on `rect`'s edge.
    pub fn stroke_rrect(&mut self, rect: Rect, radius: Corners, width: f32, color: Color) {
        let s = self.scale;
        let Some(path) = rrect_path(scale_rect(rect, s), scale_corners(radius, s)) else { return };
        let mut paint = Paint { anti_alias: true, ..Default::default() };
        paint.set_color(color.to_skia());
        let stroke = Stroke { width: width * s, ..Default::default() };
        self.clipped(rect.outset(width), |pm, t, m| pm.stroke_path(&path, &paint, &stroke, t, m));
    }

    /// Draw a straight line.
    pub fn line(&mut self, a: Point, b: Point, width: f32, color: Color) {
        let s = self.scale;
        let mut pb = PathBuilder::new();
        pb.move_to(a.x * s, a.y * s);
        pb.line_to(b.x * s, b.y * s);
        let Some(path) = pb.finish() else { return };
        let mut paint = Paint { anti_alias: true, ..Default::default() };
        paint.set_color(color.to_skia());
        let stroke = Stroke { width: width * s, line_cap: LineCap::Round, ..Default::default() };
        let bounds = Rect::new(a.x.min(b.x), a.y.min(b.y), (a.x - b.x).abs(), (a.y - b.y).abs()).outset(width);
        self.clipped(bounds, |pm, t, m| pm.stroke_path(&path, &paint, &stroke, t, m));
    }

    /// Fill a circle.
    pub fn fill_circle(&mut self, center: Point, radius: f32, color: Color) {
        let r = Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0);
        self.fill_rrect(r, Corners::all(radius), &Fill::Solid(color));
    }

    /// Stroke or fill arbitrary SVG path data, mapped from a `view` box onto `rect`.
    pub fn svg_path(&mut self, d: &str, view: f32, rect: Rect, color: Color, stroke_width: Option<f32>) {
        if let Some(p) = crate::icons::parse_svg_path(d) {
            self.draw_path(&p, view, rect, color, stroke_width);
        }
    }

    fn draw_path(&mut self, p: &Path, view: f32, rect: Rect, color: Color, stroke_width: Option<f32>) {
        let s = self.scale;
        let k = rect.w.min(rect.h) / view * s;
        let tx = rect.x * s + (rect.w * s - view * k) / 2.0;
        let ty = rect.y * s + (rect.h * s - view * k) / 2.0;
        let local = Transform::from_row(k, 0.0, 0.0, k, tx, ty);
        let mut paint = Paint { anti_alias: true, ..Default::default() };
        paint.set_color(color.to_skia());
        self.clipped(rect.outset(2.0), |pm, t, m| {
            let t = local.post_concat(t);
            match stroke_width {
                Some(w) => {
                    let stroke =
                        Stroke { width: w, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
                    pm.stroke_path(p, &paint, &stroke, t, m);
                }
                None => pm.fill_path(p, &paint, FillRule::Winding, t, m),
            }
        });
    }

    /// Draw an icon centered in `rect`. `stroke` is in 24-unit icon space
    /// (2.0 is the standard weight).
    pub fn icon(&mut self, icon: &Icon, rect: Rect, color: Color, stroke: f32) {
        let Some(p) = icon.path() else { return };
        let sw = if icon.filled() { None } else { Some(stroke) };
        self.draw_path(&p, 24.0, rect, color, sw);
    }

    /// Draw a single line of text with its top-left corner at `pos`.
    pub fn text(&mut self, pos: Point, s: &str, style: &TextStyle, color: Color) {
        let size = self.text.measure(s, style, None, self.scale);
        self.text_block(s, style, Rect::new(pos.x, pos.y, size.w, size.h), None, color, false);
    }

    /// Draw text into `rect`, optionally wrapped at `wrap_width` and/or truncated with "…".
    pub fn text_block(
        &mut self,
        s: &str,
        style: &TextStyle,
        rect: Rect,
        wrap_width: Option<f32>,
        color: Color,
        ellipsis: bool,
    ) {
        let clip = self.clip_rect();
        let (scale, origin) = (self.scale, self.origin);
        self.text.draw(&mut self.pixmap, s, style, rect, wrap_width, color, scale, clip, ellipsis, origin);
    }

    /// Measure text in logical pixels.
    pub fn measure_text(&mut self, s: &str, style: &TextStyle, max_width: Option<f32>) -> crate::geometry::Size {
        self.text.measure(s, style, max_width, self.scale)
    }

    // --------------------------------------------------------------- shadows

    /// Draw a CSS-style box shadow for a box at `rect` with `radius`.
    pub fn box_shadow(&mut self, rect: Rect, radius: Corners, sh: &Shadow) {
        if sh.color.a <= 0.0 {
            return;
        }
        let s = self.scale;
        let r = rect.translate(sh.x, sh.y).outset(sh.spread);
        if r.is_empty() {
            return;
        }
        let radius_box = radius;
        let radius = radius.grow(sh.spread);
        if sh.blur <= 0.0 {
            self.fill_rrect(r, radius, &Fill::Solid(sh.color));
            return;
        }
        let blur_px = sh.blur * s;
        let pad = (blur_px * 1.5).ceil();
        let key = ShadowKey {
            w: (r.w * s).round() as u32,
            h: (r.h * s).round() as u32,
            radius: [
                (radius.tl * s).round() as u32,
                (radius.tr * s).round() as u32,
                (radius.br * s).round() as u32,
                (radius.bl * s).round() as u32,
            ],
            blur: blur_px.round() as u32,
        };
        let frame = self.cache.frame;
        let entry = self
            .cache
            .shadows
            .entry(key)
            .or_insert_with_key(|k| (build_shadow_mask(k.w as f32, k.h as f32, &k.radius, blur_px, pad), frame));
        entry.1 = frame;
        let mask = &entry.0;
        let x0 = (r.x * s - self.origin.0).round() as i32 - mask.pad as i32;
        let y0 = (r.y * s - self.origin.1).round() as i32 - mask.pad as i32;
        let clip = self.clips.last().map(|c| {
            let p = scale_rect(c.rect, s);
            Rect::new(p.x - self.origin.0, p.y - self.origin.1, p.w, p.h)
        });
        // CSS never paints an outer shadow beneath its box: skip the box interior
        // (inset by the corner radius, where the box itself is fully opaque).
        let inset = rect_radius_max(&radius_box) * s + 1.0;
        let bx = scale_rect(rect, s);
        let skip = (
            (bx.x - self.origin.0 + inset).ceil() as i32,
            (bx.y - self.origin.1 + inset).ceil() as i32,
            (bx.right() - self.origin.0 - inset).floor() as i32,
            (bx.bottom() - self.origin.1 - inset).floor() as i32,
        );
        let skip = if skip.2 > skip.0 && skip.3 > skip.1 { Some(skip) } else { None };
        blend_alpha(&mut self.pixmap, x0, y0, mask.w, mask.h, &mask.alpha, sh.color, clip, skip);
    }
}

fn rect_radius_max(c: &Corners) -> f32 {
    c.max()
}

/// Clip snapshot used when escaping clipping for overlays.
pub(crate) struct Clip2(Rect, Corners);

fn build_shadow_mask(w: f32, h: f32, radius: &[u32; 4], blur: f32, pad: f32) -> ShadowMask {
    let mw = (w + pad * 2.0) as usize;
    let mh = (h + pad * 2.0) as usize;
    let mut alpha = vec![0u8; mw * mh];
    if let Some(mut pm) = Pixmap::new(mw as u32, mh as u32) {
        let c = Corners { tl: radius[0] as f32, tr: radius[1] as f32, br: radius[2] as f32, bl: radius[3] as f32 };
        if let Some(p) = rrect_path(Rect::new(pad, pad, w, h), c) {
            let mut paint = Paint { anti_alias: true, ..Default::default() };
            paint.set_color(tiny_skia::Color::BLACK);
            pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
        }
        for (i, px) in pm.data().chunks_exact(4).enumerate() {
            alpha[i] = px[3];
        }
    }
    // CSS blur radius ≈ 2σ. Three box blurs approximate a Gaussian.
    let sigma = blur / 2.0;
    for bs in boxes_for_gauss(sigma, 3) {
        let r = ((bs - 1) / 2) as usize;
        if r > 0 {
            box_blur_h(&mut alpha, mw, mh, r);
            box_blur_v(&mut alpha, mw, mh, r);
        }
    }
    ShadowMask { w: mw, h: mh, pad, alpha }
}

fn boxes_for_gauss(sigma: f32, n: usize) -> Vec<i32> {
    let w_ideal = ((12.0 * sigma * sigma / n as f32) + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m_ideal = (12.0 * sigma * sigma - (n as i32 * wl * wl) as f32 - 4.0 * n as f32 * wl as f32 - 3.0 * n as f32)
        / (-4.0 * wl as f32 - 4.0);
    let m = m_ideal.round() as i32;
    (0..n as i32).map(|i| if i < m { wl } else { wu }).collect()
}

fn box_blur_h(a: &mut [u8], w: usize, h: usize, r: usize) {
    let mut row = vec![0u32; w];
    let d = (2 * r + 1) as u32;
    for y in 0..h {
        let line = &mut a[y * w..(y + 1) * w];
        let mut acc: u32 = line[..=r.min(w - 1)].iter().map(|&v| v as u32).sum();
        for x in 0..w {
            row[x] = acc;
            if x + r + 1 < w {
                acc += line[x + r + 1] as u32;
            }
            if x >= r {
                acc -= line[x - r] as u32;
            }
        }
        for x in 0..w {
            line[x] = (row[x] / d) as u8;
        }
    }
}

fn box_blur_v(a: &mut [u8], w: usize, h: usize, r: usize) {
    let mut col = vec![0u32; h];
    let d = (2 * r + 1) as u32;
    for x in 0..w {
        let mut acc: u32 = 0;
        for y in 0..=r.min(h - 1) {
            acc += a[y * w + x] as u32;
        }
        for y in 0..h {
            col[y] = acc;
            if y + r + 1 < h {
                acc += a[(y + r + 1) * w + x] as u32;
            }
            if y >= r {
                acc -= a[(y - r) * w + x] as u32;
            }
        }
        for y in 0..h {
            a[y * w + x] = (col[y] / d) as u8;
        }
    }
}

/// Composite `color` × `alpha` (an 8-bit coverage mask) onto the pixmap at
/// (`x0`, `y0`). Pixels inside `skip` (target coordinates) are left untouched.
#[allow(clippy::too_many_arguments)]
fn blend_alpha(
    pm: &mut Pixmap,
    x0: i32,
    y0: i32,
    w: usize,
    h: usize,
    alpha: &[u8],
    color: Color,
    clip: Option<Rect>,
    skip: Option<(i32, i32, i32, i32)>,
) {
    let pw = pm.width() as i32;
    let ph = pm.height() as i32;
    let (cx0, cy0, cx1, cy1) = match clip {
        Some(c) => (c.x.floor() as i32, c.y.floor() as i32, c.right().ceil() as i32, c.bottom().ceil() as i32),
        None => (0, 0, pw, ph),
    };
    let (cx0, cy0, cx1, cy1) =
        (cx0.max(0).max(x0), cy0.max(0).max(y0), cx1.min(pw).min(x0 + w as i32), cy1.min(ph).min(y0 + h as i32));
    if cx1 <= cx0 || cy1 <= cy0 {
        return;
    }
    let data = pm.data_mut();
    let ca = (color.a.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    let (cr, cg, cb) = (
        (color.r.clamp(0.0, 1.0) * 255.0 + 0.5) as u32,
        (color.g.clamp(0.0, 1.0) * 255.0 + 0.5) as u32,
        (color.b.clamp(0.0, 1.0) * 255.0 + 0.5) as u32,
    );
    for py in cy0..cy1 {
        let row = (py - y0) as usize * w;
        let (sx0, sx1) = match skip {
            Some((a, b, c, d)) if py >= b && py < d => (a, c),
            _ => (i32::MAX, i32::MAX),
        };
        let mut px = cx0;
        while px < cx1 {
            if px == sx0 {
                px = sx1.max(px + 1);
                continue;
            }
            let m = alpha[row + (px - x0) as usize] as u32;
            if m != 0 {
                // a in 0..=255
                let a = (m * ca + 127) / 255;
                let inv = 255 - a;
                let di = ((py * pw + px) * 4) as usize;
                data[di] = ((cr * a + data[di] as u32 * inv + 127) / 255) as u8;
                data[di + 1] = ((cg * a + data[di + 1] as u32 * inv + 127) / 255) as u8;
                data[di + 2] = ((cb * a + data[di + 2] as u32 * inv + 127) / 255) as u8;
                data[di + 3] = ((a * 255 + data[di + 3] as u32 * inv + 127) / 255) as u8;
            }
            px += 1;
        }
    }
}
