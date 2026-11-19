//! Properties of a drawing entity: what it is, its size and where it is.
//! A circle's radius can be typed.

use eframe::egui::{Key, Ui};

use super::{actions, format_point, grid, metres_field, type_card};
use crate::app::FloorPlanApp;
use crate::app::theme::ACCENT;
use crate::app::ui::icons::Icon;
use crate::app::ui::widgets::{hint, property_label, property_value, section};
use crate::domain::drawing::Shape;
use crate::domain::geometry::format_meters;

fn degrees(radians: f64) -> String {
    format!("{:.1}°", radians.to_degrees().abs() + 0.0).replace('.', ",")
}

impl FloorPlanApp {
    pub(super) fn drawing_inspector(&mut self, ui: &mut Ui, id: u64) {
        let Some(shape) = self
            .project
            .drawing(id)
            .map(|drawing| drawing.shape.clone())
        else {
            Self::missing(ui, "Desenho não encontrado.");
            return;
        };
        let glyph = match &shape {
            Shape::Line { .. } => crate::app::ui::Glyph::Line,
            Shape::Polyline { .. } if shape.is_rectangle() => crate::app::ui::Glyph::Rectangle,
            Shape::Polyline { .. } => crate::app::ui::Glyph::Polyline,
            Shape::Arc { .. } => crate::app::ui::Glyph::Arc,
            Shape::Circle { .. } => crate::app::ui::Glyph::Circle,
            Shape::Spline { .. } => crate::app::ui::Glyph::Spline,
        };
        type_card(ui, Icon::Glyph(glyph), shape.label(), "Desenho 2D");

        let mut submit = false;
        section(ui, "drawing_geometry", "Geometria", |ui| {
            grid(ui, "drawing_geometry_grid", |ui| {
                match &shape {
                    Shape::Line { start, end } => {
                        property_label(ui, "Comprimento");
                        property_value(ui, &format_meters(start.distance(*end)));
                        ui.end_row();
                        property_label(ui, "Ângulo");
                        property_value(ui, &degrees((start.y - end.y).atan2(end.x - start.x)));
                        ui.end_row();
                        property_label(ui, "Início");
                        property_value(ui, &format_point(*start));
                        ui.end_row();
                        property_label(ui, "Fim");
                        property_value(ui, &format_point(*end));
                        ui.end_row();
                    }
                    Shape::Circle { center, .. } => {
                        property_label(ui, "Raio");
                        let radius = metres_field(ui, &mut self.drawing_radius_text);
                        submit |= radius.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        ui.end_row();
                        property_label(ui, "Centro");
                        property_value(ui, &format_point(*center));
                        ui.end_row();
                    }
                    Shape::Arc {
                        center,
                        radius,
                        sweep,
                        ..
                    } => {
                        property_label(ui, "Raio");
                        property_value(ui, &format_meters(*radius));
                        ui.end_row();
                        property_label(ui, "Abertura");
                        property_value(ui, &degrees(*sweep));
                        ui.end_row();
                        property_label(ui, "Centro");
                        property_value(ui, &format_point(*center));
                        ui.end_row();
                    }
                    Shape::Polyline { points, .. } | Shape::Spline { points, .. } => {
                        property_label(ui, "Pontos");
                        property_value(ui, &points.len().to_string());
                        ui.end_row();
                        property_label(ui, "Fechado");
                        property_value(ui, if shape.is_closed() { "Sim" } else { "Não" });
                        ui.end_row();
                    }
                }
                property_label(
                    ui,
                    if shape.is_closed() {
                        "Perímetro"
                    } else {
                        "Comprimento total"
                    },
                );
                property_value(ui, &format_meters(shape.length()));
                ui.end_row();
                if let Shape::Polyline {
                    points,
                    closed: true,
                } = &shape
                {
                    property_label(ui, "Área");
                    property_value(
                        ui,
                        &format!("{:.2} m²", crate::domain::geometry::polygon_area(points))
                            .replace('.', ","),
                    );
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            hint(
                ui,
                Icon::Info,
                "Desenhos 2D são só traço: não viram paredes nem entram nas áreas.",
                ACCENT,
            );
        });

        let is_circle = matches!(shape, Shape::Circle { .. });
        let (apply, delete) = actions(ui, is_circle.then_some("Aplicar"), Some("Excluir desenho"));
        if is_circle && (apply || submit) {
            self.apply_circle_radius(id);
        }
        if delete {
            self.delete_selection();
        }
    }

    /// Applies the radius typed for a circle.
    fn apply_circle_radius(&mut self, id: u64) {
        let Some(radius) = self
            .drawing_radius_text
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
        else {
            self.message = "Informe o raio em metros.".to_owned();
            return;
        };
        let Some(Shape::Circle {
            center,
            radius: old,
        }) = self
            .project
            .drawing(id)
            .map(|drawing| drawing.shape.clone())
        else {
            return;
        };
        if (old - radius).abs() < 1e-6 {
            return;
        }
        let previous = self.project.clone();
        match self
            .project
            .update_drawing(id, Shape::Circle { center, radius })
        {
            Ok(()) => {
                if !self.record_change(previous) {
                    return;
                }
                self.message = format!("Raio alterado para {}.", format_meters(radius));
            }
            Err(error) => self.message = error,
        }
    }

    /// Fills the radius field from the selected circle.
    pub(in crate::app) fn sync_drawing_fields(&mut self, id: u64) {
        if let Some(Shape::Circle { radius, .. }) = self.project.drawing(id).map(|d| &d.shape) {
            self.drawing_radius_text = format!("{radius:.2}").replace('.', ",");
        }
    }
}
