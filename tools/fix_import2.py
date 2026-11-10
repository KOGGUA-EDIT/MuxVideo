import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
s = p.read_text(encoding="utf-8")
if "WindowFromPoint," not in s.split("use windows_sys")[1][:3000]:
    s = s.replace(
        "    GetForegroundWindow, GetWindowRect,\n",
        "    GetForegroundWindow, GetWindowRect, WindowFromPoint,\n",
        1,
    )
    p.write_text(s, encoding="utf-8")
    print("added WindowFromPoint")
else:
    print("already there")
