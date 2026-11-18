#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod domain;
mod io;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Trazo — Plantas baixas")
            .with_inner_size([1380.0, 860.0])
            .with_min_inner_size([1000.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Trazo",
        options,
        Box::new(|context| Ok(Box::new(app::FloorPlanApp::new(&context.egui_ctx)))),
    )
}
