//! Material 3 colour roles for MuxVideo.
//!
//! [`Palette::from_seed`] turns one seed colour into a complete set of Material
//! 3 colour roles, for either the dark or the light scheme. Every role is a
//! specific *tone* (CIE L\*) of one of the six tonal palettes in
//! [`crate::ui::dynamic_color::SeedPalettes`]:
//!
//! | role family | palette | dark tones | light tones |
//! |---|---|---|---|
//! | `primary` / `on_primary` / `primary_container` / `on_primary_container` | primary | 80 / 20 / 30 / 90 | 40 / 100 / 90 / 10 |
//! | `secondary*`, `tertiary*`, `error*` | same pattern | 80 / 20 / 30 / 90 | 40 / 100 / 90 / 10 |
//! | `surface` / `on_surface` | neutral | 6 / 90 | 98 / 10 |
//! | `surface_container_lowest … highest` | neutral | 4 / 10 / 12 / 17 / 22 | 100 / 96 / 94 / 92 / 90 |
//! | `surface_variant` / `on_surface_variant` | neutral-variant | 30 / 80 | 90 / 30 |
//! | `outline` / `outline_variant` | neutral-variant | 60 / 30 | 50 / 80 |
//! | `inverse_surface` / `inverse_on_surface` | neutral | 90 / 20 | 20 / 95 |
//!
//! The tone numbers are the canonical Material 3 mapping. Tone `0` is black and
//! tone `100` is white; "on" roles are far from their container in tone, which
//! is what keeps them above 4.5:1 contrast.

use eframe::egui::Color32;

use super::dynamic_color::{SeedPalettes, TonalPalette};

/// Dark or light Material 3 scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    /// Dark scheme: low-tone surfaces, high-tone accents.
    Dark,
    /// Light scheme: high-tone surfaces, low-tone accents.
    Light,
}

impl Scheme {
    /// Both schemes, dark first.
    pub const ALL: [Self; 2] = [Self::Dark, Self::Light];

    /// Whether this is the dark scheme.
    #[must_use]
    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    /// English label, useful for menus and the showcase.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }

    /// The other scheme.
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

/// One Material 3 colour role.
///
/// [`Role::ALL`] lists every role exactly once, in the order the Material 3
/// spec documents them. Use [`Role::color`] to read a role from a palette, or
/// [`Palette::role`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Highest-emphasis accent.
    Primary,
    /// Content on `primary`.
    OnPrimary,
    /// Lower-emphasis accent container.
    PrimaryContainer,
    /// Content on `primary_container`.
    OnPrimaryContainer,
    /// Muted accent.
    Secondary,
    /// Content on `secondary`.
    OnSecondary,
    /// Muted accent container.
    SecondaryContainer,
    /// Content on `secondary_container`.
    OnSecondaryContainer,
    /// Contrasting third accent.
    Tertiary,
    /// Content on `tertiary`.
    OnTertiary,
    /// Contrasting third accent container.
    TertiaryContainer,
    /// Content on `tertiary_container`.
    OnTertiaryContainer,
    /// Error accent.
    Error,
    /// Content on `error`.
    OnError,
    /// Error container.
    ErrorContainer,
    /// Content on `error_container`.
    OnErrorContainer,
    /// Page background.
    Surface,
    /// Primary text on `surface`.
    OnSurface,
    /// Lower-emphasis surface (inputs, chips).
    SurfaceVariant,
    /// Secondary text on `surface`.
    OnSurfaceVariant,
    /// Dimmed background (window in the background).
    SurfaceDim,
    /// Brightened background (elevated sheet).
    SurfaceBright,
    /// Lowest surface container.
    SurfaceContainerLowest,
    /// Low surface container.
    SurfaceContainerLow,
    /// Default surface container (cards).
    SurfaceContainer,
    /// High surface container (dialogs, elevated cards).
    SurfaceContainerHigh,
    /// Highest surface container.
    SurfaceContainerHighest,
    /// Strong separator (focused input outline).
    Outline,
    /// Subtle separator (card border, dividers).
    OutlineVariant,
    /// Inverted background (snackbars, tooltips).
    InverseSurface,
    /// Content on `inverse_surface`.
    InverseOnSurface,
    /// Accent on `inverse_surface`.
    InversePrimary,
    /// Tint colour used to tint elevated surfaces.
    SurfaceTint,
    /// Shadow colour (pure black).
    Shadow,
    /// Scrim colour (pure black).
    Scrim,
}

impl Role {
    /// Every role, in Material 3 order.
    pub const ALL: [Self; 35] = [
        Self::Primary,
        Self::OnPrimary,
        Self::PrimaryContainer,
        Self::OnPrimaryContainer,
        Self::Secondary,
        Self::OnSecondary,
        Self::SecondaryContainer,
        Self::OnSecondaryContainer,
        Self::Tertiary,
        Self::OnTertiary,
        Self::TertiaryContainer,
        Self::OnTertiaryContainer,
        Self::Error,
        Self::OnError,
        Self::ErrorContainer,
        Self::OnErrorContainer,
        Self::Surface,
        Self::OnSurface,
        Self::SurfaceVariant,
        Self::OnSurfaceVariant,
        Self::SurfaceDim,
        Self::SurfaceBright,
        Self::SurfaceContainerLowest,
        Self::SurfaceContainerLow,
        Self::SurfaceContainer,
        Self::SurfaceContainerHigh,
        Self::SurfaceContainerHighest,
        Self::Outline,
        Self::OutlineVariant,
        Self::InverseSurface,
        Self::InverseOnSurface,
        Self::InversePrimary,
        Self::SurfaceTint,
        Self::Shadow,
        Self::Scrim,
    ];

    /// The Material 3 role name (`"on_primary_container"`, …).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::OnPrimary => "on_primary",
            Self::PrimaryContainer => "primary_container",
            Self::OnPrimaryContainer => "on_primary_container",
            Self::Secondary => "secondary",
            Self::OnSecondary => "on_secondary",
            Self::SecondaryContainer => "secondary_container",
            Self::OnSecondaryContainer => "on_secondary_container",
            Self::Tertiary => "tertiary",
            Self::OnTertiary => "on_tertiary",
            Self::TertiaryContainer => "tertiary_container",
            Self::OnTertiaryContainer => "on_tertiary_container",
            Self::Error => "error",
            Self::OnError => "on_error",
            Self::ErrorContainer => "error_container",
            Self::OnErrorContainer => "on_error_container",
            Self::Surface => "surface",
            Self::OnSurface => "on_surface",
            Self::SurfaceVariant => "surface_variant",
            Self::OnSurfaceVariant => "on_surface_variant",
            Self::SurfaceDim => "surface_dim",
            Self::SurfaceBright => "surface_bright",
            Self::SurfaceContainerLowest => "surface_container_lowest",
            Self::SurfaceContainerLow => "surface_container_low",
            Self::SurfaceContainer => "surface_container",
            Self::SurfaceContainerHigh => "surface_container_high",
            Self::SurfaceContainerHighest => "surface_container_highest",
            Self::Outline => "outline",
            Self::OutlineVariant => "outline_variant",
            Self::InverseSurface => "inverse_surface",
            Self::InverseOnSurface => "inverse_on_surface",
            Self::InversePrimary => "inverse_primary",
            Self::SurfaceTint => "surface_tint",
            Self::Shadow => "shadow",
            Self::Scrim => "scrim",
        }
    }

    /// Resolves the role against a palette.
    #[must_use]
    pub fn color(self, palette: &Palette) -> Color32 {
        self.resolve(&palette.palettes, palette.scheme)
    }

    fn resolve(self, palettes: &SeedPalettes, scheme: Scheme) -> Color32 {
        let dark = scheme.is_dark();
        let primary = palettes.primary;
        let secondary = palettes.secondary;
        let tertiary = palettes.tertiary;
        let neutral = palettes.neutral;
        let neutral_variant = palettes.neutral_variant;
        let error = palettes.error;

        match self {
            Self::Primary => primary.tone(if dark { 80 } else { 40 }),
            Self::OnPrimary => primary.tone(if dark { 20 } else { 100 }),
            Self::PrimaryContainer => primary.tone(if dark { 30 } else { 90 }),
            Self::OnPrimaryContainer => primary.tone(if dark { 90 } else { 10 }),

            Self::Secondary => secondary.tone(if dark { 80 } else { 40 }),
            Self::OnSecondary => secondary.tone(if dark { 20 } else { 100 }),
            Self::SecondaryContainer => secondary.tone(if dark { 30 } else { 90 }),
            Self::OnSecondaryContainer => secondary.tone(if dark { 90 } else { 10 }),

            Self::Tertiary => tertiary.tone(if dark { 80 } else { 40 }),
            Self::OnTertiary => tertiary.tone(if dark { 20 } else { 100 }),
            Self::TertiaryContainer => tertiary.tone(if dark { 30 } else { 90 }),
            Self::OnTertiaryContainer => tertiary.tone(if dark { 90 } else { 10 }),

            Self::Error => error.tone(if dark { 80 } else { 40 }),
            Self::OnError => error.tone(if dark { 20 } else { 100 }),
            Self::ErrorContainer => error.tone(if dark { 30 } else { 90 }),
            Self::OnErrorContainer => error.tone(if dark { 90 } else { 10 }),

            Self::Surface => neutral.tone(if dark { 6 } else { 98 }),
            Self::OnSurface => neutral.tone(if dark { 90 } else { 10 }),
            Self::SurfaceVariant => neutral_variant.tone(if dark { 30 } else { 90 }),
            Self::OnSurfaceVariant => neutral_variant.tone(if dark { 80 } else { 30 }),
            Self::SurfaceDim => neutral.tone(if dark { 6 } else { 87 }),
            Self::SurfaceBright => neutral.tone(if dark { 24 } else { 98 }),
            Self::SurfaceContainerLowest => neutral.tone(if dark { 4 } else { 100 }),
            Self::SurfaceContainerLow => neutral.tone(if dark { 10 } else { 96 }),
            Self::SurfaceContainer => neutral.tone(if dark { 12 } else { 94 }),
            Self::SurfaceContainerHigh => neutral.tone(if dark { 17 } else { 92 }),
            Self::SurfaceContainerHighest => neutral.tone(if dark { 22 } else { 90 }),

            Self::Outline => neutral_variant.tone(if dark { 60 } else { 50 }),
            Self::OutlineVariant => neutral_variant.tone(if dark { 30 } else { 80 }),

            Self::InverseSurface => neutral.tone(if dark { 90 } else { 20 }),
            Self::InverseOnSurface => neutral.tone(if dark { 20 } else { 95 }),
            Self::InversePrimary => primary.tone(if dark { 40 } else { 80 }),

            Self::SurfaceTint => primary.tone(if dark { 80 } else { 40 }),
            Self::Shadow | Self::Scrim => Color32::BLACK,
        }
    }
}

/// A complete Material 3 colour scheme, generated from a seed colour.
///
/// Build one with [`Palette::from_seed`]; read individual roles through
/// [`Palette::role`] or the public fields. The [`Role`] enum is convenient for
/// generic UI (swatch grids, theme editors).
#[derive(Clone, Debug)]
pub struct Palette {
    /// The seed this palette was generated from.
    pub seed: Color32,
    /// Dark or light.
    pub scheme: Scheme,
    /// The six tonal palettes; `ramp()` feeds tone pickers.
    pub palettes: SeedPalettes,

    /// Highest-emphasis accent.
    pub primary: Color32,
    /// Content on `primary`.
    pub on_primary: Color32,
    /// Lower-emphasis accent container.
    pub primary_container: Color32,
    /// Content on `primary_container`.
    pub on_primary_container: Color32,

    /// Muted accent.
    pub secondary: Color32,
    /// Content on `secondary`.
    pub on_secondary: Color32,
    /// Muted accent container.
    pub secondary_container: Color32,
    /// Content on `secondary_container`.
    pub on_secondary_container: Color32,

    /// Contrasting third accent.
    pub tertiary: Color32,
    /// Content on `tertiary`.
    pub on_tertiary: Color32,
    /// Contrasting third accent container.
    pub tertiary_container: Color32,
    /// Content on `tertiary_container`.
    pub on_tertiary_container: Color32,

    /// Error accent.
    pub error: Color32,
    /// Content on `error`.
    pub on_error: Color32,
    /// Error container.
    pub error_container: Color32,
    /// Content on `error_container`.
    pub on_error_container: Color32,

    /// Page background.
    pub surface: Color32,
    /// Primary text on `surface`.
    pub on_surface: Color32,
    /// Lower-emphasis surface (inputs, chips).
    pub surface_variant: Color32,
    /// Secondary text on `surface`.
    pub on_surface_variant: Color32,
    /// Dimmed background.
    pub surface_dim: Color32,
    /// Brightened background.
    pub surface_bright: Color32,
    /// Lowest surface container.
    pub surface_container_lowest: Color32,
    /// Low surface container.
    pub surface_container_low: Color32,
    /// Default surface container (cards).
    pub surface_container: Color32,
    /// High surface container (dialogs, elevated cards).
    pub surface_container_high: Color32,
    /// Highest surface container.
    pub surface_container_highest: Color32,

    /// Strong separator.
    pub outline: Color32,
    /// Subtle separator.
    pub outline_variant: Color32,

    /// Inverted background (snackbars, tooltips).
    pub inverse_surface: Color32,
    /// Content on `inverse_surface`.
    pub inverse_on_surface: Color32,
    /// Accent on `inverse_surface`.
    pub inverse_primary: Color32,

    /// Tint for elevated surfaces.
    pub surface_tint: Color32,
    /// Shadow colour (pure black).
    pub shadow: Color32,
    /// Scrim colour (pure black).
    pub scrim: Color32,
}

impl Palette {
    /// Generates the full role set for `scheme` from `seed`.
    ///
    /// Both schemes keep the seed hue; only the tones swap. A grey seed has no
    /// hue, so it falls back to the Material 3 baseline hue (see
    /// [`SeedPalettes::from_seed`]).
    #[must_use]
    pub fn from_seed(seed: Color32, scheme: Scheme) -> Self {
        let palettes = SeedPalettes::from_seed(seed);
        let palettes_for_role = palettes;
        let color = |role: Role| role.resolve(&palettes_for_role, scheme);

        Self {
            seed,
            scheme,
            palettes,
            primary: color(Role::Primary),
            on_primary: color(Role::OnPrimary),
            primary_container: color(Role::PrimaryContainer),
            on_primary_container: color(Role::OnPrimaryContainer),
            secondary: color(Role::Secondary),
            on_secondary: color(Role::OnSecondary),
            secondary_container: color(Role::SecondaryContainer),
            on_secondary_container: color(Role::OnSecondaryContainer),
            tertiary: color(Role::Tertiary),
            on_tertiary: color(Role::OnTertiary),
            tertiary_container: color(Role::TertiaryContainer),
            on_tertiary_container: color(Role::OnTertiaryContainer),
            error: color(Role::Error),
            on_error: color(Role::OnError),
            error_container: color(Role::ErrorContainer),
            on_error_container: color(Role::OnErrorContainer),
            surface: color(Role::Surface),
            on_surface: color(Role::OnSurface),
            surface_variant: color(Role::SurfaceVariant),
            on_surface_variant: color(Role::OnSurfaceVariant),
            surface_dim: color(Role::SurfaceDim),
            surface_bright: color(Role::SurfaceBright),
            surface_container_lowest: color(Role::SurfaceContainerLowest),
            surface_container_low: color(Role::SurfaceContainerLow),
            surface_container: color(Role::SurfaceContainer),
            surface_container_high: color(Role::SurfaceContainerHigh),
            surface_container_highest: color(Role::SurfaceContainerHighest),
            outline: color(Role::Outline),
            outline_variant: color(Role::OutlineVariant),
            inverse_surface: color(Role::InverseSurface),
            inverse_on_surface: color(Role::InverseOnSurface),
            inverse_primary: color(Role::InversePrimary),
            surface_tint: color(Role::SurfaceTint),
            shadow: color(Role::Shadow),
            scrim: color(Role::Scrim),
        }
    }

    /// Reads a role by its enum value.
    #[must_use]
    pub fn role(&self, role: Role) -> Color32 {
        role.color(self)
    }

    /// Whether this is the dark scheme.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.scheme.is_dark()
    }

    /// The tonal palette behind the primary family (useful for tone pickers).
    #[must_use]
    pub fn tonal(&self, palette: TonalPalette) -> TonalPalette {
        palette
    }

    /// `color` with the given opacity (`0.0..=1.0`), for state layers and
    /// selections.
    #[must_use]
    pub fn alpha(color: Color32, opacity: f32) -> Color32 {
        let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::dynamic_color::{Hct, contrast_ratio, hue_delta, relative_luminance};

    const SEEDS: [Color32; 5] = [
        Color32::from_rgb(0x67, 0x50, 0xA4), // baseline violet
        Color32::from_rgb(0x00, 0x68, 0x74), // teal
        Color32::from_rgb(0x7D, 0x52, 0x60), // rose
        Color32::from_rgb(0x4C, 0x66, 0x2B), // moss
        Color32::from_rgb(0x8C, 0x4A, 0x60), // berry
    ];

    fn schemes() -> [Scheme; 2] {
        [Scheme::Dark, Scheme::Light]
    }

    #[test]
    fn role_set_is_complete() {
        const EXPECTED: [&str; 35] = [
            "primary",
            "on_primary",
            "primary_container",
            "on_primary_container",
            "secondary",
            "on_secondary",
            "secondary_container",
            "on_secondary_container",
            "tertiary",
            "on_tertiary",
            "tertiary_container",
            "on_tertiary_container",
            "error",
            "on_error",
            "error_container",
            "on_error_container",
            "surface",
            "on_surface",
            "surface_variant",
            "on_surface_variant",
            "surface_dim",
            "surface_bright",
            "surface_container_lowest",
            "surface_container_low",
            "surface_container",
            "surface_container_high",
            "surface_container_highest",
            "outline",
            "outline_variant",
            "inverse_surface",
            "inverse_on_surface",
            "inverse_primary",
            "surface_tint",
            "shadow",
            "scrim",
        ];

        assert_eq!(Role::ALL.len(), EXPECTED.len());
        for name in EXPECTED {
            assert!(
                Role::ALL.iter().any(|role| role.name() == name),
                "role `{name}` is missing from Role::ALL"
            );
        }

        for seed in SEEDS {
            for scheme in schemes() {
                let palette = Palette::from_seed(seed, scheme);
                for role in Role::ALL {
                    assert_eq!(
                        role.color(&palette).a(),
                        255,
                        "{} / {scheme:?} should be opaque",
                        role.name()
                    );
                }
            }
        }
    }

    #[test]
    fn dark_surface_is_dark_and_containers_steepen() {
        for seed in SEEDS {
            let palette = Palette::from_seed(seed, Scheme::Dark);
            assert!(
                relative_luminance(palette.surface) < 0.05,
                "dark surface {:?} is too bright",
                palette.surface
            );
            let ramps = [
                palette.surface_container_lowest,
                palette.surface_container_low,
                palette.surface_container,
                palette.surface_container_high,
                palette.surface_container_highest,
            ];
            for pair in ramps.windows(2) {
                assert!(
                    relative_luminance(pair[0]) < relative_luminance(pair[1]),
                    "container tones are not ordered: {pair:?}"
                );
            }
        }
    }

    #[test]
    fn light_surface_is_light_and_containers_soften() {
        for seed in SEEDS {
            let palette = Palette::from_seed(seed, Scheme::Light);
            assert!(
                relative_luminance(palette.surface) > 0.7,
                "light surface {:?} is too dark",
                palette.surface
            );
            let ramps = [
                palette.surface_container_lowest,
                palette.surface_container_low,
                palette.surface_container,
                palette.surface_container_high,
                palette.surface_container_highest,
            ];
            for pair in ramps.windows(2) {
                assert!(
                    relative_luminance(pair[0]) > relative_luminance(pair[1]),
                    "container tones are not ordered: {pair:?}"
                );
            }
        }
    }

    #[test]
    fn on_colors_meet_contrast_requirements() {
        for seed in SEEDS {
            for scheme in schemes() {
                let palette = Palette::from_seed(seed, scheme);
                let pairs = [
                    ("primary", palette.primary, palette.on_primary),
                    (
                        "primary_container",
                        palette.primary_container,
                        palette.on_primary_container,
                    ),
                    ("secondary", palette.secondary, palette.on_secondary),
                    (
                        "secondary_container",
                        palette.secondary_container,
                        palette.on_secondary_container,
                    ),
                    ("tertiary", palette.tertiary, palette.on_tertiary),
                    (
                        "tertiary_container",
                        palette.tertiary_container,
                        palette.on_tertiary_container,
                    ),
                    ("error", palette.error, palette.on_error),
                    (
                        "error_container",
                        palette.error_container,
                        palette.on_error_container,
                    ),
                    ("surface", palette.surface, palette.on_surface),
                    (
                        "surface_variant",
                        palette.surface_variant,
                        palette.on_surface_variant,
                    ),
                    (
                        "inverse_surface",
                        palette.inverse_surface,
                        palette.inverse_on_surface,
                    ),
                ];
                for (name, background, foreground) in pairs {
                    let ratio = contrast_ratio(background, foreground);
                    assert!(
                        ratio >= 4.5,
                        "{name} / on_{name} in {scheme:?} seeded {seed:?} has {ratio:.2}:1"
                    );
                }
            }
        }
    }

    #[test]
    fn hue_is_preserved_for_primary_and_secondary() {
        for seed in SEEDS {
            let seed_hue = Hct::from_color(seed).hue;
            for scheme in schemes() {
                let palette = Palette::from_seed(seed, scheme);
                for (name, color) in [
                    ("primary", palette.primary),
                    ("secondary", palette.secondary),
                ] {
                    let hue = Hct::from_color(color).hue;
                    assert!(
                        hue_delta(hue, seed_hue) < 4.0,
                        "{name} hue {hue} drifted from seed hue {seed_hue} ({scheme:?})"
                    );
                }
            }
        }
    }

    #[test]
    fn schemes_and_seeds_are_distinct() {
        let seed = SEEDS[0];
        let dark = Palette::from_seed(seed, Scheme::Dark);
        let light = Palette::from_seed(seed, Scheme::Light);
        assert_ne!(dark.surface, light.surface);
        assert_ne!(dark.primary, light.primary);

        let other = Palette::from_seed(SEEDS[1], Scheme::Dark);
        assert_ne!(dark.primary, other.primary);
    }

    #[test]
    fn palette_roles_agree_with_the_tonal_ramp() {
        let palette = Palette::from_seed(SEEDS[0], Scheme::Dark);
        assert_eq!(palette.primary, palette.palettes.primary.tone(80));
        assert_eq!(palette.surface, palette.palettes.neutral.tone(6));
        assert_eq!(palette.outline, palette.palettes.neutral_variant.tone(60));
        assert_eq!(palette.primary, palette.role(Role::Primary));
    }
}
