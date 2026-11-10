import pathlib, re

root = pathlib.Path(r"D:\app edit\Muxvideo")

# ---------------------------------------------------------------- childwin.rs
cw = root / "src/video/childwin.rs"
t = cw.read_text(encoding="utf-8")
if "pub fn set_window_rect" not in t:
    t += '''
/// Move and resize our own top-level window without going through the
/// activation-dependent modal move/resize loops.
pub fn set_window_rect(hwnd: i64, x: i32, y: i32, w: i32, h: i32) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
    };

    if hwnd == 0 {
        return;
    }
    // SAFETY: our own window; the rectangle comes from a tracked drag.
    unsafe {
        let _ = SetWindowPos(
            hwnd as HWND,
            std::ptr::null_mut(),
            x,
            y,
            w.max(320),
            h.max(200),
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}
'''
    cw.write_text(t, encoding="utf-8")

# ---------------------------------------------------------------- mod.rs
m = root / "src/app/mod.rs"
t = m.read_text(encoding="utf-8")

t = t.replace(
    "    /// Hint shown in the transport bar for the widget under the pointer.",
    "    /// Manual window move / resize in progress (see `begin_window_move`).\n    window_move: Option<WindowMove>,\n    /// Last completed click in the picture, for our own double-click window.\n    last_click: Option<(std::time::Instant, i32, i32)>,\n    /// Hint shown in the transport bar for the widget under the pointer.",
    1,
)
t = t.replace(
    "            hover_hint: None,",
    "            window_move: None,\n            last_click: None,\n            hover_hint: None,",
    1,
)

# struct + helpers right before `impl eframe::App` marker: insert before "impl eframe::App for MuxApp"
marker = "impl eframe::App for MuxApp {"
assert marker in t
helpers = '''/// A window move or resize driven by us instead of the OS.
///
/// `ViewportCommand::StartDrag` / `BeginResize` hand the gesture to the Windows
/// modal loops, which silently do nothing while our window is not the
/// foreground window — that is why dragging "worked, then stopped working"
/// after the window had been fullscreen once. Tracking the rectangle here and
/// applying it with `SetWindowPos` has no such dependency.
pub(super) struct WindowMove {
    /// `true` to move the window, `false` to resize it.
    moving: bool,
    /// Which borders are being dragged: west, east, north, south.
    edges: (bool, bool, bool, bool),
    start_cursor: (i32, i32),
    start_rect: (i32, i32, i32, i32),
}

const MIN_WINDOW_W: i32 = 640;
const MIN_WINDOW_H: i32 = 400;

'''
t = t.replace(marker, helpers + marker, 1)

# methods: insert before "    // ── per-frame sync ──"
anchor = "    // ── per-frame sync ────────────────────────────────────────────────────"
assert anchor in t
methods = '''    /// Start a manual window move from a chrome area.
    pub(super) fn begin_window_move(&mut self, ctx: &egui::Context) {
        if self.immersive || self.window_move.is_some() {
            return;
        }
        let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE)
        else {
            return;
        };
        let (Some(rect), Some(cursor)) = (
            crate::video::childwin::window_rect(hwnd),
            crate::video::childwin::cursor_pos(),
        ) else {
            return;
        };
        self.window_move = Some(WindowMove {
            moving: true,
            edges: (false, false, false, false),
            start_cursor: cursor,
            start_rect: rect,
        });
        ctx.request_repaint();
    }

    /// Apply the tracked move / resize until the button is released.
    fn pump_window_move(&mut self, ctx: &egui::Context) {
        let Some(state) = &self.window_move else {
            return;
        };
        if !crate::video::childwin::left_button_down() {
            self.window_move = None;
            return;
        }
        let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE)
        else {
            self.window_move = None;
            return;
        };
        let Some((cx, cy)) = crate::video::childwin::cursor_pos() else {
            return;
        };
        let dx = cx - state.start_cursor.0;
        let dy = cy - state.start_cursor.1;
        let (x, y, w, h) = state.start_rect;
        let (mut nx, mut ny, mut nw, mut nh) = (x, y, w, h);
        if state.moving {
            nx = x + dx;
            ny = y + dy;
        } else {
            let (west, east, north, south) = state.edges;
            if west {
                nx = x + dx;
                nw = w - dx;
            }
            if east {
                nw = w + dx;
            }
            if north {
                ny = y + dy;
                nh = h - dy;
            }
            if south {
                nh = h + dy;
            }
            if nw < MIN_WINDOW_W {
                nw = MIN_WINDOW_W;
                if west {
                    nx = x + w - MIN_WINDOW_W;
                }
            }
            if nh < MIN_WINDOW_H {
                nh = MIN_WINDOW_H;
                if north {
                    ny = y + h - MIN_WINDOW_H;
                }
            }
        }
        crate::video::childwin::set_window_rect(hwnd, nx, ny, nw, nh);
        ctx.request_repaint();
    }

'''
t = t.replace(anchor, methods + anchor, 1)
m.write_text(t, encoding="utf-8")
print("mod.rs: manual window move added")
