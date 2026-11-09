//! Win32 child windows that host an mpv video surface.
//!
//! One [`ChildWindow`] per video plane (A / B / difference). The window class is
//! registered once per process, paints solid black, never erases its background
//! (resizing does not flicker) and refuses mouse activation so clicking the
//! video keeps keyboard focus in the eframe window.

use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard};

use anyhow::bail;
use windows_sys::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    BLACK_BRUSH, CreateRectRgn, DeleteObject, FillRect, GetDC, GetStockObject, ReleaseDC,
    SetWindowRgn, ValidateRect,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, EnumWindows,
    GetClientRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, HMENU, HWND_TOP,
    IDC_ARROW, IsWindow, LoadCursorW, MA_NOACTIVATE, RegisterClassW, SW_HIDE, SW_SHOW,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos, ShowWindow, WM_ERASEBKGND,
    WM_MOUSEACTIVATE, WM_PAINT, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
};
use windows_sys::core::BOOL;

/// Window class name shared by every native video surface.
const CLASS_NAME: &str = "MuxVideoSurface";

/// Finds the toplevel window with an exact title that belongs to this process.
///
/// Enumerating only this process' windows keeps the lookup from matching a
/// foreign application that happens to use the same title.
pub fn find_window_by_title(title: &str) -> Option<i64> {
    let mut search = Search {
        process_id: current_process_id(),
        title,
        found: None,
    };

    let context = std::ptr::from_mut(&mut search).cast::<c_void>() as LPARAM;
    // SAFETY: EnumWindows calls the callback synchronously and `search` outlives
    // the call; the callback treats the pointer as a `Search<'_>`.
    unsafe {
        EnumWindows(Some(enum_window), context);
    }
    search.found
}

pub fn current_process_id() -> u32 {
    std::process::id()
}

struct Search<'a> {
    process_id: u32,
    title: &'a str,
    found: Option<i64>,
}

/// `EnumWindows` callback: matches an exact title within the calling process.
///
/// # Safety
///
/// `lparam` must be a pointer to a live [`Search`] provided by
/// [`find_window_by_title`], and `hwnd` must be a valid window handle supplied by
/// `EnumWindows`.
unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if lparam == 0 {
        return 1;
    }
    // SAFETY: see the function contract.
    let search = unsafe { &mut *(lparam as *mut Search<'_>) };

    let mut process_id = 0u32;
    // SAFETY: `hwnd` is a valid handle for the duration of the callback.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut process_id);
    }
    if process_id != search.process_id {
        return 1;
    }

    // SAFETY: standard window title query; the buffer is sized from the length
    // (plus one for the NUL terminator).
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return 1;
    }
    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if copied <= 0 {
        return 1;
    }

    let text = String::from_utf16_lossy(&buffer[..copied as usize]);
    if text == search.title {
        search.found = Some(hwnd as isize as i64);
        // Stop enumerating: the first exact match wins.
        return 0;
    }
    1
}

/// Registration state of [`CLASS_NAME`]: `None` until the class exists.
struct ClassModule(Mutex<Option<HINSTANCE>>);

// SAFETY: the stored value is an opaque Win32 module handle. Windows module
// handles are process-wide values that this code never frees, and read access
// from several threads is allowed.
unsafe impl Send for ClassModule {}
unsafe impl Sync for ClassModule {}

static CLASS_MODULE: ClassModule = ClassModule(Mutex::new(None));

/// Registers the surface window class exactly once per process.
fn ensure_class() -> anyhow::Result<()> {
    let mut registered = lock(&CLASS_MODULE.0);
    if registered.is_some() {
        return Ok(());
    }

    // SAFETY: passing NULL asks for the handle of the current executable; the
    // returned handle stays valid for the process lifetime.
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        // SAFETY: no arguments, always safe to call.
        let error = unsafe { GetLastError() };
        bail!("GetModuleHandleW(NULL) failed (Win32 error {error})");
    }

    let class_name = wide(CLASS_NAME);
    let window_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        hInstance: instance,
        // SAFETY: stock resources are process-wide and not owned by us.
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        // SAFETY: BLACK_BRUSH is a stock object; the cast is a type alias
        // conversion (HGDIOBJ and HBRUSH are both `*mut c_void`).
        hbrBackground: unsafe { GetStockObject(BLACK_BRUSH) },
        lpszClassName: class_name.as_ptr(),
        ..Default::default()
    };

    // SAFETY: `window_class` is fully initialised and `class_name` is a live
    // NUL-terminated UTF-16 string for the duration of the call.
    let atom = unsafe { RegisterClassW(&window_class) };
    if atom == 0 {
        // SAFETY: no arguments, always safe to call.
        let error = unsafe { GetLastError() };
        if error != ERROR_CLASS_ALREADY_EXISTS {
            bail!("RegisterClassW({CLASS_NAME}) failed (Win32 error {error})");
        }
    }

    *registered = Some(instance);
    Ok(())
}

/// A native child window hosting one video plane.
pub struct ChildWindow {
    hwnd: HWND,
}

impl ChildWindow {
    /// Creates a black, focus-preserving child surface under `parent`.
    ///
    /// `id` is the Win32 child identifier; `name` becomes the window title (used
    /// for debugging and by the probe's window enumeration).
    pub fn create(parent: i64, id: u32, name: &str) -> anyhow::Result<Self> {
        ensure_class()?;

        // SAFETY: current-executable handle, alive for the process lifetime.
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        if instance.is_null() {
            bail!("GetModuleHandleW(NULL) failed");
        }

        let parent = parent as usize as HWND;
        if parent.is_null() {
            bail!("cannot create {CLASS_NAME}: parent handle is NULL");
        }

        let class_name = wide(CLASS_NAME);
        let title = wide(name);

        // SAFETY: both UTF-16 strings are live for the call. The class is
        // registered and the parent handle belongs to the caller.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                0,
                0,
                16,
                16,
                parent,
                id as usize as HMENU,
                instance,
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            // SAFETY: no arguments, always safe to call.
            let error = unsafe { GetLastError() };
            bail!("CreateWindowExW({CLASS_NAME}) failed (Win32 error {error})");
        }

        Ok(Self { hwnd })
    }

    /// Native window handle as `i64` (the value expected by libmpv's `wid`).
    pub fn hwnd(&self) -> i64 {
        self.hwnd as isize as i64
    }

    /// Moves and resizes the surface within its parent's client area.
    pub fn set_rect(&self, x: i32, y: i32, w: i32, h: i32) {
        // SAFETY: `self.hwnd` is valid while `self` exists; the window is not
        // activated by the call.
        unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                x,
                y,
                w.max(0),
                h.max(0),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn show(&self, visible: bool) {
        // SAFETY: `self.hwnd` is valid while `self` exists.
        let _ = unsafe { ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE }) };
    }

    /// Clips the surface to a rectangle in its own client-area coordinates.
    ///
    /// Used by the wipe mode: plane B keeps its full window size and only the
    /// right/bottom part is visible. Repeated calls are idempotent; each
    /// successful `SetWindowRgn` hands the region to the system.
    pub fn set_clip_rect(&self, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            self.clear_clip();
            return;
        }

        let right = x.saturating_add(w);
        let bottom = y.saturating_add(h);
        // SAFETY: CreateRectRgn allocates a GDI region and needs no window.
        let region = unsafe { CreateRectRgn(x, y, right, bottom) };
        if region.is_null() {
            return;
        }

        // SAFETY: `region` is a valid GDI region. On success the system owns it
        // and deletes it when replaced/destroyed; on failure we must delete it.
        let result = unsafe { SetWindowRgn(self.hwnd, region, 1) };
        if result == 0 {
            // SAFETY: SetWindowRgn failed, so the region is still ours.
            unsafe {
                DeleteObject(region);
            }
        }
    }

    /// Legacy helper retained for the stub API: clips to the left `width`
    /// columns of the full client area.
    pub fn clip_to_left(&self, width: i32) {
        let mut rect = RECT::default();
        // SAFETY: client rect query on a live HWND; `RECT` is a plain struct.
        let height = unsafe {
            if GetClientRect(self.hwnd, &mut rect) != 0 {
                rect.bottom - rect.top
            } else {
                0
            }
        };
        self.set_clip_rect(0, 0, width, height);
    }

    /// Removes any clip region, showing the full surface again.
    pub fn clear_clip(&self) {
        // SAFETY: passing a NULL region removes the current region; the system
        // keeps ownership of the old one.
        unsafe {
            SetWindowRgn(self.hwnd, std::ptr::null_mut(), 1);
        }
    }

    /// Raises the surface above its sibling windows without activating it.
    pub fn bring_to_top(&self) {
        // SAFETY: `HWND_TOP` re-orders siblings only; the window is live.
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

impl Drop for ChildWindow {
    fn drop(&mut self) {
        if self.hwnd.is_null() {
            return;
        }
        // SAFETY: the handle was created by `ChildWindow::create` and is
        // destroyed exactly once here.
        unsafe {
            if IsWindow(self.hwnd) != 0 {
                DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = std::ptr::null_mut();
    }
}

/// Window procedure for every video surface.
///
/// Only three messages are handled: background erasure is suppressed, paints
/// fill black, and mouse clicks never activate the window.
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            // SAFETY: standard paint sequence; every acquired DC is released and
            // the update region is validated before returning.
            unsafe {
                let mut rect = RECT::default();
                if GetClientRect(hwnd, &mut rect) != 0 {
                    let dc = GetDC(hwnd);
                    if !dc.is_null() {
                        FillRect(dc, &rect, GetStockObject(BLACK_BRUSH));
                        ReleaseDC(hwnd, dc);
                    }
                }
                ValidateRect(hwnd, std::ptr::null());
            }
            0
        }
        // Clicking the video must not take keyboard focus away from eframe.
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        // SAFETY: default processing for everything else.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_encoding_is_nul_terminated() {
        assert_eq!(wide(""), vec![0]);
        assert_eq!(wide("abc"), vec![0x61, 0x62, 0x63, 0]);
    }
}
