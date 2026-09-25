//! Platform-specific window integration.

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

/// Tell the OS whether the app uses a dark theme (system menus, borders).
#[allow(unused_variables)]
pub(crate) fn set_dark_mode(window: &winit::window::Window, dark: bool) {
    #[cfg(windows)]
    windows::set_dark_mode(window, dark);
}
