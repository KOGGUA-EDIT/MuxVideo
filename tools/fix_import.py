import pathlib
p = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
s = p.read_text(encoding="utf-8")
before = s
s = s.replace("    GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, WindowFromPoint,\n", "    GetForegroundWindow, GetWindowRect,\n", 1)
if s == before:
    print("pattern not found; showing import block")
    i = s.find("GetForegroundWindow")
    print(s[max(0, i - 400): i + 200])
else:
    p.write_text(s, encoding="utf-8")
    print("deduped")
