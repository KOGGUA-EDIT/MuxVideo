//! Boot preload.
//!
//! Window creation and the GL context take long enough that the three expensive
//! things the first frame needs — the settings file, the ~20 MB CJK face and
//! `libmpv-2.dll` — are fetched on a worker thread started from `main`, and
//! joined once eframe asks for the app. On a warm page cache the join is free;
//! on a cold one it overlaps the window work instead of adding to it.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use crate::app::state::Settings;
use crate::fonts::Face;
use crate::mpv::LibMpv;

/// Handle to the boot worker.
pub struct Boot {
    rx: Receiver<Parts>,
}

/// Everything the worker produced, plus how long each step took (ms).
pub struct Parts {
    pub settings: Settings,
    pub face: Option<Face>,
    pub lib: Result<Arc<LibMpv>, String>,
    pub ms: [f64; 3],
}

impl Boot {
    /// Starts the worker. Never fails: a thread that cannot start simply leaves
    /// the receiver empty and [`Boot::join`] returns `None`.
    #[must_use]
    pub fn start() -> Self {
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new()
            .name("muxvideo-boot".to_owned())
            .stack_size(256 * 1024)
            .spawn(move || {
                let t = std::time::Instant::now();
                let elapsed = |t: std::time::Instant| t.elapsed().as_secs_f64() * 1000.0;

                let settings = Settings::load();
                let ms0 = elapsed(t);
                let face = crate::fonts::load_face();
                let ms1 = elapsed(t);
                let lib = LibMpv::load().map_err(|err| format!("{err:#}"));
                let ms2 = elapsed(t);

                let _ = tx.send(Parts {
                    settings,
                    face,
                    lib,
                    ms: [ms0, ms1, ms2],
                });
            });
        if let Err(err) = spawned {
            crate::diag!("[boot] worker did not start: {err}");
        }
        Self { rx }
    }

    /// Waits for the worker and logs its timings.
    #[must_use]
    pub fn join(self) -> Option<Parts> {
        let parts = self.rx.recv().ok()?;
        crate::diag!(
            "[boot] worker: settings {:.1} ms · cjk face {:.1} ms · libmpv {:.1} ms",
            parts.ms[0],
            parts.ms[1],
            parts.ms[2]
        );
        Some(parts)
    }
}
