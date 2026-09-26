//! The declarative element tree.
//!
//! Views are plain functions that return an [`Element`]. Elements are styled
//! with chainable, CSS/Tailwind-flavoured builder methods and wire user
//! interaction to application messages:
//!
//! ```no_run
//! use rust_ui::prelude::*;
//! # #[derive(Clone)] enum Msg { Save }
//! let e: Element<Msg> = row()
//!     .gap(8.0)
//!     .p(12.0)
//!     .bg(hex("#1e1e1e"))
//!     .rounded(8.0)
//!     .child(text("Hello").font_size(15.0).bold())
//!     .child(button("Save").on_click(Msg::Save));
//! ```

use std::rc::Rc;

use crate::color::{Color, Fill};
use crate::geometry::{Axis, Point, Rect};
use crate::icons::Icon;
use crate::paint::Canvas;
use crate::style::*;

/// Keyboard key, platform independent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Space,
    F(u8),
    Other,
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Cmd on macOS, Windows key elsewhere.
    pub meta: bool,
}

impl Modifiers {
    /// Ctrl on Windows/Linux, Cmd on macOS.
    pub fn command(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.meta
        } else {
            self.ctrl
        }
    }
}

/// A key press delivered to the app or a focused element.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: Modifiers,
    pub repeat: bool,
}

/// Information about an in-progress pointer drag on an element with `on_drag`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragEvent {
    pub phase: DragPhase,
    /// Pointer position in window coordinates.
    pub pos: Point,
    /// Offset from where the drag started.
    pub delta: Point,
    /// The element's rectangle at the time of the event.
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragPhase {
    Start,
    Move,
    End,
}

/// Scroll position of a scroll container, delivered by [`Element::on_scroll`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollInfo {
    /// Current offset (logical px).
    pub offset: Point,
    /// Maximum offset on each axis.
    pub max: Point,
    /// True when scrolled to the bottom (within 2px).
    pub at_end: bool,
}

/// Delivered to drop targets while something is dragged over them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropEvent {
    pub phase: DropPhase,
    /// Pointer position in window coordinates.
    pub pos: Point,
    /// The drop target's rectangle.
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPhase {
    /// The drag moved over the target.
    Over,
    /// The drag left the target (or was cancelled).
    Leave,
    /// The drag was released over the target.
    Drop,
}

/// Built-in window control actions for custom (frameless) title bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControl {
    Minimize,
    ToggleMaximize,
    Close,
}

pub(crate) type Cb<A, M> = Rc<dyn Fn(A) -> M>;
pub(crate) type KeyCb<M> = Rc<dyn Fn(&KeyEvent) -> Option<M>>;
/// Custom paint callback for [`canvas`] elements.
pub type PaintFn = Rc<dyn Fn(&mut Canvas, Rect)>;

pub(crate) struct Handlers<M> {
    pub click: Option<M>,
    pub double_click: Option<M>,
    pub context_menu: Option<Cb<Point, M>>,
    pub hover: Option<Cb<bool, M>>,
    pub drag: Option<Cb<DragEvent, M>>,
    pub key: Option<KeyCb<M>>,
    pub value: Option<Cb<f32, M>>,
    pub input: Option<Cb<String, M>>,
    pub submit: Option<M>,
    pub collapse: Option<Cb<(usize, bool), M>>,
    pub resize: Option<Cb<Vec<f32>, M>>,
    pub drop_target: Option<Cb<DropEvent, M>>,
    pub scroll: Option<Cb<ScrollInfo, M>>,
    pub link: Option<Cb<String, M>>,
    pub select: Option<Cb<usize, M>>,
}

impl<M> Default for Handlers<M> {
    fn default() -> Self {
        Self {
            click: None,
            double_click: None,
            context_menu: None,
            hover: None,
            drag: None,
            key: None,
            value: None,
            input: None,
            submit: None,
            collapse: None,
            resize: None,
            drop_target: None,
            scroll: None,
            link: None,
            select: None,
        }
    }
}

impl<M: 'static> Handlers<M> {
    fn map<N: 'static>(self, f: Rc<dyn Fn(M) -> N>) -> Handlers<N> {
        fn wrap<A: 'static, M: 'static, N: 'static>(cb: Option<Cb<A, M>>, f: &Rc<dyn Fn(M) -> N>) -> Option<Cb<A, N>> {
            cb.map(|cb| {
                let f = f.clone();
                Rc::new(move |a| f(cb(a))) as Cb<A, N>
            })
        }
        Handlers {
            click: self.click.map(|m| f(m)),
            double_click: self.double_click.map(|m| f(m)),
            context_menu: wrap(self.context_menu, &f),
            hover: wrap(self.hover, &f),
            drag: wrap(self.drag, &f),
            key: self.key.map(|k| {
                let f = f.clone();
                Rc::new(move |e: &KeyEvent| k(e).map(|m| f(m))) as KeyCb<N>
            }),
            value: wrap(self.value, &f),
            input: wrap(self.input, &f),
            submit: self.submit.map(|m| f(m)),
            collapse: wrap(self.collapse, &f),
            resize: wrap(self.resize, &f),
            drop_target: wrap(self.drop_target, &f),
            scroll: wrap(self.scroll, &f),
            link: wrap(self.link, &f),
            select: wrap(self.select, &f),
        }
    }
}

/// Text content and its layout options.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextSpec {
    pub text: String,
    pub wrap: bool,
    pub ellipsis: bool,
    /// Styled runs (rich text); `text` is their concatenation.
    pub spans: Option<Rc<[crate::text::Span]>>,
    pub selectable: bool,
}

/// Single-line text input configuration.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InputSpec {
    pub value: String,
    pub placeholder: String,
    pub password: bool,
    /// Multi-line editing (text area).
    pub multiline: bool,
    /// For multi-line inputs: Enter submits, Shift+Enter inserts a newline.
    pub submit_on_enter: bool,
    /// Visible rows (min, max) for multi-line inputs; the box grows with its content.
    pub rows: (u32, u32),
}

/// A pane inside a [`split`](crate::split) container.
///
/// Panes are either *fixed* (a pixel size the user can drag) or *flex*
/// (a share of the remaining space, like CSS `flex-grow`). Splitters between
/// two flex panes redistribute their shares.
pub struct Pane<M> {
    pub(crate) content: Element<M>,
    /// Fixed size in px, or `None` for a flex pane.
    pub(crate) fixed: Option<f32>,
    /// Flex weight (ignored for fixed panes).
    pub(crate) weight: f32,
    pub(crate) min: f32,
    pub(crate) max: f32,
    pub(crate) collapsed: bool,
    pub(crate) collapsible: bool,
}

impl<M> Pane<M> {
    /// A pane with an initial fixed size (in px) that the user can resize.
    pub fn fixed(size: f32, content: impl Into<Element<M>>) -> Self {
        Self {
            content: content.into(),
            fixed: Some(size),
            weight: 1.0,
            min: 60.0,
            max: f32::INFINITY,
            collapsed: false,
            collapsible: false,
        }
    }
    /// A pane that fills the remaining space (flex weight 1).
    pub fn fill(content: impl Into<Element<M>>) -> Self {
        Self::flex(1.0, content)
    }
    /// A pane that takes a proportional share of the remaining space.
    pub fn flex(weight: f32, content: impl Into<Element<M>>) -> Self {
        Self {
            content: content.into(),
            fixed: None,
            weight: weight.max(0.0001),
            min: 60.0,
            max: f32::INFINITY,
            collapsed: false,
            collapsible: false,
        }
    }
    pub fn min(mut self, v: f32) -> Self {
        self.min = v;
        self
    }
    pub fn max(mut self, v: f32) -> Self {
        self.max = v;
        self
    }
    /// Collapse (slide closed) or expand the pane. Changes are animated.
    pub fn collapsed(mut self, c: bool) -> Self {
        self.collapsed = c;
        self
    }
    /// Allow the user to collapse the pane by dragging its splitter past half
    /// its minimum size (VS Code style). Pair with `Element::on_collapse`.
    pub fn collapsible(mut self, c: bool) -> Self {
        self.collapsible = c;
        self
    }
}

/// A dropdown (pick list / combo box).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DropdownSpec {
    pub options: Vec<String>,
    pub selected: Option<usize>,
    pub placeholder: String,
    pub searchable: bool,
}

pub(crate) struct SplitSpec<M> {
    pub axis: Axis,
    pub panes: Vec<Pane<M>>,
}

pub(crate) enum Content<M> {
    None,
    Text(TextSpec),
    Icon(Icon),
    Canvas(PaintFn),
    Input(InputSpec),
    Split(SplitSpec<M>),
    Dropdown(DropdownSpec),
}

/// Special interactive behaviours implemented by the runtime.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Behavior {
    None,
    Scroll {
        x: bool,
        y: bool,
    },
    Slider {
        value: f32,
        min: f32,
        max: f32,
        step: f32,
    },
    WindowDrag,
    WindowControl(WindowControl),
    Splitter {
        split: u64,
        index: usize,
        axis: Axis,
    },
    /// Copy text to the clipboard on click.
    Copy(String),
    DropdownToggle(u64),
    DropdownClose(u64),
    DropdownPick(u64, usize),
}

/// A node in the UI tree. Build them with [`div`], [`row`], [`col`],
/// [`text`], [`icon`] and the widget functions, then style with the builder
/// methods below.
pub struct Element<M> {
    pub(crate) key: Option<u64>,
    pub(crate) style: Style,
    pub(crate) hover: Option<StylePatch>,
    pub(crate) active: Option<StylePatch>,
    pub(crate) focus: Option<StylePatch>,
    pub(crate) disabled_style: Option<StylePatch>,
    pub(crate) children: Vec<Element<M>>,
    pub(crate) content: Content<M>,
    pub(crate) handlers: Handlers<M>,
    pub(crate) behavior: Behavior,
    pub(crate) focusable: bool,
    pub(crate) disabled: bool,
    pub(crate) hit_slop: f32,
    pub(crate) pointer_events: bool,
    pub(crate) tooltip: Option<String>,
    pub(crate) follow_end: bool,
}

fn hash_str(s: &str) -> u64 {
    // FNV-1a: stable across runs, which keeps retained state keyed by ids stable.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub(crate) fn mix_id(parent: u64, v: u64) -> u64 {
    let mut h = parent ^ v.wrapping_mul(0x9E3779B97F4A7C15);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58476D1CE4E5B9);
    h ^= h >> 32;
    h
}

impl<M> Element<M> {
    pub(crate) fn new(content: Content<M>) -> Self {
        Self {
            key: None,
            style: Style::default(),
            hover: None,
            active: None,
            focus: None,
            disabled_style: None,
            children: Vec::new(),
            content,
            handlers: Handlers::default(),
            behavior: Behavior::None,
            focusable: false,
            disabled: false,
            hit_slop: 0.0,
            pointer_events: true,
            tooltip: None,
            follow_end: false,
        }
    }
}

/// A generic container (flex row by default, like a `display:flex` div).
pub fn div<M: 'static>() -> Element<M> {
    Element::new(Content::None)
}

/// A horizontal flex container.
pub fn row<M: 'static>() -> Element<M> {
    div().direction(Direction::Row)
}

/// A vertical flex container.
pub fn col<M: 'static>() -> Element<M> {
    div().direction(Direction::Column)
}

/// A flexible spacer that pushes siblings apart.
pub fn spacer<M: 'static>() -> Element<M> {
    div().grow(1.0)
}

/// A run of text. Wraps by default; see [`Element::nowrap`] and [`Element::ellipsis`].
pub fn text<M: 'static>(s: impl Into<String>) -> Element<M> {
    Element::new(Content::Text(TextSpec {
        text: s.into(),
        wrap: true,
        ellipsis: false,
        spans: None,
        selectable: false,
    }))
}

/// Styled text made of [`Span`](crate::text::Span)s (bold, italic, mono,
/// colors, sizes, links). Wraps like [`text`].
///
/// ```
/// # use rust_ui::prelude::*;
/// # #[derive(Clone)] enum Msg { Open(String) }
/// let e: Element<Msg> = rich_text([
///     span("Read the "),
///     span("docs").link("https://example.com"),
///     span(" or run "),
///     span("cargo doc").mono(),
/// ])
/// .selectable()
/// .on_link(Msg::Open);
/// ```
pub fn rich_text<M: 'static>(spans: impl IntoIterator<Item = crate::text::Span>) -> Element<M> {
    let spans: Rc<[crate::text::Span]> = spans.into_iter().collect();
    let text: String = spans.iter().map(|s| s.text.as_str()).collect();
    Element::new(Content::Text(TextSpec { text, wrap: true, ellipsis: false, spans: Some(spans), selectable: false }))
}

/// A vector icon, sized by `font_size` unless given an explicit size.
pub fn icon<M: 'static>(i: Icon) -> Element<M> {
    Element::new(Content::Icon(i))
}

/// Custom drawing. The closure receives a canvas and the element's rect.
pub fn canvas<M: 'static>(f: impl Fn(&mut Canvas, Rect) + 'static) -> Element<M> {
    Element::new(Content::Canvas(Rc::new(f)))
}

/// A resizable split container. Panes are separated by draggable splitters,
/// can be nested in any direction and can be collapsed with animation.
///
/// `id` must be unique among splits; the user's resized sizes are retained
/// under it across frames.
pub fn split<M: 'static>(id: &str, axis: Axis, panes: Vec<Pane<M>>) -> Element<M> {
    let mut e = Element::new(Content::Split(SplitSpec { axis, panes }));
    e.key = Some(hash_str(id));
    e.style.grow = 1.0;
    e.style.min_width = Length::Px(0.0);
    e.style.min_height = Length::Px(0.0);
    e.style.overflow = Overflow::Hidden;
    e.style.direction = match axis {
        Axis::Horizontal => Direction::Row,
        Axis::Vertical => Direction::Column,
    };
    e
}

/// Horizontal split (panes side by side).
pub fn hsplit<M: 'static>(id: &str, panes: Vec<Pane<M>>) -> Element<M> {
    split(id, Axis::Horizontal, panes)
}

/// Vertical split (panes stacked).
pub fn vsplit<M: 'static>(id: &str, panes: Vec<Pane<M>>) -> Element<M> {
    split(id, Axis::Vertical, panes)
}

impl<M: 'static> From<&str> for Element<M> {
    fn from(s: &str) -> Self {
        text(s)
    }
}

impl<M: 'static> From<String> for Element<M> {
    fn from(s: String) -> Self {
        text(s)
    }
}

macro_rules! setter {
    ($(#[$m:meta])* $name:ident, $field:ident, $ty:ty) => {
        $(#[$m])*
        pub fn $name(mut self, v: $ty) -> Self {
            self.style.$field = v;
            self
        }
    };
}

impl<M: 'static> Element<M> {
    // ------------------------------------------------------------------ tree

    /// Stable identity for this element. Retained state (scroll offsets, hover
    /// animations, text cursors...) follows the key when siblings move.
    pub fn key(mut self, k: impl std::hash::Hash) -> Self {
        use std::hash::Hasher;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        k.hash(&mut h);
        self.key = Some(h.finish());
        self
    }

    /// Like [`Element::key`] but for string ids.
    pub fn id(mut self, id: &str) -> Self {
        self.key = Some(hash_str(id));
        self
    }

    pub fn child(mut self, c: impl Into<Element<M>>) -> Self {
        self.children.push(c.into());
        self
    }

    pub fn child_if(self, cond: bool, c: impl FnOnce() -> Element<M>) -> Self {
        if cond {
            self.child(c())
        } else {
            self
        }
    }

    pub fn children<I, E>(mut self, it: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<Element<M>>,
    {
        self.children.extend(it.into_iter().map(Into::into));
        self
    }

    /// Transform this element's messages, for composing components that have
    /// their own message type.
    pub fn map<N: 'static>(self, f: impl Fn(M) -> N + 'static) -> Element<N> {
        self.map_rc(Rc::new(f))
    }

    fn map_rc<N: 'static>(self, f: Rc<dyn Fn(M) -> N>) -> Element<N> {
        let content = match self.content {
            Content::None => Content::None,
            Content::Text(t) => Content::Text(t),
            Content::Icon(i) => Content::Icon(i),
            Content::Canvas(c) => Content::Canvas(c),
            Content::Input(i) => Content::Input(i),
            Content::Dropdown(d) => Content::Dropdown(d),
            Content::Split(s) => Content::Split(SplitSpec {
                axis: s.axis,
                panes: s
                    .panes
                    .into_iter()
                    .map(|p| Pane {
                        content: p.content.map_rc(f.clone()),
                        fixed: p.fixed,
                        weight: p.weight,
                        min: p.min,
                        max: p.max,
                        collapsed: p.collapsed,
                        collapsible: p.collapsible,
                    })
                    .collect(),
            }),
        };
        Element {
            key: self.key,
            style: self.style,
            hover: self.hover,
            active: self.active,
            focus: self.focus,
            disabled_style: self.disabled_style,
            children: self.children.into_iter().map(|c| c.map_rc(f.clone())).collect(),
            content,
            handlers: self.handlers.map(f),
            behavior: self.behavior,
            focusable: self.focusable,
            disabled: self.disabled,
            hit_slop: self.hit_slop,
            pointer_events: self.pointer_events,
            tooltip: self.tooltip,
            follow_end: self.follow_end,
        }
    }

    /// Apply a function to this element; handy for reusable style mixins.
    pub fn apply(self, f: impl FnOnce(Self) -> Self) -> Self {
        f(self)
    }

    /// Apply a function only when `cond` is true.
    pub fn when(self, cond: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if cond {
            f(self)
        } else {
            self
        }
    }

    /// Replace the whole style.
    pub fn style(mut self, s: Style) -> Self {
        self.style = s;
        self
    }

    /// Edit the style in place.
    pub fn style_with(mut self, f: impl FnOnce(&mut Style)) -> Self {
        f(&mut self.style);
        self
    }

    // ---------------------------------------------------------------- layout

    setter!(direction, direction, Direction);
    setter!(display, display, Display);
    setter!(grow, grow, f32);
    setter!(shrink, shrink, f32);
    setter!(position, position, Position);
    setter!(overflow, overflow, Overflow);
    setter!(z_index, z_index, i32);

    pub fn flex_row(self) -> Self {
        self.direction(Direction::Row)
    }
    pub fn flex_col(self) -> Self {
        self.direction(Direction::Column)
    }
    pub fn flex_wrap(mut self) -> Self {
        self.style.wrap = true;
        self
    }
    /// `flex: 1 1 0` — take an equal share of free space.
    pub fn flex1(mut self) -> Self {
        self.style.grow = 1.0;
        self.style.shrink = 1.0;
        self.style.basis = Length::Px(0.0);
        self
    }
    pub fn basis(mut self, v: impl Into<Length>) -> Self {
        self.style.basis = v.into();
        self
    }
    pub fn gap(mut self, v: f32) -> Self {
        self.style.gap = (v, v);
        self
    }
    pub fn gap_xy(mut self, x: f32, y: f32) -> Self {
        self.style.gap = (x, y);
        self
    }
    pub fn justify(mut self, j: Justify) -> Self {
        self.style.justify = Some(j);
        self
    }
    pub fn items(mut self, a: Align) -> Self {
        self.style.align_items = Some(a);
        self
    }
    pub fn self_align(mut self, a: Align) -> Self {
        self.style.align_self = Some(a);
        self
    }
    /// `align-items: center`.
    pub fn items_center(self) -> Self {
        self.items(Align::Center)
    }
    /// `justify-content: center; align-items: center`.
    pub fn center(self) -> Self {
        self.justify(Justify::Center).items(Align::Center)
    }
    pub fn justify_between(self) -> Self {
        self.justify(Justify::SpaceBetween)
    }

    pub fn w(mut self, v: impl Into<Length>) -> Self {
        self.style.width = v.into();
        self
    }
    pub fn h(mut self, v: impl Into<Length>) -> Self {
        self.style.height = v.into();
        self
    }
    pub fn size(self, w: impl Into<Length>, h: impl Into<Length>) -> Self {
        self.w(w).h(h)
    }
    /// Square size.
    pub fn square(self, v: f32) -> Self {
        self.w(v).h(v)
    }
    pub fn w_full(self) -> Self {
        self.w(Length::Percent(100.0))
    }
    pub fn h_full(self) -> Self {
        self.h(Length::Percent(100.0))
    }
    pub fn size_full(self) -> Self {
        self.w_full().h_full()
    }
    pub fn min_w(mut self, v: impl Into<Length>) -> Self {
        self.style.min_width = v.into();
        self
    }
    pub fn min_h(mut self, v: impl Into<Length>) -> Self {
        self.style.min_height = v.into();
        self
    }
    pub fn max_w(mut self, v: impl Into<Length>) -> Self {
        self.style.max_width = v.into();
        self
    }
    pub fn max_h(mut self, v: impl Into<Length>) -> Self {
        self.style.max_height = v.into();
        self
    }
    pub fn aspect_ratio(mut self, r: f32) -> Self {
        self.style.aspect_ratio = Some(r);
        self
    }

    pub fn p(mut self, v: f32) -> Self {
        self.style.padding = Edges::all(v);
        self
    }
    pub fn px(mut self, v: f32) -> Self {
        self.style.padding.left = v;
        self.style.padding.right = v;
        self
    }
    pub fn py(mut self, v: f32) -> Self {
        self.style.padding.top = v;
        self.style.padding.bottom = v;
        self
    }
    pub fn pt(mut self, v: f32) -> Self {
        self.style.padding.top = v;
        self
    }
    pub fn pb(mut self, v: f32) -> Self {
        self.style.padding.bottom = v;
        self
    }
    pub fn pl(mut self, v: f32) -> Self {
        self.style.padding.left = v;
        self
    }
    pub fn pr(mut self, v: f32) -> Self {
        self.style.padding.right = v;
        self
    }
    pub fn padding(mut self, e: Edges) -> Self {
        self.style.padding = e;
        self
    }
    pub fn m(mut self, v: f32) -> Self {
        self.style.margin = Edges::all(v);
        self
    }
    pub fn mx(mut self, v: f32) -> Self {
        self.style.margin.left = v;
        self.style.margin.right = v;
        self
    }
    pub fn my(mut self, v: f32) -> Self {
        self.style.margin.top = v;
        self.style.margin.bottom = v;
        self
    }
    pub fn mt(mut self, v: f32) -> Self {
        self.style.margin.top = v;
        self
    }
    pub fn mb(mut self, v: f32) -> Self {
        self.style.margin.bottom = v;
        self
    }
    pub fn ml(mut self, v: f32) -> Self {
        self.style.margin.left = v;
        self
    }
    pub fn mr(mut self, v: f32) -> Self {
        self.style.margin.right = v;
        self
    }
    /// `margin-left: auto` — push this element to the end of a row.
    pub fn ml_auto(mut self) -> Self {
        self.style.margin_auto[3] = true;
        self
    }
    /// `margin-top: auto` — push this element to the end of a column.
    pub fn mt_auto(mut self) -> Self {
        self.style.margin_auto[0] = true;
        self
    }

    /// `position: absolute` with the given insets (top, right, bottom, left).
    pub fn absolute(mut self) -> Self {
        self.style.position = Position::Absolute;
        self
    }
    /// `position: fixed` — positioned relative to the window and painted
    /// above normal content when combined with a `z_index`.
    pub fn fixed(mut self) -> Self {
        self.style.position = Position::Fixed;
        self
    }
    pub fn top(mut self, v: impl Into<Length>) -> Self {
        self.style.inset[0] = v.into();
        self
    }
    pub fn right(mut self, v: impl Into<Length>) -> Self {
        self.style.inset[1] = v.into();
        self
    }
    pub fn bottom(mut self, v: impl Into<Length>) -> Self {
        self.style.inset[2] = v.into();
        self
    }
    pub fn left(mut self, v: impl Into<Length>) -> Self {
        self.style.inset[3] = v.into();
        self
    }
    /// Absolutely fill the parent (`inset: 0`).
    pub fn inset0(self) -> Self {
        self.absolute().top(0.0).right(0.0).bottom(0.0).left(0.0)
    }

    /// Clip children to this element's bounds.
    pub fn clip(self) -> Self {
        self.overflow(Overflow::Hidden)
    }

    /// CSS grid with the given column tracks.
    pub fn grid(mut self, columns: Vec<Track>) -> Self {
        self.style.display = Display::Grid;
        self.style.grid_columns = columns;
        self
    }
    pub fn grid_rows(mut self, rows: Vec<Track>) -> Self {
        self.style.grid_rows = rows;
        self
    }
    pub fn col_span(mut self, n: u16) -> Self {
        self.style.grid_column_span = n;
        self
    }
    pub fn row_span(mut self, n: u16) -> Self {
        self.style.grid_row_span = n;
        self
    }
    pub fn hidden(mut self, hide: bool) -> Self {
        if hide {
            self.style.display = Display::None;
        }
        self
    }

    // --------------------------------------------------------------- visuals

    pub fn bg(mut self, f: impl Into<Fill>) -> Self {
        self.style.background = Some(f.into());
        self
    }
    /// Linear gradient background (CSS angle semantics).
    pub fn gradient(self, angle: f32, stops: impl IntoIterator<Item = (f32, Color)>) -> Self {
        self.bg(Fill::linear(angle, stops))
    }
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.style.border_width = Edges::all(width);
        self.style.border_color = color;
        self
    }
    pub fn border_widths(mut self, e: Edges, color: Color) -> Self {
        self.style.border_width = e;
        self.style.border_color = color;
        self
    }
    pub fn border_t(mut self, w: f32, color: Color) -> Self {
        self.style.border_width.top = w;
        self.style.border_color = color;
        self
    }
    pub fn border_b(mut self, w: f32, color: Color) -> Self {
        self.style.border_width.bottom = w;
        self.style.border_color = color;
        self
    }
    pub fn border_l(mut self, w: f32, color: Color) -> Self {
        self.style.border_width.left = w;
        self.style.border_color = color;
        self
    }
    pub fn border_r(mut self, w: f32, color: Color) -> Self {
        self.style.border_width.right = w;
        self.style.border_color = color;
        self
    }
    pub fn border_color(mut self, c: Color) -> Self {
        self.style.border_color = c;
        self
    }
    pub fn rounded(mut self, r: f32) -> Self {
        self.style.radius = Corners::all(r);
        self
    }
    pub fn radius(mut self, c: Corners) -> Self {
        self.style.radius = c;
        self
    }
    /// Fully rounded ("pill") corners.
    pub fn pill(self) -> Self {
        self.rounded(9999.0)
    }
    pub fn shadow(mut self, s: Shadow) -> Self {
        self.style.shadows.push(s);
        self
    }
    pub fn shadows(mut self, s: impl IntoIterator<Item = Shadow>) -> Self {
        self.style.shadows.extend(s);
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        self.style.opacity = o;
        self
    }
    pub fn outline(mut self, width: f32, offset: f32, color: Color) -> Self {
        self.style.outline = Some(Outline { width, offset, color });
        self
    }
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.style.translate = (x, y);
        self
    }
    pub fn cursor(mut self, c: Cursor) -> Self {
        self.style.cursor = Some(c);
        self
    }
    /// Animate interaction-state and style changes over `secs` seconds
    /// (CSS `transition: all <secs>s ease-out`).
    pub fn transition(mut self, secs: f32) -> Self {
        self.style.transition = secs;
        self
    }

    /// Smoothly animate changes to this element's position and size (for
    /// indicators, drop previews, reordering). Children move along with it.
    pub fn animate_layout(mut self, secs: f32) -> Self {
        self.style.layout_transition = secs;
        self
    }
    /// Timing function used by [`Element::transition`].
    pub fn easing(mut self, e: crate::anim::Easing) -> Self {
        self.style.easing = e;
        self
    }
    /// Style applied while hovered.
    pub fn hover(mut self, f: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.hover = Some(f(self.hover.take().unwrap_or_default()));
        self
    }
    /// Style applied while pressed.
    pub fn active(mut self, f: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.active = Some(f(self.active.take().unwrap_or_default()));
        self
    }
    /// Style applied while keyboard-focused.
    pub fn focus_style(mut self, f: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.focus = Some(f(self.focus.take().unwrap_or_default()));
        self
    }
    /// Style applied while disabled.
    pub fn disabled_style(mut self, f: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.disabled_style = Some(f(self.disabled_style.take().unwrap_or_default()));
        self
    }

    // ------------------------------------------------------------------ text

    pub fn color(mut self, c: Color) -> Self {
        self.style.color = Some(c);
        self
    }
    /// Font size in logical pixels (also sizes icons).
    pub fn font_size(mut self, s: f32) -> Self {
        self.style.font_size = Some(s);
        self
    }
    /// Alias for [`Element::font_size`]; reads nicely on text: `text("Hi").text_size(18.)`.
    pub fn text_size(self, s: f32) -> Self {
        self.font_size(s)
    }
    pub fn weight(mut self, w: Weight) -> Self {
        self.style.font_weight = Some(w);
        self
    }
    pub fn medium(self) -> Self {
        self.weight(Weight::MEDIUM)
    }
    pub fn semibold(self) -> Self {
        self.weight(Weight::SEMIBOLD)
    }
    pub fn bold(self) -> Self {
        self.weight(Weight::BOLD)
    }
    pub fn italic(mut self) -> Self {
        self.style.italic = Some(true);
        self
    }
    pub fn mono(mut self) -> Self {
        self.style.font_family = Some(FontFamily::Mono);
        self
    }
    pub fn font(mut self, f: FontFamily) -> Self {
        self.style.font_family = Some(f);
        self
    }
    /// Line height as a multiple of the font size.
    pub fn line_height(mut self, lh: f32) -> Self {
        self.style.line_height = Some(lh);
        self
    }
    pub fn letter_spacing(mut self, v: f32) -> Self {
        self.style.letter_spacing = Some(v);
        self
    }
    pub fn text_align(mut self, a: TextAlign) -> Self {
        self.style.text_align = Some(a);
        self
    }
    /// Keep text on one line.
    pub fn nowrap(mut self) -> Self {
        if let Content::Text(t) = &mut self.content {
            t.wrap = false;
        }
        self
    }
    /// Let the user select this text with the mouse and copy it with Ctrl+C
    /// (double-click selects a word, triple-click everything).
    pub fn selectable(mut self) -> Self {
        if let Content::Text(t) = &mut self.content {
            t.selectable = true;
        }
        self
    }

    /// Called with the target when a link span in rich text is clicked.
    pub fn on_link(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.handlers.link = Some(Rc::new(f));
        self
    }

    /// Keep text on one line and truncate it with "…" when it doesn't fit.
    pub fn ellipsis(mut self) -> Self {
        if let Content::Text(t) = &mut self.content {
            t.wrap = false;
            t.ellipsis = true;
        }
        self.style.min_width = Length::Px(0.0);
        self.style.shrink = 1.0;
        self
    }

    // ----------------------------------------------------------- interaction

    pub fn on_click(mut self, m: M) -> Self {
        self.handlers.click = Some(m);
        if self.style.cursor.is_none() {
            self.style.cursor = Some(Cursor::Pointer);
        }
        self
    }
    pub fn on_double_click(mut self, m: M) -> Self {
        self.handlers.double_click = Some(m);
        self
    }
    /// Right click; receives the pointer position (for context menus).
    pub fn on_context_menu(mut self, f: impl Fn(Point) -> M + 'static) -> Self {
        self.handlers.context_menu = Some(Rc::new(f));
        self
    }
    pub fn on_hover(mut self, f: impl Fn(bool) -> M + 'static) -> Self {
        self.handlers.hover = Some(Rc::new(f));
        self
    }
    /// Receive pointer drag events that start on this element.
    pub fn on_drag(mut self, f: impl Fn(DragEvent) -> M + 'static) -> Self {
        self.handlers.drag = Some(Rc::new(f));
        self
    }
    /// Make this element a drop target for drags started with [`Element::on_drag`].
    /// Receives `Over` while hovered during a drag, `Leave`, and `Drop` on release.
    pub fn on_drop_target(mut self, f: impl Fn(DropEvent) -> M + 'static) -> Self {
        self.handlers.drop_target = Some(Rc::new(f));
        self
    }
    /// Key presses while this element (or a descendant) has focus.
    pub fn on_key(mut self, f: impl Fn(&KeyEvent) -> Option<M> + 'static) -> Self {
        self.handlers.key = Some(Rc::new(f));
        self
    }
    /// For split containers: called when the user collapses/expands a pane by
    /// dragging its splitter. Receives `(pane_index, collapsed)`.
    pub fn on_collapse(mut self, f: impl Fn(usize, bool) -> M + 'static) -> Self {
        self.handlers.collapse = Some(Rc::new(move |(i, c)| f(i, c)));
        self
    }
    /// For split containers: called with the new pane sizes after the user
    /// finishes resizing (useful for persisting layouts).
    pub fn on_resize(mut self, f: impl Fn(Vec<f32>) -> M + 'static) -> Self {
        self.handlers.resize = Some(Rc::new(f));
        self
    }
    /// Make the element reachable with Tab and focusable by click.
    pub fn focusable(mut self) -> Self {
        self.focusable = true;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    /// Extend the hit-test area beyond the visual bounds.
    pub fn hit_slop(mut self, v: f32) -> Self {
        self.hit_slop = v;
        self
    }
    /// `pointer-events: none` when false.
    pub fn pointer_events(mut self, v: bool) -> Self {
        self.pointer_events = v;
        self
    }
    /// Show a tooltip after hovering.
    pub fn tooltip(mut self, t: impl Into<String>) -> Self {
        self.tooltip = Some(t.into());
        self
    }
    /// Make this element (typically a custom title bar) drag the window.
    pub fn window_drag_area(mut self) -> Self {
        self.behavior = Behavior::WindowDrag;
        self
    }
    /// Make this element perform a window control action when clicked.
    pub fn window_control(mut self, c: WindowControl) -> Self {
        self.behavior = Behavior::WindowControl(c);
        self.style.cursor.get_or_insert(Cursor::Default);
        self
    }
    /// Make this element a scroll container on the vertical axis.
    pub fn scroll_y(mut self) -> Self {
        self.behavior = Behavior::Scroll { x: false, y: true };
        self.style.overflow = Overflow::Scroll;
        self.style.min_height = Length::Px(0.0);
        self
    }
    /// Make this element a scroll container on the horizontal axis.
    pub fn scroll_x(mut self) -> Self {
        self.behavior = Behavior::Scroll { x: true, y: false };
        self.style.overflow = Overflow::Scroll;
        self.style.min_width = Length::Px(0.0);
        self
    }
    /// Keep a scroll container pinned to its end as content grows (chat
    /// transcripts, logs). Scrolling up unpins it; scrolling back to the end
    /// re-pins it.
    pub fn follow_end(mut self) -> Self {
        self.follow_end = true;
        self
    }

    /// Called whenever this scroll container's position changes.
    pub fn on_scroll(mut self, f: impl Fn(ScrollInfo) -> M + 'static) -> Self {
        self.handlers.scroll = Some(Rc::new(f));
        self
    }

    /// Scroll on both axes.
    pub fn scroll_both(mut self) -> Self {
        self.behavior = Behavior::Scroll { x: true, y: true };
        self.style.overflow = Overflow::Scroll;
        self.style.min_width = Length::Px(0.0);
        self.style.min_height = Length::Px(0.0);
        self
    }
}

/// Hash used for elements addressed globally by `.id(...)`.
pub(crate) fn global_id(s: &str) -> u64 {
    hash_str(s)
}
