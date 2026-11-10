import pathlib

q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")

old = """    } else if !down && state.button_down {
        state.button_down = false;
        // A release the hook never reported would otherwise leave the gesture
        // stuck forever.
        if app.gesture.is_some() {
            release(app);
            ctx.request_repaint();
        }
        state.synthetic = false;
        return;
    }"""
new = """    } else if !down && state.button_down {
        state.button_down = false;
        // A release the hook never reported would otherwise leave the gesture
        // stuck forever.
        if app.gesture.is_some() {
            release(app);
            ctx.request_repaint();
        } else if !state.synthetic {
            // The poll saw a press but the gesture never started (released
            // inside the grace window): treat it as a plain click.
            synthetic_tap(app, ctx);
        }
        state.synthetic = false;
        return;
    }"""
assert old in t, "release block not found"
t = t.replace(old, new, 1)

old_tap = """        if tapped
            && app.gesture.is_none()
            && let Some((sx, sy)) = crate::video::childwin::cursor_pos()
            && let Some((plane, cx, cy)) = app.media.plane_at_screen(sx, sy)
        {
            crate::diag!("[poll] synthetic tap ({cx},{cy})");
            press(app, plane, cx, cy, 0);
            release(app);
            ctx.request_repaint();
        }"""
new_tap = """        if tapped && app.gesture.is_none() {
            synthetic_tap(app, ctx);
        }"""
assert old_tap in t, "tap block not found"
t = t.replace(old_tap, new_tap, 1)

anchor = "fn poll_fallback(app: &mut MuxApp, ctx: &egui::Context) {"
helper = """/// Replays a click that only the polling loop saw: press + release in place,
/// so the click-vs-drag logic in `release` decides between play/pause and a
/// zero-length scrub.
fn synthetic_tap(app: &mut MuxApp, ctx: &egui::Context) {
    if app.gesture.is_some() {
        return;
    }
    let Some((sx, sy)) = crate::video::childwin::cursor_pos() else {
        return;
    };
    let Some((plane, cx, cy)) = app.media.plane_at_screen(sx, sy) else {
        return;
    };
    crate::diag!("[poll] synthetic tap ({cx},{cy})");
    press(app, plane, cx, cy, 0);
    release(app);
    ctx.request_repaint();
}

fn poll_fallback(app: &mut MuxApp, ctx: &egui::Context) {"""
assert anchor in t
t = t.replace(anchor, helper, 1)
q.write_text(t, encoding="utf-8")
print("tap helper wired")
