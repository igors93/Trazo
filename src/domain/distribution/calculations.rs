//! Voltage drop, demand factors, phase balancing, service entrance and sub-board sizing.

use crate::domain::{
    conduits::conduit_size,
    electrical::{
        BREAKERS, CircuitKind, DeviceRole, LoadSchedule, WIRE_CAPACITY, WIRE_CAPACITY_3_LOADED,
    },
    model::Project,
};

use super::{
    COPPER_RESISTIVITY, Demand, MAX_DROP_PERCENT, MAX_FEEDER_DROP_PERCENT, ServiceEntrance,
    SubBoardSummary, Supply, decimal,
};

/// Demand factor of lighting and general-use outlets, by their installed
/// load in kVA (the table most Brazilian utilities use for dwellings).
#[cfg(test)]
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

/// Demand factor of specific-use circuits, by how many there are (matches
/// `utility_profiles::STANDARD_SPECIFIC_DEMAND_TABLE`).
#[cfg(test)]
pub fn specific_demand_factor(count: usize) -> f64 {
    const TABLE: [f64; 25] = [
        1.00, 1.00, 0.84, 0.76, 0.70, 0.65, 0.60, 0.57, 0.54, 0.52, 0.49, 0.48, 0.46, 0.45, 0.44,
        0.43, 0.42, 0.41, 0.40, 0.40, 0.39, 0.39, 0.39, 0.38, 0.38,
    ];
    match count {
        0 => 1.0,
        n => TABLE.get(n - 1).copied().unwrap_or(0.38),
    }
}

/// Voltage drop in percent of a 1-phase or 2-phase circuit at `voltage_v` with
/// wire `section`, for loads given as (cable length m, current A):
/// `2·ρ·Σ(L·I) / (S·V) · 100`.
pub fn drop_percent(loads: &[(f64, f64)], section_mm2: f64, voltage_v: f64) -> f64 {
    let moment: f64 = loads.iter().map(|(length, current)| length * current).sum();
    2.0 * COPPER_RESISTIVITY * moment / section_mm2.max(0.1) / voltage_v.max(1.0) * 100.0
}

/// Voltage drop in percent for a feeder with `phases` (1 = 1F+N, 2 = 2F+N, 3 = 3F+N):
/// - 1F+N: `2·ρ·L·I / (S·V_fn) · 100`
/// - 2F+N: `2·ρ·L·I / (S·V_ref) · 100` where `V_ref = V_fn` if any 1P loads exist, else `V_ll`
/// - 3F+N: `√3·ρ·L·I / (S·V_ll) · 100` (= `ρ·L·I / (S·V_fn) · 100` for balanced 3P)
fn feeder_drop_percent(
    length_m: f64,
    current_a: f64,
    section_mm2: f64,
    feeder_phases: usize,
    general_v: f64,
    has_single_pole_loads: bool,
) -> f64 {
    let s = section_mm2.max(0.1);
    match feeder_phases {
        3 => {
            let v_ll = general_v * 3.0_f64.sqrt();
            3.0_f64.sqrt() * COPPER_RESISTIVITY * length_m * current_a / s / v_ll.max(1.0) * 100.0
        }
        2 => {
            let v_ref = if has_single_pole_loads {
                general_v
            } else {
                general_v * 3.0_f64.sqrt()
            };
            2.0 * COPPER_RESISTIVITY * length_m * current_a / s / v_ref.max(1.0) * 100.0
        }
        _ => 2.0 * COPPER_RESISTIVITY * length_m * current_a / s / general_v.max(1.0) * 100.0,
    }
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

impl Project {
    /// Completes the load schedule with what the board needs: poles,
    /// cable lengths and voltage drop (raising wire sections where the drop
    /// passes 4 %), demand, the supply, phase balancing and the entrance.
    pub(crate) fn complete_schedule(&self, schedule: &mut LoadSchedule) {
        let settings = self.electrical;
        let general_v = settings.voltage_v.max(1.0);
        // Specific circuits at line-to-line voltage (e.g. 220 V on 127 V, or 380 V on 220 V)
        // connect across two phases.
        for row in &mut schedule.circuits {
            row.poles =
                if row.kind == CircuitKind::SpecificOutlet && row.voltage_v > general_v * 1.3 {
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
            let mut loads_by_circuit: std::collections::HashMap<
                u32,
                Vec<&crate::domain::fixtures::Fixture>,
            > = std::collections::HashMap::new();
            for device in self.electrical_devices() {
                if matches!(
                    device.kind.role(),
                    Some(
                        DeviceRole::Lighting
                            | DeviceRole::GeneralOutlet
                            | DeviceRole::SpecificOutlet
                    )
                ) && let Some(num) = device.electrical.and_then(|d| d.circuit)
                {
                    loads_by_circuit.entry(num).or_default().push(device);
                }
            }
            for row in &mut schedule.circuits {
                let mut boards_seen: Vec<u64> = Vec::new();
                let loads: Vec<(f64, f64)> = loads_by_circuit
                    .get(&row.number)
                    .into_iter()
                    .flat_map(|list| list.iter().copied())
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
        // Track general and specific-use phase currents separately so each receives
        // its own NBR/utility demand factor (never diluting a 1.00-factor shower
        // by a 0.27-factor general load).
        let mut gen_current = [0.0_f64; 3];
        let mut spec_current = [0.0_f64; 3];
        let used = usize::from(phases);
        for index in order {
            let row = &mut schedule.circuits[index];
            let amps = row.power_va / row.voltage_v.max(1.0);
            let is_specific = row.kind == CircuitKind::SpecificOutlet;
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
                    if is_specific {
                        spec_current[phase] += amps;
                    } else {
                        gen_current[phase] += amps;
                    }
                }
                row.phases = vec![best.0 as u8, best.1 as u8];
            } else {
                let phase = (0..used)
                    .min_by(|a, b| load[*a].total_cmp(&load[*b]))
                    .unwrap_or(0);
                load[phase] += row.power_va;
                if is_specific {
                    spec_current[phase] += amps;
                } else {
                    gen_current[phase] += amps;
                }
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

        // Service entrance, on the most loaded phase at demand (applying general
        // and specific demand factors to their respective phase currents).
        let current_a = (0..used)
            .map(|p| {
                gen_current[p] * demand.general_factor + spec_current[p] * demand.specific_factor
            })
            .fold(0.0, f64::max);
        let breaker_a = BREAKERS
            .iter()
            .copied()
            .find(|rating| *rating + 1e-9 >= current_a.max(rules.min_breaker_a));
        // NBR 5410 Table 46: 2F+N and 3F+N circuits have THREE loaded conductors
        // (neutral carries full phase current in 2F+N at 120°), requiring the
        // 3-loaded-conductor Table 36 ampacity column.
        let entrance_ampacity = if used >= 2 {
            &WIRE_CAPACITY_3_LOADED
        } else {
            &WIRE_CAPACITY
        };
        let wire_mm2 = breaker_a.and_then(|rating| {
            entrance_ampacity
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
            let mut sub_phases_used: [bool; 3] = [false; 3];
            let mut sub_gen_current = [0.0_f64; 3];
            let mut sub_spec_current = [0.0_f64; 3];
            let mut has_single_pole = false;
            for row in &schedule.circuits {
                if circuit_board.get(&row.number).copied() == Some(sub_id) {
                    circuits.push(row.number);
                    let amps = row.power_va / row.voltage_v.max(1.0);
                    if row.poles == 1 {
                        has_single_pole = true;
                    }
                    if row.kind == CircuitKind::SpecificOutlet {
                        spec_va += row.power_va;
                        spec_count += 1;
                        for &p in &row.phases {
                            let idx = usize::from(p).min(2);
                            sub_phases_used[idx] = true;
                            sub_spec_current[idx] += amps;
                        }
                    } else {
                        gen_va += row.power_va;
                        for &p in &row.phases {
                            let idx = usize::from(p).min(2);
                            sub_phases_used[idx] = true;
                            sub_gen_current[idx] += amps;
                        }
                    }
                }
            }
            let installed_va = gen_va + spec_va;
            // Use the active utility profile's demand tables (including custom overrides).
            let sub_gen_f = rules.general_demand_factor(gen_va / 1000.0);
            let sub_spec_f = rules.specific_demand_factor(spec_count);
            let sub_demand_va = gen_va * sub_gen_f + spec_va * sub_spec_f;
            let feeder_phases = sub_phases_used
                .iter()
                .filter(|&&u| u)
                .count()
                .clamp(1, used);
            // Peak per-phase demand current (uses 220 V for 2-pole loads instead of 2 × 127 = 254 V).
            let sub_current = (0..3)
                .map(|p| sub_gen_current[p] * sub_gen_f + sub_spec_current[p] * sub_spec_f)
                .fold(0.0, f64::max);
            let feeder_length = self.cable_length(&tree, panel_fixture);
            let sub_breaker = if installed_va > 0.0 {
                BREAKERS
                    .iter()
                    .copied()
                    .find(|rating| *rating + 1e-9 >= sub_current.max(16.0))
            } else {
                None
            };
            let feeder_ampacity = if feeder_phases >= 2 {
                &WIRE_CAPACITY_3_LOADED
            } else {
                &WIRE_CAPACITY
            };
            let mut sub_wire = sub_breaker.and_then(|rating| {
                feeder_ampacity
                    .iter()
                    .find(|(section, capacity)| *section >= 4.0 && *capacity + 1e-9 >= rating)
                    .map(|(section, _)| *section)
            });
            let mut feeder_drop = None;
            if let (Some(length), Some(mut wire)) = (feeder_length, sub_wire) {
                let drop_for = |sec: f64| {
                    feeder_drop_percent(
                        length,
                        sub_current,
                        sec,
                        feeder_phases,
                        general_v,
                        has_single_pole,
                    )
                };
                if drop_for(wire) > MAX_FEEDER_DROP_PERCENT {
                    if let Some(larger) = feeder_ampacity
                        .iter()
                        .map(|(s, _)| *s)
                        .filter(|s| *s > wire)
                        .find(|s| drop_for(*s) <= MAX_FEEDER_DROP_PERCENT)
                    {
                        wire = larger;
                    } else {
                        schedule.warnings.push(format!(
                            "{label}: queda de tensão no alimentador acima de {MAX_FEEDER_DROP_PERCENT:.0} %; aproxime o subquadro ou divida a carga."
                        ));
                    }
                }
                sub_wire = Some(wire);
                feeder_drop = Some(drop_for(wire));
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
