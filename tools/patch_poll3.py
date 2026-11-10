import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
s = p.read_text(encoding="utf-8")

# --- 1. pump: remember the last real button event and run the poll fallback.
old_head = """pub(super) fn pump(app: &mut MuxApp, ctx: &egui::Context) {
    let events = crate::video::take_input_events();
    if events.is_empty() {
        return;
    }
    for event in events {"""

new_head = """pub(super) fn pump(app: &mut MuxApp, ctx: &egui::Context) {
    let events = crate::video::take_input_events();
    let now = Instant::now();
    for event in events {
        // Remember that the native hook is alive: the cursor-polling fallback
        // below only steps in when no hook press showed up for a while.
        if matches!(
            event.event,
            MouseEvent::Down { button: 0, .. } | MouseEvent::Up { button: 0, .. }
        ) {
            let mut state = lock(poll_state());
            state.last_hook_button = now;
            state.synthetic = false;
        }"""

assert old_head in s
s = s.replace(old_head, new_head, 1)

# --- 2. tail of pump: call the fallback.
old_tail = """        MouseEvent::Wheel { delta } => wheel(app, ctx, delta),
        MouseEvent::Leave => {}
    }
    }
}"""
new_tail = """        MouseEvent::Wheel { delta } => wheel(app, ctx, delta),
        MouseEvent::Leave => {}
    }
    }

    poll_fallback(app, ctx);
}

/// Padding on the native hook before the polling fallback takes over.
const POLL_GRACE: Duration = Duration::from_millis(80);

/// State for the cursor-polling fallback.
struct PollState {
    /// When the native hook last reported a left button event.
    last_hook_button: Instant,
    /// Whether the current gesture was synthesised from the cursor poll.
    synthetic: bool,
}

fn poll_state() -> &'static Mutex<PollState> {
    static STATE: OnceLock<Mutex<PollState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(PollState {
            last_hook_button: Instant::now() - Duration::from_secs(60),
            synthetic: false,
        })
    })
}

fn lock<T>(mutex: &'static Mutex<T>) -> MutexGuard<'static, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Drives the scrub gesture from the OS cursor state.
///
/// libmpv recreates its renderer window whenever it re-initialises the video
/// output (first load, resolution or pixel-format changes, some seeks). There is
/// always a window in which that happens *after* our subclass is installed on
/// the previous one, and any press landing in that gap was lost — which is why
/// dragging used to work only "sometimes". Polling the physical button and the
/// cursor position makes the gesture independent of which window owns the
/// pointer.
fn poll_fallback(app: &mut MuxApp, ctx: &egui::Context) {
    let down = crate::video::childwin::left_button_down();
    let mut state = lock(poll_state());

    if down {
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
        } else if state.synthetic {
            if let Some((sx, sy)) = cursor
                && let Some((plane, cx, cy)) = app.media.plane_at_screen(sx, sy)
                && app.gesture.as_ref().is_some_and(|d| d.plane == plane)
            {
                drag_to(app, ctx, cx, cy);
            }
            ctx.request_repaint();
        }
    } else if state.synthetic {
        state.synthetic = false;
        release(app);
        ctx.request_repaint();
    }
}"""
assert old_tail in s
s = s.replace(old_tail, new_tail, 1)

# --- 3. imports for the poll state.
if "use std::sync::{Mutex, MutexGuard, OnceLock};" not in s:
    s = s.replace(
        "use std::time::{Duration, Instant};",
        "use std::sync::{Mutex, MutexGuard, OnceLock};\nuse std::time::{Duration, Instant};",
        1,
    )

p.write_text(s, encoding="utf-8")
print("ok")
