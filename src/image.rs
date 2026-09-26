//! Raster images and SVGs.
//!
//! [`Image`] holds decoded pixels (PNG always; JPEG with the `jpeg`
//! feature), [`Svg`] a parsed vector image (`svg` feature). Show them with
//! [`image`](crate::widgets::image) / [`svg`](crate::widgets::svg): they
//! size themselves (natural size, or keeping the aspect ratio when you set
//! one side), fit into their box with [`Fit`], take rounded corners from
//! `.rounded()`, and are rasterized once per displayed size, crisp at any
//! scale.
//!
//! Both are cheap to clone (shared) and are decoded once: keep them in your
//! app state rather than decoding in `view`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

use crate::color::Color;
use crate::geometry::Rect;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// A decoded raster image (premultiplied RGBA).
#[derive(Clone)]
pub struct Image {
    id: u64,
    pixmap: Rc<Pixmap>,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Image({}x{})", self.width(), self.height())
    }
}

impl PartialEq for Image {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
    }
}

impl Image {
    /// From straight-alpha RGBA8 pixels (`width * height * 4` bytes).
    pub fn from_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Image> {
        if rgba.len() != (width as usize) * (height as usize) * 4 {
            return None;
        }
        let mut pm = Pixmap::new(width, height)?;
        for (dst, src) in pm.pixels_mut().iter_mut().zip(rgba.chunks_exact(4)) {
            *dst = tiny_skia::ColorU8::from_rgba(src[0], src[1], src[2], src[3]).premultiply();
        }
        Some(Self::from_pixmap(pm))
    }

    fn from_pixmap(pm: Pixmap) -> Image {
        Image { id: next_id(), pixmap: Rc::new(pm) }
    }

    /// Decode PNG bytes.
    ///
    /// ```
    /// use rust_ui::image::Image;
    ///
    /// // A 2×1 image: one red pixel, one transparent.
    /// let img = Image::from_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 0, 0]).unwrap();
    /// let png = img.to_png().unwrap();
    ///
    /// let decoded = Image::from_png(&png).unwrap();
    /// assert_eq!((decoded.width(), decoded.height()), (2, 1));
    /// assert_eq!(decoded.to_rgba(), img.to_rgba());
    /// assert!(Image::from_png(b"not a png").is_none());
    /// ```
    pub fn from_png(bytes: &[u8]) -> Option<Image> {
        Pixmap::decode_png(bytes).ok().map(Self::from_pixmap)
    }

    /// Decode JPEG bytes.
    #[cfg(feature = "jpeg")]
    pub fn from_jpeg(bytes: &[u8]) -> Option<Image> {
        use zune_jpeg::zune_core::bytestream::ZCursor;
        use zune_jpeg::zune_core::colorspace::ColorSpace;
        use zune_jpeg::zune_core::options::DecoderOptions;
        let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
        let mut dec = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), opts);
        let px = dec.decode().ok()?;
        let (w, h) = dec.dimensions()?;
        Self::from_rgba(w as u32, h as u32, &px)
    }

    /// Decode PNG, or JPEG with the `jpeg` feature, by their signatures.
    pub fn decode(bytes: &[u8]) -> Option<Image> {
        if bytes.starts_with(b"\x89PNG") {
            return Self::from_png(bytes);
        }
        #[cfg(feature = "jpeg")]
        if bytes.starts_with(&[0xFF, 0xD8]) {
            return Self::from_jpeg(bytes);
        }
        None
    }

    /// Read and decode an image file.
    pub fn open(path: impl AsRef<std::path::Path>) -> Option<Image> {
        Self::decode(&std::fs::read(path).ok()?)
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    /// Straight-alpha RGBA8 pixels (e.g. for the clipboard).
    pub fn to_rgba(&self) -> Vec<u8> {
        self.pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect()
    }

    /// Encode as PNG.
    pub fn to_png(&self) -> Option<Vec<u8>> {
        self.pixmap.encode_png().ok()
    }
}

/// A parsed SVG image.
#[cfg(feature = "svg")]
#[derive(Clone)]
pub struct Svg {
    id: u64,
    tree: Rc<resvg::usvg::Tree>,
}

#[cfg(feature = "svg")]
impl std::fmt::Debug for Svg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (w, h) = self.size();
        write!(f, "Svg({w}x{h})")
    }
}

#[cfg(feature = "svg")]
impl PartialEq for Svg {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
    }
}

#[cfg(feature = "svg")]
impl Svg {
    /// Parse SVG data (text or bytes). Text in it uses the system fonts.
    pub fn parse(data: impl AsRef<[u8]>) -> Option<Svg> {
        thread_local! {
            static FONTS: std::sync::Arc<resvg::usvg::fontdb::Database> = {
                let mut db = resvg::usvg::fontdb::Database::new();
                db.load_system_fonts();
                std::sync::Arc::new(db)
            };
        }
        let opts = resvg::usvg::Options { fontdb: FONTS.with(|f| f.clone()), ..Default::default() };
        let tree = resvg::usvg::Tree::from_data(data.as_ref(), &opts).ok()?;
        Some(Svg { id: next_id(), tree: Rc::new(tree) })
    }

    /// Read and parse an SVG file.
    pub fn open(path: impl AsRef<std::path::Path>) -> Option<Svg> {
        Self::parse(std::fs::read(path).ok()?)
    }

    /// The natural size (the SVG's width and height).
    pub fn size(&self) -> (f32, f32) {
        let s = self.tree.size();
        (s.width(), s.height())
    }
}

/// How an image fills its box (CSS `object-fit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Fit {
    /// Scaled to fit inside, keeping its aspect ratio (may leave bands).
    #[default]
    Contain,
    /// Scaled to cover the box, keeping its aspect ratio (cropped).
    Cover,
    /// Stretched to the box.
    Fill,
    /// Natural size, centered and clipped.
    None,
    /// Natural size, or smaller to fit (never enlarged).
    ScaleDown,
}

/// What an image element shows.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ImageSource {
    /// A decoded raster image.
    Raster(Image),
    /// A parsed SVG.
    #[cfg(feature = "svg")]
    Svg(Svg),
}

impl ImageSource {
    pub(crate) fn id(&self) -> u64 {
        match self {
            ImageSource::Raster(i) => i.id,
            #[cfg(feature = "svg")]
            ImageSource::Svg(s) => s.id,
        }
    }

    /// Natural size in logical px.
    pub(crate) fn size(&self) -> (f32, f32) {
        match self {
            ImageSource::Raster(i) => (i.width() as f32, i.height() as f32),
            #[cfg(feature = "svg")]
            ImageSource::Svg(s) => s.size(),
        }
    }
}

impl From<Image> for ImageSource {
    fn from(i: Image) -> Self {
        ImageSource::Raster(i)
    }
}

#[cfg(feature = "svg")]
impl From<Svg> for ImageSource {
    fn from(s: Svg) -> Self {
        ImageSource::Svg(s)
    }
}

/// Where the image lands in `rect` (logical px) and which part of the source
/// (in source units) it shows.
pub(crate) fn place(fit: Fit, natural: (f32, f32), rect: Rect) -> (Rect, Rect) {
    let (nw, nh) = (natural.0.max(1e-3), natural.1.max(1e-3));
    let full = Rect::new(0.0, 0.0, nw, nh);
    let centered = |w: f32, h: f32| Rect::new(rect.x + (rect.w - w) / 2.0, rect.y + (rect.h - h) / 2.0, w, h);
    match fit {
        Fit::Fill => (rect, full),
        Fit::Contain => {
            let k = (rect.w / nw).min(rect.h / nh);
            (centered(nw * k, nh * k), full)
        }
        Fit::ScaleDown => {
            let k = (rect.w / nw).min(rect.h / nh).min(1.0);
            (centered(nw * k, nh * k), full)
        }
        Fit::Cover => {
            let k = (rect.w / nw).max(rect.h / nh);
            let (cw, ch) = (rect.w / k, rect.h / k);
            (rect, Rect::new((nw - cw) / 2.0, (nh - ch) / 2.0, cw, ch))
        }
        Fit::None => {
            // The part of the natural-size image that falls inside the box.
            let dest = centered(nw, nh);
            let vis = dest.intersect(&rect);
            (vis, Rect::new(vis.x - dest.x, vis.y - dest.y, vis.w, vis.h))
        }
    }
}

/// A rasterized image: (source, crop, size, tint).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct RasterKey {
    id: u64,
    crop: [u32; 4],
    w: u32,
    h: u32,
    tint: Option<[u8; 4]>,
}

struct Cached {
    pixmap: Rc<Pixmap>,
    used: u64,
}

thread_local! {
    static CACHE: RefCell<(HashMap<RasterKey, Cached>, u64, usize)> = RefCell::new((HashMap::new(), 0, 0));
}

/// Rasters kept around (bytes), least recently used dropped first.
const CACHE_BYTES: usize = 96 << 20;

pub(crate) fn key(src: &ImageSource, crop: Rect, w: u32, h: u32, tint: Option<Color>) -> RasterKey {
    RasterKey {
        id: src.id(),
        crop: [crop.x.to_bits(), crop.y.to_bits(), crop.w.to_bits(), crop.h.to_bits()],
        w,
        h,
        tint: tint.map(|c| [c.r, c.g, c.b, c.a].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)),
    }
}

/// The `crop` part of `src` rendered at `w`×`h` device pixels (cached).
pub(crate) fn raster(src: &ImageSource, crop: Rect, w: u32, h: u32, tint: Option<Color>) -> Option<Rc<Pixmap>> {
    if w == 0 || h == 0 || w > 16384 || h > 16384 {
        return None;
    }
    let k = key(src, crop, w, h, tint);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.1 += 1;
        let now = c.1;
        if let Some(e) = c.0.get_mut(&k) {
            e.used = now;
            return Some(e.pixmap.clone());
        }
        let mut pm = render(src, crop, w, h)?;
        if let Some(t) = tint {
            // Monochrome tint (like an icon's currentColor): keep coverage,
            // take the color.
            for p in pm.pixels_mut() {
                let a = p.alpha() as f32 / 255.0 * t.a;
                let a8 = (a * 255.0).round() as u8;
                *p = tiny_skia::PremultipliedColorU8::from_rgba(
                    (t.r * a * 255.0).round() as u8,
                    (t.g * a * 255.0).round() as u8,
                    (t.b * a * 255.0).round() as u8,
                    a8,
                )
                .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
            }
        }
        let bytes = (w * h * 4) as usize;
        c.2 += bytes;
        let pm = Rc::new(pm);
        c.0.insert(k, Cached { pixmap: pm.clone(), used: now });
        // Evict the least recently used rasters over the budget.
        while c.2 > CACHE_BYTES && c.0.len() > 1 {
            let Some((&old, _)) = c.0.iter().filter(|(kk, _)| **kk != k).min_by_key(|(_, e)| e.used) else { break };
            if let Some(e) = c.0.remove(&old) {
                c.2 -= (e.pixmap.width() * e.pixmap.height() * 4) as usize;
            }
        }
        Some(pm)
    })
}

fn render(src: &ImageSource, crop: Rect, w: u32, h: u32) -> Option<Pixmap> {
    let mut out = Pixmap::new(w, h)?;
    match src {
        ImageSource::Raster(img) => {
            let mut from: Rc<Pixmap> = img.pixmap.clone();
            let (mut cx, mut cy, mut cw, mut ch) = (crop.x, crop.y, crop.w, crop.h);
            // Big reductions: halve first (bicubic alone would alias).
            while cw / w as f32 >= 2.0 && ch / h as f32 >= 2.0 {
                let (hw, hh) = ((from.width() / 2).max(1), (from.height() / 2).max(1));
                let mut half = Pixmap::new(hw, hh)?;
                let t = Transform::from_scale(hw as f32 / from.width() as f32, hh as f32 / from.height() as f32);
                let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
                half.draw_pixmap(0, 0, (*from).as_ref(), &paint, t, None);
                let k = hw as f32 / from.width() as f32;
                (cx, cy, cw, ch) = (cx * k, cy * k, cw * k, ch * k);
                from = Rc::new(half);
            }
            let t = Transform::from_translate(-cx, -cy).post_scale(w as f32 / cw, h as f32 / ch);
            let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() };
            out.draw_pixmap(0, 0, (*from).as_ref(), &paint, t, None);
        }
        #[cfg(feature = "svg")]
        ImageSource::Svg(svg) => {
            let t = Transform::from_translate(-crop.x, -crop.y).post_scale(w as f32 / crop.w, h as f32 / crop.h);
            resvg::render(&svg.tree, t, &mut out.as_mut());
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits() {
        let r = Rect::new(0.0, 0.0, 200.0, 100.0);
        let (d, c) = place(Fit::Contain, (100.0, 100.0), r);
        assert_eq!((d, c), (Rect::new(50.0, 0.0, 100.0, 100.0), Rect::new(0.0, 0.0, 100.0, 100.0)));
        let (d, c) = place(Fit::Cover, (100.0, 100.0), r);
        assert_eq!((d, c), (r, Rect::new(0.0, 25.0, 100.0, 50.0)));
        let (d, _) = place(Fit::ScaleDown, (50.0, 20.0), r);
        assert_eq!(d, Rect::new(75.0, 40.0, 50.0, 20.0));
        let (d, c) = place(Fit::None, (300.0, 50.0), r);
        assert_eq!((d, c), (Rect::new(0.0, 25.0, 200.0, 50.0), Rect::new(50.0, 0.0, 200.0, 50.0)));
    }

    #[test]
    fn decode_and_raster() {
        // 2×1: red, transparent.
        let img = Image::from_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 0, 0]).unwrap();
        let png = img.to_png().unwrap();
        let back = Image::decode(&png).unwrap();
        assert_eq!(back.to_rgba(), img.to_rgba());
        let src = ImageSource::Raster(img);
        let r = raster(&src, Rect::new(0.0, 0.0, 2.0, 1.0), 4, 2, None).unwrap();
        assert_eq!((r.width(), r.height()), (4, 2));
        assert!(r.pixel(0, 0).unwrap().red() > 200);
        // Cached: the same raster comes back.
        let again = raster(&src, Rect::new(0.0, 0.0, 2.0, 1.0), 4, 2, None).unwrap();
        assert!(Rc::ptr_eq(&r, &again));
    }
}
