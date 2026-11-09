//! Chrome drawing: title bar, navigation rail, compare bar, stage and transport.

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, Sense, Stroke, StrokeKind,
    Ui, UiBuilder, Vec2, pos2, vec2,
};

use super::controls::{self, icon_button};
use super::icons;
use super::state::{CompareMode, RailTab, SplitAxis, format_timecode, frame_at};
use super::{MuxApp, TITLEBAR_H, Target};

/// Draws one plane tag ("A" / "B") plus its file name, or a hint when empty.
fn file_badge(ui: &mut Ui, app: &mut MuxApp, plane: Target) {
    let p = app.palette.clone();
    let (path, tint) = match plane {
        Target::A => (app.media.file_a.clone(), p.primary_container),
        _ => (
            app.media.file_b.clone(),
            controls::container(p.tertiary, p.surface),
        ),
    };
    let path = path.as_deref();
    let (label, name) = match path {
        Some(path) => (
            if plane == Target::B { "B" } else { "A" },
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
        ),
        None => (
            if plane == Target::B { "B" } else { "A" },
            "未载入".to_string(),
        ),
    };
    let name = truncate(&name, 26);

    let text_color = if path.is_some() {
        p.on_surface
    } else {
        p.on_surface_variant
    };
    let galley = ui
        .painter()
        .layout_no_wrap(name.clone(), FontId::proportional(12.0), text_color);
    let size = vec2(galley.size().x + 44.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.1);
    let fill = if path.is_some() {
        p.surface_container
    } else {
        p.surface_container_low
    };
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(8),
        if hover > 0.0 {
            fill.gamma_multiply(1.15)
        } else {
            fill
        },
    );
    let tag_rect = Rect::from_min_size(rect.min + vec2(6.0, 5.0), vec2(18.0, 16.0));
    ui.painter().rect_filled(
        tag_rect,
        CornerRadius::same(5),
        if path.is_some() {
            tint
        } else {
            p.surface_container_high
        },
    );
    ui.painter().text(
        tag_rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.5),
        if path.is_some() {
            p.on_primary_container
        } else {
            p.on_surface_variant
        },
    );
    ui.painter().galley(
        pos2(rect.min.x + 30.0, rect.center().y - galley.size().y * 0.5),
        galley,
        text_color,
    );
    if response.clicked() {
        app.pick_files(ui.ctx(), plane);
    }
    response.on_hover_text(match path {
        Some(path) => path.to_string_lossy().to_string(),
        None => "点击选择文件".into(),
    });
}

/// Truncates with an ellipsis so chrome rows stay stable.
pub(super) fn truncate(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn window_button(
    ui: &mut Ui,
    app: &MuxApp,
    glyph: fn(&mut Ui, f32, Color32) -> egui::Response,
    danger: bool,
) -> egui::Response {
    let p = app.palette.clone();
    let (rect, response) = ui.allocate_exact_size(vec2(44.0, TITLEBAR_H), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.1);
    if hover > 0.0 {
        let fill = if danger {
            p.error.gamma_multiply(0.9 * hover)
        } else {
            p.surface_container_highest.gamma_multiply(hover)
        };
        ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    }
    let fg = if danger && response.hovered() {
        p.on_error
    } else {
        p.on_surface_variant
    };
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(16.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(icon_rect)
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    glyph(&mut child, 16.0, fg);
    response
}

/// Custom MD3 title bar with window controls.
pub(super) fn titlebar(ui: &mut Ui, app: &mut MuxApp) {
    let p = app.palette.clone();
    let bar = ui.max_rect();
    let drag = ui.interact(bar, ui.id().with("titlebar-drag"), Sense::click_and_drag());
    if drag.dragged() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }

    let inner = bar.shrink2(vec2(14.0, 0.0));
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            // Brand mark.
            let (logo_rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
            ui.painter()
                .rect_filled(logo_rect, CornerRadius::same(9), p.primary_container);
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(logo_rect)
                    .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
            );
            icons::play(&mut child, 16.0, p.on_primary_container);

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("MuxVideo")
                    .size(14.5)
                    .strong()
                    .color(p.on_surface),
            );
            ui.add_space(12.0);
            let (sep_rect, _) = ui.allocate_exact_size(vec2(1.0, 22.0), Sense::hover());
            ui.painter()
                .rect_filled(sep_rect, CornerRadius::same(1), p.outline_variant);
            ui.add_space(12.0);

            file_badge(ui, app, Target::A);
            ui.add_space(6.0);
            file_badge(ui, app, Target::B);
            ui.add_space(6.0);
            if app.settings.mode == CompareMode::Blink {
                controls::badge(
                    ui,
                    &p,
                    "A⇄B",
                    controls::container(p.tertiary, p.surface),
                    p.on_surface,
                );
            }

            // Right cluster: window controls, then toggles.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let close = window_button(ui, app, icons::close, true);
                if close.clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                let maximize = window_button(ui, app, icons::maximize, false);
                if maximize.clicked() {
                    let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }
                let minimize = window_button(ui, app, icons::minimize, false);
                if minimize.clicked() {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                ui.add_space(6.0);
                let (sep_rect, _) = ui.allocate_exact_size(vec2(1.0, 22.0), Sense::hover());
                ui.painter()
                    .rect_filled(sep_rect, CornerRadius::same(1), p.outline_variant);
                ui.add_space(4.0);
                let full = ui.ctx().input(|i| i.viewport().fullscreen.unwrap_or(false));
                if icon_button(ui, &p, 38.0, full, icons::fullscreen)
                    .on_hover_text("全屏（F）")
                    .clicked()
                {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
                }
                let settings_active = app.rail == RailTab::Settings;
                if icon_button(ui, &p, 38.0, settings_active, icons::settings).clicked() {
                    app.rail = if settings_active {
                        RailTab::Viewer
                    } else {
                        RailTab::Settings
                    };
                }
                let list_active = app.settings.show_playlist;
                if icon_button(ui, &p, 38.0, list_active, icons::playlist).clicked() {
                    app.settings.show_playlist = !app.settings.show_playlist;
                    app.settings_dirty = true;
                }
                ui.add_space(4.0);
            });
        },
    );
}

/// Navigation rail item: pill behind the icon, caption underneath.
fn rail_item(
    ui: &mut Ui,
    app: &mut MuxApp,
    tab: RailTab,
    glyph: fn(&mut Ui, f32, Color32) -> egui::Response,
) {
    let p = app.palette.clone();
    let active = app.rail == tab;
    let size = vec2(64.0, 62.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.12);
    let pill = Rect::from_center_size(pos2(rect.center().x, rect.min.y + 16.0), vec2(56.0, 32.0));
    let fill = if active {
        p.secondary_container
    } else if hover > 0.0 {
        p.surface_container_high.gamma_multiply(hover * 0.9)
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(pill, CornerRadius::same(16), fill);
    }
    let fg = if active {
        p.on_secondary_container
    } else if response.hovered() {
        p.on_surface
    } else {
        p.on_surface_variant
    };
    let icon_rect = Rect::from_center_size(pill.center(), Vec2::splat(20.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(icon_rect)
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    glyph(&mut child, 20.0, fg);
    ui.painter().text(
        pos2(rect.center().x, rect.max.y - 12.0),
        Align2::CENTER_CENTER,
        tab.label(),
        FontId::proportional(11.5),
        if active {
            p.on_surface
        } else {
            p.on_surface_variant
        },
    );
    response.clone().on_hover_text(tab.label());
    if response.clicked() {
        app.rail = tab;
        match tab {
            RailTab::Viewer => app.set_mode(CompareMode::Single),
            RailTab::Compare => {
                if app.settings.mode == CompareMode::Single {
                    app.set_mode(CompareMode::Pair);
                }
            }
            RailTab::Settings => {}
        }
    }
}

pub(super) fn rail(ui: &mut Ui, app: &mut MuxApp) {
    let p = app.palette.clone();
    let full = ui.max_rect();
    ui.painter()
        .rect_filled(full, CornerRadius::ZERO, p.surface);

    ui.scope_builder(
        UiBuilder::new()
            .max_rect(full.shrink2(vec2(0.0, 8.0)))
            .layout(Layout::top_down(Align::Center)),
        |ui| {
            // Brand.
            let (logo, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
            ui.painter()
                .rect_filled(logo, CornerRadius::same(14), p.primary_container);
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(logo)
                    .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
            );
            icons::play(&mut child, 22.0, p.on_primary_container);
            ui.add_space(20.0);

            rail_item(ui, app, RailTab::Viewer, icons::grid);
            rail_item(ui, app, RailTab::Compare, icons::compare);
            rail_item(ui, app, RailTab::Settings, icons::settings);
        },
    );

    // Bottom cluster pinned to the rail foot.
    let foot = Rect::from_min_max(pos2(full.min.x, full.max.y - 96.0), full.max);
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(foot)
            .layout(Layout::top_down(Align::Center)),
        |ui| {
            let palette_resp = icon_button(ui, &p, 40.0, false, icons::palette);
            if palette_resp.clicked() {
                app.rail = RailTab::Settings;
            }
            ui.add_space(2.0);
            ui.painter().text(
                pos2(foot.center().x, foot.min.y + 74.0),
                Align2::CENTER_CENTER,
                format!("v{}", env!("CARGO_PKG_VERSION")),
                FontId::proportional(10.0),
                p.outline,
            );
        },
    );

    // Rail click handling happens through the item responses' interaction with
    // `app.rail`; the items are drawn above so we apply clicks after painting.
    let _ = app.rail;
}

/// Compare toolbar + the video stage rectangle.
pub(super) fn stage_area(ui: &mut Ui, app: &mut MuxApp) {
    let p = app.palette.clone();

    // Compare bar.
    let (bar_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::hover());
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(bar_rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            for mode in CompareMode::ALL {
                let selected = app.settings.mode == mode;
                if controls::chip(ui, &p, mode.label(), selected)
                    .on_hover_text(format!("{}\n{}", mode.hint(), mode.about()))
                    .clicked()
                {
                    app.set_mode(mode);
                }
                ui.add_space(6.0);
            }
            ui.add_space(6.0);

            if matches!(app.settings.mode, CompareMode::Pair | CompareMode::Wipe) {
                let (sep, _) = ui.allocate_exact_size(vec2(1.0, 20.0), Sense::hover());
                ui.painter()
                    .rect_filled(sep, CornerRadius::same(1), p.outline_variant);
                ui.add_space(8.0);
                for axis in [SplitAxis::Horizontal, SplitAxis::Vertical] {
                    if controls::chip(ui, &p, axis.label(), app.settings.axis == axis).clicked() {
                        app.settings.axis = axis;
                        app.settings_dirty = true;
                    }
                    ui.add_space(4.0);
                }
                if app.settings.mode == CompareMode::Wipe {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("分界")
                            .size(12.0)
                            .color(p.on_surface_variant),
                    );
                    ui.add_space(6.0);
                    let mut wipe = app.settings.wipe;
                    if controls::slider(ui, &p, 150.0, &mut wipe, true).changed() {
                        app.settings.wipe = wipe;
                        app.settings_dirty = true;
                    }
                    ui.label(
                        egui::RichText::new(format!("{:>3.0}%", app.settings.wipe * 100.0))
                            .size(11.0)
                            .monospace()
                            .color(p.on_surface_variant),
                    );
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, &p, 34.0, false, icons::camera)
                    .on_hover_text("截图：把当前帧存到「图片」文件夹（S）")
                    .clicked()
                {
                    super::input::snapshot(app);
                }
                // 适应 / 填充: letterbox vs crop-to-fill the stage.
                {
                    let fill = app.settings.fill;
                    if controls::chip(ui, &p, if fill { "填充" } else { "适应" }, fill)
                        .on_hover_text("适应＝完整显示（可能有黑边）；填充＝铺满画面（裁掉溢出）")
                        .clicked()
                    {
                        app.settings.fill = !fill;
                        app.settings_dirty = true;
                        app.view_dirty();
                    }
                    ui.add_space(6.0);
                }
                if app.media.file_a.is_some() && app.media.file_b.is_some() {
                    if icon_button(ui, &p, 34.0, false, icons::swap)
                        .on_hover_text("交换 A / B")
                        .clicked()
                    {
                        app.swap_planes();
                    }
                    ui.add_space(6.0);
                }
                if app.settings.mode.needs_b() && app.media.file_b.is_none() {
                    if controls::pill_button(ui, &p, "载入 B", false, Some(icons::playlist))
                        .clicked()
                    {
                        app.pick_files(ui.ctx(), Target::B);
                    }
                    ui.add_space(6.0);
                }
                if let Some(text) = drift_text(app) {
                    ui.label(
                        egui::RichText::new(text)
                            .size(11.0)
                            .monospace()
                            .color(p.on_surface_variant),
                    );
                }
                if app.settings.mode.needs_b() && ui.available_width() > 200.0 {
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new(app.settings.mode.hint())
                            .size(11.0)
                            .color(p.on_surface_variant.gamma_multiply(0.85)),
                    );
                }
            });
        },
    );

    ui.add_space(8.0);

    // Stage.
    let avail = ui.available_size();
    let (stage, response) = ui.allocate_exact_size(avail, Sense::click());
    app.stage_rect = stage;
    app.stage_reported = true;

    ui.painter()
        .rect_filled(stage, CornerRadius::same(18), p.surface_container_lowest);
    ui.painter().rect_stroke(
        stage,
        CornerRadius::same(18),
        Stroke::new(1.0_f32, p.outline_variant.gamma_multiply(0.5)),
        StrokeKind::Inside,
    );

    if !app.has_video() {
        empty_stage(ui, app, stage);
    } else {
        // Wipe mode leaves a 3 px hole between the native planes; paint the
        // handle colour into it so the split line is always visible.
        if app.settings.mode == CompareMode::Wipe && app.media.file_b.is_some() {
            let frac = app.settings.wipe.clamp(0.0, 1.0);
            let line = match app.settings.axis {
                SplitAxis::Horizontal => {
                    let x = stage.left() + stage.width() * frac;
                    Rect::from_min_max(pos2(x - 3.0, stage.top()), pos2(x + 3.0, stage.bottom()))
                }
                SplitAxis::Vertical => {
                    let y = stage.top() + stage.height() * frac;
                    Rect::from_min_max(pos2(stage.left(), y - 3.0), pos2(stage.right(), y + 3.0))
                }
            };
            ui.painter().rect_filled(
                line.shrink2(vec2(
                    0.0,
                    if app.settings.axis == SplitAxis::Horizontal {
                        0.0
                    } else {
                        1.0
                    },
                )),
                CornerRadius::same(2),
                p.primary,
            );
        }

        // Corner caption sits inside the stage but outside the video chrome.
        let caption = format!(
            "{} · {}",
            app.settings.mode.label(),
            app.settings.mode.hint()
        );
        ui.painter().text(
            pos2(stage.min.x + 16.0, stage.min.y + 14.0),
            Align2::LEFT_TOP,
            caption,
            FontId::proportional(11.5),
            p.on_surface_variant.gamma_multiply(0.55),
        );
    }
    let _ = response;

    if app.boot_error().is_some() {
        let err = app.boot_error().unwrap_or_default().to_owned();
        ui.painter().text(
            pos2(stage.center().x, stage.max.y - 24.0),
            Align2::CENTER_CENTER,
            format!("libmpv 未就绪：{}", truncate(&err, 90)),
            FontId::proportional(12.0),
            p.error,
        );
    }
}

/// Centred empty state with quick actions.
fn empty_stage(ui: &mut Ui, app: &mut MuxApp, stage: Rect) {
    let p = app.palette.clone();
    let center = Rect::from_center_size(stage.center(), vec2(stage.width().min(560.0), 260.0));

    // Drop hint.
    ui.painter().rect_stroke(
        center.shrink(2.0),
        CornerRadius::same(16),
        Stroke::new(1.2_f32, p.outline_variant),
        StrokeKind::Inside,
    );

    ui.scope_builder(
        UiBuilder::new()
            .max_rect(center.shrink(24.0))
            .layout(Layout::top_down(Align::Center)),
        |ui| {
            ui.add_space(18.0);
            let (ico, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
            ui.painter()
                .circle_filled(ico.center(), 26.0, p.primary_container.gamma_multiply(0.9));
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(ico)
                    .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
            );
            icons::folder_open(&mut child, 24.0, p.on_primary_container);
            ui.add_space(14.0);
            ui.label(
                egui::RichText::new("拖入视频，或直接打开")
                    .size(17.0)
                    .strong()
                    .color(p.on_surface),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("A 面作基准，B 面作对照 · mp4 / mkv / mov / webm / ts …")
                    .size(12.0)
                    .color(p.on_surface_variant),
            );
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if controls::pill_button(ui, &p, "打开 A 面", true, Some(icons::play)).clicked()
                {
                    app.pick_files(ui.ctx(), Target::A);
                }
                ui.add_space(8.0);
                if controls::pill_button(ui, &p, "打开 B 面", false, Some(icons::compare)).clicked()
                {
                    app.pick_files(ui.ctx(), Target::B);
                }
            });
        },
    );
}

fn drift_text(app: &MuxApp) -> Option<String> {
    if !app.settings.mode.needs_b() {
        return None;
    }
    if app.media.player_b.is_none() {
        return Some("等待 B 面".into());
    }
    let drift = app.media.drift;
    Some(format!("同步 Δ{:+.0} ms", drift * 1000.0))
}

/// Big primary play button.
fn play_button(ui: &mut Ui, app: &MuxApp) -> egui::Response {
    let p = app.palette.clone();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.12);
    let fill = if response.hovered() {
        p.primary.gamma_multiply(1.0 + hover * 0.1)
    } else {
        p.primary
    };
    ui.painter().circle_filled(rect.center(), 24.0, fill);
    if response.hovered() {
        ui.painter()
            .circle_filled(rect.center(), 28.0, p.primary.gamma_multiply(0.18));
    }
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(22.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(icon_rect)
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    if app.is_playing() {
        icons::pause(&mut child, 22.0, p.on_primary);
    } else {
        icons::play(&mut child, 22.0, p.on_primary);
    }
    response
}

const SPEEDS: [f64; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 4.0];

/// Playback transport: scrubber on top, controls underneath.
pub(super) fn transport(ui: &mut Ui, app: &mut MuxApp) {
    let p = app.palette.clone();
    let card = ui.max_rect();
    ui.painter()
        .rect_filled(card, CornerRadius::same(28), p.surface_container_low);

    let inner = card.shrink2(vec2(22.0, 10.0));

    // Row 1 — progress.
    let row1 = Rect::from_min_size(inner.min, vec2(inner.width(), 40.0));
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(row1)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            let dur = app.duration();
            let pos = app.time_pos().min(if dur > 0.0 { dur } else { f64::MAX });
            let fps = app.fps();
            let label_w = 104.0;
            ui.allocate_ui_with_layout(
                vec2(label_w, 20.0),
                Layout::right_to_left(Align::Center),
                |ui| {
                    ui.label(
                        egui::RichText::new(format_timecode(pos, fps))
                            .size(12.0)
                            .monospace()
                            .color(if app.has_video() {
                                p.on_surface
                            } else {
                                p.outline
                            }),
                    );
                },
            );
            let slider_w = (ui.available_width() - label_w - 12.0).max(80.0);
            let mut frac = if dur > 0.0 { (pos / dur) as f32 } else { 0.0 };
            let playing = app.is_playing();
            let resp = crate::ui::wavy_slider(ui, &p, "seek", slider_w, &mut frac, playing);
            let target = if dur > 0.0 {
                Some((frac as f64) * dur)
            } else {
                None
            };
            // While scrubbing, report the dragged position instead of the
            // decoder's, so the time label tracks the thumb without jitter.
            app.scrub = if resp.dragging { target } else { None };
            if resp.changed
                && let Some(pos) = target
            {
                app.seek_to(pos);
            }
            ui.allocate_ui_with_layout(
                vec2(label_w, 20.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.label(
                        egui::RichText::new(format_timecode(dur, fps))
                            .size(12.0)
                            .monospace()
                            .color(p.on_surface_variant),
                    );
                },
            );
        },
    );

    ui.add_space(2.0);

    // Row 2 — controls.
    let row2 = Rect::from_min_max(
        pos2(inner.min.x, inner.min.y + 44.0),
        pos2(inner.max.x, inner.max.y),
    );
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(row2)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            if icon_button(ui, &p, 34.0, false, icons::step_back)
                .on_hover_text("上一帧（,）")
                .clicked()
            {
                app.step_frame(false);
            }
            ui.add_space(6.0);
            if play_button(ui, app)
                .on_hover_text("播放 / 暂停（空格）")
                .clicked()
            {
                app.toggle_play();
            }
            ui.add_space(6.0);
            if icon_button(ui, &p, 34.0, false, icons::step_forward)
                .on_hover_text("下一帧（.）")
                .clicked()
            {
                app.step_frame(true);
            }
            ui.add_space(10.0);
            let info = if app.has_video() {
                format!(
                    "{} / {}",
                    app.settings.mode.label(),
                    app.settings.axis.label()
                )
            } else {
                "无媒体".to_string()
            };
            ui.label(
                egui::RichText::new(info)
                    .size(11.5)
                    .color(p.on_surface_variant),
            );

            // Source metadata for whichever planes are loaded.
            let mut parts: Vec<String> = Vec::new();
            if let Some(meta) = &app.media.meta_a {
                let summary = meta.summary();
                if !summary.is_empty() {
                    parts.push(format!("A {summary}"));
                }
            }
            if let Some(meta) = &app.media.meta_b {
                let summary = meta.summary();
                if !summary.is_empty() {
                    parts.push(format!("B {summary}"));
                }
            }
            if app.has_video() {
                let fps = app.fps();
                let cur = frame_at(app.time_pos(), fps);
                let all = frame_at(app.duration(), fps);
                parts.push(format!("帧 {cur} / {all}"));
            }
            if !parts.is_empty() {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(parts.join("  ·  "))
                        .size(10.5)
                        .color(p.outline),
                );
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Volume mute toggle.
                if icon_button(
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
                .on_hover_text("静音（M）")
                .clicked()
                {
                    app.settings.muted = !app.settings.muted;
                    app.apply_volume();
                }
                let mut vol = (app.settings.volume / 130.0) as f32;
                if controls::slider(ui, &p, 96.0, &mut vol, true).changed() {
                    app.settings.volume = (vol as f64 * 130.0).round();
                    app.apply_volume();
                }
                ui.add_space(8.0);

                // Loop toggle.
                if icon_button(ui, &p, 34.0, app.settings.loop_file, icons::loop_icon)
                    .on_hover_text("循环播放")
                    .clicked()
                {
                    app.settings.loop_file = !app.settings.loop_file;
                    app.apply_loop();
                }
                ui.add_space(8.0);

                // Speed chip cycles through the common rates.
                let speed_label = format!("{}×", trim_speed(app.settings.speed));
                if controls::chip(ui, &p, &speed_label, app.settings.speed != 1.0)
                    .on_hover_text("播放速度")
                    .clicked()
                {
                    let idx = SPEEDS
                        .iter()
                        .position(|s| (*s - app.settings.speed).abs() < 0.001)
                        .unwrap_or(3);
                    app.settings.speed = SPEEDS[(idx + 1) % SPEEDS.len()];
                    app.apply_speed();
                }
            });
        },
    );

    // Transient feedback (volume / zoom). Drawn over the transport because the
    // stage is a native surface: anything egui paints there stays hidden.
    if let Some(toast) = &app.toast {
        let alpha = toast.alpha();
        if alpha > 0.0 {
            let galley = ui.painter().layout_no_wrap(
                toast.text.clone(),
                egui::FontId::proportional(12.0),
                p.on_surface,
            );
            let size = vec2(galley.size().x + 26.0, 28.0);
            let rect = Rect::from_center_size(pos2(card.center().x, row2.center().y), size);
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(14),
                p.surface_container_highest.gamma_multiply(alpha),
            );
            ui.painter().galley(
                rect.center() - galley.size() * 0.5,
                galley,
                p.on_surface.gamma_multiply(alpha),
            );
        }
    }
}

fn trim_speed(speed: f64) -> String {
    if (speed.fract()).abs() < 0.001 {
        format!("{speed:.0}")
    } else {
        format!("{speed:.2}")
    }
}
