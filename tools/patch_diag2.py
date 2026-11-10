import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\stage.rs")
s = p.read_text(encoding="utf-8")

old = """        self.grip.place(grip_rect);
        crate::diag!(
            "[stage] a={:?} b={:?} clip_a={:?} clip_b={:?} grip={grip_rect:?}",
            a_rect,
            b_rect,
            a_clip,
            b_clip
        );"""
new = """        self.grip.place(grip_rect);
        let signature = (
            self.plane_a.rect,
            self.plane_b.rect,
            self.plane_a.clip,
            self.plane_b.clip,
            self.grip.rect,
        );
        if self.logged_layout != Some(signature) {
            self.logged_layout = Some(signature);
            crate::diag!(
                "[stage] a={:?} b={:?} clip_a={:?} clip_b={:?} grip={grip_rect:?}",
                a_rect,
                b_rect,
                a_clip,
                b_clip
            );
        }"""
assert old in s
s = s.replace(old, new, 1)

# Media field
old_field = "    pub meta_a: Option<MediaInfo>,"
assert old_field in s
s = s.replace(
    old_field,
    "    /// Last layout signature that was written to the diagnostics log.\n    logged_layout: Option<(\n        PxRect,\n        PxRect,\n        Option<PxRect>,\n        Option<PxRect>,\n        PxRect,\n    )>,\n    pub meta_a: Option<MediaInfo>,",
    1,
)
p.write_text(s, encoding="utf-8")
print("patched field")
