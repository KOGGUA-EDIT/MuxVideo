import pathlib

root = pathlib.Path(r"D:\app edit\Muxvideo")
mod = root / "src" / "app" / "mod.rs"
bars = root / "src" / "app" / "bars.rs"
stage = root / "src" / "app" / "stage.rs"
inp = root / "src" / "app" / "input.rs"

m = mod.read_text(encoding="utf-8")
if "pub mod diag;" not in m:
    m = m.replace("mod controls;", "mod controls;\npub mod diag;", 1)
assert "pub mod diag;" in m

old = """        self.immersive = on;"""
if "diag!(\"[state] immersive" not in m:
    new = """        self.immersive = on;
        crate::diag!("[state] immersive -> {on} (fullscreen cmd sent)");"""
    assert old in m
    m = m.replace(old, new, 1)

old = """        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }"""
if "begin resize" not in m:
    new = """        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            crate::diag!(
                "[window] begin resize {dir:?} at ({:.0},{:.0}) content={:?}",
                pointer.x,
                pointer.y,
                rect
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }"""
    assert old in m
    m = m.replace(old, new, 1)

anchor = """        // Native window polish: rounded DWM corners normally, no border at all"""
if "last_viewport" not in m:
    new_block = """        // Window geometry, only when it changes: the fastest way to see whether
        // a resize actually happened.
        {
            let area = ctx.input(|i| i.viewport().outer_rect);
            let frame = self.frame_no;
            if self.last_viewport != area {
                self.last_viewport = area;
                crate::diag!("[window] frame={frame} outer_rect={area:?} immersive={}", self.immersive);
            }
        }

"""
    assert anchor in m
    m = m.replace(anchor, new_block + anchor, 1)
    m = m.replace("        frame_immersive: false,\n", "        frame_immersive: false,\n        last_viewport: None,\n", 1)
    m = m.replace(
        "    /// Which `apply_modern_frame` flavour the window currently uses.\n    frame_immersive: bool,\n",
        "    /// Which `apply_modern_frame` flavour the window currently uses.\n    frame_immersive: bool,\n    /// Last known window rectangle, for change logging.\n    last_viewport: Option<egui::Rect>,\n",
        1,
    )
mod.write_text(m, encoding="utf-8")

s = bars.read_text(encoding="utf-8")
s = s.replace(
    """    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);""",
    """    crate::diag!("[window] titlebar drag");
    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);""",
    1,
)
s = s.replace(
    """        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);""",
    """        crate::diag!("[window] chrome drag");
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);""",
)
bars.write_text(s, encoding="utf-8")

i = inp.read_text(encoding="utf-8")
if "[native]" not in i:
    old = """        debug_log(&event);"""
    new = """        debug_log(&event);
        crate::diag!("[native] {:?} plane={plane:?}", event.event);"""
    assert old in i
    i = i.replace(old, new, 1)
inp.write_text(i, encoding="utf-8")

st = stage.read_text(encoding="utf-8")
if "[stage]" not in st:
    old = """        self.grip.place(grip_rect);"""
    new = """        self.grip.place(grip_rect);
        crate::diag!(
            "[stage] a={:?} b={:?} clip_a={:?} clip_b={:?} grip={grip_rect:?}",
            a_rect,
            b_rect,
            a_clip,
            b_clip
        );"""
    assert old in st
    st = st.replace(old, new, 1)
stage.write_text(st, encoding="utf-8")
print("ok")
