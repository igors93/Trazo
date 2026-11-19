//! Assistant tools for shapes beyond rectangles: rooms of any shape.

use serde_json::{Value, json};

use super::{Change, ToolOutput, number, square_metres, to_model};
use crate::app::{FloorPlanApp, Selection};
use crate::domain::geometry::Point;

impl FloorPlanApp {
    /// `criar_comodo` with `pontos`: a room from its corners in order.
    pub(super) fn create_polygon_room(
        &mut self,
        name: &str,
        corners: &Value,
    ) -> Result<ToolOutput, String> {
        let corners = points_of(corners)?;
        self.apply_change(None, |project| {
            let id = project.add_polygon_room(&corners, name)?;
            let room = project.rooms.iter().find(|room| room.id == id);
            let area = room.and_then(|room| project.room_area(room)).unwrap_or(0.0);
            let walls = room.map(|room| room.wall_ids.clone()).unwrap_or_default();
            Ok(Change::new(format!(
                "Cômodo \"{name}\" criado (id {id}), {} cantos, {}.",
                corners.len(),
                square_metres(area)
            ))
            .select(Selection::Room(id))
            .data(json!({ "id": id, "paredes": walls })))
        })
    }
}

/// A list of points given as `[{"x": 0, "y": 0}, ...]` or `[[0, 0], ...]`,
/// in the assistant's coordinates.
pub(super) fn points_of(value: &Value) -> Result<Vec<Point>, String> {
    let list = value
        .as_array()
        .ok_or("'pontos' deve ser uma lista de pontos, como [{\"x\": 0, \"y\": 0}, ...].")?;
    list.iter()
        .map(|item| {
            let (x, y) = match item {
                Value::Array(pair) if pair.len() == 2 => (pair[0].clone(), pair[1].clone()),
                Value::Object(map) => (
                    map.get("x").cloned().unwrap_or(Value::Null),
                    map.get("y").cloned().unwrap_or(Value::Null),
                ),
                _ => return Err("Cada ponto deve ter x e y.".to_owned()),
            };
            let coordinate = |value: Value| {
                number(&json!({ "v": value }), "v")?.ok_or("Cada ponto deve ter x e y.".to_owned())
            };
            Ok(to_model(coordinate(x)?, coordinate(y)?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::app::FloorPlanApp;
    use eframe::egui;
    use serde_json::json;

    #[test]
    fn assistants_create_rooms_of_any_shape() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let corners = json!([
            { "x": 0, "y": 0 }, { "x": 4, "y": 0 }, { "x": 4, "y": 2 },
            { "x": 2, "y": 2 }, { "x": 2, "y": 4 }, { "x": 0, "y": 4 }
        ]);
        let output = app.run_mcp_tool(
            "criar_comodo",
            &json!({ "nome": "Sala em L", "pontos": corners }),
        );
        assert!(!output.is_error, "{}", output.text);
        let plan = app.plan_summary();
        let room = &plan["comodos"][0];
        assert_eq!(room["forma"], "poligono");
        assert_eq!(room["area_m2"], 12.0);
        assert_eq!(room["cantos"].as_array().unwrap().len(), 6);

        // Pairs work too; crossing sides are refused with a reason.
        let crossing = app.run_mcp_tool(
            "criar_comodo",
            &json!({ "nome": "X", "pontos": [[10, 0], [14, 4], [14, 0], [10, 4]] }),
        );
        assert!(crossing.is_error && crossing.text.contains("cruzar"));
        assert_eq!(app.project.rooms.len(), 1);
    }
}
