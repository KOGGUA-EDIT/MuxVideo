import io, sys, pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\panels.rs")
src = p.read_text(encoding="utf-8")

pairs = [
    ('"播放列表",', 'app.t("播放列表", "Playlist"),'),
    ('format!("{} 项", app.playlist.len()),', 'format!("{} {}", app.playlist.len(), app.t("项", "items")),'),
    ('.on_hover_text("添加视频")', '.on_hover_text(app.t("添加视频", "Add videos"))'),
    ('egui::RichText::new("还没有视频")', 'egui::RichText::new(app.t("还没有视频", "No videos yet"))'),
    ('egui::RichText::new("拖入文件或点右上角 +")', 'egui::RichText::new(app.t("拖入文件或点右上角 +", "Drop files in, or hit + above"))'),
    ('"双击载入 A 面 · 右键移出列表",', 'app.t("双击载入 A 面 · 右键移出列表", "Double-click to load into A · right-click to remove"),'),
    ('"媒体信息",', 'app.t("媒体信息", "Media info"),'),
    ('Some("A 面")', 'Some(app.t("A 面", "A side"))'),
    ('Some("B 面")', 'Some(app.t("B 面", "B side"))'),
    ('plane.unwrap_or("未载入")', 'plane.unwrap_or(app.t("未载入", "Empty"))'),
    ('Some("A 面") => app.media.meta_a.clone(),', 'Some("A 面") => app.media.meta_a.clone(),'),
    ('"分辨率",', 'app.t("分辨率", "Resolution"),'),
    ('"帧率",', 'app.t("帧率", "Frame rate"),'),
    ('"时长",', 'app.t("时长", "Duration"),'),
    ('"{} · {} 帧",', '"{} · {} {}",'),
    ('"容器",', 'app.t("容器", "Container"),'),
    ('"视频编码",', 'app.t("视频编码", "Video codec"),'),
    ('"码率",', 'app.t("码率", "Bitrate"),'),
    ('"音频",', 'app.t("音频", "Audio"),'),
    ('"无".into()', 'app.t("无", "none").into()'),
    ('"文件大小",', 'app.t("文件大小", "File size"),'),
    ('rows.push(("文件大小", super::state::format_size(entry.size)));', 'rows.push((app.t("文件大小", "File size"), super::state::format_size(entry.size)));'),
    ('rows.push(("提示", "载入 A / B 面后显示完整信息".into()));', 'rows.push((app.t("提示", "Hint"), app.t("载入 A / B 面后显示完整信息", "Load into A or B to see everything").into()));'),
    ('"载入 A 面"', 'app.t("载入 A 面", "Load into A")'),
    ('"载入 B 面"', 'app.t("载入 B 面", "Load into B")'),
    ('egui::RichText::new("设置")', 'egui::RichText::new(app.t("设置", "Settings"))'),
    ('egui::RichText::new("以下设置会自动保存到 %APPDATA%\\\\MuxVideo\\\\muxvideo.settings.json")',
     'egui::RichText::new(format!("{} %APPDATA%\\\\MuxVideo\\\\muxvideo.settings.json", app.t("以下设置会自动保存到", "Saved automatically to")))'),
    ('if controls::pill_button(ui, &p, "返回画面", false, None).clicked() {', 'if controls::pill_button(ui, &p, app.t("返回画面", "Back to player"), false, None).clicked() {'),
    ('section_title(ui, &p, "外观");', 'section_title(ui, &p, app.t("外观", "Appearance"));'),
    ('egui::RichText::new("配色")', 'egui::RichText::new(app.t("配色", "Colour scheme"))'),
    ('for (label, dark) in [("深色", true), ("浅色", false)] {', 'for (label, dark) in [(app.t("深色", "Dark"), true), (app.t("浅色", "Light"), false)] {'),
    ('egui::RichText::new("主题色")', 'egui::RichText::new(app.t("主题色", "Seed colour"))'),
    ('section_title(ui, &p, "播放");', 'section_title(ui, &p, app.t("播放", "Playback"));'),
    ('egui::RichText::new("硬件解码")', 'egui::RichText::new(app.t("硬件解码", "Hardware decoding"))'),
    ('egui::RichText::new("下次载入生效")', 'egui::RichText::new(app.t("下次载入生效", "applies on next load"))'),
    ('egui::RichText::new("循环当前文件")', 'egui::RichText::new(app.t("循环当前文件", "Loop current file"))'),
    ('egui::RichText::new("显示播放列表")', 'egui::RichText::new(app.t("显示播放列表", "Show playlist"))'),
    ('egui::RichText::new("默认音量")', 'egui::RichText::new(app.t("默认音量", "Default volume"))'),
    ('egui::RichText::new("擦洗灵敏度")', 'egui::RichText::new(app.t("擦洗灵敏度", "Scrub sensitivity"))'),
    ('"{:.0} px / 帧（越小越跟手）",', '"{}",'),
    ('section_title(ui, &p, "快捷键");', 'section_title(ui, &p, app.t("快捷键", "Shortcuts"));'),
    ('section_title(ui, &p, "关于");', 'section_title(ui, &p, app.t("关于", "About"));'),
    ('"已加载"', 'app.t("已加载", "loaded")'),
    ('"未加载"', 'app.t("未加载", "missing")'),
    ('egui::RichText::new("进程内 libmpv + Win32 子窗口 · Material You 主题")', 'egui::RichText::new(app.t("进程内 libmpv + Win32 子窗口 · Material You 主题", "in-process libmpv + Win32 child windows · Material You theming"))'),
]

missing = []
for old, new in pairs:
    if old not in src:
        missing.append(old)
        continue
    src = src.replace(old, new)

p.write_text(src, encoding="utf-8")
print("missing:", len(missing))
for m in missing:
    print("  ", m)
