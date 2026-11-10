import pathlib, re

p = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
t = p.read_text(encoding="utf-8")

start = t.index("/// Move and resize our own top-level window")
fn_src = t[start:].rstrip() + "\n"
t = t[:start].rstrip() + "\n"

# put it before the test module
marker = t.index("#[cfg(test)]")
t = t[:marker] + fn_src + "\n" + t[marker:]
p.write_text(t, encoding="utf-8")
print("set_window_rect moved before tests")
