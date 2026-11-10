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

use crate::app::scrub::{CACHE_BYTES, FrameCache, Grabber};
use crate::app::state::{CompareMode, SplitAxis};
use crate::mpv::{LibMpv, MpvEvent, MpvOptions, MpvPlayer};
use crate::video::childwin::{ChildWindow, find_window_by_title};

/// Title of the eframe toplevel; used to find the HWND we parent into.
pub(crate) const WINDOW_TITLE: &str = "MuxVideo";

/// Slave drift tolerated before B is nudged back onto A.
const MAX_DRIFT_SECS: f64 = 0.08;
/// How often the drift check runs.
const SYNC_INTERVAL: Duration = Duration::from_millis(250);

/// How often the plane children (mpv's renderer window) are re-scanned for
/// subclassing, so a recreated renderer cannot silently swallow the pointer.
const HOOK_SCAN_INTERVAL: Duration = Duration::from_millis(400);

/// Wipe grip pill: long/short side in physical pixels. The short side also
/// doubles as the corner radius, so the pill reads as fully rounded.
const GRIP_PILL_LONG: i32 = 64;
const GRIP_PILL_SHORT: i32 = 20;

/// Physical pixel rectangle.
type PxRect = (i32, i32, i32, i32);

/// Signature of the last layout that was written to the native windows,
/// used to keep the debug log quiet unless something actually changed.
type LayoutSig = (PxRect, PxRect, Option<PxRect>, Option<PxRect>, PxRect);

/// Source metadata shown next to the compare controls and in the inspector.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub codec: String,
    /// Demuxer name reported by mpv, e.g. `matroska,webm`.
    pub container: String,
    /// Seconds; `0.0` when unknown.
    pub duration: f64,
    /// `aac 2ch 48 kHz` style one-liner.
    pub audio: String,
    /// Video bitrate as `12.4 Mbps`.
    pub bitrate: String,
    /// Container/file size in bytes.
    pub size: u64,
    /// `yuv420p`, `nv12` … the pixel format mpv decodes to.
    pub pix_fmt: String,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    /// Renderer windows libmpv created below `hwnd` (they get the pointer
    /// messages, so they are subclassed and mapped back to this plane).
    children: Vec<i64>,
    /// Rounded-pill region radius; `> 0` for the wipe grip, `0` for planes.
    round: i32,
    /// Last clip rectangle, kept so the diagnostics log can show it.
    clip_rect: Option<PxRect>,
}

impl Surface {
    fn new() -> Self {
        Self {
            window: None,
            hwnd: 0,
            rect: (-1, -1, 0, 0),
            visible: false,
            children: Vec::new(),
            round: 0,
            clip_rect: None,
        }
    }

    /// Hooks any renderer window mpv created (or recreated) below this plane.
    fn refresh_children(&mut self) {
        if self.hwnd == 0 {
            return;
        }
        for child in crate::video::child_windows(self.hwnd) {
            if !self.children.contains(&child) {
                crate::video::childwin::install_input_hook(child);
                self.children.push(child);
                crate::diag!("[hooks] plane hwnd={} hooked child={child}", self.hwnd);
            }
        }
        // Drop handles the renderer destroyed so the list cannot grow forever.
        self.children
            .retain(|h| crate::video::childwin::is_window(*h));
    }

    fn owns(&self, hwnd: i64) -> bool {
        hwnd != 0 && (self.hwnd == hwnd || self.children.contains(&hwnd))
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
            if self.round > 0 {
                win.round_corners(self.round);
            }
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
    /// Material drag handle for the wipe divider. It lives in the same native
    /// layer as the planes so it stays visible above the video.
    grip: Surface,
    planes_ready: bool,

    pub player_a: Option<Arc<MpvPlayer>>,
    pub player_b: Option<Arc<MpvPlayer>>,

    /// Opaque child window that shows a cached frame on top of a plane while
    /// libmpv is still seeking during a scrub.
    preview: Surface,
    pub cache_a: FrameCache,
    cache_b: FrameCache,
    grabber: Option<Grabber>,
    last_grab: Instant,
    /// Frame currently painted into `preview`, if any.
    preview_frame: Option<u64>,

    pub file_a: Option<PathBuf>,
    pub file_b: Option<PathBuf>,

    /// Resolution / fps / codec, filled in after each FileLoaded.
    /// Last layout signature that was written to the diagnostics log.
    logged_layout: Option<LayoutSig>,
    pub meta_a: Option<MediaInfo>,
    pub meta_b: Option<MediaInfo>,

    last_sync: Instant,
    last_hook_scan: Instant,
    /// Last colours pushed to the grip window, so a frame that changes nothing
    /// does not repaint it.
    grip_style: Option<(u32, u32, bool)>,
    pub drift: f64,
}

impl Default for Media {
    fn default() -> Self {
        Self {
            container: None,
            container_rect: (-1, -1, 0, 0),
            plane_a: Surface::new(),
            plane_b: Surface::new(),
            grip: Surface::new(),
            planes_ready: false,
            player_a: None,
            player_b: None,
            preview: Surface::new(),
            cache_a: FrameCache::new(CACHE_BYTES),
            cache_b: FrameCache::new(CACHE_BYTES),
            grabber: None,
            last_grab: Instant::now() - Duration::from_secs(1),
            preview_frame: None,
            file_a: None,
            file_b: None,
            meta_a: None,
            meta_b: None,
            last_sync: Instant::now(),
            last_hook_scan: Instant::now() - HOOK_SCAN_INTERVAL,
            grip_style: None,
            drift: 0.0,
            logged_layout: None,
        }
    }
}

impl Media {
    /// Locates the toplevel and lazily creates the stage container + planes.
    fn ensure_native(&mut self) -> bool {
        if self.container.is_none() {
            let Some(parent) = find_window_by_title(WINDOW_TITLE) else {
                crate::diag!("[stage] ensure_native: no toplevel window yet");
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
                crate::diag!("[stage] ensure_native: container has no hwnd");
                return false;
            }
            let ok = [Plane::A, Plane::B]
                .into_iter()
                .all(|plane| self.surface_mut(plane).ensure(stage, plane).is_ok());
            self.grip.round = 0; // shaped by `set_grip_region`, not a plain pill
            let grip_ok = ChildWindow::create_grip(stage, 0x4D56_0005, "muxvideo-grip")
                .map(|win| {
                    self.grip.hwnd = win.hwnd();
                    self.grip.window = Some(win);
                })
                .is_ok();
            self.preview.round = 0;
            let preview_ok = ChildWindow::create_preview(stage, 0x4D56_0006, "muxvideo-preview")
                .map(|win| {
                    self.preview.hwnd = win.hwnd();
                    self.preview.window = Some(win);
                })
                .is_ok();
            self.planes_ready = ok && grip_ok && preview_ok;
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
    ) -> anyhow::Result<Arc<MpvPlayer>> {
        if !self.ensure_native() {
            anyhow::bail!("舞台窗口尚未创建（找不到主窗口或子窗口创建失败）");
        }
        let hwnd = self.surface_mut(plane).hwnd;
        if hwnd == 0 {
            anyhow::bail!("视频平面句柄无效");
        }
        crate::diag!("[stage] spawn player plane={plane:?} wid={hwnd}");
        let player = lib.new_player(hwnd, opts)?;
        // Only the A plane carries audio; B is a visual reference and playing
        // both would merely double the sound with a slight echo.
        if plane == Plane::B {
            player.set_mute(true);
        }
        Ok(Arc::new(player))
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
        // Frame numbers belong to the file that was open, so the previous
        // cache would paint the wrong picture after a switch.
        self.cache_a.clear();
        self.hide_preview();
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
        self.cache_b.clear();
        self.hide_preview();
        if let Some(player) = &self.player_b {
            player.load_file(&path.to_string_lossy());
        }
        self.file_b = Some(path.to_path_buf());
        Ok(())
    }

    // ── scrub cache ───────────────────────────────────────────────────────

    /// Grabs frames in the background and paints them over a plane right away.
    ///
    /// Called once per UI frame. `busy` is true while the user drags, which
    /// makes the sampler follow the pointer instead of the playback position.
    pub fn cache_tick(&mut self, playing: bool, scrubbing: bool) {
        if self.grabber.is_none() {
            self.grabber = Some(Grabber::spawn());
        }
        let Some(grabber) = &self.grabber else {
            return;
        };
        let mut landed = 0usize;
        for (_frame, image) in grabber.drain() {
            self.cache_a.insert(_frame, image);
            landed += 1;
        }
        if landed > 0 {
            let (frames, bytes, hits) = self.cache_a.stats();
            crate::diag!(
                "[cache] +{landed} frames={frames} bytes={bytes} hits={hits} preview={}",
                self.preview.visible
            );
        }

        let interval = if scrubbing {
            Duration::from_millis(140)
        } else if playing {
            Duration::from_millis(220)
        } else {
            Duration::from_millis(700)
        };
        if self.last_grab.elapsed() < interval {
            return;
        }
        let Some(player) = self.player_a.clone() else {
            return;
        };
        let fps = self.meta_a.as_ref().map(|meta| meta.fps).unwrap_or(30.0);
        let frame = (player.time_pos().max(0.0) * fps).round() as u64;
        self.last_grab = Instant::now();
        let Some(grabber) = &self.grabber else {
            return;
        };
        if self.cache_a.get(frame).is_none() {
            grabber.request(&player, frame);
        }
    }

    /// Paints the cached frame closest to `frame` over `plane`.
    ///
    /// Returns true when something was painted, so the caller can keep the
    /// gesture alive even though libmpv has not caught up yet.
    pub fn show_cached(&mut self, plane: Plane, frame: u64, window: u64) -> bool {
        if self.preview_frame == Some(frame) && self.preview.visible {
            return true;
        }
        let cache = match plane {
            Plane::A => &mut self.cache_a,
            Plane::B => &mut self.cache_b,
        };
        let Some(image) = cache.get(frame).or_else(|| cache.nearest(frame, window)) else {
            return false;
        };
        let Some(hwnd) = self.preview.window.as_ref().map(ChildWindow::hwnd) else {
            return false;
        };
        let rect = self.surface(plane).rect;
        if rect.2 <= 0 || rect.3 <= 0 {
            return false;
        }
        crate::video::childwin::set_preview_frame(hwnd, image);
        self.preview.place(Some(rect));
        self.preview_frame = Some(frame);
        if let Some(win) = &self.preview.window {
            win.bring_to_top();
        }
        true
    }

    /// Hides the cached-frame overlay (libmpv is showing the real frame again).
    pub fn hide_preview(&mut self) {
        if self.preview_frame.take().is_some() {
            if let Some(hwnd) = self.preview.window.as_ref().map(ChildWindow::hwnd) {
                crate::video::childwin::clear_preview_frame(hwnd);
            }
            self.preview.place(None);
        }
    }

    /// Snapshot of the A plane cache: (frames, bytes, hits).
    pub fn cache_stats(&self) -> (usize, usize, u64) {
        self.cache_a.stats()
    }

    fn surface(&self, plane: Plane) -> &Surface {
        match plane {
            Plane::A => &self.plane_a,
            Plane::B => &self.plane_b,
        }
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

        // Round the far edges (not the size) so a fractional DPI scale cannot
        // leave a one-pixel seam on the right/bottom of the picture.
        let x = (stage.min.x * ppp).round() as i32;
        let y = (stage.min.y * ppp).round() as i32;
        let x1 = (stage.max.x * ppp).round() as i32;
        let y1 = (stage.max.y * ppp).round() as i32;
        let w = (x1 - x).max(0);
        let h = (y1 - y).max(0);
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
            self.grip.place(None);
            return;
        }

        let has_a = self.file_a.is_some() && self.player_a.is_some();
        let has_b = self.file_b.is_some() && self.player_b.is_some();
        let pair = has_a && has_b;
        let full: PxRect = (0, 0, w, h);

        let mut a_rect: Option<PxRect> = None;
        let mut b_rect: Option<PxRect> = None;
        let mut a_clip: Option<PxRect> = None;
        let mut b_clip: Option<PxRect> = None;
        let mut grip_rect: Option<PxRect> = None;

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
                        // The seam is a single pixel: plane A owns the column to
                        // the left of `split`, plane B owns it from `split` on.
                        // The grip window draws the 1 px divider and its pill on
                        // top of the two planes, so nothing is left unpainted.
                        const SEAM: i32 = 0;
                        let before = (split - SEAM).max(0);
                        let after =
                            (split + SEAM).min(if axis == SplitAxis::Horizontal { w } else { h });
                        match axis {
                            SplitAxis::Horizontal => {
                                a_clip = Some((0, 0, before, h));
                                b_clip = Some((after, 0, (w - after).max(0), h));
                                // Full-height strip: the grip paints a 1 px line
                                // plus a pill in the middle of it.
                                grip_rect = Some((split - 10, 0, 20, h));
                            }
                            SplitAxis::Vertical => {
                                a_clip = Some((0, 0, w, before));
                                b_clip = Some((0, after, w, (h - after).max(0)));
                                grip_rect = Some((0, split - 10, w, 20));
                            }
                        }
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
        self.plane_a.clip(a_clip);
        self.plane_b.clip(b_clip);
        self.grip.place(grip_rect);
        let signature = (
            self.plane_a.rect,
            self.plane_b.rect,
            self.plane_a.clip_rect,
            self.plane_b.clip_rect,
            self.grip.rect,
        );
        if self.logged_layout != Some(signature) {
            self.logged_layout = Some(signature);
            crate::diag!(
                "[stage] a={:?} b={:?} clip_a={:?} clip_b={:?} grip={grip_rect:?}",
                a_rect,
                b_rect,
                a_clip,
                b_clip
            );
        }
        if grip_rect.is_some()
            && let Some(win) = &self.grip.window
        {
            // Shape the grip: one-pixel divider plus a rounded pill in the
            // middle of it. Doing this here keeps the region in sync with the
            // seam whenever the split moves or the axis flips.
            win.set_grip_region(
                axis == SplitAxis::Horizontal,
                GRIP_PILL_LONG,
                GRIP_PILL_SHORT,
            );
            // Draw the handle over the planes, never behind them.
            win.bring_to_top();
        }

        // mpv creates (and on reloads recreates) its own renderer window below
        // each plane; that window is what receives the pointer, so it has to be
        // subclassed as well or dragging silently stops working.
        self.refresh_input_hooks();
    }

    /// Subclasses the renderer windows mpv put below the planes.
    ///
    /// Cheap enough to run from the layout pass, but throttled so a 60 Hz
    /// repaint does not enumerate the window tree every frame.
    pub fn refresh_input_hooks(&mut self) {
        if self.last_hook_scan.elapsed() < HOOK_SCAN_INTERVAL {
            return;
        }
        self.last_hook_scan = Instant::now();
        self.plane_a.refresh_children();
        self.plane_b.refresh_children();
    }

    /// Handle of the wipe grip window (`0` before it exists).
    pub fn grip_hwnd(&self) -> i64 {
        self.grip.hwnd
    }

    /// Handle of plane A, used to normalise grip coordinates.
    pub fn plane_a_hwnd(&self) -> i64 {
        self.plane_a.hwnd
    }

    /// Pushes the palette colours into the native grip window.
    ///
    /// The handle is a real Win32 window (egui cannot paint above the video),
    /// so its Material colours have to be forwarded by hand.
    pub fn sync_grip_style(&mut self, bg: u32, fg: u32, vertical: bool) {
        if self.grip_style == Some((bg, fg, vertical)) {
            return;
        }
        self.grip_style = Some((bg, fg, vertical));
        crate::video::childwin::set_grip_style(bg, fg, vertical);
        if let Some(win) = &self.grip.window {
            win.invalidate();
        }
    }

    // ── events / sync ─────────────────────────────────────────────────────

    /// Seeks every plane, so a scrub keeps A and B locked together.
    pub fn seek_all(&self, secs: f64) {
        for player in [self.player_a.as_ref(), self.player_b.as_ref()]
            .into_iter()
            .flatten()
        {
            player.seek_absolute(secs);
        }
    }

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
            Plane::A => self.player_a.as_deref(),
            Plane::B => self.player_b.as_deref(),
        }
    }

    /// Plane whose visible native surface owns `hwnd`.
    ///
    /// The stage container and the eframe toplevel also receive mouse messages
    /// (wheel events bubble up), so input is only accepted from a visible plane.
    pub fn plane_for_hwnd(&self, hwnd: i64) -> Option<Plane> {
        // `visible` is deliberately not required: resizing the stage (toggling
        // the playlist, entering fullscreen) hides a plane for a frame or two,
        // and dropping those events made dragging feel intermittent.
        let hit = |s: &Surface| s.rect.2 > 0 && s.owns(hwnd);
        if hit(&self.plane_a) {
            Some(Plane::A)
        } else if hit(&self.plane_b) {
            Some(Plane::B)
        } else {
            None
        }
    }

    /// Whether the player toplevel currently owns the keyboard focus.
    ///
    /// The cursor-polling fallback only runs then: otherwise a click on another
    /// window that happens to sit above the picture would scrub the video.
    pub fn app_is_foreground(&self) -> bool {
        find_window_by_title(WINDOW_TITLE).is_some_and(crate::video::childwin::is_foreground)
    }

    /// Hit-tests a screen point against the visible planes.
    ///
    /// Returns the plane plus the point in that plane's client coordinates.
    /// Used by the cursor-polling fallback, which does not depend on the
    /// renderer windows forwarding their mouse messages to us.
    pub fn plane_at_screen(&self, sx: i32, sy: i32) -> Option<(Plane, i32, i32)> {
        for plane in [Plane::A, Plane::B] {
            let surface = match plane {
                Plane::A => &self.plane_a,
                Plane::B => &self.plane_b,
            };
            if !surface.visible || surface.rect.2 <= 0 || surface.rect.3 <= 0 {
                continue;
            }
            let hwnd = surface.hwnd;
            if hwnd == 0 {
                continue;
            }
            let Some((x, y, w, h)) = crate::video::childwin::window_rect(hwnd) else {
                continue;
            };
            if sx < x || sy < y || sx >= x + w || sy >= y + h {
                continue;
            }
            if let Some((cx, cy)) = crate::video::childwin::screen_to_client(hwnd, sx, sy) {
                return Some((plane, cx, cy));
            }
        }
        None
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
        let text = |name: &str| player.get_property(name).unwrap_or_default();
        let container = pretty_container(&text("file-format"));
        // After hwdec the pixelformat turns into `d3d11`; the sw format is
        // what the user cares about, so prefer the hw-pixelformat in that case.
        let pf = text("video-params/pixelformat");
        let pix_fmt = if matches!(
            pf.as_str(),
            "d3d11" | "d3d11va" | "dxva2" | "vaapi" | "vdpau" | "cuda" | "videotoolbox"
        ) {
            text("video-params/hw-pixelformat")
        } else {
            pf
        };
        let audio_codec = text("audio-codec");
        let audio = if audio_codec.is_empty() || audio_codec == "no" {
            String::new()
        } else {
            let ch = number("audio-params/channel-count").unwrap_or(0.0) as i32;
            let rate = number("audio-params/samplerate").unwrap_or(0.0) as i32;
            let mut out = audio_codec;
            if ch > 0 {
                out.push_str(&format!(" {ch}ch"));
            }
            if rate > 0 {
                out.push_str(&format!(" {} kHz", (rate as f64 / 1000.0).round()));
            }
            out
        };
        let duration = number("duration").unwrap_or(0.0);
        let size = number("file-size").unwrap_or(0.0) as u64;
        let bps = number("video-bitrate").filter(|v| *v > 0.0).or_else(|| {
            // Some encodes do not report a bitrate; derive it from size.
            (duration > 0.0 && size > 0)
                .then(|| size as f64 * 8.0 / duration)
                .filter(|v| *v > 0.0)
        });
        let bitrate = match bps {
            Some(bps) if bps >= 1_000_000.0 => format!("{:.1} Mbps", bps / 1_000_000.0),
            Some(bps) => format!("{:.0} Kbps", bps / 1000.0),
            None => String::new(),
        };
        let info = MediaInfo {
            width,
            height,
            fps,
            codec,
            container,
            duration,
            audio,
            bitrate,
            size,
            pix_fmt,
        };
        match plane {
            Plane::A => self.meta_a = Some(info),
            Plane::B => self.meta_b = Some(info),
        }
    }
}

/// Turns mpv's demuxer list into the name people use, e.g. `matroska,webm`
/// becomes `Matroska / WebM`.
fn pretty_container(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let lower = raw.to_ascii_lowercase();
    let named = if lower.contains("matroska") {
        "Matroska / WebM"
    } else if lower.contains("mov,mp4") || lower.contains("mp4") {
        "MP4"
    } else if lower.contains("avi") {
        "AVI"
    } else if lower.contains("mpegts") {
        "MPEG-TS"
    } else if lower.contains("flv") {
        "FLV"
    } else if lower.contains("ogg") {
        "Ogg"
    } else if lower.contains("wav") {
        "WAV"
    } else if lower.contains("aac") {
        "AAC"
    } else {
        return raw.to_string();
    };
    named.to_string()
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
