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
