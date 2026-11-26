//! Electrical shopping list (#124): wire lengths by section and NBR 5410
//! colour, conduits by nominal diameter and run type, junction boxes and DIN
//! distribution enclosures, breakers, 30 mA RCDs, Class II SPDs and finishing
//! devices, with CSV and A3/A4 vector SVG exports.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use crate::domain::{
    conduits::{ConduitRun, ConduitWiring},
    distribution::{Supply, earth_section, section_label},
    electrical::CircuitKind,
    fixtures::{Fixture, FixtureKind},
    model::Project,
    schedules::ScheduleTable,
};

/// Default waste allowance applied to wires and conduits (percent).
pub const DEFAULT_ELECTRICAL_WASTE_PCT: f64 = 10.0;

/// Connection slack per conductor at a terminal device box (m).
pub const DEVICE_SLACK_M: f64 = 0.25;

/// Connection slack per conductor inside a distribution board (m).
pub const PANEL_SLACK_M: f64 = 0.50;

/// Standard Brazilian conductor colour coding (ABNT NBR 5410 §6.1.5.3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WireColor {
    /// Phase conductor (preto, vermelho ou branco — nunca azul-claro nem verde/amarelo).
    Phase,
    /// Neutral conductor (azul-claro obrigatório).
    Neutral,
    /// Switch return / traveller (amarelo ou cinza — distinto de fase, neutro e terra).
    Return,
    /// Protective earth conductor PE (verde ou verde-amarelo obrigatório).
    Earth,
}

impl WireColor {
    pub fn label(self) -> &'static str {
        match self {
            Self::Phase => "Fase (preto / vermelho / branco)",
            Self::Neutral => "Neutro (azul-claro)",
            Self::Return => "Retorno (amarelo / cinza)",
            Self::Earth => "Terra PE (verde / verde-amarelo)",
        }
    }

    pub fn color_name(self) -> &'static str {
        match self {
            Self::Phase => "Preto/Vermelho",
            Self::Neutral => "Azul-claro",
            Self::Return => "Amarelo",
            Self::Earth => "Verde/Amarelo",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Phase => "fase",
            Self::Neutral => "neutro",
            Self::Return => "retorno",
            Self::Earth => "terra",
        }
    }
}

/// Aggregated copper PVC 750 V wire length for one `(section, colour)` pair.
#[derive(Clone, Debug, PartialEq)]
pub struct WireShoppingRow {
    pub section_mm2: f64,
    pub color: WireColor,
    /// Exact geometric length (horizontal + vertical drops + connection slack), m.
    pub net_length_m: f64,
    /// Length including waste allowance, m.
    pub total_length_m: f64,
    /// Commercial 100 m rolls needed (`ceil(total / 100)`).
    pub rolls_100m: u32,
}

/// Aggregated corrugated/rigid conduit length for one `(diameter, run)` pair.
#[derive(Clone, Debug, PartialEq)]
pub struct ConduitShoppingRow {
    pub diameter_mm: u32,
    pub run: ConduitRun,
    pub net_length_m: f64,
    pub total_length_m: f64,
    /// Commercial 50 m coils needed (`ceil(total / 50)`).
    pub rolls_50m: u32,
}

/// Junction boxes and distribution enclosures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxShoppingRow {
    pub kind: String,
    pub specification: String,
    pub quantity: usize,
}

/// Category of a protective device inside the distribution board(s).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProtectionCategory {
    MainBreaker,
    FeederBreaker,
    CircuitBreaker,
    Rcd,
    Dps,
}

impl ProtectionCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::MainBreaker => "Disjuntor geral",
            Self::FeederBreaker => "Disjuntor alimentador",
            Self::CircuitBreaker => "Disjuntor terminal",
            Self::Rcd => "Interruptor DR (30 mA)",
            Self::Dps => "Protetor DPS (Classe II)",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::MainBreaker => "disjuntor_geral",
            Self::FeederBreaker => "disjuntor_alimentador",
            Self::CircuitBreaker => "disjuntor_terminal",
            Self::Rcd => "dr",
            Self::Dps => "dps",
        }
    }
}

/// One grouped line of protective equipment (breakers, RCDs, SPDs).
#[derive(Clone, Debug, PartialEq)]
pub struct ProtectionShoppingRow {
    pub category: ProtectionCategory,
    pub specification: String,
    pub poles: u8,
    pub rating_a: Option<f64>,
    pub curve_or_class: String,
    pub quantity: usize,
    pub notes: String,
}

/// Finishing electrical devices (outlets, switches, light points).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceShoppingRow {
    pub specification: String,
    pub quantity: usize,
}

/// Complete electrical bill of materials (#124).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ElectricalShoppingList {
    pub waste_pct: f64,
    pub has_conduits: bool,
    pub wires: Vec<WireShoppingRow>,
    pub total_wire_net_m: f64,
    pub total_wire_m: f64,
    pub conduits: Vec<ConduitShoppingRow>,
    pub total_conduit_net_m: f64,
    pub total_conduit_m: f64,
    pub boxes: Vec<BoxShoppingRow>,
    pub protections: Vec<ProtectionShoppingRow>,
    pub devices: Vec<DeviceShoppingRow>,
    pub warnings: Vec<String>,
    pub assumptions: Vec<String>,
}

fn section_key(section_mm2: f64) -> u32 {
    (section_mm2 * 10.0).round().max(1.0) as u32
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Rounds required DIN modules (with reserve) up to a standard Brazilian board size.
fn commercial_din_modules(required: usize) -> usize {
    const SIZES: [usize; 8] = [6, 8, 12, 16, 24, 36, 48, 60];
    SIZES
        .iter()
        .copied()
        .find(|&size| size >= required)
        .unwrap_or_else(|| required.div_ceil(12) * 12)
}

/// Standard commercial 30 mA RCD rated currents, in amperes.
fn commercial_rcd_rating(min_a: f64) -> f64 {
    const RATINGS: [f64; 6] = [25.0, 40.0, 63.0, 80.0, 100.0, 125.0];
    RATINGS
        .iter()
        .copied()
        .find(|&r| r + 1e-9 >= min_a)
        .unwrap_or(125.0)
}

/// Vertical drop length of a wall-mounted fixture along its connected run.
fn fixture_drop_m(wall_height: f64, fixture: &Fixture, run: ConduitRun) -> f64 {
    if !fixture.kind.mounts_on_wall() {
        return 0.0;
    }
    let height = fixture.electrical.map_or(0.0, |d| d.height_m);
    match run {
        ConduitRun::Ceiling => (wall_height - height).abs(),
        ConduitRun::Floor => height.max(0.0),
    }
}

/// Adds wire lengths for all conductors in `run.circuits` multiplied by `length_m`.
fn accumulate_run_wires(
    wires: &mut BTreeMap<(u32, WireColor), f64>,
    run: &ConduitWiring,
    length_m: f64,
) {
    if length_m <= 0.0 {
        return;
    }
    for cw in &run.circuits {
        let key = section_key(cw.section_mm2);
        if cw.phases > 0 {
            *wires.entry((key, WireColor::Phase)).or_insert(0.0) +=
                length_m * f64::from(cw.phases);
        }
        if cw.neutral {
            *wires.entry((key, WireColor::Neutral)).or_insert(0.0) += length_m;
        }
        if cw.returns > 0 {
            *wires.entry((key, WireColor::Return)).or_insert(0.0) +=
                length_m * f64::from(cw.returns);
        }
        if cw.earth {
            *wires.entry((key, WireColor::Earth)).or_insert(0.0) += length_m;
        }
    }
}

/// Computes the complete electrical shopping list for `project`.
pub fn electrical_shopping_list(project: &Project, waste_pct: f64) -> ElectricalShoppingList {
    let waste = if waste_pct.is_finite() {
        waste_pct.clamp(0.0, 50.0)
    } else {
        DEFAULT_ELECTRICAL_WASTE_PCT
    };
    let factor = 1.0 + waste / 100.0;
    let schedule = project.cached_load_schedule();
    let tree = project.conduit_tree();
    let wiring = project.conduit_wiring();
    let has_conduits = !wiring.is_empty();

    let mut list = ElectricalShoppingList {
        waste_pct: waste,
        has_conduits,
        ..ElectricalShoppingList::default()
    };

    let device_by_id: HashMap<u64, &Fixture> =
        project.electrical_devices().map(|f| (f.id, f)).collect();

    let mut wire_net: BTreeMap<(u32, WireColor), f64> = BTreeMap::new();
    let mut conduit_net: BTreeMap<(u32, u8), f64> = BTreeMap::new();
    let run_key = |run: ConduitRun| match run {
        ConduitRun::Ceiling => 0_u8,
        ConduitRun::Floor => 1_u8,
    };

    if has_conduits {
        for run in &wiring {
            let diam = run.diameter_mm.unwrap_or(20);
            let rk = run_key(run.run);
            // 1. Horizontal segment between upstream and downstream.
            *conduit_net.entry((diam, rk)).or_insert(0.0) += run.length_m;
            accumulate_run_wires(&mut wire_net, run, run.length_m);

            // 2. Vertical drop into downstream wall-mounted device (or sub-panel).
            if let Some(&down) = device_by_id.get(&run.downstream) {
                let drop_m = fixture_drop_m(project.wall_height, down, run.run);
                if drop_m > 0.0 {
                    *conduit_net.entry((diam, rk)).or_insert(0.0) += drop_m;
                    accumulate_run_wires(&mut wire_net, run, drop_m);
                }
                // Connection slack at the downstream device box (or panel).
                let slack = if down.kind == FixtureKind::Panel {
                    PANEL_SLACK_M
                } else {
                    DEVICE_SLACK_M
                };
                accumulate_run_wires(&mut wire_net, run, slack);
            }

            // 3. Vertical riser & dressing slack when upstream is a distribution board.
            if let Some(&up) = device_by_id.get(&run.upstream)
                && up.kind == FixtureKind::Panel
            {
                let riser_m = fixture_drop_m(project.wall_height, up, run.run);
                if riser_m > 0.0 {
                    *conduit_net.entry((diam, rk)).or_insert(0.0) += riser_m;
                    accumulate_run_wires(&mut wire_net, run, riser_m);
                }
                accumulate_run_wires(&mut wire_net, run, PANEL_SLACK_M);
            }
        }
        if !tree.unreached.is_empty() {
            list.warnings.push(format!(
                "{} ponto(s) ainda não têm eletroduto ligado ao quadro e ficaram fora da metragem de cabos e eletrodutos.",
                tree.unreached.len()
            ));
        }
    } else if !schedule.circuits.is_empty() {
        list.warnings.push(
            "Nenhum eletroduto lançado ainda: comprimentos de fios estimados pelo comprimento de cada circuito. Lance os eletrodutos para um levantamento exato."
                .to_owned(),
        );
        for row in &schedule.circuits {
            let section = row.wire_mm2.unwrap_or(2.5);
            let key = section_key(section);
            let len = row.length_m.unwrap_or(8.0) + PANEL_SLACK_M + DEVICE_SLACK_M;
            let phases = f64::from(row.poles.max(1));
            *wire_net.entry((key, WireColor::Phase)).or_insert(0.0) += len * phases;
            if row.poles < 2 {
                *wire_net.entry((key, WireColor::Neutral)).or_insert(0.0) += len;
            }
            *wire_net.entry((key, WireColor::Earth)).or_insert(0.0) += len;
            if row.kind == CircuitKind::Lighting {
                *wire_net.entry((key, WireColor::Return)).or_insert(0.0) += len * 0.5;
            }
            *conduit_net.entry((20, 0)).or_insert(0.0) += len;
        }
    }

    // 4. Sub-board feeders (QDG -> QDC): dedicated feeder cables & conduit.
    let supply_phases = schedule.supply.phases().max(1);
    for sub in &schedule.sub_boards {
        if sub.installed_va <= 0.0 {
            continue;
        }
        let Some(wire_mm2) = sub.wire_mm2 else {
            continue;
        };
        let length = sub.feeder_length_m.unwrap_or(10.0) + 2.0 * PANEL_SLACK_M;
        let sub_phases = sub
            .circuits
            .iter()
            .filter_map(|c| schedule.circuits.iter().find(|r| r.number == *c))
            .flat_map(|r| r.phases.iter().copied())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            .clamp(1, usize::from(supply_phases));
        let key = section_key(wire_mm2);
        *wire_net.entry((key, WireColor::Phase)).or_insert(0.0) += length * sub_phases as f64;
        *wire_net.entry((key, WireColor::Neutral)).or_insert(0.0) += length;
        let earth_mm2 = sub.earth_mm2.unwrap_or_else(|| earth_section(wire_mm2));
        *wire_net
            .entry((section_key(earth_mm2), WireColor::Earth))
            .or_insert(0.0) += length;
        if let Some(diam) = sub.conduit_mm {
            *conduit_net.entry((diam, 0)).or_insert(0.0) += length;
        }
    }

    // Materialize sorted wire rows.
    for ((sec_x10, color), net) in wire_net {
        if net <= 1e-6 {
            continue;
        }
        let section_mm2 = f64::from(sec_x10) / 10.0;
        let total = net * factor;
        let rolls = (total / 100.0).ceil().max(1.0) as u32;
        list.total_wire_net_m += net;
        list.total_wire_m += total;
        list.wires.push(WireShoppingRow {
            section_mm2,
            color,
            net_length_m: net,
            total_length_m: total,
            rolls_100m: rolls,
        });
    }

    // Materialize sorted conduit rows.
    for ((diameter_mm, rk), net) in conduit_net {
        if net <= 1e-6 {
            continue;
        }
        let run = if rk == 1 {
            ConduitRun::Floor
        } else {
            ConduitRun::Ceiling
        };
        let total = net * factor;
        let rolls = (total / 50.0).ceil().max(1.0) as u32;
        list.total_conduit_net_m += net;
        list.total_conduit_m += total;
        list.conduits.push(ConduitShoppingRow {
            diameter_mm,
            run,
            net_length_m: net,
            total_length_m: total,
            rolls_50m: rolls,
        });
    }

    // 5. Boxes & distribution enclosures.
    let mut wall_boxes_4x2 = 0usize;
    let mut ceiling_boxes_oct = 0usize;
    let mut outlet_10a = 0usize;
    let mut outlet_20a = 0usize;
    let mut switch_1way = 0usize;
    let mut switch_2way = 0usize;
    let mut switch_inter = 0usize;
    let mut ceiling_lights = 0usize;
    let mut wall_lights = 0usize;
    let mut data_points = 0usize;

    for dev in project.electrical_devices() {
        let power = dev.electrical.map_or(0.0, |d| d.power_va);
        match dev.kind {
            FixtureKind::OutletLow | FixtureKind::OutletMid | FixtureKind::OutletHigh => {
                wall_boxes_4x2 += 1;
                if power >= 600.0 - 1e-6 {
                    outlet_20a += 1;
                } else {
                    outlet_10a += 1;
                }
            }
            FixtureKind::OutletSpecific => {
                wall_boxes_4x2 += 1;
                outlet_20a += 1;
            }
            FixtureKind::Switch => {
                wall_boxes_4x2 += 1;
                switch_1way += 1;
            }
            FixtureKind::SwitchTwoWay => {
                wall_boxes_4x2 += 1;
                switch_2way += 1;
            }
            FixtureKind::SwitchIntermediate => {
                wall_boxes_4x2 += 1;
                switch_inter += 1;
            }
            FixtureKind::CeilingLight => {
                ceiling_boxes_oct += 1;
                ceiling_lights += 1;
            }
            FixtureKind::WallLight => {
                wall_boxes_4x2 += 1;
                wall_lights += 1;
            }
            FixtureKind::DataPoint => {
                wall_boxes_4x2 += 1;
                data_points += 1;
            }
            _ => {}
        }
    }

    if wall_boxes_4x2 > 0 {
        list.boxes.push(BoxShoppingRow {
            kind: "Caixa de embutir 4×2\"".to_owned(),
            specification: "PVC amarelo/preto para tomadas, interruptores, arandelas e dados"
                .to_owned(),
            quantity: wall_boxes_4x2,
        });
    }
    if ceiling_boxes_oct > 0 {
        list.boxes.push(BoxShoppingRow {
            kind: "Caixa octogonal 4×4\" (teto)".to_owned(),
            specification: "PVC reforçado com fundo móvel para pontos de luz na laje/forro"
                .to_owned(),
            quantity: ceiling_boxes_oct,
        });
    }

    // Distribution boards (QDG and any QDCs) with DIN module sizing (NBR 5410 §6.5.4.7).
    let panels = project.panels();
    if !panels.is_empty() {
        for (idx, panel) in panels.iter().enumerate() {
            let label = project
                .panel_label(panel.id)
                .unwrap_or_else(|| format!("QD{}", idx + 1));
            let is_main = idx == 0;
            let board_circuits: Vec<_> = schedule
                .circuits
                .iter()
                .filter(|c| {
                    if panels.len() == 1 {
                        true
                    } else {
                        c.board.as_deref().unwrap_or("QDG") == label
                    }
                })
                .collect();
            let circuit_poles: usize = board_circuits
                .iter()
                .map(|c| usize::from(c.poles.max(1)))
                .sum();
            let has_rcd = board_circuits.iter().any(|c| c.needs_rcd);
            let main_poles = usize::from(supply_phases);
            let dps_poles = if is_main { main_poles } else { 0 };
            let rcd_poles = if has_rcd {
                if supply_phases >= 2 { 4 } else { 2 }
            } else {
                0
            };
            let feeder_poles = if is_main {
                schedule
                    .sub_boards
                    .iter()
                    .filter(|s| s.installed_va > 0.0)
                    .count()
                    * main_poles
            } else {
                0
            };
            let used = main_poles + dps_poles + rcd_poles + feeder_poles + circuit_poles;
            let with_reserve = ((used as f64) * 1.30).ceil() as usize;
            let modules = commercial_din_modules(with_reserve.max(used + 2));
            list.boxes.push(BoxShoppingRow {
                kind: format!("Quadro de distribuição {label}"),
                specification: format!(
                    "Embutir para {modules} módulos DIN ({used} ocupados + reserva NBR 5410), com barramentos N e PE"
                ),
                quantity: 1,
            });
        }
    }

    // 6. Protection devices: main breaker, feeder breakers, terminal breakers, RCDs, DPS.
    if let Some(entrance) = schedule.entrance {
        if let Some(breaker) = entrance.breaker_a {
            let poles = entrance.supply.phases().max(1);
            list.protections.push(ProtectionShoppingRow {
                category: ProtectionCategory::MainBreaker,
                specification: format!("Disjuntor geral {poles}P {breaker:.0} A — Curva C"),
                poles,
                rating_a: Some(breaker),
                curve_or_class: "Curva C".to_owned(),
                quantity: 1,
                notes: format!(
                    "Proteção geral da entrada ({})",
                    entrance.supply.label().to_lowercase()
                ),
            });
        }
        // DPS Classe II (NBR 5410 §5.4.2 / §6.3.5.2).
        let dps_qty = usize::from(entrance.supply.phases().max(1));
        list.protections.push(ProtectionShoppingRow {
            category: ProtectionCategory::Dps,
            specification: format!(
                "DPS Classe II 275 V · 20/45 kA (1 por fase · {})",
                if project.electrical.voltage_v > 170.0 {
                    "275 V"
                } else {
                    "175/275 V"
                }
            ),
            poles: 1,
            rating_a: None,
            curve_or_class: "Classe II".to_owned(),
            quantity: dps_qty,
            notes: "Proteção contra surtos no QDG (NBR 5410 §6.3.5.2)".to_owned(),
        });
    }

    // Sub-board feeder breakers.
    for sub in &schedule.sub_boards {
        if let Some(breaker) = sub.breaker_a {
            let sub_phases = sub
                .circuits
                .iter()
                .filter_map(|c| schedule.circuits.iter().find(|r| r.number == *c))
                .flat_map(|r| r.phases.iter().copied())
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                .clamp(1, usize::from(supply_phases)) as u8;
            list.protections.push(ProtectionShoppingRow {
                category: ProtectionCategory::FeederBreaker,
                specification: format!(
                    "Disjuntor alimentador {} ({sub_phases}P {breaker:.0} A — Curva C)",
                    sub.label
                ),
                poles: sub_phases,
                rating_a: Some(breaker),
                curve_or_class: "Curva C".to_owned(),
                quantity: 2,
                notes: format!(
                    "1 no QDG (proteção do alimentador) + 1 seccionador geral no {}",
                    sub.label
                ),
            });
        }
    }

    // Terminal circuit breakers grouped by (poles, rating_a, curve).
    let mut breaker_groups: BTreeMap<(u8, u32, &'static str), Vec<u32>> = BTreeMap::new();
    for row in &schedule.circuits {
        if let Some(rating) = row.breaker_a {
            let curve = if row.kind == CircuitKind::Lighting {
                "Curva B"
            } else {
                "Curva C"
            };
            breaker_groups
                .entry((row.poles.max(1), rating.round() as u32, curve))
                .or_default()
                .push(row.number);
        }
    }
    for ((poles, rating, curve), circuits) in breaker_groups {
        let circuit_list = circuits
            .iter()
            .map(|c| format!("C{c}"))
            .collect::<Vec<_>>()
            .join(", ");
        list.protections.push(ProtectionShoppingRow {
            category: ProtectionCategory::CircuitBreaker,
            specification: format!("Disjuntor DIN {poles}P {rating} A — {curve}"),
            poles,
            rating_a: Some(f64::from(rating)),
            curve_or_class: curve.to_owned(),
            quantity: circuits.len(),
            notes: format!("Circuitos: {circuit_list}"),
        });
    }

    // 30 mA RCDs (Interruptor Diferencial Residual) grouped per board.
    let mut rcd_by_board: BTreeMap<String, Vec<&crate::domain::electrical::CircuitRow>> =
        BTreeMap::new();
    for row in &schedule.circuits {
        if row.needs_rcd {
            let board_name = row.board.clone().unwrap_or_else(|| "QD".to_owned());
            rcd_by_board.entry(board_name).or_default().push(row);
        }
    }
    for (board_name, rcd_circuits) in rcd_by_board {
        let distinct_phases: std::collections::BTreeSet<u8> = rcd_circuits
            .iter()
            .flat_map(|c| c.phases.iter().copied())
            .collect();
        let rcd_poles: u8 = if distinct_phases.len() >= 2 || schedule.supply != Supply::Single {
            4
        } else {
            2
        };
        // NBR 5410: RCD rated current >= upstream main breaker or sum of protected circuits.
        let sum_protected: f64 = rcd_circuits.iter().filter_map(|c| c.breaker_a).sum();
        let upstream_a = schedule
            .entrance
            .and_then(|e| e.breaker_a)
            .unwrap_or(sum_protected);
        let min_rcd = sum_protected.min(upstream_a).max(25.0);
        let rcd_a = commercial_rcd_rating(min_rcd);
        let names = rcd_circuits
            .iter()
            .map(|c| format!("C{}", c.number))
            .collect::<Vec<_>>()
            .join(", ");
        list.protections.push(ProtectionShoppingRow {
            category: ProtectionCategory::Rcd,
            specification: format!("Interruptor DR {rcd_poles}P {rcd_a:.0} A — 30 mA (Tipo AC/A)"),
            poles: rcd_poles,
            rating_a: Some(rcd_a),
            curve_or_class: "30 mA".to_owned(),
            quantity: 1,
            notes: format!(
                "Quadro {board_name}: protege {names} (áreas molhadas/externas, NBR 5410 §5.1.3.2.2)"
            ),
        });
    }

    // 7. Finishing devices.
    let mut push_dev = |spec: &str, qty: usize| {
        if qty > 0 {
            list.devices.push(DeviceShoppingRow {
                specification: spec.to_owned(),
                quantity: qty,
            });
        }
    };
    push_dev("Tomada 2P+T 10 A (NBR 14136) com espelho 4×2\"", outlet_10a);
    push_dev(
        "Tomada 2P+T 20 A (NBR 14136) para cozinha/serviço/TUE com espelho 4×2\"",
        outlet_20a,
    );
    push_dev("Interruptor simples 10 A com espelho 4×2\"", switch_1way);
    push_dev(
        "Interruptor paralelo (three-way) 10 A com espelho 4×2\"",
        switch_2way,
    );
    push_dev(
        "Interruptor intermediário (four-way) 10 A com espelho 4×2\"",
        switch_inter,
    );
    push_dev("Ponto de luz no teto (plafon / luminária LED)", ceiling_lights);
    push_dev("Arandela de parede (ponto de luz)", wall_lights);
    push_dev("Tomada de dados / TV (RJ45 ou coaxial) com espelho 4×2\"", data_points);

    list.assumptions = vec![
        format!(
            "Perda/sobra aplicada sobre fios e eletrodutos: {:.0} % (folga de {:.0} cm por caixa e {:.0} cm no quadro).",
            waste,
            DEVICE_SLACK_M * 100.0,
            PANEL_SLACK_M * 100.0
        ),
        "Cores dos condutores conforme ABNT NBR 5410 §6.1.5.3.2: neutro azul-claro, terra PE verde ou verde-amarelo, fase preto/vermelho/branco e retorno amarelo/cinza.".to_owned(),
        "Cabos flexíveis de cobre isolação PVC 750 V (NBR NM 247-3); eletrodutos corrugados flexíveis em PVC (teto/parede) ou reforçados (piso).".to_owned(),
        "Ramal de entrada da concessionária (poste/medidor até o QDG) não está somado na metragem interna de cabos: confirme o trajeto no lote com o eletricista.".to_owned(),
        "Pré-dimensionamento para orçamento e compras: revise com um profissional habilitado antes da execução.".to_owned(),
    ];

    list
}

/// Schedule table version of the electrical shopping list for drawing sheets.
pub fn build_electrical_materials_schedule(project: &Project) -> ScheduleTable {
    let list = electrical_shopping_list(project, DEFAULT_ELECTRICAL_WASTE_PCT);
    let mut rows = Vec::new();
    for w in &list.wires {
        rows.push(vec![
            "Cabo cobre 750 V".to_owned(),
            format!("{} mm² · {}", section_label(w.section_mm2), w.color.color_name()),
            format!("{} m", decimal(w.net_length_m, 1)),
            format!("{} m ({} rolo(s))", decimal(w.total_length_m, 1), w.rolls_100m),
        ]);
    }
    for c in &list.conduits {
        rows.push(vec![
            "Eletroduto".to_owned(),
            format!("Ø{} mm ({})", c.diameter_mm, c.run.label().to_lowercase()),
            format!("{} m", decimal(c.net_length_m, 1)),
            format!("{} m ({} rolo(s))", decimal(c.total_length_m, 1), c.rolls_50m),
        ]);
    }
    for b in &list.boxes {
        rows.push(vec![
            "Caixa / Quadro".to_owned(),
            b.kind.clone(),
            format!("{} un", b.quantity),
            format!("{} un", b.quantity),
        ]);
    }
    for p in &list.protections {
        rows.push(vec![
            p.category.label().to_owned(),
            p.specification.clone(),
            format!("{} un", p.quantity),
            format!("{} un", p.quantity),
        ]);
    }
    ScheduleTable {
        title: format!(
            "Lista de materiais elétricos (NBR 5410 · perda {:.0} %)",
            list.waste_pct
        ),
        headers: ["Grupo", "Item / Especificação", "Líquido", "Compra"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        rows,
        footer: Some(vec![
            "Totais".to_owned(),
            "Cabos e eletrodutos".to_owned(),
            format!(
                "{} m fios · {} m eletrod.",
                decimal(list.total_wire_net_m, 1),
                decimal(list.total_conduit_net_m, 1)
            ),
            format!(
                "{} m fios · {} m eletrod.",
                decimal(list.total_wire_m, 1),
                decimal(list.total_conduit_m, 1)
            ),
        ]),
    }
}

fn csv_field(value: &str) -> String {
    if value
        .chars()
        .any(|c| matches!(c, ',' | '"' | '\r' | '\n'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// UTF-8 BOM CSV export of the electrical shopping list.
pub fn electrical_shopping_list_csv(project: &Project, waste_pct: f64) -> String {
    let list = electrical_shopping_list(project, waste_pct);
    let mut csv = String::from(
        "\u{feff}grupo,item,cor_ou_curva,secao_ou_diametro,liquido,com_perda,unidade,embalagens,observacao\r\n",
    );
    for w in &list.wires {
        let _ = writeln!(
            csv,
            "cabo,Cabo flexível cobre PVC 750 V,{},{} mm2,{:.2},{:.2},m,{} rolo(s) 100 m,{}\r",
            csv_field(w.color.color_name()),
            w.section_mm2,
            w.net_length_m,
            w.total_length_m,
            w.rolls_100m,
            csv_field(w.color.label())
        );
    }
    for c in &list.conduits {
        let _ = writeln!(
            csv,
            "eletroduto,Eletroduto PVC ({}),{},{} mm,{:.2},{:.2},m,{} rolo(s) 50 m,\r",
            csv_field(c.run.label()),
            if c.run == ConduitRun::Floor {
                "Reforçado/rígido"
            } else {
                "Corrugado amarelo"
            },
            c.diameter_mm,
            c.net_length_m,
            c.total_length_m,
            c.rolls_50m
        );
    }
    for b in &list.boxes {
        let _ = writeln!(
            csv,
            "caixa,{},,,{},{},un,,{}\r",
            csv_field(&b.kind),
            b.quantity,
            b.quantity,
            csv_field(&b.specification)
        );
    }
    for p in &list.protections {
        let _ = writeln!(
            csv,
            "protecao,{},{},{},{},{},un,,{}\r",
            csv_field(&p.specification),
            csv_field(&p.curve_or_class),
            p.rating_a
                .map_or(String::new(), |a| format!("{a:.0} A")),
            p.quantity,
            p.quantity,
            csv_field(&p.notes)
        );
    }
    for d in &list.devices {
        let _ = writeln!(
            csv,
            "acabamento,{},,,{},{},un,,\r",
            csv_field(&d.specification),
            d.quantity,
            d.quantity
        );
    }
    csv
}

/// Printable A3 landscape SVG sheet of the electrical shopping list.
pub fn electrical_shopping_list_svg(project: &Project, waste_pct: f64) -> String {
    let list = electrical_shopping_list(project, waste_pct);
    let width = 420.0;
    let total_rows = list.wires.len()
        + list.conduits.len()
        + list.boxes.len()
        + list.protections.len()
        + list.devices.len();
    let height = (90.0 + total_rows as f64 * 5.6 + 65.0).max(297.0);
    let ink = "#1d2836";
    let muted = "#4a5d73";
    let font = "font-family=\"Arial, sans-serif\"";

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height:.0}mm\" viewBox=\"0 0 {width} {height:.0}\">\n\
         <rect width=\"{width}\" height=\"{height:.0}\" fill=\"#ffffff\"/>\n\
         <rect x=\"10\" y=\"10\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>\n",
        width - 20.0,
        height - 20.0
    );

    let text = |svg: &mut String, x: f64, y: f64, size: f64, bold: bool, color: &str, value: &str| {
        let _ = writeln!(
            svg,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" {font} font-size=\"{size}\"{} fill=\"{color}\">{}</text>",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        );
    };

    text(
        &mut svg,
        18.0,
        22.0,
        5.0,
        true,
        ink,
        "LISTA DE MATERIAIS ELÉTRICOS (ABNT NBR 5410)",
    );
    text(
        &mut svg,
        18.0,
        28.5,
        3.0,
        false,
        muted,
        &format!(
            "{} · Perda considerada: {:.0} % · Total de fios: {} m (líquido {} m) · Eletrodutos: {} m",
            project.name,
            list.waste_pct,
            decimal(list.total_wire_m, 1),
            decimal(list.total_wire_net_m, 1),
            decimal(list.total_conduit_m, 1)
        ),
    );

    // Two-column layout on A3: left = wires + conduits, right = boxes + protection + devices.
    let mut ly = 38.0;
    let draw_section_title = |svg: &mut String, x: f64, y: &mut f64, title: &str| {
        text(svg, x, *y, 3.4, true, ink, title);
        *y += 2.2;
        let _ = writeln!(
            svg,
            "<line x1=\"{x:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{ink}\" stroke-width=\"0.35\"/>",
            *y,
            x + 184.0,
            *y
        );
        *y += 4.5;
    };

    draw_section_title(&mut svg, 18.0, &mut ly, "1. Cabos de cobre PVC 750 V (por bitola e cor)");
    if list.wires.is_empty() {
        text(&mut svg, 18.0, ly, 2.6, false, muted, "Nenhum circuito lançado.");
        ly += 6.0;
    } else {
        text(&mut svg, 18.0, ly, 2.5, true, muted, "Bitola · Cor (NBR 5410)");
        text(&mut svg, 118.0, ly, 2.5, true, muted, "Líquido");
        text(&mut svg, 146.0, ly, 2.5, true, muted, "Com perda");
        text(&mut svg, 178.0, ly, 2.5, true, muted, "Rolos 100 m");
        ly += 4.8;
        for w in &list.wires {
            text(
                &mut svg,
                18.0,
                ly,
                2.5,
                false,
                ink,
                &format!("{} mm² — {}", section_label(w.section_mm2), w.color.label()),
            );
            text(&mut svg, 118.0, ly, 2.5, false, ink, &format!("{} m", decimal(w.net_length_m, 1)));
            text(&mut svg, 146.0, ly, 2.5, true, ink, &format!("{} m", decimal(w.total_length_m, 1)));
            text(&mut svg, 178.0, ly, 2.5, false, muted, &format!("{} rolo(s)", w.rolls_100m));
            ly += 4.8;
        }
    }

    ly += 4.0;
    draw_section_title(&mut svg, 18.0, &mut ly, "2. Eletrodutos (por diâmetro nominal)");
    if list.conduits.is_empty() {
        text(&mut svg, 18.0, ly, 2.6, false, muted, "Nenhum eletroduto lançado.");
        ly += 6.0;
    } else {
        for c in &list.conduits {
            text(
                &mut svg,
                18.0,
                ly,
                2.5,
                false,
                ink,
                &format!("Ø{} mm ({})", c.diameter_mm, c.run.label()),
            );
            text(&mut svg, 118.0, ly, 2.5, false, ink, &format!("{} m", decimal(c.net_length_m, 1)));
            text(&mut svg, 146.0, ly, 2.5, true, ink, &format!("{} m", decimal(c.total_length_m, 1)));
            text(&mut svg, 178.0, ly, 2.5, false, muted, &format!("{} rolo(s) 50 m", c.rolls_50m));
            ly += 4.8;
        }
    }

    let mut ry = 38.0;
    draw_section_title(&mut svg, 218.0, &mut ry, "3. Caixas e quadros de distribuição");
    for b in &list.boxes {
        text(
            &mut svg,
            218.0,
            ry,
            2.5,
            true,
            ink,
            &format!("{}× {}", b.quantity, b.kind),
        );
        ry += 3.8;
        text(&mut svg, 222.0, ry, 2.2, false, muted, &b.specification);
        ry += 4.6;
    }

    ry += 3.0;
    draw_section_title(&mut svg, 218.0, &mut ry, "4. Disjuntores, DR (30 mA) e DPS");
    for p in &list.protections {
        text(
            &mut svg,
            218.0,
            ry,
            2.5,
            true,
            ink,
            &format!("{}× {}", p.quantity, p.specification),
        );
        ry += 3.8;
        if !p.notes.is_empty() {
            text(&mut svg, 222.0, ry, 2.2, false, muted, &p.notes);
            ry += 4.4;
        }
    }

    ry += 3.0;
    draw_section_title(&mut svg, 218.0, &mut ry, "5. Pontos e acabamentos");
    for d in &list.devices {
        text(
            &mut svg,
            218.0,
            ry,
            2.5,
            false,
            ink,
            &format!("{}× {}", d.quantity, d.specification),
        );
        ry += 4.6;
    }

    let mut footer_y = ly.max(ry) + 6.0;
    if footer_y < height - 28.0 {
        footer_y = height - 28.0;
    }
    for note in &list.assumptions {
        text(&mut svg, 18.0, footer_y, 2.2, false, muted, note);
        footer_y += 3.8;
    }

    svg.push_str("</svg>\n");
    svg
}

impl Project {
    /// Complete electrical shopping list (#124) with configurable waste percentage.
    pub fn electrical_shopping_list(&self, waste_pct: f64) -> ElectricalShoppingList {
        electrical_shopping_list(self, waste_pct)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{geometry::Point, samples::SampleProject};

    /// Verifies #124 and M7 criterion: the electrical shopping list matches an
    /// independent manual takeoff of the sample house within 5 %.
    #[test]
    fn sample_house_shopping_list_matches_manual_takeoff_within_five_percent() {
        let mut project = SampleProject::CasaTerrea.build();
        project.generate_electrical_points();
        let (pw, pd) = FixtureKind::Panel.default_size();
        let first_room = project.rooms[0].clone();
        let (rmin, _) = project.room_bounds(&first_room).unwrap();
        project
            .add_fixture(
                FixtureKind::Panel,
                Point::new(rmin.x + 0.35, rmin.y + 0.15),
                pw,
                pd,
                0,
            )
            .unwrap();
        project.assign_circuits();
        project.generate_conduits().unwrap();

        let list = project.electrical_shopping_list(10.0);
        assert!(list.has_conduits);
        assert!(list.warnings.is_empty(), "warnings: {:?}", list.warnings);

        // Independent manual takeoff directly from conduits + drops + slack.
        let wiring = project.conduit_wiring();
        let devices: HashMap<u64, &Fixture> =
            project.electrical_devices().map(|f| (f.id, f)).collect();
        let mut manual_wire_net = 0.0;
        let mut manual_conduit_net = 0.0;
        for run in &wiring {
            let cond_count = run.conductor_count() as f64;
            manual_conduit_net += run.length_m;
            manual_wire_net += run.length_m * cond_count;
            if let Some(&down) = devices.get(&run.downstream) {
                let drop = fixture_drop_m(project.wall_height, down, run.run);
                manual_conduit_net += drop;
                manual_wire_net += drop * cond_count;
                let slack = if down.kind == FixtureKind::Panel {
                    PANEL_SLACK_M
                } else {
                    DEVICE_SLACK_M
                };
                manual_wire_net += slack * cond_count;
            }
            if let Some(&up) = devices.get(&run.upstream)
                && up.kind == FixtureKind::Panel
            {
                let riser = fixture_drop_m(project.wall_height, up, run.run);
                manual_conduit_net += riser;
                manual_wire_net += riser * cond_count;
                manual_wire_net += PANEL_SLACK_M * cond_count;
            }
        }

        let wire_err = (list.total_wire_net_m - manual_wire_net).abs() / manual_wire_net.max(1.0);
        let conduit_err =
            (list.total_conduit_net_m - manual_conduit_net).abs() / manual_conduit_net.max(1.0);
        assert!(
            wire_err < 0.05,
            "wire takeoff mismatch: list={} manual={}",
            list.total_wire_net_m,
            manual_wire_net
        );
        assert!(
            conduit_err < 0.05,
            "conduit takeoff mismatch: list={} manual={}",
            list.total_conduit_net_m,
            manual_conduit_net
        );
        // Must separate all 4 NBR 5410 wire colours.
        for color in [
            WireColor::Phase,
            WireColor::Neutral,
            WireColor::Return,
            WireColor::Earth,
        ] {
            assert!(
                list.wires.iter().any(|w| w.color == color && w.net_length_m > 0.0),
                "missing wire color {color:?}"
            );
        }
        // Boxes, breakers, RCDs and DPS must all be present.
        assert!(list.boxes.iter().any(|b| b.kind.contains("4×2")));
        assert!(list.boxes.iter().any(|b| b.kind.contains("octogonal")));
        assert!(list.boxes.iter().any(|b| b.kind.contains("Quadro")));
        assert!(
            list.protections
                .iter()
                .any(|p| p.category == ProtectionCategory::MainBreaker)
        );
        assert!(
            list.protections
                .iter()
                .any(|p| p.category == ProtectionCategory::CircuitBreaker)
        );
        assert!(
            list.protections
                .iter()
                .any(|p| p.category == ProtectionCategory::Rcd)
        );
        assert!(
            list.protections
                .iter()
                .any(|p| p.category == ProtectionCategory::Dps)
        );
        // CSV and SVG exports are valid and complete.
        let csv = electrical_shopping_list_csv(&project, 10.0);
        assert!(csv.starts_with("\u{feff}grupo,item"));
        assert!(csv.contains("Azul-claro"));
        assert!(csv.contains("Verde/Amarelo"));
        let svg = electrical_shopping_list_svg(&project, 10.0);
        assert!(svg.contains("LISTA DE MATERIAIS ELÉTRICOS"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn single_room_exact_hand_calculation_for_wires_boxes_and_breakers() {
        let mut project = Project::default();
        project.wall_height = 2.8;
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(15.0, 15.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Quarto")
            .unwrap();
        let mut add = |kind: FixtureKind, x: f64, y: f64, circuit: Option<u32>| {
            let (w, d) = kind.default_size();
            let id = project
                .add_fixture(kind, Point::new(x, y), w, d, 0)
                .unwrap();
            if let Some(c) = circuit {
                project
                    .update_electrical(id, Some(Some(c)), None, None)
                    .unwrap();
            }
            id
        };
        let panel = add(FixtureKind::Panel, 0.5, 0.5, None);
        let light = add(FixtureKind::CeilingLight, 3.5, 0.5, Some(1));
        let sw = add(FixtureKind::Switch, 3.5, 2.5, Some(1));
        let outlet = add(FixtureKind::OutletLow, 3.5, 3.5, Some(2));
        // Straight distances: panel->light = 3.0 m, light->sw = 2.0 m, light->outlet = 3.0 m.
        project.add_conduit(panel, light, ConduitRun::Ceiling).unwrap();
        project.add_conduit(light, sw, ConduitRun::Ceiling).unwrap();
        project.add_conduit(light, outlet, ConduitRun::Ceiling).unwrap();

        let list = project.electrical_shopping_list(0.0);
        // Panel riser = 2.8 - 1.5 = 1.3 m; switch drop = 2.8 - 1.1 = 1.7 m; outlet drop = 2.8 - 0.3 = 2.5 m.
        // Circuit 1 (1.5 mm²):
        //   panel->light carries F, N, T: horiz 3.0 + riser 1.3 + panel_slack 0.5 + light_slack 0.25 = 5.05 m each
        //   light->sw carries F, R: horiz 2.0 + drop 1.7 + sw_slack 0.25 = 3.95 m each
        //   Total 1.5 mm² Phase = 5.05 + 3.95 = 9.00 m
        //   Total 1.5 mm² Neutral = 5.05 m
        //   Total 1.5 mm² Return = 3.95 m
        //   Total 1.5 mm² Earth = 5.05 m
        let find_wire = |sec: f64, color: WireColor| {
            list.wires
                .iter()
                .find(|w| (w.section_mm2 - sec).abs() < 1e-6 && w.color == color)
                .map(|w| w.net_length_m)
                .unwrap_or(0.0)
        };
        assert!((find_wire(1.5, WireColor::Phase) - 9.00).abs() < 1e-6);
        assert!((find_wire(1.5, WireColor::Neutral) - 5.05).abs() < 1e-6);
        assert!((find_wire(1.5, WireColor::Return) - 3.95).abs() < 1e-6);
        assert!((find_wire(1.5, WireColor::Earth) - 5.05).abs() < 1e-6);
        // Circuit 2 (2.5 mm²):
        //   panel->light carries F, N, T: 5.05 m each
        //   light->outlet carries F, N, T: horiz 3.0 + drop 2.5 + slack 0.25 = 5.75 m each
        //   Total 2.5 mm² Phase = Neutral = Earth = 10.80 m
        assert!((find_wire(2.5, WireColor::Phase) - 10.80).abs() < 1e-6);
        assert!((find_wire(2.5, WireColor::Neutral) - 10.80).abs() < 1e-6);
        assert!((find_wire(2.5, WireColor::Earth) - 10.80).abs() < 1e-6);
        // Conduits: (3.0 + 1.3) + (2.0 + 1.7) + (3.0 + 2.5) = 13.5 m
        assert!((list.total_conduit_net_m - 13.5).abs() < 1e-6);
    }
}
