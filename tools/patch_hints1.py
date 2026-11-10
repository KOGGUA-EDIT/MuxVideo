import pathlib, re

root = pathlib.Path(r"D:\app edit\Muxvideo")
bars = root / "src/app/bars.rs"
mod_ = root / "src/app/mod.rs"

# ---------------------------------------------------------------- mod.rs
t = mod_.read_text(encoding="utf-8")
if "hover_hint" not in t:
    t = t.replace(
        "    frame_immersive: bool,",
        "    frame_immersive: bool,\n    /// Hint shown in the transport bar for the widget under the pointer.\n    /// Tooltips cannot be used next to the stage: the native video planes sit\n    /// above egui and swallow anything drawn over the picture.\n    pub(super) hover_hint: Option<&'static str>,",
        1,
    )
    t = t.replace(
        "            frame_immersive: false,",
        "            frame_immersive: false,\n            hover_hint: None,",
        1,
    )
    t = t.replace(
        "        self.frame_no = self.frame_no.wrapping_add(1);",
        "        self.frame_no = self.frame_no.wrapping_add(1);\n        self.hover_hint = None;",
        1,
    )
mod_.write_text(t, encoding="utf-8")

# ---------------------------------------------------------------- bars.rs
t = bars.read_text(encoding="utf-8")

old_cam = """                if icon_button(ui, &p, 34.0, false, icons::camera)
                    .on_hover_text(app.t("截图：把当前帧存到「图片」文件夹（S）", "Save the current frame to Pictures (S)"))
                    .clicked()
                {
                    super::input::snapshot(app);
                }"""
new_cam = """                let cam = icon_button(ui, &p, 34.0, false, icons::camera);
                if cam.hovered() {
                    app.hover_hint = Some(app.t(
                        "截图：把当前帧存到「图片」文件夹（S）",
                        "Save the current frame to Pictures (S)",
                    ));
                }
                if cam.clicked() {
                    super::input::snapshot(app);
                }"""
assert old_cam in t
t = t.replace(old_cam, new_cam, 1)

old_fill = """                    if controls::chip(ui, &p, fill_label, fill)
                        .on_hover_text(app.t("适应＝完整显示（可能有黑边）；填充＝铺满画面（裁掉溢出）", "Fit shows the whole frame (may letterbox); Fill crops to cover the stage"))
                        .clicked()
                    {"""
new_fill = """                    let fill_chip = controls::chip(ui, &p, fill_label, fill);
                    if fill_chip.hovered() {
                        app.hover_hint = Some(app.t(
                            "适应＝完整显示（可能有黑边）；填充＝铺满画面（裁掉溢出）",
                            "Fit shows the whole frame (may letterbox); Fill crops to cover the stage",
                        ));
                    }
                    if fill_chip.clicked() {"""
assert old_fill in t
t = t.replace(old_fill, new_fill, 1)

# transport buttons: turn tooltips into hover hints
pairs = [
    ('app.t("上一帧（,）", "Previous frame (,)")', "prev"),
    ('app.t("播放 / 暂停（空格）", "Play / pause (Space)")', "play"),
    ('app.t("下一帧（.）", "Next frame (.)")', "next"),
    ('app.t("静音（M）", "Mute (M)")', "mute"),
    ('app.t("循环播放", "Loop")', "loop"),
    ('app.t("播放速度", "Playback speed")', "speed"),
]
for expr, _ in pairs:
    # .on_hover_text(<expr>) -> handled below by regex on each site
    pass

pattern = re.compile(r"\)\s*\.on_hover_text\((app\.t\([^;]*?\))\)\s*\.clicked\(\)", re.S)
def repl(m):
    text = m.group(1)
    return ")"

bars.write_text(t, encoding="utf-8")
print("stage toolbar patched")
