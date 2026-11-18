use eframe::egui::{self, Color32, Ui};

mod dialogs;
mod header;
mod icons;
mod inspector;
mod status_bar;
mod toolbar;

use super::FloorPlanApp;

impl FloorPlanApp {
    pub(super) fn show_layout(&mut self, ui: &mut Ui) {
        let context = ui.ctx().clone();

        if context.input(|i| i.viewport().close_requested()) && self.dirty {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending_action = Some(super::PendingAction::CloseApp);
        }

        self.handle_shortcuts(&context);

        egui::Panel::top("menu_bar")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(248, 250, 251))
                    .inner_margin(egui::Margin::symmetric(15, 5)),
            )
            .show(ui, |ui| self.menu_bar(ui));

        egui::Panel::top("tool_ribbon")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(239, 244, 245))
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ui, |ui| self.ribbon(ui));

        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(231, 239, 241))
                    .inner_margin(egui::Margin::symmetric(16, 7)),
            )
            .show(ui, |ui| self.status_bar(ui));

        if self.inspector_visible() {
            egui::Panel::right("terrain_inspector")
                .resizable(false)
                .exact_size(260.0)
                .frame(
                    egui::Frame::new()
                        .fill(Color32::from_rgb(250, 252, 253))
                        .inner_margin(egui::Margin::symmetric(16, 16)),
                )
                .show(ui, |ui| self.show_inspector(ui));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(248, 250, 251)))
            .show(ui, |ui| self.canvas(ui));
        self.room_name_dialog(ui);
        self.unsaved_changes_dialog(&context);
    }
}
