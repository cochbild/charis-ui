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

    /// Fill a (rounded) rectangle.
    pub fn fill_rrect(&mut self, rect: Rect, radius: Corners, fill: &Fill) {
        if fill.is_transparent() || rect.is_empty() || !self.visible(rect) {
            return;
        }
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
