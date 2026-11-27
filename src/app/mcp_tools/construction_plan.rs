//! MCP tool `planejamento_obra` for Section H ("From model to construction"):
//! #135 materials catalogue, #136 stage shopping list, #87 cost estimate &
//! SINAPI import, #137 construction schedule, #138 NR-18 safety & permits,
//! and #139 site progress log.

use serde_json::{Value, json};

use super::super::FloorPlanApp;
use super::{Change, flag, number, text};
use crate::{
    domain::construction::{
        cost::{cost_estimate_csv, cost_estimate_svg, edit_unit_price, format_brl},
        materials::{MaterialOverride, materials_catalogue_csv, materials_catalogue_svg},
        progress::{add_progress_entry, progress_log_csv, progress_log_svg, remove_progress_entry},
        safety_guide::{export_construction_bundle_pdf, safety_permits_svg, set_safety_check},
        schedule::{construction_schedule_csv, construction_schedule_gantt_svg},
        shopping::{ConstructionStage, stage_shopping_list_csv, stage_shopping_list_svg},
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
            "lista_compras" => {
                let stage_filter = match text(args, "etapa") {
                    Some(raw) => Some(ConstructionStage::parse(&raw).ok_or_else(|| {
                        format!(
                            "Etapa '{raw}' inválida. Use: preliminares, fundacoes, estrutura, alvenaria, cobertura, instalacoes, reboco ou acabamentos."
                        )
                    })?),
                    None => None,
                };
                let list = self.project.stage_shopping_list();
                let exported = if let Some(fmt) = text(args, "formato").map(|f| f.to_lowercase()) {
                    let ext = match fmt.as_str() {
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
                            "{}-lista-compras-etapas.{ext}",
                            super::file_stem(&self.project)
                        ),
                    )?;
                    super::ensure_parent(&path)?;
                    match ext {
                        "csv" => std::fs::write(&path, stage_shopping_list_csv(&self.project))
                            .map_err(|e| e.to_string())?,
                        "svg" => {
                            let svg = stage_shopping_list_svg(&self.project);
                            crate::domain::views::export_view_svg(&svg, &path)?;
                        }
                        _ => {
                            let svg = stage_shopping_list_svg(&self.project);
                            crate::domain::views::export_view_pdf(&svg, &path)?;
                        }
                    }
                    Some(path.display().to_string())
                } else {
                    None
                };

                let stages_json: Vec<Value> = list
                    .stages
                    .iter()
                    .filter(|g| stage_filter.is_none_or(|f| g.stage == f))
                    .map(|g| {
                        let items: Vec<Value> = g
                            .items
                            .iter()
                            .map(|it| {
                                json!({
                                    "codigo": it.item_code,
                                    "disciplina": it.discipline,
                                    "descricao": it.description,
                                    "liquido": super::round(it.net_quantity, 2),
                                    "comprar": super::round(it.buy_quantity, 2),
                                    "perda_pct": it.waste_pct,
                                    "unidade": it.unit,
                                    "observacao_comercial": it.commercial_note,
                                })
                            })
                            .collect();
                        json!({
                            "etapa": g.stage.key(),
                            "numero": g.stage.number(),
                            "rotulo": g.stage.label(),
                            "itens": items,
                        })
                    })
                    .collect();

                let item_count = list.all_items().count();
                let msg = match &exported {
                    Some(p) => format!(
                        "Lista de compras por etapa: {} etapas, {} itens ({} sc cimento 50 kg, {:.0} kg aço CA-50); exportada em {p}.",
                        list.stages.len(),
                        item_count,
                        list.summary.cement_bags_50kg,
                        list.summary.steel_ca50_kg
                    ),
                    None => format!(
                        "Lista de compras por etapa: {} etapas, {} itens ({} sc cimento 50 kg, {} sc cal 20 kg, {:.1} m³ areia, {:.1} m³ brita, {:.0} kg aço CA-50).",
                        list.stages.len(),
                        item_count,
                        list.summary.cement_bags_50kg,
                        list.summary.lime_bags_20kg,
                        list.summary.sand_m3,
                        list.summary.gravel_m3,
                        list.summary.steel_ca50_kg
                    ),
                };

                Ok(ToolOutput::ok(msg).with_structured(json!({
                    "total_itens": item_count,
                    "arquivo_exportado": exported,
                    "resumo": {
                        "cimento_sacos_50kg": list.summary.cement_bags_50kg,
                        "cal_sacos_20kg": list.summary.lime_bags_20kg,
                        "areia_m3": super::round(list.summary.sand_m3, 2),
                        "brita_m3": super::round(list.summary.gravel_m3, 2),
                        "concreto_estrutural_m3": super::round(list.summary.structural_concrete_m3, 2),
                        "concreto_magro_m3": super::round(list.summary.lean_concrete_m3, 2),
                        "aco_ca50_kg": super::round(list.summary.steel_ca50_kg, 1),
                        "aco_barras_12m": list.summary.steel_bars_12m,
                        "blocos_total": list.summary.total_blocks,
                        "cabos_eletricos_m": super::round(list.summary.wire_total_m, 1),
                        "eletrodutos_m": super::round(list.summary.conduit_total_m, 1),
                        "tubos_pvc_m": super::round(list.summary.pvc_pipe_total_m, 1),
                    },
                    "etapas": stages_json,
                    "premissas": list.assumptions,
                })))
            }
            "orcamento" => {
                let report = self.project.cost_estimate();
                let exported = if let Some(fmt) = text(args, "formato").map(|f| f.to_lowercase()) {
                    let ext = match fmt.as_str() {
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
                        &format!("{}-orcamento.{ext}", super::file_stem(&self.project)),
                    )?;
                    super::ensure_parent(&path)?;
                    match ext {
                        "csv" => std::fs::write(&path, cost_estimate_csv(&self.project))
                            .map_err(|e| e.to_string())?,
                        "svg" => {
                            let svg = cost_estimate_svg(&self.project);
                            crate::domain::views::export_view_svg(&svg, &path)?;
                        }
                        _ => {
                            let svg = cost_estimate_svg(&self.project);
                            crate::domain::views::export_view_pdf(&svg, &path)?;
                        }
                    }
                    Some(path.display().to_string())
                } else {
                    None
                };

                let stages_json: Vec<Value> = report
                    .stages
                    .iter()
                    .map(|st| {
                        json!({
                            "etapa": st.stage.key(),
                            "rotulo": st.stage.label(),
                            "itens": st.item_count,
                            "material_brl": super::round(st.material_brl, 2),
                            "mao_obra_brl": super::round(st.labor_brl, 2),
                            "bdi_brl": super::round(st.bdi_brl, 2),
                            "total_brl": super::round(st.total_brl, 2),
                            "participacao_pct": super::round(st.share_pct, 1),
                        })
                    })
                    .collect();
                let abc_json: Vec<Value> = report
                    .abc_curve
                    .iter()
                    .map(|row| {
                        json!({
                            "classe": row.class.to_string(),
                            "codigo": row.item_code,
                            "descricao": row.description,
                            "quantidade": super::round(row.total_quantity, 2),
                            "unidade": row.unit,
                            "total_brl": super::round(row.total_brl, 2),
                            "participacao_pct": super::round(row.share_pct, 1),
                            "acumulado_pct": super::round(row.cumulative_pct, 1),
                        })
                    })
                    .collect();

                let msg = match &exported {
                    Some(p) => format!("{} Exportado em {p}.", report.plain_summary),
                    None => report.plain_summary.clone(),
                };
                Ok(ToolOutput::ok(msg).with_structured(json!({
                    "resumo": report.plain_summary,
                    "referencia": report.reference_label,
                    "inclui_mao_obra": report.include_labor,
                    "bdi_pct": report.bdi_pct,
                    "area_util_m2": super::round(report.built_area_m2, 2),
                    "materiais_brl": super::round(report.material_total_brl, 2),
                    "mao_obra_brl": super::round(report.labor_total_brl, 2),
                    "custo_direto_brl": super::round(report.direct_total_brl, 2),
                    "bdi_brl": super::round(report.bdi_total_brl, 2),
                    "total_brl": super::round(report.grand_total_brl, 2),
                    "custo_por_m2_brl": super::round(report.cost_per_m2_brl, 2),
                    "arquivo_exportado": exported,
                    "etapas": stages_json,
                    "curva_abc": abc_json,
                    "premissas": report.assumptions,
                })))
            }
            "editar_preco" => {
                let code = text(args, "codigo_item")
                    .ok_or("Informe 'codigo_item' (ex.: cimento_50kg, aco_ca50_kg, areia_m3).")?;
                let mat = number(args, "preco_material_brl")?;
                let lab = number(args, "preco_mao_obra_brl")?;
                if mat.is_none() && lab.is_none() {
                    return Err(
                        "Informe 'preco_material_brl' e/ou 'preco_mao_obra_brl'.".to_owned()
                    );
                }
                self.apply_change(None, |project| {
                    let msg = edit_unit_price(project, &code, mat, lab)?;
                    let est = project.cost_estimate();
                    Ok(Change::new(format!(
                        "{msg} Novo total estimado: {}.",
                        format_brl(est.grand_total_brl)
                    ))
                    .data(json!({
                        "codigo_item": code,
                        "total_brl": super::round(est.grand_total_brl, 2),
                        "custo_por_m2_brl": super::round(est.cost_per_m2_brl, 2),
                    })))
                })
            }
            "configurar_orcamento" => {
                let bdi = number(args, "bdi_pct")?;
                let include_labor = args
                    .get("incluir_mao_obra")
                    .map(|_| flag(args, "incluir_mao_obra"));
                let reference = text(args, "referencia");
                if let Some(v) = bdi
                    && !(0.0..=50.0).contains(&v)
                {
                    return Err("O BDI deve ficar entre 0 e 50 %.".to_owned());
                }
                self.apply_change(None, |project| {
                    if let Some(v) = bdi {
                        project.construction.cost.bdi_pct = v;
                    }
                    if let Some(il) = include_labor {
                        project.construction.cost.include_labor = il;
                    }
                    if let Some(r) = &reference {
                        project.construction.cost.sinapi_reference = r.clone();
                    }
                    let est = project.cost_estimate();
                    Ok(Change::new(est.plain_summary.clone()).data(json!({
                        "bdi_pct": est.bdi_pct,
                        "inclui_mao_obra": est.include_labor,
                        "referencia": est.reference_label,
                        "total_brl": super::round(est.grand_total_brl, 2),
                    })))
                })
            }
            "importar_sinapi" => {
                let content = if let Some(csv) = text(args, "conteudo_csv") {
                    csv
                } else if let Some(raw_path) = text(args, "caminho") {
                    let resolved = super::resolve_path(Some(&raw_path), "sinapi.csv")?;
                    std::fs::read_to_string(&resolved).map_err(|e| {
                        format!("Não foi possível ler '{}': {e}", resolved.display())
                    })?
                } else {
                    return Err(
                        "Informe 'conteudo_csv' ou 'caminho' com a tabela SINAPI (CSV/TSV)."
                            .to_owned(),
                    );
                };
                let reference = text(args, "referencia");
                self.apply_change(None, |project| {
                    let res = project.import_sinapi_table(&content, reference.as_deref())?;
                    let est = project.cost_estimate();
                    Ok(Change::new(format!(
                        "SINAPI '{}' importado: {} preços atualizados ({} ignorados). {}",
                        res.reference_label, res.imported, res.skipped, est.plain_summary
                    ))
                    .data(json!({
                        "importados": res.imported,
                        "ignorados": res.skipped,
                        "referencia": res.reference_label,
                        "codigos_atualizados": res.matched_codes,
                        "total_brl": super::round(est.grand_total_brl, 2),
                        "custo_por_m2_brl": super::round(est.cost_per_m2_brl, 2),
                    })))
                })
            }
            "cronograma" => {
                let report = self.project.construction_schedule();
                let exported = if let Some(fmt) = text(args, "formato").map(|f| f.to_lowercase()) {
                    let ext = match fmt.as_str() {
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
                        &format!("{}-cronograma.{ext}", super::file_stem(&self.project)),
                    )?;
                    super::ensure_parent(&path)?;
                    match ext {
                        "csv" => std::fs::write(&path, construction_schedule_csv(&self.project))
                            .map_err(|e| e.to_string())?,
                        "svg" => {
                            let svg = construction_schedule_gantt_svg(&self.project);
                            crate::domain::views::export_view_svg(&svg, &path)?;
                        }
                        _ => {
                            let svg = construction_schedule_gantt_svg(&self.project);
                            crate::domain::views::export_view_pdf(&svg, &path)?;
                        }
                    }
                    Some(path.display().to_string())
                } else {
                    None
                };

                let tasks_json: Vec<Value> = report
                    .tasks
                    .iter()
                    .map(|t| {
                        json!({
                            "id": t.id,
                            "etapa": t.stage.key(),
                            "atividade": t.title,
                            "tipo": t.kind.key(),
                            "equipe": t.crew_label,
                            "produtividade": t.productivity_note,
                            "dependencias": t.dependencies,
                            "duracao_dias": t.duration_days,
                            "inicio_dia": t.early_start + 1,
                            "fim_dia": t.early_finish,
                            "folga_dias": t.slack_days,
                            "critica": t.is_critical,
                            "custo_brl": super::round(t.cost_brl, 2),
                        })
                    })
                    .collect();
                let periods_json: Vec<Value> = report
                    .periods
                    .iter()
                    .map(|p| {
                        json!({
                            "semana": p.week,
                            "periodo": p.label,
                            "desembolso_brl": super::round(p.cost_brl, 2),
                            "acumulado_brl": super::round(p.cumulative_cost_brl, 2),
                            "fisico_pct": super::round(p.physical_pct, 1),
                            "acumulado_pct": super::round(p.cumulative_pct, 1),
                            "etapas_ativas": p.active_stages,
                        })
                    })
                    .collect();

                let msg = match &exported {
                    Some(p) => format!("{} Exportado em {p}.", report.plain_summary),
                    None => report.plain_summary.clone(),
                };
                Ok(ToolOutput::ok(msg).with_structured(json!({
                    "resumo": report.plain_summary,
                    "dias_uteis_total": report.total_workdays,
                    "semanas_total": report.total_weeks,
                    "meses_estimados": super::round(report.total_months_est, 1),
                    "custo_total_brl": super::round(report.total_cost_brl, 2),
                    "caminho_critico": report.critical_path_ids,
                    "arquivo_exportado": exported,
                    "atividades": tasks_json,
                    "fisico_financeiro": periods_json,
                    "premissas": report.assumptions,
                })))
            }
            "configurar_cronograma" => {
                let wpw = super::optional_count(args, "dias_semana")?;
                let masons = super::optional_count(args, "pedreiros")?;
                let helpers = super::optional_count(args, "serventes")?;
                let elec = super::optional_count(args, "eletricistas")?;
                let plumb = super::optional_count(args, "encanadores")?;
                let start_date = text(args, "data_inicio");
                self.apply_change(None, |project| {
                    let s = &mut project.construction.schedule;
                    if let Some(v) = wpw {
                        s.workdays_per_week = (v as u8).clamp(5, 6);
                    }
                    if let Some(v) = masons {
                        s.masons = (v as u32).clamp(1, 10);
                    }
                    if let Some(v) = helpers {
                        s.helpers = (v as u32).clamp(1, 12);
                    }
                    if let Some(v) = elec {
                        s.electricians = (v as u32).clamp(1, 4);
                    }
                    if let Some(v) = plumb {
                        s.plumbers = (v as u32).clamp(1, 4);
                    }
                    if let Some(d) = &start_date {
                        s.start_date = (!d.trim().is_empty()).then(|| d.trim().to_owned());
                    }
                    let sched = project.construction_schedule();
                    Ok(Change::new(sched.plain_summary.clone()).data(json!({
                        "dias_uteis_total": sched.total_workdays,
                        "semanas_total": sched.total_weeks,
                        "caminho_critico": sched.critical_path_ids,
                    })))
                })
            }
            "seguranca_licencas" => {
                let stage_filter = match text(args, "etapa") {
                    Some(raw) => Some(ConstructionStage::parse(&raw).ok_or_else(|| {
                        format!(
                            "Etapa '{raw}' inválida. Use: preliminares, fundacoes, estrutura, alvenaria, cobertura, instalacoes, reboco ou acabamentos."
                        )
                    })?),
                    None => None,
                };
                let guide = self.project.site_safety_and_permits();
                let exported = if let Some(fmt) = text(args, "formato").map(|f| f.to_lowercase()) {
                    let ext = match fmt.as_str() {
                        "svg" => "svg",
                        "pdf" => "pdf",
                        other => {
                            return Err(format!(
                                "Formato '{other}' inválido para seguranca_licencas; use svg ou pdf."
                            ));
                        }
                    };
                    let path = super::resolve_path(
                        text(args, "caminho").as_deref(),
                        &format!(
                            "{}-seguranca-nr18-licencas.{ext}",
                            super::file_stem(&self.project)
                        ),
                    )?;
                    super::ensure_parent(&path)?;
                    let svg = safety_permits_svg(&self.project);
                    if ext == "pdf" {
                        crate::domain::views::export_view_pdf(&svg, &path)?;
                    } else {
                        crate::domain::views::export_view_svg(&svg, &path)?;
                    }
                    Some(path.display().to_string())
                } else {
                    None
                };

                let stages_json: Vec<Value> = guide
                    .stages
                    .iter()
                    .filter(|s| stage_filter.is_none_or(|f| s.stage == f))
                    .map(|st| {
                        let items: Vec<Value> = st
                            .items
                            .iter()
                            .map(|it| {
                                json!({
                                    "id": it.id,
                                    "norma": it.norm,
                                    "prioridade": it.priority.key(),
                                    "texto": it.text,
                                    "conferido": it.checked,
                                })
                            })
                            .collect();
                        json!({
                            "etapa": st.stage.key(),
                            "rotulo": st.stage.label(),
                            "epis": st.ppe,
                            "itens": items,
                        })
                    })
                    .collect();
                let permits_json: Vec<Value> = guide
                    .permits
                    .iter()
                    .map(|p| {
                        json!({
                            "id": p.id,
                            "titulo": p.title,
                            "orgao": p.authority,
                            "quando": p.when_needed,
                            "motivo": p.why_it_matters,
                            "obrigatorio": p.mandatory,
                        })
                    })
                    .collect();

                let msg = match &exported {
                    Some(p) => format!(
                        "Guia de segurança NR-18 e licenças: {}/{} itens conferidos ({} etapas, {} licenças/documentos); exportado em {p}.",
                        guide.completed_checks,
                        guide.total_checks,
                        guide.stages.len(),
                        guide.permits.len()
                    ),
                    None => format!(
                        "Guia de segurança NR-18 e licenças: {}/{} itens conferidos em {} etapas e {} requisitos legais (ART/RRT, alvará, CNO, habite-se e ATHIS).",
                        guide.completed_checks,
                        guide.total_checks,
                        guide.stages.len(),
                        guide.permits.len()
                    ),
                };

                Ok(ToolOutput::ok(msg).with_structured(json!({
                    "conferidos": guide.completed_checks,
                    "total_itens": guide.total_checks,
                    "alertas_terreno": guide.site_alerts,
                    "ajuda_gratuita": guide.help_guidance,
                    "arquivo_exportado": exported,
                    "etapas": stages_json,
                    "licencas": permits_json,
                })))
            }
            "marcar_checklist" => {
                let item_id = text(args, "item_id").ok_or(
                    "Informe 'item_id' do item de segurança NR-18 (ex.: nr18_s2_escoramento_vala).",
                )?;
                let checked = args
                    .get("conferido")
                    .map(|_| flag(args, "conferido"))
                    .unwrap_or(true);
                self.apply_change(None, |project| {
                    let msg = set_safety_check(project, &item_id, checked)?;
                    let g = project.site_safety_and_permits();
                    Ok(Change::new(msg).data(json!({
                        "item_id": item_id,
                        "conferido": checked,
                        "conferidos": g.completed_checks,
                        "total_itens": g.total_checks,
                    })))
                })
            }
            "pacote_obra" => {
                let path = super::resolve_path(
                    text(args, "caminho").as_deref(),
                    &format!("{}-pacote-obra.pdf", super::file_stem(&self.project)),
                )?;
                super::ensure_parent(&path)?;
                let pages = export_construction_bundle_pdf(&self.project, &path)?;
                Ok(ToolOutput::ok(format!(
                    "Pacote completo de planejamento de obra ({pages} páginas A3: catálogo de materiais, compras por etapa, orçamento SINAPI, cronograma Gantt e guia NR-18/licenças) exportado em {}.",
                    path.display()
                ))
                .with_structured(json!({
                    "arquivo": path.display().to_string(),
                    "paginas": pages,
                })))
            }
            "diario_obra" => {
                if let Some(day) = super::optional_count(args, "dia_referencia")? {
                    self.project.construction.progress_reference_day = day as u32;
                }
                let report = self.project.progress_report();
                let exported = if let Some(fmt) = text(args, "formato").map(|f| f.to_lowercase()) {
                    let ext = match fmt.as_str() {
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
                        &format!("{}-diario-obra.{ext}", super::file_stem(&self.project)),
                    )?;
                    super::ensure_parent(&path)?;
                    match ext {
                        "csv" => std::fs::write(&path, progress_log_csv(&self.project))
                            .map_err(|e| e.to_string())?,
                        "svg" => {
                            let svg = progress_log_svg(&self.project);
                            crate::domain::views::export_view_svg(&svg, &path)?;
                        }
                        _ => {
                            let svg = progress_log_svg(&self.project);
                            crate::domain::views::export_view_pdf(&svg, &path)?;
                        }
                    }
                    Some(path.display().to_string())
                } else {
                    None
                };

                let stages_json: Vec<Value> = report
                    .stages
                    .iter()
                    .map(|st| {
                        json!({
                            "etapa": st.stage.key(),
                            "rotulo": st.stage.label(),
                            "previsto_pct": super::round(st.planned_pct, 1),
                            "realizado_pct": super::round(st.actual_pct, 1),
                            "desvio_pp": super::round(st.variance_pct, 1),
                            "situacao": st.status.key(),
                            "orcado_brl": super::round(st.budget_brl, 2),
                            "valor_agregado_brl": super::round(st.earned_budget_brl, 2),
                            "gasto_real_brl": super::round(st.actual_cost_brl, 2),
                            "desvio_custo_brl": super::round(st.cost_variance_brl, 2),
                            "registros": st.entry_count,
                            "fotos": st.photo_count,
                            "ultima_data": st.last_date,
                            "ultima_nota": st.last_note,
                        })
                    })
                    .collect();
                let entries_json: Vec<Value> = report
                    .entries
                    .iter()
                    .map(|e| {
                        json!({
                            "id": e.id,
                            "data": e.date,
                            "etapa": e.stage.key(),
                            "avanco_pct": e.reported_pct,
                            "nota": e.note,
                            "fotos": e.photo_paths,
                            "gasto_brl": e.actual_cost_brl,
                        })
                    })
                    .collect();

                let msg = match &exported {
                    Some(p) => format!("{} Exportado em {p}.", report.plain_summary),
                    None => report.plain_summary.clone(),
                };
                Ok(ToolOutput::ok(msg).with_structured(json!({
                    "resumo": report.plain_summary,
                    "dia_referencia": report.reference_workday,
                    "dias_planejados": report.total_planned_workdays,
                    "previsto_geral_pct": super::round(report.overall_planned_pct, 1),
                    "realizado_geral_pct": super::round(report.overall_actual_pct, 1),
                    "desvio_geral_pp": super::round(report.overall_variance_pct, 1),
                    "orcado_total_brl": super::round(report.overall_budget_brl, 2),
                    "gasto_total_brl": super::round(report.overall_actual_cost_brl, 2),
                    "total_registros": report.total_entries,
                    "total_fotos": report.total_photos,
                    "arquivo_exportado": exported,
                    "etapas": stages_json,
                    "registros": entries_json,
                })))
            }
            "registrar_avanco" => {
                let raw_stage = text(args, "etapa")
                    .ok_or("Informe 'etapa' (ex.: preliminares, fundacoes, estrutura, alvenaria, cobertura, instalacoes, reboco ou acabamentos).")?;
                let stage = ConstructionStage::parse(&raw_stage)
                    .ok_or_else(|| format!("Etapa '{raw_stage}' inválida."))?;
                let date = text(args, "data").unwrap_or_else(|| "2026-11-27".to_owned());
                let pct = number(args, "avanco_pct")?.ok_or("Informe 'avanco_pct' (0 a 100).")?;
                let note = text(args, "nota").unwrap_or_default();
                let cost_brl = number(args, "gasto_brl")?;
                let ref_day = super::optional_count(args, "dia_referencia")?;
                let photos: Vec<String> = args
                    .get("fotos")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();

                self.apply_change(None, |project| {
                    if let Some(d) = ref_day {
                        project.construction.progress_reference_day = d as u32;
                    }
                    let id = add_progress_entry(
                        project,
                        date.clone(),
                        stage,
                        pct,
                        note.clone(),
                        photos.clone(),
                        cost_brl,
                    )?;
                    let report = project.progress_report();
                    Ok(Change::new(format!(
                        "Registro #{id} salvo no diário de obra ({} — {:.0} %). {}",
                        stage.short_label(),
                        pct,
                        report.plain_summary
                    ))
                    .data(json!({
                        "registro_id": id,
                        "realizado_geral_pct": super::round(report.overall_actual_pct, 1),
                        "previsto_geral_pct": super::round(report.overall_planned_pct, 1),
                        "total_fotos": report.total_photos,
                    })))
                })
            }
            "remover_registro" => {
                let id = super::optional_count(args, "registro_id")?
                    .ok_or("Informe 'registro_id' para remover do diário de obra.")?;
                self.apply_change(Some("Registro não encontrado."), |project| {
                    let removed = remove_progress_entry(project, id);
                    Ok(Change::new(if removed {
                        format!("Registro #{id} removido do diário de obra.")
                    } else {
                        format!("Registro #{id} não encontrado no diário de obra.")
                    })
                    .data(json!({ "registro_id": id, "removido": removed })))
                })
            }
            other => Err(format!(
                "Ação '{other}' desconhecida em planejamento_obra. Use: catalogo_materiais, editar_material, restaurar_material, lista_compras, orcamento, editar_preco, configurar_orcamento, importar_sinapi, cronograma, configurar_cronograma, seguranca_licencas, marcar_checklist, pacote_obra, diario_obra, registrar_avanco ou remover_registro."
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

        let shop = app.run_mcp_tool("planejamento_obra", &json!({ "acao": "lista_compras" }));
        assert!(!shop.is_error, "{}", shop.text);
        let sdata = shop.structured.unwrap();
        assert!(sdata["total_itens"].as_u64().unwrap() >= 10);
        assert!(sdata["resumo"]["cimento_sacos_50kg"].as_u64().unwrap() >= 20);

        let cost_before = app.run_mcp_tool("planejamento_obra", &json!({ "acao": "orcamento" }));
        assert!(!cost_before.is_error, "{}", cost_before.text);
        let total_before = cost_before.structured.unwrap()["total_brl"]
            .as_f64()
            .unwrap();
        assert!(total_before > 10_000.0);

        let imported = app.run_mcp_tool(
            "planejamento_obra",
            &json!({
                "acao": "importar_sinapi",
                "referencia": "SINAPI MG 2026-01",
                "conteudo_csv": "codigo;descricao;unidade;preco\n00001379;CIMENTO CP II 50KG;UN;58,00\n00000034;ACO CA-50;KG;15,00\n"
            }),
        );
        assert!(!imported.is_error, "{}", imported.text);
        let idata = imported.structured.unwrap();
        assert_eq!(idata["importados"].as_u64().unwrap(), 2);
        assert!(idata["total_brl"].as_f64().unwrap() > total_before);

        let sched = app.run_mcp_tool("planejamento_obra", &json!({ "acao": "cronograma" }));
        assert!(!sched.is_error, "{}", sched.text);
        let sc = sched.structured.unwrap();
        assert!(sc["dias_uteis_total"].as_u64().unwrap() >= 30);
        assert!(!sc["fisico_financeiro"].as_array().unwrap().is_empty());

        let safety = app.run_mcp_tool(
            "planejamento_obra",
            &json!({ "acao": "seguranca_licencas" }),
        );
        assert!(!safety.is_error, "{}", safety.text);
        let sf = safety.structured.unwrap();
        assert_eq!(sf["etapas"].as_array().unwrap().len(), 8);
        assert!(sf["licencas"].as_array().unwrap().len() >= 5);

        let mark = app.run_mcp_tool(
            "planejamento_obra",
            &json!({
                "acao": "marcar_checklist",
                "item_id": "nr18_s2_escoramento_vala",
                "conferido": true
            }),
        );
        assert!(!mark.is_error, "{}", mark.text);
        assert_eq!(mark.structured.unwrap()["conferidos"].as_u64().unwrap(), 1);

        let log_add = app.run_mcp_tool(
            "planejamento_obra",
            &json!({
                "acao": "registrar_avanco",
                "etapa": "fundacoes",
                "data": "2026-11-27",
                "avanco_pct": 75.0,
                "gasto_brl": 9500.0,
                "dia_referencia": 10,
                "nota": "Sapatas concretadas",
                "fotos": ["fotos/sapatas-01.jpg"]
            }),
        );
        assert!(!log_add.is_error, "{}", log_add.text);
        let lid = log_add.structured.unwrap()["registro_id"].as_u64().unwrap();

        let diary = app.run_mcp_tool("planejamento_obra", &json!({ "acao": "diario_obra" }));
        assert!(!diary.is_error, "{}", diary.text);
        let ddata = diary.structured.unwrap();
        assert_eq!(ddata["total_registros"].as_u64().unwrap(), 1);
        assert_eq!(ddata["total_fotos"].as_u64().unwrap(), 1);

        let rem = app.run_mcp_tool(
            "planejamento_obra",
            &json!({ "acao": "remover_registro", "registro_id": lid }),
        );
        assert!(!rem.is_error, "{}", rem.text);
        assert!(rem.structured.unwrap()["removido"].as_bool().unwrap());
    }
}
