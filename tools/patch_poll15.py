import pathlib
q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")
t = t.replace("""            synthetic: false,
            moves_while_down: 0,
        })""", """            synthetic: false,
            moves_while_down: 0,
            hook_press: false,
        })""", 1)
q.write_text(t, encoding="utf-8")
print("ok")
