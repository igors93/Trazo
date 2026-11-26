//! The vertical circulation tool: stairs, ramps and level changes.

use serde_json::{Value, json};

use super::super::{Change, element_id, number, required_number, text, to_model, xy};
use super::{parse_railing_side, parse_stair_shape, round2};
use crate::{
    app::FloorPlanApp,
    domain::circulation::{CirculationKind, StairShape},
    mcp::ToolOutput,
};

impl FloorPlanApp {
    /// Vertical circulation & level transitions (`circulacao`).
    pub(in crate::app::mcp_tools) fn tool_circulation(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        if text(args, "acao").as_deref() == Some("editar") {
            return self.tool_edit_circulation(args);
        }
        let action = text(args, "acao")
            .unwrap_or_else(|| "listar".to_owned())
            .to_lowercase();
        match action.as_str() {
            "listar" => {
                let items: Vec<Value> = self
                    .project
                    .circulations
                    .iter()
                    .map(|c| {
                        let kind = match &c.kind {
                            CirculationKind::Stair { .. } => "escada",
                            CirculationKind::Ramp { .. } => "rampa",
                            CirculationKind::Landing { .. } => "patamar",
                            CirculationKind::Railing { .. } => "guarda_corpo",
                            CirculationKind::LevelChange { .. } => "desnivel",
                        };
                        let stair = c.stair_metrics().map(|m| {
                            json!({
                                "degraus": m.risers,
                                "espelho_cm": round2(m.riser_height_m * 100.0),
                                "piso_cm": round2(m.tread_m * 100.0),
                                "blondel_cm": round2(m.blondel_m * 100.0),
                                "blondel_ok": m.blondel_ok,
                            })
                        });
                        let ramp = c.ramp_metrics().map(|m| {
                            json!({
                                "inclinacao_pct": round2(m.slope_percent),
                                "nbr_9050_ok": m.nbr_9050_ok,
                            })
                        });
                        json!({
                            "id": c.id,
                            "nome": c.name,
                            "tipo": kind,
                            "origem": xy(c.origin),
                            "rotacao_graus": round2(c.rotation_deg),
                            "resumo": c.summary(),
                            "escada": stair,
                            "rampa": ramp,
                        })
                    })
                    .collect();
                Ok(ToolOutput::ok(format!("{} elemento(s) de circulação.", items.len()))
                    .with_structured(json!({
                        "projeto_id": self.active_project_id(),
                        "circulacoes": items,
                    })))
            }
            "adicionar_escada" => {
                let origin = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let shape = match text(args, "formato") {
                    Some(raw) => parse_stair_shape(&raw)
                        .ok_or("'formato' deve ser reta, em_l ou em_u.")?,
                    None => StairShape::Straight,
                };
                let width = number(args, "largura_m")?.unwrap_or(1.0);
                let rise = number(args, "desnivel_m")?.unwrap_or(self.project.wall_height);
                let tread = number(args, "piso_m")?.unwrap_or(0.28);
                let rot = number(args, "rotacao_graus")?;
                let railing = match text(args, "guarda_corpo") {
                    Some(raw) => Some(parse_railing_side(&raw).ok_or(
                        "'guarda_corpo' deve ser nenhum, esquerda, direita ou ambos.",
                    )?),
                    None => None,
                };
                let custom_name = text(args, "nome");
                self.apply_change(None, |project| {
                    let id = project.add_stair(origin, shape, width, rise, tread)?;
                    if let Some(el) = project.circulations.iter_mut().find(|c| c.id == id) {
                        if let Some(deg) = rot {
                            el.rotation_deg = deg;
                        }
                        if let Some(r) = railing {
                            el.railing = r;
                        }
                        if let Some(n) = &custom_name {
                            el.name = n.clone();
                        }
                    }
                    let el = project
                        .circulations
                        .iter()
                        .find(|c| c.id == id)
                        .ok_or("Escada não encontrada após inserção.")?;
                    let m = el.stair_metrics().ok_or("Métricas da escada indisponíveis.")?;
                    Ok(Change::new(format!("{} adicionada (id {id}): {}.", el.name, el.summary()))
                        .data(json!({
                            "id": id,
                            "degraus": m.risers,
                            "blondel_cm": round2(m.blondel_m * 100.0),
                            "blondel_ok": m.blondel_ok,
                        })))
                })
            }
            "adicionar_rampa" => {
                let origin = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let width = number(args, "largura_m")?.unwrap_or(1.20);
                let length = required_number(args, "comprimento_m")?;
                let rise = required_number(args, "desnivel_m")?;
                let rot = number(args, "rotacao_graus")?;
                let railing = match text(args, "guarda_corpo") {
                    Some(raw) => Some(parse_railing_side(&raw).ok_or(
                        "'guarda_corpo' deve ser nenhum, esquerda, direita ou ambos.",
                    )?),
                    None => None,
                };
                let custom_name = text(args, "nome");
                self.apply_change(None, |project| {
                    let id = project.add_ramp(origin, width, length, rise)?;
                    if let Some(el) = project.circulations.iter_mut().find(|c| c.id == id) {
                        if let Some(deg) = rot {
                            el.rotation_deg = deg;
                        }
                        if let Some(r) = railing {
                            el.railing = r;
                        }
                        if let Some(n) = &custom_name {
                            el.name = n.clone();
                        }
                    }
                    let el = project
                        .circulations
                        .iter()
                        .find(|c| c.id == id)
                        .ok_or("Rampa não encontrada após inserção.")?;
                    let m = el.ramp_metrics().ok_or("Métricas da rampa indisponíveis.")?;
                    Ok(Change::new(format!("{} adicionada (id {id}): {}.", el.name, el.summary()))
                        .data(json!({
                            "id": id,
                            "inclinacao_pct": round2(m.slope_percent),
                            "nbr_9050_ok": m.nbr_9050_ok,
                        })))
                })
            }
            "adicionar_patamar" => {
                let origin = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let width = required_number(args, "largura_m")?;
                let depth = required_number(args, "profundidade_m")?;
                let elev = number(args, "nivel_m")?.unwrap_or(0.0);
                let rot = number(args, "rotacao_graus")?;
                self.apply_change(None, |project| {
                    let id = project.add_landing(origin, width, depth, elev)?;
                    if let Some(deg) = rot
                        && let Some(el) = project.circulations.iter_mut().find(|c| c.id == id)
                    {
                        el.rotation_deg = deg;
                    }
                    Ok(Change::new(format!("Patamar {id} adicionado.")).data(json!({ "id": id })))
                })
            }
            "adicionar_guarda_corpo" => {
                let start = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                let end = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                let height = number(args, "altura_m")?.unwrap_or(1.10);
                self.apply_change(None, |project| {
                    let id = project.add_railing(start, end, height)?;
                    Ok(Change::new(format!("Guarda-corpo {id} adicionado."))
                        .data(json!({ "id": id })))
                })
            }
            "adicionar_desnivel" => {
                let start = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                let end = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                let from_elev = number(args, "nivel_de_m")?.unwrap_or(0.0);
                let to_elev = required_number(args, "nivel_para_m")?;
                self.apply_change(None, |project| {
                    let id = project.add_level_change(start, end, from_elev, to_elev)?;
                    Ok(Change::new(format!("Desnível {id} adicionado.")).data(json!({ "id": id })))
                })
            }
            "remover" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.remove_circulation(id) {
                        return Err("Elemento de circulação não encontrado.".to_owned());
                    }
                    Ok(Change::new(format!("Circulação {id} removida.")))
                })
            }
            _ => Err(
                "Informe 'acao': listar, adicionar_escada, adicionar_rampa, adicionar_patamar, adicionar_guarda_corpo, adicionar_desnivel ou remover."
                    .to_owned(),
            ),
        }
    }
}
