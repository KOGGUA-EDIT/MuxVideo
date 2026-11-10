import pathlib

q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")

# The hook reports Down before the poll sees the physical edge, so the poll
# must not clear the flag there - only after it has used it on release.
old_reset = """        state.synthetic = false;
        state.hook_press = false;"""
if old_reset in t:
    t = t.replace(old_reset, """        state.synthetic = false;""", 1)

old_rel = """        } else if !state.synthetic && !state.hook_press {
            // The poll saw a press but the gesture never started (released
            // inside the grace window): treat it as a plain click.
            synthetic_tap(app, ctx);
        }
        state.synthetic = false;"""
new_rel = """        } else if !state.synthetic && !state.hook_press {
            // The poll saw a press but the gesture never started (released
            // inside the grace window): treat it as a plain click.
            synthetic_tap(app, ctx);
        }
        state.hook_press = false;
        state.synthetic = false;"""
assert old_rel in t
t = t.replace(old_rel, new_rel, 1)
q.write_text(t, encoding="utf-8")
print("ordering fixed")
