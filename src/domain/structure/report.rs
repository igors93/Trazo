//! Full structural load-path analysis (`structure_report`): upper floors,
//! slabs, concentrated loads, continuous beams, columns, footings, wall
//! foundations, balance beams, roofs, wind, masonry and 2D portal frames.

use std::collections::{HashMap, HashSet};

use crate::domain::{
    fixtures::{Fixture, FixtureKind},
    geometry::Point,
    model::Project,
};

use super::{
    Beam, BeamLine, BeamLineResult, BeamSupport, CONCRETE_KN_M3, ColumnPosition, ColumnResult,
    ConcentratedLoadResult, FootingResult, MisalignedColumn, SlabKind, SlabResult, SpanPointLoad,
    SpanResult, StructureReport, check_footing, continuous_beam_with_point_loads, decimal,
    decompose_slab_panels, effective_beam_stiffness, natural, reading, room_live_load_kn_m2,
    round_up, section_cm, span_deflection_with_point_loads, yield_line_shares,
};

/// Internal transfer record passed from an upper floor to the floor below.
struct UpperColumnLoad {
    floor_name: String,
    label: String,
    center: Point,
    load_kn: f64,
}

#[derive(Clone, Copy)]
struct BeamPointLoad {
    /// Parameter along `beam.from -> beam.to`.
    t: f64,
    load_kn: f64,
    /// Fraction retained in the quasi-permanent service combination.
    quasi_factor: f64,
}

impl Project {
    fn floor_snapshot(&self, level: &crate::domain::floors::FloorLevel) -> Project {
        let mut snap = Project::default();
        snap.wall_height = level.ceiling_height_m.max(2.0);
        snap.walls = level.data.walls.clone();
        snap.rooms = level.data.rooms.clone();
        snap.openings = level.data.openings.clone();
        snap.fixtures = level.data.fixtures.clone();
        snap.structure.settings = self.structure.settings;
        snap.structure.settings.floors = 1;
        snap.structure.beams = level.data.beams.clone();
        snap.structure.slabs = level.data.slabs.clone();
        snap.structure.roofs = level.data.roofs.clone();
        snap
    }

    /// The load path from the slabs (and any real upper floors above) down to
    /// the soil, and the pre-sizing checks of every element.
    pub fn structure_report(&self) -> StructureReport {
        // When real upper floors with structural elements exist above the
        // active floor, propagate their actual column loads top-down instead
        // of using the scalar `settings.floors` multiplier.
        let active_id = self.floors.active_floor_id;
        let active_elev = self
            .active_floor()
            .map(|level| level.elevation_m)
            .unwrap_or(0.0);
        let mut upper_levels: Vec<&crate::domain::floors::FloorLevel> = self
            .floors
            .levels
            .iter()
            .filter(|level| {
                Some(level.id) != active_id
                    && level.elevation_m > active_elev + 1e-4
                    && (!level.data.beams.is_empty()
                        || !level.data.slabs.is_empty()
                        || level
                            .data
                            .fixtures
                            .iter()
                            .any(|f| f.kind == FixtureKind::Column))
            })
            .collect();
        if upper_levels.is_empty() {
            return self.structure_report_with_incoming(&[], false, true);
        }
        // Top-down: highest storey first down to the storey directly above.
        upper_levels.sort_by(|a, b| b.elevation_m.total_cmp(&a.elevation_m));
        let mut incoming: Vec<UpperColumnLoad> = Vec::new();
        let mut inherited_misaligned: Vec<MisalignedColumn> = Vec::new();
        let mut inherited_warnings: Vec<String> = Vec::new();
        for level in upper_levels {
            let snap = self.floor_snapshot(level);
            let rep = snap.structure_report_with_incoming(&incoming, true, false);
            inherited_misaligned.extend(rep.misaligned_columns);
            for w in rep.warnings {
                if w.contains("desalinhado") || w.contains("sem prumada") {
                    inherited_warnings.push(w);
                }
            }
            incoming = rep
                .columns
                .iter()
                .filter_map(|col| {
                    let fix = snap.column(col.id)?;
                    Some(UpperColumnLoad {
                        floor_name: level.name.clone(),
                        label: col.label.clone(),
                        center: fix.center,
                        load_kn: col.load_kn,
                    })
                })
                .collect();
        }
        let mut report = self.structure_report_with_incoming(&incoming, false, false);
        report.misaligned_columns.extend(inherited_misaligned);
        report.warnings.extend(inherited_warnings);
        report
    }

    fn structure_report_with_incoming(
        &self,
        incoming_upper: &[UpperColumnLoad],
        include_own_walls_on_beams: bool,
        use_multiplier: bool,
    ) -> StructureReport {
        let settings = self.structure.settings;
        let labels = self.structure_labels();
        let label = |id: u64| labels.get(&id).cloned().unwrap_or_else(|| "?".to_owned());
        let mut report = StructureReport::default();
        let floors = if use_multiplier {
            f64::from(settings.floors.max(1))
        } else {
            1.0
        };
        let beams: Vec<(Beam, Point, Point)> = self
            .valid_beams()
            .filter_map(|beam| {
                let (a, b) = self.beam_endpoints(beam)?;
                Some((*beam, a, b))
            })
            .collect();
        // Match upper-floor columns against columns on this floor; misaligned
        // columns transfer as point loads onto a beam below or warn if unsupported.
        let mut incoming_column_load: HashMap<u64, f64> = HashMap::new();
        let mut beam_point_loads: HashMap<u64, Vec<BeamPointLoad>> = HashMap::new();
        // Timber roofs (#114): each main-beam end bears on the column under
        // it, or on the concrete beam under it.
        let mut roof_on_column: HashMap<u64, f64> = HashMap::new();
        for roof in &self.structure.roofs {
            let mut result = self.analyse_roof(roof);
            for reaction in &mut result.reactions {
                let column = self
                    .structural_columns()
                    .filter(|c| {
                        c.contains(reaction.point) || c.center.distance(reaction.point) <= 0.35
                    })
                    .min_by(|a, b| {
                        a.center
                            .distance(reaction.point)
                            .total_cmp(&b.center.distance(reaction.point))
                    });
                if let Some(column) = column {
                    *roof_on_column.entry(column.id).or_default() += reaction.load_kn;
                    reaction.support = Some(label(column.id));
                    continue;
                }
                let host = beams
                    .iter()
                    .filter_map(|(beam, a, b)| {
                        let (nearest, t) = crate::domain::geometry::nearest_point_on_segment(
                            reaction.point,
                            *a,
                            *b,
                        );
                        let dist = nearest.distance(reaction.point);
                        (dist <= 0.25).then_some((dist, beam.id, t))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, beam_id, t)) = host {
                    beam_point_loads
                        .entry(beam_id)
                        .or_default()
                        .push(BeamPointLoad {
                            t: t.clamp(0.02, 0.98),
                            load_kn: reaction.load_kn,
                            quasi_factor: reaction.dead_kn / reaction.load_kn.max(1e-9),
                        });
                    reaction.support = Some(label(beam_id));
                }
            }
            if !result.slope_ok() {
                report.warnings.push(format!(
                    "Telhado {}: inclinação de {:.0} % abaixo dos {:.0} % que a telha pede.",
                    result.label, result.slope_percent, result.min_slope_percent
                ));
            }
            if result.sheet_span_exceeded {
                report.warnings.push(format!(
                    "Telhado {}: as telhas não vencem o espaçamento das terças; aproxime as terças.",
                    result.label
                ));
            }
            for element in result.elements.iter().filter(|e| !e.ok) {
                report.warnings.push(format!(
                    "Telhado {}: nem {}×{} cm resolve as {}; diminua o vão ou o espaçamento, ou use madeira mais forte.",
                    result.label,
                    element.section_cm.0,
                    element.section_cm.1,
                    element.element.label().to_lowercase()
                ));
            }
            report.roofs.push(result);
        }
        // The rest bears on timber posts (#146), directly or through a
        // timber beam along the edge.
        let posts = self.post_effects(&mut report.roofs);
        for result in &report.roofs {
            let loose: Vec<&crate::domain::roof::RoofReaction> = result
                .reactions
                .iter()
                .filter(|r| r.support.is_none())
                .collect();
            if !loose.is_empty() {
                report.warnings.push(format!(
                    "Telhado {}: {} apoio(s) de viga sem pilar, mourão nem viga embaixo (até {} kN cada); ponha pilares ou mourões nesses pontos.",
                    result.label,
                    loose.len(),
                    decimal(loose.iter().map(|r| r.load_kn).fold(0.0, f64::max), 1)
                ));
            }
        }
        for post in &posts.posts {
            if !post.treated {
                report.warnings.push(format!(
                    "Mourão {}: madeira externa sem tratamento preservativo; especifique tratamento e mantenha a peça afastada do solo pelo chumbador.",
                    post.label
                ));
            }
            if post.ratio > 1.0 + 1e-9
                || post.slenderness > crate::domain::timber_posts::MAX_POST_SLENDERNESS + 1e-9
            {
                report.warnings.push(match post.needed_m {
                    Some(size) => format!(
                        "Mourão {}: {} cm não resiste à compressão, flambagem e vento lateral (λ = {:.0}, uso {:.0} %); use {} cm.",
                        post.label,
                        decimal(post.size_m * 100.0, 0),
                        post.slenderness,
                        (post.ratio * 100.0).min(999.0),
                        decimal(size * 100.0, 0)
                    ),
                    None => format!(
                        "Mourão {}: nem 30 cm resiste à carga vertical e ao vento; diminua a altura/carga ou projete contraventamento.",
                        post.label
                    ),
                });
            }
            if post.base_kpa > post.soil_kpa + 1e-9 || post.base_stability_ratio > 1.0 + 1e-9 {
                report.warnings.push(format!(
                    "Mourão {}: o bloco máximo de {}×{} cm não fecha pressão/estabilidade sob o vento; projete uma fundação e ancoragem específicas.",
                    post.label,
                    decimal(post.base_m * 100.0, 0),
                    decimal(post.base_depth_m * 100.0, 0)
                ));
            }
        }
        for beam in posts.eave_beams.iter().filter(|b| !b.ok) {
            report.warnings.push(format!(
                "Viga de madeira {}: nem {}×{} cm vence {} m entre {} e {}; ponha um mourão no meio.",
                beam.label,
                beam.section_cm.0,
                beam.section_cm.1,
                decimal(beam.span_m, 2),
                beam.from_post,
                beam.to_post
            ));
        }
        report.timber_posts = posts.posts;
        report.eave_beams = posts.eave_beams;
        for upper in incoming_upper {
            let aligned = self
                .structural_columns()
                .filter(|col| {
                    col.contains(upper.center) || col.center.distance(upper.center) <= 0.15
                })
                .min_by(|a, b| {
                    a.center
                        .distance(upper.center)
                        .total_cmp(&b.center.distance(upper.center))
                });
            if let Some(col) = aligned {
                *incoming_column_load.entry(col.id).or_default() += upper.load_kn;
            } else {
                // Check if the misaligned column lands on a beam below (transfer girder).
                let host_beam = beams
                    .iter()
                    .filter_map(|(beam, a, b)| {
                        let (nearest, t) =
                            crate::domain::geometry::nearest_point_on_segment(upper.center, *a, *b);
                        let dist = nearest.distance(upper.center);
                        (dist <= 0.20).then_some((dist, beam.id, t))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, beam_id, t)) = host_beam {
                    let beam_label = label(beam_id);
                    if upper.load_kn > 0.0 {
                        beam_point_loads
                            .entry(beam_id)
                            .or_default()
                            .push(BeamPointLoad {
                                t: t.clamp(0.02, 0.98),
                                load_kn: upper.load_kn,
                                quasi_factor: 1.0,
                            });
                    }
                    report.misaligned_columns.push(MisalignedColumn {
                        floor_name: upper.floor_name.clone(),
                        column_label: upper.label.clone(),
                        center: upper.center,
                        load_kn: upper.load_kn,
                        transfer_beam: Some(beam_label.clone()),
                    });
                    report.warnings.push(format!(
                        "Pilar {} ({}) desalinhado: nasce sobre {} (viga de transição com {} kN); confira o detalhamento.",
                        upper.label,
                        upper.floor_name,
                        beam_label,
                        decimal(upper.load_kn, 1)
                    ));
                } else {
                    report.misaligned_columns.push(MisalignedColumn {
                        floor_name: upper.floor_name.clone(),
                        column_label: upper.label.clone(),
                        center: upper.center,
                        load_kn: upper.load_kn,
                        transfer_beam: None,
                    });
                    report.warnings.push(format!(
                        "Pilar {} ({}) sem prumada abaixo ({} kN): não há pilar nem viga alinhados no pavimento inferior.",
                        upper.label,
                        upper.floor_name,
                        decimal(upper.load_kn, 1)
                    ));
                }
            }
        }

        // Loads that physically overlap a beam go straight to that beam.
        // The remaining loads are picked up by the slab panel below and
        // distributed to its nearest supported edges in the slab pass.
        let mut concentrated: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|fixture| {
                matches!(
                    fixture.kind,
                    FixtureKind::WaterTank | FixtureKind::HeavyEquipment
                ) && fixture.structural_load_kn.is_some()
            })
            .collect();
        concentrated.sort_by(|a, b| reading(a.center, b.center));
        let concentrated_labels: HashMap<u64, String> = concentrated
            .iter()
            .enumerate()
            .map(|(index, fixture)| (fixture.id, format!("CC{}", index + 1)))
            .collect();
        let mut handled_concentrated = HashSet::new();
        for fixture in &concentrated {
            let load_kn = fixture.structural_load_kn.unwrap_or(0.0);
            if !load_kn.is_finite() || load_kn <= 0.0 {
                report.warnings.push(format!(
                    "{}: carga concentrada inválida; informe um valor positivo em kN.",
                    concentrated_labels[&fixture.id]
                ));
                continue;
            }
            let direct = beams
                .iter()
                .filter_map(|(beam, a, b)| {
                    let (nearest, t) =
                        crate::domain::geometry::nearest_point_on_segment(fixture.center, *a, *b);
                    fixture.contains(nearest).then_some((
                        nearest.distance(fixture.center),
                        beam.id,
                        t,
                    ))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, beam_id, t)) = direct {
                beam_point_loads
                    .entry(beam_id)
                    .or_default()
                    .push(BeamPointLoad {
                        t: t.clamp(0.02, 0.98),
                        load_kn,
                        quasi_factor: 1.0,
                    });
                let beam_label = label(beam_id);
                report.concentrated_loads.push(ConcentratedLoadResult {
                    id: fixture.id,
                    label: concentrated_labels[&fixture.id].clone(),
                    kind: fixture.kind,
                    center: fixture.center,
                    load_kn,
                    support: format!("direto na viga {beam_label}"),
                    slab: None,
                    local_moment_knm_m: 0.0,
                    punching_ratio: None,
                    supported: true,
                });
                handled_concentrated.insert(fixture.id);
            }
        }
        // Slab loads shared by the beams along the slab's sides (separated into G1 self-weight and G2 finishes).
        let mut beam_g1: HashMap<u64, f64> = HashMap::new();
        let mut beam_g2: HashMap<u64, f64> = HashMap::new();
        let mut beam_live: HashMap<u64, f64> = HashMap::new();
        for slab in &self.structure.slabs {
            let Some(room) = self.rooms.iter().find(|room| room.id == slab.room_id) else {
                continue;
            };
            let (lx, ly) = self.slab_spans(room);
            let area = self.room_area(room).unwrap_or(lx * ly);
            let slab_g1 = slab.kind.self_weight(slab.thickness_m);
            let slab_g2 = settings.finish_kn_m2;
            let dead = slab_g1 + slab_g2;
            let live = if settings.per_room_live_loads {
                room_live_load_kn_m2(room.display_name(), settings.live_kn_m2)
            } else {
                settings.live_kn_m2
            };
            let mut supported_segments: Vec<(u64, Point, Point)> = Vec::new();
            let shares = if let Some(outline) = self.room_outline(room) {
                for i in 0..outline.len() {
                    let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
                    let Some(dir) = b.minus(a).normalized() else {
                        continue;
                    };
                    let edge_len = a.distance(b);
                    for (beam, p, q) in &beams {
                        let off = |x: Point| x.minus(a).cross(dir).abs();
                        if off(*p) > 0.12 || off(*q) > 0.12 {
                            continue;
                        }
                        let (s0, s1) = (p.minus(a).dot(dir), q.minus(a).dot(dir));
                        let lo = s0.min(s1).max(0.0);
                        let hi = s0.max(s1).min(edge_len);
                        if hi - lo > 0.05 {
                            supported_segments.push((
                                beam.id,
                                a.translated(dir.scaled(lo)),
                                a.translated(dir.scaled(hi)),
                            ));
                        }
                    }
                }
                yield_line_shares(&outline, &supported_segments)
            } else {
                HashMap::new()
            };
            let total: f64 = shares.values().sum();
            for (id, share) in &shares {
                *beam_g1.entry(*id).or_default() += slab_g1 * area * share / total.max(1e-9);
                *beam_g2.entry(*id).or_default() += slab_g2 * area * share / total.max(1e-9);
                *beam_live.entry(*id).or_default() += live * area * share / total.max(1e-9);
            }
            let mut point_load_kn = 0.0;
            let mut point_moment_knm_m = 0.0;
            if let Some(outline) = self.room_outline(room) {
                let slab_loads: Vec<_> = concentrated
                    .iter()
                    .copied()
                    .filter(|fixture| {
                        !handled_concentrated.contains(&fixture.id)
                            && crate::domain::geometry::polygon_contains_point(
                                &outline,
                                fixture.center,
                            )
                    })
                    .collect();
                for fixture in slab_loads {
                    let load_kn = fixture.structural_load_kn.unwrap_or(0.0);
                    if !load_kn.is_finite() || load_kn <= 0.0 {
                        continue;
                    }
                    let effective_width = (fixture.width.min(fixture.depth)
                        + 2.0 * slab.thickness_m)
                        .clamp(0.50, ly.max(0.50));
                    let local_moment = load_kn * lx / (4.0 * effective_width);
                    let punching_ratio = if slab.kind == SlabKind::Solid {
                        let d = (slab.thickness_m - 0.025).max(0.05);
                        let fck = settings.fck_mpa.max(20.0);
                        let f_sd = 1.4 * load_kn;
                        let fcd_kn_m2 = fck / 1.4 * 1000.0;
                        let alpha_v2 = (1.0 - fck / 250.0).max(0.1);
                        let tau_rd2 = 0.27 * alpha_v2 * fcd_kn_m2;
                        // NBR 6118 §19.5.2.1: C0 at loaded-area face and C' at 2d
                        // with rounded corners (circular for water tanks).
                        let (u0, u1) = if fixture.kind == FixtureKind::WaterTank {
                            let diam = fixture.width.min(fixture.depth).max(0.20);
                            (
                                (std::f64::consts::PI * diam).max(0.40),
                                std::f64::consts::PI * (diam + 4.0 * d),
                            )
                        } else {
                            (
                                (2.0 * (fixture.width + fixture.depth)).max(0.40),
                                2.0 * (fixture.width + fixture.depth)
                                    + 4.0 * std::f64::consts::PI * d,
                            )
                        };
                        let tau_sd0 = f_sd / (u0 * d);
                        let tau_sd1 = f_sd / (u1 * d);
                        let k_size = (1.0 + (0.20 / d).sqrt()).min(2.0);
                        let tau_rd1 = 0.13 * k_size * (100.0 * 0.002 * fck).cbrt() * 1000.0;
                        Some((tau_sd0 / tau_rd2).max(tau_sd1 / tau_rd1))
                    } else {
                        None
                    };
                    point_load_kn += load_kn;
                    point_moment_knm_m += local_moment;

                    let mut distance_by_beam: HashMap<u64, f64> = HashMap::new();
                    for (beam_id, a, b) in &supported_segments {
                        let distance =
                            crate::domain::geometry::distance_to_segment(fixture.center, *a, *b);
                        distance_by_beam
                            .entry(*beam_id)
                            .and_modify(|old| *old = old.min(distance))
                            .or_insert(distance);
                    }
                    let nearest = distance_by_beam
                        .values()
                        .copied()
                        .fold(f64::INFINITY, f64::min);
                    let hosts: Vec<u64> = distance_by_beam
                        .iter()
                        .filter_map(|(beam_id, distance)| {
                            (*distance <= nearest + 0.05).then_some(*beam_id)
                        })
                        .collect();
                    if hosts.is_empty() {
                        report.concentrated_loads.push(ConcentratedLoadResult {
                            id: fixture.id,
                            label: concentrated_labels[&fixture.id].clone(),
                            kind: fixture.kind,
                            center: fixture.center,
                            load_kn,
                            support: "laje sem viga de apoio".to_owned(),
                            slab: Some(label(slab.id)),
                            local_moment_knm_m: local_moment,
                            punching_ratio,
                            supported: false,
                        });
                        report.warnings.push(format!(
                            "{}: {} de {} kN está sobre a laje {} sem viga de apoio.",
                            concentrated_labels[&fixture.id],
                            fixture.kind.label(),
                            decimal(load_kn, 1),
                            label(slab.id)
                        ));
                    } else {
                        let share = load_kn / hosts.len() as f64;
                        let mut host_labels = Vec::new();
                        for beam_id in hosts {
                            let Some((_, a, b)) =
                                beams.iter().find(|(beam, _, _)| beam.id == beam_id)
                            else {
                                continue;
                            };
                            let (_, t) = crate::domain::geometry::nearest_point_on_segment(
                                fixture.center,
                                *a,
                                *b,
                            );
                            beam_point_loads
                                .entry(beam_id)
                                .or_default()
                                .push(BeamPointLoad {
                                    t: t.clamp(0.02, 0.98),
                                    load_kn: share,
                                    quasi_factor: 1.0,
                                });
                            host_labels.push(label(beam_id));
                        }
                        host_labels.sort();
                        host_labels.dedup();
                        report.concentrated_loads.push(ConcentratedLoadResult {
                            id: fixture.id,
                            label: concentrated_labels[&fixture.id].clone(),
                            kind: fixture.kind,
                            center: fixture.center,
                            load_kn,
                            support: format!(
                                "laje {} → {}",
                                label(slab.id),
                                host_labels.join(", ")
                            ),
                            slab: Some(label(slab.id)),
                            local_moment_knm_m: local_moment,
                            punching_ratio,
                            supported: true,
                        });
                        if slab.kind == SlabKind::Ribbed {
                            report.warnings.push(format!(
                                "{}: carga concentrada sobre laje treliçada; apoie sobre viga ou detalhe faixa maciça e vigotas com o fabricante.",
                                concentrated_labels[&fixture.id]
                            ));
                        } else if punching_ratio.is_some_and(|ratio| ratio > 1.0) {
                            report.warnings.push(format!(
                                "{}: punção local da laje excedida ({:.0}%); aumente a base/espessura ou apoie a carga sobre vigas.",
                                concentrated_labels[&fixture.id],
                                punching_ratio.unwrap_or(0.0) * 100.0
                            ));
                        }
                    }
                    handled_concentrated.insert(fixture.id);
                }
            }
            let panel_dims = self
                .room_outline(room)
                .map(|outline| decompose_slab_panels(&outline))
                .filter(|panels| !panels.is_empty())
                .unwrap_or_else(|| vec![(lx, ly)]);
            let panels: Vec<_> = panel_dims
                .into_iter()
                .enumerate()
                .map(|(index, (panel_lx, panel_ly))| {
                    crate::domain::slabs::analyse_panel(
                        index + 1,
                        panel_lx,
                        panel_ly,
                        dead,
                        live,
                        slab.thickness_m,
                        settings.modulus_kn_m2(),
                    )
                })
                .collect();
            let governing_x = panels
                .iter()
                .max_by(|a, b| a.moment_x_knm_m.total_cmp(&b.moment_x_knm_m));
            let governing_y = panels
                .iter()
                .max_by(|a, b| a.moment_y_knm_m.total_cmp(&b.moment_y_knm_m));
            let governing_deflection = panels.iter().max_by(|a, b| {
                (a.deflection_mm / a.limit_mm.max(1e-9))
                    .total_cmp(&(b.deflection_mm / b.limit_mm.max(1e-9)))
            });
            let (moment_x, method_x) = governing_x
                .map(|panel| (panel.moment_x_knm_m, panel.method_x))
                .unwrap_or((0.0, crate::domain::slabs::SlabTableMethod::OneWay));
            let (moment_y, method_y) = governing_y
                .map(|panel| (panel.moment_y_knm_m, panel.method_y))
                .unwrap_or((0.0, crate::domain::slabs::SlabTableMethod::OneWay));
            let (deflection_mm, deflection_limit_mm) = governing_deflection
                .map(|panel| (panel.deflection_mm, panel.limit_mm))
                .unwrap_or((0.0, lx / 250.0 * 1000.0));
            report.slabs.push(SlabResult {
                id: slab.id,
                label: label(slab.id),
                room: room.display_name().to_owned(),
                kind: slab.kind,
                lx_m: lx,
                ly_m: ly,
                thickness_m: slab.thickness_m,
                thickness_min_m: slab.kind.minimum_thickness(lx),
                load_kn_m2: dead + live,
                point_load_kn,
                point_moment_knm_m,
                moment_x_knm_m: moment_x + point_moment_knm_m,
                moment_y_knm_m: moment_y + point_moment_knm_m,
                method_x,
                method_y,
                deflection_mm,
                deflection_limit_mm,
                panels,
                supported: total > 0.0,
            });
        }

        for fixture in concentrated {
            if handled_concentrated.contains(&fixture.id) {
                continue;
            }
            let load_kn = fixture.structural_load_kn.unwrap_or(0.0);
            if !load_kn.is_finite() || load_kn <= 0.0 {
                continue;
            }
            report.concentrated_loads.push(ConcentratedLoadResult {
                id: fixture.id,
                label: concentrated_labels[&fixture.id].clone(),
                kind: fixture.kind,
                center: fixture.center,
                load_kn,
                support: "sem laje ou viga abaixo".to_owned(),
                slab: None,
                local_moment_knm_m: 0.0,
                punching_ratio: None,
                supported: false,
            });
            report.warnings.push(format!(
                "{}: {} de {} kN está sem laje ou viga abaixo; reposicione ou projete apoio próprio.",
                concentrated_labels[&fixture.id],
                fixture.kind.label(),
                decimal(load_kn, 1)
            ));
        }

        // Continuous beams, ordered deepest-first so secondary beams resting on
        // other beams transfer their reactions as point loads onto host girders.
        let modulus = settings.modulus_kn_m2();
        let mut column_load: HashMap<u64, f64> = HashMap::new();
        // Wall load already included once inside `column_load` when `floors > 1`
        // (subtracted once from `column_load * floors` so N storeys carry N-1
        // upper walls instead of N·(N-1) walls!).
        let mut column_wall_load: HashMap<u64, f64> = HashMap::new();
        let mut incident: HashMap<u64, Vec<Point>> = HashMap::new();
        let mut ordered_lines = self.beam_lines();
        ordered_lines.sort_by(|a, b| {
            let line_depth = |line: &BeamLine| {
                line.spans
                    .iter()
                    .filter_map(|id| self.beam_by_id(*id))
                    .map(|beam| {
                        let mut v = HashSet::from([beam.id]);
                        self.beam_support_depth(beam, &mut v)
                    })
                    .max()
                    .unwrap_or(0)
            };
            line_depth(b).cmp(&line_depth(a))
        });
        for line in ordered_lines {
            let mut spans = Vec::new();
            let mut stiff = Vec::new();
            let mut qp_stiff = Vec::new();
            let mut span_points: Vec<Vec<(f64, f64)>> = Vec::new();
            let mut span_pts_qp: Vec<Vec<(f64, f64)>> = Vec::new();
            let mut span_point_records: Vec<Vec<BeamPointLoad>> = Vec::new();
            let mut span_wall_kn_m: Vec<f64> = Vec::new();
            for (i, id) in line.spans.iter().enumerate() {
                let Some((beam, b_start, b_end)) = beams.iter().find(|(b, _, _)| b.id == *id)
                else {
                    continue;
                };
                let mut v0 = HashSet::new();
                let a = self.resolve_support(line.supports[i], &mut v0);
                let mut v1 = HashSet::new();
                let b = self.resolve_support(line.supports[i + 1], &mut v1);
                let (Some(a), Some(b)) = (a, b) else {
                    continue;
                };
                let length = a.distance(b).max(0.1);
                for (support, to) in [(line.supports[i], b), (line.supports[i + 1], a)] {
                    if let BeamSupport::Column { id: col_id } = support
                        && let Some(c) = self.column(col_id)
                        && let Some(d) = to.minus(c.center).normalized()
                    {
                        incident.entry(col_id).or_default().push(d);
                    }
                }
                // Masonry walls carried by this beam: at most ONE storey of walls
                // sits directly on any single beam (multi-storey columns accumulate
                // N-1 storeys of upper walls, never N·(N-1)).
                let wall_factor = if include_own_walls_on_beams || (use_multiplier && floors > 1.0)
                {
                    1.0
                } else {
                    0.0
                };
                // Allow flush-inside-lot boundary beams (shifted ~half wall/column width)
                // to still pick up their boundary wall load.
                let wall_tol = 0.06 + beam.width_m.max(0.14) * 0.6;
                let walls_kn_m: f64 = if wall_factor > 0.0 {
                    self.walls
                        .iter()
                        .filter(|wall| {
                            crate::domain::geometry::distance_to_segment(
                                a.midpoint(b),
                                wall.start,
                                wall.end,
                            ) < wall_tol
                        })
                        .map(|wall| {
                            // The wall's own top over this span (#145).
                            wall.thickness()
                                * wall.height_at(self.wall_height, a.midpoint(b))
                                * settings.masonry_kn_m3
                        })
                        .fold(0.0, f64::max)
                        * wall_factor
                } else {
                    0.0
                };
                let own = CONCRETE_KN_M3 * beam.width_m * beam.height_m;
                let g1_dead = own + beam_g1.get(id).copied().unwrap_or(0.0) / length;
                let g2_dead = walls_kn_m + beam_g2.get(id).copied().unwrap_or(0.0) / length;
                let dead = g1_dead + g2_dead;
                let live = beam_live.get(id).copied().unwrap_or(0.0) / length;
                let inertia = beam.width_m * beam.height_m.powi(3) / 12.0;
                // Map incoming secondary-beam reactions `(t, P)` along `beam.from -> beam.to`
                // into distance `x` from this span's left joint `a`.
                let reversed = a.distance(*b_end) < a.distance(*b_start);
                let point_records: Vec<BeamPointLoad> = beam_point_loads
                    .get(id)
                    .map(|list| {
                        list.iter()
                            .map(|point| {
                                let t_span = if reversed { 1.0 - point.t } else { point.t };
                                BeamPointLoad {
                                    t: (t_span * length).clamp(0.0, length),
                                    ..*point
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let pts: Vec<(f64, f64)> = point_records
                    .iter()
                    .map(|point| (point.t, point.load_kn))
                    .collect();
                let pts_qp: Vec<(f64, f64)> = point_records
                    .iter()
                    .map(|point| (point.t, point.load_kn * point.quasi_factor))
                    .collect();
                let psi2_q = settings.occupancy.psi2();
                spans.push((
                    beam.id,
                    length,
                    g1_dead,
                    g2_dead,
                    live,
                    beam.width_m,
                    beam.height_m,
                ));
                stiff.push((length, dead + live, modulus * inertia));
                qp_stiff.push((length, dead + psi2_q * live, modulus * inertia));
                span_points.push(pts);
                span_pts_qp.push(pts_qp);
                span_point_records.push(point_records);
                span_wall_kn_m.push(walls_kn_m);
            }
            if spans.len() != line.spans.len() {
                continue;
            }
            let moments = continuous_beam_with_point_loads(&stiff, &span_points);
            // Solve quasi-permanent end moments directly on (w_qp, pts_qp) so
            // permanent point loads never inflate hogging moments over reduced w_qp.
            let qp_moments = continuous_beam_with_point_loads(&qp_stiff, &span_pts_qp);
            let mut reactions = vec![0.0; line.joints.len()];
            let mut wall_reactions = vec![0.0; line.joints.len()];
            let mut results = Vec::new();
            let count = spans.len();
            let psi2_q = settings.occupancy.psi2();
            for (
                i,
                (((id, length, g1_dead, g2_dead, live, b, h), (left, right)), (qp_left, qp_right)),
            ) in spans.iter().zip(&moments).zip(&qp_moments).enumerate()
            {
                let dead = *g1_dead + *g2_dead;
                let w = dead + live;
                let pts = &span_points[i];
                let pts_qp = &span_pts_qp[i];
                let _point_records = &span_point_records[i];
                let p_sum: f64 = pts.iter().map(|(_, p)| *p).sum();
                let point_v_left: f64 = pts
                    .iter()
                    .map(|&(a_pos, p)| p * (length - a_pos) / length)
                    .sum();
                let v_left = w * length / 2.0 + point_v_left + (right - left) / length;
                let v_right = v_left - w * length - p_sum;
                reactions[i] += v_left;
                reactions[i + 1] -= v_right;
                let wall_span_load = span_wall_kn_m[i] * length * 0.5;
                wall_reactions[i] += wall_span_load;
                wall_reactions[i + 1] += wall_span_load;
                let moment_at = |x: f64| {
                    let mut m = left + v_left * x - w * x * x / 2.0;
                    for &(a_pos, p) in pts {
                        if x > a_pos {
                            m -= p * (x - a_pos);
                        }
                    }
                    m
                };
                let span_moment = if pts.is_empty() {
                    let x = (v_left / w.max(1e-9)).clamp(0.0, *length);
                    moment_at(x)
                } else {
                    (0..=100)
                        .map(|k| moment_at(*length * k as f64 / 100.0))
                        .fold(f64::NEG_INFINITY, f64::max)
                };
                // Quasi-permanent load (NBR 8681 / NBR 6118 Table 11.2 ψ2), exact
                // quasi-permanent moments, Branson's equivalent inertia with Mr and
                // CA-50 steel, plus creep factor 1 + 1.32 (NBR 6118 §17.3.2.1).
                let w_qp = dead + psi2_q * live;
                let p_qp_v_left: f64 = pts_qp
                    .iter()
                    .map(|&(a_pos, p)| p * (length - a_pos) / length)
                    .sum();
                let v_left_qp = w_qp * length / 2.0 + p_qp_v_left + (qp_right - qp_left) / length;
                let qp_moment_at = |x: f64| {
                    let mut m = qp_left + v_left_qp * x - w_qp * x * x / 2.0;
                    for &(a_pos, p) in pts_qp {
                        if x > a_pos {
                            m -= p * (x - a_pos);
                        }
                    }
                    m
                };
                let span_moment_qp = if pts_qp.is_empty() {
                    let x = (v_left_qp / w_qp.max(1e-9)).clamp(0.0, *length);
                    qp_moment_at(x)
                } else {
                    (0..=100)
                        .map(|k| qp_moment_at(*length * k as f64 / 100.0))
                        .fold(f64::NEG_INFINITY, f64::max)
                };
                let m_k_max = span_moment.max(0.0).max(left.abs()).max(right.abs());
                let m_a_qp = span_moment_qp
                    .max(0.0)
                    .max(qp_left.abs())
                    .max(qp_right.abs());
                let stiffness =
                    effective_beam_stiffness(*b, *h, settings.fck_mpa, modulus, m_k_max, m_a_qp);
                let ei = modulus * stiffness.i_eq_m4;
                let deflection = span_deflection_with_point_loads(
                    *length, w_qp, pts_qp, *qp_left, *qp_right, ei,
                ) * (1.0 + 1.32);
                let end_span = i == 0 || i + 1 == count;
                let divisor = if count > 1 && !end_span { 12.0 } else { 10.0 };
                results.push(SpanResult {
                    beam: *id,
                    length_m: *length,
                    width_m: *b,
                    height_m: *h,
                    dead_kn_m: dead,
                    g1_dead_kn_m: *g1_dead,
                    g2_dead_kn_m: *g2_dead,
                    live_kn_m: *live,
                    point_loads: span_point_records[i]
                        .iter()
                        .map(|point| SpanPointLoad {
                            position_m: point.t,
                            load_kn: point.load_kn,
                            quasi_factor: point.quasi_factor,
                        })
                        .collect(),
                    moment_left: *left,
                    moment_right: *right,
                    moment_span: span_moment.max(0.0),
                    shear: v_left.abs().max(v_right.abs()),
                    deflection_mm: deflection * 1000.0,
                    limit_mm: length / 250.0 * 1000.0,
                    cracking_moment_knm: stiffness.m_r_knm,
                    cracked: m_a_qp > stiffness.m_r_knm + 1e-9,
                    i_eq_ratio: (stiffness.i_eq_m4 / stiffness.ic_m4.max(1e-12)).clamp(0.0, 1.0),
                    branson_as_cm2: stiffness.as_cm2,
                    height_min_m: round_up(length / divisor, 0.05).max(0.25),
                });
            }
            for (((joint, support), reaction), wall_r) in line
                .joints
                .iter()
                .zip(&line.supports)
                .zip(&reactions)
                .zip(&wall_reactions)
            {
                match *support {
                    BeamSupport::Column { id } => {
                        // An upward reaction does not relieve the column below.
                        *column_load.entry(id).or_default() += reaction.max(0.0);
                        *column_wall_load.entry(id).or_default() += *wall_r;
                    }
                    BeamSupport::Beam { beam_id, t } => {
                        if *reaction > 0.0 {
                            beam_point_loads
                                .entry(beam_id)
                                .or_default()
                                .push(BeamPointLoad {
                                    t,
                                    load_kn: *reaction,
                                    // A supporting beam receives a conservative permanent
                                    // reaction until dead/live components are separated.
                                    quasi_factor: 1.0,
                                });
                        }
                    }
                }
                if *reaction < -0.5 {
                    report.warnings.push(format!(
                        "{}: o apoio {} é puxado para cima ({} kN); evite um vão muito curto ao lado de um longo.",
                        line.label,
                        label(*joint),
                        decimal(-reaction, 1)
                    ));
                }
            }
            for span in &results {
                if span.height_m + 1e-6 < span.height_min_m {
                    report.warnings.push(format!(
                        "{}: vão de {} m pede altura de pelo menos {:.0} cm (tem {:.0} cm).",
                        line.label,
                        decimal(span.length_m, 2),
                        span.height_min_m * 100.0,
                        span.height_m * 100.0
                    ));
                }
                if span.deflection_mm > span.limit_mm {
                    report.warnings.push(format!(
                        "{}: flecha de {} mm passa do limite L/250 ({} mm); aumente a altura.",
                        line.label,
                        decimal(span.deflection_mm, 1),
                        decimal(span.limit_mm, 1)
                    ));
                }
                if span.width_m < 0.12 - 1e-9 {
                    report.warnings.push(format!(
                        "{}: largura abaixo de 12 cm (NBR 6118 13.2.2).",
                        line.label
                    ));
                }
            }
            report.beams.push(BeamLineResult {
                label: line.label.clone(),
                joints: line.joints.clone(),
                spans: results,
                reactions,
            });
        }

        // Columns.
        let fcd = settings.fck_mpa.max(10.0) / 1.4 / 10.0; // kN/cm²
        let capacity = 0.85 * fcd + 0.015 * 42.0;
        let mut columns: Vec<&Fixture> = self.structural_columns().collect();
        columns.sort_by(|a, b| reading(a.center, b.center));
        for column in columns {
            let directions = incident.get(&column.id).cloned().unwrap_or_default();
            let mut distinct: Vec<Point> = Vec::new();
            for d in directions {
                if !distinct.iter().any(|e| e.dot(d) > 0.98) {
                    distinct.push(d);
                }
            }
            let position = match distinct.len() {
                0 => ColumnPosition::Isolated,
                1 => ColumnPosition::Corner,
                2 if distinct[0].dot(distinct[1]) < -0.98 => ColumnPosition::Edge,
                2 => ColumnPosition::Corner,
                3 => ColumnPosition::Edge,
                _ => ColumnPosition::Interior,
            };
            // As tall as the tallest wall it stands in (#145).
            let height = self.column_height(column);
            let own = CONCRETE_KN_M3 * column.width * column.depth * height;
            let upper_load_kn = incoming_column_load.get(&column.id).copied().unwrap_or(0.0);
            // The roof (#114) only bears on the top: never multiplied.
            let roof = roof_on_column.get(&column.id).copied().unwrap_or(0.0);
            let load = if use_multiplier {
                // `column_load` already contains 1 storey of upper walls when `floors > 1`;
                // subtract 1 storey of walls after multiplying by `floors` so N storeys
                // carry N-1 upper walls total (never N·(N-1)).
                let wall_extra = if floors > 1.0 {
                    column_wall_load.get(&column.id).copied().unwrap_or(0.0)
                } else {
                    0.0
                };
                ((column_load.get(&column.id).copied().unwrap_or(0.0) + own) * floors - wall_extra)
                    .max(0.0)
                    + roof
            } else {
                column_load.get(&column.id).copied().unwrap_or(0.0) + own + upper_load_kn + roof
            };
            let needed = 1.4 * position.factor() * load / capacity;
            let result = ColumnResult {
                id: column.id,
                label: label(column.id),
                width_m: column.width,
                depth_m: column.depth,
                position,
                load_kn: load,
                upper_load_kn,
                area_needed_cm2: needed,
                area_cm2: column.width * column.depth * 10_000.0,
                height_m: height,
                slenderness: crate::domain::wall_heights::slenderness(
                    height,
                    column.width.min(column.depth),
                ),
            };
            if position == ColumnPosition::Isolated {
                report
                    .warnings
                    .push(format!("{}: nenhuma viga chega ao pilar.", result.label));
            } else if !result.slender_ok() {
                report.warnings.push(format!(
                    "{}: esbeltez λ = {:.0} com {} m de altura e {:.0} cm de lado; acima de 90 o pré-dimensionamento não vale: aumente o menor lado para {:.0} cm.",
                    result.label,
                    result.slenderness,
                    decimal(height, 2),
                    column.width.min(column.depth) * 100.0,
                    crate::domain::wall_heights::side_for_slenderness(height) * 100.0
                ));
            } else if !result.ok() {
                report.warnings.push(format!(
                    "{}: seção de {} cm² abaixo do necessário ({:.0} cm², mínimo 360 cm² e 14 cm de lado).",
                    result.label,
                    decimal(result.area_cm2, 0),
                    needed.max(360.0)
                ));
            }
            report.columns.push(result);
        }

        // Footings. Balance beams (#144) centre the boundary footings under
        // the lever's reaction and relieve the interior columns.
        // Ground beams (#143) bring the walls' weight down to the footings.
        let ground = self.wall_foundation_effects();
        let balance = self.balance_effects(&report.columns, &ground.at_column);
        let mut footings: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
            .collect();
        footings.sort_by(|a, b| reading(a.center, b.center));
        for footing in footings {
            let col_fixture = report
                .columns
                .iter()
                .filter_map(|c| Some((c, self.column(c.id)?)))
                .find(|(_, col)| footing.contains(col.center));
            let column = col_fixture.map(|(c, _)| c);
            let centred = balance.centred_load.get(&footing.id).copied();
            let load = centred.unwrap_or_else(|| {
                column.map_or(0.0, |c| {
                    (c.load_kn + ground.at_column.get(&c.id).copied().unwrap_or(0.0)
                        - balance.relief.get(&c.id).copied().unwrap_or(0.0))
                    .max(0.0)
                })
            });
            let area = 1.05 * load / settings.soil_kpa.max(10.0);
            // Same overhang on all sides of the column.
            let ((cw, cd), (ex, ey)) = col_fixture.map_or(((0.0, 0.0), (0.0, 0.0)), |(_, col)| {
                let dims = if col.quarter_turns % 2 == footing.quarter_turns % 2 {
                    (col.width, col.depth)
                } else {
                    (col.depth, col.width)
                };
                let local = footing.to_local(col.center);
                // The balance beam takes the moment of the offset.
                if centred.is_some() {
                    (dims, (0.0, 0.0))
                } else {
                    (dims, (local.x.abs(), local.y.abs()))
                }
            });
            let diff = cw - cd;
            let depth = ((-diff + (diff * diff + 4.0 * area).sqrt()) / 2.0).max(0.0);
            let needed_depth = round_up(depth, 0.05).max(0.60);
            let needed_width = round_up(depth + diff, 0.05).max(0.60);
            // Rigid-footing cantilever includes any column eccentricity (ex, ey).
            let overhang =
                (((footing.width - cw) / 2.0 + ex).max((footing.depth - cd) / 2.0 + ey)).max(0.0);
            let height_m = round_up((overhang * 2.0 / 3.0).max(0.25), 0.05);
            let check = check_footing(
                load,
                footing.width,
                footing.depth,
                height_m,
                cw,
                cd,
                ex,
                ey,
                settings.fck_mpa,
            );
            let settlement_mm = if column.is_some() {
                let (n_bar, soil_kind) = self
                    .structure
                    .soil_survey
                    .as_ref()
                    .and_then(|s| s.governing_footing_bulb(footing.width.max(footing.depth)))
                    .map(|(n, kind, _)| (n, kind))
                    .unwrap_or((
                        settings.soil_kpa / 20.0,
                        crate::domain::soil::SoilKind::Clay,
                    ));
                let mean_q = 1.05 * load / (footing.width * footing.depth).max(1e-6);
                crate::domain::soil::estimate_footing_settlement_mm(
                    mean_q,
                    footing.width,
                    footing.depth,
                    soil_kind,
                    n_bar,
                )
            } else {
                0.0
            };
            let result = FootingResult {
                id: footing.id,
                label: label(footing.id),
                column: column.map(|c| c.label.clone()),
                width_m: footing.width,
                depth_m: footing.depth,
                load_kn: load,
                needed_width_m: needed_width,
                needed_depth_m: needed_depth,
                height_m,
                eccentricity_m: ex.hypot(ey),
                max_pressure_kpa: check.max_pressure_kpa,
                soil_kpa: settings.soil_kpa,
                punching_ratio: check.punching_ratio,
                overturning_fs: check.overturning_fs,
                settlement_mm,
            };
            if result.column.is_none() {
                report
                    .warnings
                    .push(format!("{}: sapata sem pilar em cima.", result.label));
            } else {
                if result.width_m * result.depth_m + 1e-6
                    < result.needed_width_m * result.needed_depth_m
                {
                    report.warnings.push(format!(
                        "{}: {} m² é pouco para {:.0} kN no solo de {:.0} kPa; use {}.",
                        result.label,
                        decimal(result.width_m * result.depth_m, 2),
                        load,
                        settings.soil_kpa,
                        section_cm(needed_width, needed_depth)
                    ));
                } else if result.max_pressure_kpa > settings.soil_kpa + 1e-3 {
                    report.warnings.push(format!(
                        "{}: excentricidade de {:.0} cm eleva a tensão no solo para {:.0} kPa (limite {:.0} kPa); centralize ou aumente a sapata.",
                        result.label,
                        result.eccentricity_m * 100.0,
                        result.max_pressure_kpa,
                        settings.soil_kpa
                    ));
                }
                if result.overturning_fs < 1.5 {
                    report.warnings.push(format!(
                        "{}: risco de tombamento (FS = {} < 1,5, excentricidade {:.0} cm); centralize a sapata ou use viga de equilíbrio.",
                        result.label,
                        decimal(result.overturning_fs, 2),
                        result.eccentricity_m * 100.0
                    ));
                } else if check.outside_middle_third {
                    report.warnings.push(format!(
                        "{}: carga fora do núcleo central (excentricidade {:.0} cm, descolamento parcial da base).",
                        result.label,
                        result.eccentricity_m * 100.0
                    ));
                }
                if result.punching_ratio > 1.0 + 1e-6 {
                    report.warnings.push(format!(
                        "{}: verificação à punção excedida ({:.0} % do limite NBR 6118); aumente a altura da sapata (h atual {:.0} cm).",
                        result.label,
                        result.punching_ratio * 100.0,
                        result.height_m * 100.0
                    ));
                }
                if result.settlement_mm > crate::domain::soil::MAX_FOOTING_SETTLEMENT_MM + 1e-6 {
                    report.warnings.push(format!(
                        "{}: recalque elástico estimado de {} mm excede {:.0} mm (NBR 6122); aumente a base ou avalie radier/estacas.",
                        result.label,
                        decimal(result.settlement_mm, 1),
                        crate::domain::soil::MAX_FOOTING_SETTLEMENT_MM
                    ));
                }
            }
            report.footings.push(result);
        }
        for slab in &report.slabs {
            if !slab.supported {
                report.warnings.push(format!(
                    "{} ({}): nenhuma viga ao longo das bordas da laje.",
                    slab.label, slab.room
                ));
            } else if slab.thickness_m + 1e-6 < slab.thickness_min_m {
                report.warnings.push(format!(
                    "{} ({}): {:.0} cm é pouco para o vão de {} m; use {:.0} cm.",
                    slab.label,
                    slab.room,
                    slab.thickness_m * 100.0,
                    decimal(slab.lx_m, 2),
                    slab.thickness_min_m * 100.0
                ));
            }
            if slab.kind == SlabKind::Solid && slab.deflection_mm > slab.deflection_limit_mm + 1e-6
            {
                report.warnings.push(format!(
                    "{} ({}): flecha final de {} mm excede L/250 = {} mm; aumente a espessura ou reduza o vão.",
                    slab.label,
                    slab.room,
                    decimal(slab.deflection_mm, 1),
                    decimal(slab.deflection_limit_mm, 1)
                ));
            }
        }
        for beam in &balance.results {
            if beam.uplift() {
                report.warnings.push(format!(
                    "{}: o alívio de {} kN levanta o pilar {} ({} kN); aumente o peso no apoio interno ou use sapata corrida.",
                    beam.label,
                    decimal(beam.relief_kn, 1),
                    beam.anchor,
                    decimal(beam.anchor_load_kn, 1)
                ));
            } else if !beam.overturning_ok() {
                report.warnings.push(format!(
                    "{}: fator de segurança ao tombamento do par FS = {} < 1,5 (NBR 6122); aumente o contrapeso em {} ou adote sapata corrida na divisa.",
                    beam.label,
                    decimal(beam.pair_overturning_fs, 2),
                    beam.anchor
                ));
            }
            if !beam.pair_soil_ok() {
                report.warnings.push(format!(
                    "{}: tensão no solo do par ({:.0}/{:.0} kPa) excede {:.0} kPa (NBR 6122); use Ajustar sapatas.",
                    beam.label,
                    beam.boundary_pressure_kpa.unwrap_or(0.0),
                    beam.anchor_pressure_kpa.unwrap_or(0.0),
                    beam.soil_kpa
                ));
            }
            if beam.height_m.is_none() {
                report.warnings.push(format!(
                    "{}: nenhuma seção até 120 cm de altura resiste ao momento de {} kN·m; aproxime o pilar interno ou aumente a largura.",
                    beam.label,
                    decimal(beam.moment_knm, 1)
                ));
            }
        }
        report.balance_beams = balance.results;
        // Wind on the tall walls and the roofs (#147).
        let wind = self.wind_effects(&report);
        for column in wind.columns.iter().filter(|c| c.ratio > 1.0 + 1e-9) {
            report.warnings.push(format!(
                "{}: o vento na parede de {} m ({}) pede {} atravessando a parede; hoje tem {:.0} cm.",
                column.label,
                decimal(column.height_m, 2),
                if column.braced {
                    "topo travado pelo telhado"
                } else {
                    "pilar em balanço"
                },
                column
                    .needed_depth_m
                    .map_or("mais de 80 cm".to_owned(), |h| format!("{:.0} cm", h * 100.0)),
                column.depth_m * 100.0
            ));
        }
        for roof in wind.roofs.iter().filter(|r| r.lifts()) {
            report.warnings.push(format!(
                "Telhado {}: o vento levanta o telhado{} ({} kN/m² de sucção contra {} kN/m² de peso); ancore cada ponta de viga para {} kN e amarre as telhas.",
                roof.label,
                if roof.open { ", que tem lado aberto" } else { "" },
                decimal(roof.uplift_kpa, 2),
                decimal(roof.dead_kpa, 2),
                decimal(roof.anchor_kn, 1)
            ));
        }
        for brace in wind.bracing.iter().filter(|brace| !brace.ok()) {
            report.warnings.push(format!(
                "Telhado {}: o caminho de contraventamento até as fundações pede {:.1} kN de resistência lateral de cálculo; resistência verificada {}.",
                brace.label,
                brace.demand_kn,
                brace.capacity_kn.map_or("não informada".to_owned(), |kn| format!("{kn:.1} kN"))
            ));
        }
        for global in wind.global.iter().filter(|global| !global.ok()) {
            report.warnings.push(format!(
                "Estabilidade global {}: α = {:.2}/{:.2}, deslocamento = {:.1}/{:.1} mm, momento estabilizante/solicitante = {:.2}; confira apoios, travamento e fundações.",
                global.direction, global.alpha, global.alpha_limit,
                global.drift_mm, global.drift_limit_mm, global.overturning_ratio
            ));
        }
        report.wind = wind;
        // The survey (#112) may allow less than the pressure adopted.
        if let Some(survey) = &self.structure.soil_survey {
            let widest = report
                .footings
                .iter()
                .map(|f| f.width_m.max(f.depth_m))
                .fold(1.0, f64::max);
            match survey.governing_footing_bulb(widest) {
                Some((n_bar, _, gov)) => {
                    let kpa = (20.0 * n_bar).min(crate::domain::soil::MAX_FOOTING_KPA);
                    if kpa + 1e-6 < settings.soil_kpa {
                        report.warnings.push(format!(
                            "A sondagem ({gov}) indica {:.0} kPa na cota das sapatas ({} m), menos que os {:.0} kPa adotados: use a sondagem na tensão do solo.",
                            kpa,
                            decimal(survey.footing_depth_m, 2),
                            settings.soil_kpa
                        ));
                    }
                }
                None => report.warnings.push(format!(
                    "A sondagem {} não chega à cota das sapatas ({} m).",
                    survey.name,
                    decimal(survey.footing_depth_m, 2)
                )),
            }
        }
        for result in &ground.results {
            if result.unsupported {
                report.warnings.push(format!(
                    "{}: baldrame solicitado sem dois apoios ligados à rede de pilares e baldrames; foi analisada sapata corrida sob a parede.",
                    result.label
                ));
            }
            for (column_id, reaction) in &result.reactions {
                if *reaction < -1e-6 {
                    report.warnings.push(format!(
                        "{}: reação ascendente de {:.1} kN no pilar {}. Verifique a ligação viga-pilar, a ancoragem e a estabilidade do apoio com as demais cargas.",
                        result.label,
                        -reaction,
                        column_id
                    ));
                }
            }
            if result.kind == crate::domain::wall_foundations::WallFoundationKind::GroundBeam
                && result.height_m.is_none()
            {
                report.warnings.push(format!(
                    "{}: nenhuma seção de baldrame até 80 cm resiste ({} kN·m); acrescente pilares na parede.",
                    result.label,
                    decimal(result.moment_knm, 1)
                ));
            }
            if result.kind == crate::domain::wall_foundations::WallFoundationKind::StripFooting
                && result.pressure_kpa > result.soil_kpa + 1e-6
            {
                report.warnings.push(format!(
                    "{}: largura de {:.0} cm é pouca para o solo; use {:.0} cm.",
                    result.label,
                    result.width_m * 100.0,
                    result.needed_width_m * 100.0
                ));
            }
        }
        report.wall_foundations = ground.results;
        report.masonry = self.masonry_structure_report();
        report
            .warnings
            .extend(report.masonry.warnings.iter().cloned());
        report.slabs.sort_by(|a, b| natural(&a.label, &b.label));
        report.beams.sort_by(|a, b| natural(&a.label, &b.label));
        // NBR 6120 load cases, NBR 6123 building wind and NBR 8681 combinations (#105).
        report.loads = crate::domain::loads::analyze_project_loads(self, &report);
        if report.loads.nbr6120.total_g_kn() > 1e-6 {
            for dir in &report.loads.building_wind.directions {
                if !dir.overturning_ok() {
                    report.warnings.push(format!(
                        "Vento na edificação ({}): momento estabilizante em ELU-EQU ({} kN·m) abaixo do momento solicitante de cálculo ({} kN·m); verifique estabilidade global e ancoragens.",
                        dir.direction.label(),
                        decimal(dir.stabilizing_moment_knm, 1),
                        decimal(
                            crate::domain::loads::GAMMA_W * dir.overturning_moment_knm,
                            1
                        )
                    ));
                }
            }
        }
        let frame_analysis = crate::domain::frame2d::analyze_project_frames(self, &report);
        report.frames = frame_analysis.frames;
        report.frame_failures = frame_analysis.failures;
        report.frames.sort_by(|a, b| natural(&a.label, &b.label));
        report
            .frame_failures
            .sort_by(|a, b| natural(&a.label, &b.label));
        for failure in &report.frame_failures {
            report.warnings.push(format!(
                "{}: análise 2D não concluída — {}. Revise apoios, vínculos e geometria.",
                failure.label, failure.reason
            ));
        }
        for frame in &report.frames {
            if !frame.drift_ok() {
                report.warnings.push(format!(
                    "{}: deslocamento lateral ({:.2} mm) excede H/850 ({:.2} mm); enrijeça os pilares ou adicione contraventamento.",
                    frame.label, frame.max_lateral_drift_mm, frame.drift_limit_mm
                ));
            }
            if frame.envelope.has_column_uplift {
                report.warnings.push(format!(
                    "{}: tração/arrancamento em pilar sob vento ({}, N_mín = {} kN); verifique ancoragem e contrapeso na fundação.",
                    frame.label,
                    frame.envelope.elu_min_column_axial_combo,
                    decimal(frame.envelope.elu_min_column_axial_kn, 1)
                ));
            }
        }
        report
    }
}
