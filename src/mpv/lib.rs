//! Runtime loader for `libmpv-2.dll`.
//!
//! The application links libmpv dynamically so a missing DLL surfaces as a
//! readable start-up error instead of a loader failure. [`LibMpv`] owns the
//! `libloading::Library`; every [`MpvPlayer`] shares it through an `Arc`, and
//! the library is only unloaded once the last player is gone.

use std::ffi::{c_char, c_int, c_ulong, c_void};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use libloading::Library;

use crate::mpv::MpvOptions;
use crate::mpv::player::MpvPlayer;

/// File name of the shared library this crate targets.
const DLL_NAME: &str = "libmpv-2.dll";

/// Opaque `mpv_handle *` owned by exactly one player.
pub(crate) type MpvHandle = *mut c_void;

/// `mpv_format` values used by the player.
pub(crate) mod format {
    /// Property is unavailable / observation was cancelled.
    pub const NONE: std::ffi::c_int = 0;
    pub const STRING: std::ffi::c_int = 1;
    pub const FLAG: std::ffi::c_int = 3;
    pub const INT64: std::ffi::c_int = 4;
    pub const DOUBLE: std::ffi::c_int = 5;
}

/// `mpv_event_id` values the player reacts to.
pub(crate) mod event_id {
    pub const NONE: std::ffi::c_int = 0;
    pub const SHUTDOWN: std::ffi::c_int = 1;
    pub const LOG_MESSAGE: std::ffi::c_int = 2;
    pub const END_FILE: std::ffi::c_int = 7;
    pub const FILE_LOADED: std::ffi::c_int = 8;
    pub const PLAYBACK_RESTART: std::ffi::c_int = 21;
    pub const PROPERTY_CHANGE: std::ffi::c_int = 22;
}

/// Raw `mpv_event` returned by `mpv_wait_event`.
///
/// Mirrors `libmpv/client.h`; the extra fields are part of the C layout.
#[repr(C)]
pub(crate) struct MpvEventRaw {
    pub event_id: c_int,
    /// Error associated with this event (0 for most events).
    #[allow(dead_code)] // Required for the C layout.
    pub error: c_int,
    /// Identifies the observation/command that triggered the event.
    #[allow(dead_code)] // Required for the C layout; the payload already carries names.
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

/// Raw `mpv_event_property` payload of `MPV_EVENT_PROPERTY_CHANGE`.
#[repr(C)]
pub(crate) struct MpvEventProperty {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}

/// Raw `mpv_event_log_message` payload.
#[repr(C)]
pub(crate) struct MpvEventLogMessage {
    pub prefix: *const c_char,
    pub level: *const c_char,
    pub text: *const c_char,
    /// Log level as a number; the string form is enough for the UI.
    #[allow(dead_code)] // Required for the C layout.
    pub log_level: c_int,
}

/// Raw `mpv_event_end_file` payload.
#[repr(C)]
pub(crate) struct MpvEventEndFile {
    pub reason: c_int,
    pub error: c_int,
    /// Playlist fields are part of the C layout but not needed by the app.
    #[allow(dead_code)]
    pub playlist_entry_id: i64,
    #[allow(dead_code)]
    pub playlist_insert_id: i64,
    #[allow(dead_code)]
    pub playlist_insert_num_entries: c_int,
}

/// Declares the libmpv functions used by this crate and resolves them on load.
macro_rules! mpv_api {
    ($( $field:ident : $name:literal, $symbol:literal => fn($($arg:ty),* $(,)?) -> $ret:ty; )*) => {
        /// Function pointers into one loaded `libmpv-2.dll`.
        pub(crate) struct Api {
            $(
                pub(crate) $field: unsafe extern "C" fn($($arg),*) -> $ret,
            )*
        }

        impl Api {
            /// Resolves every function this crate needs.
            ///
            /// # Safety
            ///
            /// `library` must stay loaded for as long as any function pointer in
            /// the returned table can be called. [`LibMpv`] upholds this by
            /// keeping the `Library` in the same allocation as the `Api`.
            unsafe fn load(library: &Library) -> anyhow::Result<Self> {
                Ok(Self {
                    $(
                        $field: {
                            // SAFETY: symbol name and signature match libmpv's
                            // public C API; the caller guarantees that the
                            // library outlives this table.
                            let function = unsafe {
                                library.get::<unsafe extern "C" fn($($arg),*) -> $ret>($symbol)
                            }
                            .with_context(|| {
                                format!("libmpv is missing the {} export", $name)
                            })?;
                            *function
                        },
                    )*
                })
            }
        }
    };
}

mpv_api! {
    create: "mpv_create", b"mpv_create\0" => fn() -> MpvHandle;
    set_option_string: "mpv_set_option_string", b"mpv_set_option_string\0" =>
        fn(MpvHandle, *const c_char, *const c_char) -> c_int;
    initialize: "mpv_initialize", b"mpv_initialize\0" => fn(MpvHandle) -> c_int;
    command: "mpv_command", b"mpv_command\0" =>
        fn(MpvHandle, *const *const c_char) -> c_int;
    command_string: "mpv_command_string", b"mpv_command_string\0" =>
        fn(MpvHandle, *const c_char) -> c_int;
    set_property: "mpv_set_property", b"mpv_set_property\0" =>
        fn(MpvHandle, *const c_char, c_int, *mut c_void) -> c_int;
    set_property_string: "mpv_set_property_string", b"mpv_set_property_string\0" =>
        fn(MpvHandle, *const c_char, *const c_char) -> c_int;
    get_property_string: "mpv_get_property_string", b"mpv_get_property_string\0" =>
        fn(MpvHandle, *const c_char) -> *mut c_char;
    free: "mpv_free", b"mpv_free\0" => fn(*mut c_void);
    observe_property: "mpv_observe_property", b"mpv_observe_property\0" =>
        fn(MpvHandle, u64, *const c_char, c_int) -> c_int;
    unobserve_property: "mpv_unobserve_property", b"mpv_unobserve_property\0" =>
        fn(MpvHandle, u64) -> c_int;
    wait_event: "mpv_wait_event", b"mpv_wait_event\0" =>
        fn(MpvHandle, f64) -> *mut MpvEventRaw;
    request_log_messages: "mpv_request_log_messages", b"mpv_request_log_messages\0" =>
        fn(MpvHandle, *const c_char) -> c_int;
    set_wakeup_callback: "mpv_set_wakeup_callback", b"mpv_set_wakeup_callback\0" =>
        fn(MpvHandle, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void);
    terminate_destroy: "mpv_terminate_destroy", b"mpv_terminate_destroy\0" =>
        fn(MpvHandle);
    error_string: "mpv_error_string", b"mpv_error_string\0" =>
        fn(c_int) -> *const c_char;
    client_api_version: "mpv_client_api_version", b"mpv_client_api_version\0" =>
        fn() -> c_ulong;
}

/// One loaded `libmpv-2.dll` shared by all players.
pub struct LibMpv {
    /// Keeps the DLL loaded. Dropped after `api`, so the function pointers stay
    /// valid for the whole life of the struct.
    _library: Library,
    api: Api,
    path: PathBuf,
}

impl LibMpv {
    /// Locates and loads `libmpv-2.dll`.
    ///
    /// Search order: `MUXVIDEO_LIBMPV`, the executable directory, the
    /// executable directory's `tools/mpv`, the project's `tools/mpv`, and the
    /// development machine's mpv.net-DW install. The first candidate that both
    /// exists and exports a compatible libmpv client API wins.
    pub fn load() -> anyhow::Result<Arc<Self>> {
        let candidates = Self::candidates();
        let mut failures = Vec::with_capacity(candidates.len());
        for candidate in &candidates {
            if !candidate.is_file() {
                failures.push(format!("{}: file not found", candidate.display()));
                continue;
            }
            match Self::load_from(candidate) {
                Ok(lib) => return Ok(Arc::new(lib)),
                Err(err) => failures.push(format!("{}: {err:#}", candidate.display())),
            }
        }

        bail!(
            "could not load {DLL_NAME} (tried {} locations):\n  {}",
            candidates.len(),
            failures.join("\n  ")
        )
    }

    /// Creates a player bound to the native window handle `wid`.
    pub fn new_player(self: &Arc<Self>, wid: i64, opts: &MpvOptions) -> anyhow::Result<MpvPlayer> {
        MpvPlayer::new(self, wid, opts)
    }

    /// Absolute path of the DLL this instance loaded.
    pub fn dll_path(&self) -> &Path {
        &self.path
    }

    /// Function table used by the player implementation.
    pub(crate) fn api(&self) -> &Api {
        &self.api
    }

    fn load_from(path: &Path) -> anyhow::Result<Self> {
        // SAFETY: loading a shared library executes its initialisation code but
        // reads no memory we own. The handle is kept in `Self` until every
        // function pointer below has been dropped.
        let library = unsafe { Library::new(path) }
            .with_context(|| format!("loading {}", path.display()))?;

        // SAFETY: the library is held in `library` (and then in `Self`) for the
        // whole time the resolved function pointers are used.
        let api = unsafe { Api::load(&library) }
            .with_context(|| format!("resolving libmpv exports from {}", path.display()))?;

        // The player relies on the 2.x client API. Reject mpv 1.x builds early;
        // the returned version is `(major << 16) | minor`.
        // SAFETY: `mpv_client_api_version` is safe to call without a handle.
        let version = unsafe { (api.client_api_version)() };
        let major = version >> 16;
        if major != 2 {
            bail!(
                "unsupported libmpv client API {major}.{} (expected 2.x)",
                version & 0xffff
            );
        }

        Ok(Self {
            _library: library,
            api,
            path: path.to_path_buf(),
        })
    }

    /// Candidate DLL paths in search order, with duplicates removed.
    fn candidates() -> Vec<PathBuf> {
        let mut paths = Vec::with_capacity(6);

        if let Some(explicit) = std::env::var_os("MUXVIDEO_LIBMPV") {
            if !explicit.is_empty() {
                paths.push(PathBuf::from(explicit));
            }
        }

        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                paths.push(dir.join(DLL_NAME));
                paths.push(dir.join("tools").join("mpv").join(DLL_NAME));
            }
        }

        // `target/debug/mpv_probe.exe` lives two levels below the project root,
        // so during development the manifest directory is the reliable anchor.
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        paths.push(project.join("tools").join("mpv").join(DLL_NAME));

        // Handy when the binary is started from a copied build tree.
        if let Ok(cwd) = std::env::current_dir() {
            paths.push(cwd.join("tools").join("mpv").join(DLL_NAME));
        }

        paths.push(
            PathBuf::from(r"D:\ruanjian\mpv\mpv.net-DW v2.1.0").join(DLL_NAME),
        );

        let mut unique: Vec<PathBuf> = Vec::with_capacity(paths.len());
        for path in paths {
            if !unique.contains(&path) {
                unique.push(path);
            }
        }
        unique
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_are_unique_and_include_the_project_copy() {
        let candidates = LibMpv::candidates();
        assert!(!candidates.is_empty());

        let mut deduped = candidates.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(candidates.len(), deduped.len(), "candidate list contains duplicates");

        let project_copy = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tools")
            .join("mpv")
            .join(DLL_NAME);
        assert!(
            candidates.contains(&project_copy),
            "project-local libmpv path is missing from {:?}",
            candidates
        );
    }
}
