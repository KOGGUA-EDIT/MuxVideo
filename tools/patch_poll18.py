import pathlib

q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")
old = '    crate::diag!("[poll] synthetic tap ({cx},{cy})");'
new = '''    {
        let s = lock(poll_state());
        crate::diag!(
            "[poll] synthetic tap ({cx},{cy}) hook_press={} synthetic={} button_down={}",
            s.hook_press,
            s.synthetic,
            s.button_down
        );
    }'''
assert old in t
t = t.replace(old, new, 1)
q.write_text(t, encoding="utf-8")
print("instrumented tap")
