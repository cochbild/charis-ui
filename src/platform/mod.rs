//! Platform-specific window integration.

#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(windows)]
pub(crate) mod windows;

use std::sync::{Arc, Mutex};

use crate::runtime::ChromeMap;

/// Shared chrome map: written by the UI thread after each frame, read by the
/// native window procedure during hit testing.
pub(crate) type SharedChrome = Arc<Mutex<ChromeMap>>;

/// Install native frameless-window behaviour. Returns true when the platform
/// handles window chrome natively (edge resizing, dragging, snapping), in which
/// case the runtime's own frameless handling is disabled.
#[allow(unused_variables)]
pub(crate) fn install_frameless(window: &winit::window::Window, chrome: SharedChrome) -> bool {
    #[cfg(windows)]
    {
        windows::install(window, chrome)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Tell the OS whether the app uses a dark theme: the title bar and
/// borders (Windows), the window appearance (macOS), and the client-side
/// decorations winit draws where the compositor has none (GNOME Wayland).
pub(crate) fn set_dark_mode(window: &winit::window::Window, dark: bool) {
    window.set_theme(Some(if dark { winit::window::Theme::Dark } else { winit::window::Theme::Light }));
    #[cfg(windows)]
    windows::set_dark_mode(window, dark);
}

/// Let a system backdrop show through the whole client area (Windows).
#[allow(unused_variables)]
pub(crate) fn extend_frame_into_client(window: &winit::window::Window) {
    #[cfg(windows)]
    windows::extend_frame_into_client(window);
}
