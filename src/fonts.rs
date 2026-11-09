//! System CJK font registration.
//!
//! egui's bundled fonts stop at Latin, so Chinese labels need a system face.
//! We load Microsoft YaHei (or a fallback) once, straight from `%WINDIR%\Fonts`.

use std::sync::OnceLock;

use eframe::egui;

const CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\msyhl.ttc",
    r"C:\Windows\Fonts\simhei.ttf",
    r"C:\Windows\Fonts\Deng.ttf",
    r"C:\Windows\Fonts\simsun.ttc",
];

static LOADED: OnceLock<Option<(String, Vec<u8>)>> = OnceLock::new();

/// Registers a CJK font as a fallback family. Safe to call more than once.
pub fn install(ctx: &egui::Context) {
    let found = LOADED.get_or_init(|| {
        for path in CANDIDATES {
            if let Ok(bytes) = std::fs::read(path) {
                return Some(((*path).to_owned(), bytes));
            }
        }
        None
    });

    let Some((path, bytes)) = found else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    let name = "system-cjk".to_owned();
    fonts.font_data.insert(
        name.clone(),
        std::sync::Arc::new(egui::FontData::from_owned(bytes.clone())),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push(name.clone());
    }
    ctx.set_fonts(fonts);

    tracing_note(path);
}

fn tracing_note(_path: &str) {}
