//! Text shaping, measurement and rasterization (cosmic-text + swash).

use std::collections::HashMap;

use cosmic_text::{fontdb, Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent, Wrap};

use crate::color::Color;
use crate::geometry::{Rect, Size};
use crate::style::FontFamily;

/// Resolved text properties used for shaping.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub weight: u16,
    pub family: FontFamily,
    pub italic: bool,
    /// Line height multiplier.
    pub line_height: f32,
    pub letter_spacing: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self { size: 13.0, weight: 400, family: FontFamily::Ui, italic: false, line_height: 1.4, letter_spacing: 0.0 }
    }
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct Key {
    text: String,
    size: u32,
    weight: u16,
    family: FontFamily,
    italic: bool,
    lh: u32,
    ls: u32,
    width: Option<u32>,
    scale: u32,
}

struct Entry {
    buffer: Buffer,
    last_used: u64,
    size: (f32, f32),
}

/// Shapes and caches text layouts and rasterizes glyphs.
pub struct TextSystem {
    fs: FontSystem,
    swash: SwashCache,
    cache: HashMap<Key, Entry>,
    frame: u64,
    ui_family: Option<String>,
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
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        #[cfg(feature = "bundled-fonts")]
        let mut ui_family = {
            for data in BUNDLED {
                db.load_font_data(data.to_vec());
            }
            db.set_sans_serif_family("Inter");
            Some("Inter".to_string())
        };
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
        let fs = FontSystem::new_with_locale_and_db("en-US".into(), db);
        Self { fs, swash: SwashCache::new(), cache: HashMap::new(), frame: 0, ui_family }
    }

    /// Register an additional font (TTF/OTF bytes). Use its family name with
    /// `FontFamily::Named`.
    pub fn load_font(&mut self, data: Vec<u8>) {
        self.fs.db_mut().load_font_data(data);
        self.cache.clear();
    }

    /// Use a registered family as the UI font.
    pub fn set_ui_family(&mut self, name: &str) {
        self.ui_family = Some(name.to_string());
        self.cache.clear();
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
        let key = Key {
            text: text.to_string(),
            size: st.size.to_bits(),
            weight: st.weight,
            family: st.family.clone(),
            italic: st.italic,
            lh: st.line_height.to_bits(),
            ls: st.letter_spacing.to_bits(),
            width: max_width.map(|w| (w * scale).ceil().max(0.0) as u32),
            scale: scale.to_bits(),
        };
        let frame = self.frame;
        let fs = &mut self.fs;
        let ui = self.ui_family.clone();
        let e = self.cache.entry(key).or_insert_with_key(|k| {
            let font_px = (st.size * scale).max(1.0);
            let metrics = Metrics::new(font_px, (st.size * st.line_height * scale).max(1.0));
            let mut buffer = Buffer::new_empty(metrics);
            buffer.set_wrap(if k.width.is_some() { Wrap::WordOrGlyph } else { Wrap::None });
            buffer.set_size(k.width.map(|w| w as f32), None);
            let family = match &st.family {
                FontFamily::Ui => match &ui {
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
            buffer.set_text(text, &attrs, Shaping::Advanced, None);
            buffer.shape_until_scroll(fs, false);
            let mut w: f32 = 0.0;
            let mut lines = 0;
            for run in buffer.layout_runs() {
                w = w.max(run.line_w);
                lines += 1;
            }
            let h = lines.max(1) as f32 * metrics.line_height;
            Entry { buffer, last_used: frame, size: (w / scale, h / scale) }
        });
        e.last_used = frame;
        e
    }

    /// Measure text in logical pixels. `max_width` enables wrapping.
    pub fn measure(&mut self, text: &str, st: &TextStyle, max_width: Option<f32>, scale: f32) -> Size {
        let (w, h) = self.entry(text, st, max_width, scale).size;
        Size::new(w, h)
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

    /// Draw text with its top-left at `rect.x, rect.y` (logical px).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(
        &mut self,
        pixmap: &mut tiny_skia::Pixmap,
        text: &str,
        st: &TextStyle,
        rect: Rect,
        wrap_width: Option<f32>,
        color: Color,
        scale: f32,
        clip: Option<Rect>,
        ellipsis: bool,
        origin: (f32, f32),
    ) {
        if text.is_empty() || color.a <= 0.0 {
            return;
        }
        let avail = rect.w * scale;
        let ell_w = if ellipsis { self.entry("…", st, None, scale).size.0 * scale } else { 0.0 };
        let clip_px = clip.map(|c| {
            (
                (c.x * scale - origin.0).floor() as i32,
                (c.y * scale - origin.1).floor() as i32,
                (c.right() * scale - origin.0).ceil() as i32,
                (c.bottom() * scale - origin.1).ceil() as i32,
            )
        });
        let ox = (rect.x * scale).round() - origin.0;
        let oy = (rect.y * scale).round() - origin.1;
        let mut glyphs = Vec::new();
        let mut ell_at = None;
        {
            let e = self.entry(text, st, wrap_width, scale);
            let truncate = ellipsis && e.size.0 * scale > avail + 0.5;
            for run in e.buffer.layout_runs() {
                for g in run.glyphs {
                    if truncate && g.x + g.w > avail - ell_w {
                        ell_at = Some((g.x, run.line_y));
                        break;
                    }
                    let pg = g.physical((ox, oy + run.line_y), 1.0);
                    glyphs.push(pg);
                }
                if truncate {
                    if ell_at.is_none() {
                        ell_at = Some((run.line_w, run.line_y));
                    }
                    break;
                }
            }
        }
        for pg in glyphs {
            self.blit_glyph(pixmap, pg.cache_key, pg.x, pg.y, color, clip_px);
        }
        if let Some((x, _)) = ell_at {
            let mut ell = Vec::new();
            {
                let e = self.entry("…", st, None, scale);
                for run in e.buffer.layout_runs() {
                    for g in run.glyphs {
                        ell.push(g.physical((ox + x, oy + run.line_y), 1.0));
                    }
                }
            }
            for pg in ell {
                self.blit_glyph(pixmap, pg.cache_key, pg.x, pg.y, color, clip_px);
            }
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
