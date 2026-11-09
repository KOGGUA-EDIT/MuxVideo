//! Right-hand playlist panel and the settings screen.

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Layout, Rect, Sense, Stroke, Ui, UiBuilder, Vec2,
    pos2, vec2,
};

use super::controls::{self};
use super::icons;
use super::state::{RailTab, Settings, format_size, parse_hex};
use super::{MuxApp, Target};

/// Preset Material You seed colours shown in settings.
const SEEDS: [&str; 8] = [
    "#D0BCFF", "#B4E4C1", "#F8C8A0", "#F2B8B5", "#A8C7FA", "#FFD8E4", "#C8E6C9", "#E8DEF8",
];

// ── playlist ──────────────────────────────────────────────────────────────

pub(super) fn playlist(ui: &mut Ui, app: &mut MuxApp) {
    let p = app.palette.clone();
    let width = ui.available_width();

    // Header.
    let (head, _) = ui.allocate_exact_size(vec2(width, 46.0), Sense::hover());
    ui.painter()
        .rect_filled(head, CornerRadius::ZERO, p.surface);
    ui.painter().text(
        pos2(head.min.x + 10.0, head.center().y),
        egui::Align2::LEFT_CENTER,
        "播放列表",
        FontId::proportional(14.0),
        p.on_surface,
    );
    ui.painter().text(
        pos2(head.min.x + 96.0, head.center().y + 1.0),
        egui::Align2::LEFT_CENTER,
        format!("{} 项", app.playlist.len()),
        FontId::proportional(11.0),
        p.on_surface_variant,
    );

    let add_rect =
        Rect::from_center_size(pos2(head.max.x - 18.0, head.center().y), Vec2::splat(32.0));
    let add = ui.interact(add_rect, ui.id().with("pl-add"), Sense::click());
    if add.hovered() {
        ui.painter()
            .circle_filled(add_rect.center(), 16.0, p.surface_container_high);
    }
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_center_size(add_rect.center(), Vec2::splat(18.0)))
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    icons::folder_open(&mut child, 18.0, p.on_surface_variant);
    if add.on_hover_text("添加视频").clicked() {
        app.pick_files(ui.ctx(), Target::Auto);
    }

    // List body.
    let body = Rect::from_min_max(
        pos2(ui.min_rect().min.x, head.max.y + 4.0),
        pos2(ui.max_rect().max.x, ui.max_rect().max.y - 34.0),
    );
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(body)
            .layout(Layout::top_down(Align::Min)),
        |ui| {
            if app.playlist.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("还没有视频")
                            .size(13.0)
                            .color(p.on_surface_variant),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("拖入文件或点右上角 +")
                            .size(11.0)
                            .color(p.outline),
                    );
                });
                return;
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let mut action: Option<(usize, Target)> = None;
                    let mut remove: Option<usize> = None;
                    for index in 0..app.playlist.len() {
                        let entry = app.playlist[index].clone();
                        let selected = app.selected == Some(index);
                        let row = playlist_row(ui, app, index, &entry.name, entry.size, selected);
                        if row.double_clicked() {
                            action = Some((index, Target::A));
                        } else if let Some(target) = row_action(ui, app, row.rect, index) {
                            action = Some((index, target));
                        }
                        // Right click removes from the list only.
                        if row.secondary_clicked() {
                            remove = Some(index);
                        }
                    }
                    if let Some((index, target)) = action {
                        app.selected = Some(index);
                        let path = app.playlist[index].path.clone();
                        app.open_path(path, target);
                    }
                    if let Some(index) = remove {
                        app.playlist.remove(index);
                        app.selected = None;
                    }
                });
        },
    );

    // Footer hint.
    ui.painter().text(
        pos2(head.min.x + 10.0, ui.max_rect().max.y - 16.0),
        egui::Align2::LEFT_CENTER,
        "双击载入 A 面 · 右键移出列表",
        FontId::proportional(10.5),
        p.outline,
    );
}

/// Draws one playlist row and returns the whole-row interaction.
fn playlist_row(
    ui: &mut Ui,
    app: &MuxApp,
    index: usize,
    name: &str,
    size: u64,
    selected: bool,
) -> egui::Response {
    let p = app.palette.clone();
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 62.0), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.1);

    let fill = if selected {
        p.secondary_container
    } else if hover > 0.0 {
        p.surface_container.gamma_multiply(0.6 + hover * 0.4)
    } else {
        p.surface_container_low
    };
    ui.painter()
        .rect_filled(rect.shrink2(vec2(2.0, 1.0)), CornerRadius::same(16), fill);

    let inner = rect.shrink2(vec2(10.0, 6.0));
    let name_color = if selected {
        p.on_secondary_container
    } else {
        p.on_surface
    };
    let short = super::bars::truncate(name, 26);
    ui.painter().text(
        pos2(inner.min.x + 2.0, inner.min.y + 9.0),
        egui::Align2::LEFT_CENTER,
        format!("{:02}", index + 1),
        FontId::monospace(10.5),
        p.outline,
    );
    ui.painter().text(
        pos2(inner.min.x + 28.0, inner.min.y + 9.0),
        egui::Align2::LEFT_CENTER,
        short,
        FontId::proportional(12.5),
        name_color,
    );
    ui.painter().text(
        pos2(inner.min.x + 28.0, inner.min.y + 28.0),
        egui::Align2::LEFT_CENTER,
        format_size(size),
        FontId::proportional(10.5),
        p.on_surface_variant,
    );

    // Plane assignment chips.
    let chip_size = vec2(40.0, 24.0);
    let a_rect = Rect::from_min_size(pos2(rect.max.x - 96.0, rect.center().y - 12.0), chip_size);
    let b_rect = Rect::from_min_size(pos2(rect.max.x - 50.0, rect.center().y - 12.0), chip_size);
    let listed = app.playlist_path(index);
    for (plane, chip_rect, label) in [(Target::A, a_rect, "A"), (Target::B, b_rect, "B")] {
        let id = ui.id().with(("pl-chip", index, label));
        let resp = ui.interact(chip_rect, id, Sense::click());
        let active = match plane {
            Target::A => app.media.file_a.as_deref() == listed,
            _ => app.media.file_b.as_deref() == listed,
        };
        let fill = if active {
            p.primary
        } else if resp.hovered() {
            p.surface_container_highest
        } else {
            p.surface_container
        };
        ui.painter()
            .rect_filled(chip_rect, CornerRadius::same(8), fill);
        ui.painter().text(
            chip_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.5),
            if active {
                p.on_primary
            } else {
                p.on_surface_variant
            },
        );
        let _ = resp.on_hover_text(if plane == Target::A {
            "载入 A 面"
        } else {
            "载入 B 面"
        });
    }
    response
}

/// Chip hit-testing for the row (returns the plane to load into).
fn row_action(ui: &Ui, app: &MuxApp, row: Rect, index: usize) -> Option<Target> {
    let a_rect = Rect::from_min_size(
        pos2(row.max.x - 96.0, row.center().y - 12.0),
        vec2(40.0, 24.0),
    );
    let b_rect = Rect::from_min_size(
        pos2(row.max.x - 50.0, row.center().y - 12.0),
        vec2(40.0, 24.0),
    );
    if ui.ctx().input(|i| i.pointer.any_click())
        && let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos())
    {
        if a_rect.contains(pos) {
            return Some(Target::A);
        }
        if b_rect.contains(pos) {
            return Some(Target::B);
        }
    }
    let _ = (app, index);
    None
}

// ── settings ──────────────────────────────────────────────────────────────

pub(super) fn settings(ui: &mut Ui, app: &mut MuxApp) {
    let p = app.palette.clone();

    // Header row with a back affordance.
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("设置")
                .size(22.0)
                .strong()
                .color(p.on_surface),
        );
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new("以下设置会自动保存到 muxvideo.settings.json")
                .size(11.5)
                .color(p.on_surface_variant),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if controls::pill_button(ui, &p, "返回画面", false, None).clicked() {
                app.rail = RailTab::Viewer;
            }
        });
    });
    ui.add_space(12.0);

    let mut ctx_for_palette: Option<egui::Context> = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ── appearance ──
            controls::surface_card(ui, &p, |ui| {
                section_title(ui, &p, "外观");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("配色")
                            .size(12.5)
                            .color(p.on_surface_variant),
                    );
                    ui.add_space(12.0);
                    for (label, dark) in [("深色", true), ("浅色", false)] {
                        let selected = app.settings.dark == dark;
                        if controls::chip(ui, &p, label, selected).clicked() && !selected {
                            app.settings.dark = dark;
                            ctx_for_palette = Some(ui.ctx().clone());
                        }
                        ui.add_space(4.0);
                    }
                });
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("主题色")
                        .size(12.5)
                        .color(p.on_surface_variant),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    for seed in SEEDS {
                        let color = parse_hex(seed).unwrap_or(p.primary);
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
                        let active = app.settings.seed.eq_ignore_ascii_case(seed);
                        ui.painter().circle_filled(rect.center(), 13.0, color);
                        if active {
                            ui.painter().circle_stroke(
                                rect.center(),
                                15.5,
                                Stroke::new(2.0_f32, p.on_surface),
                            );
                        }
                        if resp.clicked() {
                            app.settings.seed = (*seed).to_owned();
                            ctx_for_palette = Some(ui.ctx().clone());
                        }
                        resp.on_hover_text(seed);
                        ui.add_space(4.0);
                    }
                    ui.add_space(8.0);
                    let mut seed_text = app.settings.seed.clone();
                    let edit = egui::TextEdit::singleline(&mut seed_text)
                        .desired_width(90.0)
                        .font(egui::TextStyle::Monospace)
                        .text_color(p.on_surface)
                        .background_color(p.surface_container_high)
                        .hint_text("#RRGGBB");
                    if ui.add(edit).changed() {
                        app.settings.seed = seed_text;
                        ctx_for_palette = Some(ui.ctx().clone());
                    }
                });
                ui.add_space(10.0);
                palette_strip(ui, &p);
            });
            ui.add_space(10.0);

            // ── playback ──
            controls::surface_card(ui, &p, |ui| {
                section_title(ui, &p, "播放");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("硬件解码")
                            .size(12.5)
                            .color(p.on_surface_variant),
                    );
                    ui.add_space(14.0);
                    for value in ["auto-safe", "d3d11va", "no"] {
                        let selected = app.settings.hwdec == value;
                        if controls::chip(ui, &p, value, selected).clicked() && !selected {
                            app.settings.hwdec = value.to_owned();
                            app.settings_dirty = true;
                        }
                        ui.add_space(4.0);
                    }
                    ui.label(
                        egui::RichText::new("下次载入生效")
                            .size(10.0)
                            .color(p.outline),
                    );
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("循环当前文件")
                            .size(12.5)
                            .color(p.on_surface_variant),
                    );
                    ui.add_space(14.0);
                    let mut on = app.settings.loop_file;
                    if controls::switch(ui, &p, &mut on).changed() {
                        app.settings.loop_file = on;
                        app.apply_loop();
                    }
                    ui.add_space(24.0);
                    ui.label(
                        egui::RichText::new("显示播放列表")
                            .size(12.5)
                            .color(p.on_surface_variant),
                    );
                    ui.add_space(14.0);
                    let mut on = app.settings.show_playlist;
                    if controls::switch(ui, &p, &mut on).changed() {
                        app.settings.show_playlist = on;
                        app.settings_dirty = true;
                    }
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("默认音量")
                            .size(12.5)
                            .color(p.on_surface_variant),
                    );
                    ui.add_space(14.0);
                    let mut vol = (app.settings.volume / 130.0) as f32;
                    if controls::slider(ui, &p, 160.0, &mut vol, true).changed() {
                        app.settings.volume = (vol as f64 * 130.0).round();
                        app.apply_volume();
                    }
                    ui.label(
                        egui::RichText::new(format!("{:.0}%", app.settings.volume))
                            .size(11.5)
                            .monospace()
                            .color(p.on_surface_variant),
                    );
                });
            });
            ui.add_space(10.0);

            // ── shortcuts ──
            controls::surface_card(ui, &p, |ui| {
                section_title(ui, &p, "快捷键");
                ui.add_space(8.0);
                egui::Grid::new("shortcuts")
                    .num_columns(2)
                    .spacing([24.0, 6.0])
                    .show(ui, |ui| {
                        for (keys, what) in [
                            ("空格", "播放 / 暂停"),
                            ("画面左键拖动", "逐帧擦洗，左退右进"),
                            ("画面中键拖动", "平移画面"),
                            ("滚轮 / Ctrl+滚轮", "音量 / 缩放"),
                            ("画面双击", "全屏"),
                            ("← →", "后退 / 前进 5 秒"),
                            ("J L", "后退 / 前进 10 秒"),
                            (", .", "上一帧 / 下一帧"),
                            ("↑ ↓", "音量增减"),
                            ("M", "静音"),
                            ("1 2 3 4", "单画面 / 并排 / 划像 / 闪烁"),
                            ("[ ]", "划像分界移动"),
                            ("Tab", "显示 / 隐藏播放列表"),
                            ("F / Esc", "全屏 / 退出全屏"),
                        ] {
                            controls::badge(ui, &p, keys, p.surface_container_high, p.on_surface);
                            ui.label(
                                egui::RichText::new(what)
                                    .size(12.0)
                                    .color(p.on_surface_variant),
                            );
                            ui.end_row();
                        }
                    });
            });
            ui.add_space(10.0);

            // ── about ──
            controls::surface_card(ui, &p, |ui| {
                section_title(ui, &p, "关于");
                ui.add_space(6.0);
                let lib_state = if app.lib.is_some() {
                    "已加载"
                } else {
                    "未加载"
                };
                ui.label(
                    egui::RichText::new(format!(
                        "MuxVideo v{} · libmpv {} · Rust / eframe {}",
                        env!("CARGO_PKG_VERSION"),
                        lib_state,
                        "0.33"
                    ))
                    .size(12.0)
                    .color(p.on_surface_variant),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("进程内 libmpv + Win32 子窗口 · Material You 主题")
                        .size(11.0)
                        .color(p.outline),
                );
                if let Some(err) = app.boot_error.clone() {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(err).size(11.0).color(p.error));
                }
            });
            ui.add_space(16.0);
        });

    if let Some(ctx) = ctx_for_palette {
        app.rebuild_palette(&ctx);
        ctx.request_repaint();
    }
}

fn section_title(ui: &mut Ui, p: &crate::ui::Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(15.0)
            .strong()
            .color(p.on_surface),
    );
}

/// A row of live palette role swatches, so the theme is verifiable at a glance.
fn palette_strip(ui: &mut Ui, p: &crate::ui::Palette) {
    let roles: [(&str, Color32); 6] = [
        ("primary", p.primary),
        ("on-primary", p.on_primary),
        ("secondary", p.secondary),
        ("surface", p.surface_container),
        ("tertiary", p.tertiary),
        ("error", p.error),
    ];
    ui.horizontal(|ui| {
        for (name, color) in roles {
            let (rect, resp) = ui.allocate_exact_size(vec2(74.0, 34.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::same(8), color);
            let fg = if is_dark(color) {
                Color32::from_rgb(0xF0, 0xF0, 0xF0)
            } else {
                Color32::from_rgb(0x20, 0x20, 0x20)
            };
            ui.painter().text(
                pos2(rect.min.x + 8.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                name,
                FontId::proportional(10.5),
                fg,
            );
            resp.on_hover_text(format!("{name}  {}", super::state::format_hex(color)));
            ui.add_space(4.0);
        }
    });
}

fn is_dark(color: Color32) -> bool {
    let luma = 0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32;
    luma < 128.0
}

#[allow(dead_code)]
fn _unused(_: Settings) {}
