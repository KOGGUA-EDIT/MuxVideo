import pathlib

# childwin: expose a debug description of the window under the pointer
p = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
s = p.read_text(encoding="utf-8")
if "pub fn hit_test_debug" not in s:
    anchor = "pub fn hit_test_owns(sx: i32, sy: i32) -> bool {"
    add = '''/// Debug helper: `(pid, class)` of the window under a screen point.
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

pub fn hit_test_owns(sx: i32, sy: i32) -> bool {'''
    assert anchor in s
    s = s.replace(anchor, add, 1)
    if "GetClassNameW" not in s.split("use windows_sys")[1][:3000]:
        s = s.replace("    GetClientRect, GetCursorPos,", "    GetClassNameW, GetClientRect, GetCursorPos,", 1)
    p.write_text(s, encoding="utf-8")
    print("debug helper added")

q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")
old = """        if app.media.app_is_foreground() {
            crate::diag!("[poll] pointer not ours although focused");
        }"""
new = """        if let Some((sx, sy)) = crate::video::childwin::cursor_pos() {
            let (pid, class) = crate::video::childwin::hit_test_debug(sx, sy);
            crate::diag!(
                "[poll] skip own={} focused={} hit_pid={pid} class={class}",
                owns_pointer,
                app.media.app_is_foreground()
            );
        }"""
assert old in t
t = t.replace(old, new, 1)
q.write_text(t, encoding="utf-8")
print("guard diag added")
