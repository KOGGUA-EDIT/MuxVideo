//! Opt-in diagnostics.
//!
//! Some problems are intermittent and cannot be reasoned about by reading the
//! code: a click that sometimes reaches the wrong window, a resize that
//! sometimes turns into a move. Drop a file named `input-debug.flag` next to
//! the executable (or set `MUXVIDEO_LOG=1`) and every interesting event of the
//! native/top level interaction layer is appended to `app-debug.log` in the
//! same folder.

use std::io::Write;
use std::path::PathBuf;

/// Where the log goes: next to the executable, so installed and dev builds
/// never share a file.
fn log_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("app-debug.log"))
}

/// True when diagnostics were requested.
pub fn enabled() -> bool {
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
    if !enabled() {
        return;
    }
    let Some(path) = log_path() else { return };
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = writeln!(file, "{args}");
}

/// `diag!("format {}", value)` — no-op unless diagnostics are enabled.
#[macro_export]
macro_rules! diag {
    ($($arg:tt)*) => {
        if $crate::app::diag::enabled() {
            $crate::app::diag::log(format_args!($($arg)*));
        }
    };
}
