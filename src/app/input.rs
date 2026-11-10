//! Gestures on the native video surfaces.
//!
//! The planes are real Win32 windows, so egui never sees these clicks; the
//! input hook in [`crate::video`] queues them and this module turns the raw
//! stream into the interactions a review player is expected to have:
//!
//! * click — play / pause
//! * drag — scrub the playhead frame by frame (RV Player style: the picture
//!   follows the cursor, one pixel of travel is one frame)
//! * right drag in wipe mode — move the split line
//! * middle drag — pan the picture
//! * wheel — volume, `Ctrl`+wheel — zoom
//! * double click — fullscreen

use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::video::MouseEvent;

use super::MuxApp;
use super::stage::Plane;
use super::state::{CompareMode, Toast};

/// Longer than this and a press is not a click any more.
const CLICK_MAX_MS: u64 = 400;
/// Movement (physical pixels) that turns a press into a drag.
const DRAG_SLOP: i32 = 5;
/// Our own double-click window. The system one is ~500 ms, which turned quick
/// play/pause toggles into fullscreen toggles.
const DOUBLE_CLICK_MS: u64 = 250;
/// Two clicks further apart than this (physical pixels) are not a double click.
const DOUBLE_CLICK_PX: i32 = 12;
/// Volume per wheel notch, in mpv volume units.
const VOLUME_STEP: f64 = 4.0;
/// Zoom per wheel notch.
const ZOOM_STEP: f64 = 0.10;
const ZOOM_MIN: f64 = -2.0;
const ZOOM_MAX: f64 = 3.0;

/// How close (in pixels) the pointer must be to the wipe divider to grab it.
const HANDLE_GRAB: f32 = 22.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// Left press that drags the playhead frame by frame (RV Player style).
    Scrub,
    /// Right press that moves the wipe split line.
    Wipe,
    /// Middle press that pans the picture.
    Pan,
}

pub(crate) struct Drag {
    plane: Plane,
    mode: Mode,
    start: (i32, i32),
    last: (i32, i32),
    /// Playhead when the press happened, in seconds.
    frame0: f64,
    /// Last seek target, so we do not flood mpv with duplicate seeks.
    last_target: f64,
    moved: bool,
    began: Instant,
}

/// Per-frame entry point: drains the native input queue into player actions.
pub(super) fn pump(app: &mut MuxApp, ctx: &egui::Context) {
    let events = crate::video::take_input_events();
    for event in events {
        note_hook_event(&event.event);
        // The wipe grip is its own native window: translate its coordinates
        // into plane A so the seam drag maths stays the same.
        let mapped = if app.media.plane_for_hwnd(event.hwnd).is_none()
            && app.media.grip_hwnd() != 0
            && event.hwnd == app.media.grip_hwnd()
        {
            match event.event {
                MouseEvent::Down { x, y, button } => {
                    crate::video::childwin::map_point(event.hwnd, app.media.plane_a_hwnd(), x, y)
                        .map(|(x, y)| MouseEvent::Down { x, y, button })
                }
                MouseEvent::Up { x, y, button } => {
                    crate::video::childwin::map_point(event.hwnd, app.media.plane_a_hwnd(), x, y)
                        .map(|(x, y)| MouseEvent::Up { x, y, button })
                }
                MouseEvent::Move { x, y } => {
                    crate::video::childwin::map_point(event.hwnd, app.media.plane_a_hwnd(), x, y)
                        .map(|(x, y)| MouseEvent::Move { x, y })
                }
                other => Some(other),
            }
        } else {
            None
        };
        let event = match mapped {
            Some(inner) => crate::video::InputEvent {
                hwnd: app.media.plane_a_hwnd(),
                event: inner,
            },
            None => event,
        };
        let Some(plane) = app.media.plane_for_hwnd(event.hwnd) else {
            // The stage container and the toplevel get the bubbled-up copies.
            crate::diag!("[native] unclaimed {:?} hwnd={}", event.event, event.hwnd);
            continue;
        };
        debug_log(&event);
        crate::diag!("[native] {:?} plane={plane:?}", event.event);
        match event.event {
            MouseEvent::Down { x, y, button } => press(app, plane, x, y, button),
            MouseEvent::Move { x, y } => drag_to(app, ctx, x, y),
            MouseEvent::Up { .. } => release(app, ctx),
            // The OS double-click uses the system interval (0.5 s by default),
            // which is long enough that quick play/pause toggles landed here.
            // Clicks are paired by `release` instead, with a tighter window.
            MouseEvent::DoubleClick => {}
            MouseEvent::Wheel { delta } => wheel(app, ctx, delta),
            MouseEvent::Leave => {}
        }
    }

    poll_fallback(app, ctx);
}

fn press(app: &mut MuxApp, plane: Plane, x: i32, y: i32, button: u8) {
    // A left press on the wipe handle grabs the divider; everywhere else the
    // left button scrubs.
    let on_handle = button == 0
        && app.settings.mode == CompareMode::Wipe
        && app.media.file_b.is_some()
        && seam_near(app, plane, x, y);
    let mode = match button {
        0 if on_handle => Mode::Wipe,
        0 => Mode::Scrub,
        1 if app.settings.mode == CompareMode::Wipe => Mode::Wipe,
        2 => Mode::Pan,
        _ => return,
    };
    let frame0 = app.time_pos();
    crate::diag!("[gesture] press mode={mode:?} plane={plane:?} at=({x},{y}) t={frame0:.3}");
    // Do not touch playback here: a plain click must keep its play/pause
    // meaning, so the picture is only parked once a real scrub starts.
    app.gesture = Some(Drag {
        plane,
        mode,
        start: (x, y),
        last: (x, y),
        frame0,
        last_target: frame0,
        moved: false,
        began: Instant::now(),
    });
}

/// Whether the pointer is close enough to the wipe divider to grab it.
fn seam_near(app: &MuxApp, plane: Plane, x: i32, y: i32) -> bool {
    let Some((w, h)) = app.media.plane_size(plane) else {
        return false;
    };
    let frac = app.settings.wipe.clamp(0.0, 1.0);
    match app.settings.axis {
        crate::app::state::SplitAxis::Horizontal => {
            (f32::from(x as i16) - frac * w as f32).abs() <= HANDLE_GRAB
        }
        crate::app::state::SplitAxis::Vertical => {
            (f32::from(y as i16) - frac * h as f32).abs() <= HANDLE_GRAB
        }
    }
}

/// Debug tap: enabled only when an `input-debug.flag` file sits next to the exe.
fn debug_enabled() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    dir.join("input-debug.flag").exists().then_some(dir)
}

fn debug_note(line: &str) {
    use std::io::Write;
    let Some(dir) = debug_enabled() else {
        return;
    };
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("input-debug.log"))
    {
        let _ = writeln!(f, "{line}");
    }
}

fn debug_log(event: &crate::video::InputEvent) {
    let line = match event.event {
        MouseEvent::Down { x, y, button } => format!("down {x} {y} b{button}"),
        MouseEvent::Move { x, y } => format!("move {x} {y}"),
        MouseEvent::Up { x, y, button } => format!("up {x} {y} b{button}"),
        MouseEvent::DoubleClick => "double".to_string(),
        MouseEvent::Wheel { delta } => format!("wheel {delta}"),
        MouseEvent::Leave => "leave".to_string(),
    };
    debug_note(&line);
}

fn drag_to(app: &mut MuxApp, ctx: &egui::Context, x: i32, y: i32) {
    let fps = app.fps();
    let duration = app.duration();
    // Park playback before borrowing the gesture: a plain click never reaches
    // this point, so play/pause keeps its meaning.
    if app.gesture.as_ref().is_some_and(|d| d.mode == Mode::Scrub)
        && app.is_playing()
        && app
            .gesture
            .as_ref()
            .is_some_and(|d| (x - d.start.0).abs() > DRAG_SLOP || (y - d.start.1).abs() > DRAG_SLOP)
    {
        app.set_paused(true);
    }
    let Some(drag) = app.gesture.as_mut() else {
        return;
    };
    let (dx, dy) = (x - drag.last.0, y - drag.last.1);
    drag.last = (x, y);
    let (tx, ty) = (x - drag.start.0, y - drag.start.1);
    if !drag.moved && (tx.abs() > DRAG_SLOP || ty.abs() > DRAG_SLOP) {
        drag.moved = true;
    }
    if !drag.moved {
        return;
    }
    match drag.mode {
        Mode::Scrub => {
            // One pixel of travel per frame (user tunable on the settings
            // page), so the picture follows the pointer frame by frame.
            let plane = drag.plane;
            let frame0 = drag.frame0;
            let last_target = drag.last_target;
            // Pixels per frame from the settings page (1 = follows the pointer
            // frame for frame); Shift always gives the slow, precise mode.
            let fine = ctx.input(|i| i.modifiers.shift);
            let ppf: f64 = if fine {
                10.0
            } else {
                app.settings.scrub_ppf.clamp(
                    *super::state::SCRUB_PPF_RANGE.start(),
                    *super::state::SCRUB_PPF_RANGE.end(),
                )
            };
            let frames = f64::from(tx) / ppf.max(0.01);
            let mut target = frame0 + frames / fps;
            if duration > 0.0 {
                target = target.clamp(0.0, duration);
            } else {
                target = target.max(0.0);
            }
            debug_note(&format!(
                "scrub tx={tx} ppf={ppf:.2} fps={fps:.3} frame0={frame0:.3} target={target:.3} dur={duration:.3}"
            ));
            if (target - last_target).abs() >= 0.25 / fps {
                app.scrub = Some(target);
                app.media.seek_all(target);
                let total = crate::app::state::frame_at(duration, fps);
                app.toast = Some(crate::app::state::Toast::info(format!(
                    "{} {} / {}",
                    app.t("帧", "Frame"),
                    crate::app::state::frame_at(target, fps),
                    total
                )));
                if let Some(drag) = app.gesture.as_mut() {
                    drag.last_target = target;
                }
            }
            let _ = plane;
        }
        Mode::Wipe => {
            if let Some((w, h)) = app.media.plane_size(drag.plane) {
                let frac = match app.settings.axis {
                    crate::app::state::SplitAxis::Horizontal => x as f32 / w.max(1) as f32,
                    crate::app::state::SplitAxis::Vertical => y as f32 / h.max(1) as f32,
                };
                app.settings.wipe = frac.clamp(0.02, 0.98);
                app.settings_dirty = true;
            }
        }
        Mode::Pan => {
            if let Some((w, h)) = app.media.plane_size(drag.plane) {
                let sx = dx as f64 / w.max(1) as f64;
                let sy = dy as f64 / h.max(1) as f64;
                app.pan = (
                    (app.pan.0 + sx).clamp(-2.0, 2.0),
                    (app.pan.1 + sy).clamp(-2.0, 2.0),
                );
                app.view_dirty();
            }
        }
    }
}

fn release(app: &mut MuxApp, ctx: &egui::Context) {
    let Some(drag) = app.gesture.take() else {
        return;
    };
    let quick = drag.began.elapsed() < Duration::from_millis(CLICK_MAX_MS);
    if drag.mode == Mode::Scrub {
        // Scrubbing leaves the player parked on the frame under the cursor.
        app.scrub = None;
        if drag.moved || !quick {
            return;
        }
        let now = Instant::now();
        let is_double = app.last_click.is_some_and(|(at, x, y)| {
            now.duration_since(at) < Duration::from_millis(DOUBLE_CLICK_MS)
                && (x - drag.start.0).abs() <= DOUBLE_CLICK_PX
                && (y - drag.start.1).abs() <= DOUBLE_CLICK_PX
        });
        if is_double {
            // Undo the toggle from the first click so a double click only
            // changes the view, then go immersive.
            app.last_click = None;
            app.toggle_play();
            toggle_fullscreen(app, ctx);
        } else {
            app.last_click = Some((now, drag.start.0, drag.start.1));
            app.toggle_play();
        }
    }
}

fn wheel(app: &mut MuxApp, ctx: &egui::Context, delta: i32) {
    let notches = f64::from(delta) / 120.0;
    if notches == 0.0 {
        return;
    }
    let ctrl = ctx.input(|i| i.modifiers.ctrl);
    if ctrl {
        app.zoom = (app.zoom + notches * ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
        app.view_dirty();
        app.toast = Some(Toast::info(format!(
            "{} {:+.0}%",
            app.t("缩放", "Zoom"),
            app.zoom * 100.0
        )));
        return;
    }
    let next = (app.settings.volume + notches * VOLUME_STEP).clamp(0.0, 130.0);
    if (next - app.settings.volume).abs() < 0.01 {
        return;
    }
    app.settings.volume = next;
    app.settings.muted = false;
    app.apply_volume();
    app.toast = Some(Toast::info(format!(
        "{} {:.0}%",
        app.t("音量", "Volume"),
        next
    )));
}

fn toggle_fullscreen(app: &mut MuxApp, ctx: &egui::Context) {
    app.toggle_immersive(ctx);
}

/// Resets pan/zoom; also reachable from the 0 key.
pub(crate) fn reset_view(app: &mut MuxApp) {
    app.pan = (0.0, 0.0);
    app.zoom = 0.0;
    app.view_dirty();
}

/// Saves the current frame into the user's Pictures folder.
pub(crate) fn snapshot(app: &mut MuxApp) {
    use super::state::{Toast, file_stamp, frame_at, pictures_dir};
    let Some(player) = app.active_player() else {
        app.toast = Some(Toast::error(
            app.t("没有可截图的画面", "Nothing to capture"),
        ));
        return;
    };
    let frame = frame_at(app.time_pos(), app.fps());
    let dir = pictures_dir();
    let name = format!(
        "MuxVideo_{}_{}{frame:05}.png",
        file_stamp(),
        app.t("帧", "frame")
    );
    let path = dir.join(&name);
    let path_str = path.to_string_lossy().to_string();
    if player.screenshot_to_file(&path_str) {
        app.toast = Some(Toast::info(format!("{}{name}", app.t("已保存 ", "Saved "))));
    } else {
        app.toast = Some(Toast::error(app.t("截图失败", "Screenshot failed")));
    }
}

/// How long the native hook gets to report a press before the poll takes over.
const POLL_GRACE: Duration = Duration::from_millis(30);

/// Extra grace before a silent drag (press seen, moves missing) is adopted.
const POLL_ADOPT: Duration = Duration::from_millis(120);
/// A gesture younger than this can still be live; older ones are leftovers.
const POLL_STALE: Duration = Duration::from_secs(3);

/// State for the cursor-polling fallback.
struct PollState {
    /// Whether the message hook reported the press of the current cycle.
    hook_press: bool,
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
            hook_press: false,
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
    if matches!(event, MouseEvent::Down { .. }) {
        lock(poll_state()).hook_press = true;
    }
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
/// Replays a click that only the polling loop saw: press + release in place,
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
    {
        let s = lock(poll_state());
        crate::diag!(
            "[poll] synthetic tap ({cx},{cy}) hook_press={} synthetic={} button_down={}",
            s.hook_press,
            s.synthetic,
            s.button_down
        );
    }
    press(app, plane, cx, cy, 0);
    release(app, ctx);
    ctx.request_repaint();
}

fn poll_fallback(app: &mut MuxApp, ctx: &egui::Context) {
    // A gesture already in flight is ours no matter which window sits under the
    // pointer: drive it from the cursor, because the child window that owns the
    // capture can be recreated at any moment (libmpv rebuilds its render window
    // whenever a file is swapped in) and then WM_MOUSEMOVE simply never arrives.
    if app.gesture.is_some() {
        let (down, _) = crate::video::childwin::left_button_state();
        if down
            && let Some((sx, sy)) = crate::video::childwin::cursor_pos()
            && let Some((plane, cx, cy)) = app.media.plane_at_screen(sx, sy)
            && app.gesture.as_ref().is_some_and(|d| d.plane == plane)
        {
            drag_to(app, ctx, cx, cy);
            ctx.request_repaint();
            return;
        }
    }

    // Only trust the poll while the player owns the foreground: otherwise a
    // click on whatever window covers the picture would scrub it.
    // The only trustworthy signal: the window under the pointer belongs to
    // this process. A window can be "foreground" while another one covers it,
    // so focus alone is not enough.
    let owns_pointer = crate::video::childwin::cursor_pos()
        .is_some_and(|(sx, sy)| crate::video::childwin::hit_test_owns(sx, sy));
    if !owns_pointer {
        if let Some((sx, sy)) = crate::video::childwin::cursor_pos() {
            let (pid, class) = crate::video::childwin::hit_test_debug(sx, sy);
            crate::diag!(
                "[poll] skip own={} focused={} hit_pid={pid} class={class}",
                owns_pointer,
                app.media.app_is_foreground()
            );
        }
        let mut state = lock(poll_state());
        state.button_down = false;
        state.synthetic = false;
        return;
    }
    let (down, tapped) = crate::video::childwin::left_button_state();
    let now = Instant::now();
    let mut state = lock(poll_state());

    if down && !state.button_down {
        state.button_down = true;
        if app
            .gesture
            .as_ref()
            .is_some_and(|d| d.began.elapsed() > POLL_STALE)
        {
            crate::diag!("[poll] dropping stale gesture");
            app.gesture = None;
            app.scrub = None;
        }
        state.deadline = now + POLL_GRACE;
        state.moves_while_down = 0;
        state.synthetic = false;
    } else if !down && state.button_down {
        state.button_down = false;
        let hook_press = std::mem::replace(&mut state.hook_press, false);
        let synthetic = std::mem::replace(&mut state.synthetic, false);
        // `synthetic_tap` locks the same state: never hold the guard here.
        drop(state);
        // A release the hook never reported would otherwise leave the gesture
        // stuck forever.
        if app.gesture.is_some() {
            release(app, ctx);
            ctx.request_repaint();
        } else if !synthetic && !hook_press {
            // The poll saw a press but the gesture never started (released
            // inside the grace window): treat it as a plain click.
            synthetic_tap(app, ctx);
        }
        return;
    }

    if !down {
        // A tap that started and ended between two polls still counts as a
        // click, so play/pause keeps working when the hooks missed the press.
        let hook_press = std::mem::replace(&mut state.hook_press, false);
        drop(state);
        if tapped && app.gesture.is_none() && !hook_press {
            synthetic_tap(app, ctx);
        }
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
            crate::diag!("[poll] synthetic press ({cx},{cy})");
            press(app, plane, cx, cy, 0);
            ctx.request_repaint();
        }
        return;
    }

    // Press seen by the hook, but it stopped reporting movement: take over.
    if !state.synthetic && state.moves_while_down == 0 && now >= state.deadline + POLL_ADOPT {
        state.synthetic = true;
        crate::diag!("[poll] adopt stalled gesture");
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
