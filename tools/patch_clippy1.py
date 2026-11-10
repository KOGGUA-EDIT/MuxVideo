import pathlib

# 1. stage.rs: give the logged layout a name
p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\stage.rs")
s = p.read_text(encoding="utf-8")
if "type LayoutSig" not in s:
    s = s.replace(
        "    logged_layout: Option<(PxRect, PxRect, Option<PxRect>, Option<PxRect>, PxRect)>,",
        "    logged_layout: Option<LayoutSig>,",
        1,
    )
    anchor = "type PxRect = (i32, i32, i32, i32);"
    assert anchor in s
    s = s.replace(
        anchor,
        anchor
        + "\n\n/// Signature of the last layout that was written to the native windows,\n/// used to keep the debug log quiet unless something actually changed.\ntype LayoutSig = (PxRect, PxRect, Option<PxRect>, Option<PxRect>, PxRect);",
        1,
    )
    p.write_text(s, encoding="utf-8")
    print("named LayoutSig")

# 2. app_is_foreground no longer used by the guard: use it for the diag line
q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")
if "app_is_foreground()" not in t:
    old = '    if !owns_pointer {'
    new = '''    if !owns_pointer {
        if app.media.app_is_foreground() {
            crate::diag!("[poll] pointer not ours although focused");
        }'''
    assert old in t
    t = t.replace(old, new, 1)
    q.write_text(t, encoding="utf-8")
    print("used app_is_foreground in diag")
