use eframe::egui::{self, Color32, Key, RichText, Ui};

use super::super::{FloorPlanApp, Selection, Tool};
use crate::domain::model::OpeningKind;

impl FloorPlanApp {
    pub(super) fn show_inspector(&mut self, ui: &mut Ui) {
        match self.selection {
            Some(Selection::Wall(id)) => self.wall_inspector(ui, id),
            Some(Selection::Room(id)) => self.room_inspector(ui, id),
            Some(Selection::Opening(id)) => self.opening_inspector(ui, id),
            Some(Selection::Terrain(_)) => self.terrain_inspector(ui),
            None => {
                if self.tool == Tool::Terrain {
                    self.terrain_inspector(ui);
                } else {
                    ui.label("Selecione um elemento para visualizar suas propriedades.");
                }
            }
        }
    }

    fn wall_inspector(&mut self, ui: &mut Ui, id: u64) {
        ui.heading("Parede");
        ui.add_space(6.0);
        ui.add(egui::Label::new("Edição de medidas da parede selecionada.").wrap());
        ui.add_space(12.0);

        let Some(wall) = self.project.walls.iter().find(|w| w.id == id).cloned() else {
            ui.label(RichText::new("Parede não encontrada.").color(Color32::from_rgb(167, 76, 56)));
            return;
        };

        ui.label(
            RichText::new(format!("Comprimento atual: {:.2} m", wall.length()).replace('.', ","))
                .strong()
                .color(Color32::from_rgb(34, 91, 135)),
        );
        ui.add_space(8.0);

        ui.label("Novo comprimento (m)");
        let edit_resp =
            ui.add(egui::TextEdit::singleline(&mut self.wall_length_text).desired_width(120.0));
        let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
        let apply_clicked = ui.button("Aplicar comprimento").clicked();

        if apply_clicked || (edit_resp.lost_focus() && enter_pressed) {
            self.apply_wall_length(id);
        }

        ui.add_space(8.0);
        ui.add(
            egui::Label::new(
                "O comprimento é ajustado na direção da parede, preservando os limites do terreno.",
            )
            .wrap(),
        );

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        if ui.button("Excluir parede").clicked() {
            self.delete_selection();
        }
    }

    fn room_inspector(&mut self, ui: &mut Ui, id: u64) {
        ui.heading("Cômodo");
        ui.add_space(6.0);
        ui.add(egui::Label::new("Edição de nome e dimensões do cômodo selecionado.").wrap());
        ui.add_space(12.0);

        let Some(room) = self.project.rooms.iter().find(|r| r.id == id).cloned() else {
            ui.label(RichText::new("Cômodo não encontrado.").color(Color32::from_rgb(167, 76, 56)));
            return;
        };

        ui.label("Nome do cômodo");
        let name_resp =
            ui.add(egui::TextEdit::singleline(&mut self.room_name_text).desired_width(180.0));
        ui.add_space(8.0);

        ui.label("Largura (m)");
        let width_resp =
            ui.add(egui::TextEdit::singleline(&mut self.room_width_text).desired_width(120.0));
        ui.add_space(8.0);

        ui.label("Comprimento (m)");
        let height_resp =
            ui.add(egui::TextEdit::singleline(&mut self.room_height_text).desired_width(120.0));
        ui.add_space(10.0);

        let area_text = if let Some(area) = self.project.room_area(&room) {
            format!("Área calculada: {:.2} m²", area).replace('.', ",")
        } else {
            "Área: —".to_owned()
        };
        ui.label(
            RichText::new(area_text)
                .strong()
                .color(Color32::from_rgb(34, 91, 135)),
        );

        ui.add_space(10.0);
        let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
        let apply_clicked = ui.button("Aplicar medidas").clicked();

        let any_lost_focus =
            name_resp.lost_focus() || width_resp.lost_focus() || height_resp.lost_focus();
        if apply_clicked || (any_lost_focus && enter_pressed) {
            self.apply_room_dimensions(id);
        }

        ui.add_space(8.0);
        ui.add(
            egui::Label::new(
                "As medidas alteram as paredes que delimitam o cômodo respeitando o terreno.",
            )
            .wrap(),
        );

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        if ui.button("Excluir cômodo").clicked() {
            self.delete_selection();
        }
    }

    fn opening_inspector(&mut self, ui: &mut Ui, id: u64) {
        let Some(opening) = self.project.openings.iter().find(|o| o.id == id).cloned() else {
            ui.label(
                RichText::new("Abertura não encontrada.").color(Color32::from_rgb(167, 76, 56)),
            );
            return;
        };

        let title = match opening.kind {
            OpeningKind::Door => "Porta",
            OpeningKind::Window => "Janela",
        };
        ui.heading(title);
        ui.add_space(6.0);
        ui.add(egui::Label::new(format!("Edição da {} selecionada.", title.to_lowercase())).wrap());
        ui.add_space(12.0);

        let wall_len = self
            .project
            .walls
            .iter()
            .find(|w| w.id == opening.wall_id)
            .map(|w| w.length())
            .unwrap_or(0.0);

        ui.label(
            RichText::new(format!("Largura atual: {:.2} m", opening.width()).replace('.', ","))
                .strong()
                .color(Color32::from_rgb(34, 91, 135)),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                format!(
                    "Posição: {:.0}% da parede ({:.2} m)",
                    opening.t * 100.0,
                    opening.t * wall_len
                )
                .replace('.', ","),
            )
            .color(Color32::from_rgb(100, 115, 120)),
        );
        ui.add_space(10.0);

        ui.label("Nova largura (m)");
        let width_resp =
            ui.add(egui::TextEdit::singleline(&mut self.opening_width_text).desired_width(120.0));
        let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
        let apply_clicked = ui.button("Aplicar largura").clicked();

        if apply_clicked || (width_resp.lost_focus() && enter_pressed) {
            self.apply_opening_width(id);
        }

        ui.add_space(8.0);
        let limits_tip = match opening.kind {
            OpeningKind::Door => "Portas suportam larguras de 0,60 m a 2,00 m.",
            OpeningKind::Window => "Janelas suportam larguras de 0,40 m a 4,00 m.",
        };
        ui.add(egui::Label::new(limits_tip).wrap());

        ui.add_space(8.0);
        ui.add(
            egui::Label::new(
                "Você também pode arrastar a abertura ao longo da parede com o mouse.",
            )
            .wrap(),
        );

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        let delete_label = match opening.kind {
            OpeningKind::Door => "Excluir porta",
            OpeningKind::Window => "Excluir janela",
        };
        if ui.button(delete_label).clicked() {
            self.delete_selection();
        }
    }

    fn terrain_inspector(&mut self, ui: &mut Ui) {
        ui.heading("Terreno");
        ui.add_space(6.0);
        ui.add(egui::Label::new("Defina as medidas exatas em metros.").wrap());
        ui.add_space(12.0);
        ui.label("Largura (m)");
        let w_resp =
            ui.add(egui::TextEdit::singleline(&mut self.terrain_width_text).desired_width(90.0));
        ui.add_space(8.0);
        ui.label("Comprimento (m)");
        let h_resp =
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
        let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
        let any_lost = w_resp.lost_focus() || h_resp.lost_focus();
        let clicked = ui
            .add_enabled(
                self.terrain_dimensions().is_some(),
                egui::Button::new(if self.project.terrain.is_some() {
                    "Aplicar medidas"
                } else {
                    "Criar terreno"
                }),
            )
            .clicked();

        if (clicked || (any_lost && enter_pressed)) && self.terrain_dimensions().is_some() {
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
        self.tool == Tool::Terrain || self.selection.is_some()
    }
}
