//! Material 3 Expressive wavy slider ("squiggly" seek / progress indicator).
//!
//! This is the 2025 Expressive shape: an 8 px track, a vertical pill handle and
//! a sine wave that only lives on the played portion of the track. The wave
//! grows over 800 ms when playback starts, flattens while paused or dragged,
//! tapers out before the handle and scrolls at a slow, calm pace.
//!
//! Measurements follow the progress-indicator spec and AOSP's
//! `SystemUISquigglyProgress`:
//!
//! - 8 px track with 4 px end caps;
//! - 3.5 px peak amplitude, 22 px wavelength, 52 px/s scroll speed;
//! - 40 px state layer on hover / focus / drag (12 % / 16 % primary);
//! - 4 × 22 px pill handle that widens to 6 px on hover, focus and drag.
//!
//! The widget paints inside its own [`Rect`], so it can sit flush next to
//! labels without spilling the state layer onto its neighbours.

use std::time::Duration;

use eframe::egui::{self, Color32, CornerRadius, Mesh, Rect, Response, Sense, Shape, Vec2};

use crate::ui::Palette;

/// Track thickness. Material 3 linear indicators use a 4 dp stroke; 6 px keeps
/// the wave readable on a desktop-sized seek bar without looking like a hairline.
const TRACK_HEIGHT: f32 = 8.0;
/// Radius of the rounded track ends, in whole pixels.
const TRACK_RADIUS_PX: u8 = 4;
/// Gap between the active indicator and the remaining track (M3: 4 dp).
const TRACK_GAP: f32 = 6.0;
/// Radius of the stop indicator dot at the end of the track (M3: 4 dp wide).
const STOP_RADIUS: f32 = 3.0;
/// Horizontal track inset: leaves room for the handle's 40 px state layer.
const TRACK_INSET: f32 = 10.0;
/// Height reserved by the widget; fits the 40 px state layer.
const HEIGHT: f32 = 40.0;

/// Peak wave displacement from the track centre line (M3: 3.5 dp crest).
const WAVE_AMPLITUDE: f32 = 5.5;
/// Distance between wave crests (M3 determinate linear: 28 dp).
const WAVE_LENGTH: f32 = 52.0;
/// Scroll speed of the wave: one wavelength per second, as M3 does.
const PHASE_SPEED: f32 = 40.0;
/// Seconds the wave takes to morph from flat to wavy (and back).
const WAVE_ANIMATION: f32 = 0.30;
/// Horizontal distance over which the wave ramps in from the flat start.
const WAVE_RAMP_IN: f32 = 16.0;
/// Horizontal distance over which the wave tapers out before the handle.
const WAVE_TAPER: f32 = 20.0;
/// Upper bound on mesh segments, so a very wide track cannot explode.
const MAX_SEGMENTS: usize = 1024;

/// Handle body height.
const HANDLE_HEIGHT: f32 = 18.0;
/// Handle body width at rest.
const HANDLE_WIDTH: f32 = 3.0;
/// Handle body width while hovered, focused or dragged.
const HANDLE_WIDTH_ACTIVE: f32 = 5.0;
/// Handle corner radius, in whole pixels (a pill, never a circle).
const HANDLE_RADIUS_PX: u8 = 2;
/// Radius of the round hover / focus / drag state layer.
const STATE_LAYER_RADIUS: f32 = 20.0;
/// State-layer opacity over the primary track.
const STATE_LAYER_HOVER: f32 = 0.12;
/// State-layer opacity while dragging.
const STATE_LAYER_DRAG: f32 = 0.16;

/// Values closer than this are considered unchanged / empty.
const EPSILON: f32 = 0.0001;

/// The result of [`wavy_slider`].
///
/// Mirrors egui's `Response` enough to drive a player: use [`changed`] to seek
/// and [`dragging`] to distinguish a scrub from a plain click.
///
/// [`changed`]: Self::changed
/// [`dragging`]: Self::dragging
pub struct WavySliderResponse {
    /// The underlying interaction response (id, hover, focus, click, drag).
    pub response: Response,
    /// The value changed this frame (a click on the track or a drag).
    pub changed: bool,
    /// A pointer button is held on the widget, i.e. the user is scrubbing.
    pub dragging: bool,
}

/// A Material 3 Expressive wavy slider.
///
/// `value` is clamped to `0.0..=1.0` and updated in place while the user clicks
/// or drags anywhere on the track. `animated` should be the player's "playing"
/// state: while true the wave scrolls and expands to full amplitude; while
/// false (paused, ended, or dragging) it flattens back to a plain 8 px bar.
///
/// `id_salt` is mixed into the egui [`egui::Id`] of the widget and of its
/// animations, so several sliders can share a frame (pass a stable salt such as
/// `"seek"` or `"volume"`). `width` includes the 10 px track inset on either
/// side; the widget reserves 40 px of height. The wave needs roughly 80 px to
/// read as a wave, anything shorter simply looks flatter.
///
/// The widget paints only inside its allocated rect, so wrapping it in a
/// rounded container does not require extra clipping.
pub fn wavy_slider(
    ui: &mut egui::Ui,
    palette: &Palette,
    id_salt: impl std::hash::Hash + Copy,
    width: f32,
    value: &mut f32,
    animated: bool,
) -> WavySliderResponse {
    ui.push_id(id_salt, |ui| {
        wavy_slider_impl(ui, palette, width, value, animated)
    })
    .inner
}

fn wavy_slider_impl(
    ui: &mut egui::Ui,
    palette: &Palette,
    width: f32,
    value: &mut f32,
    animated: bool,
) -> WavySliderResponse {
    let width = width.max(1.0);
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::click_and_drag());
    let track = track_rect(rect);
    let enabled = ui.is_enabled();

    let mut changed = false;
    if enabled
        && (response.clicked() || response.dragged())
        && let Some(pointer) = response.interact_pointer_pos()
        && track.width() > 0.0
    {
        let next = ((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0);
        if (next - *value).abs() > EPSILON {
            *value = next;
            changed = true;
            response.mark_changed();
        }
    }
    *value = value.clamp(0.0, 1.0);
    let t = *value;

    let dragging = response.dragged() || response.is_pointer_button_down_on();
    let active_color = if enabled {
        palette.primary
    } else {
        Palette::alpha(palette.on_surface, 0.38)
    };
    let track_color = if !enabled {
        Palette::alpha(palette.on_surface, 0.12)
    } else if palette.is_dark() {
        palette.surface_variant
    } else {
        palette.surface_container_highest
    };

    // ---- animation -------------------------------------------------------
    let anim_id = response.id;
    let ctx = ui.ctx();
    // Ease the flat <-> wavy transition instead of snapping: a 0.3 s
    // smoothstep looks like the M3 Expressive morph.
    let wave_t = ctx.animate_bool_with_time(
        anim_id.with("wave-playing"),
        enabled && animated && !dragging && t > EPSILON,
        WAVE_ANIMATION,
    );
    let amplitude = WAVE_AMPLITUDE * smoothstep01(wave_t);
    let hover_t = ctx.animate_bool_with_time(
        anim_id.with("hover"),
        enabled && (response.hovered() || response.has_focus()),
        0.12,
    );
    let drag_t = ctx.animate_bool_with_time(anim_id.with("drag"), enabled && dragging, 0.10);
    let handle_t = ctx.animate_bool_with_time(
        anim_id.with("handle-width"),
        enabled && (response.hovered() || response.has_focus() || dragging),
        0.15,
    );
    // Keep repainting while the wave scrolls or while it is morphing in/out.
    let morphing = wave_t > EPSILON && wave_t < 1.0 - EPSILON;
    if enabled
        && !dragging
        && t > EPSILON
        && ui.is_rect_visible(rect)
        && (morphing || (animated && amplitude > EPSILON))
    {
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    // ---- paint -----------------------------------------------------------
    let painter = ui.painter().with_clip_rect(rect);
    let active_end = lerp(track.left(), track.right(), t);

    if t <= EPSILON {
        painter.rect_filled(track, CornerRadius::same(TRACK_RADIUS_PX), track_color);
    } else {
        // M3 Expressive leaves a small gap between the active indicator and the
        // rest of the track, and both pieces are round-capped.
        let inactive =
            Rect::from_min_max(egui::pos2(active_end + TRACK_GAP, track.top()), track.max);
        if inactive.width() > EPSILON {
            painter.rect_filled(inactive, CornerRadius::same(TRACK_RADIUS_PX), track_color);
        }
    }

    // The stop indicator sits at the very end of the track.
    painter.circle_filled(
        egui::pos2(track.right() - STOP_RADIUS, track.center().y),
        STOP_RADIUS,
        Palette::alpha(track_color, 0.85),
    );

    if t > EPSILON {
        let phase = wave_phase(ui);
        painter.add(Shape::mesh(wavy_band_mesh(
            track,
            active_end,
            amplitude,
            phase,
            active_color,
        )));
    }

    let center = egui::pos2(active_end, rect.center().y);
    if enabled {
        let opacity = (STATE_LAYER_HOVER * hover_t).max(STATE_LAYER_DRAG * drag_t);
        if opacity > 0.0 {
            painter.circle_filled(
                center,
                STATE_LAYER_RADIUS,
                Palette::alpha(active_color, opacity),
            );
        }
    }

    let handle_width = HANDLE_WIDTH + (HANDLE_WIDTH_ACTIVE - HANDLE_WIDTH) * handle_t;
    let handle = Rect::from_center_size(center, Vec2::new(handle_width, HANDLE_HEIGHT));
    painter.rect_filled(handle, CornerRadius::same(HANDLE_RADIUS_PX), active_color);

    let percent = t * 100.0;
    response
        .widget_info(|| egui::WidgetInfo::slider(enabled, f64::from(t), format!("{percent:.0}%")));

    WavySliderResponse {
        response,
        changed,
        dragging,
    }
}

/// The track (without the handle's state-layer inset).
fn track_rect(rect: Rect) -> Rect {
    let inset = TRACK_INSET.min((rect.width() * 0.5 - 0.5).max(0.0));
    let width = (rect.width() - 2.0 * inset).max(1.0);
    Rect::from_center_size(rect.center(), Vec2::new(width, TRACK_HEIGHT))
}

/// Scroll phase in radians, derived from egui's f64 time so a long session does
/// not lose precision in the sine argument.
fn wave_phase(ui: &egui::Ui) -> f32 {
    let time = ui.input(|input| input.time);
    let turns = time * f64::from(PHASE_SPEED) / f64::from(WAVE_LENGTH);
    (turns.rem_euclid(1.0) * std::f64::consts::TAU) as f32
}

/// The filled band between the wave's upper and lower edge.
///
/// One vertex pair per horizontal pixel: two triangles per segment. The shape
/// is a capsule at the left end, flat where it meets the handle, and round again
/// at the right end when the played span reaches the end of the track.
fn wavy_band_mesh(track: Rect, end: f32, amplitude: f32, phase: f32, color: Color32) -> Mesh {
    let left = track.left();
    let span = (end - left).max(0.0);
    let mut mesh = Mesh::default();
    if span <= EPSILON {
        return mesh;
    }

    let radius = track.height() * 0.5;
    let cap_right = track.right() - end <= 0.5;
    let steps = (span.ceil() as usize).clamp(1, MAX_SEGMENTS);
    mesh.reserve_vertices(2 * (steps + 1));
    mesh.reserve_triangles(2 * steps);

    let center_y = track.center().y;
    for i in 0..=steps {
        let x = left + span * (i as f32 / steps as f32);
        let half = band_half_height(x, left, end, radius, cap_right);
        let wave = amplitude
            * wave_envelope(x, left, end)
            * ((std::f32::consts::TAU * (x - left) / WAVE_LENGTH) - phase).sin();
        let y = center_y + wave;
        mesh.colored_vertex(egui::pos2(x, y - half), color);
        mesh.colored_vertex(egui::pos2(x, y + half), color);
    }

    for i in 0..steps {
        let base = 2 * i as u32;
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    mesh
}

/// Vertical half-height of the band at `x`.
fn band_half_height(x: f32, left: f32, right: f32, radius: f32, cap_right: bool) -> f32 {
    let half = left_cap_half(x, left, radius);
    if cap_right {
        half.min(right_cap_half(x, right, radius))
    } else {
        half
    }
}

fn left_cap_half(x: f32, left: f32, radius: f32) -> f32 {
    let dx = x - (left + radius);
    if dx <= -radius {
        0.0
    } else if dx >= 0.0 {
        radius
    } else {
        (radius * radius - dx * dx).sqrt()
    }
}

fn right_cap_half(x: f32, right: f32, radius: f32) -> f32 {
    let dx = x - (right - radius);
    if dx <= 0.0 {
        radius
    } else if dx >= radius {
        0.0
    } else {
        (radius * radius - dx * dx).sqrt()
    }
}

/// Spatial amplitude envelope: grow out of the flat start, taper out before the
/// handle so the wave never fights the pill.
fn wave_envelope(x: f32, left: f32, end: f32) -> f32 {
    if end <= left {
        return 0.0;
    }
    smoothstep01((x - left) / WAVE_RAMP_IN) * smoothstep01((end - x) / WAVE_TAPER)
}

fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Scheme;

    fn palette() -> Palette {
        Palette::from_seed(Color32::from_rgb(0x67, 0x50, 0xA4), Scheme::Dark)
    }

    #[test]
    fn envelope_vanishes_at_both_ends() {
        let (left, end) = (10.0, 210.0);
        assert_eq!(wave_envelope(left, left, end), 0.0);
        assert_eq!(wave_envelope(end, left, end), 0.0);
        assert!(wave_envelope(0.5 * (left + end), left, end) > 0.99);
    }

    #[test]
    fn envelope_is_zero_for_an_empty_span() {
        assert_eq!(wave_envelope(10.0, 10.0, 10.0), 0.0);
        assert_eq!(wave_envelope(10.0, 20.0, 10.0), 0.0);
    }

    #[test]
    fn band_rounds_the_left_cap_and_the_full_right_end() {
        let radius = 4.0;
        assert_eq!(band_half_height(10.0, 10.0, 410.0, radius, true), 0.0);
        assert_eq!(band_half_height(14.0, 10.0, 410.0, radius, true), 4.0);
        assert_eq!(band_half_height(410.0, 10.0, 410.0, radius, true), 0.0);
        // A short span keeps the band slim, never negative.
        for i in 0..=20 {
            let x = 10.0 + i as f32 * 0.1;
            let half = band_half_height(x, 10.0, 12.0, radius, true);
            assert!((0.0..=radius).contains(&half));
        }
    }

    #[test]
    fn mesh_is_a_wave_with_two_vertices_per_sample() {
        let track = Rect::from_min_max(egui::pos2(10.0, 28.0), egui::pos2(410.0, 36.0));
        let mesh = wavy_band_mesh(track, 210.0, WAVE_AMPLITUDE, 0.0, Color32::WHITE);
        assert!(mesh.is_valid());
        assert!(mesh.vertices.len() > 2);
        assert_eq!(mesh.vertices.len() % 2, 0);
        assert_eq!(mesh.triangles().count(), mesh.vertices.len() - 2);
        let min_y = mesh
            .vertices
            .iter()
            .map(|v| v.pos.y)
            .fold(f32::INFINITY, f32::min);
        let max_y = mesh
            .vertices
            .iter()
            .map(|v| v.pos.y)
            .fold(f32::NEG_INFINITY, f32::max);
        // centre 32, half height 4, amplitude 3.5 => 24.5 .. 39.5.
        assert!(min_y < 26.0);
        assert!(max_y > 38.0);
    }

    #[test]
    fn widget_paints_without_panicking() {
        let ctx = egui::Context::default();
        let mut value = 0.5_f32;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let palette = palette();
                let out = wavy_slider(ui, &palette, "wavy-test", 240.0, &mut value, true);
                assert!(!out.changed);
                assert!(!out.dragging);
            });
        });
    }
}
