//! Shell state: compare modes, persisted settings, playlist entries.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How the stage compares A and B.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareMode {
    /// Only A (or B when only B is loaded).
    #[default]
    Single,
    /// A and B side by side (or stacked), each scaled to its pane.
    Pair,
    /// Both planes in place, split by a movable edge.
    Wipe,
    /// A and B alternate in place, the classic flicker comparison.
    #[serde(alias = "Difference")]
    Blink,
}

impl CompareMode {
    pub const ALL: [Self; 4] = [Self::Single, Self::Pair, Self::Wipe, Self::Blink];

    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "单画面",
            Self::Pair => "并排",
            Self::Wipe => "划像",
            Self::Blink => "闪烁",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Single => "只看 A 面，B 静音待命",
            Self::Pair => "A / B 同帧并排，比构图与调色",
            Self::Wipe => "两路叠在一起，拖画面分界逐处对照",
            Self::Blink => "A / B 同位置快速切换，抖动即差异",
        }
    }

    /// Longer explanation used in tooltips on the compare bar.
    pub fn about(self) -> &'static str {
        match self {
            Self::Single => "只显示 A 面。切回单画面时 B 面会暂停解码，省性能。",
            Self::Pair => {
                "A / B 各占一半，同一时间点并排播放。适合对比两个版本的分辨率、\
构图、调色、字幕位置。"
            }
            Self::Wipe => {
                "A / B 画面完全重叠，只有分界线左侧显示 A、右侧显示 B。\
直接用鼠标在画面上左右拖动分界线，就能逐处对照——比并排更容易看出细微位移。"
            }
            Self::Blink => {
                "A / B 在同一个位置、同一时间点轮流显示，每秒切换几次。\
人眼对「闪一下动了的地方」极其敏感，所以修图、磨皮、降噪、锐化这类改动，\
闪烁比并排和划像都更容易看出来。"
            }
        }
    }

    /// Whether the stage needs a B plane.
    pub fn needs_b(self) -> bool {
        !matches!(self, Self::Single)
    }
}

/// Orientation of the split / wipe edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitAxis {
    /// Side by side: A left, B right; wipe edge is vertical.
    #[default]
    Horizontal,
    /// Stacked: A top, B bottom; wipe edge is horizontal.
    Vertical,
}

impl SplitAxis {
    pub fn label(self) -> &'static str {
        match self {
            Self::Horizontal => "左右",
            Self::Vertical => "上下",
        }
    }
}

/// Left navigation rail destinations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RailTab {
    #[default]
    Viewer,
    Compare,
    Settings,
}

impl RailTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Viewer => "查看",
            Self::Compare => "对比",
            Self::Settings => "设置",
        }
    }
}

/// One media file in the playlist.
#[derive(Clone, Debug)]
pub struct PlayEntry {
    pub path: PathBuf,
    pub name: String,
    /// File size in bytes, read once when the entry is created.
    pub size: u64,
}

impl PlayEntry {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Self { path, name, size }
    }

    pub fn path_str(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
}

/// Everything that survives a restart.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Material You seed colour as `#rrggbb`.
    pub seed: String,
    pub dark: bool,
    pub mode: CompareMode,
    pub axis: SplitAxis,
    /// Wipe position, 0.0 … 1.0 of the stage.
    pub wipe: f32,
    pub volume: f64,
    pub speed: f64,
    pub muted: bool,
    pub loop_file: bool,
    pub show_playlist: bool,
    /// `auto-safe`, `no`, `d3d11va` …
    pub hwdec: String,
    /// Horizontal pixels per frame when scrubbing on the picture (1 … 100).
    pub scrub_ppf: f64,
    /// Crop the picture to fill the stage instead of letterboxing it.
    pub fill: bool,
    /// Recently opened directory for the file picker.
    pub last_dir: String,
    /// Last files, restored as a playlist.
    pub recent: Vec<String>,
    /// Schema version, used to migrate older settings files once.
    #[serde(default)]
    pub cfg_version: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            seed: "#D0BCFF".into(),
            dark: true,
            mode: CompareMode::Single,
            axis: SplitAxis::Horizontal,
            wipe: 0.5,
            volume: 80.0,
            speed: 1.0,
            muted: false,
            loop_file: false,
            show_playlist: true,
            hwdec: "auto-safe".into(),
            scrub_ppf: SCRUB_PPF_DEFAULT,
            fill: false,
            last_dir: String::new(),
            recent: Vec::new(),
            cfg_version: CONFIG_VERSION,
        }
    }
}

fn settings_path() -> PathBuf {
    let base = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("muxvideo.settings.json")
}

impl Settings {
    pub fn load() -> Self {
        let path = settings_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut s: Self = serde_json::from_str(&text).unwrap_or_default();
        if s.cfg_version < CONFIG_VERSION {
            // v2: the scrub sensitivity default was wrong (20 px per frame made
            // dragging look broken on long clips); every older file gets the
            // responsive 1 px = 1 frame back.
            s.scrub_ppf = SCRUB_PPF_DEFAULT;
            s.cfg_version = CONFIG_VERSION;
            s.save();
        }
        s
    }

    pub fn save(&self) {
        let path = settings_path();
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }

    /// Parses `#rrggbb` into an sRGB colour, falling back to the MD3 baseline.
    pub fn seed_color(&self) -> eframe::egui::Color32 {
        parse_hex(&self.seed).unwrap_or(eframe::egui::Color32::from_rgb(0xD0, 0xBC, 0xFF))
    }

    pub fn scheme(&self) -> crate::ui::Scheme {
        if self.dark {
            crate::ui::Scheme::Dark
        } else {
            crate::ui::Scheme::Light
        }
    }
}

/// `#rgb` / `#rrggbb` → colour.
pub fn parse_hex(text: &str) -> Option<eframe::egui::Color32> {
    let t = text.trim().trim_start_matches('#');
    let (r, g, b) = match t.len() {
        3 => {
            let v = u32::from_str_radix(t, 16).ok()?;
            let f = |n: u32| ((n & 0xF) * 17) as u8;
            (f(v >> 8), f(v >> 4), f(v))
        }
        6 => {
            let v = u32::from_str_radix(t, 16).ok()?;
            ((v >> 16) as u8, (v >> 8) as u8, v as u8)
        }
        _ => return None,
    };
    Some(eframe::egui::Color32::from_rgb(r, g, b))
}

pub fn format_hex(color: eframe::egui::Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
}

/// `125.4` → `02:05.4`
pub fn format_time(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "00:00.0".into();
    }
    let total = secs;
    let h = (total / 3600.0).floor() as u64;
    let m = ((total % 3600.0) / 60.0).floor() as u64;
    let s = total % 60.0;
    if h > 0 {
        format!("{h}:{m:02}:{s:04.1}")
    } else {
        format!("{m:02}:{s:04.1}")
    }
}

/// Frame rate to use for timecode maths; falls back to 30 when unknown.
pub fn safe_fps(fps: f64) -> f64 {
    if fps.is_finite() && fps > 0.5 && fps < 1000.0 {
        fps
    } else {
        30.0
    }
}

/// Frame index containing `secs`.
pub fn frame_at(secs: f64, fps: f64) -> u64 {
    let fps = safe_fps(fps);
    (secs.max(0.0) * fps).round() as u64
}

/// `125.4 @ 24fps` → `00:02:05:09` — the timecode editors expect.
pub fn format_timecode(secs: f64, fps: f64) -> String {
    let fps = safe_fps(fps);
    let total = if secs.is_finite() && secs > 0.0 {
        secs
    } else {
        0.0
    };
    let whole = total.floor();
    let h = (whole / 3600.0) as u64;
    let m = ((whole % 3600.0) / 60.0) as u64;
    let s = (whole % 60.0) as u64;
    let per_sec = fps.round().max(1.0) as u64;
    let f = (((total - whole) * fps).round() as u64).min(per_sec.saturating_sub(1));
    format!("{h:02}:{m:02}:{s:02}:{f:02}")
}

/// Human readable file size for the playlist rows.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Toast messages shown bottom-left of the stage.
#[derive(Clone, Debug)]
pub struct Toast {
    pub text: String,
    pub error: bool,
    /// `Instant` when the toast appeared.
    pub since: std::time::Instant,
}

impl Toast {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: false,
            since: std::time::Instant::now(),
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: true,
            since: std::time::Instant::now(),
        }
    }

    pub fn alpha(&self) -> f32 {
        let age = self.since.elapsed().as_secs_f32();
        if age > 5.5 {
            0.0
        } else if age > 4.2 {
            (5.5 - age) / 1.3
        } else {
            1.0
        }
    }
}

/// Extensions the viewer accepts.
pub const VIDEO_EXTS: &[&str] = &[
    "mp4", "mkv", "mov", "avi", "webm", "m4v", "mpg", "mpeg", "ts", "m2ts", "flv", "wmv", "ogv",
    "rmvb", "mts", "3gp",
];

/// Default drag sensitivity: one frame every 20 px of horizontal travel.
pub const SCRUB_PPF_DEFAULT: f64 = 1.0;

/// Bump when an older settings file needs a one-time fix-up in `Settings::load`.
pub const CONFIG_VERSION: u32 = 2;
/// Hard bounds for the scrub sensitivity, matching the settings slider.
pub const SCRUB_PPF_RANGE: std::ops::RangeInclusive<f64> = 1.0..=100.0;

/// `%USERPROFILE%\Pictures`, falling back to the executable directory.
pub fn pictures_dir() -> std::path::PathBuf {
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let dir = std::path::Path::new(&profile).join("Pictures");
        if dir.is_dir() {
            return dir;
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// Local wall clock as `(year, month, day, hour, minute, second)`.
///
/// Uses `GetLocalTime` so screenshot names match the user's clock instead of
/// drifting by the UTC offset.
pub fn local_now() -> (u16, u16, u16, u16, u16, u16) {
    // SAFETY: `GetLocalTime` only writes into the supplied struct.
    unsafe {
        let mut st: windows_sys::Win32::Foundation::SYSTEMTIME = std::mem::zeroed();
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut st);
        (
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond,
        )
    }
}

/// `2026-11-10 02-52-31` style stamp for file names.
pub fn file_stamp() -> String {
    let (y, mo, d, h, mi, s) = local_now();
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let e = e.to_ascii_lowercase();
            VIDEO_EXTS.contains(&e.as_str())
        })
        .unwrap_or(false)
}
