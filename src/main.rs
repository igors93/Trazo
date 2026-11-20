#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod domain;
mod io;
mod mcp;

use eframe::egui;

fn main() -> eframe::Result {
    io::crash_log::install_panic_hook();

    // `trazo --mcp-stdio`: no window of its own; it serves MCP on
    // stdin/stdout for clients such as Claude Desktop.
    if std::env::args()
        .skip(1)
        .any(|argument| argument == "--mcp-stdio")
    {
        mcp::bridge::run(mcp::port());
        return Ok(());
    }

    let initial_file = io::updates::parse_startup_file(std::env::args_os());

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
        Box::new(move |context| {
            let app = match initial_file {
                Some(path) => app::FloorPlanApp::new_with_file(&context.egui_ctx, Some(path)),
                None => app::FloorPlanApp::new(&context.egui_ctx),
            };
            Ok(Box::new(app))
        }),
    )
}
