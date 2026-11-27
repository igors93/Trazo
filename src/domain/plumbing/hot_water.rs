//! Hot water (#119): the heater the house needs, by how it heats the water.
//!
//! - **Electric showers** (the usual case): no hot-water pipes; each shower
//!   needs its own specific-use outlet (TUE) and circuit in the electrical
//!   design, with the shower's power.
//! - **Gas**: an instantaneous heater (*aquecedor de passagem*) sized by the
//!   hot water used at once, with its power, its gas and where it may stand.
//! - **Solar**: a hot-water tank (*reservatório térmico*, *boiler*) for the
//!   day's hot water and the collector area that heats it.
//!
//! With gas or solar, the hot-water pipes from the heater to the fixtures
//! are laid and sized like the cold water's, and the pressure left at each
//! fixture is checked the same way.

use crate::domain::fixtures::FixtureKind;
use crate::domain::geometry::Point;
use crate::domain::model::Project;

use super::Heater;
use super::cold_water::{ColdWater, cold_water, decimal};
use super::network::{GRAVITY, Network, Node, build_network};
use super::points::{PlumbingPoint, PointKind, plumbing_points};
use super::rules::{ColdWaterRules, FixtureTable};

/// Hot water a person uses in a day, at 40 °C, L (Trazo's reference for a
/// house; NBR 15569 leaves it to the designer).
pub const HOT_PER_PERSON_L: f64 = 50.0;
/// Hot water a square metre of collector heats in a day, L (a Brazilian
/// average; colder or cloudier cities need more area).
pub const COLLECTOR_L_PER_M2: f64 = 80.0;
/// Commercial hot-water tanks, L.
const BOILERS_L: [f64; 9] = [
    100.0, 150.0, 200.0, 300.0, 400.0, 500.0, 600.0, 800.0, 1000.0,
];
/// Commercial gas heaters, L/min.
const GAS_HEATERS_L_MIN: [f64; 7] = [7.5, 10.0, 15.0, 20.0, 26.0, 33.0, 43.0];
/// Hot water a shower takes, L/min at 40 °C.
const SHOWER_L_MIN: f64 = 8.0;
/// Hot water a tap takes, L/min.
const TAP_L_MIN: f64 = 4.0;
/// How much the heater warms the water, K (about 20 °C in, 40 °C out).
const RISE_K: f64 = 20.0;
/// Share of the gas's energy that reaches the water.
const GAS_EFFICIENCY: f64 = 0.80;
/// Energy of a kilogram of LPG, kWh.
const LPG_KWH_PER_KG: f64 = 12.8;
/// Head a gas heater takes from the water passing through it, m.
pub const GAS_HEATER_LOSS_M: f64 = 1.0;
/// Height of a gas heater on its wall, m.
const GAS_HEATER_Z_M: f64 = 1.60;
/// Pressure most gas heaters need at their inlet to light, m of water.
const GAS_HEATER_MIN_INLET_M: f64 = 2.0;
/// The least power of an electric shower, W.
pub const SHOWER_MIN_POWER_W: f64 = 4400.0;
/// A usual electric shower's power at 127 V and at 220 V, W.
const SHOWER_POWER_127_W: f64 = 5500.0;
const SHOWER_POWER_220_W: f64 = 7500.0;
/// The hot-water tank sits this far under the cold tank's base, m.
const BOILER_UNDER_TANK_M: f64 = 0.30;

/// One electric shower and its outlet in the electrical design.
#[derive(Clone, Debug, PartialEq)]
pub struct ElectricShower {
    pub fixture_id: u64,
    pub room: Option<String>,
    /// The power of its specific-use outlet, VA; `None` when it has none.
    pub outlet_va: Option<f64>,
}

impl ElectricShower {
    /// Whether its outlet takes a shower's power.
    pub fn ok(&self) -> bool {
        self.outlet_va
            .is_some_and(|va| va + 1e-6 >= SHOWER_MIN_POWER_W)
    }
}

/// The hot-water design.
#[derive(Clone, Debug, PartialEq)]
pub struct HotWater {
    pub heater: Heater,
    pub residents: u32,
    /// Hot water for a day, L.
    pub daily_l: f64,
    /// Electric showers and their outlets.
    pub showers: Vec<ElectricShower>,
    /// The power a shower of the circuit's voltage usually has, W.
    pub shower_power_w: f64,
    /// Whether the plan has an electrical design to check the showers against.
    pub electrical_design: bool,
    /// Gas: hot water used at once, L/min; the heater's size, its power
    /// and its gas.
    pub gas_flow_l_min: f64,
    pub gas_heater_l_min: f64,
    pub gas_power_kw: f64,
    pub gas_kg_h: f64,
    /// Solar: the hot-water tank, the collector area and the weight of the
    /// full tank, kN.
    pub boiler_l: f64,
    pub collector_m2: f64,
    pub boiler_full_kn: f64,
    /// Where the heater (or the hot-water tank) stands.
    pub heater_at: Option<Point>,
    pub heater_given: bool,
    /// The hot-water pipes, with gas or solar.
    pub network: Option<Network>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

fn next_size(sizes: &[f64], value: f64) -> f64 {
    sizes
        .iter()
        .copied()
        .find(|s| *s >= value - 1e-9)
        .unwrap_or_else(|| sizes.last().copied().unwrap_or(value).max(value))
}

/// The room a fixture stands in, by name.
fn room_of(project: &Project, at: Point) -> Option<String> {
    project
        .room_at(at)
        .and_then(|id| project.rooms.iter().find(|r| r.id == id))
        .map(|r| r.display_name().to_owned())
}

/// Where a gas heater goes when nobody says: the service area, else the
/// kitchen (never the bathroom), else the tank's place.
fn default_gas_heater_at(project: &Project, cold: &ColdWater) -> Point {
    for words in [["serviço", "lavanderia"], ["cozinha", "copa"]] {
        if let Some(centre) = project
            .rooms
            .iter()
            .filter(|r| {
                let name = r.display_name().to_lowercase();
                words.iter().any(|w| name.contains(w))
            })
            .find_map(|r| project.room_centroid(r))
        {
            return centre;
        }
    }
    cold.tank_at
}

/// The hot-water design, or `None` when no fixture takes water.
pub fn hot_water(project: &Project) -> Option<HotWater> {
    let cold = cold_water(project)?;
    let heater = project.plumbing.heater;
    let points = plumbing_points(project);
    let showers_fixtures: Vec<_> = project
        .fixtures
        .iter()
        .filter(|f| f.kind == FixtureKind::Shower)
        .collect();
    let daily_l = f64::from(cold.residents) * HOT_PER_PERSON_L;
    let mut warnings = Vec::new();
    let mut notes = Vec::new();

    // Electric showers: their outlets in the electrical design.
    let voltage = project.electrical.specific_voltage_v;
    let shower_power_w = if voltage < 170.0 {
        SHOWER_POWER_127_W
    } else {
        SHOWER_POWER_220_W
    };
    let electrical_design = project.fixtures.iter().any(|f| f.kind.is_electrical());
    let showers: Vec<ElectricShower> = if heater == Heater::ElectricShower {
        showers_fixtures
            .iter()
            .map(|shower| {
                let room = project.room_at(shower.center);
                let outlet_va = project
                    .fixtures
                    .iter()
                    .filter(|f| f.kind == FixtureKind::OutletSpecific)
                    .filter(|f| room.is_some() && project.room_at(f.center) == room)
                    .map(|f| f.electrical.map_or(0.0, |e| e.power_va))
                    .fold(None, |best: Option<f64>, va| {
                        Some(best.map_or(va, |b| b.max(va)))
                    });
                ElectricShower {
                    fixture_id: shower.id,
                    room: room_of(project, shower.center),
                    outlet_va,
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    if heater == Heater::ElectricShower && electrical_design {
        for shower in showers.iter().filter(|s| !s.ok()) {
            let place = shower
                .room
                .clone()
                .unwrap_or_else(|| "sem cômodo".to_owned());
            warnings.push(match shower.outlet_va {
                None => format!(
                    "Chuveiro ({place}) sem tomada de uso específico no projeto elétrico: ele precisa de circuito próprio de cerca de {} W; ligado numa tomada comum, o fio esquenta.",
                    decimal(shower_power_w, 0)
                ),
                Some(va) => format!(
                    "A tomada do chuveiro ({place}) tem {} VA: um chuveiro elétrico pede pelo menos {} W; corrija a potência para o fio e o disjuntor saírem certos.",
                    decimal(va, 0),
                    decimal(SHOWER_MIN_POWER_W, 0)
                ),
            });
        }
    }

    // Gas: the hot water used at once.
    let taps = points
        .points
        .iter()
        .filter(|p| p.kind == PointKind::Hot && p.fixture != FixtureKind::Shower)
        .count();
    let gas_flow_l_min = (showers_fixtures.len().min(2) as f64) * SHOWER_L_MIN
        + if taps > 0 { TAP_L_MIN } else { 0.0 };
    let gas_heater_l_min = next_size(&GAS_HEATERS_L_MIN, gas_flow_l_min.max(GAS_HEATERS_L_MIN[0]));
    let gas_power_kw = gas_heater_l_min / 60.0 * 4.186 * RISE_K / GAS_EFFICIENCY;
    let gas_kg_h = gas_power_kw / LPG_KWH_PER_KG;

    // Solar: a day of hot water in the tank.
    let boiler_l = next_size(&BOILERS_L, daily_l);
    let collector_m2 = (boiler_l / COLLECTOR_L_PER_M2 * 2.0).ceil() / 2.0;
    let boiler_full_kn = boiler_l * 1.15 * GRAVITY / 1000.0;

    // The hot-water pipes.
    let hot_points: Vec<&PlumbingPoint> = points
        .points
        .iter()
        .filter(|p| p.kind == PointKind::Hot)
        .collect();
    let settings = &project.plumbing;
    let heater_at = match heater {
        Heater::ElectricShower => None,
        Heater::Gas => Some(
            settings
                .heater_at
                .unwrap_or_else(|| default_gas_heater_at(project, &cold)),
        ),
        Heater::Solar => Some(
            settings
                .heater_at
                .unwrap_or(Point::new(cold.tank_at.x + 1.5, cold.tank_at.y)),
        ),
    };
    let network = match (heater, heater_at) {
        (Heater::ElectricShower, _) | (_, None) => None,
        _ if hot_points.is_empty() => None,
        (heater, Some(at)) => {
            let table = FixtureTable::for_project(project);
            let rules = ColdWaterRules::for_project(project);
            let (z, head) = if heater == Heater::Gas {
                (GAS_HEATER_Z_M, cold.tank_base_m - GAS_HEATER_LOSS_M)
            } else {
                let z = cold.tank_base_m - BOILER_UNDER_TANK_M;
                (z, z)
            };
            Some(build_network(
                &hot_points,
                Node::new(at, z),
                head,
                "AQ",
                &table,
                &rules,
            ))
        }
    };
    if let Some(network) = &network {
        for outlet in network
            .outlets
            .iter()
            .filter(|o| o.state != super::network::Pressure::Good)
        {
            warnings.push(format!(
                "Água quente no {}{}: {} m de pressão ({}).",
                outlet.fixture.label().to_lowercase(),
                outlet
                    .room
                    .as_ref()
                    .map_or(String::new(), |r| format!(" ({r})")),
                decimal(outlet.pressure_m.max(0.0), 2),
                outlet.state.label()
            ));
        }
    }

    match heater {
        Heater::ElectricShower => {
            notes.push(format!(
                "Chuveiro elétrico: cada um com circuito próprio, fio e disjuntor do projeto elétrico, cerca de {} W em {} V; a resistência e o aterramento seguem o manual.",
                decimal(shower_power_w, 0),
                decimal(voltage, 0)
            ));
            if !electrical_design && !showers.is_empty() {
                notes.push(
                    "Sem projeto elétrico ainda: ao gerar os pontos elétricos, cada chuveiro ganha sua tomada de uso específico."
                        .to_owned(),
                );
            }
        }
        Heater::Gas => {
            let head = cold.tank_base_m - GAS_HEATER_Z_M;
            notes.push(format!(
                "Aquecedor a gás de passagem de {} L/min ({} kW, cerca de {} kg/h de GLP), para {} L/min usados ao mesmo tempo (até dois chuveiros de {} L/min e uma torneira).",
                decimal(gas_heater_l_min, 1),
                decimal(gas_power_kw, 1),
                decimal(gas_kg_h, 2),
                decimal(gas_flow_l_min, 0),
                decimal(SHOWER_L_MIN, 0)
            ));
            warnings.push(
                "Aquecedor a gás nunca dentro do banheiro nem em cômodo de dormir: só em área ventilada, com chaminé para fora (NBR 13103). O monóxido de carbono mata sem cheiro."
                    .to_owned(),
            );
            if head < GAS_HEATER_MIN_INLET_M {
                warnings.push(format!(
                    "A caixa-d'água fica só {} m acima do aquecedor: muitos aquecedores pedem {} m para acender. Escolha um modelo de baixa pressão ou suba a caixa.",
                    decimal(head.max(0.0), 2),
                    decimal(GAS_HEATER_MIN_INLET_M, 1)
                ));
            }
            if settings.heater_at.is_none() {
                notes.push(
                    "Posição do aquecedor escolhida pelo Trazo (área de serviço ou cozinha); ajuste-a ao lugar ventilado onde ele vai ficar."
                        .to_owned(),
                );
            }
        }
        Heater::Solar => {
            notes.push(format!(
                "Aquecedor solar: {} L de água quente por dia ({} morador(es) × {} L); reservatório térmico de {} L e {} m² de coletores voltados para o norte, inclinados a latitude da cidade + 10°.",
                decimal(daily_l, 0),
                cold.residents,
                decimal(HOT_PER_PERSON_L, 0),
                decimal(boiler_l, 0),
                decimal(collector_m2, 1)
            ));
            notes.push(format!(
                "O reservatório cheio pesa cerca de {} kN e os coletores cerca de 0,15 kN/m²: confira a laje ou o telhado que os recebe. A caixa-d'água fica acima do reservatório, que fica acima dos coletores.",
                decimal(boiler_full_kn, 1)
            ));
            notes.push(
                "Nos dias nublados, a resistência elétrica do reservatório completa o aquecimento: ela pede circuito próprio no projeto elétrico."
                    .to_owned(),
            );
        }
    }

    Some(HotWater {
        heater,
        residents: cold.residents,
        daily_l,
        showers,
        shower_power_w,
        electrical_design,
        gas_flow_l_min,
        gas_heater_l_min,
        gas_power_kw,
        gas_kg_h,
        boiler_l,
        collector_m2,
        boiler_full_kn,
        heater_given: settings.heater_at.is_some(),
        heater_at,
        network,
        warnings,
        notes,
    })
}
