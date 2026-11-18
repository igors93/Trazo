use super::super::FloorPlanApp;
use eframe::egui::{self, Align, Color32, RichText, Ui};

impl FloorPlanApp {
    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&self.message).color(Color32::from_rgb(64, 89, 99)));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(format!("Zoom {:.0}%", self.scale / 60.0 * 100.0));
                let mut zoom = self.scale / 60.0 * 100.0;
                if ui
                    .add_sized(
                        [140.0, ui.spacing().interact_size.y],
                        egui::Slider::new(&mut zoom, 5.0..=400.0)
                            .show_value(false)
                            .text("")
                            .trailing_fill(true)
                            .clamping(egui::SliderClamping::Always),
                    )
                    .on_hover_text("Ajustar zoom")
                    .changed()
                {
                    self.scale = zoom / 100.0 * 60.0;
                }
                if ui
                    .button("Enquadrar")
                    .on_hover_text("Ajustar toda a planta na janela")
                    .clicked()
                {
                    self.fit_view_requested();
                }
                ui.separator();
                ui.label(format!(
                    "{} paredes | {} cômodos | {} aberturas",
                    self.project.walls.len(),
                    self.project.rooms.len(),
                    self.project.openings.len()
                ));
            });
        });
    }
}
