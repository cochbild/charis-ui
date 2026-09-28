//! Text shaping, measurement and rasterization (cosmic-text + swash).

use cosmic_text::{fontdb, Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent, Wrap};

use crate::color::Color;
use crate::geometry::{Rect, Size};
use crate::style::FontFamily;

/// A positioned glyph in physical window pixels (baseline origin).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GlyphInst {
    pub key: cosmic_text::CacheKey,
    pub x: i32,
    pub y: i32,
    /// Index of the rich-text span this glyph belongs to (`u32::MAX` = none).
    pub span: u32,
}

/// A run of styled text inside [`rich_text`](crate::rich_text).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Span {
    /// The text itself.
    pub text: String,
    /// Font weight (400 regular, 700 bold); `None` inherits.
    pub weight: Option<u16>,
    /// Italic style.
    pub italic: bool,
    /// Use the monospace font.
    pub mono: bool,
    /// Text color; `None` inherits.
    pub color: Option<Color>,
    /// Font size in logical px (defaults to the element's size).
    pub size: Option<f32>,
    /// A link target; clicks are delivered to `Element::on_link`.
    pub link: Option<String>,
}

/// Shorthand for a plain [`Span`].
///
/// ```
/// use charis_ui::prelude::*;
///
/// let msg: Element<()> = rich_text([
///     span("Saved to "),
///     span("notes.txt").mono(),
///     span(". "),
///     span("Undo").bold().link("undo"),
/// ]);
/// ```
pub fn span(text: impl Into<String>) -> Span {
    Span { text: text.into(), ..Default::default() }
}

impl Span {
    /// Bold (weight 700).
    pub fn bold(mut self) -> Self {
        self.weight = Some(700);
        self
    }
    /// Semibold (weight 600).
    pub fn semibold(mut self) -> Self {
        self.weight = Some(600);
        self
    }
    /// Set the font weight (100–900).
    pub fn weight(mut self, w: u16) -> Self {
        self.weight = Some(w);
        self
    }
    /// Italic.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    /// Monospace font.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }
    /// Set the text color.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// Set the font size in logical px.
    pub fn size(mut self, s: f32) -> Self {
        self.size = Some(s);
        self
    }
    /// Make this span a link to `url`; clicks go to `Element::on_link`.
    pub fn link(mut self, url: impl Into<String>) -> Self {
        self.link = Some(url.into());
        self
    }
}

fn hash_spans(spans: &[Span]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for s in spans {
        s.text.hash(&mut h);
        s.weight.hash(&mut h);
        s.italic.hash(&mut h);
        s.mono.hash(&mut h);
        s.size.map(f32::to_bits).hash(&mut h);
    }
    h.finish() | 1
}

/// Resolved text properties used for shaping.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Font size in logical px.
    pub size: f32,
    /// Font weight (400 regular, 700 bold).
    pub weight: u16,
    /// Font family.
    pub family: FontFamily,
    /// Italic style.
    pub italic: bool,
    /// Line height multiplier.
    pub line_height: f32,
    /// Extra space between letters, in logical px.
    pub letter_spacing: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self { size: 13.0, weight: 400, family: FontFamily::Ui, italic: false, line_height: 1.4, letter_spacing: 0.0 }
    }
}

use crate::fxhash::{FxHasher, IdMap};

/// Everything that affects shaping except the wrap width.
#[derive(PartialEq, Clone)]
struct ShapeKey {
    text: String,
    spans: u64,
    size: u32,
    weight: u16,
    family: FontFamily,
    italic: bool,
    lh: u32,
    ls: u32,
    scale: u32,
}

impl ShapeKey {
    fn matches(&self, text: &str, spans: u64, st: &TextStyle, scale: f32) -> bool {
        self.spans == spans
            && self.size == st.size.to_bits()
            && self.weight == st.weight
            && self.italic == st.italic
            && self.lh == st.line_height.to_bits()
            && self.ls == st.letter_spacing.to_bits()
            && self.scale == scale.to_bits()
            && self.family == st.family
            && self.text == text
    }
}

/// A shaped text: shaped once, re-wrapped (cheaply) for each width asked for.
struct Entry {
    key: ShapeKey,
    buffer: Buffer,
    /// Width (physical px) the buffer is currently laid out at.
    width: Option<u32>,
    /// Logical size at the current width.
    size: (f32, f32),
    /// Sizes measured at other widths, so layout's repeated min/max/definite
    /// queries don't re-wrap back and forth.
    sizes: Vec<(Option<u32>, (f32, f32))>,
    last_used: u64,
}

fn buffer_size(buffer: &Buffer, scale: f32) -> (f32, f32) {
    let mut w: f32 = 0.0;
    let mut h: f32 = 0.0;
    let mut lines = 0;
    for run in buffer.layout_runs() {
        w = w.max(run.line_w);
        h += run.line_height;
        lines += 1;
    }
    if lines == 0 {
        h = buffer.metrics().line_height;
    }
    (w / scale, h / scale)
}

/// Shapes and caches text layouts and rasterizes glyphs.
pub struct TextSystem {
    fs: FontSystem,
    swash: SwashCache,
    cache: IdMap<Entry>,
    /// Paragraph layouts of multi-line inputs, by node id.
    pub(crate) docs: IdMap<crate::text_doc::Doc>,
    frame: u64,
    ui_family: Option<String>,
    /// Apply DirectWrite-style contrast/gamma correction to glyph coverage.
    pub text_correction: bool,
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "bundled-fonts")]
const BUNDLED: &[&[u8]] = &[
    include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Inter-Bold.ttf"),
];

impl TextSystem {
    /// A text system with system (and bundled) fonts loaded.
    pub fn new() -> Self {
        // Scanning system fonts is the slow part (hundreds of fonts on a
        // typical desktop): do it once per thread and give later windows a
        // copy (font data is shared, not copied).
        thread_local! {
            static BASE: std::cell::RefCell<Option<(fontdb::Database, Option<String>)>> =
                const { std::cell::RefCell::new(None) };
        }
        let (db, ui_family) = BASE.with(|b| b.borrow_mut().get_or_insert_with(Self::scan_fonts).clone());
        let fs = FontSystem::new_with_locale_and_db("en-US".into(), db);
        Self {
            fs,
            swash: SwashCache::new(),
            cache: IdMap::default(),
            docs: IdMap::default(),
            frame: 0,
            ui_family,
            text_correction: true,
        }
    }

    fn scan_fonts() -> (fontdb::Database, Option<String>) {
        let mut db = fontdb::Database::new();
        // Bundled faces go in first: fontdb picks the earliest of equally good
        // matches, so a system-installed Inter of another version can't replace them.
        #[cfg(feature = "bundled-fonts")]
        let mut ui_family = {
            for data in BUNDLED {
                db.load_font_data(data.to_vec());
            }
            db.set_sans_serif_family("Inter");
            Some("Inter".to_string())
        };
        db.load_system_fonts();
        #[cfg(not(feature = "bundled-fonts"))]
        let mut ui_family: Option<String> = None;
        let families: Vec<String> = db.faces().flat_map(|f| f.families.iter().map(|(n, _)| n.clone())).collect();
        for mono in
            ["JetBrains Mono", "Cascadia Code", "SF Mono", "Menlo", "Consolas", "DejaVu Sans Mono", "Liberation Mono"]
        {
            if families.iter().any(|f| f == mono) {
                db.set_monospace_family(mono);
                break;
            }
        }
        if ui_family.is_none() {
            for ui in [
                "Segoe UI",
                "SF Pro Text",
                ".AppleSystemUIFont",
                "Cantarell",
                "Ubuntu",
                "Noto Sans",
                "DejaVu Sans",
                "Liberation Sans",
            ] {
                if families.iter().any(|f| f == ui) {
                    ui_family = Some(ui.to_string());
                    break;
                }
            }
        }
        (db, ui_family)
    }

    /// Register an additional font (TTF/OTF bytes). Use its family name with
    /// `FontFamily::Named`.
    pub fn load_font(&mut self, data: Vec<u8>) {
        self.fs.db_mut().load_font_data(data);
        self.cache.clear();
        self.docs.clear();
    }

    /// The platform's UI font, if installed: Segoe UI Variable (Segoe UI
    /// before Windows 11) on Windows, the system font (SF Pro) on macOS,
    /// and the desktop's font on Linux (Adwaita Sans or Cantarell on GNOME,
    /// Noto Sans on KDE, Ubuntu on Ubuntu).
    pub fn system_ui_family(&self) -> Option<String> {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
        let candidates: Vec<&str> = if cfg!(windows) {
            vec!["Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI"]
        } else if cfg!(target_os = "macos") {
            vec![
                "SF Pro Text",
                "SF Pro",
                ".SF NS Text",
                ".SF NS",
                "System Font",
                ".AppleSystemUIFont",
                "Helvetica Neue",
            ]
        } else if desktop.contains("kde") {
            vec!["Noto Sans", "Inter", "DejaVu Sans"]
        } else if desktop.contains("ubuntu") {
            vec!["Ubuntu Sans", "Ubuntu", "Cantarell", "Noto Sans", "DejaVu Sans"]
        } else {
            vec!["Adwaita Sans", "Cantarell", "Noto Sans", "Ubuntu Sans", "Ubuntu", "DejaVu Sans", "Liberation Sans"]
        };
        let db = self.fs.db();
        candidates.into_iter().find(|c| db.faces().any(|f| f.families.iter().any(|(n, _)| n == c))).map(str::to_string)
    }

    /// Use a registered family as the UI font.
    pub fn set_ui_family(&mut self, name: &str) {
        self.ui_family = Some(name.to_string());
        self.cache.clear();
        self.docs.clear();
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame += 1;
        // Evict layouts not used in the last few frames.
        if self.frame.is_multiple_of(120) || self.cache.len() > 4096 {
            let f = self.frame;
            self.cache.retain(|_, e| f - e.last_used < 60);
        }
    }

    fn entry(&mut self, text: &str, st: &TextStyle, max_width: Option<f32>, scale: f32) -> &mut Entry {
        self.entry_rich(text, None, st, max_width, scale)
    }

    fn shape_hash(text: &str, spans: u64, st: &TextStyle, scale: f32) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = FxHasher::default();
        text.hash(&mut h);
        spans.hash(&mut h);
        st.size.to_bits().hash(&mut h);
        st.weight.hash(&mut h);
        st.family.hash(&mut h);
        st.italic.hash(&mut h);
        st.line_height.to_bits().hash(&mut h);
        st.letter_spacing.to_bits().hash(&mut h);
        scale.to_bits().hash(&mut h);
        h.finish()
    }

    /// The cached, shaped entry for this text and style (width-independent).
    fn shaped(&mut self, text: &str, spans: Option<&[Span]>, st: &TextStyle, scale: f32) -> u64 {
        let sh = spans.map(hash_spans).unwrap_or(0);
        let hash = Self::shape_hash(text, sh, st, scale);
        let frame = self.frame;
        if let Some(e) = self.cache.get_mut(&hash) {
            if e.key.matches(text, sh, st, scale) {
                e.last_used = frame;
                return hash;
            }
        }
        let font_px = (st.size * scale).max(1.0);
        let metrics = Metrics::new(font_px, (st.size * st.line_height * scale).max(1.0));
        let mut buffer = Buffer::new_empty(metrics);
        buffer.set_wrap(Wrap::WordOrGlyph);
        buffer.set_size(None, None);
        let family = match &st.family {
            FontFamily::Ui => match &self.ui_family {
                Some(n) => Family::Name(n.as_str()),
                None => Family::SansSerif,
            },
            FontFamily::Mono => Family::Monospace,
            FontFamily::Named(n) => Family::Name(n.as_str()),
        };
        let mut attrs = Attrs::new().family(family).weight(fontdb::Weight(st.weight));
        if st.italic {
            attrs = attrs.style(fontdb::Style::Italic);
        }
        if st.letter_spacing != 0.0 {
            // cosmic-text expects letter spacing in em units.
            attrs = attrs.letter_spacing(st.letter_spacing / st.size.max(1.0));
        }
        match spans {
            Some(spans) => {
                let mono = Family::Monospace;
                let items: Vec<(&str, Attrs)> = spans
                    .iter()
                    .enumerate()
                    .map(|(i, sp)| {
                        let mut a = attrs.clone().metadata(i);
                        if sp.mono {
                            a = a.family(mono);
                        }
                        if let Some(w) = sp.weight {
                            a = a.weight(fontdb::Weight(w));
                        }
                        if sp.italic {
                            a = a.style(fontdb::Style::Italic);
                        }
                        if let Some(sz) = sp.size {
                            a = a.metrics(Metrics::new(sz * scale, sz * st.line_height * scale));
                        }
                        (sp.text.as_str(), a)
                    })
                    .collect();
                buffer.set_rich_text(items, &attrs, Shaping::Advanced, None);
            }
            None => buffer.set_text(text, &attrs, Shaping::Advanced, None),
        }
        buffer.shape_until_scroll(&mut self.fs, false);
        let size = buffer_size(&buffer, scale);
        let key = ShapeKey {
            text: text.to_string(),
            spans: sh,
            size: st.size.to_bits(),
            weight: st.weight,
            family: st.family.clone(),
            italic: st.italic,
            lh: st.line_height.to_bits(),
            ls: st.letter_spacing.to_bits(),
            scale: scale.to_bits(),
        };
        self.cache.insert(hash, Entry { key, buffer, width: None, size, sizes: vec![(None, size)], last_used: frame });
        hash
    }

    fn entry_rich(
        &mut self,
        text: &str,
        spans: Option<&[Span]>,
        st: &TextStyle,
        max_width: Option<f32>,
        scale: f32,
    ) -> &mut Entry {
        let hash = self.shaped(text, spans, st, scale);
        let width = max_width.map(|w| (w * scale).ceil().max(0.0) as u32);
        let fs = &mut self.fs;
        let Some(e) = self.cache.get_mut(&hash) else { unreachable!("entry was just inserted") };
        if e.width != width {
            // Re-wrap only: shaping is kept per line by cosmic-text.
            e.buffer.set_size(width.map(|w| w as f32), None);
            e.buffer.shape_until_scroll(fs, false);
            e.width = width;
            e.size = buffer_size(&e.buffer, scale);
            if !e.sizes.iter().any(|(w, _)| *w == width) {
                if e.sizes.len() >= 6 {
                    e.sizes.remove(1);
                }
                e.sizes.push((width, e.size));
            }
        }
        e
    }

    /// Measure rich text in logical pixels.
    pub(crate) fn measure_rich(
        &mut self,
        text: &str,
        spans: Option<&[Span]>,
        st: &TextStyle,
        max_width: Option<f32>,
        scale: f32,
    ) -> Size {
        let hash = self.shaped(text, spans, st, scale);
        let width = max_width.map(|w| (w * scale).ceil().max(0.0) as u32);
        if let Some((_, (w, h))) = self.cache.get(&hash).and_then(|e| e.sizes.iter().find(|(x, _)| *x == width)) {
            return Size::new(*w, *h);
        }
        let (w, h) = self.entry_rich(text, spans, st, max_width, scale).size;
        Size::new(w, h)
    }

    /// Byte offset in `text` closest to the point (logical px, relative to the
    /// text's top-left).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn hit_byte(
        &mut self,
        text: &str,
        spans: Option<&[Span]>,
        st: &TextStyle,
        wrap: Option<f32>,
        scale: f32,
        x: f32,
        y: f32,
    ) -> usize {
        let starts = line_starts(text);
        let e = self.entry_rich(text, spans, st, wrap, scale);
        let total_h = e.size.1 * scale;
        let yy = (y * scale).clamp(0.0, (total_h - 1.0).max(0.0));
        match e.buffer.hit(x * scale, yy) {
            Some(c) => (starts.get(c.line).copied().unwrap_or(0) + c.index).min(text.len()),
            None => text.len(),
        }
    }

    /// Caret rectangle (logical px, relative to the text origin) for a byte offset.
    pub(crate) fn caret_rect(
        &mut self,
        text: &str,
        st: &TextStyle,
        wrap: Option<f32>,
        scale: f32,
        byte: usize,
    ) -> Rect {
        let starts = line_starts(text);
        let line = starts.iter().rposition(|&s| s <= byte).unwrap_or(0);
        let idx = byte - starts[line];
        let e = self.entry(text, st, wrap, scale);
        let mut best: Option<Rect> = None;
        for run in e.buffer.layout_runs() {
            if run.line_i != line {
                continue;
            }
            let r = |x: f32| Rect::new(x / scale, run.line_top / scale, 0.0, run.line_height / scale);
            if run.glyphs.is_empty() {
                best = Some(r(0.0));
                break;
            }
            let first = run.glyphs.first().map(|g| g.start).unwrap_or(0);
            let last = run.glyphs.last().map(|g| g.end).unwrap_or(0);
            if idx < first {
                continue;
            }
            if let Some(g) = run.glyphs.iter().find(|g| g.start <= idx && idx < g.end) {
                let x = if g.start == idx || g.level.is_rtl() { g.x } else { g.x + g.w };
                return r(x);
            }
            if idx >= last {
                // End of this visual line (may continue on the next run).
                best = Some(r(run.line_w));
            }
        }
        best.unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, st.size * st.line_height))
    }

    /// Rectangles covering bytes `a..b` (logical px, relative to the text origin).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn selection_rects(
        &mut self,
        text: &str,
        spans: Option<&[Span]>,
        st: &TextStyle,
        wrap: Option<f32>,
        scale: f32,
        a: usize,
        b: usize,
    ) -> Vec<Rect> {
        let starts = line_starts(text);
        let e = self.entry_rich(text, spans, st, wrap, scale);
        let mut out = Vec::new();
        for run in e.buffer.layout_runs() {
            let base = starts.get(run.line_i).copied().unwrap_or(0);
            let (mut x0, mut x1) = (f32::MAX, f32::MIN);
            for g in run.glyphs {
                let (gs, ge) = (base + g.start, base + g.end);
                if ge > a && gs < b {
                    x0 = x0.min(g.x);
                    x1 = x1.max(g.x + g.w);
                }
            }
            // Selected line breaks show as a small tail, like browsers.
            let line_end = base + run.glyphs.last().map(|g| g.end).unwrap_or(0);
            let nl_selected = a <= line_end && b > line_end;
            if x1 < x0 && nl_selected && run.glyphs.is_empty() {
                x0 = 0.0;
                x1 = st.size * 0.3 * scale;
            }
            if x1 > x0 {
                out.push(Rect::new(x0 / scale, run.line_top / scale, (x1 - x0) / scale, run.line_height / scale));
            }
        }
        out
    }

    /// Measure text in logical pixels. `max_width` enables wrapping.
    pub fn measure(&mut self, text: &str, st: &TextStyle, max_width: Option<f32>, scale: f32) -> Size {
        self.measure_rich(text, None, st, max_width, scale)
    }

    /// x offsets (logical, relative to text start) of every caret stop, as
    /// `(byte_index, x)` pairs sorted by byte index. Single-line only.
    pub fn caret_stops(&mut self, text: &str, st: &TextStyle, scale: f32) -> Vec<(usize, f32)> {
        let e = self.entry(text, st, None, scale);
        let mut stops: Vec<(usize, f32)> = vec![(0, 0.0)];
        for run in e.buffer.layout_runs() {
            for g in run.glyphs {
                stops.push((g.start, g.x / scale));
                stops.push((g.end, (g.x + g.w) / scale));
            }
        }
        stops.push((text.len(), e.size.0));
        stops.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)));
        stops.dedup_by_key(|s| s.0);
        // Only keep valid char boundaries.
        stops.retain(|(i, _)| text.is_char_boundary(*i));
        stops
    }

    /// Lay out text into positioned glyphs (physical window pixels), with its
    /// top-left at `rect.x, rect.y` (logical px). Handles wrapping, pixel
    /// snapping and "…" truncation.
    pub(crate) fn glyphs(
        &mut self,
        text: &str,
        st: &TextStyle,
        rect: Rect,
        wrap_width: Option<f32>,
        scale: f32,
        ellipsis: bool,
    ) -> Vec<GlyphInst> {
        self.glyphs_rich(text, None, st, rect, wrap_width, scale, ellipsis)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn glyphs_rich(
        &mut self,
        text: &str,
        spans: Option<&[Span]>,
        st: &TextStyle,
        rect: Rect,
        wrap_width: Option<f32>,
        scale: f32,
        ellipsis: bool,
    ) -> Vec<GlyphInst> {
        let rich = spans.is_some();
        let mut out = Vec::new();
        if text.is_empty() {
            return out;
        }
        let avail = rect.w * scale;
        let ell_w = if ellipsis { self.entry("…", st, None, scale).size.0 * scale } else { 0.0 };
        let ox = (rect.x * scale).round();
        let oy = (rect.y * scale).round();
        let mut ell_at = None;
        {
            let e = self.entry_rich(text, spans, st, wrap_width, scale);
            let truncate = ellipsis && e.size.0 * scale > avail + 0.5;
            for run in e.buffer.layout_runs() {
                for g in run.glyphs {
                    if truncate && g.x + g.w > avail - ell_w {
                        ell_at = Some(g.x);
                        break;
                    }
                    // Snap each baseline to the pixel grid: fractional baselines blur text.
                    let pg = g.physical((ox, (oy + run.line_y).round()), 1.0);
                    let span = if rich { g.metadata as u32 } else { u32::MAX };
                    out.push(GlyphInst { key: pg.cache_key, x: pg.x, y: pg.y, span });
                }
                if truncate {
                    if ell_at.is_none() {
                        ell_at = Some(run.line_w);
                    }
                    break;
                }
            }
        }
        if let Some(x) = ell_at {
            let e = self.entry("…", st, None, scale);
            for run in e.buffer.layout_runs() {
                for g in run.glyphs {
                    let pg = g.physical((ox + x, (oy + run.line_y).round()), 1.0);
                    out.push(GlyphInst { key: pg.cache_key, x: pg.x, y: pg.y, span: u32::MAX });
                }
            }
        }
        out
    }

    /// Rasterized glyph image (cached).
    #[cfg_attr(not(feature = "gpu"), allow(dead_code))]
    pub(crate) fn glyph_image(&mut self, key: cosmic_text::CacheKey) -> Option<&cosmic_text::SwashImage> {
        self.swash.get_image(&mut self.fs, key).as_ref()
    }

    /// Blit positioned glyphs into a pixmap whose top-left sits at `origin`
    /// (physical window px). `clip` is in physical window px.
    pub(crate) fn blit(
        &mut self,
        pixmap: &mut tiny_skia::Pixmap,
        glyphs: &[GlyphInst],
        color: Color,
        clip: Option<(i32, i32, i32, i32)>,
        origin: (i32, i32),
    ) {
        if color.a <= 0.0 {
            return;
        }
        let clip = clip.map(|(a, b, c, d)| (a - origin.0, b - origin.1, c - origin.0, d - origin.1));
        for g in glyphs {
            self.blit_glyph(pixmap, g.key, g.x - origin.0, g.y - origin.1, color, clip);
        }
    }

    fn blit_glyph(
        &mut self,
        pixmap: &mut tiny_skia::Pixmap,
        key: cosmic_text::CacheKey,
        gx: i32,
        gy: i32,
        color: Color,
        clip: Option<(i32, i32, i32, i32)>,
    ) {
        let Some(img) = self.swash.get_image(&mut self.fs, key) else { return };
        let pw = pixmap.width() as i32;
        let ph = pixmap.height() as i32;
        let (cx0, cy0, cx1, cy1) = clip.unwrap_or((0, 0, pw, ph));
        let (cx0, cy0, cx1, cy1) = (cx0.max(0), cy0.max(0), cx1.min(pw), cy1.min(ph));
        let x0 = gx + img.placement.left;
        let y0 = gy - img.placement.top;
        let w = img.placement.width as i32;
        let h = img.placement.height as i32;
        let lut = if self.text_correction { text_alpha_lut(color) } else { IDENTITY_LUT };
        let data = pixmap.data_mut();
        let ca = (color.a.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        let (cr, cg, cb) = (
            (color.r.clamp(0.0, 1.0) * 255.0 + 0.5) as u32,
            (color.g.clamp(0.0, 1.0) * 255.0 + 0.5) as u32,
            (color.b.clamp(0.0, 1.0) * 255.0 + 0.5) as u32,
        );
        let ys = cy0.max(y0)..cy1.min(y0 + h);
        let xs = cx0.max(x0)..cx1.min(x0 + w);
        for py in ys {
            let yy = py - y0;
            for px in xs.clone() {
                let xx = px - x0;
                let di = ((py * pw + px) * 4) as usize;
                // Premultiplied source (0..=255 per channel) and its alpha.
                let (sr, sg, sb, sa) = match img.content {
                    SwashContent::Mask | SwashContent::SubpixelMask => {
                        let m = if matches!(img.content, SwashContent::Mask) {
                            img.data[(yy * w + xx) as usize] as u32
                        } else {
                            img.data[((yy * w + xx) * 4 + 1) as usize] as u32
                        };
                        if m == 0 {
                            continue;
                        }
                        let m = lut[m as usize] as u32;
                        let a = (m * ca + 127) / 255;
                        ((cr * a + 127) / 255, (cg * a + 127) / 255, (cb * a + 127) / 255, a)
                    }
                    SwashContent::Color => {
                        let si = ((yy * w + xx) * 4) as usize;
                        let a = (img.data[si + 3] as u32 * ca + 127) / 255;
                        if a == 0 {
                            continue;
                        }
                        (
                            (img.data[si] as u32 * a + 127) / 255,
                            (img.data[si + 1] as u32 * a + 127) / 255,
                            (img.data[si + 2] as u32 * a + 127) / 255,
                            a,
                        )
                    }
                };
                let inv = 255 - sa;
                data[di] = (sr + (data[di] as u32 * inv + 127) / 255).min(255) as u8;
                data[di + 1] = (sg + (data[di + 1] as u32 * inv + 127) / 255).min(255) as u8;
                data[di + 2] = (sb + (data[di + 2] as u32 * inv + 127) / 255).min(255) as u8;
                data[di + 3] = (sa + (data[di + 3] as u32 * inv + 127) / 255).min(255) as u8;
            }
        }
    }
}

/// Byte offsets where each `\n`-separated line starts.
pub(crate) fn line_starts(text: &str) -> Vec<usize> {
    let mut v = vec![0];
    for (i, c) in text.char_indices() {
        if c == '\n' {
            v.push(i + 1);
        }
    }
    v
}

const IDENTITY_LUT: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = i as u8;
        i += 1;
    }
    t
};

/// DirectWrite gamma-correction ratios for gamma 1.8 (the Windows default),
/// as used by Windows Terminal's and Zed's text shaders.
pub(crate) const GAMMA_RATIOS: [f32; 4] = [0.1469 / 4.0, -0.8911 / 4.0, 1.4644 / 4.0, -0.3234 / 4.0];

/// Grayscale "enhanced contrast" amount (DirectWrite default).
pub(crate) const ENHANCED_CONTRAST: f32 = 1.0;

/// Map raw glyph coverage to perceptually corrected coverage for `color`.
///
/// Browsers and DirectWrite don't blend glyph edges naively: they boost
/// contrast (less for light-on-dark text) and apply gamma-aware alpha
/// correction. Without this, dark text looks heavy and light text on dark
/// backgrounds looks thin and washed out.
pub(crate) fn correct_alpha(a: f32, color: Color) -> f32 {
    let luma = 0.25 * color.r + 0.5 * color.g + 0.25 * color.b;
    let k = ENHANCED_CONTRAST * (luma * -4.0 + 3.0).clamp(0.0, 1.0);
    let contrasted = a * (k + 1.0) / (a * k + 1.0);
    let f = 0.30 * color.r + 0.59 * color.g + 0.11 * color.b;
    let g = GAMMA_RATIOS;
    let c = contrasted + contrasted * (1.0 - contrasted) * ((g[0] * f + g[1]) * contrasted + (g[2] * f + g[3]));
    c.clamp(0.0, 1.0)
}

pub(crate) fn text_alpha_lut(color: Color) -> [u8; 256] {
    let mut t = [0u8; 256];
    for (i, v) in t.iter_mut().enumerate() {
        *v = (correct_alpha(i as f32 / 255.0, color) * 255.0 + 0.5) as u8;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_correction_endpoints_and_direction() {
        for c in [Color::WHITE, Color::BLACK, Color::hex("#5b8cff")] {
            assert_eq!(correct_alpha(0.0, c), 0.0);
            assert!((correct_alpha(1.0, c) - 1.0).abs() < 1e-5);
        }
        // Light text on dark gets slightly heavier edges than naive blending.
        assert!(correct_alpha(0.5, Color::WHITE) > 0.5);
        // Dark text keeps full contrast enhancement.
        assert!(correct_alpha(0.5, Color::BLACK) > 0.5);
    }
}
