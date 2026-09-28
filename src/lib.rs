//! # Charis
//!
//! A desktop UI framework for Rust that looks like a modern web app, with no
//! browser inside.
//!
//! * **Styling like CSS.** Flexbox and grid layout, per-corner radii, blurred
//!   shadows, gradients, transitions, and the Inter font bundled.
//! * **Themes from a few knobs.** An accent, a gray tint, a radius and a
//!   density generate every color and size in light, dark and high contrast.
//!   Every widget is an [`Element`] you can restyle.
//! * **IDE-style panels.** [`hsplit`]/[`vsplit`] panes, docking with tabs that
//!   tear out into their own windows, tool windows and named layouts.
//! * **Menus and commands.** Menu bars, context menus, a command palette and
//!   a rebindable keymap.
//! * **Native behavior.** Frameless windows with custom title bars, screen
//!   readers through AccessKit, IME, and a GPU renderer with a CPU fallback.
//! * **Testable.** The [`headless`] runtime drives apps without a window:
//!   click, type and drag, then check the layout, pixels or accessibility tree.
//!
//! The Charis Book (`book/` in the repository) is the guide.
//!
//! ```no_run
//! use charis_ui::prelude::*;
//!
//! struct Counter { n: i32 }
//!
//! #[derive(Clone)]
//! enum Msg { Inc, Dec }
//!
//! impl App for Counter {
//!     type Msg = Msg;
//!     fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
//!         match msg { Msg::Inc => self.n += 1, Msg::Dec => self.n -= 1 }
//!     }
//!     fn view(&self) -> Element<Msg> {
//!         col().size_full().center().gap(12.0)
//!             .child(text(format!("{}", self.n)).font_size(48.0).bold())
//!             .child(row().gap(8.0)
//!                 .child(button("−").on_click(Msg::Dec))
//!                 .child(primary_button("+").on_click(Msg::Inc)))
//!     }
//! }
//!
//! # #[cfg(feature = "window")]
//! fn main() {
//!     charis_ui::run(Counter { n: 0 }, WindowOptions::new("Counter")).unwrap();
//! }
//! # #[cfg(not(feature = "window"))]
//! # fn main() {}
//! ```

#![warn(missing_docs)]

pub mod anim;
pub mod color;
pub mod commands;
pub mod component;
#[doc(hidden)]
pub mod cpu;
pub mod damage;
pub mod dialog;
pub mod dock;
pub(crate) mod edit;
pub mod effects;
pub mod element;
mod fxhash;
pub mod geometry;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod headless;
pub mod icons;
pub mod image;
pub mod layouts;
#[cfg(feature = "markdown")]
pub mod markdown;
pub mod menu;
#[cfg(all(feature = "native-menu", any(target_os = "macos", windows)))]
mod native_menu;
pub mod paint;
#[cfg(feature = "window")]
mod platform;
pub mod runtime;
pub mod scene;
pub mod semantics;
pub mod style;
pub mod stylesheet;
pub mod subscription;
pub mod system;
pub mod table;
pub mod text;
mod text_doc;
pub mod theme;
pub mod toolwin;
pub mod tree;
pub mod widgets;
#[cfg(feature = "window")]
pub mod window;

pub use anim::Easing;
pub use color::{hex, oklch, rgb, rgba, Color, Fill};
pub use effects::{Proxy, TaskHandle};
pub use element::*;
pub use geometry::{Axis, Point, Rect, Size};
pub use icons::Icon;
#[cfg(feature = "markdown")]
pub use markdown::markdown;
pub use paint::Canvas;
pub use runtime::{
    window_info, App, ChromeHit, ChromeMap, ClipboardContent, Cx, Event, MouseButton, Runtime, WindowInfo,
    WindowRequest,
};
pub use semantics::{Role, Semantics};
pub use style::*;
pub use subscription::Subscriptions;
pub use table::{cell_text, table, Column, ColumnWidth, SortDir, Table};
pub use text::{span, Span};
pub use theme::{
    theme, Accent, ClassFn, Contrast, Density, GrayTint, Palette, Scale, Scales, StyleClasses, Theme, ThemeConfig,
};
pub use widgets::*;
#[cfg(feature = "window")]
pub use window::{run, Backdrop, WindowOptions};

/// Everything needed to build an app.
pub mod prelude {
    pub use crate::anim::Easing;
    pub use crate::color::{hex, oklch, rgb, rgba, Color, Fill};
    pub use crate::commands::{Command, Commands, KeyBinding, Keymap};
    pub use crate::component::{component, stateful, Component};
    pub use crate::dialog::FileDialog;
    pub use crate::dock::{Dock, DockMsg, DockNode, DropZone};
    pub use crate::effects::{Proxy, TaskHandle};
    pub use crate::element::{
        canvas, col, div, hsplit, icon, lazy, rich_text, row, spacer, split, text, virtual_list, vsplit, DragEvent,
        DragPhase, DropEvent, DropPhase, Element, Key, KeyEvent, Modifiers, Pane, Priority, ScrollInfo, WindowControl,
    };
    pub use crate::geometry::{Axis, Point, Rect, Size};
    pub use crate::headless::Headless;
    pub use crate::icons::Icon;
    #[cfg(feature = "markdown")]
    pub use crate::markdown::markdown;
    pub use crate::menu::Shortcut;
    pub use crate::paint::Canvas;
    pub use crate::runtime::{window_info, App, Cx, WindowSpec};
    pub use crate::semantics::Role;
    pub use crate::style::{
        pct, Align, Corners, Cursor, Direction, Edges, FontFamily, Justify, Length, Overflow, Shadow, StylePatch,
        TextAlign, Track, Weight,
    };
    pub use crate::subscription::Subscriptions;
    pub use crate::system::{system_prefs, SystemPrefs};
    pub use crate::table::{cell_text, table, Column, ColumnWidth, SortDir};
    pub use crate::text::{span, Span};
    pub use crate::theme::Contrast;
    pub use crate::theme::{theme, Accent, Density, GrayTint, Palette, Theme, ThemeConfig};
    pub use crate::widgets::*;
    #[cfg(feature = "window")]
    pub use crate::window::{run, Backdrop, WindowOptions};
}

/// The book's chapters, compiled as doc tests so their examples stay correct.
#[cfg(doctest)]
mod book {
    #[doc = include_str!("../book/src/getting-started.md")]
    struct GettingStarted;
    #[doc = include_str!("../book/src/app-model.md")]
    struct AppModel;
    #[doc = include_str!("../book/src/layout.md")]
    struct Layout;
    #[doc = include_str!("../book/src/styling.md")]
    struct Styling;
    #[doc = include_str!("../book/src/theming.md")]
    struct Theming;
    #[doc = include_str!("../book/src/widgets.md")]
    struct Widgets;
    #[doc = include_str!("../book/src/panels.md")]
    struct Panels;
    #[doc = include_str!("../book/src/commands.md")]
    struct Commands;
    #[doc = include_str!("../book/src/windows.md")]
    struct Windows;
    #[doc = include_str!("../book/src/accessibility.md")]
    struct Accessibility;
    #[doc = include_str!("../book/src/testing.md")]
    struct Testing;
    #[doc = include_str!("../book/src/performance.md")]
    struct Performance;
}
