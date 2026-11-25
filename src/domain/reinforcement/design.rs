//! Preliminary reinforcement report builder across beams, columns, slabs and footings.

use std::collections::{HashMap, HashSet};

use crate::domain::{
    fixtures::FixtureKind,
    limits,
    model::Project,
    structure::{BeamSupport, FootingResult, SlabKind, StructureReport},
};

use super::{
    BarSet, BeamReinforcement, BeamTopBars, BondZone, COMMERCIAL_BAR_LENGTH_M, ColumnReinforcement,
    FootingReinforcement, MeshBars, ReinforcementReport, STEEL_FYK_MPA, STEEL_WASTE_FACTOR,
    STIRRUP_DIAMETERS_MM, SlabReinforcement, SteelDiameterSummary, SteelScheduleRow,
    TransverseBars, bar_mass_kg_m,
    bond::required_anchorage_with_cover_m,
    lap_splice_m,
    sizing::{
        bar_set, beam_design_stirrups, beam_stirrups, choose_beam_bars, choose_column_bars,
        choose_mesh, flexural_demand, flexural_design_demand,
    },
};

fn schedule_bar_set(
    schedule: &mut Vec<SteelScheduleRow>,
    mark: impl Into<String>,
    element: &str,
    position: &str,
    shape: &str,
    bars: &BarSet,
) {
    schedule.push(SteelScheduleRow {
        mark: mark.into(),
        element: element.to_owned(),
        position: position.to_owned(),
        shape: shape.to_owned(),
        diameter_mm: bars.diameter_mm,
        quantity: bars.count,
        unit_length_m: bars.length_each_m,
        total_length_m: bars.total_length_m,
        mass_kg: bars.mass_kg,
    });
}

fn schedule_transverse(
    schedule: &mut Vec<SteelScheduleRow>,
    mark: impl Into<String>,
    element: &str,
    position: &str,
    shape: &str,
    bars: &TransverseBars,
) {
    schedule.push(SteelScheduleRow {
        mark: mark.into(),
        element: element.to_owned(),
        position: position.to_owned(),
        shape: shape.to_owned(),
        diameter_mm: bars.diameter_mm,
        quantity: bars.count,
        unit_length_m: bars.length_each_m,
        total_length_m: bars.total_length_m,
        mass_kg: bars.mass_kg,
    });
}

fn schedule_mesh(
    schedule: &mut Vec<SteelScheduleRow>,
    mark: impl Into<String>,
    element: &str,
    position: &str,
    shape: &str,
    bars: &MeshBars,
) {
    schedule.push(SteelScheduleRow {
        mark: mark.into(),
        element: element.to_owned(),
        position: position.to_owned(),
        shape: shape.to_owned(),
        diameter_mm: bars.diameter_mm,
        quantity: bars.count,
        unit_length_m: bars.length_each_m,
        total_length_m: bars.total_length_m,
        mass_kg: bars.mass_kg,
    });
}

fn summarize_by_diameter(schedule: &[SteelScheduleRow]) -> Vec<SteelDiameterSummary> {
    let mut totals: Vec<(i32, f64, f64, f64)> = Vec::new();
    for row in schedule {
        let key = (row.diameter_mm * 10.0).round() as i32;
        if let Some(entry) = totals.iter_mut().find(|(k, _, _, _)| *k == key) {
            entry.2 += row.total_length_m;
            entry.3 += row.mass_kg;
        } else {
            totals.push((key, row.diameter_mm, row.total_length_m, row.mass_kg));
        }
    }
    totals.sort_by_key(|(key, _, _, _)| *key);
    totals
        .into_iter()
        .map(
            |(_, diameter_mm, total_length_m, mass_kg)| SteelDiameterSummary {
                diameter_mm,
                total_length_m,
                bars_12m: ((total_length_m * STEEL_WASTE_FACTOR) / COMMERCIAL_BAR_LENGTH_M).ceil()
                    as u32,
                mass_kg,
                mass_with_waste_kg: mass_kg * STEEL_WASTE_FACTOR,
            },
        )
        .collect()
}

fn footing_column_dimensions(project: &Project, footing: &FootingResult) -> Option<(f64, f64)> {
    let footing_fixture = project
        .fixtures
        .iter()
        .find(|fixture| fixture.id == footing.id && fixture.kind == FixtureKind::Footing)?;
    let column = project
        .structural_columns()
        .find(|column| footing_fixture.contains(column.center))?;
    if column.quarter_turns % 2 == footing_fixture.quarter_turns % 2 {
        Some((column.width, column.depth))
    } else {
        Some((column.depth, column.width))
    }
}

/// Builds the reinforcement proposal from the structural analysis results.
pub fn build_reinforcement_report(
    project: &Project,
    structure: &StructureReport,
) -> ReinforcementReport {
    let mut out = ReinforcementReport::default();
    if let Some(refusal) = limits::structural_refusal(project) {
        out.warnings.push(refusal);
        return out;
    }
    if !structure.frame_failures.is_empty() {
        for failure in &structure.frame_failures {
            out.warnings.push(format!(
                "{}: armaduras não emitidas porque a análise do pórtico falhou — {}.",
                failure.label, failure.reason
            ));
        }
        return out;
    }
    let fck = project.structure.settings.fck_mpa;
    let frame_beams: HashMap<_, _> = structure
        .frames
        .iter()
        .flat_map(|frame| {
            frame
                .beams
                .iter()
                .map(|beam| (beam.beam_id, &beam.ultimate))
        })
        .collect();
    let framed_beam_ids: HashSet<_> = project
        .beam_lines()
        .into_iter()
        .filter(|line| {
            line.supports
                .iter()
                .any(|s| matches!(s, BeamSupport::Column { .. }))
        })
        .flat_map(|line| line.spans)
        .collect();

    for line in &structure.beams {
        for (index, span) in line.spans.iter().enumerate() {
            let frame_envelope = frame_beams.get(&span.beam).copied();
            if framed_beam_ids.contains(&span.beam) && frame_envelope.is_none() {
                out.warnings.push(format!(
                    "{} vão {}: armadura não emitida porque falta a envoltória ELU do pórtico.",
                    line.label,
                    index + 1
                ));
                continue;
            }
            if span.length_m > limits::MAX_BEAM_SPAN_M + 1e-9 {
                out.warnings.push(format!(
                    "{} vão {}: armadura recusada para vão acima de {:.1} m.",
                    line.label,
                    index + 1,
                    limits::MAX_BEAM_SPAN_M
                ));
                continue;
            }
            // Effective depth uses d' = 4 cm (c_nom + stirrup + bar/2), while
            // bar-fit uses nominal cover c_nom = 3 cm (CAA II, NBR 6118 Table 7.2),
            // matching `beam_stirrups`.
            let d_prime = 0.04;
            let nominal_cover = 0.03;
            let (md_positive, md_left, md_right) = if let Some(envelope) = frame_envelope {
                (
                    envelope.max_positive_moment_knm,
                    envelope.min_negative_moment_knm.abs(),
                    envelope.min_negative_moment_knm.abs(),
                )
            } else {
                (
                    1.4 * span.moment_span,
                    1.4 * span.moment_left.abs(),
                    1.4 * span.moment_right.abs(),
                )
            };
            let bottom_demand =
                flexural_design_demand(span.width_m, span.height_m, d_prime, md_positive, fck);
            let left_demand =
                flexural_design_demand(span.width_m, span.height_m, d_prime, md_left, fck);
            let right_demand =
                flexural_design_demand(span.width_m, span.height_m, d_prime, md_right, fck);
            // ELU demands are already factored. Preserve the steel assumed by
            // the existing service deflection check as an explicit lower bound;
            // otherwise a stiffer frame could suggest less steel than Branson
            // relied on and invalidate the displayed service deflection.
            let service_minimum_cm2 = span.branson_as_cm2;
            let bottom_required = bottom_demand.required_cm2.max(service_minimum_cm2);
            let stirrups = if let Some(envelope) = frame_envelope {
                beam_design_stirrups(
                    span.length_m,
                    span.width_m,
                    span.height_m,
                    envelope.max_shear_kn,
                    fck,
                )
            } else {
                beam_stirrups(span.length_m, span.width_m, span.height_m, span.shear, fck)
            };
            let bottom_choice = choose_beam_bars(
                bottom_required,
                span.width_m,
                nominal_cover,
                stirrups.diameter_mm,
            );
            let left_choice = choose_beam_bars(
                left_demand.required_cm2,
                span.width_m,
                nominal_cover,
                stirrups.diameter_mm,
            );
            let right_choice = choose_beam_bars(
                right_demand.required_cm2,
                span.width_m,
                nominal_cover,
                stirrups.diameter_mm,
            );
            let bottom_anchorage_m = required_anchorage_with_cover_m(
                bottom_choice.diameter_mm,
                fck,
                BondZone::Good,
                true,
                nominal_cover,
                bottom_required,
                bottom_choice.provided_cm2,
            );
            let top_zone = if span.height_m >= 0.30 {
                BondZone::Poor
            } else {
                BondZone::Good
            };
            let left_anchorage_m = required_anchorage_with_cover_m(
                left_choice.diameter_mm,
                fck,
                top_zone,
                true,
                nominal_cover,
                left_demand.required_cm2,
                left_choice.provided_cm2,
            );
            let right_anchorage_m = required_anchorage_with_cover_m(
                right_choice.diameter_mm,
                fck,
                top_zone,
                true,
                nominal_cover,
                right_demand.required_cm2,
                right_choice.provided_cm2,
            );
            let top_anchorage_m = left_anchorage_m.max(right_anchorage_m);
            let bottom = bar_set(
                bottom_choice,
                bottom_required,
                span.length_m + 2.0 * bottom_anchorage_m,
            );
            let top = if frame_envelope.is_some() {
                BeamTopBars::Continuous(bar_set(
                    left_choice,
                    left_demand.required_cm2,
                    span.length_m + left_anchorage_m + right_anchorage_m,
                ))
            } else {
                let support_length = (0.30 * span.length_m).max(0.60);
                BeamTopBars::Supports {
                    left: bar_set(
                        left_choice,
                        left_demand.required_cm2,
                        support_length + left_anchorage_m,
                    ),
                    right: bar_set(
                        right_choice,
                        right_demand.required_cm2,
                        support_length + right_anchorage_m,
                    ),
                }
            };
            let label = if line.spans.len() > 1 {
                format!("{} v{}", line.label, index + 1)
            } else {
                line.label.clone()
            };
            let flexural_domain_ok =
                bottom_demand.domain_ok && left_demand.domain_ok && right_demand.domain_ok;
            let ac_beam_cm2 = span.width_m * span.height_m * 10_000.0;
            let max_long_cm2 =
                bottom.provided_cm2 + top.left().provided_cm2.max(top.right().provided_cm2);
            if max_long_cm2 > 0.04 * ac_beam_cm2 + 1e-6 {
                out.warnings.push(format!(
                    "{label}: taxa longitudinal ({:.1}%) excede 4% da seção (NBR 6118 §17.3.5.2.4); aumente a seção.",
                    max_long_cm2 / ac_beam_cm2.max(1e-9) * 100.0
                ));
            }
            if !flexural_domain_ok {
                out.warnings.push(format!(
                    "{label}: momento acima do domínio de armadura simples; aumente a seção e revise com engenheiro."
                ));
            }
            if !bottom.fits_section || !top.left().fits_section || !top.right().fits_section {
                out.warnings.push(format!(
                    "{label}: as barras não cabem em uma camada com cobrimento nominal de 3 cm (CAA II); aumente a largura ou detalhe em camadas."
                ));
            }
            if !stirrups.shear_capacity_ok {
                out.warnings.push(format!(
                    "{label}: compressão diagonal excedida; estribos sozinhos não resolvem, aumente a seção."
                ));
            }
            let mass_kg = bottom.mass_kg + top.mass_kg() + stirrups.mass_kg;
            schedule_bar_set(
                &mut out.schedule,
                format!("{label}-INF"),
                &label,
                "longitudinal inferior",
                "reta + 2 ganchos 90°",
                &bottom,
            );
            match &top {
                BeamTopBars::Continuous(bars) => schedule_bar_set(
                    &mut out.schedule,
                    format!("{label}-SUP"),
                    &label,
                    "superior contínua",
                    "reta + 2 ganchos 90°",
                    bars,
                ),
                BeamTopBars::Supports { left, right } => {
                    schedule_bar_set(
                        &mut out.schedule,
                        format!("{label}-SUP-E"),
                        &label,
                        "superior apoio esquerdo",
                        "reta + gancho 90°",
                        left,
                    );
                    schedule_bar_set(
                        &mut out.schedule,
                        format!("{label}-SUP-D"),
                        &label,
                        "superior apoio direito",
                        "reta + gancho 90°",
                        right,
                    );
                }
            }
            schedule_transverse(
                &mut out.schedule,
                format!("{label}-E"),
                &label,
                "estribos",
                "estribo fechado",
                &stirrups,
            );
            out.beams.push(BeamReinforcement {
                beam_id: span.beam,
                label,
                span_index: index + 1,
                width_m: span.width_m,
                height_m: span.height_m,
                length_m: span.length_m,
                bottom,
                top,
                frame_envelope: frame_envelope.cloned(),
                service_minimum_cm2,
                bottom_anchorage_m,
                top_anchorage_m,
                stirrups,
                flexural_domain_ok,
                mass_kg,
            });
        }
    }

    for column in &structure.columns {
        let ac_cm2 = column.area_cm2;
        let fcd_kn_cm2 = fck / 1.4 / 10.0;
        let fyd_kn_cm2 = STEEL_FYK_MPA / 1.15 / 10.0;
        let nsd = 1.4 * column.position.factor() * column.load_kn;
        // Domain-5 compression steel stress σ_sd = 0.002·Es = 420 MPa (42 kN/cm²),
        // deducting concrete displaced by steel (0.85 fcd) per NBR 6118 §17.2.2.
        let sigma_sd_comp = 42.0;
        let steel_by_load = ((nsd - 0.85 * fcd_kn_cm2 * ac_cm2)
            / (sigma_sd_comp - 0.85 * fcd_kn_cm2).max(10.0))
        .max(0.0);
        // NBR 6118:2023 §17.3.5.3.1: As,min = max(0.15·Nd/fyd, 0.004·Ac).
        let as_min_cm2 = (0.15 * nsd / fyd_kn_cm2).max(0.004 * ac_cm2);
        let required = steel_by_load.max(as_min_cm2);
        let choice = choose_column_bars(required);
        // Column starter bars splice 100% of bars at floor level: α0t = 2.0
        // (NBR 6118 Table 9.3), also satisfying compression §9.5.2.3 (>= 0.6 lb).
        let lap_length_m = lap_splice_m(
            choice.diameter_mm,
            fck,
            BondZone::Good,
            2.0,
            required,
            choice.provided_cm2,
        )
        .max(0.40);
        // Bars as long as the column (#145: the tallest wall it stands in).
        let longitudinal = bar_set(choice, required, column.height_m + lap_length_m);
        let tie_spacing_m = 0.20_f64
            .min(12.0 * choice.diameter_mm / 1000.0)
            .min(column.width_m.min(column.depth_m))
            .max(0.05);
        let spacing_cm = (tie_spacing_m * 100.0 / 5.0).floor().max(1.0) * 5.0;
        let tie_count = (column.height_m / (spacing_cm / 100.0)).ceil() as u32 + 1;
        // NBR 6118 §18.4.3: tie diameter >= max(5 mm, Ø_l / 4).
        let min_tie_mm = (choice.diameter_mm / 4.0).max(5.0);
        let tie_diameter = STIRRUP_DIAMETERS_MM
            .iter()
            .copied()
            .find(|&d| d + 1e-6 >= min_tie_mm)
            .unwrap_or(10.0);
        let core_b = (column.width_m - 0.06).max(0.04);
        let core_h = (column.depth_m - 0.06).max(0.04);
        let tie_length = 2.0 * (core_b + core_h) + 2.0 * 10.0 * tie_diameter / 1000.0;
        let tie_total = f64::from(tie_count) * tie_length;
        let ties = TransverseBars {
            diameter_mm: tie_diameter,
            spacing_cm,
            count: tie_count,
            length_each_m: tie_length,
            total_length_m: tie_total,
            mass_kg: tie_total * bar_mass_kg_m(tie_diameter),
            shear_capacity_ok: true,
        };
        let steel_ratio = longitudinal.provided_cm2 / ac_cm2.max(1e-9);
        if steel_ratio > 0.04 + 1e-9 {
            out.warnings.push(format!(
                "{}: taxa longitudinal de {:.1}% excede o limite de pré-detalhamento; aumente a seção.",
                column.label,
                steel_ratio * 100.0
            ));
        }
        let mass_kg = longitudinal.mass_kg + ties.mass_kg;
        schedule_bar_set(
            &mut out.schedule,
            format!("{}-L", column.label),
            &column.label,
            "longitudinal",
            "reta + traspasse",
            &longitudinal,
        );
        schedule_transverse(
            &mut out.schedule,
            format!("{}-T", column.label),
            &column.label,
            "estribos",
            "estribo fechado",
            &ties,
        );
        out.columns.push(ColumnReinforcement {
            column_id: column.id,
            label: column.label.clone(),
            width_m: column.width_m,
            depth_m: column.depth_m,
            height_m: column.height_m,
            longitudinal,
            lap_length_m,
            ties,
            steel_ratio,
            mass_kg,
        });
    }

    for slab in &structure.slabs {
        if slab.lx_m > limits::MAX_SLAB_SPAN_M + 1e-9 || !slab.supported {
            out.warnings.push(format!(
                "{} ({}): armadura não emitida porque a laje está fora dos limites ou sem apoio.",
                slab.label, slab.room
            ));
            continue;
        }
        let characteristic_moment = if slab.kind == SlabKind::Solid {
            slab.moment_x_knm_m
        } else {
            slab.load_kn_m2 * slab.lx_m.powi(2) / 8.0 + slab.point_moment_knm_m
        };
        // NBR 6118 §20.1: max bar spacing in slabs is min(2h, 20 cm).
        let slab_s_max_cm = (2.0 * slab.thickness_m * 100.0).min(20.0);
        let demand_x = flexural_demand(1.0, slab.thickness_m, 0.025, characteristic_moment, fck);
        let main_length = slab.lx_m + 0.30;
        let main = choose_mesh(demand_x.required_cm2, slab.ly_m, main_length, slab_s_max_cm);
        // Upper orthogonal layer (Y) sits above X layer: deduct ~Ø_x from effective depth.
        let d_prime_y = 0.025 + main.diameter_mm / 1000.0;
        let demand_y = flexural_demand(1.0, slab.thickness_m, d_prime_y, slab.moment_y_knm_m, fck);
        let distribution_required = if slab.kind == SlabKind::Solid {
            demand_y.required_cm2.max(0.90)
        } else {
            (0.20 * main.required_cm2_m).max(0.90)
        };
        let distribution = choose_mesh(
            distribution_required,
            slab.lx_m,
            slab.ly_m + 0.30,
            slab_s_max_cm,
        );
        if slab.kind == SlabKind::Ribbed {
            out.warnings.push(format!(
                "{} ({}): aço principal é uma estimativa equivalente por metro; confirme a vigota/treliça com o fabricante.",
                slab.label, slab.room
            ));
        }
        let domain_ok = demand_x.domain_ok && (slab.kind != SlabKind::Solid || demand_y.domain_ok);
        if !domain_ok {
            out.warnings.push(format!(
                "{} ({}): seção insuficiente para armadura simples; aumente a espessura.",
                slab.label, slab.room
            ));
        }
        let mass_kg = main.mass_kg + distribution.mass_kg;
        schedule_mesh(
            &mut out.schedule,
            format!("{}-P", slab.label),
            &slab.label,
            "direção x (menor vão)",
            if slab.kind == SlabKind::Solid {
                "malha reta"
            } else {
                "equivalente vigota"
            },
            &main,
        );
        schedule_mesh(
            &mut out.schedule,
            format!("{}-D", slab.label),
            &slab.label,
            if slab.kind == SlabKind::Solid {
                "direção y (maior vão)"
            } else {
                "distribuição"
            },
            "malha reta",
            &distribution,
        );
        out.slabs.push(SlabReinforcement {
            slab_id: slab.id,
            label: slab.label.clone(),
            room: slab.room.clone(),
            kind: slab.kind,
            main,
            distribution,
            domain_ok,
            mass_kg,
        });
    }

    for footing in &structure.footings {
        if footing.soil_kpa < limits::MIN_SOIL_KPA - 1e-9 || footing.column.is_none() {
            out.warnings.push(format!(
                "{}: armadura não emitida sem solo aplicável e pilar associado.",
                footing.label
            ));
            continue;
        }
        let Some((column_width, column_depth)) = footing_column_dimensions(project, footing) else {
            out.warnings.push(format!(
                "{}: não foi possível identificar a seção do pilar.",
                footing.label
            ));
            continue;
        };
        let cantilever_x =
            ((footing.width_m - column_width) * 0.5 + footing.eccentricity_m).max(0.0);
        let cantilever_y =
            ((footing.depth_m - column_depth) * 0.5 + footing.eccentricity_m).max(0.0);
        let q = footing.max_pressure_kpa;
        let moment_x = q * cantilever_x.powi(2) / 2.0;
        let moment_y = q * cantilever_y.powi(2) / 2.0;
        let demand_x = flexural_demand(1.0, footing.height_m, 0.07, moment_x, fck);
        let demand_y = flexural_demand(1.0, footing.height_m, 0.07, moment_y, fck);
        // For rigid footings (NBR 6118 §22.5.3 / Blevot strut-and-tie), envelope
        // cantilever flexure with tie tension T_d = 1.1 · N_d · (B - b) / (8 · d).
        let d_foot = (footing.height_m - 0.07).max(0.15);
        let fyd_kn_m2 = (STEEL_FYK_MPA / 1.15) * 1000.0;
        let n_d = 1.4 * footing.load_kn.max(0.0);
        let strut_x_cm2_m = 1.1 * n_d * (footing.width_m - column_width).max(0.0)
            / (8.0 * d_foot * fyd_kn_m2 * footing.depth_m.max(0.4))
            * 10_000.0;
        let strut_y_cm2_m = 1.1 * n_d * (footing.depth_m - column_depth).max(0.0)
            / (8.0 * d_foot * fyd_kn_m2 * footing.width_m.max(0.4))
            * 10_000.0;
        let req_x_cm2_m = demand_x.required_cm2.max(strut_x_cm2_m);
        let req_y_cm2_m = demand_y.required_cm2.max(strut_y_cm2_m);
        if !demand_x.domain_ok || !demand_y.domain_ok {
            out.warnings.push(format!(
                "{}: altura insuficiente para flexão simples na sapata; aumente a altura.",
                footing.label
            ));
        }
        let hook_return = (footing.height_m - 0.14).clamp(0.10, 0.25);
        let bars_x = choose_mesh(
            req_x_cm2_m,
            (footing.depth_m - 0.14).max(0.20),
            (footing.width_m - 0.14).max(0.30) + 2.0 * hook_return,
            20.0,
        );
        let bars_y = choose_mesh(
            req_y_cm2_m,
            (footing.width_m - 0.14).max(0.20),
            (footing.depth_m - 0.14).max(0.30) + 2.0 * hook_return,
            20.0,
        );
        let mass_kg = bars_x.mass_kg + bars_y.mass_kg;
        schedule_mesh(
            &mut out.schedule,
            format!("{}-X", footing.label),
            &footing.label,
            "malha inferior X",
            "malha c/ dobras 90°",
            &bars_x,
        );
        schedule_mesh(
            &mut out.schedule,
            format!("{}-Y", footing.label),
            &footing.label,
            "malha inferior Y",
            "malha c/ dobras 90°",
            &bars_y,
        );
        out.footings.push(FootingReinforcement {
            footing_id: footing.id,
            label: footing.label.clone(),
            width_m: footing.width_m,
            depth_m: footing.depth_m,
            height_m: footing.height_m,
            bars_x,
            bars_y,
            mass_kg,
        });
    }

    out.by_diameter = summarize_by_diameter(&out.schedule);
    out
}
