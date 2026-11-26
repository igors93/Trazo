//! Assistant tools for the electrical design: placing devices, editing their
//! circuit and power, generating the NBR 5410 minimum, distributing circuits,
//! laying conduits, choosing the supply and reading the load schedule with
//! the board (phases, voltage drop, demand and service entrance).

use serde_json::{Value, json};

use super::{
    Change, ToolOutput, element_id, number, quarter_turns, required_number, text, to_model, xy,
};
use crate::app::{FloorPlanApp, Selection};
use crate::domain::conduits::ConduitRun;
use crate::domain::distribution::{Supply, UtilityProfile, phase_label};
use crate::domain::fixtures::{Fixture, FixtureKind};

fn parse_utility_profile(raw: &str) -> Option<UtilityProfile> {
    UtilityProfile::parse(raw)
}

fn utility_profile_key(profile: UtilityProfile) -> &'static str {
    match profile {
        UtilityProfile::Generic => "generico",
        other => other.api_key(),
    }
}

fn run_arg(args: &Value) -> Result<ConduitRun, String> {
    match text(args, "trecho") {
        None => Ok(ConduitRun::Ceiling),
        Some(key) => ConduitRun::from_key(&key.to_lowercase())
            .ok_or_else(|| "'trecho' deve ser teto ou piso.".to_owned()),
    }
}

/// `circuito` as a change: absent keeps it, 0 removes it, n sets it.
fn circuit_arg(args: &Value) -> Result<Option<Option<u32>>, String> {
    match number(args, "circuito")? {
        None => Ok(None),
        Some(n) if n < 0.0 || n > 999.0 || n.fract() != 0.0 => Err(
            "'circuito' deve ser um número inteiro de 1 a 999 (0 para tirar do circuito)."
                .to_owned(),
        ),
        Some(0.0) => Ok(Some(None)),
        Some(n) => Ok(Some(Some(n as u32))),
    }
}

impl FloorPlanApp {
    pub(super) fn tool_insert_electrical(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let keys: Vec<&str> = FixtureKind::ELECTRICAL
            .iter()
            .map(|kind| kind.key())
            .collect();
        let kind = text(args, "tipo")
            .and_then(|kind| FixtureKind::from_key(&kind.to_lowercase()))
            .filter(|kind| kind.is_electrical())
            .ok_or_else(|| format!("Informe 'tipo': {}.", keys.join(", ")))?;
        let point = to_model(required_number(args, "x")?, required_number(args, "y")?);
        let turns = quarter_turns(args)?.unwrap_or(0);
        let circuit = circuit_arg(args)?;
        let power = number(args, "potencia_va")?;
        let height = number(args, "altura_m")?;
        let (width, depth) = kind.default_size();
        // Wall devices stand against the nearest wall; ceiling points stay put.
        let placed = if kind.mounts_on_wall() {
            self.place_against_wall(kind, point, width, depth, turns)
        } else {
            Fixture {
                id: 0,
                kind,
                center: point,
                width,
                depth,
                quarter_turns: turns,
                angle_rad: 0.0,
                electrical: kind.default_electrical(),
                structural_load_kn: None,
                pile: None,
            }
        };
        let on_wall = kind.mounts_on_wall() && placed.center != point;
        self.apply_change(None, |project| {
            let id = project
                .add_fixture(kind, placed.center, width, depth, placed.quarter_turns)
                .ok_or("Não foi possível inserir: o ponto precisa ficar dentro do terreno.")?;
            project.update_electrical(id, circuit, power, height)?;
            let data = project
                .fixtures
                .iter()
                .find(|fixture| fixture.id == id)
                .and_then(|fixture| fixture.electrical);
            let room = project
                .room_for_fixture(&placed)
                .map(|room| room.display_name().to_owned());
            Ok(Change::new(format!(
                "{} inserido(a) (id {id}){}{}.",
                kind.label(),
                room.as_ref()
                    .map_or(String::new(), |name| format!(" em {name}")),
                if on_wall { ", na face da parede" } else { "" }
            ))
            .select(Selection::Fixture(id))
            .data(json!({
                "id": id,
                "centro": xy(placed.center),
                "comodo": room,
                "circuito": data.and_then(|d| d.circuit),
                "potencia_va": data.map(|d| d.power_va),
                "altura_m": data.map(|d| d.height_m),
            })))
        })
    }

    pub(super) fn tool_edit_electrical(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let id = element_id(args, "id")?;
        let circuit = circuit_arg(args)?;
        let power = number(args, "potencia_va")?;
        let height = number(args, "altura_m")?;
        if circuit.is_none() && power.is_none() && height.is_none() {
            return Err("Informe circuito, potencia_va ou altura_m.".to_owned());
        }
        self.apply_change(Some("Nada mudou no ponto elétrico."), |project| {
            project.update_electrical(id, circuit, power, height)?;
            let fixture = project
                .fixtures
                .iter()
                .find(|fixture| fixture.id == id)
                .copied()
                .ok_or("Ponto elétrico não encontrado.")?;
            let data = fixture.electrical;
            Ok(
                Change::new(format!("{} (id {id}) atualizado(a).", fixture.kind.label()))
                    .select(Selection::Fixture(id))
                    .data(json!({
                        "id": id,
                        "circuito": data.and_then(|d| d.circuit),
                        "potencia_va": data.map(|d| d.power_va),
                        "altura_m": data.map(|d| d.height_m),
                    })),
            )
        })
    }

    pub(super) fn tool_electrical_design(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_default();
        match action.as_str() {
            "gerar_pontos" => self.apply_change(
                Some("Todos os cômodos já atendem ao mínimo da NBR 5410."),
                |project| {
                    let report = project.generate_electrical_points();
                    Ok(Change::new(format!(
                        "Pontos gerados: {} luz(es), {} tomada(s), {} interruptor(es), {} TUE.",
                        report.lights, report.outlets, report.switches, report.specific
                    ))
                    .data(json!({
                        "luzes": report.lights,
                        "tomadas": report.outlets,
                        "interruptores": report.switches,
                        "tue": report.specific,
                    })))
                },
            ),
            "distribuir_circuitos" => self.apply_change(
                Some("Não há pontos de luz ou tomadas para distribuir."),
                |project| {
                    let circuits = project.assign_circuits();
                    if circuits == 0 {
                        return Err("Não há pontos de luz ou tomadas para distribuir.".to_owned());
                    }
                    Ok(
                        Change::new(format!("{circuits} circuito(s) distribuído(s)."))
                            .data(json!({ "circuitos": circuits })),
                    )
                },
            ),
            "definir_tensao" => {
                let voltage = required_number(args, "tensao")?;
                if !matches!(voltage as i64, 127 | 220) || voltage.fract() != 0.0 {
                    return Err("'tensao' deve ser 127 ou 220.".to_owned());
                }
                self.apply_change(Some("A tensão já era essa."), |project| {
                    project.electrical.voltage_v = voltage;
                    Ok(Change::new(format!(
                        "Tensão dos circuitos: {voltage:.0} V."
                    )))
                })
            }
            "gerar_eletrodutos" => self.apply_change(
                Some("Todos os pontos já chegam ao quadro por eletrodutos."),
                |project| {
                    let added = project.generate_conduits()?;
                    Ok(Change::new(format!("{added} eletroduto(s) lançado(s)."))
                        .data(json!({ "eletrodutos": added })))
                },
            ),
            "ligar_pontos" => {
                let from = element_id(args, "de")?;
                let to = element_id(args, "para")?;
                let run = run_arg(args)?;
                self.apply_change(None, |project| {
                    let id = project.add_conduit(from, to, run)?;
                    Ok(Change::new(format!(
                        "Eletroduto {id} ligando {from} a {to} ({}).",
                        run.label().to_lowercase()
                    ))
                    .data(json!({ "id": id, "de": from, "para": to, "trecho": run.key() })))
                })
            }
            "definir_trecho" => {
                let id = element_id(args, "id")?;
                let run = run_arg(args)?;
                self.apply_change(Some("O eletroduto já estava assim."), |project| {
                    project.set_conduit_run(id, run)?;
                    Ok(Change::new(format!(
                        "Eletroduto {id}: {}.",
                        run.label().to_lowercase()
                    )))
                })
            }
            "remover_eletroduto" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.delete_conduit(id) {
                        return Err("Eletroduto não encontrado.".to_owned());
                    }
                    Ok(Change::new(format!("Eletroduto {id} removido.")))
                })
            }
            "definir_alimentacao" => {
                let supply = text(args, "alimentacao")
                    .and_then(|key| Supply::from_key(&key.to_lowercase()))
                    .ok_or("'alimentacao' deve ser auto, monofasica, bifasica ou trifasica.")?;
                self.apply_change(Some("A alimentação já era essa."), |project| {
                    project.electrical.supply = supply;
                    Ok(Change::new(format!(
                        "Alimentação: {}.",
                        supply.label().to_lowercase()
                    )))
                })
            }
            "definir_concessionaria" => {
                let profile = text(args, "concessionaria")
                    .and_then(|key| parse_utility_profile(&key))
                    .ok_or("'concessionaria' deve ser generico, cpfl, enel_sp, celesc, cemig, copel, energisa, equatorial, fecoergs ou hidropan.")?;
                self.apply_change(Some("O perfil de concessionária já era esse."), |project| {
                    project.electrical.utility = profile;
                    let _ = project.codes.set_base(
                        crate::domain::codes::CodeDiscipline::Electrical,
                        if profile == UtilityProfile::Generic {
                            "generic"
                        } else {
                            "nbr-5410"
                        },
                    );
                    if profile == UtilityProfile::Generic {
                        let _ = project.codes.set_amendments(
                            crate::domain::codes::CodeDiscipline::Electrical,
                            Vec::new(),
                        );
                    } else {
                        let _ = project.codes.set_amendments(
                            crate::domain::codes::CodeDiscipline::Electrical,
                            vec![profile.code_key().to_owned()],
                        );
                    }
                    Ok(Change::new(format!(
                        "Perfil de concessionária: {}.",
                        profile.label()
                    )))
                })
            }
            "demanda_coletiva" => {
                let util = text(args, "concessionaria")
                    .and_then(|key| parse_utility_profile(&key))
                    .unwrap_or(self.project.electrical.utility);
                let method = util
                    .building_demand_method(Some(&self.project.codes))
                    .ok_or_else(|| {
                        format!(
                            "A concessionária '{}' não possui tabela de demanda coletiva embutida. Use celesc, cemig, fecoergs ou hidropan.",
                            util.label()
                        )
                    })?;
                let mut dwellings = Vec::new();
                if let Some(arr) = args.get("unidades").and_then(Value::as_array) {
                    for item in arr {
                        let count = item
                            .get("quantidade")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as usize;
                        let area = item
                            .get("area_m2")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0);
                        if count > 0 && area > 0.0 {
                            dwellings.push(crate::domain::codes::DwellingGroup {
                                count,
                                area_m2: area,
                            });
                        }
                    }
                } else {
                    let count = number(args, "quantidade")?.unwrap_or(0.0) as usize;
                    let area = number(args, "area_m2")?.unwrap_or(0.0);
                    if count > 0 && area > 0.0 {
                        dwellings.push(crate::domain::codes::DwellingGroup {
                            count,
                            area_m2: area,
                        });
                    }
                }
                let mut common_kw = std::collections::BTreeMap::new();
                if let Some(v) = number(args, "iluminacao_comum_kw")?
                    && v > 0.0
                {
                    common_kw.insert("lighting".to_owned(), v);
                }
                if let Some(v) = number(args, "tomadas_comum_kw")?
                    && v > 0.0
                {
                    common_kw.insert("outlets".to_owned(), v);
                }
                let input = crate::domain::codes::BuildingDemandInput {
                    dwellings,
                    common_installed_kw: common_kw,
                    common_demand_extra: number(args, "demanda_comum_kva")?.unwrap_or(0.0),
                    special_demand: number(args, "demanda_especial_kva")?.unwrap_or(0.0),
                    commercial_demand: number(args, "demanda_comercial_kva")?.unwrap_or(0.0),
                    as_printed: args
                        .get("como_impresso")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                };
                let res = crate::domain::codes::calculate_building_demand(&method, &input)?;
                let msg = format!(
                    "Demanda coletiva ({}): {} unidades (área média {:.1} m²) → {:.2} {} (F(n)={:.2}, d={:.2}, fator {:.2}).",
                    res.standard,
                    res.total_dwellings,
                    res.weighted_mean_area_m2,
                    res.total_demand,
                    res.unit,
                    res.diversity_factor_f,
                    res.dwelling_demand_d,
                    res.safety_factor
                );
                Ok(ToolOutput::ok(msg).with_structured(serde_json::to_value(&res).unwrap_or_else(|_| json!({}))))
            }
            _ => Err(
                "Informe 'acao': gerar_pontos, distribuir_circuitos, definir_tensao, gerar_eletrodutos, ligar_pontos, definir_trecho, remover_eletroduto, definir_alimentacao, definir_concessionaria ou demanda_coletiva.".to_owned(),
            ),
        }
    }

    pub(super) fn tool_load_schedule(&mut self) -> Result<ToolOutput, String> {
        let forecast: Vec<Value> = self
            .project
            .electrical_forecast()
            .iter()
            .map(|room| {
                json!({
                    "comodo": room.name,
                    "tipo": room.class.label(),
                    "area_m2": (room.area_m2 * 100.0).round() / 100.0,
                    "perimetro_m": (room.perimeter_m * 100.0).round() / 100.0,
                    "iluminacao_minima_va": room.min_lighting_va,
                    "iluminacao_va": room.lighting_va,
                    "pontos_de_luz": room.lights,
                    "tomadas_minimas": room.min_outlets,
                    "tomadas": room.outlets,
                    "pendencias": room.missing(),
                })
            })
            .collect();
        let schedule = self.project.cached_load_schedule();
        let circuits: Vec<Value> = schedule
            .circuits
            .iter()
            .map(|row| {
                json!({
                    "circuito": row.number,
                    "quadro": row.board,
                    "tipo": row.kind.label(),
                    "comodos": row.rooms,
                    "pontos": row.points,
                    "potencia_va": row.power_va,
                    "tensao_v": row.voltage_v,
                    "corrente_a": (row.current_a * 10.0).round() / 10.0,
                    "fio_mm2": row.wire_mm2,
                    "fio_aumentado_por_queda": row.wire_by_drop,
                    "disjuntor_a": row.breaker_a,
                    "polos": row.poles,
                    "fases": phase_label(&row.phases),
                    "comprimento_m": row.length_m.map(|l| (l * 10.0).round() / 10.0),
                    "queda_tensao_pct": row.drop_percent.map(|d| (d * 100.0).round() / 100.0),
                    "dr": row.needs_rcd,
                })
            })
            .collect();
        let sub_boards: Vec<Value> = schedule
            .sub_boards
            .iter()
            .map(|sb| {
                json!({
                    "id": sb.panel_id,
                    "rotulo": sb.label,
                    "comodo": sb.room,
                    "circuitos": sb.circuits,
                    "carga_instalada_va": sb.installed_va,
                    "demanda_va": sb.demand_va.round(),
                    "corrente_a": (sb.current_a * 10.0).round() / 10.0,
                    "alimentador_m": sb.feeder_length_m.map(|l| (l * 10.0).round() / 10.0),
                    "queda_alimentador_pct": sb.feeder_drop_percent.map(|d| (d * 100.0).round() / 100.0),
                    "disjuntor_a": sb.breaker_a,
                    "fio_mm2": sb.wire_mm2,
                    "terra_mm2": sb.earth_mm2,
                    "eletroduto_mm": sb.conduit_mm,
                })
            })
            .collect();
        let conduits: Vec<Value> = self
            .project
            .conduit_wiring()
            .iter()
            .map(|run| {
                json!({
                    "id": run.conduit,
                    "de": run.upstream,
                    "para": run.downstream,
                    "trecho": run.run.key(),
                    "comprimento_m": (run.length_m * 100.0).round() / 100.0,
                    "diametro_mm": run.diameter_mm,
                    "fios": run.circuits.iter().map(|w| json!({
                        "circuito": w.circuit,
                        "condutores": w.short(),
                        "secao_mm2": w.section_mm2,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let tree = self.project.conduit_tree();
        let demand = schedule.demand;
        let entrance = schedule.entrance.map(|e| {
            json!({
                "ligacao": e.supply.key(),
                "corrente_a": (e.current_a * 10.0).round() / 10.0,
                "disjuntor_geral_a": e.breaker_a,
                "ramal_mm2": e.wire_mm2,
                "terra_mm2": e.earth_mm2,
                "eletroduto_mm": e.conduit_mm,
            })
        });
        let text = format!(
            "{} circuito(s), carga instalada {:.0} VA, {} alerta(s).",
            schedule.circuits.len(),
            schedule.total_va,
            schedule.warnings.len()
        );
        let rules = self
            .project
            .electrical
            .utility
            .rules_from_codes(Some(&self.project.codes))
            .with_overrides(self.project.electrical.custom_rules);
        let cite = rules.citation;
        let category_summary = self.project.electrical.utility.entrance_category_summary(
            Some(&self.project.codes),
            schedule.total_va / 1000.0,
            demand.demand_va / 1000.0,
            self.project.electrical.voltage_v,
            schedule.supply.phases().max(1),
            None,
        );
        let member_ids: Vec<String> = self
            .project
            .electrical
            .utility
            .members()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        Ok(ToolOutput::ok(text).with_structured(json!({
            "projeto_id": self.active_project_id(),
            "tensao_v": self.project.electrical.voltage_v,
            "concessionaria": utility_profile_key(self.project.electrical.utility),
            "categoria_concessionaria": category_summary,
            "distribuidoras_membro": member_ids,
            "norma_concessionaria": {
                "norma": cite.standard,
                "edicao": cite.edition,
                "tabelas": cite.tables,
                "sistema_nominal": cite.nominal_system,
                "limite_monofasico_kva": rules.single_phase_max_kva,
                "limite_bifasico_kva": rules.two_phase_max_kva,
                "limite_bt_kva": rules.max_bt_kva,
                "disjuntor_minimo_a": rules.min_breaker_a,
                "cabo_minimo_mm2": rules.min_wire_mm2,
                "observacao": cite.verification_note,
            },
            "previsao": forecast,
            "circuitos": circuits,
            "quadros_secundarios": sub_boards,
            "carga_instalada_va": schedule.total_va,
            "demanda": {
                "iluminacao_tug_va": demand.general_va,
                "fator_iluminacao_tug": demand.general_factor,
                "tue_va": demand.specific_va,
                "circuitos_tue": demand.specific_count,
                "fator_tue": demand.specific_factor,
                "demanda_va": demand.demand_va.round(),
            },
            "alimentacao": self.project.electrical.supply.key(),
            "carga_por_fase_va": {
                "A": schedule.phase_va[0],
                "B": schedule.phase_va[1],
                "C": schedule.phase_va[2],
            },
            "entrada": entrance,
            "eletrodutos": conduits,
            "pontos_sem_eletroduto": tree.unreached,
            "sem_circuito": schedule.unassigned,
            "semaforo": super::structure::verdicts_json(&crate::domain::verdict::electrical_verdicts(&schedule)),
            "alertas": schedule.warnings,
        })))
    }
}

#[cfg(test)]
mod tests {
    use crate::app::FloorPlanApp;
    use eframe::egui;
    use serde_json::json;

    #[test]
    fn an_assistant_designs_the_electrical_plan_end_to_end() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let built = app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [
                { "ferramenta": "definir_terreno", "argumentos": { "largura": 12, "comprimento": 20 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 1, "y": 1, "largura": 5, "comprimento": 4 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Cozinha", "x": 6, "y": 1, "largura": 3, "comprimento": 4 } },
            ] }),
        );
        assert!(!built.is_error, "{}", built.text);

        let generated = app.run_mcp_tool("projeto_eletrico", &json!({ "acao": "gerar_pontos" }));
        assert!(!generated.is_error, "{}", generated.text);
        let distributed = app.run_mcp_tool(
            "projeto_eletrico",
            &json!({ "acao": "distribuir_circuitos" }),
        );
        assert!(!distributed.is_error, "{}", distributed.text);

        let schedule = app.run_mcp_tool("quadro_de_cargas", &json!({}));
        let data = schedule.structured.unwrap();
        assert!(data["circuitos"].as_array().unwrap().len() >= 3);
        assert_eq!(data["sem_circuito"], 0);
        for room in data["previsao"].as_array().unwrap() {
            assert!(room["pendencias"].as_array().unwrap().is_empty(), "{room}");
        }

        // A point placed near a wall lands on its face, with the given data.
        let placed = app.run_mcp_tool(
            "inserir_ponto_eletrico",
            &json!({ "tipo": "tomada_alta", "x": 3, "y": 1.3, "circuito": 9, "potencia_va": 1200 }),
        );
        assert!(!placed.is_error, "{}", placed.text);
        let placed = placed.structured.unwrap();
        assert_eq!(placed["circuito"], 9);
        assert_eq!(placed["comodo"], "Sala");
        let id = placed["id"].as_u64().unwrap();
        let edited = app.run_mcp_tool(
            "editar_ponto_eletrico",
            &json!({ "id": id, "circuito": 0, "altura_m": 2.1 }),
        );
        assert!(!edited.is_error, "{}", edited.text);
        assert!(edited.structured.unwrap()["circuito"].is_null());

        // Furniture keys are refused, and every call is one undo step.
        assert!(
            app.run_mcp_tool(
                "inserir_ponto_eletrico",
                &json!({ "tipo": "mesa", "x": 3, "y": 3 })
            )
            .is_error
        );
        let before = app.project.clone();
        app.run_mcp_tool(
            "projeto_eletrico",
            &json!({ "acao": "definir_tensao", "tensao": 220 }),
        );
        assert_eq!(app.project.electrical.voltage_v, 220.0);
        app.undo();
        assert_eq!(app.project, before);
    }

    #[test]
    fn an_assistant_lays_conduits_and_reads_the_board() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let built = app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [
                { "ferramenta": "definir_terreno", "argumentos": { "largura": 12, "comprimento": 20 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 1, "y": 1, "largura": 5, "comprimento": 4 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Banheiro", "x": 6, "y": 1, "largura": 2, "comprimento": 2 } },
                { "ferramenta": "projeto_eletrico", "argumentos": { "acao": "gerar_pontos" } },
                { "ferramenta": "projeto_eletrico", "argumentos": { "acao": "distribuir_circuitos" } },
            ] }),
        );
        assert!(!built.is_error, "{}", built.text);
        // No board yet: nothing to lay conduits from.
        let refused = app.run_mcp_tool("projeto_eletrico", &json!({ "acao": "gerar_eletrodutos" }));
        assert!(refused.is_error);
        let panel = app.run_mcp_tool(
            "inserir_ponto_eletrico",
            &json!({ "tipo": "quadro", "x": 1.3, "y": 1.2 }),
        );
        assert!(!panel.is_error, "{}", panel.text);
        let laid = app.run_mcp_tool("projeto_eletrico", &json!({ "acao": "gerar_eletrodutos" }));
        assert!(!laid.is_error, "{}", laid.text);
        assert!(laid.structured.unwrap()["eletrodutos"].as_u64().unwrap() >= 3);

        let board = app
            .run_mcp_tool("quadro_de_cargas", &json!({}))
            .structured
            .unwrap();
        assert!(
            board["pontos_sem_eletroduto"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let conduits = board["eletrodutos"].as_array().unwrap();
        assert!(
            conduits
                .iter()
                .all(|c| !c["fios"].as_array().unwrap().is_empty()),
            "{conduits:?}"
        );
        for circuit in board["circuitos"].as_array().unwrap() {
            assert!(circuit["queda_tensao_pct"].as_f64().unwrap() <= 4.0);
            assert!(!circuit["fases"].as_str().unwrap().is_empty());
        }
        assert!(board["entrada"]["disjuntor_geral_a"].as_f64().is_some());

        // A run moved to the floor, the supply forced, a run removed.
        let id = conduits[0]["id"].as_u64().unwrap();
        let floor = app.run_mcp_tool(
            "projeto_eletrico",
            &json!({ "acao": "definir_trecho", "id": id, "trecho": "piso" }),
        );
        assert!(!floor.is_error, "{}", floor.text);
        let three = app.run_mcp_tool(
            "projeto_eletrico",
            &json!({ "acao": "definir_alimentacao", "alimentacao": "trifasica" }),
        );
        assert!(!three.is_error, "{}", three.text);
        assert_eq!(app.project.load_schedule().supply.phases(), 3);
        let removed = app.run_mcp_tool(
            "projeto_eletrico",
            &json!({ "acao": "remover_eletroduto", "id": id }),
        );
        assert!(!removed.is_error, "{}", removed.text);
        assert!(app.project.conduits.iter().all(|c| c.id != id));
    }
}
