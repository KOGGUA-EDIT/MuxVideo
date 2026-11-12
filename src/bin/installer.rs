//! MuxVideo setup wizard.
//!
//! A small native installer that looks like the app it installs: same Material
//! You palette, same chip rows and sliders, same rounded frame. It copies the
//! executable next to a `libmpv-2.dll`, optionally plants a desktop shortcut,
//! registers an uninstall entry and can start the player when it is done.
//!
//! * Release setup: built with `--features payload`, which embeds the player and
//!   libmpv straight into this binary (see `tools/build_setup.ps1`).
//! * Dev run: without the feature the wizard picks the files up from its own
//!   directory, so `cargo run --bin installer` works too.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, RichText, Sense, Stroke, Ui, Vec2,
    pos2, vec2,
};

use muxvideo::app::controls;
use muxvideo::app::icons;
use muxvideo::ui::{Palette, Scheme, install_style};

const APP_NAME: &str = "MuxVideo";
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Keep the animated install screen visible for at least this long.
const MIN_INSTALL_SCREEN: std::time::Duration = std::time::Duration::from_millis(1300);
const WINDOW_W: f32 = 760.0;
const WINDOW_H: f32 = 560.0;

#[cfg(feature = "payload")]
static PAYLOAD_EXE: &[u8] = include_bytes!("../../assets/payload/muxvideo.exe");
#[cfg(feature = "payload")]
static PAYLOAD_MPV: &[u8] = include_bytes!("../../assets/payload/libmpv-2.dll");
#[cfg(feature = "payload")]
static PAYLOAD_README: &[u8] = include_bytes!("../../assets/payload/README.txt");

/// Which screen the wizard is on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Options,
    Installing,
    Done,
}

/// What the worker thread reports back.
enum Progress {
    Stage(String, f32),
    Finished(Result<PathBuf, String>),
}

struct Installer {
    palette: Palette,
    step: Step,
    target: String,
    desktop_shortcut: bool,
    launch_after: bool,
    /// 0..1, driven by the worker; eased here so the bar never jumps.
    progress: f32,
    shown: f32,
    status: String,
    error: Option<String>,
    worker: Option<Receiver<Progress>>,
    pending: Option<Result<PathBuf, String>>,
    finished_copy: Option<Instant>,
    installed: Option<PathBuf>,
    finished_at: Option<Instant>,
    brand: Option<egui::TextureHandle>,
    started: Instant,
    /// Set once the DWM frame has been matched to the app's rounded style.
    dressed: bool,
}

impl Installer {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        if let Some(face) = muxvideo::fonts::load_face() {
            muxvideo::fonts::install(&cc.egui_ctx, face);
        }
        let palette = Palette::from_seed(Color32::from_rgb(0xD0, 0xBC, 0xFF), Scheme::Dark);
        install_style(&cc.egui_ctx, &palette);
        let brand = muxvideo::app::icon::brand_texture(&cc.egui_ctx);
        Self {
            palette,
            step: Step::Welcome,
            target: default_target_dir().display().to_string(),
            desktop_shortcut: true,
            launch_after: true,
            progress: 0.0,
            shown: 0.0,
            status: String::new(),
            error: None,
            worker: None,
            pending: None,
            finished_copy: None,
            installed: None,
            finished_at: None,
            brand,
            started: Instant::now(),
            dressed: false,
        }
    }

    fn begin_install(&mut self, ctx: &egui::Context) {
        let target = PathBuf::from(self.target.trim());
        let desktop = self.desktop_shortcut;
        let (tx, rx) = channel();
        self.worker = Some(rx);
        self.step = Step::Installing;
        self.error = None;
        self.progress = 0.0;
        self.shown = 0.0;
        self.status.clear();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let report = |name: &str, frac: f32| {
                let _ = tx.send(Progress::Stage(name.to_owned(), frac));
            };
            let result = install_to(&target, desktop, &report);
            match result {
                Ok(path) => {
                    let _ = tx.send(Progress::Finished(Ok(path)));
                }
                Err(err) => {
                    let _ = tx.send(Progress::Finished(Err(err)));
                }
            }
            ctx.request_repaint();
        });
    }

    /// Reads whatever the worker queued and advances the eased progress.
    fn pump(&mut self, ctx: &egui::Context) {
        let mut finished = None;
        if let Some(rx) = &self.worker {
            while let Ok(message) = rx.try_recv() {
                match message {
                    Progress::Stage(name, frac) => {
                        self.status = name;
                        self.progress = frac;
                    }
                    Progress::Finished(result) => finished = Some(result),
                }
            }
        }
        // Hold the animated screen for a moment even when the copy is instant,
        // otherwise the wizard flashes and the wave never shows.
        if let Some(result) = finished {
            self.worker = None;
            self.progress = 1.0;
            self.pending = Some(result);
            self.finished_copy = Some(Instant::now());
        }
        if let Some(result) = self.pending.clone()
            && self
                .finished_copy
                .is_some_and(|at| at.elapsed() >= MIN_INSTALL_SCREEN)
        {
            self.pending = None;
            self.finished_copy = None;
            match result {
                Ok(path) => {
                    self.installed = Some(path);
                    self.finished_at = Some(Instant::now());
                    self.step = Step::Done;
                    if self.launch_after
                        && let Some(exe) = &self.installed
                    {
                        let _ = std::process::Command::new(exe).spawn();
                    }
                }
                Err(err) => {
                    self.error = Some(err);
                    self.step = Step::Options;
                }
            }
        }
        // Ease the visible value towards the reported one: fast, then settle.
        let delta = self.progress - self.shown;
        if delta.abs() > 0.0005 {
            self.shown += delta * 0.18;
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        } else {
            self.shown = self.progress;
        }
        if self.step == Step::Installing {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}

impl eframe::App for Installer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.dressed
            && let Some(hwnd) = muxvideo::video::childwin::find_window_by_title("MuxVideo Setup")
        {
            muxvideo::app::window::apply_modern_frame(hwnd, false);
            self.dressed = true;
        }
        self.pump(ctx);
        let p = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(p.surface))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                header(ui, &p, self.brand.as_ref());
                let body = egui::Rect::from_min_max(
                    pos2(full.min.x + 36.0, full.min.y + 116.0),
                    pos2(full.max.x - 36.0, full.max.y - 96.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                    match self.step {
                        Step::Welcome => self.welcome(ui),
                        Step::Options => self.options(ui),
                        Step::Installing => self.installing(ui, ctx),
                        Step::Done => self.done(ui),
                    }
                });
                footer(ui, &p, self.step, self.started.elapsed().as_secs_f32());
                if let Some(err) = self.error.clone() {
                    ui.painter().text(
                        pos2(full.center().x, full.max.y - 66.0),
                        Align2::CENTER_CENTER,
                        err,
                        FontId::proportional(11.5),
                        p.error,
                    );
                }
            });
    }
}

impl Installer {
    fn welcome(&mut self, ui: &mut Ui) {
        let p = self.palette.clone();
        ui.add_space(26.0);
        ui.label(
            RichText::new(format!("{APP_NAME} {VERSION}"))
                .size(34.0)
                .strong()
                .color(p.on_surface),
        );
        ui.add_space(6.0);
        ui.label(RichText::new(INTRO).size(13.0).color(p.on_surface_variant));
        ui.add_space(26.0);
        for (title, detail) in FEATURES {
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                ui.painter()
                    .circle_filled(dot.center(), 13.0, p.primary_container);
                let tick = [
                    pos2(dot.center().x - 5.5, dot.center().y + 0.5),
                    pos2(dot.center().x - 1.5, dot.center().y + 4.5),
                    pos2(dot.center().x + 6.0, dot.center().y - 4.5),
                ];
                ui.painter().add(egui::Shape::line(
                    tick.to_vec(),
                    Stroke::new(2.0_f32, p.on_primary_container),
                ));
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).size(14.0).color(p.on_surface));
                    ui.label(RichText::new(detail).size(11.5).color(p.on_surface_variant));
                });
            });
            ui.add_space(14.0);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if controls::pill_button(ui, &p, "继续", true, None).clicked() {
                self.step = Step::Options;
            }
            ui.add_space(10.0);
            if controls::text_button(ui, &p, "取消").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn options(&mut self, ui: &mut Ui) {
        let p = self.palette.clone();
        ui.label(
            RichText::new("安装选项")
                .size(26.0)
                .strong()
                .color(p.on_surface),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new("选择安装位置，以及安装完成后要做的事。")
                .size(12.5)
                .color(p.on_surface_variant),
        );
        ui.add_space(22.0);

        ui.label(
            RichText::new("安装位置")
                .size(12.0)
                .color(p.on_surface_variant),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let width = ui.available_width() - 92.0;
            ui.allocate_ui_with_layout(
                vec2(width, 40.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    let frame = egui::Frame::NONE
                        .fill(p.surface_container_low)
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(egui::Margin::symmetric(12, 10));
                    frame.show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.target)
                                .desired_width(f32::INFINITY)
                                .frame(false)
                                .text_color(p.on_surface),
                        );
                    });
                },
            );
            ui.add_space(8.0);
            if controls::pill_button(ui, &p, "浏览…", false, None).clicked()
                && let Some(picked) = pick_folder(&self.target)
            {
                self.target = picked;
            }
        });
        ui.add_space(18.0);

        ui.horizontal(|ui| {
            controls::switch(ui, &p, &mut self.desktop_shortcut);
            ui.add_space(10.0);
            ui.label(
                RichText::new("在桌面创建快捷方式")
                    .size(13.0)
                    .color(p.on_surface),
            );
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            controls::switch(ui, &p, &mut self.launch_after);
            ui.add_space(10.0);
            ui.label(
                RichText::new("安装完成后启动 MuxVideo")
                    .size(13.0)
                    .color(p.on_surface),
            );
        });
        ui.add_space(26.0);
        ui.label(
            RichText::new(format!("将写入：{}", self.target.trim()))
                .size(11.0)
                .color(p.outline),
        );
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            if controls::pill_button(ui, &p, "安装", true, None).clicked() {
                self.begin_install(ui.ctx());
            }
            ui.add_space(10.0);
            if controls::text_button(ui, &p, "上一步").clicked() {
                self.step = Step::Welcome;
            }
        });
    }

    fn installing(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let p = self.palette.clone();
        ui.add_space(60.0);
        ui.label(
            RichText::new("正在安装…")
                .size(26.0)
                .strong()
                .color(p.on_surface),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(if self.status.is_empty() {
                "准备文件…"
            } else {
                self.status.as_str()
            })
            .size(13.0)
            .color(p.on_surface_variant),
        );
        ui.add_space(26.0);
        wavy_progress(ui, &p, self.shown, ctx);
        ui.add_space(12.0);
        ui.label(
            RichText::new(format!("{:.0}%", self.shown * 100.0))
                .size(12.0)
                .color(p.outline),
        );
    }

    fn done(&mut self, ui: &mut Ui) {
        let p = self.palette.clone();
        let t = self
            .finished_at
            .map(|at| at.elapsed().as_secs_f32())
            .unwrap_or(1.0);
        let pop = (t / 0.35).clamp(0.0, 1.0);
        let scale = 0.6 + 0.4 * ease_out(pop);
        ui.add_space(46.0);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 30.0 * scale, p.primary_container);
            let inner = egui::Rect::from_center_size(rect.center(), vec2(30.0, 30.0) * scale);
            ui.painter()
                .circle_filled(inner.center(), 15.0 * scale, p.primary);
            let tick = [
                pos2(rect.center().x - 6.5 * scale, rect.center().y + 0.5 * scale),
                pos2(rect.center().x - 1.8 * scale, rect.center().y + 5.2 * scale),
                pos2(rect.center().x + 7.0 * scale, rect.center().y - 5.0 * scale),
            ];
            ui.painter().add(egui::Shape::line(
                tick.to_vec(),
                Stroke::new(2.4 * scale, p.on_primary),
            ));
            ui.add_space(16.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("安装完成")
                        .size(26.0)
                        .strong()
                        .color(p.on_surface),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new(
                        self.installed
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_default(),
                    )
                    .size(11.5)
                    .color(p.outline),
                );
            });
        });
        ui.add_space(28.0);
        ui.label(
            RichText::new("你已经可以双击桌面快捷方式开始使用，或直接在开始菜单里搜索 MuxVideo。")
                .size(12.5)
                .color(p.on_surface_variant),
        );
        ui.add_space(22.0);
        ui.horizontal(|ui| {
            if controls::pill_button(ui, &p, "启动 MuxVideo", true, None).clicked()
                && let Some(exe) = &self.installed
            {
                let _ = std::process::Command::new(exe).spawn();
            }
            ui.add_space(10.0);
            if controls::text_button(ui, &p, "完成").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}

const INTRO: &str = "一个用 Rust 写的双画面视频对比播放器：进程内 libmpv、Material You 界面。";
const FEATURES: [(&str, &str); 3] = [
    (
        "并排 / 划像 / 闪烁",
        "同一个时间点对比两个版本：左右或上下分屏、拖把手划像、轮流闪烁找差异。",
    ),
    (
        "逐帧擦洗与完整时间码",
        "画面里左右拖动 = 逐帧前后移动，时间码到帧（HH:MM:SS:FF），面向剪辑与动画。",
    ),
    (
        "轻量、无外部依赖",
        "只带一个 libmpv DLL，不需要 VLC / mpv 安装，绿色安装到任意目录。",
    ),
];

/// Draws the animated wavy progress bar used while copying files.
fn wavy_progress(ui: &mut Ui, p: &Palette, value: f32, ctx: &egui::Context) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
    let track = egui::Rect::from_center_size(rect.center(), vec2(width - 8.0, 8.0));
    ui.painter()
        .rect_filled(track, CornerRadius::same(4), p.surface_container_high);
    let value = value.clamp(0.0, 1.0);
    let end = track.min.x + track.width() * value;
    if end - track.min.x < 2.0 {
        return;
    }
    let phase = ctx.input(|i| i.time) as f32 * 3.0;
    let amplitude = 4.0;
    let wavelength = 48.0;
    let steps = ((end - track.min.x) / 3.0).ceil() as usize + 2;
    let points: Vec<_> = (0..=steps)
        .map(|index| {
            let x = track.min.x + (end - track.min.x) * index as f32 / steps as f32;
            let fade = ((end - x) / 24.0).min(1.0) * ((x - track.min.x) / 12.0).min(1.0);
            let y = rect.center().y
                + (phase + (x - track.min.x) / wavelength * std::f32::consts::TAU).sin()
                    * amplitude
                    * fade;
            pos2(x, y)
        })
        .collect();
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(8.0_f32, p.primary)));
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// The window chrome: brand block, title, close button.
fn header(ui: &mut Ui, p: &Palette, brand: Option<&egui::TextureHandle>) {
    let full = ui.max_rect();
    let bar = egui::Rect::from_min_size(full.min, vec2(full.width(), 72.0));
    let drag = ui.interact(bar, ui.id().with("drag"), Sense::click_and_drag());
    if drag.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    ui.painter().rect_filled(bar, CornerRadius::ZERO, p.surface);
    if let Some(texture) = brand {
        let logo =
            egui::Rect::from_min_size(pos2(bar.min.x + 28.0, bar.min.y + 18.0), vec2(36.0, 36.0));
        ui.painter().add(egui::Shape::image(
            texture.id(),
            logo,
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        ));
    }
    ui.painter().text(
        pos2(bar.min.x + 76.0, bar.center().y),
        Align2::LEFT_CENTER,
        format!("{APP_NAME} Setup"),
        FontId::proportional(14.0),
        p.on_surface,
    );
    let close = egui::Rect::from_min_size(pos2(bar.max.x - 46.0, bar.min.y), vec2(46.0, 46.0));
    let response = ui.interact(close, ui.id().with("close"), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(close, CornerRadius::same(8), p.error_container);
    }
    icons::close(
        ui,
        15.0,
        if response.hovered() {
            p.on_error_container
        } else {
            p.on_surface_variant
        },
    );
    if response.clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// Footer: version on the left, a thin progress of the flow on the right.
fn footer(ui: &mut Ui, p: &Palette, step: Step, elapsed: f32) {
    let full = ui.max_rect();
    let bar = egui::Rect::from_min_max(
        pos2(full.min.x, full.max.y - 40.0),
        pos2(full.max.x, full.max.y),
    );
    ui.painter().text(
        pos2(bar.min.x + 28.0, bar.center().y),
        Align2::LEFT_CENTER,
        format!("v{VERSION}"),
        FontId::proportional(11.0),
        p.outline,
    );
    let index = match step {
        Step::Welcome => 0.0,
        Step::Options => 1.0,
        Step::Installing => 2.0,
        Step::Done => 3.0,
    };
    let total = 3.0;
    let width = 132.0;
    let x = bar.max.x - 28.0 - width;
    for i in 0..=total as usize {
        let cx = x + width * i as f32 / total;
        let active = i as f32 <= index;
        ui.painter().circle_filled(
            pos2(cx, bar.center().y),
            if active { 3.2 } else { 2.4 },
            if active { p.primary } else { p.outline_variant },
        );
    }
    let _ = elapsed;
}

/// Default install directory: `%LOCALAPPDATA%\Programs\MuxVideo`.
fn default_target_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Program Files"));
    base.join("Programs").join(APP_NAME)
}

/// Asks Windows for a folder (PowerShell keeps this dependency-free).
fn pick_folder(start: &str) -> Option<String> {
    let script = format!(
        r#"Add-Type -AssemblyName System.Windows.Forms | Out-Null
$d = New-Object System.Windows.Forms.FolderBrowserDialog
$d.Description = '选择 MuxVideo 的安装位置'
$d.SelectedPath = '{}'
$d.ShowNewFolderButton = $true
if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ [Console]::Write($d.SelectedPath) }}"#,
        start.replace('\'', "''")
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-STA", "-Command", &script])
        .output()
        .ok()?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if path.is_empty() { None } else { Some(path) }
}

/// Copies the player, creates the shortcut and registers the uninstall entry.
fn install_to(target: &Path, desktop: bool, report: &dyn Fn(&str, f32)) -> Result<PathBuf, String> {
    std::fs::create_dir_all(target).map_err(|err| format!("无法创建目录：{err}"))?;

    report("复制 libmpv-2.dll", 0.10);
    write_file(target.join("libmpv-2.dll"), "libmpv-2.dll", 1_000_000)?;
    report("复制 muxvideo.exe", 0.55);
    write_file(target.join("muxvideo.exe"), "muxvideo.exe", 1_000_000)?;
    report("写入说明文件", 0.72);
    write_file(target.join("README.txt"), "README.txt", 1_000_000)?;

    let exe = target.join("muxvideo.exe");
    report("创建快捷方式", 0.80);
    if desktop && let Some(link) = desktop_dir().map(|dir| dir.join("MuxVideo.lnk")) {
        create_shortcut(&link, &exe, target)?;
    }
    create_shortcut(
        &start_menu_dir()
            .ok_or("找不到开始菜单目录".to_owned())?
            .join("MuxVideo.lnk"),
        &exe,
        target,
    )?;

    report("登记卸载信息", 0.90);
    register_uninstall(target, &exe);

    report("完成", 1.0);
    Ok(exe)
}

/// Writes one payload file, either from the embedded blob or from disk.
fn write_file(target: PathBuf, name: &str, _hint: u32) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("无法创建目录：{err}"))?;
    }
    #[cfg(feature = "payload")]
    {
        let bytes: &[u8] = match name {
            "muxvideo.exe" => PAYLOAD_EXE,
            "libmpv-2.dll" => PAYLOAD_MPV,
            _ => PAYLOAD_README,
        };
        std::fs::write(&target, bytes).map_err(|err| format!("写入 {name} 失败：{err}"))
    }
    #[cfg(not(feature = "payload"))]
    {
        let source = std::env::current_exe()
            .map_err(|err| format!("找不到安装包位置：{err}"))?
            .parent()
            .ok_or("安装包目录无效".to_owned())?
            .join(name);
        std::fs::copy(&source, &target).map_err(|err| {
            format!("复制 {name} 失败：{err}（开发模式请在安装包目录放置 {name}）")
        })?;
        Ok(())
    }
}

fn desktop_dir() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE").map(PathBuf::from)?;
    let desktop = profile.join("Desktop");
    if desktop.is_dir() {
        Some(desktop)
    } else {
        None
    }
}

fn start_menu_dir() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from)?;
    let dir = appdata
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");
    if dir.is_dir() { Some(dir) } else { None }
}

/// Creates a `.lnk` through PowerShell's WScript.Shell object.
fn create_shortcut(link: &Path, exe: &Path, workdir: &Path) -> Result<(), String> {
    let script = format!(
        r#"$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}')
$s.TargetPath = '{}'
$s.WorkingDirectory = '{}'
$s.IconLocation = '{},0'
$s.Description = 'MuxVideo'
$s.Save()"#,
        link.display(),
        exe.display(),
        workdir.display(),
        exe.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .status()
        .map_err(|err| format!("创建快捷方式失败：{err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("创建快捷方式失败".to_owned())
    }
}

/// Adds the app to "Apps & features" with a self-deleting uninstall command.
fn register_uninstall(target: &Path, exe: &Path) {
    let uninstall = format!(
        "powershell -NoProfile -Command \"Remove-Item -LiteralPath '{}' -Recurse -Force; Remove-Item -LiteralPath \\\"$env:USERPROFILE\\Desktop\\MuxVideo.lnk\\\" -Force -ErrorAction SilentlyContinue; Remove-Item -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\MuxVideo' -Recurse -Force\"",
        target.display()
    );
    let script = format!(
        r#"$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MuxVideo'
New-Item -Path $key -Force | Out-Null
Set-ItemProperty -Path $key -Name DisplayName -Value 'MuxVideo'
Set-ItemProperty -Path $key -Name DisplayVersion -Value '{}'
Set-ItemProperty -Path $key -Name DisplayIcon -Value '{}'
Set-ItemProperty -Path $key -Name InstallLocation -Value '{}'
Set-ItemProperty -Path $key -Name Publisher -Value 'KOGGUA-EDIT'
Set-ItemProperty -Path $key -Name UninstallString -Value '{}'
Set-ItemProperty -Path $key -Name NoModify -Value 1 -Type DWord
Set-ItemProperty -Path $key -Name NoRepair -Value 1 -Type DWord"#,
        VERSION,
        exe.display(),
        target.display(),
        uninstall.replace('\'', "''")
    );
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .status();
}

fn main() -> eframe::Result<()> {
    let icon = muxvideo::app::icon::window_icon().map(std::sync::Arc::new);
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(Vec2::new(WINDOW_W, WINDOW_H))
        .with_min_inner_size(Vec2::new(WINDOW_W, WINDOW_H))
        .with_max_inner_size(Vec2::new(WINDOW_W, WINDOW_H))
        .with_decorations(false)
        .with_resizable(false)
        .with_title(format!("{APP_NAME} Setup"));
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        &format!("{APP_NAME} Setup"),
        options,
        Box::new(|cc| Ok(Box::new(Installer::new(cc)))),
    )
}
