import pathlib

q = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = q.read_text(encoding="utf-8")

# 1. track whether the hook reported this press cycle
old = """struct PollState {"""
if "hook_press" not in t:
    t = t.replace(old, """struct PollState {
    /// Whether the message hook reported the press of the current cycle.
    hook_press: bool,""", 1)

    t = t.replace(
        """fn note_hook_event(event: &MouseEvent) {""",
        """fn note_hook_event(event: &MouseEvent) {
    if matches!(event, MouseEvent::Down { .. }) {
        lock(poll_state()).hook_press = true;
    }""",
        1,
    )

    t = t.replace(
        """        state.moves_while_down = 0;
        state.synthetic = false;""",
        """        state.moves_while_down = 0;
        state.synthetic = false;
        state.hook_press = false;""",
        1,
    )

    t = t.replace(
        """        } else if !state.synthetic {""",
        """        } else if !state.synthetic && !state.hook_press {""",
        1,
    )
    q.write_text(t, encoding="utf-8")
    print("hook_press tracking added")
else:
    print("already present")
