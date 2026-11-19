//! Assistant tools for plain drawing entities: `desenhar` creates lines,
//! polylines, rectangles, arcs, circles and splines, and the plan summary
//! lists them.

use serde_json::{Value, json};

use super::shapes::points_of;
use super::{Change, ToolOutput, flag, number, round, text, xy};
use crate::app::{FloorPlanApp, Selection};
use crate::domain::drawing::Shape;
use crate::domain::geometry::format_meters;
use crate::domain::model::Project;

impl FloorPlanApp {
    /// `desenhar`: one drawing entity from its points.
    pub(super) fn tool_draw(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let kind = text(args, "tipo")
            .ok_or("Informe 'tipo': linha, polilinha, retangulo, arco, circulo ou spline.")?;
        let points = match args.get("pontos").filter(|value| !value.is_null()) {
            Some(value) => points_of(value)?,
            None => Vec::new(),
        };
        let count = |needed: usize, what: &str| {
            if points.len() == needed {
                Ok(())
            } else {
                Err(format!("'{kind}' precisa de {needed} pontos ({what})."))
            }
        };
        let closed = flag(args, "fechado");
        let shape = match kind.as_str() {
            "linha" => {
                count(2, "início e fim")?;
                Shape::Line {
                    start: points[0],
                    end: points[1],
                }
            }
            "retangulo" => {
                count(2, "dois cantos opostos")?;
                Shape::rectangle(points[0], points[1])
            }
            "arco" => {
                count(3, "início, um ponto do arco e fim")?;
                Shape::arc_through(points[0], points[1], points[2])
                    .ok_or("Os três pontos do arco estão em linha.")?
            }
            "circulo" => {
                let center = match args.get("centro") {
                    Some(value) if !value.is_null() => points_of(&json!([value]))?[0],
                    _ if points.len() == 2 => points[0],
                    _ => return Err("Informe 'centro' e 'raio' do círculo.".to_owned()),
                };
                let radius = match number(args, "raio")? {
                    Some(radius) => radius,
                    None if points.len() == 2 => points[0].distance(points[1]),
                    None => return Err("Informe 'raio' do círculo em metros.".to_owned()),
                };
                Shape::Circle { center, radius }
            }
            "polilinha" => Shape::Polyline { points, closed },
            "spline" => Shape::Spline { points, closed },
            other => {
                return Err(format!(
                    "Tipo '{other}' desconhecido: use linha, polilinha, retangulo, arco, circulo ou spline."
                ));
            }
        };
        self.apply_change(None, |project| {
            let label = shape.label();
            let length = shape.length();
            let id = project.add_drawing(shape)?;
            Ok(Change::new(format!(
                "{label} desenhado(a) (id {id}), {} de traço.",
                format_meters(length)
            ))
            .select(Selection::Drawing(id))
            .data(json!({ "id": id })))
        })
    }
}

/// The drawing entities of the plan, as `obter_planta` lists them.
pub(super) fn drawings_summary(project: &Project) -> Vec<Value> {
    project
        .drawings
        .iter()
        .map(|drawing| {
            let shape = &drawing.shape;
            let kind = match shape {
                Shape::Line { .. } => "linha",
                Shape::Polyline { .. } if shape.is_rectangle() => "retangulo",
                Shape::Polyline { .. } => "polilinha",
                Shape::Arc { .. } => "arco",
                Shape::Circle { .. } => "circulo",
                Shape::Spline { .. } => "spline",
            };
            let (center, radius) = match shape {
                Shape::Arc { center, radius, .. } | Shape::Circle { center, radius } => {
                    (Some(xy(*center)), Some(round(*radius)))
                }
                _ => (None, None),
            };
            json!({
                "id": drawing.id,
                "tipo": kind,
                "pontos": shape.key_points().into_iter().map(xy).collect::<Vec<_>>(),
                "centro": center,
                "raio": radius,
                "fechado": shape.is_closed(),
                "comprimento": round(shape.length()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::app::FloorPlanApp;
    use eframe::egui;
    use serde_json::json;

    #[test]
    fn assistants_draw_lines_arcs_and_circles() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        for args in [
            json!({ "tipo": "linha", "pontos": [[0, 0], [3, 4]] }),
            json!({ "tipo": "circulo", "centro": { "x": 10, "y": 0 }, "raio": 1.5 }),
            json!({ "tipo": "arco", "pontos": [[-2, 0], [0, 2], [2, 0]] }),
            json!({ "tipo": "polilinha", "pontos": [[0, 5], [4, 5], [4, 8]], "fechado": true }),
        ] {
            let output = app.run_mcp_tool("desenhar", &args);
            assert!(!output.is_error, "{}", output.text);
        }
        let plan = app.plan_summary();
        let drawings = plan["desenhos"].as_array().unwrap();
        assert_eq!(drawings.len(), 4);
        assert_eq!(drawings[0]["comprimento"], 5.0);
        assert_eq!(drawings[1]["raio"], 1.5);
        assert_eq!(drawings[2]["tipo"], "arco");
        // The arc passes over the top, through (0, 2) in Y-up coordinates.
        assert_eq!(drawings[2]["centro"]["y"], 0.0);
        assert!(drawings[3]["fechado"].as_bool().unwrap());

        let missing = app.run_mcp_tool("desenhar", &json!({ "tipo": "linha", "pontos": [[0, 0]] }));
        assert!(missing.is_error);
        let element = app.run_mcp_tool("excluir_elemento", &json!({ "id": drawings[0]["id"] }));
        assert!(!element.is_error, "{}", element.text);
        assert_eq!(app.project.drawings.len(), 3);
    }
}
