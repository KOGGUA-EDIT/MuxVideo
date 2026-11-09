//! Public UI surface: Material You palette + widget kit.
//!
//! `theme` holds the dynamic colour system, `widgets` the reusable controls,
//! `kit` the egui style installation. Implemented by the theme track; the app
//! consumes [`ui::Palette`], [`ui::install_style`] and [`ui::widgets`].

pub mod kit;
pub mod palette;
pub mod widgets;

pub use kit::install_style;
pub use palette::{Palette, Scheme};
