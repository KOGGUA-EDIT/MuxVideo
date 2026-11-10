import pathlib
p = pathlib.Path(r"D:\app edit\Muxvideo\src\video\childwin.rs")
s = p.read_text(encoding="utf-8")
s = s.replace("GetAsyncKeyState(VK_LBUTTON) } as u16;", "GetAsyncKeyState(i32::from(VK_LBUTTON)) } as u16;")
p.write_text(s, encoding="utf-8")
print("fixed VK cast")
