//! MCP tools for Phase 2 (`T-08`, `T-09`, `T-10`):
//! - `codigos_normativos`: layered codes (`Base -> Amendments -> Project`), presets, XML/JSON export/import
//! - `local_clima`: shared site & climate parameters + Brazilian city presets synced with `V0`
//! - `verificar_normas`: real-time architectural code compliance report + interactive `resolver_escada` (`solve_stair`)

use serde_json::{Value, json};

use super::{Change, ToolOutput, element_id, number, text};
use crate::app::FloorPlanApp;
use crate::domain::codes::{
    CodeDiscipline, KeraunicLevel, StairLocks, apply_stair_proposal, builtin_presets,
    builtin_site_presets, check_project_codes, solve_stair, xml,
};

impl FloorPlanApp {
    pub(super) fn tool_building_codes(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "listar".to_owned());
        match action.as_str() {
            "listar" => {
                let presets: Vec<Value> = builtin_presets()
                    .iter()
                    .map(|p| {
                        json!({
                            "id": p.id,
                            "nome": p.name,
                            "regiao": p.region,
                            "nota": p.note,
                        })
                    })
                    .collect();
                let mut active = serde_json::Map::new();
                for disc in CodeDiscipline::ALL {
                    let base = self.project.codes.active_base_key(disc);
                    let amends = self.project.codes.choice(disc).amendments.clone();
                    let pinned = self.project.codes.choice(disc).editions.clone();
                    let avail: Vec<Value> = self
                        .project
                        .codes
                        .available_codes(disc)
                        .into_iter()
                        .map(|c| {
                            json!({
                                "codigo": c.code,
                                "nome": c.name,
                                "edicao": c.edition,
                                "emenda_de": c.amends,
                                "auditado": c.is_checked(),
                            })
                        })
                        .collect();
                    active.insert(
                        disc.key().to_owned(),
                        json!({
                            "base": base,
                            "emendas": amends,
                            "edicoes_pinadas": pinned,
                            "disponiveis": avail,
                        }),
                    );
                }
                let drifts: Vec<Value> = self
                    .project
                    .codes
                    .edition_drifts()
                    .into_iter()
                    .map(|d| {
                        json!({
                            "disciplina": d.discipline.key(),
                            "codigo": d.code,
                            "edicao_pinada": d.pinned,
                            "edicao_disponivel": d.available,
                        })
                    })
                    .collect();
                Ok(ToolOutput::ok("Normas e camadas ativas consultadas.").with_structured(json!({
                    "projeto_id": self.active_project_id(),
                    "presets": presets,
                    "disciplinas": active,
                    "desvios_edicao": drifts,
                })))
            }
            "aplicar_preset" => {
                let preset_id = text(args, "preset")
                    .ok_or_else(|| "Informe 'preset' (ex.: generic, brasil, brasil-emergencia).".to_owned())?;
                self.apply_change(None, |project| {
                    let preset = project.codes.apply_preset(&preset_id)?;
                    Ok(Change::new(format!(
                        "Preset normativo aplicado: {}.",
                        preset.name
                    )))
                })
            }
            "definir_base" => {
                let disc = parse_discipline(args)?;
                let code = text(args, "codigo")
                    .ok_or_else(|| "Informe 'codigo' da norma base.".to_owned())?;
                self.apply_change(None, |project| {
                    project.codes.set_base(disc, &code)?;
                    Ok(Change::new(format!(
                        "Norma base de {} definida para '{code}'.",
                        disc.label()
                    )))
                })
            }
            "adicionar_emenda" => {
                let disc = parse_discipline(args)?;
                let code = text(args, "codigo")
                    .ok_or_else(|| "Informe 'codigo' da emenda.".to_owned())?;
                self.apply_change(None, |project| {
                    project.codes.add_amendment(disc, &code)?;
                    Ok(Change::new(format!(
                        "Emenda '{code}' adicionada em {}.",
                        disc.label()
                    )))
                })
            }
            "remover_emenda" => {
                let disc = parse_discipline(args)?;
                let code = text(args, "codigo")
                    .ok_or_else(|| "Informe 'codigo' da emenda.".to_owned())?;
                self.apply_change(None, |project| {
                    project.codes.remove_amendment(disc, &code);
                    Ok(Change::new(format!(
                        "Emenda '{code}' removida de {}.",
                        disc.label()
                    )))
                })
            }
            "definir_regra" => {
                let disc = parse_discipline(args)?;
                let section = text(args, "secao")
                    .ok_or_else(|| "Informe 'secao' (ex.: stairs, doors, ramps, rooms).".to_owned())?;
                let rule = text(args, "regra")
                    .ok_or_else(|| "Informe 'regra' (ex.: width_min, clear_width_min).".to_owned())?;
                let value = args
                    .get("valor")
                    .cloned()
                    .ok_or_else(|| "Informe 'valor' para a regra.".to_owned())?;
                self.apply_change(None, |project| {
                    project.codes.set_project_rule(disc, &section, &rule, value)?;
                    Ok(Change::new(format!(
                        "Regra {section}.{rule} sobrescrita no projeto."
                    )))
                })
            }
            "resetar_regra" => {
                let disc = parse_discipline(args)?;
                let section = text(args, "secao")
                    .ok_or_else(|| "Informe 'secao'.".to_owned())?;
                let rule = text(args, "regra");
                self.apply_change(None, |project| {
                    project
                        .codes
                        .reset_project_rule(disc, &section, rule.as_deref());
                    Ok(Change::new(format!(
                        "Ajuste do projeto restaurado em {section}."
                    )))
                })
            }
            "consultar_regras" => {
                let disc = parse_discipline(args)?;
                let section = text(args, "secao")
                    .ok_or_else(|| "Informe 'secao' (ex.: stairs, ramps, doors, rooms, utility_profile).".to_owned())?;
                let resolved = self.project.codes.resolve_section(disc, &section);
                let rules_json: Vec<Value> = resolved
                    .rules
                    .values()
                    .map(|r| {
                        json!({
                            "regra": r.key,
                            "valor": r.value,
                            "camada": r.layer.label(),
                            "tipo": r.kind.key(),
                            "fonte": r.source,
                            "auditado": r.checked,
                        })
                    })
                    .collect();
                Ok(ToolOutput::ok(format!(
                    "Seção '{section}' de {} com {} regra(s) resolvida(s).",
                    disc.label(),
                    rules_json.len()
                ))
                .with_structured(json!({
                    "projeto_id": self.active_project_id(),
                    "disciplina": disc.key(),
                    "secao": section,
                    "valor": resolved.value,
                    "regras": rules_json,
                })))
            }
            "exportar_xml" => {
                let xml_text = if let Some(d) = text(args, "disciplina")
                    .and_then(|s| CodeDiscipline::parse(&s))
                {
                    xml::export_code_xml(&self.project.codes.export_effective_pack(d))
                } else {
                    self.project.codes.export_all_xml()
                };
                Ok(ToolOutput::ok("Pacote de normas exportado em XML Formato 2.")
                    .with_structured(json!({ "formato": "xml", "conteudo": xml_text })))
            }
            "exportar_json" => {
                let packs: Vec<_> = if let Some(d) = text(args, "disciplina")
                    .and_then(|s| CodeDiscipline::parse(&s))
                {
                    vec![self.project.codes.export_effective_pack(d)]
                } else {
                    CodeDiscipline::ALL
                        .iter()
                        .map(|&d| self.project.codes.export_effective_pack(d))
                        .collect()
                };
                let json_text = xml::export_bundle_json(&packs)?;
                Ok(ToolOutput::ok("Pacote de normas exportado em JSON.")
                    .with_structured(json!({ "formato": "json", "conteudo": json_text })))
            }
            "importar" => {
                let content = text(args, "conteudo")
                    .ok_or_else(|| "Informe 'conteudo' (XML ou JSON do pacote de norma).".to_owned())?;
                let activate = args.get("ativar").and_then(Value::as_bool).unwrap_or(true);
                let mut count = 0usize;
                let mut left_out = Vec::new();
                let out = self.apply_change(None, |project| {
                    let imported = project.codes.import_packs(&content, activate)?;
                    count = imported.codes.len();
                    left_out = imported.left_out;
                    Ok(Change::new(format!(
                        "{count} pacote(s) de norma importado(s)."
                    )))
                })?;
                Ok(out.with_structured(json!({
                    "importados": count,
                    "ignorados": left_out,
                })))
            }
            _ => Err(
                "Informe 'acao': listar, aplicar_preset, definir_base, adicionar_emenda, remover_emenda, definir_regra, resetar_regra, consultar_regras, exportar_xml, exportar_json ou importar.".to_owned(),
            ),
        }
    }

    pub(super) fn tool_site_climate(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "consultar".to_owned());
        match action.as_str() {
            "consultar" => {
                let s = &self.project.codes.site;
                Ok(ToolOutput::ok(
                    s.assumption_line().unwrap_or_else(|| {
                        "Nenhum parâmetro de local/clima configurado.".to_owned()
                    }),
                )
                .with_structured(json!({
                    "projeto_id": self.active_project_id(),
                    "local_clima": s,
                    "vento_estrutural_v0_ms": self.project.structure.wind.v0_ms,
                })))
            }
            "listar_cidades" => {
                let cities: Vec<Value> = builtin_site_presets()
                    .iter()
                    .map(|p| {
                        json!({
                            "id": p.id,
                            "cidade": p.city,
                            "uf": p.state_uf,
                            "zona_bioclimatica": p.bioclimatic_zone,
                            "v0_ms": p.wind_v0_ms,
                            "chuva_mm_h": p.rain_intensity_mm_h,
                            "ceraunico": p.thunderstorms.code(),
                        })
                    })
                    .collect();
                Ok(
                    ToolOutput::ok(format!("{} cidades brasileiras disponíveis.", cities.len()))
                        .with_structured(json!({ "cidades": cities })),
                )
            }
            "aplicar_cidade" => {
                let city = text(args, "cidade").ok_or_else(|| {
                    "Informe 'cidade' (id ou nome da cidade brasileira).".to_owned()
                })?;
                self.apply_change(None, |project| {
                    let preset = project.apply_site_preset(&city)?;
                    Ok(Change::new(format!(
                        "Local/clima aplicado: {} / {} (ZB{}, V0={:.0} m/s, I={:.0} mm/h, {}).",
                        preset.city,
                        preset.state_uf,
                        preset.bioclimatic_zone,
                        preset.wind_v0_ms,
                        preset.rain_intensity_mm_h,
                        preset.thunderstorms.code()
                    )))
                })
            }
            "definir" => {
                let mut next = self.project.codes.site.clone();
                if let Some(c) = text(args, "cidade") {
                    next.city = Some(c);
                }
                if let Some(uf) = text(args, "uf") {
                    next.state_uf = Some(uf);
                }
                if let Some(v) = number(args, "latitude")? {
                    next.latitude = Some(v);
                }
                if let Some(v) = number(args, "longitude")? {
                    next.longitude = Some(v);
                }
                if let Some(v) = number(args, "altitude_m")? {
                    next.altitude_m = Some(v);
                }
                if let Some(v) = number(args, "zona_bioclimatica")? {
                    next.bioclimatic_zone = Some(v.round().clamp(0.0, 255.0) as u8);
                }
                if let Some(v) = number(args, "v0_ms")? {
                    next.wind_v0_ms = Some(v);
                }
                if let Some(v) = number(args, "chuva_mm_h")? {
                    next.rain_intensity_mm_h = Some(v);
                }
                if let Some(v) = number(args, "periodo_retorno_anos")? {
                    next.rain_return_period_years = Some(v);
                }
                if let Some(aq_raw) = text(args, "ceraunico") {
                    let aq = KeraunicLevel::parse(&aq_raw)
                        .ok_or_else(|| "'ceraunico' deve ser AQ1, AQ2 ou AQ3.".to_owned())?;
                    next.thunderstorms = Some(aq);
                }
                if let Some(v) = number(args, "dias_trovoada")? {
                    next.thunderstorm_days_year = Some(v);
                }
                if let Some(v) = number(args, "temp_verao_c")? {
                    next.cooling_design_c = Some(v);
                }
                if let Some(v) = number(args, "temp_inverno_c")? {
                    next.heating_design_c = Some(v);
                }
                self.apply_change(None, |project| {
                    project.set_site_climate(next)?;
                    Ok(Change::new(
                        "Parâmetros de local e clima atualizados.".to_owned(),
                    ))
                })
            }
            _ => Err(
                "Informe 'acao': consultar, listar_cidades, aplicar_cidade ou definir.".to_owned(),
            ),
        }
    }

    pub(super) fn tool_check_codes(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_else(|| "verificar".to_owned());
        match action.as_str() {
            "verificar" => {
                let report = check_project_codes(&self.project);
                let msg = format!(
                    "Códigos de obra ({}): {} achado(s) ({} requisito(s), {} recomendação(ões)).",
                    report.active_code,
                    report.findings.len(),
                    report.requirement_count(),
                    report.advisory_count()
                );
                Ok(ToolOutput::ok(msg)
                    .with_structured(serde_json::to_value(&report).unwrap_or_else(|_| json!({}))))
            }
            "resolver_escada" => {
                let stair_id = element_id(args, "id")?;
                let locks = StairLocks {
                    risers: args
                        .get("travar_espelhos")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    tread: args
                        .get("travar_piso")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    width: args
                        .get("travar_largura")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    landings: args
                        .get("travar_patamares")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                };
                let apply = args
                    .get("aplicar")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let proposal_idx = args
                    .get("indice_proposta")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;

                let stair = self
                    .project
                    .circulations
                    .iter()
                    .find(|c| c.id == stair_id)
                    .cloned()
                    .ok_or_else(|| format!("Escada #{stair_id} não encontrada."))?;
                let proposals = solve_stair(&self.project, &stair, locks);

                if apply {
                    let chosen = proposals.get(proposal_idx).cloned().ok_or_else(|| {
                        "Nenhuma proposta disponível no índice informado.".to_owned()
                    })?;
                    let out = self.apply_change(None, |project| {
                        let el = project
                            .circulations
                            .iter_mut()
                            .find(|c| c.id == stair_id)
                            .ok_or_else(|| format!("Escada #{stair_id} não encontrada."))?;
                        apply_stair_proposal(el, &chosen)?;
                        Ok(Change::new(format!(
                            "Solução aplicada à escada #{stair_id}: {} espelhos, piso {:.0} cm, largura {:.2} m.",
                            chosen.risers,
                            chosen.tread_m * 100.0,
                            chosen.width_m
                        )))
                    })?;
                    return Ok(out.with_structured(json!({
                        "aplicada": chosen,
                        "propostas": proposals,
                    })));
                }

                Ok(ToolOutput::ok(format!(
                    "{} proposta(s) gerada(s) pelo Solver para a escada #{stair_id}.",
                    proposals.len()
                ))
                .with_structured(json!({
                    "escada_id": stair_id,
                    "travas": locks,
                    "propostas": proposals,
                })))
            }
            _ => Err("Informe 'acao': verificar ou resolver_escada.".to_owned()),
        }
    }
}

fn parse_discipline(args: &Value) -> Result<CodeDiscipline, String> {
    let raw = text(args, "disciplina").unwrap_or_else(|| "architectural".to_owned());
    CodeDiscipline::parse(&raw).ok_or_else(|| {
        "'disciplina' deve ser architectural, structural, electrical, plumbing ou mechanical."
            .to_owned()
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::app::FloorPlanApp;
    use crate::domain::{circulation::StairShape, geometry::Point};

    #[test]
    fn mcp_codes_site_climate_stair_solver_and_collective_demand_work_end_to_end() {
        let ctx = eframe::egui::Context::default();
        let mut app = FloorPlanApp::new(&ctx);

        // 1. Apply Brasil preset via `codigos_normativos`
        let res = app.run_mcp_tool(
            "codigos_normativos",
            &json!({ "acao": "aplicar_preset", "preset": "brasil" }),
        );
        assert!(!res.is_error, "{res:?}");

        // 2. Apply city preset via `local_clima` and confirm structural wind V0 syncs
        let res = app.run_mcp_tool(
            "local_clima",
            &json!({ "acao": "aplicar_cidade", "cidade": "porto_alegre_rs" }),
        );
        assert!(!res.is_error, "{res:?}");
        assert!((app.project.structure.wind.v0_ms - 45.0).abs() < 1e-6);

        // 3. Add a non-compliant stair, check via `verificar_normas`, and fix via `resolver_escada`
        let sid = app
            .project
            .add_stair(Point::new(0.0, 0.0), StairShape::Straight, 0.90, 3.60, 0.24)
            .unwrap();
        let check = app.run_mcp_tool("verificar_normas", &json!({ "acao": "verificar" }));
        assert!(!check.is_error);
        let findings = check.structured.as_ref().unwrap()["findings"]
            .as_array()
            .unwrap();
        assert!(!findings.is_empty());

        let solved = app.run_mcp_tool(
            "verificar_normas",
            &json!({ "acao": "resolver_escada", "id": sid, "aplicar": true }),
        );
        assert!(!solved.is_error, "{solved:?}");
        let recheck = app.run_mcp_tool("verificar_normas", &json!({ "acao": "verificar" }));
        assert!(
            recheck.structured.as_ref().unwrap()["findings"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        // 4. Collective multifamily demand via `projeto_eletrico`
        let coll = app.run_mcp_tool(
            "projeto_eletrico",
            &json!({
                "acao": "demanda_coletiva",
                "concessionaria": "celesc",
                "quantidade": 8,
                "area_m2": 75.0,
                "iluminacao_comum_kw": 2.5
            }),
        );
        assert!(!coll.is_error, "{coll:?}");
        assert!(
            coll.structured.as_ref().unwrap()["total_demand"]
                .as_f64()
                .unwrap()
                > 15.0
        );

        // 5. Export XML & import round-trip via `codigos_normativos`
        let xml_out = app.run_mcp_tool(
            "codigos_normativos",
            &json!({ "acao": "exportar_xml", "disciplina": "architectural" }),
        );
        assert!(!xml_out.is_error);
        let xml_text = xml_out.structured.as_ref().unwrap()["conteudo"]
            .as_str()
            .unwrap()
            .to_owned();
        let imp = app.run_mcp_tool(
            "codigos_normativos",
            &json!({ "acao": "importar", "conteudo": xml_text, "ativar": false }),
        );
        assert!(!imp.is_error, "{imp:?}");
    }
}
