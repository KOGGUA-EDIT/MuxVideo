//! MuxVideo — Material You video comparison & viewer on top of mpv.

// Release builds are a GUI app; debug builds keep the console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    // Start the boot stopwatch before anything else; every `[boot]` line in
    // app-debug.log is an offset from this point.
    muxvideo::app::diag::boot_start();
    muxvideo::app::diag::mark("main");

    // Settings, the CJK face and libmpv are fetched while winit builds the
    // window and the GL context (see `boot`).
    let boot = muxvideo::boot::Boot::start();

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 600.0])
        .with_title("MuxVideo")
        .with_decorations(false)
        .with_transparent(false);
    if let Some(icon) = muxvideo::app::icon::window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    muxvideo::app::diag::mark("viewport ready");
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        vsync: false,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        ..Default::default()
    };
    eframe::run_native(
        "MuxVideo",
        options,
        Box::new(move |cc| Ok(Box::new(muxvideo::app::MuxApp::new(cc, boot)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}
