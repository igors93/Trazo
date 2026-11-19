//! The options bar of the drawing tools: how many points are placed and
//! how to finish.

use eframe::egui::Ui;

use super::{caption, chip};
use crate::app::FloorPlanApp;
use crate::app::tools::DrawKind;

impl FloorPlanApp {
    pub(super) fn draw_options(&mut self, ui: &mut Ui, kind: DrawKind) {
        let placed = self.draw_points.len();
        chip(
            ui,
            &match placed {
                0 => "nenhum ponto".to_owned(),
                1 => "1 ponto".to_owned(),
                count => format!("{count} pontos"),
            },
        );
        caption(
            ui,
            match kind {
                DrawKind::Polyline | DrawKind::Spline => {
                    "Enter ou duplo clique termina · 1º ponto fecha · Backspace desfaz"
                }
                DrawKind::Line => "Continua do último ponto · Esc encerra · Backspace desfaz",
                _ => "Backspace desfaz o último ponto · Esc cancela",
            },
        );
    }
}
