//! The terrain tool: the lot's size, shape, position and setbacks.

use serde_json::{Value, json};

use super::{Change, number, required_number, round, square_metres, to_model};
use crate::app::{FloorPlanApp, Selection};
use crate::{
    domain::geometry::{Point, format_meters},
    mcp::ToolOutput,
};

impl FloorPlanApp {
    pub(super) fn tool_terrain(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let recuo_frente = number(args, "recuo_frente")?;
        let recuo_fundos = number(args, "recuo_fundos")?;
        let recuo_lateral = number(args, "recuo_lateral")?;
        let has_setbacks =
            recuo_frente.is_some() || recuo_fundos.is_some() || recuo_lateral.is_some();
        let width_opt = number(args, "largura")?;
        let length_opt = number(args, "comprimento")?;

        if width_opt.is_none() && length_opt.is_none() && has_setbacks {
            if self.project.terrain.is_none() {
                return Err(
                    "Defina 'largura' e 'comprimento' do terreno antes de configurar recuos."
                        .to_owned(),
                );
            }
            let mut setbacks = self.project.setbacks;
            if let Some(v) = recuo_frente {
                setbacks.front = v;
            }
            if let Some(v) = recuo_fundos {
                setbacks.rear = v;
            }
            if let Some(v) = recuo_lateral {
                setbacks.side = v;
            }
            setbacks.validate()?;
            let mut output =
                self.apply_change(Some("Os recuos já tinham esses valores."), |project| {
                    project.set_setbacks(setbacks)?;
                    Ok(Change::new(format!(
                        "Recuos definidos: frente {}, fundos {}, lateral {}.",
                        format_meters(setbacks.front),
                        format_meters(setbacks.rear),
                        format_meters(setbacks.side)
                    ))
                    .data(json!({
                        "recuos": {
                            "frente": round(setbacks.front),
                            "fundos": round(setbacks.rear),
                            "lateral": round(setbacks.side),
                        }
                    })))
                })?;
            if !self.project.content_inside_buildable_limit() {
                output
                    .text
                    .push_str(" Atenção: há paredes invadindo a faixa de recuo configurada.");
            }
            return Ok(output);
        }

        let width = required_number(args, "largura")?;
        let length = required_number(args, "comprimento")?;
        if width < 0.25 || length < 0.25 {
            return Err(
                "O terreno precisa ter pelo menos 0,25 m de largura e de comprimento.".to_owned(),
            );
        }
        let mut setbacks = self.project.setbacks;
        if let Some(v) = recuo_frente {
            setbacks.front = v;
        }
        if let Some(v) = recuo_fundos {
            setbacks.rear = v;
        }
        if let Some(v) = recuo_lateral {
            setbacks.side = v;
        }
        if has_setbacks {
            setbacks.validate()?;
        }
        let (min, max) = match (number(args, "x")?, number(args, "y")?) {
            (Some(x), Some(y)) => (to_model(x, y + length), to_model(x + width, y)),
            (None, None) => match (&self.project.terrain, self.project.content_bounds()) {
                // Keep the lower left corner the assistant already knows.
                (Some(terrain), _) => (
                    Point::new(terrain.min.x, terrain.max.y - length),
                    Point::new(terrain.min.x + width, terrain.max.y),
                ),
                (None, Some((low, high))) => {
                    let origin = Point::new(
                        (low.x + high.x - width) / 2.0,
                        (low.y + high.y - length) / 2.0,
                    );
                    (origin, Point::new(origin.x + width, origin.y + length))
                }
                (None, None) => (to_model(0.0, length), to_model(width, 0.0)),
            },
            _ => return Err("Informe x e y juntos, ou nenhum dos dois.".to_owned()),
        };
        let mut output = self.apply_change(None, |project| {
            let id = project
                .set_terrain(min, max)
                .ok_or("Medidas de terreno inválidas.")?;
            if has_setbacks {
                project.set_setbacks(setbacks)?;
            }
            Ok(Change::new(format!(
                "Terreno de {} × {} ({}) definido.",
                format_meters(width),
                format_meters(length),
                square_metres(width * length)
            ))
            .select(Selection::Terrain(id))
            .data(json!({
                "id": id,
                "recuos": project.setbacks.is_active().then(|| json!({
                    "frente": round(project.setbacks.front),
                    "fundos": round(project.setbacks.rear),
                    "lateral": round(project.setbacks.side),
                })),
            })))
        })?;
        if !self.project.content_inside_terrain() {
            output.text.push_str(
                " Atenção: há paredes ou cômodos fora do terreno; use centralizar_planta ou mova-os.",
            );
        } else if self.project.setbacks.is_active()
            && !self.project.content_inside_buildable_limit()
        {
            output
                .text
                .push_str(" Atenção: há paredes invadindo a faixa de recuo configurada.");
        }
        self.sync_terrain_fields();
        self.fit_view_requested();
        Ok(output)
    }
}
