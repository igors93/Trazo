//! Verdicts for what goes into the ground besides the house: the presumed
//! soil profile (no survey yet) and in-ground swimming pools.

use crate::domain::fixtures::FixtureKind;
use crate::domain::model::Project;
use crate::domain::pool::{DURABLE_FCK_MPA, PoolResult};
use crate::domain::wall_foundations::WallFoundationKind;

use super::{Check, Discipline, Light, Verdict, cm, decimal};

/// The presumed soil and every pool, as structure verdicts.
pub fn site_works_verdicts(project: &Project) -> Vec<Verdict> {
    let mut verdicts: Vec<Verdict> = presumed_soil_verdict(project).into_iter().collect();
    verdicts.extend(project.pool_report().iter().map(pool_verdict));
    verdicts
}

fn presumed_soil_verdict(project: &Project) -> Option<Verdict> {
    let presumed = project.structure.presumed_soil.as_ref()?;
    if project.structure.soil_survey.is_some() {
        return None;
    }
    let analysis = presumed.analysis();
    let mut checks = vec![Check {
        name: "Sondagem".to_owned(),
        utilisation: 1.0,
        light: Light::Yellow,
        simple: format!(
            "O solo foi estimado pelo perfil provável, sem sondagem: os cálculos usam {} kPa, o lado seguro. Faça uma sondagem SPT antes da obra; ela pode mudar o tipo de fundação.",
            decimal(project.structure.settings.soil_kpa, 0)
        ),
        detail: analysis.notes.join(" "),
    }];
    if analysis.collapsible {
        let footings = project
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
            .count();
        let strips = project
            .structure
            .wall_foundations
            .iter()
            .filter(|f| f.kind == WallFoundationKind::StripFooting)
            .count();
        let shallow = footings + strips;
        checks.push(Check {
            name: "Solo colapsível".to_owned(),
            utilisation: if shallow > 0 { 1.0 } else { 0.5 },
            light: if shallow > 0 {
                Light::Yellow
            } else {
                Light::Green
            },
            simple: if shallow > 0 {
                format!(
                    "Solo vermelho poroso pode afundar quando encharca, e há {shallow} fundação(ões) rasa(s) (sapatas ou sapatas corridas): prefira brocas que atravessem a camada porosa, e mantenha a água longe das fundações (calçada em volta, calhas, nada de sumidouro ou vazamento perto)."
                )
            } else {
                "Solo vermelho poroso pode afundar quando encharca: as fundações em broca passam da camada mais porosa (confirme a profundidade na sondagem). Mesmo assim, mantenha a água longe das fundações.".to_owned()
            },
            detail: "Solos lateríticos porosos (argila arenosa e areia argilosa vermelhas) podem ser colapsíveis: perdem a estrutura quando a umidade sobe. Confirme com sondagem e ensaio de colapso (edométrico).".to_owned(),
        });
    }
    Some(Verdict {
        discipline: Discipline::Structure,
        element: "Solo provável (sem sondagem)".to_owned(),
        checks,
    })
}

fn pool_verdict(pool: &PoolResult) -> Verdict {
    let wall = &pool.wall;
    let mut checks = vec![Check::new(
        "Parede · empuxo",
        wall.utilisation,
        if wall.utilisation > 1.0 + 1e-9 {
            format!(
                "A parede de {} não aguenta a água e a terra com {} m de profundidade: faça-a com {}.",
                cm(wall.thickness_m),
                decimal(wall.height_m, 2),
                cm(wall.needed_thickness_m)
            )
        } else {
            format!(
                "Parede de concreto de {} com {} em cada face (vertical) aguenta a piscina cheia e a terra com ela vazia.",
                cm(wall.thickness_m),
                wall.bars
            )
        },
        format!(
            "Balanço engastado no fundo, H = {} m. Água: M = γw·H³/6 = {} kN·m/m. Terra em repouso K0 = 1 − sen φ' = {} (φ' = {}°, γ = {} kN/m³) com sobrecarga de {} kN/m²: M = {} kN·m/m. Md = 1,4·{} = {} kN·m/m; μ/0,295 = {}; Vd = {} kN/m ≤ VRd1 = {} kN/m (NBR 6118 §19.4.1). Aço por face: ELU {} cm²/m, serviço σs ≤ 160 MPa (estanqueidade) {} cm²/m, mínimo 0,15 % {} cm²/m → {}.",
            decimal(wall.height_m, 2),
            decimal(wall.moment_water_knm_m, 2),
            decimal(pool.k0, 2),
            decimal(pool.friction_deg, 0),
            decimal(pool.unit_weight_kn_m3, 1),
            decimal(pool.surcharge_kpa, 1),
            decimal(wall.moment_soil_knm_m, 2),
            decimal(wall.moment_water_knm_m.max(wall.moment_soil_knm_m), 2),
            decimal(wall.md_knm_m, 2),
            decimal(wall.bending_ratio, 2),
            decimal(wall.vd_kn_m, 1),
            decimal(wall.vrd1_kn_m, 1),
            decimal(wall.steel_uls_cm2_m, 2),
            decimal(wall.steel_service_cm2_m, 2),
            decimal(wall.steel_min_cm2_m, 2),
            wall.bars
        ),
    )];
    checks.push(Check::new(
        "Fundo · pressão no solo",
        pool.floor_ratio,
        format!(
            "O fundo de {} cm com {} nas duas faces leva {} kPa ao solo, que aguenta {} kPa.",
            decimal(pool.spec.floor_thickness_m * 100.0, 0),
            pool.floor_bars,
            decimal(pool.floor_pressure_kpa, 1),
            decimal(pool.allowable_kpa, 0)
        ),
        format!(
            "Água + laje de fundo + paredes espalhadas na base: {} kPa ≤ σadm {} kPa. Armadura mínima 0,15 % em cada face; lastro de {} cm.",
            decimal(pool.floor_pressure_kpa, 1),
            decimal(pool.allowable_kpa, 0),
            5
        ),
    ));
    match (&pool.flotation, pool.groundwater_unknown) {
        (Some(f), _) => checks.push(Check::new(
            "Flutuação (vazia)",
            f.ratio,
            if f.ratio > 1.0 + 1e-9 {
                "Com água no solo, a piscina vazia flutua: engrosse o fundo, prolongue o fundo para fora das paredes (aba com terra em cima) ou instale dreno com válvula de alívio.".to_owned()
            } else {
                "Com água no solo, o peso da piscina vazia segura o empuxo.".to_owned()
            },
            format!(
                "Empuxo da água U = 10·{}·área = {} kN; peso próprio G = {} kN; 1,1·U / 0,9·G = {}.",
                decimal(f.water_depth_m, 2),
                decimal(f.uplift_kn, 0),
                decimal(f.weight_kn, 0),
                decimal(f.ratio, 2)
            ),
        )),
        (None, true) => checks.push(Check {
            name: "Flutuação (vazia)".to_owned(),
            utilisation: 1.0,
            light: Light::Yellow,
            simple: "Ninguém respondeu se brota água do chão: com o lençol alto, a piscina vazia pode subir. Pergunte à família (projeto_estrutural → riscos_terreno) ou veja na sondagem.".to_owned(),
            detail: "A flutuação é conferida com o nível d'água da sondagem ou, se a família disser que brota água, com o nível no terreno.".to_owned(),
        }),
        (None, false) => {}
    }
    if !pool.foundations_in_zone.is_empty() {
        let list = pool
            .foundations_in_zone
            .iter()
            .map(|(label, clearance, needed)| {
                format!(
                    "{label} a {} m (pede {} m)",
                    decimal(*clearance, 2),
                    decimal(*needed, 2)
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        checks.push(Check {
            name: "Fundações vizinhas".to_owned(),
            utilisation: 1.0,
            light: Light::Yellow,
            simple: format!(
                "Fundações rasas perto da escavação da piscina ({list}): construa a piscina primeiro, reaterre compactando, e faça essas fundações em broca com a ponta abaixo do fundo da piscina."
            ),
            detail: format!(
                "NBR 6122, fundações em cotas diferentes: a reta entre os bordos não pode passar dentro do solo que a escavação de {} m tira; em solo pouco resistente ela faz 60° com a vertical. A parede da piscina foi calculada com mais 10 kN/m² de sobrecarga.",
                decimal(pool.excavation_depth_m, 2)
            ),
        });
    }
    if pool.boundary_in_zone()
        && let Some(clearance) = pool.boundary_clearance_m
    {
        checks.push(Check {
            name: "Divisa".to_owned(),
            utilisation: 1.0,
            light: Light::Yellow,
            simple: format!(
                "A escavação de {} m fica a {} m da divisa: escore a vala enquanto estiver aberta, avise o vizinho e fotografe as paredes dele antes. A parede da piscina já foi calculada com a sobrecarga de uma fundação vizinha.",
                decimal(pool.excavation_depth_m, 2),
                decimal(clearance.max(0.0), 2)
            ),
            detail: format!(
                "Zona de influência até {} m da escavação (NBR 6122, cotas diferentes).",
                decimal(pool.boundary_needed_m, 2)
            ),
        });
    }
    if pool.collapsible {
        checks.push(Check {
            name: "Vazamento em solo colapsível".to_owned(),
            utilisation: 1.0,
            light: Light::Yellow,
            simple: "O solo provável é colapsível: um vazamento da piscina encharca o solo e afunda o que está perto. Impermeabilize, faça o teste de 72 h, ponha um dreno testemunha sob o fundo e troque a base por solo-cimento compactado.".to_owned(),
            detail: "Solos porosos lateríticos perdem a estrutura quando a umidade sobe (colapso).".to_owned(),
        });
    }
    if pool.fck_mpa + 1e-9 < DURABLE_FCK_MPA {
        checks.push(Check {
            name: "Concreto (durabilidade)".to_owned(),
            utilisation: 1.0,
            light: Light::Yellow,
            simple: format!(
                "Piscina pede concreto C{} ou mais, com fator água/cimento baixo: compre concreto usinado com nota fiscal. O cálculo usou fck {}.",
                decimal(DURABLE_FCK_MPA, 0),
                decimal(pool.fck_mpa, 0)
            ),
            detail: "NBR 6118 Tabela 7.1: classe de agressividade II (urbana, em contato com água) pede C25 e a/c ≤ 0,60; cobrimento de 3 cm nas faces molhadas.".to_owned(),
        });
    }
    Verdict {
        discipline: Discipline::Structure,
        element: format!("Piscina {} ({})", pool.label, pool.room),
        checks,
    }
}
