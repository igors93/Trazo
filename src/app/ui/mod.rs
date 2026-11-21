use eframe::egui::{self, Margin, Ui};

mod browser;
mod circulation;
mod compare;
pub(in crate::app) use compare::CompareUiState;
mod connect;
mod constraints;
mod dialogs;
mod dimensions;
mod electrical;
mod ergonomics;
mod floors;
mod header;
pub(super) mod icons;
pub(in crate::app) use icons::Glyph;
mod inspector;
mod layers;
mod library;
mod modify_menu;
mod notes;
mod options_bar;
mod print;
mod quantities;
pub(in crate::app) use print::PrintDialogState;
mod repeat;
mod report_problem;
mod safety;
mod schedules;
mod selection;
mod sheet;
mod sheet_set;
mod status_bar;
mod structure;
mod tabs;
mod templates;
mod toolbar;
mod underlay;
pub(in crate::app) use underlay::{UnderlayUiState, paint_image_underlays};
mod updates;
mod view;
mod views;
mod widgets;
mod xref;

use super::{FloorPlanApp, theme};

impl FloorPlanApp {
    pub(super) fn show_layout(&mut self, ui: &mut Ui) {
        let context = ui.ctx().clone();

        if context.input(|i| i.viewport().close_requested())
            && self.any_tab_dirty()
            && self.pending_action.is_none()
        {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_close(&context);
        }

        self.handle_shortcuts(&context);
        self.update_window_title(&context);
        self.browser_hover = None;

        egui::Panel::top("menu_bar")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(Margin::symmetric(10, 3)),
            )
            .show(ui, |ui| self.menu_bar(ui));

        egui::Panel::top("ribbon")
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(theme::PANEL))
            .show(ui, |ui| self.ribbon(ui));

        egui::Panel::top("options_bar")
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(Margin::symmetric(10, 3)),
            )
            .show(ui, |ui| self.options_bar(ui));

        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(theme::BAND)
                    .inner_margin(Margin::symmetric(10, 2)),
            )
            .show(ui, |ui| self.status_bar(ui));

        if self.show_browser {
            egui::Panel::left("project_browser")
                .resizable(true)
                .default_size(232.0)
                .size_range(190.0..=420.0)
                .frame(egui::Frame::new().fill(theme::PANEL))
                .show(ui, |ui| self.project_browser(ui));
        }

        if self.show_properties {
            egui::Panel::right("properties")
                .resizable(true)
                .default_size(288.0)
                .size_range(250.0..=480.0)
                .frame(egui::Frame::new().fill(theme::PANEL))
                .show(ui, |ui| self.show_inspector(ui));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BAND))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                self.plan_area(ui);
            });
        self.room_name_dialog(ui);
        self.unsaved_changes_dialog(&context);
        self.help_dialog(&context);
        self.report_problem_dialog(&context);
        self.updates_dialog(&context);
        self.repeat_dialog(&context);
        self.connect_dialog(&context);
        self.layers_dialog(&context);
        self.dimensions_dialog(&context);
        self.notes_dialog(&context);
        self.sheet_dialog(&context);
        self.print_dialog(&context);
        self.schedules_dialog(&context);
        self.templates_dialog(&context);
        self.views_dialog(&context);
        self.sheet_set_dialog(&context);
        self.circulation_dialog(&context);
        self.floors_dialog(&context);
        self.library_dialog(&context);
        self.ergonomics_dialog(&context);
        self.electrical_dialog(&context);
        self.structure_dialog(&context);
        self.safety_dialog(&context);
        self.quantities_dialog(&context);
        self.underlay_dialog(&context);
        self.poll_xrefs(&context);
        self.xref_dialog(&context);
        self.compare_dialog(&context);
    }
}
