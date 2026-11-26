//! The associative dimensions tool.

use serde_json::{Value, json};

use super::super::{
    Change, element_id, flag, number, optional_count, required_number, round, text, to_model,
};
use super::round2;
use crate::{
    app::FloorPlanApp,
    domain::dimensions::{DimensionAnchor, DimensionKind, ResolvedDimension},
    mcp::ToolOutput,
};

impl FloorPlanApp {
    /// Persistent associative dimensions (`cotas`).
    pub(in crate::app::mcp_tools) fn tool_dimensions(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "listar".to_owned())
            .to_lowercase();
        match action.as_str() {
            "listar" => {
                let dims: Vec<Value> = self
                    .project
                    .dimensions
                    .iter()
                    .map(|d| {
                        let resolved = self.project.resolve_dimension(d);
                        let (valor_m, rotulo) = match resolved {
                            Some(ResolvedDimension::Linear { value_m, label, .. }) => {
                                (Some(round(value_m)), Some(label))
                            }
                            Some(ResolvedDimension::Angular { angle_deg, label, .. }) => {
                                (Some(round2(angle_deg)), Some(label))
                            }
                            None => (None, None),
                        };
                        json!({
                            "id": d.id,
                            "tipo": d.kind.label(),
                            "valor": valor_m,
                            "rotulo": rotulo,
                            "texto_personalizado": d.override_text,
                            "visivel": d.visible,
                        })
                    })
                    .collect();
                Ok(ToolOutput::ok(format!("{} cota(s) associativa(s).", dims.len()))
                    .with_structured(json!({
                        "projeto_id": self.active_project_id(),
                        "cotas": dims,
                    })))
            }
            "gerar_internas" => self.apply_change(
                Some("Todos os cômodos já possuem cotas internas."),
                |project| {
                    let added = project.generate_room_internal_dimensions();
                    Ok(Change::new(format!(
                        "{added} cota(s) interna(s) gerada(s) nos cômodos."
                    ))
                    .data(json!({ "adicionadas": added })))
                },
            ),
            "gerar_aberturas" => {
                let wall_id = optional_count(args, "parede_id")?;
                let offset = number(args, "deslocamento")?.unwrap_or(0.45);
                self.apply_change(
                    Some("As cadeias de cotas de aberturas já existem (ou não há aberturas)."),
                    |project| {
                        let summary = project.generate_opening_dimension_chains(wall_id, offset)?;
                        Ok(Change::new(format!(
                            "{} cota(s) de abertura gerada(s) em {} parede(s).",
                            summary.dimensions_added, summary.walls_processed
                        ))
                        .data(json!({
                            "paredes": summary.walls_processed,
                            "adicionadas": summary.dimensions_added,
                        })))
                    },
                )
            }
            "gerar_completas" => {
                let offset = number(args, "deslocamento")?.unwrap_or(0.45);
                self.apply_change(
                    Some("Todas as cotas internas e cadeias de aberturas já existem."),
                    |project| {
                        let summary = project.generate_site_dimensions(offset);
                        Ok(Change::new(format!(
                            "{} cota(s) gerada(s) ({} interna(s), {} de abertura em {} parede(s)).",
                            summary.total_added(),
                            summary.internal_added,
                            summary.opening_chains.dimensions_added,
                            summary.opening_chains.walls_processed,
                        ))
                        .data(json!({
                            "internas": summary.internal_added,
                            "aberturas": summary.opening_chains.dimensions_added,
                            "paredes": summary.opening_chains.walls_processed,
                            "adicionadas": summary.total_added(),
                        })))
                    },
                )
            }
            "adicionar_parede" => {
                let wall_id = element_id(args, "parede_id")?;
                let offset = number(args, "deslocamento")?.unwrap_or(0.45);
                self.apply_change(None, |project| {
                    let id = project.add_wall_aligned_dimension(wall_id, offset)?;
                    Ok(Change::new(format!(
                        "Cota associativa {id} adicionada à parede {wall_id}."
                    ))
                    .data(json!({ "id": id })))
                })
            }
            "adicionar_alinhada" => {
                let p1 = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                let p2 = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                let offset = number(args, "deslocamento")?.unwrap_or(0.45);
                let external = flag(args, "externa");
                self.apply_change(None, |project| {
                    let a = DimensionAnchor::from_point(p1);
                    let b = DimensionAnchor::from_point(p2);
                    let kind = if external {
                        DimensionKind::External { a, b, offset }
                    } else {
                        DimensionKind::Aligned { a, b, offset }
                    };
                    let id = project.add_dimension(kind)?;
                    Ok(Change::new(format!("Cota {id} adicionada.")).data(json!({ "id": id })))
                })
            }
            "editar" => {
                let id = element_id(args, "id")?;
                let has_text = args.get("texto").is_some();
                let override_text = text(args, "texto");
                let offset = number(args, "deslocamento")?;
                if !has_text && offset.is_none() {
                    return Err("Informe 'texto' e/ou 'deslocamento'.".to_owned());
                }
                self.apply_change(Some("A cota já estava assim."), |project| {
                    if has_text {
                        project.update_dimension_override(id, override_text.clone())?;
                    }
                    if let Some(off) = offset {
                        project.update_dimension_offset(id, off)?;
                    }
                    Ok(Change::new(format!("Cota {id} atualizada.")))
                })
            }
            "remover" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.delete_dimension(id) {
                        return Err("Cota não encontrada.".to_owned());
                    }
                    Ok(Change::new(format!("Cota {id} removida.")))
                })
            }
            _ => Err(
                "Informe 'acao': listar, gerar_internas, gerar_aberturas, gerar_completas, adicionar_parede, adicionar_alinhada, editar ou remover."
                    .to_owned(),
            ),
        }
    }
}
