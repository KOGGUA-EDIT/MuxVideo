import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
s = p.read_text(encoding="utf-8")

s = s.replace(
    """        // Remember that the native hook is alive: the cursor-polling fallback
        // below only steps in when no hook press showed up for a while.
        if matches!(
            event.event,
            MouseEvent::Down { button: 0, .. } | MouseEvent::Up { button: 0, .. }
        ) {
            let mut state = lock(poll_state());
            state.last_hook_button = now;
            state.synthetic = false;
        }""",
    """        // Remember that the native hook is alive: the cursor-polling fallback
        // below only steps in when hook events stop arriving.
        {
            let mut state = lock(poll_state());
            state.last_hook_event = now;
            if matches!(
                event.event,
                MouseEvent::Down { button: 0, .. } | MouseEvent::Up { button: 0, .. }
            ) {
                state.synthetic = false;
            }
        }""",
    1,
)

s = s.replace(
    """struct PollState {
    /// When the native hook last reported a left button event.
    last_hook_button: Instant,""",
    """struct PollState {
    /// When the native hook last reported any mouse event.
    last_hook_event: Instant,""",
    1,
)

s = s.replace(
    """        Mutex::new(PollState {
            last_hook_button: Instant::now() - Duration::from_secs(60),
            synthetic: false,
        })""",
    """        Mutex::new(PollState {
            last_hook_event: Instant::now() - Duration::from_secs(60),
            synthetic: false,
        })""",
    1,
)

old = """    if down {
        let cursor = crate::video::childwin::cursor_pos();
        if app.gesture.is_none() {
            // No hook press: start the scrub ourselves once the grace elapsed.
            if state.last_hook_button.elapsed() < POLL_GRACE {
                return;
            }
            if let Some((sx, sy)) = cursor
                && let Some((plane, cx, cy)) = app.media.plane_at_screen(sx, sy)
            {
                state.synthetic = true;
                press(app, plane, cx, cy, 0);
                ctx.request_repaint();
            }
        } else if state.synthetic {"""
new = """    if down {
        let cursor = crate::video::childwin::cursor_pos();
        // Adopt the gesture when the hook went quiet mid-drag as well: mpv can
        // swap its renderer window between two moves, and from then on the old
        // window never reports again.
        if app.gesture.is_some() && state.last_hook_event.elapsed() > POLL_GRACE {
            state.synthetic = true;
        }
        if app.gesture.is_none() {
            // No hook press: start the scrub ourselves once the grace elapsed.
            if state.last_hook_event.elapsed() < POLL_GRACE {
                return;
            }
            if let Some((sx, sy)) = cursor
                && let Some((plane, cx, cy)) = app.media.plane_at_screen(sx, sy)
            {
                state.synthetic = true;
                press(app, plane, cx, cy, 0);
                ctx.request_repaint();
            }
        } else if state.synthetic {"""
assert old in s
s = s.replace(old, new, 1)
p.write_text(s, encoding="utf-8")

stage = pathlib.Path(r"D:\app edit\Muxvideo\src\app\stage.rs")
t = stage.read_text(encoding="utf-8")
dead = """    /// Handle of a plane window, if it exists.
    pub fn plane_hwnd(&self, plane: Plane) -> i64 {
        match plane {
            Plane::A => self.plane_a.hwnd,
            Plane::B => self.plane_b.hwnd,
        }
    }

"""
t = t.replace(dead, "", 1)
stage.write_text(t, encoding="utf-8")
print("ok")
