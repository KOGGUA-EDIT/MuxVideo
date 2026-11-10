import pathlib

q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")

old_rel = """    } else if !down && state.button_down {
        state.button_down = false;
        // A release the hook never reported would otherwise leave the gesture
        // stuck forever.
        if app.gesture.is_some() {
            release(app);
            ctx.request_repaint();
        } else if !state.synthetic && !state.hook_press {
            // The poll saw a press but the gesture never started (released
            // inside the grace window): treat it as a plain click.
            synthetic_tap(app, ctx);
        }
        state.hook_press = false;
        state.synthetic = false;
        return;
    }"""
new_rel = """    } else if !down && state.button_down {
        state.button_down = false;
        let hook_press = std::mem::replace(&mut state.hook_press, false);
        let synthetic = std::mem::replace(&mut state.synthetic, false);
        // `synthetic_tap` locks the same state: never hold the guard here.
        drop(state);
        // A release the hook never reported would otherwise leave the gesture
        // stuck forever.
        if app.gesture.is_some() {
            release(app);
            ctx.request_repaint();
        } else if !synthetic && !hook_press {
            // The poll saw a press but the gesture never started (released
            // inside the grace window): treat it as a plain click.
            synthetic_tap(app, ctx);
        }
        return;
    }"""
assert old_rel in t
t = t.replace(old_rel, new_rel, 1)

old_tap = """    if !down {
        // A tap that started and ended between two polls still counts as a
        // click, so play/pause keeps working when the hooks missed the press.
        if tapped && app.gesture.is_none() && !state.hook_press {
            synthetic_tap(app, ctx);
        }
        state.hook_press = false;
        return;
    }"""
new_tap = """    if !down {
        // A tap that started and ended between two polls still counts as a
        // click, so play/pause keeps working when the hooks missed the press.
        let hook_press = std::mem::replace(&mut state.hook_press, false);
        drop(state);
        if tapped && app.gesture.is_none() && !hook_press {
            synthetic_tap(app, ctx);
        }
        return;
    }"""
assert old_tap in t
t = t.replace(old_tap, new_tap, 1)
q.write_text(t, encoding="utf-8")
print("deadlock fixed")
