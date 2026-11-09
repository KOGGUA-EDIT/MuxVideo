//! Stage: the native video planes and the players bound to them.
//!
//! libmpv draws straight into a Win32 child window (`wid`), so the stage is a
//! stack of native windows that egui lays out but does not paint into:
//!
//! ```text
//! MuxVideo toplevel HWND
//!   └── stage container (WS_CLIPCHILDREN, black)   ← follows the egui stage rect
//!         ├── plane A  ← player A (`--wid`)
//!         └── plane B  ← player B (`--wid`, clipped for wipe, blinked)
//! ```
//!
//! Coordinates are physical pixels relative to the client area, computed from
//! the egui stage rect every frame. Repositioning is cheap (`SetWindowPos`),
//! so layout is simply re-applied every frame; that also covers DPI changes and
//! live resizing without extra bookkeeping.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::Rect;

use crate::app::state::{CompareMode, SplitAxis};
use crate::mpv::{LibMpv, MpvEvent, MpvOptions, MpvPlayer};
use crate::video::childwin::{ChildWindow, find_window_by_title};

/// Title of the eframe toplevel; used to find the HWND we parent into.
pub(crate) const WINDOW_TITLE: &str = "MuxVideo";

/// Slave drift tolerated before B is nudged back onto A.
const MAX_DRIFT_SECS: f64 = 0.08;
/// How often the drift check runs.
const SYNC_INTERVAL: Duration = Duration::from_millis(250);

/// Physical pixel rectangle.
type PxRect = (i32, i32, i32, i32);

/// Source metadata shown next to the compare controls.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub codec: String,
}

impl MediaInfo {
    /// Compact one-line summary, e.g. 1280×720 30p h264.
    pub fn summary(&self) -> String {
        if self.width == 0 && self.height == 0 {
            return String::new();
        }
        let mut out = format!("{}×{}", self.width, self.height);
        if self.fps > 0.0 {
            out.push_str(&format!(" {:.0}p", self.fps));
        }
        if !self.codec.is_empty() && self.codec != "—" {
            out.push(' ');
            out.push_str(&self.codec);
        }
        out
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Plane {
    A,
    B,
}

impl Plane {
    fn id(self) -> u32 {
        match self {
            Self::A => 0x4D56_0002,
            Self::B => 0x4D56_0003,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::A => "muxvideo-plane-a",
            Self::B => "muxvideo-plane-b",
        }
    }
}

struct Surface {
    window: Option<ChildWindow>,
    hwnd: i64,
    rect: PxRect,
    visible: bool,
}

impl Surface {
    fn new() -> Self {
        Self {
            window: None,
            hwnd: 0,
            rect: (-1, -1, 0, 0),
            visible: false,
        }
    }

    fn ensure(&mut self, parent: i64, plane: Plane) -> anyhow::Result<i64> {
        if let Some(_win) = &self.window {
            return Ok(self.hwnd);
        }
        let win = ChildWindow::create(parent, plane.id(), plane.name())?;
        self.hwnd = win.hwnd();
        self.window = Some(win);
        Ok(self.hwnd)
    }

    /// Places the plane; `None` hides it.
    fn place(&mut self, rect: Option<PxRect>) {
        let Some(win) = &self.window else {
            return;
        };
        let target = rect.filter(|r| r.2 > 0 && r.3 > 0);
        let visible = target.is_some();
        let rect = target.unwrap_or((0, 0, 0, 0));
        if self.rect != rect {
            win.set_rect(rect.0, rect.1, rect.2, rect.3);
            self.rect = rect;
        }
        if self.visible != visible {
            win.show(visible);
            self.visible = visible;
        }
    }

    fn clip(&self, rect: Option<PxRect>) {
        let Some(win) = &self.window else {
            return;
        };
        match rect {
            Some((x, y, w, h)) if w > 0 && h > 0 => win.set_clip_rect(x, y, w, h),
            _ => win.clear_clip(),
        }
    }
}

/// Native planes plus the libmpv instances rendering into them.
pub(crate) struct Media {
    container: Option<ChildWindow>,
    container_rect: PxRect,
    plane_a: Surface,
    plane_b: Surface,
    planes_ready: bool,

    pub player_a: Option<MpvPlayer>,
    pub player_b: Option<MpvPlayer>,

    pub file_a: Option<PathBuf>,
    pub file_b: Option<PathBuf>,

    /// Resolution / fps / codec, filled in after each FileLoaded.
    pub meta_a: Option<MediaInfo>,
    pub meta_b: Option<MediaInfo>,

    last_sync: Instant,
    pub drift: f64,
}

impl Default for Media {
    fn default() -> Self {
        Self {
            container: None,
            container_rect: (-1, -1, 0, 0),
            plane_a: Surface::new(),
            plane_b: Surface::new(),
            planes_ready: false,
            player_a: None,
            player_b: None,
            file_a: None,
            file_b: None,
            meta_a: None,
            meta_b: None,
            last_sync: Instant::now(),
            drift: 0.0,
        }
    }
}

impl Media {
    /// Locates the toplevel and lazily creates the stage container + planes.
    fn ensure_native(&mut self) -> bool {
        if self.container.is_none() {
            let Some(parent) = find_window_by_title(WINDOW_TITLE) else {
                return false;
            };
            match ChildWindow::create(parent, 0x4D56_0001, "muxvideo-stage") {
                Ok(win) => self.container = Some(win),
                Err(_) => return false,
            }
        }
        if !self.planes_ready {
            let stage = self.stage_hwnd();
            if stage == 0 {
                return false;
            }
            let ok = [Plane::A, Plane::B]
                .into_iter()
                .all(|plane| self.surface_mut(plane).ensure(stage, plane).is_ok());
            self.planes_ready = ok;
        }
        self.planes_ready
    }

    fn stage_hwnd(&self) -> i64 {
        self.container.as_ref().map(|c| c.hwnd()).unwrap_or(0)
    }

    fn surface_mut(&mut self, plane: Plane) -> &mut Surface {
        match plane {
            Plane::A => &mut self.plane_a,
            Plane::B => &mut self.plane_b,
        }
    }

    /// Creates a player bound to `plane`, ensuring the native window exists.
    fn spawn_player(
        &mut self,
        lib: &Arc<LibMpv>,
        plane: Plane,
        opts: &MpvOptions,
    ) -> anyhow::Result<MpvPlayer> {
        if !self.ensure_native() {
            anyhow::bail!("舞台窗口尚未创建（找不到主窗口或子窗口创建失败）");
        }
        let hwnd = self.surface_mut(plane).hwnd;
        if hwnd == 0 {
            anyhow::bail!("视频平面句柄无效");
        }
        let player = lib.new_player(hwnd, opts)?;
        // Only the A plane carries audio; B is a visual reference and playing
        // both would merely double the sound with a slight echo.
        if plane == Plane::B {
            player.set_mute(true);
        }
        Ok(player)
    }

    // ── loading ───────────────────────────────────────────────────────────

    pub fn open_a(
        &mut self,
        lib: &Arc<LibMpv>,
        path: &Path,
        opts: &MpvOptions,
    ) -> anyhow::Result<()> {
        if self.player_a.is_none() {
            let player = self.spawn_player(lib, Plane::A, opts)?;
            self.player_a = Some(player);
        }
        if let Some(player) = &self.player_a {
            player.load_file(&path.to_string_lossy());
        }
        self.file_a = Some(path.to_path_buf());
        Ok(())
    }

    pub fn open_b(
        &mut self,
        lib: &Arc<LibMpv>,
        path: &Path,
        opts: &MpvOptions,
    ) -> anyhow::Result<()> {
        if self.player_b.is_none() {
            let player = self.spawn_player(lib, Plane::B, opts)?;
            self.player_b = Some(player);
        }
        if let Some(player) = &self.player_b {
            player.load_file(&path.to_string_lossy());
        }
        self.file_b = Some(path.to_path_buf());
        Ok(())
    }

    // ── layout ────────────────────────────────────────────────────────────

    pub fn sync_layout(
        &mut self,
        stage: Rect,
        ppp: f32,
        mode: CompareMode,
        axis: SplitAxis,
        wipe: f32,
        blink_b: bool,
    ) {
        if !self.ensure_native() {
            return;
        }
        let Some(container) = &self.container else {
            return;
        };

        let x = (stage.min.x * ppp).round() as i32;
        let y = (stage.min.y * ppp).round() as i32;
        let w = ((stage.width() * ppp).round() as i32).max(0);
        let h = ((stage.height() * ppp).round() as i32).max(0);
        let rect = (x, y, w, h);
        let any_video = self.player_a.is_some() || self.player_b.is_some();
        // No media yet: shrink the native box away so the egui empty state shows.
        let rect = if any_video && w >= 8 && h >= 8 {
            rect
        } else {
            (0, 0, 0, 0)
        };
        if self.container_rect != rect {
            container.set_rect(rect.0, rect.1, rect.2, rect.3);
            self.container_rect = rect;
        }
        if w < 8 || h < 8 || !any_video {
            self.plane_a.place(None);
            self.plane_b.place(None);
            return;
        }

        let has_a = self.file_a.is_some() && self.player_a.is_some();
        let has_b = self.file_b.is_some() && self.player_b.is_some();
        let pair = has_a && has_b;
        let full: PxRect = (0, 0, w, h);

        let mut a_rect: Option<PxRect> = None;
        let mut b_rect: Option<PxRect> = None;
        let mut b_clip: Option<PxRect> = None;

        match mode {
            CompareMode::Single => {
                if has_a {
                    a_rect = Some(full);
                } else if has_b {
                    b_rect = Some(full);
                }
            }
            CompareMode::Pair | CompareMode::Wipe => {
                if pair {
                    if mode == CompareMode::Pair {
                        let (ra, rb) = match axis {
                            SplitAxis::Horizontal => {
                                let half = w / 2;
                                (
                                    (0, 0, half.saturating_sub(1), h),
                                    (half + 1, 0, w - half - 1, h),
                                )
                            }
                            SplitAxis::Vertical => {
                                let half = h / 2;
                                (
                                    (0, 0, w, half.saturating_sub(1)),
                                    (0, half + 1, w, h - half - 1),
                                )
                            }
                        };
                        a_rect = Some(ra);
                        b_rect = Some(rb);
                    } else {
                        a_rect = Some(full);
                        b_rect = Some(full);
                        let split = (wipe.clamp(0.0, 1.0)
                            * if axis == SplitAxis::Horizontal { w } else { h } as f32)
                            .round() as i32;
                        b_clip = Some(match axis {
                            SplitAxis::Horizontal => (split, 0, w - split, h),
                            SplitAxis::Vertical => (0, split, w, h - split),
                        });
                    }
                } else if has_a {
                    a_rect = Some(full);
                } else if has_b {
                    b_rect = Some(full);
                }
            }
            CompareMode::Blink => {
                // Flicker comparison: the two takes alternate on the same spot,
                // so any retouch shows up as a visible jump.
                if pair {
                    if blink_b {
                        b_rect = Some(full);
                    } else {
                        a_rect = Some(full);
                    }
                } else if has_a {
                    a_rect = Some(full);
                } else if has_b {
                    b_rect = Some(full);
                }
            }
        }

        self.plane_a.place(a_rect);
        self.plane_b.place(b_rect);
        self.plane_b.clip(b_clip);
    }

    // ── events / sync ─────────────────────────────────────────────────────

    pub(crate) fn pump(&mut self) -> Vec<(char, MpvEvent)> {
        let mut events = Vec::new();
        if let Some(p) = &self.player_a {
            events.extend(p.poll_events().into_iter().map(|e| ('A', e)));
        }
        if let Some(p) = &self.player_b {
            events.extend(p.poll_events().into_iter().map(|e| ('B', e)));
        }
        events
    }

    /// Keeps the visible slave aligned with the A plane.
    ///
    /// A is always the master. Hidden planes are paused so they do not burn
    /// decode time.
    pub fn sync_playback(&mut self, mode: CompareMode) {
        let Some(a) = self.player_a.as_ref() else {
            return;
        };
        if self.last_sync.elapsed() < SYNC_INTERVAL {
            return;
        }
        self.last_sync = Instant::now();
        let ta = a.time_pos();
        if ta <= 0.0 {
            return;
        }

        // B: only decoded while it is on screen.
        if let Some(b) = self.player_b.as_ref() {
            if self.plane_b.visible && mode.needs_b() {
                let tb = b.time_pos();
                self.drift = ta - tb;
                if self.drift.abs() > MAX_DRIFT_SECS || tb <= 0.0 {
                    b.seek_absolute(ta);
                }
            } else if !b.paused() {
                b.set_pause(true);
            }
        }
    }

    pub fn player_of(&self, plane: Plane) -> Option<&MpvPlayer> {
        match plane {
            Plane::A => self.player_a.as_ref(),
            Plane::B => self.player_b.as_ref(),
        }
    }

    /// Plane whose visible native surface owns `hwnd`.
    ///
    /// The stage container and the eframe toplevel also receive mouse messages
    /// (wheel events bubble up), so input is only accepted from a visible plane.
    pub fn plane_for_hwnd(&self, hwnd: i64) -> Option<Plane> {
        let hit = |s: &Surface| s.visible && s.hwnd != 0 && s.hwnd == hwnd;
        if hit(&self.plane_a) {
            Some(Plane::A)
        } else if hit(&self.plane_b) {
            Some(Plane::B)
        } else {
            None
        }
    }

    /// Client size of a plane in physical pixels.
    pub fn plane_size(&self, plane: Plane) -> Option<(i32, i32)> {
        let s = match plane {
            Plane::A => &self.plane_a,
            Plane::B => &self.plane_b,
        };
        (s.rect.2 > 0 && s.rect.3 > 0).then_some((s.rect.2, s.rect.3))
    }

    /// Applies a RV-style pan/zoom to every live plane, so A/B stay aligned.
    pub fn apply_pan_zoom(&self, pan: (f64, f64), zoom: f64) {
        for plane in [Plane::A, Plane::B] {
            if let Some(player) = self.player_of(plane) {
                player.set_pan(pan.0, pan.1);
                player.set_zoom(zoom);
            }
        }
    }

    /// Reads resolution / frame rate / codec once a file finished loading.
    pub fn refresh_meta(&mut self, plane: Plane) {
        let Some(player) = self.player_of(plane) else {
            return;
        };
        let number = |name: &str| -> Option<f64> {
            player
                .get_property(name)
                .and_then(|v| v.parse::<f64>().ok())
        };
        let width = number("width").unwrap_or(0.0) as u32;
        let height = number("height").unwrap_or(0.0) as u32;
        let fps = number("estimated-vf-fps")
            .filter(|v| *v > 0.0)
            .or_else(|| number("container-fps").filter(|v| *v > 0.0))
            .unwrap_or(0.0);
        let codec = player
            .get_property("video-format")
            .unwrap_or_else(|| "—".into());
        let info = MediaInfo {
            width,
            height,
            fps,
            codec,
        };
        match plane {
            Plane::A => self.meta_a = Some(info),
            Plane::B => self.meta_b = Some(info),
        }
    }
}

impl Drop for Media {
    fn drop(&mut self) {
        if let Some(p) = self.player_a.take() {
            p.stop();
        }
        if let Some(p) = self.player_b.take() {
            p.stop();
        }
    }
}
