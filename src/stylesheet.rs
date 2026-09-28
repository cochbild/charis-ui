//! Stylesheets: restyle widgets from a CSS-like text file, reloaded while
//! the app runs.
//!
//! Rules target the style classes widgets tag themselves with (`button`,
//! `button-primary`, `card`, `input`, `tab-active`, `tree-row`… and your
//! own, see [`Element::class`](crate::Element::class)), optionally in a
//! state (`:hover`, `:active`, `:focus`):
//!
//! ```css
//! /* Rounder buttons, a flat card. */
//! .button { radius: 999; padding: 6 18; }
//! .button:hover { background: accent-soft; }
//! .card, .panel { background: #1b1d24; border: 1 border-strong; shadow: none; }
//! .tree-row { font-size: 13; }
//! ```
//!
//! Colors are `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(…)`, `rgba(…)`,
//! `transparent`, or a theme color by name (`accent`, `text-muted`,
//! `surface`, …), optionally with an alpha: `accent / 0.3`. Lengths are
//! logical pixels (`12` or `12px`).
//!
//! Properties: `background`, `color`, `border` (width and color),
//! `border-width`, `border-color`, `radius`, `padding`, `margin`
//! (1–4 values, CSS order), `gap`, `width`, `height`, `min-width`,
//! `max-width`, `min-height`, `max-height`, `font-size`, `font-weight`,
//! `line-height`, `letter-spacing`, `opacity`, `shadow` (`x y blur
//! [spread] color`, or `none`), `transition` (seconds). In a state rule:
//! `background`, `color`, `border-color`, `radius`, `opacity`, `shadow`.
//!
//! Apply one with [`Stylesheet::apply`] on your theme, or give the window a
//! file to watch with
//! [`WindowOptions::stylesheet`](crate::WindowOptions::stylesheet): every
//! save restyles the running app (parse errors are printed, and the last
//! good version stays).

use crate::color::Color;
use crate::element::Element;
use crate::style::{Corners, Edges, Shadow, StylePatch, Weight};
use crate::theme::{theme, Palette, Theme};

/// A parse error, with its 1-based line.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleError {
    /// Line of the error, starting at 1.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

impl std::fmt::Display for StyleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Normal,
    Hover,
    Active,
    Focus,
}

/// A color, possibly from the theme (resolved when the style applies).
#[derive(Debug, Clone, PartialEq)]
enum ColorRef {
    Literal(Color),
    Theme(String, f32),
}

impl ColorRef {
    fn resolve(&self) -> Color {
        match self {
            ColorRef::Literal(c) => *c,
            ColorRef::Theme(name, a) => {
                let c = palette_color(&theme().colors, name).unwrap_or(Color::TRANSPARENT);
                c.with_alpha(c.a * a)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Decl {
    Background(ColorRef),
    Color(ColorRef),
    Border(f32, ColorRef),
    BorderWidth(Edges),
    BorderColor(ColorRef),
    Radius(f32),
    Padding(Edges),
    Margin(Edges),
    Gap(f32),
    Width(f32),
    Height(f32),
    MinWidth(f32),
    MaxWidth(f32),
    MinHeight(f32),
    MaxHeight(f32),
    FontSize(f32),
    FontWeight(u16),
    LineHeight(f32),
    LetterSpacing(f32),
    Opacity(f32),
    Shadows(Vec<(f32, f32, f32, f32, ColorRef)>),
    Transition(f32),
}

#[derive(Debug, Clone, PartialEq)]
struct Rule {
    class: String,
    state: State,
    decls: Vec<Decl>,
}

/// A parsed stylesheet; see the [module docs](self).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stylesheet {
    rules: Vec<Rule>,
}

/// A theme color by its stylesheet name (`text-muted` or `text_muted`).
pub fn palette_color(p: &Palette, name: &str) -> Option<Color> {
    Some(match name.replace('-', "_").as_str() {
        "background" => p.background,
        "surface" => p.surface,
        "panel" => p.panel,
        "chrome" => p.chrome,
        "elevated" => p.elevated,
        "input" => p.input,
        "border" => p.border,
        "border_strong" => p.border_strong,
        "text" => p.text,
        "text_muted" => p.text_muted,
        "text_faint" => p.text_faint,
        "accent" => p.accent,
        "accent_hover" => p.accent_hover,
        "accent_text" => p.accent_text,
        "accent_soft" => p.accent_soft,
        "danger" => p.danger,
        "success" => p.success,
        "warning" => p.warning,
        "hover" => p.hover,
        "pressed" => p.pressed,
        "selection" => p.selection,
        "focus_ring" => p.focus_ring,
        "scrollbar" => p.scrollbar,
        "status_bar" => p.status_bar,
        "status_text" => p.status_text,
        "code_text" => p.code_text,
        "danger_text" => p.danger_text,
        _ => return None,
    })
}

fn parse_color(v: &str) -> Result<ColorRef, String> {
    let v = v.trim();
    // "accent / 0.3": a theme color with an alpha.
    if let Some((name, a)) = v.split_once('/') {
        let a: f32 = a.trim().trim_end_matches('%').parse().map_err(|_| format!("bad alpha in {v:?}"))?;
        let a = if v.trim_end().ends_with('%') { a / 100.0 } else { a };
        return match parse_color(name)? {
            ColorRef::Theme(n, _) => Ok(ColorRef::Theme(n, a.clamp(0.0, 1.0))),
            ColorRef::Literal(c) => Ok(ColorRef::Literal(c.with_alpha(c.a * a.clamp(0.0, 1.0)))),
        };
    }
    if v == "transparent" || v == "none" {
        return Ok(ColorRef::Literal(Color::TRANSPARENT));
    }
    if let Some(h) = v.strip_prefix('#') {
        let hex = |s: &str| u8::from_str_radix(s, 16).map_err(|_| format!("bad color {v:?}"));
        let ch = |i: usize| hex(&h[i..i + 1].repeat(2));
        let (r, g, b, a) = match h.len() {
            3 => (ch(0)?, ch(1)?, ch(2)?, 255),
            4 => (ch(0)?, ch(1)?, ch(2)?, ch(3)?),
            6 => (hex(&h[0..2])?, hex(&h[2..4])?, hex(&h[4..6])?, 255),
            8 => (hex(&h[0..2])?, hex(&h[2..4])?, hex(&h[4..6])?, hex(&h[6..8])?),
            _ => return Err(format!("bad color {v:?}")),
        };
        return Ok(ColorRef::Literal(Color::rgba(r, g, b, a as f32 / 255.0)));
    }
    for (prefix, n) in [("rgba(", 4), ("rgb(", 3)] {
        if let Some(inner) = v.strip_prefix(prefix).and_then(|x| x.strip_suffix(')')) {
            let parts: Vec<f32> = inner
                .split(',')
                .map(|p| p.trim().parse::<f32>())
                .collect::<Result<_, _>>()
                .map_err(|_| format!("bad color {v:?}"))?;
            if parts.len() != n {
                return Err(format!("bad color {v:?}"));
            }
            let a = if n == 4 { parts[3] } else { 1.0 };
            let c = |x: f32| x.clamp(0.0, 255.0) as u8;
            return Ok(ColorRef::Literal(Color::rgba(c(parts[0]), c(parts[1]), c(parts[2]), a)));
        }
    }
    if palette_color(&crate::theme::Theme::dark().colors, v).is_some() {
        return Ok(ColorRef::Theme(v.to_string(), 1.0));
    }
    Err(format!("unknown color {v:?}"))
}

fn parse_len(v: &str) -> Result<f32, String> {
    v.trim().trim_end_matches("px").parse::<f32>().map_err(|_| format!("bad length {v:?}"))
}

fn parse_edges(v: &str) -> Result<Edges, String> {
    let n: Vec<f32> = v.split_whitespace().map(parse_len).collect::<Result<_, _>>()?;
    let (t, r, b, l) = match n.as_slice() {
        [a] => (*a, *a, *a, *a),
        [v, h] => (*v, *h, *v, *h),
        [t, h, b] => (*t, *h, *b, *h),
        [t, r, b, l] => (*t, *r, *b, *l),
        _ => return Err(format!("expected 1 to 4 lengths, got {v:?}")),
    };
    Ok(Edges { top: t, right: r, bottom: b, left: l })
}

fn parse_decl(name: &str, v: &str) -> Result<Decl, String> {
    Ok(match name {
        "background" | "bg" => Decl::Background(parse_color(v)?),
        "color" => Decl::Color(parse_color(v)?),
        "border" => {
            let mut it = v.trim().splitn(2, char::is_whitespace);
            let w = parse_len(it.next().unwrap_or(""))?;
            let c = parse_color(it.next().unwrap_or("border"))?;
            Decl::Border(w, c)
        }
        "border-width" => Decl::BorderWidth(parse_edges(v)?),
        "border-color" => Decl::BorderColor(parse_color(v)?),
        "radius" | "border-radius" => Decl::Radius(parse_len(v)?),
        "padding" => Decl::Padding(parse_edges(v)?),
        "margin" => Decl::Margin(parse_edges(v)?),
        "gap" => Decl::Gap(parse_len(v)?),
        "width" => Decl::Width(parse_len(v)?),
        "height" => Decl::Height(parse_len(v)?),
        "min-width" => Decl::MinWidth(parse_len(v)?),
        "max-width" => Decl::MaxWidth(parse_len(v)?),
        "min-height" => Decl::MinHeight(parse_len(v)?),
        "max-height" => Decl::MaxHeight(parse_len(v)?),
        "font-size" => Decl::FontSize(parse_len(v)?),
        "font-weight" => Decl::FontWeight(match v.trim() {
            "normal" => 400,
            "bold" => 700,
            n => n.parse().map_err(|_| format!("bad font weight {v:?}"))?,
        }),
        "line-height" => Decl::LineHeight(v.trim().parse().map_err(|_| format!("bad line height {v:?}"))?),
        "letter-spacing" => Decl::LetterSpacing(parse_len(v)?),
        "opacity" => Decl::Opacity(v.trim().parse().map_err(|_| format!("bad opacity {v:?}"))?),
        "transition" => {
            Decl::Transition(v.trim().trim_end_matches('s').parse().map_err(|_| format!("bad duration {v:?}"))?)
        }
        "shadow" | "box-shadow" => {
            if v.trim() == "none" {
                Decl::Shadows(Vec::new())
            } else {
                let mut out = Vec::new();
                for part in v.split(',').filter(|p| !p.trim().is_empty()) {
                    let words: Vec<&str> = part.split_whitespace().collect();
                    let (nums, color) = match words.as_slice() {
                        [x, y, b, c] => (vec![*x, *y, *b, "0"], *c),
                        [x, y, b, s, c] => (vec![*x, *y, *b, *s], *c),
                        _ => return Err(format!("shadow is \"x y blur [spread] color\", got {part:?}")),
                    };
                    let n: Vec<f32> = nums.into_iter().map(parse_len).collect::<Result<_, _>>()?;
                    out.push((n[0], n[1], n[2], n[3], parse_color(color)?));
                }
                Decl::Shadows(out)
            }
        }
        other => return Err(format!("unknown property {other:?}")),
    })
}

fn state_ok(d: &Decl) -> bool {
    matches!(
        d,
        Decl::Background(_)
            | Decl::Color(_)
            | Decl::BorderColor(_)
            | Decl::Radius(_)
            | Decl::Opacity(_)
            | Decl::Shadows(_)
    )
}

impl Stylesheet {
    /// Parse a stylesheet. Rules and declarations with errors are skipped
    /// (and reported); the rest applies.
    pub fn parse(text: &str) -> (Stylesheet, Vec<StyleError>) {
        let mut rules = Vec::new();
        let mut errors = Vec::new();
        // Strip comments, keeping line numbers.
        let mut src = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '/' && chars.peek() == Some(&'*') {
                chars.next();
                let mut prev = ' ';
                for c in chars.by_ref() {
                    if c == '\n' {
                        src.push('\n');
                    }
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            } else {
                src.push(c);
            }
        }
        let mut rest = src.as_str();
        let mut line = 1;
        while let Some(open) = rest.find('{') {
            let selector = &rest[..open];
            let sel_line = line + selector.trim_end().matches('\n').count() - selector.trim().matches('\n').count();
            line += selector.matches('\n').count();
            let Some(close) = rest[open..].find('}') else {
                errors.push(StyleError { line, message: "missing '}'".into() });
                break;
            };
            let body = &rest[open + 1..open + close];
            let body_line = line;
            line += body.matches('\n').count();
            rest = &rest[open + close + 1..];
            // Declarations.
            let mut decls: Vec<(usize, Decl)> = Vec::new();
            let mut offset = 0;
            for d in body.split(';') {
                // The line where the declaration's text starts.
                let lead = d.len() - d.trim_start().len();
                let dl = body_line + body[..offset + lead].matches('\n').count();
                offset += d.len() + 1;
                let d = d.trim();
                if d.is_empty() {
                    continue;
                }
                let Some((name, value)) = d.split_once(':') else {
                    errors.push(StyleError { line: dl, message: format!("expected \"property: value\", got {d:?}") });
                    continue;
                };
                match parse_decl(name.trim(), value) {
                    Ok(decl) => decls.push((dl, decl)),
                    Err(message) => errors.push(StyleError { line: dl, message }),
                }
            }
            for sel in selector.split(',') {
                let sel = sel.trim();
                let Some(s) = sel.strip_prefix('.') else {
                    errors.push(StyleError {
                        line: sel_line,
                        message: format!("selector {sel:?} should be a class, like .button"),
                    });
                    continue;
                };
                let (class, state) = match s.split_once(':') {
                    None => (s, State::Normal),
                    Some((c, "hover")) => (c, State::Hover),
                    Some((c, "active")) => (c, State::Active),
                    Some((c, "focus")) => (c, State::Focus),
                    Some((_, st)) => {
                        errors.push(StyleError { line: sel_line, message: format!("unknown state :{st}") });
                        continue;
                    }
                };
                let mut kept = Vec::new();
                for (dl, d) in &decls {
                    if state != State::Normal && !state_ok(d) {
                        errors.push(StyleError {
                            line: *dl,
                            message: "only colors, radius, opacity and shadow can change in a state".into(),
                        });
                    } else {
                        kept.push(d.clone());
                    }
                }
                rules.push(Rule { class: class.to_string(), state, decls: kept });
            }
        }
        if !rest.trim().is_empty() {
            errors.push(StyleError { line, message: format!("unexpected {:?}", rest.trim()) });
        }
        (Stylesheet { rules }, errors)
    }

    /// Read and parse a stylesheet file.
    pub fn load(path: impl AsRef<std::path::Path>) -> std::io::Result<(Stylesheet, Vec<StyleError>)> {
        Ok(Self::parse(&std::fs::read_to_string(path)?))
    }

    /// The class names it styles.
    pub fn classes(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.rules.iter().map(|r| r.class.as_str()).collect();
        v.dedup();
        v
    }

    /// `theme` with this stylesheet's rules added as style classes (after
    /// the theme's own).
    pub fn apply(&self, mut theme: Theme) -> Theme {
        for rule in &self.rules {
            let rule = rule.clone();
            theme = theme.style_class(rule.class.clone(), move |e| apply_rule(e, &rule));
        }
        theme
    }
}

fn apply_rule(mut e: Element<()>, rule: &Rule) -> Element<()> {
    let patch = |mut p: StylePatch| {
        for d in &rule.decls {
            p = match d {
                Decl::Background(c) => p.bg(c.resolve()),
                Decl::Color(c) => p.color(c.resolve()),
                Decl::BorderColor(c) => p.border_color(c.resolve()),
                Decl::Radius(r) => {
                    p.radius = Some(Corners::all(*r));
                    p
                }
                Decl::Opacity(o) => p.opacity(*o),
                Decl::Shadows(list) => {
                    let mut p = p.no_shadow();
                    for (x, y, blur, spread, c) in list {
                        p = p.shadow(Shadow { x: *x, y: *y, blur: *blur, spread: *spread, color: c.resolve() });
                    }
                    p
                }
                _ => p,
            };
        }
        p
    };
    match rule.state {
        State::Hover => return e.hover(patch),
        State::Active => return e.active(patch),
        State::Focus => return e.focus_style(patch),
        State::Normal => {}
    }
    for d in &rule.decls {
        e = match d {
            Decl::Background(c) => e.bg(c.resolve()),
            Decl::Color(c) => e.color(c.resolve()),
            Decl::Border(w, c) => e.border(*w, c.resolve()),
            Decl::BorderWidth(w) => {
                e.style.border_width = *w;
                e
            }
            Decl::BorderColor(c) => {
                e.style.border_color = c.resolve();
                e
            }
            Decl::Radius(r) => e.rounded(*r),
            Decl::Padding(p) => {
                e.style.padding = *p;
                e
            }
            Decl::Margin(m) => {
                e.style.margin = *m;
                e
            }
            Decl::Gap(g) => e.gap(*g),
            Decl::Width(w) => e.w(*w),
            Decl::Height(h) => e.h(*h),
            Decl::MinWidth(v) => e.min_w(*v),
            Decl::MaxWidth(v) => e.max_w(*v),
            Decl::MinHeight(v) => e.min_h(*v),
            Decl::MaxHeight(v) => e.max_h(*v),
            Decl::FontSize(v) => e.font_size(*v),
            Decl::FontWeight(w) => e.weight(Weight(*w)),
            Decl::LineHeight(v) => e.line_height(*v),
            Decl::LetterSpacing(v) => {
                e.style.letter_spacing = Some(*v);
                e
            }
            Decl::Opacity(o) => e.opacity(*o),
            Decl::Shadows(list) => {
                e.style.shadows = list
                    .iter()
                    .map(|(x, y, blur, spread, c)| Shadow {
                        x: *x,
                        y: *y,
                        blur: *blur,
                        spread: *spread,
                        color: c.resolve(),
                    })
                    .collect();
                e
            }
            Decl::Transition(t) => e.transition(*t),
        };
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rules_and_reports_errors() {
        let (sheet, errors) = Stylesheet::parse(
            "/* comment */\n.button { radius: 999; padding: 6 18; }\n.button:hover, .tab:hover { background: accent / 0.5; }\n.card {\n  bakground: red;\n  shadow: 0 4 12 #0006;\n}\n.x:hover { padding: 3; }\n",
        );
        assert_eq!(sheet.classes(), ["button", "tab", "card", "x"]);
        assert_eq!(sheet.rules.len(), 5);
        assert_eq!(sheet.rules[0].decls.len(), 2);
        assert_eq!(sheet.rules[3].decls.len(), 1, "the bad property is skipped");
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(errors[0].line, 5);
        assert!(errors[0].message.contains("bakground"));
        assert_eq!(errors[1].line, 8);
        assert!(
            matches!(&sheet.rules[1].decls[0], Decl::Background(ColorRef::Theme(n, a)) if n == "accent" && *a == 0.5)
        );
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#fff").unwrap(), ColorRef::Literal(Color::rgba(255, 255, 255, 1.0)));
        assert_eq!(parse_color("#11223380").unwrap(), ColorRef::Literal(Color::rgba(0x11, 0x22, 0x33, 128.0 / 255.0)));
        assert_eq!(parse_color("rgb(1, 2, 3)").unwrap(), ColorRef::Literal(Color::rgba(1, 2, 3, 1.0)));
        assert_eq!(parse_color("text-muted").unwrap(), ColorRef::Theme("text-muted".into(), 1.0));
        assert!(parse_color("nope").is_err());
    }
}
