#![windows_subsystem = "windows"]
//! Standalone Material You showcase for the MuxVideo design system.
//!
//! Run with `cargo run --bin theme_showcase`. The window is the living spec for
//! [`muxvideo::ui::widgets`]: seed swatches, the full role grid, tonal ramps,
//! every button family, rail items, chips, cards, switches, sliders, the type
//! scale and snackbars — all driven by one seed colour and the light/dark
//! toggle in the title bar.
//!
//! Like the app, this binary calls `muxvideo::fonts::install` **before**
//! `muxvideo::ui::install_style`: the style maps egui text styles to font sizes
//! and expects the Microsoft YaHei CJK fallback to be registered first.

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Margin, Pos2, Sense, Stroke, StrokeKind, Vec2,
};
use muxvideo::ui::dynamic_color::{TONES, relative_luminance};
use muxvideo::ui::widgets::{
    self, CardKind, Icon, assist_chip, body, card, display, divider, filter_chip, headline,
    icon_button, icon_button_toggle, label, label_small, linear_progress, nav_rail_item,
    paint_icon, segmented, shape, slider, snackbar, space, switch, text_button, title, title_small,
};
use muxvideo::ui::{Palette, Role, Scheme, TonalPalette, install_style, wavy_slider};

const SEEDS: [Color32; 5] = [
    Color32::from_rgb(0x67, 0x50, 0xA4), // baseline violet
    Color32::from_rgb(0x00, 0x68, 0x74), // teal
    Color32::from_rgb(0x7D, 0x52, 0x60), // rose
    Color32::from_rgb(0x4C, 0x66, 0x2B), // moss
    Color32::from_rgb(0x8C, 0x4A, 0x60), // berry
];

const DESTINATIONS: [(Icon, &str); 3] = [
    (Icon::Compare, "对比"),
    (Icon::Play, "播放"),
    (Icon::Settings, "设置"),
];

fn main() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MuxVideo Theme Showcase")
            .with_icon(window_icon())
            .with_inner_size([1200.0, 840.0])
            .with_min_inner_size([960.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "MuxVideo Theme Showcase",
        options,
        Box::new(|cc| Ok(Box::new(Showcase::new(cc)) as Box<dyn eframe::App>)),
    )
    .map_err(|err| anyhow::anyhow!("theme showcase failed: {err}"))?;
    Ok(())
}

struct Showcase {
    seed_index: usize,
    scheme: Scheme,
    palette: Palette,
    rail: usize,
    mode: usize,
    sync: bool,
    waveform: bool,
    opacity: f32,
    zoom: f32,
    wavy_paused: f32,
    wavy_playing: f32,
    wavy_start: f32,
    wavy_end: f32,
    filter_a: bool,
    undos: u32,
}

impl Showcase {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Fonts first: install_style sizes text styles, and the CJK fallback
        // must exist before any label is laid out.
        if let Some(face) = muxvideo::fonts::load_face() {
            muxvideo::fonts::install(&cc.egui_ctx, face);
        }
        let palette = Palette::from_seed(SEEDS[0], Scheme::Dark);
        install_style(&cc.egui_ctx, &palette);
        Self {
            seed_index: 0,
            scheme: Scheme::Dark,
            palette,
            rail: 0,
            mode: 0,
            sync: true,
            waveform: false,
            opacity: 0.62,
            zoom: 1.0,
            wavy_paused: 0.5,
            wavy_playing: 0.5,
            wavy_start: 0.0,
            wavy_end: 1.0,
            filter_a: true,
            undos: 0,
        }
    }

    fn rebuild(&mut self, ctx: &egui::Context) {
        self.palette = Palette::from_seed(SEEDS[self.seed_index], self.scheme);
        install_style(ctx, &self.palette);
    }

    fn cycle_seed(&mut self, ctx: &egui::Context) {
        self.seed_index = (self.seed_index + 1) % SEEDS.len();
        self.rebuild(ctx);
    }
}

impl eframe::App for Showcase {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let palette = self.palette.clone();

        egui::TopBottomPanel::top("showcase_top")
            .frame(
                egui::Frame::NONE
                    .fill(palette.surface)
                    .inner_margin(Margin::symmetric(24, 16)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(headline(&palette, "Material You"));
                    ui.add_space(space::SM);
                    ui.label(title_small(&palette, "MuxVideo 主题预览 Theme showcase"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut scheme_index = if self.scheme == Scheme::Dark { 0 } else { 1 };
                        if segmented(
                            ui,
                            &palette,
                            "showcase_scheme",
                            &["深色 Dark", "浅色 Light"],
                            &mut scheme_index,
                        )
                        .changed()
                        {
                            self.scheme = if scheme_index == 0 {
                                Scheme::Dark
                            } else {
                                Scheme::Light
                            };
                            self.rebuild(ui.ctx());
                        }
                        ui.add_space(space::SM);
                        if widgets::tonal_button(ui, &palette, "换种子 Cycle seed").clicked() {
                            self.cycle_seed(ui.ctx());
                        }
                    });
                });
            });

        egui::SidePanel::left("showcase_rail")
            .exact_width(96.0)
            .frame(
                egui::Frame::NONE
                    .fill(palette.surface_container_lowest)
                    .inner_margin(Margin::symmetric(8, 16)),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    for (index, (icon, name)) in DESTINATIONS.iter().enumerate() {
                        if nav_rail_item(ui, &palette, *icon, name, self.rail == index).clicked() {
                            self.rail = index;
                        }
                        ui.add_space(space::XS);
                    }
                    ui.add_space(space::XL);
                    divider(ui, &palette);
                    ui.add_space(space::SM);
                    ui.vertical_centered(|ui| {
                        ui.label(label_small(&palette, "M3"));
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(palette.surface)
                    .inner_margin(Margin::symmetric(24, 20)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let seed = self.palette.seed;
                        ui.horizontal(|ui| {
                            for (index, candidate) in SEEDS.iter().enumerate() {
                                let (rect, response) =
                                    ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
                                let painter = ui.painter();
                                painter.circle_filled(rect.center(), 12.0, *candidate);
                                if index == self.seed_index {
                                    painter.circle_stroke(
                                        rect.center(),
                                        15.0,
                                        Stroke::new(2.0_f32, palette.on_surface),
                                    );
                                }
                                if response.clicked() {
                                    self.seed_index = index;
                                    self.rebuild(ui.ctx());
                                }
                                response.on_hover_text(hex(*candidate));
                            }
                            ui.add_space(space::SM);
                            ui.label(body(
                                &palette,
                                format!("Seed {} · Scheme {}", hex(seed), self.scheme.label()),
                            ));
                        });

                        section(ui, &palette, "颜色角色", "Colour roles", |ui| {
                            ui.label(label_small(
                                &palette,
                                "35 个 Material 3 角色 · 由同一个 seed 生成，悬停查看十六进制值",
                            ));
                            ui.add_space(space::SM);
                            ui.horizontal_wrapped(|ui| {
                                for role in Role::ALL {
                                    role_swatch(ui, &palette, role);
                                }
                            });
                        });

                        section(ui, &palette, "色调梯", "Tonal ramps", |ui| {
                            ui.label(label_small(
                                &palette,
                                "每个色板 11 档 Tone 0–100 · 色调 Tone 40 是浅色方案的主色",
                            ));
                            ui.add_space(space::SM);
                            let ramps: [(&str, TonalPalette); 6] = [
                                ("Primary 主色", palette.palettes.primary),
                                ("Secondary 次色", palette.palettes.secondary),
                                ("Tertiary 第三色", palette.palettes.tertiary),
                                ("Neutral 中性色", palette.palettes.neutral),
                                ("Neutral variant 变体", palette.palettes.neutral_variant),
                                ("Error 错误色", palette.palettes.error),
                            ];
                            for (name, tonal) in ramps {
                                ui.horizontal(|ui| {
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(140.0, 28.0),
                                        Layout::left_to_right(Align::Center),
                                        |ui| {
                                            ui.label(label(&palette, name));
                                        },
                                    );
                                    for tone in TONES {
                                        let color = tonal.tone(tone);
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::new(36.0, 28.0),
                                            Sense::hover(),
                                        );
                                        let painter = ui.painter();
                                        painter.rect_filled(rect, shape::radius(shape::XS), color);
                                        let text_color = readable_on(color);
                                        painter.text(
                                            rect.center(),
                                            Align2::CENTER_CENTER,
                                            tone.to_string(),
                                            FontId::proportional(9.0),
                                            text_color,
                                        );
                                    }
                                });
                                ui.add_space(space::XS);
                            }
                        });

                        section(ui, &palette, "按钮", "Buttons", |ui| {
                            ui.horizontal_wrapped(|ui| {
                                let _ = widgets::filled_button(ui, &palette, "已填充按钮 Filled");
                                let _ = widgets::tonal_button(ui, &palette, "色调按钮 Tonal");
                                let _ = widgets::elevated_button(ui, &palette, "阴影按钮 Elevated");
                                let _ = widgets::outlined_button(ui, &palette, "描边按钮 Outlined");
                                let _ = widgets::text_button(ui, &palette, "文本按钮 Text");
                            });
                            ui.add_space(space::SM);
                            ui.add_enabled_ui(false, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    let _ = widgets::filled_button(ui, &palette, "禁用 Disabled");
                                    let _ = widgets::outlined_button(ui, &palette, "禁用 Disabled");
                                });
                            });
                            ui.add_space(space::MD);
                            ui.horizontal_wrapped(|ui| {
                                for icon in [
                                    Icon::Play,
                                    Icon::Pause,
                                    Icon::SkipBack,
                                    Icon::SkipForward,
                                    Icon::Volume,
                                    Icon::Search,
                                    Icon::Plus,
                                    Icon::Close,
                                ] {
                                    icon_button(ui, &palette, icon)
                                        .on_hover_text(format!("{icon:?}"));
                                }
                                icon_button_toggle(ui, &palette, Icon::Contrast, true)
                                    .on_hover_text("选中状态 Selected");
                            });
                        });

                        section(ui, &palette, "选择", "Selection", |ui| {
                            ui.horizontal(|ui| {
                                ui.allocate_ui_with_layout(
                                    Vec2::new(120.0, 40.0),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        ui.label(label(&palette, "对比模式 Mode"));
                                    },
                                );
                                let _ = segmented(
                                    ui,
                                    &palette,
                                    "showcase_mode",
                                    &["A 面", "B 面", "Wipe 划像"],
                                    &mut self.mode,
                                );
                            });
                            ui.add_space(space::SM);
                            ui.horizontal(|ui| {
                                let _ = switch(ui, &palette, &mut self.sync);
                                ui.label(body(&palette, "对比同步 Sync"));
                                ui.add_space(space::XL);
                                let _ = switch(ui, &palette, &mut self.waveform);
                                ui.label(body(&palette, "显示波形 Waveform"));
                            });
                        });

                        section(ui, &palette, "滑杆", "Slider", |ui| {
                            ui.label(label_small(
                                &palette,
                                "4 px 轨道 · 20 px 手柄 · 悬停状态层 Hover state layer",
                            ));
                            ui.add_space(space::SM);
                            ui.horizontal(|ui| {
                                ui.allocate_ui_with_layout(
                                    Vec2::new(120.0, 44.0),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        ui.label(label(&palette, "不透明度 Opacity"));
                                    },
                                );
                                let _ = slider(ui, &palette, &mut self.opacity, 0.0..=1.0, 320.0);
                                ui.label(body(&palette, format!("{:>3.0}%", self.opacity * 100.0)));
                            });
                            ui.horizontal(|ui| {
                                ui.allocate_ui_with_layout(
                                    Vec2::new(120.0, 44.0),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        ui.label(label(&palette, "缩放 Zoom"));
                                    },
                                );
                                let _ = slider(ui, &palette, &mut self.zoom, 0.5..=4.0, 320.0);
                                ui.label(body(&palette, format!("{:.2}×", self.zoom)));
                            });
                        });

                        section(ui, &palette, "波浪滑杆", "Wavy slider", |ui| {
                            ui.label(label_small(
                                &palette,
                                "Expressive · 8 px 轨道 · 振幅 Amplitude 6 px · 76 px 波长 · 46 px/s 滚动 · 竖长药丸把手 Pill handle",
                            ));
                            ui.add_space(space::SM);
                            wavy_demo_row(
                                ui,
                                &palette,
                                "暂停 50% Paused",
                                "wavy-paused",
                                &mut self.wavy_paused,
                                false,
                            );
                            wavy_demo_row(
                                ui,
                                &palette,
                                "播放中 50% Playing",
                                "wavy-playing",
                                &mut self.wavy_playing,
                                true,
                            );
                            wavy_demo_row(
                                ui,
                                &palette,
                                "起点 0% Start",
                                "wavy-start",
                                &mut self.wavy_start,
                                false,
                            );
                            wavy_demo_row(
                                ui,
                                &palette,
                                "终点 100% End",
                                "wavy-end",
                                &mut self.wavy_end,
                                false,
                            );
                        });

                        section(ui, &palette, "卡片", "Cards", |ui| {
                            ui.columns(3, |columns| {
                                for (index, (kind, name, copy, icon)) in [
                                    (
                                        CardKind::Filled,
                                        "已填充卡片 Filled",
                                        "承载主要信息：A/B 两路画面的元数据与时间码。",
                                        Icon::Compare,
                                    ),
                                    (
                                        CardKind::Elevated,
                                        "阴影卡片 Elevated",
                                        "适合浮起的控制面板，比如播放列表、章节与收藏夹。",
                                        Icon::Film,
                                    ),
                                    (
                                        CardKind::Outlined,
                                        "描边卡片 Outlined",
                                        "信息密度高的区域：设置项、快捷键与当前文件信息。",
                                        Icon::Settings,
                                    ),
                                ]
                                .into_iter()
                                .enumerate()
                                {
                                    card(&mut columns[index], &palette, kind, |ui| {
                                        ui.set_width(280.0);
                                        ui.set_min_height(150.0);
                                        let (rect, _) = ui
                                            .allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                                        paint_icon(ui.painter(), icon, rect, palette.primary);
                                        ui.add_space(space::SM);
                                        ui.label(title_small(&palette, name));
                                        ui.add_space(space::XS);
                                        ui.label(body(&palette, copy));
                                        ui.add_space(space::MD);
                                        let _ = text_button(ui, &palette, "查看详情 View");
                                    });
                                }
                            });
                        });

                        section(ui, &palette, "标签", "Chips", |ui| {
                            ui.horizontal_wrapped(|ui| {
                                let _ =
                                    assist_chip(ui, &palette, Some(Icon::Volume), "音量 Volume");
                                let _ = assist_chip(ui, &palette, Some(Icon::Film), "片源 Source");
                                let _ = assist_chip(ui, &palette, None, "同步 Sync");
                                let _ = filter_chip(ui, &palette, "只看 A 面", &mut self.filter_a);
                                let mut selected = true;
                                let _ = filter_chip(ui, &palette, "已选中 Selected", &mut selected);
                            });
                        });

                        section(ui, &palette, "排版", "Typography", |ui| {
                            egui::Grid::new("showcase_type")
                                .num_columns(2)
                                .spacing([24.0, 10.0])
                                .show(ui, |ui| {
                                    ui.label(label(&palette, "Display 36"));
                                    ui.label(display(&palette, "MuxVideo 影音对比"));
                                    ui.end_row();
                                    ui.label(label(&palette, "Headline 28"));
                                    ui.label(headline(&palette, "双画面同步比较"));
                                    ui.end_row();
                                    ui.label(label(&palette, "Title 22"));
                                    ui.label(title(&palette, "已载入 A 面 · 4K HDR"));
                                    ui.end_row();
                                    ui.label(label(&palette, "Title small 16"));
                                    ui.label(title_small(&palette, "章节 Chapters · 12"));
                                    ui.end_row();
                                    ui.label(label(&palette, "Body 14"));
                                    ui.label(body(
                                        &palette,
                                        "正文用于说明与元数据，14 px，行高与 8 px 网格对齐。",
                                    ));
                                    ui.end_row();
                                    ui.label(label(&palette, "Label 12"));
                                    ui.label(label(&palette, "标签 LABEL · 12 PX"));
                                    ui.end_row();
                                    ui.label(label_small(&palette, "Label small 11"));
                                    ui.label(label_small(&palette, "辅助标注 CAPTION · 11 PX"));
                                    ui.end_row();
                                });
                        });

                        section(ui, &palette, "其他", "Misc", |ui| {
                            ui.label(label(&palette, "线性进度 Linear progress"));
                            ui.add_space(space::XS);
                            ui.allocate_ui_with_layout(
                                Vec2::new(560.0, 4.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    let _ = linear_progress(ui, &palette, self.opacity, 560.0);
                                },
                            );
                            ui.add_space(space::MD);
                            ui.label(body(
                                &palette,
                                "悬停在文字上查看 Tooltip · 样式来自 window / menu visuals",
                            ))
                            .on_hover_text("Material You tooltip");
                            ui.add_space(space::MD);
                            ui.allocate_ui_with_layout(
                                Vec2::new(560.0, 48.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    let result = snackbar(
                                        ui,
                                        &palette,
                                        "已保存主题 Saved theme",
                                        Some("撤销"),
                                    );
                                    if let Some(action) = result.action
                                        && action.clicked()
                                    {
                                        self.undos += 1;
                                    }
                                    let _ = result.bar;
                                },
                            );
                            if self.undos > 0 {
                                ui.add_space(space::XS);
                                ui.label(label_small(
                                    &palette,
                                    format!("已撤销 Undone ×{}", self.undos),
                                ));
                            }
                        });

                        ui.add_space(space::XL);
                        divider(ui, &palette);
                        ui.add_space(space::SM);
                        ui.label(label_small(
                            &palette,
                            "Material You · 无第三方依赖 · CIELAB/HCT 近似 · 种子驱动",
                        ));
                        ui.add_space(space::XL);
                    });
            });
    }
}

fn section(
    ui: &mut egui::Ui,
    palette: &Palette,
    title_cn: &str,
    title_en: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    ui.add_space(space::XL);
    ui.label(title(palette, format!("{title_cn} {title_en}")));
    ui.add_space(space::SM);
    add(ui);
    ui.add_space(space::LG);
    divider(ui, palette);
}

/// One labelled [`wavy_slider`] row: name, slider, live percentage, state.
fn wavy_demo_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    name: &str,
    salt: &str,
    value: &mut f32,
    animated: bool,
) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(190.0, 40.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(label(palette, name));
            },
        );
        let out = wavy_slider(ui, palette, salt, 360.0, value, animated);
        ui.add_space(space::SM);
        ui.allocate_ui_with_layout(
            Vec2::new(56.0, 40.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(body(palette, format!("{:>3.0}%", *value * 100.0)));
            },
        );
        ui.add_space(space::SM);
        let status = if out.dragging {
            "拖拽中 Dragging"
        } else if animated {
            "播放中 Playing"
        } else {
            "已暂停 Paused"
        };
        ui.allocate_ui_with_layout(
            Vec2::new(120.0, 40.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(label_small(palette, status));
            },
        );
    });
}

fn role_swatch(ui: &mut egui::Ui, palette: &Palette, role: Role) {
    let color = role.color(palette);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(132.0, 56.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, shape::radius(shape::SM), color);
    painter.rect_stroke(
        rect,
        shape::radius(shape::SM),
        Stroke::new(1.0_f32, Palette::alpha(palette.outline_variant, 0.6)),
        StrokeKind::Inside,
    );
    let text_color = readable_on(color);
    painter.text(
        Pos2::new(rect.left() + 8.0, rect.top() + 7.0),
        Align2::LEFT_TOP,
        role.name(),
        FontId::proportional(9.5),
        text_color,
    );
    painter.text(
        Pos2::new(rect.left() + 8.0, rect.bottom() - 7.0),
        Align2::LEFT_BOTTOM,
        hex(color),
        FontId::proportional(9.0),
        text_color,
    );
    response.on_hover_text(format!("{} · {}", role.name(), hex(color)));
}

fn readable_on(color: Color32) -> Color32 {
    if relative_luminance(color) > 0.45 {
        Color32::from_black_alpha(230)
    } else {
        Color32::from_white_alpha(235)
    }
}

fn hex(color: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
}

/// Generated window icon: a rounded baseline-violet square with a play glyph.
fn window_icon() -> egui::IconData {
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    let center = SIZE as f32 / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = x as f32 + 0.5 - center;
            let py = y as f32 + 0.5 - center;
            if rounded_square(px, py, 15.0, 7.0) > 0.0 {
                continue;
            }
            let color = if point_in_triangle(px, py) {
                [0xFF, 0xFF, 0xFF, 0xFF]
            } else {
                [0x67, 0x50, 0xA4, 0xFF]
            };
            let index = ((y * SIZE + x) * 4) as usize;
            rgba[index..index + 4].copy_from_slice(&color);
        }
    }
    egui::IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    }
}

fn rounded_square(x: f32, y: f32, half: f32, radius: f32) -> f32 {
    let qx = x.abs() - (half - radius);
    let qy = y.abs() - (half - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

fn point_in_triangle(x: f32, y: f32) -> bool {
    let (ax, ay) = (-5.0_f32, -8.0_f32);
    let (bx, by) = (-5.0_f32, 8.0_f32);
    let (cx, cy) = (8.0_f32, 0.0_f32);
    let d1 = (x - bx) * (ay - by) - (ax - bx) * (y - by);
    let d2 = (x - cx) * (by - cy) - (bx - cx) * (y - cy);
    let d3 = (x - ax) * (cy - ay) - (cx - ax) * (y - ay);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}
