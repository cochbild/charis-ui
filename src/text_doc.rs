//! Layout of multi-line text inputs by paragraph, so large documents stay
//! fast.
//!
//! The value is split at `\n` into paragraphs. Each paragraph is shaped on
//! its own (through the regular text cache) and only when needed: when it's
//! on screen, holds the caret, or is hit-tested. Paragraphs that haven't been
//! measured use an estimated height. A prefix sum of heights maps between
//! document y and paragraphs. When the value changes, the common prefix and
//! suffix with the previous value keep the heights of untouched paragraphs,
//! so typing in a 100k-line document re-measures one paragraph.

use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::fxhash::FxHasher;
use crate::geometry::Rect;
use crate::text::{TextStyle, TextSystem};

/// An IME composition shown inside a document: `text` inserted at byte `at`
/// of the value.
#[derive(Clone, PartialEq, Default, Debug)]
pub(crate) struct DocPreedit {
    pub at: usize,
    pub text: String,
}

/// What a document query is about.
pub(crate) struct DocQuery<'a> {
    /// The input's node id (one layout per input).
    pub id: u64,
    pub text: &'a Rc<str>,
    pub style: &'a TextStyle,
    /// Wrap width (logical px).
    pub width: f32,
    pub scale: f32,
    pub preedit: Option<&'a DocPreedit>,
}

/// The per-input paragraph layout.
#[derive(Default)]
pub(crate) struct Doc {
    text: Rc<str>,
    /// Style, scale and width the heights were measured for.
    key: u64,
    preedit: Option<DocPreedit>,
    /// Start byte of each paragraph.
    starts: Vec<usize>,
    /// Height of each paragraph (logical px); NaN = not measured yet.
    heights: Vec<f32>,
    /// `ys[i]` = top of paragraph `i`; `ys[n]` = total height.
    ys: Vec<f32>,
    ys_dirty: bool,
    line_h: f32,
    /// Estimated logical px per byte of text, for unmeasured paragraphs.
    px_per_byte: f32,
    width: f32,
}

pub(crate) fn common_prefix(a: &[u8], b: &[u8]) -> usize {
    let n = a.len().min(b.len());
    let mut i = 0;
    while i + 64 <= n && a[i..i + 64] == b[i..i + 64] {
        i += 64;
    }
    while i < n && a[i] == b[i] {
        i += 1;
    }
    i
}

pub(crate) fn common_suffix(a: &[u8], b: &[u8], max: usize) -> usize {
    let (la, lb) = (a.len(), b.len());
    let mut i = 0;
    while i + 64 <= max && a[la - i - 64..la - i] == b[lb - i - 64..lb - i] {
        i += 64;
    }
    while i < max && a[la - i - 1] == b[lb - i - 1] {
        i += 1;
    }
    i
}

/// Start of every paragraph of `text[from..to]`, where `from` starts one;
/// `last` also counts a newline right before `to` (the text's end).
fn scan_starts(text: &[u8], from: usize, to: usize, last: bool, out: &mut Vec<usize>) {
    out.push(from);
    for (j, &c) in text[from..to].iter().enumerate() {
        if c == b'\n' {
            let s = from + j + 1;
            if s < to || last {
                out.push(s);
            }
        }
    }
}

impl Doc {
    fn n(&self) -> usize {
        self.starts.len()
    }

    /// Byte range of paragraph `i` in the value (without its newline).
    fn range(&self, i: usize) -> (usize, usize) {
        let s = self.starts[i];
        let e = self.starts.get(i + 1).map_or(self.text.len(), |&n| n - 1);
        (s, e)
    }

    /// Paragraph `i`'s text, with the IME composition spliced in.
    fn para(&self, i: usize) -> Cow<'_, str> {
        let (s, e) = self.range(i);
        let t = &self.text[s..e];
        match &self.preedit {
            Some(p) if p.at >= s && p.at <= e => {
                let k = p.at - s;
                Cow::Owned(format!("{}{}{}", &t[..k], p.text, &t[k..]))
            }
            _ => Cow::Borrowed(t),
        }
    }

    /// Paragraph holding value byte `b`.
    fn para_of(&self, b: usize) -> usize {
        self.starts.partition_point(|&s| s <= b).saturating_sub(1)
    }

    /// Byte `b` of the value as an offset in its paragraph's (composed) text.
    fn local(&self, i: usize, b: usize) -> usize {
        let s = self.starts[i];
        match &self.preedit {
            Some(p) if b > p.at && self.para_of(p.at) == i => b - s + p.text.len(),
            _ => b - s,
        }
    }

    /// A local offset of paragraph `i` back to a value byte (a point inside
    /// the composition maps to where it's inserted).
    fn global(&self, i: usize, local: usize) -> usize {
        let s = self.starts[i];
        match &self.preedit {
            Some(p) if self.para_of(p.at) == i => {
                let k = p.at - s;
                if local <= k {
                    s + local
                } else if local <= k + p.text.len() {
                    p.at
                } else {
                    s + local - p.text.len()
                }
            }
            _ => s + local,
        }
    }

    fn estimate(&self, i: usize) -> f32 {
        let (s, e) = self.range(i);
        let px = (e - s) as f32 * self.px_per_byte;
        let lines = (px / self.width.max(1.0)).ceil().max(1.0);
        lines * self.line_h
    }

    fn height(&self, i: usize) -> f32 {
        let h = self.heights[i];
        if h.is_nan() {
            self.estimate(i)
        } else {
            h
        }
    }

    fn rebuild_ys(&mut self) {
        if !self.ys_dirty && self.ys.len() == self.n() + 1 {
            return;
        }
        self.ys.clear();
        self.ys.reserve(self.n() + 1);
        let mut y = 0.0;
        for i in 0..self.n() {
            self.ys.push(y);
            y += self.height(i);
        }
        self.ys.push(y);
        self.ys_dirty = false;
    }

    fn set_text(&mut self, new: &Rc<str>) {
        if self.starts.is_empty() {
            self.text = new.clone();
            scan_starts(new.as_bytes(), 0, new.len(), true, &mut self.starts);
            self.heights = vec![f32::NAN; self.n()];
            self.ys_dirty = true;
            return;
        }
        let (old, newb) = (self.text.as_bytes(), new.as_bytes());
        let p = common_prefix(old, newb);
        let s = common_suffix(old, newb, old.len().min(newb.len()) - p);
        let old_len = old.len();
        let delta = new.len() as isize - old_len as isize;
        // Paragraphs whose text and newline lie in the common prefix, and
        // those whose preceding newline and text lie in the common suffix.
        let a = self.starts.partition_point(|&x| x <= p).saturating_sub(1);
        let b = (a + 1..self.n()).find(|&i| self.starts[i] > old_len - s).unwrap_or(self.n());
        let mid_end = if b < self.n() { (self.starts[b] as isize + delta) as usize } else { new.len() };
        let mut starts = Vec::with_capacity(self.n() + 8);
        starts.extend_from_slice(&self.starts[..a]);
        scan_starts(newb, self.starts[a], mid_end, b == self.n(), &mut starts);
        let mid = starts.len() - a;
        starts.extend(self.starts[b..].iter().map(|&x| (x as isize + delta) as usize));
        let mut heights = Vec::with_capacity(starts.len());
        heights.extend_from_slice(&self.heights[..a]);
        heights.extend(std::iter::repeat_n(f32::NAN, mid));
        heights.extend_from_slice(&self.heights[b..]);
        self.starts = starts;
        self.heights = heights;
        self.text = new.clone();
        self.ys_dirty = true;
    }
}

fn style_key(st: &TextStyle, scale: f32, width: f32) -> u64 {
    let mut h = FxHasher::default();
    (st.size.to_bits(), st.weight, &st.family, st.italic, st.line_height.to_bits(), st.letter_spacing.to_bits())
        .hash(&mut h);
    (scale.to_bits(), width.to_bits()).hash(&mut h);
    h.finish()
}

impl TextSystem {
    /// Run `f` on the input's document layout, brought up to date with `q`.
    fn with_doc<R>(&mut self, q: &DocQuery, f: impl FnOnce(&mut TextSystem, &mut Doc) -> R) -> R {
        let mut doc = self.docs.remove(&q.id).unwrap_or_default();
        if doc.starts.is_empty() {
            doc.set_text(q.text);
        } else if !Rc::ptr_eq(&doc.text, q.text) {
            if *doc.text == **q.text {
                // Same text in a new allocation: later queries compare pointers.
                doc.text = q.text.clone();
            } else {
                doc.set_text(q.text);
            }
        }
        let key = style_key(q.style, q.scale, q.width);
        if doc.key != key {
            doc.key = key;
            doc.heights.iter_mut().for_each(|h| *h = f32::NAN);
            doc.line_h = q.style.size * q.style.line_height;
            doc.px_per_byte = q.style.size * 0.55;
            doc.width = q.width;
            doc.ys_dirty = true;
        }
        let pre = q.preedit.filter(|p| p.at <= q.text.len() && q.text.is_char_boundary(p.at)).cloned();
        if doc.preedit != pre {
            for p in [&doc.preedit, &pre].into_iter().flatten() {
                let i = doc.para_of(p.at);
                doc.heights[i] = f32::NAN;
            }
            doc.preedit = pre;
            doc.ys_dirty = true;
        }
        let r = f(self, &mut doc);
        self.docs.insert(q.id, doc);
        r
    }

    fn doc_measure(&mut self, doc: &mut Doc, i: usize, scale: f32, st: &TextStyle) {
        if !doc.heights[i].is_nan() {
            return;
        }
        let t = doc.para(i);
        let h = if t.is_empty() { doc.line_h } else { self.measure(&t, st, Some(doc.width.max(1.0)), scale).h };
        doc.heights[i] = h;
        doc.ys_dirty = true;
    }

    /// Measure the paragraphs covering document y `y0..y1`.
    fn doc_ensure(&mut self, doc: &mut Doc, q: &DocQuery, y0: f32, y1: f32) {
        doc.rebuild_ys();
        let mut i = doc.ys.partition_point(|&y| y <= y0.max(0.0)).saturating_sub(1).min(doc.n() - 1);
        let mut y = doc.ys[i];
        while i < doc.n() && y < y1 {
            self.doc_measure(doc, i, q.scale, q.style);
            y += doc.heights[i];
            i += 1;
        }
        doc.rebuild_ys();
    }

    /// Total height of the document (logical px), with estimates for
    /// paragraphs not measured yet.
    pub(crate) fn doc_height(&mut self, q: &DocQuery) -> f32 {
        self.with_doc(q, |_, doc| {
            doc.rebuild_ys();
            doc.ys[doc.n()]
        })
    }

    /// The paragraph at document y, and y's offset into it.
    pub(crate) fn doc_anchor(&mut self, q: &DocQuery, y: f32) -> (usize, f32) {
        self.with_doc(q, |_, doc| {
            doc.rebuild_ys();
            let i = doc.ys.partition_point(|&v| v <= y.max(0.0)).saturating_sub(1).min(doc.n() - 1);
            (i, y - doc.ys[i])
        })
    }

    /// Top of paragraph `i` (logical px).
    pub(crate) fn doc_para_y(&mut self, q: &DocQuery, i: usize) -> f32 {
        self.with_doc(q, |_, doc| {
            doc.rebuild_ys();
            doc.ys[i.min(doc.n())]
        })
    }

    /// Measure what's visible in `y0..y1` so later queries see real heights.
    pub(crate) fn doc_prepare(&mut self, q: &DocQuery, y0: f32, y1: f32) {
        self.with_doc(q, |ts, doc| ts.doc_ensure(doc, q, y0, y1));
    }

    /// Caret rectangle for value byte `b` (plus `extra` bytes into the
    /// composition when it sits there), in document coordinates.
    pub(crate) fn doc_caret_rect(&mut self, q: &DocQuery, b: usize, extra: usize) -> Rect {
        self.with_doc(q, |ts, doc| {
            let b = b.min(doc.text.len());
            let i = doc.para_of(b);
            ts.doc_measure(doc, i, q.scale, q.style);
            doc.rebuild_ys();
            let local = doc.local(i, b) + extra;
            let t = doc.para(i).into_owned();
            let r = ts.caret_rect(&t, q.style, Some(doc.width.max(1.0)), q.scale, local.min(t.len()));
            r.translate(0.0, doc.ys[i])
        })
    }

    /// Value byte nearest to document point `(x, y)`.
    pub(crate) fn doc_hit(&mut self, q: &DocQuery, x: f32, y: f32) -> usize {
        self.with_doc(q, |ts, doc| {
            ts.doc_ensure(doc, q, y, y + 1.0);
            let total = doc.ys[doc.n()];
            if y >= total {
                return doc.text.len();
            }
            let i = doc.ys.partition_point(|&v| v <= y.max(0.0)).saturating_sub(1).min(doc.n() - 1);
            let t = doc.para(i).into_owned();
            let local = ts.hit_byte(&t, None, q.style, Some(doc.width.max(1.0)), q.scale, x, y - doc.ys[i]);
            doc.global(i, local)
        })
    }

    /// The paragraphs visible in document y `y0..y1`: text and top.
    pub(crate) fn doc_visible(&mut self, q: &DocQuery, y0: f32, y1: f32) -> Vec<(String, f32)> {
        self.with_doc(q, |ts, doc| {
            ts.doc_ensure(doc, q, y0, y1);
            let mut out = Vec::new();
            let mut i = doc.ys.partition_point(|&v| v <= y0.max(0.0)).saturating_sub(1);
            while i < doc.n() && doc.ys[i] < y1 {
                out.push((doc.para(i).into_owned(), doc.ys[i]));
                i += 1;
            }
            out
        })
    }

    /// Rectangles covering value bytes `a..b` within document y `y0..y1`
    /// (a selected line break shows as a short tail, like browsers).
    pub(crate) fn doc_selection_rects(&mut self, q: &DocQuery, a: usize, b: usize, y0: f32, y1: f32) -> Vec<Rect> {
        self.doc_range_rects(q, a, b, y0, y1, true)
    }

    fn doc_range_rects(&mut self, q: &DocQuery, a: usize, b: usize, y0: f32, y1: f32, tails: bool) -> Vec<Rect> {
        self.with_doc(q, |ts, doc| {
            ts.doc_ensure(doc, q, y0, y1);
            let mut out = Vec::new();
            let first = doc.para_of(a).max(doc.ys.partition_point(|&v| v <= y0.max(0.0)).saturating_sub(1));
            let wrap = Some(doc.width.max(1.0));
            let mut i = first;
            while i < doc.n() && doc.ys[i] < y1 && doc.starts[i] <= b {
                let (s, e) = doc.range(i);
                let (la, lb) = (a.max(s), b.min(e));
                let t = doc.para(i).into_owned();
                let y = doc.ys[i];
                if la < lb {
                    let (ra, rb) = (doc.local(i, la), doc.local(i, lb));
                    for r in ts.selection_rects(&t, None, q.style, wrap, q.scale, ra, rb) {
                        out.push(r.translate(0.0, y));
                    }
                }
                if tails && a <= e && b > e && i + 1 < doc.n() {
                    let c = ts.caret_rect(&t, q.style, wrap, q.scale, t.len());
                    out.push(Rect::new(c.x, y + c.y, q.style.size * 0.3, c.h));
                }
                i += 1;
            }
            out
        })
    }

    /// Rectangles under the IME composition (document coordinates).
    pub(crate) fn doc_preedit_rects(&mut self, q: &DocQuery, y0: f32, y1: f32) -> Vec<Rect> {
        let Some(p) = q.preedit else { return Vec::new() };
        let (at, len) = (p.at, p.text.len());
        self.with_doc(q, |ts, doc| {
            ts.doc_ensure(doc, q, y0, y1);
            if doc.preedit.is_none() {
                return Vec::new();
            }
            let i = doc.para_of(at);
            let k = doc.local(i, at);
            let t = doc.para(i).into_owned();
            let y = doc.ys[i];
            ts.selection_rects(&t, None, q.style, Some(doc.width.max(1.0)), q.scale, k, k + len)
                .into_iter()
                .map(|r| r.translate(0.0, y))
                .collect()
        })
    }

    /// Height of the input's text up to `cap` (for sizing an input between
    /// its min and max rows); stops measuring once `cap` is reached.
    pub(crate) fn doc_height_capped(
        &mut self,
        value: &str,
        st: &TextStyle,
        width: Option<f32>,
        scale: f32,
        cap: f32,
    ) -> f32 {
        let lh = st.size * st.line_height;
        let mut h = 0.0;
        for para in value.split('\n') {
            h += if para.is_empty() { lh } else { self.measure(para, st, width, scale).h };
            if h >= cap {
                break;
            }
        }
        h
    }

    /// Drop the layouts of inputs that are gone.
    pub(crate) fn retain_docs(&mut self, keep: impl Fn(u64) -> bool) {
        self.docs.retain(|id, _| keep(*id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_starts(t: &str) -> Vec<usize> {
        let mut v = Vec::new();
        scan_starts(t.as_bytes(), 0, t.len(), true, &mut v);
        v
    }

    #[test]
    fn incremental_paragraph_starts_match_a_full_scan() {
        // A small deterministic PRNG, so the test needs no dependency.
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n.max(1) as u64) as usize
        };
        let pieces = ["a", "bc", "\n", "\n\n", "xyz\n", "é", "", "line\nline"];
        let mut text = String::from("one\ntwo\nthree\n\nfive");
        let mut doc = Doc::default();
        doc.set_text(&Rc::from(text.as_str()));
        for _ in 0..3000 {
            let a = rnd(text.len() + 1);
            let b = (a + rnd(6)).min(text.len());
            let (a, b) = {
                let mut a = a;
                let mut b = b;
                while !text.is_char_boundary(a) {
                    a -= 1;
                }
                while !text.is_char_boundary(b) {
                    b += 1;
                }
                (a, b)
            };
            let ins = pieces[rnd(pieces.len())];
            text.replace_range(a..b, ins);
            // Measure a few paragraphs, so kept heights can be checked.
            let n = doc.n();
            for (i, h) in doc.heights.iter_mut().enumerate() {
                if i % 3 == 0 && h.is_nan() && i < n {
                    *h = i as f32;
                }
            }
            let before: Vec<(String, f32)> = (0..doc.n()).map(|i| (doc.para(i).into_owned(), doc.heights[i])).collect();
            doc.set_text(&Rc::from(text.as_str()));
            assert_eq!(doc.starts, full_starts(&text), "after replacing {a}..{b} with {ins:?} in {text:?}");
            assert_eq!(doc.heights.len(), doc.starts.len());
            // Every kept height belongs to a paragraph with the same text as before.
            for i in 0..doc.n() {
                if !doc.heights[i].is_nan() {
                    let p = doc.para(i).into_owned();
                    assert!(before.iter().any(|(t, h)| *t == p && *h == doc.heights[i]), "stale height for {p:?}");
                }
            }
        }
    }
}
