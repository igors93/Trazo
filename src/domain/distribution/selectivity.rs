//! Multi-board topology, simplified IEC 60909 / ABNT NBR 5410 §5.3.5
//! prospective short-circuit current (`Icc`) estimate, cumulative feeder +
//! terminal voltage drop and breaker selectivity coordination (#127, B09).

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::domain::{
    distribution::{MAX_DROP_PERCENT, Supply, section_label},
    electrical::{CircuitKind, LoadSchedule},
    model::Project,
    verdict::Light,
};

/// IEC 60909 maximum short-circuit voltage factor for LV systems.
const C_MAX: f64 = 1.05;
/// IEC 60909 minimum short-circuit voltage factor for LV systems.
const C_MIN: f64 = 0.95;
/// Copper resistivity at 20 °C (cold conductor, max fault current), Ω·mm²/m.
const RHO_CU_20: f64 = 0.0185;
/// Copper resistivity at 70 °C (hot conductor, min end-of-line fault), Ω·mm²/m.
const RHO_CU_70: f64 = RHO_CU_20 * 1.20;
/// Typical LV cable inductive reactance per metre, Ω/m (0.08 mΩ/m).
const X_CABLE_OHM_M: f64 = 0.00008;
/// NBR 5410 Table 30 thermal constant `k` for PVC-insulated copper, A·√s/mm².
const K_PVC_CU: f64 = 115.0;
/// Assumed DIN MCB current-limiting clearing time for thermal check, s (10 ms).
const MCB_CLEAR_TIME_S: f64 = 0.01;

/// Standard DIN miniature circuit breaker tripping curves (ABNT NBR NM 60898).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakerCurve {
    /// 3–5 × In (lighting and long resistive circuits).
    B,
    /// 5–10 × In (general outlets, motors, residential default).
    #[default]
    C,
    /// 10–20 × In (high inrush loads).
    D,
}

impl BreakerCurve {
    pub fn label(self) -> &'static str {
        match self {
            Self::B => "Curva B",
            Self::C => "Curva C",
            Self::D => "Curva D",
        }
    }

    pub fn mag_min_mult(self) -> f64 {
        match self {
            Self::B => 3.0,
            Self::C => 5.0,
            Self::D => 10.0,
        }
    }

    pub fn mag_max_mult(self) -> f64 {
        match self {
            Self::B => 5.0,
            Self::C => 10.0,
            Self::D => 20.0,
        }
    }
}

fn default_transformer_kva() -> f64 {
    75.0
}
fn default_transformer_z_pct() -> f64 {
    3.5
}
fn default_service_drop_m() -> f64 {
    25.0
}
fn default_service_drop_mm2() -> f64 {
    35.0
}
fn default_entrance_length_m() -> f64 {
    10.0
}
fn default_breaker_icn_ka() -> f64 {
    3.0
}

/// Source impedance & breaker rating settings persisted inside `ElectricalSettings` (`Copy`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortCircuitSettings {
    /// Utility pole/pad transformer rated power, kVA (typical 45–300 kVA).
    #[serde(default = "default_transformer_kva")]
    pub transformer_kva: f64,
    /// Transformer percentage impedance `Z%` (ABNT NBR 5440, typical 3.5 %).
    #[serde(default = "default_transformer_z_pct")]
    pub transformer_z_pct: f64,
    /// Distance from utility transformer to the meter box, m.
    #[serde(default = "default_service_drop_m")]
    pub service_drop_length_m: f64,
    /// Equivalent copper section of the utility LV line / service drop, mm².
    #[serde(default = "default_service_drop_mm2")]
    pub service_drop_section_mm2: f64,
    /// Entrance feeder length from the meter box to the main board (QDG), m.
    #[serde(default = "default_entrance_length_m")]
    pub entrance_length_m: f64,
    /// Rated short-circuit breaking capacity `Icn` of installed DIN MCBs, kA (3, 4.5, 6, 10).
    #[serde(default = "default_breaker_icn_ka")]
    pub breaker_icn_ka: f64,
}

impl Default for ShortCircuitSettings {
    fn default() -> Self {
        Self {
            transformer_kva: default_transformer_kva(),
            transformer_z_pct: default_transformer_z_pct(),
            service_drop_length_m: default_service_drop_m(),
            service_drop_section_mm2: default_service_drop_mm2(),
            entrance_length_m: default_entrance_length_m(),
            breaker_icn_ka: default_breaker_icn_ka(),
        }
    }
}

impl ShortCircuitSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Complex impedance `(R, X)` in ohms.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Impedance {
    r: f64,
    x: f64,
}

impl Impedance {
    fn add(self, other: Self) -> Self {
        Self {
            r: self.r + other.r,
            x: self.x + other.x,
        }
    }

    fn scale(self, k: f64) -> Self {
        Self {
            r: self.r * k,
            x: self.x * k,
        }
    }

    fn mag(self) -> f64 {
        self.r.hypot(self.x)
    }
}

fn cable_impedance(length_m: f64, section_mm2: f64, rho: f64) -> Impedance {
    let l = length_m.max(0.0);
    let s = section_mm2.max(0.5);
    Impedance {
        r: rho * l / s,
        x: X_CABLE_OHM_M * l,
    }
}

fn transformer_impedance(kva: f64, z_pct: f64, v_ll: f64) -> Impedance {
    let s_va = (kva.max(15.0)) * 1000.0;
    let z = (z_pct.clamp(1.5, 10.0) / 100.0) * v_ll * v_ll / s_va;
    let xr = if kva <= 112.5 { 1.5 } else { 2.5 };
    let r = z / (1.0 + xr * xr).sqrt();
    Impedance { r, x: r * xr }
}

/// Smallest standard commercial DIN MCB breaking capacity `Icn` ≥ `icc_ka`.
pub fn recommended_icn_ka(icc_ka: f64) -> f64 {
    const STEPS: [f64; 5] = [3.0, 4.5, 6.0, 10.0, 15.0];
    STEPS
        .iter()
        .copied()
        .find(|&s| s + 1e-9 >= icc_ka)
        .unwrap_or(15.0)
}

/// Short-circuit estimate at one distribution board (`QDG` or `QDCn`).
#[derive(Clone, Debug, PartialEq)]
pub struct BoardShortCircuit {
    pub label: String,
    pub is_main: bool,
    pub room: Option<String>,
    pub upstream_board: Option<String>,
    pub feeder_length_m: f64,
    pub feeder_wire_mm2: Option<f64>,
    pub feeder_drop_percent: f64,
    pub main_breaker_a: Option<f64>,
    pub circuits: Vec<u32>,
    pub installed_va: f64,
    pub demand_va: f64,
    /// Positive-sequence resistance/reactance to this board, mΩ.
    pub r_mohm: f64,
    pub x_mohm: f64,
    /// Prospective 3-phase symmetrical short-circuit current, kA.
    pub icc_3p_ka: f64,
    /// Prospective phase-neutral short-circuit current, kA.
    pub icc_1p_ka: f64,
    /// Governing maximum short-circuit current at this board's busbar, kA.
    pub icc_max_ka: f64,
    pub installed_icn_ka: f64,
    pub recommended_icn_ka: f64,
    pub light: Light,
    pub note: String,
}

/// End-of-line short-circuit, cable withstand & cumulative drop check for one circuit.
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitFaultCheck {
    pub circuit: u32,
    pub board: String,
    pub breaker_a: Option<f64>,
    pub curve: BreakerCurve,
    pub wire_mm2: Option<f64>,
    /// Cumulative voltage drop (sub-board feeder + terminal circuit), %.
    pub total_drop_percent: Option<f64>,
    /// Max short-circuit at circuit breaker terminals, kA.
    pub icc_board_ka: f64,
    /// Minimum short-circuit at farthest point (hot conductor 70 °C), A.
    pub icc_min_end_a: f64,
    /// Breaker instantaneous magnetic trip upper bound (`Im_max`), A.
    pub mag_trip_max_a: f64,
    /// Cable adiabatic thermal withstand at 10 ms (`k·S / √t`), kA.
    pub cable_thermal_ka: f64,
    pub magnetic_trip_ok: bool,
    pub cable_thermal_ok: bool,
    pub total_drop_ok: bool,
    pub light: Light,
}

/// Selectivity classification between an upstream and downstream breaker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectivityStatus {
    /// Selective for both overloads and the full prospective fault current.
    Total,
    /// Selective for overloads and short-circuits up to `is_ka` (normal for DIN MCBs).
    Partial,
    /// Overload selectivity ratio is tight (`1.25 <= ratio < 1.6`).
    Tight,
    /// No selectivity (`In_up <= In_down` or `ratio < 1.25`): upstream trips with downstream.
    None,
}

impl SelectivityStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Total => "Seletividade total",
            Self::Partial => "Seletividade parcial",
            Self::Tight => "Margem apertada",
            Self::None => "Sem seletividade",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Total => "total",
            Self::Partial => "parcial",
            Self::Tight => "apertada",
            Self::None => "nenhuma",
        }
    }
}

/// Coordination check between one upstream and one downstream breaker.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectivityPair {
    pub upstream_label: String,
    pub upstream_in_a: f64,
    pub upstream_curve: BreakerCurve,
    pub downstream_label: String,
    pub downstream_in_a: f64,
    pub downstream_curve: BreakerCurve,
    pub ratio: f64,
    /// Magnetic selectivity limit `Is` (kA) where upstream instantaneous trip begins.
    pub selectivity_limit_ka: f64,
    /// Prospective short-circuit current at downstream breaker, kA.
    pub icc_downstream_ka: f64,
    pub status: SelectivityStatus,
    pub light: Light,
    pub explanation: String,
}

/// Full multi-board, short-circuit and breaker selectivity study (#127).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShortCircuitReport {
    pub settings: ShortCircuitSettings,
    pub supply: Supply,
    pub boards: Vec<BoardShortCircuit>,
    pub circuits: Vec<CircuitFaultCheck>,
    pub selectivity: Vec<SelectivityPair>,
    pub max_icc_ka: f64,
    pub min_recommended_icn_ka: f64,
    pub light: Light,
    pub warnings: Vec<String>,
    pub assumptions: Vec<String>,
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

fn evaluate_selectivity(
    upstream_label: String,
    upstream_in_a: f64,
    upstream_curve: BreakerCurve,
    downstream_label: String,
    downstream_in_a: f64,
    downstream_curve: BreakerCurve,
    icc_downstream_ka: f64,
) -> SelectivityPair {
    let ratio = upstream_in_a / downstream_in_a.max(1.0);
    let is_ka = (upstream_in_a * upstream_curve.mag_min_mult()) / 1000.0;
    let (status, light, explanation) = if upstream_in_a <= downstream_in_a + 1e-6 || ratio < 1.25 {
        (
            SelectivityStatus::None,
            Light::Red,
            format!(
                "{upstream_label} ({upstream_in_a:.0} A) e {downstream_label} ({downstream_in_a:.0} A) têm razão {} (< 1,25): sobrecarga ou curto desarmam o quadro acima.",
                decimal(ratio, 2)
            ),
        )
    } else if ratio < 1.60 {
        (
            SelectivityStatus::Tight,
            Light::Yellow,
            format!(
                "Razão {} (entre 1,25 e 1,60) entre {upstream_label} ({upstream_in_a:.0} A) e {downstream_label} ({downstream_in_a:.0} A): seletividade térmica apertada.",
                decimal(ratio, 2)
            ),
        )
    } else if icc_downstream_ka <= is_ka + 1e-6 {
        (
            SelectivityStatus::Total,
            Light::Green,
            format!(
                "Seletividade total: {downstream_label} ({downstream_in_a:.0} A) atua antes de {upstream_label} ({upstream_in_a:.0} A) até o curto máximo de {} kA.",
                decimal(icc_downstream_ka, 2)
            ),
        )
    } else {
        (
            SelectivityStatus::Partial,
            Light::Green,
            format!(
                "Seletividade térmica garantida (razão {}) e magnética até Is = {} kA (curto local {} kA); acima de Is ambos podem atuar.",
                decimal(ratio, 2),
                decimal(is_ka, 2),
                decimal(icc_downstream_ka, 2)
            ),
        )
    };
    SelectivityPair {
        upstream_label,
        upstream_in_a,
        upstream_curve,
        downstream_label,
        downstream_in_a,
        downstream_curve,
        ratio,
        selectivity_limit_ka: is_ka,
        icc_downstream_ka,
        status,
        light,
        explanation,
    }
}

/// Computes the multi-board, short-circuit (`Icc`) and selectivity report (#127).
pub fn short_circuit_report(project: &Project, schedule: &LoadSchedule) -> ShortCircuitReport {
    let settings = project.electrical.short_circuit;
    let v_fn = project.electrical.voltage_v.max(1.0);
    let v_ll = v_fn * 3.0_f64.sqrt();
    let supply = schedule.supply;
    let is_single = supply == Supply::Single;

    let mut report = ShortCircuitReport {
        settings,
        supply,
        light: Light::Green,
        ..ShortCircuitReport::default()
    };

    if schedule.circuits.is_empty() {
        return report;
    }

    let z_tr = transformer_impedance(settings.transformer_kva, settings.transformer_z_pct, v_ll);
    let z_drop = cable_impedance(
        settings.service_drop_length_m,
        settings.service_drop_section_mm2,
        RHO_CU_20,
    );
    let ent_wire = schedule
        .entrance
        .and_then(|e| e.wire_mm2)
        .unwrap_or(10.0);
    let z_ent = cable_impedance(settings.entrance_length_m, ent_wire, RHO_CU_20);
    let z_qdg_pos = z_tr.add(z_drop).add(z_ent);
    let z_qdg_loop = z_tr.add(z_drop.add(z_ent).scale(2.0));

    let icc_3p_qdg = C_MAX * v_fn / z_qdg_pos.mag().max(1e-6) / 1000.0;
    let icc_1p_qdg = C_MAX * v_fn / z_qdg_loop.mag().max(1e-6) / 1000.0;
    let icc_max_qdg = if is_single {
        icc_1p_qdg
    } else {
        icc_3p_qdg.max(icc_1p_qdg)
    };
    let rec_icn_qdg = recommended_icn_ka(icc_max_qdg);
    let icn_ok_qdg = settings.breaker_icn_ka + 1e-6 >= icc_max_qdg;
    let main_label = if schedule.sub_boards.is_empty() {
        "QD".to_owned()
    } else {
        "QDG".to_owned()
    };
    let main_circuits: Vec<u32> = schedule
        .circuits
        .iter()
        .filter(|c| c.board.as_deref().unwrap_or(&main_label) == main_label)
        .map(|c| c.number)
        .collect();
    let main_panel_room = project
        .main_panel()
        .and_then(|p| project.room_for_fixture(p))
        .map(|r| r.display_name().to_owned());
    let main_breaker = schedule.entrance.and_then(|e| e.breaker_a);

    report.boards.push(BoardShortCircuit {
        label: main_label.clone(),
        is_main: true,
        room: main_panel_room,
        upstream_board: Some("Medidor / Rede".to_owned()),
        feeder_length_m: settings.entrance_length_m,
        feeder_wire_mm2: Some(ent_wire),
        feeder_drop_percent: 0.0,
        main_breaker_a: main_breaker,
        circuits: main_circuits,
        installed_va: schedule.total_va,
        demand_va: schedule.demand.demand_va,
        r_mohm: z_qdg_pos.r * 1000.0,
        x_mohm: z_qdg_pos.x * 1000.0,
        icc_3p_ka: icc_3p_qdg,
        icc_1p_ka: icc_1p_qdg,
        icc_max_ka: icc_max_qdg,
        installed_icn_ka: settings.breaker_icn_ka,
        recommended_icn_ka: rec_icn_qdg,
        light: Light::from_utilisation(icc_max_qdg / settings.breaker_icn_ka.max(0.5)),
        note: if icn_ok_qdg {
            format!(
                "Disjuntores de {} kA suportam o curto presumido de {} kA no {main_label}.",
                decimal(settings.breaker_icn_ka, 1),
                decimal(icc_max_qdg, 2)
            )
        } else {
            format!(
                "Curto presumido de {} kA no {main_label} excede os disjuntores de {} kA: especifique disjuntores de {} kA.",
                decimal(icc_max_qdg, 2),
                decimal(settings.breaker_icn_ka, 1),
                decimal(rec_icn_qdg, 1)
            )
        },
    });
    if !icn_ok_qdg {
        report.warnings.push(format!(
            "{main_label}: corrente de curto-circuito presumida ({} kA) acima da capacidade de interrupção dos disjuntores ({} kA). Use disjuntores de {} kA.",
            decimal(icc_max_qdg, 2),
            decimal(settings.breaker_icn_ka, 1),
            decimal(rec_icn_qdg, 1)
        ));
    }

    // Track per-board impedance (pos, loop), feeder drop, breaker, and icc_max for circuits.
    let mut board_data: std::collections::HashMap<String, (Impedance, Impedance, f64, Option<f64>, f64)> =
        std::collections::HashMap::new();
    board_data.insert(
        main_label.clone(),
        (z_qdg_pos, z_qdg_loop, 0.0, main_breaker, icc_max_qdg),
    );

    // Sub-boards (QDC1, QDC2, ...).
    for sub in &schedule.sub_boards {
        let f_len = sub.feeder_length_m.unwrap_or(10.0);
        let f_wire = sub.wire_mm2.unwrap_or(6.0);
        let z_feeder = cable_impedance(f_len, f_wire, RHO_CU_20);
        let z_sub_pos = z_qdg_pos.add(z_feeder);
        let z_sub_loop = z_qdg_loop.add(z_feeder.scale(2.0));
        let icc_3p = C_MAX * v_fn / z_sub_pos.mag().max(1e-6) / 1000.0;
        let icc_1p = C_MAX * v_fn / z_sub_loop.mag().max(1e-6) / 1000.0;
        let icc_max = if is_single {
            icc_1p
        } else {
            icc_3p.max(icc_1p)
        };
        let rec_icn = recommended_icn_ka(icc_max);
        let icn_ok = settings.breaker_icn_ka + 1e-6 >= icc_max;
        let f_drop = sub.feeder_drop_percent.unwrap_or(0.0);

        report.boards.push(BoardShortCircuit {
            label: sub.label.clone(),
            is_main: false,
            room: sub.room.clone(),
            upstream_board: Some(main_label.clone()),
            feeder_length_m: f_len,
            feeder_wire_mm2: sub.wire_mm2,
            feeder_drop_percent: f_drop,
            main_breaker_a: sub.breaker_a,
            circuits: sub.circuits.clone(),
            installed_va: sub.installed_va,
            demand_va: sub.demand_va,
            r_mohm: z_sub_pos.r * 1000.0,
            x_mohm: z_sub_pos.x * 1000.0,
            icc_3p_ka: icc_3p,
            icc_1p_ka: icc_1p,
            icc_max_ka: icc_max,
            installed_icn_ka: settings.breaker_icn_ka,
            recommended_icn_ka: rec_icn,
            light: Light::from_utilisation(icc_max / settings.breaker_icn_ka.max(0.5)),
            note: if icn_ok {
                format!(
                    "{} com curto presumido de {} kA (disjuntores de {} kA atendem).",
                    sub.label,
                    decimal(icc_max, 2),
                    decimal(settings.breaker_icn_ka, 1)
                )
            } else {
                format!(
                    "{}: curto de {} kA passa dos {} kA; use disjuntores de {} kA.",
                    sub.label,
                    decimal(icc_max, 2),
                    decimal(settings.breaker_icn_ka, 1),
                    decimal(rec_icn, 1)
                )
            },
        });
        board_data.insert(
            sub.label.clone(),
            (z_sub_pos, z_sub_loop, f_drop, sub.breaker_a, icc_max),
        );

        // Selectivity: QDG main breaker -> QDC feeder breaker.
        if let (Some(dg), Some(sub_brk)) = (main_breaker, sub.breaker_a) {
            let pair = evaluate_selectivity(
                format!("DG ({main_label})"),
                dg,
                BreakerCurve::C,
                format!("Alimentador {}", sub.label),
                sub_brk,
                BreakerCurve::C,
                icc_max,
            );
            if pair.status == SelectivityStatus::None {
                report.warnings.push(pair.explanation.clone());
            }
            report.selectivity.push(pair);
        }
    }

    // Terminal circuit fault, thermal withstand, cumulative drop and selectivity checks.
    for row in &schedule.circuits {
        let board_name = row.board.clone().unwrap_or_else(|| main_label.clone());
        let (_z_pos, z_loop, feeder_drop, board_brk, icc_board_ka) = board_data
            .get(&board_name)
            .copied()
            .unwrap_or((z_qdg_pos, z_qdg_loop, 0.0, main_breaker, icc_max_qdg));
        let curve = if row.kind == CircuitKind::Lighting {
            BreakerCurve::B
        } else {
            BreakerCurve::C
        };
        let len = row.length_m.unwrap_or(10.0);
        let wire = row.wire_mm2.unwrap_or(2.5);
        let z_term_hot = cable_impedance(len, wire, RHO_CU_70).scale(2.0);
        let z_end_loop = z_loop.add(z_term_hot);
        let icc_min_end_a = C_MIN * row.voltage_v.max(1.0) / z_end_loop.mag().max(1e-6);
        let mag_trip_max_a = row
            .breaker_a
            .map_or(0.0, |b| b * curve.mag_max_mult());
        let magnetic_trip_ok = row.breaker_a.is_none() || icc_min_end_a + 1e-6 >= mag_trip_max_a;
        let cable_thermal_ka = (K_PVC_CU * wire / MCB_CLEAR_TIME_S.sqrt()) / 1000.0;
        let cable_thermal_ok = icc_board_ka <= cable_thermal_ka + 1e-6;
        let total_drop = row.drop_percent.map(|d| d + feeder_drop);
        let total_drop_ok = total_drop.is_none_or(|d| d <= MAX_DROP_PERCENT + 1e-6);

        let light = if !cable_thermal_ok || !total_drop_ok {
            Light::Red
        } else if !magnetic_trip_ok {
            Light::Yellow
        } else {
            Light::Green
        };
        if !total_drop_ok
            && let Some(td) = total_drop
        {
            report.warnings.push(format!(
                "Circuito C{} ({board_name}): queda acumulada (alimentador + terminal) de {} % passa dos 4 % (NBR 5410 §6.2.7).",
                row.number,
                decimal(td, 2)
            ));
        }
        if !cable_thermal_ok {
            report.warnings.push(format!(
                "Circuito C{}: curto de {} kA excede o suporte térmico do cabo de {} mm² ({} kA em 10 ms).",
                row.number,
                decimal(icc_board_ka, 2),
                section_label(wire),
                decimal(cable_thermal_ka, 2)
            ));
        }

        report.circuits.push(CircuitFaultCheck {
            circuit: row.number,
            board: board_name.clone(),
            breaker_a: row.breaker_a,
            curve,
            wire_mm2: row.wire_mm2,
            total_drop_percent: total_drop,
            icc_board_ka,
            icc_min_end_a,
            mag_trip_max_a,
            cable_thermal_ka,
            magnetic_trip_ok,
            cable_thermal_ok,
            total_drop_ok,
            light,
        });

        // Selectivity: serving board upstream breaker -> terminal circuit breaker.
        if let (Some(up_brk), Some(circ_brk)) = (board_brk, row.breaker_a) {
            let up_label = if board_name == main_label {
                format!("DG ({main_label})")
            } else {
                format!("Alim. {board_name}")
            };
            let pair = evaluate_selectivity(
                up_label,
                up_brk,
                BreakerCurve::C,
                format!("C{} ({board_name})", row.number),
                circ_brk,
                curve,
                icc_board_ka,
            );
            if pair.status == SelectivityStatus::None {
                report.warnings.push(pair.explanation.clone());
            }
            report.selectivity.push(pair);
        }
    }

    report.max_icc_ka = report
        .boards
        .iter()
        .map(|b| b.icc_max_ka)
        .fold(0.0, f64::max);
    report.min_recommended_icn_ka = recommended_icn_ka(report.max_icc_ka);
    report.light = report
        .boards
        .iter()
        .map(|b| b.light)
        .chain(report.circuits.iter().map(|c| c.light))
        .chain(report.selectivity.iter().map(|s| s.light))
        .max()
        .unwrap_or(Light::Green);

    report.assumptions = vec![
        format!(
            "Estimativa simplificada de curto-circuito (IEC 60909 / NBR 5410 §5.3.5): transformador {:.0} kVA (Z = {} %), ramal de rede {:.0} m ({} mm² Cu eq.), ramal de entrada {:.0} m ({} mm²).",
            settings.transformer_kva,
            decimal(settings.transformer_z_pct, 1),
            settings.service_drop_length_m,
            section_label(settings.service_drop_section_mm2),
            settings.entrance_length_m,
            section_label(ent_wire)
        ),
        format!(
            "Capacidade de interrupção adotada para disjuntores DIN: Icn = {} kA (NBR NM 60898). Suporte térmico de cabos PVC 70 °C com k = 115 A·√s/mm² e tempo de atuação de 10 ms.",
            decimal(settings.breaker_icn_ka, 1)
        ),
        "Seletividade térmica avaliada pela razão In_montante / In_jusante ≥ 1,6 e seletividade magnética até o limite Is do disjuntor a montante; confirme tabelas de filiação/coordenação do fabricante escolhido.".to_owned(),
        "Pré-dimensionamento: a corrente de curto-circuito real depende da rede da concessionária e deve ser confirmada por engenheiro eletricista.".to_owned(),
    ];

    report
}

/// UTF-8 BOM CSV export of the short-circuit and selectivity study.
pub fn short_circuit_csv(project: &Project) -> String {
    let schedule = project.cached_load_schedule();
    let r = short_circuit_report(project, &schedule);
    let mut csv = String::from(
        "\u{feff}tipo,elemento,montante,icc_max_ka,icn_ka,icc_min_fim_a,razao_seletividade,limite_is_ka,status\r\n",
    );
    for b in &r.boards {
        let _ = writeln!(
            csv,
            "quadro,{},{},{:.2},{:.1},,,,{}\r",
            b.label,
            b.upstream_board.as_deref().unwrap_or(""),
            b.icc_max_ka,
            b.installed_icn_ka,
            b.light.label()
        );
    }
    for s in &r.selectivity {
        let _ = writeln!(
            csv,
            "seletividade,{},{},{:.2},,,{:.2},{:.2},{}\r",
            s.downstream_label,
            s.upstream_label,
            s.icc_downstream_ka,
            s.ratio,
            s.selectivity_limit_ka,
            s.status.label()
        );
    }
    csv
}

/// Printable A3 landscape SVG sheet of the multi-board short-circuit & selectivity study (#127).
pub fn short_circuit_sheet_svg(project: &Project) -> String {
    let schedule = project.cached_load_schedule();
    let r = short_circuit_report(project, &schedule);
    let width = 420.0;
    let rows = r.boards.len() + r.selectivity.len() + r.circuits.len();
    let height = (95.0 + rows as f64 * 5.2 + 45.0).max(297.0);
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
        "ESTUDO DE QUADROS, CURTO-CIRCUITO (Icc) E SELETIVIDADE (NBR 5410)",
    );
    text(
        &mut svg,
        18.0,
        28.5,
        3.0,
        false,
        muted,
        &format!(
            "{} · {} quadro(s) · Icc máx = {} kA · Icn adotado = {} kA (mín. recomendado {} kA)",
            project.name,
            r.boards.len(),
            decimal(r.max_icc_ka, 2),
            decimal(r.settings.breaker_icn_ka, 1),
            decimal(r.min_recommended_icn_ka, 1)
        ),
    );

    let mut y = 40.0;
    text(&mut svg, 18.0, y, 3.4, true, ink, "1. Quadros e níveis de curto-circuito");
    y += 5.5;
    for b in &r.boards {
        let color = match b.light {
            Light::Green => "#2e7d32",
            Light::Yellow => "#b85d00",
            Light::Red => "#c62828",
        };
        text(
            &mut svg,
            18.0,
            y,
            2.6,
            true,
            ink,
            &format!(
                "{} (montante: {}, alim. {} mm² · {:.1} m · ΔV {} %)",
                b.label,
                b.upstream_board.as_deref().unwrap_or("—"),
                b.feeder_wire_mm2.map_or("—".to_owned(), section_label),
                b.feeder_length_m,
                decimal(b.feeder_drop_percent, 2)
            ),
        );
        text(
            &mut svg,
            210.0,
            y,
            2.6,
            true,
            color,
            &format!(
                "Icc3φ = {} kA · Icc1φ = {} kA · Icn = {} kA ({})",
                decimal(b.icc_3p_ka, 2),
                decimal(b.icc_1p_ka, 2),
                decimal(b.installed_icn_ka, 1),
                b.light.label()
            ),
        );
        y += 5.5;
    }

    y += 5.0;
    text(&mut svg, 18.0, y, 3.4, true, ink, "2. Coordenação e seletividade entre disjuntores");
    y += 5.5;
    for s in &r.selectivity {
        let color = match s.light {
            Light::Green => "#2e7d32",
            Light::Yellow => "#b85d00",
            Light::Red => "#c62828",
        };
        text(
            &mut svg,
            18.0,
            y,
            2.5,
            false,
            ink,
            &format!(
                "{} ({:.0} A) → {} ({:.0} A {}) · razão {} · Is = {} kA",
                s.upstream_label,
                s.upstream_in_a,
                s.downstream_label,
                s.downstream_in_a,
                s.downstream_curve.label(),
                decimal(s.ratio, 2),
                decimal(s.selectivity_limit_ka, 2)
            ),
        );
        text(&mut svg, 240.0, y, 2.5, true, color, s.status.label());
        y += 5.0;
    }

    y += 6.0;
    for note in &r.assumptions {
        text(&mut svg, 18.0, y, 2.2, false, muted, note);
        y += 4.2;
    }

    svg.push_str("</svg>\n");
    svg
}

impl Project {
    /// Multi-board short-circuit & breaker selectivity study (#127).
    pub fn short_circuit_report(&self) -> ShortCircuitReport {
        let schedule = self.cached_load_schedule();
        short_circuit_report(self, &schedule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{conduits::ConduitRun, fixtures::FixtureKind, geometry::Point};

    /// Worked example 1 (IEC 60909): 75 kVA, Z = 3.5 %, 127/220 V transformer + 15 m 35 mm² drop + 10 m 10 mm² entrance.
    /// Hand calculation:
    ///   Z_tr = 0.035 * 220² / 75000 = 0.022587 Ω (X/R = 1.5 -> R_tr = 0.012529 Ω, X_tr = 0.018793 Ω)
    ///   R_drop = 0.0185 * 15 / 35 = 0.007929 Ω, X_drop = 0.0012 Ω
    ///   R_ent  = 0.0185 * 10 / 10 = 0.018500 Ω, X_ent  = 0.0008 Ω
    ///   Z_pos  = hypot(0.038958, 0.020793) = 0.044159 Ω
    ///   Icc3p  = 1.05 * 127 / 0.044159 = 3.020 kA.
    #[test]
    fn worked_example_short_circuit_matches_iec_60909_hand_calculation() {
        let mut project = Project::default();
        project.electrical.voltage_v = 127.0;
        project.electrical.supply = Supply::Three;
        project.electrical.short_circuit = ShortCircuitSettings {
            transformer_kva: 75.0,
            transformer_z_pct: 3.5,
            service_drop_length_m: 15.0,
            service_drop_section_mm2: 35.0,
            entrance_length_m: 10.0,
            breaker_icn_ka: 3.0,
        };
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala")
            .unwrap();
        let (pw, pd) = FixtureKind::Panel.default_size();
        let panel = project
            .add_fixture(FixtureKind::Panel, Point::new(0.5, 0.5), pw, pd, 0)
            .unwrap();
        let (ow, od) = FixtureKind::OutletSpecific.default_size();
        let shower = project
            .add_fixture(FixtureKind::OutletSpecific, Point::new(3.5, 0.5), ow, od, 0)
            .unwrap();
        project
            .update_electrical(shower, Some(Some(1)), Some(5500.0), None)
            .unwrap();
        project.add_conduit(panel, shower, ConduitRun::Ceiling).unwrap();

        let report = project.short_circuit_report();
        assert_eq!(report.boards.len(), 1);
        let qdg = &report.boards[0];
        // Note entrance wire for 10 A/3-phase defaults to utility min_wire (6 or 10 mm²).
        // Verify exact formula with whatever entrance wire was selected!
        let ent_wire = qdg.feeder_wire_mm2.unwrap();
        let z_tr = transformer_impedance(75.0, 3.5, 127.0 * 3.0_f64.sqrt());
        let z_drop = cable_impedance(15.0, 35.0, RHO_CU_20);
        let z_ent = cable_impedance(10.0, ent_wire, RHO_CU_20);
        let expected_3p = C_MAX * 127.0 / z_tr.add(z_drop).add(z_ent).mag() / 1000.0;
        assert!((qdg.icc_3p_ka - expected_3p).abs() < 1e-4);

        // Next to a 300 kVA transformer (5 m drop), 3 kA breakers must turn Red and ask for higher Icn.
        project.electrical.short_circuit.transformer_kva = 300.0;
        project.electrical.short_circuit.service_drop_length_m = 5.0;
        project.electrical.short_circuit.entrance_length_m = 2.0;
        let high_fault = project.short_circuit_report();
        assert!(high_fault.boards[0].icc_max_ka > 3.0);
        assert_eq!(high_fault.boards[0].light, Light::Red);
        assert!(high_fault.min_recommended_icn_ka >= 4.5);
    }

    /// Worked example 2 (#127 & M7): main board QDG feeding sub-board QDC1 attenuates Icc,
    /// checks selectivity (DG -> QDC1 feeder -> terminal circuits) and cumulative voltage drop.
    #[test]
    fn main_board_with_sub_board_models_attenuated_icc_selectivity_and_cumulative_drop() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(35.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(10.0, 6.0), "Casa")
            .unwrap();
        project
            .add_room(Point::new(18.0, 0.0), Point::new(28.0, 6.0), "Edícula")
            .unwrap();
        let mut add = |kind: FixtureKind, x: f64, y: f64, c: Option<u32>, va: Option<f64>| {
            let (w, d) = kind.default_size();
            let id = project
                .add_fixture(kind, Point::new(x, y), w, d, 0)
                .unwrap();
            if c.is_some() || va.is_some() {
                project.update_electrical(id, c.map(Some), va, None).unwrap();
            }
            id
        };
        let qdg = add(FixtureKind::Panel, 1.0, 1.0, None, None);
        let qdc1 = add(FixtureKind::Panel, 19.0, 1.0, None, None);
        let light_casa = add(FixtureKind::CeilingLight, 5.0, 3.0, Some(1), Some(200.0));
        let shower_casa = add(FixtureKind::OutletSpecific, 8.0, 3.0, Some(2), Some(5500.0));
        let light_ed = add(FixtureKind::CeilingLight, 23.0, 3.0, Some(3), Some(200.0));
        let outlet_ed = add(FixtureKind::OutletLow, 26.0, 3.0, Some(4), Some(1200.0));
        project.add_conduit(qdg, light_casa, ConduitRun::Ceiling).unwrap();
        project.add_conduit(light_casa, shower_casa, ConduitRun::Ceiling).unwrap();
        project.add_conduit(qdg, qdc1, ConduitRun::Floor).unwrap();
        project.add_conduit(qdc1, light_ed, ConduitRun::Ceiling).unwrap();
        project.add_conduit(light_ed, outlet_ed, ConduitRun::Ceiling).unwrap();

        let report = project.short_circuit_report();
        assert_eq!(report.boards.len(), 2);
        assert_eq!(report.boards[0].label, "QDG");
        assert_eq!(report.boards[1].label, "QDC1");
        // Sub-board QDC1 is 18 m downstream so its short-circuit current is strictly lower than QDG.
        assert!(report.boards[1].icc_max_ka < report.boards[0].icc_max_ka);
        // Selectivity pairs exist for QDG -> QDC1 feeder AND QDC1 -> C3/C4.
        assert!(
            report
                .selectivity
                .iter()
                .any(|s| s.downstream_label.contains("QDC1"))
        );
        assert!(
            report
                .selectivity
                .iter()
                .any(|s| s.upstream_label.contains("QDC1") && s.downstream_label.starts_with("C3"))
        );
        // Cumulative drop on C4 includes QDC1 feeder drop.
        let c4 = report.circuits.iter().find(|c| c.circuit == 4).unwrap();
        assert!(c4.total_drop_percent.unwrap() > 0.0);
        // SVG & CSV exports render cleanly.
        let svg = short_circuit_sheet_svg(&project);
        assert!(svg.contains("CURTO-CIRCUITO"));
        assert!(svg.contains("QDC1"));
        assert!(svg.contains("</svg>"));
        let csv = short_circuit_csv(&project);
        assert!(csv.contains("QDG"));
        assert!(csv.contains("QDC1"));
    }
}
