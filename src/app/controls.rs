//! App-level Material You controls.
//!
//! The theme track owns the generic `ui::widgets` kit; these are the
//! composition helpers the shell needs on top of it — icon buttons, mode chips,
//! MD3 sliders with a draggable thumb and cards with hover elevation.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Ui, Vec2, pos2, vec2,
};

use crate::ui::Palette;

/// Icon button: 40×40 round, state layer on hover, tonal when toggled on.
pub fn icon_button(
    ui: &mut Ui,
    palette: &Palette,
    size: f32,
    active: bool,
    glyph: impl FnOnce(&mut Ui, f32, Color32) -> Response,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.12);
    // M3 state layer: the on-surface colour at 8 % over the transparent button.
    let fill = if active {
        palette.secondary_container
    } else if hover > 0.01 {
        palette.on_surface.gamma_multiply(hover * 0.08)
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, CornerRadius::same((size * 0.5) as u8), fill);
    }

    // Paint the glyph centred in the button.
    let icon_size = (size * 0.5).min(20.0);
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(icon_size));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(icon_rect).layout(
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
    ));
    let fg = if active {
        palette.on_secondary_container
    } else if response.hovered() {
        palette.on_surface
    } else {
        palette.on_surface_variant
    };
    glyph(&mut child, icon_size, fg);
    response
}

/// MD3 filled / tonal pill button with an optional leading icon painter.
pub fn pill_button(
    ui: &mut Ui,
    palette: &Palette,
    label: &str,
    filled: bool,
    icon: Option<fn(&mut Ui, f32, Color32) -> Response>,
) -> Response {
    let text_color = if filled {
        palette.on_primary
    } else {
        palette.on_secondary_container
    };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(14.0), text_color);
    let icon_w = if icon.is_some() { 18.0 } else { 0.0 };
    // M3 button: 40 dp tall, full pill, 24 px leading/trailing padding.
    let pad_x = 24.0;
    let size = vec2(galley.size().x + pad_x * 2.0 + icon_w, 40.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.12);
    let base = if filled {
        palette.primary
    } else {
        palette.secondary_container
    };
    // State layer (8 % of the on-colour) instead of lightening the container.
    let state = if filled {
        palette.on_primary
    } else {
        palette.on_secondary_container
    };
    let fill = if hover > 0.01 {
        mix(base, state, hover * 0.08)
    } else {
        base
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same((size.y * 0.5) as u8), fill);

    let mut x = rect.min.x + pad_x;
    let cy = rect.center().y;
    if let Some(icon_fn) = icon {
        let icon_rect = Rect::from_center_size(pos2(x + 9.0, cy), Vec2::splat(18.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(icon_rect).layout(
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        ));
        icon_fn(&mut child, 18.0, text_color);
        x += 18.0 + 8.0;
    }
    ui.painter()
        .galley(pos2(x, cy - galley.size().y * 0.5), galley, text_color);
    response
}

/// Text-only button: no fill, primary label, M3 state layer on hover.
pub fn text_button(ui: &mut Ui, palette: &Palette, label: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(14.0),
        palette.primary,
    );
    let size = vec2(galley.size().x + 24.0, 40.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.12);
    if hover > 0.01 {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(20),
            palette.primary.gamma_multiply(0.08 * hover),
        );
    }
    ui.painter().galley(
        pos2(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        palette.primary,
    );
    response
}

/// Segmented mode chip: tonal when selected, state layer when hovered.
pub fn chip(ui: &mut Ui, palette: &Palette, label: &str, selected: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(14.0),
        palette.on_surface,
    );
    // M3 chip: 32 dp tall, 8 dp corner, no outline (tonal fill does the work).
    let size = vec2(galley.size().x + 28.0, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.1);
    let (fill, text) = if selected {
        (palette.secondary_container, palette.on_secondary_container)
    } else if hover > 0.01 {
        (
            mix(palette.surface_container_high, palette.on_surface, hover * 0.08),
            palette.on_surface,
        )
    } else {
        (palette.surface_container_high, palette.on_surface_variant)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, text);
    response
}

/// State for the sliding selection pill of a [`chip_row`].
#[derive(Clone, Copy)]
struct PillSlide {
    from: f32,
    goal: f32,
    t0: f64,
}

/// Ease-out cubic: moves quickly at first, then settles. Matches the rail pill.
fn slide_at(state: &PillSlide, now: f64) -> f32 {
    const DURATION: f64 = 0.30;
    let t = ((now - state.t0) / DURATION).clamp(0.0, 1.0) as f32;
    let eased = 1.0 - (1.0 - t).powi(3);
    state.from + (state.goal - state.from) * eased
}

/// A row of chips whose selected background slides between items.
///
/// Returns the index the user clicked. Chips keep their own rounded shape; only
/// the highlight travels, so a mode switch reads as motion instead of a blink.
pub fn chip_row(
    ui: &mut Ui,
    palette: &Palette,
    id_salt: impl std::hash::Hash + Copy,
    labels: &[&str],
    selected: usize,
) -> Option<usize> {
    const HEIGHT: f32 = 40.0;
    const GAP: f32 = 0.0;
    const PAD_X: f32 = 16.0;
    if labels.is_empty() {
        return None;
    }
    let selected = selected.min(labels.len() - 1);
    let galleys: Vec<_> = labels
        .iter()
        .map(|label| {
            ui.painter().layout_no_wrap(
                (*label).to_owned(),
                FontId::proportional(14.0),
                palette.on_surface,
            )
        })
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + PAD_X * 2.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + GAP * labels.len().saturating_sub(1) as f32;
    let (rect, _) = ui.allocate_exact_size(vec2(total, HEIGHT), Sense::hover());
    let id = ui.id().with(id_salt);

    let now = ui.input(|i| i.time);
    let target = selected as f32;
    let mut state = ui
        .data_mut(|d| d.get_temp::<PillSlide>(id))
        .unwrap_or(PillSlide {
            from: target,
            goal: target,
            t0: now,
        });
    if (state.goal - target).abs() > 0.001 {
        let current = slide_at(&state, now);
        state = PillSlide {
            from: current,
            goal: target,
            t0: now,
        };
    }
    let pos = slide_at(&state, now);
    ui.data_mut(|d| d.insert_temp(id, state));
    if (pos - target).abs() > 0.004 {
        ui.ctx().request_repaint();
    }

    let mut x = rect.min.x;
    let mut item_rects = Vec::with_capacity(labels.len());
    for width in &widths {
        item_rects.push(Rect::from_min_size(
            pos2(x, rect.min.y),
            vec2(*width, HEIGHT),
        ));
        x += *width + GAP;
    }

    // Group track (single rounded container), then the moving pill, then labels.
    let pill_radius = CornerRadius::same((HEIGHT * 0.5) as u8);
    ui.painter()
        .rect_filled(rect, pill_radius, palette.surface_container);
    let first = pos.floor().clamp(0.0, labels.len() as f32 - 1.0);
    let second = (first + 1.0).min(labels.len() as f32 - 1.0);
    let blend = pos - first;
    let a = item_rects[first as usize];
    let b = item_rects[second as usize];
    let highlight = Rect::from_min_max(
        pos2(a.min.x + (b.min.x - a.min.x) * blend, a.min.y + 4.0),
        pos2(a.max.x + (b.max.x - a.max.x) * blend, a.max.y - 4.0),
    );
    ui.painter()
        .rect_filled(highlight, pill_radius, palette.secondary_container);

    let mut clicked = None;
    for (index, (item, galley)) in item_rects.iter().zip(galleys).enumerate() {
        let response = ui.interact(*item, id.with(index), Sense::click());
        if response.clicked() {
            clicked = Some(index);
        }
        let near = 1.0 - (pos - index as f32).abs().clamp(0.0, 1.0);
        let color = mix(
            palette.on_surface_variant,
            palette.on_secondary_container,
            near,
        );
        ui.painter()
            .galley(item.center() - galley.size() * 0.5, galley, color);
    }
    clicked
}

/// Material 3 Expressive slider: 8 px rounded track, pill-shaped thumb.
///
/// `value` is a fraction of the full width; the caller maps it to its own range.
pub fn slider(
    ui: &mut Ui,
    palette: &Palette,
    width: f32,
    value: &mut f32,
    enabled: bool,
) -> Response {
    const HEIGHT: f32 = 30.0;
    const TRACK_H: f32 = 8.0;
    const GAP: f32 = 6.0;
    const THUMB_W: f32 = 4.0;
    const THUMB_H: f32 = 18.0;
    let (rect, mut response) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), vec2(rect.width() - 12.0, TRACK_H));

    if enabled
        && (response.dragged() || response.clicked())
        && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
    {
        let t = ((pos.x - track.min.x) / track.width()).clamp(0.0, 1.0);
        if (t - *value).abs() > 0.0005 {
            *value = t;
            response.mark_changed();
        }
    }
    let frac = value.clamp(0.0, 1.0);
    let thumb_x = track.min.x + track.width() * frac;
    let inactive_color = palette.surface_container_highest;
    let active_color = if enabled {
        palette.primary
    } else {
        palette.outline_variant
    };

    ui.painter()
        .rect_filled(track, CornerRadius::same(TRACK_H as u8 / 2), inactive_color);
    let active = Rect::from_min_max(
        track.min,
        pos2((thumb_x - GAP).max(track.min.x), track.max.y),
    );
    if active.width() > 1.0 {
        ui.painter()
            .rect_filled(active, CornerRadius::same(TRACK_H as u8 / 2), active_color);
    }
    let tail = Rect::from_min_max(
        pos2((thumb_x + GAP).min(track.max.x), track.min.y),
        track.max,
    );
    if tail.width() > 1.0 {
        ui.painter()
            .rect_filled(tail, CornerRadius::same(TRACK_H as u8 / 2), inactive_color);
    }

    // Pill thumb: a narrow rounded rectangle, wider while hovered or dragged.
    let hot = response.hovered() || response.dragged();
    let thumb_size = if hot {
        vec2(THUMB_W + 2.0, THUMB_H + 4.0)
    } else {
        vec2(THUMB_W, THUMB_H)
    };
    let thumb = Rect::from_center_size(pos2(thumb_x, rect.center().y), thumb_size);
    if hot {
        ui.painter().rect_filled(
            Rect::from_center_size(thumb.center(), thumb_size + vec2(10.0, 8.0)),
            CornerRadius::same(10),
            palette.primary.gamma_multiply(0.18),
        );
    }
    ui.painter().rect_filled(
        thumb,
        CornerRadius::same(thumb_size.x.min(thumb_size.y) as u8 / 2),
        if enabled {
            palette.primary
        } else {
            palette.outline
        },
    );
    response
}

/// Vertical slider used for the volume column.
#[allow(dead_code)] // kept as a kit primitive for the upcoming mixer panel
pub fn v_slider(ui: &mut Ui, palette: &Palette, height: f32, value: &mut f32) -> Response {
    let width = 20.0;
    let (rect, mut response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), vec2(4.0, rect.height() - 10.0));
    if (response.dragged() || response.clicked())
        && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
    {
        let t = ((track.max.y - pos.y) / track.height()).clamp(0.0, 1.0);
        if (t - *value).abs() > 0.0005 {
            *value = t;
            response.mark_changed();
        }
    }
    let frac = value.clamp(0.0, 1.0);
    let active = Rect::from_min_max(
        pos2(track.min.x, track.max.y - track.height() * frac),
        track.max,
    );
    ui.painter().rect_filled(
        track,
        CornerRadius::same(2),
        palette.surface_container_highest,
    );
    ui.painter()
        .rect_filled(active, CornerRadius::same(2), palette.primary);
    let thumb_y = track.max.y - track.height() * frac;
    ui.painter()
        .circle_filled(pos2(rect.center().x, thumb_y), 7.0, palette.primary);
    response
}

/// Rounded card surface, returns the response of its frame for hover effects.
#[allow(dead_code)] // kit primitive
pub fn card(_ui: &mut Ui, fill: Color32, radius: u8) -> egui::Frame {
    egui::Frame::NONE
        .fill(fill)
        .corner_radius(CornerRadius::same(radius))
        .inner_margin(egui::Margin::same(16))
}

/// Small caption text in the M3 label-small style.
#[allow(dead_code)] // kit primitive
pub fn label_small(ui: &mut Ui, palette: &Palette, text: &str) -> Response {
    ui.label(
        egui::RichText::new(text)
            .size(11.0)
            .color(palette.on_surface_variant),
    )
}

/// Title text (M3 title-large-ish) honouring CJK metrics.
#[allow(dead_code)] // kit primitive
pub fn title_large(ui: &mut Ui, palette: &Palette, text: &str) -> Response {
    ui.label(
        egui::RichText::new(text)
            .size(22.0)
            .color(palette.on_surface),
    )
}

/// Rounded tonal badge, e.g. "A" / "B" plane tags.
pub fn badge(ui: &mut Ui, _palette: &Palette, text: &str, bg: Color32, fg: Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(11.0), fg);
    let size = vec2(galley.size().x + 12.0, 20.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, fg);
}

/// Material 3 switch (52 × 32).
pub fn switch(ui: &mut Ui, palette: &Palette, on: &mut bool) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(52.0, 32.0), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("knob"), *on, 0.16);
    // M3 switch: 52 x 32 track, handle 20 dp at rest growing to 24 dp, and the
    // handle always keeps 4 dp of track visible on the side it travels towards.
    let track_fill = if *on {
        palette.primary
    } else {
        palette.surface_container_highest
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(16), track_fill);
    let knob_r = 10.0 + 2.0 * t;
    let inset = 4.0 + knob_r;
    let cx = rect.min.x + inset + (rect.width() - inset * 2.0) * t;
    let knob_color = if *on {
        palette.on_primary
    } else {
        palette.outline
    };
    ui.painter()
        .circle_filled(pos2(cx, rect.center().y), knob_r, knob_color);
    response
}

/// Rounded surface, used for the settings grouped cards (M3 card: 12 dp).
pub fn surface_card(ui: &mut Ui, palette: &Palette, add: impl FnOnce(&mut Ui)) -> egui::Response {
    egui::Frame::NONE
        .fill(palette.surface_container_low)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        })
        .response
}

/// Draws text inside an already-allocated rect (used for the transport clock).
#[allow(dead_code)] // kit primitive
pub fn text_at(ui: &Ui, rect: Rect, text: &str, size: f32, color: Color32, align: Align2) {
    ui.painter().text(
        align.pos_in_rect(&rect),
        align,
        text,
        FontId::proportional(size),
        color,
    );
}

/// Linear mix in gamma space; `t` = 0 keeps `base`, 1 keeps `over`.
pub fn mix(base: Color32, over: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| ((a as f32 * (1.0 - t)) + (b as f32 * t)).round() as u8;
    Color32::from_rgb(
        lerp(base.r(), over.r()),
        lerp(base.g(), over.g()),
        lerp(base.b(), over.b()),
    )
}

/// A tonal "container" colour derived from an accent: `accent` washed into the
/// surface, matching the MD3 container roles closely enough for accents the
/// palette does not expose directly.
pub fn container(accent: Color32, surface: Color32) -> Color32 {
    mix(surface, accent, 0.26)
}
