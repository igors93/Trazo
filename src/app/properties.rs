//! Keeping the properties panel's text fields in step with the selected
//! element, and applying what the person types: wall length and height,
//! room size, opening sizes, fixture sizes and masonry.

use super::{FloorPlanApp, Selection};
use crate::domain::model::OpeningKind;

impl FloorPlanApp {
    /// The material fields show the active brick or block, in centimetres.
    pub(super) fn sync_masonry_fields(&mut self) {
        let cm = |value: f64| {
            let value = value * 100.0;
            if (value - value.round()).abs() < 0.05 {
                format!("{value:.0}")
            } else {
                format!("{value:.1}").replace('.', ",")
            }
        };
        let material = &self.project.masonry;
        self.masonry_width_text = cm(material.block_width);
        self.masonry_length_text = cm(material.block_length);
        self.masonry_joint_text = cm(material.joint);
        self.masonry_plaster_text = cm(material.plaster);
        self.wall_height_text = format!("{:.2}", self.project.wall_height).replace('.', ",");
    }

    /// Makes `material` the one new walls are built with (one undo step).
    pub(super) fn set_masonry(&mut self, material: crate::domain::masonry::Masonry) {
        if !material.valid() {
            self.message =
                "Medidas de bloco inválidas: largura de 4 a 50 cm, comprimento de 5 a 100 cm, junta até 5 cm e reboco até 8 cm."
                    .to_owned();
            self.sync_masonry_fields();
            return;
        }
        if material == self.project.masonry {
            return;
        }
        let previous = self.project.clone();
        self.project.masonry = material;
        if !self.record_change(previous) {
            return;
        }
        self.sync_masonry_fields();
        self.message = format!(
            "Novas paredes: {} (parede acabada de {}).",
            self.project.masonry.label(),
            crate::domain::geometry::format_meters(self.project.masonry.thickness())
        );
    }

    /// Reads the brick or block typed in centimetres.
    pub(super) fn apply_masonry_fields(&mut self) {
        let parse = |value: &str| {
            value
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .ok()
                .map(|cm| cm / 100.0)
        };
        let (Some(width), Some(length), Some(joint), Some(plaster)) = (
            parse(&self.masonry_width_text),
            parse(&self.masonry_length_text),
            parse(&self.masonry_joint_text),
            parse(&self.masonry_plaster_text),
        ) else {
            self.message = "Informe as medidas do bloco em centímetros (ex.: 14 e 39).".to_owned();
            return;
        };
        let mut material = self.project.masonry.clone();
        let changed = (material.block_width - width).abs() > 1e-5
            || (material.block_length - length).abs() > 1e-5;
        material.block_width = width;
        material.block_length = length;
        material.joint = joint;
        material.plaster = plaster;
        if changed {
            material.name = "Personalizado".to_owned();
        }
        self.set_masonry(material);
    }

    /// Builds the selected wall, or every wall, with the active material.
    pub(super) fn apply_masonry_to_walls(&mut self, only: Option<u64>) {
        let material = self.project.masonry.clone();
        let previous = self.project.clone();
        match only {
            Some(id) => {
                self.project.set_wall_masonry(id, &material);
            }
            None => self.project.set_all_walls_masonry(&material),
        }
        if !self.record_change(previous) {
            return;
        }
        self.message = format!(
            "{} construída(s) com {}.",
            if only.is_some() {
                "Parede"
            } else {
                "Todas as paredes"
            },
            material.label()
        );
    }

    pub(super) fn apply_wall_height(&mut self) {
        let Some(height) = self
            .wall_height_text
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|height| (1.5..=10.0).contains(height))
        else {
            self.message = "Pé-direito deve estar entre 1,50 e 10,00 m.".to_owned();
            return;
        };
        if (height - self.project.wall_height).abs() < 1e-4 {
            return;
        }
        let previous = self.project.clone();
        self.project.wall_height = height;
        self.record_change(previous);
    }

    /// Applies the height and sill typed in the properties panel.
    pub(super) fn apply_opening_heights(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let Some(opening) = self.project.openings.iter().find(|o| o.id == id).cloned() else {
            return;
        };
        let height = parse(&self.opening_height_text).filter(|height| {
            opening
                .height()
                .is_none_or(|current| (current - height).abs() > 1e-4)
        });
        let sill = if opening.kind == OpeningKind::Window {
            parse(&self.opening_sill_text).filter(|sill| (opening.sill() - sill).abs() > 1e-4)
        } else {
            None
        };
        if height.is_none() && sill.is_none() {
            return;
        }
        let previous = self.project.clone();
        match self.project.set_opening_heights(id, height, sill) {
            Ok(()) => {
                if !self.record_change(previous) {
                    return;
                }
                self.sync_inspector_fields();
            }
            Err(error) => self.message = error,
        }
    }

    /// Applies the width and depth typed in the properties panel.
    pub(super) fn apply_fixture_size(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let (Some(width), Some(depth)) = (
            parse(&self.fixture_width_text),
            parse(&self.fixture_depth_text),
        ) else {
            self.message = "Informe largura e profundidade válidas em metros.".to_owned();
            return;
        };
        let previous = self.project.clone();
        let has_structural_load = self
            .project
            .fixtures
            .iter()
            .find(|fixture| fixture.id == id)
            .is_some_and(|fixture| fixture.structural_load_kn.is_some());
        let structural_load = match (has_structural_load, parse(&self.fixture_load_text)) {
            (true, Some(value)) if (0.1..=2_000.0).contains(&value) => Some(value),
            (true, _) => {
                self.message = "Informe uma carga entre 0,1 e 2.000 kN.".to_owned();
                return;
            }
            (false, _) => None,
        };
        match self
            .project
            .update_fixture(id, None, Some((width, depth)), None)
        {
            Ok(()) => {
                if let Some(load) = structural_load
                    && let Some(fixture) = self
                        .project
                        .fixtures
                        .iter_mut()
                        .find(|fixture| fixture.id == id)
                {
                    fixture.structural_load_kn = Some(load);
                }
                if !self.record_change(previous) {
                    return;
                }
                self.sync_inspector_fields();
                self.message = format!("Objeto com {width:.2} × {depth:.2} m.").replace('.', ",");
            }
            Err(error) => self.message = error,
        }
    }

    pub fn sync_inspector_fields(&mut self) {
        self.sync_terrain_fields();
        self.sync_masonry_fields();
        if let Some(group) = self
            .selected_group()
            .and_then(|id| self.project.groups.iter().find(|group| group.id == id))
        {
            self.group_name_text = group.name.clone();
        }
        self.sync_constraint_fields();
        match self.selection {
            Some(Selection::Fixture(id)) => {
                if let Some(fixture) = self.project.fixtures.iter().find(|f| f.id == id) {
                    self.fixture_width_text = format!("{:.2}", fixture.width).replace('.', ",");
                    self.fixture_depth_text = format!("{:.2}", fixture.depth).replace('.', ",");
                    self.fixture_load_text = fixture
                        .structural_load_kn
                        .map(|load| format!("{load:.2}").replace('.', ","))
                        .unwrap_or_default();
                }
            }
            Some(Selection::Wall(id)) => {
                if let Some(wall) = self.project.walls.iter().find(|w| w.id == id) {
                    self.wall_length_text = format!("{:.2}", wall.length()).replace('.', ",");
                }
            }
            Some(Selection::Room(id)) => {
                if let Some(room) = self.project.rooms.iter().find(|r| r.id == id) {
                    self.room_name_text = room.display_name().to_string();
                    if let Some((min, max)) = self.project.room_bounds(room) {
                        self.room_width_text = format!("{:.2}", max.x - min.x).replace('.', ",");
                        self.room_height_text = format!("{:.2}", max.y - min.y).replace('.', ",");
                    }
                }
            }
            Some(Selection::Opening(id)) => {
                if let Some(op) = self.project.openings.iter().find(|o| o.id == id) {
                    self.opening_width_text = format!("{:.2}", op.width()).replace('.', ",");
                    self.opening_height_text = op
                        .height()
                        .map(|height| format!("{height:.2}").replace('.', ","))
                        .unwrap_or_default();
                    self.opening_sill_text = format!("{:.2}", op.sill()).replace('.', ",");
                }
            }
            Some(Selection::Drawing(id)) => self.sync_drawing_fields(id),
            _ => {}
        }
    }

    pub fn apply_wall_length(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let Some(new_length) = parse(&self.wall_length_text) else {
            self.message = "Informe uma medida válida em metros (ex: 3,50).".to_owned();
            return;
        };
        let Some(wall) = self.project.walls.iter().find(|w| w.id == id) else {
            self.message = "Parede não encontrada.".to_owned();
            return;
        };
        if (wall.length() - new_length).abs() < 1e-4 {
            return;
        }
        let previous = self.project.clone();
        match self.project.resize_wall_length(id, new_length) {
            Ok(()) => {
                if !self.record_change(previous) {
                    return;
                }
                self.sync_inspector_fields();
                self.message = format!("Comprimento da parede alterado para {:.2} m.", new_length)
                    .replace('.', ",");
            }
            Err(err) => {
                self.message = err;
            }
        }
    }

    pub fn apply_room_dimensions(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let (Some(new_w), Some(new_h)) =
            (parse(&self.room_width_text), parse(&self.room_height_text))
        else {
            self.message = "Informe largura e comprimento válidos em metros.".to_owned();
            return;
        };

        let Some(room) = self.project.rooms.iter().find(|r| r.id == id).cloned() else {
            self.message = "Cômodo não encontrado.".to_owned();
            return;
        };

        let name_changed = room.name != self.room_name_text.trim();
        // A free-shape room has no width and length to type.
        let dimensions_changed = if !self.project.room_is_rectangle(&room) {
            false
        } else if let Some((min, max)) = self.project.room_bounds(&room) {
            let current_w = max.x - min.x;
            let current_h = max.y - min.y;
            (current_w - new_w).abs() > 1e-4 || (current_h - new_h).abs() > 1e-4
        } else {
            true
        };

        if !name_changed && !dimensions_changed {
            return;
        }

        let mut tx = self.project.clone();
        if name_changed && let Err(err) = tx.rename_room(id, &self.room_name_text) {
            self.message = err;
            return;
        }
        if dimensions_changed && let Err(err) = tx.resize_room_dimensions(id, new_w, new_h) {
            self.message = err;
            return;
        }

        let previous = self.project.clone();
        self.project = tx;
        if !self.record_change(previous) {
            return;
        }
        self.sync_inspector_fields();
        if name_changed && dimensions_changed {
            self.message = format!(
                "Nome e medidas do cômodo alterados para {:.2} × {:.2} m.",
                new_w, new_h
            )
            .replace('.', ",");
        } else if name_changed {
            self.message = format!(
                "Nome do cômodo alterado para \"{}\".",
                self.room_name_text.trim()
            );
        } else {
            self.message = format!(
                "Medidas do cômodo alteradas para {:.2} × {:.2} m.",
                new_w, new_h
            )
            .replace('.', ",");
        }
    }

    pub fn apply_opening_width(&mut self, id: u64) {
        let parse = |value: &str| value.trim().replace(',', ".").parse::<f64>().ok();
        let Some(new_w) = parse(&self.opening_width_text) else {
            self.message = "Informe uma largura válida em metros.".to_owned();
            return;
        };
        let Some(op) = self.project.openings.iter().find(|o| o.id == id) else {
            self.message = "Abertura não encontrada.".to_owned();
            return;
        };
        if (op.width() - new_w).abs() < 1e-4 {
            return;
        }
        let previous = self.project.clone();
        match self.project.update_opening_width(id, new_w) {
            Ok(()) => {
                if !self.record_change(previous) {
                    return;
                }
                self.sync_inspector_fields();
                self.message =
                    format!("Largura da abertura alterada para {:.2} m.", new_w).replace('.', ",");
            }
            Err(err) => {
                self.message = err;
            }
        }
    }
}
