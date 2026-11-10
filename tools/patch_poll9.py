import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
s = p.read_text(encoding="utf-8")

a = """            state.synthetic = true;
            press(app, plane, cx, cy, 0);"""
b = """            state.synthetic = true;
            crate::diag!("[poll] synthetic press ({cx},{cy})");
            press(app, plane, cx, cy, 0);"""
if a in s:
    s = s.replace(a, b, 1)

a2 = """    if !state.synthetic && state.moves_while_down == 0 && now >= state.deadline + POLL_ADOPT {
        state.synthetic = true;
    }"""
b2 = """    if !state.synthetic && state.moves_while_down == 0 && now >= state.deadline + POLL_ADOPT {
        state.synthetic = true;
        crate::diag!("[poll] adopt stalled gesture");
    }"""
if a2 in s:
    s = s.replace(a2, b2, 1)

p.write_text(s, encoding="utf-8")
print("ok")
