//! MCP tool `planejamento_obra` for Section H ("From model to construction"):
//! #135 materials catalogue, #136 stage shopping list, #87 cost estimate &
//! SINAPI import, #137 construction schedule, #138 NR-18 safety & permits,
//! and #139 site progress log.

use serde_json::{Value, json};

use super::super::FloorPlanApp;
use super::{Change, number, text};
use crate::{
    domain::construction::materials::{
        MaterialOverride, materials_catalogue_csv, materials_catalogue_svg,
    },
    mcp::ToolOutput,
};

impl FloorPlanApp {
    pub(super) fn tool_construction_plan(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "catalogo_materiais".to_owned())
            .to_lowercase();

        match action.as_str() {
            "catalogo_materiais" => {
                let cat = self.project.materials_catalogue();
                let format = text(args, "formato").map(|f| f.to_lowercase());
                let exported = if let Some(fmt) = format.as_deref() {
                    let ext = match fmt {
                        "csv" => "csv",
                        "svg" => "svg",
                        "pdf" => "pdf",
                        other => {
                            return Err(format!(
                                "Formato '{other}' inválido; use csv, svg ou pdf."
                            ));
                        }
                    };
                    let path = super::resolve_path(
                        text(args, "caminho").as_deref(),
                        &format!(
                            "{}-catalogo-materiais.{ext}",
                            super::file_stem(&self.project)
                        ),
                    )?;
                    super::ensure_parent(&path)?;
                    match ext {
                        "csv" => std::fs::write(&path, materials_catalogue_csv(&self.project))
                            .map_err(|e| e.to_string())?,
                        "svg" => {
                            let svg = materials_catalogue_svg(&self.project);
                            crate::domain::views::export_view_svg(&svg, &path)?;
                        }
                        _ => {
                            let svg = materials_catalogue_svg(&self.project);
                            crate::domain::views::export_view_pdf(&svg, &path)?;
                        }
                    }
                    Some(path.display().to_string())
                } else {
                    None
                };

                let items: Vec<Value> = cat
                    .items
                    .iter()
                    .map(|m| {
                        json!({
                            "id": m.id,
                            "categoria": m.category.key(),
                            "categoria_rotulo": m.category.label(),
                            "nome": m.name,
                            "norma": m.standard,
                            "unidade": m.unit,
                            "densidade_kg_m3": m.density_kg_m3,
                            "resistencia_mpa": m.strength_mpa,
                            "perda_pct": m.waste_pct,
                            "traco_cimento_kg_m3": m.mix_cement_kg_m3,
                            "traco_cal_kg_m3": m.mix_lime_kg_m3,
                            "traco_areia_m3": m.mix_sand_m3_per_m3,
                            "traco_brita_m3": m.mix_gravel_m3_per_m3,
                            "em_uso": m.used_in_model,
                            "uso": m.usage_summary,
                            "editado": m.edited,
                        })
                    })
                    .collect();

                let summary = match &exported {
                    Some(p) => format!(
                        "Catálogo de materiais: {} itens ({} em uso, {} editado(s)); exportado em {p}.",
                        cat.items.len(),
                        cat.active_count,
                        cat.edited_count
                    ),
                    None => format!(
                        "Catálogo de materiais: {} itens ({} em uso no modelo, {} editado(s)).",
                        cat.items.len(),
                        cat.active_count,
                        cat.edited_count
                    ),
                };

                Ok(ToolOutput::ok(summary).with_structured(json!({
                    "total": cat.items.len(),
                    "em_uso": cat.active_count,
                    "editados": cat.edited_count,
                    "arquivo_exportado": exported,
                    "materiais": items,
                })))
            }
            "editar_material" => {
                let id = text(args, "material_id")
                    .ok_or("Informe 'material_id' (ex.: concrete_structural, block_default, mortar_laying).")?;
                let patch = MaterialOverride {
                    name: text(args, "nome"),
                    density_kg_m3: number(args, "densidade_kg_m3")?,
                    strength_mpa: number(args, "resistencia_mpa")?,
                    waste_pct: number(args, "perda_pct")?,
                    mix_cement_kg_m3: number(args, "traco_cimento_kg_m3")?,
                    mix_lime_kg_m3: number(args, "traco_cal_kg_m3")?,
                    mix_sand_m3_per_m3: number(args, "traco_areia_m3")?,
                    mix_gravel_m3_per_m3: number(args, "traco_brita_m3")?,
                };
                self.apply_change(None, |project| {
                    let msg = project.edit_catalogue_material(&id, patch.clone())?;
                    let cat = project.materials_catalogue();
                    let updated = cat.find(&id).map(|m| {
                        json!({
                            "id": m.id,
                            "nome": m.name,
                            "densidade_kg_m3": m.density_kg_m3,
                            "resistencia_mpa": m.strength_mpa,
                            "perda_pct": m.waste_pct,
                            "editado": m.edited,
                        })
                    });
                    Ok(Change::new(msg).data(json!({
                        "material": updated,
                        "fck_mpa": project.structure.settings.fck_mpa,
                    })))
                })
            }
            "restaurar_material" => {
                let id = text(args, "material_id")
                    .ok_or("Informe 'material_id' para restaurar ao padrão do modelo.")?;
                self.apply_change(Some("O material já estava no padrão."), |project| {
                    let existed = project.reset_catalogue_material(&id);
                    Ok(Change::new(if existed {
                        format!("Material '{id}' restaurado ao padrão do modelo.")
                    } else {
                        format!("Material '{id}' já estava no padrão do modelo.")
                    })
                    .data(json!({ "material_id": id, "restaurado": existed })))
                })
            }
            other => Err(format!(
                "Ação '{other}' desconhecida em planejamento_obra. Use: catalogo_materiais, editar_material ou restaurar_material."
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::app::FloorPlanApp;
    use crate::domain::samples::SampleProject;

    #[test]
    fn mcp_materials_catalogue_reads_edits_and_resets() {
        let mut app = FloorPlanApp::default();
        app.project = SampleProject::CasaTerrea.build();
        app.project.generate_structure();
        app.project.generate_electrical_points();
        app.project.assign_circuits();

        let res = app.run_mcp_tool(
            "planejamento_obra",
            &json!({ "acao": "catalogo_materiais" }),
        );
        assert!(!res.is_error, "{}", res.text);
        let data = res.structured.unwrap();
        assert!(data["total"].as_u64().unwrap() >= 10);
        assert!(data["em_uso"].as_u64().unwrap() >= 6);

        let edit = app.run_mcp_tool(
            "planejamento_obra",
            &json!({
                "acao": "editar_material",
                "material_id": "concrete_structural",
                "resistencia_mpa": 25.0,
                "perda_pct": 6.0
            }),
        );
        assert!(!edit.is_error, "{}", edit.text);
        assert!((app.project.structure.settings.fck_mpa - 25.0).abs() < 1e-6);

        let reset = app.run_mcp_tool(
            "planejamento_obra",
            &json!({
                "acao": "restaurar_material",
                "material_id": "concrete_structural"
            }),
        );
        assert!(!reset.is_error, "{}", reset.text);
        assert!(reset.structured.unwrap()["restaurado"].as_bool().unwrap());
    }
}
