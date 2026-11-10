import pathlib

q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")

old = """    let owns_pointer = crate::video::childwin::cursor_pos()
        .is_some_and(|(sx, sy)| crate::video::childwin::hit_test_owns(sx, sy));
    if !app.media.app_is_foreground() && !owns_pointer {"""
new = """    // The only trustworthy signal: the window under the pointer belongs to
    // this process. A window can be "foreground" while another one covers it,
    // so focus alone is not enough.
    let owns_pointer = crate::video::childwin::cursor_pos()
        .is_some_and(|(sx, sy)| crate::video::childwin::hit_test_owns(sx, sy));
    if !owns_pointer {"""
assert old in t
t = t.replace(old, new, 1)
q.write_text(t, encoding="utf-8")
print("guard tightened")
