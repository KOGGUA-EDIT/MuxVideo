import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\mod.rs")
t = p.read_text(encoding="utf-8")

old = """        self.immersive = on;
        crate::diag!("[state] immersive -> {on} (fullscreen cmd sent)");
        // Borderless fullscreen still leaves the taskbar on top for windows that
        // are not topmost, which shows as a strip along the bottom edge; keep
        // the picture window above it while the chrome is hidden.
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if on {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(true));
        ctx.request_repaint();"""

new = """        self.immersive = on;
        crate::diag!("[state] immersive -> {on} (window geometry switched)");
        // The picture must reach every screen edge. Instead of asking the OS for
        // fullscreen (which re-applies the DWM frame when entered from a
        // maximized window, leaving a groove and corner artefacts), the window
        // is simply resized to its monitor with the decorations already off.
        let ppp = ctx.pixels_per_point();
        if let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE) {
            if on {
                self.saved_outer = ctx.input(|i| {
                    i.viewport()
                        .outer_rect
                        .map(|r| (r.min.x, r.min.y, r.width(), r.height()))
                });
                window::cover_monitor(hwnd);
            } else if let Some((x, y, w, h)) = self.saved_outer.take() {
                window::restore_rect(
                    hwnd,
                    (x * ppp).round() as i32,
                    (y * ppp).round() as i32,
                    (w * ppp).round() as i32,
                    (h * ppp).round() as i32,
                );
            }
            window::apply_modern_frame(hwnd, on);
            self.frame_immersive = on;
            self.frame_dressed = true;
        }
        // Borderless fullscreen still leaves the taskbar on top for windows that
        // are not topmost, which shows as a strip along the bottom edge; keep
        // the picture window above it while the chrome is hidden.
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if on {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(true));
        ctx.request_repaint();"""
assert old in t
t = t.replace(old, new, 1)

t = t.replace(
    "    frame_immersive: bool,",
    "    frame_immersive: bool,\n    /// Outer rectangle (in points) to restore when leaving immersive playback.\n    saved_outer: Option<(f32, f32, f32, f32)>,",
    1,
)
t = t.replace(
    "            frame_immersive: false,",
    "            frame_immersive: false,\n            saved_outer: None,",
    1,
)
p.write_text(t, encoding="utf-8")
print("immersive reworked")
