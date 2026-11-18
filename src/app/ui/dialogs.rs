use super::super::FloorPlanApp;
use eframe::egui::{self, Key, Ui};

impl FloorPlanApp {
    pub(super) fn room_name_dialog(&mut self, ui: &Ui) {
        if self.pending_room.is_none() {
            return;
        }
        let mut confirm = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("room_name_dialog")).show(ui.ctx(), |ui| {
            ui.heading("Nome do cômodo");
            ui.add_space(5.0);
            ui.label("Como deseja chamar este cômodo?");
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.room_name)
                    .hint_text("Ex.: Suíte, escritório, sala de estar")
                    .desired_width(280.0),
            );
            if self.room_dialog_focus {
                edit.request_focus();
                self.room_dialog_focus = false;
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Cancelar").clicked() {
                    cancel = true;
                }
                if ui
                    .add_enabled(
                        !self.room_name.trim().is_empty(),
                        egui::Button::new("Criar cômodo"),
                    )
                    .clicked()
                {
                    confirm = true;
                }
            });
            if ui.input(|input| input.key_pressed(Key::Enter)) && !self.room_name.trim().is_empty()
            {
                confirm = true;
            }
        });
        if cancel {
            self.pending_room = None;
            self.message = "Criação do cômodo cancelada.".to_owned();
        } else if confirm {
            if let Some((start, end)) = self.pending_room.take() {
                let previous = self.project.clone();
                if self
                    .project
                    .add_room(start, end, self.room_name.trim())
                    .is_some()
                {
                    self.record_change(previous);
                    self.message = format!("Cômodo {} criado.", self.room_name.trim());
                }
            }
        }
    }
}
