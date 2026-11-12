//! System CJK font registration.
//!
//! egui's bundled fonts stop at Latin, so Chinese labels need a system face.
//! The face is read on the boot worker ([`crate::boot`]) and installed once with
//! [`install`]; the bytes are moved into egui, never copied twice.

use std::sync::Arc;

use eframe::egui;

const CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\msyhl.ttc",
    r"C:\Windows\Fonts\simhei.ttf",
    r"C:\Windows\Fonts\Deng.ttf",
    r"C:\Windows\Fonts\simsun.ttc",
];

/// Name the face is registered under inside egui.
const FAMILY: &str = "system-cjk";

/// A CJK face read from the system font directory.
pub struct Face {
    path: String,
    bytes: Vec<u8>,
}

/// Reads the first available CJK face. Roughly 20 MB, so it runs off the UI
/// thread (see [`crate::boot`]).
#[must_use]
pub fn load_face() -> Option<Face> {
    // `MUXVIDEO_FONT` pins one face; used to measure parse/raster cost of the
    // lighter single-face TTF against the 19 MB collection.
    if let Some(custom) = std::env::var_os("MUXVIDEO_FONT") {
        let path = std::path::PathBuf::from(custom);
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(Face {
                path: path.display().to_string(),
                bytes,
            });
        }
    }
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            return Some(Face {
                path: (*path).to_owned(),
                bytes,
            });
        }
    }
    None
}

/// Registers `face` as the fallback family. Call once, before the style.
pub fn install(ctx: &egui::Context, face: Face) {
    if std::env::var_os("MUXVIDEO_NO_CJK").is_some() {
        crate::diag!("[boot] cjk face skipped by env");
        return;
    }
    let Face { path, bytes } = face;
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(FAMILY.to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push(FAMILY.to_owned());
    }
    ctx.set_fonts(fonts);
    crate::diag!("[boot] cjk face {path}");
}
