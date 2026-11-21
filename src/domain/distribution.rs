//! The distribution board: phase balancing (#91), voltage drop, demand
//! factors and service entrance sizing (#90), and the single-line diagram
//! with the panel schedule (#89), all computed from the circuits.
//!
//! These are pre-sizing aids, in line with NBR 5410 and the usual practice
//! of Brazilian utilities; each utility publishes its own entrance tables,
//! so the result is a starting point to check, not a final design.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use super::conduits::conduit_size;
use super::electrical::{BREAKERS, CircuitKind, DeviceRole, LoadSchedule, WIRE_CAPACITY};
use super::model::Project;
use super::schedules::ScheduleTable;

/// Regional Brazilian utility profile controlling demand factors and service
/// entrance thresholds (monophasic/biphasic/triphasic cutoffs, minimum main
/// breaker and entrance wire gauge).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtilityProfile {
    /// Generic Brazilian dwelling tables (NBR 5410 baseline).
    #[default]
    Generic,
    /// Enel SP (LIG-BT): 127/220 V, mono up to 12 kVA, bi up to 25 kVA.
    EnelSp,
    /// CPFL (GED-13): 127/220 V, mono up to 12 kVA, bi up to 25 kVA.
    Cpfl,
    /// CEMIG (ND 5.1): 127/220 V, mono up to 10 kVA, bi up to 15 kVA.
    Cemig,
    /// COPEL / Celesc (NTC 901100, 220/380 V): mono up to 15 kVA, then three-phase.
    Copel,
}

impl UtilityProfile {
    pub const ALL: [UtilityProfile; 5] = [
        UtilityProfile::Generic,
        UtilityProfile::EnelSp,
        UtilityProfile::Cpfl,
        UtilityProfile::Cemig,
        UtilityProfile::Copel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Generic => "Padrão genérico (NBR 5410)",
            Self::EnelSp => "Enel SP (LIG-BT)",
            Self::Cpfl => "CPFL (GED-13)",
            Self::Cemig => "CEMIG (ND 5.1)",
            Self::Copel => "COPEL / Celesc (NTC 901100)",
        }
    }

    pub fn rules(self) -> UtilityRules {
        match self {
            Self::Generic => UtilityRules {
                single_phase_max_kva: 8.0,
                two_phase_max_kva: 15.0,
                max_bt_kva: 75.0,
                min_breaker_a: 32.0,
                min_wire_mm2: 6.0,
                general_demand_scale: 1.0,
                specific_demand_scale: 1.0,
            },
            Self::EnelSp => UtilityRules {
                single_phase_max_kva: 12.0,
                two_phase_max_kva: 25.0,
                max_bt_kva: 75.0,
                min_breaker_a: 40.0,
                min_wire_mm2: 10.0,
                general_demand_scale: 1.0,
                specific_demand_scale: 0.95,
            },
            Self::Cpfl => UtilityRules {
                single_phase_max_kva: 12.0,
                two_phase_max_kva: 25.0,
                max_bt_kva: 75.0,
                min_breaker_a: 40.0,
                min_wire_mm2: 10.0,
                general_demand_scale: 0.95,
                specific_demand_scale: 1.0,
            },
            Self::Cemig => UtilityRules {
                single_phase_max_kva: 10.0,
                two_phase_max_kva: 15.0,
                max_bt_kva: 75.0,
                min_breaker_a: 40.0,
                min_wire_mm2: 10.0,
                general_demand_scale: 1.0,
                specific_demand_scale: 1.0,
            },
            Self::Copel => UtilityRules {
                single_phase_max_kva: 15.0,
                two_phase_max_kva: 15.0, // 220/380 V jumps directly to three-phase above 15 kVA
                max_bt_kva: 75.0,
                min_breaker_a: 40.0,
                min_wire_mm2: 10.0,
                general_demand_scale: 0.90,
                specific_demand_scale: 0.95,
            },
        }
    }
}

/// Service-entrance limits and demand multipliers for a utility profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UtilityRules {
    pub single_phase_max_kva: f64,
    pub two_phase_max_kva: f64,
    pub max_bt_kva: f64,
    pub min_breaker_a: f64,
    pub min_wire_mm2: f64,
    pub general_demand_scale: f64,
    pub specific_demand_scale: f64,
}

/// Optional per-project overrides for regional utility tables not covered by a
/// built-in [`UtilityProfile`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UtilityOverrides {
    pub single_phase_max_kva: Option<f64>,
    pub two_phase_max_kva: Option<f64>,
    pub max_bt_kva: Option<f64>,
    pub min_breaker_a: Option<f64>,
    pub min_wire_mm2: Option<f64>,
    pub general_demand_scale: Option<f64>,
    pub specific_demand_scale: Option<f64>,
}

impl UtilityRules {
    pub fn with_overrides(mut self, overrides: Option<UtilityOverrides>) -> Self {
        if let Some(o) = overrides {
            if let Some(v) = o.single_phase_max_kva {
                self.single_phase_max_kva = v.max(1.0);
            }
            if let Some(v) = o.two_phase_max_kva {
                self.two_phase_max_kva = v.max(self.single_phase_max_kva);
            }
            if let Some(v) = o.max_bt_kva {
                self.max_bt_kva = v.max(10.0);
            }
            if let Some(v) = o.min_breaker_a {
                self.min_breaker_a = v.max(10.0);
            }
            if let Some(v) = o.min_wire_mm2 {
                self.min_wire_mm2 = v.max(2.5);
            }
            if let Some(v) = o.general_demand_scale {
                self.general_demand_scale = v.clamp(0.1, 2.0);
            }
            if let Some(v) = o.specific_demand_scale {
                self.specific_demand_scale = v.clamp(0.1, 2.0);
            }
        }
        self
    }

    pub fn general_demand_factor(&self, kva: f64) -> f64 {
        (general_demand_factor(kva) * self.general_demand_scale).clamp(0.10, 1.0)
    }

    pub fn specific_demand_factor(&self, count: usize) -> f64 {
        (specific_demand_factor(count) * self.specific_demand_scale).clamp(0.10, 1.0)
    }
}

/// How the building is supplied by the utility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Supply {
    /// Chosen from the demand.
    #[default]
    Auto,
    /// One phase and neutral.
    Single,
    /// Two phases and neutral.
    Two,
    /// Three phases and neutral.
    Three,
}

impl Supply {
    pub const ALL: [Supply; 4] = [Supply::Auto, Supply::Single, Supply::Two, Supply::Three];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automática",
            Self::Single => "Monofásica",
            Self::Two => "Bifásica",
            Self::Three => "Trifásica",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Single => "monofasica",
            Self::Two => "bifasica",
            Self::Three => "trifasica",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "auto" | "automatica" | "automatico" => Some(Self::Auto),
            "monofasica" | "monofasico" | "mono" => Some(Self::Single),
            "bifasica" | "bifasico" | "bi" => Some(Self::Two),
            "trifasica" | "trifasico" | "tri" => Some(Self::Three),
            _ => None,
        }
    }

    /// Number of phases; 0 while automatic.
    pub fn phases(self) -> u8 {
        match self {
            Self::Auto => 0,
            Self::Single => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }
}

pub const PHASE_NAMES: [&str; 3] = ["A", "B", "C"];

/// Copper resistivity used for voltage drop, in Ω·mm²/m (1/56, the usual
/// value in Brazilian practice).
pub const COPPER_RESISTIVITY: f64 = 1.0 / 56.0;

/// NBR 5410 6.2.7.1: at most 4 % drop in terminal circuits.
pub const MAX_DROP_PERCENT: f64 = 4.0;

/// Demand factor of lighting and general-use outlets, by their installed
/// load in kVA (the table most Brazilian utilities use for dwellings).
pub fn general_demand_factor(kva: f64) -> f64 {
    const TABLE: [(f64, f64); 10] = [
        (1.0, 0.86),
        (2.0, 0.75),
        (3.0, 0.66),
        (4.0, 0.59),
        (5.0, 0.52),
        (6.0, 0.45),
        (7.0, 0.40),
        (8.0, 0.35),
        (9.0, 0.31),
        (10.0, 0.27),
    ];
    TABLE
        .iter()
        .find(|(up_to, _)| kva <= *up_to + 1e-9)
        .map_or(0.24, |(_, factor)| *factor)
}

/// Demand factor of specific-use circuits, by how many there are.
pub fn specific_demand_factor(count: usize) -> f64 {
    const TABLE: [f64; 25] = [
        1.00, 1.00, 0.84, 0.76, 0.70, 0.65, 0.60, 0.57, 0.54, 0.52, 0.49, 0.48, 0.46, 0.45, 0.44,
        0.43, 0.40, 0.40, 0.40, 0.40, 0.39, 0.39, 0.39, 0.38, 0.38,
    ];
    match count {
        0 => 1.0,
        n => TABLE.get(n - 1).copied().unwrap_or(0.38),
    }
}

/// Voltage drop in percent of a circuit at `voltage_v` with wire `section`,
/// for loads given as (cable length m, current A): 2·ρ·Σ(L·I) / S / V.
pub fn drop_percent(loads: &[(f64, f64)], section_mm2: f64, voltage_v: f64) -> f64 {
    let moment: f64 = loads.iter().map(|(length, current)| length * current).sum();
    2.0 * COPPER_RESISTIVITY * moment / section_mm2.max(0.1) / voltage_v.max(1.0) * 100.0
}

/// NBR 5410 table 58: protective conductor for a phase section.
pub fn earth_section(phase_mm2: f64) -> f64 {
    if phase_mm2 <= 16.0 {
        phase_mm2
    } else if phase_mm2 <= 35.0 {
        16.0
    } else {
        phase_mm2 / 2.0
    }
}

/// The likely maximum load, with the demand factors applied.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Demand {
    /// Lighting and general-use outlets, installed.
    pub general_va: f64,
    pub general_factor: f64,
    /// Specific-use circuits, installed.
    pub specific_va: f64,
    pub specific_count: usize,
    pub specific_factor: f64,
    pub demand_va: f64,
}

/// The utility connection and what it takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServiceEntrance {
    pub supply: Supply,
    /// Demand current on the most loaded phase.
    pub current_a: f64,
    pub breaker_a: Option<f64>,
    pub wire_mm2: Option<f64>,
    pub earth_mm2: Option<f64>,
    pub conduit_mm: Option<u32>,
}

/// NBR 5410: keep sub-board feeder voltage drop at or below 2 % so feeder plus
/// terminal circuits remain within the overall drop budget.
pub const MAX_FEEDER_DROP_PERCENT: f64 = 2.0;

/// Summary and feeder sizing for a secondary distribution board (QDC).
#[derive(Clone, Debug, PartialEq)]
pub struct SubBoardSummary {
    pub panel_id: u64,
    pub label: String,
    pub room: Option<String>,
    pub circuits: Vec<u32>,
    pub installed_va: f64,
    pub demand_va: f64,
    pub current_a: f64,
    pub feeder_length_m: Option<f64>,
    pub feeder_drop_percent: Option<f64>,
    pub breaker_a: Option<f64>,
    pub wire_mm2: Option<f64>,
    pub earth_mm2: Option<f64>,
    pub conduit_mm: Option<u32>,
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// "2,5" for wires, "10" for the larger ones.
pub fn section_label(section: f64) -> String {
    if section.fract().abs() > 1e-9 {
        decimal(section, 1)
    } else {
        format!("{section:.0}")
    }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl Project {
    /// Completes the load schedule with what the board needs: poles,
    /// cable lengths and voltage drop (raising wire sections where the drop
    /// passes 4 %), demand, the supply, phase balancing and the entrance.
    pub(super) fn complete_schedule(&self, schedule: &mut LoadSchedule) {
        let settings = self.electrical;
        let general_v = settings.voltage_v.max(1.0);
        // Specific circuits at 220 V on a 127 V network go between phases.
        for row in &mut schedule.circuits {
            row.poles = if row.kind == CircuitKind::SpecificOutlet
                && general_v < 170.0
                && row.voltage_v > 170.0
            {
                2
            } else {
                1
            };
        }

        // Voltage drop along the conduits (or square to the axes), measured
        // from each circuit's serving board (QDG or sub-board QDC).
        let tree = self.conduit_tree();
        let has_sub_boards = !tree.sub_panels.is_empty();
        let mut circuit_board: std::collections::HashMap<u32, u64> =
            std::collections::HashMap::new();
        if tree.panel.is_some() {
            for row in &mut schedule.circuits {
                let mut boards_seen: Vec<u64> = Vec::new();
                let loads: Vec<(f64, f64)> = self
                    .electrical_devices()
                    .filter(|device| device.electrical.and_then(|d| d.circuit) == Some(row.number))
                    .filter(|device| {
                        matches!(
                            device.kind.role(),
                            Some(
                                DeviceRole::Lighting
                                    | DeviceRole::GeneralOutlet
                                    | DeviceRole::SpecificOutlet
                            )
                        )
                    })
                    .filter_map(|device| {
                        if let Some(board_id) = self.serving_board(&tree, device)
                            && !boards_seen.contains(&board_id)
                        {
                            boards_seen.push(board_id);
                        }
                        let length = self.cable_length(&tree, device)?;
                        let power = device.electrical.map_or(0.0, |d| d.power_va);
                        Some((length, power / row.voltage_v.max(1.0)))
                    })
                    .collect();
                if let Some(&board_id) = boards_seen.first() {
                    circuit_board.insert(row.number, board_id);
                    if has_sub_boards {
                        row.board = self.panel_label(board_id);
                    }
                }
                if boards_seen.len() > 1 {
                    schedule.warnings.push(format!(
                        "Circuito {}: pontos atendidos por quadros diferentes; mantenha cada circuito em um único quadro.",
                        row.number
                    ));
                }
                if loads.is_empty() {
                    continue;
                }
                row.length_m = loads.iter().map(|(length, _)| *length).reduce(f64::max);
                if let Some(mut wire) = row.wire_mm2 {
                    if drop_percent(&loads, wire, row.voltage_v) > MAX_DROP_PERCENT {
                        let larger = WIRE_CAPACITY
                            .iter()
                            .map(|(section, _)| *section)
                            .filter(|section| *section > wire)
                            .find(|section| {
                                drop_percent(&loads, *section, row.voltage_v) <= MAX_DROP_PERCENT
                            });
                        match larger {
                            Some(section) => {
                                wire = section;
                                row.wire_by_drop = true;
                            }
                            None => schedule.warnings.push(format!(
                                "Circuito {}: queda de tensão acima de {MAX_DROP_PERCENT:.0} % mesmo com o maior fio; aproxime o quadro ou divida o circuito.",
                                row.number
                            )),
                        }
                    }
                    row.wire_mm2 = Some(wire);
                    row.drop_percent = Some(drop_percent(&loads, wire, row.voltage_v));
                }
            }
            if !self.conduits.is_empty() && !tree.unreached.is_empty() {
                schedule.warnings.push(format!(
                    "{} ponto(s) sem eletroduto até o quadro: use Gerar eletrodutos ou ligue-os.",
                    tree.unreached.len()
                ));
            }
        }

        // Demand, using the selected regional utility profile (and any overrides).
        let rules = settings
            .utility
            .rules()
            .with_overrides(settings.custom_rules);
        let mut demand = Demand::default();
        for row in &schedule.circuits {
            if row.kind == CircuitKind::SpecificOutlet {
                demand.specific_va += row.power_va;
                demand.specific_count += 1;
            } else {
                demand.general_va += row.power_va;
            }
        }
        demand.general_factor = rules.general_demand_factor(demand.general_va / 1000.0);
        demand.specific_factor = rules.specific_demand_factor(demand.specific_count);
        demand.demand_va =
            demand.general_va * demand.general_factor + demand.specific_va * demand.specific_factor;
        schedule.demand = demand;
        if schedule.circuits.is_empty() {
            return;
        }

        // Supply.
        let needed = if schedule.circuits.iter().any(|row| row.poles >= 2) {
            2
        } else {
            1
        };
        let kva = demand.demand_va / 1000.0;
        let supply = match settings.supply {
            Supply::Auto if kva <= rules.single_phase_max_kva && needed == 1 => Supply::Single,
            Supply::Auto
                if kva <= rules.two_phase_max_kva
                    && rules.two_phase_max_kva > rules.single_phase_max_kva =>
            {
                Supply::Two
            }
            Supply::Auto => Supply::Three,
            chosen => chosen,
        };
        let phases = supply.phases().max(1);
        if phases < needed {
            schedule.warnings.push(
                "Há circuitos de 220 V entre fases: a ligação precisa ser ao menos bifásica."
                    .to_owned(),
            );
        }
        if kva > rules.max_bt_kva {
            schedule.warnings.push(format!(
                "Demanda acima de {:.0} kVA ({}): consulte a distribuidora (pode exigir transformador próprio).",
                rules.max_bt_kva,
                settings.utility.label()
            ));
        }
        schedule.supply = supply;

        // Phase balancing: largest circuits first, each on the least loaded
        // phase (two-pole circuits on the least loaded pair).
        let mut order: Vec<usize> = (0..schedule.circuits.len()).collect();
        order.sort_by(|a, b| {
            schedule.circuits[*b]
                .power_va
                .total_cmp(&schedule.circuits[*a].power_va)
                .then(
                    schedule.circuits[*a]
                        .number
                        .cmp(&schedule.circuits[*b].number),
                )
        });
        let mut load = [0.0_f64; 3];
        let mut current = [0.0_f64; 3];
        let used = usize::from(phases);
        for index in order {
            let row = &mut schedule.circuits[index];
            let amps = row.power_va / row.voltage_v.max(1.0);
            if row.poles >= 2 && used >= 2 {
                let mut best = (0, 1);
                for i in 0..used {
                    for j in i + 1..used {
                        if load[i] + load[j] < load[best.0] + load[best.1] - 1e-9 {
                            best = (i, j);
                        }
                    }
                }
                for phase in [best.0, best.1] {
                    load[phase] += row.power_va / 2.0;
                    current[phase] += amps;
                }
                row.phases = vec![best.0 as u8, best.1 as u8];
            } else {
                let phase = (0..used)
                    .min_by(|a, b| load[*a].total_cmp(&load[*b]))
                    .unwrap_or(0);
                load[phase] += row.power_va;
                current[phase] += amps;
                row.phases = vec![phase as u8];
            }
        }
        schedule.phase_va = load;
        if used >= 2 {
            let loads = &load[..used];
            let high = loads.iter().copied().fold(0.0, f64::max);
            let low = loads.iter().copied().fold(f64::MAX, f64::min);
            if high > 0.0 && (high - low) / high > 0.10 {
                schedule.warnings.push(format!(
                    "Desequilíbrio de {:.0} % entre as fases: redistribua ou divida circuitos.",
                    (high - low) / high * 100.0
                ));
            }
        }

        // Service entrance, on the most loaded phase at demand.
        let ratio = if schedule.total_va > 0.0 {
            demand.demand_va / schedule.total_va
        } else {
            1.0
        };
        let current_a = current[..used].iter().copied().fold(0.0, f64::max) * ratio;
        let breaker_a = BREAKERS
            .iter()
            .copied()
            .find(|rating| *rating + 1e-9 >= current_a.max(rules.min_breaker_a));
        let wire_mm2 = breaker_a.and_then(|rating| {
            WIRE_CAPACITY
                .iter()
                .find(|(section, capacity)| {
                    *section >= rules.min_wire_mm2 && *capacity + 1e-9 >= rating
                })
                .map(|(section, _)| *section)
        });
        let earth_mm2 = wire_mm2.map(earth_section);
        let conduit_mm = wire_mm2.and_then(|section| {
            let mut sections = vec![section; used + 1];
            sections.push(earth_section(section));
            conduit_size(&sections)
        });
        if breaker_a.is_none() || wire_mm2.is_none() {
            schedule.warnings.push(format!(
                "Entrada: {} A por fase passa das tabelas; consulte a distribuidora.",
                decimal(current_a, 0)
            ));
        }
        schedule.entrance = Some(ServiceEntrance {
            supply,
            current_a,
            breaker_a,
            wire_mm2,
            earth_mm2,
            conduit_mm,
        });

        // Sub-boards (QDC): demand and feeder sizing from the main board.
        for &sub_id in &tree.sub_panels {
            let Some(panel_fixture) = self.fixtures.iter().find(|f| f.id == sub_id) else {
                continue;
            };
            let label = self
                .panel_label(sub_id)
                .unwrap_or_else(|| format!("QDC{sub_id}"));
            let room = self
                .room_for_fixture(panel_fixture)
                .map(|r| r.display_name().to_owned());
            let mut circuits: Vec<u32> = Vec::new();
            let mut gen_va = 0.0;
            let mut spec_va = 0.0;
            let mut spec_count = 0usize;
            let mut sub_phases = 1usize;
            for row in &schedule.circuits {
                if circuit_board.get(&row.number).copied() == Some(sub_id) {
                    circuits.push(row.number);
                    if row.kind == CircuitKind::SpecificOutlet {
                        spec_va += row.power_va;
                        spec_count += 1;
                    } else {
                        gen_va += row.power_va;
                    }
                    sub_phases = sub_phases.max(usize::from(row.poles));
                }
            }
            let installed_va = gen_va + spec_va;
            let sub_demand_va = gen_va * general_demand_factor(gen_va / 1000.0)
                + spec_va * specific_demand_factor(spec_count);
            let feeder_phases = sub_phases.min(used).max(1);
            let sub_current = sub_demand_va / (general_v * feeder_phases as f64).max(1.0);
            let feeder_length = self.cable_length(&tree, panel_fixture);
            let sub_breaker = if installed_va > 0.0 {
                BREAKERS
                    .iter()
                    .copied()
                    .find(|rating| *rating + 1e-9 >= sub_current.max(16.0))
            } else {
                None
            };
            let mut sub_wire = sub_breaker.and_then(|rating| {
                WIRE_CAPACITY
                    .iter()
                    .find(|(section, capacity)| *section >= 4.0 && *capacity + 1e-9 >= rating)
                    .map(|(section, _)| *section)
            });
            let mut feeder_drop = None;
            if let (Some(length), Some(mut wire)) = (feeder_length, sub_wire) {
                let loads = [(length, sub_current)];
                let v_ref = if feeder_phases >= 2 && general_v < 170.0 {
                    220.0
                } else {
                    general_v
                };
                if drop_percent(&loads, wire, v_ref) > MAX_FEEDER_DROP_PERCENT {
                    if let Some(larger) = WIRE_CAPACITY
                        .iter()
                        .map(|(s, _)| *s)
                        .filter(|s| *s > wire)
                        .find(|s| drop_percent(&loads, *s, v_ref) <= MAX_FEEDER_DROP_PERCENT)
                    {
                        wire = larger;
                    } else {
                        schedule.warnings.push(format!(
                            "{label}: queda de tensão no alimentador acima de {MAX_FEEDER_DROP_PERCENT:.0} %; aproxime o subquadro ou divida a carga."
                        ));
                    }
                }
                sub_wire = Some(wire);
                feeder_drop = Some(drop_percent(&loads, wire, v_ref));
            }
            let sub_earth = sub_wire.map(earth_section);
            let sub_conduit = sub_wire.and_then(|section| {
                let mut sections = vec![section; feeder_phases + 1];
                sections.push(earth_section(section));
                conduit_size(&sections)
            });
            schedule.sub_boards.push(SubBoardSummary {
                panel_id: sub_id,
                label,
                room,
                circuits,
                installed_va,
                demand_va: sub_demand_va,
                current_a: sub_current,
                feeder_length_m: feeder_length,
                feeder_drop_percent: feeder_drop,
                breaker_a: sub_breaker,
                wire_mm2: sub_wire,
                earth_mm2: sub_earth,
                conduit_mm: sub_conduit,
            });
        }
    }
}

/// "A", "B+C"...
pub fn phase_label(phases: &[u8]) -> String {
    phases
        .iter()
        .map(|p| PHASE_NAMES[usize::from(*p).min(2)])
        .collect::<Vec<_>>()
        .join("+")
}

/// The panel schedule for drawing sheets.
pub fn build_panel_schedule(project: &Project) -> ScheduleTable {
    let schedule = project.load_schedule();
    let rows = schedule
        .circuits
        .iter()
        .map(|row| {
            vec![
                format!("C{}", row.number),
                row.kind.short().to_owned(),
                phase_label(&row.phases),
                format!("{:.0}", row.power_va),
                decimal(row.current_a, 1),
                row.wire_mm2.map_or("—".to_owned(), section_label),
                row.breaker_a.map_or("—".to_owned(), |b| format!("{b:.0}")),
                if row.needs_rcd { "Sim" } else { "—" }.to_owned(),
            ]
        })
        .collect();
    let main = schedule
        .entrance
        .and_then(|e| e.breaker_a)
        .map_or("—".to_owned(), |b| format!("{b:.0}"));
    ScheduleTable {
        title: format!(
            "Quadro de cargas (QD) · {} · demanda {} kVA",
            schedule.supply.label(),
            decimal(schedule.demand.demand_va / 1000.0, 2)
        ),
        headers: ["Circ.", "Tipo", "Fase", "VA", "A", "mm²", "Disj.", "DR"]
            .iter()
            .map(|h| (*h).to_owned())
            .collect(),
        rows,
        footer: Some(vec![
            "Total".to_owned(),
            String::new(),
            String::new(),
            format!("{:.0}", schedule.total_va),
            String::new(),
            String::new(),
            format!("DG {main}"),
            String::new(),
        ]),
    }
}

/// The single-line diagram of the board and its panel schedule, as a
/// landscape A3 drawing (taller when there are many circuits).
pub fn single_line_svg(project: &Project) -> String {
    let schedule = project.load_schedule();
    let settings = project.electrical;
    let rows = &schedule.circuits;
    let step = 8.0;
    let top = 92.0;
    let width = 420.0;
    let table_rows = rows.len() as f64 + 1.0;
    let height = (top + rows.len() as f64 * step + 40.0)
        .max(60.0 + table_rows * 6.0 + 110.0)
        .max(297.0);
    let ink = "#1d2836";
    let red = "#c62828";
    let font = "font-family=\"Arial, sans-serif\"";
    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height:.0}mm\" viewBox=\"0 0 {width} {height:.0}\">\n\
         <rect width=\"{width}\" height=\"{height:.0}\" fill=\"#ffffff\"/>\n\
         <rect x=\"10\" y=\"10\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>\n",
        width - 20.0,
        height - 20.0
    );
    let text = |svg: &mut String,
                x: f64,
                y: f64,
                size: f64,
                bold: bool,
                anchor: &str,
                value: &str| {
        let _ = writeln!(
            svg,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" {font} font-size=\"{size}\"{} fill=\"{ink}\" text-anchor=\"{anchor}\">{}</text>",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        );
    };
    let line = |svg: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, w: f64, color: &str| {
        let _ = writeln!(
            svg,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"{color}\" stroke-width=\"{w}\"/>"
        );
    };
    text(
        &mut svg,
        20.0,
        24.0,
        5.0,
        true,
        "start",
        "DIAGRAMA UNIFILAR · QUADRO DE DISTRIBUIÇÃO (QD)",
    );
    text(
        &mut svg,
        20.0,
        31.0,
        3.2,
        false,
        "start",
        &format!(
            "{} · ligação {} · {:.0}/{:.0} V · {}",
            project.name,
            schedule.supply.label().to_lowercase(),
            settings.voltage_v,
            if settings.voltage_v < 170.0 {
                220.0
            } else {
                380.0
            },
            settings.utility.label()
        ),
    );
    if rows.is_empty() {
        text(
            &mut svg,
            20.0,
            48.0,
            3.6,
            false,
            "start",
            "Nenhum circuito ainda: gere os pontos e distribua os circuitos.",
        );
        svg.push_str("</svg>\n");
        return svg;
    }
    let entrance = schedule.entrance;
    let phases = schedule.supply.phases().max(1);
    let poles = |n: u8| format!("{n}P");

    // Utility, meter, main breaker and surge protection.
    let x = 40.0;
    text(
        &mut svg,
        x,
        42.0,
        3.0,
        true,
        "middle",
        "Rede da distribuidora",
    );
    line(&mut svg, x, 44.0, x, 50.0, 0.5, ink);
    let _ = writeln!(
        svg,
        "<rect x=\"{:.2}\" y=\"50\" width=\"14\" height=\"8\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>",
        x - 7.0
    );
    text(&mut svg, x, 55.4, 3.0, true, "middle", "kWh");
    line(&mut svg, x, 58.0, x, 64.0, 0.5, ink);
    // A breaker: the blade open at an angle, a cross at the fixed contact.
    let breaker_v = |svg: &mut String, x: f64, y: f64| {
        line(svg, x, y, x + 3.2, y + 6.0, 0.5, ink);
        line(svg, x - 1.0, y + 6.0, x + 1.0, y + 8.0, 0.4, ink);
        line(svg, x + 1.0, y + 6.0, x - 1.0, y + 8.0, 0.4, ink);
        line(svg, x, y + 7.0, x, y + 10.0, 0.5, ink);
    };
    breaker_v(&mut svg, x, 64.0);
    let main_label = match entrance.and_then(|e| e.breaker_a) {
        Some(b) => format!("DG {} {b:.0} A", poles(phases)),
        None => "DG —".to_owned(),
    };
    text(&mut svg, x + 6.0, 69.0, 3.0, true, "start", &main_label);
    if let Some(e) = entrance {
        let feeder = match (e.wire_mm2, e.earth_mm2) {
            (Some(w), Some(t)) => format!(
                "Ramal: {}#{} mm² (F) + {} mm² (N) + {} mm² (T){}",
                phases,
                section_label(w),
                section_label(w),
                section_label(t),
                e.conduit_mm
                    .map_or(String::new(), |d| format!(" · Ø{d} mm"))
            ),
            _ => "Ramal: consulte a distribuidora".to_owned(),
        };
        text(&mut svg, x + 12.0, 51.0, 2.6, false, "start", &feeder);
        text(
            &mut svg,
            x + 12.0,
            56.0,
            2.6,
            false,
            "start",
            &format!(
                "Demanda {} kVA · {} A na fase mais carregada",
                decimal(schedule.demand.demand_va / 1000.0, 2),
                decimal(e.current_a, 1)
            ),
        );
    }
    line(&mut svg, x, 74.0, x, 82.0, 0.5, ink);
    // Surge protection (DPS) to earth, beside the main breaker.
    line(&mut svg, x, 78.0, x + 22.0, 78.0, 0.35, ink);
    let _ = writeln!(
        svg,
        "<rect x=\"{:.2}\" y=\"75.5\" width=\"8\" height=\"5\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.4\"/>",
        x + 22.0
    );
    text(&mut svg, x + 26.0, 79.0, 2.2, true, "middle", "DPS");
    line(&mut svg, x + 30.0, 78.0, x + 34.0, 78.0, 0.35, ink);
    for (i, half) in [3.0, 2.0, 1.0].iter().enumerate() {
        let gx = x + 34.0 + i as f64 * 0.8;
        line(&mut svg, gx, 78.0 - half, gx, 78.0 + half, 0.35, ink);
    }

    // The busbar and one branch per circuit.
    let bus_x = x;
    let bus_bottom = top + (rows.len() as f64 - 1.0) * step + 2.0;
    line(&mut svg, bus_x, 82.0, bus_x, bus_bottom, 1.2, ink);
    for (i, row) in rows.iter().enumerate() {
        let y = top + i as f64 * step;
        line(&mut svg, bus_x, y, bus_x + 14.0, y, 0.4, ink);
        // Breaker, lying down.
        let bx = bus_x + 14.0;
        line(&mut svg, bx, y, bx + 6.0, y - 3.0, 0.45, ink);
        line(&mut svg, bx + 6.0, y - 1.0, bx + 8.0, y + 1.0, 0.35, ink);
        line(&mut svg, bx + 6.0, y + 1.0, bx + 8.0, y - 1.0, 0.35, ink);
        line(&mut svg, bx + 7.0, y, bx + 24.0, y, 0.4, ink);
        text(
            &mut svg,
            bx + 4.0,
            y - 3.6,
            2.4,
            true,
            "middle",
            &row.breaker_a.map_or("—".to_owned(), |b| {
                format!("{}{b:.0} A", if row.poles >= 2 { "2P " } else { "" })
            }),
        );
        let mut cx = bx + 24.0;
        if row.needs_rcd {
            let _ = writeln!(
                svg,
                "<ellipse cx=\"{:.2}\" cy=\"{y:.2}\" rx=\"3.4\" ry=\"1.8\" fill=\"none\" stroke=\"{red}\" stroke-width=\"0.4\"/>",
                cx + 4.0
            );
            text(
                &mut svg,
                cx + 4.0,
                y - 2.6,
                2.0,
                false,
                "middle",
                "DR 30 mA",
            );
            line(&mut svg, cx, y, cx + 12.0, y, 0.4, ink);
            cx += 12.0;
        } else {
            line(&mut svg, cx, y, cx + 12.0, y, 0.4, ink);
            cx += 12.0;
        }
        line(&mut svg, cx, y, cx + 40.0, y, 0.4, ink);
        // Wires ticks: phases, neutral, earth.
        let wires = usize::from(row.poles.max(1)) + usize::from(row.poles < 2) + 1;
        for k in 0..wires {
            let tx = cx + 6.0 + k as f64 * 1.6;
            line(&mut svg, tx - 0.6, y + 1.2, tx + 0.6, y - 1.2, 0.3, ink);
        }
        text(
            &mut svg,
            cx + 13.0,
            y - 1.0,
            2.4,
            false,
            "start",
            &format!(
                "{} mm²{}",
                row.wire_mm2.map_or("—".to_owned(), section_label),
                if row.wire_by_drop { " *" } else { "" }
            ),
        );
        text(
            &mut svg,
            cx + 30.0,
            y - 1.0,
            2.4,
            true,
            "start",
            &phase_label(&row.phases),
        );
        text(
            &mut svg,
            cx + 42.0,
            y + 0.9,
            2.6,
            true,
            "start",
            &format!("C{}", row.number),
        );
        let rooms = if row.rooms.is_empty() {
            String::new()
        } else {
            format!(" · {}", row.rooms.join(", "))
        };
        let board_tag = row
            .board
            .as_deref()
            .filter(|b| *b != "QDG")
            .map_or(String::new(), |b| format!("[{b}] "));
        let mut description = format!(
            "{board_tag}{}{} · {:.0} VA",
            row.kind.label(),
            rooms,
            row.power_va
        );
        if description.chars().count() > 46 {
            description = description.chars().take(45).collect::<String>() + "…";
        }
        text(
            &mut svg,
            cx + 50.0,
            y + 0.9,
            2.4,
            false,
            "start",
            &description,
        );
    }
    // Earth and neutral bars.
    let bars_y = bus_bottom + 8.0;
    text(
        &mut svg,
        bus_x,
        bars_y,
        2.4,
        false,
        "start",
        "Barramentos de neutro e de terra (PE) no quadro; PE ligado ao aterramento.",
    );
    if !schedule.sub_boards.is_empty() {
        let mut sy = bars_y + 7.0;
        text(
            &mut svg,
            bus_x,
            sy,
            2.6,
            true,
            "start",
            "Quadros secundários (QDC):",
        );
        for sub in &schedule.sub_boards {
            sy += 4.5;
            let circs = if sub.circuits.is_empty() {
                "sem circuitos".to_owned()
            } else {
                sub.circuits
                    .iter()
                    .map(|c| format!("C{c}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let feeder = format!(
                "{} ({}) · demanda {} kVA · disj. {} · alim. {} mm²",
                sub.label,
                circs,
                decimal(sub.demand_va / 1000.0, 2),
                sub.breaker_a
                    .map_or("—".to_owned(), |b| format!("{b:.0} A")),
                sub.wire_mm2.map_or("—".to_owned(), section_label),
            );
            text(&mut svg, bus_x + 2.0, sy, 2.3, false, "start", &feeder);
        }
    }

    // Panel schedule.
    let tx = 222.0;
    let columns: [(&str, f64); 12] = [
        ("Circ.", 11.0),
        ("Tipo", 14.0),
        ("Fases", 11.0),
        ("Pot. (VA)", 17.0),
        ("A (VA)", 15.0),
        ("B (VA)", 15.0),
        ("C (VA)", 15.0),
        ("I (A)", 12.0),
        ("Fio (mm²)", 15.0),
        ("Disj.", 12.0),
        ("DR", 9.0),
        ("Queda", 12.0),
    ];
    let table_w: f64 = columns.iter().map(|(_, w)| w).sum();
    let row_h = 6.0;
    let mut y = 42.0;
    text(
        &mut svg,
        tx,
        y - 3.0,
        3.4,
        true,
        "start",
        "QUADRO DE CARGAS E DISTRIBUIÇÃO",
    );
    let total_rows = rows.len() + 2;
    let _ = writeln!(
        svg,
        "<rect x=\"{tx}\" y=\"{y}\" width=\"{table_w}\" height=\"{:.1}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.4\"/>\n\
         <rect x=\"{tx}\" y=\"{y}\" width=\"{table_w}\" height=\"{row_h}\" fill=\"#edf2f7\" stroke=\"{ink}\" stroke-width=\"0.3\"/>",
        total_rows as f64 * row_h
    );
    let cell_row = |svg: &mut String, y: f64, cells: &[String], bold: bool| {
        let mut cx = tx;
        for ((_, w), cell) in columns.iter().zip(cells) {
            text(svg, cx + w / 2.0, y + 4.1, 2.4, bold, "middle", cell);
            cx += w;
        }
    };
    cell_row(
        &mut svg,
        y,
        &columns
            .iter()
            .map(|(h, _)| (*h).to_owned())
            .collect::<Vec<_>>(),
        true,
    );
    y += row_h;
    for row in rows {
        let mut per_phase = [String::new(), String::new(), String::new()];
        for phase in &row.phases {
            per_phase[usize::from(*phase).min(2)] =
                format!("{:.0}", row.power_va / row.phases.len().max(1) as f64);
        }
        let [a, b, c] = per_phase;
        cell_row(
            &mut svg,
            y,
            &[
                format!("C{}", row.number),
                row.kind.short().to_owned(),
                phase_label(&row.phases),
                format!("{:.0}", row.power_va),
                a,
                b,
                c,
                decimal(row.current_a, 1),
                row.wire_mm2.map_or("—".to_owned(), section_label),
                row.breaker_a.map_or("—".to_owned(), |v| format!("{v:.0}")),
                if row.needs_rcd { "Sim" } else { "—" }.to_owned(),
                row.drop_percent
                    .map_or("—".to_owned(), |d| format!("{} %", decimal(d, 1))),
            ],
            false,
        );
        line(&mut svg, tx, y, tx + table_w, y, 0.15, "#b8c4d0");
        y += row_h;
    }
    line(&mut svg, tx, y, tx + table_w, y, 0.35, ink);
    let phase_total = |i: usize| {
        if i < usize::from(phases) {
            format!("{:.0}", schedule.phase_va[i])
        } else {
            String::new()
        }
    };
    cell_row(
        &mut svg,
        y,
        &[
            "Total".to_owned(),
            String::new(),
            String::new(),
            format!("{:.0}", schedule.total_va),
            phase_total(0),
            phase_total(1),
            phase_total(2),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ],
        true,
    );
    let mut cx = tx;
    for (_, w) in &columns[..columns.len() - 1] {
        cx += w;
        line(&mut svg, cx, 42.0, cx, y + row_h, 0.15, "#b8c4d0");
    }
    y += row_h + 10.0;

    // Demand and entrance summary.
    let d = schedule.demand;
    let mut summary = vec![
        format!(
            "Carga instalada: {:.0} VA ({} kVA) em {} circuito(s).",
            schedule.total_va,
            decimal(schedule.total_va / 1000.0, 2),
            rows.len()
        ),
        format!(
            "Iluminação e TUG: {:.0} VA × {} = {:.0} VA",
            d.general_va,
            decimal(d.general_factor, 2),
            d.general_va * d.general_factor
        ),
        format!(
            "TUE: {:.0} VA em {} circuito(s) × {} = {:.0} VA",
            d.specific_va,
            d.specific_count,
            decimal(d.specific_factor, 2),
            d.specific_va * d.specific_factor
        ),
        format!(
            "Demanda provável: {} kVA · ligação {}",
            decimal(d.demand_va / 1000.0, 2),
            schedule.supply.label().to_lowercase()
        ),
    ];
    if let Some(e) = entrance {
        summary.push(format!(
            "Disjuntor geral: {} · ramal {} mm² · eletroduto Ø{} mm",
            e.breaker_a
                .map_or("—".to_owned(), |b| format!("{} {b:.0} A", poles(phases))),
            e.wire_mm2.map_or("—".to_owned(), section_label),
            e.conduit_mm.map_or("—".to_owned(), |c| c.to_string())
        ));
    }
    for (i, item) in summary.iter().enumerate() {
        text(&mut svg, tx, y + i as f64 * 5.0, 2.8, i == 3, "start", item);
    }
    y += summary.len() as f64 * 5.0 + 6.0;
    for (i, note) in [
        "Fios de cobre com isolação de PVC 750 V, método B1, 30 °C. Seções mínimas: 1,5 mm² (iluminação) e 2,5 mm² (tomadas).",
        "* Fio aumentado para manter a queda de tensão até 4 % (NBR 5410 6.2.7). Queda calculada com ρ = 1/56 Ω·mm²/m.",
        "Fatores de demanda usuais das distribuidoras para residências; confira a norma da sua distribuidora.",
        "Pré-dimensionamento: confira sempre o projeto com um profissional habilitado (ART/RRT).",
    ]
    .iter()
    .enumerate()
    {
        text(&mut svg, tx, y + i as f64 * 4.2, 2.3, false, "start", note);
    }
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;
    use crate::domain::geometry::Point;

    fn house() -> Project {
        let mut project = Project::default();
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala")
            .unwrap();
        project
            .add_room(Point::new(5.0, 0.0), Point::new(8.0, 4.0), "Cozinha")
            .unwrap();
        project
            .add_room(Point::new(0.0, 4.0), Point::new(2.0, 6.0), "Banheiro")
            .unwrap();
        project
            .add_fixture(FixtureKind::Shower, Point::new(0.6, 5.4), 0.9, 0.9, 0)
            .unwrap();
        project.generate_electrical_points();
        project.assign_circuits();
        project
    }

    #[test]
    fn demand_factors_follow_the_usual_tables() {
        assert_eq!(general_demand_factor(0.8), 0.86);
        assert_eq!(general_demand_factor(3.4), 0.59);
        assert_eq!(general_demand_factor(12.0), 0.24);
        assert_eq!(specific_demand_factor(1), 1.0);
        assert_eq!(specific_demand_factor(4), 0.76);
        assert_eq!(specific_demand_factor(40), 0.38);
        assert_eq!(earth_section(10.0), 10.0);
        assert_eq!(earth_section(25.0), 16.0);
        assert_eq!(earth_section(70.0), 35.0);
        // 10 m to a 1270 VA load at 127 V on 2.5 mm²: 2·(1/56)·10·10/2.5/127.
        let drop = drop_percent(&[(10.0, 10.0)], 2.5, 127.0);
        assert!((drop - 1.125).abs() < 0.01, "{drop}");
    }

    #[test]
    fn the_board_balances_phases_and_sizes_the_entrance() {
        let project = house();
        let schedule = project.load_schedule();
        // The 220 V shower on a 127 V network goes between two phases, so
        // the supply is at least two-phase.
        let shower = schedule
            .circuits
            .iter()
            .find(|row| row.kind == CircuitKind::SpecificOutlet)
            .unwrap();
        assert_eq!(shower.poles, 2);
        assert_eq!(shower.phases.len(), 2);
        assert_eq!(schedule.supply, Supply::Two);
        // Every circuit is on a phase and the two phases carry the load.
        assert!(schedule.circuits.iter().all(|row| !row.phases.is_empty()));
        let total: f64 = schedule.phase_va.iter().sum();
        assert!((total - schedule.total_va).abs() < 1e-6);
        assert!(schedule.phase_va[2] == 0.0);
        let (a, b) = (schedule.phase_va[0], schedule.phase_va[1]);
        assert!((a - b).abs() / a.max(b) <= 0.10, "{a} × {b}");
        // Demand below the installed load; entrance breaker and feeder.
        assert!(schedule.demand.demand_va < schedule.total_va);
        let entrance = schedule.entrance.unwrap();
        assert!(entrance.breaker_a.unwrap() >= entrance.current_a);
        assert!(entrance.wire_mm2.unwrap() >= 6.0);
        assert!(schedule.warnings.is_empty(), "{:?}", schedule.warnings);
    }

    #[test]
    fn a_long_run_gets_a_thicker_wire_for_voltage_drop() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(60.0, 10.0));
        let (w, d) = FixtureKind::Panel.default_size();
        project
            .add_fixture(FixtureKind::Panel, Point::new(0.0, 0.0), w, d, 0)
            .unwrap();
        let (w, d) = FixtureKind::OutletSpecific.default_size();
        let far = project
            .add_fixture(FixtureKind::OutletSpecific, Point::new(50.0, 0.0), w, d, 0)
            .unwrap();
        project
            .update_electrical(far, Some(Some(1)), Some(4400.0), None)
            .unwrap();
        let row = project.load_schedule().circuits[0].clone();
        // 20 A needs a 20 A breaker and 2.5 mm², but 50 m away the drop
        // asks for more.
        assert_eq!(row.breaker_a, Some(20.0));
        assert!(row.wire_by_drop);
        assert!(row.wire_mm2.unwrap() > 2.5);
        assert!(row.drop_percent.unwrap() <= MAX_DROP_PERCENT);
        assert!(row.length_m.unwrap() > 50.0);
    }

    #[test]
    fn a_forced_single_phase_supply_warns_about_two_pole_circuits() {
        let mut project = house();
        project.electrical.supply = Supply::Single;
        let schedule = project.load_schedule();
        assert_eq!(schedule.supply, Supply::Single);
        assert!(schedule.warnings.iter().any(|w| w.contains("bifásica")));
        assert!(schedule.circuits.iter().all(|row| row.phases == vec![0]));
    }

    #[test]
    fn the_single_line_diagram_is_valid_svg_with_every_circuit() {
        let project = house();
        let svg = single_line_svg(&project);
        let tree = svg2pdf::usvg::Tree::from_str(&svg, &svg2pdf::usvg::Options::default());
        assert!(tree.is_ok());
        for row in project.load_schedule().circuits {
            assert!(svg.contains(&format!(">C{}<", row.number)));
        }
        assert!(svg.contains("DPS"));
        assert!(svg.contains("Demanda provável"));
        let table = build_panel_schedule(&project);
        assert_eq!(table.rows.len(), project.load_schedule().circuits.len());
        // An empty project still draws a page.
        assert!(single_line_svg(&Project::default()).contains("Nenhum circuito"));
    }

    #[test]
    fn sub_boards_are_modelled_with_dedicated_feeders_and_shortened_terminal_runs() {
        use super::super::conduits::ConduitRun;

        let mut project = Project::default();
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(50.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        project
            .add_room(Point::new(30.0, 0.0), Point::new(35.0, 3.0), "Edícula")
            .unwrap();
        let (pw, pd) = FixtureKind::Panel.default_size();
        let main = project
            .add_fixture(FixtureKind::Panel, Point::new(0.4, 0.4), pw, pd, 0)
            .unwrap();
        let sub = project
            .add_fixture(FixtureKind::Panel, Point::new(30.4, 0.4), pw, pd, 0)
            .unwrap();
        let (ow, od) = FixtureKind::OutletSpecific.default_size();
        let near_main = project
            .add_fixture(FixtureKind::OutletSpecific, Point::new(2.0, 1.5), ow, od, 0)
            .unwrap();
        let near_sub = project
            .add_fixture(
                FixtureKind::OutletSpecific,
                Point::new(32.0, 1.5),
                ow,
                od,
                0,
            )
            .unwrap();
        project
            .update_electrical(near_main, Some(Some(1)), Some(2200.0), None)
            .unwrap();
        project
            .update_electrical(near_sub, Some(Some(2)), Some(2200.0), None)
            .unwrap();

        project
            .add_conduit(main, near_main, ConduitRun::Ceiling)
            .unwrap();
        project.add_conduit(main, sub, ConduitRun::Floor).unwrap();
        project
            .add_conduit(sub, near_sub, ConduitRun::Ceiling)
            .unwrap();

        let schedule = project.load_schedule();
        assert_eq!(schedule.sub_boards.len(), 1);
        let qdc = &schedule.sub_boards[0];
        assert_eq!(qdc.panel_id, sub);
        assert_eq!(qdc.label, "QDC1");
        assert_eq!(qdc.circuits, vec![2]);
        assert!(qdc.feeder_length_m.unwrap() >= 30.0);
        assert!(qdc.breaker_a.is_some());
        assert!(qdc.wire_mm2.is_some());

        let c1 = schedule.circuits.iter().find(|c| c.number == 1).unwrap();
        let c2 = schedule.circuits.iter().find(|c| c.number == 2).unwrap();
        assert_eq!(c1.board.as_deref(), Some("QDG"));
        assert_eq!(c2.board.as_deref(), Some("QDC1"));
        // Terminal run for C2 starts at QDC1 (~2 m away), not QDG (~32 m away).
        assert!(c2.length_m.unwrap() < 10.0);

        let svg = single_line_svg(&project);
        assert!(svg.contains("QDC1"));
        assert!(svg2pdf::usvg::Tree::from_str(&svg, &svg2pdf::usvg::Options::default()).is_ok());
    }

    #[test]
    fn utility_profiles_and_regional_overrides_change_demand_and_entrance() {
        let mut project = house();
        let generic = project.load_schedule();

        // COPEL (220/380 V) has a 15 kVA single-phase limit, 40 A / 10 mm² floor
        // and lower general demand scaling.
        project.electrical.utility = UtilityProfile::Copel;
        project.electrical.voltage_v = 220.0;
        let copel = project.load_schedule();
        assert!(copel.demand.general_factor < generic.demand.general_factor);
        let copel_ent = copel.entrance.unwrap();
        assert!(copel_ent.breaker_a.unwrap() >= 40.0);
        assert!(copel_ent.wire_mm2.unwrap() >= 10.0);
        assert!(single_line_svg(&project).contains("COPEL"));

        // Custom regional overrides can tailor thresholds for any utility.
        project.electrical.utility = UtilityProfile::Generic;
        project.electrical.voltage_v = 127.0;
        project.electrical.custom_rules = Some(UtilityOverrides {
            min_breaker_a: Some(50.0),
            min_wire_mm2: Some(16.0),
            ..Default::default()
        });
        let custom = project.load_schedule();
        let custom_ent = custom.entrance.unwrap();
        assert!(custom_ent.breaker_a.unwrap() >= 50.0);
        assert!(custom_ent.wire_mm2.unwrap() >= 16.0);

        // Soil and occupancy presets update structural settings cleanly.
        use super::super::structure::{OccupancyClass, SoilClass, room_live_load_kn_m2};
        let mut struct_settings = project.structure.settings;
        struct_settings.apply_soil_class(SoilClass::SoftClay);
        assert!((struct_settings.soil_kpa - 80.0).abs() < 1e-9);
        struct_settings.apply_occupancy(OccupancyClass::Office);
        assert!((struct_settings.live_kn_m2 - 2.0).abs() < 1e-9);
        assert!((room_live_load_kn_m2("Garagem", 1.5) - 3.0).abs() < 1e-9);
        assert!((room_live_load_kn_m2("Varanda", 1.5) - 2.5).abs() < 1e-9);
    }
}
