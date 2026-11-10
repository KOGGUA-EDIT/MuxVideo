import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\mod.rs")
t = p.read_text(encoding="utf-8")
t = t.replace("    pub(super) suppress_click: Option<Instant>,\n", "", 1)
t = t.replace("            suppress_click: None,\n", "", 1)
p.write_text(t, encoding="utf-8")
print("suppress_click removed")
