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
/// Volume per wheel notch, in mpv volume units.
const VOLUME_STEP: f64 = 4.0;
/// Zoom per wheel notch.
const ZOOM_STEP: f64 = 0.10;
const ZOOM_MIN: f64 = -2.0;
const ZOOM_MAX: f64 = 3.0;

#[derive(Clone, Copy, PartialEq, Eq)]
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
    if events.is_empty() {
        return;
    }
    for event in events {
        let Some(plane) = app.media.plane_for_hwnd(event.hwnd) else {
            // The stage container and the toplevel get the bubbled-up copies.
            continue;
        };
        match event.event {
            MouseEvent::Down { x, y, button } => press(app, plane, x, y, button),
            MouseEvent::Move { x, y } => drag_to(app, ctx, x, y),
            MouseEvent::Up { .. } => release(app),
            MouseEvent::DoubleClick => {
                app.gesture = None;
                app.suppress_click = Some(Instant::now());
                toggle_fullscreen(app, ctx);
            }
            MouseEvent::Wheel { delta } => wheel(app, ctx, delta),
            MouseEvent::Leave => {}
        }
    }
}

fn press(app: &mut MuxApp, plane: Plane, x: i32, y: i32, button: u8) {
    let mode = match button {
        0 => Mode::Scrub,
        1 if app.settings.mode == CompareMode::Wipe => Mode::Wipe,
        2 => Mode::Pan,
        _ => return,
    };
    let frame0 = app.time_pos();
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

fn drag_to(app: &mut MuxApp, _ctx: &egui::Context, x: i32, y: i32) {
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
            // Travel per frame is user configurable (1 … 100 px).
            let ppf = app.settings.scrub_ppf.clamp(
                *super::state::SCRUB_PPF_RANGE.start(),
                *super::state::SCRUB_PPF_RANGE.end(),
            );
            let frames = f64::from(tx) / ppf.max(0.01);
            let mut target = drag.frame0 + frames / fps;
            if duration > 0.0 {
                target = target.clamp(0.0, duration);
            } else {
                target = target.max(0.0);
            }
            // Re-issuing a seek every mouse move starves the decoder; once per
            // half frame is plenty for the eye and cheap for mpv.
            if (target - drag.last_target).abs() >= 0.5 / fps {
                drag.last_target = target;
                let target = target;
                app.scrub = Some(target);
                app.media.seek_all(target);
                let total = crate::app::state::frame_at(duration, fps);
                app.toast = Some(crate::app::state::Toast::info(format!(
                    "帧 {} / {}",
                    crate::app::state::frame_at(target, fps),
                    total
                )));
            }
        }
        Mode::Wipe => {
            if let Some((w, _)) = app.media.plane_size(drag.plane) {
                let frac = (x as f32 / w.max(1) as f32).clamp(0.02, 0.98);
                app.settings.wipe = frac;
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

fn release(app: &mut MuxApp) {
    let Some(drag) = app.gesture.take() else {
        return;
    };
    let quick = drag.began.elapsed() < Duration::from_millis(CLICK_MAX_MS);
    let suppressed = app
        .suppress_click
        .is_some_and(|at| at.elapsed() < Duration::from_millis(CLICK_MAX_MS));
    if drag.mode == Mode::Scrub {
        // Scrubbing leaves the player parked on the frame under the cursor.
        app.scrub = None;
        if !drag.moved && quick && !suppressed {
            app.suppress_click = None;
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
        app.toast = Some(Toast::info(format!("缩放 {:+.0}%", app.zoom * 100.0)));
        return;
    }
    let next = (app.settings.volume + notches * VOLUME_STEP).clamp(0.0, 130.0);
    if (next - app.settings.volume).abs() < 0.01 {
        return;
    }
    app.settings.volume = next;
    app.settings.muted = false;
    app.apply_volume();
    app.toast = Some(Toast::info(format!("音量 {:.0}%", next)));
}

fn toggle_fullscreen(app: &mut MuxApp, ctx: &egui::Context) {
    let _ = app;
    let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
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
        app.toast = Some(Toast::error("没有可截图的画面"));
        return;
    };
    let frame = frame_at(app.time_pos(), app.fps());
    let dir = pictures_dir();
    let name = format!("MuxVideo_{}_帧{frame:05}.png", file_stamp());
    let path = dir.join(&name);
    let path_str = path.to_string_lossy().to_string();
    if player.screenshot_to_file(&path_str) {
        app.toast = Some(Toast::info(format!("已保存 {name}")));
    } else {
        app.toast = Some(Toast::error("截图失败"));
    }
}
