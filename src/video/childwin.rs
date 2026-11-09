//! Win32 child windows that host an mpv video surface.
//!
//! One `ChildWindow` per video plane (A / B / difference). The window class is
//! registered once per process, paints solid black and never erases its
//! background so resizing does not flicker.

/// Finds the toplevel window with an exact title that belongs to this process.
pub fn find_window_by_title(title: &str) -> Option<i64> {
    let _ = title;
    None
}

pub fn current_process_id() -> u32 {
    std::process::id()
}

pub struct ChildWindow {
    _private: (),
}

impl ChildWindow {
    pub fn create(parent: i64, id: u32, name: &str) -> anyhow::Result<Self> {
        let _ = (parent, id, name);
        anyhow::bail!("child window not implemented yet")
    }

    pub fn hwnd(&self) -> i64 {
        0
    }

    pub fn set_rect(&self, x: i32, y: i32, w: i32, h: i32) {
        let _ = (x, y, w, h);
    }

    pub fn show(&self, visible: bool) {
        let _ = visible;
    }

    pub fn clip_to_left(&self, width: i32) {
        let _ = width;
    }

    /// Restricts the surface to the given client-space rect (wipe mode).
    pub fn set_clip_rect(&self, x: i32, y: i32, w: i32, h: i32) {
        let _ = (x, y, w, h);
    }

    pub fn clear_clip(&self) {}

    pub fn bring_to_top(&self) {}
}
