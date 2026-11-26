//! The technical views tool: sections, elevations and details.

use serde_json::{Value, json};

use super::super::{
    Change, documents_dir, element_id, ensure_parent, file_stem, number, required_number,
    resolve_path, text, to_model, unique_path, xy,
};
use super::{parse_elevation_side, parse_scale, round2};
use crate::{
    app::FloorPlanApp,
    domain::{
        sheet::DrawingScale,
        views::{detail_svg, elevation_svg, export_view_pdf, export_view_svg, section_svg},
    },
    mcp::ToolOutput,
};

impl FloorPlanApp {
    /// Architectural sections, facade elevations and detail callouts (`vistas_tecnicas`).
    pub(in crate::app::mcp_tools) fn tool_views(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        if text(args, "acao").as_deref() == Some("editar") {
            return self.tool_edit_view(args);
        }
        let action = text(args, "acao")
            .unwrap_or_else(|| "listar".to_owned())
            .to_lowercase();
        match action.as_str() {
            "listar" => {
                let sections: Vec<Value> = self
                    .project
                    .views
                    .sections
                    .iter()
                    .map(|s| {
                        json!({
                            "id": s.id,
                            "rotulo": s.label,
                            "inicio": xy(s.start),
                            "fim": xy(s.end),
                            "olhar_esquerda": s.look_left,
                            "prancha_ref": s.target_sheet,
                        })
                    })
                    .collect();
                let elevations: Vec<Value> = self
                    .project
                    .views
                    .elevations
                    .iter()
                    .map(|e| {
                        json!({
                            "id": e.id,
                            "rotulo": e.label,
                            "lado": e.side.short().to_lowercase(),
                            "prancha_ref": e.target_sheet,
                        })
                    })
                    .collect();
                let details: Vec<Value> = self
                    .project
                    .views
                    .details
                    .iter()
                    .map(|d| {
                        json!({
                            "id": d.id,
                            "rotulo": d.label,
                            "centro": xy(d.center),
                            "raio_m": round2(d.radius),
                            "escala": d.scale.label(),
                            "prancha_ref": d.target_sheet,
                        })
                    })
                    .collect();
                Ok(ToolOutput::ok(format!(
                    "{} corte(s), {} fachada(s), {} detalhe(s).",
                    sections.len(),
                    elevations.len(),
                    details.len()
                ))
                .with_structured(json!({
                    "projeto_id": self.active_project_id(),
                    "cortes": sections,
                    "fachadas": elevations,
                    "detalhes": details,
                })))
            }
            "adicionar_corte" => {
                let start = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
                let end = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
                let label = text(args, "rotulo").unwrap_or_default();
                let look_left = args
                    .get("olhar_esquerda")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let target_sheet = text(args, "prancha_ref");
                self.apply_change(None, |project| {
                    let id = project.add_section_line(
                        &label,
                        start,
                        end,
                        look_left,
                        target_sheet.clone(),
                    )?;
                    let saved = project
                        .views
                        .sections
                        .iter()
                        .find(|s| s.id == id)
                        .ok_or("Corte não encontrado após inserção.")?;
                    Ok(Change::new(format!("Corte {} adicionado (id {id}).", saved.label))
                        .data(json!({ "id": id, "rotulo": saved.label })))
                })
            }
            "adicionar_corte_padrao" => {
                let horizontal = args
                    .get("horizontal")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                self.apply_change(None, |project| {
                    let id = project.add_default_section(horizontal)?;
                    let saved = project
                        .views
                        .sections
                        .iter()
                        .find(|s| s.id == id)
                        .ok_or("Corte padrão não encontrado após criação.")?;
                    Ok(Change::new(format!("Corte padrão {} criado (id {id}).", saved.label))
                        .data(json!({ "id": id, "rotulo": saved.label })))
                })
            }
            "adicionar_fachada" => {
                let side_raw = text(args, "lado").ok_or("Informe 'lado': norte, sul, leste ou oeste.")?;
                let side = parse_elevation_side(&side_raw)
                    .ok_or("'lado' deve ser norte, sul, leste ou oeste.")?;
                let target_sheet = text(args, "prancha_ref");
                self.apply_change(None, |project| {
                    let id = project.add_elevation_view(side, target_sheet.clone());
                    Ok(Change::new(format!("{} adicionada (id {id}).", side.label()))
                        .data(json!({ "id": id, "rotulo": side.label() })))
                })
            }
            "adicionar_detalhe" => {
                let center = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let radius = number(args, "raio_m")?.unwrap_or(1.0);
                let label = text(args, "rotulo").unwrap_or_default();
                let scale = match text(args, "escala") {
                    Some(raw) => parse_scale(&raw)
                        .ok_or("'escala' inválida (ex.: 1:25, 1:50, 1:20).")?,
                    None => DrawingScale::S1_25,
                };
                let target_sheet = text(args, "prancha_ref");
                self.apply_change(None, |project| {
                    let id = project.add_detail_callout(
                        &label,
                        center,
                        radius,
                        scale,
                        target_sheet.clone(),
                    )?;
                    let saved = project
                        .views
                        .details
                        .iter()
                        .find(|d| d.id == id)
                        .ok_or("Detalhe não encontrado após inserção.")?;
                    Ok(Change::new(format!("Detalhe {} adicionado (id {id}).", saved.label))
                        .data(json!({ "id": id, "rotulo": saved.label })))
                })
            }
            "remover" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    let removed = project.remove_section(id)
                        || project.remove_elevation(id)
                        || project.remove_detail_callout(id);
                    if !removed {
                        return Err("Vista técnica (corte, fachada ou detalhe) não encontrada.".to_owned());
                    }
                    Ok(Change::new(format!("Vista técnica {id} removida.")))
                })
            }
            "exportar" => {
                let id = element_id(args, "id")?;
                let format = text(args, "formato")
                    .unwrap_or_else(|| "svg".to_owned())
                    .to_lowercase();
                if format != "svg" && format != "pdf" {
                    return Err("'formato' deve ser svg ou pdf.".to_owned());
                }
                let (label, markup) = if let Some(s) =
                    self.project.views.sections.iter().find(|s| s.id == id)
                {
                    (format!("corte-{}", s.label), section_svg(&self.project, s))
                } else if let Some(e) =
                    self.project.views.elevations.iter().find(|e| e.id == id)
                {
                    (
                        format!("fachada-{}", e.side.short().to_lowercase()),
                        elevation_svg(&self.project, e),
                    )
                } else if let Some(d) = self.project.views.details.iter().find(|d| d.id == id) {
                    (format!("detalhe-{}", d.label), detail_svg(&self.project, d))
                } else {
                    return Err("Vista técnica não encontrada.".to_owned());
                };
                let path = match text(args, "caminho") {
                    Some(input) => resolve_path(&input, &format),
                    None => unique_path(documents_dir().join(format!(
                        "{}-{}.{format}",
                        file_stem(&self.project.name),
                        file_stem(&label)
                    ))),
                };
                ensure_parent(&path)?;
                if format == "pdf" {
                    export_view_pdf(&markup, &path)?;
                } else {
                    export_view_svg(&markup, &path)?;
                }
                Ok(ToolOutput::ok(format!(
                    "Vista técnica exportada em {}",
                    path.display()
                ))
                .with_structured(json!({
                    "id": id,
                    "formato": format,
                    "caminho": path.display().to_string(),
                })))
            }
            _ => Err(
                "Informe 'acao': listar, adicionar_corte, adicionar_corte_padrao, adicionar_fachada, adicionar_detalhe, remover ou exportar."
                    .to_owned(),
            ),
        }
    }
}
