//! Terrain commands: creating the lot from typed dimensions, centring the
//! plan on it, subdividing it and detecting enclosed rooms.

use super::{FloorPlanApp, Selection};
use crate::domain::geometry::Point;

impl FloorPlanApp {
    pub(super) fn create_terrain_from_dimensions(&mut self) {
        let Some((width, height)) = self.terrain_dimensions() else {
            self.message = "Informe medidas válidas de pelo menos 0,25 m.".to_owned();
            return;
        };
        if !width.is_finite() || !height.is_finite() || width < 0.25 || height < 0.25 {
            self.message = "Informe medidas de pelo menos 0,25 m.".to_owned();
            return;
        }
        if let Some(terrain) = &self.project.terrain
            && (terrain.width() - width).abs() < 1e-4
            && (terrain.height() - height).abs() < 1e-4
        {
            return;
        }
        let previous = self.project.clone();
        let existing = self.project.terrain.as_ref().map(|terrain| terrain.id);
        let id = if let Some(id) = existing {
            self.project.resize_terrain(width, height);
            id
        } else {
            let origin = self
                .project
                .content_bounds()
                .map(|(min, max)| {
                    Point::new(
                        (min.x + max.x - width) / 2.0,
                        (min.y + max.y - height) / 2.0,
                    )
                })
                .unwrap_or(Point::new(0.0, 0.0));
            let Some(id) = self
                .project
                .set_terrain(origin, Point::new(origin.x + width, origin.y + height))
            else {
                return;
            };
            id
        };
        if !self.record_change(previous) {
            return;
        }
        self.set_selection(Some(Selection::Terrain(id)));
        self.fit_view_requested();
        self.message = format!(
            "Terreno {}: {:.2} m².",
            if existing.is_some() {
                "atualizado"
            } else {
                "criado"
            },
            width * height
        )
        .replace('.', ",");
        if !self.project.content_inside_terrain() {
            self.message
                .push_str(" Há paredes fora do terreno; use Centralizar planta.");
        }
    }

    pub(super) fn center_content_on_terrain(&mut self) {
        let previous = self.project.clone();
        if self.project.center_content_on_terrain() {
            if !self.record_change(previous) {
                return;
            }
            self.fit_view_requested();
            self.message = "Planta centralizada no terreno.".to_owned();
        } else {
            self.message = "A planta precisa caber no terreno para ser centralizada.".to_owned();
        }
    }

    pub(super) fn detect_enclosed_rooms(&mut self) {
        let previous = self.project.clone();
        let count = self.project.create_rooms_from_enclosed_walls();
        if count > 0 {
            self.record_change(previous);
            self.message = format!(
                "{} {} detectado{} automaticamente.",
                count,
                if count == 1 { "ambiente" } else { "ambientes" },
                if count == 1 { "" } else { "s" }
            );
        } else {
            self.message = "Nenhum novo ambiente fechado foi encontrado.".to_owned();
        }
    }

    pub(super) fn subdivide_terrain(&mut self, vertical: bool) {
        let Some(terrain) = &self.project.terrain else {
            self.message = "Defina o terreno antes de desmembrar.".to_owned();
            return;
        };
        let margin = terrain.width().max(terrain.height()) + 1.0;
        let (a, b) = if vertical {
            let x = (terrain.min.x + terrain.max.x) / 2.0;
            (
                Point::new(x, terrain.min.y - margin),
                Point::new(x, terrain.max.y + margin),
            )
        } else {
            let y = (terrain.min.y + terrain.max.y) / 2.0;
            (
                Point::new(terrain.min.x - margin, y),
                Point::new(terrain.max.x + margin, y),
            )
        };
        let previous = self.project.clone();
        match self.project.subdivide_terrain(a, b) {
            Ok(_) => {
                self.record_change(previous);
                self.message =
                    "Terreno desmembrado em dois lotes com áreas conservadas.".to_owned();
            }
            Err(error) => self.message = error,
        }
    }

    pub(super) fn terrain_dimensions(&self) -> Option<(f64, f64)> {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let (width, height) = (
            parse(&self.terrain_width_text)?,
            parse(&self.terrain_height_text)?,
        );
        (width.is_finite()
            && height.is_finite()
            && width >= 0.25
            && height >= 0.25
            && width <= 10000.0
            && height <= 10000.0)
            .then_some((width, height))
    }
}
