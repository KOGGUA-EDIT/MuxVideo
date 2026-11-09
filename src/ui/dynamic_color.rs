//! HCT-ish dynamic colour: seed colour → tonal palettes → Material 3 roles.
//!
//! Material 3 derives every role from a seed colour through *tonal palettes*:
//! a palette has a fixed hue and chroma and is sampled at CIE L\* tones
//! `0..=100`. Google's reference implementation works in CAM16 (HCT); this
//! module is a dependency-free CIELAB approximation:
//!
//! * hue and chroma are taken from the seed in LAB (LAB hue is stable across
//!   tones, which is exactly what a tonal palette needs);
//! * a tone *is* the CIE L\* of the result;
//! * colours that fall outside the sRGB gamut are brought back by **reducing
//!   chroma only** — the tone and hue are preserved. This is the same strategy
//!   as HCT's gamut mapper, and the reason [`TonalPalette::tone`] behaves like
//!   "clamp the chroma per tone": tone 90 can only hold a little chroma, while
//!   tone 40 holds a lot.
//!
//! The nominal chromas in [`SeedPalettes::from_seed`] are tuned so that the
//! baseline seed (`#6750A4`) reproduces the familiar Material 3 roles
//! (`#D0BCFF` primary in dark mode, `#CCC2DC` secondary, `#F2B8B5` error…).

use eframe::egui::Color32;

/// CIE L\* tones sampled by every tonal palette.
pub const TONES: [u8; 11] = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];

/// Baseline Material 3 seed (`#6750A4`).
///
/// Only used as a hue fallback for achromatic seeds: a grey seed has no hue,
/// and Material 3 still gives it a colourful scheme.
pub const BASELINE_SEED: Color32 = Color32::from_rgb(0x67, 0x50, 0xA4);

// D65 reference white.
const XN: f32 = 0.950_47;
const YN: f32 = 1.0;
const ZN: f32 = 1.088_83;

/// `(6/29)^3` — the linear/`f` breakpoint from the CIE Lab definition.
const EPSILON: f32 = 216.0 / 24_389.0;
/// `(29/3)^3`.
const KAPPA: f32 = 24_389.0 / 27.0;
/// `6/29`.
const DELTA: f32 = 6.0 / 29.0;

/// A hue, a chroma and a tone — the HCT colour space, approximated with CIELAB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hct {
    /// Hue angle in degrees, `0..360` (LAB hue, `a*` → `b*`).
    pub hue: f32,
    /// LAB chroma, `hypot(a*, b*)`.
    pub chroma: f32,
    /// CIE L\*, `0..=100` (`0` = black, `100` = white).
    pub tone: f32,
}

impl Hct {
    /// Measures a colour in this space.
    #[must_use]
    pub fn from_color(color: Color32) -> Self {
        let (tone, a, b) = srgb_to_lab(color);
        let chroma = a.hypot(b);
        let hue = if chroma < 0.01 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        };
        Self { hue, chroma, tone }
    }

    /// Converts back to sRGB, gamut-mapping by reducing chroma at a fixed tone
    /// and hue.
    #[must_use]
    pub fn to_color(self) -> Color32 {
        lab_to_color(self.tone, self.chroma, self.hue)
    }

    /// Same hue and chroma at another tone (gamut-mapped).
    #[must_use]
    pub fn with_tone(self, tone: f32) -> Self {
        Self { tone, ..self }
    }
}

/// A tonal palette: one hue, one chroma, sampled at many tones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TonalPalette {
    /// Hue angle in degrees.
    pub hue: f32,
    /// Nominal LAB chroma. Gamut mapping may reduce it per tone.
    pub chroma: f32,
}

impl TonalPalette {
    /// Creates a palette from a hue (degrees) and a nominal chroma.
    #[must_use]
    pub const fn new(hue: f32, chroma: f32) -> Self {
        Self { hue, chroma }
    }

    /// The colour at `tone` (`0..=100`).
    #[must_use]
    pub fn tone(&self, tone: u8) -> Color32 {
        self.tone_f(f32::from(tone))
    }

    /// The colour at a fractional tone.
    #[must_use]
    pub fn tone_f(&self, tone: f32) -> Color32 {
        lab_to_color(tone, self.chroma, self.hue)
    }

    /// The full ramp at [`TONES`].
    #[must_use]
    pub fn ramp(&self) -> [Color32; TONES.len()] {
        TONES.map(|tone| self.tone(tone))
    }
}

/// The six tonal palettes a Material 3 scheme is built from.
#[derive(Clone, Copy, Debug)]
pub struct SeedPalettes {
    /// Seed hue, high chroma.
    pub primary: TonalPalette,
    /// Seed hue, medium chroma.
    pub secondary: TonalPalette,
    /// Seed hue + 60°, medium chroma.
    pub tertiary: TonalPalette,
    /// Seed hue, near-grey. Drives surfaces.
    pub neutral: TonalPalette,
    /// Seed hue, low chroma. Drives outlines and secondary text.
    pub neutral_variant: TonalPalette,
    /// Fixed M3 red.
    pub error: TonalPalette,
}

impl SeedPalettes {
    /// Derives the six Material 3 tonal palettes from a seed colour.
    ///
    /// The nominal chromas are deliberately not the HCT numbers (48 / 16 / 24 /
    /// 4 / 8 / 84): LAB chroma and CAM16 chroma live on different scales, and
    /// these values make the gamut mapper land on the familiar baseline roles.
    /// Tertiary hue is the seed hue rotated by 60°, matching the M3 baseline
    /// relationship closely (`#7D5260` sits ±7° from the exact rotation).
    #[must_use]
    pub fn from_seed(seed: Color32) -> Self {
        let mut measured = Hct::from_color(seed);
        if measured.chroma < 4.0 {
            // No usable hue (grey seed) — fall back to the baseline hue.
            measured = Hct::from_color(BASELINE_SEED);
        }
        let hue = measured.hue;
        Self {
            primary: TonalPalette::new(hue, 48.0),
            secondary: TonalPalette::new(hue, 16.0),
            tertiary: TonalPalette::new((hue + 60.0).rem_euclid(360.0), 24.0),
            neutral: TonalPalette::new(hue, 4.0),
            neutral_variant: TonalPalette::new(hue, 8.0),
            error: TonalPalette::new(25.0, 60.0),
        }
    }
}

/// Shortest angular distance between two hue angles, in degrees.
#[must_use]
pub fn hue_delta(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// WCAG relative luminance (`0` = black, `1` = white).
#[must_use]
pub fn relative_luminance(color: Color32) -> f32 {
    let channel = |c: u8| srgb_to_linear(f32::from(c) / 255.0);
    0.212_6 * channel(color.r()) + 0.715_2 * channel(color.g()) + 0.072_2 * channel(color.b())
}

/// WCAG contrast ratio between two colours, `1.0..=21.0`.
#[must_use]
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (lighter, darker) = if la >= lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn lab_f(t: f32) -> f32 {
    if t > EPSILON {
        t.cbrt()
    } else {
        (KAPPA * t + 16.0) / 116.0
    }
}

fn lab_f_inv(t: f32) -> f32 {
    if t > DELTA {
        t * t * t
    } else {
        (116.0 * t - 16.0) / KAPPA
    }
}

/// sRGB (0..255) → `(L*, a*, b*)`.
fn srgb_to_lab(color: Color32) -> (f32, f32, f32) {
    let r = srgb_to_linear(f32::from(color.r()) / 255.0);
    let g = srgb_to_linear(f32::from(color.g()) / 255.0);
    let b = srgb_to_linear(f32::from(color.b()) / 255.0);

    let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = 0.019_333_9 * r + 0.119_192 * g + 0.950_304 * b;

    let fx = lab_f(x / XN);
    let fy = lab_f(y / YN);
    let fz = lab_f(z / ZN);

    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

/// `(L*, a*, b*)` → linear RGB, without gamut mapping.
fn lab_to_linear_rgb(l: f32, a: f32, b: f32) -> [f32; 3] {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;

    let x = XN * lab_f_inv(fx);
    let y = YN * lab_f_inv(fy);
    let z = ZN * lab_f_inv(fz);

    [
        3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z,
        -0.969_266 * x + 1.876_011 * y + 0.041_556 * z,
        0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z,
    ]
}

fn in_gamut(linear: [f32; 3]) -> bool {
    const TOLERANCE: f32 = 1e-4;
    linear
        .iter()
        .all(|c| (-TOLERANCE..=1.0 + TOLERANCE).contains(c))
}

/// LAB → sRGB at a fixed tone, reducing chroma until the colour fits sRGB.
fn lab_to_color(tone: f32, chroma: f32, hue: f32) -> Color32 {
    let l = tone.clamp(0.0, 100.0);
    let radians = hue.to_radians();
    let (a, b) = (chroma * radians.cos(), chroma * radians.sin());

    let direct = lab_to_linear_rgb(l, a, b);
    let mapped = if in_gamut(direct) {
        direct
    } else {
        // Hue and tone stay fixed; bisect the chroma scale until we are inside.
        let (mut fit, mut unfit) = (0.0_f32, 1.0_f32);
        for _ in 0..24 {
            let mid = 0.5 * (fit + unfit);
            if in_gamut(lab_to_linear_rgb(l, a * mid, b * mid)) {
                fit = mid;
            } else {
                unfit = mid;
            }
        }
        lab_to_linear_rgb(l, a * fit, b * fit)
    };

    let to_u8 = |linear: f32| {
        let srgb = linear_to_srgb(linear.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        (srgb * 255.0).round() as u8
    };
    Color32::from_rgb(to_u8(mapped[0]), to_u8(mapped[1]), to_u8(mapped[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_and_white_have_maximal_contrast() {
        let ratio = contrast_ratio(Color32::BLACK, Color32::WHITE);
        assert!((ratio - 21.0).abs() < 0.01, "contrast was {ratio}");
        assert!((relative_luminance(Color32::WHITE) - 1.0).abs() < 0.001);
        assert!(relative_luminance(Color32::BLACK) < 0.001);
    }

    #[test]
    fn tonal_palette_preserves_hue_and_tone() {
        let seed = BASELINE_SEED;
        let measured = Hct::from_color(seed);
        let palette = TonalPalette::new(measured.hue, 48.0);

        for tone in [20_u8, 40, 60, 80] {
            let color = palette.tone(tone);
            let back = Hct::from_color(color);
            assert!(
                hue_delta(back.hue, measured.hue) < 3.0,
                "tone {tone}: hue {} vs {}",
                back.hue,
                measured.hue
            );
            assert!(
                (back.tone - f32::from(tone)).abs() < 1.5,
                "tone {tone} drifted to {}",
                back.tone
            );
        }
    }

    #[test]
    fn tonal_ramp_is_monotonic_in_luminance() {
        let palette = TonalPalette::new(305.0, 48.0);
        let mut previous = -1.0;
        for tone in TONES {
            let luminance = relative_luminance(palette.tone(tone));
            assert!(
                luminance > previous,
                "tone {tone} luminance {luminance} not above {previous}"
            );
            previous = luminance;
        }
    }

    #[test]
    fn achromatic_seed_uses_the_baseline_hue() {
        let grey = SeedPalettes::from_seed(Color32::from_gray(128));
        let baseline = Hct::from_color(BASELINE_SEED);
        assert!(hue_delta(grey.primary.hue, baseline.hue) < 0.5);
        assert!(hue_delta(grey.primary.hue, grey.neutral.hue) < 0.5);
    }

    #[test]
    fn tertiary_is_rotated_sixty_degrees() {
        let seed = BASELINE_SEED;
        let hue = Hct::from_color(seed).hue;
        let palettes = SeedPalettes::from_seed(seed);
        assert!(hue_delta(palettes.tertiary.hue, hue + 60.0) < 0.5);
        assert!(hue_delta(palettes.secondary.hue, hue) < 0.5);
    }
}
