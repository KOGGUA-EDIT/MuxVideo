import pathlib

# 1. childwin: expose a combined button sample
p = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
s = p.read_text(encoding="utf-8")
if "pub fn left_button_state" not in s:
    old = """pub fn left_button_down() -> bool {"""
    assert old in s
    new = """/// Samples the physical left button.
///
/// Returns `(down, pressed_since_last_call)`. The second flag comes from the
/// low bit of `GetAsyncKeyState`, which the system sets when the key went down
/// since the previous sample - that catches fast taps that start *and* end
/// between two polls.
pub fn left_button_state() -> (bool, bool) {
    // SAFETY: read-only query, no pointers.
    let state = unsafe { GetAsyncKeyState(VK_LBUTTON) } as u16;
    (state & 0x8000 != 0, state & 0x0001 != 0)
}

pub fn left_button_down() -> bool {"""
    s = s.replace(old, new, 1)
    # keep the old body but implement it via the new sampler
    old_body = """pub fn left_button_down() -> bool {
    // SAFETY: read-only query, no pointers.
    let state = unsafe { GetAsyncKeyState(VK_LBUTTON) } as u16;
    state & 0x8000 != 0
}"""
    new_body = """pub fn left_button_down() -> bool {
    left_button_state().0
}"""
    if old_body in s:
        s = s.replace(old_body, new_body, 1)
    p.write_text(s, encoding="utf-8")
    print("sampler added")
else:
    print("already present")

# 2. input: handle fast taps in the fallback
q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")
old_line = "    let down = crate::video::childwin::left_button_down();"
assert old_line in t
new_line = "    let (down, tapped) = crate::video::childwin::left_button_state();"
t = t.replace(old_line, new_line, 1)

anchor = """    if !down {
        return;
    }"""
new_anchor = """    if !down {
        // A tap that started and ended between two polls still counts as a
        // click, so play/pause keeps working when the hooks missed the press.
        if tapped
            && app.gesture.is_none()
            && let Some((sx, sy)) = crate::video::childwin::cursor_pos()
            && let Some((plane, cx, cy)) = app.media.plane_at_screen(sx, sy)
        {
            crate::diag!("[poll] synthetic tap ({cx},{cy})");
            press(app, plane, cx, cy, 0);
            release(app);
            ctx.request_repaint();
        }
        return;
    }"""
assert anchor in t
t = t.replace(anchor, new_anchor, 1)
q.write_text(t, encoding="utf-8")
print("tap handling added")
