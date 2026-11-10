import pathlib
import re

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
s = p.read_text(encoding="utf-8")

start = s.index("/// Padding on the native hook before the polling fallback takes over.")
new_block = '''/// How long the native hook gets to report a press before the poll takes over.
const POLL_GRACE: Duration = Duration::from_millis(80);

/// Extra grace before a silent drag (press seen, moves missing) is adopted.
const POLL_ADOPT: Duration = Duration::from_millis(120);

/// State for the cursor-polling fallback.
struct PollState {
    /// Whether the physical left button was down on the previous frame.
    button_down: bool,
    /// When the current press stops waiting for the native hook.
    deadline: Instant,
    /// Whether the current gesture was synthesised from the cursor poll.
    synthetic: bool,
    /// Hook move events seen while the button is held.
    moves_while_down: u32,
}

fn poll_state() -> &'static Mutex<PollState> {
    static STATE: OnceLock<Mutex<PollState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(PollState {
            button_down: false,
            deadline: Instant::now(),
            synthetic: false,
            moves_while_down: 0,
        })
    })
}

fn lock<T>(mutex: &'static Mutex<T>) -> MutexGuard<'static, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Counts hook moves that happen while the left button is held.
fn note_hook_event(event: &MouseEvent) {
    let down = crate::video::childwin::left_button_down();
    let mut state = lock(poll_state());
    match event {
        MouseEvent::Down { button: 0, .. } => {
            // The hook handled the press; the poll stays out of the way.
            state.synthetic = false;
            state.moves_while_down = 0;
        }
        MouseEvent::Move { .. } if down => state.moves_while_down += 1,
        _ => {}
    }
}

/// Drives the scrub gesture from the OS cursor state.
///
/// libmpv recreates its renderer window whenever it re-initialises the video
/// output (first load, resolution or pixel-format change, some seeks), and the
/// window that owns the pointer can also change while a drag is running. Both
/// cases used to swallow presses and moves, which is why dragging only worked
/// "sometimes". Watching the physical button and the cursor makes the gesture
/// independent of which window currently receives the messages.
fn poll_fallback(app: &mut MuxApp, ctx: &egui::Context) {
    let down = crate::video::childwin::left_button_down();
    let now = Instant::now();
    let mut state = lock(poll_state());

    if down && !state.button_down {
        state.button_down = true;
        state.deadline = now + POLL_GRACE;
        state.moves_while_down = 0;
        state.synthetic = false;
    } else if !down && state.button_down {
        state.button_down = false;
        // A release the hook never reported would otherwise leave the gesture
        // stuck forever.
        if app.gesture.is_some() {
            release(app);
            ctx.request_repaint();
        }
        state.synthetic = false;
        return;
    }

    if !down {
        return;
    }

    if app.gesture.is_none() {
        if now < state.deadline {
            return;
        }
        if let Some((sx, sy)) = crate::video::childwin::cursor_pos()
            && let Some((plane, cx, cy)) = app.media.plane_at_screen(sx, sy)
        {
            state.synthetic = true;
            press(app, plane, cx, cy, 0);
            ctx.request_repaint();
        }
        return;
    }

    // Press seen by the hook, but it stopped reporting movement: take over.
    if !state.synthetic && state.moves_while_down == 0 && now >= state.deadline + POLL_ADOPT {
        state.synthetic = true;
    }
    if state.synthetic {
        if let Some((sx, sy)) = crate::video::childwin::cursor_pos()
            && let Some((plane, cx, cy)) = app.media.plane_at_screen(sx, sy)
            && app.gesture.as_ref().is_some_and(|d| d.plane == plane)
        {
            drag_to(app, ctx, cx, cy);
        }
        ctx.request_repaint();
    }
}
'''
s = s[:start] + new_block
p.write_text(s, encoding="utf-8")

# pump: replace the inline state update with note_hook_event
old = """    for event in events {
        // Remember that the native hook is alive: the cursor-polling fallback
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
        }"""
new = """    for event in events {
        note_hook_event(&event.event);"""
s = p.read_text(encoding="utf-8")
assert old in s
s = s.replace(old, new, 1)
s = s.replace("    let now = Instant::now();\n    for event in events {", "    for event in events {", 1)
p.write_text(s, encoding="utf-8")
print("ok")
