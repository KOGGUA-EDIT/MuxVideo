import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
s = p.read_text(encoding="utf-8")

# imports
if "use std::sync::{Mutex, MutexGuard, OnceLock};" not in s:
    s = s.replace(
        "use std::time::{Duration, Instant};",
        "use std::sync::{Mutex, MutexGuard, OnceLock};\nuse std::time::{Duration, Instant};",
        1,
    )

# pump head: no early return, per-event bookkeeping for the poll fallback
old_head = """pub(super) fn pump(app: &mut MuxApp, ctx: &egui::Context) {
    let events = crate::video::take_input_events();
    if events.is_empty() {
        return;
    }
    for event in events {"""
new_head = """pub(super) fn pump(app: &mut MuxApp, ctx: &egui::Context) {
    let events = crate::video::take_input_events();
    for event in events {
        note_hook_event(&event.event);"""
assert old_head in s
s = s.replace(old_head, new_head, 1)

# diag line
old_dbg = """        debug_log(&event);
        match event.event {"""
new_dbg = """        debug_log(&event);
        crate::diag!("[native] {:?} plane={plane:?}", event.event);
        match event.event {"""
assert old_dbg in s
s = s.replace(old_dbg, new_dbg, 1)

# pump tail: run the fallback
old_tail = """            MouseEvent::Wheel { delta } => wheel(app, ctx, delta),
            MouseEvent::Leave => {}
        }
    }
}
"""
new_tail = """            MouseEvent::Wheel { delta } => wheel(app, ctx, delta),
            MouseEvent::Leave => {}
        }
    }

    poll_fallback(app, ctx);
}
"""
assert old_tail in s
s = s.replace(old_tail, new_tail, 1)

s += '''
/// How long the native hook gets to report a press before the poll takes over.
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
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Counts hook moves that happen while the left button is held.
fn note_hook_event(event: &MouseEvent) {
    let down_now = crate::video::childwin::left_button_down();
    let mut state = lock(poll_state());
    match event {
        MouseEvent::Down { button: 0, .. } => {
            // The hook handled the press; the poll stays out of the way.
            state.synthetic = false;
            state.moves_while_down = 0;
        }
        MouseEvent::Move { .. } if down_now => state.moves_while_down += 1,
        _ => {}
    }
}

/// Drives the scrub gesture from the OS cursor state.
///
/// libmpv recreates its renderer window whenever it re-initialises the video
/// output (first load, resolution or pixel-format change, some seeks), and the
/// window that owns the pointer can change while a drag is running too. Both
/// cases used to swallow presses and moves, which is why dragging only worked
/// "sometimes". Watching the physical button and the cursor position makes the
/// gesture independent of which window currently receives the messages.
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
p.write_text(s, encoding="utf-8")
print("ok")
