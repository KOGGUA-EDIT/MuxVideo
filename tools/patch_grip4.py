import pathlib

stage = pathlib.Path(r"D:\app edit\Muxvideo\src\app\stage.rs")
s = stage.read_text(encoding="utf-8")

def sub(old, new):
    global s
    assert old in s, old[:70]
    s = s.replace(old, new)

# Remember the last grip style so repaints only invalidate on change.
sub("""    last_sync: Instant,
    last_hook_scan: Instant,""", """    last_sync: Instant,
    last_hook_scan: Instant,
    /// Last colours pushed to the grip window, so a frame that changes nothing
    /// does not repaint it.
    grip_style: Option<(u32, u32, bool)>,""")

sub("""            last_sync: Instant::now(),
            last_hook_scan: Instant::now() - HOOK_SCAN_INTERVAL,""", """            last_sync: Instant::now(),
            last_hook_scan: Instant::now() - HOOK_SCAN_INTERVAL,
            grip_style: None,""")

sub("""    // ── events / sync ─────────────────────────────────────────────────────""",
"""    /// Handle of the wipe grip window (`0` before it exists).
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

    // ── events / sync ─────────────────────────────────────────────────────""")

stage.write_text(s, encoding="utf-8")
print("stage patched")
