//! Properties of a room of free shape (an L, a U...): it has no width and
//! length to type, so the panel shows its area, perimeter and corners.

use eframe::egui::{RichText, Ui};

use super::{area, property_label, property_value};
use crate::app::theme::{ACCENT_TEXT, strong};
use crate::domain::geometry::{Point, format_meters, polygon_perimeter};

/// The rows of the "Dimensões" grid for a free-shape room.
pub(super) fn free_shape_dimensions(
    ui: &mut Ui,
    room_area: Option<f64>,
    outline: Option<&[Point]>,
    bounds: Option<(Point, Point)>,
) {
    property_label(ui, "Área");
    ui.label(
        RichText::new(room_area.map(area).unwrap_or_else(|| "—".to_owned()))
            .font(strong(13.0))
            .color(ACCENT_TEXT),
    );
    ui.end_row();
    if let Some(outline) = outline {
        property_label(ui, "Perímetro");
        property_value(ui, &format_meters(polygon_perimeter(outline)));
        ui.end_row();
        property_label(ui, "Cantos");
        property_value(ui, &outline.len().to_string());
        ui.end_row();
    }
    if let Some((min, max)) = bounds {
        property_label(ui, "Extensão");
        property_value(
            ui,
            &format!(
                "{} × {}",
                format_meters(max.x - min.x),
                format_meters(max.y - min.y)
            ),
        );
        ui.end_row();
    }
}
