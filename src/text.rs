//! Text shaping, measurement and rasterization (cosmic-text + swash).

use std::collections::HashMap;

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
}

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
        Self { fs, swash: SwashCache::new(), cache: HashMap::new(), frame: 0, ui_family, text_correction: true }
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
            let e = self.entry(text, st, wrap_width, scale);
            let truncate = ellipsis && e.size.0 * scale > avail + 0.5;
            for run in e.buffer.layout_runs() {
                for g in run.glyphs {
                    if truncate && g.x + g.w > avail - ell_w {
                        ell_at = Some(g.x);
                        break;
                    }
                    // Snap each baseline to the pixel grid: fractional baselines blur text.
                    let pg = g.physical((ox, (oy + run.line_y).round()), 1.0);
                    out.push(GlyphInst { key: pg.cache_key, x: pg.x, y: pg.y });
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
                    out.push(GlyphInst { key: pg.cache_key, x: pg.x, y: pg.y });
                }
            }
        }
        out
    }

    /// Rasterized glyph image (cached).
    #[allow(dead_code)]
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
