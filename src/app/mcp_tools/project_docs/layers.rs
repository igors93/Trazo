//! The CAD layers tool: listing, creating, editing and removing layers.

use serde_json::{Value, json};

use super::super::{Change, element_id, flag, number, optional_count, text};
use super::{line_type_key, parse_hex_color, parse_line_type, round2};
use crate::{app::FloorPlanApp, domain::layers::LineType, mcp::ToolOutput};

impl FloorPlanApp {
    /// CAD layer management (`camadas`).
    pub(in crate::app::mcp_tools) fn tool_layers(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "listar".to_owned())
            .to_lowercase();
        match action.as_str() {
            "listar" => {
                let list: Vec<Value> = self
                    .project
                    .layers
                    .iter()
                    .map(|l| {
                        json!({
                            "id": l.id,
                            "nome": l.name,
                            "cor": l.hex_color(),
                            "espessura_mm": round2(l.line_weight_mm),
                            "tipo_linha": line_type_key(l.line_type),
                            "visivel": l.visible,
                            "bloqueada": l.locked,
                            "padrao": l.category.is_some(),
                            "ativa": self.project.active_layer_id == Some(l.id),
                        })
                    })
                    .collect();
                Ok(
                    ToolOutput::ok(format!("{} camada(s) no projeto.", list.len()))
                        .with_structured(json!({
                            "projeto_id": self.active_project_id(),
                            "camada_ativa_id": self.project.active_layer_id,
                            "camadas": list,
                        })),
                )
            }
            "criar" => {
                let name = text(args, "nome").ok_or("Informe 'nome' da camada.")?;
                let color = match text(args, "cor") {
                    Some(hex) => parse_hex_color(&hex)?,
                    None => [40, 90, 150],
                };
                let weight = number(args, "espessura_mm")?.unwrap_or(0.25);
                let line_type = match text(args, "tipo_linha") {
                    Some(raw) => parse_line_type(&raw).ok_or(
                        "'tipo_linha' deve ser continua, tracejada, traco_ponto ou pontilhada.",
                    )?,
                    None => LineType::Solid,
                };
                let activate = flag(args, "ativar");
                self.apply_change(None, |project| {
                    let id = project.add_layer(&name, color, weight, line_type)?;
                    if activate {
                        project.active_layer_id = Some(id);
                    }
                    Ok(Change::new(format!("Camada \"{name}\" criada (id {id})."))
                        .data(json!({ "id": id, "nome": name })))
                })
            }
            "editar" => {
                let id = element_id(args, "id")?;
                let current = self
                    .project
                    .layer(id)
                    .cloned()
                    .ok_or_else(|| format!("Camada {id} não encontrada."))?;
                let new_name = text(args, "nome").unwrap_or_else(|| current.name.clone());
                let new_color = match text(args, "cor") {
                    Some(hex) => parse_hex_color(&hex)?,
                    None => current.color,
                };
                let new_weight = number(args, "espessura_mm")?.unwrap_or(current.line_weight_mm);
                let new_line_type = match text(args, "tipo_linha") {
                    Some(raw) => parse_line_type(&raw).ok_or(
                        "'tipo_linha' deve ser continua, tracejada, traco_ponto ou pontilhada.",
                    )?,
                    None => current.line_type,
                };
                let visible = args.get("visivel").and_then(Value::as_bool);
                let locked = args.get("bloqueada").and_then(Value::as_bool);
                let activate = args.get("ativar").and_then(Value::as_bool);
                self.apply_change(
                    Some("A camada já estava com essas propriedades."),
                    |project| {
                        project.update_layer(
                            id,
                            &new_name,
                            new_color,
                            new_weight,
                            new_line_type,
                        )?;
                        if let Some(layer) = project.layers.iter_mut().find(|l| l.id == id) {
                            if let Some(v) = visible {
                                layer.visible = v;
                            }
                            if let Some(b) = locked {
                                layer.locked = b;
                            }
                        }
                        if let Some(act) = activate {
                            project.active_layer_id = if act {
                                Some(id)
                            } else if project.active_layer_id == Some(id) {
                                None
                            } else {
                                project.active_layer_id
                            };
                        }
                        Ok(Change::new(format!(
                            "Camada {id} (\"{new_name}\") atualizada."
                        )))
                    },
                )
            }
            "excluir" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    project.delete_layer(id)?;
                    Ok(Change::new(format!("Camada {id} excluída.")))
                })
            }
            "atribuir" => {
                let layer_id = optional_count(args, "id")?;
                let wall_id = optional_count(args, "parede_id")?;
                let drawing_id = optional_count(args, "desenho_id")?;
                if wall_id.is_none() && drawing_id.is_none() {
                    return Err(
                        "Informe 'parede_id' e/ou 'desenho_id' para atribuir à camada.".to_owned(),
                    );
                }
                self.apply_change(Some("O elemento já estava nessa camada."), |project| {
                    if let Some(wid) = wall_id {
                        project.set_wall_layer(wid, layer_id)?;
                    }
                    if let Some(did) = drawing_id {
                        project.set_drawing_layer(did, layer_id)?;
                    }
                    Ok(Change::new("Camada atribuída ao elemento.".to_owned()))
                })
            }
            _ => Err("Informe 'acao': listar, criar, editar, excluir ou atribuir.".to_owned()),
        }
    }
}
