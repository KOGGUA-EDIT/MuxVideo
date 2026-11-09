import pathlib

base = pathlib.Path(r"D:\app edit\Muxvideo\src\app")

def patch(name, pairs):
    p = base / name
    src = p.read_text(encoding="utf-8")
    missing = []
    for old, new in pairs:
        if old not in src:
            missing.append(old)
            continue
        src = src.replace(old, new)
    p.write_text(src, encoding="utf-8")
    print(name, "missing:", len(missing))
    for m in missing:
        print("   ", m)

patch("mod.rs", [
    ('Toast::error("libmpv 未加载，无法播放")',
     'Toast::error(self.t("libmpv 未加载，无法播放", "libmpv is not loaded; cannot play"))'),
    ('"{} → {} 面",',
     '"{} → {} {}",'),
    ('Toast::error(format!("载入失败：{err:#}"))',
     'Toast::error(format!("{}{err:#}", self.t("载入失败：", "Load failed: ")))'),
    ('Toast::error(format!("{plane} 面播放出错：{reason}"))',
     'Toast::error(format!(\n                "{}{plane} {}{reason}",\n                self.t("播放出错：", "Playback error: "),\n                self.t("面：", "side: ")\n            ))'),
    ('"没有载入视频 · 按 F 或 Esc 退出全屏",',
     'self.t("没有载入视频 · 按 F 或 Esc 退出全屏", "No video loaded · press F or Esc to leave full screen"),'),
])

patch("input.rs", [
    ('"帧 {} / {}",', '"{}",'),
    ('Toast::info(format!("缩放 {:+.0}%", app.zoom * 100.0))',
     'Toast::info(format!("{} {:+.0}%", app.t("缩放", "Zoom"), app.zoom * 100.0))'),
    ('Toast::info(format!("音量 {:.0}%", next))',
     'Toast::info(format!("{} {:.0}%", app.t("音量", "Volume"), next))'),
    ('Toast::error("没有可截图的画面")',
     'Toast::error(app.t("没有可截图的画面", "Nothing to capture"))'),
    ('format!("MuxVideo_{}_帧{frame:05}.png", file_stamp())',
     'format!("MuxVideo_{}_{}{frame:05}.png", file_stamp(), app.t("帧", "frame"))'),
    ('Toast::info(format!("已保存 {name}"))',
     'Toast::info(format!("{}{name}", app.t("已保存 ", "Saved ")))'),
    ('Toast::error("截图失败")',
     'Toast::error(app.t("截图失败", "Screenshot failed"))'),
])
