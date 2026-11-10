import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\input.rs")
t = p.read_text(encoding="utf-8")

# constants
t = t.replace(
    "/// Movement (physical pixels) that turns a press into a drag.\nconst DRAG_SLOP: i32 = 5;",
    "/// Movement (physical pixels) that turns a press into a drag.\nconst DRAG_SLOP: i32 = 5;\n/// Our own double-click window. The system one is ~500 ms, which turned quick\n/// play/pause toggles into fullscreen toggles.\nconst DOUBLE_CLICK_MS: u64 = 250;\n/// Two clicks further apart than this (physical pixels) are not a double click.\nconst DOUBLE_CLICK_PX: i32 = 12;",
    1,
)

# ignore the OS double-click message: we time clicks ourselves
t = t.replace(
    """            MouseEvent::Up { .. } => release(app),
            MouseEvent::DoubleClick => {
                app.gesture = None;
                app.suppress_click = Some(Instant::now());
                toggle_fullscreen(app, ctx);
            }""",
    """            MouseEvent::Up { .. } => release(app, ctx),
            // The OS double-click uses the system interval (0.5 s by default),
            // which is long enough that quick play/pause toggles landed here.
            // Clicks are paired by `release` instead, with a tighter window.
            MouseEvent::DoubleClick => {}""",
    1,
)

# release(): own double-click detection
old_release = """fn release(app: &mut MuxApp) {
    let Some(drag) = app.gesture.take() else {
        return;
    };
    let quick = drag.began.elapsed() < Duration::from_millis(CLICK_MAX_MS);
    let suppressed = app
        .suppress_click
        .is_some_and(|at| at.elapsed() < Duration::from_millis(CLICK_MAX_MS));
    if drag.mode == Mode::Scrub {
        // Scrubbing leaves the player parked on the frame under the cursor.
        app.scrub = None;
        if !drag.moved && quick && !suppressed {
            app.suppress_click = None;
            app.toggle_play();
        }
    }
}"""
new_release = """fn release(app: &mut MuxApp, ctx: &egui::Context) {
    let Some(drag) = app.gesture.take() else {
        return;
    };
    let quick = drag.began.elapsed() < Duration::from_millis(CLICK_MAX_MS);
    if drag.mode == Mode::Scrub {
        // Scrubbing leaves the player parked on the frame under the cursor.
        app.scrub = None;
        if drag.moved || !quick {
            return;
        }
        let now = Instant::now();
        let is_double = app.last_click.is_some_and(|(at, x, y)| {
            now.duration_since(at) < Duration::from_millis(DOUBLE_CLICK_MS)
                && (x - drag.start.0).abs() <= DOUBLE_CLICK_PX
                && (y - drag.start.1).abs() <= DOUBLE_CLICK_PX
        });
        if is_double {
            // Undo the toggle from the first click so a double click only
            // changes the view, then go immersive.
            app.last_click = None;
            app.toggle_play();
            toggle_fullscreen(app, ctx);
        } else {
            app.last_click = Some((now, drag.start.0, drag.start.1));
            app.toggle_play();
        }
    }
}"""
assert old_release in t
t = t.replace(old_release, new_release, 1)

# any remaining `release(app)` call sites
t = t.replace("release(app);", "release(app, ctx);")
p.write_text(t, encoding="utf-8")
print("double click retimed")
