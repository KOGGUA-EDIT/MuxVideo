//! Win32 child windows that host an mpv video surface.
//!
//! One [`ChildWindow`] per video plane (A / B / difference). The window class is
//! registered once per process, paints solid black, never erases its background
//! (resizing does not flicker) and refuses mouse activation so clicking the
//! video keeps keyboard focus in the eframe window.
//!
//! Every surface is also subclassed so pointer input reaches the application:
//! libmpv creates its own hit-test-transparent child window inside the surface,
//! so `WM_MOUSEMOVE` / button messages are delivered to our window even though
//! mpv is painting it. The subclass chains to the class procedure, converts the
//! messages into [`MouseEvent`]s and queues them for [`take_input_events`].

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering as AtomicOrdering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use anyhow::bail;
use windows_sys::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACK_BRUSH, BeginPaint, ClientToScreen, CombineRgn,
    CreatePen, CreateRectRgn, CreateRoundRectRgn, CreateSolidBrush, DIB_RGB_COLORS, DeleteObject,
    EndPaint, FillRect, GetDC, GetStockObject, InvalidateRect, LineTo, MoveToEx, PAINTSTRUCT,
    RGN_OR, ReleaseDC, SRCCOPY, ScreenToClient, SelectObject, SetWindowRgn, StretchDIBits,
    ValidateRect,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CallWindowProcW, CreateWindowExW, DefWindowProcW,
    DestroyWindow, EnumChildWindows, EnumWindows, GWLP_WNDPROC, GetClassNameW, GetClientRect,
    GetCursorPos, GetForegroundWindow, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, HMENU, HWND_TOP, IDC_ARROW, IsWindow, LoadCursorW, MA_NOACTIVATE,
    RegisterClassW, SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_CAPTURECHANGED, WM_ERASEBKGND,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY, WM_PAINT, WM_RBUTTONDBLCLK,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WNDCLASSW, WNDPROC, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WS_VISIBLE, WindowFromPoint,
};
use windows_sys::core::BOOL;

/// Window class name shared by every native video surface.
const CLASS_NAME: &str = "MuxVideoSurface";

/// Window class name of the wipe grip handle.
const GRIP_CLASS: &str = "MuxVideoGrip";

/// Window class name of the scrub preview surface.
///
/// The preview is a plain opaque child window that blits a cached frame while
/// libmpv is still seeking, so dragging backwards shows a picture instead of a
/// frozen last frame.
const PREVIEW_CLASS: &str = "MuxVideoPreview";

/// One cached frame ready to be blitted: BGRA rows, top row first.
pub struct PreviewFrame {
    pub width: u32,
    pub height: u32,
    /// Bytes per row (at least `width * 4`).
    pub stride: usize,
    pub bgra: std::sync::Arc<Vec<u8>>,
}

/// Frame currently attached to each preview window.
fn preview_frames()
-> &'static std::sync::Mutex<std::collections::HashMap<i64, std::sync::Arc<PreviewFrame>>> {
    static FRAMES: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<i64, std::sync::Arc<PreviewFrame>>>,
    > = std::sync::OnceLock::new();
    FRAMES.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Attaches `frame` to preview window `hwnd` and repaints it.
pub fn set_preview_frame(hwnd: i64, frame: std::sync::Arc<PreviewFrame>) {
    if let Ok(mut frames) = preview_frames().lock() {
        frames.insert(hwnd, frame);
    }
    // SAFETY: invalidating a window this process owns is always allowed.
    unsafe { InvalidateRect(hwnd as HWND, std::ptr::null(), 0) };
}

/// Forgets the frame attached to `hwnd` (the window keeps the last pixels).
pub fn clear_preview_frame(hwnd: i64) {
    if let Ok(mut frames) = preview_frames().lock() {
        frames.remove(&hwnd);
    }
}

/// Paints a preview window: stretches the cached frame over the whole client
/// area, or fills black when nothing is attached.
unsafe extern "system" fn preview_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_ERASEBKGND => 1, // painted in WM_PAINT; skip the flicker
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            // SAFETY: BeginPaint requires a window being painted; `paint` is
            // stack storage owned by this frame.
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            if !dc.is_null() {
                let mut rect = RECT::default();
                // SAFETY: client rect of a live window.
                unsafe { GetClientRect(hwnd, &mut rect) };
                let width = (rect.right - rect.left).max(0);
                let height = (rect.bottom - rect.top).max(0);
                let frame = preview_frames()
                    .lock()
                    .ok()
                    .and_then(|frames| frames.get(&(hwnd as i64)).cloned());
                // SAFETY: brushes and DIBs are created and released in this
                // scope; the DC is valid until EndPaint.
                unsafe {
                    match frame {
                        Some(frame) if width > 0 && height > 0 => {
                            let info = BITMAPINFO {
                                bmiHeader: BITMAPINFOHEADER {
                                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                                    biWidth: frame.width as i32,
                                    // Negative height: rows are stored top-down.
                                    biHeight: -(frame.height as i32),
                                    biPlanes: 1,
                                    biBitCount: 32,
                                    biCompression: BI_RGB,
                                    ..BITMAPINFOHEADER::default()
                                },
                                ..BITMAPINFO::default()
                            };
                            StretchDIBits(
                                dc,
                                0,
                                0,
                                width,
                                height,
                                0,
                                0,
                                frame.width as i32,
                                frame.height as i32,
                                frame.bgra.as_ptr().cast(),
                                &info,
                                DIB_RGB_COLORS,
                                SRCCOPY,
                            );
                        }
                        _ => {
                            let black = CreateSolidBrush(0);
                            if !black.is_null() {
                                FillRect(dc, &rect, black);
                                DeleteObject(black);
                            }
                        }
                    }
                }
                // SAFETY: pairs with BeginPaint above.
                unsafe { EndPaint(hwnd, &paint) };
            }
            0
        }
        _ => {
            // SAFETY: default handling for every message we do not paint.
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
    }
}

/// Registers the preview window class exactly once per process.
fn ensure_preview_class() -> anyhow::Result<()> {
    static REGISTERED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    if REGISTERED.get().is_some() {
        return Ok(());
    }
    // SAFETY: current-executable handle, alive for the process lifetime.
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        bail!("GetModuleHandleW(NULL) failed");
    }
    let class_name = wide(PREVIEW_CLASS);
    let window_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(preview_wndproc),
        hInstance: instance,
        // SAFETY: `class_name` outlives the call.
        lpszClassName: class_name.as_ptr(),
        ..WNDCLASSW::default()
    };
    // SAFETY: `window_class` is fully initialised and its name outlives the call.
    let atom = unsafe { RegisterClassW(&window_class) };
    if atom == 0 {
        let error = unsafe { GetLastError() };
        // A class registered by an earlier call is fine.
        if error != ERROR_CLASS_ALREADY_EXISTS {
            bail!("RegisterClassW({PREVIEW_CLASS}) failed (Win32 error {error})");
        }
    }
    let _ = REGISTERED.set(());
    Ok(())
}

/// Handle colours, as `0x00BBGGRR` (Win32 `COLORREF`).
static GRIP_BG: AtomicU32 = AtomicU32::new(0x00FF_BCD0);
static GRIP_FG: AtomicU32 = AtomicU32::new(0x0022_1938);
/// `true` when the divider is vertical (handle is a tall pill).
static GRIP_VERTICAL: AtomicBool = AtomicBool::new(true);

/// Updates the grip colours and orientation.
///
/// `bg` / `fg` are `0xRRGGBB` values straight from the Material palette so the
/// handle follows the seed colour like everything else in the UI.
pub fn set_grip_style(bg: u32, fg: u32, vertical: bool) {
    let to_colorref = |rgb: u32| -> u32 {
        let r = (rgb >> 16) & 0xFF;
        let g = (rgb >> 8) & 0xFF;
        let b = rgb & 0xFF;
        (b << 16) | (g << 8) | r
    };
    GRIP_BG.store(to_colorref(bg), AtomicOrdering::Relaxed);
    GRIP_FG.store(to_colorref(fg), AtomicOrdering::Relaxed);
    GRIP_VERTICAL.store(vertical, AtomicOrdering::Relaxed);
}

/// Paints the grip: a solid rounded pill plus three short grip lines.
///
/// # Safety
/// Called by the window manager with a live `HWND` it owns.
unsafe extern "system" fn grip_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_ERASEBKGND => 1, // painted in WM_PAINT; skip the flicker
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            // SAFETY: BeginPaint requires a window being painted; `paint` is
            // stack storage owned by this frame.
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            if !dc.is_null() {
                let mut rect = RECT::default();
                // SAFETY: client rect of a live window.
                let (w, h) = unsafe {
                    if GetClientRect(hwnd, &mut rect) != 0 {
                        (rect.right, rect.bottom)
                    } else {
                        (0, 0)
                    }
                };
                if w > 0 && h > 0 {
                    // SAFETY: brushes/pens are created and deleted in this
                    // scope; the DC is valid until EndPaint.
                    unsafe {
                        let bg = CreateSolidBrush(GRIP_BG.load(AtomicOrdering::Relaxed));
                        if !bg.is_null() {
                            FillRect(dc, &rect, bg);
                            DeleteObject(bg);
                        }
                        let fg = CreatePen(0, 2, GRIP_FG.load(AtomicOrdering::Relaxed));
                        if !fg.is_null() {
                            let old = SelectObject(dc, fg);
                            let vertical = GRIP_VERTICAL.load(AtomicOrdering::Relaxed);
                            let cx = w / 2;
                            let cy = h / 2;
                            for i in -1..=1 {
                                let offset = i * 5;
                                if vertical {
                                    MoveToEx(dc, cx - 3, cy + offset, std::ptr::null_mut());
                                    LineTo(dc, cx + 3, cy + offset);
                                } else {
                                    MoveToEx(dc, cx + offset, cy - 3, std::ptr::null_mut());
                                    LineTo(dc, cx + offset, cy + 3);
                                }
                            }
                            SelectObject(dc, old);
                            DeleteObject(fg);
                        }
                    }
                }
                // SAFETY: pairs with BeginPaint above.
                unsafe { EndPaint(hwnd, &paint) };
            }
            0
        }
        _ => {
            // SAFETY: default handling for every message we do not paint.
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
    }
}

/// Registers the grip window class exactly once per process.
fn ensure_grip_class() -> anyhow::Result<()> {
    let mut registered = lock(&GRIP_MODULE.0);
    if registered.is_some() {
        return Ok(());
    }

    // SAFETY: current-executable handle, alive for the process lifetime.
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        bail!("GetModuleHandleW(NULL) failed");
    }

    let class_name = wide(GRIP_CLASS);
    let window_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(grip_wndproc),
        hInstance: instance,
        // SAFETY: stock resources are process-wide and not owned by us.
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        // SAFETY: BLACK_BRUSH is a stock object (HGDIOBJ -> HBRUSH cast).
        hbrBackground: unsafe { GetStockObject(BLACK_BRUSH) },
        lpszClassName: class_name.as_ptr(),
        ..Default::default()
    };

    // SAFETY: `window_class` is fully initialised, `class_name` is NUL
    // terminated and stays alive for the call.
    let atom = unsafe { RegisterClassW(&window_class) };
    if atom == 0 {
        // SAFETY: no arguments, always safe to call.
        let error = unsafe { GetLastError() };
        if error != ERROR_CLASS_ALREADY_EXISTS {
            bail!("RegisterClassW({GRIP_CLASS}) failed (Win32 error {error})");
        }
    }
    *registered = Some(instance);
    Ok(())
}

/// Maps a client point of `from` into the client space of `to`.
///
/// Used when the pointer is captured by the grip window but the drag has to be
/// interpreted in the coordinates of the picture plane underneath.
pub fn map_point(from: i64, to: i64, x: i32, y: i32) -> Option<(i32, i32)> {
    let from = from as usize as HWND;
    let to = to as usize as HWND;
    if from.is_null() || to.is_null() {
        return None;
    }
    let mut point = POINT { x, y };
    // SAFETY: both handles are live windows; `point` is stack storage.
    unsafe {
        if ClientToScreen(from, &mut point) == 0 {
            return None;
        }
        if ScreenToClient(to, &mut point) == 0 {
            return None;
        }
    }
    Some((point.x, point.y))
}

/// Whether `hwnd` is the foreground window.
pub fn is_foreground(hwnd: i64) -> bool {
    let hwnd = hwnd as usize as HWND;
    if hwnd.is_null() {
        return false;
    }
    // SAFETY: no pointers involved; the call only reads global state.
    let foreground = unsafe { GetForegroundWindow() };
    !foreground.is_null() && foreground == hwnd
}

/// Whether the window under a screen point belongs to this process.
///
/// Used by the pointer-polling fallback: it must not scrub when the click is
/// meant for another application that happens to sit above the picture.
/// Debug helper: `(pid, class)` of the window under a screen point.
pub fn hit_test_debug(sx: i32, sy: i32) -> (u32, String) {
    // SAFETY: read-only queries.
    let hit = unsafe { WindowFromPoint(POINT { x: sx, y: sy }) };
    if hit.is_null() {
        return (0, "<null>".to_owned());
    }
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid out pointer.
    unsafe { GetWindowThreadProcessId(hit, &mut pid) };
    let mut buf = [0u16; 128];
    // SAFETY: buffer length is passed alongside the pointer.
    let len = unsafe { GetClassNameW(hit, buf.as_mut_ptr(), buf.len() as i32) };
    let class = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
    (pid, class)
}

pub fn hit_test_owns(sx: i32, sy: i32) -> bool {
    // SAFETY: pure read-only Win32 queries.
    let hit = unsafe { WindowFromPoint(POINT { x: sx, y: sy }) };
    if hit.is_null() {
        return false;
    }
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid out pointer.
    unsafe { GetWindowThreadProcessId(hit, &mut pid) };
    pid != 0 && pid == std::process::id()
}

/// Current pointer position in screen coordinates.
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: `point` is stack storage owned by the caller.
    let ok = unsafe { GetCursorPos(&mut point) };
    if ok == 0 {
        None
    } else {
        Some((point.x, point.y))
    }
}

/// Whether the physical left mouse button is held down right now.
///
/// Polling the button state is the fallback for the cases where the native
/// hook misses a press (the renderer can recreate its window at any time).
/// Samples the physical left button.
///
/// Returns `(down, pressed_since_last_call)`. The second flag comes from the
/// low bit of `GetAsyncKeyState`, which the system sets when the key went down
/// since the previous sample - that catches fast taps that start *and* end
/// between two polls.
pub fn left_button_state() -> (bool, bool) {
    // SAFETY: read-only query, no pointers.
    let state = unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON)) } as u16;
    (state & 0x8000 != 0, state & 0x0001 != 0)
}

pub fn left_button_down() -> bool {
    // SAFETY: no pointers involved; the call only reads global key state.
    let state = unsafe { GetAsyncKeyState(VK_LBUTTON as i32) };
    (state as u16 & 0x8000) != 0
}

/// Screen rectangle `(x, y, w, h)` of a window, in screen coordinates.
pub fn window_rect(hwnd: i64) -> Option<(i32, i32, i32, i32)> {
    let hwnd = hwnd as usize as HWND;
    if hwnd.is_null() {
        return None;
    }
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `rect` is stack storage; `hwnd` is checked to be non-null.
    let ok = unsafe { GetWindowRect(hwnd, &mut rect) };
    if ok == 0 {
        return None;
    }
    Some((
        rect.left,
        rect.top,
        (rect.right - rect.left).max(0),
        (rect.bottom - rect.top).max(0),
    ))
}

/// Maps a screen point into `hwnd`'s client coordinates.
pub fn screen_to_client(hwnd: i64, x: i32, y: i32) -> Option<(i32, i32)> {
    let hwnd = hwnd as usize as HWND;
    if hwnd.is_null() {
        return None;
    }
    let mut point = POINT { x, y };
    // SAFETY: `point` is stack storage and `hwnd` is non-null.
    let ok = unsafe { ScreenToClient(hwnd, &mut point) };
    if ok == 0 {
        return None;
    }
    Some((point.x, point.y))
}

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

/// Module handle recorded once the grip class is registered.
static GRIP_MODULE: ClassModule = ClassModule(Mutex::new(None));

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
        // `CS_DBLCLKS` makes Windows deliver `WM_*BUTTONDBLCLK` for real
        // double clicks; the subclass reports those as Down + DoubleClick.
        style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
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

/// One pointer action captured on a native video surface.
///
/// Coordinates are client-area pixels of the surface named by
/// [`InputEvent::hwnd`]. A button press is bounded by [`MouseEvent::Down`] and
/// [`MouseEvent::Up`]; moves are reported whether or not a button is held, so
/// the consumer can derive the pressed state from those two events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseEvent {
    /// A button went down. `button`: 0 = left, 1 = right, 2 = middle.
    Down { x: i32, y: i32, button: u8 },
    /// A button went up. `button`: 0 = left, 1 = right, 2 = middle.
    Up { x: i32, y: i32, button: u8 },
    /// The pointer moved. Consecutive moves for one window are coalesced.
    Move { x: i32, y: i32 },
    /// A double click happened. The matching [`MouseEvent::Down`] is emitted
    /// first so consumers that only care about presses stay correct.
    DoubleClick,
    /// Raw wheel delta: 120 per notch, positive when scrolling up/away.
    Wheel { delta: i32 },
    /// The pointer left the surface.
    Leave,
}

/// A [`MouseEvent`] tagged with the native surface that produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputEvent {
    /// Native window handle of the surface, as an `i64`.
    pub hwnd: i64,
    pub event: MouseEvent,
}

/// Soft queue bound; overflow discards the oldest `Move` before anything else.
const INPUT_QUEUE_LIMIT: usize = 256;

/// `WM_MOUSELEAVE` (not exposed by the enabled windows-sys features).
const WM_MOUSELEAVE: u32 = 0x02A3;
/// `TME_LEAVE`: request a `WM_MOUSELEAVE` when the pointer exits the window.
const TME_LEAVE: u32 = 0x0000_0002;

static INPUT_QUEUE: OnceLock<Mutex<Vec<InputEvent>>> = OnceLock::new();

fn input_queue() -> &'static Mutex<Vec<InputEvent>> {
    INPUT_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Removes every captured pointer event, oldest first.
///
/// The application polls this once per frame and dispatches the events to the
/// plane that owns the handle. The returned vector is never longer than the
/// queue limit (256 entries).
pub fn take_input_events() -> Vec<InputEvent> {
    std::mem::take(&mut *lock(input_queue()))
}

fn queue_event(event: InputEvent) {
    let mut queue = lock(input_queue());
    let coalesces = matches!(event.event, MouseEvent::Move { .. })
        && queue.last().is_some_and(|last| {
            last.hwnd == event.hwnd && matches!(last.event, MouseEvent::Move { .. })
        });
    if coalesces {
        if let Some(last) = queue.last_mut() {
            last.event = event.event;
        }
        return;
    }

    while queue.len() >= INPUT_QUEUE_LIMIT {
        let oldest_move = queue
            .iter()
            .position(|queued| matches!(queued.event, MouseEvent::Move { .. }));
        match oldest_move {
            Some(index) => {
                queue.remove(index);
            }
            None => {
                queue.remove(0);
            }
        }
    }
    queue.push(event);
}

/// Per-surface state needed to turn raw messages into self-contained events.
struct HookedSurface {
    /// Procedure that was installed before [`install_input_hook`].
    original: WNDPROC,
    /// Bit mask of held buttons; bit 0 = left, 1 = right, 2 = middle.
    pressed: u8,
    /// Whether `TrackMouseEvent(TME_LEAVE)` is currently armed.
    leave_tracking: bool,
    last_x: i32,
    last_y: i32,
}

static INPUT_HOOKS: OnceLock<Mutex<HashMap<isize, HookedSurface>>> = OnceLock::new();

fn input_hooks() -> &'static Mutex<HashMap<isize, HookedSurface>> {
    INPUT_HOOKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn hwnd_key(hwnd: HWND) -> isize {
    hwnd as isize
}

fn client_point(lparam: LPARAM) -> (i32, i32) {
    let packed = lparam as u32;
    let x = (packed & 0xffff) as u16 as i16 as i32;
    let y = ((packed >> 16) & 0xffff) as u16 as i16 as i32;
    (x, y)
}

fn wheel_delta(wparam: WPARAM) -> i32 {
    ((wparam as u32 >> 16) & 0xffff) as u16 as i16 as i32
}

/// Whether `hwnd` still refers to a live window.
pub fn is_window(hwnd: i64) -> bool {
    hwnd != 0 && unsafe { IsWindow(hwnd as usize as HWND) } != 0
}

/// Handle of every direct child window of `hwnd`.
///
/// With `--wid` libmpv renders inside a window it creates below the surface we
/// hand it, so pointer messages are delivered to that child instead of our own
/// surface. Callers subclass those handles as well, otherwise dragging stops
/// working as soon as the renderer window is (re)created.
pub fn child_windows(hwnd: i64) -> Vec<i64> {
    if hwnd == 0 {
        return Vec::new();
    }
    let mut out: Vec<i64> = Vec::new();
    let context = std::ptr::from_mut(&mut out).cast::<c_void>() as LPARAM;
    // SAFETY: `EnumChildWindows` calls the callback synchronously while
    // `context` is alive, and the callback only appends to that `Vec`.
    unsafe { EnumChildWindows(hwnd as usize as HWND, Some(enum_child), context) };
    out
}

/// # Safety
/// `lparam` must be the `Vec<i64>` pointer passed by [`child_windows`].
unsafe extern "system" fn enum_child(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: see the contract above; the pointer outlives the enumeration.
    let out = unsafe { &mut *(lparam as *mut Vec<i64>) };
    out.push(hwnd as usize as i64);
    1
}

/// Installs the pointer capture subclass on a native surface.
///
/// [`ChildWindow::create`] calls this automatically, so applications normally
/// never need to. Installing twice for one handle is a no-op.
/// Escape hatch: `MUXVIDEO_NO_HOOKS=1` disables the message hooks so the
/// pointer-polling fallback is the only input path (useful for diagnosis).
pub fn hooks_disabled() -> bool {
    std::env::var_os("MUXVIDEO_NO_HOOKS").is_some()
}

pub fn install_input_hook(hwnd: i64) {
    if hooks_disabled() {
        return;
    }
    if hwnd == 0 {
        return;
    }
    let key = hwnd as isize;
    let mut hooks = lock(input_hooks());
    if hooks.contains_key(&key) {
        return;
    }

    let window = hwnd as usize as HWND;
    // SAFETY: `window` is a live window created by this module; replacing its
    // window procedure is the documented subclassing mechanism, and the
    // previous procedure is chained from `input_wndproc`.
    let previous =
        unsafe { SetWindowLongPtrW(window, GWLP_WNDPROC, input_wndproc as *const () as isize) };
    if previous == 0 {
        // The window died (or the call failed); leave it untouched.
        return;
    }
    // SAFETY: `SetWindowLongPtrW` returned the previous window procedure, an
    // address that is never NULL for a live window (that case was rejected
    // above), and function pointers have the same size as `isize` on Windows.
    let original: WNDPROC = Some(unsafe { std::mem::transmute::<isize, SurfaceWndProc>(previous) });
    hooks.insert(
        key,
        HookedSurface {
            original,
            pressed: 0,
            leave_tracking: false,
            last_x: 0,
            last_y: 0,
        },
    );
}

type SurfaceWndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

/// Chains a message to the procedure that preceded [`install_input_hook`].
///
/// # Safety
///
/// `hwnd` must be a live window; the function only forwards to the stored
/// window procedure (or `DefWindowProcW`).
unsafe fn call_original(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let original = lock(input_hooks())
        .get(&hwnd_key(hwnd))
        .and_then(|state| state.original);
    // SAFETY: the stored procedure was active on this window before this
    // subclass was installed; the fallback keeps default processing intact.
    match original {
        Some(procedure) => unsafe {
            CallWindowProcW(Some(procedure), hwnd, message, wparam, lparam)
        },
        None => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// `WM_NCDESTROY`: forget the surface, finish in-flight gestures, then chain.
fn handle_destroy(hwnd: HWND) -> Option<SurfaceWndProc> {
    let state = lock(input_hooks()).remove(&hwnd_key(hwnd))?;
    let key = hwnd_key(hwnd) as i64;
    if state.pressed != 0 {
        for button in 0..3u8 {
            if state.pressed & (1u8 << button) != 0 {
                queue_event(InputEvent {
                    hwnd: key,
                    event: MouseEvent::Up {
                        x: state.last_x,
                        y: state.last_y,
                        button,
                    },
                });
            }
        }
    }
    queue_event(InputEvent {
        hwnd: key,
        event: MouseEvent::Leave,
    });
    state.original
}

fn handle_move(hwnd: HWND, lparam: LPARAM) {
    let (x, y) = client_point(lparam);
    let key = hwnd_key(hwnd);
    let mut arm_leave = false;
    if let Some(state) = lock(input_hooks()).get_mut(&key) {
        state.last_x = x;
        state.last_y = y;
        if state.pressed == 0 && !state.leave_tracking {
            state.leave_tracking = true;
            arm_leave = true;
        }
    }

    if arm_leave {
        let mut request = MouseLeaveTrack {
            cb_size: std::mem::size_of::<MouseLeaveTrack>() as u32,
            flags: TME_LEAVE,
            hwnd,
            hover_time: 0,
        };
        // SAFETY: `request` is a correctly sized TME_LEAVE request for a live
        // window; the system copies it before returning.
        let tracked = unsafe { TrackMouseEvent(&mut request) };
        if tracked == 0
            && let Some(state) = lock(input_hooks()).get_mut(&key)
        {
            state.leave_tracking = false;
        }
    }

    queue_event(InputEvent {
        hwnd: key as i64,
        event: MouseEvent::Move { x, y },
    });
}

fn handle_leave(hwnd: HWND) {
    if let Some(state) = lock(input_hooks()).get_mut(&hwnd_key(hwnd)) {
        state.leave_tracking = false;
    }
    queue_event(InputEvent {
        hwnd: hwnd_key(hwnd) as i64,
        event: MouseEvent::Leave,
    });
}

fn handle_button(hwnd: HWND, lparam: LPARAM, button: u8, down: bool) {
    let (x, y) = client_point(lparam);
    let key = hwnd_key(hwnd);
    let mut capture = false;
    let mut release = false;
    if let Some(state) = lock(input_hooks()).get_mut(&key) {
        state.last_x = x;
        state.last_y = y;
        let bit = 1u8 << button;
        if down {
            capture = state.pressed == 0;
            state.pressed |= bit;
        } else {
            state.pressed &= !bit;
            release = state.pressed == 0;
        }
    }

    // Capture on the first press so a drag keeps reporting while the pointer
    // leaves the surface; release only when every button is up again.
    // SAFETY: `hwnd` is live; both calls are the standard capture sequence.
    unsafe {
        if capture {
            SetCapture(hwnd);
        }
        if release {
            ReleaseCapture();
        }
    }

    queue_event(InputEvent {
        hwnd: key as i64,
        event: if down {
            MouseEvent::Down { x, y, button }
        } else {
            MouseEvent::Up { x, y, button }
        },
    });
}

fn handle_double_click(hwnd: HWND, lparam: LPARAM, button: u8) {
    let (x, y) = client_point(lparam);
    let key = hwnd_key(hwnd);
    let mut capture = false;
    if let Some(state) = lock(input_hooks()).get_mut(&key) {
        state.last_x = x;
        state.last_y = y;
        capture = state.pressed == 0;
        state.pressed |= 1u8 << button;
    }
    if capture {
        // SAFETY: keep drags that start with a double click captured.
        unsafe {
            SetCapture(hwnd);
        }
    }
    queue_event(InputEvent {
        hwnd: key as i64,
        event: MouseEvent::Down { x, y, button },
    });
    queue_event(InputEvent {
        hwnd: key as i64,
        event: MouseEvent::DoubleClick,
    });
}

fn handle_capture_changed(hwnd: HWND) {
    let key = hwnd_key(hwnd);
    let Some((x, y, pressed)) = lock(input_hooks()).get_mut(&key).map(|state| {
        let pressed = std::mem::take(&mut state.pressed);
        (state.last_x, state.last_y, pressed)
    }) else {
        return;
    };
    if pressed == 0 {
        return;
    }
    for button in 0..3u8 {
        if pressed & (1u8 << button) != 0 {
            queue_event(InputEvent {
                hwnd: key as i64,
                event: MouseEvent::Up { x, y, button },
            });
        }
    }
    // Capture was lost mid-drag (alt-tab, another window); tell the consumer
    // the pointer is no longer tracked over this surface.
    queue_event(InputEvent {
        hwnd: key as i64,
        event: MouseEvent::Leave,
    });
}

fn handle_wheel(hwnd: HWND, wparam: WPARAM) {
    queue_event(InputEvent {
        hwnd: hwnd_key(hwnd) as i64,
        event: MouseEvent::Wheel {
            delta: wheel_delta(wparam),
        },
    });
}

/// Subclass procedure: records pointer input, then chains every message.
unsafe extern "system" fn input_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_MOUSEMOVE => handle_move(hwnd, lparam),
        WM_LBUTTONDOWN => handle_button(hwnd, lparam, 0, true),
        WM_LBUTTONUP => handle_button(hwnd, lparam, 0, false),
        WM_RBUTTONDOWN => handle_button(hwnd, lparam, 1, true),
        WM_RBUTTONUP => handle_button(hwnd, lparam, 1, false),
        WM_MBUTTONDOWN => handle_button(hwnd, lparam, 2, true),
        WM_MBUTTONUP => handle_button(hwnd, lparam, 2, false),
        WM_LBUTTONDBLCLK => handle_double_click(hwnd, lparam, 0),
        WM_RBUTTONDBLCLK => handle_double_click(hwnd, lparam, 1),
        WM_MBUTTONDBLCLK => handle_double_click(hwnd, lparam, 2),
        WM_MOUSEWHEEL => handle_wheel(hwnd, wparam),
        WM_MOUSELEAVE => handle_leave(hwnd),
        WM_CAPTURECHANGED => handle_capture_changed(hwnd),
        WM_NCDESTROY => {
            let original = handle_destroy(hwnd);
            // SAFETY: the surface is being destroyed; chain to the procedure
            // that owned it, falling back to default processing.
            return match original {
                Some(procedure) => unsafe {
                    CallWindowProcW(Some(procedure), hwnd, message, wparam, lparam)
                },
                None => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
            };
        }
        _ => {}
    }

    // SAFETY: every recorded message is also forwarded, so the original class
    // procedure keeps observing the same sequence it always saw.
    unsafe { call_original(hwnd, message, wparam, lparam) }
}

/// `TrackMouseEvent` request structure (the windows-sys feature that would
/// provide it is not enabled by this crate).
#[repr(C)]
struct MouseLeaveTrack {
    cb_size: u32,
    flags: u32,
    hwnd: HWND,
    hover_time: u32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SetCapture(hwnd: HWND) -> HWND;
    fn ReleaseCapture() -> BOOL;
    fn TrackMouseEvent(track: *mut MouseLeaveTrack) -> BOOL;
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

        install_input_hook(hwnd as isize as i64);
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

    /// Creates the wipe grip: a small native window that paints itself like a
    /// Material handle.
    ///
    /// egui cannot draw over the native picture planes, so the divider handle
    /// has to be a native window of its own; [`set_grip_style`] supplies the
    /// colours and orientation, and the window rounds its own corners.
    pub fn create_grip(parent: i64, id: u32, name: &str) -> anyhow::Result<Self> {
        ensure_grip_class()?;

        // SAFETY: current-executable handle, alive for the process lifetime.
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        if instance.is_null() {
            bail!("GetModuleHandleW(NULL) failed");
        }

        let parent = parent as usize as HWND;
        if parent.is_null() {
            bail!("cannot create {GRIP_CLASS}: parent handle is NULL");
        }

        let class_name = wide(GRIP_CLASS);
        let title = wide(name);
        // SAFETY: both UTF-16 strings are live for the call; the class is
        // registered and `parent` is a live window of ours.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                18,
                56,
                parent,
                id as usize as HMENU,
                instance,
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            // SAFETY: no arguments, always safe to call.
            let error = unsafe { GetLastError() };
            bail!("CreateWindowExW({GRIP_CLASS}) failed (Win32 error {error})");
        }

        install_input_hook(hwnd as isize as i64);
        Ok(Self { hwnd })
    }

    /// Creates the scrub preview surface (opaque, blits cached frames).
    pub fn create_preview(parent: i64, id: u32, name: &str) -> anyhow::Result<Self> {
        ensure_preview_class()?;

        // SAFETY: current-executable handle, alive for the process lifetime.
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        if instance.is_null() {
            bail!("GetModuleHandleW(NULL) failed");
        }

        let parent = parent as usize as HWND;
        if parent.is_null() {
            bail!("cannot create {PREVIEW_CLASS}: parent handle is NULL");
        }

        let class_name = wide(PREVIEW_CLASS);
        let title = wide(name);
        // SAFETY: both UTF-16 strings are live for the call; the class is
        // registered and `parent` is a live window of ours.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                64,
                64,
                parent,
                id as usize as HMENU,
                instance,
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            // SAFETY: no arguments, always safe to call.
            let error = unsafe { GetLastError() };
            bail!("CreateWindowExW({PREVIEW_CLASS}) failed (Win32 error {error})");
        }
        Ok(Self { hwnd })
    }

    /// Shapes the wipe grip: a one-pixel divider plus a rounded pill centred on
    /// it.
    ///
    /// `vertical` orients the divider (a vertical seam for a left/right split).
    /// `long`/`short` are the pill's dimensions; `short` is also its corner
    /// radius, so the pill stays fully rounded.
    pub fn set_grip_region(&self, vertical: bool, long: i32, short: i32) {
        let mut rect = RECT::default();
        // SAFETY: client rect query on a live HWND; `RECT` is a plain struct.
        let (w, h) = unsafe {
            if GetClientRect(self.hwnd, &mut rect) != 0 {
                (rect.right - rect.left, rect.bottom - rect.top)
            } else {
                (0, 0)
            }
        };
        if w <= 0 || h <= 0 {
            return;
        }
        let cx = w / 2;
        let cy = h / 2;
        // A vertical seam gets a tall pill, a horizontal seam a wide one.
        let (pill_w, pill_h) = if vertical {
            (short, long)
        } else {
            (long, short)
        };
        // SAFETY: regions are plain GDI objects created and released here; on
        // success SetWindowRgn takes ownership of the combined one.
        unsafe {
            let line = if vertical {
                CreateRectRgn(cx, 0, cx + 1, h)
            } else {
                CreateRectRgn(0, cy, w, cy + 1)
            };
            if line.is_null() {
                return;
            }
            let pill = CreateRoundRectRgn(
                cx - pill_w / 2,
                cy - pill_h / 2,
                cx + pill_w / 2 + 1,
                cy + pill_h / 2 + 1,
                pill_h.min(pill_w),
                pill_h.min(pill_w),
            );
            if pill.is_null() {
                DeleteObject(line);
                return;
            }
            if CombineRgn(line, line, pill, RGN_OR) == 0 {
                DeleteObject(pill);
                DeleteObject(line);
                return;
            }
            DeleteObject(pill);
            if SetWindowRgn(self.hwnd, line, 1) == 0 {
                DeleteObject(line);
            }
        }
    }

    /// Applies the rounded-pill region matching the current size.
    ///
    /// Called after every resize so the handle keeps its Material shape
    /// instead of becoming a rectangle.
    pub fn round_corners(&self, radius: i32) {
        let mut rect = RECT::default();
        // SAFETY: client rect query on a live HWND; `RECT` is a plain struct.
        let (w, h) = unsafe {
            if GetClientRect(self.hwnd, &mut rect) != 0 {
                (rect.right - rect.left, rect.bottom - rect.top)
            } else {
                (0, 0)
            }
        };
        if w <= 0 || h <= 0 {
            return;
        }
        // SAFETY: CreateRoundRectRgn needs no window and returns an owned
        // region, which SetWindowRgn takes over on success.
        let region = unsafe { CreateRoundRectRgn(0, 0, w + 1, h + 1, radius, radius) };
        if region.is_null() {
            return;
        }
        // SAFETY: `region` is valid; on success the system owns it.
        let result = unsafe { SetWindowRgn(self.hwnd, region, 1) };
        if result == 0 {
            // SAFETY: SetWindowRgn failed, so the region is still ours.
            unsafe {
                DeleteObject(region);
            }
        }
    }

    /// Repaints the handle after a style change.
    pub fn invalidate(&self) {
        // SAFETY: live HWND; a NULL rect invalidates the whole client area.
        unsafe {
            InvalidateRect(self.hwnd, std::ptr::null(), 0);
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
        // The class background brush is black, so let it erase: a resized or
        // exposed surface then repaints opaque instead of showing whatever was
        // behind it (which looked like the window going transparent).
        WM_ERASEBKGND => {
            // SAFETY: default handling uses the class brush registered above.
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
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

/// Whether the window is currently maximized.
pub fn is_maximized(hwnd: i64) -> bool {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::IsZoomed;

    if hwnd == 0 {
        return false;
    }
    // SAFETY: read-only query on our own window.
    unsafe { IsZoomed(hwnd as HWND) != 0 }
}

/// Restore a maximized window to its previous floating size.
pub fn restore_window(hwnd: i64) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_RESTORE, ShowWindow};

    if hwnd == 0 {
        return;
    }
    // SAFETY: our own window; SW_RESTORE only affects the show state.
    unsafe {
        let _ = ShowWindow(hwnd as HWND, SW_RESTORE);
    }
}

/// Move and resize our own top-level window without going through the
/// activation-dependent modal move/resize loops.
pub fn set_window_rect(hwnd: i64, x: i32, y: i32, w: i32, h: i32) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos};

    if hwnd == 0 {
        return;
    }
    // SAFETY: our own window; the rectangle comes from a tracked drag.
    unsafe {
        let _ = SetWindowPos(
            hwnd as HWND,
            std::ptr::null_mut(),
            x,
            y,
            w.max(320),
            h.max(200),
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_encoding_is_nul_terminated() {
        assert_eq!(wide(""), vec![0]);
        assert_eq!(wide("abc"), vec![0x61, 0x62, 0x63, 0]);
    }

    #[test]
    fn input_queue_coalesces_moves_and_evicts_moves_first() {
        let _ = take_input_events();
        let hwnd = 0x1234_i64;

        queue_event(InputEvent {
            hwnd,
            event: MouseEvent::Down {
                x: 1,
                y: 2,
                button: 0,
            },
        });
        for x in [0, 0, 1, 1] {
            queue_event(InputEvent {
                hwnd,
                event: MouseEvent::Move { x, y: 0 },
            });
        }
        let events = take_input_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].event, MouseEvent::Move { x: 1, y: 0 });

        // Fill the queue completely, then overflow it: the oldest Move goes and
        // the button event is preserved.
        queue_event(InputEvent {
            hwnd,
            event: MouseEvent::Move { x: 0, y: 0 },
        });
        for _ in 0..(INPUT_QUEUE_LIMIT - 1) {
            queue_event(InputEvent {
                hwnd,
                event: MouseEvent::Wheel { delta: 120 },
            });
        }
        queue_event(InputEvent {
            hwnd,
            event: MouseEvent::Down {
                x: 3,
                y: 4,
                button: 1,
            },
        });
        let events = take_input_events();
        assert_eq!(events.len(), INPUT_QUEUE_LIMIT);
        assert_eq!(
            events.first().map(|event| event.event),
            Some(MouseEvent::Wheel { delta: 120 })
        );
        assert_eq!(
            events.last().map(|event| event.event),
            Some(MouseEvent::Down {
                x: 3,
                y: 4,
                button: 1
            })
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event.event, MouseEvent::Move { .. }))
        );
    }
}
