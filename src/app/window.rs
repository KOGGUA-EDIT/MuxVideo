//! Windows 11 frame polish for the undecorated window.
//!
//! An undecorated eframe window still gets the DWM shadow, but without the
//! rounded corners modern Windows apps have. `DwmSetWindowAttribute` fixes
//! that; every call is best effort because the attribute set depends on the
//! Windows version.

/// `DWMWA_WINDOW_CORNER_PREFERENCE` from `dwmapi.h`.
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
/// `DWMWCP_ROUND`.
const DWMWCP_ROUND: i32 = 2;
/// `DWMWCP_DONOTROUND`.
const DWMWCP_DONOTROUND: i32 = 1;
/// `DWMWA_BORDER_COLOR`.
const DWMWA_BORDER_COLOR: u32 = 34;
/// `DWMWA_COLOR_NONE`: tell DWM not to draw the border at all.
const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;

/// Rounds the corners of `hwnd` and gives it a hairline border.
///
/// In immersive playback the picture has to reach the very edge of the screen:
/// the 1px DWM border and the rounded corners would show up as a light groove
/// along the bottom edge, so they are switched off instead.
pub fn apply_modern_frame(hwnd: i64, immersive: bool) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    if hwnd == 0 {
        return;
    }
    let hwnd = hwnd as HWND;
    let preference: i32 = if immersive {
        DWMWCP_DONOTROUND
    } else {
        DWMWCP_ROUND
    };
    // SAFETY: the HWND comes from our own window; the attribute and the
    // pointed-to value match what dwmapi expects for corner preference.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }

    // Dark hairline normally; no border at all while immersive.
    let border: u32 = if immersive {
        DWMWA_COLOR_NONE
    } else {
        0x0022_1F26 // COLORREF is 0x00BBGGRR
    };
    // SAFETY: as above; DWMWA_BORDER_COLOR takes a COLORREF.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

/// Blow the window up to the monitor it currently sits on.
///
/// Immersive playback deliberately avoids `ViewportCommand::Fullscreen`: the
/// OS fullscreen state machine re-applies the DWM border and rounded corners
/// whenever it is entered on top of a maximized window, which is what showed up
/// as a light groove and white dots along the bottom edge. Covering the monitor
/// with plain window geometry is deterministic and leaves the DWM attributes
/// we set in [`apply_modern_frame`] alone.
pub fn cover_monitor(hwnd: i64) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_TOP, SW_RESTORE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SetWindowPos, ShowWindow,
    };

    if hwnd == 0 {
        return;
    }
    let hwnd = hwnd as HWND;
    // SAFETY: `hwnd` is our own top-level window; the monitor helpers take it
    // by value and only fill the structure we hand them.
    unsafe {
        // A maximized window keeps its restore rect; leave that state first so
        // the geometry below is what the user actually sees.
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            let r = info.rcMonitor;
            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

/// Put the window back at an exact physical-pixel rectangle.
pub fn restore_rect(hwnd: i64, x: i32, y: i32, w: i32, h: i32) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_TOP, SWP_FRAMECHANGED, SWP_NOACTIVATE, SetWindowPos,
    };

    if hwnd == 0 {
        return;
    }
    // SAFETY: our own window; the rectangle comes from a viewport report.
    unsafe {
        let _ = SetWindowPos(
            hwnd as HWND,
            HWND_TOP,
            x,
            y,
            w.max(320),
            h.max(200),
            SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}
