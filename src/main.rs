#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod domain;
mod io;
mod mcp;

use eframe::egui;

fn main() -> eframe::Result {
    // `trazo --mcp-stdio`: no window of its own; it serves MCP on
    // stdin/stdout for clients such as Claude Desktop.
    if std::env::args().skip(1).any(|argument| argument == "--mcp-stdio") {
        mcp::bridge::run(mcp::port());
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Trazo")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([1000.0, 600.0])
            .with_maximized(true),
        ..Default::default()
    };

    eframe::run_native(
        "Trazo",
        options,
        Box::new(|context| Ok(Box::new(app::FloorPlanApp::new(&context.egui_ctx)))),
    )
}
