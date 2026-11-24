#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod domain;
mod io;
mod mcp;

use eframe::egui;

/// Global counting allocator (zero `unsafe` in our crate) feeding `io::memory`
/// so oversized or hostile files abort cleanly before exhausting system RAM.
#[global_allocator]
static ALLOCATOR: cap::Cap<std::alloc::System> = cap::Cap::new(std::alloc::System, usize::MAX);

fn main() -> eframe::Result {
    io::memory::install_system_meter(|| ALLOCATOR.allocated());
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
            .with_icon(std::sync::Arc::new(app::app_icon::window_icon(256)))
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
