//! Several windows of one app: each window has its own [`Runtime`] (hover,
//! focus, scroll and layout state) around a shared app.

use std::cell::RefCell;
use std::rc::Rc;

use super::{App, Cx};
use crate::element::{Element, KeyEvent};
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
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub min_width: f32,
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
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = w;
        self.height = h;
        self
    }
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
    pub app: Rc<RefCell<A>>,
    pub key: Option<String>,
}

impl<A: App> Shared<A> {
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

    fn windows(&self) -> Vec<super::WindowSpec<A::Msg>> {
        self.app.borrow().windows()
    }
}
