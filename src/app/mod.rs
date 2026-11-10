//! MuxVideo application shell: chrome, layout, playback actions and wiring.

mod bars;
mod controls;
pub mod diag;
pub mod i18n;
pub mod icon;
pub mod icons;
mod input;
mod open_dialog;
mod panels;
pub mod stage;
pub mod state;
mod window;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::Color32;

use crate::mpv::{LibMpv, MpvEvent, MpvOptions};
use crate::ui::Palette;
use crate::video::childwin::find_window_by_title;

use state::{CompareMode, PlayEntry, RailTab, Settings, SplitAxis, Toast};

/// Height of the custom title bar.
pub(super) const TITLEBAR_H: f32 = 56.0;
/// Width of the navigation rail.
pub(super) const RAIL_W: f32 = 88.0;
/// Height of the transport panel.
pub(super) const TRANSPORT_H: f32 = 128.0;
/// Width of the playlist side panel.
pub(super) const PLAYLIST_W: f32 = 328.0;
/// How long flicker comparison holds one input before switching (seconds).
const BLINK_HALF_SECS: f64 = 0.45;

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
    /// Title-bar brand texture, created on first paint.
    pub(super) brand: Option<egui::TextureHandle>,
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
    /// Gesture in progress on a native video surface.
    pub(super) gesture: Option<input::Drag>,
    /// Set after a double click so its trailing press is not read as a click.
    /// Picture pan/zoom (RV style), applied to every plane.
    pub(super) pan: (f64, f64),
    pub(super) zoom: f64,
    /// Rail pill slide animation (fast off the mark, easing out).
    pub(super) rail_pill: f32,
    pub(super) rail_pill_from: f32,
    pub(super) rail_pill_goal: f32,
    pub(super) rail_pill_t0: Instant,
    pub(super) rail_pill_ready: bool,
    /// Signature of the last applied view so pan/zoom/fit only re-sends on change.
    view_key: Option<(bool, i32, i32, u64, u64, u64, u8)>,
    /// Files to open on the first frame, when the native window already exists.
    pending: Vec<(PathBuf, Target)>,
    /// Whether the DWM frame attributes were already applied.
    frame_dressed: bool,
    /// Which `apply_modern_frame` flavour the window currently uses.
    frame_immersive: bool,
    /// Outer rectangle (in points) to restore when leaving immersive playback.
    saved_outer: Option<(f32, f32, f32, f32)>,
    /// Whether the window was maximized before going immersive.
    saved_maximized: bool,
    /// Manual window move / resize in progress (see `begin_window_move`).
    window_move: Option<WindowMove>,
    /// Last completed click in the picture, for our own double-click window.
    last_click: Option<(std::time::Instant, i32, i32)>,
    /// Hint shown in the transport bar for the widget under the pointer.
    /// Tooltips cannot be used next to the stage: the native video planes sit
    /// above egui and swallow anything drawn over the picture.
    pub(super) hover_hint: Option<&'static str>,
    /// Last known window rectangle, for change logging.
    last_viewport: Option<egui::Rect>,
    /// Start of the flicker clock (CompareMode::Blink).
    blink_clock: Instant,
    /// Immersive playback: chrome hidden, picture owns the window.
    immersive: bool,
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
            brand: None,
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
            gesture: None,
            pan: (0.0, 0.0),
            zoom: 0.0,
            rail_pill: 0.0,
            rail_pill_from: 0.0,
            rail_pill_goal: 0.0,
            rail_pill_t0: Instant::now(),
            rail_pill_ready: false,
            view_key: None,
            pending: Vec::new(),
            frame_dressed: false,
            frame_immersive: false,
            saved_outer: None,
            saved_maximized: false,
            window_move: None,
            last_click: None,
            hover_hint: None,
            last_viewport: None,
            blink_clock: Instant::now(),
            immersive: false,
        };
        crate::diag!(
            "[boot] version={} exe={:?}",
            env!("CARGO_PKG_VERSION"),
            std::env::current_exe().ok()
        );
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

    /// Active-language text; see [i18n].
    pub(super) fn t(&self, zh: &'static str, en: &'static str) -> &'static str {
        i18n::t(self.settings.lang, zh, en)
    }

    /// Eases the rail pill towards `goal` (cubic ease-out: quick start, gentle
    /// landing — the curve the reference screenshot shows) and returns its
    /// current centre y.
    pub(super) fn animate_rail_pill(&mut self, goal: f32, ctx: &egui::Context) -> f32 {
        const DURATION: f32 = 0.34;
        if !self.rail_pill_ready {
            self.rail_pill = goal;
            self.rail_pill_goal = goal;
            self.rail_pill_from = goal;
            self.rail_pill_ready = true;
            return goal;
        }
        if (self.rail_pill_goal - goal).abs() > 0.5 {
            self.rail_pill_from = self.rail_pill;
            self.rail_pill_goal = goal;
            self.rail_pill_t0 = Instant::now();
        }
        let t = (self.rail_pill_t0.elapsed().as_secs_f32() / DURATION).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t);
        self.rail_pill = self.rail_pill_from + (self.rail_pill_goal - self.rail_pill_from) * eased;
        if t < 1.0 {
            ctx.request_repaint();
        }
        self.rail_pill
    }

    /// Frame rate of whichever plane is loaded, for timecode and frame maths.
    pub(super) fn fps(&self) -> f64 {
        self.media
            .meta_a
            .as_ref()
            .or(self.media.meta_b.as_ref())
            .map(|m| m.fps)
            .unwrap_or(30.0)
    }

    /// Forces the next `sync_fit` to re-apply pan/zoom/fill.
    pub(super) fn view_dirty(&mut self) {
        self.view_key = None;
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
        for player in [self.media.player_a.as_ref(), self.media.player_b.as_ref()]
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
            self.toast = Some(Toast::error(self.t(
                "libmpv 未加载，无法播放",
                "libmpv is not loaded; cannot play",
            )));
            return;
        };

        // Track in the playlist and in the recent list.
        let path = std::path::absolute(&path).unwrap_or(path);
        let path_str = path.to_string_lossy().to_string();
        if !self.playlist.iter().any(|e| e.path == path) {
            self.playlist.push(PlayEntry::new(path.clone()));
        }
        if let Some(index) = self.playlist.iter().position(|e| e.path == path) {
            self.selected = Some(index);
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
                    "{} → {} {}",
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    if target == Target::B { "B" } else { "A" },
                    self.t("面", "side")
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
            }
            Err(err) => {
                self.toast = Some(Toast::error(format!(
                    "{}{err:#}",
                    self.t("载入失败：", "Load failed: ")
                )));
            }
        }
    }

    pub(super) fn set_mode(&mut self, mode: CompareMode) {
        if self.settings.mode == mode {
            return;
        }
        self.settings.mode = mode;
        self.settings_dirty = true;
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
                        _ => {}
                    }
                    ctx.request_repaint();
                }
                MpvEvent::EndFile { reason } => {
                    if reason.contains("error") {
                        self.toast = Some(Toast::error(format!(
                            "{}{plane} {}{reason}",
                            self.t("播放出错：", "Playback error: "),
                            self.t("面：", "side: ")
                        )));
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

    /// True while the video alone owns the window: no titlebar, rail,
    /// transport or playlist — just the picture, edge to edge.
    pub(super) fn immersive(&self) -> bool {
        self.immersive
    }

    /// Enter or leave immersive playback. The window itself also goes
    /// fullscreen so the picture reaches the screen edges.
    pub(super) fn set_immersive(&mut self, ctx: &egui::Context, on: bool) {
        if self.immersive == on {
            return;
        }
        self.immersive = on;
        crate::diag!("[state] immersive -> {on} (window geometry switched)");
        // The picture must reach every screen edge. Instead of asking the OS for
        // fullscreen (which re-applies the DWM frame when entered from a
        // maximized window, leaving a groove and corner artefacts), the window
        // is simply resized to its monitor with the decorations already off.
        let ppp = ctx.pixels_per_point();
        if let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE) {
            if on {
                self.saved_outer = ctx.input(|i| {
                    i.viewport()
                        .outer_rect
                        .map(|r| (r.min.x, r.min.y, r.width(), r.height()))
                });
                self.saved_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                window::cover_monitor(hwnd);
            } else if self.saved_maximized {
                // A maximized window being "restored" must stay maximized, or it
                // ends up slightly larger than the work area in the normal state.
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                self.saved_outer = None;
            } else if let Some((x, y, w, h)) = self.saved_outer.take() {
                window::restore_rect(
                    hwnd,
                    (x * ppp).round() as i32,
                    (y * ppp).round() as i32,
                    (w * ppp).round() as i32,
                    (h * ppp).round() as i32,
                );
            }
            window::apply_modern_frame(hwnd, on);
            self.frame_immersive = on;
            self.frame_dressed = true;
        }
        // Borderless fullscreen still leaves the taskbar on top for windows that
        // are not topmost, which shows as a strip along the bottom edge; keep
        // the picture window above it while the chrome is hidden.
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if on {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(true));
        ctx.request_repaint();
    }

    pub(super) fn toggle_immersive(&mut self, ctx: &egui::Context) {
        let on = !self.immersive;
        self.set_immersive(ctx, on);
    }

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
            self.set_mode(CompareMode::Blink);
        }
        if pressed(Key::Num0) {
            crate::app::input::reset_view(self);
        }
        if pressed(Key::S) {
            crate::app::input::snapshot(self);
        }
        if pressed(Key::Tab) {
            self.settings.show_playlist = !self.settings.show_playlist;
            self.settings_dirty = true;
        }
        if pressed(Key::F) {
            self.toggle_immersive(ctx);
        }
        if pressed(Key::Escape) {
            self.set_immersive(ctx, false);
        }
        if pressed(Key::OpenBracket) {
            self.settings.wipe = (self.settings.wipe - 0.02).clamp(0.0, 1.0);
        }
        if pressed(Key::CloseBracket) {
            self.settings.wipe = (self.settings.wipe + 0.02).clamp(0.0, 1.0);
        }
    }

    /// Start a manual window move from a chrome area.
    pub(super) fn begin_window_move(&mut self, ctx: &egui::Context) {
        if self.immersive || self.window_move.is_some() {
            return;
        }
        let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE) else {
            return;
        };
        let (Some(rect), Some(cursor)) = (
            crate::video::childwin::window_rect(hwnd),
            crate::video::childwin::cursor_pos(),
        ) else {
            return;
        };
        self.window_move = Some(WindowMove {
            moving: true,
            edges: (false, false, false, false),
            start_cursor: cursor,
            start_rect: rect,
        });
        ctx.request_repaint();
    }

    /// Apply the tracked move / resize until the button is released.
    fn pump_window_move(&mut self, ctx: &egui::Context) {
        let Some(state) = &self.window_move else {
            return;
        };
        if !crate::video::childwin::left_button_down() {
            self.window_move = None;
            return;
        }
        let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE) else {
            self.window_move = None;
            return;
        };
        let Some((cx, cy)) = crate::video::childwin::cursor_pos() else {
            return;
        };
        let dx = cx - state.start_cursor.0;
        let dy = cy - state.start_cursor.1;
        let (x, y, w, h) = state.start_rect;
        let (mut nx, mut ny, mut nw, mut nh) = (x, y, w, h);
        if state.moving {
            nx = x + dx;
            ny = y + dy;
        } else {
            let (west, east, north, south) = state.edges;
            if west {
                nx = x + dx;
                nw = w - dx;
            }
            if east {
                nw = w + dx;
            }
            if north {
                ny = y + dy;
                nh = h - dy;
            }
            if south {
                nh = h + dy;
            }
            if nw < MIN_WINDOW_W {
                nw = MIN_WINDOW_W;
                if west {
                    nx = x + w - MIN_WINDOW_W;
                }
            }
            if nh < MIN_WINDOW_H {
                nh = MIN_WINDOW_H;
                if north {
                    ny = y + h - MIN_WINDOW_H;
                }
            }
        }
        crate::video::childwin::set_window_rect(hwnd, nx, ny, nw, nh);
        ctx.request_repaint();
    }

    // ── per-frame sync ────────────────────────────────────────────────────

    /// Undecorated windows need their own resize borders.
    fn resize_borders(&mut self, ctx: &egui::Context) {
        let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        let rect = ctx.content_rect();
        const EDGE: f32 = 10.0;
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
        let _ = dir;
        if self.immersive || self.window_move.is_some() {
            return;
        }
        if ctx.input(|i| i.pointer.primary_pressed()) {
            let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE)
            else {
                return;
            };
            let (Some(win), Some(cursor_pos)) = (
                crate::video::childwin::window_rect(hwnd),
                crate::video::childwin::cursor_pos(),
            ) else {
                return;
            };
            crate::diag!("[window] begin resize w={west} e={east} n={north} s={south} win={win:?}");
            self.window_move = Some(WindowMove {
                moving: false,
                edges: (west, east, north, south),
                start_cursor: cursor_pos,
                start_rect: win,
            });
        }
    }

    /// True when the pointer sits in the window's resize band. The chrome
    /// drag handlers bow out there so resizing never turns into a window move.
    pub(super) fn near_edge(&self, ctx: &egui::Context) -> bool {
        let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else {
            return false;
        };
        let r = ctx.content_rect();
        p.x <= r.left() + 10.0
            || p.x >= r.right() - 10.0
            || p.y <= r.top() + 10.0
            || p.y >= r.bottom() - 10.0
    }

    fn sync_stage(&mut self, ctx: &egui::Context) {
        let hidden = self.rail == RailTab::Settings && !self.immersive;
        let rect = if hidden || !self.stage_reported {
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO)
        } else if self.immersive {
            // Over-cover by a pixel so fractional DPI scaling can never leave a
            // hairline of desktop along the screen edge.
            self.stage_rect.expand(1.0)
        } else {
            self.stage_rect
        };
        let ppp = ctx.pixels_per_point();
        // The wipe grip is a native window, so its colours come from here.
        let bg = colour_to_rgb(self.palette.primary);
        let fg = colour_to_rgb(self.palette.on_primary);
        let vertical = self.settings.axis == SplitAxis::Horizontal;
        self.media.sync_grip_style(bg, fg, vertical);
        let blink_b = self.settings.mode == CompareMode::Blink && self.blink_show_b();
        self.media.sync_layout(
            rect,
            ppp,
            self.settings.mode,
            self.settings.axis,
            self.settings.wipe,
            blink_b,
        );
        self.media.sync_playback(self.settings.mode);
        self.stage_reported = false;
        self.sync_fit(ppp);
    }

    /// Fits each plane's picture to its surface: either letterboxed
    /// (`video-zoom` 0) or cropped to fill when the user picked 填充.
    fn sync_fit(&mut self, ppp: f32) {
        let _ = ppp;
        let mode_tag = match self.settings.mode {
            CompareMode::Single => 0u8,
            CompareMode::Pair => 1,
            CompareMode::Wipe => 2,
            CompareMode::Blink => 3,
        };
        let key = (
            self.settings.fill,
            self.stage_rect.width().round() as i32,
            self.stage_rect.height().round() as i32,
            self.zoom.to_bits(),
            self.pan.0.to_bits(),
            self.pan.1.to_bits(),
            mode_tag,
        );
        if self.view_key == Some(key) {
            return;
        }
        self.view_key = Some(key);
        for (plane, meta) in [
            (stage::Plane::A, self.media.meta_a.clone()),
            (stage::Plane::B, self.media.meta_b.clone()),
        ] {
            let Some(meta) = meta else { continue };
            let Some((pw, ph)) = self.media.plane_size(plane) else {
                continue;
            };
            if meta.width == 0 || meta.height == 0 || pw <= 0 || ph <= 0 {
                continue;
            }
            let (sw, sh) = (f64::from(pw), f64::from(ph));
            let (vw, vh) = (f64::from(meta.width), f64::from(meta.height));
            let fit = (sw / vw).min(sh / vh);
            let cover = (sw / vw).max(sh / vh);
            let base = if self.settings.fill && fit > 0.0 {
                (cover / fit).log2()
            } else {
                0.0
            };
            if let Some(player) = self.media.player_of(plane) {
                player.set_pan(self.pan.0, self.pan.1);
                player.set_zoom(base + self.zoom);
            }
        }
    }

    /// Which input the flicker comparison shows right now.
    ///
    /// A small duty cycle in favour of A keeps the reference recognisable while
    /// B still pops out on every switch.
    fn blink_show_b(&self) -> bool {
        let phase = self.blink_clock.elapsed().as_secs_f64() / BLINK_HALF_SECS;
        (phase as u64) % 2 == 1
    }

    fn maybe_save(&mut self) {
        if self.settings_dirty && self.last_save.elapsed() > Duration::from_millis(700) {
            self.settings.save();
            self.settings_dirty = false;
            self.last_save = Instant::now();
        }
    }

    fn toast_layer(&mut self, ctx: &egui::Context) {
        // On the settings page there are no native planes, so the floating
        // layer is visible. Elsewhere the transport draws the snackbar itself,
        // because egui cannot paint over the video windows.
        if self.rail != RailTab::Settings {
            return;
        }
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

/// A window move or resize driven by us instead of the OS.
///
/// `ViewportCommand::StartDrag` / `BeginResize` hand the gesture to the Windows
/// modal loops, which silently do nothing while our window is not the
/// foreground window — that is why dragging "worked, then stopped working"
/// after the window had been fullscreen once. Tracking the rectangle here and
/// applying it with `SetWindowPos` has no such dependency.
pub(super) struct WindowMove {
    /// `true` to move the window, `false` to resize it.
    moving: bool,
    /// Which borders are being dragged: west, east, north, south.
    edges: (bool, bool, bool, bool),
    start_cursor: (i32, i32),
    start_rect: (i32, i32, i32, i32),
}

const MIN_WINDOW_W: i32 = 640;
const MIN_WINDOW_H: i32 = 400;

impl eframe::App for MuxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_no += 1;
        if !self.immersive {
            self.resize_borders(ctx);
            self.pump_window_move(ctx);
        }

        // Command-line files open on the first frame, once the native window exists.
        let pending = std::mem::take(&mut self.pending);
        for (path, target) in pending {
            self.open_path(path, target);
        }

        // Window geometry, only when it changes: the fastest way to see whether
        // a resize actually happened.
        {
            let area = ctx.input(|i| i.viewport().outer_rect);
            let frame = self.frame_no;
            if self.last_viewport != area {
                self.last_viewport = area;
                crate::diag!(
                    "[window] frame={frame} outer_rect={area:?} immersive={}",
                    self.immersive
                );
            }
        }

        // Native window polish: rounded DWM corners normally, no border at all
        // while immersive (otherwise a light 1px groove shows along the edge).
        let immersive_now = self.immersive;
        if !self.frame_dressed
            && let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE)
        {
            window::apply_modern_frame(hwnd, immersive_now);
            self.frame_dressed = true;
            self.frame_immersive = immersive_now;
        } else if self.frame_dressed
            && self.frame_immersive != immersive_now
            && let Some(hwnd) = crate::video::childwin::find_window_by_title(stage::WINDOW_TITLE)
        {
            window::apply_modern_frame(hwnd, immersive_now);
            self.frame_immersive = immersive_now;
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
        input::pump(self, ctx);
        self.pump_events(ctx);

        let palette = self.palette.clone();
        let surface = palette.surface;

        if self.immersive {
            // Immersive: the picture alone fills the window, no chrome at all.
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(Color32::BLACK))
                .show(ctx, |ui| {
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
                    self.stage_rect = rect;
                    self.stage_reported = true;
                    let has_video = self.has_video();
                    if !has_video {
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            self.t(
                                "没有载入视频 · 按 F 或 Esc 退出全屏",
                                "No video loaded · press F or Esc to leave full screen",
                            ),
                            egui::FontId::proportional(15.0),
                            Color32::from_gray(150),
                        );
                    }
                });
            self.sync_stage(ctx);
            self.maybe_save();
            ctx.request_repaint_after(if self.is_playing() {
                Duration::from_millis(16)
            } else {
                Duration::from_millis(100)
            });
            return;
        }

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
        let blinking = self.settings.mode == CompareMode::Blink && self.media.file_b.is_some();
        ctx.request_repaint_after(if playing {
            Duration::from_millis(16)
        } else if blinking {
            // Even on a paused frame the planes must keep swapping.
            Duration::from_millis(30)
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

/// Packs an egui colour into `0xRRGGBB` for the Win32 painting code.
fn colour_to_rgb(color: egui::Color32) -> u32 {
    ((color.r() as u32) << 16) | ((color.g() as u32) << 8) | color.b() as u32
}
