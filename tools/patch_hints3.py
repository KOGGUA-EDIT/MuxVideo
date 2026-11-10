import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\bars.rs")
t = p.read_text(encoding="utf-8")

old_mute = """                if icon_button(
                    ui,
                    &p,
                    34.0,
                    false,
                    if app.settings.muted {
                        icons::mute
                    } else {
                        icons::volume
                    },
                )
                .on_hover_text(app.t("静音（M）", "Mute (M)"))
                .clicked()
                {"""
new_mute = """                let mute = icon_button(
                    ui,
                    &p,
                    34.0,
                    false,
                    if app.settings.muted {
                        icons::mute
                    } else {
                        icons::volume
                    },
                );
                if mute.hovered() {
                    app.hover_hint = Some(app.t("静音（M）", "Mute (M)"));
                }
                if mute.clicked() {"""
assert old_mute in t
t = t.replace(old_mute, new_mute, 1)

old_loop = """                if icon_button(ui, &p, 34.0, app.settings.loop_file, icons::loop_icon)
                    .on_hover_text(app.t("循环播放", "Loop"))
                    .clicked()
                {"""
new_loop = """                let loop_btn = icon_button(ui, &p, 34.0, app.settings.loop_file, icons::loop_icon);
                if loop_btn.hovered() {
                    app.hover_hint = Some(app.t("循环播放", "Loop"));
                }
                if loop_btn.clicked() {"""
assert old_loop in t
t = t.replace(old_loop, new_loop, 1)

old_speed = """                if controls::chip(ui, &p, &speed_label, app.settings.speed != 1.0)
                    .on_hover_text(app.t("播放速度", "Playback speed"))
                    .clicked()
                {"""
new_speed = """                let speed_chip = controls::chip(ui, &p, &speed_label, app.settings.speed != 1.0);
                if speed_chip.hovered() {
                    app.hover_hint = Some(app.t("播放速度", "Playback speed"));
                }
                if speed_chip.clicked() {"""
assert old_speed in t
t = t.replace(old_speed, new_speed, 1)

p.write_text(t, encoding="utf-8")
print("mute/loop/speed patched")
