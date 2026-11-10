import pathlib

# 1. childwin: is_foreground
child = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
s = child.read_text(encoding="utf-8")
if "pub fn is_foreground" not in s:
    s = s.replace(
        "    DestroyWindow, EnumChildWindows, EnumWindows, GWLP_WNDPROC, GetClientRect, GetCursorPos,\n    GetWindowRect,",
        "    DestroyWindow, EnumChildWindows, EnumWindows, GWLP_WNDPROC, GetClientRect, GetCursorPos,\n    GetForegroundWindow, GetWindowRect,",
        1,
    )
    anchor = "/// Current pointer position in screen coordinates."
    addition = '''/// Whether `hwnd` is the foreground window.
pub fn is_foreground(hwnd: i64) -> bool {
    let hwnd = hwnd as usize as HWND;
    if hwnd.is_null() {
        return false;
    }
    // SAFETY: no pointers involved; the call only reads global state.
    let foreground = unsafe { GetForegroundWindow() };
    !foreground.is_null() && foreground == hwnd
}

/// Current pointer position in screen coordinates.'''
    assert anchor in s
    s = s.replace(anchor, addition, 1)
child.write_text(s, encoding="utf-8")

# 2. stage: app_is_foreground
stage = pathlib.Path(r"D:\app edit\Muxvideo\src\app\stage.rs")
t = stage.read_text(encoding="utf-8")
if "pub fn app_is_foreground" not in t:
    anchor = "    /// Hit-tests a screen point against the visible planes."
    addition = '''    /// Whether the player toplevel currently owns the keyboard focus.
    ///
    /// The cursor-polling fallback only runs then: otherwise a click on another
    /// window that happens to sit above the picture would scrub the video.
    pub fn app_is_foreground(&self) -> bool {
        find_window_by_title(WINDOW_TITLE)
            .is_some_and(crate::video::childwin::is_foreground)
    }

    /// Hit-tests a screen point against the visible planes.'''
    assert anchor in t
    t = t.replace(anchor, addition, 1)
stage.write_text(t, encoding="utf-8")

# 3. input: guard the fallback on foreground
inp = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
i = inp.read_text(encoding="utf-8")
old = """fn poll_fallback(app: &mut MuxApp, ctx: &egui::Context) {
    let down = crate::video::childwin::left_button_down();"""
new = """fn poll_fallback(app: &mut MuxApp, ctx: &egui::Context) {
    // Only trust the poll while the player owns the foreground: otherwise a
    // click on whatever window covers the picture would scrub it.
    if !app.media.app_is_foreground() {
        let mut state = lock(poll_state());
        state.button_down = false;
        state.synthetic = false;
        return;
    }
    let down = crate::video::childwin::left_button_down();"""
assert old in i
i = i.replace(old, new, 1)
inp.write_text(i, encoding="utf-8")
print("ok")
