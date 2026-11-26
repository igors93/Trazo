//! Electrical installation safety verdicts (NBR 5410).

use crate::domain::{
    distribution::{MAX_DROP_PERCENT, MAX_FEEDER_DROP_PERCENT},
    electrical::{CircuitKind, LoadSchedule},
    limits,
};

use super::{Check, Discipline, Verdict, decimal, limit_detail};

/// The electrical checks, from the load schedule.
pub fn electrical_verdicts(schedule: &LoadSchedule) -> Vec<Verdict> {
    let mut verdicts: Vec<Verdict> = limits::electrical_violations(schedule)
        .into_iter()
        .map(|v| Verdict {
            discipline: Discipline::Electrical,
            element: v.element.clone(),
            checks: vec![Check::fail(
                "Fora dos limites",
                v.message.clone(),
                limit_detail(&v),
            )],
        })
        .collect();

    // Service entrance & supply checks (NBR 5410 + utility rules).
    if let Some(entrance) = &schedule.entrance {
        let mut entrance_checks = Vec::new();
        let needs_two_phases = schedule.circuits.iter().any(|c| c.poles >= 2);
        if needs_two_phases && entrance.supply.phases() < 2 {
            entrance_checks.push(Check::fail(
                "Fornecimento",
                "Há circuitos bifásicos (220 V) em ligação monofásica: altere o fornecimento para bifásico ou trifásico.".to_owned(),
                format!("Ligação {} possui apenas 1 fase.", entrance.supply.label()),
            ));
        }
        match (entrance.breaker_a, entrance.wire_mm2) {
            (Some(breaker), Some(wire)) => {
                entrance_checks.push(Check::new(
                    "Disjuntor geral",
                    entrance.current_a / breaker,
                    if entrance.current_a <= breaker + 1e-9 {
                        format!(
                            "Entrada {} com disjuntor geral de {breaker:.0} A e ramal de {} mm² atende {} A.",
                            entrance.supply.label().to_lowercase(),
                            decimal(wire, 1),
                            decimal(entrance.current_a, 1)
                        )
                    } else {
                        "Corrente de demanda excede o disjuntor geral.".to_owned()
                    },
                    format!(
                        "I_dem = {} A ≤ In = {breaker:.0} A ≤ Iz (Tabela 36/46, {} mm²).",
                        decimal(entrance.current_a, 1),
                        decimal(wire, 1)
                    ),
                ));
            }
            _ if schedule.total_va > 0.0 => {
                entrance_checks.push(Check::fail(
                    "Disjuntor geral",
                    "A corrente de entrada passa das tabelas residenciais (até 125 A): consulte a distribuidora.".to_owned(),
                    format!("I_dem = {} A por fase.", decimal(entrance.current_a, 1)),
                ));
            }
            _ => {}
        }
        let used = usize::from(entrance.supply.phases());
        if used >= 2 {
            let loads = &schedule.phase_va[..used];
            let high = loads.iter().copied().fold(0.0, f64::max);
            let low = loads.iter().copied().fold(f64::MAX, f64::min);
            if high > 0.0 {
                let imbalance = (high - low) / high;
                if imbalance > 0.10 + 1e-6 {
                    entrance_checks.push(Check::new(
                        "Equilíbrio entre fases",
                        (imbalance / 0.10).min(1.0),
                        format!(
                            "Desequilíbrio de {:.0} % entre as fases (recomendado ≤ 10 %): redistribua circuitos.",
                            imbalance * 100.0
                        ),
                        format!(
                            "(P_máx − P_mín) / P_máx = {:.1} % (meta ≤ 10 %).",
                            imbalance * 100.0
                        ),
                    ));
                }
            }
        }
        if !entrance_checks.is_empty() {
            verdicts.push(Verdict {
                discipline: Discipline::Electrical,
                element: "Entrada de energia (QDG)".to_owned(),
                checks: entrance_checks,
            });
        }
    }

    // Sub-boards (QDC) feeder checks.
    for sub in &schedule.sub_boards {
        if sub.installed_va <= 0.0 {
            continue;
        }
        let mut checks = Vec::new();
        match (sub.breaker_a, sub.wire_mm2) {
            (Some(breaker), Some(wire)) => {
                checks.push(Check::new(
                    "Alimentador",
                    sub.current_a / breaker,
                    if sub.current_a <= breaker + 1e-9 {
                        format!(
                            "Alimentador de {} mm² com disjuntor de {breaker:.0} A atende {} A.",
                            decimal(wire, 1),
                            decimal(sub.current_a, 1)
                        )
                    } else {
                        "Corrente do subquadro excede o disjuntor do alimentador.".to_owned()
                    },
                    format!(
                        "I = {} A ≤ In = {breaker:.0} A, cabo {} mm².",
                        decimal(sub.current_a, 1),
                        decimal(wire, 1)
                    ),
                ));
            }
            _ => checks.push(Check::fail(
                "Alimentador",
                "A corrente do subquadro excede as tabelas: divida a carga.".to_owned(),
                format!("I = {} A.", decimal(sub.current_a, 1)),
            )),
        }
        if let Some(drop) = sub.feeder_drop_percent {
            checks.push(Check::new(
                "Queda no alimentador",
                drop / MAX_FEEDER_DROP_PERCENT,
                if drop <= MAX_FEEDER_DROP_PERCENT + 1e-9 {
                    format!(
                        "Queda de tensão no alimentador ({} %) dentro dos {MAX_FEEDER_DROP_PERCENT:.0} %.",
                        decimal(drop, 2)
                    )
                } else {
                    "Queda de tensão no alimentador excede 2 %: aproxime o subquadro ou aumente a bitola.".to_owned()
                },
                format!(
                    "ΔV = {} % ≤ {MAX_FEEDER_DROP_PERCENT:.0} % em {} m.",
                    decimal(drop, 2),
                    decimal(sub.feeder_length_m.unwrap_or(0.0), 1)
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Electrical,
            element: format!("Subquadro {}", sub.label),
            checks,
        });
    }

    for row in &schedule.circuits {
        let mut checks = Vec::new();
        if row.kind == CircuitKind::Mixed {
            checks.push(Check::fail(
                "Separação (NBR 5410)",
                "Circuito mistura iluminação e tomadas: separe em circuitos distintos (NBR 5410 §9.5.3).".to_owned(),
                "NBR 5410 §9.5.3 exige circuitos terminais independentes para iluminação e tomadas.".to_owned(),
            ));
        }
        match (row.breaker_a, row.wire_mm2) {
            (Some(breaker), Some(wire)) => {
                checks.push(Check::new(
                    "Corrente",
                    row.current_a / breaker,
                    if row.current_a <= breaker + 1e-9 {
                        format!("O disjuntor de {breaker:.0} A protege os {} A do circuito.", decimal(row.current_a, 1))
                    } else {
                        "O circuito passa do disjuntor: divida as cargas.".to_owned()
                    },
                    format!(
                        "I = {} A ≤ In = {breaker:.0} A ≤ Iz do fio de {} mm² (NBR 5410 5.3.4 e tabela 36).",
                        decimal(row.current_a, 1),
                        decimal(wire, 1)
                    ),
                ));
            }
            _ => checks.push(Check::fail(
                "Corrente",
                "A corrente é alta demais para um circuito: divida as cargas.".to_owned(),
                format!(
                    "{} A não cabe nos disjuntores e fios da tabela.",
                    decimal(row.current_a, 1)
                ),
            )),
        }
        if let Some(drop) = row.drop_percent {
            checks.push(Check::new(
                "Queda de tensão",
                drop / MAX_DROP_PERCENT,
                if drop <= MAX_DROP_PERCENT + 1e-9 {
                    format!("A queda de tensão de {} % está dentro dos 4 %.", decimal(drop, 1))
                } else {
                    "A tensão cai demais no fim do circuito: aproxime o quadro ou divida o circuito.".to_owned()
                },
                format!(
                    "ΔV = {} % ≤ 4 % (NBR 5410 6.2.7.1), ρ = 1/56 Ω·mm²/m, {} m até o ponto mais longe.",
                    decimal(drop, 2),
                    decimal(row.length_m.unwrap_or(0.0), 1)
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Electrical,
            element: format!("Circuito C{} ({})", row.number, row.kind.short()),
            checks,
        });
    }
    verdicts
}

/// Project-level electrical checks beyond the load schedule: room illuminance
/// (NBR ISO/CIE 8995-1, #125), photovoltaic pre-sizing (#126), short-circuit &
/// selectivity (#127) and earthing / SPDA (#129), emitted only when those
/// electrical features are present in the project.
pub fn electrical_project_verdicts(
    project: &crate::domain::model::Project,
    schedule: &LoadSchedule,
) -> Vec<Verdict> {
    let mut verdicts = electrical_verdicts(schedule);

    // #125 Lighting calculation verdicts (only when at least one light fixture is drawn).
    let has_lights = project
        .electrical_devices()
        .any(|d| d.kind.role() == Some(crate::domain::electrical::DeviceRole::Lighting));
    if has_lights {
        let lighting = project.lighting_report();
        for room in &lighting.rooms {
            let check = Check::new(
                "Iluminância (lux)",
                room.utilisation,
                room.plain_note.clone(),
                format!(
                    "E_calc = {} lx vs. meta Em = {:.0} lx ({}). K = {}, Fu = {}, Fd = {}, {} luminária(s) ({:.0} lm).",
                    decimal(room.calculated_lux, 0),
                    room.target_lux,
                    room.clause,
                    decimal(room.room_index_k, 2),
                    decimal(room.utilization_factor, 2),
                    decimal(room.maintenance_factor, 2),
                    room.luminaires,
                    room.installed_lumens
                ),
            );
            verdicts.push(Verdict {
                discipline: Discipline::Electrical,
                element: format!("Iluminação · {}", room.room_name),
                checks: vec![check],
            });
        }
    }

    // #126 Photovoltaic pre-sizing verdicts (when PV is enabled on the project).
    if project.electrical.pv.enabled {
        let pv = project.photovoltaic_report();
        let mut pv_checks = Vec::new();
        if pv.refused_above_limit {
            pv_checks.push(Check::fail(
                "Limite microgeração (75 kW)",
                format!(
                    "Inversor de {} kW excede o limite residencial de 75 kW (Lei 14.300): exige projeto de minigeração por engenheiro eletricista.",
                    decimal(pv.inverter_kw, 1)
                ),
                format!(
                    "P_CA = {} kW > 75 kW (microgeração distribuída BT).",
                    decimal(pv.inverter_kw, 1)
                ),
            ));
        } else {
            if !pv.planes.is_empty() {
                let ratio = if pv.modules_placed > 0 {
                    pv.modules_required as f64 / pv.modules_placed as f64
                } else {
                    f64::INFINITY
                };
                pv_checks.push(Check::new(
                    "Área no telhado",
                    ratio,
                    if pv.modules_placed >= pv.modules_required {
                        format!(
                            "Cabem todos os {} módulos ({} kWp) nos planos do telhado.",
                            pv.modules_placed,
                            decimal(pv.installed_kwp, 2)
                        )
                    } else {
                        format!(
                            "Falta área no telhado: cabem {} de {} módulos necessários ({} kWp).",
                            pv.modules_placed,
                            pv.modules_required,
                            decimal(pv.installed_kwp, 2)
                        )
                    },
                    format!(
                        "{} módulos alocados de {} necessários ({} m² estimados, sobrecarga 0,15 kN/m²).",
                        pv.modules_placed,
                        pv.modules_required,
                        decimal(pv.required_roof_area_m2, 1)
                    ),
                ));
            }
            pv_checks.push(Check::new(
                "Inversor (FDI CC/CA)",
                pv.dc_ac_ratio / 1.30,
                format!(
                    "Inversor de {} kW atende {} kWp em {} (geração média {:.0} kWh/mês).",
                    decimal(pv.inverter_kw, 1),
                    decimal(pv.installed_kwp, 2),
                    pv.city_label,
                    pv.mean_monthly_generation_kwh
                ),
                format!(
                    "FDI = {} ≤ 1,30 (NBR 16690), HSP = {} kWh/m²·dia, PR = {:.0} %.",
                    decimal(pv.dc_ac_ratio, 2),
                    decimal(pv.mean_hsp_kwh_m2_day, 2),
                    pv.settings.performance_ratio * 100.0
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Electrical,
            element: format!("Fotovoltaico ({} kWp)", decimal(pv.installed_kwp, 2)),
            checks: pv_checks,
        });
    }

    // #127 Multi-board short-circuit capacity & selectivity verdicts (when circuits exist).
    if !schedule.circuits.is_empty() {
        let sc = crate::domain::distribution::selectivity::short_circuit_report(project, schedule);
        let mut sc_checks = Vec::new();
        for b in &sc.boards {
            sc_checks.push(Check::new(
                &format!("Curto-circuito {}", b.label),
                b.icc_max_ka / b.installed_icn_ka.max(0.5),
                b.note.clone(),
                format!(
                    "Icc máx = {} kA (3φ {} kA · 1φ {} kA) vs. Icn = {} kA (mín. recomendado {} kA, IEC 60909 / NBR 5410 §5.3.5).",
                    decimal(b.icc_max_ka, 2),
                    decimal(b.icc_3p_ka, 2),
                    decimal(b.icc_1p_ka, 2),
                    decimal(b.installed_icn_ka, 1),
                    decimal(b.recommended_icn_ka, 1)
                ),
            ));
        }
        for pair in sc.selectivity.iter().filter(|s| {
            s.status == crate::domain::distribution::selectivity::SelectivityStatus::None
        }) {
            sc_checks.push(Check::fail(
                &format!(
                    "Seletividade {} → {}",
                    pair.upstream_label, pair.downstream_label
                ),
                pair.explanation.clone(),
                format!(
                    "Razão In_montante / In_jusante = {} < 1,25 (NBR 5410 §5.3.6).",
                    decimal(pair.ratio, 2)
                ),
            ));
        }
        for c in sc.circuits.iter().filter(|c| !c.total_drop_ok) {
            if let Some(td) = c.total_drop_percent {
                sc_checks.push(Check::new(
                    &format!("Queda acumulada C{}", c.circuit),
                    td / MAX_DROP_PERCENT,
                    format!(
                        "Circuito C{} ({}) soma {} % de queda (alimentador + terminal), acima dos 4 %: aumente o alimentador do subquadro ou o fio do circuito.",
                        c.circuit,
                        c.board,
                        decimal(td, 2)
                    ),
                    format!("ΔV_alimentador + ΔV_terminal = {} % > 4 % (NBR 5410 §6.2.7.1).", decimal(td, 2)),
                ));
            }
        }
        if !sc_checks.is_empty() {
            verdicts.push(Verdict {
                discipline: Discipline::Electrical,
                element: "Curto-circuito e seletividade".to_owned(),
                checks: sc_checks,
            });
        }
    }

    // #129 Earthing layout & simplified NBR 5419-2 SPDA risk check (when a panel or circuits exist).
    if project.main_panel().is_some() || !schedule.circuits.is_empty() {
        let es = project.earthing_and_spda_report();
        let mut es_checks = Vec::new();
        if es.settings.scheme == crate::domain::electrical::earthing::EarthingScheme::Tt
            && es.tt_missing_rcd_circuits > 0
        {
            es_checks.push(Check::fail(
                "Esquema TT · DR obrigatório",
                es.earthing_note.clone(),
                format!(
                    "NBR 5410 §5.1.2.2.4.2 exige proteção diferencial (DR 30 mA) em todos os circuitos no esquema TT ({} sem DR).",
                    es.tt_missing_rcd_circuits
                ),
            ));
        } else {
            // 0..25 Ω green/yellow; > 50 Ω red (utilisation scaled to 50 Ω hard limit, with 25 Ω target).
            let util = if es.array_resistance_ohm <= es.target_resistance_ohm {
                (es.array_resistance_ohm / es.target_resistance_ohm) * 0.85
            } else if es.array_resistance_ohm <= 50.0 {
                0.85 + ((es.array_resistance_ohm - es.target_resistance_ohm) / 25.0) * 0.15
            } else {
                es.array_resistance_ohm / 50.0
            };
            es_checks.push(Check::new(
                "Aterramento (NBR 5410)",
                util,
                es.earthing_note.clone(),
                format!(
                    "Esquema {}, {} hastes de {} m em ρ = {:.0} Ω·m → Ra = {} Ω (meta ≤ 25 Ω, cabo #{} mm²).",
                    es.settings.scheme.code(),
                    es.rod_count,
                    decimal(es.settings.rod_length_m, 2),
                    es.settings.soil_resistivity_ohm_m,
                    decimal(es.array_resistance_ohm, 1),
                    decimal(es.buried_conductor_mm2, 0)
                ),
            ));
        }
        if es.spda_refused_height {
            es_checks.push(Check::fail(
                "SPDA · fora da triagem (NBR 5419)",
                es.spda_note.clone(),
                format!(
                    "Altura H = {} m > 15 m: exige estudo completo NBR 5419-2 por engenheiro eletricista.",
                    decimal(es.building_height_m, 1)
                ),
            ));
        } else {
            es_checks.push(Check::new(
                "Risco SPDA (NBR 5419-2)",
                es.risk_r1 / es.tolerable_risk_rt,
                es.spda_note.clone(),
                format!(
                    "R1 = {:.2e}/ano vs. RT = 1,00e-5/ano (Ng = {}, Ad = {:.0} m², Cd = {}, Nd = {:.4}/ano).",
                    es.risk_r1,
                    decimal(es.flash_density_ng, 2),
                    es.collection_area_ad_m2,
                    decimal(es.settings.surroundings.cd(), 2),
                    es.annual_direct_flashes_nd
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Electrical,
            element: "Aterramento e SPDA".to_owned(),
            checks: es_checks,
        });
    }

    verdicts
}
