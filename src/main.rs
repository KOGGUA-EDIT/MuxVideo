//! MuxVideo — Material You video comparison & viewer on top of mpv.

// Release builds are a GUI app; debug builds keep the console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 600.0])
        .with_title("MuxVideo")
        .with_decorations(false)
        .with_transparent(false);
    if let Some(icon) = muxvideo::app::icon::window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "MuxVideo",
        options,
        Box::new(|cc| Ok(Box::new(muxvideo::app::MuxApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}
