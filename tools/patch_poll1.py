import pathlib

cargo = pathlib.Path(r"D:\app edit\Muxvideo\Cargo.toml")
c = cargo.read_text(encoding="utf-8")
if "Win32_UI_Input_KeyboardAndMouse" not in c:
    c = c.replace(
        '    "Win32_UI_WindowsAndMessaging",',
        '    "Win32_UI_Input_KeyboardAndMouse",\n    "Win32_UI_WindowsAndMessaging",',
        1,
    )
cargo.write_text(c, encoding="utf-8")

child = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
s = child.read_text(encoding="utf-8")

# extra imports
s = s.replace(
    """use windows_sys::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, ClientToScreen, CreatePen, CreateRectRgn, CreateRoundRectRgn,""",
    """use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows_sys::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, ClientToScreen, CreatePen, CreateRectRgn, CreateRoundRectRgn,""",
    1,
)
s = s.replace(
    "    DestroyWindow, EnumChildWindows, EnumWindows, GWLP_WNDPROC, GetClientRect,",
    "    DestroyWindow, EnumChildWindows, EnumWindows, GWLP_WNDPROC, GetClientRect, GetCursorPos,\n    GetWindowRect,",
    1,
)

helpers = '''
/// Current pointer position in screen coordinates.
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: `point` is stack storage owned by the caller.
    let ok = unsafe { GetCursorPos(&mut point) };
    if ok == 0 { None } else { Some((point.x, point.y)) }
}

/// Whether the physical left mouse button is held down right now.
///
/// Polling the button state is the fallback for the cases where the native
/// hook misses a press (the renderer can recreate its window at any time).
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

/// Finds the toplevel window with an exact title that belongs to this process.'''

s = s.replace(
    "/// Finds the toplevel window with an exact title that belongs to this process.",
    helpers,
    1,
)
child.write_text(s, encoding="utf-8")
print("ok")
