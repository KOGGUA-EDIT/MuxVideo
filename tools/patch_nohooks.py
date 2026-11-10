import pathlib

# 1. childwin: honour MUXVIDEO_NO_HOOKS
p = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
s = p.read_text(encoding="utf-8")
if "MUXVIDEO_NO_HOOKS" not in s:
    anchor = "pub fn install_input_hook(hwnd: i64) {"
    assert anchor in s
    s = s.replace(
        anchor,
        """/// Escape hatch: `MUXVIDEO_NO_HOOKS=1` disables the message hooks so the
/// pointer-polling fallback is the only input path (useful for diagnosis).
pub fn hooks_disabled() -> bool {
    std::env::var_os("MUXVIDEO_NO_HOOKS").is_some()
}

pub fn install_input_hook(hwnd: i64) {
    if hooks_disabled() {
        return;
    }""",
        1,
    )
    p.write_text(s, encoding="utf-8")
    print("hook switch added")
else:
    print("already present")
