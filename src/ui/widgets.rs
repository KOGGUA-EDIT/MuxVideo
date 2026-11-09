//! Reusable Material You widgets, painted directly with [`egui::Painter`].
//!
//! egui's stock widgets cannot express Material 3 state layers (their fill is
//! constant across hover/press), so the controls here paint their own surfaces
//! and only use egui for layout, hit-testing and animation. Every widget takes a
//! [`Palette`] because the app may show several palettes in one frame (the
//! showcase switches seed and scheme live).
//!
//! # Composition
//!
//! A typical screen composes the kit like this:
//!
//! ```text
//! ┌ rail (88 px) ─┐ ┌ stage card ──────────────────────────────┐
//! │ nav_rail_item │ │ title / headline text                     │
//! │ nav_rail_item │ │ card(CardKind::Filled, ...)               │
//! │ nav_rail_item │ │ segmented(ui, palette, "mode", …, …)      │
//! └───────────────┘ │ slider(ui, palette, &mut pos, 0..=1, …)   │
//!                   └──────────────────────────────────────────┘
//! ┌ transport bar ──────────────────────────────────────────────┐
//! │ icon_button(Play)  icon_button(Pause)  filter_chip(…)       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! - **nav rail**: one [`nav_rail_item`] per destination inside an 88 px column;
//!   the selected item owns the pill indicator.
//! - **card**: wrap any block in [`card`]; the 28 px radius and 24 px padding
//!   come from the kit, so children should not paint another full-width fill.
//! - **transport bar**: [`icon_button`] for play/pause, [`slider`] for scrub,
//!   [`segmented`] for A/B/wipe modes, [`snackbar`] for transient feedback.
//! - **chip**: [`assist_chip`] for actions, [`filter_chip`] for toggles.
//!
//! Colours follow the Material 3 contract: containers carry `on_*` content, and
//! interactive surfaces get an 8 % / 10 % state layer of their content colour on
//! hover / press. Sizes are on the 8 px grid; the 40 px buttons, 44 px slider
//! box and 88 px rail keep desktop touch targets comfortable.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Margin, Pos2, Rect, Response, RichText, Sense,
    Stroke, StrokeKind, Vec2,
};

use crate::ui::Palette;

/// Shape tokens, in logical pixels.
pub mod shape {
    use eframe::egui::CornerRadius;

    /// 4 px — tooltips, tiny badges.
    pub const XS: u8 = 4;
    /// 8 px — chips, snackbars.
    pub const SM: u8 = 8;
    /// 12 px — menus, text fields.
    pub const MD: u8 = 12;
    /// 16 px — rail indicators, small cards.
    pub const LG: u8 = 16;
    /// 20 px — buttons, switches, sliders.
    pub const XL: u8 = 20;
    /// 28 px — cards, dialogs.
    pub const CARD: u8 = 28;

    /// A corner radius from a token.
    #[must_use]
    pub const fn radius(px: u8) -> CornerRadius {
        CornerRadius::same(px)
    }

    /// Fully rounded (pill) corners.
    #[must_use]
    pub const fn pill() -> CornerRadius {
        CornerRadius::same(255)
    }
}

/// Spacing tokens on the 8 px grid (plus the 4 px half step).
pub mod space {
    /// 4 px.
    pub const XS: f32 = 4.0;
    /// 8 px.
    pub const SM: f32 = 8.0;
    /// 12 px.
    pub const MD: f32 = 12.0;
    /// 16 px.
    pub const LG: f32 = 16.0;
    /// 24 px.
    pub const XL: f32 = 24.0;
    /// 32 px.
    pub const XXL: f32 = 32.0;
}

/// State-layer opacities from the Material 3 interaction spec.
pub mod state_layer {
    /// Hover: 8 % of the content colour.
    pub const HOVER: f32 = 0.08;
    /// Keyboard focus: 10 %.
    pub const FOCUS: f32 = 0.10;
    /// Press: 10 %.
    pub const PRESSED: f32 = 0.10;
    /// Drag: 16 %.
    pub const DRAGGED: f32 = 0.16;
    /// Disabled content: 38 %.
    pub const DISABLED: f32 = 0.38;
}

/// Paints Material 3 state layers (hover / focus / press) over a widget.
pub struct StateLayer;

impl StateLayer {
    /// `content` blended with the given opacity (premultiplied, ready to paint).
    #[must_use]
    pub fn overlay(content: Color32, opacity: f32) -> Color32 {
        Palette::alpha(content, opacity)
    }

    /// The animated layer opacity for `response`.
    ///
    /// Hover fades in over 120 ms, press over 100 ms, so pointer interaction
    /// feels like Material rather than an instant colour flip.
    #[must_use]
    pub fn opacity_for(ui: &egui::Ui, response: &Response) -> f32 {
        let ctx = ui.ctx();
        let hover = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12);
        let press = ctx.animate_bool_with_time(
            response.id.with("press"),
            response.is_pointer_button_down_on(),
            0.10,
        );
        let focus = if response.has_focus() {
            state_layer::FOCUS
        } else {
            0.0
        };
        (state_layer::HOVER * hover)
            .max(state_layer::PRESSED * press)
            .max(focus)
    }

    /// Paints the layer over `rect` using the widget's `content` colour.
    pub fn paint(
        ui: &egui::Ui,
        rect: Rect,
        radius: CornerRadius,
        content: Color32,
        response: &Response,
    ) {
        let opacity = Self::opacity_for(ui, response);
        if opacity > 0.001 {
            ui.painter()
                .rect_filled(rect, radius, Self::overlay(content, opacity));
        }
    }
}

/// Vector icons, hand-drawn so the kit does not depend on an icon font.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// Filled play triangle.
    Play,
    /// Two pause bars.
    Pause,
    /// Skip to previous frame / clip.
    SkipBack,
    /// Skip to next frame / clip.
    SkipForward,
    /// Speaker with sound arcs.
    Volume,
    /// Two overlapping frames (comparison).
    Compare,
    /// Gear-ish settings glyph.
    Settings,
    /// Film strip.
    Film,
    /// Half-filled circle (contrast / difference).
    Contrast,
    /// Check mark.
    Check,
    /// Right chevron.
    ChevronRight,
    /// Down chevron.
    ChevronDown,
    /// Plus sign.
    Plus,
    /// Close cross.
    Close,
    /// Magnifying glass.
    Search,
}

/// Paints `icon` inside `rect` (which should be square) with `color`.
pub fn paint_icon(painter: &egui::Painter, icon: Icon, rect: Rect, color: Color32) {
    let scale = (rect.width().min(rect.height()) / 24.0).max(0.01);
    let stroke = Stroke::new((2.0 * scale).max(1.0), color);
    let p = |x: f32, y: f32| {
        egui::pos2(
            rect.center().x + (x - 12.0) * scale,
            rect.center().y + (y - 12.0) * scale,
        )
    };
    let line = |points: Vec<Pos2>| {
        painter.add(egui::Shape::line(points, stroke));
    };
    let fill = |points: Vec<Pos2>| {
        painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
    };
    let bar = |x0: f32, y0: f32, x1: f32, y1: f32| {
        painter.rect_filled(Rect::from_min_max(p(x0, y0), p(x1, y1)), 0, color);
    };

    match icon {
        Icon::Play => fill(vec![p(8.0, 5.0), p(19.0, 12.0), p(8.0, 19.0)]),
        Icon::Pause => {
            bar(7.0, 5.0, 10.5, 19.0);
            bar(13.5, 5.0, 17.0, 19.0);
        }
        Icon::SkipBack => {
            fill(vec![p(18.0, 5.0), p(8.0, 12.0), p(18.0, 19.0)]);
            bar(5.0, 5.0, 7.5, 19.0);
        }
        Icon::SkipForward => {
            fill(vec![p(6.0, 5.0), p(16.0, 12.0), p(6.0, 19.0)]);
            bar(16.5, 5.0, 19.0, 19.0);
        }
        Icon::Volume => {
            fill(vec![
                p(4.0, 9.0),
                p(8.0, 9.0),
                p(13.0, 5.0),
                p(13.0, 19.0),
                p(8.0, 15.0),
                p(4.0, 15.0),
            ]);
            for radius in [4.5_f32, 7.5_f32] {
                let mut arc = Vec::with_capacity(12);
                for step in 0..=10 {
                    let angle = (-70.0 + 140.0 * (step as f32 / 10.0)).to_radians();
                    arc.push(p(13.0 + radius * angle.cos(), 12.0 + radius * angle.sin()));
                }
                line(arc);
            }
        }
        Icon::Compare => {
            painter.rect_stroke(
                Rect::from_min_max(p(4.0, 5.0), p(16.0, 17.0)),
                shape::radius(2),
                stroke,
                StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(8.0, 8.0), p(20.0, 20.0)),
                shape::radius(2),
                stroke,
                StrokeKind::Middle,
            );
        }
        Icon::Settings => {
            painter.circle_stroke(p(12.0, 12.0), 3.6 * scale, stroke);
            for tick in 0..8 {
                let angle = std::f32::consts::TAU * (tick as f32 / 8.0);
                line(vec![
                    p(12.0 + 6.4 * angle.cos(), 12.0 + 6.4 * angle.sin()),
                    p(12.0 + 9.0 * angle.cos(), 12.0 + 9.0 * angle.sin()),
                ]);
            }
        }
        Icon::Film => {
            painter.rect_stroke(
                Rect::from_min_max(p(3.0, 5.0), p(21.0, 19.0)),
                shape::radius(2),
                stroke,
                StrokeKind::Middle,
            );
            for x in [8.0_f32, 12.0, 16.0] {
                line(vec![p(x, 5.0), p(x, 19.0)]);
            }
        }
        Icon::Contrast => {
            painter.circle_stroke(p(12.0, 12.0), 8.0 * scale, stroke);
            let mut half = Vec::with_capacity(18);
            half.push(p(12.0, 12.0));
            for step in 0..=16 {
                let angle =
                    -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * (step as f32 / 16.0);
                half.push(p(12.0 + 8.0 * angle.cos(), 12.0 + 8.0 * angle.sin()));
            }
            fill(half);
        }
        Icon::Check => line(vec![p(5.0, 12.0), p(10.0, 17.0), p(19.0, 7.0)]),
        Icon::ChevronRight => line(vec![p(9.0, 6.0), p(15.0, 12.0), p(9.0, 18.0)]),
        Icon::ChevronDown => line(vec![p(6.0, 9.0), p(12.0, 15.0), p(18.0, 9.0)]),
        Icon::Plus => {
            line(vec![p(12.0, 5.0), p(12.0, 19.0)]);
            line(vec![p(5.0, 12.0), p(19.0, 12.0)]);
        }
        Icon::Close => {
            line(vec![p(6.0, 6.0), p(18.0, 18.0)]);
            line(vec![p(18.0, 6.0), p(6.0, 18.0)]);
        }
        Icon::Search => {
            painter.circle_stroke(p(10.5, 10.5), 6.0 * scale, stroke);
            line(vec![p(15.0, 15.0), p(20.0, 20.0)]);
        }
    }
}

/// The Material 3 button families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// Highest emphasis: filled with `primary`.
    Filled,
    /// Medium emphasis: `secondary_container`.
    Tonal,
    /// Medium emphasis with depth: `surface_container_low` + shadow.
    Elevated,
    /// Low emphasis: outline, `primary` label.
    Outlined,
    /// Lowest emphasis: no container, `primary` label.
    Text,
}

/// Paints a Material 3 button with a 40 px height and pill corners.
///
/// The label is always centred; the width grows with the label but never drops
/// below 64 px. Disabled buttons keep their shape and fade their content to
/// 38 % on top of a 12 % `on_surface` container.
pub fn button(ui: &mut egui::Ui, palette: &Palette, kind: ButtonKind, text: &str) -> Response {
    let font = FontId::proportional(14.0);
    let horizontal_padding = if kind == ButtonKind::Text { 12.0 } else { 24.0 };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), Color32::PLACEHOLDER);
    let width = (galley.size().x + horizontal_padding * 2.0)
        .max(64.0)
        .ceil();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 40.0), Sense::click());

    let enabled = ui.is_enabled();
    let radius = shape::radius(shape::XL);
    let (fill, content) = button_colors(palette, kind, enabled);
    let painter = ui.painter().clone();

    if kind == ButtonKind::Elevated && enabled {
        painter.add(
            egui::Shadow {
                offset: [0, 1],
                blur: 4,
                spread: 0,
                color: Palette::alpha(palette.shadow, 0.24),
            }
            .as_shape(rect, radius),
        );
    }

    painter.rect_filled(rect, radius, fill);
    if kind == ButtonKind::Outlined {
        let outline = if enabled {
            palette.outline
        } else {
            Palette::alpha(palette.on_surface, 0.12)
        };
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, outline),
            StrokeKind::Inside,
        );
    }
    if enabled {
        StateLayer::paint(ui, rect, radius, content, &response);
    }
    if response.has_focus() && enabled {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(2.0_f32, palette.primary),
            StrokeKind::Inside,
        );
    }
    painter.text(rect.center(), Align2::CENTER_CENTER, text, font, content);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, text));
    response
}

fn button_colors(palette: &Palette, kind: ButtonKind, enabled: bool) -> (Color32, Color32) {
    let (fill, content) = match kind {
        ButtonKind::Filled => (palette.primary, palette.on_primary),
        ButtonKind::Tonal => (palette.secondary_container, palette.on_secondary_container),
        ButtonKind::Elevated => (palette.surface_container_low, palette.primary),
        ButtonKind::Outlined | ButtonKind::Text => (Color32::TRANSPARENT, palette.primary),
    };
    if enabled {
        return (fill, content);
    }
    let disabled_content = Palette::alpha(palette.on_surface, state_layer::DISABLED);
    match kind {
        ButtonKind::Outlined | ButtonKind::Text => (Color32::TRANSPARENT, disabled_content),
        _ => (Palette::alpha(palette.on_surface, 0.12), disabled_content),
    }
}

/// A filled button (highest emphasis).
pub fn filled_button(ui: &mut egui::Ui, palette: &Palette, label: &str) -> Response {
    button(ui, palette, ButtonKind::Filled, label)
}

/// A tonal button (medium emphasis).
pub fn tonal_button(ui: &mut egui::Ui, palette: &Palette, label: &str) -> Response {
    button(ui, palette, ButtonKind::Tonal, label)
}

/// An elevated button (medium emphasis with a shadow).
pub fn elevated_button(ui: &mut egui::Ui, palette: &Palette, label: &str) -> Response {
    button(ui, palette, ButtonKind::Elevated, label)
}

/// An outlined button (low emphasis).
pub fn outlined_button(ui: &mut egui::Ui, palette: &Palette, label: &str) -> Response {
    button(ui, palette, ButtonKind::Outlined, label)
}

/// A text button (lowest emphasis).
pub fn text_button(ui: &mut egui::Ui, palette: &Palette, label: &str) -> Response {
    button(ui, palette, ButtonKind::Text, label)
}

/// A 40 × 40 icon button.
pub fn icon_button(ui: &mut egui::Ui, palette: &Palette, icon: Icon) -> Response {
    icon_button_impl(ui, palette, icon, false)
}

/// A 40 × 40 icon button that shows a selected (`secondary_container`) state.
pub fn icon_button_toggle(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Icon,
    selected: bool,
) -> Response {
    icon_button_impl(ui, palette, icon, selected)
}

fn icon_button_impl(ui: &mut egui::Ui, palette: &Palette, icon: Icon, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
    let radius = shape::radius(shape::XL);
    let enabled = ui.is_enabled();
    let (fill, content) = if selected {
        (palette.secondary_container, palette.on_secondary_container)
    } else {
        (
            Color32::TRANSPARENT,
            if response.hovered() {
                palette.on_surface
            } else {
                palette.on_surface_variant
            },
        )
    };
    let content = if enabled {
        content
    } else {
        Palette::alpha(palette.on_surface, state_layer::DISABLED)
    };
    let painter = ui.painter().clone();
    painter.rect_filled(rect, radius, fill);
    if enabled {
        StateLayer::paint(ui, rect, radius, content, &response);
    }
    if response.has_focus() && enabled {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(2.0_f32, palette.primary),
            StrokeKind::Inside,
        );
    }
    paint_icon(
        &painter,
        icon,
        Rect::from_center_size(rect.center(), Vec2::splat(20.0)),
        content,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "icon"));
    response
}

/// A Material 3 segmented button group.
///
/// `labels` are laid out in one pill-shaped outline; the selected segment gets a
/// `secondary_container` fill. Clicks and horizontal drags both select, so
/// scrubbing across the group feels natural.
pub fn segmented(
    ui: &mut egui::Ui,
    palette: &Palette,
    id_salt: impl std::hash::Hash,
    labels: &[&str],
    selected: &mut usize,
) -> Response {
    if labels.is_empty() {
        let (_, response) = ui.allocate_exact_size(Vec2::new(0.0, 40.0), Sense::hover());
        return response;
    }
    let font = FontId::proportional(14.0);
    let widths: Vec<f32> = labels
        .iter()
        .map(|label| {
            let galley = ui.painter().layout_no_wrap(
                (*label).to_owned(),
                font.clone(),
                Color32::PLACEHOLDER,
            );
            (galley.size().x + 40.0).max(72.0).ceil()
        })
        .collect();
    let total: f32 = widths.iter().sum();
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(total, 40.0), Sense::click_and_drag());

    let index_at = |x: f32| {
        let mut left = rect.left();
        for (index, width) in widths.iter().enumerate() {
            if x >= left && x <= left + width {
                return Some(index);
            }
            left += width;
        }
        None
    };

    if let Some(pointer) = response.interact_pointer_pos()
        && rect.contains(pointer)
        && let Some(index) = index_at(pointer.x)
        && *selected != index
    {
        *selected = index;
        response.mark_changed();
    }
    *selected = (*selected).min(labels.len() - 1);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            labels.join(" / "),
        )
    });

    let radius = shape::radius(shape::XL);
    let painter = ui.painter().clone();
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, palette.outline),
        StrokeKind::Inside,
    );

    let hovered_index = ui
        .ctx()
        .pointer_hover_pos()
        .filter(|pointer| rect.contains(*pointer))
        .and_then(|pointer| index_at(pointer.x));
    let animation_id = response.id.with(id_salt);
    let hover = ui.ctx().animate_bool_with_time(
        animation_id.with("segment_hover"),
        hovered_index.is_some(),
        0.12,
    );

    let mut left = rect.left();
    for (index, (label, width)) in labels.iter().zip(widths.iter()).enumerate() {
        let segment = Rect::from_min_size(
            egui::pos2(left, rect.top()),
            Vec2::new(*width, rect.height()),
        );
        let is_selected = index == *selected;
        if is_selected {
            painter.rect_filled(segment.shrink(1.0), radius, palette.secondary_container);
        } else if hovered_index == Some(index) {
            painter.rect_filled(
                segment.shrink(1.0),
                radius,
                Palette::alpha(palette.on_surface, state_layer::HOVER * hover),
            );
        }
        let color = if is_selected {
            palette.on_secondary_container
        } else {
            palette.on_surface_variant
        };
        painter.text(
            segment.center(),
            Align2::CENTER_CENTER,
            *label,
            font.clone(),
            color,
        );
        left += *width;
    }
    response
}

/// A Material 3 switch (52 × 32 track, animated handle).
pub fn switch(ui: &mut egui::Ui, palette: &Palette, on: &mut bool) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(52.0, 32.0), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let enabled = ui.is_enabled();
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("switch"), *on && enabled, 0.15);
    let painter = ui.painter().clone();

    let track = rect.shrink(2.0);
    let track_color = lerp_color(palette.surface_container_highest, palette.primary, t);
    painter.rect_filled(track, shape::radius(shape::LG), track_color);
    if t < 0.999 {
        painter.rect_stroke(
            track,
            shape::radius(shape::LG),
            Stroke::new(2.0_f32, Palette::alpha(palette.outline, 1.0 - t)),
            StrokeKind::Inside,
        );
    }

    let center_x = lerp(rect.left() + 12.0, rect.right() - 16.0, t);
    let handle_radius = lerp(8.0, 12.0, t);
    let handle_center = egui::pos2(center_x, rect.center().y);

    if response.hovered() {
        let layer = StateLayer::overlay(palette.on_surface, state_layer::HOVER);
        painter.circle_filled(handle_center, handle_radius + 8.0, layer);
    }
    let handle_color = lerp_color(palette.outline, palette.on_primary, t);
    painter.circle_filled(handle_center, handle_radius, handle_color);

    if response.has_focus() {
        painter.circle_stroke(
            handle_center,
            handle_radius + 4.0,
            Stroke::new(2.0_f32, palette.primary),
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Checkbox,
            enabled,
            if *on { "on" } else { "off" },
        )
    });
    response
}

/// A Material 3 slider with a 4 px track and a 20 px handle.
///
/// The widget reserves a 44 px tall box (touch target); the track is inset by
/// the handle radius so the handle never clips at either end. Clicking anywhere
/// on the track jumps to that value; dragging scrubs continuously.
pub fn slider(
    ui: &mut egui::Ui,
    palette: &Palette,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    width: f32,
) -> Response {
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, 44.0), Sense::click_and_drag());
    let start = *range.start();
    let end = *range.end();
    let span = end - start;
    let track = Rect::from_min_max(
        egui::pos2(rect.left() + 10.0, rect.center().y - 2.0),
        egui::pos2(rect.right() - 10.0, rect.center().y + 2.0),
    );

    if (response.clicked() || response.dragged())
        && let Some(pointer) = response.interact_pointer_pos()
        && track.width() > 0.0
    {
        let t = ((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0);
        let next = start + t * span;
        if (next - *value).abs() > f32::EPSILON {
            *value = next;
            response.mark_changed();
        }
    }
    *value = value.clamp(start.min(end), start.max(end));

    let t = if span.abs() < f32::EPSILON {
        0.0
    } else {
        ((*value - start) / span).clamp(0.0, 1.0)
    };
    let enabled = ui.is_enabled();
    let painter = ui.painter().clone();
    let radius = shape::radius(2);
    let track_color = if enabled {
        palette.surface_container_highest
    } else {
        Palette::alpha(palette.on_surface, 0.12)
    };
    let active_color = if enabled {
        palette.primary
    } else {
        Palette::alpha(palette.on_surface, state_layer::DISABLED)
    };
    painter.rect_filled(track, radius, track_color);

    let handle_center = egui::pos2(lerp(track.left(), track.right(), t), rect.center().y);
    if t > 0.0 {
        let active = Rect::from_min_max(track.min, egui::pos2(handle_center.x, track.max.y));
        painter.rect_filled(active, radius, active_color);
    }
    if enabled && (response.hovered() || response.dragged()) {
        painter.circle_filled(
            handle_center,
            20.0,
            StateLayer::overlay(active_color, state_layer::HOVER),
        );
    }
    painter.circle_filled(handle_center, 10.0, active_color);
    if response.has_focus() {
        painter.circle_stroke(handle_center, 14.0, Stroke::new(2.0_f32, palette.primary));
    }
    response.widget_info(|| {
        egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*value), format!("{value:.2}"))
    });
    response
}

/// A navigation-rail destination: a 56 × 32 pill indicator with icon + label.
///
/// The item fills the available width, so place it in the rail's 88 px column.
/// Selected items use `secondary_container` behind the icon and `on_surface`
/// for the label; unselected items are quiet `on_surface_variant`.
pub fn nav_rail_item(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Icon,
    label: &str,
    selected: bool,
) -> Response {
    let width = ui.available_width().max(56.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::click());
    let radius = shape::radius(shape::LG);
    let indicator = Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + 16.0),
        Vec2::new(56.0, 32.0),
    );
    let (fill, content, label_color) = if selected {
        (
            palette.secondary_container,
            palette.on_secondary_container,
            palette.on_surface,
        )
    } else {
        (
            Color32::TRANSPARENT,
            palette.on_surface_variant,
            palette.on_surface_variant,
        )
    };
    let painter = ui.painter().clone();
    painter.rect_filled(indicator, radius, fill);
    StateLayer::paint(ui, indicator, radius, content, &response);
    if response.has_focus() {
        painter.rect_stroke(
            indicator,
            radius,
            Stroke::new(2.0_f32, palette.primary),
            StrokeKind::Inside,
        );
    }
    paint_icon(
        &painter,
        icon,
        Rect::from_center_size(indicator.center(), Vec2::splat(20.0)),
        content,
    );
    painter.text(
        egui::pos2(rect.center().x, rect.top() + 42.0),
        Align2::CENTER_TOP,
        label,
        FontId::proportional(12.0),
        label_color,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response
}

/// The Material 3 card variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    /// Filled card: `surface_container`, no outline.
    Filled,
    /// Elevated card: `surface_container_low` with a soft shadow.
    Elevated,
    /// Outlined card: `surface` with a 1 px `outline_variant` border.
    Outlined,
}

/// Wraps `add_contents` in a 28 px radius card with 24 px padding.
///
/// The returned [`egui::InnerResponse`] keeps the closure's value, so callers
/// can read state (for example a button click) without a `let mut` dance.
pub fn card<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    kind: CardKind,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let frame = egui::Frame::NONE
        .inner_margin(Margin::same(24))
        .corner_radius(shape::radius(shape::CARD));
    let frame = match kind {
        CardKind::Filled => frame.fill(palette.surface_container),
        CardKind::Elevated => frame
            .fill(palette.surface_container_low)
            .shadow(egui::Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color: Palette::alpha(palette.shadow, 0.20),
            }),
        CardKind::Outlined => frame
            .fill(palette.surface)
            .stroke(Stroke::new(1.0_f32, palette.outline_variant)),
    };
    frame.show(ui, add_contents)
}

/// An assist chip: outlined pill with an optional leading icon.
pub fn assist_chip(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Option<Icon>,
    label: &str,
) -> Response {
    chip(ui, palette, icon, label, false, true)
}

/// A filter chip: toggles between outlined and `secondary_container` selected.
pub fn filter_chip(
    ui: &mut egui::Ui,
    palette: &Palette,
    label: &str,
    selected: &mut bool,
) -> Response {
    let mut response = chip(ui, palette, None, label, *selected, false);
    if response.clicked() {
        *selected = !*selected;
        response.mark_changed();
    }
    response
}

fn chip(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Option<Icon>,
    label: &str,
    selected: bool,
    outlined: bool,
) -> Response {
    let font = FontId::proportional(14.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), Color32::PLACEHOLDER);
    let icon_space = if icon.is_some() || selected {
        24.0
    } else {
        0.0
    };
    let width = (galley.size().x + 24.0 + icon_space).max(48.0).ceil();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click());
    let radius = shape::radius(shape::SM);
    let enabled = ui.is_enabled();
    let (fill, content) = if selected {
        (palette.secondary_container, palette.on_secondary_container)
    } else if enabled {
        (Color32::TRANSPARENT, palette.on_surface_variant)
    } else {
        (
            Color32::TRANSPARENT,
            Palette::alpha(palette.on_surface, state_layer::DISABLED),
        )
    };
    let painter = ui.painter().clone();
    painter.rect_filled(rect, radius, fill);
    if outlined {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, palette.outline_variant),
            StrokeKind::Inside,
        );
    }
    if enabled {
        StateLayer::paint(ui, rect, radius, content, &response);
    }
    let leading = icon.or(if selected { Some(Icon::Check) } else { None });
    let mut text_left = rect.left() + 12.0;
    if let Some(leading) = leading {
        paint_icon(
            &painter,
            leading,
            Rect::from_center_size(
                egui::pos2(text_left + 9.0, rect.center().y),
                Vec2::splat(18.0),
            ),
            content,
        );
        text_left += 24.0;
    }
    painter.text(
        egui::pos2(text_left, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        content,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response
}

/// What [`snackbar`] returns: the bar itself plus the action, if present.
pub struct SnackbarResponse {
    /// The whole bar (hover, so action clicks are handled separately).
    pub bar: Response,
    /// The optional action label; click it via [`Response::clicked`].
    pub action: Option<Response>,
}

/// A Material 3 snackbar: inverse surface, one line, optional action.
///
/// The bar spans the available width (minus nothing — callers usually put it in
/// a padded bottom panel). The action sits at the trailing edge and is its own
/// click target.
pub fn snackbar(
    ui: &mut egui::Ui,
    palette: &Palette,
    message: &str,
    action: Option<&str>,
) -> SnackbarResponse {
    let width = ui.available_width().max(160.0);
    let (rect, bar) = ui.allocate_exact_size(Vec2::new(width, 48.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, shape::radius(shape::SM), palette.inverse_surface);
    painter.text(
        egui::pos2(rect.left() + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        message,
        FontId::proportional(14.0),
        palette.inverse_on_surface,
    );

    let action = action.map(|label| {
        let galley = ui.painter().layout_no_wrap(
            label.to_owned(),
            FontId::proportional(14.0),
            palette.inverse_primary,
        );
        let text_width = galley.size().x;
        let action_rect = Rect::from_min_size(
            egui::pos2(rect.right() - 16.0 - text_width, rect.top()),
            Vec2::new(text_width, rect.height()),
        );
        let response = ui.interact(action_rect, ui.id().with("snackbar_action"), Sense::click());
        if response.hovered() {
            painter.rect_filled(
                action_rect.expand2(Vec2::new(8.0, 8.0)),
                shape::radius(shape::SM),
                Palette::alpha(palette.inverse_primary, state_layer::HOVER),
            );
        }
        painter.text(
            action_rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(14.0),
            palette.inverse_primary,
        );
        response
    });

    SnackbarResponse { bar, action }
}

/// A linear progress indicator: 4 px track, rounded ends.
pub fn linear_progress(
    ui: &mut egui::Ui,
    palette: &Palette,
    progress: f32,
    width: f32,
) -> Response {
    let progress = progress.clamp(0.0, 1.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 4.0), Sense::hover());
    let painter = ui.painter().clone();
    let radius = shape::radius(2);
    painter.rect_filled(rect, radius, palette.surface_container_highest);
    if progress > 0.0 {
        let filled =
            Rect::from_min_size(rect.min, Vec2::new(rect.width() * progress, rect.height()));
        painter.rect_filled(filled, radius, palette.primary);
    }
    response
}

/// A 1 px `outline_variant` divider.
pub fn divider(ui: &mut egui::Ui, palette: &Palette) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0_f32, palette.outline_variant),
    );
}

/// A 2 px `outline_variant` divider for section breaks.
pub fn divider_thick(ui: &mut egui::Ui, palette: &Palette) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 2.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(2.0_f32, palette.outline_variant),
    );
}

/// Display type: 36 px, tight tracking. Use for empty states and cover titles.
#[must_use]
pub fn display(palette: &Palette, text: impl Into<String>) -> RichText {
    RichText::new(text)
        .size(36.0)
        .color(palette.on_surface)
        .extra_letter_spacing(-0.5)
}

/// Headline type: 28 px, tight tracking.
#[must_use]
pub fn headline(palette: &Palette, text: impl Into<String>) -> RichText {
    RichText::new(text)
        .size(28.0)
        .color(palette.on_surface)
        .extra_letter_spacing(-0.25)
}

/// Title type: 22 px.
#[must_use]
pub fn title(palette: &Palette, text: impl Into<String>) -> RichText {
    RichText::new(text).size(22.0).color(palette.on_surface)
}

/// Small title type: 16 px, used for card headers and list rows.
#[must_use]
pub fn title_small(palette: &Palette, text: impl Into<String>) -> RichText {
    RichText::new(text).size(16.0).color(palette.on_surface)
}

/// Body type: 14 px.
#[must_use]
pub fn body(palette: &Palette, text: impl Into<String>) -> RichText {
    RichText::new(text).size(14.0).color(palette.on_surface)
}

/// Label type: 12 px with open tracking (Material 3 label style).
#[must_use]
pub fn label(palette: &Palette, text: impl Into<String>) -> RichText {
    RichText::new(text)
        .size(12.0)
        .color(palette.on_surface_variant)
        .extra_letter_spacing(0.4)
}

/// Small label type: 11 px with open tracking.
#[must_use]
pub fn label_small(palette: &Palette, text: impl Into<String>) -> RichText {
    RichText::new(text)
        .size(11.0)
        .color(palette.on_surface_variant)
        .extra_letter_spacing(0.5)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        lerp(f32::from(a.r()), f32::from(b.r()), t).round() as u8,
        lerp(f32::from(a.g()), f32::from(b.g()), t).round() as u8,
        lerp(f32::from(a.b()), f32::from(b.b()), t).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Scheme;

    fn palette() -> Palette {
        Palette::from_seed(Color32::from_rgb(0x67, 0x50, 0xA4), Scheme::Dark)
    }

    #[test]
    fn lerp_color_hits_both_ends() {
        let black = Color32::BLACK;
        let white = Color32::WHITE;
        assert_eq!(lerp_color(black, white, 0.0), black);
        assert_eq!(lerp_color(black, white, 1.0), white);
        assert_eq!(
            lerp_color(black, white, 0.5),
            Color32::from_rgb(128, 128, 128)
        );
    }

    #[test]
    fn widgets_paint_without_panicking() {
        let palette = palette();
        let ctx = egui::Context::default();
        let mut value = 0.4_f32;
        let mut on = false;
        let mut selected = 0_usize;
        let mut filtered = true;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = button(ui, &palette, ButtonKind::Filled, "Play");
                let _ = icon_button(ui, &palette, Icon::Play);
                let _ = segmented(ui, &palette, "modes", &["A", "B", "Wipe"], &mut selected);
                let _ = switch(ui, &palette, &mut on);
                let _ = slider(ui, &palette, &mut value, 0.0..=1.0, 220.0);
                let _ = nav_rail_item(ui, &palette, Icon::Compare, "对比", true);
                card(ui, &palette, CardKind::Outlined, |ui| {
                    ui.label(title(&palette, "卡片 Card"));
                });
                let _ = assist_chip(ui, &palette, Some(Icon::Volume), "音量");
                let _ = filter_chip(ui, &palette, "只看 A 面", &mut filtered);
                let _ = snackbar(ui, &palette, "已保存设置 Saved", Some("撤销"));
                let _ = linear_progress(ui, &palette, value, 220.0);
                divider(ui, &palette);
                divider_thick(ui, &palette);
            });
        });
    }
}
