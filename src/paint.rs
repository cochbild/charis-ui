//! The public drawing API. [`Canvas`] records drawing commands into a
//! [`Scene`] that is then rendered by the CPU or GPU backend.

use std::rc::Rc;

use tiny_skia::{PathBuilder, Transform};

use crate::color::{Color, Fill};
use crate::geometry::{Point, Rect, Size};
use crate::icons::Icon;
use crate::scene::{Cmd, Scene};
use crate::style::{Corners, Edges, Shadow};
use crate::text::{TextStyle, TextSystem};

/// A drawing surface. Coordinates are logical pixels. Custom widgets draw
/// with it via [`canvas`](crate::canvas).
pub struct Canvas<'a> {
    pub(crate) scene: Scene,
    clips: Vec<(Rect, Corners)>,
    pub(crate) text: &'a mut TextSystem,
    pub(crate) scale: f32,
}

impl<'a> Canvas<'a> {
    pub(crate) fn new(scene: Scene, text: &'a mut TextSystem) -> Self {
        let scale = scene.scale;
        Self { scene, clips: Vec::new(), text, scale }
    }

    pub(crate) fn finish(self) -> Scene {
        self.scene
    }

    /// Display scale factor (physical pixels per logical pixel).
    pub fn scale(&self) -> f32 {
        self.scale
    }

    fn visible(&self, bounds: Rect) -> bool {
        match self.clips.last() {
            Some(c) => !c.0.intersect(&bounds).is_empty(),
            None => true,
        }
    }

    // ------------------------------------------------------------- clipping

    /// Restrict drawing to `rect` (intersected with the current clip).
    pub fn push_clip(&mut self, rect: Rect, radius: Corners) {
        let rect = match self.clips.last() {
            Some(c) => c.0.intersect(&rect),
            None => rect,
        };
        self.clips.push((rect, radius));
        self.scene.cmds.push(Cmd::PushClip { rect, radius });
    }

    pub fn pop_clip(&mut self) {
        self.clips.pop();
        self.scene.cmds.push(Cmd::PopClip);
    }

    /// The current clip rectangle, if any.
    pub fn clip_rect(&self) -> Option<Rect> {
        self.clips.last().map(|c| c.0)
    }

    /// Temporarily disable clipping (used for overlays/portals).
    pub(crate) fn take_clips(&mut self) -> Vec<(Rect, Corners)> {
        self.scene.cmds.push(Cmd::SetClips(Vec::new()));
        std::mem::take(&mut self.clips)
    }

    pub(crate) fn restore_clips(&mut self, c: Vec<(Rect, Corners)>) {
        self.scene.cmds.push(Cmd::SetClips(c.clone()));
        self.clips = c;
    }

    // --------------------------------------------------------------- layers

    /// Begin a group composited with `opacity` when popped. `bounds` (logical)
    /// limits the group's extent; `None` covers the whole window.
    pub fn push_layer(&mut self, opacity: f32, bounds: Option<Rect>) {
        self.scene.cmds.push(Cmd::PushLayer { opacity, bounds });
    }

    pub fn pop_layer(&mut self) {
        self.scene.cmds.push(Cmd::PopLayer);
    }

    // ------------------------------------------------------------ primitives

    /// At a fractional scale (125%, 150%…), logical pixel edges fall
    /// between device pixels: returns the scale then, for snapping.
    fn fractional(&self) -> Option<f32> {
        let s = self.scale;
        ((s - s.round()).abs() > 0.01).then_some(s)
    }

    /// Fill a (rounded) rectangle.
    pub fn fill_rrect(&mut self, rect: Rect, radius: Corners, fill: &Fill) {
        if fill.is_transparent() || rect.is_empty() || !self.visible(rect) {
            return;
        }
        // Square-cornered fills get whole device pixels at fractional
        // scales, so edges and 1 px lines stay crisp instead of blending.
        let rect = match self.fractional() {
            Some(s) if radius.is_zero() => snap_rect(rect, s),
            _ => rect,
        };
        self.scene.cmds.push(Cmd::Fill { rect, radius, fill: fill.clone() });
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
        if widths.is_zero() || color.a <= 0.0 || rect.is_empty() || !self.visible(rect) {
            return;
        }
        if let (Some(s), true) = (self.fractional(), radius.is_zero()) {
            // Each side as a whole number of device pixels (at least one),
            // from the snapped outer edge, so every line has the same
            // thickness wherever it lands.
            let o = snap_rect(rect, s);
            let px = |w: f32| if w > 0.0 { (w * s).round().max(1.0) / s } else { 0.0 };
            let (t, b, l, r) = (px(widths.top), px(widths.bottom), px(widths.left), px(widths.right));
            let sides = [
                Rect::new(o.x, o.y, o.w, t),
                Rect::new(o.x, o.bottom() - b, o.w, b),
                Rect::new(o.x, o.y + t, l, o.h - t - b),
                Rect::new(o.right() - r, o.y + t, r, o.h - t - b),
            ];
            for side in sides {
                if side.w > 0.0 && side.h > 0.0 {
                    self.scene.cmds.push(Cmd::Fill { rect: side, radius: Corners::ZERO, fill: Fill::Solid(color) });
                }
            }
            return;
        }
        self.scene.cmds.push(Cmd::Border { rect, radius, widths, color });
    }

    /// Stroke a rounded rectangle outline centered on `rect`'s edge.
    pub fn stroke_rrect(&mut self, rect: Rect, radius: Corners, width: f32, color: Color) {
        if color.a <= 0.0 || !self.visible(rect.outset(width)) {
            return;
        }
        self.scene.cmds.push(Cmd::Stroke { rect, radius, width, color });
    }

    /// Draw a straight line with round caps.
    pub fn line(&mut self, a: Point, b: Point, width: f32, color: Color) {
        let mut pb = PathBuilder::new();
        pb.move_to(a.x, a.y);
        pb.line_to(b.x, b.y);
        let Some(path) = pb.finish() else { return };
        let bounds = Rect::new(a.x.min(b.x), a.y.min(b.y), (a.x - b.x).abs(), (a.y - b.y).abs()).outset(width);
        let s = self.scale;
        self.path(Rc::new(path), Transform::from_scale(s, s), color, Some(width), bounds);
    }

    /// Fill a circle.
    pub fn fill_circle(&mut self, center: Point, radius: f32, color: Color) {
        let r = Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0);
        self.fill_rrect(r, Corners::all(radius), &Fill::Solid(color));
    }

    /// Stroke or fill arbitrary SVG path data, mapped from a square `view` box onto `rect`.
    pub fn svg_path(&mut self, d: &str, view: f32, rect: Rect, color: Color, stroke_width: Option<f32>) {
        if let Some(p) = crate::icons::parse_svg_path(d) {
            let t = self.fit(view, rect);
            self.path(Rc::new(p), t, color, stroke_width, rect.outset(2.0));
        }
    }

    fn fit(&self, view: f32, rect: Rect) -> Transform {
        let s = self.scale;
        let k = rect.w.min(rect.h) / view * s;
        let tx = rect.x * s + (rect.w * s - view * k) / 2.0;
        let ty = rect.y * s + (rect.h * s - view * k) / 2.0;
        Transform::from_row(k, 0.0, 0.0, k, tx, ty)
    }

    pub(crate) fn path(
        &mut self,
        path: Rc<tiny_skia::Path>,
        transform: Transform,
        color: Color,
        stroke: Option<f32>,
        bounds: Rect,
    ) {
        if color.a <= 0.0 || !self.visible(bounds) {
            return;
        }
        self.scene.cmds.push(Cmd::Path { path, transform, color, stroke, bounds });
    }

    /// Draw an icon centered in `rect`. `stroke` is in 24-unit icon space
    /// (2.0 is the standard weight).
    pub fn icon(&mut self, icon: &Icon, rect: Rect, color: Color, stroke: f32) {
        let Some(p) = icon.path() else { return };
        let sw = if icon.filled() { None } else { Some(stroke) };
        let t = self.fit(24.0, rect);
        self.path(p, t, color, sw, rect.outset(2.0));
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
        if s.is_empty() || color.a <= 0.0 || !self.visible(rect.outset(4.0)) {
            return;
        }
        let glyphs = self.text.glyphs(s, style, rect, wrap_width, self.scale, ellipsis);
        if !glyphs.is_empty() {
            self.scene.cmds.push(Cmd::Glyphs { glyphs, color });
        }
    }

    /// Draw rich text; each span may override `color`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rich_text_block(
        &mut self,
        s: &str,
        spans: Option<&[crate::text::Span]>,
        style: &TextStyle,
        rect: Rect,
        wrap_width: Option<f32>,
        color: Color,
        link_color: Color,
        ellipsis: bool,
    ) {
        let Some(spans) = spans else {
            self.text_block(s, style, rect, wrap_width, color, ellipsis);
            return;
        };
        if s.is_empty() || !self.visible(rect.outset(4.0)) {
            return;
        }
        let glyphs = self.text.glyphs_rich(s, Some(spans), style, rect, wrap_width, self.scale, ellipsis);
        let color_of = |g: &crate::text::GlyphInst| {
            spans
                .get(g.span as usize)
                .map(|sp| sp.color.unwrap_or(if sp.link.is_some() { link_color } else { color }))
                .unwrap_or(color)
        };
        // Group consecutive glyphs by color.
        let mut run: Vec<crate::text::GlyphInst> = Vec::new();
        let mut cur: Option<Color> = None;
        for g in glyphs {
            let c = color_of(&g);
            if cur.is_some_and(|x| x != c) {
                self.scene.cmds.push(Cmd::Glyphs { glyphs: std::mem::take(&mut run), color: cur.unwrap_or(color) });
            }
            cur = Some(c);
            run.push(g);
        }
        if !run.is_empty() {
            self.scene.cmds.push(Cmd::Glyphs { glyphs: run, color: cur.unwrap_or(color) });
        }
    }

    /// Measure text in logical pixels.
    pub fn measure_text(&mut self, s: &str, style: &TextStyle, max_width: Option<f32>) -> Size {
        self.text.measure(s, style, max_width, self.scale)
    }

    /// Draw a CSS-style box shadow for a box at `rect` with `radius`.
    pub fn box_shadow(&mut self, rect: Rect, radius: Corners, sh: &Shadow) {
        let reach = sh.blur * 1.5 + sh.spread.abs() + sh.x.abs().max(sh.y.abs());
        if sh.color.a <= 0.0 || !self.visible(rect.outset(reach)) {
            return;
        }
        self.scene.cmds.push(Cmd::Shadow { rect, radius, shadow: *sh });
    }
}

/// `r` with its edges moved to the nearest device pixel boundaries (keeping
/// at least one device pixel of size).
fn snap_rect(r: Rect, s: f32) -> Rect {
    let x0 = (r.x * s).round();
    let y0 = (r.y * s).round();
    let x1 = ((r.x + r.w) * s).round().max(x0 + 1.0);
    let y1 = ((r.y + r.h) * s).round().max(y0 + 1.0);
    Rect::new(x0 / s, y0 / s, (x1 - x0) / s, (y1 - y0) / s)
}
