//! Several windows of one app: each window has its own [`Runtime`] (hover,
//! focus, scroll and layout state) around a shared app.

use std::cell::RefCell;
use std::rc::Rc;

use super::{App, Cx, Runtime};
use crate::element::{Element, KeyEvent};
use crate::geometry::{Point, Rect};
use crate::subscription::Subscriptions;
use crate::theme::Theme;

/// An extra window declared by [`App::windows`].
///
/// ```
/// # use rust_ui::prelude::*;
/// # #[derive(Clone)] enum Msg { CloseInspector }
/// # struct MyApp { inspector_open: bool }
/// # impl MyApp {
/// fn windows(&self) -> Vec<WindowSpec<Msg>> {
///     let mut w = Vec::new();
///     if self.inspector_open {
///         w.push(WindowSpec::new("inspector", "Inspector", Msg::CloseInspector).size(360.0, 520.0));
///     }
///     w
/// }
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct WindowSpec<M> {
    /// Identifies the window across frames (and in [`App::window_view`]).
    pub key: String,
    /// The window title.
    pub title: String,
    /// Initial width (logical px).
    pub width: f32,
    /// Initial height (logical px).
    pub height: f32,
    /// Minimum width (logical px).
    pub min_width: f32,
    /// Minimum height (logical px).
    pub min_height: f32,
    /// Initial position of the window's top-left corner on the screen
    /// (logical px); the OS places it when `None`.
    pub position: Option<(f32, f32)>,
    /// Draw no OS title bar (see [`WindowOptions::frameless`](crate::WindowOptions)).
    pub frameless: bool,
    /// Sent when the user closes the window.
    pub on_close: M,
}

impl<M> WindowSpec<M> {
    /// A 640×480 window identified by `key`, sending `on_close` when closed.
    pub fn new(key: impl Into<String>, title: impl Into<String>, on_close: M) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            width: 640.0,
            height: 480.0,
            min_width: 200.0,
            min_height: 150.0,
            position: None,
            frameless: false,
            on_close,
        }
    }
    /// Set the initial size (logical px).
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = w;
        self.height = h;
        self
    }
    /// Set the minimum size (logical px).
    pub fn min_size(mut self, w: f32, h: f32) -> Self {
        self.min_width = w;
        self.min_height = h;
        self
    }
    /// Place the window's top-left corner at this screen position (logical px).
    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.position = Some((x, y));
        self
    }
    /// Draw no OS title bar.
    pub fn frameless(mut self, f: bool) -> Self {
        self.frameless = f;
        self
    }
}

/// One window's view of an app shared by several windows: the main window
/// (`key: None`) or an extra window. Each window has its own [`Runtime`]
/// around one of these; see [`HeadlessApp`](crate::headless::HeadlessApp).
///
/// [`Runtime`]: super::Runtime
pub struct Shared<A: App> {
    /// The app, shared by all windows.
    pub app: Rc<RefCell<A>>,
    /// The extra window's key, or `None` for the main window.
    pub key: Option<String>,
}

impl<A: App> Shared<A> {
    /// The window `key` (`None` for the main window) of the shared `app`.
    pub fn new(app: Rc<RefCell<A>>, key: Option<String>) -> Self {
        Self { app, key }
    }
}

impl<A: App> App for Shared<A> {
    type Msg = A::Msg;

    fn update(&mut self, msg: A::Msg, cx: &mut Cx<A::Msg>) {
        self.app.borrow_mut().update(msg, cx);
    }

    fn view(&self) -> Element<A::Msg> {
        let app = self.app.borrow();
        match &self.key {
            None => app.view(),
            Some(k) => app.window_view(k),
        }
    }

    fn theme(&self) -> Theme {
        self.app.borrow().theme()
    }

    fn subscriptions(&self) -> Subscriptions<A::Msg> {
        // Timers and app-level subscriptions run once, in the main window.
        match self.key {
            None => self.app.borrow().subscriptions(),
            Some(_) => Subscriptions::none(),
        }
    }

    fn on_key(&self, e: &KeyEvent) -> Option<A::Msg> {
        self.app.borrow().on_key(e)
    }

    fn menu(&self) -> Vec<crate::widgets::Menu<A::Msg>> {
        self.app.borrow().menu()
    }

    fn commands(&self) -> crate::commands::Commands<A::Msg> {
        self.app.borrow().commands()
    }

    fn windows(&self) -> Vec<super::WindowSpec<A::Msg>> {
        self.app.borrow().windows()
    }
}

/// Forward a drag in window `src` (pointer at `local`, its coordinates) to
/// the app's other windows: the window under the pointer gets it as an
/// external drag (and the drop, with `drop`); the others are told it left.
/// Windows without a known screen position (Wayland) take no part.
pub(crate) fn route_drag<A: App>(rts: &mut [&mut Runtime<Shared<A>>], src: usize, local: Point, drop: bool) {
    let Some(origin) = rts[src].screen_origin() else { return };
    let size = rts[src].window_size();
    let screen = origin + local;
    let inside_src = Rect::new(0.0, 0.0, size.w, size.h).contains(local);
    let target = (!inside_src)
        .then(|| {
            (0..rts.len()).find(|&j| {
                j != src
                    && rts[j].screen_origin().is_some_and(|o| {
                        let s = rts[j].window_size();
                        Rect::new(o.x, o.y, s.w, s.h).contains(screen)
                    })
            })
        })
        .flatten();
    for (j, rt) in rts.iter_mut().enumerate() {
        if j == src {
            continue;
        }
        match (Some(j) == target, rt.screen_origin()) {
            (true, Some(o)) => rt.external_drag(Some(screen - o), drop),
            _ => rt.external_drag(None, false),
        }
    }
}

/// Drop a drag from window `src` into window `dst` at `local` (`dst`'s
/// coordinates), for platforms that don't report window positions
/// (Wayland). There the pointer shows up in `dst` right after the button is
/// released over it: `dst` takes the drop, then `src`'s drag ends.
pub(crate) fn drop_into<A: App>(rts: &mut [&mut Runtime<Shared<A>>], src: usize, dst: usize, local: Point) {
    if src == dst {
        return;
    }
    rts[dst].external_drag(Some(local), false);
    rts[dst].external_drag(Some(local), true);
}
