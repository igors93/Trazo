//! Assistant tools for the structure: laying it out, placing columns and
//! footings, joining columns with beams, setting slabs and materials, and
//! reading the pre-sizing with the forces in every beam.

use serde_json::{Value, json};

use super::{Change, ToolOutput, element_id, number, required_number, text, to_model, xy};
use crate::app::{FloorPlanApp, Selection};
use crate::domain::discipline::Discipline;
use crate::domain::fixtures::FixtureKind;
use crate::domain::model::Project;
use crate::domain::structure::{BeamSupport, OccupancyClass, SlabKind, SoilClass, section_cm};

fn parse_soil_class(raw: &str) -> Option<SoilClass> {
    match raw.trim().to_lowercase().as_str() {
        "argila_mole" | "soft_clay" => Some(SoilClass::SoftClay),
        "argila_media" | "argila_média" | "medium_clay" => Some(SoilClass::MediumClay),
        "areia_media" | "areia_média" | "medium_sand" => Some(SoilClass::MediumSand),
        "areia_compacta" | "dense_sand" => Some(SoilClass::DenseSand),
        "rocha" | "rock" => Some(SoilClass::Rock),
        _ => None,
    }
}

fn parse_occupancy_class(raw: &str) -> Option<OccupancyClass> {
    match raw.trim().to_lowercase().as_str() {
        "residencial" | "residential" => Some(OccupancyClass::Residential),
        "escritorio" | "escritório" | "office" => Some(OccupancyClass::Office),
        "garagem" | "garage" => Some(OccupancyClass::Garage),
        "comercial" | "comercio" | "comércio" | "retail" => Some(OccupancyClass::Retail),
        _ => None,
    }
}

/// The soil survey from 'leituras' [{profundidade_m, n, solo}], (#112).
fn survey_arg(args: &Value) -> Result<crate::domain::soil::SoilSurvey, String> {
    use crate::domain::soil::{SoilKind, SoilSurvey, SptReading};
    let rows = args
        .get("leituras")
        .and_then(Value::as_array)
        .ok_or("Informe 'leituras': [{profundidade_m, n, solo}], uma por metro.")?;
    let mut readings = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let depth = number(row, "profundidade_m")?
            .ok_or_else(|| format!("Leitura {}: falta 'profundidade_m'.", i + 1))?;
        let n = number(row, "n")?.ok_or_else(|| format!("Leitura {}: falta 'n'.", i + 1))?;
        if n < 0.0 || n.fract() != 0.0 {
            return Err(format!(
                "Leitura {}: 'n' é um número inteiro de golpes.",
                i + 1
            ));
        }
        let soil = match text(row, "solo") {
            Some(raw) => SoilKind::parse(&raw).ok_or_else(|| {
                format!(
                    "Leitura {}: 'solo' deve ser argila, silte_argiloso, silte_arenoso ou areia.",
                    i + 1
                )
            })?,
            None => SoilKind::Clay,
        };
        readings.push(SptReading {
            depth_m: depth,
            n: n as u32,
            soil,
        });
    }
    SoilSurvey::new(
        text(args, "nome").unwrap_or_else(|| "SP-01".to_owned()),
        readings,
        number(args, "nivel_agua_m")?,
        number(args, "cota_sapata_m")?.unwrap_or(1.0),
    )
}

/// The foundation study as tool output (#112).
pub(super) fn foundation_study_json(study: &crate::domain::soil::FoundationStudy) -> Value {
    let round1 = |v: f64| (v * 10.0).round() / 10.0;
    json!({
        "com_sondagem": study.survey,
        "carga_total_kn": round1(study.total_load_kn),
        "projecao_m2": round1(study.footprint_m2),
        "solo_kpa_sondagem": study.survey_footing_kpa.map(|v| v.round()),
        "recomendada": study.recommended.map(|o| o.key()),
        "opcoes": study.options.iter().map(|o| json!({
            "tipo": o.option.key(),
            "nome": o.option.label(),
            "viavel": o.viable,
            "resumo": o.summary,
            "observacoes": o.notes,
            "concreto_m3": (o.concrete_m3 * 100.0).round() / 100.0,
            "solo_kpa": o.soil_kpa.map(|v| v.round()),
        })).collect::<Vec<_>>(),
        "broca": study.pile.map(|p| json!({
            "diametro_cm": (p.diameter_m * 100.0).round(),
            "comprimento_m": p.length_m,
            "quantidade": study.piles,
            "carga_admissivel_kn": round1(p.allowable_kn),
            "ponta_ruptura_kn": round1(p.tip_kn),
            "atrito_ruptura_kn": round1(p.shaft_kn),
            "limite_concreto_kn": round1(p.structural_kn),
            "n_ponta": round1(p.tip_n),
            "n_fuste": round1(p.shaft_n),
        })),
    })
}

/// The roof from the arguments of definir_telhado, starting from `base`
/// when editing one (#114).
fn roof_arg(
    args: &Value,
    base: Option<crate::domain::roof::Roof>,
) -> Result<crate::domain::roof::Roof, String> {
    use crate::domain::roof::{Roof, RoofSide, TileKind, TimberClass};
    let mut roof = match base {
        Some(roof) => roof,
        None => {
            let x = required_number(args, "x")?;
            let y = required_number(args, "y")?;
            let width = required_number(args, "largura_m")?;
            let length = required_number(args, "comprimento_m")?;
            let high = required_number(args, "altura_alta_m")?;
            let low = required_number(args, "altura_baixa_m")?;
            Roof::new(
                to_model(x, y),
                to_model(x + width, y + length),
                RoofSide::West,
                high,
                low,
            )
        }
    };
    if base.is_some() && (args.get("x").is_some() || args.get("largura_m").is_some()) {
        let x = required_number(args, "x")?;
        let y = required_number(args, "y")?;
        let width = required_number(args, "largura_m")?;
        let length = required_number(args, "comprimento_m")?;
        let fresh = Roof::new(
            to_model(x, y),
            to_model(x + width, y + length),
            roof.high_side,
            2.0,
            1.0,
        );
        roof.min = fresh.min;
        roof.max = fresh.max;
    }
    if let Some(raw) = text(args, "lado_alto") {
        roof.high_side =
            RoofSide::parse(&raw).ok_or("'lado_alto' deve ser oeste, leste, norte ou sul.")?;
    }
    if let Some(v) = number(args, "altura_alta_m")? {
        roof.high_m = v;
    }
    if let Some(v) = number(args, "altura_baixa_m")? {
        roof.low_m = v;
    }
    if let Some(raw) = text(args, "telha") {
        roof.tile = TileKind::parse(&raw)
            .ok_or("'telha' deve ser ceramica, concreto, fibrocimento ou metalica.")?;
    }
    if let Some(raw) = text(args, "madeira") {
        roof.timber = TimberClass::parse(&raw).ok_or("'madeira' deve ser c20, c30, c40 ou c60.")?;
    }
    if let Some(v) = number(args, "espacamento_vigas_m")? {
        roof.beam_spacing_m = v;
    }
    if let Some(v) = number(args, "espacamento_tercas_m")? {
        roof.purlin_spacing_m = v;
    }
    if let Some(v) = number(args, "espacamento_caibros_m")? {
        roof.rafter_spacing_m = v;
    }
    if let Some(v) = args.get("aparente").and_then(Value::as_bool) {
        roof.exposed = v;
    }
    Ok(roof)
}

/// A roof result as tool output (#114).
pub(super) fn roof_json(result: &crate::domain::roof::RoofResult) -> Value {
    let round2 = |v: f64| (v * 100.0).round() / 100.0;
    json!({
        "id": result.id,
        "rotulo": result.label,
        "inclinacao_pct": round2(result.slope_percent),
        "inclinacao_minima_pct": result.min_slope_percent,
        "carga_permanente_kn_m2": round2(result.dead_kn_m2),
        "sobrecarga_kn_m2": result.live_kn_m2,
        "area_telhado_m2": round2(result.surface_m2),
        "telhas": result.tiles,
        "madeira_m3": round2(result.timber_m3),
        "pecas": result.elements.iter().map(|e| json!({
            "peca": e.element.key(),
            "secao_cm": format!("{}x{}", e.section_cm.0, e.section_cm.1),
            "atende": e.ok,
            "vao_m": round2(e.span_m),
            "espacamento_m": round2(e.spacing_m),
            "carga_kn_m": round2(e.load_kn_m),
            "flexao": round2(e.bending_ratio),
            "cisalhamento": round2(e.shear_ratio),
            "flecha": round2(e.deflection_ratio),
            "quantidade": e.count,
            "comprimento_total_m": round2(e.total_length_m),
            "volume_m3": round2(e.volume_m3),
        })).collect::<Vec<_>>(),
        "apoios": result.reactions.iter().map(|r| json!({
            "ponto": xy(r.point),
            "lado": if r.high { "alto" } else { "baixo" },
            "carga_kn": round2(r.load_kn),
            "permanente_kn": round2(r.dead_kn),
            "apoio": r.support,
        })).collect::<Vec<_>>(),
    })
}

/// A size argument in centimetres, as metres.
fn centimetres(args: &Value, key: &str) -> Result<Option<f64>, String> {
    Ok(number(args, key)?.map(|cm| cm / 100.0))
}

/// The room named by 'comodo' (its name, or its id).
fn room_arg(project: &Project, args: &Value) -> Result<u64, String> {
    if let Some(id) = args.get("comodo").and_then(Value::as_u64) {
        return project
            .rooms
            .iter()
            .find(|room| room.id == id)
            .map(|room| room.id)
            .ok_or_else(|| format!("Cômodo {id} não encontrado."));
    }
    let name = text(args, "comodo").ok_or("Informe 'comodo' (nome ou id).")?;
    project
        .rooms
        .iter()
        .find(|room| room.display_name().eq_ignore_ascii_case(name.trim()))
        .map(|room| room.id)
        .ok_or_else(|| format!("Nenhum cômodo chamado '{name}'."))
}

/// Traffic-light verdicts as tool output (#101).
pub(super) fn verdicts_json(verdicts: &[crate::domain::verdict::Verdict]) -> Value {
    Value::Array(
        verdicts
            .iter()
            .map(|verdict| {
                json!({
                    "elemento": verdict.element,
                    "disciplina": verdict.discipline.label(),
                    "semaforo": verdict.light().key(),
                    "verificacoes": verdict.checks.iter().map(|check| json!({
                        "nome": check.name,
                        "semaforo": check.light.key(),
                        "uso_pct": check.utilisation.is_finite().then(|| (check.utilisation * 1000.0).round() / 10.0),
                        "folga_pct": check.utilisation.is_finite().then(|| (check.margin_percent() * 10.0).round() / 10.0),
                        "o_que_fazer": check.simple,
                        "detalhe": check.detail,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

impl FloorPlanApp {
    pub(super) fn tool_structural_design(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let action = text(args, "acao").unwrap_or_default();
        match action.as_str() {
            "gerar_estrutura" => self.apply_change(
                Some("A estrutura já cobre todas as paredes e cômodos."),
                |project| {
                    if project.walls.is_empty() {
                        return Err("Desenhe as paredes antes de gerar a estrutura.".to_owned());
                    }
                    if let Some(refusal) = crate::domain::limits::structural_refusal(project) {
                        return Err(refusal);
                    }
                    let report = project.generate_structure();
                    project.ensure_default_layers();
                    let skipped = if report.skipped > 0 {
                        format!(
                            " {} pilar(es) não couberam no terreno e ficaram de fora.",
                            report.skipped
                        )
                    } else {
                        String::new()
                    };
                    Ok(Change::new(format!(
                        "Estrutura gerada: {} pilar(es), {} viga(s), {} laje(s), {} sapata(s).{skipped}",
                        report.columns, report.beams, report.slabs, report.footings
                    ))
                    .data(json!({
                        "pilares": report.columns,
                        "vigas": report.beams,
                        "lajes": report.slabs,
                        "sapatas": report.footings,
                        "pilares_fora": report.skipped,
                    })))
                },
            ),
            "inserir_pilar" | "inserir_sapata" => {
                let kind = if action == "inserir_pilar" {
                    FixtureKind::Column
                } else {
                    FixtureKind::Footing
                };
                let point = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let (default_w, default_d) = kind.default_size();
                let width = centimetres(args, "largura_cm")?.unwrap_or(default_w);
                let depth = centimetres(args, "profundidade_cm")?.unwrap_or(default_d);
                self.apply_change(None, |project| {
                    // Flush inside the lot on a boundary wall (B23).
                    let (center, turns, angle_rad) =
                        project.structural_placement_fitted(kind, point, width, depth);
                    let id = project
                        .add_fixture(kind, center, width, depth, turns)
                        .ok_or_else(|| {
                            format!(
                                "Não foi possível inserir: confira o tamanho ({:.0} cm a {:.0} m) e se o ponto está no terreno.",
                                kind.min_size() * 100.0,
                                crate::domain::fixtures::MAX_SIZE
                            )
                        })?;
                    if angle_rad.abs() > 1e-9
                        && let Some(f) = project.fixtures.iter_mut().find(|f| f.id == id)
                    {
                        f.angle_rad = angle_rad;
                    }
                    project.ensure_default_layers();
                    let label = project.structure_labels().get(&id).cloned();
                    Ok(Change::new(format!(
                        "{} {} inserido(a) (id {id}){}.",
                        kind.label(),
                        label.clone().unwrap_or_default(),
                        if center == point { "" } else { ", no eixo da parede" }
                    ))
                    .select(Selection::Fixture(id))
                    .data(json!({
                        "id": id,
                        "rotulo": label,
                        "centro": xy(center),
                        "secao_cm": section_cm(width, depth),
                    })))
                })
            }
            "inserir_carga" => {
                let kind = match text(args, "tipo")
                    .unwrap_or_else(|| "caixa_agua".to_owned())
                    .to_lowercase()
                    .as_str()
                {
                    "caixa_agua" | "caixa-d'agua" | "caixa-d'água" => FixtureKind::WaterTank,
                    "equipamento" | "equipamento_pesado" => FixtureKind::HeavyEquipment,
                    _ => return Err("'tipo' deve ser caixa_agua ou equipamento_pesado.".to_owned()),
                };
                let point = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let (default_w, default_d) = kind.default_size();
                let width = centimetres(args, "largura_cm")?.unwrap_or(default_w);
                let depth = centimetres(args, "profundidade_cm")?.unwrap_or(default_d);
                let load_kn = number(args, "carga_kn")?
                    .or_else(|| kind.default_structural_load_kn())
                    .unwrap_or(0.0);
                if !(0.1..=2_000.0).contains(&load_kn) {
                    return Err("'carga_kn' deve ficar entre 0,1 e 2.000.".to_owned());
                }
                self.apply_change(None, |project| {
                    let id = project
                        .add_fixture(kind, point, width, depth, 0)
                        .ok_or("A carga deve caber dentro do terreno.")?;
                    let fixture = project
                        .fixtures
                        .iter_mut()
                        .find(|fixture| fixture.id == id)
                        .ok_or("Carga não encontrada depois da inserção.")?;
                    fixture.structural_load_kn = Some(load_kn);
                    project.ensure_default_layers();
                    Ok(Change::new(format!(
                        "{} inserido(a) com {} kN (id {id}).",
                        kind.label(),
                        round1(load_kn)
                    ))
                    .select(Selection::Fixture(id))
                    .data(json!({
                        "id": id,
                        "tipo": kind.key(),
                        "centro": xy(point),
                        "carga_kn": round1(load_kn),
                        "base_cm": section_cm(width, depth),
                    })))
                })
            }
            "editar_carga" => {
                let id = element_id(args, "id")?;
                let load_kn = required_number(args, "carga_kn")?;
                if !(0.1..=2_000.0).contains(&load_kn) {
                    return Err("'carga_kn' deve ficar entre 0,1 e 2.000.".to_owned());
                }
                self.apply_change(Some("A carga já tinha esse valor."), |project| {
                    let fixture = project
                        .fixtures
                        .iter_mut()
                        .find(|fixture| {
                            fixture.id == id
                                && matches!(
                                    fixture.kind,
                                    FixtureKind::WaterTank | FixtureKind::HeavyEquipment
                                )
                        })
                        .ok_or("Carga concentrada não encontrada.")?;
                    fixture.structural_load_kn = Some(load_kn);
                    Ok(Change::new(format!(
                        "Carga {} atualizada para {} kN.",
                        id,
                        round1(load_kn)
                    )))
                })
            }
            "remover_carga" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    let exists = project.fixtures.iter().any(|fixture| {
                        fixture.id == id
                            && matches!(
                                fixture.kind,
                                FixtureKind::WaterTank | FixtureKind::HeavyEquipment
                            )
                    });
                    if !exists || !project.delete_fixture(id) {
                        return Err("Carga concentrada não encontrada.".to_owned());
                    }
                    Ok(Change::new(format!("Carga {id} removida.")))
                })
            }
            "ligar_pilares" => {
                let from = element_id(args, "de")?;
                let to = element_id(args, "para")?;
                let width = centimetres(args, "largura_cm")?;
                let height = centimetres(args, "altura_cm")?;
                self.apply_change(None, |project| {
                    let centre = |id: u64| {
                        project
                            .fixtures
                            .iter()
                            .find(|f| f.id == id && f.kind == FixtureKind::Column)
                            .map(|f| (f.center, f.width.min(f.depth)))
                    };
                    let (a, b) = (centre(from), centre(to));
                    let (Some((a, side)), Some((b, _))) = (a, b) else {
                        return Err("'de' e 'para' devem ser ids de pilares.".to_owned());
                    };
                    let span = a.distance(b);
                    let width = width.unwrap_or(side.max(0.12));
                    let height =
                        height.unwrap_or(((span / 10.0 / 0.05).ceil() * 0.05).max(0.30));
                    let id = project.add_beam(from, to, width, height)?;
                    Ok(Change::new(format!(
                        "Viga {id} ({}) ligando os pilares {from} e {to}.",
                        section_cm(width, height)
                    ))
                    .data(json!({ "id": id, "vao_m": round1(span * 10.0) / 10.0, "secao_cm": section_cm(width, height) })))
                })
            }
            "editar_viga" => {
                let id = element_id(args, "id")?;
                let width = centimetres(args, "largura_cm")?;
                let height = centimetres(args, "altura_cm")?;
                if width.is_none() && height.is_none() {
                    return Err("Informe largura_cm ou altura_cm.".to_owned());
                }
                self.apply_change(Some("A viga já tinha essa seção."), |project| {
                    project.update_beam(id, width, height)?;
                    Ok(Change::new(format!("Viga {id} atualizada.")))
                })
            }
            "remover_viga" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.delete_beam(id) {
                        return Err("Viga não encontrada.".to_owned());
                    }
                    Ok(Change::new(format!("Viga {id} removida.")))
                })
            }
            "definir_laje" => {
                let kind = match text(args, "tipo") {
                    None => SlabKind::Ribbed,
                    Some(key) => SlabKind::from_key(&key.to_lowercase())
                        .ok_or("'tipo' deve ser trelicada ou macica.")?,
                };
                let thickness = centimetres(args, "espessura_cm")?;
                self.apply_change(Some("A laje já era assim."), |project| {
                    let room = room_arg(project, args)?;
                    let id = project.set_slab(room, kind, thickness)?;
                    Ok(Change::new(format!("Laje {id} {}.", kind.label().to_lowercase()))
                        .data(json!({ "id": id })))
                })
            }
            "remover_laje" => self.apply_change(None, |project| {
                let room = room_arg(project, args)?;
                if !project.remove_slab(room) {
                    return Err("Esse cômodo não tem laje.".to_owned());
                }
                Ok(Change::new("Laje removida.".to_owned()))
            }),
            "configurar" => {
                let mut settings = self.project.structure.settings;
                if let Some(raw) = text(args, "classe_solo") {
                    let soil = parse_soil_class(&raw).ok_or(
                        "'classe_solo' deve ser argila_mole, argila_media, areia_media, areia_compacta ou rocha.",
                    )?;
                    settings.soil_kpa = soil.allowable_kpa();
                }
                if let Some(raw) = text(args, "ocupacao") {
                    let occ = parse_occupancy_class(&raw).ok_or(
                        "'ocupacao' deve ser residencial, escritorio, garagem ou comercial.",
                    )?;
                    settings.live_kn_m2 = occ.live_kn_m2();
                }
                if let Some(v) = number(args, "fck_mpa")? {
                    if !(20.0..=50.0).contains(&v) {
                        return Err("'fck_mpa' deve ficar entre 20 e 50.".to_owned());
                    }
                    settings.fck_mpa = v;
                }
                if let Some(v) = number(args, "solo_kpa")? {
                    if !(50.0..=600.0).contains(&v) {
                        return Err("'solo_kpa' deve ficar entre 50 e 600.".to_owned());
                    }
                    settings.soil_kpa = v;
                }
                if let Some(v) = number(args, "pavimentos")? {
                    if !(1.0..=10.0).contains(&v) || v.fract() != 0.0 {
                        return Err("'pavimentos' deve ser um inteiro de 1 a 10.".to_owned());
                    }
                    settings.floors = v as u32;
                }
                if let Some(v) = number(args, "sobrecarga_kn_m2")? {
                    if !(0.5..=10.0).contains(&v) {
                        return Err("'sobrecarga_kn_m2' deve ficar entre 0,5 e 10.".to_owned());
                    }
                    settings.live_kn_m2 = v;
                }
                if let Some(v) = number(args, "revestimento_kn_m2")? {
                    if !(0.0..=5.0).contains(&v) {
                        return Err("'revestimento_kn_m2' deve ficar entre 0 e 5.".to_owned());
                    }
                    settings.finish_kn_m2 = v;
                }
                self.apply_change(Some("Nada mudou nos materiais e cargas."), |project| {
                    project.structure.settings = settings;
                    Ok(Change::new("Materiais e cargas atualizados.".to_owned()))
                })
            }
            "ajustar_sapatas" => self.apply_change(
                Some("As sapatas já têm o tamanho que o solo pede."),
                |project| {
                    if let Some(refusal) = crate::domain::limits::structural_refusal(project) {
                        return Err(refusal);
                    }
                    let changed = project.size_footings();
                    Ok(Change::new(format!("{changed} sapata(s) redimensionada(s).")))
                },
            ),
            "apoiar_em_viga" => {
                let width = centimetres(args, "largura_cm")?.unwrap_or(0.14);
                let height = centimetres(args, "altura_cm")?.unwrap_or(0.35);
                let from_support = if let Some(bid) = args.get("viga_de").and_then(Value::as_u64) {
                    let t = number(args, "t_de")?.unwrap_or(0.5);
                    BeamSupport::Beam { beam_id: bid, t }
                } else {
                    BeamSupport::Column {
                        id: element_id(args, "de")?,
                    }
                };
                let to_support = if let Some(bid) = args.get("viga_para").and_then(Value::as_u64) {
                    let t = number(args, "t_para")?.unwrap_or(0.5);
                    BeamSupport::Beam { beam_id: bid, t }
                } else {
                    BeamSupport::Column {
                        id: element_id(args, "para")?,
                    }
                };
                self.apply_change(None, |project| {
                    let id = project.add_beam_on_supports(from_support, to_support, width, height)?;
                    Ok(Change::new(format!(
                        "Viga secundária {id} ({}) criada com apoio em viga.",
                        section_cm(width, height)
                    ))
                    .data(json!({ "id": id, "secao_cm": section_cm(width, height) })))
                })
            }
            "vista" => {
                let discipline = text(args, "disciplina")
                    .and_then(|key| Discipline::from_key(&key))
                    .ok_or("'disciplina' deve ser tudo, arquitetura, eletrica, hidraulica, estrutura ou fundacoes.")?;
                self.set_discipline(discipline);
                Ok(ToolOutput::ok(format!(
                    "Vista: {}.",
                    discipline.label().to_lowercase()
                )))
            }
            "gerar_vigas_alavanca" => self.apply_change(
                Some("Nenhuma sapata de divisa precisa de viga alavanca, ou todas já têm."),
                |project| {
                    let (added, alone) = project.generate_balance_beams();
                    if added == 0 && alone == 0 {
                        return Err("Nenhuma sapata de divisa fora do centro do pilar.".to_owned());
                    }
                    Ok(Change::new(format!(
                        "{added} viga(s) alavanca lançada(s){}.",
                        if alone > 0 {
                            format!(
                                "; {alone} pilar(es) de divisa sem pilar interno a até 10 m para ancorar"
                            )
                        } else {
                            String::new()
                        }
                    ))
                    .data(json!({ "vigas_alavanca": added, "sem_ancoragem": alone })))
                },
            ),
            "ligar_viga_alavanca" => {
                let column = element_id(args, "de")?;
                let anchor = element_id(args, "para")?;
                self.apply_change(None, |project| {
                    let id = project.add_balance_beam(column, anchor)?;
                    Ok(Change::new(format!(
                        "Viga alavanca {id} do pilar {column} ao pilar {anchor}."
                    ))
                    .data(json!({ "id": id })))
                })
            }
            "gerar_fundacao_paredes" => self.apply_change(
                Some("Todas as paredes já têm fundação."),
                |project| {
                    let (beams, strips) = project.generate_wall_foundations();
                    Ok(Change::new(format!(
                        "Fundação das paredes: {beams} viga(s) baldrame e {strips} sapata(s) corrida(s)."
                    ))
                    .data(json!({ "baldrames": beams, "sapatas_corridas": strips })))
                },
            ),
            "definir_fundacao_parede" => {
                let wall = element_id(args, "id")?;
                let kind = match text(args, "tipo").as_deref() {
                    Some("baldrame") | None => {
                        crate::domain::wall_foundations::WallFoundationKind::GroundBeam
                    }
                    Some("sapata_corrida") => {
                        crate::domain::wall_foundations::WallFoundationKind::StripFooting
                    }
                    Some(_) => return Err("'tipo' deve ser baldrame ou sapata_corrida.".to_owned()),
                };
                let width = centimetres(args, "largura_cm")?;
                self.apply_change(Some("A fundação da parede já era essa."), |project| {
                    let id = project.set_wall_foundation(wall, kind, width)?;
                    Ok(Change::new(format!(
                        "Parede {wall}: {}.",
                        kind.label().to_lowercase()
                    ))
                    .data(json!({ "id": id })))
                })
            }
            "remover_fundacao_paredes" => self.apply_change(None, |project| {
                let count = project.remove_wall_foundations();
                if count == 0 {
                    return Err("Nenhuma fundação de parede para remover.".to_owned());
                }
                Ok(Change::new(format!("{count} fundação(ões) de parede removida(s).")))
            }),
            "definir_sondagem" => {
                let survey = survey_arg(args)?;
                let apply = args
                    .get("aplicar_ao_solo")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                self.apply_change(None, |project| {
                    let name = survey.name.clone();
                    let count = survey.readings.len();
                    let deepest = survey.deepest_m();
                    project.set_soil_survey(Some(survey));
                    let soil = if apply {
                        project.apply_survey_to_soil()
                    } else {
                        None
                    };
                    let study = project.foundation_study();
                    Ok(Change::new(format!(
                        "Sondagem {name}: {count} leitura(s) até {} m{}{}",
                        format!("{deepest:.1}").replace('.', ","),
                        soil.map_or(String::new(), |kpa| format!(
                            "; tensão do solo agora {kpa:.0} kPa"
                        )),
                        study.recommended.map_or(".".to_owned(), |o| format!(
                            "; fundação recomendada: {}.",
                            o.label().to_lowercase()
                        ))
                    ))
                    .data(foundation_study_json(&study)))
                })
            }
            "remover_sondagem" => self.apply_change(None, |project| {
                if project.structure.soil_survey.is_none() {
                    return Err("O projeto não tem sondagem.".to_owned());
                }
                project.set_soil_survey(None);
                Ok(Change::new("Sondagem removida.".to_owned()))
            }),
            "simular_fundacoes" => {
                let study = self.project.foundation_study();
                if study.options.is_empty() {
                    return Err(
                        "Nada para fundar ainda: gere a estrutura (pilares) e as fundações das paredes."
                            .to_owned(),
                    );
                }
                Ok(ToolOutput::ok(format!(
                    "{} opção(ões) viável(is) de {}; recomendada: {}.",
                    study.options.iter().filter(|o| o.viable).count(),
                    study.options.len(),
                    study
                        .recommended
                        .map_or("nenhuma (consulte um engenheiro de fundações)", |o| o.label())
                ))
                .with_structured(foundation_study_json(&study)))
            }
            "inserir_mourao" => {
                let x = required_number(args, "x")?;
                let y = required_number(args, "y")?;
                let size = centimetres(args, "diametro_cm")?.unwrap_or(0.15);
                if !(0.10..=0.40).contains(&size) {
                    return Err("'diametro_cm' deve ficar entre 10 e 40.".to_owned());
                }
                let timber = match text(args, "madeira") {
                    Some(raw) => Some(
                        crate::domain::roof::TimberClass::parse(&raw)
                            .ok_or("'madeira' deve ser c20, c30, c40 ou c60.")?,
                    ),
                    None => None,
                };
                self.apply_change(None, |project| {
                    if let Some(class) = timber {
                        project.structure.post_timber = class;
                    }
                    let id = project
                        .add_fixture(FixtureKind::TimberPost, to_model(x, y), size, size, 0)
                        .ok_or("O mourão não cabe aí (fora do terreno?).")?;
                    Ok(
                        Change::new(format!("Mourão {id} de Ø {:.0} cm.", size * 100.0))
                            .select(Selection::Fixture(id))
                            .data(json!({ "id": id })),
                    )
                })
            }
            "definir_telhado" => {
                let existing = match number(args, "id")? {
                    Some(id) => Some(
                        self.project
                            .structure
                            .roofs
                            .iter()
                            .find(|r| r.id == id as u64)
                            .copied()
                            .ok_or_else(|| format!("Telhado {id} não encontrado."))?,
                    ),
                    None => None,
                };
                let roof = roof_arg(args, existing)?;
                let fit = args
                    .get("ajustar_paredes")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                self.apply_change(None, |project| {
                    let id = match existing {
                        Some(old) => {
                            project.update_roof(old.id, roof)?;
                            old.id
                        }
                        None => project.add_roof(roof)?,
                    };
                    let walls = if fit {
                        project.fit_walls_to_roof(id)?
                    } else {
                        0
                    };
                    let report = project.structure_report();
                    let result = report
                        .roofs
                        .iter()
                        .find(|r| r.id == id)
                        .ok_or("Telhado não analisado.")?;
                    Ok(Change::new(format!(
                        "Telhado {}: meia-água de {:.0} %, {} m³ de madeira{}{}.",
                        result.label,
                        result.slope_percent,
                        format!("{:.2}", result.timber_m3).replace('.', ","),
                        if walls > 0 {
                            format!(", {walls} parede(s) ajustada(s) à altura do telhado")
                        } else {
                            String::new()
                        },
                        match result.unsupported() {
                            0 => String::new(),
                            n => format!("; {n} ponta(s) de viga sem apoio embaixo"),
                        }
                    ))
                    .data(roof_json(result)))
                })
            }
            "remover_telhado" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.remove_roof(id) {
                        return Err("Telhado não encontrado.".to_owned());
                    }
                    Ok(Change::new(format!("Telhado {id} removido.")))
                })
            }
            "remover_viga_alavanca" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.delete_balance_beam(id) {
                        return Err("Viga alavanca não encontrada.".to_owned());
                    }
                    Ok(Change::new(format!("Viga alavanca {id} removida.")))
                })
            }
            _ => Err(
                "Informe 'acao': gerar_estrutura, inserir_pilar, inserir_sapata, inserir_carga, editar_carga, remover_carga, ligar_pilares, apoiar_em_viga, editar_viga, remover_viga, definir_laje, remover_laje, configurar, ajustar_sapatas, gerar_vigas_alavanca, ligar_viga_alavanca, remover_viga_alavanca, gerar_fundacao_paredes, definir_fundacao_parede, remover_fundacao_paredes, definir_sondagem, remover_sondagem, simular_fundacoes, definir_telhado, remover_telhado, inserir_mourao ou vista.".to_owned(),
            ),
        }
    }

    pub(super) fn tool_presizing(&mut self) -> Result<ToolOutput, String> {
        let report = self.project.cached_structure_report();
        let reinforcement = self.project.cached_reinforcement_report();
        let settings = self.project.structure.settings;
        let slabs: Vec<Value> = report
            .slabs
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "rotulo": s.label,
                    "comodo": s.room,
                    "tipo": s.kind.key(),
                    "vaos_m": [round1(s.lx_m * 10.0) / 10.0, round1(s.ly_m * 10.0) / 10.0],
                    "espessura_cm": round1(s.thickness_m * 100.0),
                    "espessura_minima_cm": round1(s.thickness_min_m * 100.0),
                    "carga_kn_m2": round1(s.load_kn_m2 * 10.0) / 10.0,
                    "cargas_concentradas_kn": round1(s.point_load_kn),
                    "momento_local_knm_m": round1(s.point_moment_knm_m),
                    "apoiada": s.supported,
                    "atende": s.ok(),
                })
            })
            .collect();
        let beams: Vec<Value> = report
            .beams
            .iter()
            .map(|line| {
                json!({
                    "rotulo": line.label,
                    "pilares": line.joints,
                    "reacoes_kn": line.reactions.iter().map(|r| round1(*r)).collect::<Vec<_>>(),
                    "vaos": line.spans.iter().map(|s| json!({
                        "id": s.beam,
                        "vao_m": round1(s.length_m * 100.0) / 100.0,
                        "secao_cm": section_cm(s.width_m, s.height_m),
                        "altura_minima_cm": round1(s.height_min_m * 100.0),
                        "carga_permanente_kn_m": round1(s.dead_kn_m),
                        "carga_variavel_kn_m": round1(s.live_kn_m),
                        "momento_apoio_esq_knm": round1(s.moment_left),
                        "momento_apoio_dir_knm": round1(s.moment_right),
                        "momento_vao_knm": round1(s.moment_span),
                        "cortante_kn": round1(s.shear),
                        "flecha_mm": round1(s.deflection_mm),
                        "flecha_limite_mm": round1(s.limit_mm),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let columns: Vec<Value> = report
            .columns
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "rotulo": c.label,
                    "secao_cm": section_cm(c.width_m, c.depth_m),
                    "posicao": c.position.label(),
                    "carga_kn": round1(c.load_kn),
                    "carga_pavimentos_superiores_kn": round1(c.upper_load_kn),
                    "area_necessaria_cm2": c.area_needed_cm2.max(360.0).round(),
                    "area_cm2": c.area_cm2.round(),
                    "altura_m": (c.height_m * 100.0).round() / 100.0,
                    "esbeltez": c.slenderness.round(),
                    "atende": c.ok(),
                })
            })
            .collect();
        let footings: Vec<Value> = report
            .footings
            .iter()
            .map(|f| {
                json!({
                    "id": f.id,
                    "rotulo": f.label,
                    "pilar": f.column,
                    "dimensoes_cm": section_cm(f.width_m, f.depth_m),
                    "carga_kn": round1(f.load_kn),
                    "necessaria_cm": section_cm(f.needed_width_m, f.needed_depth_m),
                    "altura_cm": round1(f.height_m * 100.0),
                    "excentricidade_m": (f.eccentricity_m * 1000.0).round() / 1000.0,
                    "tensao_max_kpa": round1(f.max_pressure_kpa),
                    "puncao_razao": (f.punching_ratio * 100.0).round() / 100.0,
                    "fator_tombamento": f.overturning_fs.is_finite().then(|| round1(f.overturning_fs)),
                    "atende": f.ok(),
                })
            })
            .collect();
        let misaligned: Vec<Value> = report
            .misaligned_columns
            .iter()
            .map(|m| {
                json!({
                    "pavimento": m.floor_name,
                    "pilar": m.column_label,
                    "centro": xy(m.center),
                    "carga_kn": round1(m.load_kn),
                    "viga_transicao": m.transfer_beam,
                })
            })
            .collect();
        let concentrated_loads: Vec<Value> = report
            .concentrated_loads
            .iter()
            .map(|load| {
                json!({
                    "id": load.id,
                    "rotulo": load.label,
                    "tipo": load.kind.key(),
                    "centro": xy(load.center),
                    "carga_kn": round1(load.load_kn),
                    "apoio": load.support,
                    "laje": load.slab,
                    "momento_local_knm_m": round1(load.local_moment_knm_m),
                    "puncao_razao": load.punching_ratio.map(|ratio| (ratio * 100.0).round() / 100.0),
                    "apoiada": load.supported,
                })
            })
            .collect();
        let reinforcement_beams: Vec<Value> = reinforcement
            .beams
            .iter()
            .map(|beam| {
                json!({
                    "id": beam.beam_id,
                    "rotulo": beam.label,
                    "vao": beam.span_index,
                    "comprimento_m": round1(beam.length_m * 10.0) / 10.0,
                    "inferior": beam.bottom.notation(),
                    "superior_esquerda": beam.top_left.notation(),
                    "superior_direita": beam.top_right.notation(),
                    "estribos": beam.stirrups.notation(),
                    "as_inferior_necessaria_cm2": round1(beam.bottom.required_cm2),
                    "as_inferior_fornecida_cm2": round1(beam.bottom.provided_cm2),
                    "barras_cabem_na_secao": beam.bottom.fits_section
                        && beam.top_left.fits_section
                        && beam.top_right.fits_section,
                    "cortante_atende": beam.stirrups.shear_capacity_ok,
                    "massa_kg": round1(beam.mass_kg),
                })
            })
            .collect();
        let reinforcement_columns: Vec<Value> = reinforcement
            .columns
            .iter()
            .map(|column| {
                json!({
                    "id": column.column_id,
                    "rotulo": column.label,
                    "longitudinal": column.longitudinal.notation(),
                    "estribos": column.ties.notation(),
                    "as_necessaria_cm2": round1(column.longitudinal.required_cm2),
                    "as_fornecida_cm2": round1(column.longitudinal.provided_cm2),
                    "taxa_aco_percentual": round1(column.steel_ratio * 100.0),
                    "massa_kg": round1(column.mass_kg),
                })
            })
            .collect();
        let reinforcement_slabs: Vec<Value> = reinforcement
            .slabs
            .iter()
            .map(|slab| {
                json!({
                    "id": slab.slab_id,
                    "rotulo": slab.label,
                    "comodo": slab.room,
                    "tipo": slab.kind.key(),
                    "principal": slab.main.notation(),
                    "distribuicao": slab.distribution.notation(),
                    "as_principal_necessaria_cm2_m": round1(slab.main.required_cm2_m),
                    "as_principal_fornecida_cm2_m": round1(slab.main.provided_cm2_m),
                    "massa_kg": round1(slab.mass_kg),
                })
            })
            .collect();
        let reinforcement_footings: Vec<Value> = reinforcement
            .footings
            .iter()
            .map(|footing| {
                json!({
                    "id": footing.footing_id,
                    "rotulo": footing.label,
                    "direcao_x": footing.bars_x.notation(),
                    "direcao_y": footing.bars_y.notation(),
                    "massa_kg": round1(footing.mass_kg),
                })
            })
            .collect();
        let steel_schedule: Vec<Value> = reinforcement
            .schedule
            .iter()
            .map(|row| {
                json!({
                    "marca": row.mark,
                    "elemento": row.element,
                    "posicao": row.position,
                    "diametro_mm": row.diameter_mm,
                    "quantidade": row.quantity,
                    "comprimento_unitario_m": round1(row.unit_length_m * 10.0) / 10.0,
                    "comprimento_total_m": round1(row.total_length_m * 10.0) / 10.0,
                    "massa_kg": round1(row.mass_kg),
                })
            })
            .collect();
        Ok(ToolOutput::ok(format!(
            "{} laje(s), {} viga(s), {} pilar(es), {} sapata(s), {} kg de aço preliminar, {} alerta(s).",
            slabs.len(),
            beams.len(),
            columns.len(),
            footings.len(),
            round1(reinforcement.total_mass_kg()),
            report.warnings.len()
        ))
        .with_structured(json!({
            "projeto_id": self.active_project_id(),
            "materiais": {
                "fck_mpa": settings.fck_mpa,
                "solo_kpa": settings.soil_kpa,
                "pavimentos": settings.floors,
                "sobrecarga_kn_m2": settings.live_kn_m2,
                "revestimento_kn_m2": settings.finish_kn_m2,
                "sobrecarga_por_comodo": settings.per_room_live_loads,
            },
            "lajes": slabs,
            "vigas": beams,
            "pilares": columns,
            "sapatas": footings,
            "telhados": report.roofs.iter().map(roof_json).collect::<Vec<_>>(),
            "mouroes": report.timber_posts.iter().map(|p| json!({
                "id": p.id,
                "rotulo": p.label,
                "secao": if p.round { "redonda" } else { "quadrada" },
                "diametro_cm": round1(p.size_m * 100.0),
                "altura_m": (p.height_m * 100.0).round() / 100.0,
                "carga_kn": round1(p.load_kn),
                "esbeltez": p.slenderness.round(),
                "uso": (p.ratio.min(9.99) * 100.0).round() / 100.0,
                "atende": p.ok(),
                "diametro_necessario_cm": p.needed_m.map(|s| round1(s * 100.0)),
                "base_cm": round1(p.base_m * 100.0),
                "pressao_base_kpa": round1(p.base_kpa),
            })).collect::<Vec<_>>(),
            "vigas_madeira": report.eave_beams.iter().map(|b| json!({
                "rotulo": b.label,
                "de": b.from_post,
                "para": b.to_post,
                "vao_m": (b.span_m * 100.0).round() / 100.0,
                "secao_cm": format!("{}x{}", b.section_cm.0, b.section_cm.1),
                "atende": b.ok,
                "cargas_kn": b.loads.iter().map(|(_, p)| round1(*p)).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "fundacao_paredes": report.wall_foundations.iter().map(|f| json!({
                "rotulo": f.label,
                "parede": f.wall_id,
                "tipo": match f.kind {
                    crate::domain::wall_foundations::WallFoundationKind::GroundBeam => "baldrame",
                    crate::domain::wall_foundations::WallFoundationKind::StripFooting => "sapata_corrida",
                },
                "comprimento_m": round1(f.length_m * 100.0) / 100.0,
                "carga_kn_m": round1(f.load_kn_m),
                "secao_cm": f.height_m.map(|h| section_cm(f.width_m, h)),
                "largura_cm": round1(f.width_m * 100.0),
                "momento_knm": round1(f.moment_knm),
                "armadura_cm2": (f.steel_cm2 * 100.0).round() / 100.0,
                "barras": f.bars,
                "pressao_solo_kpa": round1(f.pressure_kpa),
                "largura_necessaria_cm": round1(f.needed_width_m * 100.0),
            })).collect::<Vec<_>>(),
            "vigas_alavanca": report.balance_beams.iter().map(|b| json!({
                "id": b.id,
                "rotulo": b.label,
                "pilar_divisa": b.column,
                "pilar_interno": b.anchor,
                "vao_m": round1(b.span_m * 100.0) / 100.0,
                "excentricidade_cm": round1(b.eccentricity_m * 100.0),
                "carga_pilar_kn": round1(b.column_load_kn),
                "reacao_sapata_kn": round1(b.reaction_kn),
                "alivio_kn": round1(b.relief_kn),
                "levanta_apoio": b.uplift(),
                "momento_knm": round1(b.moment_knm),
                "cortante_kn": round1(b.shear_kn),
                "secao_cm": b.height_m.map(|h| section_cm(b.width_m, h)),
                "armadura_superior_cm2": (b.top_steel_cm2 * 100.0).round() / 100.0,
                "barras": b.bars,
            })).collect::<Vec<_>>(),
            "pilares_desalinhados": misaligned,
            "cargas_concentradas": concentrated_loads,
            "armaduras": {
                "carater": "estimativa preliminar para revisão; não é detalhamento executivo",
                "vigas": reinforcement_beams,
                "pilares": reinforcement_columns,
                "lajes": reinforcement_slabs,
                "sapatas": reinforcement_footings,
                "tabela_aco": steel_schedule,
                "massa_total_kg": round1(reinforcement.total_mass_kg()),
                "avisos": reinforcement.warnings,
            },
            "semaforo": verdicts_json(&crate::domain::verdict::structure_verdicts(
                &self.project,
                &report,
            )),
            "limites": {
                "recusado": crate::domain::limits::structural_refusal(&self.project),
                "elementos_fora": crate::domain::limits::element_violations(&report)
                    .iter()
                    .map(|v| json!({ "elemento": v.element, "motivo": v.message }))
                    .collect::<Vec<_>>(),
            },
            "premissas": crate::domain::verdict::assumptions(&self.project),
            "alertas": report.warnings,
        })))
    }
}

#[cfg(test)]
mod tests {
    use crate::app::FloorPlanApp;
    use eframe::egui;
    use serde_json::json;

    #[test]
    fn an_assistant_lays_out_and_checks_the_structure() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let built = app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [
                { "ferramenta": "definir_terreno", "argumentos": { "largura": 15, "comprimento": 20 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 1, "y": 1, "largura": 4, "comprimento": 4 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Quarto", "x": 5, "y": 1, "largura": 3, "comprimento": 4 } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "gerar_estrutura" } },
            ] }),
        );
        assert!(!built.is_error, "{}", built.text);
        let report = app
            .run_mcp_tool("pre_dimensionamento", &json!({}))
            .structured
            .unwrap();
        assert_eq!(report["pilares"].as_array().unwrap().len(), 6);
        assert_eq!(report["lajes"].as_array().unwrap().len(), 2);
        assert!(!report["armaduras"]["vigas"].as_array().unwrap().is_empty());
        assert!(
            !report["armaduras"]["tabela_aco"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(report["armaduras"]["massa_total_kg"].as_f64().unwrap() > 0.0);
        // Every element gets its traffic light (#101).
        let lights = report["semaforo"].as_array().unwrap();
        assert!(!lights.is_empty());
        assert!(
            lights
                .iter()
                .all(|v| v["verificacoes"].as_array().is_some_and(|c| !c.is_empty()))
        );
        assert!(report["alertas"].as_array().unwrap().is_empty(), "{report}");
        let beam = report["vigas"][0]["vaos"][0]["id"].as_u64().unwrap();
        assert!(
            report["vigas"][0]["vaos"][0]["momento_vao_knm"]
                .as_f64()
                .unwrap()
                > 0.0
        );

        let load = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({
                "acao": "inserir_carga",
                "tipo": "caixa_agua",
                "x": 3.0,
                "y": 3.0,
                "carga_kn": 12.0
            }),
        );
        assert!(!load.is_error, "{}", load.text);
        let load_id = load.structured.unwrap()["id"].as_u64().unwrap();
        let with_load = app
            .run_mcp_tool("pre_dimensionamento", &json!({}))
            .structured
            .unwrap();
        assert_eq!(
            with_load["cargas_concentradas"].as_array().unwrap().len(),
            1
        );
        assert_eq!(with_load["cargas_concentradas"][0]["carga_kn"], 12.0);
        let edited_load = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "editar_carga", "id": load_id, "carga_kn": 13.5 }),
        );
        assert!(!edited_load.is_error, "{}", edited_load.text);

        // A beam too shallow is flagged; a solid slab is set by room name.
        let edited = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "editar_viga", "id": beam, "altura_cm": 20 }),
        );
        assert!(!edited.is_error, "{}", edited.text);
        let slab = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "definir_laje", "comodo": "sala", "tipo": "macica" }),
        );
        assert!(!slab.is_error, "{}", slab.text);
        let report = app
            .run_mcp_tool("pre_dimensionamento", &json!({}))
            .structured
            .unwrap();
        assert!(
            report["alertas"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w.as_str().unwrap().contains("pede altura"))
        );
        // A column snaps onto the nearest junction.
        let column = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "inserir_pilar", "x": 1.1, "y": 9.1, "largura_cm": 20, "profundidade_cm": 20 }),
        );
        assert!(!column.is_error, "{}", column.text);
        assert!(
            app.run_mcp_tool(
                "projeto_estrutural",
                &json!({ "acao": "configurar", "fck_mpa": 99 })
            )
            .is_error
        );
        let view = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "vista", "disciplina": "estrutura" }),
        );
        assert!(!view.is_error, "{}", view.text);
    }

    #[test]
    fn an_assistant_roofs_the_gourmet_area_and_the_columns_take_it() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let built = app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [
                { "ferramenta": "definir_terreno", "argumentos": { "largura": 10, "comprimento": 50 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Gourmet", "x": 0, "y": 25.75, "largura": 5, "comprimento": 14.75 } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "gerar_estrutura" } },
            ] }),
        );
        assert!(!built.is_error, "{}", built.text);
        let before = app.project.structure_report();
        let load_before: f64 = before.columns.iter().map(|c| c.load_kn).sum();
        let roofed = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({
                "acao": "definir_telhado",
                "x": 0, "y": 25.75, "largura_m": 5, "comprimento_m": 14.75,
                "lado_alto": "oeste", "altura_alta_m": 4.3, "altura_baixa_m": 2.6,
                "telha": "ceramica", "madeira": "c30",
            }),
        );
        assert!(!roofed.is_error, "{}", roofed.text);
        let data = roofed.structured.unwrap();
        assert_eq!(data["pecas"].as_array().unwrap().len(), 4, "{data}");
        assert!((data["inclinacao_pct"].as_f64().unwrap() - 34.0).abs() < 0.01);
        // The west wall now stands 4.30 m tall.
        let west = app
            .project
            .walls
            .iter()
            .find(|w| w.start.x.abs() < 0.01 && w.end.x.abs() < 0.01)
            .unwrap();
        assert_eq!(west.heights(app.project.wall_height), (4.3, 4.3));
        // The roof weight reaches the columns.
        let after = app.project.structure_report();
        let load_after: f64 = after.columns.iter().map(|c| c.load_kn).sum();
        let carried: f64 = after.roofs[0]
            .reactions
            .iter()
            .filter(|r| r.support.as_deref().is_some_and(|s| s.starts_with('P')))
            .map(|r| r.load_kn)
            .sum();
        assert!(carried > 0.0);
        assert!(
            load_after > load_before + carried * 0.99,
            "{load_before} → {load_after}"
        );
        let report = app
            .run_mcp_tool("pre_dimensionamento", &json!({}))
            .structured
            .unwrap();
        assert_eq!(report["telhados"].as_array().unwrap().len(), 1);
        let id = data["id"].as_u64().unwrap();
        let edited = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "definir_telhado", "id": id, "telha": "metalica", "ajustar_paredes": false }),
        );
        assert!(!edited.is_error, "{}", edited.text);
        assert_eq!(
            edited.structured.unwrap()["pecas"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let removed = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "remover_telhado", "id": id }),
        );
        assert!(!removed.is_error, "{}", removed.text);
        assert!(app.project.structure.roofs.is_empty());
    }

    #[test]
    fn timber_posts_carry_the_low_edge_of_a_roof() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let built = app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [
                { "ferramenta": "definir_terreno", "argumentos": { "largura": 10, "comprimento": 50 } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "inserir_mourao", "x": 4.92, "y": 25.83, "diametro_cm": 15 } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "inserir_mourao", "x": 4.92, "y": 33.1, "diametro_cm": 15, "madeira": "c40" } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "inserir_mourao", "x": 4.92, "y": 40.42, "diametro_cm": 15 } },
                { "ferramenta": "projeto_estrutural", "argumentos": {
                    "acao": "definir_telhado", "x": 0, "y": 25.75, "largura_m": 5, "comprimento_m": 14.75,
                    "lado_alto": "oeste", "altura_alta_m": 4.3, "altura_baixa_m": 2.6,
                } },
            ] }),
        );
        assert!(!built.is_error, "{}", built.text);
        assert_eq!(
            app.project.structure.post_timber,
            crate::domain::roof::TimberClass::C40
        );
        let report = app
            .run_mcp_tool("pre_dimensionamento", &json!({}))
            .structured
            .unwrap();
        let posts = report["mouroes"].as_array().unwrap();
        assert_eq!(posts.len(), 3, "{report}");
        assert!(posts.iter().all(|p| p["carga_kn"].as_f64().unwrap() > 0.0));
        // Two timber beams, post to post, carry the beam ends in between.
        assert_eq!(report["vigas_madeira"].as_array().unwrap().len(), 2);
        let low_loose = report["telhados"][0]["apoios"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["lado"] == "baixo" && a["apoio"].is_null())
            .count();
        assert_eq!(low_loose, 0, "{report}");
        let bad = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "inserir_mourao", "x": 2, "y": 2, "diametro_cm": 60 }),
        );
        assert!(bad.is_error);
    }

    #[test]
    fn a_soil_survey_sets_the_soil_and_compares_the_foundations() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let built = app.run_mcp_tool(
            "executar_lote",
            &json!({ "acoes": [
                { "ferramenta": "definir_terreno", "argumentos": { "largura": 15, "comprimento": 20 } },
                { "ferramenta": "criar_comodo", "argumentos": { "nome": "Sala", "x": 1, "y": 1, "largura": 4, "comprimento": 4 } },
                { "ferramenta": "projeto_estrutural", "argumentos": { "acao": "gerar_estrutura" } },
            ] }),
        );
        assert!(!built.is_error, "{}", built.text);
        let defined = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({
                "acao": "definir_sondagem",
                "nome": "SP-01",
                "nivel_agua_m": 5.2,
                "leituras": [
                    { "profundidade_m": 1, "n": 6, "solo": "argila" },
                    { "profundidade_m": 2, "n": 7, "solo": "argila" },
                    { "profundidade_m": 3, "n": 8, "solo": "silte_argiloso" },
                    { "profundidade_m": 4, "n": 12, "solo": "silte_arenoso" },
                    { "profundidade_m": 5, "n": 15, "solo": "areia" },
                    { "profundidade_m": 6, "n": 18, "solo": "areia" },
                ],
            }),
        );
        assert!(!defined.is_error, "{}", defined.text);
        // N̄ from 1 to 3 m = 7 → 140 kPa.
        assert!((app.project.structure.settings.soil_kpa - 140.0).abs() < 1e-9);
        let study = app
            .run_mcp_tool(
                "projeto_estrutural",
                &json!({ "acao": "simular_fundacoes" }),
            )
            .structured
            .unwrap();
        let options = study["opcoes"].as_array().unwrap();
        assert_eq!(options.len(), 3);
        assert!(study["recomendada"].is_string(), "{study}");
        // Brocas stop above the water table at 5.2 m.
        assert!(study["broca"]["comprimento_m"].as_f64().unwrap() <= 5.0);
        let bad = app.run_mcp_tool(
            "projeto_estrutural",
            &json!({ "acao": "definir_sondagem", "leituras": [{ "profundidade_m": 1, "n": 5, "solo": "turfa" }] }),
        );
        assert!(bad.is_error);
        let removed =
            app.run_mcp_tool("projeto_estrutural", &json!({ "acao": "remover_sondagem" }));
        assert!(!removed.is_error, "{}", removed.text);
        assert!(app.project.structure.soil_survey.is_none());
    }
}
