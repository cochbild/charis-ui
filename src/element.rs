//! The declarative element tree.
//!
//! Views are plain functions that return an [`Element`]. Elements are styled
//! with chainable, CSS/Tailwind-flavoured builder methods and wire user
//! interaction to application messages:
//!
//! ```no_run
//! use charis_ui::prelude::*;
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
use crate::semantics::{Role, Semantics};
use crate::style::*;

/// Keyboard key, platform independent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// A printable character.
    Char(char),
    /// Enter / Return.
    Enter,
    /// Tab.
    Tab,
    /// Escape.
    Escape,
    /// Backspace.
    Backspace,
    /// Forward delete.
    Delete,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Space bar.
    Space,
    /// Function key `F1`, `F2`, … (the number).
    F(u8),
    /// Any key not listed above.
    Other,
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Modifiers {
    /// Shift is held.
    pub shift: bool,
    /// Ctrl is held.
    pub ctrl: bool,
    /// Alt (Option on macOS) is held.
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
    /// The key pressed.
    pub key: Key,
    /// Modifier keys held during the press.
    pub mods: Modifiers,
    /// True for auto-repeat presses while the key is held down.
    pub repeat: bool,
}

/// Information about an in-progress pointer drag on an element with `on_drag`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragEvent {
    /// Whether the drag started, moved or ended.
    pub phase: DragPhase,
    /// Pointer position in window coordinates.
    pub pos: Point,
    /// Offset from where the drag started.
    pub delta: Point,
    /// The element's rectangle at the time of the event.
    pub rect: Rect,
    /// The pointer is outside the window (e.g. a tab dragged out of it).
    pub outside: bool,
    /// Pointer position on the screen (logical px), where the platform
    /// reports window positions (not on Wayland).
    pub screen: Option<Point>,
}

/// Stage of a pointer drag, see [`DragEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragPhase {
    /// The pointer moved far enough after a press to start a drag.
    Start,
    /// The pointer moved during the drag.
    Move,
    /// The pointer was released (or the drag was cancelled).
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
    /// Whether the drag is over the target, left it or was dropped on it.
    pub phase: DropPhase,
    /// Pointer position in window coordinates.
    pub pos: Point,
    /// The drop target's rectangle.
    pub rect: Rect,
}

/// Stage of a drag over a drop target, see [`DropEvent`].
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
#[non_exhaustive]
pub enum WindowControl {
    /// Minimize the window.
    Minimize,
    /// Maximize the window, or restore it if maximized.
    ToggleMaximize,
    /// Close the window.
    Close,
}

pub(crate) type Cb<A, M> = Rc<dyn Fn(A) -> M>;

/// What an interaction produces: an app message, or an update to a
/// [`component`](crate::component)'s local state (which may in turn emit a
/// message).
pub(crate) enum Out<M> {
    Msg(M),
    Local(Local<M>),
}

/// Applies a local update to the component store, possibly producing a message.
pub(crate) type LocalFn<M> = Rc<dyn Fn(&mut crate::component::Store) -> Option<M>>;

/// A component-local update: applied to the component state store (a
/// nested component's update can feed its enclosing component's update).
pub(crate) struct Local<M> {
    pub apply: LocalFn<M>,
}

impl<M: Clone> Clone for Out<M> {
    fn clone(&self) -> Self {
        match self {
            Out::Msg(m) => Out::Msg(m.clone()),
            Out::Local(l) => Out::Local(Local { apply: l.apply.clone() }),
        }
    }
}

impl<M: 'static> Out<M> {
    pub(crate) fn map<N: 'static>(self, f: &Rc<dyn Fn(M) -> N>) -> Out<N> {
        match self {
            Out::Msg(m) => Out::Msg(f(m)),
            Out::Local(l) => {
                let (apply, f) = (l.apply, f.clone());
                Out::Local(Local { apply: Rc::new(move |s| apply(s).map(|m| f(m))) })
            }
        }
    }
}

/// Wrap an app-message callback as a handler.
pub(crate) fn cb<A: 'static, M: 'static>(f: impl Fn(A) -> M + 'static) -> Cb<A, Out<M>> {
    Rc::new(move |a| Out::Msg(f(a)))
}
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
    pub key_capture: Option<KeyCb<M>>,
    pub focus: Option<Cb<bool, M>>,
    pub paste_image: Option<Cb<crate::image::Image, M>>,
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

impl<M: Clone> Clone for Handlers<M> {
    fn clone(&self) -> Self {
        Self {
            click: self.click.clone(),
            double_click: self.double_click.clone(),
            context_menu: self.context_menu.clone(),
            hover: self.hover.clone(),
            drag: self.drag.clone(),
            key: self.key.clone(),
            key_capture: self.key_capture.clone(),
            focus: self.focus.clone(),
            paste_image: self.paste_image.clone(),
            value: self.value.clone(),
            input: self.input.clone(),
            submit: self.submit.clone(),
            collapse: self.collapse.clone(),
            resize: self.resize.clone(),
            drop_target: self.drop_target.clone(),
            scroll: self.scroll.clone(),
            link: self.link.clone(),
            select: self.select.clone(),
        }
    }
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
            key_capture: None,
            focus: None,
            paste_image: None,
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
            key_capture: self.key_capture.map(|k| {
                let f = f.clone();
                Rc::new(move |e: &KeyEvent| k(e).map(|m| f(m))) as KeyCb<N>
            }),
            focus: wrap(self.focus, &f),
            paste_image: wrap(self.paste_image, &f),
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
    /// Shared, so the runtime can hold on to large values without copying them.
    pub value: std::rc::Rc<str>,
    pub placeholder: String,
    pub password: bool,
    /// Multi-line editing (text area).
    pub multiline: bool,
    /// For multi-line inputs: Enter submits, Shift+Enter inserts a newline.
    pub submit_on_enter: bool,
    /// Visible rows (min, max) for multi-line inputs; the box grows with its content.
    pub rows: (u32, u32),
}

/// Which fixed-size panes give up or take space first when a split's
/// container changes size (VS Code's `LayoutPriority`).
///
/// When the window is too small for every pane, fixed panes shrink (down
/// to their minimum) highest priority first; when a split has no flex pane
/// to absorb extra space, it goes to the highest priority fixed panes.
/// Each pane keeps its own size, so it returns to it when there's room
/// again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Priority {
    /// Keeps its size the longest (a sidebar).
    Low,
    /// The default.
    #[default]
    Normal,
    /// Absorbs size changes first (the main content).
    High,
}

impl Priority {
    /// Flex factor tiers: each tier outweighs the one below ~1000 to 1.
    pub(crate) fn factor(self) -> f32 {
        match self {
            Priority::Low => 1.0,
            Priority::Normal => 1e3,
            Priority::High => 1e6,
        }
    }
}

/// A pane inside a [`split`] container.
///
/// Panes are either *fixed* (a pixel size the user can drag) or *flex*
/// (a share of the remaining space, like CSS `flex-grow`). Splitters between
/// two flex panes redistribute their shares. See [`Priority`] for fixed
/// panes when space runs out, and [`Pane::key`] for panes that come and go.
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
    pub(crate) priority: Priority,
    pub(crate) key: Option<u64>,
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
            priority: Priority::Normal,
            key: None,
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
            priority: Priority::Normal,
            key: None,
        }
    }
    /// Minimum size in logical px (default 60).
    pub fn min(mut self, v: f32) -> Self {
        self.min = v;
        self
    }
    /// Maximum size in logical px (default unbounded).
    pub fn max(mut self, v: f32) -> Self {
        self.max = v;
        self
    }
    /// Collapse (slide closed) or expand the pane. Changes are animated.
    pub fn collapsed(mut self, c: bool) -> Self {
        self.collapsed = c;
        self
    }
    /// Which fixed panes shrink or grow first when space changes; see
    /// [`Priority`].
    pub fn priority(mut self, p: Priority) -> Self {
        self.priority = p;
        self
    }
    /// A stable identity for a pane that can be removed and added back
    /// (e.g. a toggled sidebar): it gets back the size the user gave it,
    /// and keeps its content's state when other panes come and go.
    pub fn key(mut self, k: impl std::hash::Hash) -> Self {
        use std::hash::Hasher;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        k.hash(&mut h);
        self.key = Some(h.finish());
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

/// A virtualized list: only rows near the viewport are built.
pub(crate) struct VirtualSpec<M> {
    pub count: usize,
    /// Row height used for rows that haven't been measured yet (exact for
    /// uniform rows).
    pub estimate: f32,
    /// Extra pixels built above and below the viewport.
    pub overscan: f32,
    pub builder: Rc<dyn Fn(usize) -> Element<M>>,
}

/// A memoized subtree (see [`lazy`]). `built` is `None` when the runtime
/// will reuse last frame's subtree instead.
pub(crate) struct LazySpec<M> {
    pub key: u64,
    pub deps: u64,
    pub built: Option<Box<Element<M>>>,
    /// Components rendered inside (their state changes invalidate it).
    pub components: Vec<u64>,
}

pub(crate) enum Content<M> {
    None,
    Text(TextSpec),
    Icon(Icon),
    Canvas(PaintFn),
    Input(InputSpec),
    Split(SplitSpec<M>),
    Dropdown(DropdownSpec),
    Virtual(VirtualSpec<M>),
    Lazy(LazySpec<M>),
    Image(ImageSpec),
}

/// An image element's content.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ImageSpec {
    pub source: crate::image::ImageSource,
    pub fit: crate::image::Fit,
    pub tint: Option<Color>,
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
    /// A table cell: its width follows the table's user-resized column width.
    TableCell {
        table: u64,
        col: usize,
    },
    /// Drag handle that resizes a table column (double-click resets it).
    ColumnResize {
        table: u64,
        col: usize,
        min: f32,
    },
}

/// A node in the UI tree. Build them with [`div`], [`row`], [`col`],
/// [`text`], [`icon`] and the widget functions, then style with the builder
/// methods below.
///
/// `M` is the app's message type, produced by handlers like
/// [`on_click`](Self::on_click).
///
/// ```
/// use charis_ui::prelude::*;
/// #[derive(Clone)]
/// enum Msg { Inc }
/// fn view(count: i32) -> Element<Msg> {
///     col()
///         .gap(8.0)
///         .p(16.0)
///         .child(text(format!("Count: {count}")).font_size(18.0).bold())
///         .child(button("+1").on_click(Msg::Inc))
/// }
/// let _ = view(0);
/// ```
pub struct Element<M> {
    pub(crate) key: Option<u64>,
    pub(crate) style: Style,
    // Boxed: most elements have none, and elements are moved by value
    // through every builder call.
    pub(crate) hover: Option<Box<StylePatch>>,
    pub(crate) active: Option<Box<StylePatch>>,
    pub(crate) focus: Option<Box<StylePatch>>,
    pub(crate) disabled_style: Option<Box<StylePatch>>,
    pub(crate) children: Vec<Element<M>>,
    pub(crate) content: Content<M>,
    pub(crate) handlers: Handlers<Out<M>>,
    pub(crate) behavior: Behavior,
    pub(crate) focusable: bool,
    pub(crate) autofocus: bool,
    pub(crate) disabled: bool,
    pub(crate) hit_slop: f32,
    /// `None` inherits from the parent.
    pub(crate) pointer_events: Option<bool>,
    pub(crate) tooltip: Option<String>,
    pub(crate) follow_end: bool,
    pub(crate) sem: Option<Box<Semantics>>,
    /// For the inspector: the string id and style classes (recorded only
    /// while it's enabled).
    pub(crate) debug: Option<Box<DebugInfo>>,
}

/// What the inspector shows about an element beyond its style.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DebugInfo {
    pub id: Option<String>,
    pub classes: Vec<String>,
}

thread_local! {
    /// Record [`DebugInfo`] (the runtime turns it on with the inspector).
    pub(crate) static RECORD_DEBUG: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn recording() -> bool {
    RECORD_DEBUG.with(|r| r.get())
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
            autofocus: false,
            disabled: false,
            hit_slop: 0.0,
            pointer_events: None,
            tooltip: None,
            follow_end: false,
            sem: None,
            debug: None,
        }
    }

    fn sem_mut(&mut self) -> &mut Semantics {
        self.sem.get_or_insert_with(Default::default)
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
///
/// ```
/// use charis_ui::prelude::*;
/// let sidebar: Element<()> = col()
///     .w(240.0)
///     .gap(4.0)
///     .p(8.0)
///     .children(["Inbox", "Sent", "Drafts"].map(text));
/// ```
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
/// # use charis_ui::prelude::*;
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

/// A vertically scrolling list that only builds the rows near the viewport,
/// so it stays fast with 100k+ rows. `item(i)` builds row `i` on demand.
///
/// Rows may have different heights: each row is measured when it is first
/// shown and cached, and scrolling stays anchored while estimates are
/// replaced by real heights. Set [`Element::item_height`] to the typical (or
/// exact) row height. Use `.gap()` / padding on the list as usual, and
/// `.follow_end()`, `.on_scroll()`, `Cx::scroll_to_end` and
/// `Cx::scroll_to_item` like any scroll container.
pub fn virtual_list<M: 'static>(count: usize, item: impl Fn(usize) -> Element<M> + 'static) -> Element<M> {
    let mut e = div().flex_col().scroll_y();
    e.content = Content::Virtual(VirtualSpec { count, estimate: 28.0, overscan: 300.0, builder: Rc::new(item) });
    e
}

/// A memoized part of the UI: `build` runs only when `deps` changed since
/// the last frame. Otherwise the runtime reuses last frame's elements and
/// their layout, skipping the view code, tree building and layout for the
/// whole subtree.
///
/// ```
/// # use charis_ui::prelude::*;
/// # #[derive(Clone)] enum Msg {}
/// # struct Message { id: u64, version: u32, text: String }
/// fn message_view(m: &Message) -> Element<Msg> {
///     lazy(("message", m.id), m.version, || card().child(text(m.text.clone())))
/// }
/// ```
///
/// - `key` identifies the subtree and must be unique among `lazy` elements
///   in the window; it also gives the subtree stable identity, so moving it
///   elsewhere in the tree keeps its state.
/// - `deps` must cover everything `build` reads that can change: the
///   subtree (including the messages its handlers send) is reused as long as
///   `deps` hashes the same. Theme changes rebuild it automatically.
/// - Hover, press, focus and running transitions inside the subtree rebuild
///   it automatically, as do virtual lists, splits, dropdowns and tables
///   inside it (those depend on live state). Nested `lazy`s are reused
///   independently, so put them around the parts that change rarely.
/// - The returned element is a column container you can style like any
///   other (`.grow(1.0)`, `.w_full()`…); `build` runs right away (it can
///   borrow the app state), or not at all.
pub fn lazy<M: 'static>(
    key: impl std::hash::Hash,
    deps: impl std::hash::Hash,
    build: impl FnOnce() -> Element<M>,
) -> Element<M> {
    use std::hash::Hasher;
    let hash = |v: &dyn Fn(&mut std::collections::hash_map::DefaultHasher)| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        v(&mut h);
        h.finish()
    };
    let key = hash(&|h| key.hash(h));
    let deps = hash(&|h| deps.hash(h));
    let (built, components) = match crate::runtime::memo::claim(key, deps) {
        Some(components) => (None, components),
        None => {
            let (el, components) = crate::runtime::memo::collect_components(build);
            (Some(Box::new(el)), components)
        }
    };
    let mut e = div().flex_col();
    e.content = Content::Lazy(LazySpec { key, deps, built, components });
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
        if recording() {
            self.debug.get_or_insert_with(Default::default).id = Some(id.to_string());
        }
        self
    }

    /// Append a child element (strings become [`text`]).
    pub fn child(mut self, c: impl Into<Element<M>>) -> Self {
        self.children.push(c.into());
        self
    }

    /// Append the child built by `c` only when `cond` is true.
    pub fn child_if(self, cond: bool, c: impl FnOnce() -> Element<M>) -> Self {
        if cond {
            self.child(c())
        } else {
            self
        }
    }

    /// Append every element from an iterator.
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
        self.map_out(Rc::new(move |o: Out<M>| o.map(&f)))
    }

    /// Transform this element's handler outputs (messages and local updates).
    pub(crate) fn map_out<N: 'static>(self, f: Rc<dyn Fn(Out<M>) -> Out<N>>) -> Element<N> {
        let content = match self.content {
            Content::None => Content::None,
            Content::Text(t) => Content::Text(t),
            Content::Icon(i) => Content::Icon(i),
            Content::Image(i) => Content::Image(i),
            Content::Canvas(c) => Content::Canvas(c),
            Content::Input(i) => Content::Input(i),
            Content::Dropdown(d) => Content::Dropdown(d),
            Content::Virtual(v) => {
                let (b, f) = (v.builder, f.clone());
                Content::Virtual(VirtualSpec {
                    count: v.count,
                    estimate: v.estimate,
                    overscan: v.overscan,
                    builder: Rc::new(move |i| b(i).map_out(f.clone())),
                })
            }
            Content::Lazy(l) => Content::Lazy(LazySpec {
                key: l.key,
                deps: l.deps,
                built: l.built.map(|b| Box::new(b.map_out(f.clone()))),
                components: l.components,
            }),
            Content::Split(s) => Content::Split(SplitSpec {
                axis: s.axis,
                panes: s
                    .panes
                    .into_iter()
                    .map(|p| Pane {
                        content: p.content.map_out(f.clone()),
                        fixed: p.fixed,
                        weight: p.weight,
                        min: p.min,
                        max: p.max,
                        collapsed: p.collapsed,
                        collapsible: p.collapsible,
                        priority: p.priority,
                        key: p.key,
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
            children: self.children.into_iter().map(|c| c.map_out(f.clone())).collect(),
            content,
            handlers: self.handlers.map(f),
            behavior: self.behavior,
            focusable: self.focusable,
            autofocus: self.autofocus,
            disabled: self.disabled,
            hit_slop: self.hit_slop,
            pointer_events: self.pointer_events,
            tooltip: self.tooltip,
            follow_end: self.follow_end,
            sem: self.sem,
            debug: self.debug,
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
    ///
    /// ```
    /// use charis_ui::prelude::*;
    /// use charis_ui::Style;
    /// let panel = Style { padding: Edges::all(12.0), opacity: 0.9, ..Default::default() };
    /// let e: Element<()> = div().style(panel).rounded(6.0);
    /// ```
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

    setter!(
        /// Main axis for children (`flex-direction`).
        direction, direction, Direction);
    setter!(
        /// Layout mode (`display`): flex, grid or none.
        display, display, Display);
    setter!(
        /// How much of the free space this element takes (`flex-grow`).
        grow, grow, f32);
    setter!(
        /// How much this element shrinks when space is short (`flex-shrink`).
        shrink, shrink, f32);
    setter!(
        /// Positioning scheme (`position`).
        position, position, Position);
    setter!(
        /// What happens to content that doesn't fit (`overflow`).
        overflow, overflow, Overflow);
    setter!(
        /// Paint order among siblings (`z-index`); higher is drawn on top.
        z_index, z_index, i32);

    /// Lay out children horizontally (`flex-direction: row`).
    pub fn flex_row(self) -> Self {
        self.direction(Direction::Row)
    }
    /// Lay out children vertically (`flex-direction: column`).
    pub fn flex_col(self) -> Self {
        self.direction(Direction::Column)
    }
    /// Wrap children onto new lines when they don't fit (`flex-wrap: wrap`).
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
    /// Initial main-axis size before growing/shrinking (`flex-basis`).
    pub fn basis(mut self, v: impl Into<Length>) -> Self {
        self.style.basis = v.into();
        self
    }
    /// Space between children on both axes, in logical px (`gap`).
    pub fn gap(mut self, v: f32) -> Self {
        self.style.gap = (v, v);
        self
    }
    /// Separate column (`x`) and row (`y`) gaps, in logical px.
    pub fn gap_xy(mut self, x: f32, y: f32) -> Self {
        self.style.gap = (x, y);
        self
    }
    /// Main-axis distribution of children (`justify-content`).
    pub fn justify(mut self, j: Justify) -> Self {
        self.style.justify = Some(j);
        self
    }
    /// Cross-axis alignment of children (`align-items`).
    pub fn items(mut self, a: Align) -> Self {
        self.style.align_items = Some(a);
        self
    }
    /// Cross-axis alignment of this element in its parent (`align-self`).
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
    /// `justify-content: space-between`.
    pub fn justify_between(self) -> Self {
        self.justify(Justify::SpaceBetween)
    }

    /// Width: a number is logical px; also [`Length`] / [`pct`].
    pub fn w(mut self, v: impl Into<Length>) -> Self {
        self.style.width = v.into();
        self
    }
    /// Height: a number is logical px; also [`Length`] / [`pct`].
    pub fn h(mut self, v: impl Into<Length>) -> Self {
        self.style.height = v.into();
        self
    }
    /// Width and height.
    pub fn size(self, w: impl Into<Length>, h: impl Into<Length>) -> Self {
        self.w(w).h(h)
    }
    /// Square size.
    pub fn square(self, v: f32) -> Self {
        self.w(v).h(v)
    }
    /// `width: 100%`.
    pub fn w_full(self) -> Self {
        self.w(Length::Percent(100.0))
    }
    /// `height: 100%`.
    pub fn h_full(self) -> Self {
        self.h(Length::Percent(100.0))
    }
    /// `width: 100%; height: 100%`.
    pub fn size_full(self) -> Self {
        self.w_full().h_full()
    }
    /// Minimum width (`min-width`).
    pub fn min_w(mut self, v: impl Into<Length>) -> Self {
        self.style.min_width = v.into();
        self
    }
    /// Minimum height (`min-height`).
    pub fn min_h(mut self, v: impl Into<Length>) -> Self {
        self.style.min_height = v.into();
        self
    }
    /// Maximum width (`max-width`).
    pub fn max_w(mut self, v: impl Into<Length>) -> Self {
        self.style.max_width = v.into();
        self
    }
    /// Maximum height (`max-height`).
    pub fn max_h(mut self, v: impl Into<Length>) -> Self {
        self.style.max_height = v.into();
        self
    }
    /// Width / height ratio (`aspect-ratio`).
    pub fn aspect_ratio(mut self, r: f32) -> Self {
        self.style.aspect_ratio = Some(r);
        self
    }

    /// Padding on all sides, in logical px.
    pub fn p(mut self, v: f32) -> Self {
        self.style.padding = Edges::all(v);
        self
    }
    /// Left and right padding.
    pub fn px(mut self, v: f32) -> Self {
        self.style.padding.left = v;
        self.style.padding.right = v;
        self
    }
    /// Top and bottom padding.
    pub fn py(mut self, v: f32) -> Self {
        self.style.padding.top = v;
        self.style.padding.bottom = v;
        self
    }
    /// Top padding.
    pub fn pt(mut self, v: f32) -> Self {
        self.style.padding.top = v;
        self
    }
    /// Bottom padding.
    pub fn pb(mut self, v: f32) -> Self {
        self.style.padding.bottom = v;
        self
    }
    /// Left padding.
    pub fn pl(mut self, v: f32) -> Self {
        self.style.padding.left = v;
        self
    }
    /// Right padding.
    pub fn pr(mut self, v: f32) -> Self {
        self.style.padding.right = v;
        self
    }
    /// Padding per side.
    pub fn padding(mut self, e: Edges) -> Self {
        self.style.padding = e;
        self
    }
    /// Margin on all sides, in logical px.
    pub fn m(mut self, v: f32) -> Self {
        self.style.margin = Edges::all(v);
        self
    }
    /// Left and right margin.
    pub fn mx(mut self, v: f32) -> Self {
        self.style.margin.left = v;
        self.style.margin.right = v;
        self
    }
    /// Top and bottom margin.
    pub fn my(mut self, v: f32) -> Self {
        self.style.margin.top = v;
        self.style.margin.bottom = v;
        self
    }
    /// Top margin.
    pub fn mt(mut self, v: f32) -> Self {
        self.style.margin.top = v;
        self
    }
    /// Bottom margin.
    pub fn mb(mut self, v: f32) -> Self {
        self.style.margin.bottom = v;
        self
    }
    /// Left margin.
    pub fn ml(mut self, v: f32) -> Self {
        self.style.margin.left = v;
        self
    }
    /// Right margin.
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
    /// `top` inset for absolute/fixed positioning.
    pub fn top(mut self, v: impl Into<Length>) -> Self {
        self.style.inset[0] = v.into();
        self
    }
    /// `right` inset for absolute/fixed positioning.
    pub fn right(mut self, v: impl Into<Length>) -> Self {
        self.style.inset[1] = v.into();
        self
    }
    /// `bottom` inset for absolute/fixed positioning.
    pub fn bottom(mut self, v: impl Into<Length>) -> Self {
        self.style.inset[2] = v.into();
        self
    }
    /// `left` inset for absolute/fixed positioning.
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
    /// Row tracks for a [`grid`](Self::grid) (`grid-template-rows`).
    pub fn grid_rows(mut self, rows: Vec<Track>) -> Self {
        self.style.grid_rows = rows;
        self
    }
    /// Number of grid columns this child spans.
    pub fn col_span(mut self, n: u16) -> Self {
        self.style.grid_column_span = n;
        self
    }
    /// Number of grid rows this child spans.
    pub fn row_span(mut self, n: u16) -> Self {
        self.style.grid_row_span = n;
        self
    }
    /// `display: none` when `hide` is true: takes no space and isn't drawn.
    pub fn hidden(mut self, hide: bool) -> Self {
        if hide {
            self.style.display = Display::None;
        }
        self
    }

    // --------------------------------------------------------------- visuals

    /// Background color or gradient.
    pub fn bg(mut self, f: impl Into<Fill>) -> Self {
        self.style.background = Some(f.into());
        self
    }
    /// Linear gradient background (CSS angle semantics).
    pub fn gradient(self, angle: f32, stops: impl IntoIterator<Item = (f32, Color)>) -> Self {
        self.bg(Fill::linear(angle, stops))
    }
    /// Border of `width` logical px on all sides.
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.style.border_width = Edges::all(width);
        self.style.border_color = color;
        self
    }
    /// Border with a width per side.
    pub fn border_widths(mut self, e: Edges, color: Color) -> Self {
        self.style.border_width = e;
        self.style.border_color = color;
        self
    }
    /// Top border (sets the color of the whole border).
    pub fn border_t(mut self, w: f32, color: Color) -> Self {
        self.style.border_width.top = w;
        self.style.border_color = color;
        self
    }
    /// Bottom border (sets the color of the whole border).
    pub fn border_b(mut self, w: f32, color: Color) -> Self {
        self.style.border_width.bottom = w;
        self.style.border_color = color;
        self
    }
    /// Left border (sets the color of the whole border).
    pub fn border_l(mut self, w: f32, color: Color) -> Self {
        self.style.border_width.left = w;
        self.style.border_color = color;
        self
    }
    /// Right border (sets the color of the whole border).
    pub fn border_r(mut self, w: f32, color: Color) -> Self {
        self.style.border_width.right = w;
        self.style.border_color = color;
        self
    }
    /// Border color, keeping the widths.
    pub fn border_color(mut self, c: Color) -> Self {
        self.style.border_color = c;
        self
    }
    /// Corner radius on all corners, in logical px.
    pub fn rounded(mut self, r: f32) -> Self {
        self.style.radius = Corners::all(r);
        self
    }
    /// Corner radius per corner.
    pub fn radius(mut self, c: Corners) -> Self {
        self.style.radius = c;
        self
    }
    /// Fully rounded ("pill") corners.
    pub fn pill(self) -> Self {
        self.rounded(9999.0)
    }
    /// Add a box shadow (`box-shadow`); may be called repeatedly.
    pub fn shadow(mut self, s: Shadow) -> Self {
        self.style.shadows.push(s);
        self
    }
    /// Remove all box shadows (e.g. to flatten a widget from a style class).
    pub fn no_shadow(mut self) -> Self {
        self.style.shadows.clear();
        self
    }
    /// Add several box shadows.
    pub fn shadows(mut self, s: impl IntoIterator<Item = Shadow>) -> Self {
        self.style.shadows.extend(s);
        self
    }
    /// Opacity of the element and its children, 0.0–1.0.
    pub fn opacity(mut self, o: f32) -> Self {
        self.style.opacity = o;
        self
    }
    /// Outline drawn `offset` px outside the border box (CSS `outline`).
    pub fn outline(mut self, width: f32, offset: f32, color: Color) -> Self {
        self.style.outline = Some(Outline { width, offset, color });
        self
    }
    /// Paint-time offset in logical px (`transform: translate`); doesn't affect layout.
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.style.translate = (x, y);
        self
    }
    /// Mouse cursor shown while hovering.
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
    ///
    /// ```
    /// use charis_ui::prelude::*;
    /// let item: Element<()> = row()
    ///     .p(6.0)
    ///     .rounded(4.0)
    ///     .transition(0.12)
    ///     .hover(|s| s.bg(hex("#2a2d2e")))
    ///     .active(|s| s.bg(hex("#37373d")));
    /// ```
    pub fn hover(mut self, f: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.hover = Some(Box::new(f(self.hover.take().map(|b| *b).unwrap_or_default())));
        self
    }
    /// Style applied while pressed.
    pub fn active(mut self, f: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.active = Some(Box::new(f(self.active.take().map(|b| *b).unwrap_or_default())));
        self
    }
    /// Style applied while keyboard-focused.
    pub fn focus_style(mut self, f: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.focus = Some(Box::new(f(self.focus.take().map(|b| *b).unwrap_or_default())));
        self
    }
    /// Style applied while disabled.
    pub fn disabled_style(mut self, f: impl FnOnce(StylePatch) -> StylePatch) -> Self {
        self.disabled_style = Some(Box::new(f(self.disabled_style.take().map(|b| *b).unwrap_or_default())));
        self
    }

    // ------------------------------------------------------------------ text

    /// Text and icon color (inherited by children).
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
    /// Font weight.
    pub fn weight(mut self, w: Weight) -> Self {
        self.style.font_weight = Some(w);
        self
    }
    /// Medium (500) font weight.
    pub fn medium(self) -> Self {
        self.weight(Weight::MEDIUM)
    }
    /// Semibold (600) font weight.
    pub fn semibold(self) -> Self {
        self.weight(Weight::SEMIBOLD)
    }
    /// Bold (700) font weight.
    pub fn bold(self) -> Self {
        self.weight(Weight::BOLD)
    }
    /// Italic text.
    pub fn italic(mut self) -> Self {
        self.style.italic = Some(true);
        self
    }
    /// Monospace font.
    pub fn mono(mut self) -> Self {
        self.style.font_family = Some(FontFamily::Mono);
        self
    }
    /// Font family.
    pub fn font(mut self, f: FontFamily) -> Self {
        self.style.font_family = Some(f);
        self
    }
    /// Line height as a multiple of the font size.
    pub fn line_height(mut self, lh: f32) -> Self {
        self.style.line_height = Some(lh);
        self
    }
    /// Extra space between letters, in logical px.
    pub fn letter_spacing(mut self, v: f32) -> Self {
        self.style.letter_spacing = Some(v);
        self
    }
    /// Horizontal text alignment (`text-align`).
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
        self.handlers.link = Some(cb(f));
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

    /// Send `m` when clicked (or activated with Enter/Space while focused).
    /// Sets a pointer cursor unless one is set.
    ///
    /// ```
    /// use charis_ui::prelude::*;
    /// #[derive(Clone)]
    /// enum Msg { Select(usize) }
    /// let items: Vec<Element<Msg>> =
    ///     (0..3).map(|i| row().p(6.0).child(text(format!("Item {i}"))).on_click(Msg::Select(i))).collect();
    /// ```
    pub fn on_click(mut self, m: M) -> Self {
        self.handlers.click = Some(Out::Msg(m));
        if self.style.cursor.is_none() {
            self.style.cursor = Some(Cursor::Pointer);
        }
        self
    }
    /// Send `m` on double click.
    pub fn on_double_click(mut self, m: M) -> Self {
        self.handlers.double_click = Some(Out::Msg(m));
        self
    }
    /// Right click; receives the pointer position (for context menus).
    pub fn on_context_menu(mut self, f: impl Fn(Point) -> M + 'static) -> Self {
        self.handlers.context_menu = Some(cb(f));
        self
    }
    /// Called with `true` when the pointer enters and `false` when it leaves.
    pub fn on_hover(mut self, f: impl Fn(bool) -> M + 'static) -> Self {
        self.handlers.hover = Some(cb(f));
        self
    }
    /// Receive pointer drag events that start on this element.
    ///
    /// ```
    /// use charis_ui::prelude::*;
    /// #[derive(Clone)]
    /// enum Msg { DragStart, DragBy(Point), DragEnd }
    /// let handle: Element<Msg> = div().square(12.0).on_drag(|e| match e.phase {
    ///     DragPhase::Start => Msg::DragStart,
    ///     DragPhase::Move => Msg::DragBy(e.delta),
    ///     DragPhase::End => Msg::DragEnd,
    /// });
    /// ```
    pub fn on_drag(mut self, f: impl Fn(DragEvent) -> M + 'static) -> Self {
        self.handlers.drag = Some(cb(f));
        self
    }
    /// Make this element a drop target for drags started with [`Element::on_drag`].
    /// Receives `Over` while hovered during a drag, `Leave`, and `Drop` on release.
    ///
    /// ```
    /// use charis_ui::prelude::*;
    /// #[derive(Clone)]
    /// enum Msg { Highlight(bool), MoveTo(usize) }
    /// let slot: Element<Msg> = div().h(40.0).on_drop_target(|e| match e.phase {
    ///     DropPhase::Over => Msg::Highlight(true),
    ///     DropPhase::Leave => Msg::Highlight(false),
    ///     DropPhase::Drop => Msg::MoveTo(2),
    /// });
    /// ```
    pub fn on_drop_target(mut self, f: impl Fn(DropEvent) -> M + 'static) -> Self {
        self.handlers.drop_target = Some(cb(f));
        self
    }
    /// Key presses while this element (or a descendant) has focus.
    pub fn on_key(mut self, f: impl Fn(&KeyEvent) -> Option<M> + 'static) -> Self {
        self.handlers.key = Some(Rc::new(move |e: &KeyEvent| f(e).map(Out::Msg)));
        self
    }
    /// Like [`on_key`](Self::on_key), but runs first, while the element or
    /// a descendant has focus: before text inputs, key bindings and menu
    /// shortcuts. For widgets that own the keyboard, such as a shortcut
    /// recorder or a list driven from its search field.
    pub fn on_key_capture(mut self, f: impl Fn(&KeyEvent) -> Option<M> + 'static) -> Self {
        self.handlers.key_capture = Some(Rc::new(move |e: &KeyEvent| f(e).map(Out::Msg)));
        self
    }
    /// Called when the user pastes an image (Ctrl/Cmd+V with an image on
    /// the clipboard) while this element or a descendant has focus.
    pub fn on_paste_image(mut self, f: impl Fn(crate::image::Image) -> M + 'static) -> Self {
        self.handlers.paste_image = Some(cb(f));
        self
    }
    /// Called with `true` when this element gets keyboard focus and `false`
    /// when it loses it (focus and blur).
    pub fn on_focus_change(mut self, f: impl Fn(bool) -> M + 'static) -> Self {
        self.handlers.focus = Some(cb(f));
        self
    }
    /// For split containers: called when the user collapses/expands a pane by
    /// dragging its splitter. Receives `(pane_index, collapsed)`.
    pub fn on_collapse(mut self, f: impl Fn(usize, bool) -> M + 'static) -> Self {
        self.handlers.collapse = Some(cb(move |(i, c)| f(i, c)));
        self
    }
    /// For split containers: called with the new pane sizes after the user
    /// finishes resizing (useful for persisting layouts).
    pub fn on_resize(mut self, f: impl Fn(Vec<f32>) -> M + 'static) -> Self {
        self.handlers.resize = Some(cb(f));
        self
    }
    /// Make the element reachable with Tab and focusable by click.
    pub fn focusable(mut self) -> Self {
        self.focusable = true;
        self
    }
    /// Focus this element when it appears (e.g. the first item of a menu
    /// or the input of a dialog). Implies [`focusable`](Self::focusable).
    pub fn autofocus(mut self) -> Self {
        self.focusable = true;
        self.autofocus = true;
        self
    }
    /// Disable interaction: handlers don't fire, it can't be focused, and [`disabled_style`](Self::disabled_style) applies.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    /// Extend the hit-test area beyond the visual bounds.
    pub fn hit_slop(mut self, v: f32) -> Self {
        self.hit_slop = v;
        self
    }
    /// `pointer-events: none` when false. Like CSS, it's inherited, and a
    /// descendant can opt back in with `pointer_events(true)` (e.g. a
    /// dialog inside a click-through overlay).
    pub fn pointer_events(mut self, v: bool) -> Self {
        self.pointer_events = Some(v);
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

    /// Apply the current theme's style class `name` (see
    /// [`Theme::style_class`](crate::Theme::style_class)). Unknown names do
    /// nothing, so widgets and apps can tag elements freely.
    ///
    /// ```
    /// use charis_ui::prelude::*;
    /// // Register the class on the app's theme...
    /// let th = Theme::dark().style_class("toolbar", |e| e.px(8.0).gap(4.0).bg(hex("#252526")));
    /// // ...then tag elements with it.
    /// let bar: Element<()> = row().class("toolbar").child(text("File"));
    /// # let _ = th;
    /// ```
    pub fn class(mut self, name: &str) -> Self {
        if recording() {
            self.debug.get_or_insert_with(Default::default).classes.push(name.to_string());
        }
        let th = crate::theme::theme();
        let Some(fns) = th.classes.get(name) else { return self };
        for f in fns {
            let mut proxy: Element<()> = Element::new(Content::None);
            proxy.style = std::mem::take(&mut self.style);
            proxy.hover = self.hover.take();
            proxy.active = self.active.take();
            proxy.focus = self.focus.take();
            proxy.disabled_style = self.disabled_style.take();
            let out = f(proxy);
            self.style = out.style;
            self.hover = out.hover;
            self.active = out.active;
            self.focus = out.focus;
            self.disabled_style = out.disabled_style;
        }
        self
    }

    /// What this element is to assistive technology (like ARIA `role`).
    pub fn role(mut self, role: Role) -> Self {
        self.sem_mut().role = Some(role);
        self
    }
    /// Accessible name (like `aria-label`). Defaults to the element's text.
    pub fn aria_label(mut self, label: impl Into<String>) -> Self {
        self.sem_mut().label = Some(label.into());
        self
    }
    /// Longer accessible description (like `aria-description`).
    pub fn aria_description(mut self, d: impl Into<String>) -> Self {
        self.sem_mut().description = Some(d.into());
        self
    }
    /// Checked / on state for checkboxes, switches and radio buttons.
    pub fn aria_checked(mut self, on: bool) -> Self {
        self.sem_mut().checked = Some(on);
        self
    }
    /// Selected state for tabs, list items and rows.
    pub fn aria_selected(mut self, on: bool) -> Self {
        self.sem_mut().selected = Some(on);
        self
    }
    /// Expanded state for tree items and disclosure controls.
    pub fn aria_expanded(mut self, on: bool) -> Self {
        self.sem_mut().expanded = Some(on);
        self
    }
    /// A numeric value with its range, e.g. for progress bars.
    pub fn aria_value(mut self, value: f64, min: f64, max: f64) -> Self {
        self.sem_mut().value = Some((value, min, max));
        self
    }
    /// Mark as a heading of `level` 1–6.
    pub fn heading(mut self, level: u8) -> Self {
        let s = self.sem_mut();
        s.role = Some(Role::Heading);
        s.level = Some(level.clamp(1, 6));
        self
    }
    /// Mark as a modal dialog.
    pub fn aria_modal(mut self) -> Self {
        let s = self.sem_mut();
        s.role = Some(Role::Dialog);
        s.modal = true;
        self
    }
    /// Hide this subtree from assistive technology (decorations, duplicates).
    pub fn aria_hidden(mut self) -> Self {
        self.sem_mut().hidden = true;
        self
    }

    /// For [`virtual_list`]: the height of rows that haven't been measured yet.
    /// When every row has this exact height, scrolling never has to correct.
    pub fn item_height(mut self, h: f32) -> Self {
        if let Content::Virtual(v) = &mut self.content {
            v.estimate = h.max(1.0);
        }
        self
    }

    /// For [`virtual_list`]: how many extra pixels of rows to build above and
    /// below the viewport (default 300).
    pub fn overscan(mut self, px: f32) -> Self {
        if let Content::Virtual(v) = &mut self.content {
            v.overscan = px.max(0.0);
        }
        self
    }

    /// Called whenever this scroll container's position changes.
    pub fn on_scroll(mut self, f: impl Fn(ScrollInfo) -> M + 'static) -> Self {
        self.handlers.scroll = Some(cb(f));
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
