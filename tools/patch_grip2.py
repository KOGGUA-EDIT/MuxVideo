import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\stage.rs")
s = p.read_text(encoding="utf-8")

def sub(old, new):
    global s
    assert old in s, old[:70]
    s = s.replace(old, new)

# Hide the grip wherever the planes are hidden.
sub("""        if w < 8 || h < 8 || !any_video {
            self.plane_a.place(None);
            self.plane_b.place(None);
            return;
        }""", """        if w < 8 || h < 8 || !any_video {
            self.plane_a.place(None);
            self.plane_b.place(None);
            self.grip.place(None);
            return;
        }""")

# Track where the grip must go while computing the wipe split.
sub("""        let mut a_rect: Option<PxRect> = None;
        let mut b_rect: Option<PxRect> = None;
        let mut a_clip: Option<PxRect> = None;
        let mut b_clip: Option<PxRect> = None;""", """        let mut a_rect: Option<PxRect> = None;
        let mut b_rect: Option<PxRect> = None;
        let mut a_clip: Option<PxRect> = None;
        let mut b_clip: Option<PxRect> = None;
        let mut grip_rect: Option<PxRect> = None;""")

sub("""                        match axis {
                            SplitAxis::Horizontal => {
                                a_clip = Some((0, 0, before, h));
                                b_clip = Some((after, 0, (w - after).max(0), h));
                            }
                            SplitAxis::Vertical => {
                                a_clip = Some((0, 0, w, before));
                                b_clip = Some((0, after, w, (h - after).max(0)));
                            }
                        }""", """                        match axis {
                            SplitAxis::Horizontal => {
                                a_clip = Some((0, 0, before, h));
                                b_clip = Some((after, 0, (w - after).max(0), h));
                                // 18 x 56 pill centred on the seam.
                                let cx = split - 9;
                                let cy = (h - 56) / 2;
                                grip_rect = Some((cx, cy.max(0), 18, 56.min(h)));
                            }
                            SplitAxis::Vertical => {
                                a_clip = Some((0, 0, w, before));
                                b_clip = Some((0, after, w, (h - after).max(0)));
                                let cx = (w - 56) / 2;
                                let cy = split - 9;
                                grip_rect = Some((cx.max(0), cy, 56.min(w), 18));
                            }
                        }""")

sub("""        self.plane_a.place(a_rect);
        self.plane_b.place(b_rect);
        self.plane_a.clip(a_clip);
        self.plane_b.clip(b_clip);""", """        self.plane_a.place(a_rect);
        self.plane_b.place(b_rect);
        self.plane_a.clip(a_clip);
        self.plane_b.clip(b_clip);
        self.grip.place(grip_rect);
        if grip_rect.is_some() {
            self.grip.bring_to_top();
        }""")

p.write_text(s, encoding="utf-8")
print("patched")
