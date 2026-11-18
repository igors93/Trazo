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
                    self.request_new_project();
                    ui.close();
                }
                if ui.button("Abrir...                 Ctrl+O").clicked() {
                    self.request_open();
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
                ui.separator();
                if ui.button("Sair").clicked() {
                    self.request_close();
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
                ui.separator();
                ui.checkbox(&mut self.snap_enabled, "Encaixe magnético");
            });
            ui.menu_button("Ajuda", |ui| {
                ui.label("Dicas rápidas:");
                ui.label("• Selecionar (V): clique para selecionar paredes, cômodos, aberturas ou terreno.");
                ui.label("• Parede, Cômodo e Terreno: clique e arraste ou clique em dois pontos.");
                ui.label("• Porta e Janela: clique ou arraste para uma parede.");
                ui.label("• Roda do mouse: zoom. Botão do meio: mover a vista.");
                ui.label("• Delete: excluir seleção. Esc: cancelar ação.");
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            if ui
                .add(
                    egui::Button::new(RichText::new("Salvar").size(12.0))
                )
                .on_hover_text("Salvar projeto atual (Ctrl+S)")
                .clicked()
            {
                self.save();
            }

            ui.menu_button("Exportar", |ui| {
                if ui.button("Exportar imagem PNG...").clicked() {
                    self.export_png();
                    ui.close();
                }
                if ui.button("Exportar PDF...").clicked() {
                    self.export_pdf();
                    ui.close();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if self.dirty {
                    ui.label(
                        RichText::new("● Não salvo")
                            .size(11.5)
                            .strong()
                            .color(Color32::from_rgb(205, 105, 35)),
                    );
                } else {
                    ui.label(
                        RichText::new("✓ Salvo")
                            .size(11.5)
                            .color(Color32::from_rgb(45, 140, 125)),
                    );
                }
                ui.add_space(8.0);
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.project.name)
                            .hint_text("Nome do projeto")
                            .desired_width(175.0),
                    )
                    .changed()
                {
                    self.dirty = true;
                }
                ui.label(
                    RichText::new("Projeto:")
                        .size(12.0)
                        .color(Color32::from_rgb(115, 133, 140)),
                );
            });
        });
    }
}
