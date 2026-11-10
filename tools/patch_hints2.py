import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\bars.rs")
t = p.read_text(encoding="utf-8")

edits = [
    # previous frame
    (
        """            tall(ui, 34.0, &mut |ui| {
                if icon_button(ui, &p, 34.0, false, icons::step_back)
                    .on_hover_text(app.t("上一帧（,）", "Previous frame (,)"))
                    .clicked()
                {
                    app.step_frame(false);
                }
            });""",
        """            tall(ui, 34.0, &mut |ui| {
                let prev = icon_button(ui, &p, 34.0, false, icons::step_back);
                if prev.hovered() {
                    app.hover_hint = Some(app.t("上一帧（,）", "Previous frame (,)"));
                }
                if prev.clicked() {
                    app.step_frame(false);
                }
            });""",
    ),
    # play / pause
    (
        """            tall(ui, 48.0, &mut |ui| {
                if play_button(ui, app)
                    .on_hover_text(app.t("播放 / 暂停（空格）", "Play / pause (Space)"))
                    .clicked()
                {
                    app.toggle_play();
                }
            });""",
        """            tall(ui, 48.0, &mut |ui| {
                let play = play_button(ui, app);
                if play.hovered() {
                    app.hover_hint = Some(app.t("播放 / 暂停（空格）", "Play / pause (Space)"));
                }
                if play.clicked() {
                    app.toggle_play();
                }
            });""",
    ),
    # next frame
    (
        """            tall(ui, 34.0, &mut |ui| {
                if icon_button(ui, &p, 34.0, false, icons::step_forward)
                    .on_hover_text(app.t("下一帧（.）", "Next frame (.)"))
                    .clicked()
                {
                    app.step_frame(true);
                }
            });""",
        """            tall(ui, 34.0, &mut |ui| {
                let next = icon_button(ui, &p, 34.0, false, icons::step_forward);
                if next.hovered() {
                    app.hover_hint = Some(app.t("下一帧（.）", "Next frame (.)"));
                }
                if next.clicked() {
                    app.step_frame(true);
                }
            });""",
    ),
]
for old, new in edits:
    assert old in t, old[:60]
    t = t.replace(old, new, 1)

p.write_text(t, encoding="utf-8")
print("transport buttons patched")
