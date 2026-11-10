import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
s = p.read_text(encoding="utf-8")

if "[poll]" not in s:
    s = s.replace(
        """    if !app.media.app_is_foreground() {""",
        """    if !app.media.app_is_foreground() {
        crate::diag!("[poll] skip: not foreground");""",
        1,
    )
    s = s.replace(
        """            let cx = x - r.0;""",
        """            crate::diag!("[poll] synthetic press at ({cx},{cy})");
            let cx = x - r.0;""",
        1,
    )
    s = s.replace(
        """            state.synthetic = true;
            drag_to(app, cx, cy);""",
        """            state.synthetic = true;
            crate::diag!("[poll] adopt + follow at ({cx},{cy})");
            drag_to(app, cx, cy);""",
        1,
    )
p.write_text(s, encoding="utf-8")
print("ok")
