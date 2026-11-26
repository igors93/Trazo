//! Assistant tools for the structure: laying it out, placing columns and
//! footings, joining columns with beams, setting slabs and materials, and
//! reading the pre-sizing with the forces in every beam.

mod args;
mod presizing;
mod site_works;
#[cfg(test)]
mod tests;
mod to_json;

use serde_json::{Value, json};

use args::{centimetres, parse_occupancy_class, parse_soil_class, roof_arg, room_arg, survey_arg};
pub(super) use to_json::verdicts_json;
use to_json::{
    beam_ultimate_envelope_json, cantilevers_json, foundation_study_json, grid_analysis_json,
    hypotheses_json, loads_analysis_json, roof_json, round1, wind_json,
};

use super::{Change, ToolOutput, element_id, number, required_number, text, to_model, xy};
use crate::app::{FloorPlanApp, Selection};
use crate::domain::discipline::Discipline;
use crate::domain::fixtures::FixtureKind;
use crate::domain::structure::{BeamSupport, SlabKind, section_cm};

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
            "inserir_bloco_broca" => {
                let kind = FixtureKind::PileCap;
                let point = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let (default_w, default_d) = kind.default_size();
                let width = centimetres(args, "largura_cm")?.unwrap_or(default_w);
                let depth = centimetres(args, "profundidade_cm")?.unwrap_or(default_d);
                let mut spec = crate::domain::fixtures::PileCapSpec::default();
                if let Some(diam) = centimetres(args, "diametro_broca_cm")? {
                    spec.diameter_m = diam;
                }
                if let Some(len) = number(args, "comprimento_broca_m")? {
                    spec.depth_m = len;
                }
                if let Some(cnt) = args.get("quantidade_brocas").and_then(|v| v.as_u64()) {
                    spec.count = cnt.clamp(1, 4) as u32;
                }
                if let Some(anchor) = number(args, "arranque_m")? {
                    spec.anchor_depth_m = anchor;
                }
                if !spec.valid() {
                    return Err(
                        "Parâmetros inválidos da broca: diâmetro 15–80 cm, profundidade 2–20 m, 1–4 brocas e arranque até a profundidade."
                            .to_owned(),
                    );
                }
                self.apply_change(None, |project| {
                    let (center, turns, angle_rad) =
                        project.structural_placement_fitted(kind, point, width, depth);
                    let id = project
                        .add_fixture(kind, center, width, depth, turns)
                        .ok_or_else(|| {
                            format!(
                                "Não foi possível inserir o bloco sobre broca: confira o tamanho ({:.0} cm a {:.0} m) e se o ponto está no terreno.",
                                kind.min_size() * 100.0,
                                crate::domain::fixtures::MAX_SIZE
                            )
                        })?;
                    project.set_pile_cap_spec(id, spec)?;
                    if angle_rad.abs() > 1e-9
                        && let Some(f) = project.fixtures.iter_mut().find(|f| f.id == id)
                    {
                        f.angle_rad = angle_rad;
                    }
                    project.ensure_default_layers();
                    let label = project.structure_labels().get(&id).cloned();
                    Ok(Change::new(format!(
                        "Bloco sobre broca {} inserido (id {id}, {}× Ø{:.0} cm × {:.1} m, arranque {:.2} m).",
                        label.clone().unwrap_or_default(),
                        spec.count,
                        spec.diameter_m * 100.0,
                        spec.depth_m,
                        spec.anchor_depth_m
                    ))
                    .select(Selection::Fixture(id))
                    .data(json!({
                        "id": id,
                        "rotulo": label,
                        "centro": xy(center),
                        "secao_cm": section_cm(width, depth),
                        "diametro_broca_cm": round1(spec.diameter_m * 100.0),
                        "comprimento_broca_m": round1(spec.depth_m),
                        "quantidade_brocas": spec.count,
                        "arranque_m": (spec.anchor_depth_m * 100.0).round() / 100.0,
                    })))
                })
            }
            "editar_bloco_broca" => {
                let id = element_id(args, "id")?;
                let current = self
                    .project
                    .fixtures
                    .iter()
                    .find(|f| f.id == id && f.kind == FixtureKind::PileCap)
                    .copied()
                    .ok_or_else(|| format!("Bloco sobre broca {id} não encontrado."))?;
                let mut spec = current.pile.unwrap_or_default();
                if let Some(diam) = centimetres(args, "diametro_broca_cm")? {
                    spec.diameter_m = diam;
                }
                if let Some(len) = number(args, "comprimento_broca_m")? {
                    spec.depth_m = len;
                }
                if let Some(cnt) = args.get("quantidade_brocas").and_then(|v| v.as_u64()) {
                    spec.count = cnt.clamp(1, 4) as u32;
                }
                if let Some(anchor) = number(args, "arranque_m")? {
                    spec.anchor_depth_m = anchor;
                }
                let new_w = centimetres(args, "largura_cm")?;
                let new_d = centimetres(args, "profundidade_cm")?;
                self.apply_change(None, |project| {
                    project.set_pile_cap_spec(id, spec)?;
                    if let (Some(w), Some(d)) = (
                        new_w.or(Some(current.width)),
                        new_d.or(Some(current.depth)),
                    ) && (new_w.is_some() || new_d.is_some())
                    {
                        project.update_fixture(id, None, Some((w, d)), None)?;
                    }
                    Ok(Change::new(format!(
                        "Bloco sobre broca {id} atualizado ({}× Ø{:.0} cm × {:.1} m, arranque {:.2} m).",
                        spec.count,
                        spec.diameter_m * 100.0,
                        spec.depth_m,
                        spec.anchor_depth_m
                    ))
                    .data(json!({
                        "id": id,
                        "diametro_broca_cm": round1(spec.diameter_m * 100.0),
                        "comprimento_broca_m": round1(spec.depth_m),
                        "quantidade_brocas": spec.count,
                        "arranque_m": (spec.anchor_depth_m * 100.0).round() / 100.0,
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
            "etapas" => {
                use crate::domain::protection::stages::{
                    extension_beyond_built, mark_foundation_built, plan_future_floors,
                };
                let future = match number(args, "andares_futuros")? {
                    Some(v) if v.fract() != 0.0 || !(0.0..=2.0).contains(&v) => {
                        return Err("'andares_futuros' é um inteiro de 0 a 2.".to_owned());
                    }
                    Some(v) => Some(v as u32),
                    None => None,
                };
                let built = args.get("fundacao_executada").and_then(Value::as_bool);
                let summary = |project: &crate::domain::model::Project| {
                    json!({
                        "andares_futuros": project.protection.stages.future_floors,
                        "pavimentos_carregados": project.structure.settings.floors,
                        "fundacao_feita_para": project.protection.stages.built_for,
                        "ampliacao_alem_da_fundacao": extension_beyond_built(project)
                            .map(|(built, now)| json!({ "feita_para": built, "agora": now })),
                    })
                };
                if future.is_none() && built.is_none() {
                    return Ok(ToolOutput::ok("Construção em etapas.")
                        .with_structured(summary(&self.project)));
                }
                self.apply_change(Some("As etapas já eram essas."), |project| {
                    if let Some(future) = future {
                        plan_future_floors(project, future)?;
                    }
                    if let Some(built) = built {
                        mark_foundation_built(project, built);
                    }
                    let message = match extension_beyond_built(project) {
                        Some((built, now)) => format!(
                            "Atenção: a fundação foi feita para {built} pavimento(s) e o projeto agora tem {now}. Não suba mais andares sem um profissional. {}",
                            crate::domain::protection::help::HELP_SENTENCE
                        ),
                        None => format!(
                            "Etapas: os pilares carregam {} pavimento(s), contando {} futuro(s). Ajuste as sapatas às novas cargas.",
                            project.structure.settings.floors,
                            project.protection.stages.future_floors
                        ),
                    };
                    Ok(Change::new(message).data(summary(project)))
                })
            }
            "riscos_terreno" => {
                use crate::domain::protection::hypotheses::HypothesisSource;
                use crate::domain::protection::site_risks::SiteRisk;
                let mut answers = Vec::new();
                for risk in SiteRisk::ALL {
                    match args.get(risk.key()) {
                        Some(Value::Bool(yes)) => answers.push((risk, Some(*yes))),
                        Some(Value::Null) => answers.push((risk, None)),
                        Some(_) => {
                            return Err(format!(
                                "'{}' deve ser true (sim), false (não) ou null (não sei).",
                                risk.key()
                            ));
                        }
                        None => {}
                    }
                }
                let questions = |project: &crate::domain::model::Project| {
                    let risks = &project.protection.site_risks;
                    SiteRisk::ALL
                        .iter()
                        .map(|&risk| {
                            json!({
                                "chave": risk.key(),
                                "pergunta": risk.question(),
                                "resposta": risks.answer(risk).map(|a| a.yes),
                                "respondido_por": risks.answer(risk).map(|a| a.by.key()),
                                "vale": risks.effective(risk),
                                "risco": risk.danger(),
                            })
                        })
                        .collect::<Vec<_>>()
                };
                if answers.is_empty() {
                    return Ok(ToolOutput::ok(
                        "Perguntas sobre o terreno: faça cada uma à pessoa e registre só o que ela responder. Um \"não\" dado pela IA não vale até a pessoa confirmar na janela Segurança.",
                    )
                    .with_structured(json!({ "perguntas": questions(&self.project) })));
                }
                self.apply_change(Some("As respostas já eram essas."), |project| {
                    for (risk, yes) in &answers {
                        project
                            .protection
                            .site_risks
                            .set(*risk, *yes, HypothesisSource::Assistant);
                    }
                    let risky = project.protection.site_risks.risky();
                    let message = if risky.is_empty() {
                        "Respostas registradas. Um \"não\" dado pela IA só vale depois que a pessoa confirmar na janela Segurança.".to_owned()
                    } else {
                        format!(
                            "Há risco no terreno: {} As fundações ficam vermelhas até um profissional avaliar. {}",
                            risky.iter().map(|r| r.danger()).collect::<Vec<_>>().join(" "),
                            crate::domain::protection::help::HELP_SENTENCE
                        )
                    };
                    Ok(Change::new(message).data(json!({ "perguntas": questions(project) })))
                })
            }
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
                    settings.apply_occupancy(occ);
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
                if let Some(v) = number(args, "peso_alvenaria_kn_m3")? {
                    if !(8.0..=25.0).contains(&v) {
                        return Err("'peso_alvenaria_kn_m3' deve ficar entre 8 e 25.".to_owned());
                    }
                    settings.masonry_kn_m3 = v;
                }
                if let Some(v) = args.get("sobrecarga_por_comodo").and_then(Value::as_bool) {
                    settings.per_room_live_loads = v;
                }
                if let Some(v) = args.get("portico_nos_rigidos").and_then(Value::as_bool) {
                    settings.frame_rigid_joints = v;
                }
                if let Some(v) = args.get("portico_bases_engastadas").and_then(Value::as_bool) {
                    settings.frame_fixed_bases = v;
                }
                // The standard's minimum live load cannot be lowered (#169).
                let minimum = settings.occupancy.live_kn_m2();
                if settings.live_kn_m2 + 1e-9 < minimum {
                    return Err(format!(
                        "A sobrecarga não pode ficar abaixo do mínimo da NBR 6120 para o uso {} ({} kN/m²).",
                        settings.occupancy.label(),
                        crate::domain::protection::decimal(minimum, 1)
                    ));
                }
                let cited = text(args, "evidencia");
                self.apply_change(Some("Nada mudou nos materiais e cargas."), |project| {
                    let previous = project.clone();
                    project.structure.settings = settings;
                    if let Some(cited) = &cited {
                        crate::domain::protection::hypotheses::cite_evidence(
                            &previous, project, cited,
                        );
                    }
                    Ok(Change::new("Materiais e cargas atualizados.".to_owned())
                        .data(json!({ "hipoteses": hypotheses_json(project) })))
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
            "definir_solo_provavel" => {
                let profile = site_works::presumed_soil_arg(args)?;
                self.apply_change(None, |project| {
                    let layers = profile.layers.len();
                    let applied = project.set_presumed_soil(Some(profile));
                    let kpa = project.structure.settings.soil_kpa;
                    let message = match applied {
                        Some(kpa) => format!(
                            "Solo provável definido ({layers} camada(s)). Sem sondagem, a tensão do solo baixou para {kpa:.0} kPa, o lado seguro para esse solo."
                        ),
                        None if project.structure.soil_survey.is_some() => format!(
                            "Solo provável guardado ({layers} camada(s)); a sondagem prevalece ({kpa:.0} kPa)."
                        ),
                        None => format!(
                            "Solo provável guardado ({layers} camada(s)); a tensão do solo continua em {kpa:.0} kPa."
                        ),
                    };
                    Ok(Change::new(message).data(json!({
                        "solo_provavel": site_works::presumed_soil_json(project),
                        "hipoteses": hypotheses_json(project),
                    })))
                })
            }
            "remover_solo_provavel" => self.apply_change(None, |project| {
                if project.structure.presumed_soil.is_none() {
                    return Err("O projeto não tem solo provável.".to_owned());
                }
                let _ = project.set_presumed_soil(None);
                Ok(Change::new(
                    "Solo provável removido; a tensão do solo ficou como estava.".to_owned(),
                ))
            }),
            "configurar_piscina" => {
                let room = room_arg(&self.project, args)?;
                let mut spec = self
                    .project
                    .structure
                    .pools
                    .iter()
                    .find(|p| p.room_id == room)
                    .copied()
                    .unwrap_or_else(|| crate::domain::pool::PoolSpec::new(room));
                if let Some(depth) = number(args, "profundidade_m")? {
                    spec.depth_m = depth;
                }
                if let Some(wall) = centimetres(args, "espessura_parede_cm")? {
                    spec.wall_thickness_m = wall;
                }
                if let Some(floor) = centimetres(args, "espessura_fundo_cm")? {
                    spec.floor_thickness_m = floor;
                }
                if let Some(edge) = number(args, "sobrecarga_borda_kn_m2")? {
                    spec.edge_surcharge_kpa = edge;
                }
                spec.validate()?;
                self.apply_change(Some("A piscina já estava assim."), |project| {
                    project.set_pool_spec(spec)?;
                    let pools = site_works::pools_json(project);
                    let this = pools
                        .iter()
                        .find(|p| p.get("comodo_id").and_then(Value::as_u64) == Some(room))
                        .cloned()
                        .unwrap_or(Value::Null);
                    Ok(Change::new(format!(
                        "Piscina configurada: {} m de profundidade, parede de {:.0} cm e fundo de {:.0} cm.",
                        format!("{:.2}", spec.depth_m).replace('.', ","),
                        spec.wall_thickness_m * 100.0,
                        spec.floor_thickness_m * 100.0
                    ))
                    .data(json!({ "piscina": this })))
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
            "configurar_vento" => {
                let mut wind = self.project.structure.wind;
                if let Some(v) = number(args, "v0_ms")? {
                    wind.v0_ms = v;
                }
                if let Some(v) = number(args, "categoria_terreno")? {
                    if v.fract() != 0.0 {
                        return Err("'categoria_terreno' é um inteiro de 1 a 5.".to_owned());
                    }
                    wind.category = v as u8;
                }
                if let Some(v) = number(args, "s1")? {
                    wind.s1 = v;
                }
                if let Some(v) = number(args, "s3")? {
                    wind.s3 = v;
                }
                // The city's wind from the NBR 6123 map cannot be lowered (#169).
                if let Some(city) = self.project.codes.site.wind_v0_ms
                    && wind.v0_ms + 0.01 < city
                {
                    return Err(format!(
                        "O V0 não pode ficar abaixo do vento da cidade do terreno ({city:.0} m/s, mapa de isopletas da NBR 6123)."
                    ));
                }
                let cited = text(args, "evidencia");
                self.apply_change(Some("O vento já era esse."), |project| {
                    let previous = project.clone();
                    project.set_wind(wind)?;
                    if let Some(cited) = &cited {
                        crate::domain::protection::hypotheses::cite_evidence(
                            &previous, project, cited,
                        );
                    }
                    let report = project.structure_report();
                    Ok(Change::new(format!(
                        "Vento: V0 = {:.0} m/s, categoria {}.",
                        wind.v0_ms, wind.category
                    ))
                    .data(wind_json(&project.structure.wind, &report.wind, &report.masonry)))
                })
            }
            "inserir_mourao" => {
                let x = required_number(args, "x")?;
                let y = required_number(args, "y")?;
                let size = centimetres(args, "diametro_cm")?.unwrap_or(0.15);
                if !(0.10..=0.40).contains(&size) {
                    return Err("'diametro_cm' deve ficar entre 10 e 40.".to_owned());
                }
                let species = match text(args, "madeira") {
                    Some(raw) => Some(
                        crate::domain::roof::TimberSpecies::parse(&raw).ok_or(
                            "'madeira' deve ser c20, c30, c40, c60, pinus_elliottii, eucalipto_citriodora, itauba, cumaru ou jatoba.",
                        )?,
                    ),
                    None => None,
                };
                let treated = args.get("tratada").and_then(Value::as_bool);
                self.apply_change(None, |project| {
                    if let Some(species) = species {
                        project.structure.post_species = species;
                    }
                    if let Some(treated) = treated {
                        project.structure.post_treated = treated;
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
                            .cloned()
                            .ok_or_else(|| format!("Telhado {id} não encontrado."))?,
                    ),
                    None => None,
                };
                let roof = roof_arg(args, existing.clone())?;
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
                    let mut data = roof_json(result);
                    if let Some(brace) = report.wind.bracing.iter().find(|brace| brace.roof_id == id) {
                        data["contraventamento"] = json!({
                            "demanda_kn": brace.demand_kn,
                            "resistencia_informada_kn": brace.capacity_kn,
                            "apoios_ancorados": brace.path_ok,
                            "atende": brace.ok(),
                        });
                    }
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
                    .data(data))
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
            "ajustar_paredes_telhado" => {
                let id = element_id(args, "id")?;
                self.apply_change(
                    Some("As paredes sob esse telhado já estavam na altura certa."),
                    |project| {
                        let walls = project.fit_walls_to_roof(id)?;
                        Ok(Change::new(format!(
                            "{walls} parede(s) ajustada(s) ao plano do telhado {id}."
                        ))
                        .data(json!({ "id": id, "paredes_ajustadas": walls })))
                    },
                )
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
            "configurar_sobreposicao" => {
                use crate::domain::plan_results::BeamDiagramOverlay;
                let mut next = self.project.structure.overlay;
                if let Some(v) = args.get("cores_aproveitamento").and_then(Value::as_bool) {
                    next.utilisation_colors = v;
                }
                if let Some(raw) = text(args, "diagrama_planta") {
                    next.beam_diagrams = BeamDiagramOverlay::from_key(&raw).ok_or(
                        "'diagrama_planta' deve ser nenhum, momento ou cortante.",
                    )?;
                }
                if let Some(v) = args.get("deformada_planta").and_then(Value::as_bool) {
                    next.deformed_shape = v;
                }
                if let Some(v) = args.get("caminho_cargas").and_then(Value::as_bool) {
                    next.load_path = v;
                }
                self.apply_change(None, |project| {
                    project.structure.overlay = next;
                    Ok(Change::new("Sobreposição estrutural na planta atualizada.").data(
                        json!({
                            "cores_aproveitamento": next.utilisation_colors,
                            "diagrama_planta": next.beam_diagrams.key(),
                            "deformada_planta": next.deformed_shape,
                            "caminho_cargas": next.load_path,
                        }),
                    ))
                })
            }
            "simular_whatif" => {
                use crate::domain::whatif::{
                    WhatIfParams, apply_whatif_to_project, evaluate_whatif,
                };
                let mut params = WhatIfParams::from_project(&self.project);
                if let Some(v) = number(args, "escala_vao")? {
                    params.span_scale = v.clamp(0.5, 2.0);
                }
                if let Some(v) = number(args, "largura_viga_cm")? {
                    params.beam_width_cm = v.clamp(12.0, 40.0);
                }
                if let Some(v) = number(args, "altura_viga_cm")? {
                    params.beam_height_cm = v.clamp(20.0, 80.0);
                }
                if let Some(v) = number(args, "lado_pilar_cm")? {
                    params.column_side_cm = v.clamp(14.0, 60.0);
                }
                if let Some(v) = number(args, "espessura_laje_cm")? {
                    params.slab_thickness_cm = v.clamp(8.0, 25.0);
                }
                if let Some(v) = number(args, "sobrecarga_kn_m2")? {
                    params.live_load_kn_m2 = v.clamp(0.5, 6.0);
                }
                if let Some(v) = number(args, "fck_mpa")? {
                    params.fck_mpa = v.clamp(20.0, 50.0);
                }
                if let Some(v) = number(args, "solo_kpa")? {
                    params.soil_kpa = v.clamp(60.0, 500.0);
                }
                if let Some(v) = number(args, "vento_v0_ms")? {
                    params.wind_v0_ms = v.clamp(30.0, 50.0);
                }
                if let Some(raw) = text(args, "ocupacao") {
                    params.occupancy = parse_occupancy_class(&raw).ok_or(
                        "'ocupacao' deve ser residencial, escritorio, garagem ou comercial.",
                    )?;
                }
                let result = evaluate_whatif(&self.project, &params);
                let apply = args
                    .get("aplicar")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let payload = serde_json::to_value(&result)
                    .map_err(|e| format!("Falha ao serializar What-if: {e}"))?;
                if apply {
                    self.apply_change(None, |project| {
                        apply_whatif_to_project(project, &params);
                        Ok(Change::new(result.advice.clone()).data(payload))
                    })
                } else {
                    Ok(ToolOutput::ok(result.advice).with_structured(payload))
                }
            }
            "definir_balanco" => {
                use crate::domain::cantilevers::CantileverSpec;
                let kind = text(args, "tipo")
                    .unwrap_or_else(|| "laje".to_owned())
                    .to_lowercase();
                let span_m = required_number(args, "balanco_m")?;
                let back_span_m = number(args, "contravao_m")?.unwrap_or(0.0);
                let label = text(args, "nome").unwrap_or_default();
                let spec = match kind.as_str() {
                    "laje" | "sacada" | "marquise" => {
                        let width_m = number(args, "largura_m")?.unwrap_or(3.0);
                        let thickness_m = number(args, "espessura_cm")?.unwrap_or(14.0) / 100.0;
                        let tip_load_kn_m = number(args, "carga_ponta_kn_m")?.unwrap_or(1.50);
                        CantileverSpec::Slab {
                            id: 0,
                            label,
                            span_m,
                            width_m,
                            thickness_m,
                            back_span_m,
                            tip_load_kn_m,
                        }
                    }
                    "viga" => {
                        let width_m = number(args, "largura_cm")?.unwrap_or(14.0) / 100.0;
                        let height_m = number(args, "altura_cm")?.unwrap_or(40.0) / 100.0;
                        let distributed_kn_m = number(args, "carga_distribuida_kn_m")?.unwrap_or(8.0);
                        let tip_point_load_kn = number(args, "carga_kn")?.unwrap_or(0.0);
                        CantileverSpec::Beam {
                            id: 0,
                            label,
                            span_m,
                            width_m,
                            height_m,
                            distributed_kn_m,
                            tip_point_load_kn,
                            back_span_m,
                        }
                    }
                    _ => return Err("'tipo' para definir_balanco deve ser laje ou viga.".to_owned()),
                };
                self.apply_change(None, |project| {
                    let id = project.add_cantilever(spec)?;
                    let report = project.analyse_cantilevers();
                    Ok(Change::new(format!("Balanço #{id} registrado e verificado."))
                        .data(json!({
                            "id": id,
                            "escadas_e_balancos": cantilevers_json(&report),
                        })))
                })
            }
            "remover_balanco" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.remove_cantilever(id) {
                        return Err("Balanço não encontrado.".to_owned());
                    }
                    Ok(Change::new(format!("Balanço #{id} removido.")))
                })
            }
            _ => Err(
                "Informe 'acao': gerar_estrutura, inserir_pilar, inserir_sapata, inserir_bloco_broca, editar_bloco_broca, inserir_carga, editar_carga, remover_carga, ligar_pilares, apoiar_em_viga, editar_viga, remover_viga, definir_laje, remover_laje, configurar, configurar_sobreposicao, simular_whatif, definir_balanco, remover_balanco, ajustar_sapatas, gerar_vigas_alavanca, ligar_viga_alavanca, remover_viga_alavanca, gerar_fundacao_paredes, definir_fundacao_parede, remover_fundacao_paredes, definir_sondagem, remover_sondagem, definir_solo_provavel, remover_solo_provavel, configurar_piscina, simular_fundacoes, definir_telhado, remover_telhado, inserir_mourao, configurar_vento, ajustar_paredes_telhado ou vista.".to_owned(),
            ),
        }
    }
}
