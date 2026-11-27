//! Plumbing verdicts (section E): the pressure left at every fixture of
//! each wet room (NBR 5626) and the weight of the full water tank against
//! the load the structure was checked for.
//!
//! A pressure below the standard's least is yellow, not red: the fixture
//! works badly, nothing falls. The tank's weight is red when the structure
//! was checked for less: that one is about the slab.

use crate::domain::model::Project;
use crate::domain::plumbing::cold_water::cold_water;
use crate::domain::plumbing::network::Pressure;

use super::{Check, Discipline, Light, Verdict, decimal};

pub fn plumbing_verdicts(project: &Project) -> Vec<Verdict> {
    let mut verdicts: Vec<Verdict> = septic_verdict(project).into_iter().collect();
    verdicts.extend(hot_water_verdicts(project));
    verdicts.extend(sewage_verdict(project));
    let Some(cold) = cold_water(project) else {
        return verdicts;
    };
    let network = &cold.network;
    for (index, group) in network.groups.iter().enumerate() {
        let checks: Vec<Check> = network
            .outlets
            .iter()
            .filter(|o| o.path.last().is_some_and(|p| network.pipes[*p].group == index))
            .map(|outlet| {
                let target = outlet.target_m(network.min_m);
                let utilisation = if outlet.pressure_m > 0.0 {
                    target / outlet.pressure_m
                } else {
                    f64::INFINITY
                };
                let simple = match outlet.state {
                    Pressure::Good => format!(
                        "A água chega com {} m de pressão.",
                        decimal(outlet.pressure_m, 2)
                    ),
                    Pressure::Weak => format!(
                        "O chuveiro funciona, mas fraco ({} m; bom a partir de {} m): suba a caixa-d'água para {} m do piso antes de fazer a base dela.",
                        decimal(outlet.pressure_m, 2),
                        decimal(target, 1),
                        decimal(cold.comfort_base_m, 2)
                    ),
                    Pressure::Low => format!(
                        "A água chega fraca demais ({} m, abaixo do mínimo da NBR 5626): suba a caixa-d'água para {} m do piso antes de fazer a base dela, ou use um pressurizador.",
                        decimal(outlet.pressure_m.max(0.0), 2),
                        decimal(cold.needed_base_m, 2)
                    ),
                };
                Check {
                    name: format!("Pressão · {}", outlet.fixture.label()),
                    utilisation,
                    light: if outlet.state == Pressure::Good {
                        Light::Green
                    } else {
                        Light::Yellow
                    },
                    simple,
                    detail: format!(
                        "Desnível da base da caixa ({} m) ao ponto ({} m) = {} m; perdas nos tubos e conexões = {} m; pressão dinâmica = {} m (mínimo NBR 5626 = {} m).",
                        decimal(network.source.z, 2),
                        decimal(outlet.node.z, 2),
                        decimal(outlet.static_m, 2),
                        decimal(outlet.loss_m, 2),
                        decimal(outlet.pressure_m, 2),
                        decimal(network.min_m, 2)
                    ),
                }
            })
            .collect();
        if checks.is_empty() {
            continue;
        }
        verdicts.push(Verdict {
            discipline: Discipline::Plumbing,
            element: match &group.room {
                Some(room) => format!("Água fria · {room}"),
                None => format!("Água fria · {}", group.label),
            },
            checks,
        });
    }
    if let Some(check) = crate::domain::plumbing::cold_water::tank_weight(
        project,
        cold.reserve_l,
        cold.tank_l,
        cold.tank_full_kn,
    ) {
        let short = check.short;
        verdicts.push(Verdict {
            discipline: Discipline::Plumbing,
            element: "Caixa-d'água".to_owned(),
            checks: vec![Check {
                name: "Peso na estrutura".to_owned(),
                utilisation: check.utilisation,
                light: if short { Light::Red } else { Light::Green },
                simple: check.message,
                detail: format!(
                    "{} morador(es) × {} L/dia × {} dia(s) = {} L → caixa comercial de {} L; peso cheia = água + 1,0 kN de caixa e conexões.",
                    cold.residents,
                    decimal(cold.per_person_l, 0),
                    decimal(cold.reserve_days, 1),
                    decimal(cold.reserve_l, 0),
                    decimal(cold.tank_l, 0)
                ),
            }],
        });
    }
    verdicts
}

/// The septic tank and the soakaway (NBR 7229, NBR 13969): the distances
/// the norm asks are red when broken; an absorption rate nobody measured is
/// yellow, because the percolation test decides the soakaway's size.
fn septic_verdict(project: &Project) -> Option<Verdict> {
    use crate::domain::plumbing::septic::{
        AbsorptionSource, MAX_SOAKAWAY_DEPTH_M, WATER_TABLE_CLEARANCE_M, septic_design,
    };
    let design = septic_design(project)?;
    let mut checks = vec![Check {
        name: "Tamanho do tanque".to_owned(),
        utilisation: 0.0,
        light: Light::Green,
        simple: format!(
            "Tanque séptico de {} L úteis para {} pessoa(s): cilíndrico (anéis) de {} m de diâmetro com {} m de água, ou retangular de {} × {} m com {} m de água.",
            decimal(design.volume_l, 0),
            design.people,
            decimal(design.diameter_m, 2),
            decimal(design.cylinder_depth_m, 2),
            decimal(design.length_m, 2),
            decimal(design.width_m, 2),
            decimal(design.depth_m, 2)
        ),
        detail: format!(
            "NBR 7229: V = 1000 + N·(C·T + K·Lf); N = {}, C = {} L/dia, T = {} dia, K = {} dias, Lf = 1 L/dia → {} L. Profundidade útil de {} a {} m (Tabela 4); largura mínima 0,80 m, comprimento 2 a 4 vezes a largura.",
            design.people,
            decimal(design.per_person_l, 0),
            decimal(design.detention_days, 2),
            decimal(design.sludge_k_days, 0),
            decimal(design.volume_l, 0),
            decimal(design.depth_m, 2),
            decimal(design.max_depth_m, 2)
        ),
    }];
    if design.placed {
        for distance in &design.distances {
            let utilisation = distance.needed_m / distance.distance_m.max(1e-9);
            let light = if !distance.ok() {
                Light::Red
            } else if !distance.comfortable() {
                Light::Yellow
            } else {
                Light::from_utilisation(utilisation)
            };
            let simple = if !distance.ok() {
                format!(
                    "O {} fica a {} m da {}: a norma pede pelo menos {} m. Mude o lugar dele.",
                    distance.element,
                    decimal(distance.distance_m, 2),
                    distance.to,
                    decimal(distance.normative_m, 2)
                )
            } else if !distance.comfortable() {
                format!(
                    "O {} fica a {} m da {}: atende a norma, mas em solo colapsível o Trazo pede {} m para a água não chegar às fundações. Afaste-o se puder, ou apoie as fundações vizinhas em brocas.",
                    distance.element,
                    decimal(distance.distance_m, 2),
                    distance.to,
                    decimal(distance.needed_m, 2)
                )
            } else {
                format!(
                    "O {} fica a {} m da {} (mínimo {} m).",
                    distance.element,
                    decimal(distance.distance_m, 2),
                    distance.to,
                    decimal(distance.needed_m, 2)
                )
            };
            checks.push(Check {
                name: format!("Distância · {} → {}", distance.element, distance.to),
                utilisation: if distance.ok() {
                    utilisation.min(1.0)
                } else {
                    utilisation
                },
                light,
                simple,
                detail: "NBR 7229 §5.1: 1,50 m de construções, divisas, sumidouros e ramal de água; 3,0 m de árvores e da rede pública de água; 15 m de poços e corpos d'água. Medido da face externa do tanque cilíndrico e do sumidouro (paredes de 10 cm).".to_owned(),
            });
        }
    } else {
        checks.push(Check {
            name: "Posição".to_owned(),
            utilisation: 1.0,
            light: Light::Yellow,
            simple: "Marque onde ficam o tanque séptico e o sumidouro (projeto_hidraulico → configurar_fossa) para o Trazo conferir as distâncias da norma.".to_owned(),
            detail: "Sem posição, as distâncias a construções, divisas e entre o tanque e o sumidouro não são conferidas.".to_owned(),
        });
    }
    let depth_ratio = design.soakaway_height_m / MAX_SOAKAWAY_DEPTH_M;
    let tested = design.absorption_source == AbsorptionSource::Tested;
    checks.push(Check {
        name: "Sumidouro".to_owned(),
        utilisation: depth_ratio,
        light: if !tested && depth_ratio <= 1.0 + 1e-9 {
            Light::Yellow
        } else {
            Light::from_utilisation(depth_ratio)
        },
        simple: if depth_ratio > 1.0 + 1e-9 {
            format!(
                "{} sumidouro(s) de {} m de diâmetro precisariam de {} m de altura útil: fundo demais para cavar. Use mais sumidouros, um diâmetro maior ou valas de infiltração, e faça o teste de infiltração.",
                design.soakaway_count,
                decimal(design.soakaway_diameter_m, 2),
                decimal(design.soakaway_height_m, 1)
            )
        } else if tested {
            format!(
                "{} sumidouro(s) de {} m de diâmetro com {} m de altura útil abaixo da entrada (escavação de {} m).",
                design.soakaway_count,
                decimal(design.soakaway_diameter_m, 2),
                decimal(design.soakaway_height_m, 1),
                decimal(design.soakaway_depth_m, 1)
            )
        } else {
            format!(
                "Estimativa: {} sumidouro(s) de {} m de diâmetro com {} m de altura útil (escavação de {} m). A taxa de infiltração ({} L/m²·dia) não foi medida: faça o teste de infiltração no lugar do sumidouro antes de cavar.",
                design.soakaway_count,
                decimal(design.soakaway_diameter_m, 2),
                decimal(design.soakaway_height_m, 1),
                decimal(design.soakaway_depth_m, 1),
                decimal(design.absorption_l_m2_day, 0)
            )
        },
        detail: format!(
            "Q = {} L/dia; TAS = {} L/m²·dia → A = Q/TAS = {} m²; por sumidouro A = π·D²/4 + π·D·H (NBR 13969). Fundo a pelo menos {} m acima do lençol; 20 m de poços.",
            decimal(design.daily_l, 0),
            decimal(design.absorption_l_m2_day, 0),
            decimal(design.area_needed_m2, 1),
            decimal(WATER_TABLE_CLEARANCE_M, 2)
        ),
    });
    if let Some(water) = design.water_table_m {
        let needed = design.soakaway_depth_m + WATER_TABLE_CLEARANCE_M;
        checks.push(Check::new(
            "Lençol freático",
            needed / water.max(1e-9),
            format!(
                "O fundo do sumidouro ({} m) precisa ficar {} m acima do lençol ({} m).",
                decimal(design.soakaway_depth_m, 1),
                decimal(WATER_TABLE_CLEARANCE_M, 1),
                decimal(water, 1)
            ),
            "NBR 13969: distância mínima de 1,50 m entre o fundo do sumidouro e o nível máximo do lençol freático.".to_owned(),
        ));
    }
    Some(Verdict {
        discipline: Discipline::Plumbing,
        element: "Fossa séptica e sumidouro".to_owned(),
        checks,
    })
}

/// Hot water (#119): each electric shower's outlet in the electrical design
/// and the pressure left at the hot taps. A shower without its own outlet is
/// yellow (the design is incomplete); an outlet too weak for the shower is
/// red: its wire and breaker would come out too small and heat up.
fn hot_water_verdicts(project: &Project) -> Vec<Verdict> {
    use crate::domain::plumbing::hot_water::{SHOWER_MIN_POWER_W, hot_water};
    let Some(hot) = hot_water(project) else {
        return Vec::new();
    };
    let mut verdicts = Vec::new();
    if hot.electrical_design {
        for shower in &hot.showers {
            let (light, utilisation, simple) = match shower.outlet_va {
                None => (
                    Light::Yellow,
                    1.0,
                    format!(
                        "O chuveiro não tem tomada de uso específico no projeto elétrico: gere os pontos elétricos para ele ganhar circuito próprio (cerca de {} W).",
                        decimal(hot.shower_power_w, 0)
                    ),
                ),
                Some(va) if va + 1e-6 < SHOWER_MIN_POWER_W => (
                    Light::Red,
                    SHOWER_MIN_POWER_W / va.max(1.0),
                    format!(
                        "A tomada do chuveiro tem {} VA, mas um chuveiro elétrico pede pelo menos {} W: corrija a potência para o fio e o disjuntor saírem certos.",
                        decimal(va, 0),
                        decimal(SHOWER_MIN_POWER_W, 0)
                    ),
                ),
                Some(va) => (
                    Light::Green,
                    SHOWER_MIN_POWER_W / va.max(1.0),
                    format!("Tomada própria de {} VA para o chuveiro.", decimal(va, 0)),
                ),
            };
            verdicts.push(Verdict {
                discipline: Discipline::Plumbing,
                element: match &shower.room {
                    Some(room) => format!("Chuveiro elétrico · {room}"),
                    None => "Chuveiro elétrico".to_owned(),
                },
                checks: vec![Check {
                    name: "Tomada de uso específico".to_owned(),
                    utilisation,
                    light,
                    simple,
                    detail: format!(
                        "NBR 5410: aparelho de mais de 10 A em circuito próprio; chuveiro de referência {} W.",
                        decimal(hot.shower_power_w, 0)
                    ),
                }],
            });
        }
    }
    if let Some(network) = &hot.network {
        let checks: Vec<Check> = network
            .outlets
            .iter()
            .map(|outlet| Check {
                name: format!("Água quente · {}", outlet.fixture.label()),
                utilisation: if outlet.pressure_m > 0.0 {
                    outlet.target_m(network.min_m) / outlet.pressure_m
                } else {
                    f64::INFINITY
                },
                light: if outlet.state == Pressure::Good {
                    Light::Green
                } else {
                    Light::Yellow
                },
                simple: format!(
                    "A água quente chega com {} m de pressão ({}).",
                    decimal(outlet.pressure_m.max(0.0), 2),
                    outlet.state.label()
                ),
                detail: format!(
                    "Desnível = {} m; perdas = {} m; pressão dinâmica = {} m (mínimo NBR 5626 = {} m).",
                    decimal(outlet.static_m, 2),
                    decimal(outlet.loss_m, 2),
                    decimal(outlet.pressure_m, 2),
                    decimal(network.min_m, 2)
                ),
            })
            .collect();
        if !checks.is_empty() {
            verdicts.push(Verdict {
                discipline: Discipline::Plumbing,
                element: format!("Água quente · {}", hot.heater.label()),
                checks,
            });
        }
    }
    verdicts
}

/// Sewage (#120): the main's capacity in drainage units and the depth of
/// the inspection boxes. A box deeper than a metre is yellow: it needs a
/// manhole, nothing falls.
fn sewage_verdict(project: &Project) -> Option<Verdict> {
    use crate::domain::plumbing::sewage::{DeviceKind, MAX_BOX_DEPTH_M, sewage};
    let design = sewage(project)?;
    let mut checks = vec![Check::new(
        "Capacidade do coletor",
        design.total_uhc / design.main_capacity_uhc.max(1e-9),
        format!(
            "Coletor DN {} leva {} UHC; a casa tem {} UHC.",
            decimal(design.main_dn, 0),
            decimal(design.main_capacity_uhc, 0),
            decimal(design.total_uhc, 0)
        ),
        "NBR 8160 Tabela 7: capacidade do subcoletor e do coletor predial pela declividade."
            .to_owned(),
    )];
    let deepest = design
        .devices
        .iter()
        .filter(|d| d.kind == DeviceKind::InspectionBox)
        .filter_map(|d| d.depth_m.map(|z| (d.label.clone(), z)))
        .max_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((label, depth)) = deepest {
        let deep = depth > MAX_BOX_DEPTH_M + 1e-6;
        checks.push(Check {
            name: "Profundidade das caixas".to_owned(),
            utilisation: depth / MAX_BOX_DEPTH_M,
            light: if deep { Light::Yellow } else { Light::Green },
            simple: if deep {
                format!(
                    "A {label} fica a {} m: acima de {} m é poço de visita, não caixa de inspeção.",
                    decimal(depth, 2),
                    decimal(MAX_BOX_DEPTH_M, 2)
                )
            } else {
                format!(
                    "A caixa mais funda ({label}) fica a {} m.",
                    decimal(depth, 2)
                )
            },
            detail: "Caixas de inspeção de 60 × 60 cm até 1,00 m de profundidade (NBR 8160)."
                .to_owned(),
        });
    }
    Some(Verdict {
        discipline: Discipline::Plumbing,
        element: "Esgoto".to_owned(),
        checks,
    })
}
