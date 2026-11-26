//! Assistant tools for water, sewage and rain (roadmap section E): the
//! plumbing report `instalacoes_hidraulicas` and the settings
//! `projeto_hidraulico`.

use serde_json::{Value, json};

use super::{Change, ToolOutput, number, optional_count, round, text, to_model, xy};
use crate::app::FloorPlanApp;
use crate::domain::plumbing::Heater;
use crate::domain::plumbing::cold_water::{ColdWater, TankPlace, cold_water};
use crate::domain::plumbing::points::{PointKind, plumbing_points};
use crate::domain::plumbing::septic::{
    AbsorptionSource, ColdestMonth, SepticDesign, SewageStandard, septic_design,
};

impl FloorPlanApp {
    /// `instalacoes_hidraulicas`: the plumbing points, their counts and
    /// the fixture schedule.
    pub(super) fn tool_plumbing_report(&self) -> Result<ToolOutput, String> {
        let project = &self.project;
        let points = plumbing_points(project);
        if points.is_empty() {
            return Ok(ToolOutput::ok(
                "Nenhum aparelho sanitário na planta: coloque vaso, lavatório, chuveiro, pia ou máquina de lavar com inserir_objeto.",
            )
            .with_structured(json!({ "pontos": [] })));
        }
        let counts = PointKind::ALL
            .iter()
            .map(|k| format!("{} {}", points.count(*k), k.label().to_lowercase()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut text = format!(
            "Pontos: {counts}. Aquecimento: {}.\nQuadro de aparelhos:",
            project.plumbing.heater.label().to_lowercase()
        );
        for row in &points.schedule {
            let rooms = if row.rooms.is_empty() {
                String::new()
            } else {
                format!(" ({})", row.rooms.join(", "))
            };
            text.push_str(&format!(
                "\n- {} × {}{rooms}: peso {}, {} UHC cada",
                row.count,
                row.kind.label(),
                round(row.weight),
                round(row.uhc)
            ));
        }
        text.push_str(&format!(
            "\nSoma dos pesos {}, soma das UHC {}.",
            round(points.total_weight),
            round(points.total_uhc)
        ));
        for note in &points.notes {
            text.push_str(&format!("\n{note}"));
        }
        let cold = cold_water(project);
        if let Some(cold) = &cold {
            text.push_str(&format!(
                "\nÁgua fria: {} morador(es), {} L/dia, caixa-d'água de {} L ({} kN cheia) com a base a {} m do piso.",
                cold.residents,
                round(cold.daily_l),
                round(cold.tank_l),
                round(cold.tank_full_kn),
                round(cold.tank_base_m)
            ));
            for outlet in &cold.network.outlets {
                text.push_str(&format!(
                    "\n- {}{}: {} m de pressão ({})",
                    outlet.fixture.label(),
                    outlet
                        .room
                        .as_ref()
                        .map_or(String::new(), |r| format!(" ({r})")),
                    round(outlet.pressure_m),
                    outlet.state.label()
                ));
            }
            for warning in &cold.warnings {
                text.push_str(&format!("\n{warning}"));
            }
        }
        let list: Vec<Value> = points
            .points
            .iter()
            .map(|p| {
                json!({
                    "aparelho_id": p.fixture_id,
                    "aparelho": p.fixture.key(),
                    "ponto": p.kind.key(),
                    "sigla": p.kind.tag(),
                    "posicao": xy(p.at),
                    "altura_m": round(p.height_m),
                    "diametro_mm": round(p.diameter_mm),
                    "comodo": p.room,
                })
            })
            .collect();
        let schedule: Vec<Value> = points
            .schedule
            .iter()
            .map(|row| {
                json!({
                    "aparelho": row.kind.key(),
                    "nome": row.kind.label(),
                    "quantidade": row.count,
                    "comodos": row.rooms,
                    "agua_fria_altura_m": row.cold_m.map(round),
                    "agua_quente_altura_m": row.hot_m.map(round),
                    "gas_altura_m": row.gas_m.map(round),
                    "esgoto": row.drain.map(|(dn, h)| json!({ "dn": dn, "altura_m": round(h) })),
                    "peso": round(row.weight),
                    "uhc": round(row.uhc),
                    "valor_de_referencia_trazo": row.trazo_reference,
                })
            })
            .collect();
        let count: serde_json::Map<String, Value> = PointKind::ALL
            .iter()
            .map(|k| (k.key().to_owned(), json!(points.count(*k))))
            .collect();
        let septic = septic_design(project);
        if let Some(design) = &septic {
            text.push_str(&format!(
                "\nFossa séptica: tanque de {} L úteis ({} × {} m, {} m de água) e {} sumidouro(s) de {} m com {} m de altura útil.",
                round(design.volume_l),
                round(design.length_m),
                round(design.width_m),
                round(design.depth_m),
                design.soakaway_count,
                round(design.soakaway_diameter_m),
                round(design.soakaway_height_m)
            ));
        }
        Ok(ToolOutput::ok(text).with_structured(json!({
            "aquecimento": project.plumbing.heater.key(),
            "pontos": list,
            "contagem": count,
            "quadro_de_aparelhos": schedule,
            "soma_pesos": round(points.total_weight),
            "soma_uhc": round(points.total_uhc),
            "observacoes": points.notes,
            "agua_fria": cold.as_ref().map(cold_water_json),
            "fossa_septica": septic.as_ref().map(septic_json),
        })))
    }
}

/// The septic tank and soakaway for the report.
pub(super) fn septic_json(design: &SepticDesign) -> Value {
    json!({
        "padrao": design.standard.key(),
        "padrao_nome": design.standard.label(),
        "clima": design.coldest_month.key(),
        "mes_mais_frio": design.coldest_month.label(),
        "intervalo_limpeza_anos": design.cleaning_years,
        "moradores": design.residents,
        "pessoas": design.people,
        "contribuicao_l_pessoa_dia": round(design.per_person_l),
        "contribuicao_diaria_l": round(design.daily_l),
        "tempo_detencao_dias": round(design.detention_days),
        "taxa_lodo_k_dias": round(design.sludge_k_days),
        "volume_util_l": round(design.volume_l),
        "tanque_prismatico": {
            "comprimento_m": round(design.length_m),
            "largura_m": round(design.width_m),
            "profundidade_util_m": round(design.depth_m),
            "profundidade_util_maxima_m": round(design.max_depth_m),
        },
        "tanque_cilindrico": {
            "diametro_m": round(design.diameter_m),
            "profundidade_util_m": round(design.cylinder_depth_m),
        },
        "infiltracao_l_m2_dia": round(design.absorption_l_m2_day),
        "infiltracao_origem": match design.absorption_source {
            AbsorptionSource::Tested => "teste de infiltração",
            AbsorptionSource::PresumedSoil => "solo provável (estimativa)",
            AbsorptionSource::Fallback => "padrão seguro do Trazo (estimativa)",
        },
        "area_infiltracao_m2": round(design.area_needed_m2),
        "sumidouros": design.soakaway_count,
        "sumidouro_diametro_m": round(design.soakaway_diameter_m),
        "sumidouro_altura_util_m": round(design.soakaway_height_m),
        "sumidouro_escavacao_m": round(design.soakaway_depth_m),
        "lencol_m": design.water_table_m.map(round),
        "solo_colapsivel": design.collapsible,
        "posicionado": design.placed,
        "distancias": design.distances.iter().map(|d| json!({
            "de": d.element,
            "ate": d.to,
            "distancia_m": round(d.distance_m),
            "minimo_norma_m": round(d.normative_m),
            "minimo_trazo_m": round(d.needed_m),
            "atende_norma": d.ok(),
        })).collect::<Vec<_>>(),
        "observacoes": design.notes,
    })
}

/// The settings `restaurar` can put back to Trazo's defaults.
const FIELDS: [&str; 6] = [
    "moradores",
    "consumo_l_pessoa_dia",
    "dias_reserva",
    "caixa",
    "altura_base_caixa_m",
    "aquecimento",
];

impl FloorPlanApp {
    /// `projeto_hidraulico`: the plumbing design's settings.
    pub(super) fn tool_plumbing_design(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .ok_or("Informe 'acao': configurar, restaurar, configurar_fossa ou remover_fossa.")?;
        let output = match action.as_str() {
            "configurar_fossa" => {
                let mut settings = self.project.plumbing.septic.clone().unwrap_or_default();
                match (number(args, "fossa_x")?, number(args, "fossa_y")?) {
                    (Some(x), Some(y)) => settings.tank_at = Some(to_model(x, y)),
                    (None, None) => {}
                    _ => return Err("Informe fossa_x e fossa_y juntos.".to_owned()),
                }
                if let Some(list) = args.get("sumidouros").and_then(Value::as_array) {
                    let mut points = Vec::with_capacity(list.len());
                    for (i, item) in list.iter().enumerate() {
                        let x = number(item, "x")?
                            .ok_or_else(|| format!("Sumidouro {}: falta 'x'.", i + 1))?;
                        let y = number(item, "y")?
                            .ok_or_else(|| format!("Sumidouro {}: falta 'y'.", i + 1))?;
                        points.push(to_model(x, y));
                    }
                    settings.soakaways_at = points;
                }
                if let Some(d) = number(args, "diametro_sumidouro_m")? {
                    settings.soakaway_diameter_m = d;
                }
                if let Some(raw) = text(args, "padrao") {
                    settings.standard = SewageStandard::parse(&raw)
                        .ok_or("padrao deve ser alto, medio ou baixo (NBR 7229).")?;
                }
                if let Some(years) = optional_count(args, "intervalo_limpeza_anos")? {
                    settings.cleaning_years = years as u32;
                }
                if let Some(raw) = text(args, "clima") {
                    settings.coldest_month = ColdestMonth::parse(&raw).ok_or(
                        "clima deve ser frio (mês mais frio até 10 °C), ameno (10 a 20 °C) ou quente (acima de 20 °C).",
                    )?;
                }
                if let Some(extra) = optional_count(args, "pessoas_extras")? {
                    settings.extra_people = extra as u32;
                }
                match args.get("taxa_infiltracao_l_m2_dia") {
                    Some(Value::Null) => settings.tested_absorption_l_m2_day = None,
                    Some(_) => {
                        settings.tested_absorption_l_m2_day =
                            number(args, "taxa_infiltracao_l_m2_dia")?;
                    }
                    None => {}
                }
                settings.validate()?;
                self.apply_change(Some("A fossa já estava assim."), |project| {
                    project.plumbing.septic = Some(settings);
                    let design = septic_design(project).ok_or("Fossa não configurada.")?;
                    Ok(Change::new(format!(
                        "Fossa séptica: tanque de {} L úteis para {} pessoa(s) e {} sumidouro(s) de {} m com {} m de altura útil.",
                        round(design.volume_l),
                        design.people,
                        design.soakaway_count,
                        round(design.soakaway_diameter_m),
                        round(design.soakaway_height_m)
                    ))
                    .data(json!({ "fossa_septica": septic_json(&design) })))
                })?
            }
            "remover_fossa" => self.apply_change(None, |project| {
                if project.plumbing.septic.take().is_none() {
                    return Err("O projeto não tem fossa configurada.".to_owned());
                }
                Ok(Change::new("Fossa séptica removida do projeto."))
            })?,
            "configurar" => {
                let residents = optional_count(args, "moradores")?;
                let consumption = number(args, "consumo_l_pessoa_dia")?;
                let days = number(args, "dias_reserva")?;
                let base = number(args, "altura_base_caixa_m")?;
                let tank = match (number(args, "caixa_x")?, number(args, "caixa_y")?) {
                    (Some(x), Some(y)) => Some(to_model(x, y)),
                    (None, None) => None,
                    _ => return Err("Informe caixa_x e caixa_y juntos.".to_owned()),
                };
                let heater = match text(args, "aquecimento") {
                    Some(key) => Some(
                        Heater::from_key(&key)
                            .ok_or("aquecimento deve ser chuveiro_eletrico, gas ou solar.")?,
                    ),
                    None => None,
                };
                if let Some(n) = residents
                    && !(1..=100).contains(&n)
                {
                    return Err("moradores deve ficar entre 1 e 100.".to_owned());
                }
                if let Some(c) = consumption
                    && !(50.0..=400.0).contains(&c)
                {
                    return Err("consumo_l_pessoa_dia deve ficar entre 50 e 400 L.".to_owned());
                }
                if let Some(d) = days
                    && !(1.0..=3.0).contains(&d)
                {
                    return Err("dias_reserva deve ficar entre 1 e 3 (NBR 5626).".to_owned());
                }
                if let Some(b) = base
                    && !(0.5..=20.0).contains(&b)
                {
                    return Err("altura_base_caixa_m deve ficar entre 0,5 e 20 m.".to_owned());
                }
                self.apply_change(
                    Some("Nada para configurar: informe ao menos um campo."),
                    |project| {
                        let settings = &mut project.plumbing;
                        if let Some(n) = residents {
                            settings.residents = Some(n as u32);
                        }
                        if consumption.is_some() {
                            settings.consumption_l_day = consumption;
                        }
                        if days.is_some() {
                            settings.reserve_days = days;
                        }
                        if base.is_some() {
                            settings.tank_base_m = base;
                        }
                        if tank.is_some() {
                            settings.tank_at = tank;
                        }
                        if let Some(heater) = heater {
                            settings.heater = heater;
                        }
                        Ok(Change::new("Projeto hidrossanitário configurado."))
                    },
                )?
            }
            "restaurar" => {
                let fields: Vec<String> = args
                    .get("campos")
                    .and_then(Value::as_array)
                    .map(|list| {
                        list.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                if fields.is_empty() {
                    return Err(format!("Informe 'campos': {}.", FIELDS.join(", ")));
                }
                if let Some(unknown) = fields.iter().find(|f| !FIELDS.contains(&f.as_str())) {
                    return Err(format!(
                        "Campo desconhecido: {unknown}. Use {}.",
                        FIELDS.join(", ")
                    ));
                }
                self.apply_change(None, |project| {
                    let settings = &mut project.plumbing;
                    for field in &fields {
                        match field.as_str() {
                            "moradores" => settings.residents = None,
                            "consumo_l_pessoa_dia" => settings.consumption_l_day = None,
                            "dias_reserva" => settings.reserve_days = None,
                            "caixa" => settings.tank_at = None,
                            "altura_base_caixa_m" => settings.tank_base_m = None,
                            _ => settings.heater = Heater::default(),
                        }
                    }
                    Ok(Change::new("Campos de volta ao padrão do Trazo."))
                })?
            }
            other => {
                return Err(format!(
                    "Ação desconhecida: {other}. Use configurar, restaurar, configurar_fossa ou remover_fossa."
                ));
            }
        };
        let mut text = output.text;
        if let Some(cold) = cold_water(&self.project) {
            text.push_str(&format!(
                "\nCaixa-d'água de {} L ({} kN cheia), base a {} m do piso.",
                round(cold.tank_l),
                round(cold.tank_full_kn),
                round(cold.tank_base_m)
            ));
            for warning in &cold.warnings {
                text.push_str(&format!("\n{warning}"));
            }
        }
        let septic = septic_design(&self.project);
        if let Some(design) = &septic {
            for distance in design.distances.iter().filter(|d| !d.ok()) {
                text.push_str(&format!(
                    "\nO {} fica a {} m da {}: a NBR 7229 pede {} m.",
                    distance.element,
                    round(distance.distance_m),
                    distance.to,
                    round(distance.normative_m)
                ));
            }
        }
        Ok(ToolOutput::ok(text).with_structured(json!({
            "configuracao": self.project.plumbing,
            "fossa_septica": septic.as_ref().map(septic_json),
        })))
    }
}

/// The cold-water design for the report.
pub(super) fn cold_water_json(cold: &ColdWater) -> Value {
    let network = &cold.network;
    json!({
        "moradores": cold.residents,
        "moradores_informados": cold.residents_given,
        "consumo_l_pessoa_dia": round(cold.per_person_l),
        "consumo_diario_l": round(cold.daily_l),
        "dias_reserva": round(cold.reserve_days),
        "reserva_l": round(cold.reserve_l),
        "caixa_l": round(cold.tank_l),
        "caixa_cheia_kn": round(cold.tank_full_kn),
        "caixa_posicao": xy(cold.tank_at),
        "caixa_posicao_origem": match cold.tank_place {
            TankPlace::Set => "informada",
            TankPlace::Structure => "caixa do projeto estrutural",
            TankPlace::Trazo => "escolhida pelo Trazo",
        },
        "base_caixa_m": round(cold.tank_base_m),
        "base_necessaria_m": round(cold.needed_base_m),
        "base_para_banho_forte_m": round(cold.comfort_base_m),
        "pressao_minima_m": round(network.min_m),
        "colunas": network.groups.iter().map(|g| json!({
            "coluna": g.label,
            "comodo": g.room,
            "posicao": xy(g.riser_at),
        })).collect::<Vec<_>>(),
        "tubos": network.pipes.iter().map(|p| json!({
            "funcao": p.role.label(),
            "coluna": network.groups.get(p.group).map(|g| g.label.clone()),
            "dn": p.dn,
            "comprimento_m": round(p.length_m),
            "vazao_l_s": round(p.flow_l_s),
            "velocidade_m_s": round(p.speed_m_s),
            "conexoes": p.fittings.iter().map(|f| f.label()).collect::<Vec<_>>(),
            "perda_m": round(p.loss_m),
        })).collect::<Vec<_>>(),
        "pontos": network.outlets.iter().map(|o| json!({
            "aparelho_id": o.fixture_id,
            "aparelho": o.fixture.key(),
            "comodo": o.room,
            "altura_m": round(o.node.z),
            "desnivel_m": round(o.static_m),
            "perdas_m": round(o.loss_m),
            "pressao_m": round(o.pressure_m),
            "pressao": o.state.label(),
        })).collect::<Vec<_>>(),
        "tubos_por_dn_m": network
            .lengths_by_dn()
            .iter()
            .map(|(dn, m)| json!({ "dn": dn, "metros": round(*m) }))
            .collect::<Vec<_>>(),
        "alertas": cold.warnings,
        "observacoes": cold.notes,
    })
}
