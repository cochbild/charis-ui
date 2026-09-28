//! Declarative subscriptions to time and window events.
//!
//! [`App::subscriptions`](crate::App::subscriptions) is re-evaluated after
//! every update, so subscriptions can be conditional:
//!
//! ```
//! # use charis_ui::prelude::*;
//! # use std::time::Duration;
//! # #[derive(Clone)] enum Msg { Tick, CloseRequested, Resized(Size) }
//! # let busy = true;
//! let subs = Subscriptions::none()
//!     .every_if(busy, Duration::from_secs(1), Msg::Tick)
//!     .on_close_request(Msg::CloseRequested)
//!     .on_resize(Msg::Resized);
//! ```

use std::rc::Rc;
use std::time::Duration;

use crate::geometry::Size;

/// The set of external events an app listens to.
pub struct Subscriptions<M> {
    pub(crate) timers: Vec<(Duration, M)>,
    pub(crate) close_requested: Option<M>,
    pub(crate) resized: Option<Rc<dyn Fn(Size) -> M>>,
    pub(crate) focus: Option<Rc<dyn Fn(bool) -> M>>,
}

impl<M> Default for Subscriptions<M> {
    fn default() -> Self {
        Self { timers: Vec::new(), close_requested: None, resized: None, focus: None }
    }
}

impl<M> Subscriptions<M> {
    /// No subscriptions.
    pub fn none() -> Self {
        Self::default()
    }

    /// Deliver `msg` every `period`. Timers with the same period keep their
    /// phase across re-evaluations.
    pub fn every(mut self, period: Duration, msg: M) -> Self {
        self.timers.push((period.max(Duration::from_millis(1)), msg));
        self
    }

    /// Like [`Subscriptions::every`] but only while `cond` holds.
    pub fn every_if(self, cond: bool, period: Duration, msg: M) -> Self {
        if cond {
            self.every(period, msg)
        } else {
            self
        }
    }

    /// Intercept the window's close button. The window then stays open until
    /// the app calls [`Cx::close_window`](crate::Cx::close_window).
    pub fn on_close_request(mut self, msg: M) -> Self {
        self.close_requested = Some(msg);
        self
    }

    /// Called with the new logical size whenever the window is resized.
    pub fn on_resize(mut self, f: impl Fn(Size) -> M + 'static) -> Self {
        self.resized = Some(Rc::new(f));
        self
    }

    /// Called when the window gains or loses focus.
    pub fn on_focus_change(mut self, f: impl Fn(bool) -> M + 'static) -> Self {
        self.focus = Some(Rc::new(f));
        self
    }
}
