use super::super::{FloorPlanApp, McpStatus, theme::*};
use super::icons::{self, Icon};
use super::inspector::format_point;
use super::widgets::{icon_button_sized, tip, toggle_pill};
use crate::mcp;
use eframe::egui::{self, Align, Pos2, RichText, Sense, Stroke, Ui, vec2};

fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(11.0, 16.0), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top()),
            Pos2::new(rect.center().x, rect.bottom()),
        ],
        Stroke::new(1.0, BORDER_STRONG),
    );
}

impl FloorPlanApp {
    /// Whether AI assistants can reach Trazo, and the address to give them.
    fn mcp_pill(&mut self, ui: &mut Ui) {
        let (on, tooltip) = match &self.mcp_status {
            McpStatus::Listening(port) => (
                true,
                format!(
                    "Servidor MCP ativo em {}
{} ação(ões) da IA nesta sessão.
Clique para conectar o ChatGPT ou o Claude.",
                    mcp::local_url(*port),
                    self.mcp_actions
                ),
            ),
            McpStatus::Failed(error) => (
                false,
                format!(
                    "Servidor MCP desligado: {error}.
Clique para ver como conectar uma IA."
                ),
            ),
            McpStatus::Off => (false, "Servidor MCP desligado.".to_owned()),
        };
        if toggle_pill(ui, "IA", on, &tooltip).clicked() {
            self.show_mcp_dialog = true;
        }
    }

    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.set_min_height(22.0);
            ui.spacing_mut().item_spacing.x = 4.0;
            let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            icons::paint(ui.painter(), rect, Icon::Info, MUTED, ACCENT);
            ui.add_space(2.0);

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let total = self.project.walls.len()
                    + self.project.rooms.len()
                    + self.project.openings.len()
                    + usize::from(self.project.terrain.is_some());
                ui.label(
                    RichText::new(format!(
                        "{total} elemento{}",
                        if total == 1 { "" } else { "s" }
                    ))
                    .size(12.0)
                    .color(MUTED),
                );
                divider(ui);

                if icon_button_sized(ui, Icon::ZoomFit, "Enquadrar tudo (F)", false, 22.0, 15.0)
                    .clicked()
                {
                    self.fit_view_requested();
                }
                if icon_button_sized(ui, Icon::ZoomIn, "Ampliar", false, 22.0, 15.0).clicked() {
                    self.zoom_by(1.25);
                }
                let zoom = format!("{:.0}%", self.zoom_percent());
                let response = ui.add(
                    egui::Label::new(RichText::new(zoom).monospace().size(12.0).color(TEXT_2))
                        .sense(Sense::click()),
                );
                if tip(response, "Clique para voltar a 100%")
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.zoom_by(60.0 / self.scale);
                }
                if icon_button_sized(ui, Icon::ZoomOut, "Reduzir", false, 22.0, 15.0).clicked() {
                    self.zoom_by(0.8);
                }
                divider(ui);

                self.mcp_pill(ui);
                divider(ui);

                if toggle_pill(ui, "Encaixe", self.snap_enabled, "Encaixe magnético (F3)").clicked()
                {
                    self.toggle_snap();
                }
                if toggle_pill(ui, "Orto", self.ortho_enabled, "Modo orto (F8)").clicked() {
                    self.toggle_ortho();
                }
                if toggle_pill(ui, "Grade", self.show_grid, "Mostrar a grade (F7)").clicked() {
                    self.toggle_grid();
                }
                if toggle_pill(
                    ui,
                    "Cotas",
                    self.show_dimensions,
                    "Cotas em cm em volta da construção",
                )
                .clicked()
                {
                    self.show_dimensions = !self.show_dimensions;
                }
                divider(ui);

                let coordinates = self
                    .cursor_world
                    .map(|point| format!("{} m", format_point(point)))
                    .unwrap_or_else(|| "X —  Y —".to_owned());
                let galley = ui.painter().layout_no_wrap(coordinates, mono(12.0), TEXT_2);
                let width = galley.size().x.max(178.0);
                let (rect, response) =
                    ui.allocate_exact_size(vec2(width + 8.0, 20.0), Sense::hover());
                ui.painter().galley(
                    Pos2::new(
                        rect.right() - galley.size().x - 4.0,
                        rect.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    TEXT_2,
                );
                tip(response, "Coordenadas do cursor (Y para cima, em metros)");
                divider(ui);

                // The message takes whatever room is left, cut short if need be.
                let room = ui.available_width();
                ui.allocate_ui_with_layout(
                    vec2(room, 20.0),
                    egui::Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&self.message).color(TEXT_2)).truncate(),
                        );
                    },
                );
            });
        });
    }
}
