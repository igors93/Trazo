//! Assistant tools for the electrical design: placing devices, editing their
//! circuit and power, generating the NBR 5410 minimum, distributing circuits
//! and reading the load schedule.

use serde_json::{Value, json};

use super::{
    Change, ToolOutput, element_id, number, quarter_turns, required_number, text, to_model, xy,
};
use crate::app::{FloorPlanApp, Selection};
use crate::domain::fixtures::{Fixture, FixtureKind};

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
                electrical: kind.default_electrical(),
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
                .room_holding(placed.center)
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
            _ => Err(
                "Informe 'acao': gerar_pontos, distribuir_circuitos ou definir_tensao.".to_owned(),
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
        let schedule = self.project.load_schedule();
        let circuits: Vec<Value> = schedule
            .circuits
            .iter()
            .map(|row| {
                json!({
                    "circuito": row.number,
                    "tipo": row.kind.label(),
                    "comodos": row.rooms,
                    "pontos": row.points,
                    "potencia_va": row.power_va,
                    "tensao_v": row.voltage_v,
                    "corrente_a": (row.current_a * 10.0).round() / 10.0,
                    "fio_mm2": row.wire_mm2,
                    "disjuntor_a": row.breaker_a,
                    "dr": row.needs_rcd,
                })
            })
            .collect();
        let text = format!(
            "{} circuito(s), carga instalada {:.0} VA, {} alerta(s).",
            schedule.circuits.len(),
            schedule.total_va,
            schedule.warnings.len()
        );
        Ok(ToolOutput::ok(text).with_structured(json!({
            "tensao_v": self.project.electrical.voltage_v,
            "previsao": forecast,
            "circuitos": circuits,
            "carga_instalada_va": schedule.total_va,
            "sem_circuito": schedule.unassigned,
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
}
