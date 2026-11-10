import pathlib
import re

root = pathlib.Path(r"D:\app edit\Muxvideo\src")
stage = root / "app" / "stage.rs"
s = stage.read_text(encoding="utf-8")

# 1. ensure_native failure logging
s = s.replace(
    """        if self.container.is_none() {
            let Some(parent) = find_window_by_title(WINDOW_TITLE) else {
                return false;
            };""",
    """        if self.container.is_none() {
            let Some(parent) = find_window_by_title(WINDOW_TITLE) else {
                crate::diag!("[stage] ensure_native: no toplevel window yet");
                return false;
            };""",
    1,
)

s = s.replace(
    """            let stage = self.stage_hwnd();
            if stage == 0 {
                return false;
            }""",
    """            let stage = self.stage_hwnd();
            if stage == 0 {
                crate::diag!("[stage] ensure_native: container has no hwnd");
                return false;
            }""",
    1,
)

# 2. spawn_player logging
s = s.replace(
    """        let player = lib.new_player(hwnd, opts)?;""",
    """        crate::diag!("[stage] spawn player plane={plane:?} wid={hwnd}");
        let player = lib.new_player(hwnd, opts)?;""",
    1,
)

# 3. refresh_children logging
s = s.replace(
    """        for child in crate::video::child_windows(self.hwnd) {
            if !self.children.contains(&child) {
                crate::video::childwin::install_input_hook(child);
                self.children.push(child);
            }
        }""",
    """        for child in crate::video::child_windows(self.hwnd) {
            if !self.children.contains(&child) {
                crate::video::childwin::install_input_hook(child);
                self.children.push(child);
                crate::diag!("[hooks] plane hwnd={} hooked child={child}", self.hwnd);
            }
        }""",
    1,
)

stage.write_text(s, encoding="utf-8")

mod = root / "app" / "mod.rs"
m = mod.read_text(encoding="utf-8")
if "[boot]" not in m:
    m = m.replace(
        """    app.palette = Palette::from_seed(app.settings.seed_color(), app.settings.scheme());""",
        """    crate::diag!(
        "[boot] version={} exe={:?}",
        env!("CARGO_PKG_VERSION"),
        std::env::current_exe().ok()
    );
    app.palette = Palette::from_seed(app.settings.seed_color(), app.settings.scheme());""",
        1,
    )
mod.write_text(m, encoding="utf-8")
print("patched")
