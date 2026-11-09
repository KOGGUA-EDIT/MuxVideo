//! Standalone Win32 probe for the in-process libmpv engine.
//!
//! The probe creates a plain Win32 toplevel window with two native child
//! surfaces, embeds one [`LibMpv`] player per surface and plays the two test
//! clips for about 20 seconds. It prints one status line per plane per second,
//! writes `tools\mpv_probe_report.txt` and exits with a non-zero status unless
//! both planes created a GPU video output and advanced their `time-pos`.
//!
//! The GUI must be launched outside the sandboxed shell (for example through
//! the computer-use `launch_app` tool) because sandboxed child GUI processes are
//! terminated as soon as the invoking command returns.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use muxvideo::mpv::{LibMpv, MpvEvent, MpvOptions, MpvPlayer};
use muxvideo::video::childwin::ChildWindow;
use windows_sys::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, IDC_ARROW, IsWindow, LoadCursorW, MSG, PM_REMOVE, PeekMessageW,
    PostQuitMessage, RegisterClassW, TranslateMessage, WM_CLOSE, WM_DESTROY, WM_QUIT, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

const WINDOW_CLASS: &str = "MuxVideoProbeWindow";
const WINDOW_TITLE: &str = "MuxVideo MPV Probe";
const WINDOW_WIDTH: i32 = 1000;
const WINDOW_HEIGHT: i32 = 620;
const RUN_SECONDS: u64 = 20;
const REPORT_FILE: &str = "mpv_probe_report.txt";
const ADVANCE_THRESHOLD_SECS: f64 = 0.25;
const MAX_SAVED_VO_LINES: usize = 24;
const DROP_PROPERTIES: [&str; 4] = [
    "vo-drop-frame-count",
    "frame-drop-count",
    "drop-frame-count",
    "decoder-frame-drop-count",
];

fn main() -> Result<()> {
    let project_root = find_project_root();
    let tools_dir = project_root.join("tools");
    let report_path = tools_dir.join(REPORT_FILE);

    match run(&project_root) {
        Ok(report) => {
            let text = report.render();
            print!("{text}");
            if let Err(error) = std::fs::write(&report_path, &text) {
                return Err(error).with_context(|| format!("writing {}", report_path.display()));
            }
            if report.passed() {
                Ok(())
            } else {
                bail!(
                    "probe FAILED (see {}); planes: {}",
                    report_path.display(),
                    report.planes_summary()
                )
            }
        }
        Err(error) => {
            let text = format!(
                "MuxVideo MPV probe report\nProject: {}\nResult: FAIL\nError: {error:#}\n",
                project_root.display()
            );
            print!("{text}");
            let _ = std::fs::create_dir_all(&tools_dir);
            let _ = std::fs::write(&report_path, &text);
            Err(error)
        }
    }
}

/// Runs the probe and returns the collected report data.
fn run(project_root: &Path) -> Result<ProbeReport> {
    let media_a = project_root.join("media").join("clipA.mp4");
    let media_b = project_root.join("media").join("clipB.mp4");
    for media in [&media_a, &media_b] {
        if !media.is_file() {
            bail!("test clip is missing: {}", media.display());
        }
    }

    let window = ProbeWindow::create()?;
    let plane_a_window = ChildWindow::create(window.hwnd(), 1, "Plane A")?;
    let plane_b_window = ChildWindow::create(window.hwnd(), 2, "Plane B")?;
    plane_a_window.set_rect(16, 64, 480, 270);
    plane_b_window.set_rect(504, 64, 480, 270);

    let lib = LibMpv::load()?;
    let options = MpvOptions {
        audio: false,
        loop_file: true,
        ..MpvOptions::default()
    };
    let player_a = lib.new_player(plane_a_window.hwnd(), &options)?;
    let player_b = lib.new_player(plane_b_window.hwnd(), &options)?;

    player_a.load_file(&media_a.to_string_lossy());
    player_b.load_file(&media_b.to_string_lossy());

    let mut plane_a = PlaneStats::new("A", media_a);
    let mut plane_b = PlaneStats::new("B", media_b);

    let started = Instant::now();
    let mut next_sample = started + Duration::from_secs(1);
    while started.elapsed() < Duration::from_secs(RUN_SECONDS) {
        if !pump_messages() {
            break;
        }

        drain_events(&mut plane_a, &player_a);
        drain_events(&mut plane_b, &player_b);

        let now = Instant::now();
        if now >= next_sample {
            println!("{}", sample(&mut plane_a, &player_a));
            println!("{}", sample(&mut plane_b, &player_b));
            next_sample = now + Duration::from_secs(1);
        }

        std::thread::sleep(Duration::from_millis(50));
    }

    // Final sample after the last frame; also drains any remaining events.
    drain_events(&mut plane_a, &player_a);
    drain_events(&mut plane_b, &player_b);
    println!("{}", sample(&mut plane_a, &player_a));
    println!("{}", sample(&mut plane_b, &player_b));

    Ok(ProbeReport {
        dll_path: lib.dll_path().to_path_buf(),
        elapsed: started.elapsed(),
        planes: vec![plane_a, plane_b],
    })
}

/// Minimal toplevel window used only by the probe.
struct ProbeWindow {
    hwnd: HWND,
}

impl ProbeWindow {
    fn create() -> Result<Self> {
        let class_name = wide(WINDOW_CLASS);
        // SAFETY: NULL requests the handle of the current executable, which is
        // valid for the whole process.
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        if instance.is_null() {
            // SAFETY: no arguments, always safe to call.
            let error = unsafe { GetLastError() };
            bail!("GetModuleHandleW(NULL) failed (Win32 error {error})");
        }

        let window_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(probe_wndproc),
            hInstance: instance,
            // SAFETY: stock resources are process-wide and never freed by us.
            hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
            hbrBackground: unsafe { GetStockObject(BLACK_BRUSH) },
            lpszClassName: class_name.as_ptr(),
            ..Default::default()
        };
        // SAFETY: the class description and its UTF-16 name are live for the
        // duration of the call.
        let atom = unsafe { RegisterClassW(&window_class) };
        if atom == 0 {
            // SAFETY: no arguments, always safe to call.
            let error = unsafe { GetLastError() };
            if error != ERROR_CLASS_ALREADY_EXISTS {
                bail!("RegisterClassW({WINDOW_CLASS}) failed (Win32 error {error})");
            }
        }

        let title = wide(WINDOW_TITLE);
        // SAFETY: both UTF-16 strings are live for the call; the window has no
        // parent and uses the module instance registered above.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            // SAFETY: no arguments, always safe to call.
            let error = unsafe { GetLastError() };
            bail!("CreateWindowExW({WINDOW_CLASS}) failed (Win32 error {error})");
        }

        Ok(Self { hwnd })
    }

    fn hwnd(&self) -> i64 {
        self.hwnd as isize as i64
    }
}

impl Drop for ProbeWindow {
    fn drop(&mut self) {
        if self.hwnd.is_null() {
            return;
        }
        // SAFETY: the handle was created by `ProbeWindow::create`; destroying it
        // once here is safe, and `IsWindow` guards a user-triggered close.
        unsafe {
            if IsWindow(self.hwnd) != 0 {
                DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = std::ptr::null_mut();
    }
}

/// Pumps the calling thread's message queue. Returns `false` once `WM_QUIT`
/// has been received (the probe window was closed by the user).
fn pump_messages() -> bool {
    loop {
        let mut message = MSG::default();
        // SAFETY: `message` is a live, writable MSG; a NULL window filter asks
        // for messages of the calling thread.
        let available =
            unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) };
        if available == 0 {
            return true;
        }
        if message.message == WM_QUIT {
            return false;
        }
        // SAFETY: `message` was filled in by PeekMessageW above.
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Window procedure for the probe toplevel window.
///
/// # Safety
///
/// Standard Win32 window-procedure contract: `hwnd` is supplied by the system
/// and the remaining arguments are message-specific.
unsafe extern "system" fn probe_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLOSE => {
            // SAFETY: `hwnd` is the window receiving the message.
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            // SAFETY: no arguments; ends the probe's message loop.
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        // SAFETY: default handling for all other messages.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// Per-plane state gathered while the probe runs.
struct PlaneStats {
    label: &'static str,
    media: PathBuf,
    max_time_pos: f64,
    final_time_pos: f64,
    duration: f64,
    hwdec_current: String,
    dropped: String,
    vo_lines: Vec<String>,
    saw_gpu_vo: bool,
}

impl PlaneStats {
    fn new(label: &'static str, media: PathBuf) -> Self {
        Self {
            label,
            media,
            max_time_pos: 0.0,
            final_time_pos: 0.0,
            duration: 0.0,
            hwdec_current: "unknown".to_owned(),
            dropped: "n/a".to_owned(),
            vo_lines: Vec::new(),
            saw_gpu_vo: false,
        }
    }

    fn passed(&self) -> bool {
        self.saw_gpu_vo && self.max_time_pos > ADVANCE_THRESHOLD_SECS
    }
}

fn drain_events(stats: &mut PlaneStats, player: &MpvPlayer) {
    for event in player.poll_events() {
        if let MpvEvent::Log { text, .. } = event {
            let line = text.trim();
            if line.contains("VO:") {
                stats.saw_gpu_vo |= line.contains("VO: [gpu]");
                if stats.vo_lines.len() < MAX_SAVED_VO_LINES {
                    stats.vo_lines.push(line.to_owned());
                }
            }
        }
    }
}

/// Reads the current playback properties and formats one status line.
fn sample(stats: &mut PlaneStats, player: &MpvPlayer) -> String {
    stats.final_time_pos = player.time_pos();
    stats.duration = player.duration();
    stats.max_time_pos = stats.max_time_pos.max(stats.final_time_pos);

    if let Some(hwdec) = player.get_property("hwdec-current") {
        let hwdec = hwdec.trim();
        if !hwdec.is_empty() {
            stats.hwdec_current = hwdec.to_owned();
        }
    }

    stats.dropped = DROP_PROPERTIES
        .iter()
        .find_map(|name| {
            let value = player.get_property(name)?;
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(format!("{name}={value}"))
            }
        })
        .unwrap_or_else(|| "n/a".to_owned());

    format!(
        "{} pos={:.2} dur={:.2} vf-drop={} hwdec={}",
        stats.label, stats.final_time_pos, stats.duration, stats.dropped, stats.hwdec_current
    )
}

struct ProbeReport {
    dll_path: PathBuf,
    elapsed: Duration,
    planes: Vec<PlaneStats>,
}

impl ProbeReport {
    fn passed(&self) -> bool {
        self.planes.len() == 2 && self.planes.iter().all(PlaneStats::passed)
    }

    fn planes_summary(&self) -> String {
        self.planes
            .iter()
            .map(|plane| {
                format!(
                    "{}: VO: [gpu]={}, max pos={:.2}s",
                    plane.label,
                    yes_no(plane.saw_gpu_vo),
                    plane.max_time_pos
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "MuxVideo MPV probe report");
        let _ = writeln!(out, "DLL: {}", self.dll_path.display());
        let _ = writeln!(out, "Elapsed: {:.1} s", self.elapsed.as_secs_f64());

        for plane in &self.planes {
            let _ = writeln!(out, "\nPlane {}: {}", plane.label, plane.media.display());
            let _ = writeln!(out, "  VO lines ({}):", plane.vo_lines.len());
            if plane.vo_lines.is_empty() {
                let _ = writeln!(out, "    (none)");
            }
            for line in &plane.vo_lines {
                let _ = writeln!(out, "    {line}");
            }
            let _ = writeln!(out, "  hwdec-current: {}", plane.hwdec_current);
            let _ = writeln!(
                out,
                "  final time-pos: {:.2} s (max {:.2} s, duration {:.2} s)",
                plane.final_time_pos, plane.max_time_pos, plane.duration
            );
            let _ = writeln!(out, "  frame drops: {}", plane.dropped);
            let _ = writeln!(out, "  saw VO: [gpu]: {}", yes_no(plane.saw_gpu_vo));
            let _ = writeln!(
                out,
                "  status: {}",
                if plane.passed() { "PASS" } else { "FAIL" }
            );
        }

        let both_gpu = self.planes.len() == 2 && self.planes.iter().all(|plane| plane.saw_gpu_vo);
        let _ = writeln!(
            out,
            "\nBoth planes reported VO: [gpu]: {}",
            yes_no(both_gpu)
        );
        let _ = writeln!(
            out,
            "Result: {}",
            if self.passed() { "PASS" } else { "FAIL" }
        );
        out
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

/// Finds the project root by walking up from the executable and the current
/// directory until a `Cargo.toml` next to a `media` directory is found.
fn find_project_root() -> PathBuf {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().skip(1) {
            roots.push(ancestor.to_path_buf());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.extend(cwd.ancestors().map(Path::to_path_buf));
    }

    for root in &roots {
        if root.join("Cargo.toml").is_file() && root.join("media").is_dir() {
            return root.clone();
        }
    }
    for root in &roots {
        if root.join("Cargo.toml").is_file() {
            return root.clone();
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_encoding_is_nul_terminated() {
        assert_eq!(wide(""), vec![0]);
        assert_eq!(wide("abc"), vec![0x61, 0x62, 0x63, 0]);
    }

    #[test]
    fn probe_stats_require_gpu_vo_and_progress() {
        let mut stats = PlaneStats::new("A", PathBuf::from("clip.mp4"));
        assert!(!stats.passed());
        stats.saw_gpu_vo = true;
        assert!(!stats.passed());
        stats.max_time_pos = 1.0;
        assert!(stats.passed());
    }
}
