//! Side panels with stripes: JetBrains-style tool windows.
//!
//! Tool windows live on the left, right or bottom edge. Each has a button in
//! that edge's stripe. A tool window is either:
//! - **pinned** (docked): it takes space from the main content, with a
//!   splitter to resize it;
//! - **auto-hide** (unpinned): it slides over the content when opened and
//!   hides again when you click elsewhere, press Escape or click its stripe
//!   button, so it never costs space while you work.
//!
//! One tool window per edge is open at a time; the header's pin button
//! switches the mode and the minus button hides it.
//!
//! ```no_run
//! use rust_ui::prelude::*;
//! use rust_ui::toolwin::{Side, ToolMode, ToolMsg, ToolWindows};
//!
//! #[derive(Clone, Copy, PartialEq, Debug)]
//! enum Tool { Files, Search, Terminal }
//!
//! struct Ide { tools: ToolWindows<Tool> }
//! #[derive(Clone)] enum Msg { Tool(ToolMsg<Tool>) }
//!
//! impl App for Ide {
//!     type Msg = Msg;
//!     fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
//!         let Msg::Tool(m) = msg;
//!         self.tools.update(m);
//!     }
//!     fn view(&self) -> Element<Msg> {
//!         self.tools.view(
//!             text("editor"),
//!             |t| format!("{t:?}"),
//!             |t| match t { Tool::Files => Icon::Files, Tool::Search => Icon::Search, Tool::Terminal => Icon::Terminal },
//!             |t| text(format!("{t:?} content")),
//!             Msg::Tool,
//!         )
//!     }
//! }
//! let tools = ToolWindows::new()
//!     .add(Tool::Files, Side::Left, ToolMode::Pinned)
//!     .add(Tool::Search, Side::Left, ToolMode::AutoHide)
//!     .add(Tool::Terminal, Side::Bottom, ToolMode::AutoHide);
//! # let _ = Ide { tools };
//! ```

use std::rc::Rc;

use crate::element::*;
use crate::geometry::Point;
use crate::icons::Icon;
use crate::semantics::Role;
use crate::style::*;
use crate::theme::theme;
use crate::widgets::{backdrop, context_menu, menu_panel, tooltip_icon_button, MenuItem};

/// The edge a tool window is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Side {
    Left,
    Right,
    Bottom,
}

impl Side {
    fn index(self) -> usize {
        match self {
            Side::Left => 0,
            Side::Right => 1,
            Side::Bottom => 2,
        }
    }
}

/// How an open tool window takes its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ToolMode {
    /// Docked: shrinks the main content.
    Pinned,
    /// Slides over the content; hides when focus moves elsewhere.
    AutoHide,
}

/// One tool window.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolWindow<T> {
    pub id: T,
    pub side: Side,
    pub mode: ToolMode,
    /// Width (left/right) or height (bottom) in px.
    pub size: f32,
}

/// Messages of a [`ToolWindows`] UI. Forward them to [`ToolWindows::update`].
#[derive(Debug, Clone)]
pub enum ToolMsg<T> {
    /// Open the tool window, or hide it if it's open.
    Toggle(T),
    Hide(T),
    SetMode(T, ToolMode),
    /// Move to another edge.
    Move(T, Side),
    /// Hide every open tool window.
    HideAll,
    /// Pinned panels were resized (sizes of the pinned splits).
    Resized(Side, f32),
    /// Resize drag on an auto-hide panel's edge.
    ResizeDrag(Side, DragEvent),
    /// Open a tool window's menu (move, view mode, hide), at a window
    /// position or (from the keyboard or its header button) anchored.
    Menu(T, Option<Point>),
    CloseMenu,
}

/// Tool windows around a main content area; see the [module docs](self).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolWindows<T> {
    pub windows: Vec<ToolWindow<T>>,
    /// The open tool window of each side (left, right, bottom).
    open: [Option<T>; 3],
    /// The last opened tool window of each side (kept rendered while it
    /// slides out).
    #[cfg_attr(feature = "serde", serde(skip))]
    last: [Option<T>; 3],
    #[cfg_attr(feature = "serde", serde(skip))]
    drag_start: Option<f32>,
    #[cfg_attr(feature = "serde", serde(skip))]
    menu: Option<(T, Option<Point>)>,
    /// Width of the stripes.
    pub stripe: f32,
}

impl<T: Clone + PartialEq> Default for ToolWindows<T> {
    fn default() -> Self {
        Self {
            windows: Vec::new(),
            open: [None, None, None],
            last: [None, None, None],
            drag_start: None,
            menu: None,
            stripe: 36.0,
        }
    }
}

impl<T: Clone + PartialEq> ToolWindows<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tool window (closed), with a default size.
    pub fn add(mut self, id: T, side: Side, mode: ToolMode) -> Self {
        let size = if side == Side::Bottom { 220.0 } else { 260.0 };
        self.windows.push(ToolWindow { id, side, mode, size });
        self
    }

    fn get(&self, id: &T) -> Option<&ToolWindow<T>> {
        self.windows.iter().find(|w| &w.id == id)
    }

    fn get_mut(&mut self, id: &T) -> Option<&mut ToolWindow<T>> {
        self.windows.iter_mut().find(|w| &w.id == id)
    }

    /// Is this tool window open?
    pub fn is_open(&self, id: &T) -> bool {
        self.open.iter().flatten().any(|o| o == id)
    }

    /// The open tool window on `side`, if any.
    pub fn open_on(&self, side: Side) -> Option<&T> {
        self.open[side.index()].as_ref()
    }

    /// Open a tool window (replacing the one open on its side).
    pub fn show(&mut self, id: &T) {
        if let Some(w) = self.get(id) {
            let i = w.side.index();
            self.open[i] = Some(id.clone());
            self.last[i] = Some(id.clone());
        }
    }

    pub fn hide(&mut self, id: &T) {
        for o in &mut self.open {
            if o.as_ref() == Some(id) {
                *o = None;
            }
        }
    }

    pub fn update(&mut self, msg: ToolMsg<T>) {
        if !matches!(msg, ToolMsg::Menu(..)) {
            self.menu = None;
        }
        match msg {
            ToolMsg::Menu(id, at) => self.menu = Some((id, at)),
            ToolMsg::CloseMenu => {}
            ToolMsg::Toggle(id) => {
                if self.is_open(&id) {
                    self.hide(&id)
                } else {
                    self.show(&id)
                }
            }
            ToolMsg::Hide(id) => self.hide(&id),
            ToolMsg::SetMode(id, mode) => {
                if let Some(w) = self.get_mut(&id) {
                    w.mode = mode;
                }
            }
            ToolMsg::Move(id, side) => {
                let was_open = self.is_open(&id);
                self.hide(&id);
                for l in &mut self.last {
                    if l.as_ref() == Some(&id) {
                        *l = None;
                    }
                }
                if let Some(w) = self.get_mut(&id) {
                    w.side = side;
                }
                if was_open {
                    self.show(&id);
                }
            }
            ToolMsg::HideAll => self.open = [None, None, None],
            ToolMsg::Resized(side, size) => {
                if let Some(id) = self.open[side.index()].clone() {
                    if let Some(w) = self.get_mut(&id) {
                        w.size = size.max(80.0);
                    }
                }
            }
            ToolMsg::ResizeDrag(side, ev) => {
                let Some(id) = self.open[side.index()].clone() else { return };
                let Some(w) = self.windows.iter_mut().find(|w| w.id == id) else { return };
                match ev.phase {
                    DragPhase::Start => self.drag_start = Some(w.size),
                    DragPhase::Move | DragPhase::End => {
                        let start = self.drag_start.unwrap_or(w.size);
                        let d = match side {
                            Side::Left => ev.delta.x,
                            Side::Right => -ev.delta.x,
                            Side::Bottom => -ev.delta.y,
                        };
                        w.size = (start + d).max(80.0);
                        if ev.phase == DragPhase::End {
                            self.drag_start = None;
                        }
                    }
                }
            }
        }
    }

    /// Keyboard handling for [`App::on_key`](crate::App::on_key): Escape
    /// hides open auto-hide tool windows wherever the focus is.
    pub fn key(&self, k: &KeyEvent) -> Option<ToolMsg<T>> {
        let auto_open = self.open.iter().flatten().any(|id| self.get(id).is_some_and(|w| w.mode == ToolMode::AutoHide));
        (k.key == Key::Escape && auto_open).then_some(ToolMsg::HideAll)
    }

    /// A tool window's commands, as menu items.
    pub fn menu_items<M>(&self, id: &T, map: &dyn Fn(ToolMsg<T>) -> M) -> Vec<MenuItem<M>> {
        let Some(w) = self.get(id) else { return Vec::new() };
        let open = self.is_open(id);
        let side = |label: &str, s: Side| MenuItem::check(label, w.side == s, map(ToolMsg::Move(id.clone(), s)));
        vec![
            MenuItem::action(if open { "Hide" } else { "Show" }, map(ToolMsg::Toggle(id.clone()))),
            MenuItem::Separator,
            MenuItem::Header("View Mode".into()),
            MenuItem::check("Pinned", w.mode == ToolMode::Pinned, map(ToolMsg::SetMode(id.clone(), ToolMode::Pinned))),
            MenuItem::check(
                "Auto-hide",
                w.mode == ToolMode::AutoHide,
                map(ToolMsg::SetMode(id.clone(), ToolMode::AutoHide)),
            ),
            MenuItem::Separator,
            MenuItem::submenu(
                "Move to",
                vec![side("Left", Side::Left), side("Right", Side::Right), side("Bottom", Side::Bottom)],
            ),
        ]
    }

    /// Render the tool windows around `center`.
    ///
    /// * `title`, `icon`: a tool window's name (header and tooltip) and icon.
    /// * `content`: its content.
    /// * `map`: wraps [`ToolMsg`] into your app's message type.
    pub fn view<M: Clone + 'static>(
        &self,
        center: Element<M>,
        title: impl Fn(&T) -> String,
        icon_of: impl Fn(&T) -> Icon,
        content: impl Fn(&T) -> Element<M>,
        map: impl Fn(ToolMsg<T>) -> M + 'static,
    ) -> Element<M>
    where
        T: 'static,
    {
        let th = theme();
        let c = th.colors.clone();
        let map: Rc<dyn Fn(ToolMsg<T>) -> M> = Rc::new(map);
        let stripe = |side: Side| {
            let vertical = side != Side::Bottom;
            let mut s = if vertical { col().w(self.stripe).py(6.0) } else { row().h(self.stripe - 6.0).px(6.0) }
                .items_center()
                .gap(4.0)
                .shrink(0.0)
                .bg(if crate::runtime::window_info().backdrop { crate::color::Color::TRANSPARENT } else { c.chrome })
                .role(Role::Toolbar)
                .aria_label(match side {
                    Side::Left => "Left tool windows",
                    Side::Right => "Right tool windows",
                    Side::Bottom => "Bottom tool windows",
                })
                .class("tool-stripe");
            s = match side {
                Side::Left => s.border_r(1.0, c.border),
                Side::Right => s.border_l(1.0, c.border),
                Side::Bottom => s.border_t(1.0, c.border),
            };
            for w in self.windows.iter().filter(|w| w.side == side) {
                let open = self.is_open(&w.id);
                let name = title(&w.id);
                let (m1, m2, id1, id2) = (map.clone(), map.clone(), w.id.clone(), w.id.clone());
                let mut b = tooltip_icon_button(icon_of(&w.id), &name)
                    .id(&format!("tool-stripe/{name}"))
                    .aria_selected(open)
                    .on_click(map(ToolMsg::Toggle(w.id.clone())))
                    .on_context_menu(move |p| m1(ToolMsg::Menu(id1.clone(), Some(p))))
                    .on_key(move |k| {
                        (k.key == Key::F(10) && k.mods.shift).then(|| m2(ToolMsg::Menu(id2.clone(), None)))
                    });
                if open {
                    b = b.bg(c.accent_soft).color(c.accent);
                }
                if self.menu.as_ref().is_some_and(|(m, at)| m == &w.id && at.is_none()) {
                    let pop = anchored_menu(self.menu_items(&w.id, &*map), map(ToolMsg::CloseMenu));
                    b = b.child(match side {
                        Side::Left => pop.left(pct(100.0)).top(0.0).ml(6.0),
                        Side::Right => pop.right(pct(100.0)).top(0.0).mr(6.0),
                        Side::Bottom => pop.bottom(pct(100.0)).left(0.0).mb(6.0),
                    });
                    b = b.child(backdrop(map(ToolMsg::CloseMenu), false));
                }
                s = s.child(b);
            }
            s
        };
        let panel = |w: &ToolWindow<T>, shown: bool| {
            let name = title(&w.id);
            let pinned = w.mode == ToolMode::Pinned;
            let header = row()
                .h(th.tab_height - 4.0)
                .shrink(0.0)
                .items_center()
                .gap(2.0)
                .pl(12.0)
                .pr(4.0)
                .border_b(1.0, c.border)
                .child(text(name.clone()).semibold().font_size(th.font_size_sm).nowrap().grow(1.0))
                .child({
                    let mut b = tooltip_icon_button(Icon::Pin, if pinned { "Auto-hide" } else { "Pin (dock)" })
                        .id(&format!("tool-pin/{name}"))
                        .on_click(map(ToolMsg::SetMode(
                            w.id.clone(),
                            if pinned { ToolMode::AutoHide } else { ToolMode::Pinned },
                        )));
                    if pinned {
                        b = b.color(c.accent);
                    }
                    b
                })
                .child({
                    // Options: the tool window's menu, under the button.
                    let mut b = tooltip_icon_button(Icon::More, "Options")
                        .id(&format!("tool-options/{name}"))
                        .on_click(map(ToolMsg::Menu(w.id.clone(), None)));
                    if shown && self.menu.as_ref().is_some_and(|(m, at)| m == &w.id && at.is_none()) {
                        b = b.child(backdrop(map(ToolMsg::CloseMenu), false)).child(
                            anchored_menu(self.menu_items(&w.id, &*map), map(ToolMsg::CloseMenu))
                                .top(pct(100.0))
                                .right(0.0)
                                .mt(4.0),
                        );
                    }
                    b
                })
                .child(
                    tooltip_icon_button(Icon::Minus, "Hide")
                        .id(&format!("tool-hide/{name}"))
                        .on_click(map(ToolMsg::Hide(w.id.clone()))),
                );
            let (m3, id3) = (map.clone(), w.id.clone());
            let header = header.on_context_menu(move |p| m3(ToolMsg::Menu(id3.clone(), Some(p))));
            let m = map.clone();
            let id = w.id.clone();
            col()
                .id(&format!("tool/{name}"))
                .bg(c.panel)
                .min_w(0.0)
                .min_h(0.0)
                .clip()
                .role(Role::Group)
                .aria_label(name)
                .when(!shown, |e| e.aria_hidden())
                .on_key(move |k| (k.key == Key::Escape).then(|| m(ToolMsg::Hide(id.clone()))))
                .child(header)
                .child(col().grow(1.0).min_h(0.0).min_w(0.0).child(content(&w.id).grow(1.0).min_h(0.0)))
                .class("tool-window")
        };

        // Pinned open tool windows share space with the center.
        let pinned_open = |side: Side| {
            self.open[side.index()].as_ref().and_then(|id| self.get(id)).filter(|w| w.mode == ToolMode::Pinned)
        };
        let mut main = center.grow(1.0).min_w(0.0).min_h(0.0);
        if let Some(w) = pinned_open(Side::Bottom) {
            let m = map.clone();
            main = vsplit(
                "tool/split-v",
                vec![Pane::fill(main), Pane::fixed(w.size, panel(w, true)).min(80.0).priority(Priority::Low)],
            )
            .on_resize(move |s| m(ToolMsg::Resized(Side::Bottom, s.last().copied().unwrap_or(0.0))));
        }
        let (l, r) = (pinned_open(Side::Left), pinned_open(Side::Right));
        if l.is_some() || r.is_some() {
            let mut panes = Vec::new();
            if let Some(w) = l {
                panes.push(Pane::fixed(w.size, panel(w, true)).min(80.0).priority(Priority::Low));
            }
            panes.push(Pane::fill(main));
            if let Some(w) = r {
                panes.push(Pane::fixed(w.size, panel(w, true)).min(80.0).priority(Priority::Low));
            }
            let (m, has_l, has_r) = (map.clone(), l.is_some(), r.is_some());
            main = hsplit(&format!("tool/split-h/{}{}", has_l as u8, has_r as u8), panes).on_resize(move |s| {
                // Report the edge that moved (the first one when both did).
                if has_l {
                    m(ToolMsg::Resized(Side::Left, s[0]))
                } else {
                    m(ToolMsg::Resized(Side::Right, *s.last().unwrap_or(&0.0)))
                }
            });
        }

        // Auto-hide tool windows slide over the main area. The last one
        // opened on each side stays rendered (off-screen) so hiding animates.
        let mut area = div().grow(1.0).min_w(0.0).min_h(0.0).clip().child(main.size_full());
        let any_overlay_open = [Side::Left, Side::Right, Side::Bottom].into_iter().any(|s| {
            self.open[s.index()].as_ref().and_then(|id| self.get(id)).is_some_and(|w| w.mode == ToolMode::AutoHide)
        });
        if any_overlay_open {
            // Clicking the content hides auto-hide panels (like focus leaving them).
            let catcher = div()
                .absolute()
                .top(0.0)
                .left(0.0)
                .right(0.0)
                .bottom(0.0)
                .z_index(40)
                .aria_hidden()
                .on_click(map(ToolMsg::HideAll))
                .cursor(Cursor::Default);
            area = area.child(catcher);
        }
        for side in [Side::Left, Side::Right, Side::Bottom] {
            let Some(w) = self.last[side.index()].as_ref().and_then(|id| self.get(id)) else { continue };
            if w.mode != ToolMode::AutoHide {
                continue;
            }
            let shown = self.open[side.index()].as_ref() == Some(&w.id);
            let m = map.clone();
            let handle = div()
                .absolute()
                .cursor(if side == Side::Bottom { Cursor::ResizeRow } else { Cursor::ResizeCol })
                .on_drag(move |ev| m(ToolMsg::ResizeDrag(side, ev)))
                .aria_hidden();
            let (handle, off) = match side {
                // Inside the edge: the panel clips its children.
                Side::Left => (handle.top(0.0).bottom(0.0).right(0.0).w(5.0), (-w.size - 16.0, 0.0)),
                Side::Right => (handle.top(0.0).bottom(0.0).left(0.0).w(5.0), (w.size + 16.0, 0.0)),
                Side::Bottom => (handle.left(0.0).right(0.0).top(0.0).h(5.0), (0.0, w.size + 16.0)),
            };
            let mut ov = panel(w, shown)
                .absolute()
                .z_index(50)
                .shadows(th.shadow_popover.clone())
                .transition(0.18)
                .easing(crate::anim::Easing::EaseOutCubic)
                .translate(if shown { 0.0 } else { off.0 }, if shown { 0.0 } else { off.1 })
                .pointer_events(shown)
                .child(handle);
            ov = match side {
                Side::Left => ov.top(0.0).bottom(0.0).left(0.0).w(w.size).border_r(1.0, c.border_strong),
                Side::Right => ov.top(0.0).bottom(0.0).right(0.0).w(w.size).border_l(1.0, c.border_strong),
                Side::Bottom => ov.left(0.0).right(0.0).bottom(0.0).h(w.size).border_t(1.0, c.border_strong),
            };
            area = area.child(ov);
        }

        let has = |s: Side| self.windows.iter().any(|w| w.side == s);
        let middle = row()
            .grow(1.0)
            .min_h(0.0)
            .child_if(has(Side::Left), || stripe(Side::Left))
            .child(area)
            .child_if(has(Side::Right), || stripe(Side::Right));
        let mut root =
            col().grow(1.0).min_w(0.0).min_h(0.0).child(middle).child_if(has(Side::Bottom), || stripe(Side::Bottom));
        if let Some((id, Some(at))) = &self.menu {
            root = root.child(context_menu(*at, self.menu_items(id, &*map), map(ToolMsg::CloseMenu)));
        }
        root
    }
}

/// A menu panel positioned by the caller relative to its parent, closed
/// by Escape.
fn anchored_menu<M: Clone + 'static>(items: Vec<MenuItem<M>>, dismiss: M) -> Element<M> {
    menu_panel(items).absolute().z_index(100).on_key(move |k| (k.key == Key::Escape).then(|| dismiss.clone()))
}
