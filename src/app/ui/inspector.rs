use eframe::egui::{self, Color32, RichText, Ui};

use super::super::{FloorPlanApp, Selection, Tool};

impl FloorPlanApp {
    pub(super) fn show_inspector(&mut self, ui: &mut Ui) {
        ui.heading("Terreno");
        ui.add_space(6.0);
        ui.add(egui::Label::new("Defina as medidas exatas em metros.").wrap());
        ui.add_space(12.0);
        ui.label("Largura (m)");
        ui.add(egui::TextEdit::singleline(&mut self.terrain_width_text).desired_width(90.0));
        ui.add_space(8.0);
        ui.label("Comprimento (m)");
        ui.add(egui::TextEdit::singleline(&mut self.terrain_height_text).desired_width(90.0));
        ui.add_space(12.0);
        if let Some((width, height)) = self.terrain_dimensions() {
            ui.label(
                RichText::new(format!("Área: {:.2} m²", width * height).replace('.', ","))
                    .strong()
                    .color(Color32::from_rgb(34, 91, 135)),
            );
        } else {
            ui.label(
                RichText::new("Informe valores entre 0,25 e 10.000 m.")
                    .color(Color32::from_rgb(167, 76, 56)),
            );
        }
        ui.add_space(12.0);
        if ui
            .add_enabled(
                self.terrain_dimensions().is_some(),
                egui::Button::new(if self.project.terrain.is_some() {
                    "Aplicar medidas"
                } else {
                    "Criar terreno"
                }),
            )
            .clicked()
        {
            self.create_terrain_from_dimensions();
        }
        if self.project.terrain.is_some() && !self.project.walls.is_empty() {
            ui.add_space(8.0);
            if !self.project.content_inside_terrain() {
                ui.label(
                    RichText::new("Há paredes ou cômodos fora do terreno.")
                        .color(Color32::from_rgb(167, 76, 56)),
                );
            }
            if ui
                .add_enabled(
                    self.project.content_fits_terrain(),
                    egui::Button::new("Centralizar planta no terreno"),
                )
                .clicked()
            {
                self.center_content_on_terrain();
            }
        }
        ui.add_space(12.0);
        ui.separator();
        ui.add(
            egui::Label::new(
                "Você também pode clicar em dois cantos da grade para desenhar o terreno.",
            )
            .wrap(),
        );
        if self
            .selection
            .is_some_and(|selection| matches!(selection, Selection::Terrain(_)))
        {
            ui.add_space(12.0);
            if ui.button("Excluir terreno").clicked() {
                self.delete_selection();
            }
        }
    }

    pub(in crate::app) fn sync_terrain_fields(&mut self) {
        if let Some(terrain) = &self.project.terrain {
            self.terrain_width_text = format!("{:.2}", terrain.width()).replace('.', ",");
            self.terrain_height_text = format!("{:.2}", terrain.height()).replace('.', ",");
        }
    }

    pub(super) fn inspector_visible(&self) -> bool {
        self.tool == Tool::Terrain || matches!(self.selection, Some(Selection::Terrain(_)))
    }
}
