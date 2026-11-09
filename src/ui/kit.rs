//! Installs the Material You look-and-feel on an [`egui::Context`].
//!
//! Call [`install_style`] once, after [`crate::fonts::install`] (the app installs
//! the CJK font fallback before the style) and after building the [`Palette`]:
//!
//! ```no_run
//! # use eframe::egui;
//! # fn demo(ctx: &egui::Context) {
//! let seed = egui::Color32::from_rgb(0xD0, 0xBC, 0xFF);
//! let palette = muxvideo::ui::Palette::from_seed(seed, muxvideo::ui::Scheme::Dark);
//! muxvideo::ui::install_style(ctx, &palette);
//! # }
//! ```
//!
//! The style only paints defaults. Widgets from [`crate::ui::widgets`] draw
//! their own Material 3 surfaces, so app code can also paint explicit colours
//! without fighting the style: `panel_fill`, `override_text_color` and
//! `dark_mode` are the three knobs the shell relies on.

use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontId, Margin, Shadow, Stroke, TextStyle, Vec2,
    Visuals,
    style::{HandleShape, ScrollStyle, Selection, TextCursorStyle, WidgetVisuals, Widgets},
};

use super::palette::Palette;

/// Installs the Material 3 style for `palette` on `ctx`.
///
/// This sets both the dark and the light style slot to the generated style (so
/// a system-theme change cannot suddenly show egui's default theme) and makes
/// the palette's scheme the active theme. The app calls
/// [`crate::fonts::install`] *before* this function, because text styles are
/// font sizes and need the CJK fallback to already be registered.
pub fn install_style(ctx: &egui::Context, palette: &Palette) {
    ctx.set_theme(if palette.is_dark() {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.all_styles_mut(|style| apply_style(style, palette));
}

fn apply_style(style: &mut egui::Style, palette: &Palette) {
    style.text_styles = text_styles();
    style.spacing = spacing();
    style.interaction.tooltip_delay = 0.4;
    style.interaction.tooltip_grace_time = 0.2;
    style.visuals = visuals(palette);
    style.animation_time = 0.15;
}

/// The M3 type scale as egui text styles.
///
/// `small 11`, `body 14`, `button 14`, `heading 28`, `monospace 13`, plus named
/// styles used by [`crate::ui::widgets`]: `display 36`, `headline 28`,
/// `title 22`, `title_small 16`, `body 14`, `label 12`, `label_small 11`.
fn text_styles() -> BTreeMap<TextStyle, FontId> {
    BTreeMap::from([
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Body, FontId::proportional(14.0)),
        (TextStyle::Button, FontId::proportional(14.0)),
        (TextStyle::Heading, FontId::proportional(28.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
        (style_name("display"), FontId::proportional(36.0)),
        (style_name("headline"), FontId::proportional(28.0)),
        (style_name("title"), FontId::proportional(22.0)),
        (style_name("title_small"), FontId::proportional(16.0)),
        (style_name("body"), FontId::proportional(14.0)),
        (style_name("label"), FontId::proportional(12.0)),
        (style_name("label_small"), FontId::proportional(11.0)),
    ])
}

fn spacing() -> egui::Spacing {
    let mut scroll = ScrollStyle::solid();
    scroll.bar_width = 8.0;
    scroll.handle_min_length = 24.0;
    scroll.bar_inner_margin = 2.0;
    scroll.bar_outer_margin = 2.0;
    scroll.foreground_color = false;

    egui::Spacing {
        // 8 px horizontal rhythm; the shell asks for at most 6 px vertical.
        item_spacing: Vec2::new(8.0, 6.0),
        window_margin: Margin::same(24),
        menu_margin: Margin::same(8),
        button_padding: Vec2::new(20.0, 10.0),
        interact_size: Vec2::new(44.0, 44.0),
        slider_width: 220.0,
        slider_rail_height: 4.0,
        combo_width: 200.0,
        combo_height: 44.0,
        text_edit_width: 320.0,
        icon_width: 18.0,
        icon_width_inner: 12.0,
        icon_spacing: 8.0,
        tooltip_width: 300.0,
        menu_width: 240.0,
        indent: 16.0,
        scroll,
        ..Default::default()
    }
}

fn visuals(palette: &Palette) -> Visuals {
    let dark = palette.is_dark();
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.dark_mode = dark;
    visuals.override_text_color = Some(palette.on_surface);
    visuals.weak_text_color = Some(palette.on_surface_variant);
    visuals.panel_fill = palette.surface;
    visuals.faint_bg_color = palette.surface_container_low;
    visuals.extreme_bg_color = palette.surface;
    visuals.text_edit_bg_color = Some(palette.surface_container_highest);
    visuals.code_bg_color = palette.surface_container_high;
    visuals.warn_fg_color = palette.tertiary;
    visuals.error_fg_color = palette.error;
    visuals.hyperlink_color = palette.primary;
    visuals.selection = Selection {
        bg_fill: Palette::alpha(palette.primary, 0.28),
        stroke: Stroke::new(1.0_f32, palette.primary),
    };

    visuals.window_fill = palette.surface_container_high;
    visuals.window_stroke = Stroke::new(1.0_f32, palette.outline_variant);
    visuals.window_corner_radius = CornerRadius::same(28);
    visuals.menu_corner_radius = CornerRadius::same(12);
    visuals.window_shadow = Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 120 } else { 48 }),
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 3],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 96 } else { 40 }),
    };

    visuals.slider_trailing_fill = true;
    visuals.handle_shape = HandleShape::Circle;
    visuals.interact_cursor = Some(CursorIcon::PointingHand);
    visuals.disabled_alpha = 0.38;
    visuals.text_cursor = TextCursorStyle {
        stroke: Stroke::new(2.0_f32, palette.primary),
        ..Default::default()
    };

    visuals.widgets = widget_visuals(palette);
    visuals
}

fn widget_visuals(palette: &Palette) -> Widgets {
    let radius = CornerRadius::same(20);
    let quiet = Stroke::NONE;

    Widgets {
        noninteractive: WidgetVisuals {
            bg_fill: palette.surface,
            weak_bg_fill: palette.surface,
            bg_stroke: Stroke::new(1.0_f32, palette.outline_variant),
            corner_radius: CornerRadius::ZERO,
            fg_stroke: Stroke::new(1.0_f32, palette.on_surface),
            expansion: 0.0,
        },
        inactive: WidgetVisuals {
            bg_fill: palette.surface_container_high,
            weak_bg_fill: palette.surface_container_high,
            bg_stroke: quiet,
            corner_radius: radius,
            fg_stroke: Stroke::new(1.0_f32, palette.on_surface),
            expansion: 0.0,
        },
        hovered: WidgetVisuals {
            bg_fill: palette.surface_container_highest,
            weak_bg_fill: palette.surface_container_highest,
            bg_stroke: quiet,
            corner_radius: radius,
            fg_stroke: Stroke::new(1.0_f32, palette.on_surface),
            expansion: 0.0,
        },
        active: WidgetVisuals {
            bg_fill: palette.secondary_container,
            weak_bg_fill: palette.secondary_container,
            bg_stroke: quiet,
            corner_radius: radius,
            fg_stroke: Stroke::new(1.0_f32, palette.on_secondary_container),
            expansion: 0.0,
        },
        open: WidgetVisuals {
            bg_fill: palette.surface_container_highest,
            weak_bg_fill: palette.surface_container_highest,
            bg_stroke: quiet,
            corner_radius: radius,
            fg_stroke: Stroke::new(1.0_f32, palette.on_surface),
            expansion: 0.0,
        },
    }
}

/// Builds a [`TextStyle::Name`] key for a named style (`"title"`, `"label"`,
/// …). Kept here so [`install_style`] and the widget kit share one spelling.
#[must_use]
pub fn style_name(name: &'static str) -> TextStyle {
    TextStyle::Name(Arc::from(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::palette::Scheme;

    fn dark_palette() -> Palette {
        Palette::from_seed(Color32::from_rgb(0x67, 0x50, 0xA4), Scheme::Dark)
    }

    #[test]
    fn install_style_uses_palette_colors_and_m3_metrics() {
        let palette = dark_palette();
        let ctx = egui::Context::default();
        install_style(&ctx, &palette);

        let style = ctx.style();
        assert!(style.visuals.dark_mode);
        assert_eq!(style.visuals.panel_fill, palette.surface);
        assert_eq!(style.visuals.override_text_color, Some(palette.on_surface));
        assert_eq!(style.animation_time, 0.15);
        assert_eq!(style.spacing.item_spacing, Vec2::new(8.0, 6.0));
        assert_eq!(style.spacing.slider_rail_height, 4.0);
        assert_eq!(style.spacing.scroll.bar_width, 8.0);
        assert_eq!(
            style.text_styles.get(&TextStyle::Body),
            Some(&FontId::proportional(14.0))
        );
        assert_eq!(
            style.text_styles.get(&style_name("title")),
            Some(&FontId::proportional(22.0))
        );
    }

    #[test]
    fn light_scheme_switches_theme_and_surfaces() {
        let ctx = egui::Context::default();
        let dark = Palette::from_seed(Color32::from_rgb(0x00, 0x68, 0x74), Scheme::Dark);
        install_style(&ctx, &dark);
        let light = Palette::from_seed(Color32::from_rgb(0x00, 0x68, 0x74), Scheme::Light);
        install_style(&ctx, &light);

        let style = ctx.style();
        assert!(!style.visuals.dark_mode);
        assert_eq!(style.visuals.panel_fill, light.surface);
        assert_ne!(light.surface, dark.surface);
    }
}
