import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\mod.rs")
t = p.read_text(encoding="utf-8")

t = t.replace(
    "    /// Outer rectangle (in points) to restore when leaving immersive playback.\n    saved_outer: Option<(f32, f32, f32, f32)>,",
    "    /// Outer rectangle (in points) to restore when leaving immersive playback.\n    saved_outer: Option<(f32, f32, f32, f32)>,\n    /// Whether the window was maximized before going immersive.\n    saved_maximized: bool,",
    1,
)
t = t.replace(
    "            saved_outer: None,",
    "            saved_outer: None,\n            saved_maximized: false,",
    1,
)

old = """                self.saved_outer = ctx.input(|i| {
                    i.viewport()
                        .outer_rect
                        .map(|r| (r.min.x, r.min.y, r.width(), r.height()))
                });
                window::cover_monitor(hwnd);"""
new = """                self.saved_outer = ctx.input(|i| {
                    i.viewport()
                        .outer_rect
                        .map(|r| (r.min.x, r.min.y, r.width(), r.height()))
                });
                self.saved_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                window::cover_monitor(hwnd);"""
assert old in t
t = t.replace(old, new, 1)

old_exit = """            } else if let Some((x, y, w, h)) = self.saved_outer.take() {
                window::restore_rect(
                    hwnd,
                    (x * ppp).round() as i32,
                    (y * ppp).round() as i32,
                    (w * ppp).round() as i32,
                    (h * ppp).round() as i32,
                );
            }"""
new_exit = """            } else if self.saved_maximized {
                // A maximized window being "restored" must stay maximized, or it
                // ends up slightly larger than the work area in the normal state.
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                self.saved_outer = None;
            } else if let Some((x, y, w, h)) = self.saved_outer.take() {
                window::restore_rect(
                    hwnd,
                    (x * ppp).round() as i32,
                    (y * ppp).round() as i32,
                    (w * ppp).round() as i32,
                    (h * ppp).round() as i32,
                );
            }"""
assert old_exit in t
t = t.replace(old_exit, new_exit, 1)
p.write_text(t, encoding="utf-8")
print("maximized restore added")
