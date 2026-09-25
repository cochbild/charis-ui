//! # rust-ui
//!
//! A web-inspired, highly customizable UI framework for Rust desktop apps.
//!
//! * **Looks like the modern web.** CSS-like styling: flexbox & grid layout,
//!   per-corner radii, layered blurred box shadows, gradients, opacity,
//!   outlines/focus rings, and the Inter font bundled by default.
//! * **Customizability is king.** Everything is an [`Element`] you can restyle
//!   with chainable builder methods; built-in widgets read design tokens from a
//!   [`Theme`] you can swap or tweak at runtime.
//! * **IDE-grade layout.** [`hsplit`]/[`vsplit`] panes with draggable
//!   splitters, min/max sizes, animated collapse, nested in any direction.
//! * **Electron-style chrome.** Frameless windows with custom title bars,
//!   window controls, menu bars, context menus, tooltips and modals.
//! * **Smooth.** CSS-style transitions for hover/press/focus, smooth
//!   scrolling with overlay scrollbars.
//! * **Testable.** A [`headless`] renderer drives apps without a window and
//!   saves PNG screenshots.
//!
//! ```no_run
//! use rust_ui::prelude::*;
//!
//! struct Counter { n: i32 }
//!
//! #[derive(Clone)]
//! enum Msg { Inc, Dec }
//!
//! impl App for Counter {
//!     type Msg = Msg;
//!     fn update(&mut self, msg: Msg, _cx: &mut Cx) {
//!         match msg { Msg::Inc => self.n += 1, Msg::Dec => self.n -= 1 }
//!     }
//!     fn view(&self) -> Element<Msg> {
//!         col().center().gap(12.0)
//!             .child(text(format!("{}", self.n)).font_size(48.0).bold())
//!             .child(row().gap(8.0)
//!                 .child(button("−").on_click(Msg::Dec))
//!                 .child(primary_button("+").on_click(Msg::Inc)))
//!     }
//! }
//!
//! fn main() {
//!     rust_ui::run(Counter { n: 0 }, WindowOptions::new("Counter")).unwrap();
//! }
//! ```

pub mod anim;
pub mod color;
pub mod dock;
pub mod edit;
pub mod element;
pub mod geometry;
pub mod headless;
pub mod icons;
pub mod paint;
pub mod runtime;
pub mod style;
pub mod text;
pub mod theme;
pub mod widgets;
#[cfg(feature = "window")]
pub mod window;

pub use color::{hex, rgb, rgba, Color, Fill};
pub use element::*;
pub use geometry::{Axis, Point, Rect, Size};
pub use icons::Icon;
pub use paint::Canvas;
pub use runtime::{window_info, App, Cx, Event, MouseButton, Runtime, WindowInfo, WindowRequest};
pub use style::*;
pub use theme::{theme, Palette, Theme};
pub use widgets::*;
#[cfg(feature = "window")]
pub use window::{run, WindowOptions};

/// Everything needed to build an app.
pub mod prelude {
    pub use crate::color::{hex, rgb, rgba, Color, Fill};
    pub use crate::dock::{Dock, DockMsg, DockNode, DropZone};
    pub use crate::element::{
        canvas, col, div, hsplit, icon, row, spacer, split, text, vsplit, DragEvent, DragPhase, DropEvent, DropPhase,
        Element, Key, KeyEvent, Modifiers, Pane, WindowControl,
    };
    pub use crate::geometry::{Axis, Point, Rect, Size};
    pub use crate::headless::Headless;
    pub use crate::icons::Icon;
    pub use crate::paint::Canvas;
    pub use crate::runtime::{window_info, App, Cx};
    pub use crate::style::{
        pct, Align, Corners, Cursor, Direction, Edges, FontFamily, Justify, Length, Overflow, Shadow, StylePatch,
        TextAlign, Track, Weight,
    };
    pub use crate::theme::{theme, Theme};
    pub use crate::widgets::*;
    #[cfg(feature = "window")]
    pub use crate::window::{run, WindowOptions};
}
