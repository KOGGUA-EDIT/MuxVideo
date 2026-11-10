import pathlib

p = pathlib.Path(r"D:\app edit\Muxvideo\src\app\bars.rs")
t = p.read_text(encoding="utf-8")

anchor = """            if !parts.is_empty() {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(parts.join("  ·  "))
                        .size(10.5)
                        .color(p.outline),
                );
            }
"""
addition = anchor + """
            // Hover hints for the controls next to the stage: a floating
            // tooltip would be drawn under the native video planes, so the
            // hint is painted here instead, inside the transport card.
            if let Some(hint) = app.hover_hint {
                ui.add_space(10.0);
                ui.label(egui::RichText::new(hint).size(11.0).color(p.primary));
            }
"""
assert anchor in t
t = t.replace(anchor, addition, 1)
p.write_text(t, encoding="utf-8")
print("hover hint drawn in transport")
