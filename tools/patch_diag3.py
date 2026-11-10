import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\stage.rs")
s = p.read_text(encoding="utf-8")

marker = "    /// Rounded-pill region radius; `> 0` for the wipe grip, `0` for planes.\n    round: i32,\n}"
if "clip_rect: Option<PxRect>" not in s.split("struct Surface")[1][:800]:
    assert marker in s
    s = s.replace(
        marker,
        "    /// Rounded-pill region radius; `> 0` for the wipe grip, `0` for planes.\n"
        "    round: i32,\n"
        "    /// Last clip rectangle, kept so the diagnostics log can show it.\n"
        "    clip_rect: Option<PxRect>,\n}",
        1,
    )

old_new = """            round: 0,
        }"""
if old_new in s:
    s = s.replace(old_new, """            round: 0,
            clip_rect: None,
        }""", 1)

# logged_layout default in Media::default()
old_def = "    pub meta_a: Option<MediaInfo>,"
if "logged_layout" in s and "logged_layout: None" not in s:
    s = s.replace(old_def, old_def, 1)

p.write_text(s, encoding="utf-8")
print("ok")
