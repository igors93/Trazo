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
    let Some(cold) = cold_water(project) else {
        return Vec::new();
    };
    let network = &cold.network;
    let mut verdicts = Vec::new();
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
    let tank = project
        .fixtures
        .iter()
        .find(|f| f.kind == crate::domain::fixtures::FixtureKind::WaterTank);
    if let Some(load) =
        tank.and_then(|t| t.structural_load_kn.or(t.kind.default_structural_load_kn()))
    {
        let utilisation = cold.tank_full_kn / load.max(1e-9);
        let short = load + 0.05 < cold.tank_full_kn;
        verdicts.push(Verdict {
            discipline: Discipline::Plumbing,
            element: "Caixa-d'água".to_owned(),
            checks: vec![Check {
                name: "Peso na estrutura".to_owned(),
                utilisation,
                light: if short { Light::Red } else { Light::Green },
                simple: if short {
                    format!(
                        "A caixa de {} L cheia pesa {} kN, mas a laje foi conferida para {} kN: corrija a carga da caixa no projeto estrutural e confira de novo.",
                        decimal(cold.tank_l, 0),
                        decimal(cold.tank_full_kn, 1),
                        decimal(load, 1)
                    )
                } else {
                    format!(
                        "A estrutura foi conferida para {} kN; a caixa de {} L cheia pesa {} kN.",
                        decimal(load, 1),
                        decimal(cold.tank_l, 0),
                        decimal(cold.tank_full_kn, 1)
                    )
                },
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
