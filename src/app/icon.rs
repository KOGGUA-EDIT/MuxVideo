//! App icon: the artwork lives in `图标.svg`, baked to PNGs under `assets/`.
//!
//! The same picture is used in three places: the OS window icon (taskbar /
//! alt-tab), the title-bar brand block, and — through `build.rs` + `windres` —
//! the file icon Explorer shows for `muxvideo.exe`.

use eframe::egui;

/// Baked from `图标.svg` (576 px master, downscaled with Lanczos).
const ICON_PNG: &[u8] = include_bytes!("../../assets/icon-256.png");

/// Decodes the embedded PNG into the straight-RGBA form egui wants.
fn decode() -> Option<(usize, usize, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::Cursor::new(ICON_PNG));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => {
            buf.truncate(w * h * 4);
            buf
        }
        _ => {
            // Defensive: the bake is RGBA, but widen anything else.
            let mut out = Vec::with_capacity(w * h * 4);
            for px in buf.chunks(4) {
                let a = if info.color_type == png::ColorType::Rgb {
                    255
                } else {
                    px[3]
                };
                out.extend_from_slice(&[px[0], px[1], px[2], a]);
            }
            out
        }
    };
    Some((w, h, rgba))
}

/// Window icon for `ViewportBuilder::with_icon` (taskbar, alt-tab, title bar).
pub fn window_icon() -> Option<egui::IconData> {
    let (width, height, rgba) = decode()?;
    Some(egui::IconData {
        rgba,
        width: width as u32,
        height: height as u32,
    })
}

/// Texture for the title-bar brand block.
pub fn brand_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let (w, h, rgba) = decode()?;
    let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
    Some(ctx.load_texture("muxvideo-brand", image, egui::TextureOptions::LINEAR))
}
