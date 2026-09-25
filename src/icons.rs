//! Crisp vector icons.
//!
//! Built-in icons use a 24×24 stroke-based design language (Feather/Lucide
//! style). Any SVG path data can be used with [`Icon::svg`], so icon sets such
//! as Lucide or Feather can be pasted in directly.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use tiny_skia::{Path, PathBuilder};

/// An icon: either built-in or custom SVG path data in a 24×24 viewbox.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Icon {
    ChevronRight,
    ChevronDown,
    ChevronLeft,
    ChevronUp,
    Close,
    Minus,
    Plus,
    Check,
    Maximize,
    Restore,
    Menu,
    Search,
    File,
    Files,
    Folder,
    GitBranch,
    Settings,
    Terminal,
    Bell,
    User,
    SidebarLeft,
    SidebarRight,
    PanelBottom,
    Play,
    Code,
    More,
    Grid,
    Home,
    Star,
    Info,
    Warning,
    Layers,
    Sun,
    Moon,
    Blocks,
    Dot,
    Refresh,
    Columns,
    /// Custom SVG path data (24×24 viewbox), stroked.
    Svg(Rc<str>),
    /// Custom SVG path data (24×24 viewbox), filled.
    SvgFilled(Rc<str>),
}

impl Icon {
    /// A custom stroked icon from SVG path data in a 24×24 viewbox.
    pub fn svg(d: &str) -> Icon {
        Icon::Svg(d.into())
    }

    /// A custom filled icon from SVG path data in a 24×24 viewbox.
    pub fn svg_filled(d: &str) -> Icon {
        Icon::SvgFilled(d.into())
    }

    pub(crate) fn filled(&self) -> bool {
        matches!(self, Icon::Play | Icon::Dot | Icon::SvgFilled(_))
    }

    fn data(&self) -> &str {
        match self {
            Icon::ChevronRight => "M9 6l6 6-6 6",
            Icon::ChevronDown => "M6 9l6 6 6-6",
            Icon::ChevronLeft => "M15 6l-6 6 6 6",
            Icon::ChevronUp => "M18 15l-6-6-6 6",
            Icon::Close => "M18 6L6 18M6 6l12 12",
            Icon::Minus => "M5 12h14",
            Icon::Plus => "M12 5v14M5 12h14",
            Icon::Check => "M20 6L9 17l-5-5",
            Icon::Maximize => "M6 6h12v12H6z",
            Icon::Restore => "M8 9h10v10H8z M6 15V6a1 1 0 0 1 1-1h9",
            Icon::Menu => "M4 6h16M4 12h16M4 18h16",
            Icon::Search => "M4 11a7 7 0 1 0 14 0a7 7 0 1 0-14 0 M21 21l-4.35-4.35",
            Icon::File => "M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z M14 3v5h5",
            Icon::Files => "M15 3H10a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h7a2 2 0 0 0 2-2V7z M15 3v4h4 M4 8v11a2 2 0 0 0 2 2h9",
            Icon::Folder => "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
            Icon::GitBranch => "M6 3v12 M15 6a3 3 0 1 0 6 0a3 3 0 1 0-6 0 M3 18a3 3 0 1 0 6 0a3 3 0 1 0-6 0 M18 9a9 9 0 0 1-9 9",
            Icon::Settings => "M4 6h9 M17 6h3 M4 12h3 M11 12h9 M4 18h11 M19 18h1 M13 6a2 2 0 1 0 4 0a2 2 0 1 0-4 0 M7 12a2 2 0 1 0 4 0a2 2 0 1 0-4 0 M15 18a2 2 0 1 0 4 0a2 2 0 1 0-4 0",
            Icon::Terminal => "M4 17l6-6-6-6 M12 19h8",
            Icon::Bell => "M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9 M10.3 21a1.94 1.94 0 0 0 3.4 0",
            Icon::User => "M8 7a4 4 0 1 0 8 0a4 4 0 1 0-8 0 M5 21v-2a4 4 0 0 1 4-4h6a4 4 0 0 1 4 4v2",
            Icon::SidebarLeft => "M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z M9 3v18",
            Icon::SidebarRight => "M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z M15 3v18",
            Icon::PanelBottom => "M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z M3 15h18",
            Icon::Columns => "M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z M12 3v18",
            Icon::Play => "M7 4.5v15a1 1 0 0 0 1.5.86l12.5-7.5a1 1 0 0 0 0-1.72L8.5 3.64A1 1 0 0 0 7 4.5z",
            Icon::Code => "M16 18l6-6-6-6 M8 6l-6 6 6 6",
            Icon::More => "M4 12h.01 M12 12h.01 M20 12h.01",
            Icon::Grid => "M3 3h7v7H3z M14 3h7v7h-7z M14 14h7v7h-7z M3 14h7v7H3z",
            Icon::Home => "M3 10l9-7 9 7v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z M9 22V12h6v10",
            Icon::Star => "M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01z",
            Icon::Info => "M2 12a10 10 0 1 0 20 0a10 10 0 1 0-20 0 M12 16v-4 M12 8h.01",
            Icon::Warning => "M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z M12 9v4 M12 17h.01",
            Icon::Layers => "M12 2L2 7l10 5 10-5z M2 17l10 5 10-5 M2 12l10 5 10-5",
            Icon::Sun => "M8 12a4 4 0 1 0 8 0a4 4 0 1 0-8 0 M12 2v2 M12 20v2 M4.93 4.93l1.41 1.41 M17.66 17.66l1.41 1.41 M2 12h2 M20 12h2 M6.34 17.66l-1.41 1.41 M19.07 4.93l-1.41 1.41",
            Icon::Moon => "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z",
            Icon::Blocks => "M4 4h6v6H4z M14 4h6v6h-6z M4 14h6v6H4z M14 17h6 M17 14v6",
            Icon::Dot => "M8 12a4 4 0 1 0 8 0a4 4 0 1 0-8 0z",
            Icon::Refresh => "M21 12a9 9 0 1 1-2.64-6.36L21 8 M21 3v5h-5",
            Icon::Svg(d) | Icon::SvgFilled(d) => d,
        }
    }

    /// The icon outline as a path in a 24×24 coordinate space.
    pub(crate) fn path(&self) -> Option<Rc<Path>> {
        thread_local! {
            static CACHE: RefCell<HashMap<Icon, Option<Rc<Path>>>> = RefCell::new(HashMap::new());
        }
        CACHE.with(|c| {
            c.borrow_mut().entry(self.clone()).or_insert_with(|| parse_svg_path(self.data()).map(Rc::new)).clone()
        })
    }
}

/// Parse SVG path data (`M L H V C S Q T A Z`, absolute and relative).
pub fn parse_svg_path(d: &str) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let mut toks = Tokens { s: d.as_bytes(), i: 0 };
    let (mut cx, mut cy) = (0.0f32, 0.0f32);
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    let mut last_ctrl: Option<(f32, f32)> = None;
    let mut last_quad: Option<(f32, f32)> = None;
    let mut cmd = b'M';
    let mut first = true;
    loop {
        toks.skip_ws();
        if toks.i >= toks.s.len() {
            break;
        }
        let c = toks.s[toks.i];
        if c.is_ascii_alphabetic() {
            cmd = c;
            toks.i += 1;
        } else if first {
            return None;
        }
        first = false;
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (cx, cy) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let (x, y) = (toks.num()? + ox, toks.num()? + oy);
                pb.move_to(x, y);
                cx = x;
                cy = y;
                sx = x;
                sy = y;
                // Subsequent pairs are implicit line-tos.
                cmd = if rel { b'l' } else { b'L' };
                last_ctrl = None;
                last_quad = None;
                continue;
            }
            b'L' => {
                let (x, y) = (toks.num()? + ox, toks.num()? + oy);
                pb.line_to(x, y);
                cx = x;
                cy = y;
                last_ctrl = None;
                last_quad = None;
            }
            b'H' => {
                let x = toks.num()? + ox;
                pb.line_to(x, cy);
                cx = x;
                last_ctrl = None;
                last_quad = None;
            }
            b'V' => {
                let y = toks.num()? + oy;
                pb.line_to(cx, y);
                cy = y;
                last_ctrl = None;
                last_quad = None;
            }
            b'C' => {
                let x1 = toks.num()? + ox;
                let y1 = toks.num()? + oy;
                let x2 = toks.num()? + ox;
                let y2 = toks.num()? + oy;
                let x = toks.num()? + ox;
                let y = toks.num()? + oy;
                pb.cubic_to(x1, y1, x2, y2, x, y);
                last_ctrl = Some((x2, y2));
                last_quad = None;
                cx = x;
                cy = y;
            }
            b'S' => {
                let (x1, y1) = last_ctrl.map(|(lx, ly)| (2.0 * cx - lx, 2.0 * cy - ly)).unwrap_or((cx, cy));
                let x2 = toks.num()? + ox;
                let y2 = toks.num()? + oy;
                let x = toks.num()? + ox;
                let y = toks.num()? + oy;
                pb.cubic_to(x1, y1, x2, y2, x, y);
                last_ctrl = Some((x2, y2));
                last_quad = None;
                cx = x;
                cy = y;
            }
            b'Q' => {
                let x1 = toks.num()? + ox;
                let y1 = toks.num()? + oy;
                let x = toks.num()? + ox;
                let y = toks.num()? + oy;
                pb.quad_to(x1, y1, x, y);
                last_quad = Some((x1, y1));
                last_ctrl = None;
                cx = x;
                cy = y;
            }
            b'T' => {
                let (x1, y1) = last_quad.map(|(lx, ly)| (2.0 * cx - lx, 2.0 * cy - ly)).unwrap_or((cx, cy));
                let x = toks.num()? + ox;
                let y = toks.num()? + oy;
                pb.quad_to(x1, y1, x, y);
                last_quad = Some((x1, y1));
                last_ctrl = None;
                cx = x;
                cy = y;
            }
            b'A' => {
                let rx = toks.num()?;
                let ry = toks.num()?;
                let rot = toks.num()?;
                let large = toks.flag()?;
                let sweep = toks.flag()?;
                let x = toks.num()? + ox;
                let y = toks.num()? + oy;
                arc_to(&mut pb, cx, cy, rx, ry, rot, large, sweep, x, y);
                cx = x;
                cy = y;
                last_ctrl = None;
                last_quad = None;
            }
            b'Z' => {
                pb.close();
                cx = sx;
                cy = sy;
                last_ctrl = None;
                last_quad = None;
                // `Z` takes no arguments; require a new command letter next.
                toks.skip_ws();
                if toks.i < toks.s.len() && !toks.s[toks.i].is_ascii_alphabetic() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    pb.finish()
}

struct Tokens<'a> {
    s: &'a [u8],
    i: usize,
}

impl Tokens<'_> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i].is_ascii_whitespace() || self.s[self.i] == b',') {
            self.i += 1;
        }
    }
    fn flag(&mut self) -> Option<bool> {
        self.skip_ws();
        let c = *self.s.get(self.i)?;
        self.i += 1;
        match c {
            b'0' => Some(false),
            b'1' => Some(true),
            _ => None,
        }
    }
    fn num(&mut self) -> Option<f32> {
        self.skip_ws();
        let start = self.i;
        let s = self.s;
        if self.i < s.len() && (s[self.i] == b'-' || s[self.i] == b'+') {
            self.i += 1;
        }
        let mut seen_dot = false;
        let mut seen_digit = false;
        while self.i < s.len() {
            let c = s[self.i];
            if c.is_ascii_digit() {
                seen_digit = true;
                self.i += 1;
            } else if c == b'.' && !seen_dot {
                seen_dot = true;
                self.i += 1;
            } else if (c == b'e' || c == b'E') && seen_digit {
                self.i += 1;
                if self.i < s.len() && (s[self.i] == b'-' || s[self.i] == b'+') {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
        if !seen_digit {
            self.i = start;
            return None;
        }
        std::str::from_utf8(&s[start..self.i]).ok()?.parse().ok()
    }
}

/// Append an SVG elliptical arc as cubic Béziers (SVG spec, appendix F.6).
#[allow(clippy::too_many_arguments)]
fn arc_to(
    pb: &mut PathBuilder,
    x1: f32,
    y1: f32,
    rx: f32,
    ry: f32,
    rot_deg: f32,
    large: bool,
    sweep: bool,
    x2: f32,
    y2: f32,
) {
    if (x1 - x2).abs() < 1e-6 && (y1 - y2).abs() < 1e-6 {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-6 || ry < 1e-6 {
        pb.line_to(x2, y2);
        return;
    }
    let phi = rot_deg.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();
    let dx = (x1 - x2) / 2.0;
    let dy = (y1 - y2) / 2.0;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut coef = (num / den).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let cxp = coef * rx * y1p / ry;
    let cyp = -coef * ry * x1p / rx;
    let cx = cos_phi * cxp - sin_phi * cyp + (x1 + x2) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (y1 + y2) / 2.0;
    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| {
        let dot = ux * vx + uy * vy;
        let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let mut a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            a = -a;
        }
        a
    };
    let theta1 = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut dtheta = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
    if !sweep && dtheta > 0.0 {
        dtheta -= std::f32::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f32::consts::TAU;
    }
    let segs = (dtheta.abs() / (std::f32::consts::FRAC_PI_2)).ceil().max(1.0) as usize;
    let delta = dtheta / segs as f32;
    let t = 4.0 / 3.0 * (delta / 4.0).tan();
    let point = |a: f32| {
        let (s, c) = a.sin_cos();
        (cx + rx * c * cos_phi - ry * s * sin_phi, cy + rx * c * sin_phi + ry * s * cos_phi)
    };
    let deriv = |a: f32| {
        let (s, c) = a.sin_cos();
        (-rx * s * cos_phi - ry * c * sin_phi, -rx * s * sin_phi + ry * c * cos_phi)
    };
    let mut a = theta1;
    for _ in 0..segs {
        let b = a + delta;
        let (p0x, p0y) = point(a);
        let (d0x, d0y) = deriv(a);
        let (p1x, p1y) = point(b);
        let (d1x, d1y) = deriv(b);
        pb.cubic_to(p0x + t * d0x, p0y + t * d0y, p1x - t * d1x, p1y - t * d1y, p1x, p1y);
        a = b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtin_icons_parse() {
        let all = [
            Icon::ChevronRight,
            Icon::ChevronDown,
            Icon::ChevronLeft,
            Icon::ChevronUp,
            Icon::Close,
            Icon::Minus,
            Icon::Plus,
            Icon::Check,
            Icon::Maximize,
            Icon::Restore,
            Icon::Menu,
            Icon::Search,
            Icon::File,
            Icon::Files,
            Icon::Folder,
            Icon::GitBranch,
            Icon::Settings,
            Icon::Terminal,
            Icon::Bell,
            Icon::User,
            Icon::SidebarLeft,
            Icon::SidebarRight,
            Icon::PanelBottom,
            Icon::Play,
            Icon::Code,
            Icon::More,
            Icon::Grid,
            Icon::Home,
            Icon::Star,
            Icon::Info,
            Icon::Warning,
            Icon::Layers,
            Icon::Sun,
            Icon::Moon,
            Icon::Blocks,
            Icon::Dot,
            Icon::Refresh,
            Icon::Columns,
        ];
        for i in all {
            assert!(i.path().is_some(), "{i:?} failed to parse");
        }
    }

    #[test]
    fn custom_svg_path() {
        assert!(Icon::svg("M3 3l18 18").path().is_some());
        assert!(Icon::svg("garbage").path().is_none());
    }
}
