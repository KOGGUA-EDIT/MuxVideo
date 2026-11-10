import pathlib

child = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
s = child.read_text(encoding="utf-8")

if "pub fn hit_test_owns" not in s:
    s = s.replace(
        "    GetForegroundWindow, GetWindowRect,",
        "    GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, WindowFromPoint,",
        1,
    )
    anchor = "/// Current pointer position in screen coordinates."
    add = '''/// Whether the window under a screen point belongs to this process.
///
/// Used by the pointer-polling fallback: it must not scrub when the click is
/// meant for another application that happens to sit above the picture.
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

/// Current pointer position in screen coordinates.'''
    assert anchor in s
    s = s.replace(anchor, add, 1)
    child.write_text(s, encoding="utf-8")

inp = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = inp.read_text(encoding="utf-8")

old_guard = """    if !app.media.app_is_foreground() {
        crate::diag!("[poll] skip: not foreground");
        let mut state = lock(poll_state());
        state.button_down = false;
        state.synthetic = false;
        return;
    }"""
new_guard = """    let owns_pointer = crate::video::childwin::cursor_pos()
        .is_some_and(|(sx, sy)| crate::video::childwin::hit_test_owns(sx, sy));
    if !app.media.app_is_foreground() && !owns_pointer {
        let mut state = lock(poll_state());
        state.button_down = false;
        state.synthetic = false;
        return;
    }"""
assert old_guard in t
t = t.replace(old_guard, new_guard, 1)

# Drop gestures that can no longer be real (lost release, window switch, ...).
old_down = """    if down && !state.button_down {
        state.button_down = true;"""
new_down = """    if down && !state.button_down {
        state.button_down = true;
        if app
            .gesture
            .as_ref()
            .is_some_and(|d| d.began.elapsed() > POLL_STALE)
        {
            crate::diag!("[poll] dropping stale gesture");
            app.gesture = None;
            app.scrub = None;
        }"""
assert old_down in t
t = t.replace(old_down, new_down, 1)

t = t.replace(
    "const POLL_ADOPT: Duration = Duration::from_millis(120);",
    "const POLL_ADOPT: Duration = Duration::from_millis(120);\n/// A gesture younger than this can still be live; older ones are leftovers.\nconst POLL_STALE: Duration = Duration::from_secs(3);",
    1,
)
inp.write_text(t, encoding="utf-8")
print("ok")
