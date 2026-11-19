//! The view's label and the floating navigation bar over the plan.

use super::super::{FloorPlanApp, canvas::overlay, theme::*};
use super::icons::{self, Icon};
use super::options_bar::{tool_icon, tool_name};
use super::widgets::icon_button_sized;
use eframe::egui::{self, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, vec2};

impl FloorPlanApp {
    /// The view's name at the top left of the plan, on a light plate.
    pub(in crate::app) fn paint_view_label(&self, painter: &egui::Painter, rect: Rect) {
        let title = painter.layout_no_wrap(
            "Planta baixa · Térreo".to_owned(),
            strong(12.5),
            TEXT,
        );
        let detail = painter.layout_no_wrap(
            format!("{} · metros", tool_name(self.tool)),
            regular(11.5),
            MUTED,
        );
        let plate = Rect::from_min_size(
            Pos2::new(rect.left() + 12.0, rect.top() + 12.0),
            vec2(title.size().x + detail.size().x + 62.0, 28.0),
        );
        if !rect.contains_rect(plate) {
            return;
        }
        overlay::paint_floating_plate(painter, plate);
        icons::paint(
            painter,
            Rect::from_center_size(Pos2::new(plate.left() + 16.0, plate.center().y), vec2(15.0, 15.0)),
            Icon::Plan,
            TEXT_2,
            ACCENT,
        );
        let title_width = title.size().x;
        painter.galley(
            Pos2::new(plate.left() + 30.0, plate.center().y - title.size().y / 2.0),
            title,
            TEXT,
        );
        let x = plate.left() + 30.0 + title_width + 10.0;
        painter.line_segment(
            [Pos2::new(x, plate.top() + 7.0), Pos2::new(x, plate.bottom() - 7.0)],
            Stroke::new(1.0, BORDER),
        );
        icons::paint(
            painter,
            Rect::from_center_size(Pos2::new(x + 13.0, plate.center().y), vec2(13.0, 13.0)),
            tool_icon(self.tool),
            ACCENT_TEXT,
            ACCENT,
        );
        painter.galley(
            Pos2::new(x + 23.0, plate.center().y - detail.size().y / 2.0),
            detail,
            MUTED,
        );
    }

    /// The floating zoom bar at the top right of the plan.
    pub(in crate::app) fn canvas_navigation(&mut self, ui: &mut Ui, rect: Rect) {
        overlay::paint_floating_plate(ui.painter(), rect);
        let size = overlay::NAVIGATION_BUTTON;
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(rect.shrink(3.0))
                .layout(egui::Layout::top_down(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.add_space(1.0);
                if icon_button_sized(ui, Icon::ZoomIn, "Ampliar", false, size, 17.0).clicked() {
                    self.zoom_by(1.25);
                }
                if icon_button_sized(ui, Icon::ZoomOut, "Reduzir", false, size, 17.0).clicked() {
                    self.zoom_by(0.8);
                }
                if icon_button_sized(ui, Icon::ZoomFit, "Enquadrar tudo (F)", false, size, 17.0)
                    .clicked()
                {
                    self.fit_view_requested();
                }
                let (line, _) = ui.allocate_exact_size(vec2(size, 9.0), Sense::hover());
                ui.painter().line_segment(
                    [
                        Pos2::new(line.left() + 6.0, line.center().y),
                        Pos2::new(line.right() - 6.0, line.center().y),
                    ],
                    Stroke::new(1.0, BORDER),
                );
                if icon_button_sized(
                    ui,
                    Icon::Grid,
                    "Mostrar ou ocultar a grade (F7)",
                    self.show_grid,
                    size,
                    17.0,
                )
                .clicked()
                {
                    self.toggle_grid();
                }
            },
        );
    }
}
