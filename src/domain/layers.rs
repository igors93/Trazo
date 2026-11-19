//! CAD layers with colour, line weight (mm), line type, visibility and lock.

use serde::{Deserialize, Serialize};

use super::{
    drawing::Drawing,
    model::{Project, Wall},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineType {
    #[default]
    Solid,
    Dashed,
    DashDot,
    Dotted,
}

impl LineType {
    pub const ALL: [LineType; 4] = [
        LineType::Solid,
        LineType::Dashed,
        LineType::DashDot,
        LineType::Dotted,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LineType::Solid => "Contínua",
            LineType::Dashed => "Tracejada",
            LineType::DashDot => "Traço-ponto",
            LineType::Dotted => "Pontilhada",
        }
    }

    pub fn svg_dasharray(self) -> Option<&'static str> {
        match self {
            LineType::Solid => None,
            LineType::Dashed => Some("10 6"),
            LineType::DashDot => Some("16 5 3 5"),
            LineType::Dotted => Some("2 5"),
        }
    }

    #[allow(dead_code)]
    pub fn svg_dasharray_mm(self) -> Option<&'static str> {
        match self {
            LineType::Solid => None,
            LineType::Dashed => Some("2.5 1.5"),
            LineType::DashDot => Some("4.0 1.2 0.8 1.2"),
            LineType::Dotted => Some("0.5 1.2"),
        }
    }

    pub fn screen_dash_gap(self) -> Option<(f32, f32)> {
        match self {
            LineType::Solid => None,
            LineType::Dashed => Some((8.0, 5.0)),
            LineType::DashDot => Some((12.0, 4.0)),
            LineType::Dotted => Some((2.0, 4.0)),
        }
    }
}

/// Built-in semantic layer categories used as defaults for architectural items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerCategory {
    Terrain,
    Walls,
    Openings,
    Rooms,
    Fixtures,
    Drawings,
    Dimensions,
    Annotations,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub color: [u8; 3],
    /// Technical pen weight in millimetres (e.g. 0.13, 0.18, 0.25, 0.35, 0.50).
    pub line_weight_mm: f64,
    #[serde(default)]
    pub line_type: LineType,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<LayerCategory>,
}

fn default_true() -> bool {
    true
}

impl Layer {
    pub fn hex_color(&self) -> String {
        let [r, g, b] = self.color;
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    pub fn screen_stroke_width(&self) -> f32 {
        (self.line_weight_mm as f32 * 6.5).clamp(0.8, 4.5)
    }
}

/// Standard architectural layers aligned with Brazilian CAD practice.
pub fn default_layers() -> Vec<Layer> {
    vec![
        Layer {
            id: 1,
            name: "Terreno".to_owned(),
            color: [94, 139, 191],
            line_weight_mm: 0.25,
            line_type: LineType::DashDot,
            visible: true,
            locked: false,
            category: Some(LayerCategory::Terrain),
        },
        Layer {
            id: 2,
            name: "Paredes".to_owned(),
            color: [29, 40, 54],
            line_weight_mm: 0.35,
            line_type: LineType::Solid,
            visible: true,
            locked: false,
            category: Some(LayerCategory::Walls),
        },
        Layer {
            id: 3,
            name: "Aberturas".to_owned(),
            color: [40, 120, 210],
            line_weight_mm: 0.20,
            line_type: LineType::Solid,
            visible: true,
            locked: false,
            category: Some(LayerCategory::Openings),
        },
        Layer {
            id: 4,
            name: "Cômodos".to_owned(),
            color: [142, 174, 212],
            line_weight_mm: 0.15,
            line_type: LineType::Solid,
            visible: true,
            locked: false,
            category: Some(LayerCategory::Rooms),
        },
        Layer {
            id: 5,
            name: "Mobiliário".to_owned(),
            color: [59, 78, 99],
            line_weight_mm: 0.18,
            line_type: LineType::Solid,
            visible: true,
            locked: false,
            category: Some(LayerCategory::Fixtures),
        },
        Layer {
            id: 6,
            name: "Desenho 2D".to_owned(),
            color: [28, 43, 60],
            line_weight_mm: 0.20,
            line_type: LineType::Solid,
            visible: true,
            locked: false,
            category: Some(LayerCategory::Drawings),
        },
        Layer {
            id: 7,
            name: "Cotas".to_owned(),
            color: [98, 116, 137],
            line_weight_mm: 0.13,
            line_type: LineType::Solid,
            visible: true,
            locked: false,
            category: Some(LayerCategory::Dimensions),
        },
        Layer {
            id: 8,
            name: "Anotações".to_owned(),
            color: [28, 43, 60],
            line_weight_mm: 0.18,
            line_type: LineType::Solid,
            visible: true,
            locked: false,
            category: Some(LayerCategory::Annotations),
        },
    ]
}

impl Project {
    pub fn ensure_default_layers(&mut self) {
        if self.layers.is_empty() {
            self.layers = default_layers();
            return;
        }
        let defaults = default_layers();
        for def in defaults {
            if let Some(cat) = def.category
                && !self.layers.iter().any(|l| l.category == Some(cat))
            {
                let id = self.layers.iter().map(|l| l.id).max().unwrap_or(0) + 1;
                let mut layer = def;
                layer.id = id;
                self.layers.push(layer);
            }
        }
    }

    pub fn layer(&self, id: u64) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id == id)
    }

    pub fn layer_for_category(&self, category: LayerCategory) -> Option<&Layer> {
        self.layers
            .iter()
            .find(|layer| layer.category == Some(category))
    }

    pub fn category_visible(&self, category: LayerCategory) -> bool {
        self.layer_for_category(category)
            .map(|layer| layer.visible)
            .unwrap_or(true)
    }

    pub fn category_locked(&self, category: LayerCategory) -> bool {
        self.layer_for_category(category)
            .map(|layer| layer.locked)
            .unwrap_or(false)
    }

    pub fn effective_wall_layer(&self, wall: &Wall) -> Option<&Layer> {
        wall.layer_id
            .and_then(|id| self.layer(id))
            .or_else(|| self.layer_for_category(LayerCategory::Walls))
    }

    pub fn effective_drawing_layer(&self, drawing: &Drawing) -> Option<&Layer> {
        drawing
            .layer_id
            .and_then(|id| self.layer(id))
            .or_else(|| self.layer_for_category(LayerCategory::Drawings))
    }

    pub fn is_wall_visible(&self, wall: &Wall) -> bool {
        self.effective_wall_layer(wall)
            .map(|layer| layer.visible)
            .unwrap_or(true)
    }

    pub fn is_wall_locked(&self, wall: &Wall) -> bool {
        self.effective_wall_layer(wall)
            .map(|layer| layer.locked)
            .unwrap_or(false)
    }

    pub fn is_drawing_visible(&self, drawing: &Drawing) -> bool {
        self.effective_drawing_layer(drawing)
            .map(|layer| layer.visible)
            .unwrap_or(true)
    }

    pub fn is_drawing_locked(&self, drawing: &Drawing) -> bool {
        self.effective_drawing_layer(drawing)
            .map(|layer| layer.locked)
            .unwrap_or(false)
    }

    pub fn add_layer(
        &mut self,
        name: &str,
        color: [u8; 3],
        line_weight_mm: f64,
        line_type: LineType,
    ) -> Result<u64, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Informe um nome para a camada.".to_owned());
        }
        if !line_weight_mm.is_finite() || !(0.05..=2.0).contains(&line_weight_mm) {
            return Err("A espessura da linha deve ficar entre 0,05 mm e 2,00 mm.".to_owned());
        }
        if self
            .layers
            .iter()
            .any(|l| l.name.eq_ignore_ascii_case(name))
        {
            return Err(format!("Já existe uma camada chamada \"{name}\"."));
        }
        let id = self.layers.iter().map(|l| l.id).max().unwrap_or(0) + 1;
        self.layers.push(Layer {
            id,
            name: name.to_owned(),
            color,
            line_weight_mm,
            line_type,
            visible: true,
            locked: false,
            category: None,
        });
        Ok(id)
    }

    pub fn update_layer(
        &mut self,
        id: u64,
        name: &str,
        color: [u8; 3],
        line_weight_mm: f64,
        line_type: LineType,
    ) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Informe um nome para a camada.".to_owned());
        }
        if !line_weight_mm.is_finite() || !(0.05..=2.0).contains(&line_weight_mm) {
            return Err("A espessura da linha deve ficar entre 0,05 mm e 2,00 mm.".to_owned());
        }
        let layer = self
            .layers
            .iter_mut()
            .find(|l| l.id == id)
            .ok_or("Camada não encontrada.")?;
        layer.name = name.to_owned();
        layer.color = color;
        layer.line_weight_mm = line_weight_mm;
        layer.line_type = line_type;
        Ok(())
    }

    pub fn toggle_layer_visibility(&mut self, id: u64) -> bool {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.id == id) {
            layer.visible = !layer.visible;
            return true;
        }
        false
    }

    pub fn toggle_layer_lock(&mut self, id: u64) -> bool {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.id == id) {
            layer.locked = !layer.locked;
            return true;
        }
        false
    }

    pub fn delete_layer(&mut self, id: u64) -> Result<(), String> {
        let Some(pos) = self.layers.iter().position(|l| l.id == id) else {
            return Err("Camada não encontrada.".to_owned());
        };
        if self.layers[pos].category.is_some() {
            return Err("As camadas padrão do projeto não podem ser excluídas.".to_owned());
        }
        self.layers.remove(pos);
        for wall in &mut self.walls {
            if wall.layer_id == Some(id) {
                wall.layer_id = None;
            }
        }
        for drawing in &mut self.drawings {
            if drawing.layer_id == Some(id) {
                drawing.layer_id = None;
            }
        }
        if self.active_layer_id == Some(id) {
            self.active_layer_id = None;
        }
        Ok(())
    }

    pub fn set_wall_layer(&mut self, wall_id: u64, layer_id: Option<u64>) -> Result<(), String> {
        if let Some(lid) = layer_id
            && self.layer(lid).is_none()
        {
            return Err("Camada não encontrada.".to_owned());
        }
        let wall = self
            .walls
            .iter_mut()
            .find(|w| w.id == wall_id)
            .ok_or("Parede não encontrada.")?;
        wall.layer_id = layer_id;
        Ok(())
    }

    pub fn set_drawing_layer(
        &mut self,
        drawing_id: u64,
        layer_id: Option<u64>,
    ) -> Result<(), String> {
        if let Some(lid) = layer_id
            && self.layer(lid).is_none()
        {
            return Err("Camada não encontrada.".to_owned());
        }
        let drawing = self
            .drawings
            .iter_mut()
            .find(|d| d.id == drawing_id)
            .ok_or("Desenho não encontrado.")?;
        drawing.layer_id = layer_id;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{drawing::Shape, geometry::Point};

    #[test]
    fn default_layers_support_visibility_lock_custom_layers_and_assignment() {
        let mut project = Project::default();
        assert_eq!(project.layers.len(), 8);
        assert!(project.category_visible(LayerCategory::Walls));
        assert!(!project.category_locked(LayerCategory::Walls));

        // Add a custom layer and assign a drawing and wall to it.
        let electric = project
            .add_layer("Elétrica", [210, 60, 45], 0.25, LineType::Dashed)
            .unwrap();
        assert!(
            project
                .add_layer("Elétrica", [0, 0, 0], 0.2, LineType::Solid)
                .is_err()
        );
        assert!(
            project
                .add_layer("Vazia", [0, 0, 0], 5.0, LineType::Solid)
                .is_err()
        );

        let wall_id = project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        let draw_id = project
            .add_drawing(Shape::Line {
                start: Point::new(0.0, 1.0),
                end: Point::new(3.0, 1.0),
            })
            .unwrap();

        project.set_wall_layer(wall_id, Some(electric)).unwrap();
        project.set_drawing_layer(draw_id, Some(electric)).unwrap();
        assert!(project.is_wall_visible(&project.walls[0]));
        assert!(!project.is_drawing_locked(&project.drawings[0]));

        project.toggle_layer_visibility(electric);
        project.toggle_layer_lock(electric);
        assert!(!project.is_wall_visible(&project.walls[0]));
        assert!(project.is_drawing_locked(&project.drawings[0]));

        // Built-in layers cannot be deleted; custom layers clean up references on deletion.
        assert!(project.delete_layer(1).is_err());
        project.delete_layer(electric).unwrap();
        assert_eq!(project.walls[0].layer_id, None);
        assert_eq!(project.drawings[0].layer_id, None);
        assert!(project.is_wall_visible(&project.walls[0]));
    }
}
