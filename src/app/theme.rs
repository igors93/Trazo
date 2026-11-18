use eframe::egui::{self, Color32};

pub fn apply(context: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.widgets.active.bg_fill = Color32::from_rgb(33, 164, 150);
    visuals.selection.bg_fill = Color32::from_rgb(31, 177, 159);
    context.set_visuals(visuals);
}
