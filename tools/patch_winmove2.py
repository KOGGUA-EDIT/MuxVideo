import pathlib

root = pathlib.Path(r"D:\app edit\Muxvideo")
m = root / "src/app/mod.rs"
t = m.read_text(encoding="utf-8")

# 1) resize_borders: start a manual resize instead of ViewportCommand::BeginResize
old_sig = "    fn resize_borders(&self, ctx: &egui::Context) {"
new_sig = "    fn resize_borders(&mut self, ctx: &egui::Context) {"
assert old_sig in t
t = t.replace(old_sig, new_sig, 1)

old_cmd = """        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            crate::diag!(
                "[window] begin resize {dir:?} at ({:.0},{:.0}) content={:?}",
                pointer.x,
                pointer.y,
                rect
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }"""
new_cmd = """        ctx.set_cursor_icon(cursor);
        let _ = dir;
        if self.immersive || self.window_move.is_some() {
            return;
        }
        if ctx.input(|i| i.pointer.primary_pressed()) {
            let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE)
            else {
                return;
            };
            let (Some(win), Some(cursor_pos)) = (
                crate::video::childwin::window_rect(hwnd),
                crate::video::childwin::cursor_pos(),
            ) else {
                return;
            };
            crate::diag!(
                "[window] begin resize w={west} e={east} n={north} s={south} win={win:?}"
            );
            self.window_move = Some(WindowMove {
                moving: false,
                edges: (west, east, north, south),
                start_cursor: cursor_pos,
                start_rect: win,
            });
        }
    }"""
assert old_cmd in t
t = t.replace(old_cmd, new_cmd, 1)

# 2) call pump_window_move each frame, right after resize_borders()
old_call = "        self.resize_borders(ctx);"
new_call = "        self.resize_borders(ctx);\n        self.pump_window_move(ctx);"
assert old_call in t
t = t.replace(old_call, new_call, 1)
m.write_text(t, encoding="utf-8")
print("resize switched to manual")
