#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bench;
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
    let cli_args: Vec<String> = std::env::args().skip(1).collect();
    if cli_args.iter().any(|argument| argument == "--mcp-stdio") {
        mcp::bridge::run(mcp::port());
        return Ok(());
    }

    // `trazo --bench [--bench-out FILE]`: headless stress benchmark on a
    // 3-storey, 500+ wall building (T-07).
    if cli_args.iter().any(|argument| argument == "--bench") {
        if cli_args.iter().any(|argument| argument == "--bench-out")
            && !cli_args.windows(2).any(|pair| pair[0] == "--bench-out")
        {
            eprintln!("Informe um arquivo após --bench-out.");
            std::process::exit(2);
        }
        let bench_out = cli_args
            .iter()
            .position(|a| a == "--bench-out")
            .and_then(|idx| cli_args.get(idx + 1))
            .map(std::path::PathBuf::from);
        if let Err(err) = bench::run_cli(bench_out.as_deref()) {
            eprintln!("Erro no benchmark: {err}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let initial_file = io::updates::parse_startup_file(std::env::args_os());

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Trazo")
            .with_icon(std::sync::Arc::new(app::app_icon::window_icon(256)))
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([1000.0, 600.0])
            .with_maximized(true),
        ..Default::default()
    };
    options.glow_options.vsync = app::pacing::driver_vsync_requested();

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
