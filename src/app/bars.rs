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
            app.t("未载入", "Empty").to_string(),
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
        None => app.t("点击选择文件", "Click to choose a file").into(),
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
    if drag.drag_started() {
        crate::diag!("[window] titlebar drag");
        app.begin_window_move(ui.ctx());
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
            // Brand mark: the artwork from 图标.svg, rounded to the same radius
            // the Material rail uses.
            let (logo_rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
            let radius = CornerRadius::same(9);
            if app.brand.is_none() {
                app.brand = super::icon::brand_texture(ui.ctx());
            }
            match &app.brand {
                Some(tex) => {
                    let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0));
                    ui.painter()
                        .add(egui::Shape::image(tex.id(), logo_rect, uv, Color32::WHITE));
                    ui.painter().rect_stroke(
                        logo_rect,
                        radius,
                        Stroke::new(1.0_f32, p.outline_variant),
                        StrokeKind::Inside,
                    );
                }
                None => {
                    ui.painter()
                        .rect_filled(logo_rect, radius, p.primary_container);
                    let mut child = ui.new_child(
                        UiBuilder::new()
                            .max_rect(logo_rect)
                            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
                    );
                    icons::play(&mut child, 16.0, p.on_primary_container);
                }
            }

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
                let full = app.immersive();
                if icon_button(ui, &p, 38.0, full, icons::fullscreen)
                    .on_hover_text(app.t("全屏画面（F）", "Full-screen picture (F)"))
                    .clicked()
                {
                    app.toggle_immersive(ui.ctx());
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
    // The active pill is drawn by `rail` so it can slide between items; this
    // item only paints its own hover state on top of it.
    if hover > 0.0 && !active {
        let pill =
            Rect::from_center_size(pos2(rect.center().x, rect.min.y + 16.0), vec2(56.0, 32.0));
        ui.painter().rect_filled(
            pill,
            CornerRadius::same(16),
            p.surface_container_high.gamma_multiply(hover * 0.9),
        );
    }
    let fg = if active {
        p.on_secondary_container
    } else if response.hovered() {
        p.on_surface
    } else {
        p.on_surface_variant
    };
    let icon_rect =
        Rect::from_center_size(pos2(rect.center().x, rect.min.y + 16.0), Vec2::splat(20.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(icon_rect)
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    glyph(&mut child, 20.0, fg);
    ui.painter().text(
        pos2(rect.center().x, rect.max.y - 12.0),
        Align2::CENTER_CENTER,
        match tab {
            RailTab::Viewer => app.t("查看", "View"),
            RailTab::Settings => app.t("设置", "Settings"),
        },
        FontId::proportional(11.5),
        if active {
            p.on_surface
        } else {
            p.on_surface_variant
        },
    );
    if response.clicked() {
        app.rail = tab;
        if tab == RailTab::Viewer && app.settings.mode == CompareMode::Single {
            // Keep whatever comparison mode the toolbar has selected.
        }
    }
}

pub(super) fn rail(ui: &mut Ui, app: &mut MuxApp) {
    let p = app.palette.clone();
    let full = ui.max_rect();
    ui.painter()
        .rect_filled(full, CornerRadius::ZERO, p.surface);

    // Empty rail space behaves like title bar chrome: drag it to move.
    // Registered before the items so the items keep their own clicks.
    if !app.near_edge(ui.ctx())
        && ui
            .interact(full, ui.id().with("rail-drag"), Sense::click_and_drag())
            .drag_started()
    {
        crate::diag!("[window] chrome drag");
        app.begin_window_move(ui.ctx());
    }

    type Glyph = fn(&mut Ui, f32, Color32) -> egui::Response;
    const ITEM_H: f32 = 62.0;
    const GAP: f32 = 8.0;
    const TOP: f32 = 12.0;
    let items: [(RailTab, Glyph); 2] = [
        (RailTab::Viewer, icons::grid),
        (RailTab::Settings, icons::settings),
    ];
    let index = items
        .iter()
        .position(|(tab, _)| *tab == app.rail)
        .unwrap_or(0);
    let goal = full.min.y + TOP + index as f32 * (ITEM_H + GAP) + 16.0;
    let center_y = app.animate_rail_pill(goal, ui.ctx());
    let pill = Rect::from_center_size(pos2(full.center().x, center_y), vec2(56.0, 32.0));
    ui.painter()
        .rect_filled(pill, CornerRadius::same(16), p.secondary_container);

    ui.scope_builder(
        UiBuilder::new()
            .max_rect(full)
            .layout(Layout::top_down(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(TOP);
            for (tab, glyph) in items {
                rail_item(ui, app, tab, glyph);
                ui.add_space(GAP);
            }
        },
    );

    // Bottom cluster pinned to the rail foot.
    let foot = Rect::from_min_max(pos2(full.min.x, full.max.y - 96.0), full.max);
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(foot)
            .layout(Layout::top_down(Align::Center)),
        |ui| {
            ui.add_space(42.0);
            ui.painter().text(
                pos2(foot.center().x, foot.min.y + 74.0),
                Align2::CENTER_CENTER,
                format!("v{}", env!("CARGO_PKG_VERSION")),
                FontId::proportional(10.0),
                p.outline,
            );
        },
    );
}

/// Compare toolbar + the video stage rectangle.
pub(super) fn stage_area(ui: &mut Ui, app: &mut MuxApp) {
    let p = app.palette.clone();
    // The row is shared by chips on the left and controls on the right; when the
    // stage is narrow (playlist open, long English labels) the optional labels
    // have to give way instead of overlapping the buttons.
    let row_w = ui.available_width();
    let show_split_label = row_w > 1080.0;
    let show_percent = row_w > 1140.0;

    // Compare bar.
    let (bar_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::hover());
    if !app.near_edge(ui.ctx())
        && ui
            .interact(bar_rect, ui.id().with("bar-drag"), Sense::click_and_drag())
            .drag_started()
    {
        crate::diag!("[window] chrome drag");
        app.begin_window_move(ui.ctx());
    }
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(bar_rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            let mode_labels: Vec<&str> = CompareMode::ALL
                .iter()
                .map(|m| m.label(app.settings.lang))
                .collect();
            let mode_index = CompareMode::ALL
                .iter()
                .position(|m| *m == app.settings.mode)
                .unwrap_or(0);
            if let Some(pick) =
                controls::chip_row(ui, &p, "compare-modes", &mode_labels, mode_index)
                && pick != mode_index
            {
                app.set_mode(CompareMode::ALL[pick]);
            }
            ui.add_space(10.0);

            if matches!(app.settings.mode, CompareMode::Pair | CompareMode::Wipe) {
                let (sep, _) = ui.allocate_exact_size(vec2(1.0, 20.0), Sense::hover());
                ui.painter()
                    .rect_filled(sep, CornerRadius::same(1), p.outline_variant);
                ui.add_space(8.0);
                let axis_labels = [
                    SplitAxis::Horizontal.label(app.settings.lang),
                    SplitAxis::Vertical.label(app.settings.lang),
                ];
                let axis_index = usize::from(app.settings.axis == SplitAxis::Vertical);
                if let Some(pick) =
                    controls::chip_row(ui, &p, "split-axis", &axis_labels, axis_index)
                    && pick != axis_index
                {
                    app.settings.axis = if pick == 0 {
                        SplitAxis::Horizontal
                    } else {
                        SplitAxis::Vertical
                    };
                    app.settings_dirty = true;
                }
                if app.settings.mode == CompareMode::Wipe {
                    ui.add_space(10.0);
                    if show_split_label {
                        ui.label(
                            egui::RichText::new(app.t("分界", "Split"))
                                .size(12.0)
                                .color(p.on_surface_variant),
                        );
                        ui.add_space(6.0);
                    }
                    let mut wipe = app.settings.wipe;
                    if controls::slider(ui, &p, 150.0, &mut wipe, true).changed() {
                        app.settings.wipe = wipe;
                        app.settings_dirty = true;
                    }
                    if show_percent {
                        ui.label(
                            egui::RichText::new(format!("{:>3.0}%", app.settings.wipe * 100.0))
                                .size(11.0)
                                .monospace()
                                .color(p.on_surface_variant),
                        );
                    }
                    ui.add_space(14.0);
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Space still free after the left hand chips; the sync read-out
                // and the mode hint only appear when they will not collide.
                let room = ui.available_width();
                let cam = icon_button(ui, &p, 34.0, false, icons::camera);
                if cam.hovered() {
                    app.hover_hint = Some(app.t(
                        "截图：把当前帧存到「图片」文件夹（S）",
                        "Save the current frame to Pictures (S)",
                    ));
                }
                if cam.clicked() {
                    super::input::snapshot(app);
                }
                // 适应 / 填充: letterbox vs crop-to-fill the stage.
                {
                    let fill = app.settings.fill;
                    let fill_label = if fill { app.t("填充", "Fill") } else { app.t("适应", "Fit") };
                    let fill_chip = controls::chip(ui, &p, fill_label, fill);
                    if fill_chip.hovered() {
                        app.hover_hint = Some(app.t(
                            "适应＝完整显示（可能有黑边）；填充＝铺满画面（裁掉溢出）",
                            "Fit shows the whole frame (may letterbox); Fill crops to cover the stage",
                        ));
                    }
                    if fill_chip.clicked() {
                        app.settings.fill = !fill;
                        app.settings_dirty = true;
                        app.view_dirty();
                    }
                    ui.add_space(6.0);
                }
                if app.media.file_a.is_some() && app.media.file_b.is_some() {
                    // A labelled button instead of an icon with a tooltip: the
                    // native picture sits above egui, so any tooltip opening
                    // below the toolbar gets covered by the video.
                    if controls::pill_button(
                        ui,
                        &p,
                        app.t("交换 A / B", "Swap A / B"),
                        false,
                        Some(icons::swap),
                    )
                    .clicked()
                    {
                        app.swap_planes();
                    }
                    ui.add_space(6.0);
                }
                if app.settings.mode.needs_b() && app.media.file_b.is_none() {
                    if controls::pill_button(ui, &p, app.t("载入 B", "Load B"), false, Some(icons::playlist))
                        .clicked()
                    {
                        app.pick_files(ui.ctx(), Target::B);
                    }
                    ui.add_space(6.0);
                }
                if let Some(text) = drift_text(app)
                    && room > 330.0
                {
                    ui.label(
                        egui::RichText::new(text)
                            .size(11.0)
                            .monospace()
                            .color(p.on_surface_variant),
                    );
                }
                if app.settings.mode.needs_b() && room > 560.0 {
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new(app.settings.mode.hint(app.settings.lang))
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
        // Corner caption sits inside the stage but outside the video chrome.
        let caption = format!(
            "{} · {}",
            app.settings.mode.label(app.settings.lang),
            app.settings.mode.hint(app.settings.lang)
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
                egui::RichText::new(
                    app.t("拖入视频，或直接打开", "Drop a video here, or open one"),
                )
                .size(17.0)
                .strong()
                .color(p.on_surface),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(app.t(
                    "A 面作基准，B 面作对照 · mp4 / mkv / mov / webm / ts …",
                    "A is the reference, B the comparison · mp4 / mkv / mov / webm / ts …",
                ))
                .size(12.0)
                .color(p.on_surface_variant),
            );
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if controls::pill_button(
                    ui,
                    &p,
                    app.t("打开 A 面", "Open A"),
                    true,
                    Some(icons::play),
                )
                .clicked()
                {
                    app.pick_files(ui.ctx(), Target::A);
                }
                ui.add_space(8.0);
                if controls::pill_button(
                    ui,
                    &p,
                    app.t("打开 B 面", "Open B"),
                    false,
                    Some(icons::compare),
                )
                .clicked()
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
        return Some(app.t("等待 B 面", "Waiting for B").to_string());
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

    // Padding around the transport controls doubles as a drag handle.
    if ui
        .interact(
            card,
            ui.id().with("transport-drag"),
            Sense::click_and_drag(),
        )
        .drag_started()
    {
        crate::diag!("[window] chrome drag");
        app.begin_window_move(ui.ctx());
    }

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
            //
            // This must not stomp on a picture drag: `input` owns `scrub` there,
            // and clearing it every frame made the bar and the time label flip
            // between the drag target and the decoder position — the "seek bar
            // jumps while I scrub" report.
            if resp.dragging {
                app.scrub = target;
            } else if app.gesture.is_none() {
                app.scrub = None;
            }
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
            // The three transport buttons share one 48 px baseline, so the
            // small step buttons cannot drift off the play button's centre.
            const CLUSTER_H: f32 = 48.0;
            let tall = |ui: &mut Ui, w: f32, add: &mut dyn FnMut(&mut Ui)| {
                ui.allocate_ui_with_layout(
                    vec2(w, CLUSTER_H),
                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| add(ui),
                );
            };
            tall(ui, 34.0, &mut |ui| {
                let prev = icon_button(ui, &p, 34.0, false, icons::step_back);
                if prev.hovered() {
                    app.hover_hint = Some(app.t("上一帧（,）", "Previous frame (,)"));
                }
                if prev.clicked() {
                    app.step_frame(false);
                }
            });
            ui.add_space(6.0);
            tall(ui, 48.0, &mut |ui| {
                let play = play_button(ui, app);
                if play.hovered() {
                    app.hover_hint = Some(app.t("播放 / 暂停（空格）", "Play / pause (Space)"));
                }
                if play.clicked() {
                    app.toggle_play();
                }
            });
            ui.add_space(6.0);
            tall(ui, 34.0, &mut |ui| {
                let next = icon_button(ui, &p, 34.0, false, icons::step_forward);
                if next.hovered() {
                    app.hover_hint = Some(app.t("下一帧（.）", "Next frame (.)"));
                }
                if next.clicked() {
                    app.step_frame(true);
                }
            });
            ui.add_space(10.0);
            let info = if app.has_video() {
                format!(
                    "{} / {}",
                    app.settings.mode.label(app.settings.lang),
                    app.settings.axis.label(app.settings.lang)
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

            // Hover hints for the controls next to the stage: a floating
            // tooltip would be drawn under the native video planes, so the
            // hint is painted here instead, inside the transport card.
            if let Some(hint) = app.hover_hint {
                ui.add_space(10.0);
                ui.label(egui::RichText::new(hint).size(11.0).color(p.primary));
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Volume mute toggle.
                let mute = icon_button(
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
                if mute.clicked() {
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
                let loop_btn = icon_button(ui, &p, 34.0, app.settings.loop_file, icons::loop_icon);
                if loop_btn.hovered() {
                    app.hover_hint = Some(app.t("循环播放", "Loop"));
                }
                if loop_btn.clicked() {
                    app.settings.loop_file = !app.settings.loop_file;
                    app.apply_loop();
                }
                ui.add_space(8.0);

                // Speed chip cycles through the common rates.
                let speed_label = format!("{}×", trim_speed(app.settings.speed));
                let speed_chip = controls::chip(ui, &p, &speed_label, app.settings.speed != 1.0);
                if speed_chip.hovered() {
                    app.hover_hint = Some(app.t("播放速度", "Playback speed"));
                }
                if speed_chip.clicked() {
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
