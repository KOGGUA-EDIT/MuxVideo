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
/// `DWMWA_BORDER_COLOR`.
const DWMWA_BORDER_COLOR: u32 = 34;

/// Rounds the corners of `hwnd` and gives it a hairline border.
pub fn apply_modern_frame(hwnd: i64) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    if hwnd == 0 {
        return;
    }
    let hwnd = hwnd as HWND;
    let preference: i32 = DWMWCP_ROUND;
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

    // A dark, very transparent border keeps the corner rounding crisp.
    let border: u32 = 0x0022_1F26; // COLORREF is 0x00BBGGRR
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
