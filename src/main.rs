//! MuxVideo — Material You video comparison & viewer on top of mpv.

fn main() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title("MuxVideo")
            .with_decorations(false)
            .with_transparent(false),
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
