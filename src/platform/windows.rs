//! Native frameless windows on Windows 10/11.
//!
//! The window keeps its standard `WS_OVERLAPPEDWINDOW` styles, so Windows
//! still provides everything users expect from a real window:
//! - Aero Snap and Snap Layouts: hovering the app's own maximize button shows
//!   the Windows 11 layout flyout.
//! - Window-shake minimize, the system menu (Alt+Space or right-clicking the
//!   title), and double-click to maximize.
//! - Native edge resizing, the DWM drop shadow, and Windows 11 rounded corners.
//!
//! We only remove the visible non-client frame (`WM_NCCALCSIZE`) and answer
//! `WM_NCHITTEST` from the app's [`ChromeMap`], so the app's own title bar
//! behaves like a native one.

use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND,
};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::Controls::{MARGINS, WM_MOUSELEAVE};
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, IsZoomed, PostMessageW, SetWindowPos, ShowWindow, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION,
    HTCLIENT, HTLEFT, HTMAXBUTTON, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, NCCALCSIZE_PARAMS, SM_CXFRAME,
    SM_CXPADDEDBORDER, SM_CYFRAME, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_MAXIMIZE, SW_RESTORE,
    WM_MOUSEMOVE, WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP,
    WM_NCMOUSELEAVE, WM_NCMOUSEMOVE,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

use super::SharedChrome;
use crate::geometry::Point;
use crate::runtime::ChromeHit;

const SUBCLASS_ID: usize = 0x7275_6975; // "ruiu"

struct State {
    chrome: SharedChrome,
    max_pressed: AtomicBool,
    tracking: AtomicBool,
}

fn hwnd_of(window: &winit::window::Window) -> Option<HWND> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as HWND),
        _ => None,
    }
}

pub(crate) fn install(window: &winit::window::Window, chrome: SharedChrome) -> bool {
    let Some(hwnd) = hwnd_of(window) else { return false };
    let state = Box::into_raw(Box::new(State {
        chrome,
        max_pressed: AtomicBool::new(false),
        tracking: AtomicBool::new(false),
    }));
    // SAFETY: `hwnd` is a live window owned by winit on this thread. `state` is
    // freed in WM_NCDESTROY, after which the subclass is removed.
    unsafe {
        if SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, state as usize) == 0 {
            drop(Box::from_raw(state));
            return false;
        }
        // A 1px extended frame keeps the DWM drop shadow on a frameless window.
        let margins = MARGINS { cxLeftWidth: 1, cxRightWidth: 1, cyTopHeight: 1, cyBottomHeight: 1 };
        DwmExtendFrameIntoClientArea(hwnd, &margins);
        let pref = DWMWCP_ROUND;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &pref as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&pref) as u32,
        );
        // Recompute the frame so WM_NCCALCSIZE takes effect immediately.
        SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER);
    }
    true
}

/// Extend the DWM frame over the whole client area, so a system backdrop
/// (Mica, Acrylic) shows through transparent pixels.
pub(crate) fn extend_frame_into_client(window: &winit::window::Window) {
    let Some(hwnd) = hwnd_of(window) else { return };
    let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
    // SAFETY: valid HWND owned by winit on this thread.
    unsafe {
        DwmExtendFrameIntoClientArea(hwnd, &margins);
    }
}

pub(crate) fn set_dark_mode(window: &winit::window::Window, dark: bool) {
    let Some(hwnd) = hwnd_of(window) else { return };
    let v: i32 = dark as i32;
    // SAFETY: valid HWND; the attribute takes a BOOL-sized value.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &v as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&v) as u32,
        );
    }
}

fn lparam_point(lparam: LPARAM) -> POINT {
    POINT { x: (lparam & 0xFFFF) as i16 as i32, y: ((lparam >> 16) & 0xFFFF) as i16 as i32 }
}

fn client_lparam(p: POINT) -> LPARAM {
    ((p.y as u16 as u32) << 16 | (p.x as u16 as u32)) as LPARAM
}

/// Frame thickness (resize border) in physical pixels for the window's DPI.
unsafe fn frame_thickness(hwnd: HWND) -> (i32, i32) {
    let dpi = GetDpiForWindow(hwnd);
    let pad = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
    (GetSystemMetricsForDpi(SM_CXFRAME, dpi) + pad, GetSystemMetricsForDpi(SM_CYFRAME, dpi) + pad)
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    let state = &*(data as *const State);
    match msg {
        WM_NCCALCSIZE if wparam != 0 => {
            // Remove the visible frame. When maximized, Windows positions the
            // window partly off-screen by the frame thickness: inset to compensate.
            if IsZoomed(hwnd) != 0 {
                let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
                let (fx, fy) = frame_thickness(hwnd);
                let r = &mut params.rgrc[0];
                r.left += fx;
                r.right -= fx;
                r.top += fy;
                r.bottom -= fy;
            }
            0
        }
        WM_NCHITTEST => {
            let mut p = lparam_point(lparam);
            ScreenToClient(hwnd, &mut p);
            let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect(hwnd, &mut rc);
            if IsZoomed(hwnd) == 0 {
                let (fx, fy) = frame_thickness(hwnd);
                let (l, r) = (p.x < fx, p.x >= rc.right - fx);
                let (t, b) = (p.y < fy, p.y >= rc.bottom - fy);
                let edge = match (l, r, t, b) {
                    (true, _, true, _) => Some(HTTOPLEFT),
                    (_, true, true, _) => Some(HTTOPRIGHT),
                    (true, _, _, true) => Some(HTBOTTOMLEFT),
                    (_, true, _, true) => Some(HTBOTTOMRIGHT),
                    (true, _, _, _) => Some(HTLEFT),
                    (_, true, _, _) => Some(HTRIGHT),
                    (_, _, true, _) => Some(HTTOP),
                    (_, _, _, true) => Some(HTBOTTOM),
                    _ => None,
                };
                if let Some(e) = edge {
                    return e as LRESULT;
                }
            }
            let scale = GetDpiForWindow(hwnd) as f32 / 96.0;
            let lp = Point::new(p.x as f32 / scale, p.y as f32 / scale);
            let hit = state.chrome.lock().map(|m| m.hit(lp)).unwrap_or(ChromeHit::Client);
            (match hit {
                ChromeHit::Client => HTCLIENT,
                ChromeHit::Caption => HTCAPTION,
                ChromeHit::Maximize => HTMAXBUTTON,
            }) as LRESULT
        }
        // The maximize button is non-client (for Snap Layouts); forward hover to
        // the app as client mouse moves so it can draw its hover state.
        WM_NCMOUSEMOVE if wparam == HTMAXBUTTON as usize => {
            if !state.tracking.swap(true, Ordering::Relaxed) {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut tme);
            }
            let mut p = lparam_point(lparam);
            ScreenToClient(hwnd, &mut p);
            PostMessageW(hwnd, WM_MOUSEMOVE, 0, client_lparam(p));
            0
        }
        WM_NCMOUSELEAVE => {
            state.tracking.store(false, Ordering::Relaxed);
            state.max_pressed.store(false, Ordering::Relaxed);
            PostMessageW(hwnd, WM_MOUSELEAVE, 0, 0);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if wparam == HTMAXBUTTON as usize => {
            state.max_pressed.store(true, Ordering::Relaxed);
            0
        }
        WM_NCLBUTTONUP if wparam == HTMAXBUTTON as usize => {
            if state.max_pressed.swap(false, Ordering::Relaxed) {
                ShowWindow(hwnd, if IsZoomed(hwnd) != 0 { SW_RESTORE } else { SW_MAXIMIZE });
            }
            0
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
            drop(Box::from_raw(data as *mut State));
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}
