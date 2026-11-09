import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\stage.rs")
s = p.read_text(encoding="utf-8")

def sub(old, new):
    global s
    assert old in s, old[:60]
    s = s.replace(old, new)

sub("""            rect: (-1, -1, 0, 0),
            visible: false,
            children: Vec::new(),
        }""", """            rect: (-1, -1, 0, 0),
            visible: false,
            children: Vec::new(),
            round: 0,
        }""")

sub("""        if self.rect != rect {
            win.set_rect(rect.0, rect.1, rect.2, rect.3);
            self.rect = rect;
        }""", """        if self.rect != rect {
            win.set_rect(rect.0, rect.1, rect.2, rect.3);
            if self.round > 0 {
                win.round_corners(self.round);
            }
            self.rect = rect;
        }""")

sub("""    plane_a: Surface,
    plane_b: Surface,
    planes_ready: bool,""", """    plane_a: Surface,
    plane_b: Surface,
    /// Material drag handle for the wipe divider. It lives in the same native
    /// layer as the planes so it stays visible above the video.
    grip: Surface,
    planes_ready: bool,""")

sub("""            plane_a: Surface::new(),
            plane_b: Surface::new(),
            planes_ready: false,""", """            plane_a: Surface::new(),
            plane_b: Surface::new(),
            grip: Surface::new(),
            planes_ready: false,""")

sub("""            let ok = [Plane::A, Plane::B]
                .into_iter()
                .all(|plane| self.surface_mut(plane).ensure(stage, plane).is_ok());
            self.planes_ready = ok;""", """            let ok = [Plane::A, Plane::B]
                .into_iter()
                .all(|plane| self.surface_mut(plane).ensure(stage, plane).is_ok());
            self.grip.round = 9;
            let grip_ok = ChildWindow::create_grip(stage, 0x4D56_0005, "muxvideo-grip")
                .map(|win| {
                    self.grip.hwnd = win.hwnd();
                    self.grip.window = Some(win);
                })
                .is_ok();
            self.planes_ready = ok && grip_ok;""")

p.write_text(s, encoding="utf-8")
print("patched")
