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
                    Ok(Change::new(format!(
                        "Estrutura gerada: {} pilar(es), {} viga(s), {} laje(s), {} sapata(s).",
                        report.columns, report.beams, report.slabs, report.footings
                    ))
                    .data(json!({
                        "pilares": report.columns,
                        "vigas": report.beams,
                        "lajes": report.slabs,
                        "sapatas": report.footings,
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
                    let (center, turns, angle_rad) = project.structural_placement_with_angle(point);
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
            _ => Err(
                "Informe 'acao': gerar_estrutura, inserir_pilar, inserir_sapata, ligar_pilares, apoiar_em_viga, editar_viga, remover_viga, definir_laje, remover_laje, configurar, ajustar_sapatas ou vista.".to_owned(),
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
            "pilares_desalinhados": misaligned,
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
}
