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
mod foundations;
mod frames;
mod grid;
mod header;
pub(super) mod icons;
pub(in crate::app) use icons::Glyph;
mod inspector;
mod layers;
mod library;
mod loads;
mod modify_menu;
mod notes;
mod options_bar;
mod print;
mod quantities;
pub(in crate::app) use print::PrintDialogState;
mod repeat;
mod report_problem;
mod roof;
mod safety;
mod schedules;
mod selection;
mod sheet;
mod sheet_set;
mod start_page;
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
mod whatif;
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

        self.show_ribbon_panel(ui);

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
                if self.show_start_page {
                    self.start_page_view(ui);
                } else {
                    self.plan_area(ui);
                }
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

    fn show_ribbon_panel(&mut self, ui: &mut Ui) -> egui::InnerResponse<()> {
        egui::Panel::top("ribbon")
            // Reserve room for the tallest group and the horizontal scroll bar.
            // Otherwise switching tabs moves the canvas when that bar appears.
            .exact_size(if self.ribbon_collapsed { 27.0 } else { 120.0 })
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(theme::PANEL))
            .show(ui, |ui| self.ribbon(ui))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::RibbonTab;

    #[test]
    fn ribbon_keeps_the_same_height_across_tabs_and_window_widths() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let tabs = [
            RibbonTab::Architecture,
            RibbonTab::Edit,
            RibbonTab::Interiors,
            RibbonTab::Electrical,
            RibbonTab::Structure,
            RibbonTab::Drafting,
            RibbonTab::View,
            RibbonTab::Output,
        ];

        for width in [1000.0, 1280.0] {
            for tab in tabs {
                app.ribbon_tab = tab;
                context.begin_pass(egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.0),
                    )),
                    ..Default::default()
                });
                let mut height = 0.0;
                egui::Area::new(egui::Id::unique("ribbon_test")).show(&context, |ui| {
                    ui.set_width(width);
                    height = app.show_ribbon_panel(ui).response.rect.height();
                });
                context.end_pass().textures_delta.clear();
                assert_eq!(height, 120.0, "tab {tab:?} at width {width}");
            }
        }

        app.ribbon_collapsed = true;
        context.begin_pass(egui::RawInput::default());
        egui::Area::new(egui::Id::unique("ribbon_test")).show(&context, |ui| {
            assert_eq!(app.show_ribbon_panel(ui).response.rect.height(), 27.0);
        });
        context.end_pass().textures_delta.clear();
    }
}
