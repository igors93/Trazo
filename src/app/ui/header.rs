use super::super::FloorPlanApp;
use eframe::egui::{self, Align, Color32, RichText, Ui};

impl FloorPlanApp {
    pub(super) fn menu_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Trazo")
                    .strong()
                    .size(16.0)
                    .color(Color32::from_rgb(19, 122, 111)),
            );
            ui.add_space(12.0);
            ui.menu_button("Arquivo", |ui| {
                if ui.button("Nova planta").clicked() {
                    self.new_project();
                    ui.close();
                }
                if ui.button("Abrir...                 Ctrl+O").clicked() {
                    self.open();
                    ui.close();
                }
                if ui.button("Salvar                  Ctrl+S").clicked() {
                    self.save();
                    ui.close();
                }
                ui.separator();
                if ui.button("Exportar imagem PNG...").clicked() {
                    self.export_png();
                    ui.close();
                }
                if ui.button("Exportar PDF...").clicked() {
                    self.export_pdf();
                    ui.close();
                }
            });
            ui.menu_button("Editar", |ui| {
                if ui
                    .add_enabled(
                        self.history.can_undo(),
                        egui::Button::new("Desfazer            Ctrl+Z"),
                    )
                    .clicked()
                {
                    self.undo();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.history.can_redo(),
                        egui::Button::new("Refazer            Ctrl+Y"),
                    )
                    .clicked()
                {
                    self.redo();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        self.selection.is_some(),
                        egui::Button::new("Excluir seleção     Delete"),
                    )
                    .clicked()
                {
                    self.delete_selection();
                    ui.close();
                }
            });
            ui.menu_button("Exibir", |ui| {
                if ui.button("Enquadrar tudo").clicked() {
                    self.fit_view_requested();
                    ui.close();
                }
                if ui.button("Ampliar").clicked() {
                    self.scale = (self.scale * 1.2).min(240.0);
                    ui.close();
                }
                if ui.button("Reduzir").clicked() {
                    self.scale = (self.scale / 1.2).max(3.0);
                    ui.close();
                }
            });
            ui.menu_button("Ajuda", |ui| {
                ui.label("Parede e cômodo: clique em dois pontos.");
                ui.label("Terreno: desenhe dois cantos ou informe as medidas.");
                ui.label("Porta e janela: arraste ou clique na parede.");
                ui.label("Roda do mouse: zoom. Botão do meio: mover a vista.");
                ui.label("Selecionar / mover: clique ou arraste a parede ou o interior do cômodo.");
                ui.label("Delete: excluir seleção. Esc: cancelar desenho.");
            });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(if self.dirty { "Não salvo" } else { "Salvo" })
                        .size(11.0)
                        .color(Color32::from_rgb(105, 123, 130)),
                );
                if ui
                    .add(egui::TextEdit::singleline(&mut self.project.name).desired_width(175.0))
                    .changed()
                {
                    self.dirty = true;
                }
            });
        });
    }
}
