//! The element inspector: F12 (or Ctrl+Shift+I, ⌥⌘I on macOS) shows the
//! element under the pointer, its box and a panel with its style, layout,
//! handlers and accessibility info. Click to pin an element; Escape or F12
//! closes it. On by default in debug builds
//! ([`Runtime::set_inspector_enabled`]).

use super::*;

/// What the inspector points at.
#[derive(Debug, Clone, Default)]
pub(crate) struct Inspector {
    pub hovered: Option<u64>,
    pub pinned: Option<u64>,
}

const OVERLAY: &str = "__inspector";
const PANEL: &str = "__inspector_panel";
/// The overlay's pieces (fixed elements hang off the window root, so they
/// are recognized by id rather than by ancestry).
const PIECES: [&str; 5] = [
    "__inspector/hover-box",
    "__inspector/hover-content",
    "__inspector/pin-box",
    "__inspector/pin-content",
    "__inspector_label",
];

fn hex(c: Color) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if c.a >= 0.999 {
        format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b), b(c.a))
    }
}

fn len(l: &Length) -> String {
    match l {
        Length::Auto => "auto".into(),
        Length::Px(v) => format!("{v}"),
        other => format!("{other:?}").to_lowercase(),
    }
}

fn edges(e: &Edges) -> String {
    if e.top == e.right && e.right == e.bottom && e.bottom == e.left {
        format!("{}", e.top)
    } else {
        format!("{} {} {} {}", e.top, e.right, e.bottom, e.left)
    }
}

impl<A: App> Runtime<A> {
    /// Allow opening the inspector with F12 / Ctrl+Shift+I (⌥⌘I on macOS).
    /// On by default in debug builds. While enabled, elements remember
    /// their string ids and style classes for it.
    pub fn set_inspector_enabled(&mut self, on: bool) {
        self.inspector_enabled = on;
        if !on {
            self.inspector = None;
        }
        self.invalidate();
    }

    /// Open or close the inspector.
    pub fn set_inspector_open(&mut self, open: bool) {
        self.inspector = open.then(Inspector::default);
        self.invalidate();
    }

    pub fn inspector_open(&self) -> bool {
        self.inspector.is_some()
    }

    /// The inspector's short name for the element it describes (the
    /// pinned one, else the hovered one), like `button.button #save`.
    pub fn inspected(&self) -> Option<String> {
        let ins = self.inspector.as_ref()?;
        let n = ins.pinned.or(ins.hovered).and_then(|id| self.node_by_id(id))?;
        Some(self.node_name(n))
    }

    /// The key that toggles the inspector?
    pub(crate) fn inspector_toggle_key(k: &KeyEvent) -> bool {
        let i = matches!(k.key, Key::Char('i') | Key::Char('I'));
        k.key == Key::F(12) || (i && k.mods.ctrl && k.mods.shift) || (i && k.mods.meta && k.mods.alt)
    }

    /// The node the inspector should describe: the topmost element at `p`,
    /// whatever its pointer events, but not the inspector itself.
    fn inspect_pick(&self, p: Point) -> Option<u64> {
        let overlay = global_id(OVERLAY);
        let panel = global_id(PANEL);
        let pieces = PIECES.map(global_id);
        let f = &self.frame;
        for &i in f.order.iter().rev() {
            let n = &f.nodes[i];
            if n.style.display == Display::None || !n.rect.contains(p) || n.clip.is_some_and(|c| !c.contains(p)) {
                continue;
            }
            if n.key.is_some_and(|k| pieces.contains(&k))
                || self.chain(i).iter().any(|&j| matches!(f.nodes[j].key, Some(k) if k == overlay || k == panel))
            {
                continue;
            }
            return Some(n.id);
        }
        None
    }

    fn over_panel(&self, p: Point) -> bool {
        let panel = global_id(PANEL);
        self.frame.nodes.iter().any(|n| n.key == Some(panel) && n.rect.contains(p))
    }

    /// Pointer events go to the inspector (except over its panel). True if
    /// it took the event.
    pub(crate) fn inspector_event(&mut self, ev: &Event) -> bool {
        match ev {
            Event::PointerMove(p) => {
                if self.over_panel(*p) {
                    return false;
                }
                let h = self.inspect_pick(*p);
                if let Some(ins) = &mut self.inspector {
                    if ins.hovered != h {
                        ins.hovered = h;
                        self.dirty = true;
                    }
                }
                self.pointer = Some(*p);
                true
            }
            Event::PointerDown(p, _) | Event::PointerUp(p, _) => {
                if self.over_panel(*p) {
                    return false;
                }
                if let (Event::PointerDown(..), Some(ins)) = (ev, &mut self.inspector) {
                    ins.pinned = ins.hovered;
                    self.dirty = true;
                }
                true
            }
            Event::Key(k) if k.key == Key::Escape => {
                self.inspector = None;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    /// A short name for a node: its role or kind, classes and id.
    fn node_name(&self, n: &Node<A::Msg>) -> String {
        let kind = match (&n.content, n.sem.as_ref().and_then(|s| s.role)) {
            (_, Some(r)) => format!("{r:?}").to_lowercase(),
            (NodeContent::Text(t), None) => {
                let s: String = t.text.chars().take(18).collect();
                format!("text \"{s}{}\"", if t.text.chars().count() > 18 { "…" } else { "" })
            }
            (NodeContent::Icon(_), None) => "icon".into(),
            (NodeContent::Image(_), None) => "image".into(),
            (NodeContent::Input(_), None) => "input".into(),
            (NodeContent::Canvas(_), None) => "canvas".into(),
            _ if n.split.is_some() => "split".into(),
            _ => "div".into(),
        };
        let mut s = kind;
        if let Some(d) = &n.debug {
            for c in &d.classes {
                s.push('.');
                s.push_str(c);
            }
            if let Some(id) = &d.id {
                s.push_str(" #");
                s.push_str(id);
            }
        }
        s
    }

    /// The overlay (boxes and label) and the details panel.
    pub(crate) fn inspector_overlay(&self) -> Option<Element<A::Msg>> {
        let ins = self.inspector.as_ref()?;
        let th = self.theme.clone();
        let c = th.colors.clone();
        let node = |id: Option<u64>| id.and_then(|id| self.node_by_id(id));
        let mut overlay = div().id(OVERLAY).pointer_events(false);
        let boxes = |n: &Node<A::Msg>, color: Color, ids: [&str; 2]| {
            let r = n.rect;
            let inner = inner_rect(r, &n.style.border_width);
            let p = n.style.padding;
            let content =
                Rect::new(inner.x + p.left, inner.y + p.top, inner.w - p.left - p.right, inner.h - p.top - p.bottom);
            [
                div()
                    .id(ids[0])
                    .fixed()
                    .left(r.x)
                    .top(r.y)
                    .w(r.w)
                    .h(r.h)
                    .bg(color.with_alpha(0.14))
                    .border(1.0, color)
                    .z_index(10_000),
                div()
                    .id(ids[1])
                    .fixed()
                    .left(content.x)
                    .top(content.y)
                    .w(content.w.max(0.0))
                    .h(content.h.max(0.0))
                    .border(1.0, color.with_alpha(0.55))
                    .z_index(10_001),
            ]
        };
        if let Some(n) = node(ins.pinned) {
            overlay = overlay.children(boxes(n, c.success, [PIECES[2], PIECES[3]]));
        }
        if let Some(n) = node(ins.hovered) {
            overlay = overlay.children(boxes(n, c.accent, [PIECES[0], PIECES[1]]));
            let r = n.rect;
            let label = format!("{}  {}×{}", self.node_name(n), r.w.round(), r.h.round());
            overlay = overlay.child(
                text(label)
                    .fixed()
                    .left(r.x.max(0.0))
                    .top(if r.y >= 22.0 { r.y - 22.0 } else { r.bottom() + 2.0 })
                    .z_index(10_002)
                    .px(6.0)
                    .h(20.0)
                    .rounded(4.0)
                    .bg(c.accent)
                    .color(c.accent_text)
                    .font_size(11.5)
                    .nowrap()
                    .id(PIECES[4]),
            );
        }
        // The panel describes the pinned element, or the hovered one.
        let target = node(ins.pinned).or_else(|| node(ins.hovered));
        let mut panel = col()
            .id(PANEL)
            .fixed()
            .top(0.0)
            .right(0.0)
            .bottom(0.0)
            .w(340.0)
            .z_index(10_003)
            .pointer_events(true)
            .bg(c.elevated)
            .border_l(1.0, c.border_strong)
            .shadows(th.shadow_popover.clone())
            .scroll_y()
            .p(12.0)
            .gap(3.0)
            .font_size(11.5)
            .color(c.text)
            .child(
                row()
                    .items_center()
                    .child(text("Inspector").semibold().font_size(13.0).grow(1.0))
                    .child(text("click pins · Esc closes").color(c.text_faint)),
            );
        let section = |t: &str| text(t.to_string()).semibold().color(c.accent).mt(8.0);
        let line = |k: &str, v: String| {
            row()
                .gap(8.0)
                .child(text(k.to_string()).w(92.0).shrink(0.0).color(c.text_muted))
                .child(text(v).mono().grow(1.0).min_w(0.0))
        };
        match target {
            None => panel = panel.child(text("Point at an element.").color(c.text_faint).mt(8.0)),
            Some(n) => {
                let s = &n.style;
                let mut path: Vec<String> = Vec::new();
                let mut p = n.parent;
                while let Some(i) = p {
                    let pn = &self.frame.nodes[i];
                    if pn.parent.is_some() {
                        path.push(self.node_name(pn));
                    }
                    p = pn.parent;
                }
                path.reverse();
                let path = if path.len() > 5 {
                    format!("… › {}", path[path.len() - 5..].join(" › "))
                } else {
                    path.join(" › ")
                };
                panel = panel
                    .child(text(self.node_name(n)).semibold().font_size(13.0).mt(6.0).id("__inspector_title"))
                    .child(text(path).color(c.text_faint))
                    .child(section("Box"))
                    .child(line("rect", format!("{} × {} at {}, {}", n.rect.w, n.rect.h, n.rect.x, n.rect.y)))
                    .child(line("padding", edges(&s.padding)))
                    .child(line("margin", edges(&s.margin)))
                    .child(line("border", format!("{} {}", edges(&s.border_width), hex(s.border_color))))
                    .child(section("Layout"))
                    .child(line("display", format!("{:?} {:?}", s.display, s.direction).to_lowercase()))
                    .child(line("size", format!("{} × {}", len(&s.width), len(&s.height))))
                    .child(line(
                        "min / max",
                        format!(
                            "{} × {} / {} × {}",
                            len(&s.min_width),
                            len(&s.min_height),
                            len(&s.max_width),
                            len(&s.max_height)
                        ),
                    ))
                    .child(line("flex", format!("grow {} shrink {} basis {}", s.grow, s.shrink, len(&s.basis))))
                    .child(line("gap", format!("{} {}", s.gap.0, s.gap.1)))
                    .child(line("align", format!("{:?} / justify {:?}", s.align_items, s.justify).to_lowercase()))
                    .child(line("position", format!("{:?}, z {}", s.position, s.z_index).to_lowercase()))
                    .child(line("overflow", format!("{:?}", s.overflow).to_lowercase()))
                    .child(section("Paint"))
                    .child(line(
                        "background",
                        match &s.background {
                            Some(crate::color::Fill::Solid(col)) => hex(*col),
                            Some(f) => format!("{f:?}"),
                            None => "none".into(),
                        },
                    ))
                    .child(line("color", hex(n.color)))
                    .child(line("radius", format!("{} {} {} {}", s.radius.tl, s.radius.tr, s.radius.br, s.radius.bl)))
                    .child(line("opacity", format!("{}", s.opacity)))
                    .child(line("shadows", format!("{}", s.shadows.len())))
                    .child(line("transition", format!("{}s", s.transition)))
                    .child(section("Text"))
                    .child(line(
                        "font",
                        format!(
                            "{} / {} {:?}{}",
                            n.text.size,
                            n.text.weight,
                            n.text.family,
                            if n.text.italic { " italic" } else { "" }
                        ),
                    ))
                    .child(line("line height", format!("{}", n.text.line_height)));
                let h = &n.handlers;
                let handlers: Vec<&str> = [
                    ("click", h.click.is_some()),
                    ("double-click", h.double_click.is_some()),
                    ("context-menu", h.context_menu.is_some()),
                    ("hover", h.hover.is_some()),
                    ("drag", h.drag.is_some()),
                    ("key", h.key.is_some()),
                    ("key-capture", h.key_capture.is_some()),
                    ("focus", h.focus.is_some()),
                    ("input", h.input.is_some()),
                    ("drop-target", h.drop_target.is_some()),
                    ("scroll", h.scroll.is_some()),
                ]
                .into_iter()
                .filter_map(|(k, on)| on.then_some(k))
                .collect();
                panel = panel
                    .child(section("Behavior"))
                    .child(line("handlers", if handlers.is_empty() { "none".into() } else { handlers.join(", ") }))
                    .child(line(
                        "state",
                        format!(
                            "{}{}{}{}",
                            if n.focusable { "focusable " } else { "" },
                            if self.focused == Some(n.id) { "focused " } else { "" },
                            if n.disabled { "disabled " } else { "" },
                            if self.hovered_set.contains(&n.id) { "hovered" } else { "" }
                        ),
                    ));
                if let Some(sem) = &n.sem {
                    panel = panel.child(section("Accessibility")).child(line(
                        "semantics",
                        format!(
                            "{}{}{}",
                            sem.role.map(|r| format!("{r:?} ")).unwrap_or_default(),
                            sem.label.as_ref().map(|l| format!("\"{l}\" ")).unwrap_or_default(),
                            [("checked", sem.checked), ("selected", sem.selected), ("expanded", sem.expanded)]
                                .iter()
                                .filter_map(|(k, v)| v.map(|v| format!("{k}={v} ")))
                                .collect::<String>()
                        ),
                    ));
                }
            }
        }
        Some(overlay.child(panel))
    }
}
