//! Public UI surface: Material You palette + widget kit.
//!
//! `dynamic_color` generates tonal palettes from a seed, `palette` maps them to
//! the Material 3 colour roles, `kit` installs the egui style and `widgets`
//! holds the reusable controls. The app consumes [`Palette`],
//! [`install_style`] and [`widgets`].

pub mod dynamic_color;
pub mod kit;
pub mod palette;
pub mod widgets;

pub use dynamic_color::{Hct, SeedPalettes, TonalPalette};
pub use kit::install_style;
pub use palette::{Palette, Role, Scheme};
