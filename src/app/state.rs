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
    /// Recently opened directory for the file picker.
    pub last_dir: String,
    /// Last files, restored as a playlist.
    pub recent: Vec<String>,
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
            last_dir: String::new(),
            recent: Vec::new(),
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
        serde_json::from_str(&text).unwrap_or_default()
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

pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let e = e.to_ascii_lowercase();
            VIDEO_EXTS.contains(&e.as_str())
        })
        .unwrap_or(false)
}
