//! Install the Material You look into an egui context.
//!
//! Placeholder: the theme track replaces this with the real style (typography
//! scale, shapes, spacing, animation timings, scrollbar and selection colours).

use eframe::egui;

use crate::ui::Palette;

/// Applies the Material You egui style for `palette`.
pub fn install_style(ctx: &egui::Context, palette: &Palette) {
    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = matches!(palette.scheme, crate::ui::Scheme::Dark);
    style.visuals.panel_fill = palette.surface;
    style.visuals.override_text_color = Some(palette.on_surface);
    ctx.set_style(style);
}
