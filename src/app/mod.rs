//! MuxVideo application shell: chrome, layout, playback actions and wiring.

mod bars;
mod controls;
pub mod icons;
mod open_dialog;
mod panels;
pub mod stage;
pub mod state;
mod window;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::mpv::{LibMpv, MpvEvent, MpvOptions};
use crate::ui::Palette;
use crate::video::childwin::find_window_by_title;

use state::{CompareMode, PlayEntry, RailTab, Settings, SplitAxis, Toast};

/// Height of the custom title bar.
pub(super) const TITLEBAR_H: f32 = 56.0;
/// Width of the navigation rail.
pub(super) const RAIL_W: f32 = 88.0;
/// Height of the transport panel.
pub(super) const TRANSPORT_H: f32 = 112.0;
/// Width of the playlist side panel.
pub(super) const PLAYLIST_W: f32 = 328.0;

/// Which plane a freshly opened file goes to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    /// A when empty, otherwise B while comparing, otherwise A.
    Auto,
    A,
    B,
}

pub struct MuxApp {
    pub(super) palette: Palette,
    pub(super) settings: Settings,
    pub(super) rail: RailTab,
    pub(super) lib: Option<Arc<LibMpv>>,
    pub(super) boot_error: Option<String>,

    pub(super) media: stage::Media,
    pub(super) playlist: Vec<PlayEntry>,
    pub(super) selected: Option<usize>,

    /// Video area in egui points, filled in while drawing the stage.
    pub(super) stage_rect: egui::Rect,
    /// Stage rect actually handed to the native planes last frame.
    pub(super) stage_reported: bool,

    pub(super) toast: Option<Toast>,
    pub(super) logs: Vec<String>,
    pub(super) frame_no: u64,
    pub(super) settings_dirty: bool,
    last_save: Instant,

    /// Scrub position while the progress slider is being dragged.
    pub(super) scrub: Option<f64>,
    /// Files to open on the first frame, when the native window already exists.
    pending: Vec<(PathBuf, Target)>,
    /// Whether the DWM frame attributes were already applied.
    frame_dressed: bool,
}

impl MuxApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::fonts::install(&cc.egui_ctx);
        let settings = Settings::load();
        let palette = Palette::from_seed(settings.seed_color(), settings.scheme());
        crate::ui::install_style(&cc.egui_ctx, &palette);

        let (lib, boot_error) = match LibMpv::load() {
            Ok(lib) => (Some(lib), None),
            Err(err) => (None, Some(format!("{err:#}"))),
        };

        let playlist: Vec<PlayEntry> = settings
            .recent
            .iter()
            .filter_map(|p| {
                let path = std::path::Path::new(p);
                if !path.is_absolute() || !path.is_file() || !state::is_video_path(path) {
                    return None;
                }
                Some(PlayEntry::new(path.to_path_buf()))
            })
            .collect();

        let palette_scheme = settings.dark;

        let mut app = Self {
            palette,
            settings,
            rail: RailTab::Viewer,
            lib,
            boot_error,
            media: stage::Media::default(),
            playlist,
            selected: None,
            stage_rect: egui::Rect::ZERO,
            stage_reported: false,
            toast: None,
            logs: Vec::new(),
            frame_no: 0,
            settings_dirty: false,
            last_save: Instant::now(),
            scrub: None,
            pending: Vec::new(),
            frame_dressed: false,
        };
        app.palette = Palette::from_seed(app.settings.seed_color(), app.settings.scheme());
        crate::ui::install_style(&cc.egui_ctx, &app.palette);
        let _ = palette_scheme;

        // Files given on the command line: first goes to A, the rest queue for B.
        let mut target = Target::Auto;
        for arg in std::env::args().skip(1) {
            let path = PathBuf::from(arg);
            if path.is_file() {
                app.pending.push((path, target));
                target = Target::B;
            }
        }
        app
    }

    // ── palette / style ───────────────────────────────────────────────────

    pub(super) fn rebuild_palette(&mut self, ctx: &egui::Context) {
        self.palette = Palette::from_seed(self.settings.seed_color(), self.settings.scheme());
        crate::ui::install_style(ctx, &self.palette);
        self.settings_dirty = true;
    }

    // ── playback actions ──────────────────────────────────────────────────

    pub(super) fn mpv_options(&self) -> MpvOptions {
        MpvOptions {
            hwdec: self.settings.hwdec.clone(),
            loop_file: self.settings.loop_file,
            ..MpvOptions::default()
        }
    }

    pub(super) fn active_player(&self) -> Option<&crate::mpv::MpvPlayer> {
        self.media
            .player_a
            .as_ref()
            .or(self.media.player_b.as_ref())
    }

    pub(super) fn duration(&self) -> f64 {
        self.active_player().map(|p| p.duration()).unwrap_or(0.0)
    }

    pub(super) fn time_pos(&self) -> f64 {
        self.scrub
            .unwrap_or_else(|| self.active_player().map(|p| p.time_pos()).unwrap_or(0.0))
    }

    pub(super) fn is_playing(&self) -> bool {
        self.active_player().map(|p| !p.paused()).unwrap_or(false)
    }

    pub(super) fn has_video(&self) -> bool {
        self.media.player_a.is_some() || self.media.player_b.is_some()
    }

    /// One place where players are created, so volume/speed/loop stay in sync.
    fn configure(&self, player: &crate::mpv::MpvPlayer) {
        player.set_volume(self.settings.volume);
        player.set_mute(self.settings.muted);
        player.set_speed(self.settings.speed);
        player.set_property(
            "loop-file",
            if self.settings.loop_file { "inf" } else { "no" },
        );
    }

    fn apply_to_all(&self, f: impl Fn(&crate::mpv::MpvPlayer)) {
        for player in [
            self.media.player_a.as_ref(),
            self.media.player_b.as_ref(),
            self.media.player_d.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            f(player);
        }
    }

    pub(super) fn toggle_play(&mut self) {
        let Some(master) = self
            .media
            .player_a
            .as_ref()
            .or(self.media.player_b.as_ref())
        else {
            return;
        };
        let paused = master.paused();
        let next = !paused;
        master.set_pause(next);
        if let Some(b) = &self.media.player_b {
            b.set_pause(next);
        }
    }

    pub(super) fn set_paused(&mut self, paused: bool) {
        self.apply_to_all(|p| p.set_pause(paused));
    }

    pub(super) fn seek_to(&mut self, secs: f64) {
        let clamped = secs.max(0.0);
        for player in [self.media.player_a.as_ref(), self.media.player_b.as_ref()]
            .into_iter()
            .flatten()
        {
            player.seek_absolute(clamped);
        }
        if let Some(d) = &self.media.player_d {
            d.seek_absolute(clamped);
        }
        self.scrub = None;
    }

    pub(super) fn seek_by(&mut self, delta: f64) {
        if !self.has_video() {
            return;
        }
        let target = self.time_pos() + delta;
        self.seek_to(target);
    }

    pub(super) fn step_frame(&mut self, forward: bool) {
        let Some(master) = self
            .media
            .player_a
            .as_ref()
            .or(self.media.player_b.as_ref())
        else {
            return;
        };
        master.set_pause(true);
        if forward {
            master.frame_step();
        } else {
            master.frame_back_step();
        }
        let pos = master.time_pos();
        if let Some(b) = &self.media.player_b {
            b.set_pause(true);
            b.seek_absolute(pos);
        }
    }

    pub(super) fn apply_volume(&mut self) {
        let volume = self.settings.volume;
        let mute = self.settings.muted;
        self.apply_to_all(|p| {
            p.set_volume(volume);
            p.set_mute(mute);
        });
        self.settings_dirty = true;
    }

    pub(super) fn apply_speed(&mut self) {
        let speed = self.settings.speed;
        self.apply_to_all(|p| p.set_speed(speed));
        self.settings_dirty = true;
    }

    pub(super) fn apply_loop(&mut self) {
        let value = if self.settings.loop_file { "inf" } else { "no" };
        self.apply_to_all(|p| {
            p.set_property("loop-file", value);
        });
        self.settings_dirty = true;
    }

    pub(super) fn swap_planes(&mut self) {
        std::mem::swap(&mut self.media.file_a, &mut self.media.file_b);
        std::mem::swap(&mut self.media.player_a, &mut self.media.player_b);
    }

    // ── opening media ─────────────────────────────────────────────────────

    pub(super) fn pick_files(&mut self, ctx: &egui::Context, target: Target) {
        let owner = find_window_by_title("MuxVideo").unwrap_or(0);
        let initial = if self.settings.last_dir.is_empty() {
            None
        } else {
            Some(self.settings.last_dir.as_str())
        };
        let files = open_dialog::pick_videos(owner, initial);
        if files.is_empty() {
            return;
        }
        let mut target = target;
        for path in files {
            self.open_path(path, target);
            target = Target::B;
        }
        ctx.request_repaint();
    }

    /// Path of a playlist entry, for comparing against the loaded planes.
    pub(super) fn playlist_path(&self, index: usize) -> Option<&std::path::Path> {
        self.playlist.get(index).map(|e| e.path.as_path())
    }

    /// Adds files to the playlist and opens them on `target`.
    pub(super) fn open_path(&mut self, path: PathBuf, target: Target) {
        let Some(lib) = self.lib.clone() else {
            self.toast = Some(Toast::error("libmpv 未加载，无法播放"));
            return;
        };

        // Track in the playlist and in the recent list.
        let path = std::path::absolute(&path).unwrap_or(path);
        let path_str = path.to_string_lossy().to_string();
        if !self.playlist.iter().any(|e| e.path == path) {
            self.playlist.push(PlayEntry::new(path.clone()));
        }
        self.settings.recent.retain(|p| p != &path_str);
        self.settings.recent.insert(0, path_str);
        self.settings.recent.truncate(12);
        self.settings.last_dir = path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        self.settings_dirty = true;

        let target = match target {
            Target::Auto => {
                if self.media.file_a.is_none() {
                    Target::A
                } else if self.settings.mode.needs_b() && self.media.file_b.is_none() {
                    Target::B
                } else {
                    Target::A
                }
            }
            other => other,
        };

        let opts = self.mpv_options();
        let result = match target {
            Target::B => self.media.open_b(&lib, &path, &opts),
            _ => self.media.open_a(&lib, &path, &opts),
        };
        match result {
            Ok(()) => {
                if let Some(player) = match target {
                    Target::B => self.media.player_b.as_ref(),
                    _ => self.media.player_a.as_ref(),
                } {
                    self.configure(player);
                }
                self.toast = Some(Toast::info(format!(
                    "{} → {} 面",
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    if target == Target::B { "B" } else { "A" }
                )));
                if target == Target::B {
                    // Align B with A and hold both on the same frame.
                    let pos = self
                        .media
                        .player_a
                        .as_ref()
                        .map(|p| p.time_pos())
                        .unwrap_or(0.0);
                    if let Some(b) = &self.media.player_b {
                        b.seek_absolute(pos);
                        b.set_pause(true);
                    }
                    if let Some(a) = &self.media.player_a {
                        a.set_pause(true);
                    }
                }
                self.ensure_difference();
            }
            Err(err) => {
                self.toast = Some(Toast::error(format!("载入失败：{err:#}")));
            }
        }
    }

    /// Builds the lavfi difference plane when the mode needs it.
    pub(super) fn ensure_difference(&mut self) {
        if self.settings.mode != CompareMode::Difference {
            return;
        }
        let (Some(lib), Some(a), Some(b)) = (
            self.lib.clone(),
            self.media.file_a.clone(),
            self.media.file_b.clone(),
        ) else {
            return;
        };
        if self.media.player_d.is_some() {
            return;
        }
        let opts = self.mpv_options();
        match self.media.open_difference(&lib, &a, &b, &opts) {
            Ok(()) => {
                self.set_paused(true);
            }
            Err(err) => {
                self.logs.push(format!("差值模式启动失败：{err:#}"));
                self.toast = Some(Toast::error("差值模式启动失败，已回退到单画面"));
            }
        }
    }

    pub(super) fn set_mode(&mut self, mode: CompareMode) {
        if self.settings.mode == mode {
            return;
        }
        self.settings.mode = mode;
        self.settings_dirty = true;
        match mode {
            CompareMode::Single => {
                self.media.close_difference();
            }
            CompareMode::Difference => {
                self.ensure_difference();
            }
            _ => {
                self.media.close_difference();
            }
        }
        // Comparing always starts paused and aligned.
        if mode.needs_b() && self.media.player_b.is_some() {
            let pos = self
                .media
                .player_a
                .as_ref()
                .map(|p| p.time_pos())
                .unwrap_or(0.0);
            if let Some(b) = &self.media.player_b {
                b.seek_absolute(pos);
            }
            self.set_paused(true);
        }
    }

    // ── events ────────────────────────────────────────────────────────────

    fn pump_events(&mut self, ctx: &egui::Context) {
        for (plane, event) in self.media.pump() {
            match event {
                MpvEvent::FileLoaded { .. } => {
                    match plane {
                        'A' => self.media.refresh_meta(stage::Plane::A),
                        'B' => self.media.refresh_meta(stage::Plane::B),
                        _ => {
                            if self.media.note_diff_loaded() && !self.media.apply_difference_graph()
                            {
                                self.toast = Some(Toast::error(
                                    "差值模式：mpv 拒绝了 lavfi-complex 混合图".to_string(),
                                ));
                            }
                            ctx.request_repaint();
                        }
                    }
                    ctx.request_repaint();
                }
                MpvEvent::EndFile { reason } => {
                    if reason.contains("error") {
                        self.toast = Some(Toast::error(format!("{plane} 面播放出错：{reason}")));
                    }
                }
                MpvEvent::PropertyChanged { name, value } => {
                    if std::env::var_os("MUXVIDEO_VERBOSE").is_some() {
                        self.logs.push(format!("{plane}:{name}={value}"));
                    }
                }
                MpvEvent::Log { level, text } => {
                    if self.logs.len() < 400 {
                        self.logs.push(format!("{plane} {level}: {text}"));
                    }
                    if level.eq_ignore_ascii_case("error") {
                        self.toast = Some(Toast::error(text));
                        ctx.request_repaint();
                    }
                }
                MpvEvent::PlaybackRestart => {}
            }
        }
    }

    // ── keyboard ──────────────────────────────────────────────────────────

    fn handle_keys(&mut self, ctx: &egui::Context) {
        use egui::Key;

        let pressed = |key: Key| ctx.input(|i| i.key_pressed(key));

        if pressed(Key::Space) {
            self.toggle_play();
        }
        if pressed(Key::ArrowLeft) {
            self.seek_by(-5.0);
        }
        if pressed(Key::ArrowRight) {
            self.seek_by(5.0);
        }
        if pressed(Key::J) {
            self.seek_by(-10.0);
        }
        if pressed(Key::L) {
            self.seek_by(10.0);
        }
        if pressed(Key::ArrowUp) {
            self.settings.volume = (self.settings.volume + 5.0).min(130.0);
            self.apply_volume();
        }
        if pressed(Key::ArrowDown) {
            self.settings.volume = (self.settings.volume - 5.0).max(0.0);
            self.apply_volume();
        }
        if pressed(Key::Comma) {
            self.step_frame(false);
        }
        if pressed(Key::Period) {
            self.step_frame(true);
        }
        if pressed(Key::M) {
            self.settings.muted = !self.settings.muted;
            self.apply_volume();
        }
        if pressed(Key::Home) {
            self.seek_to(0.0);
        }
        if pressed(Key::Num1) {
            self.set_mode(CompareMode::Single);
        }
        if pressed(Key::Num2) {
            self.set_mode(CompareMode::Pair);
        }
        if pressed(Key::Num3) {
            self.set_mode(CompareMode::Wipe);
        }
        if pressed(Key::Num4) {
            self.set_mode(CompareMode::Difference);
        }
        if pressed(Key::Tab) {
            self.settings.show_playlist = !self.settings.show_playlist;
            self.settings_dirty = true;
        }
        if pressed(Key::F) {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
        }
        if pressed(Key::Escape) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        if pressed(Key::OpenBracket) {
            self.settings.wipe = (self.settings.wipe - 0.02).clamp(0.0, 1.0);
        }
        if pressed(Key::CloseBracket) {
            self.settings.wipe = (self.settings.wipe + 0.02).clamp(0.0, 1.0);
        }
    }

    // ── per-frame sync ────────────────────────────────────────────────────

    /// Undecorated windows need their own resize borders.
    fn resize_borders(&self, ctx: &egui::Context) {
        let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        let rect = ctx.content_rect();
        const EDGE: f32 = 6.0;
        let west = pointer.x <= rect.left() + EDGE;
        let east = pointer.x >= rect.right() - EDGE;
        let north = pointer.y <= rect.top() + EDGE;
        let south = pointer.y >= rect.bottom() - EDGE;
        use egui::viewport::ResizeDirection as Dir;
        let (dir, cursor) = match (west, east, north, south) {
            (true, _, true, _) => (Dir::NorthWest, egui::CursorIcon::ResizeNwSe),
            (true, _, _, true) => (Dir::SouthWest, egui::CursorIcon::ResizeNeSw),
            (_, true, true, _) => (Dir::NorthEast, egui::CursorIcon::ResizeNeSw),
            (_, true, _, true) => (Dir::SouthEast, egui::CursorIcon::ResizeNwSe),
            (true, ..) => (Dir::West, egui::CursorIcon::ResizeHorizontal),
            (_, true, ..) => (Dir::East, egui::CursorIcon::ResizeHorizontal),
            (_, _, true, _) => (Dir::North, egui::CursorIcon::ResizeVertical),
            (_, _, _, true) => (Dir::South, egui::CursorIcon::ResizeVertical),
            _ => return,
        };
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }

    fn sync_stage(&mut self, ctx: &egui::Context) {
        let hidden = self.rail == RailTab::Settings;
        let rect = if hidden || !self.stage_reported {
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO)
        } else {
            self.stage_rect
        };
        let ppp = ctx.pixels_per_point();
        self.media.sync_layout(
            rect,
            ppp,
            self.settings.mode,
            self.settings.axis,
            self.settings.wipe,
        );
        self.media.sync_playback(self.settings.mode);
        self.stage_reported = false;
    }

    fn maybe_save(&mut self) {
        if self.settings_dirty && self.last_save.elapsed() > Duration::from_millis(700) {
            self.settings.save();
            self.settings_dirty = false;
            self.last_save = Instant::now();
        }
    }

    fn toast_layer(&mut self, ctx: &egui::Context) {
        let Some(toast) = self.toast.clone() else {
            return;
        };
        let alpha = toast.alpha();
        if alpha <= 0.0 {
            self.toast = None;
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(120));
        let anchor = egui::Align2::LEFT_BOTTOM;
        // Keep the snackbar above the transport bar so it never covers controls.
        let lift = if self.rail == RailTab::Settings {
            24.0
        } else {
            TRANSPORT_H + 16.0
        };
        egui::Area::new(egui::Id::new("mux-toast"))
            .anchor(anchor, egui::vec2(RAIL_W + 16.0, -lift))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let fill = if toast.error {
                    self.palette.error_container
                } else {
                    self.palette.surface_container_highest
                };
                let fg = if toast.error {
                    self.palette.on_error_container
                } else {
                    self.palette.on_surface
                };
                egui::Frame::NONE
                    .fill(fill.gamma_multiply(alpha))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::symmetric(18, 12))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&toast.text)
                                .size(13.0)
                                .color(fg.gamma_multiply(alpha)),
                        );
                    });
            });
    }
}

impl eframe::App for MuxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_no += 1;
        self.resize_borders(ctx);

        // Command-line files open on the first frame, once the native window exists.
        let pending = std::mem::take(&mut self.pending);
        for (path, target) in pending {
            self.open_path(path, target);
        }

        // Native window polish: rounded DWM corners, done once the HWND exists.
        if !self.frame_dressed
            && let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE)
        {
            window::apply_modern_frame(hwnd);
            self.frame_dressed = true;
        }

        // Files dropped onto the window.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            let mut target = Target::Auto;
            for path in dropped {
                self.open_path(path, target);
                target = Target::B;
            }
        }

        if !ctx.wants_keyboard_input() {
            self.handle_keys(ctx);
        }
        self.pump_events(ctx);

        let palette = self.palette.clone();
        let surface = palette.surface;

        egui::TopBottomPanel::top("titlebar")
            .exact_height(TITLEBAR_H)
            .frame(egui::Frame::NONE.fill(surface))
            .show(ctx, |ui| bars::titlebar(ui, self));

        if self.rail != RailTab::Settings {
            egui::TopBottomPanel::bottom("transport")
                .exact_height(TRANSPORT_H)
                .frame(
                    egui::Frame::NONE
                        .fill(surface)
                        .inner_margin(egui::Margin::symmetric(20, 8)),
                )
                .show(ctx, |ui| bars::transport(ui, self));
        }

        egui::SidePanel::left("rail")
            .exact_width(RAIL_W)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(surface))
            .show(ctx, |ui| bars::rail(ui, self));

        if self.settings.show_playlist && self.rail != RailTab::Settings {
            egui::SidePanel::right("playlist")
                .exact_width(PLAYLIST_W)
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(surface)
                        .inner_margin(egui::Margin::symmetric(8, 4)),
                )
                .show(ctx, |ui| panels::playlist(ui, self));
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(surface)
                    .inner_margin(egui::Margin::symmetric(16, 0)),
            )
            .show(ctx, |ui| match self.rail {
                RailTab::Settings => panels::settings(ui, self),
                _ => bars::stage_area(ui, self),
            });

        self.sync_stage(ctx);
        self.toast_layer(ctx);
        self.maybe_save();

        let playing = self.is_playing();
        ctx.request_repaint_after(if playing {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(100)
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.settings.save();
    }
}

/// Exposed for the boot error banner and diagnostics.
impl MuxApp {
    pub(super) fn boot_error(&self) -> Option<&str> {
        self.boot_error.as_deref()
    }
}

#[allow(dead_code)]
fn _unused(_: SplitAxis) {}
