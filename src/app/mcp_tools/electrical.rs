//! Assistant tools for the electrical design: placing devices, editing their
//! circuit and power, generating the NBR 5410 minimum, distributing circuits,
//! laying conduits, choosing the supply and reading the load schedule with
//! the board (phases, voltage drop, demand and service entrance).

use serde_json::{Value, json};

use super::{
    Change, ToolOutput, element_id, ensure_parent, number, quarter_turns, required_number,
    resolve_path, text, to_model, unique_path, xy,
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
        let lumens = number(args, "lumens")?;
        if circuit.is_none() && power.is_none() && height.is_none() && lumens.is_none() {
            return Err("Informe circuito, potencia_va, altura_m ou lumens.".to_owned());
        }
        self.apply_change(Some("Nada mudou no ponto elétrico."), |project| {
            if circuit.is_some() || power.is_some() || height.is_some() {
                project.update_electrical(id, circuit, power, height)?;
            }
            if let Some(lm) = lumens {
                project.set_device_lumens(id, (lm > 0.0).then_some(lm))?;
            }
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
                        "lumens": data.and_then(|d| d.lumens),
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
            "lista_materiais" => {
                use crate::domain::electrical::shopping_list::{
                    DEFAULT_ELECTRICAL_WASTE_PCT, electrical_shopping_list_csv,
                    electrical_shopping_list_svg,
                };
                let waste = number(args, "perda_pct")?.unwrap_or(DEFAULT_ELECTRICAL_WASTE_PCT);
                if !(0.0..=50.0).contains(&waste) {
                    return Err("'perda_pct' deve estar entre 0 e 50.".to_owned());
                }
                let list = self.project.electrical_shopping_list(waste);
                let mut exported: Option<String> = None;
                if let Some(raw) = text(args, "salvar_csv") {
                    let path = unique_path(resolve_path(&raw, "csv"));
                    ensure_parent(&path)?;
                    std::fs::write(&path, electrical_shopping_list_csv(&self.project, waste))
                        .map_err(|e| format!("Não foi possível salvar o CSV: {e}"))?;
                    exported = Some(path.display().to_string());
                } else if let Some(raw) = text(args, "salvar_svg") {
                    let path = unique_path(resolve_path(&raw, "svg"));
                    ensure_parent(&path)?;
                    let svg = electrical_shopping_list_svg(&self.project, waste);
                    crate::domain::views::export_view_svg(&svg, &path)?;
                    exported = Some(path.display().to_string());
                } else if let Some(raw) = text(args, "salvar_pdf") {
                    let path = unique_path(resolve_path(&raw, "pdf"));
                    ensure_parent(&path)?;
                    let svg = electrical_shopping_list_svg(&self.project, waste);
                    crate::domain::views::export_view_pdf(&svg, &path)?;
                    exported = Some(path.display().to_string());
                }
                let msg = format!(
                    "Lista de materiais elétricos (perda {:.0} %): {:.1} m de cabos ({:.1} m líquido), {:.1} m de eletrodutos, {} item(ns) de caixas/quadros e {} item(ns) de proteção.{}",
                    list.waste_pct,
                    list.total_wire_m,
                    list.total_wire_net_m,
                    list.total_conduit_m,
                    list.boxes.len(),
                    list.protections.len(),
                    exported
                        .as_deref()
                        .map_or(String::new(), |p| format!(" Exportado em {p}."))
                );
                Ok(ToolOutput::ok(msg).with_structured(json!({
                    "perda_pct": list.waste_pct,
                    "tem_eletrodutos": list.has_conduits,
                    "total_cabos_liquido_m": (list.total_wire_net_m * 10.0).round() / 10.0,
                    "total_cabos_m": (list.total_wire_m * 10.0).round() / 10.0,
                    "total_eletrodutos_liquido_m": (list.total_conduit_net_m * 10.0).round() / 10.0,
                    "total_eletrodutos_m": (list.total_conduit_m * 10.0).round() / 10.0,
                    "cabos": list.wires.iter().map(|w| json!({
                        "bitola_mm2": w.section_mm2,
                        "funcao": w.color.key(),
                        "cor": w.color.color_name(),
                        "descricao": w.color.label(),
                        "liquido_m": (w.net_length_m * 10.0).round() / 10.0,
                        "com_perda_m": (w.total_length_m * 10.0).round() / 10.0,
                        "rolos_100m": w.rolls_100m,
                    })).collect::<Vec<_>>(),
                    "eletrodutos": list.conduits.iter().map(|c| json!({
                        "diametro_mm": c.diameter_mm,
                        "trecho": c.run.key(),
                        "liquido_m": (c.net_length_m * 10.0).round() / 10.0,
                        "com_perda_m": (c.total_length_m * 10.0).round() / 10.0,
                        "rolos_50m": c.rolls_50m,
                    })).collect::<Vec<_>>(),
                    "caixas_e_quadros": list.boxes.iter().map(|b| json!({
                        "tipo": b.kind,
                        "especificacao": b.specification,
                        "quantidade": b.quantity,
                    })).collect::<Vec<_>>(),
                    "protecao": list.protections.iter().map(|p| json!({
                        "categoria": p.category.key(),
                        "especificacao": p.specification,
                        "polos": p.poles,
                        "corrente_a": p.rating_a,
                        "curva_ou_classe": p.curve_or_class,
                        "quantidade": p.quantity,
                        "observacao": p.notes,
                    })).collect::<Vec<_>>(),
                    "acabamentos": list.devices.iter().map(|d| json!({
                        "especificacao": d.specification,
                        "quantidade": d.quantity,
                    })).collect::<Vec<_>>(),
                    "arquivo_exportado": exported,
                    "alertas": list.warnings,
                    "premissas": list.assumptions,
                })))
            }
            "luminotecnico" => {
                use crate::domain::electrical::lighting::{
                    lighting_report_csv, lighting_report_svg,
                };
                let report = self.project.lighting_report();
                let mut exported: Option<String> = None;
                if let Some(raw) = text(args, "salvar_csv") {
                    let path = unique_path(resolve_path(&raw, "csv"));
                    ensure_parent(&path)?;
                    std::fs::write(&path, lighting_report_csv(&self.project))
                        .map_err(|e| format!("Não foi possível salvar o CSV: {e}"))?;
                    exported = Some(path.display().to_string());
                } else if let Some(raw) = text(args, "salvar_svg") {
                    let path = unique_path(resolve_path(&raw, "svg"));
                    ensure_parent(&path)?;
                    let svg = lighting_report_svg(&self.project);
                    crate::domain::views::export_view_svg(&svg, &path)?;
                    exported = Some(path.display().to_string());
                } else if let Some(raw) = text(args, "salvar_pdf") {
                    let path = unique_path(resolve_path(&raw, "pdf"));
                    ensure_parent(&path)?;
                    let svg = lighting_report_svg(&self.project);
                    crate::domain::views::export_view_pdf(&svg, &path)?;
                    exported = Some(path.display().to_string());
                }
                let msg = format!(
                    "Cálculo luminotécnico (NBR ISO/CIE 8995-1): {} de {} cômodo(s) atendem à meta de lux ({:.0} lm instalados, ~{:.0} W LED).{}",
                    report.rooms_passing,
                    report.rooms.len(),
                    report.total_installed_lumens,
                    report.total_estimated_watts,
                    exported
                        .as_deref()
                        .map_or(String::new(), |p| format!(" Exportado em {p}."))
                );
                Ok(ToolOutput::ok(msg).with_structured(json!({
                    "comodos_atendem": report.rooms_passing,
                    "comodos_abaixo": report.rooms_failing,
                    "fluxo_total_lm": report.total_installed_lumens.round(),
                    "potencia_led_estimada_w": (report.total_estimated_watts * 10.0).round() / 10.0,
                    "configuracao": {
                        "lumens_teto": report.settings.ceiling_lumens,
                        "lumens_arandela": report.settings.wall_lumens,
                        "eficiencia_lm_w": report.settings.lamp_efficacy_lm_w,
                        "plano_trabalho_m": report.settings.work_plane_m,
                        "fator_manutencao": report.settings.maintenance_factor,
                        "refletancias": report.settings.reflectance.key(),
                        "mostrar_na_planta": report.settings.show_plan_overlay,
                    },
                    "comodos": report.rooms.iter().map(|r| json!({
                        "comodo": r.room_name,
                        "categoria": r.category,
                        "area_m2": (r.area_m2 * 100.0).round() / 100.0,
                        "indice_k": (r.room_index_k * 100.0).round() / 100.0,
                        "fu": (r.utilization_factor * 100.0).round() / 100.0,
                        "fd": r.maintenance_factor,
                        "luminarias": r.luminaires,
                        "fluxo_instalado_lm": r.installed_lumens.round(),
                        "fluxo_necessario_lm": r.required_lumens.ceil(),
                        "iluminancia_lux": r.calculated_lux.round(),
                        "meta_lux": r.target_lux,
                        "luminarias_recomendadas": r.recommended_luminaires,
                        "densidade_w_m2": (r.power_density_w_m2 * 10.0).round() / 10.0,
                        "semaforo": r.light.key(),
                        "explicacao": r.plain_note,
                        "norma": r.clause,
                    })).collect::<Vec<_>>(),
                    "arquivo_exportado": exported,
                    "premissas": report.assumptions,
                })))
            }
            "configurar_iluminacao" => {
                use crate::domain::electrical::lighting::SurfaceReflectance;
                let ceil_lm = number(args, "lumens_teto")?;
                let wall_lm = number(args, "lumens_arandela")?;
                let eff = number(args, "eficiencia_lm_w")?;
                let work_plane = number(args, "plano_trabalho_m")?;
                let fd = number(args, "fator_manutencao")?;
                let refl = match text(args, "refletancias") {
                    Some(raw) => Some(
                        SurfaceReflectance::parse(&raw)
                            .ok_or("'refletancias' deve ser claras, medias ou escuras.")?,
                    ),
                    None => None,
                };
                let overlay = args.get("mostrar_na_planta").and_then(Value::as_bool);
                if let Some(v) = ceil_lm
                    && !(100.0..=50_000.0).contains(&v)
                {
                    return Err("'lumens_teto' deve estar entre 100 e 50000 lm.".to_owned());
                }
                if let Some(v) = wall_lm
                    && !(100.0..=20_000.0).contains(&v)
                {
                    return Err("'lumens_arandela' deve estar entre 100 e 20000 lm.".to_owned());
                }
                if let Some(v) = eff
                    && !(20.0..=250.0).contains(&v)
                {
                    return Err("'eficiencia_lm_w' deve estar entre 20 e 250 lm/W.".to_owned());
                }
                if let Some(v) = work_plane
                    && !(0.0..=1.5).contains(&v)
                {
                    return Err("'plano_trabalho_m' deve estar entre 0 e 1,5 m.".to_owned());
                }
                if let Some(v) = fd
                    && !(0.50..=0.95).contains(&v)
                {
                    return Err("'fator_manutencao' deve estar entre 0,50 e 0,95.".to_owned());
                }
                self.apply_change(Some("A configuração luminotécnica já estava assim."), |project| {
                    let cfg = &mut project.electrical.lighting;
                    if let Some(v) = ceil_lm {
                        cfg.ceiling_lumens = v;
                    }
                    if let Some(v) = wall_lm {
                        cfg.wall_lumens = v;
                    }
                    if let Some(v) = eff {
                        cfg.lamp_efficacy_lm_w = v;
                    }
                    if let Some(v) = work_plane {
                        cfg.work_plane_m = v;
                    }
                    if let Some(v) = fd {
                        cfg.maintenance_factor = v;
                    }
                    if let Some(r) = refl {
                        cfg.reflectance = r;
                    }
                    if let Some(show) = overlay {
                        cfg.show_plan_overlay = show;
                    }
                    Ok(Change::new(format!(
                        "Iluminação configurada: teto {:.0} lm, arandela {:.0} lm, Fd {:.2}, refletâncias {}.",
                        cfg.ceiling_lumens,
                        cfg.wall_lumens,
                        cfg.maintenance_factor,
                        cfg.reflectance.short()
                    )))
                })
            }
            "fotovoltaico" => {
                use crate::domain::electrical::photovoltaic::{
                    photovoltaic_csv, photovoltaic_sheet_svg,
                };
                let r = self.project.photovoltaic_report();
                let mut exported: Option<String> = None;
                if let Some(raw) = text(args, "salvar_csv") {
                    let path = unique_path(resolve_path(&raw, "csv"));
                    ensure_parent(&path)?;
                    std::fs::write(&path, photovoltaic_csv(&self.project))
                        .map_err(|e| format!("Não foi possível salvar o CSV: {e}"))?;
                    exported = Some(path.display().to_string());
                } else if let Some(raw) = text(args, "salvar_svg") {
                    let path = unique_path(resolve_path(&raw, "svg"));
                    ensure_parent(&path)?;
                    let svg = photovoltaic_sheet_svg(&self.project);
                    crate::domain::views::export_view_svg(&svg, &path)?;
                    exported = Some(path.display().to_string());
                } else if let Some(raw) = text(args, "salvar_pdf") {
                    let path = unique_path(resolve_path(&raw, "pdf"));
                    ensure_parent(&path)?;
                    let svg = photovoltaic_sheet_svg(&self.project);
                    crate::domain::views::export_view_pdf(&svg, &path)?;
                    exported = Some(path.display().to_string());
                }
                let msg = format!(
                    "{}{}",
                    r.summary_sentence,
                    exported
                        .as_deref()
                        .map_or(String::new(), |p| format!(" Exportado em {p}."))
                );
                Ok(ToolOutput::ok(msg).with_structured(json!({
                    "ativo": r.settings.enabled,
                    "cidade": r.city_label,
                    "cidade_padrao_conservador": r.city_is_fallback,
                    "hsp_media_kwh_m2_dia": (r.mean_hsp_kwh_m2_day * 100.0).round() / 100.0,
                    "consumo_mensal_kwh": r.monthly_consumption_kwh.round(),
                    "consumo_estimado": r.consumption_is_estimated,
                    "custo_disponibilidade_kwh": r.availability_cost_kwh,
                    "potencia_necessaria_kwp": (r.required_kwp * 100.0).round() / 100.0,
                    "potencia_instalada_kwp": (r.installed_kwp * 100.0).round() / 100.0,
                    "modulos_necessarios": r.modules_required,
                    "modulos_alocados": r.modules_placed,
                    "modulo_wp": r.settings.module_wp,
                    "inversor_kw": r.inverter_kw,
                    "fdi_cc_ca": (r.dc_ac_ratio * 100.0).round() / 100.0,
                    "corrente_ca_a": (r.ac_current_a * 10.0).round() / 10.0,
                    "disjuntor_ca_a": r.ac_breaker_a,
                    "cabo_ca_mm2": r.ac_wire_mm2,
                    "area_telhado_estimada_m2": (r.required_roof_area_m2 * 10.0).round() / 10.0,
                    "sobrecarga_telhado_kn_m2": r.roof_load_kn_m2,
                    "geracao_anual_kwh": r.annual_generation_kwh.round(),
                    "geracao_media_mensal_kwh": r.mean_monthly_generation_kwh.round(),
                    "compensacao_pct": r.offset_percent.round(),
                    "acima_limite_75kw": r.refused_above_limit,
                    "semaforo": r.light.key(),
                    "planos_telhado": r.planes.iter().map(|p| json!({
                        "telhado": p.roof_label,
                        "agua": p.plane_index,
                        "orientacao": p.orientation_label,
                        "inclinacao_pct": p.slope_percent.round(),
                        "area_m2": (p.area_m2 * 10.0).round() / 10.0,
                        "fator_orientacao": (p.orientation_factor * 100.0).round() / 100.0,
                        "capacidade_modulos": p.capacity_panels,
                        "modulos_alocados": p.placed_panels,
                    })).collect::<Vec<_>>(),
                    "geracao_mensal": r.months.iter().map(|m| json!({
                        "mes": m.month,
                        "dias": m.days,
                        "hsp": m.hsp_kwh_m2_day,
                        "geracao_kwh": m.generation_kwh.round(),
                        "consumo_kwh": m.consumption_kwh.round(),
                        "saldo_kwh": m.balance_kwh.round(),
                    })).collect::<Vec<_>>(),
                    "arquivo_exportado": exported,
                    "alertas": r.warnings,
                    "premissas": r.assumptions,
                })))
            }
            "configurar_fotovoltaico" => {
                let enabled = args.get("ativo").and_then(Value::as_bool);
                let show_on_plan = args.get("mostrar_na_planta").and_then(Value::as_bool);
                let consumption = number(args, "consumo_kwh_mes")?;
                let module_wp = number(args, "modulo_wp")?;
                let pr = number(args, "pr")?;
                let offset = number(args, "compensacao_pct")?;
                let city_query = text(args, "cidade");
                if let Some(v) = consumption
                    && !(0.0..=50_000.0).contains(&v)
                {
                    return Err("'consumo_kwh_mes' deve estar entre 0 e 50000 kWh/mês.".to_owned());
                }
                if let Some(v) = module_wp
                    && !(250.0..=800.0).contains(&v)
                {
                    return Err("'modulo_wp' deve estar entre 250 e 800 Wp.".to_owned());
                }
                if let Some(v) = pr
                    && !(0.60..=0.88).contains(&v)
                {
                    return Err("'pr' deve estar entre 0,60 e 0,88.".to_owned());
                }
                if let Some(v) = offset
                    && !(10.0..=100.0).contains(&v)
                {
                    return Err("'compensacao_pct' deve estar entre 10 e 100 %.".to_owned());
                }
                let preset = match city_query.as_deref() {
                    Some(q) => Some(
                        crate::domain::codes::SiteClimate::find_preset(q).ok_or_else(|| {
                            format!("Cidade '{q}' não encontrada na base solar brasileira.")
                        })?,
                    ),
                    None => None,
                };
                self.apply_change(Some("A configuração fotovoltaica já estava assim."), |project| {
                    if let Some(p) = preset {
                        project.codes.site = p.to_site_climate();
                    }
                    let pv = &mut project.electrical.pv;
                    if let Some(en) = enabled {
                        pv.enabled = en;
                    }
                    if let Some(show) = show_on_plan {
                        pv.show_on_plan = show;
                    }
                    if let Some(c) = consumption {
                        pv.monthly_consumption_kwh = (c > 0.0).then_some(c);
                    }
                    if let Some(wp) = module_wp {
                        pv.module_wp = wp;
                    }
                    if let Some(p) = pr {
                        pv.performance_ratio = p;
                    }
                    if let Some(o) = offset {
                        pv.target_offset_pct = o;
                    }
                    let rep = project.photovoltaic_report();
                    Ok(Change::new(rep.summary_sentence))
                })
            }
            "seletividade" => {
                use crate::domain::distribution::selectivity::{
                    short_circuit_csv, short_circuit_sheet_svg,
                };
                let r = self.project.short_circuit_report();
                let mut exported: Option<String> = None;
                if let Some(raw) = text(args, "salvar_csv") {
                    let path = unique_path(resolve_path(&raw, "csv"));
                    ensure_parent(&path)?;
                    std::fs::write(&path, short_circuit_csv(&self.project))
                        .map_err(|e| format!("Não foi possível salvar o CSV: {e}"))?;
                    exported = Some(path.display().to_string());
                } else if let Some(raw) = text(args, "salvar_svg") {
                    let path = unique_path(resolve_path(&raw, "svg"));
                    ensure_parent(&path)?;
                    let svg = short_circuit_sheet_svg(&self.project);
                    crate::domain::views::export_view_svg(&svg, &path)?;
                    exported = Some(path.display().to_string());
                } else if let Some(raw) = text(args, "salvar_pdf") {
                    let path = unique_path(resolve_path(&raw, "pdf"));
                    ensure_parent(&path)?;
                    let svg = short_circuit_sheet_svg(&self.project);
                    crate::domain::views::export_view_pdf(&svg, &path)?;
                    exported = Some(path.display().to_string());
                }
                let msg = format!(
                    "Curto-circuito e seletividade: {} quadro(s), Icc máx {:.2} kA (Icn adotado {:.1} kA, mín. recomendado {:.1} kA), {} par(es) de seletividade.{}",
                    r.boards.len(),
                    r.max_icc_ka,
                    r.settings.breaker_icn_ka,
                    r.min_recommended_icn_ka,
                    r.selectivity.len(),
                    exported
                        .as_deref()
                        .map_or(String::new(), |p| format!(" Exportado em {p}."))
                );
                Ok(ToolOutput::ok(msg).with_structured(json!({
                    "icc_max_ka": (r.max_icc_ka * 100.0).round() / 100.0,
                    "icn_adotado_ka": r.settings.breaker_icn_ka,
                    "icn_recomendado_ka": r.min_recommended_icn_ka,
                    "semaforo": r.light.key(),
                    "quadros": r.boards.iter().map(|b| json!({
                        "rotulo": b.label,
                        "principal": b.is_main,
                        "comodo": b.room,
                        "montante": b.upstream_board,
                        "alimentador_m": (b.feeder_length_m * 10.0).round() / 10.0,
                        "alimentador_mm2": b.feeder_wire_mm2,
                        "queda_alimentador_pct": (b.feeder_drop_percent * 100.0).round() / 100.0,
                        "disjuntor_a": b.main_breaker_a,
                        "circuitos": b.circuits,
                        "icc_3f_ka": (b.icc_3p_ka * 100.0).round() / 100.0,
                        "icc_1f_ka": (b.icc_1p_ka * 100.0).round() / 100.0,
                        "icc_max_ka": (b.icc_max_ka * 100.0).round() / 100.0,
                        "icn_recomendado_ka": b.recommended_icn_ka,
                        "semaforo": b.light.key(),
                        "observacao": b.note,
                    })).collect::<Vec<_>>(),
                    "seletividade": r.selectivity.iter().map(|s| json!({
                        "montante": s.upstream_label,
                        "montante_a": s.upstream_in_a,
                        "jusante": s.downstream_label,
                        "jusante_a": s.downstream_in_a,
                        "curva_jusante": s.downstream_curve.label(),
                        "razao": (s.ratio * 100.0).round() / 100.0,
                        "limite_is_ka": (s.selectivity_limit_ka * 100.0).round() / 100.0,
                        "icc_local_ka": (s.icc_downstream_ka * 100.0).round() / 100.0,
                        "status": s.status.key(),
                        "semaforo": s.light.key(),
                        "explicacao": s.explanation,
                    })).collect::<Vec<_>>(),
                    "circuitos": r.circuits.iter().map(|c| json!({
                        "circuito": c.circuit,
                        "quadro": c.board,
                        "queda_total_pct": c.total_drop_percent.map(|d| (d * 100.0).round() / 100.0),
                        "icc_quadro_ka": (c.icc_board_ka * 100.0).round() / 100.0,
                        "icc_min_fim_a": c.icc_min_end_a.round(),
                        "disparo_magnetico_a": c.mag_trip_max_a.round(),
                        "suporte_termico_cabo_ka": (c.cable_thermal_ka * 100.0).round() / 100.0,
                        "disparo_magnetico_ok": c.magnetic_trip_ok,
                        "suporte_termico_ok": c.cable_thermal_ok,
                        "queda_total_ok": c.total_drop_ok,
                        "semaforo": c.light.key(),
                    })).collect::<Vec<_>>(),
                    "arquivo_exportado": exported,
                    "alertas": r.warnings,
                    "premissas": r.assumptions,
                })))
            }
            "configurar_curto_circuito" => {
                let trafo_kva = number(args, "transformador_kva")?;
                let z_pct = number(args, "impedancia_pct")?;
                let drop_m = number(args, "ramal_poste_m")?;
                let drop_mm2 = number(args, "ramal_poste_mm2")?;
                let ent_m = number(args, "ramal_entrada_m")?;
                let icn_ka = number(args, "capacidade_icn_ka")?;
                if let Some(v) = trafo_kva
                    && !(15.0..=1000.0).contains(&v)
                {
                    return Err("'transformador_kva' deve estar entre 15 e 1000 kVA.".to_owned());
                }
                if let Some(v) = z_pct
                    && !(1.5..=10.0).contains(&v)
                {
                    return Err("'impedancia_pct' deve estar entre 1,5 e 10 %.".to_owned());
                }
                if let Some(v) = drop_m
                    && !(1.0..=300.0).contains(&v)
                {
                    return Err("'ramal_poste_m' deve estar entre 1 e 300 m.".to_owned());
                }
                if let Some(v) = drop_mm2
                    && !(6.0..=240.0).contains(&v)
                {
                    return Err("'ramal_poste_mm2' deve estar entre 6 e 240 mm².".to_owned());
                }
                if let Some(v) = ent_m
                    && !(1.0..=150.0).contains(&v)
                {
                    return Err("'ramal_entrada_m' deve estar entre 1 e 150 m.".to_owned());
                }
                if let Some(v) = icn_ka
                    && !(1.5..=25.0).contains(&v)
                {
                    return Err("'capacidade_icn_ka' deve estar entre 1,5 e 25 kA.".to_owned());
                }
                self.apply_change(Some("Os parâmetros de curto-circuito já estavam assim."), |project| {
                    let sc = &mut project.electrical.short_circuit;
                    if let Some(v) = trafo_kva {
                        sc.transformer_kva = v;
                    }
                    if let Some(v) = z_pct {
                        sc.transformer_z_pct = v;
                    }
                    if let Some(v) = drop_m {
                        sc.service_drop_length_m = v;
                    }
                    if let Some(v) = drop_mm2 {
                        sc.service_drop_section_mm2 = v;
                    }
                    if let Some(v) = ent_m {
                        sc.entrance_length_m = v;
                    }
                    if let Some(v) = icn_ka {
                        sc.breaker_icn_ka = v;
                    }
                    let rep = project.short_circuit_report();
                    Ok(Change::new(format!(
                        "Curto-circuito configurado (trafo {:.0} kVA, Icn {:.1} kA): Icc máx {:.2} kA.",
                        sc.transformer_kva, sc.breaker_icn_ka, rep.max_icc_ka
                    )))
                })
            }
            "aterramento_spda" => {
                use crate::domain::electrical::earthing::earthing_spda_sheet_svg;
                let r = self.project.earthing_and_spda_report();
                let mut exported: Option<String> = None;
                if let Some(raw) = text(args, "salvar_svg") {
                    let path = unique_path(resolve_path(&raw, "svg"));
                    ensure_parent(&path)?;
                    let svg = earthing_spda_sheet_svg(&self.project);
                    crate::domain::views::export_view_svg(&svg, &path)?;
                    exported = Some(path.display().to_string());
                } else if let Some(raw) = text(args, "salvar_pdf") {
                    let path = unique_path(resolve_path(&raw, "pdf"));
                    ensure_parent(&path)?;
                    let svg = earthing_spda_sheet_svg(&self.project);
                    crate::domain::views::export_view_pdf(&svg, &path)?;
                    exported = Some(path.display().to_string());
                }
                let msg = format!(
                    "{} {} {}",
                    r.earthing_note,
                    r.spda_note,
                    exported
                        .as_deref()
                        .map_or(String::new(), |p| format!("Exportado em {p}."))
                );
                Ok(ToolOutput::ok(msg.trim().to_owned()).with_structured(json!({
                    "aterramento": {
                        "esquema": r.settings.scheme.key(),
                        "resistividade_solo_ohm_m": r.settings.soil_resistivity_ohm_m,
                        "haste_comprimento_m": r.settings.rod_length_m,
                        "haste_diametro_mm": r.settings.rod_diameter_mm,
                        "espacamento_m": r.settings.rod_spacing_m,
                        "resistencia_haste_ohm": (r.single_rod_ohm * 10.0).round() / 10.0,
                        "numero_hastes": r.rod_count,
                        "resistencia_malha_ohm": (r.array_resistance_ohm * 10.0).round() / 10.0,
                        "meta_ohm": r.target_resistance_ohm,
                        "condutor_enterrado_mm2": r.buried_conductor_mm2,
                        "condutor_eletroduto_mm2": r.conduit_conductor_mm2,
                        "condutor_bep_mm2": r.bonding_conductor_mm2,
                        "caixas_inspecao": r.inspection_boxes,
                        "bep": r.bep_position.map(xy),
                        "hastes": r.rods.iter().map(|h| json!({
                            "indice": h.index,
                            "posicao": xy(h.position),
                        })).collect::<Vec<_>>(),
                        "semaforo": r.earthing_light.key(),
                        "explicacao": r.earthing_note,
                    },
                    "spda": {
                        "cidade": r.city_label,
                        "dias_trovoada_ano": r.thunderstorm_days_year,
                        "densidade_ng": (r.flash_density_ng * 100.0).round() / 100.0,
                        "comprimento_m": (r.building_length_m * 10.0).round() / 10.0,
                        "largura_m": (r.building_width_m * 10.0).round() / 10.0,
                        "altura_m": (r.building_height_m * 10.0).round() / 10.0,
                        "area_exposicao_ad_m2": r.collection_area_ad_m2.round(),
                        "fator_cd": r.settings.surroundings.cd(),
                        "entorno": r.settings.surroundings.key(),
                        "descargas_diretas_nd": r.annual_direct_flashes_nd,
                        "descargas_linha_nl": r.annual_line_flashes_nl,
                        "risco_r1": r.risk_r1,
                        "risco_toleravel_rt": r.tolerable_risk_rt,
                        "spda_exigido": r.spda_required,
                        "acima_limite_altura_15m": r.spda_refused_height,
                        "nivel_protecao_recomendado": r.recommended_np,
                        "semaforo": r.spda_light.key(),
                        "explicacao": r.spda_note,
                    },
                    "semaforo_geral": r.overall_light.key(),
                    "arquivo_exportado": exported,
                    "alertas": r.warnings,
                    "premissas": r.assumptions,
                })))
            }
            "configurar_aterramento" => {
                use crate::domain::electrical::earthing::{EarthingScheme, SpdaSurroundings};
                let scheme = match text(args, "esquema") {
                    Some(raw) => Some(
                        EarthingScheme::parse(&raw)
                            .ok_or("'esquema' deve ser tn_s, tt ou tn_c_s.")?,
                    ),
                    None => None,
                };
                let surroundings = match text(args, "entorno") {
                    Some(raw) => Some(
                        SpdaSurroundings::parse(&raw).ok_or(
                            "'entorno' deve ser cercada_mais_altos, urbano_similar, isolada ou topo_morro.",
                        )?,
                    ),
                    None => None,
                };
                let rho = number(args, "resistividade_solo_ohm_m")?;
                let rods = number(args, "numero_hastes")?;
                let spacing = number(args, "espacamento_m")?;
                let overhead = args.get("rede_aerea").and_then(Value::as_bool);
                let dps = args.get("tem_dps").and_then(Value::as_bool);
                let show = args.get("mostrar_na_planta").and_then(Value::as_bool);
                if let Some(v) = rho
                    && !(20.0..=5000.0).contains(&v)
                {
                    return Err("'resistividade_solo_ohm_m' deve estar entre 20 e 5000 Ω·m.".to_owned());
                }
                if let Some(v) = rods
                    && (!(0.0..=12.0).contains(&v) || v.fract() != 0.0)
                {
                    return Err("'numero_hastes' deve ser um inteiro entre 1 e 12 (ou 0 para automático).".to_owned());
                }
                if let Some(v) = spacing
                    && !(2.0..=10.0).contains(&v)
                {
                    return Err("'espacamento_m' deve estar entre 2 e 10 m.".to_owned());
                }
                self.apply_change(Some("A configuração de aterramento/SPDA já estava assim."), |project| {
                    let e = &mut project.electrical.earthing;
                    if let Some(s) = scheme {
                        e.scheme = s;
                    }
                    if let Some(s) = surroundings {
                        e.surroundings = s;
                    }
                    if let Some(v) = rho {
                        e.soil_resistivity_ohm_m = v;
                    }
                    if let Some(v) = rods {
                        e.rod_count = (v > 0.0).then_some(v as usize);
                    }
                    if let Some(v) = spacing {
                        e.rod_spacing_m = v;
                    }
                    if let Some(v) = overhead {
                        e.overhead_power_line = v;
                    }
                    if let Some(v) = dps {
                        e.has_dps_at_entrance = v;
                    }
                    if let Some(v) = show {
                        e.show_on_plan = v;
                    }
                    let rep = project.earthing_and_spda_report();
                    Ok(Change::new(format!("{} {}", rep.earthing_note, rep.spda_note)))
                })
            }
            _ => Err(
                "Informe 'acao': gerar_pontos, distribuir_circuitos, definir_tensao, gerar_eletrodutos, ligar_pontos, definir_trecho, remover_eletroduto, definir_alimentacao, definir_concessionaria, demanda_coletiva, lista_materiais, luminotecnico, configurar_iluminacao, fotovoltaico, configurar_fotovoltaico, seletividade, configurar_curto_circuito, aterramento_spda ou configurar_aterramento.".to_owned(),
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
            "iluminacao": self.project.lighting_report().rooms.iter().map(|r| json!({
                "comodo": r.room_name,
                "iluminancia_lux": r.calculated_lux.round(),
                "meta_lux": r.target_lux,
                "semaforo": r.light.key(),
            })).collect::<Vec<_>>(),
            "fotovoltaico": self.project.electrical.pv.enabled.then(|| {
                let pv = self.project.photovoltaic_report();
                json!({
                    "cidade": pv.city_label,
                    "potencia_kwp": (pv.installed_kwp * 100.0).round() / 100.0,
                    "modulos": pv.modules_placed.max(pv.modules_required),
                    "inversor_kw": pv.inverter_kw,
                    "geracao_mensal_kwh": pv.mean_monthly_generation_kwh.round(),
                    "semaforo": pv.light.key(),
                })
            }),
            "curto_circuito": (!schedule.circuits.is_empty()).then(|| {
                let sc = crate::domain::distribution::selectivity::short_circuit_report(&self.project, &schedule);
                json!({
                    "icc_max_ka": (sc.max_icc_ka * 100.0).round() / 100.0,
                    "icn_adotado_ka": sc.settings.breaker_icn_ka,
                    "icn_recomendado_ka": sc.min_recommended_icn_ka,
                    "quadros": sc.boards.len(),
                    "pares_seletividade": sc.selectivity.len(),
                    "semaforo": sc.light.key(),
                })
            }),
            "aterramento_spda": (self.project.main_panel().is_some() || !schedule.circuits.is_empty()).then(|| {
                let es = self.project.earthing_and_spda_report();
                json!({
                    "esquema": es.settings.scheme.key(),
                    "hastes": es.rod_count,
                    "resistencia_ohm": (es.array_resistance_ohm * 10.0).round() / 10.0,
                    "spda_exigido": es.spda_required,
                    "risco_r1": es.risk_r1,
                    "semaforo": es.overall_light.key(),
                })
            }),
            "semaforo": super::structure::verdicts_json(&crate::domain::verdict::electrical_project_verdicts(&self.project, &schedule)),
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

    #[test]
    fn mcp_returns_electrical_shopping_list_by_section_and_colour() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.project = crate::domain::samples::SampleProject::CasaTerrea.build();
        app.project.generate_electrical_points();
        let (pw, pd) = crate::domain::fixtures::FixtureKind::Panel.default_size();
        let (rmin, _) = app.project.room_bounds(&app.project.rooms[0]).unwrap();
        app.project
            .add_fixture(
                crate::domain::fixtures::FixtureKind::Panel,
                crate::domain::geometry::Point::new(rmin.x + 0.35, rmin.y + 0.15),
                pw,
                pd,
                0,
            )
            .unwrap();
        app.project.assign_circuits();
        app.project.generate_conduits().unwrap();

        let res = app.run_mcp_tool(
            "projeto_eletrico",
            &json!({ "acao": "lista_materiais", "perda_pct": 10 }),
        );
        assert!(!res.is_error, "{}", res.text);
        let data = res.structured.unwrap();
        assert_eq!(data["tem_eletrodutos"], true);
        assert!(data["total_cabos_m"].as_f64().unwrap() > 50.0);
        assert!(!data["cabos"].as_array().unwrap().is_empty());
        assert!(!data["eletrodutos"].as_array().unwrap().is_empty());
        assert!(!data["caixas_e_quadros"].as_array().unwrap().is_empty());
        assert!(!data["protecao"].as_array().unwrap().is_empty());
    }

    #[test]
    fn mcp_computes_and_configures_room_lighting_levels() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.project = crate::domain::samples::SampleProject::CasaTerrea.build();
        app.project.generate_electrical_points();

        let cfg = app.run_mcp_tool(
            "projeto_eletrico",
            &json!({
                "acao": "configurar_iluminacao",
                "lumens_teto": 2400,
                "fator_manutencao": 0.80,
                "refletancias": "claras"
            }),
        );
        assert!(!cfg.is_error, "{}", cfg.text);

        let res = app.run_mcp_tool("projeto_eletrico", &json!({ "acao": "luminotecnico" }));
        assert!(!res.is_error, "{}", res.text);
        let data = res.structured.unwrap();
        let rooms = data["comodos"].as_array().unwrap();
        assert_eq!(rooms.len(), app.project.rooms.len());
        assert!(rooms.iter().all(|r| r["iluminancia_lux"].as_f64().unwrap() > 0.0));

        let board = app
            .run_mcp_tool("quadro_de_cargas", &json!({}))
            .structured
            .unwrap();
        assert_eq!(
            board["iluminacao"].as_array().unwrap().len(),
            app.project.rooms.len()
        );
    }

    #[test]
    fn mcp_presizes_photovoltaic_system_for_project_city() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.project = crate::domain::samples::SampleProject::CasaTerrea.build();
        let cfg = app.run_mcp_tool(
            "projeto_eletrico",
            &json!({
                "acao": "configurar_fotovoltaico",
                "ativo": true,
                "cidade": "São Paulo",
                "consumo_kwh_mes": 420,
                "modulo_wp": 550
            }),
        );
        assert!(!cfg.is_error, "{}", cfg.text);
        let res = app.run_mcp_tool("projeto_eletrico", &json!({ "acao": "fotovoltaico" }));
        assert!(!res.is_error, "{}", res.text);
        let data = res.structured.unwrap();
        assert_eq!(data["ativo"], true);
        assert_eq!(data["cidade"], "São Paulo / SP");
        assert_eq!(data["geracao_mensal"].as_array().unwrap().len(), 12);
        assert!(data["potencia_instalada_kwp"].as_f64().unwrap() > 2.5);
    }

    #[test]
    fn mcp_evaluates_sub_boards_short_circuit_and_breaker_selectivity() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let built = app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [
                { "ferramenta": "definir_terreno", "argumentos": { "largura": 30, "comprimento": 20 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 1, "y": 1, "largura": 6, "comprimento": 4 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Edícula", "x": 15, "y": 1, "largura": 5, "comprimento": 4 } },
                { "ferramenta": "inserir_ponto_eletrico", "argumentos": { "tipo": "quadro", "x": 1.3, "y": 1.3 } },
                { "ferramenta": "inserir_ponto_eletrico", "argumentos": { "tipo": "quadro", "x": 15.3, "y": 1.3 } },
                { "ferramenta": "projeto_eletrico", "argumentos": { "acao": "gerar_pontos" } },
                { "ferramenta": "projeto_eletrico", "argumentos": { "acao": "distribuir_circuitos" } },
                { "ferramenta": "projeto_eletrico", "argumentos": { "acao": "gerar_eletrodutos" } },
            ] }),
        );
        assert!(!built.is_error, "{}", built.text);

        let cfg = app.run_mcp_tool(
            "projeto_eletrico",
            &json!({
                "acao": "configurar_curto_circuito",
                "transformador_kva": 150,
                "ramal_poste_m": 10,
                "capacidade_icn_ka": 6.0
            }),
        );
        assert!(!cfg.is_error, "{}", cfg.text);

        let res = app.run_mcp_tool("projeto_eletrico", &json!({ "acao": "seletividade" }));
        assert!(!res.is_error, "{}", res.text);
        let data = res.structured.unwrap();
        assert_eq!(data["quadros"].as_array().unwrap().len(), 2);
        assert!(!data["seletividade"].as_array().unwrap().is_empty());
        assert!(data["icc_max_ka"].as_f64().unwrap() > 1.0);
    }

    #[test]
    fn mcp_computes_and_configures_earthing_and_spda_risk() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        app.project = crate::domain::samples::SampleProject::CasaTerrea.build();
        let (pw, pd) = crate::domain::fixtures::FixtureKind::Panel.default_size();
        let (rmin, _) = app.project.room_bounds(&app.project.rooms[0]).unwrap();
        app.project
            .add_fixture(
                crate::domain::fixtures::FixtureKind::Panel,
                crate::domain::geometry::Point::new(rmin.x + 0.35, rmin.y + 0.15),
                pw,
                pd,
                0,
            )
            .unwrap();

        let cfg = app.run_mcp_tool(
            "projeto_eletrico",
            &json!({
                "acao": "configurar_aterramento",
                "esquema": "tn_s",
                "resistividade_solo_ohm_m": 200,
                "entorno": "urbano_similar",
                "tem_dps": true
            }),
        );
        assert!(!cfg.is_error, "{}", cfg.text);

        let res = app.run_mcp_tool("projeto_eletrico", &json!({ "acao": "aterramento_spda" }));
        assert!(!res.is_error, "{}", res.text);
        let data = res.structured.unwrap();
        assert_eq!(data["aterramento"]["numero_hastes"], 4);
        assert_eq!(data["spda"]["spda_exigido"], false);
        assert!(!data["aterramento"]["hastes"].as_array().unwrap().is_empty());
    }
}
