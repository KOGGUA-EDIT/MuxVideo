import pathlib

q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")

old = """        if tapped && app.gesture.is_none() {
            synthetic_tap(app, ctx);
        }
        return;"""
new = """        if tapped && app.gesture.is_none() && !state.hook_press {
            synthetic_tap(app, ctx);
        }
        state.hook_press = false;
        return;"""
assert old in t
t = t.replace(old, new, 1)
q.write_text(t, encoding="utf-8")
print("tap suppressed when hook handled it")
