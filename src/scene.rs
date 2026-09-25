//! A frame's display list. The runtime records one [`Scene`] per frame;
//! a backend (CPU rasterizer or GPU renderer) turns it into pixels.

use std::rc::Rc;

use tiny_skia::{Path, Transform};

use crate::color::{Color, Fill};
use crate::geometry::Rect;
use crate::style::{Corners, Edges, Shadow};
use crate::text::GlyphInst;

/// One drawing command. Rects are in logical pixels; glyph and path
/// transforms are in physical window pixels.
#[derive(Clone)]
pub(crate) enum Cmd {
    Fill {
        rect: Rect,
        radius: Corners,
        fill: Fill,
    },
    Border {
        rect: Rect,
        radius: Corners,
        widths: Edges,
        color: Color,
    },
    /// Stroke centered on the rect's edge (outlines, focus rings).
    Stroke {
        rect: Rect,
        radius: Corners,
        width: f32,
        color: Color,
    },
    Shadow {
        rect: Rect,
        radius: Corners,
        shadow: Shadow,
    },
    /// A vector path in its own units; `transform` maps to physical px.
    /// `stroke` is the stroke width in path units (None = fill).
    Path {
        path: Rc<Path>,
        transform: Transform,
        color: Color,
        stroke: Option<f32>,
        bounds: Rect,
    },
    Glyphs {
        glyphs: Vec<GlyphInst>,
        color: Color,
    },
    /// Intersected clip rect (logical px).
    PushClip {
        rect: Rect,
        radius: Corners,
    },
    PopClip,
    /// Replace the entire clip stack.
    SetClips(Vec<(Rect, Corners)>),
    PushLayer {
        opacity: f32,
        bounds: Option<Rect>,
    },
    PopLayer,
}

/// A recorded frame.
#[derive(Clone)]
pub struct Scene {
    pub(crate) cmds: Vec<Cmd>,
    /// Physical pixels per logical pixel.
    pub scale: f32,
    /// Physical size.
    pub width: u32,
    pub height: u32,
    pub background: Color,
}

impl Scene {
    pub(crate) fn new(width: u32, height: u32, scale: f32, background: Color) -> Self {
        Self { cmds: Vec::with_capacity(2048), scale, width, height, background }
    }

    /// Number of recorded commands.
    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }
}
