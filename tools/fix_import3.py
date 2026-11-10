import pathlib
p = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
s = p.read_text(encoding="utf-8")
s = s.replace(
    "    DestroyWindow, EnumChildWindows, EnumWindows, GWLP_WNDPROC, GetClientRect, GetCursorPos,",
    "    DestroyWindow, EnumChildWindows, EnumWindows, GWLP_WNDPROC, GetClassNameW, GetClientRect,\n    GetCursorPos,",
    1,
)
p.write_text(s, encoding="utf-8")
print("import fixed")
