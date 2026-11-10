import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\stage.rs")
s = p.read_text(encoding="utf-8")

anchor = """    /// Client size of a plane in physical pixels.
    pub fn plane_size(&self, plane: Plane) -> Option<(i32, i32)> {"""

addition = '''    /// Hit-tests a screen point against the visible planes.
    ///
    /// Returns the plane plus the point in that plane's client coordinates.
    /// Used by the cursor-polling fallback, which does not depend on the
    /// renderer windows forwarding their mouse messages to us.
    pub fn plane_at_screen(&self, sx: i32, sy: i32) -> Option<(Plane, i32, i32)> {
        for plane in [Plane::A, Plane::B] {
            let surface = match plane {
                Plane::A => &self.plane_a,
                Plane::B => &self.plane_b,
            };
            if !surface.visible || surface.rect.2 <= 0 || surface.rect.3 <= 0 {
                continue;
            }
            let hwnd = surface.hwnd;
            if hwnd == 0 {
                continue;
            }
            let Some((x, y, w, h)) = crate::video::childwin::window_rect(hwnd) else {
                continue;
            };
            if sx < x || sy < y || sx >= x + w || sy >= y + h {
                continue;
            }
            if let Some((cx, cy)) = crate::video::childwin::screen_to_client(hwnd, sx, sy) {
                return Some((plane, cx, cy));
            }
        }
        None
    }

    /// Handle of a plane window, if it exists.
    pub fn plane_hwnd(&self, plane: Plane) -> i64 {
        match plane {
            Plane::A => self.plane_a.hwnd,
            Plane::B => self.plane_b.hwnd,
        }
    }

    /// Client size of a plane in physical pixels.
    pub fn plane_size(&self, plane: Plane) -> Option<(i32, i32)> {'''

assert anchor in s
s = s.replace(anchor, addition, 1)
p.write_text(s, encoding="utf-8")
print("ok")
