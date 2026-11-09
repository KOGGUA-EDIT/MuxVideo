import pathlib

bars = pathlib.Path(r"D:\app edit\Muxvideo\src\app\bars.rs")
s = bars.read_text(encoding="utf-8")
start = s.index("        // The native planes leave a 12 px hole at the seam;")
end = s.index("        // Corner caption sits inside the stage but outside the video chrome.")
assert start < end
s = s[:start] + s[end:]
bars.write_text(s, encoding="utf-8")

stage = pathlib.Path(r"D:\app edit\Muxvideo\src\app\stage.rs")
t = stage.read_text(encoding="utf-8")
t = t.replace("                        const SEAM: i32 = 12;", "                        const SEAM: i32 = 10;")
stage.write_text(t, encoding="utf-8")
print("patched")
