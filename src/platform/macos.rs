//! macOS window integration: the traffic lights of frameless windows.
//!
//! A frameless window on macOS keeps its native title bar, made transparent
//! and overlaid on the content (`fullSizeContentView`), so the window keeps
//! the traffic lights, native full screen, and edge resizing. AppKit places
//! the buttons for its own title bar height; we move them to where the
//! app's title bar wants them (and again after resizes and full screen,
//! which reset them).

use objc2_app_kit::{NSView, NSWindowButton};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Horizontal distance between two traffic lights (AppKit's layout).
pub(crate) const BUTTON_SPACING: f32 = 20.0;
/// Width of one traffic light.
pub(crate) const BUTTON_SIZE: f32 = 14.0;

/// Move the traffic lights so the close button's top-left is at (x, y),
/// logical px from the window's top-left.
pub(crate) fn position_traffic_lights(window: &winit::window::Window, x: f32, y: f32) {
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return };
    // SAFETY: winit's NSView pointer is valid while the window lives, and
    // we're on the main thread (the event loop's).
    let view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
    let Some(win) = view.window() else { return };
    let (Some(close), Some(mini), Some(zoom)) = (
        win.standardWindowButton(NSWindowButton::CloseButton),
        win.standardWindowButton(NSWindowButton::MiniaturizeButton),
        win.standardWindowButton(NSWindowButton::ZoomButton),
    ) else {
        return;
    };
    // The buttons live in the title bar container view: make it tall enough
    // for the requested offset (AppKit's y axis points up).
    let Some(container) = unsafe { close.superview() }.and_then(|v| unsafe { v.superview() }) else { return };
    let bar_h = close.frame().size.height + y as f64;
    let mut bar = container.frame();
    bar.size.height = bar_h;
    bar.origin.y = win.frame().size.height - bar_h;
    container.setFrame(bar);
    let spacing = mini.frame().origin.x - close.frame().origin.x;
    for (i, b) in [close, mini, zoom].iter().enumerate() {
        let mut origin = b.frame().origin;
        origin.x = x as f64 + i as f64 * spacing;
        b.setFrameOrigin(origin);
    }
}

/// Width to keep clear at the left of the title bar for traffic lights
/// whose close button starts at `x`.
pub(crate) fn buttons_inset(x: f32) -> f32 {
    x + 2.0 * BUTTON_SPACING + BUTTON_SIZE + x.max(8.0)
}
