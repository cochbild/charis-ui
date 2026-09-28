//! OS accessibility and appearance preferences.
//!
//! [`system_prefs`] returns the user's high-contrast, reduced-motion and
//! dark-mode settings. It is cheap (a cached value) so it can be called from
//! `App::theme` every frame; windows refresh it when they regain focus, which
//! is when a changed OS setting takes effect.
//!
//! The runtime applies reduced motion by itself. High contrast is the app's
//! choice, typically:
//!
//! ```
//! # use charis_ui::prelude::*;
//! fn theme() -> Theme {
//!     let prefs = system_prefs();
//!     Theme::from_config(ThemeConfig {
//!         contrast: if prefs.high_contrast { Contrast::High } else { Contrast::Normal },
//!         ..ThemeConfig::dark()
//!     })
//! }
//! ```
//!
//! Set `CHARIS_HIGH_CONTRAST`, `CHARIS_REDUCED_MOTION` or `CHARIS_DARK` to `1` or `0`
//! to override detection (for testing).

use std::sync::atomic::{AtomicU8, Ordering};

/// The user's OS-level preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SystemPrefs {
    /// Windows contrast themes, macOS "Increase contrast", GNOME "High
    /// contrast".
    pub high_contrast: bool,
    /// Windows "Animation effects" off, macOS "Reduce motion", GNOME
    /// "Animations" off.
    pub reduced_motion: bool,
    /// The OS color scheme, when known.
    pub dark: Option<bool>,
}

// Bits: 1 = detected, 2 = high contrast, 4 = reduced motion, 8 = dark known,
// 16 = dark.
static PREFS: AtomicU8 = AtomicU8::new(0);

fn encode(p: SystemPrefs) -> u8 {
    let bit = |on: bool, b: u8| if on { b } else { 0 };
    1 | bit(p.high_contrast, 2) | bit(p.reduced_motion, 4) | bit(p.dark.is_some(), 8) | bit(p.dark == Some(true), 16)
}

fn decode(b: u8) -> SystemPrefs {
    SystemPrefs { high_contrast: b & 2 != 0, reduced_motion: b & 4 != 0, dark: (b & 8 != 0).then_some(b & 16 != 0) }
}

/// The user's OS preferences (detected on first use, then cached).
pub fn system_prefs() -> SystemPrefs {
    let b = PREFS.load(Ordering::Relaxed);
    if b & 1 != 0 {
        return decode(b);
    }
    refresh_system_prefs()
}

/// Re-read the OS preferences (the window shell calls this when a window
/// regains focus). Returns the new value.
pub fn refresh_system_prefs() -> SystemPrefs {
    let mut p = detect();
    let flag = |name: &str| std::env::var(name).ok().map(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    if let Some(v) = flag("CHARIS_HIGH_CONTRAST") {
        p.high_contrast = v;
    }
    if let Some(v) = flag("CHARIS_REDUCED_MOTION") {
        p.reduced_motion = v;
    }
    if let Some(v) = flag("CHARIS_DARK") {
        p.dark = Some(v);
    }
    PREFS.store(encode(p), Ordering::Relaxed);
    p
}

/// Override the cached preferences (for tests and for apps with their own
/// accessibility settings).
pub fn set_system_prefs(p: SystemPrefs) {
    PREFS.store(encode(p), Ordering::Relaxed);
}

#[cfg(windows)]
fn detect() -> SystemPrefs {
    use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST,
    };
    let mut hc = HIGHCONTRASTW { cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32, ..Default::default() };
    let mut anim: i32 = 1;
    // SAFETY: both calls write into correctly sized, owned out-parameters.
    let (hc_ok, anim_ok) = unsafe {
        (
            SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, &mut hc as *mut _ as *mut _, 0) != 0,
            SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &mut anim as *mut _ as *mut _, 0) != 0,
        )
    };
    SystemPrefs {
        high_contrast: hc_ok && hc.dwFlags & HCF_HIGHCONTRASTON != 0,
        reduced_motion: anim_ok && anim == 0,
        dark: None,
    }
}

#[cfg(target_os = "macos")]
fn detect() -> SystemPrefs {
    // `defaults` avoids an Objective-C dependency; this runs rarely.
    let read = |domain: &str, key: &str| {
        std::process::Command::new("defaults")
            .args(["read", domain, key])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    SystemPrefs {
        high_contrast: read("com.apple.universalaccess", "increaseContrast").as_deref() == Some("1"),
        reduced_motion: read("com.apple.universalaccess", "reduceMotion").as_deref() == Some("1"),
        dark: Some(read("-g", "AppleInterfaceStyle").as_deref() == Some("Dark")),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn detect() -> SystemPrefs {
    // GNOME and most GTK-based desktops; other desktops read as defaults.
    let get = |schema: &str, key: &str| {
        std::process::Command::new("gsettings")
            .args(["get", schema, key])
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let scheme = get("org.gnome.desktop.interface", "color-scheme");
    SystemPrefs {
        high_contrast: get("org.gnome.desktop.a11y.interface", "high-contrast").as_deref() == Some("true"),
        reduced_motion: get("org.gnome.desktop.interface", "enable-animations").as_deref() == Some("false"),
        dark: scheme.map(|s| s.contains("dark")),
    }
}

#[cfg(not(any(windows, unix)))]
fn detect() -> SystemPrefs {
    SystemPrefs::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for hc in [false, true] {
            for rm in [false, true] {
                for dark in [None, Some(false), Some(true)] {
                    let p = SystemPrefs { high_contrast: hc, reduced_motion: rm, dark };
                    assert_eq!(decode(encode(p)), p);
                }
            }
        }
    }
}
