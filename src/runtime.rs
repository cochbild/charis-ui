//! The platform-independent runtime: builds frames from the app's view,
//! lays them out, paints them and routes input events.
//!
//! The windowing shell (`window` module) and the headless renderer (`headless`
//! module) both drive a [`Runtime`].

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use taffy as tf;
use tiny_skia::Pixmap;

use crate::anim::Anim;
use crate::color::{Color, Fill};
use crate::cpu::PaintCache;
use crate::edit::{self, Selection};
use crate::effects::{Proxy, TaskHandle};
use crate::element::*;
use crate::geometry::{Axis, Point, Rect, Size};
use crate::icons::Icon;
use crate::paint::Canvas;
use crate::scene::Scene;
use crate::style::*;
use crate::subscription::Subscriptions;
use crate::text::{TextStyle, TextSystem};
use crate::theme::{self, Theme};

/// An application.
///
/// The runtime calls [`App::view`] to build the UI whenever something changed
/// and [`App::update`] for every message produced by user interaction.
pub trait App: 'static {
    type Msg: Clone + 'static;

    /// Handle a message. Use `cx` for side effects: async tasks, window
    /// actions, focus, scrolling and the clipboard.
    fn update(&mut self, msg: Self::Msg, cx: &mut Cx<Self::Msg>);

    /// Describe the UI for the current state.
    fn view(&self) -> Element<Self::Msg>;

    /// The theme to use. Called once per frame, so it can change at runtime.
    fn theme(&self) -> Theme {
        Theme::dark()
    }

    /// Timers and window events to listen to. Re-evaluated after every update.
    fn subscriptions(&self) -> Subscriptions<Self::Msg> {
        Subscriptions::none()
    }

    /// Global keyboard shortcuts, called for keys not handled by the focused element.
    fn on_key(&self, _event: &KeyEvent) -> Option<Self::Msg> {
        None
    }
}

/// Resize direction for frameless window edge dragging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

/// Requests from the runtime to the windowing shell.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowRequest {
    DragMove,
    DragResize(ResizeEdge),
    Minimize,
    ToggleMaximize,
    Close,
    SetTitle(String),
    SetClipboard(String),
}

/// Scroll commands issued from [`App::update`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ScrollCmd {
    ToEnd,
    To(f32),
    /// Bring row `i` of a [`virtual_list`] to the top.
    ToItem(usize),
}

/// Context passed to [`App::update`] for side effects.
pub struct Cx<M> {
    pub(crate) requests: Vec<WindowRequest>,
    pub(crate) focus: Option<Option<u64>>,
    pub(crate) scrolls: Vec<(u64, ScrollCmd)>,
    pub(crate) proxy: Proxy<M>,
}

impl<M> Cx<M> {
    /// Close the window and exit the app.
    pub fn close_window(&mut self) {
        self.requests.push(WindowRequest::Close);
    }
    pub fn minimize(&mut self) {
        self.requests.push(WindowRequest::Minimize);
    }
    pub fn toggle_maximize(&mut self) {
        self.requests.push(WindowRequest::ToggleMaximize);
    }
    pub fn set_title(&mut self, t: impl Into<String>) {
        self.requests.push(WindowRequest::SetTitle(t.into()));
    }
    /// Move keyboard focus to the element with the given `.id(...)`.
    pub fn focus(&mut self, id: &str) {
        self.focus = Some(Some(crate::element::global_id(id)));
    }
    pub fn blur(&mut self) {
        self.focus = Some(None);
    }
    pub fn copy_to_clipboard(&mut self, s: impl Into<String>) {
        self.requests.push(WindowRequest::SetClipboard(s.into()));
    }
    /// Scroll the scroll container with this `.id(...)` to its end.
    pub fn scroll_to_end(&mut self, id: &str) {
        self.scrolls.push((crate::element::global_id(id), ScrollCmd::ToEnd));
    }
    /// Scroll the scroll container with this `.id(...)` to a vertical offset.
    pub fn scroll_to(&mut self, id: &str, y: f32) {
        self.scrolls.push((crate::element::global_id(id), ScrollCmd::To(y)));
    }
    /// Scroll a [`virtual_list`] with this `.id(...)` so row `index` is at the top.
    pub fn scroll_to_item(&mut self, id: &str, index: usize) {
        self.scrolls.push((crate::element::global_id(id), ScrollCmd::ToItem(index)));
    }
    /// A handle for sending messages from other threads.
    pub fn proxy(&self) -> Proxy<M> {
        self.proxy.clone()
    }
}

impl<M: Send + 'static> Cx<M> {
    /// Run a future in the background and deliver its output as a message.
    pub fn spawn<F>(&mut self, fut: F, map: impl FnOnce(F::Output) -> M + Send + 'static) -> TaskHandle
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send,
    {
        crate::effects::spawn(self.proxy.clone(), fut, map)
    }

    /// Consume a stream in the background, delivering each item as a message.
    pub fn run<S>(&mut self, stream: S, map: impl FnMut(S::Item) -> M + Send + 'static) -> TaskHandle
    where
        S: futures::Stream + Send + 'static,
        S::Item: Send,
    {
        crate::effects::run(self.proxy.clone(), stream, map)
    }

    /// Run blocking code (file I/O, heavy computation) on a background thread.
    pub fn spawn_blocking(&mut self, f: impl FnOnce() -> M + Send + 'static) -> TaskHandle {
        crate::effects::spawn_blocking(self.proxy.clone(), f)
    }
}

/// Information about the window, readable from views via [`window_info`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WindowInfo {
    pub maximized: bool,
    pub focused: bool,
}

thread_local! {
    static WINDOW_INFO: std::cell::Cell<WindowInfo> = const { std::cell::Cell::new(WindowInfo { maximized: false, focused: true }) };
}

/// The current window state (for drawing maximize/restore icons, etc.).
pub fn window_info() -> WindowInfo {
    WINDOW_INFO.with(|w| w.get())
}

pub(crate) fn set_window_info(i: WindowInfo) {
    WINDOW_INFO.with(|w| w.set(i));
}

/// What a window-space point is, from the OS window manager's perspective
/// (used for native frameless chrome: snap, system menu, Snap Layouts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeHit {
    /// Normal app content.
    Client,
    /// Title-bar drag area.
    Caption,
    /// The maximize/restore button.
    Maximize,
}

/// A snapshot of window-chrome regions, published after every frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChromeMap {
    /// Back-to-front regions in logical pixels; later entries win.
    pub regions: Vec<(Rect, ChromeHit)>,
}

impl ChromeMap {
    pub fn hit(&self, p: Point) -> ChromeHit {
        self.regions.iter().rev().find(|(r, _)| r.contains(p)).map(|(_, h)| *h).unwrap_or(ChromeHit::Client)
    }
}

/// Mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Input events fed to the runtime by the platform shell.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    PointerMove(Point),
    PointerDown(Point, MouseButton),
    PointerUp(Point, MouseButton),
    PointerLeave,
    /// Scroll delta in logical pixels (positive y scrolls content down).
    Wheel(Point, Point),
    Key(KeyEvent),
    /// Committed text input (typed characters, IME commits).
    Text(String),
    /// Paste from the system clipboard.
    Paste(String),
    WindowFocus(bool),
}

// ---------------------------------------------------------------------------
// Retained state
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
struct Visual {
    bg: Option<Fill>,
    border: Color,
    color: Option<Color>,
    shadows: Vec<Shadow>,
    opacity: f32,
    outline: Option<Outline>,
    radius: Corners,
    translate: (f32, f32),
}

impl Visual {
    fn of(s: &Style) -> Self {
        Visual {
            bg: s.background.clone(),
            border: s.border_color,
            color: s.color,
            shadows: s.shadows.clone(),
            opacity: s.opacity,
            outline: s.outline,
            radius: s.radius,
            translate: s.translate,
        }
    }
    fn write(&self, s: &mut Style) {
        s.background = self.bg.clone();
        s.border_color = self.border;
        s.color = self.color;
        s.shadows = self.shadows.clone();
        s.opacity = self.opacity;
        s.outline = self.outline;
        s.radius = self.radius;
        s.translate = self.translate;
    }
    fn lerp(&self, to: &Visual, t: f32) -> Visual {
        let mut s = Style::default();
        self.write(&mut s);
        let patch = StylePatch {
            background: to.bg.clone().or_else(|| self.bg.as_ref().map(|b| b.fade(0.0))),
            border_color: Some(to.border),
            color: to.color,
            shadows: Some(to.shadows.clone()),
            opacity: Some(to.opacity),
            outline: to.outline.or_else(|| self.outline.map(|o| Outline { color: o.color.with_alpha(0.0), ..o })),
            radius: Some(to.radius),
            translate: Some(to.translate),
            cursor: None,
        };
        s.apply_patch(&patch, t);
        Visual::of(&s)
    }
}

struct Transition {
    from: Visual,
    to: Visual,
    start: f64,
    dur: f32,
    seen: u64,
}

struct RectAnim {
    from: Rect,
    to: Rect,
    start: f64,
    dur: f32,
    seen: u64,
}

impl RectAnim {
    fn value(&self, now: f64, easing: crate::anim::Easing) -> Rect {
        let t = if self.dur > 0.0 { easing.apply(((now - self.start) as f32 / self.dur).clamp(0.0, 1.0)) } else { 1.0 };
        let l = |a: f32, b: f32| a + (b - a) * t;
        Rect::new(
            l(self.from.x, self.to.x),
            l(self.from.y, self.to.y),
            l(self.from.w, self.to.w),
            l(self.from.h, self.to.h),
        )
    }
}

struct SplitState {
    axis: Axis,
    /// px for fixed panes, flex weight for flex panes.
    sizes: Vec<f32>,
    fixed: Vec<bool>,
    initial: Vec<f32>,
    collapse: Vec<Anim>,
    container: f32,
    /// Laid-out main-axis size of each pane (last frame).
    pane_px: Vec<f32>,
}

impl SplitState {
    fn report(&self) -> Vec<f32> {
        self.sizes.clone()
    }
}

#[derive(Default)]
struct ScrollState {
    x: Option<Anim>,
    y: Option<Anim>,
    last_activity: f64,
    max: Point,
    /// For `follow_end` containers: currently pinned to the end.
    pinned: Option<bool>,
    last_reported: Option<Point>,
}

impl ScrollState {
    fn offset(&self, now: f64) -> Point {
        Point::new(self.x.map_or(0.0, |a| a.value(now)), self.y.map_or(0.0, |a| a.value(now)))
    }
    fn target(&self) -> Point {
        Point::new(self.x.map_or(0.0, |a| a.target()), self.y.map_or(0.0, |a| a.target()))
    }
}

/// Retained state of a [`virtual_list`]: measured row heights.
#[derive(Default)]
struct VirtState {
    /// Measured height per row; NaN = not measured yet (use `estimate`).
    heights: Vec<f32>,
    known_sum: f64,
    known_n: usize,
    estimate: f32,
    gap: f32,
    /// First row intersecting the viewport when the frame was built. Rows
    /// above it that change height shift the scroll offset to compensate.
    anchor: usize,
    /// Rows built this frame: index range and list-space extent.
    built: (usize, usize),
    built_px: (f32, f32),
    seen: u64,
}

impl VirtState {
    fn resize(&mut self, n: usize) {
        if n < self.heights.len() {
            for &h in &self.heights[n..] {
                if !h.is_nan() {
                    self.known_sum -= h as f64;
                    self.known_n -= 1;
                }
            }
        }
        self.heights.resize(n, f32::NAN);
    }
    fn h(&self, i: usize) -> f32 {
        match self.heights.get(i) {
            Some(h) if !h.is_nan() => *h,
            _ => self.estimate,
        }
    }
    fn set(&mut self, i: usize, h: f32) {
        let Some(old) = self.heights.get_mut(i) else { return };
        if old.is_nan() {
            self.known_n += 1;
        } else {
            self.known_sum -= *old as f64;
        }
        self.known_sum += h as f64;
        *old = h;
    }
    fn total(&self) -> f32 {
        let n = self.heights.len();
        if n == 0 {
            return 0.0;
        }
        (self.known_sum + (n - self.known_n) as f64 * self.estimate as f64) as f32 + self.gap * (n - 1) as f32
    }
    /// List-space top of row `i`.
    fn pos(&self, i: usize) -> f32 {
        (0..i.min(self.heights.len())).map(|j| self.h(j) + self.gap).sum()
    }
}

#[derive(Default)]
struct DropdownState {
    open: bool,
    filter: String,
    highlight: usize,
    /// Option indices currently shown (after filtering).
    visible: Vec<usize>,
    /// Copied from the spec on every frame.
    options: Vec<String>,
    selected: Option<usize>,
    searchable: bool,
}

#[derive(Default, Clone, Copy)]
struct InputState {
    sel: Selection,
    scroll: f32,
    /// Vertical scroll of multi-line inputs.
    scroll_y: f32,
    blink_start: f64,
}

enum Drag {
    None,
    /// Pressed, waiting to see if this becomes a click or a drag.
    Press {
        node: u64,
        start: Point,
        dragging: bool,
    },
    Splitter {
        split: u64,
        index: usize,
        start: Point,
        start_sizes: Vec<f32>,
        start_px: Vec<f32>,
        collapsed_emitted: bool,
    },
    Slider {
        node: u64,
    },
    ScrollThumb {
        node: u64,
        vertical: bool,
        start: Point,
        start_offset: f32,
    },
    TextSelect {
        node: u64,
    },
    /// Selecting read-only text.
    ReadSelect {
        node: u64,
    },
    /// Resizing a table column.
    Column {
        table: u64,
        col: usize,
        min: f32,
        start: Point,
        start_w: f32,
    },
}

// ---------------------------------------------------------------------------
// Frame
// ---------------------------------------------------------------------------

pub(crate) enum NodeContent {
    None,
    Text(TextSpec),
    Icon(Icon),
    Canvas(PaintFn),
    Input(InputSpec),
}

pub(crate) struct Node<M> {
    pub id: u64,
    /// Raw `.id()`/`.key()` hash, for global lookup.
    pub key: Option<u64>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub style: Style,
    pub content: NodeContent,
    pub handlers: Handlers<M>,
    pub behavior: Behavior,
    pub focusable: bool,
    pub disabled: bool,
    pub hit_slop: f32,
    pub pointer_events: bool,
    pub tooltip: Option<String>,
    pub follow_end: bool,
    pub text: TextStyle,
    pub color: Color,
    pub tnode: tf::NodeId,
    pub rect: Rect,
    pub clip: Option<Rect>,
    pub content_size: Size,
    /// Scroll offset applied to children.
    pub scroll: Point,
    pub split: Option<(u64, Axis)>,
    /// For split pane wrappers: (split id, pane index).
    pub pane: Option<(u64, usize)>,
    /// For virtual list rows: (list id, row index).
    pub virt_item: Option<(u64, usize)>,
}

struct Frame<M> {
    nodes: Vec<Node<M>>,
    by_id: HashMap<u64, usize>,
    /// Paint order (for reverse hit testing).
    order: Vec<usize>,
}

impl<M> Default for Frame<M> {
    fn default() -> Self {
        Self { nodes: Vec::new(), by_id: HashMap::new(), order: Vec::new() }
    }
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// Drives an [`App`]: layout, painting and event dispatch.
pub struct Runtime<A: App> {
    pub app: A,
    pub text: TextSystem,
    paint_cache: PaintCache,
    frame: Frame<A::Msg>,
    theme: Rc<Theme>,
    size: Size,
    scale: f32,
    now: f64,
    pointer: Option<Point>,
    hovered: Vec<u64>,
    hovered_set: HashSet<u64>,
    pressed: Vec<u64>,
    focused: Option<u64>,
    focus_visible: bool,
    drag: Drag,
    last_click: Option<(f64, Point, u64, u32)>,
    transitions: HashMap<u64, Transition>,
    rect_anims: HashMap<u64, RectAnim>,
    splits: HashMap<u64, SplitState>,
    pane_meta: HashMap<(u64, usize), (f32, f32, bool)>,
    scrolls: HashMap<u64, ScrollState>,
    inputs: HashMap<u64, InputState>,
    /// Values emitted by inputs since the last rebuild. Inputs are controlled
    /// (the app owns the value), so several keystrokes between frames must
    /// build on each other rather than on the last rendered value.
    pending_values: HashMap<u64, String>,
    dropdowns: HashMap<u64, DropdownState>,
    virt: HashMap<u64, VirtState>,
    /// User-resized table column widths, by table id.
    tables: HashMap<u64, Vec<Option<f32>>>,
    tooltip: Option<(u64, f64, Point)>,
    splitter_hover: Option<(u64, f64)>,
    /// Read-only text selection: (node id, anchor byte, cursor byte).
    text_sel: Option<(u64, usize, usize)>,
    drop_target: Option<u64>,
    queue: Vec<A::Msg>,
    mailbox: crate::effects::Mailbox<A::Msg>,
    subs: Subscriptions<A::Msg>,
    timers: HashMap<(u128, usize), f64>,
    pending_scrolls: Vec<(u64, ScrollCmd)>,
    close_requested: bool,
    sized: bool,
    requests: Vec<WindowRequest>,
    dirty: bool,
    frame_no: u64,
    cursor: Cursor,
    pixmap: Option<Pixmap>,
    scene: Option<Scene>,
    /// Window chrome is drawn by the app (no OS decorations).
    pub frameless: bool,
    pub maximized: bool,
    window_focused: bool,
    clipboard: String,
}

const DOUBLE_CLICK: f64 = 0.4;
const DRAG_THRESHOLD: f32 = 3.0;
const TOOLTIP_DELAY: f64 = 0.55;
const RESIZE_BORDER: f32 = 6.0;

impl<A: App> Runtime<A> {
    pub fn new(app: A) -> Self {
        let subs = app.subscriptions();
        Self {
            app,
            text: TextSystem::new(),
            paint_cache: PaintCache::default(),
            frame: Frame::default(),
            theme: Rc::new(Theme::dark()),
            size: Size::new(800.0, 600.0),
            scale: 1.0,
            now: 0.0,
            pointer: None,
            hovered: Vec::new(),
            hovered_set: HashSet::new(),
            pressed: Vec::new(),
            focused: None,
            focus_visible: false,
            drag: Drag::None,
            last_click: None,
            transitions: HashMap::new(),
            rect_anims: HashMap::new(),
            splits: HashMap::new(),
            pane_meta: HashMap::new(),
            scrolls: HashMap::new(),
            inputs: HashMap::new(),
            pending_values: HashMap::new(),
            dropdowns: HashMap::new(),
            virt: HashMap::new(),
            tables: HashMap::new(),
            tooltip: None,
            splitter_hover: None,
            text_sel: None,
            drop_target: None,
            queue: Vec::new(),
            mailbox: crate::effects::Mailbox::new(),
            subs,
            timers: HashMap::new(),
            pending_scrolls: Vec::new(),
            close_requested: false,
            sized: false,
            requests: Vec::new(),
            dirty: true,
            frame_no: 0,
            cursor: Cursor::Default,
            pixmap: None,
            scene: None,
            frameless: false,
            maximized: false,
            window_focused: true,
            clipboard: String::new(),
        }
    }

    /// Set the logical window size and scale factor.
    pub fn resize(&mut self, size: Size, scale: f32) {
        if size != self.size || scale != self.scale {
            // The first sizing is the initial window size, not a user resize.
            let changed = size != self.size && self.sized;
            self.sized = true;
            self.size = size;
            self.scale = scale;
            self.dirty = true;
            if changed {
                if let Some(f) = self.subs.resized.clone() {
                    self.queue.push(f(size));
                    self.flush();
                }
            }
        }
    }

    /// Install a callback that wakes the event loop when background tasks post
    /// messages (used by the window shell).
    pub fn set_waker(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut w) = self.mailbox.wake.lock() {
            *w = Some(std::sync::Arc::new(wake));
        }
    }

    /// A handle for sending messages to the app from other threads.
    pub fn proxy(&self) -> Proxy<A::Msg> {
        self.mailbox.proxy()
    }

    /// Deliver messages posted by background tasks and fire due timers.
    /// Call this whenever the event loop wakes (the shells do).
    pub fn poll(&mut self) {
        while let Ok(m) = self.mailbox.rx.try_recv() {
            self.queue.push(m);
        }
        self.fire_timers();
        self.flush();
    }

    fn fire_timers(&mut self) {
        let now = self.now;
        let mut live: HashMap<(u128, usize), f64> = HashMap::new();
        let mut per_period: HashMap<u128, usize> = HashMap::new();
        for (period, msg) in &self.subs.timers {
            let p = period.as_nanos();
            let n = per_period.entry(p).or_insert(0);
            let key = (p, *n);
            *n += 1;
            let secs = period.as_secs_f64();
            let mut due = self.timers.get(&key).copied().unwrap_or(now + secs);
            if now >= due {
                self.queue.push(msg.clone());
                // Skip missed ticks rather than bursting.
                due += ((now - due) / secs).floor() * secs + secs;
            }
            live.insert(key, due);
        }
        self.timers = live;
    }

    /// The window's close button was pressed. Returns true if the window
    /// should close now (no `on_close_request` subscription).
    pub fn request_close(&mut self) -> bool {
        match self.subs.close_requested.clone() {
            Some(m) => {
                self.close_requested = true;
                self.queue.push(m);
                self.flush();
                false
            }
            None => true,
        }
    }

    /// Advance the clock (seconds, monotonic).
    pub fn set_time(&mut self, t: f64) {
        self.now = t;
    }

    pub fn time(&self) -> f64 {
        self.now
    }

    /// Last known pointer position.
    pub fn pointer_pos(&self) -> Option<Point> {
        self.pointer
    }

    /// The theme used for the last frame.
    pub fn current_theme(&self) -> &Theme {
        &self.theme
    }

    /// Window size in logical pixels.
    pub fn logical_size(&self) -> Size {
        self.size
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn take_requests(&mut self) -> Vec<WindowRequest> {
        std::mem::take(&mut self.requests)
    }

    /// The internal clipboard (mirrors the system clipboard when available).
    pub fn clipboard(&self) -> &str {
        &self.clipboard
    }

    /// Mark the UI as needing a rebuild (e.g. after external state changes).
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Deliver a message to the app as if produced by the UI.
    pub fn send(&mut self, msg: A::Msg) {
        self.queue.push(msg);
        self.flush();
    }

    /// When the next frame should be drawn: `Some(0.0)` = now, `Some(t)` = at
    /// time `t`, `None` = only after the next event.
    pub fn next_frame(&self) -> Option<f64> {
        if self.dirty {
            return Some(0.0);
        }
        let now = self.now;
        let animating = self.transitions.values().any(|t| ((now - t.start) as f32) < t.dur)
            || self.rect_anims.values().any(|a| ((now - a.start) as f32) < a.dur)
            || self.splits.values().any(|s| s.collapse.iter().any(|a| a.is_animating(now)))
            || self.scrolls.values().any(|s| {
                s.x.is_some_and(|a| a.is_animating(now))
                    || s.y.is_some_and(|a| a.is_animating(now))
                    || (now - s.last_activity) < 1.2
            });
        if animating {
            return Some(0.0);
        }
        let mut next: Option<f64> =
            self.timers.values().copied().fold(None, |a: Option<f64>, t| Some(a.map_or(t, |a| a.min(t))));
        if !self.subs.timers.is_empty() && self.timers.is_empty() {
            next = Some(now);
        }
        if let Some((_, since)) = self.splitter_hover {
            let d = self.theme.splitter_hover_delay as f64;
            if now - since < d {
                next = Some(since + d);
            } else if now - since < d + 0.12 {
                return Some(0.0);
            }
        }
        if let Some((_, since, _)) = self.tooltip {
            if now - since < TOOLTIP_DELAY {
                next = Some(since + TOOLTIP_DELAY);
            }
        }
        if let Some(f) = self.focused {
            if let (Some(st), Some(&i)) = (self.inputs.get(&f), self.frame.by_id.get(&f)) {
                if matches!(self.frame.nodes[i].content, NodeContent::Input(_)) && self.window_focused {
                    let phase = now - st.blink_start;
                    let t = st.blink_start + ((phase / 0.53).floor() + 1.0) * 0.53;
                    next = Some(next.map_or(t, |n: f64| n.min(t)));
                }
            }
        }
        next
    }

    pub fn needs_redraw(&self) -> bool {
        self.next_frame().is_some_and(|t| t <= self.now)
    }

    /// Mutable access to the text system (for GPU backends that rasterize glyphs).
    pub fn text_system(&mut self) -> &mut TextSystem {
        &mut self.text
    }

    /// Run `f` with the last recorded scene and the text system (for GPU backends).
    pub fn with_scene<R>(&mut self, f: impl FnOnce(&Scene, &mut TextSystem) -> R) -> Option<R> {
        let scene = self.scene.as_ref()?;
        Some(f(scene, &mut self.text))
    }

    /// The last recorded display list.
    pub fn scene(&self) -> Option<&Scene> {
        self.scene.as_ref()
    }

    /// The last rendered image.
    pub fn pixmap(&self) -> Option<&Pixmap> {
        self.pixmap.as_ref()
    }

    // ------------------------------------------------------------ rendering

    /// Rebuild, lay out and paint a frame on the CPU. Returns the rendered pixmap.
    pub fn render(&mut self) -> &Pixmap {
        self.render_scene();
        let pm = match &self.scene {
            Some(scene) => crate::cpu::render_cpu(scene, self.pixmap.take(), &mut self.text, &mut self.paint_cache),
            None => Pixmap::new(1, 1).unwrap_or_else(|| unreachable!()),
        };
        self.pixmap.insert(pm)
    }

    /// Rebuild, lay out and record a frame's display list without rasterizing
    /// it (for GPU backends).
    pub fn render_scene(&mut self) -> &Scene {
        self.frame_no += 1;
        self.text.begin_frame();
        let t0 = std::time::Instant::now();
        self.build();
        let t1 = t0.elapsed();
        self.dirty = false;
        // Hover may have changed because layout moved things under the pointer.
        if matches!(self.drag, Drag::None) {
            if let Some(p) = self.pointer {
                if self.update_hover(p) {
                    self.build();
                }
            }
        }
        self.update_input_scrolls();
        let t2 = std::time::Instant::now();
        self.paint();
        // Messages produced during layout (scroll reports) are applied now and
        // show up in the next frame.
        if !self.queue.is_empty() {
            self.flush();
        }
        if std::env::var("RUI_PROFILE").is_ok() {
            eprintln!("build {:?} record {:?} nodes {}", t1, t2.elapsed(), self.frame.nodes.len());
        }
        self.scene.get_or_insert_with(|| Scene::new(1, 1, 1.0, Color::TRANSPARENT))
    }

    /// Keep each text input's caret visible by adjusting its horizontal scroll.
    fn update_input_scrolls(&mut self) {
        for n in &self.frame.nodes {
            let NodeContent::Input(spec) = &n.content else { continue };
            let Some(st) = self.inputs.get_mut(&n.id) else { continue };
            let cr = content_rect(n);
            if spec.multiline {
                st.sel.clamp(&spec.value);
                let w = Some(cr.w.max(1.0));
                let caret = self.text.caret_rect(&spec.value, &n.text, w, self.scale, st.sel.cursor);
                let total = self.text.measure(&spec.value, &n.text, w, self.scale).h;
                let mut sy = st.scroll_y;
                if caret.bottom() - sy > cr.h {
                    sy = caret.bottom() - cr.h;
                }
                if caret.y - sy < 0.0 {
                    sy = caret.y;
                }
                st.scroll_y = sy.clamp(0.0, (total - cr.h).max(0.0));
                continue;
            }
            let shown = display_value(spec);
            let stops = self.text.caret_stops(&shown, &n.text, self.scale);
            st.sel.clamp(&spec.value);
            let cb = map_value_index(spec, st.sel.cursor);
            let caret_x = stops.iter().find(|s| s.0 >= cb).map(|s| s.1).unwrap_or(0.0);
            let total = stops.last().map(|s| s.1).unwrap_or(0.0);
            let mut scroll = st.scroll;
            if caret_x - scroll > cr.w - 2.0 {
                scroll = caret_x - cr.w + 2.0;
            }
            if caret_x - scroll < 0.0 {
                scroll = caret_x;
            }
            if total - scroll < cr.w - 2.0 {
                scroll = (total - cr.w + 2.0).max(0.0);
            }
            st.scroll = scroll.max(0.0);
        }
    }

    fn build(&mut self) {
        // The app has seen every emitted value by now; the view is authoritative again.
        self.pending_values.clear();
        self.theme = Rc::new(self.app.theme());
        theme::set_theme(self.theme.clone());
        let view = self.app.view();
        let th = self.theme.clone();
        let root = div()
            .id("__root")
            .size(self.size.w, self.size.h)
            .flex_col()
            .bg(th.colors.background)
            .color(th.colors.text)
            .font_size(th.font_size)
            .font(th.font.clone())
            .line_height(th.line_height)
            .child(view.grow(1.0).min_h(0.0).min_w(0.0));
        let mut frame = Frame::default();
        let base = TextStyle::default();
        self.flatten(root, None, 0x5eed, 0, &base, Color::WHITE, true, &mut frame);
        self.layout(&mut frame);
        frame.order = paint_order(&frame.nodes);
        self.frame = frame;
        self.transitions.retain(|_, t| t.seen + 2 >= self.frame_no);
        self.rect_anims.retain(|_, t| t.seen + 2 >= self.frame_no);
        self.virt.retain(|_, v| v.seen + 120 >= self.frame_no);
    }

    #[allow(clippy::too_many_arguments)]
    fn flatten(
        &mut self,
        el: Element<A::Msg>,
        parent: Option<usize>,
        parent_id: u64,
        index: usize,
        inh_text: &TextStyle,
        inh_color: Color,
        inh_pointer: bool,
        frame: &mut Frame<A::Msg>,
    ) -> usize {
        let raw_key = el.key;
        let id = match el.key {
            Some(k) => mix_id(parent_id, k),
            None => mix_id(parent_id, index as u64 ^ 0xA5A5_0000),
        };
        let Element {
            style: mut st,
            hover,
            active,
            focus,
            disabled_style,
            children,
            content,
            handlers,
            behavior,
            focusable,
            disabled,
            hit_slop,
            pointer_events,
            tooltip,
            follow_end,
            ..
        } = el;

        // Interaction states.
        let is_hover = self.hovered_set.contains(&id) && !disabled;
        let is_active = self.pressed.contains(&id) && !disabled;
        let is_focus = self.focused == Some(id);
        if let Some(p) = &hover {
            if is_hover {
                st.apply_patch(p, 1.0);
            }
        }
        if let Some(p) = &focus {
            if is_focus && (self.focus_visible || matches!(content, Content::Input(_))) {
                st.apply_patch(p, 1.0);
            }
        }
        if let Some(p) = &active {
            if is_active {
                st.apply_patch(p, 1.0);
            }
        }
        if disabled {
            match &disabled_style {
                Some(p) => st.apply_patch(p, 1.0),
                None => st.opacity *= 0.5,
            }
        }
        // Table cells follow their column's user-resized width.
        if let Behavior::TableCell { table, col } = behavior {
            if let Some(w) = self.tables.get(&table).and_then(|t| t.get(col).copied().flatten()) {
                st.width = Length::Px(w);
                st.basis = Length::Auto;
                st.grow = 0.0;
                st.shrink = 0.0;
            }
        }
        // CSS-like transitions.
        if st.transition > 0.0 {
            let target = Visual::of(&st);
            let now = self.now;
            let fno = self.frame_no;
            let tr = self.transitions.entry(id).or_insert_with(|| Transition {
                from: target.clone(),
                to: target.clone(),
                start: now,
                dur: 0.0,
                seen: fno,
            });
            tr.seen = fno;
            if tr.to != target {
                let t = if tr.dur > 0.0 { ((now - tr.start) as f32 / tr.dur).clamp(0.0, 1.0) } else { 1.0 };
                let cur = tr.from.lerp(&tr.to, st.easing.apply(t));
                tr.from = cur;
                tr.to = target;
                tr.start = now;
                tr.dur = st.transition;
            }
            let t = if tr.dur > 0.0 { ((now - tr.start) as f32 / tr.dur).clamp(0.0, 1.0) } else { 1.0 };
            let v = tr.from.lerp(&tr.to, st.easing.apply(t));
            v.write(&mut st);
        }

        // Text inheritance.
        let text = TextStyle {
            size: st.font_size.unwrap_or(inh_text.size),
            weight: st.font_weight.map(|w| w.0).unwrap_or(inh_text.weight),
            family: st.font_family.clone().unwrap_or_else(|| inh_text.family.clone()),
            italic: st.italic.unwrap_or(inh_text.italic),
            line_height: st.line_height.unwrap_or(inh_text.line_height),
            letter_spacing: st.letter_spacing.unwrap_or(inh_text.letter_spacing),
        };
        let color = st.color.unwrap_or(inh_color);
        if st.text_align.is_none() {
            if let Some(p) = parent {
                st.text_align = frame.nodes[p].style.text_align;
            }
        }
        let pointer = inh_pointer && pointer_events;

        let mut dropdown_spec: Option<DropdownSpec> = None;
        let mut virtual_spec: Option<VirtualSpec<A::Msg>> = None;
        let (node_content, split_spec) = match content {
            Content::None => (NodeContent::None, None),
            Content::Text(t) => (NodeContent::Text(t), None),
            Content::Icon(i) => (NodeContent::Icon(i), None),
            Content::Canvas(c) => (NodeContent::Canvas(c), None),
            Content::Input(i) => (NodeContent::Input(i), None),
            Content::Split(s) => (NodeContent::None, Some(s)),
            Content::Dropdown(d) => {
                dropdown_spec = Some(d);
                (NodeContent::None, None)
            }
            Content::Virtual(v) => {
                virtual_spec = Some(v);
                (NodeContent::None, None)
            }
        };

        // `position: fixed` elements are re-parented to the root (window) node.
        let parent = if st.position == Position::Fixed && parent.is_some() { Some(0) } else { parent };
        let idx = frame.nodes.len();
        frame.nodes.push(Node {
            id,
            key: raw_key,
            parent,
            children: Vec::new(),
            style: st,
            content: node_content,
            handlers,
            behavior,
            focusable,
            disabled,
            hit_slop,
            pointer_events: pointer,
            tooltip,
            follow_end,
            text: text.clone(),
            color,
            tnode: tf::NodeId::from(0u64),
            rect: Rect::default(),
            clip: None,
            content_size: Size::default(),
            scroll: Point::ZERO,
            split: None,
            pane: None,
            virt_item: None,
        });
        frame.by_id.insert(id, idx);
        if let Some(p) = parent {
            frame.nodes[p].children.push(idx);
        }

        if let Some(spec) = split_spec {
            self.flatten_split(idx, id, spec, &text, color, pointer, frame);
        } else if let Some(d) = dropdown_spec {
            self.flatten_dropdown(idx, id, d, &text, color, pointer, frame);
        } else if let Some(v) = virtual_spec {
            self.flatten_virtual(idx, id, v, &text, color, pointer, frame);
        } else {
            for (i, c) in children.into_iter().enumerate() {
                self.flatten(c, Some(idx), id, i, &text, color, pointer, frame);
            }
        }
        idx
    }

    /// Build only the rows of a virtual list that are near the viewport,
    /// absolutely positioned inside a spacer as tall as the whole list.
    #[allow(clippy::too_many_arguments)]
    fn flatten_virtual(
        &mut self,
        idx: usize,
        id: u64,
        spec: VirtualSpec<A::Msg>,
        text: &TextStyle,
        color: Color,
        pointer: bool,
        frame: &mut Frame<A::Msg>,
    ) {
        let now = self.now;
        let n = &frame.nodes[idx];
        let pad_top = n.style.padding.top;
        let gap = n.style.gap.1;
        let (key, follow) = (n.key, n.follow_end);
        // Viewport from the previous frame (the window height on the first one).
        let vh = self
            .frame
            .by_id
            .get(&id)
            .map(|&i| inner_rect(self.frame.nodes[i].rect, &self.frame.nodes[i].style.border_width).h)
            .filter(|h| *h > 0.0)
            .unwrap_or(self.size.h);
        let ss = self.scrolls.get(&id);
        let pinned = follow && ss.and_then(|s| s.pinned) != Some(false);
        let pending = key.and_then(|k| self.pending_scrolls.iter().rev().find(|(g, _)| *g == k).map(|(_, c)| *c));
        let offset = ss.map_or(0.0, |s| s.offset(now).y);
        let vs = self.virt.entry(id).or_default();
        vs.resize(spec.count);
        vs.estimate = spec.estimate;
        vs.gap = gap;
        vs.seen = self.frame_no;
        let total = vs.total();
        // Where the viewport will be once layout applies scroll commands.
        let top = match pending {
            Some(ScrollCmd::ToEnd) => total - vh,
            Some(ScrollCmd::To(y)) => y - pad_top,
            Some(ScrollCmd::ToItem(i)) => vs.pos(i) - pad_top,
            None if pinned => total - vh,
            None => offset - pad_top,
        }
        .max(0.0);
        let (lo, hi) = (top - spec.overscan, top + vh + spec.overscan);
        let mut rows = Vec::new();
        let mut anchor = None;
        let mut y = 0.0;
        for i in 0..spec.count {
            if y > hi {
                break;
            }
            let h = vs.h(i);
            if y + h >= lo {
                rows.push((i, y));
            }
            if anchor.is_none() && y + h > top {
                anchor = Some(i);
            }
            y += h + gap;
        }
        vs.anchor = anchor.unwrap_or(spec.count);
        vs.built = match (rows.first(), rows.last()) {
            (Some(a), Some(b)) => (a.0, b.0 + 1),
            _ => (0, 0),
        };
        vs.built_px = match (rows.first(), rows.last()) {
            (Some(a), Some(b)) => (a.1, b.1 + vs.h(b.0)),
            _ => (0.0, 0.0),
        };
        let spacer = div().w_full().h(total).shrink(0.0);
        let inner = self.flatten(spacer, Some(idx), id, 0, text, color, pointer, frame);
        let inner_id = frame.nodes[inner].id;
        for (i, y) in rows {
            let row = (spec.builder)(i).absolute().top(y).left(0.0).right(0.0);
            let r = self.flatten(row, Some(inner), inner_id, i, text, color, pointer, frame);
            frame.nodes[r].virt_item = Some((id, i));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn flatten_split(
        &mut self,
        idx: usize,
        id: u64,
        spec: SplitSpec<A::Msg>,
        text: &TextStyle,
        color: Color,
        pointer: bool,
        frame: &mut Frame<A::Msg>,
    ) {
        let axis = spec.axis;
        frame.nodes[idx].split = Some((id, axis));
        let now = self.now;
        let n = spec.panes.len();
        let init: Vec<f32> = spec.panes.iter().map(|p| p.fixed.unwrap_or(p.weight)).collect();
        let fixed: Vec<bool> = spec.panes.iter().map(|p| p.fixed.is_some()).collect();
        let st = self.splits.entry(id).or_insert_with(|| SplitState {
            axis,
            sizes: init.clone(),
            fixed: fixed.clone(),
            initial: init.clone(),
            collapse: spec.panes.iter().map(|p| Anim::new(if p.collapsed { 0.0 } else { 1.0 })).collect(),
            container: 0.0,
            pane_px: vec![0.0; n],
        });
        if st.sizes.len() != n || st.fixed != fixed || st.axis != axis {
            *st = SplitState {
                axis,
                sizes: init.clone(),
                fixed: fixed.clone(),
                initial: init.clone(),
                collapse: spec.panes.iter().map(|p| Anim::new(if p.collapsed { 0.0 } else { 1.0 })).collect(),
                container: 0.0,
                pane_px: vec![0.0; n],
            };
        } else if st.initial != init {
            // The app changed the requested sizes: adopt them.
            st.sizes = init.clone();
            st.initial = init.clone();
        }
        for (i, p) in spec.panes.iter().enumerate() {
            st.collapse[i].set(if p.collapsed { 0.0 } else { 1.0 }, now, 0.22);
        }
        let factors: Vec<f32> = st.collapse.iter().map(|a| a.value(now)).collect();
        let sizes = st.sizes.clone();
        let th = self.theme.clone();
        let mut child_i = 0;
        for (i, pane) in spec.panes.iter().enumerate() {
            self.pane_meta.insert((id, i), (pane.min, pane.max, pane.collapsible));
        }
        for (i, pane) in spec.panes.into_iter().enumerate() {
            if i > 0 {
                // Splitter between pane i-1 and i. Hidden when a neighbour is collapsed.
                let hidden = factors[i - 1] < 0.02 || factors[i] < 0.02;
                let mut s = div::<A::Msg>().shrink(0.0).bg(th.colors.border).hit_slop(if hidden {
                    0.0
                } else {
                    th.splitter_hit
                });
                s = match axis {
                    Axis::Horizontal => s.w(1.0).cursor(Cursor::ResizeCol),
                    Axis::Vertical => s.h(1.0).cursor(Cursor::ResizeRow),
                };
                if hidden {
                    s = s.hidden(true);
                }
                s.behavior = Behavior::Splitter { split: id, index: i - 1, axis };
                self.flatten(s.key(("splitter", i)), Some(idx), id, child_i, text, color, pointer, frame);
                child_i += 1;
            }
            let t = factors[i];
            let mut wrapper = div::<A::Msg>().clip();
            let mut inner = div::<A::Msg>().flex_col().shrink(0.0);
            if fixed[i] {
                let sz = sizes[i].clamp(pane.min, pane.max);
                wrapper = wrapper.basis(sz * t).grow(0.0).shrink(0.0);
                // Panes before a flex pane slide toward the start edge, others toward the end.
                let before_flex = fixed[i + 1..].iter().any(|f| !f);
                wrapper = wrapper.justify(if before_flex { Justify::End } else { Justify::Start });
                inner = match axis {
                    Axis::Horizontal => inner.w(sz).h_full(),
                    Axis::Vertical => inner.h(sz).w_full(),
                };
                if t < 0.999 {
                    wrapper = wrapper.opacity(0.35 + 0.65 * t);
                }
            } else {
                wrapper = wrapper.basis(0.0).grow((sizes[i] * t).max(0.0)).shrink(1.0);
                inner = inner.grow(1.0).shrink(1.0).basis(0.0);
                let min = if t < 0.999 { 0.0 } else { pane.min };
                wrapper = match axis {
                    Axis::Horizontal => wrapper.min_w(min),
                    Axis::Vertical => wrapper.min_h(min),
                };
                inner = match axis {
                    Axis::Horizontal => inner.h_full().min_w(0.0),
                    Axis::Vertical => inner.w_full().min_h(0.0),
                };
                if t < 0.999 {
                    wrapper = wrapper.opacity(0.35 + 0.65 * t);
                }
            }
            wrapper = match axis {
                Axis::Horizontal => wrapper.flex_row().items(Align::Stretch),
                Axis::Vertical => wrapper.flex_col().items(Align::Stretch),
            };
            let mut content = pane.content;
            if content.style.height == Length::Auto && content.style.grow == 0.0 {
                content.style.grow = 1.0;
            }
            if content.style.min_height == Length::Auto {
                content.style.min_height = Length::Px(0.0);
            }
            if content.style.min_width == Length::Auto {
                content.style.min_width = Length::Px(0.0);
            }
            let wrapper = wrapper.child(inner.child(content)).key(("pane", i));
            let w = self.flatten(wrapper, Some(idx), id, child_i, text, color, pointer, frame);
            frame.nodes[w].pane = Some((id, i));
            child_i += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn flatten_dropdown(
        &mut self,
        idx: usize,
        id: u64,
        d: DropdownSpec,
        text_st: &TextStyle,
        color: Color,
        pointer: bool,
        frame: &mut Frame<A::Msg>,
    ) {
        let th = self.theme.clone();
        let c = th.colors.clone();
        let st = self.dropdowns.entry(id).or_default();
        let open = st.open;
        let filter = st.filter.to_lowercase();
        let visible: Vec<usize> = (0..d.options.len())
            .filter(|&i| !d.searchable || filter.is_empty() || d.options[i].to_lowercase().contains(&filter))
            .collect();
        if st.highlight >= visible.len() {
            st.highlight = visible.len().saturating_sub(1);
        }
        st.visible = visible.clone();
        st.options = d.options.clone();
        st.selected = d.selected;
        st.searchable = d.searchable;
        let hi = st.highlight;
        let filter_text = st.filter.clone();
        let focused = self.focused == Some(id);
        // Style the container itself as the trigger.
        {
            let n = &mut frame.nodes[idx];
            n.style.direction = Direction::Row;
            n.style.align_items = Some(Align::Center);
            n.style.padding = Edges::new(0.0, 8.0, 0.0, 10.0);
            n.style.radius = Corners::all(th.radius);
            n.style.border_width = Edges::all(1.0);
            n.style.border_color = if open || focused { c.accent } else { c.border_strong };
            n.style.background = Some(c.input.into());
            n.style.gap = (8.0, 8.0);
            n.style.cursor = Some(Cursor::Pointer);
            n.behavior = Behavior::DropdownToggle(id);
        }
        let label = d.selected.and_then(|i| d.options.get(i)).cloned();
        let mut kids: Vec<Element<A::Msg>> = vec![
            text(label.clone().unwrap_or_else(|| d.placeholder.clone()))
                .ellipsis()
                .grow(1.0)
                .color(if label.is_some() { c.text } else { c.text_faint })
                .pointer_events(false),
            icon(if open { Icon::ChevronUp } else { Icon::ChevronDown })
                .font_size(14.0)
                .color(c.text_faint)
                .pointer_events(false),
        ];
        if open {
            let mut back = div::<A::Msg>().fixed().top(0.0).left(0.0).right(0.0).bottom(0.0).z_index(150);
            back.behavior = Behavior::DropdownClose(id);
            kids.push(back);
            let mut list = col().gap(1.0).p(4.0).max_h(280.0).scroll_y();
            for (vi, &oi) in visible.iter().enumerate() {
                let selected = d.selected == Some(oi);
                let mut row_ = row()
                    .key(("opt", oi))
                    .items_center()
                    .h(th.row_height + 2.0)
                    .px(8.0)
                    .gap(8.0)
                    .rounded(th.radius_sm)
                    .cursor(Cursor::Pointer)
                    .color(c.text)
                    .child(text(d.options[oi].clone()).ellipsis().grow(1.0))
                    .child_if(selected, || icon(Icon::Check).font_size(13.0).color(c.accent));
                row_ = if vi == hi { row_.bg(c.accent_soft) } else { row_.hover(|s| s.bg(c.hover)) };
                row_.behavior = Behavior::DropdownPick(id, oi);
                list = list.child(row_);
            }
            if visible.is_empty() {
                list = list.child(text("No matches").color(c.text_faint).px(8.0).py(6.0));
            }
            let mut panel = col()
                .absolute()
                .top(Length::Percent(100.0))
                .left(0.0)
                .min_w(Length::Percent(100.0))
                .mt(4.0)
                .z_index(160)
                .bg(c.elevated)
                .border(1.0, c.border_strong)
                .rounded(th.radius + 2.0)
                .shadows(th.shadow_popover.clone())
                .clip();
            if d.searchable {
                panel = panel.child(
                    row()
                        .items_center()
                        .gap(6.0)
                        .h(th.control_height)
                        .px(10.0)
                        .border_b(1.0, c.border)
                        .child(icon(Icon::Search).font_size(13.0).color(c.text_faint))
                        .child(if filter_text.is_empty() {
                            text("Type to filter…").color(c.text_faint)
                        } else {
                            text(format!("{filter_text}\u{2502}")).color(c.text)
                        }),
                );
            }
            kids.push(panel.child(list));
        }
        for (i, k) in kids.into_iter().enumerate() {
            self.flatten(k, Some(idx), id, i, text_st, color, pointer, frame);
        }
    }

    /// Close any open dropdown other than `keep`.
    fn close_dropdowns(&mut self, keep: Option<u64>) {
        for (id, st) in self.dropdowns.iter_mut() {
            if Some(*id) != keep && st.open {
                st.open = false;
                self.dirty = true;
            }
        }
    }

    fn open_dropdown(&self) -> Option<u64> {
        self.dropdowns.iter().find(|(_, s)| s.open).map(|(id, _)| *id)
    }

    fn pick_dropdown(&mut self, id: u64, option: usize) {
        if let Some(st) = self.dropdowns.get_mut(&id) {
            st.open = false;
            st.filter.clear();
        }
        if let Some(h) = self.node_by_id(id).and_then(|n| n.handlers.select.clone()) {
            self.queue.push(h(option));
        }
        self.focused = Some(id);
        self.dirty = true;
    }

    /// Keyboard handling for an open dropdown. Returns true if consumed.
    fn dropdown_key(&mut self, id: u64, k: &KeyEvent) -> bool {
        let Some(st) = self.dropdowns.get_mut(&id) else { return false };
        match k.key {
            Key::Down => st.highlight = (st.highlight + 1).min(st.visible.len().saturating_sub(1)),
            Key::Up => st.highlight = st.highlight.saturating_sub(1),
            Key::Escape => {
                st.open = false;
                st.filter.clear();
            }
            Key::Backspace => {
                st.filter.pop();
                st.highlight = 0;
            }
            Key::Enter => {
                if let Some(&o) = st.visible.get(st.highlight) {
                    self.pick_dropdown(id, o);
                } else {
                    st.open = false;
                }
            }
            Key::Tab => {
                st.open = false;
                return false;
            }
            _ => return matches!(k.key, Key::Char(_) | Key::Space) && !k.mods.command(),
        }
        self.dirty = true;
        true
    }

    // --------------------------------------------------------------- layout

    fn layout(&mut self, frame: &mut Frame<A::Msg>) {
        let mut tree: tf::TaffyTree<usize> = tf::TaffyTree::new();
        // Children have larger indices than parents: build bottom-up.
        for i in (0..frame.nodes.len()).rev() {
            let n = &frame.nodes[i];
            let mut ts = to_taffy(&n.style);
            let is_leaf = matches!(n.content, NodeContent::Text(_) | NodeContent::Input(_));
            if let NodeContent::Icon(_) = n.content {
                if n.style.width == Length::Auto {
                    ts.size.width = tf::Dimension::length(n.text.size);
                }
                if n.style.height == Length::Auto {
                    ts.size.height = tf::Dimension::length(n.text.size);
                }
                ts.flex_shrink = 0.0;
            }
            let kids: Vec<tf::NodeId> = n.children.iter().map(|&c| frame.nodes[c].tnode).collect();
            // Taffy only errors on unknown child ids, which this bottom-up build never produces.
            let t = if is_leaf { tree.new_leaf_with_context(ts, i) } else { tree.new_with_children(ts, &kids) };
            let Ok(t) = t else {
                debug_assert!(false, "taffy node creation failed");
                return;
            };
            frame.nodes[i].tnode = t;
        }
        let root = frame.nodes[0].tnode;
        let scale = self.scale;
        {
            let text = &mut self.text;
            let nodes = &frame.nodes;
            let computed = tree.compute_layout_with_measure(
                root,
                tf::Size {
                    width: tf::AvailableSpace::Definite(self.size.w),
                    height: tf::AvailableSpace::Definite(self.size.h),
                },
                |inputs, _id, ctx, style| {
                    let Some(&mut i) = ctx else {
                        return tf::compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| tf::Size::ZERO);
                    };
                    let node = &nodes[i];
                    tf::compute_leaf_layout(
                        inputs,
                        style,
                        |_, _| 0.0,
                        |known, avail| measure_node(text, node, known, avail, scale),
                    )
                },
            );
            if computed.is_err() {
                debug_assert!(false, "taffy layout failed");
                return;
            }
        }

        // Virtual lists: cache measured row heights. Rows were stacked using
        // estimates, so re-stack the ones after a mis-estimated row, grow the
        // list's extent, and shift the scroll offset by any change above the
        // viewport's first row so what the user is looking at stays put.
        let mut vdy: HashMap<usize, f32> = HashMap::new();
        let mut vgrow: HashMap<u64, f32> = HashMap::new();
        for i in 0..frame.nodes.len() {
            let Some((lid, k)) = frame.nodes[i].virt_item else { continue };
            let Some(vs) = self.virt.get_mut(&lid) else { continue };
            let h = tree.layout(frame.nodes[i].tnode).map_or(0.0, |l| l.size.height);
            let acc = vgrow.entry(lid).or_insert(0.0);
            if *acc != 0.0 {
                vdy.insert(i, *acc);
            }
            let delta = h - vs.h(k);
            if delta.abs() > 0.01 {
                *acc += delta;
                if k < vs.anchor {
                    if let Some(a) = self.scrolls.get_mut(&lid).and_then(|s| s.y.as_mut()) {
                        a.shift(delta);
                    }
                }
            }
            vs.set(k, h);
        }

        // Absolute rects, clips and scroll offsets (pre-order: parents first).
        let now = self.now;
        for i in 0..frame.nodes.len() {
            let l = tree.layout(frame.nodes[i].tnode).copied().unwrap_or_default();
            let (origin, clip, portal) = match frame.nodes[i].parent {
                None => (Point::ZERO, None, false),
                Some(p) => {
                    let pn = &frame.nodes[p];
                    let child_clip = if pn.style.overflow != Overflow::Visible {
                        let r = inner_rect(pn.rect, &pn.style.border_width);
                        Some(match pn.clip {
                            Some(c) => c.intersect(&r),
                            None => r,
                        })
                    } else {
                        pn.clip
                    };
                    let portal = frame.nodes[i].style.z_index > 0;
                    (
                        Point::new(pn.rect.x - pn.scroll.x, pn.rect.y - pn.scroll.y),
                        if portal { None } else { child_clip },
                        portal,
                    )
                }
            };
            let _ = portal;
            let n = &mut frame.nodes[i];
            let mut rel = Rect::new(
                l.location.x + n.style.translate.0,
                l.location.y + n.style.translate.1 + vdy.get(&i).copied().unwrap_or(0.0),
                l.size.width,
                l.size.height,
            );
            if n.style.layout_transition > 0.0 {
                // FLIP-style layout animation of the element's box relative to its parent.
                let dur = n.style.layout_transition;
                let easing = n.style.easing;
                let fno = self.frame_no;
                let a =
                    self.rect_anims.entry(n.id).or_insert(RectAnim { from: rel, to: rel, start: now, dur, seen: fno });
                a.seen = fno;
                if a.to != rel {
                    a.from = a.value(now, easing);
                    a.to = rel;
                    a.start = now;
                    a.dur = dur;
                }
                rel = a.value(now, easing);
            }
            n.rect = Rect::new(origin.x + rel.x, origin.y + rel.y, rel.w, rel.h);
            n.clip = clip;
            // Content extent for scroll containers.
            if let Behavior::Scroll { x, y } = n.behavior {
                let mut cw: f32 = 0.0;
                let mut ch: f32 = 0.0;
                if let Ok(kids) = tree.children(n.tnode) {
                    for kl in kids.iter().filter_map(|&k| tree.layout(k).ok()) {
                        cw = cw.max(kl.location.x + kl.size.width + kl.margin.right);
                        ch = ch.max(kl.location.y + kl.size.height + kl.margin.bottom);
                    }
                }
                cw += l.padding.right + l.border.right;
                ch += l.padding.bottom + l.border.bottom + vgrow.get(&n.id).copied().unwrap_or(0.0);
                n.content_size = Size::new(cw, ch);
                let max = Point::new((cw - l.size.width).max(0.0), (ch - l.size.height).max(0.0));
                let ss = self.scrolls.entry(n.id).or_default();
                ss.max = max;
                if x {
                    let a = ss.x.get_or_insert(Anim::new(0.0));
                    if a.target() > max.x {
                        a.snap(max.x);
                    }
                }
                if y {
                    let a = ss.y.get_or_insert(Anim::new(0.0));
                    if a.target() > max.y {
                        a.snap(max.y);
                    }
                    if n.follow_end {
                        let pinned = *ss.pinned.get_or_insert(true);
                        if pinned && a.target() < max.y {
                            a.snap(max.y);
                        }
                    }
                }
                if let Some(pos) = self.pending_scrolls.iter().rposition(|(g, _)| n.key == Some(*g)) {
                    let (_, cmd) = self.pending_scrolls.remove(pos);
                    let a = ss.y.get_or_insert(Anim::new(0.0));
                    let t = match cmd {
                        ScrollCmd::ToEnd => max.y,
                        ScrollCmd::To(v) => v.clamp(0.0, max.y),
                        ScrollCmd::ToItem(k) => self.virt.get(&n.id).map_or(0.0, |vs| vs.pos(k)).clamp(0.0, max.y),
                    };
                    if self.virt.contains_key(&n.id) {
                        // Virtual lists jump: the rows in between were never
                        // built, so an animation would chase estimated heights.
                        a.snap(t);
                    } else {
                        a.set(t, now, 0.18);
                    }
                    if n.follow_end {
                        ss.pinned = Some(t >= max.y - 2.0);
                    }
                }
                let off = ss.offset(now);
                n.scroll = Point::new(off.x.clamp(0.0, max.x), off.y.clamp(0.0, max.y));
                if let Some(vs) = self.virt.get(&n.id) {
                    // Rebuild next frame if the viewport has moved past the built rows.
                    let grow = vgrow.get(&n.id).copied().unwrap_or(0.0);
                    let top = n.scroll.y - n.style.padding.top;
                    let bottom = top + l.size.height;
                    let count = vs.heights.len();
                    if (vs.built.0 > 0 && top < vs.built_px.0 - 0.5)
                        || (vs.built.1 < count && bottom > vs.built_px.1 + grow + 0.5)
                    {
                        self.dirty = true;
                    }
                }
                if let Some(h) = &n.handlers.scroll {
                    let t = ss.target();
                    if ss.last_reported != Some(t) {
                        ss.last_reported = Some(t);
                        self.queue.push(h(ScrollInfo { offset: t, max, at_end: t.y >= max.y - 2.0 }));
                    }
                }
            }
            if let Some((sid, axis)) = n.split {
                if let Some(ss) = self.splits.get_mut(&sid) {
                    ss.container = axis.main_len(&n.rect);
                }
            }
            if let Some((sid, pi)) = n.pane {
                if let Some(ss) = self.splits.get_mut(&sid) {
                    let len = ss.axis.main_len(&n.rect);
                    if let Some(px) = ss.pane_px.get_mut(pi) {
                        *px = len;
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- paint

    fn paint(&mut self) {
        let pw = (self.size.w * self.scale).round().max(1.0) as u32;
        let ph = (self.size.h * self.scale).round().max(1.0) as u32;
        let mut order = Vec::new();
        let th = self.theme.clone();
        let scene = Scene::new(pw, ph, self.scale, th.colors.background);
        let mut canvas = Canvas::new(scene, &mut self.text);
        let ctx = PaintCtx {
            nodes: &self.frame.nodes,
            now: self.now,
            focused: self.focused,
            focus_visible: self.focus_visible,
            hovered: &self.hovered_set,
            scrolls: &self.scrolls,
            inputs: &self.inputs,
            drag: &self.drag,
            splitter_hover: self.splitter_hover,
            text_sel: self.text_sel,
            theme: &th,
            window_focused: self.window_focused,
        };
        let mut deferred: Vec<(i32, usize)> = Vec::new();
        paint_node(&ctx, 0, &mut canvas, &mut deferred, &mut order, true);
        while !deferred.is_empty() {
            deferred.sort_by_key(|d| d.0);
            let batch = std::mem::take(&mut deferred);
            let saved = canvas.take_clips();
            for (_, i) in batch {
                paint_node(&ctx, i, &mut canvas, &mut deferred, &mut order, false);
            }
            canvas.restore_clips(saved);
        }
        // Tooltip
        if let Some((tid, since, pos)) = self.tooltip {
            if self.now - since >= TOOLTIP_DELAY {
                if let Some(&ti) = self.frame.by_id.get(&tid) {
                    if let Some(tip) = &self.frame.nodes[ti].tooltip {
                        paint_tooltip(&mut canvas, &th, tip, pos, self.size);
                    }
                }
            }
        }
        self.scene = Some(canvas.finish());
    }

    // ---------------------------------------------------------- hit testing

    fn hit(&self, p: Point) -> Option<usize> {
        let f = &self.frame;
        // Elements with extended hit areas (splitters) win first.
        for &i in f.order.iter().rev() {
            let n = &f.nodes[i];
            if n.hit_slop > 0.0 && n.pointer_events && n.style.display != Display::None {
                let r = n.rect.outset(n.hit_slop);
                if r.contains(p) && n.clip.is_none_or(|c| c.outset(n.hit_slop).contains(p)) {
                    return Some(i);
                }
            }
        }
        for &i in f.order.iter().rev() {
            let n = &f.nodes[i];
            if n.pointer_events && n.rect.contains(p) && n.clip.is_none_or(|c| c.contains(p)) {
                return Some(i);
            }
        }
        None
    }

    fn chain(&self, i: usize) -> Vec<usize> {
        let mut v = vec![i];
        let mut c = i;
        while let Some(p) = self.frame.nodes[c].parent {
            v.push(p);
            c = p;
        }
        v
    }

    /// Returns true when the hovered set changed.
    fn update_hover(&mut self, p: Point) -> bool {
        let hit = self.hit(p);
        let chain: Vec<u64> =
            hit.map(|i| self.chain(i).into_iter().map(|j| self.frame.nodes[j].id).collect()).unwrap_or_default();
        // Cursor
        let mut cursor = Cursor::Default;
        if let Some(i) = hit {
            for j in self.chain(i) {
                let n = &self.frame.nodes[j];
                if n.disabled {
                    cursor = Cursor::Default;
                    break;
                }
                if let Some(c) = n.style.cursor {
                    cursor = c;
                    break;
                }
                if matches!(n.content, NodeContent::Input(_))
                    || matches!(&n.content, NodeContent::Text(t) if t.selectable)
                {
                    cursor = Cursor::Text;
                    break;
                }
            }
        }
        if let Some(i) = hit {
            let id = self.frame.nodes[i].id;
            if self.link_at(id, p).is_some() {
                cursor = Cursor::Pointer;
            }
        }
        if let Some(edge) = self.resize_edge(p) {
            cursor = edge_cursor(edge);
        }
        self.cursor = cursor;
        // Splitter hover-highlight delay tracking
        let sp = hit
            .filter(|&i| matches!(self.frame.nodes[i].behavior, Behavior::Splitter { .. }))
            .map(|i| self.frame.nodes[i].id);
        match (sp, self.splitter_hover) {
            (Some(a), Some((b, _))) if a == b => {}
            (Some(a), _) => self.splitter_hover = Some((a, self.now)),
            (None, _) => self.splitter_hover = None,
        }
        // Tooltip tracking
        let tip = hit.and_then(|i| self.chain(i).into_iter().find(|&j| self.frame.nodes[j].tooltip.is_some()));
        match (tip, self.tooltip) {
            (Some(t), Some((id, _, _))) if self.frame.nodes[t].id == id => {}
            (Some(t), _) => self.tooltip = Some((self.frame.nodes[t].id, self.now, p)),
            (None, _) => self.tooltip = None,
        }
        if chain == self.hovered {
            return false;
        }
        let new_set: HashSet<u64> = chain.iter().copied().collect();
        // Hover callbacks
        let mut events: Vec<(u64, bool)> =
            self.hovered.iter().filter(|id| !new_set.contains(id)).map(|&id| (id, false)).collect();
        events.extend(chain.iter().filter(|id| !self.hovered_set.contains(id)).map(|&id| (id, true)));
        for (id, entered) in events {
            if let Some(&i) = self.frame.by_id.get(&id) {
                if let Some(h) = &self.frame.nodes[i].handlers.hover {
                    self.queue.push(h(entered));
                }
            }
        }
        self.hovered = chain;
        self.hovered_set = new_set;
        true
    }

    fn resize_edge(&self, p: Point) -> Option<ResizeEdge> {
        if !self.frameless || self.maximized {
            return None;
        }
        let b = RESIZE_BORDER;
        let (w, h) = (self.size.w, self.size.h);
        let (l, r, t, bo) = (p.x < b, p.x > w - b, p.y < b, p.y > h - b);
        Some(match (l, r, t, bo) {
            (true, _, true, _) => ResizeEdge::NW,
            (_, true, true, _) => ResizeEdge::NE,
            (true, _, _, true) => ResizeEdge::SW,
            (_, true, _, true) => ResizeEdge::SE,
            (true, _, _, _) => ResizeEdge::W,
            (_, true, _, _) => ResizeEdge::E,
            (_, _, true, _) => ResizeEdge::N,
            (_, _, _, true) => ResizeEdge::S,
            _ => return None,
        })
    }

    fn node_by_id(&self, id: u64) -> Option<&Node<A::Msg>> {
        self.frame.by_id.get(&id).map(|&i| &self.frame.nodes[i])
    }

    // --------------------------------------------------------------- events

    /// Feed an input event. Messages produced are applied to the app immediately.
    pub fn handle(&mut self, ev: Event) {
        if self.frame.nodes.is_empty() {
            self.render();
        }
        match ev {
            Event::PointerMove(p) => self.pointer_move(p),
            Event::PointerDown(p, b) => self.pointer_down(p, b),
            Event::PointerUp(p, b) => self.pointer_up(p, b),
            Event::PointerLeave => {
                self.pointer = None;
                if matches!(self.drag, Drag::None) && !self.hovered.is_empty() {
                    for id in std::mem::take(&mut self.hovered) {
                        if let Some(h) = self.node_by_id(id).and_then(|n| n.handlers.hover.clone()) {
                            self.queue.push(h(false));
                        }
                    }
                    self.hovered_set.clear();
                    self.tooltip = None;
                    self.dirty = true;
                }
            }
            Event::Wheel(p, d) => self.wheel(p, d),
            Event::Key(k) => self.key(k),
            Event::Text(t) => self.text_input(&t),
            Event::Paste(t) => self.paste(&t),
            Event::WindowFocus(f) => {
                self.window_focused = f;
                self.dirty = true;
                if let Some(h) = self.subs.focus.clone() {
                    self.queue.push(h(f));
                }
            }
        }
        self.flush();
    }

    fn flush(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let mut cx = Cx { requests: Vec::new(), focus: None, scrolls: Vec::new(), proxy: self.mailbox.proxy() };
        while !self.queue.is_empty() {
            for m in std::mem::take(&mut self.queue) {
                self.app.update(m, &mut cx);
            }
        }
        self.subs = self.app.subscriptions();
        self.requests.append(&mut cx.requests);
        for (gid, cmd) in cx.scrolls {
            self.pending_scrolls.push((gid, cmd));
        }
        if let Some(f) = cx.focus {
            self.focused = f.and_then(|g| self.frame.nodes.iter().find(|n| n.key == Some(g)).map(|n| n.id));
            self.focus_visible = true;
        }
        self.dirty = true;
    }

    fn pointer_move(&mut self, p: Point) {
        let prev = self.pointer.replace(p);
        let now = self.now;
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::None => {
                if self.update_hover(p) {
                    self.dirty = true;
                }
                if prev.is_none() {
                    self.dirty = true;
                }
            }
            Drag::Press { node, start, mut dragging } => {
                if let Some(n) = self.node_by_id(node) {
                    let rect = n.rect;
                    let drag_handler = self.find_up(node, |n| n.handlers.drag.is_some());
                    if let Some(h) = drag_handler.and_then(|d| self.node_by_id(d)).and_then(|n| n.handlers.drag.clone())
                    {
                        if !dragging && p.distance(start) > DRAG_THRESHOLD {
                            dragging = true;
                            self.queue.push(h(DragEvent {
                                phase: DragPhase::Start,
                                pos: start,
                                delta: Point::ZERO,
                                rect,
                            }));
                        }
                        if dragging {
                            self.queue.push(h(DragEvent { phase: DragPhase::Move, pos: p, delta: p - start, rect }));
                            self.update_drop_target(p, false);
                        }
                    } else if !dragging && p.distance(start) > DRAG_THRESHOLD {
                        dragging = true;
                    }
                }
                self.drag = Drag::Press { node, start, dragging };
                self.update_hover(p);
                if dragging && self.find_up(node, |n| n.handlers.drag.is_some()).is_some() {
                    self.cursor = Cursor::Grabbing;
                }
                self.dirty = true;
            }
            Drag::Splitter { split, index, start, start_sizes, start_px, mut collapsed_emitted } => {
                if let (Some(split_node), Some(st)) = (self.frame.by_id.get(&split).copied(), self.splits.get(&split)) {
                    let axis = st.axis;
                    let delta = axis.main(p) - axis.main(start);
                    let sid = self.frame.nodes[split_node].id;
                    let meta = |i: usize| self.pane_meta.get(&(sid, i)).copied().unwrap_or((0.0, f32::INFINITY, false));
                    let fixed = st.fixed.clone();
                    let container = st.container;
                    let n = fixed.len();
                    let mut sizes = st.sizes.clone();
                    let mut collapse_msg = None;
                    if index + 1 < n {
                        if fixed[index] || fixed[index + 1] {
                            let (target, sign) = if fixed[index] { (index, 1.0) } else { (index + 1, -1.0) };
                            let (min, max, collapsible) = meta(target);
                            let raw = start_sizes[target] + sign * delta;
                            let others: f32 = (0..n).filter(|&j| j != target && fixed[j]).map(|j| sizes[j]).sum();
                            let flex_min: f32 = (0..n).filter(|&j| !fixed[j]).map(|j| meta(j).0).sum();
                            let room = (container - others - flex_min - (n as f32 - 1.0)).max(min);
                            if collapsible && raw < min * 0.5 {
                                if !collapsed_emitted {
                                    collapsed_emitted = true;
                                    // Re-expanding restores the size from before the drag.
                                    sizes[target] = start_sizes[target];
                                    collapse_msg = Some((target, true));
                                }
                            } else {
                                if collapsed_emitted {
                                    collapsed_emitted = false;
                                    collapse_msg = Some((target, false));
                                }
                                sizes[target] = raw.clamp(min, max.min(room));
                            }
                        } else {
                            // Two flex panes: move pixels between them, keeping their total weight.
                            let (pa, pb) = (start_px[index], start_px[index + 1]);
                            let (mina, maxa, _) = meta(index);
                            let (minb, maxb, _) = meta(index + 1);
                            let total = pa + pb;
                            if total > 0.0 {
                                let lo = mina.max(total - maxb);
                                let hi = (total - minb).min(maxa);
                                let na = if lo <= hi { (pa + delta).clamp(lo, hi) } else { pa };
                                let w = start_sizes[index] + start_sizes[index + 1];
                                sizes[index] = w * na / total;
                                sizes[index + 1] = w - sizes[index];
                            }
                        }
                    }
                    if let Some(st) = self.splits.get_mut(&split) {
                        st.sizes = sizes;
                    }
                    if let Some(m) = collapse_msg {
                        if let Some(h) = self.frame.nodes[split_node].handlers.collapse.clone() {
                            self.queue.push(h(m));
                        }
                    }
                }
                self.drag = Drag::Splitter { split, index, start, start_sizes, start_px, collapsed_emitted };
                self.dirty = true;
            }
            Drag::Slider { node } => {
                self.slider_set(node, p);
                self.drag = Drag::Slider { node };
            }
            Drag::Column { table, col, min, start, start_w } => {
                let w = (start_w + p.x - start.x).max(min).round();
                let widths = self.tables.entry(table).or_default();
                if widths.len() <= col {
                    widths.resize(col + 1, None);
                }
                widths[col] = Some(w);
                self.cursor = Cursor::ResizeCol;
                self.drag = Drag::Column { table, col, min, start, start_w };
                self.dirty = true;
            }
            Drag::ScrollThumb { node, vertical, start, start_offset } => {
                if let Some(n) = self.node_by_id(node) {
                    let (view, content) =
                        if vertical { (n.rect.h, n.content_size.h) } else { (n.rect.w, n.content_size.w) };
                    let track = view;
                    let thumb = (view / content * track).max(24.0);
                    let ratio = (content - view) / (track - thumb).max(1.0);
                    let d = if vertical { p.y - start.y } else { p.x - start.x };
                    let st = self.scrolls.entry(node).or_default();
                    let v = (start_offset + d * ratio).clamp(0.0, if vertical { st.max.y } else { st.max.x });
                    let a =
                        if vertical { st.y.get_or_insert(Anim::new(0.0)) } else { st.x.get_or_insert(Anim::new(0.0)) };
                    a.snap(v);
                    st.last_activity = now;
                    if vertical && st.pinned.is_some() {
                        st.pinned = Some(v >= st.max.y - 2.0);
                    }
                }
                self.drag = Drag::ScrollThumb { node, vertical, start, start_offset };
                self.dirty = true;
            }
            Drag::TextSelect { node } => {
                if let Some(pos) = self.input_hit(node, p) {
                    let st = self.inputs.entry(node).or_default();
                    st.sel.cursor = pos;
                    st.blink_start = now;
                }
                self.drag = Drag::TextSelect { node };
                self.dirty = true;
            }
            Drag::ReadSelect { node } => {
                if let (Some(b), Some(sel)) = (self.read_hit(node, p), self.text_sel.as_mut()) {
                    sel.2 = b;
                }
                self.drag = Drag::ReadSelect { node };
                self.dirty = true;
            }
        }
    }

    /// Byte offset under `p` in a text node.
    fn read_hit(&mut self, node: u64, p: Point) -> Option<usize> {
        let &i = self.frame.by_id.get(&node)?;
        let n = &self.frame.nodes[i];
        let NodeContent::Text(spec) = &n.content else { return None };
        let (tr, wrap) = text_frame(&mut self.text, n, spec, self.scale);
        Some(self.text.hit_byte(&spec.text, spec.spans.as_deref(), &n.text, wrap, self.scale, p.x - tr.x, p.y - tr.y))
    }

    /// Link target of the rich-text span at `p`, if any.
    fn link_at(&mut self, node: u64, p: Point) -> Option<String> {
        let &i = self.frame.by_id.get(&node)?;
        let has_links = matches!(&self.frame.nodes[i].content, NodeContent::Text(t) if t.spans.as_ref().is_some_and(|s| s.iter().any(|s| s.link.is_some())));
        if !has_links {
            return None;
        }
        let b = self.read_hit(node, p)?;
        let NodeContent::Text(spec) = &self.frame.nodes[i].content else { return None };
        let mut at = 0;
        for sp in spec.spans.as_deref().unwrap_or(&[]) {
            let end = at + sp.text.len();
            if b >= at && b < end {
                return sp.link.clone();
            }
            at = end;
        }
        None
    }

    fn emit_link(&mut self, node: u64, url: String) {
        if let Some(h) = self
            .find_up(node, |n| n.handlers.link.is_some())
            .and_then(|d| self.node_by_id(d))
            .and_then(|n| n.handlers.link.clone())
        {
            self.queue.push(h(url));
        }
    }

    /// Currently selected read-only text, if any.
    pub fn selected_text(&self) -> Option<String> {
        let (id, a, b) = self.text_sel?;
        let n = self.node_by_id(id)?;
        let NodeContent::Text(spec) = &n.content else { return None };
        let (a, b) = (a.min(b), a.max(b));
        (a != b).then(|| spec.text.get(a..b).map(str::to_string)).flatten()
    }

    /// Track the drop target under the pointer during a drag and notify it.
    fn update_drop_target(&mut self, p: Point, drop: bool) {
        let target = self
            .hit(p)
            .and_then(|i| self.chain(i).into_iter().find(|&j| self.frame.nodes[j].handlers.drop_target.is_some()));
        let tid = target.map(|i| self.frame.nodes[i].id);
        if self.drop_target != tid {
            if let Some(old) = self.drop_target.and_then(|o| self.node_by_id(o)) {
                if let Some(h) = old.handlers.drop_target.clone() {
                    self.queue.push(h(DropEvent { phase: DropPhase::Leave, pos: p, rect: old.rect }));
                }
            }
        }
        self.drop_target = if drop { None } else { tid };
        if let Some(i) = target {
            let n = &self.frame.nodes[i];
            if let Some(h) = n.handlers.drop_target.clone() {
                let phase = if drop { DropPhase::Drop } else { DropPhase::Over };
                self.queue.push(h(DropEvent { phase, pos: p, rect: n.rect }));
            }
        }
    }

    fn find_up(&self, id: u64, f: impl Fn(&Node<A::Msg>) -> bool) -> Option<u64> {
        let mut i = *self.frame.by_id.get(&id)?;
        loop {
            let n = &self.frame.nodes[i];
            if f(n) {
                return Some(n.id);
            }
            i = n.parent?;
        }
    }

    fn pointer_down(&mut self, p: Point, button: MouseButton) {
        self.pointer = Some(p);
        self.update_hover(p);
        let now = self.now;
        if button == MouseButton::Left {
            if let Some(edge) = self.resize_edge(p) {
                self.requests.push(WindowRequest::DragResize(edge));
                return;
            }
        }
        let Some(hit) = self.hit(p) else {
            self.focused = None;
            self.dirty = true;
            return;
        };
        let chain = self.chain(hit);
        if chain.iter().any(|&i| self.frame.nodes[i].disabled) {
            return;
        }
        if button == MouseButton::Right {
            for &i in &chain {
                if let Some(h) = &self.frame.nodes[i].handlers.context_menu {
                    self.queue.push(h(p));
                    break;
                }
            }
            return;
        }
        if button != MouseButton::Left {
            return;
        }
        let hit_id = self.frame.nodes[hit].id;
        // Double click detection
        let clicks = match self.last_click {
            Some((t, lp, id, n)) if now - t < DOUBLE_CLICK && lp.distance(p) < 5.0 && id == hit_id => n + 1,
            _ => 1,
        };
        self.last_click = Some((now, p, hit_id, clicks));

        // Focus
        let focus_target =
            chain.iter().copied().find(|&i| self.frame.nodes[i].focusable).map(|i| self.frame.nodes[i].id);
        if self.focused != focus_target {
            self.focused = focus_target;
        }
        self.focus_visible = false;
        self.pressed = chain.iter().map(|&i| self.frame.nodes[i].id).collect();
        self.dirty = true;

        // Selectable read-only text.
        if let NodeContent::Text(spec) = &self.frame.nodes[hit].content {
            if spec.selectable {
                let text = spec.text.clone();
                if let Some(b) = self.read_hit(hit_id, p) {
                    self.text_sel = Some(match clicks {
                        1 => (hit_id, b, b),
                        2 => {
                            let (a, e) = edit::word_at(&text, b);
                            (hit_id, a, e)
                        }
                        _ => (hit_id, 0, text.len()),
                    });
                    if clicks == 1 {
                        self.drag = Drag::ReadSelect { node: hit_id };
                    }
                    return;
                }
            }
        }
        self.text_sel = None;

        for &i in &chain {
            let n = &self.frame.nodes[i];
            match &n.behavior {
                Behavior::Splitter { split, index, .. } => {
                    let (split, index) = (*split, *index);
                    if clicks >= 2 {
                        if let Some(st) = self.splits.get_mut(&split) {
                            st.sizes = st.initial.clone();
                        }
                        return;
                    }
                    let Some(st) = self.splits.get(&split) else { return };
                    self.drag = Drag::Splitter {
                        split,
                        index,
                        start: p,
                        start_sizes: st.sizes.clone(),
                        start_px: st.pane_px.clone(),
                        collapsed_emitted: false,
                    };
                    return;
                }
                Behavior::ColumnResize { table, col, min } => {
                    let (table, col, min) = (*table, *col, *min);
                    if clicks >= 2 {
                        if let Some(slot) = self.tables.get_mut(&table).and_then(|t| t.get_mut(col)) {
                            *slot = None;
                        }
                        self.dirty = true;
                        return;
                    }
                    // The handle sits inside the header cell it resizes.
                    let start_w = n.parent.map_or(0.0, |p| self.frame.nodes[p].rect.w);
                    self.drag = Drag::Column { table, col, min, start: p, start_w };
                    return;
                }
                Behavior::WindowDrag => {
                    if clicks >= 2 {
                        self.requests.push(WindowRequest::ToggleMaximize);
                    } else {
                        self.requests.push(WindowRequest::DragMove);
                    }
                    self.pressed.clear();
                    return;
                }
                Behavior::Slider { .. } => {
                    let id = n.id;
                    self.drag = Drag::Slider { node: id };
                    self.slider_set(id, p);
                    return;
                }
                Behavior::Scroll { x, y } => {
                    // Scrollbar thumb grab
                    let id = n.id;
                    let (x, y) = (*x, *y);
                    if let Some((vertical, _)) = self.scrollbar_hit(i, p, x, y) {
                        let st = self.scrolls.entry(id).or_default();
                        let off = st.target();
                        self.drag = Drag::ScrollThumb {
                            node: id,
                            vertical,
                            start: p,
                            start_offset: if vertical { off.y } else { off.x },
                        };
                        return;
                    }
                }
                _ => {}
            }
            if let NodeContent::Input(spec) = &n.content {
                let id = n.id;
                let value = self.pending_values.get(&id).cloned().unwrap_or_else(|| spec.value.clone());
                let pos = self.input_hit(id, p).unwrap_or(0);
                let st = self.inputs.entry(id).or_default();
                if clicks == 2 {
                    let (a, b) = edit::word_at(&value, pos);
                    st.sel = Selection { anchor: a, cursor: b };
                } else if clicks >= 3 {
                    st.sel = Selection { anchor: 0, cursor: value.len() };
                } else {
                    st.sel = Selection::caret(pos);
                    self.drag = Drag::TextSelect { node: id };
                }
                st.blink_start = now;
                return;
            }
        }
        self.drag = Drag::Press { node: hit_id, start: p, dragging: false };
    }

    fn pointer_up(&mut self, p: Point, button: MouseButton) {
        if button != MouseButton::Left {
            return;
        }
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        let pressed = std::mem::take(&mut self.pressed);
        self.dirty = true;
        match drag {
            Drag::ReadSelect { node } => {
                // A click without a drag on a link follows it.
                if self.text_sel.is_none_or(|s| s.1 == s.2) {
                    if let Some(url) = self.link_at(node, p) {
                        self.text_sel = None;
                        self.emit_link(node, url);
                    }
                }
            }
            Drag::Press { node, start, dragging } => {
                if !dragging {
                    if let Some(url) = self.link_at(node, p) {
                        self.emit_link(node, url);
                    }
                }
                if dragging {
                    self.update_drop_target(p, true);
                    if let Some(d) = self.find_up(node, |n| n.handlers.drag.is_some()) {
                        if let Some(n) = self.node_by_id(d) {
                            if let Some(h) = n.handlers.drag.clone() {
                                let ev = DragEvent { phase: DragPhase::End, pos: p, delta: p - start, rect: n.rect };
                                self.queue.push(h(ev));
                            }
                        }
                    }
                } else {
                    // Click: the pointer must still be over the element that has the handler.
                    let over: HashSet<u64> = self
                        .hit(p)
                        .map(|i| self.chain(i).into_iter().map(|j| self.frame.nodes[j].id).collect())
                        .unwrap_or_default();
                    let clicks = self.last_click.map(|c| c.3).unwrap_or(1);
                    for id in pressed {
                        if !over.contains(&id) {
                            continue;
                        }
                        let Some(n) = self.node_by_id(id) else { continue };
                        if n.disabled {
                            break;
                        }
                        match n.behavior {
                            Behavior::DropdownToggle(d) => {
                                self.close_dropdowns(Some(d));
                                let st = self.dropdowns.entry(d).or_default();
                                let selected = st.selected;
                                st.open = !st.open;
                                st.filter.clear();
                                st.highlight =
                                    selected.and_then(|s| st.visible.iter().position(|&v| v == s)).unwrap_or(0);
                                self.focused = Some(d);
                                break;
                            }
                            Behavior::DropdownClose(d) => {
                                if let Some(st) = self.dropdowns.get_mut(&d) {
                                    st.open = false;
                                    st.filter.clear();
                                }
                                break;
                            }
                            Behavior::DropdownPick(d, o) => {
                                self.pick_dropdown(d, o);
                                break;
                            }
                            _ => {}
                        }
                        if let Behavior::Copy(t) = &n.behavior {
                            let t = t.clone();
                            self.clipboard = t.clone();
                            self.requests.push(WindowRequest::SetClipboard(t));
                            break;
                        }
                        if let Behavior::WindowControl(c) = n.behavior {
                            self.requests.push(match c {
                                WindowControl::Minimize => WindowRequest::Minimize,
                                WindowControl::ToggleMaximize => WindowRequest::ToggleMaximize,
                                WindowControl::Close => WindowRequest::Close,
                            });
                            break;
                        }
                        if clicks >= 2 {
                            if let Some(m) = n.handlers.double_click.clone() {
                                self.queue.push(m);
                                break;
                            }
                        }
                        if let Some(m) = n.handlers.click.clone() {
                            self.queue.push(m);
                            break;
                        }
                    }
                }
            }
            Drag::Splitter { split, .. } => {
                if let (Some(&i), Some(st)) = (self.frame.by_id.get(&split), self.splits.get(&split)) {
                    if let Some(h) = self.frame.nodes[i].handlers.resize.clone() {
                        self.queue.push(h(st.report()));
                    }
                }
            }
            _ => {}
        }
        self.update_hover(p);
    }

    fn wheel(&mut self, p: Point, d: Point) {
        let Some(hit) = self.hit(p) else { return };
        let now = self.now;
        // Multi-line inputs scroll their own content first.
        let ml = self
            .chain(hit)
            .into_iter()
            .find(|&i| matches!(&self.frame.nodes[i].content, NodeContent::Input(s) if s.multiline));
        if let Some(i) = ml {
            let n = &self.frame.nodes[i];
            if let NodeContent::Input(spec) = &n.content {
                let cr = content_rect(n);
                let total = self.text.measure(&spec.value, &n.text, Some(cr.w.max(1.0)), self.scale).h;
                let max = (total - cr.h).max(0.0);
                let st = self.inputs.entry(n.id).or_default();
                let before = st.scroll_y;
                st.scroll_y = (st.scroll_y + d.y).clamp(0.0, max);
                if st.scroll_y != before {
                    self.dirty = true;
                    return;
                }
            }
        }
        for i in self.chain(hit) {
            let n = &self.frame.nodes[i];
            if let Behavior::Scroll { x, y } = n.behavior {
                let st = self.scrolls.entry(n.id).or_default();
                let (mut dx, mut dy) = (d.x, d.y);
                if x && !y && dx == 0.0 {
                    // Vertical wheel scrolls horizontal-only containers.
                    dx = dy;
                    dy = 0.0;
                }
                let can_y = y && ((dy > 0.0 && st.target().y < st.max.y) || (dy < 0.0 && st.target().y > 0.0));
                let can_x = x && ((dx > 0.0 && st.target().x < st.max.x) || (dx < 0.0 && st.target().x > 0.0));
                if !can_x && !can_y {
                    continue;
                }
                if can_y {
                    let a = st.y.get_or_insert(Anim::new(0.0));
                    let t = (a.target() + dy).clamp(0.0, st.max.y);
                    a.set(t, now, 0.12);
                    if st.pinned.is_some() {
                        st.pinned = Some(t >= st.max.y - 2.0);
                    }
                }
                if can_x {
                    let a = st.x.get_or_insert(Anim::new(0.0));
                    let t = (a.target() + dx).clamp(0.0, st.max.x);
                    a.set(t, now, 0.12);
                }
                st.last_activity = now;
                self.dirty = true;
                return;
            }
        }
    }

    fn scrollbar_hit(&self, i: usize, p: Point, x: bool, y: bool) -> Option<(bool, Rect)> {
        let n = &self.frame.nodes[i];
        let bars = scrollbar_rects(n);
        if y {
            if let Some((track, thumb)) = bars.0 {
                if track.outset(2.0).contains(p) {
                    return Some((true, thumb));
                }
            }
        }
        if x {
            if let Some((track, thumb)) = bars.1 {
                if track.outset(2.0).contains(p) {
                    return Some((false, thumb));
                }
            }
        }
        None
    }

    fn slider_set(&mut self, id: u64, p: Point) {
        let Some(n) = self.node_by_id(id) else { return };
        if let Behavior::Slider { min, max, step, .. } = n.behavior {
            let r = n.rect;
            let knob = r.h;
            let t = ((p.x - r.x - knob / 2.0) / (r.w - knob).max(1.0)).clamp(0.0, 1.0);
            let mut v = min + t * (max - min);
            if step > 0.0 {
                v = ((v - min) / step).round() * step + min;
            }
            if let Some(h) = &n.handlers.value {
                self.queue.push(h(v.clamp(min, max)));
            }
        }
    }

    fn input_hit(&mut self, id: u64, p: Point) -> Option<usize> {
        let &i = self.frame.by_id.get(&id)?;
        let n = &self.frame.nodes[i];
        let NodeContent::Input(spec) = &n.content else { return None };
        let inner = content_rect(n);
        if spec.multiline {
            let sy = self.inputs.get(&id).map(|s| s.scroll_y).unwrap_or(0.0);
            let (x, y) = (p.x - inner.x, p.y - inner.y + sy);
            return Some(self.text.hit_byte(&spec.value, None, &n.text, Some(inner.w.max(1.0)), self.scale, x, y));
        }
        let shown = display_value(spec);
        let scroll = self.inputs.get(&id).map(|s| s.scroll).unwrap_or(0.0);
        let x = p.x - inner.x + scroll;
        let stops = self.text.caret_stops(&shown, &n.text, self.scale);
        let mut best = (0usize, f32::MAX);
        for (b, sx) in stops {
            let d = (sx - x).abs();
            if d < best.1 {
                best = (b, d);
            }
        }
        Some(map_display_index(spec, best.0))
    }

    fn focusables(&self) -> Vec<u64> {
        self.frame
            .nodes
            .iter()
            .filter(|n| (n.focusable || matches!(n.content, NodeContent::Input(_))) && !n.disabled && n.rect.w > 0.0)
            .map(|n| n.id)
            .collect()
    }

    fn key(&mut self, k: KeyEvent) {
        self.dirty = true;
        if let Some(d) = self.open_dropdown() {
            if self.dropdown_key(d, &k) {
                return;
            }
        } else if let Some(f) = self.focused {
            if self.dropdowns.contains_key(&f) && matches!(k.key, Key::Enter | Key::Space | Key::Down) {
                let st = self.dropdowns.entry(f).or_default();
                st.open = true;
                st.filter.clear();
                return;
            }
        }
        // Focused text input
        if let Some(fid) = self.focused {
            if let Some(&i) = self.frame.by_id.get(&fid) {
                if let NodeContent::Input(_) = self.frame.nodes[i].content {
                    if self.input_key(i, &k) {
                        return;
                    }
                }
            }
        }
        if k.mods.command() && k.key == Key::Char('c') {
            if let Some(t) = self.selected_text() {
                self.clipboard = t.clone();
                self.requests.push(WindowRequest::SetClipboard(t));
                return;
            }
        }
        if k.mods.command() && k.key == Key::Char('a') {
            if let Some((id, _, _)) = self.text_sel {
                if let Some(len) = self.node_by_id(id).and_then(|n| match &n.content {
                    NodeContent::Text(t) => Some(t.text.len()),
                    _ => None,
                }) {
                    self.text_sel = Some((id, 0, len));
                    return;
                }
            }
        }
        if k.key == Key::Tab {
            let list = self.focusables();
            if !list.is_empty() {
                let pos = self.focused.and_then(|f| list.iter().position(|&x| x == f));
                let next = match (pos, k.mods.shift) {
                    (None, false) => 0,
                    (None, true) => list.len() - 1,
                    (Some(p), false) => (p + 1) % list.len(),
                    (Some(p), true) => (p + list.len() - 1) % list.len(),
                };
                self.focused = Some(list[next]);
                self.focus_visible = true;
                if let Some(st) = self.inputs.get_mut(&list[next]) {
                    st.blink_start = self.now;
                }
            }
            return;
        }
        if let Some(fid) = self.focused {
            if let Some(&i) = self.frame.by_id.get(&fid) {
                let n = &self.frame.nodes[i];
                // Activate focused buttons
                if matches!(k.key, Key::Enter | Key::Space) && !n.disabled {
                    if let Some(m) = n.handlers.click.clone() {
                        self.queue.push(m);
                        return;
                    }
                }
                // Slider keys
                if let Behavior::Slider { value, min, max, step } = n.behavior {
                    let st = if step > 0.0 { step } else { (max - min) / 100.0 };
                    let nv = match k.key {
                        Key::Left | Key::Down => Some(value - st),
                        Key::Right | Key::Up => Some(value + st),
                        Key::Home => Some(min),
                        Key::End => Some(max),
                        _ => None,
                    };
                    if let (Some(v), Some(h)) = (nv, n.handlers.value.clone()) {
                        self.queue.push(h(v.clamp(min, max)));
                        return;
                    }
                }
                // on_key bubbling
                for j in self.chain(i) {
                    if let Some(h) = &self.frame.nodes[j].handlers.key {
                        if let Some(m) = h(&k) {
                            self.queue.push(m);
                            return;
                        }
                    }
                }
            }
        }
        if k.key == Key::Escape && self.focused.is_some() {
            if let Some(m) = self.app.on_key(&k) {
                self.queue.push(m);
            } else {
                self.focused = None;
            }
            return;
        }
        if let Some(m) = self.app.on_key(&k) {
            self.queue.push(m);
        }
    }

    /// Returns true if the key was consumed by the input.
    fn input_key(&mut self, i: usize, k: &KeyEvent) -> bool {
        let n = &self.frame.nodes[i];
        let NodeContent::Input(spec) = &n.content else { return false };
        let id = n.id;
        let value = self.pending_values.get(&id).cloned().unwrap_or_else(|| spec.value.clone());
        let password = spec.password;
        let multiline = spec.multiline;
        let submit_on_enter = spec.submit_on_enter;
        let wrap_w = content_rect(n).w.max(1.0);
        let tstyle = n.text.clone();
        let on_input = n.handlers.input.clone();
        let on_submit = n.handlers.submit.clone();
        let st = self.inputs.entry(id).or_default();
        st.sel.clamp(&value);
        st.blink_start = self.now;
        let mut sel = st.sel;
        let cmd = k.mods.command();
        let word = if cfg!(target_os = "macos") { k.mods.alt } else { k.mods.ctrl };
        let mut new_value: Option<String> = None;
        let mv = |sel: &mut Selection, to: usize, extend: bool| {
            sel.cursor = to;
            if !extend {
                sel.anchor = to;
            }
        };
        match &k.key {
            Key::Left => {
                let to = if !sel.is_empty() && !k.mods.shift {
                    sel.range().0
                } else if word {
                    edit::prev_word(&value, sel.cursor)
                } else {
                    edit::prev_char(&value, sel.cursor)
                };
                mv(&mut sel, to, k.mods.shift);
            }
            Key::Right => {
                let to = if !sel.is_empty() && !k.mods.shift {
                    sel.range().1
                } else if word {
                    edit::next_word(&value, sel.cursor)
                } else {
                    edit::next_char(&value, sel.cursor)
                };
                mv(&mut sel, to, k.mods.shift);
            }
            Key::Up | Key::Down if multiline => {
                let scale = self.scale;
                let r = self.text.caret_rect(&value, &tstyle, Some(wrap_w), scale, sel.cursor);
                let y = if k.key == Key::Up { r.y - 1.0 } else { r.bottom() + 1.0 };
                let to = if y < 0.0 {
                    0
                } else {
                    let total = self.text.measure(&value, &tstyle, Some(wrap_w), scale).h;
                    if y >= total {
                        value.len()
                    } else {
                        self.text.hit_byte(&value, None, &tstyle, Some(wrap_w), scale, r.x, y)
                    }
                };
                mv(&mut sel, to, k.mods.shift);
            }
            Key::Home if multiline && !cmd => {
                let to = value[..sel.cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
                mv(&mut sel, to, k.mods.shift);
            }
            Key::End if multiline && !cmd => {
                let to = value[sel.cursor..].find('\n').map(|i| sel.cursor + i).unwrap_or(value.len());
                mv(&mut sel, to, k.mods.shift);
            }
            Key::Home | Key::Up => mv(&mut sel, 0, k.mods.shift),
            Key::End | Key::Down => mv(&mut sel, value.len(), k.mods.shift),
            Key::Enter if multiline && (!submit_on_enter || k.mods.shift) => {
                let (v, s) = edit::replace(&value, sel, "\n");
                sel = s;
                new_value = Some(v);
            }
            Key::Backspace => {
                if sel.is_empty() {
                    sel.anchor =
                        if word { edit::prev_word(&value, sel.cursor) } else { edit::prev_char(&value, sel.cursor) };
                }
                let (v, s) = edit::replace(&value, sel, "");
                sel = s;
                new_value = Some(v);
            }
            Key::Delete => {
                if sel.is_empty() {
                    sel.anchor =
                        if word { edit::next_word(&value, sel.cursor) } else { edit::next_char(&value, sel.cursor) };
                }
                let (v, s) = edit::replace(&value, sel, "");
                sel = s;
                new_value = Some(v);
            }
            Key::Enter => {
                if let Some(m) = on_submit {
                    self.queue.push(m);
                }
            }
            Key::Escape => {
                self.focused = None;
                return true;
            }
            Key::Char(c) if cmd => match c.to_ascii_lowercase() {
                'a' => sel = Selection { anchor: 0, cursor: value.len() },
                'c' | 'x' => {
                    let (a, b) = sel.range();
                    if a != b && !password {
                        let s = value[a..b].to_string();
                        self.clipboard = s.clone();
                        self.requests.push(WindowRequest::SetClipboard(s));
                        if c.eq_ignore_ascii_case(&'x') {
                            let (v, s2) = edit::replace(&value, sel, "");
                            sel = s2;
                            new_value = Some(v);
                        }
                    }
                }
                'v' => {
                    // The shell normally sends Event::Paste with the system clipboard;
                    // fall back to the internal clipboard.
                    let clip = self.clipboard.clone();
                    let (v, s) = edit::replace(&value, sel, &clip);
                    sel = s;
                    new_value = Some(v);
                }
                _ => return false,
            },
            Key::Tab => return false,
            _ => {
                // Printable keys arrive via Event::Text.
                return !cmd && matches!(k.key, Key::Char(_) | Key::Space);
            }
        }
        if let Some(st) = self.inputs.get_mut(&id) {
            st.sel = sel;
        }
        if let (Some(v), Some(h)) = (new_value, on_input) {
            if v != value {
                self.pending_values.insert(id, v.clone());
                self.queue.push(h(v));
            }
        }
        true
    }

    fn text_input(&mut self, t: &str) {
        let t: String = t.chars().filter(|c| !c.is_control()).collect();
        if t.is_empty() {
            return;
        }
        if let Some(d) = self.open_dropdown() {
            if let Some(st) = self.dropdowns.get_mut(&d) {
                let options = &st.options;
                if st.searchable {
                    st.filter.push_str(&t);
                    st.highlight = 0;
                } else {
                    // Type-ahead: jump to the first option starting with the typed text.
                    let q = t.to_lowercase();
                    if let Some(pos) = st
                        .visible
                        .iter()
                        .position(|&o| options.get(o).is_some_and(|l| l.to_lowercase().starts_with(&q)))
                    {
                        st.highlight = pos;
                    }
                }
            }
            self.dirty = true;
            return;
        }
        self.insert_text(&t);
    }

    fn paste(&mut self, t: &str) {
        self.clipboard = t.to_string();
        let multiline = self
            .focused
            .and_then(|f| self.node_by_id(f))
            .is_some_and(|n| matches!(&n.content, NodeContent::Input(s) if s.multiline));
        if multiline {
            let t: String =
                t.replace("\r\n", "\n").chars().filter(|c| *c == '\n' || *c == '\t' || !c.is_control()).collect();
            self.insert_text(&t);
            return;
        }
        let t: String =
            t.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).filter(|c| !c.is_control()).collect();
        self.insert_text(&t);
    }

    fn insert_text(&mut self, t: &str) {
        let Some(fid) = self.focused else { return };
        let Some(&i) = self.frame.by_id.get(&fid) else { return };
        let n = &self.frame.nodes[i];
        let NodeContent::Input(spec) = &n.content else { return };
        if n.disabled {
            return;
        }
        let value = self.pending_values.get(&fid).cloned().unwrap_or_else(|| spec.value.clone());
        let h = n.handlers.input.clone();
        let st = self.inputs.entry(fid).or_default();
        st.sel.clamp(&value);
        let (v, s) = edit::replace(&value, st.sel, t);
        st.sel = s;
        st.blink_start = self.now;
        if let Some(h) = h {
            self.pending_values.insert(fid, v.clone());
            self.queue.push(h(v));
        }
        self.dirty = true;
    }

    /// Window-chrome regions of the current frame (drag areas, maximize
    /// button, and interactive elements inside them).
    pub fn chrome_map(&self) -> ChromeMap {
        let mut regions = Vec::new();
        for &i in &self.frame.order {
            let n = &self.frame.nodes[i];
            if !n.pointer_events {
                continue;
            }
            let r = match n.clip {
                Some(c) => n.rect.intersect(&c),
                None => n.rect,
            };
            if r.is_empty() {
                continue;
            }
            let h = &n.handlers;
            let kind = match n.behavior {
                Behavior::WindowDrag => ChromeHit::Caption,
                Behavior::WindowControl(WindowControl::ToggleMaximize) => ChromeHit::Maximize,
                Behavior::None
                    if h.click.is_none()
                        && h.double_click.is_none()
                        && h.drag.is_none()
                        && h.context_menu.is_none()
                        && h.drop_target.is_none()
                        && !n.focusable
                        && !matches!(n.content, NodeContent::Input(_)) =>
                {
                    continue
                }
                _ => ChromeHit::Client,
            };
            regions.push((r, kind));
        }
        // Content outside any caption doesn't need entries: the default is Client.
        let first_caption = regions.iter().position(|r| r.1 != ChromeHit::Client).unwrap_or(regions.len());
        regions.drain(..first_caption);
        ChromeMap { regions }
    }

    /// Programmatically scroll a scroll container (by `.id`) to an offset.
    pub fn scroll_to(&mut self, id: &str, y: f32) {
        let gid = global_id(id);
        if let Some(n) = self.frame.nodes.iter().find(|n| n.key == Some(gid)) {
            let st = self.scrolls.entry(n.id).or_default();
            let a = st.y.get_or_insert(Anim::new(0.0));
            a.set(y.clamp(0.0, st.max.y), self.now, 0.2);
            self.dirty = true;
        }
    }

    /// Scroll a [`virtual_list`] (by `.id`) so row `index` is at the top.
    pub fn scroll_to_item(&mut self, id: &str, index: usize) {
        self.pending_scrolls.push((global_id(id), ScrollCmd::ToItem(index)));
        self.dirty = true;
    }

    /// How many rows of the [`virtual_list`] with this `.id` are currently built
    /// (for tests and tooling).
    pub fn virtual_rows_built(&self, id: &str) -> Option<usize> {
        let gid = global_id(id);
        let n = self.frame.nodes.iter().find(|n| n.key == Some(gid))?;
        self.virt.get(&n.id).map(|v| v.built.1 - v.built.0)
    }

    /// User-resized column widths of the [`table`] with this id (`None` = the
    /// column's declared width), e.g. to persist them.
    pub fn column_widths(&self, id: &str) -> Vec<Option<f32>> {
        self.tables.get(&global_id(id)).cloned().unwrap_or_default()
    }

    /// Restore column widths saved with [`Runtime::column_widths`].
    pub fn set_column_widths(&mut self, id: &str, widths: Vec<Option<f32>>) {
        self.tables.insert(global_id(id), widths);
        self.dirty = true;
    }

    /// Current rectangle of the element with the given `.id` (for tests and tooling).
    pub fn rect_of(&self, id: &str) -> Option<Rect> {
        let gid = global_id(id);
        self.frame.nodes.iter().find(|n| n.key == Some(gid)).map(|n| n.rect)
    }

    /// Rectangle of the first text element whose content equals `s` (for tests and tooling).
    pub fn rect_of_text(&self, s: &str) -> Option<Rect> {
        self.frame.nodes.iter().find(|n| matches!(&n.content, NodeContent::Text(t) if t.text == s)).map(|n| n.rect)
    }

    /// Current user-resized sizes of a split (by its id).
    pub fn split_sizes(&self, id: &str) -> Option<Vec<f32>> {
        let gid = global_id(id);
        let n = self.frame.nodes.iter().find(|n| n.key == Some(gid))?;
        self.splits.get(&n.id).map(|s| s.report())
    }
}

fn edge_cursor(e: ResizeEdge) -> Cursor {
    match e {
        ResizeEdge::N | ResizeEdge::S => Cursor::ResizeNs,
        ResizeEdge::E | ResizeEdge::W => Cursor::ResizeEw,
        ResizeEdge::NW | ResizeEdge::SE => Cursor::ResizeNwse,
        ResizeEdge::NE | ResizeEdge::SW => Cursor::ResizeNesw,
    }
}

fn display_value(spec: &InputSpec) -> String {
    if spec.password {
        "•".repeat(spec.value.chars().count())
    } else {
        spec.value.clone()
    }
}

/// Map a byte index in the displayed string back to the real value.
fn map_display_index(spec: &InputSpec, i: usize) -> usize {
    if spec.password {
        let chars = i / '•'.len_utf8();
        spec.value.char_indices().nth(chars).map(|(b, _)| b).unwrap_or(spec.value.len())
    } else {
        i
    }
}

fn map_value_index(spec: &InputSpec, i: usize) -> usize {
    if spec.password {
        spec.value[..i.min(spec.value.len())].chars().count() * '•'.len_utf8()
    } else {
        i
    }
}

fn inner_rect(r: Rect, b: &Edges) -> Rect {
    Rect::new(r.x + b.left, r.y + b.top, (r.w - b.left - b.right).max(0.0), (r.h - b.top - b.bottom).max(0.0))
}

fn content_rect<M>(n: &Node<M>) -> Rect {
    let b = n.style.border_width;
    let p = n.style.padding;
    Rect::new(
        n.rect.x + b.left + p.left,
        n.rect.y + b.top + p.top,
        (n.rect.w - b.left - b.right - p.left - p.right).max(0.0),
        (n.rect.h - b.top - b.bottom - p.top - p.bottom).max(0.0),
    )
}

/// (vertical track+thumb, horizontal track+thumb)
type Bars = (Option<(Rect, Rect)>, Option<(Rect, Rect)>);

fn scrollbar_rects<M>(n: &Node<M>) -> Bars {
    let Behavior::Scroll { x, y } = n.behavior else { return (None, None) };
    let r = inner_rect(n.rect, &n.style.border_width);
    let thick = 10.0;
    let mut out = (None, None);
    if y && n.content_size.h > r.h + 0.5 {
        let track = Rect::new(r.right() - thick, r.y, thick, r.h);
        let th = (r.h / n.content_size.h * r.h).max(24.0);
        let max = n.content_size.h - r.h;
        let ty = r.y + (n.scroll.y / max) * (r.h - th);
        out.0 = Some((track, Rect::new(track.x, ty, thick, th)));
    }
    if x && n.content_size.w > r.w + 0.5 {
        let track = Rect::new(r.x, r.bottom() - thick, r.w, thick);
        let tw = (r.w / n.content_size.w * r.w).max(24.0);
        let max = n.content_size.w - r.w;
        let tx = r.x + (n.scroll.x / max) * (r.w - tw);
        out.1 = Some((track, Rect::new(tx, track.y, tw, thick)));
    }
    out
}

/// Where a text node's glyphs start and its wrap width (mirrors painting).
fn text_frame<M>(text: &mut TextSystem, n: &Node<M>, spec: &TextSpec, scale: f32) -> (Rect, Option<f32>) {
    let cr = content_rect(n);
    let wrap = if spec.wrap { Some(cr.w.max(1.0)) } else { None };
    let mut tr = cr;
    if !spec.wrap {
        let m = text.measure_rich(&spec.text, spec.spans.as_deref(), &n.text, None, scale);
        match n.style.text_align.unwrap_or_default() {
            TextAlign::Left => {}
            TextAlign::Center => tr.x += ((cr.w - m.w) / 2.0).max(0.0),
            TextAlign::Right => tr.x += (cr.w - m.w).max(0.0),
        }
        tr.y += ((cr.h - m.h) / 2.0).max(0.0);
    }
    (tr, wrap)
}

fn measure_node<M>(
    text: &mut TextSystem,
    node: &Node<M>,
    known: tf::Size<Option<f32>>,
    avail: tf::Size<tf::AvailableSpace>,
    scale: f32,
) -> tf::Size<f32> {
    match &node.content {
        NodeContent::Text(spec) => {
            let max_w = if spec.wrap {
                match (known.width, avail.width) {
                    (Some(w), _) => Some(w),
                    (None, tf::AvailableSpace::Definite(w)) => Some(w),
                    (None, tf::AvailableSpace::MinContent) => Some(0.0),
                    (None, tf::AvailableSpace::MaxContent) => None,
                }
            } else {
                None
            };
            let s = text.measure_rich(&spec.text, spec.spans.as_deref(), &node.text, max_w, scale);
            let mut w = s.w;
            if spec.ellipsis {
                if let tf::AvailableSpace::MinContent = avail.width {
                    w = 0.0;
                }
            }
            tf::Size { width: known.width.unwrap_or(w.ceil()), height: known.height.unwrap_or(s.h) }
        }
        NodeContent::Input(spec) if spec.multiline => {
            let lh = node.text.size * node.text.line_height;
            let w = match (known.width, avail.width) {
                (Some(w), _) | (None, tf::AvailableSpace::Definite(w)) => Some(w.max(1.0)),
                _ => None,
            };
            let content = if spec.value.is_empty() { " " } else { spec.value.as_str() };
            let h = text.measure(content, &node.text, w, scale).h;
            let rows = (h / lh).round().clamp(spec.rows.0 as f32, spec.rows.1 as f32);
            tf::Size { width: known.width.unwrap_or(0.0), height: known.height.unwrap_or((rows * lh).ceil()) }
        }
        NodeContent::Input(_) => {
            let h = (node.text.size * node.text.line_height).ceil();
            tf::Size { width: known.width.unwrap_or(0.0), height: known.height.unwrap_or(h) }
        }
        _ => tf::Size::ZERO,
    }
}

fn lp(v: f32) -> tf::LengthPercentage {
    tf::LengthPercentage::length(v)
}

fn to_dim(l: Length) -> tf::Dimension {
    match l {
        Length::Auto => tf::Dimension::auto(),
        Length::Px(v) => tf::Dimension::length(v),
        Length::Percent(p) => tf::Dimension::percent(p / 100.0),
    }
}

fn to_lpa(l: Length) -> tf::LengthPercentageAuto {
    match l {
        Length::Auto => tf::LengthPercentageAuto::auto(),
        Length::Px(v) => tf::LengthPercentageAuto::length(v),
        Length::Percent(p) => tf::LengthPercentageAuto::percent(p / 100.0),
    }
}

fn to_align(a: Align) -> tf::AlignItems {
    match a {
        Align::Start => tf::AlignItems::FLEX_START,
        Align::End => tf::AlignItems::FLEX_END,
        Align::Center => tf::AlignItems::CENTER,
        Align::Stretch => tf::AlignItems::STRETCH,
        Align::Baseline => tf::AlignItems::BASELINE,
    }
}

fn to_justify(j: Justify) -> tf::JustifyContent {
    match j {
        Justify::Start => tf::JustifyContent::FLEX_START,
        Justify::End => tf::JustifyContent::FLEX_END,
        Justify::Center => tf::JustifyContent::CENTER,
        Justify::SpaceBetween => tf::JustifyContent::SPACE_BETWEEN,
        Justify::SpaceAround => tf::JustifyContent::SPACE_AROUND,
        Justify::SpaceEvenly => tf::JustifyContent::SPACE_EVENLY,
    }
}

fn to_track(t: Track) -> tf::GridTemplateComponent<String> {
    use tf::style_helpers::*;
    match t {
        Track::Px(v) => length(v),
        Track::Fr(v) => fr(v),
        Track::Auto => auto(),
        Track::Percent(v) => percent(v / 100.0),
    }
}

fn to_taffy(s: &Style) -> tf::Style {
    let mut t = tf::Style {
        display: match s.display {
            Display::Flex => tf::Display::Flex,
            Display::Grid => tf::Display::Grid,
            Display::Block => tf::Display::Block,
            Display::None => tf::Display::None,
        },
        flex_direction: match s.direction {
            Direction::Row => tf::FlexDirection::Row,
            Direction::Column => tf::FlexDirection::Column,
            Direction::RowReverse => tf::FlexDirection::RowReverse,
            Direction::ColumnReverse => tf::FlexDirection::ColumnReverse,
        },
        flex_wrap: if s.wrap { tf::FlexWrap::Wrap } else { tf::FlexWrap::NoWrap },
        justify_content: s.justify.map(to_justify),
        align_items: s.align_items.map(to_align),
        align_self: s.align_self.map(to_align),
        align_content: s.align_content.map(to_justify),
        gap: tf::Size { width: lp(s.gap.0), height: lp(s.gap.1) },
        flex_grow: s.grow,
        flex_shrink: s.shrink,
        flex_basis: to_dim(s.basis),
        size: tf::Size { width: to_dim(s.width), height: to_dim(s.height) },
        min_size: tf::Size { width: to_lpa(s.min_width), height: to_lpa(s.min_height) },
        max_size: tf::Size { width: to_lpa(s.max_width), height: to_lpa(s.max_height) },
        aspect_ratio: s.aspect_ratio,
        padding: tf::Rect {
            left: lp(s.padding.left),
            right: lp(s.padding.right),
            top: lp(s.padding.top),
            bottom: lp(s.padding.bottom),
        },
        border: tf::Rect {
            left: lp(s.border_width.left),
            right: lp(s.border_width.right),
            top: lp(s.border_width.top),
            bottom: lp(s.border_width.bottom),
        },
        margin: tf::Rect {
            top: if s.margin_auto[0] {
                tf::LengthPercentageAuto::auto()
            } else {
                tf::LengthPercentageAuto::length(s.margin.top)
            },
            right: if s.margin_auto[1] {
                tf::LengthPercentageAuto::auto()
            } else {
                tf::LengthPercentageAuto::length(s.margin.right)
            },
            bottom: if s.margin_auto[2] {
                tf::LengthPercentageAuto::auto()
            } else {
                tf::LengthPercentageAuto::length(s.margin.bottom)
            },
            left: if s.margin_auto[3] {
                tf::LengthPercentageAuto::auto()
            } else {
                tf::LengthPercentageAuto::length(s.margin.left)
            },
        },
        position: match s.position {
            Position::Relative => tf::Position::Relative,
            Position::Absolute | Position::Fixed => tf::Position::Absolute,
        },
        inset: tf::Rect {
            top: to_lpa(s.inset[0]),
            right: to_lpa(s.inset[1]),
            bottom: to_lpa(s.inset[2]),
            left: to_lpa(s.inset[3]),
        },
        overflow: {
            let o = match s.overflow {
                Overflow::Visible => tf::Overflow::Visible,
                Overflow::Hidden => tf::Overflow::Hidden,
                Overflow::Scroll => tf::Overflow::Scroll,
            };
            tf::Point { x: o, y: o }
        },
        scrollbar_width: 0.0,
        ..Default::default()
    };
    if s.display == Display::Grid {
        t.grid_template_columns = s.grid_columns.iter().map(|&c| to_track(c)).collect();
        t.grid_template_rows = s.grid_rows.iter().map(|&c| to_track(c)).collect();
    }
    if s.grid_column_span > 1 {
        t.grid_column = tf::Line { start: tf::style_helpers::span(s.grid_column_span), end: tf::GridPlacement::Auto };
    }
    if s.grid_row_span > 1 {
        t.grid_row = tf::Line { start: tf::style_helpers::span(s.grid_row_span), end: tf::GridPlacement::Auto };
    }
    t
}

// ---------------------------------------------------------------------------
// Painting
// ---------------------------------------------------------------------------

struct PaintCtx<'a, M> {
    nodes: &'a [Node<M>],
    now: f64,
    focused: Option<u64>,
    focus_visible: bool,
    hovered: &'a HashSet<u64>,
    scrolls: &'a HashMap<u64, ScrollState>,
    inputs: &'a HashMap<u64, InputState>,
    drag: &'a Drag,
    splitter_hover: Option<(u64, f64)>,
    text_sel: Option<(u64, usize, usize)>,
    theme: &'a Theme,
    window_focused: bool,
}

fn paint_node<M>(
    ctx: &PaintCtx<M>,
    i: usize,
    c: &mut Canvas,
    deferred: &mut Vec<(i32, usize)>,
    order: &mut Vec<usize>,
    allow_portal: bool,
) {
    let n = &ctx.nodes[i];
    if n.style.display == Display::None {
        return;
    }
    if n.style.z_index > 0 && n.parent.is_some() && allow_portal {
        // Deferred to the overlay pass (escapes clipping, painted on top).
        deferred.push((n.style.z_index, i));
        return;
    }
    order.push(i);
    let s = &n.style;
    let r = n.rect;
    if let Some(cl) = n.clip {
        if cl.intersect(&r.outset(64.0)).is_empty() && s.shadows.is_empty() {
            // Entirely clipped out; children can't be visible either (they share the clip)
            // unless they are portals, which are handled separately.
            collect_portals(ctx, i, deferred);
            return;
        }
    }
    let layer = s.opacity < 0.999;
    if layer {
        if s.opacity <= 0.001 {
            collect_portals(ctx, i, deferred);
            return;
        }
        // Bound the layer to the element (plus room for shadows/outlines) when it clips its children.
        let bounds = if s.overflow != Overflow::Visible {
            Some(r.outset(
                s.shadows.iter().map(|sh| sh.blur * 1.5 + sh.spread + sh.x.abs().max(sh.y.abs())).fold(4.0, f32::max),
            ))
        } else {
            None
        };
        c.push_layer(s.opacity, bounds);
    }
    for sh in &s.shadows {
        c.box_shadow(r, s.radius, sh);
    }
    if let Some(bg) = &s.background {
        c.fill_rrect(r, s.radius, bg);
    }
    if !s.border_width.is_zero() {
        c.border(r, s.radius, s.border_width, s.border_color);
    }

    // Splitter highlight
    if let Behavior::Splitter { split, index, axis } = n.behavior {
        let dragging = matches!(ctx.drag, Drag::Splitter { split: sp, index: ix, .. } if *sp == split && *ix == index);
        // Like VS Code: highlight after a short hover delay, then fade in.
        let hover_t = match ctx.splitter_hover {
            Some((id, since)) if id == n.id => {
                ((ctx.now - since - ctx.theme.splitter_hover_delay as f64) / 0.12).clamp(0.0, 1.0) as f32
            }
            _ => 0.0,
        };
        let alpha = if dragging { 1.0 } else { hover_t };
        if alpha > 0.0 {
            let hr = match axis {
                Axis::Horizontal => Rect::new(r.center().x - 1.5, r.y, 3.0, r.h),
                Axis::Vertical => Rect::new(r.x, r.center().y - 1.5, r.w, 3.0),
            };
            let saved = c.take_clips();
            c.fill_rect(hr, ctx.theme.colors.accent.fade(alpha));
            c.restore_clips(saved);
        }
    }

    // Content
    let cr = content_rect(n);
    match &n.content {
        NodeContent::Text(spec) => {
            let scale = c.scale;
            let (tr, wrap) = text_frame(c.text, n, spec, scale);
            if let Some((id, a, b)) = ctx.text_sel {
                if id == n.id && a != b {
                    let (a, b) = (a.min(b), a.max(b));
                    let sel = ctx.theme.colors.selection;
                    for r in c.text.selection_rects(&spec.text, spec.spans.as_deref(), &n.text, wrap, scale, a, b) {
                        c.fill_rect(r.translate(tr.x, tr.y), sel);
                    }
                }
            }
            let link = ctx.theme.colors.accent;
            c.rich_text_block(&spec.text, spec.spans.as_deref(), &n.text, tr, wrap, n.color, link, spec.ellipsis);
        }
        NodeContent::Icon(icon) => {
            let stroke = 2.0 * (n.text.weight as f32 / 400.0).clamp(0.6, 1.6) * 0.9;
            c.icon(icon, cr, n.color, stroke);
        }
        NodeContent::Canvas(f) => {
            c.push_clip(r, Corners::ZERO);
            f(c, cr);
            c.pop_clip();
        }
        NodeContent::Input(spec) => paint_input(ctx, n, spec, cr, c),
        NodeContent::None => {}
    }

    // Children
    let clips = s.overflow != Overflow::Visible;
    if clips {
        let inner = inner_rect(r, &s.border_width);
        c.push_clip(inner, s.radius.shrink(s.border_width.top.max(s.border_width.left)));
    }
    for &ch in &n.children {
        paint_node(ctx, ch, c, deferred, order, true);
    }
    if clips {
        c.pop_clip();
    }

    // Overlay scrollbars
    if let Behavior::Scroll { .. } = n.behavior {
        let st = ctx.scrolls.get(&n.id);
        let recent = st.map(|s| ctx.now - s.last_activity).unwrap_or(99.0);
        let hovered = ctx.hovered.contains(&n.id);
        let dragging = matches!(ctx.drag, Drag::ScrollThumb { node, .. } if *node == n.id);
        let vis = if dragging || hovered || recent < 0.8 {
            1.0
        } else if recent < 1.2 {
            ((1.2 - recent) / 0.4) as f32
        } else {
            0.0
        };
        if vis > 0.0 {
            let (v, h) = scrollbar_rects(n);
            for (vertical, bar) in [(true, v), (false, h)] {
                if let Some((_, thumb)) = bar {
                    let wide = dragging;
                    let t = if vertical {
                        let w = if wide { 8.0 } else { 6.0 };
                        Rect::new(thumb.right() - w - 2.0, thumb.y + 2.0, w, thumb.h - 4.0)
                    } else {
                        let hh = if wide { 8.0 } else { 6.0 };
                        Rect::new(thumb.x + 2.0, thumb.bottom() - hh - 2.0, thumb.w - 4.0, hh)
                    };
                    let col = if dragging { ctx.theme.colors.scrollbar_hover } else { ctx.theme.colors.scrollbar };
                    c.fill_rounded(t, 4.0, col.fade(vis));
                }
            }
        }
    }

    // Focus ring / outline
    let show_ring = ctx.focused == Some(n.id) && ctx.focus_visible && n.focusable && s.outline.is_none();
    if let Some(o) = s.outline {
        let or = r.outset(o.offset + o.width / 2.0);
        c.stroke_rrect(or, s.radius.grow(o.offset + o.width / 2.0), o.width, o.color);
    } else if show_ring {
        // shadcn-style ring: a soft band just outside the border box, keyboard focus only.
        let w = ctx.theme.focus_ring_width;
        c.stroke_rrect(r.outset(w / 2.0), s.radius.grow(w / 2.0), w, ctx.theme.colors.focus_ring);
    }
    if layer {
        c.pop_layer();
    }
}

/// Paint order (back to front) used for hit testing. Mirrors `paint_node`:
/// elements with a positive z-index are lifted into later layers.
fn paint_order<M>(nodes: &[Node<M>]) -> Vec<usize> {
    fn walk<M>(nodes: &[Node<M>], i: usize, out: &mut Vec<usize>, deferred: &mut Vec<(i32, usize)>, root: bool) {
        let n = &nodes[i];
        if n.style.display == Display::None {
            return;
        }
        if n.style.z_index > 0 && n.parent.is_some() && !root {
            deferred.push((n.style.z_index, i));
            return;
        }
        out.push(i);
        for &c in &n.children {
            walk(nodes, c, out, deferred, false);
        }
    }
    let mut out = Vec::with_capacity(nodes.len());
    if nodes.is_empty() {
        return out;
    }
    let mut deferred = Vec::new();
    walk(nodes, 0, &mut out, &mut deferred, true);
    while !deferred.is_empty() {
        deferred.sort_by_key(|d| d.0);
        for (_, i) in std::mem::take(&mut deferred) {
            walk(nodes, i, &mut out, &mut deferred, true);
        }
    }
    out
}

fn collect_portals<M>(ctx: &PaintCtx<M>, i: usize, deferred: &mut Vec<(i32, usize)>) {
    for &ch in &ctx.nodes[i].children {
        if ctx.nodes[ch].style.z_index > 0 {
            deferred.push((ctx.nodes[ch].style.z_index, ch));
        } else {
            collect_portals(ctx, ch, deferred);
        }
    }
}

fn paint_input<M>(ctx: &PaintCtx<M>, n: &Node<M>, spec: &InputSpec, cr: Rect, c: &mut Canvas) {
    let th = ctx.theme;
    let focused = ctx.focused == Some(n.id);
    let st = ctx.inputs.get(&n.id).copied().unwrap_or_default();
    if spec.multiline {
        let scale = c.scale;
        let wrap = Some(cr.w.max(1.0));
        c.push_clip(cr.outset(1.0), Corners::ZERO);
        let origin = Rect::new(cr.x, cr.y - st.scroll_y, cr.w, cr.h + st.scroll_y);
        if spec.value.is_empty() && !spec.placeholder.is_empty() {
            c.text_block(&spec.placeholder, &n.text, origin, wrap, th.colors.text_faint, false);
        }
        let mut sel = st.sel;
        sel.clamp(&spec.value);
        if focused && !sel.is_empty() {
            let (a, b) = sel.range();
            for r in c.text.selection_rects(&spec.value, None, &n.text, wrap, scale, a, b) {
                c.fill_rect(r.translate(origin.x, origin.y), th.colors.selection);
            }
        }
        if !spec.value.is_empty() {
            c.text_block(&spec.value, &n.text, origin, wrap, n.color, false);
        }
        if focused && ctx.window_focused && ((ctx.now - st.blink_start) / 0.53).floor() as i64 % 2 == 0 {
            let r = c.text.caret_rect(&spec.value, &n.text, wrap, scale, sel.cursor);
            c.fill_rect(
                Rect::new((origin.x + r.x).round() - 0.5, origin.y + r.y + 2.0, 1.5, r.h - 4.0),
                th.colors.accent,
            );
        }
        c.pop_clip();
        return;
    }
    let shown = display_value(spec);
    let scale = c.scale;
    c.push_clip(cr.outset(1.0), Corners::ZERO);
    let line_h = n.text.size * n.text.line_height;
    let ty = cr.y + ((cr.h - line_h) / 2.0).max(0.0);
    if shown.is_empty() && !spec.placeholder.is_empty() {
        c.text_block(&spec.placeholder, &n.text, Rect::new(cr.x, ty, cr.w, line_h), None, th.colors.text_faint, true);
    }
    let stops = c.text.caret_stops(&shown, &n.text, scale);
    let x_of = |b: usize| {
        stops.iter().find(|s| s.0 >= b).map(|s| s.1).unwrap_or_else(|| stops.last().map(|s| s.1).unwrap_or(0.0))
    };
    let mut sel = st.sel;
    sel.clamp(&spec.value);
    let caret_b = map_value_index(spec, sel.cursor);
    let anchor_b = map_value_index(spec, sel.anchor);
    let caret_x = x_of(caret_b);
    let scroll = st.scroll;
    let total = stops.last().map(|s| s.1).unwrap_or(0.0);
    if focused && sel.cursor != sel.anchor {
        let (a, b) = (caret_b.min(anchor_b), caret_b.max(anchor_b));
        let (xa, xb) = (x_of(a), x_of(b));
        c.fill_rect(Rect::new(cr.x + xa - scroll, ty, xb - xa, line_h), th.colors.selection);
    }
    if !shown.is_empty() {
        c.text_block(&shown, &n.text, Rect::new(cr.x - scroll, ty, total + 4.0, line_h), None, n.color, false);
    }
    if focused && ctx.window_focused {
        let blink_on = ((ctx.now - st.blink_start) / 0.53).floor() as i64 % 2 == 0;
        if blink_on {
            c.fill_rect(
                Rect::new((cr.x + caret_x - scroll).round() - 0.5, ty + 1.0, 1.5, line_h - 2.0),
                th.colors.accent,
            );
        }
    }
    c.pop_clip();
}

fn paint_tooltip(c: &mut Canvas, th: &Theme, tip: &str, pos: Point, win: Size) {
    let ts = TextStyle {
        size: th.font_size_sm,
        weight: 400,
        family: th.font.clone(),
        italic: false,
        line_height: 1.35,
        letter_spacing: 0.0,
    };
    let m = c.measure_text(tip, &ts, Some(320.0));
    let (px, py) = (8.0, 5.0);
    let w = m.w + px * 2.0;
    let h = m.h + py * 2.0;
    let mut x = pos.x + 10.0;
    let mut y = pos.y + 22.0;
    if x + w > win.w - 4.0 {
        x = (win.w - 4.0 - w).max(4.0);
    }
    if y + h > win.h - 4.0 {
        y = pos.y - h - 8.0;
    }
    let r = Rect::new(x, y, w, h);
    for sh in &th.shadow_popover {
        c.box_shadow(r, Corners::all(th.radius_sm), sh);
    }
    c.fill_rrect(r, Corners::all(th.radius_sm), &Fill::Solid(th.colors.elevated));
    c.border(r, Corners::all(th.radius_sm), Edges::all(1.0), th.colors.border_strong);
    c.text_block(tip, &ts, Rect::new(x + px, y + py, m.w + 1.0, m.h), Some(320.0), th.colors.text, false);
}
