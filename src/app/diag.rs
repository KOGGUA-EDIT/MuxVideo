//! Diagnostics.
//!
//! Interaction bugs that only show up from time to time cannot be reasoned
//! about by reading the code: a press that reaches the wrong window, a drag
//! that loses its messages half way. So the interesting events of the native
//! and top level interaction layers are always written to `app-debug.log` next
//! to the executable — but only the *state* ones; the per-frame chatter (hover
//! moves, skipped polls) needs `input-debug.flag` (or `MUXVIDEO_LOG=1`) and is
//! then written in full.
//!
//! The log is rotated at [`MAX_BYTES`] so a forgotten flag can never fill the
//! disk like the 31 MB one that prompted this design.

use std::io::Write;
use std::path::PathBuf;

/// Rotate (truncate) once the log grows past this size.
const MAX_BYTES: u64 = 512 * 1024;

/// Process start, used to time the boot phases in `app-debug.log`.
static BOOT_T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Boot marks are buffered and written in one go: opening the log twenty times
/// during startup costs more than the phases being measured.
static BOOT_LINES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Starts (or returns) the boot stopwatch. Call it as the first thing in
/// `main` so every `[boot]` line carries a comparable offset.
pub fn boot_start() -> std::time::Instant {
    *BOOT_T0.get_or_init(std::time::Instant::now)
}

/// Logs one boot phase with its offset from [`boot_start`].
///
/// Buffered: [`flush_boot`] writes the whole batch once the first frames are
/// done, which keeps twenty appends off the startup path.
pub fn mark(phase: &str) {
    let ms = boot_start().elapsed().as_secs_f64() * 1000.0;
    if let Ok(mut lines) = BOOT_LINES.lock() {
        lines.push(format!("[boot] +{ms:7.1} ms  {phase}"));
    }
}

/// Writes the buffered [`mark`] lines to the log. Safe to call more than once.
pub fn flush_boot() {
    let Ok(mut lines) = BOOT_LINES.lock() else {
        return;
    };
    if lines.is_empty() {
        return;
    }
    let joined = lines.join("\n");
    lines.clear();
    drop(lines);
    log(format_args!("{joined}"));
}

/// Where the log goes: next to the executable, so installed and dev builds
/// never share a file.
fn log_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("app-debug.log"))
}

/// True when the noisy per-frame diagnostics were requested.
pub fn verbose() -> bool {
    if std::env::var_os("MUXVIDEO_LOG").is_some() {
        return true;
    }
    let Some(exe) = std::env::current_exe().ok() else {
        return false;
    };
    let Some(dir) = exe.parent() else {
        return false;
    };
    dir.join("input-debug.flag").exists()
}

/// Appends one line, ignoring every error: diagnostics must never break the app.
pub fn log(args: std::fmt::Arguments<'_>) {
    let Some(path) = log_path() else { return };
    if let Ok(meta) = std::fs::metadata(&path)
        && meta.len() > MAX_BYTES
    {
        let _ = std::fs::write(&path, b"");
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = writeln!(file, "{args}");
}

/// `diag!("format {}", value)` — always records, for state transitions.
#[macro_export]
macro_rules! diag {
    ($($arg:tt)*) => {
        $crate::app::diag::log(format_args!($($arg)*));
    };
}

/// `diag_verbose!(...)` — only when `input-debug.flag` is present.
#[macro_export]
macro_rules! diag_verbose {
    ($($arg:tt)*) => {
        if $crate::app::diag::verbose() {
            $crate::app::diag::log(format_args!($($arg)*));
        }
    };
}
