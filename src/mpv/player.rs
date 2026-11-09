//! One embedded libmpv instance.

use crate::mpv::MpvEvent;

/// Options applied before `mpv_initialize`.
#[derive(Clone, Debug)]
pub struct MpvOptions {
    /// `hwdec` value, e.g. `auto-safe`.
    pub hwdec: String,
    /// Video output driver (`gpu` / `gpu-next`).
    pub vo: String,
    /// `gpu-context` (`d3d11`).
    pub gpu_context: String,
    pub audio: bool,
    pub loop_file: bool,
    /// Seconds of demuxer cache.
    pub cache_secs: u32,
}

impl Default for MpvOptions {
    fn default() -> Self {
        Self {
            hwdec: "auto-safe".into(),
            vo: "gpu".into(),
            gpu_context: "d3d11".into(),
            audio: true,
            loop_file: true,
            cache_secs: 30,
        }
    }
}

pub struct MpvPlayer {
    _private: (),
}

impl MpvPlayer {
    pub fn load_file(&self, path: &str) {
        let _ = path;
    }
    /// Appends a file to the current playlist (used for lavfi blending).
    pub fn load_file_append(&self, path: &str) {
        let _ = path;
    }
    pub fn set_pause(&self, paused: bool) {
        let _ = paused;
    }
    pub fn paused(&self) -> bool {
        false
    }
    pub fn toggle_pause(&self) {}
    pub fn time_pos(&self) -> f64 {
        0.0
    }
    pub fn duration(&self) -> f64 {
        0.0
    }
    pub fn seek_absolute(&self, secs: f64) {
        let _ = secs;
    }
    pub fn seek_relative(&self, secs: f64) {
        let _ = secs;
    }
    pub fn set_speed(&self, speed: f64) {
        let _ = speed;
    }
    pub fn speed(&self) -> f64 {
        1.0
    }
    pub fn set_volume(&self, volume: f64) {
        let _ = volume;
    }
    pub fn set_mute(&self, mute: bool) {
        let _ = mute;
    }
    pub fn frame_step(&self) {}
    pub fn frame_back_step(&self) {}
    pub fn set_property(&self, name: &str, value: &str) -> bool {
        let _ = (name, value);
        false
    }
    pub fn get_property(&self, name: &str) -> Option<String> {
        let _ = name;
        None
    }
    pub fn poll_events(&self) -> Vec<MpvEvent> {
        Vec::new()
    }
    pub fn set_wid(&self, wid: i64) {
        let _ = wid;
    }
    pub fn stop(&self) {}
}
