//! Reusable Material You widgets.
//!
//! Placeholder: the theme track replaces this with filled / tonal / outlined
//! button families, segmented buttons, navigation-rail items, cards, sliders,
//! chips, snackbars and the type scale helpers.

use eframe::egui;

use crate::ui::Palette;

/// A filled Material You button.
pub fn filled_button(ui: &mut egui::Ui, palette: &Palette, label: &str) -> egui::Response {
    let button = egui::Button::new(egui::RichText::new(label).color(palette.on_primary))
        .fill(palette.primary)
        .corner_radius(egui::CornerRadius::same(20));
    ui.add(button)
}

/// A tonal Material You button.
pub fn tonal_button(ui: &mut egui::Ui, palette: &Palette, label: &str) -> egui::Response {
    let button = egui::Button::new(egui::RichText::new(label).color(palette.on_secondary_container))
        .fill(palette.secondary_container)
        .corner_radius(egui::CornerRadius::same(20));
    ui.add(button)
}
