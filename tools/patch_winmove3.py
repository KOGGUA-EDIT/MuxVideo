import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\bars.rs")
t = p.read_text(encoding="utf-8")

pairs = [
    ("""    let drag = ui.interact(bar, ui.id().with("titlebar-drag"), Sense::click_and_drag());
    if drag.dragged() {
        crate::diag!("[window] titlebar drag");
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }""",
     """    let drag = ui.interact(bar, ui.id().with("titlebar-drag"), Sense::click_and_drag());
    if drag.drag_started() {
        crate::diag!("[window] titlebar drag");
        app.begin_window_move(ui.ctx());
    }"""),
    ("""    if !app.near_edge(ui.ctx())
        && ui
            .interact(full, ui.id().with("rail-drag"), Sense::click_and_drag())
            .dragged()
    {
        crate::diag!("[window] chrome drag");
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }""",
     """    if !app.near_edge(ui.ctx())
        && ui
            .interact(full, ui.id().with("rail-drag"), Sense::click_and_drag())
            .drag_started()
    {
        crate::diag!("[window] chrome drag");
        app.begin_window_move(ui.ctx());
    }"""),
    ("""    if !app.near_edge(ui.ctx())
        && ui
            .interact(bar_rect, ui.id().with("bar-drag"), Sense::click_and_drag())
            .dragged()
    {
        crate::diag!("[window] chrome drag");
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }""",
     """    if !app.near_edge(ui.ctx())
        && ui
            .interact(bar_rect, ui.id().with("bar-drag"), Sense::click_and_drag())
            .drag_started()
    {
        crate::diag!("[window] chrome drag");
        app.begin_window_move(ui.ctx());
    }"""),
    ("""    if ui
        .interact(
            card,
            ui.id().with("transport-drag"),
            Sense::click_and_drag(),
        )
        .dragged()
    {
        crate::diag!("[window] chrome drag");
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }""",
     """    if ui
        .interact(
            card,
            ui.id().with("transport-drag"),
            Sense::click_and_drag(),
        )
        .drag_started()
    {
        crate::diag!("[window] chrome drag");
        app.begin_window_move(ui.ctx());
    }"""),
]
for old, new in pairs:
    assert old in t, old[:50]
    t = t.replace(old, new, 1)
p.write_text(t, encoding="utf-8")
print("chrome drags now manual")
