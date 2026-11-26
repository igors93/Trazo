//! The ABNT sheet tool: paper, scale, margins, title block and sheet sets.

use serde_json::{Value, json};

use super::super::{
    Change, documents_dir, element_id, ensure_parent, file_stem, required_number, resolve_path,
    text, unique_path,
};
use super::{parse_scale, round2};
use crate::{
    app::FloorPlanApp,
    domain::{
        sheet::{DrawingScale, Orientation, PaperSize, SheetMargins},
        sheet_set::{SheetContent, SheetStatus, red_checks},
    },
    mcp::ToolOutput,
};

impl FloorPlanApp {
    /// ABNT technical sheet format, scale and title block (`prancha`).
    pub(in crate::app::mcp_tools) fn tool_sheet(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "obter".to_owned())
            .to_lowercase();
        let sheet_json = |sheet: &crate::domain::sheet::SheetLayout| {
            let (w, h) = sheet.sheet_mm();
            let (_, _, pw, ph) = sheet.printable_rect_mm();
            json!({
                "formato": sheet.size.short_name(),
                "largura_mm": round2(w),
                "altura_mm": round2(h),
                "area_util_mm": [round2(pw), round2(ph)],
                "orientacao": match sheet.orientation {
                    Orientation::Landscape => "paisagem",
                    Orientation::Portrait => "retrato",
                },
                "escala": sheet.scale.label(),
                "carimbo": {
                    "titulo": sheet.title_block.drawing_title,
                    "cliente": sheet.title_block.client,
                    "endereco": sheet.title_block.address,
                    "autor": sheet.title_block.author,
                    "registro": sheet.title_block.registration,
                    "data": sheet.title_block.date,
                    "folha": sheet.title_block.sheet_number,
                    "revisao": sheet.title_block.revision,
                    "notas": sheet.title_block.notes,
                    "exibir_carimbo": sheet.title_block.show_title_block,
                    "exibir_legenda": sheet.title_block.show_legend,
                    "exibir_norte": sheet.title_block.show_north,
                    "exibir_escala": sheet.title_block.show_scale_bar,
                },
            })
        };
        match action.as_str() {
            "listar_conjunto" => {
                let red = red_checks(&self.project);
                let sheets: Vec<_> = self.project.sheet_set.sheets.iter().map(|sheet| json!({
                    "id": sheet.id,
                    "codigo": sheet.code,
                    "titulo": sheet.title,
                    "revisao": sheet.latest_revision_code(),
                    "situacao": sheet.effective_status(&red).label(),
                    "revisor": sheet.release_review.as_ref().map(|r| json!({
                        "nome": r.reviewer,
                        "registro": r.registration,
                        "data": r.date,
                    })),
                })).collect();
                Ok(ToolOutput::ok(format!("{} pranchas no conjunto; {} verificações vermelhas.", sheets.len(), red.len()))
                    .with_structured(json!({"pranchas": sheets, "verificacoes_vermelhas": red.len()})))
            }
            "adicionar_ao_conjunto" => {
                let title = text(args, "titulo").unwrap_or_else(|| "Planta Baixa".to_owned());
                self.apply_change(None, |project| {
                    let id = project.sheet_set.add_sheet(&title, SheetContent::FloorPlan, &project.sheet);
                    if let Some(sheet) = project.sheet_set.sheets.iter_mut().find(|s| s.id == id) {
                        sheet.discipline = crate::domain::discipline::Discipline::Architecture;
                    }
                    Ok(Change::new(format!("Prancha {id} adicionada ao conjunto."))
                        .data(json!({"id": id})))
                })
            }
            "definir_situacao" => {
                let id = element_id(args, "id")?;
                let status = match text(args, "situacao").unwrap_or_default().as_str() {
                    "em_elaboracao" => SheetStatus::EmElaboracao,
                    "para_aprovacao" => SheetStatus::ParaAprovacao,
                    "liberado_obra" => SheetStatus::LiberadoObra,
                    "as_built" => SheetStatus::AsBuilt,
                    _ => return Err("'situacao' deve ser em_elaboracao, para_aprovacao, liberado_obra ou as_built.".to_owned()),
                };
                let reviewer = text(args, "revisor").unwrap_or_default();
                let registration = text(args, "registro_revisor").unwrap_or_default();
                let date = text(args, "data_revisao").unwrap_or_default();
                self.apply_change(None, |project| {
                    project.set_sheet_status(id, status, Some((&reviewer, &registration, &date)))?;
                    Ok(Change::new(format!("Prancha {id}: {}.", status.label()))
                        .data(json!({"id": id, "situacao": status.label()})))
                })
            }
            "exportar_conjunto" => {
                let id = element_id(args, "id")?;
                let format = text(args, "formato").unwrap_or_else(|| "pdf".to_owned());
                if format != "pdf" && format != "svg" {
                    return Err("'formato' deve ser pdf ou svg.".to_owned());
                }
                let item = self.project.sheet_set.sheets.iter().find(|s| s.id == id)
                    .ok_or_else(|| "Prancha não encontrada.".to_owned())?;
                let path = match text(args, "caminho") {
                    Some(input) => resolve_path(&input, &format),
                    None => unique_path(documents_dir().join(format!("{}-{}.{}", file_stem(&self.project.name), item.code.to_lowercase(), format))),
                };
                ensure_parent(&path)?;
                if format == "pdf" {
                    crate::io::export::sheet::export_sheet_item_pdf(&self.project, item, &path)?;
                } else {
                    crate::io::export::sheet::export_sheet_item_svg(&self.project, item, &path)?;
                }
                Ok(ToolOutput::ok(format!("Prancha exportada em {}", path.display()))
                    .with_structured(json!({"caminho": path.display().to_string(), "situacao": item.effective_status(&red_checks(&self.project)).label()})))
            }
            "obter" => {
                let data = sheet_json(&self.project.sheet);
                Ok(ToolOutput::ok(format!(
                    "Prancha {} {} em escala {}.",
                    self.project.sheet.size.short_name(),
                    self.project.sheet.orientation.label().to_lowercase(),
                    self.project.sheet.scale.label()
                ))
                .with_structured(data))
            }
            "configurar" => {
                let mut next = self.project.sheet.clone();
                if let Some(raw) = text(args, "formato") {
                    let size = match raw.to_uppercase().as_str() {
                        "A4" => PaperSize::A4,
                        "A3" => PaperSize::A3,
                        "A2" => PaperSize::A2,
                        "A1" => PaperSize::A1,
                        "A0" => PaperSize::A0,
                        "PERSONALIZADO" | "CUSTOM" => {
                            let width_mm = required_number(args, "largura_mm")?;
                            let height_mm = required_number(args, "altura_mm")?;
                            PaperSize::Custom {
                                width_mm,
                                height_mm,
                            }
                        }
                        _ => {
                            return Err("'formato' deve ser A4, A3, A2, A1, A0 ou personalizado."
                                .to_owned());
                        }
                    };
                    next.size = size;
                    next.margins = SheetMargins::abnt_for(size);
                }
                if let Some(raw) = text(args, "orientacao") {
                    next.orientation = match raw.to_lowercase().as_str() {
                        "paisagem" | "landscape" => Orientation::Landscape,
                        "retrato" | "portrait" => Orientation::Portrait,
                        _ => return Err("'orientacao' deve ser paisagem ou retrato.".to_owned()),
                    };
                }
                if let Some(raw) = text(args, "escala") {
                    if matches!(
                        raw.to_lowercase().as_str(),
                        "melhor" | "auto" | "melhor_ajuste"
                    ) {
                        let (draw_w, draw_h) = next.drawing_area_mm();
                        let (plan_w, plan_h) = self
                            .project
                            .bounds()
                            .map(|(min, max)| ((max.x - min.x).max(1.0), (max.y - min.y).max(1.0)))
                            .unwrap_or((10.0, 10.0));
                        next.scale = DrawingScale::best_fit(plan_w, plan_h, draw_w, draw_h);
                    } else {
                        next.scale = parse_scale(&raw).ok_or(
                            "'escala' deve ser 1:25, 1:50, 1:75, 1:100, 1:200, 1:N ou 'melhor'."
                                .to_owned(),
                        )?;
                    }
                }
                if let Some(v) = text(args, "titulo") {
                    next.title_block.drawing_title = v;
                }
                if let Some(v) = text(args, "cliente") {
                    next.title_block.client = v;
                }
                if let Some(v) = text(args, "endereco") {
                    next.title_block.address = v;
                }
                if let Some(v) = text(args, "autor") {
                    next.title_block.author = v;
                }
                if let Some(v) = text(args, "registro") {
                    next.title_block.registration = v;
                }
                if let Some(v) = text(args, "data") {
                    next.title_block.date = v;
                }
                if let Some(v) = text(args, "folha") {
                    next.title_block.sheet_number = v;
                }
                if let Some(v) = text(args, "revisao") {
                    next.title_block.revision = v;
                }
                if let Some(v) = text(args, "notas") {
                    next.title_block.notes = v;
                }
                if let Some(v) = args.get("exibir_carimbo").and_then(Value::as_bool) {
                    next.title_block.show_title_block = v;
                }
                if let Some(v) = args.get("exibir_legenda").and_then(Value::as_bool) {
                    next.title_block.show_legend = v;
                }
                if let Some(v) = args.get("exibir_norte").and_then(Value::as_bool) {
                    next.title_block.show_north = v;
                }
                if let Some(v) = args.get("exibir_escala").and_then(Value::as_bool) {
                    next.title_block.show_scale_bar = v;
                }
                next.validate()?;
                let summary = sheet_json(&next);
                self.apply_change(
                    Some("A prancha já estava com essa configuração."),
                    |project| {
                        project.sheet = next.clone();
                        Ok(Change::new(format!(
                            "Prancha configurada: {} {} ({}).",
                            next.size.short_name(),
                            next.orientation.label().to_lowercase(),
                            next.scale.label()
                        ))
                        .data(summary))
                    },
                )
            }
            _ => Err("Informe 'acao': obter, configurar, listar_conjunto, adicionar_ao_conjunto, definir_situacao ou exportar_conjunto.".to_owned()),
        }
    }
}
