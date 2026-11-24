//! Structural simulation results on the plan and on paper (#106):
//! utilisation traffic-light colours on slabs/beams/columns/footings,
//! bending-moment and shear diagrams directly over the beams on the plan,
//! exaggerated deformed shapes, and the load path from slab to soil — all
//! with overlap-free labels shared identically by the screen canvas and
//! exported SVG/PDF drawings (*screen equals paper*).

use serde::{Deserialize, Serialize};

use super::{
    electrical::tag_size,
    fixtures::FixtureKind,
    geometry::Point,
    label_layout::Area,
    model::Project,
    structure::{StructureReport, beam_moment_at, beam_shear_at},
    verdict::Light,
};

/// Which internal-force diagram (if any) is drawn directly over beams on the plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeamDiagramOverlay {
    #[default]
    None,
    /// Bending moment diagram M(x) (kN·m), sagging/hogging separated by colour.
    Moment,
    /// Shear force diagram V(x) (kN).
    Shear,
}

impl BeamDiagramOverlay {
    pub fn key(self) -> &'static str {
        match self {
            Self::None => "nenhum",
            Self::Moment => "momento",
            Self::Shear => "cortante",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key.trim().to_lowercase().as_str() {
            "nenhum" | "none" | "off" => Some(Self::None),
            "momento" | "m" | "moment" => Some(Self::Moment),
            "cortante" | "v" | "shear" => Some(Self::Shear),
            _ => None,
        }
    }
}

/// Toggleable simulation overlays shared between the on-screen canvas and
/// exported SVG/PDF drawings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationOverlay {
    #[serde(default)]
    pub utilisation_colors: bool,
    #[serde(default)]
    pub beam_diagrams: BeamDiagramOverlay,
    #[serde(default)]
    pub deformed_shape: bool,
    #[serde(default)]
    pub load_path: bool,
}

impl SimulationOverlay {
    pub fn is_empty(&self) -> bool {
        !self.utilisation_colors
            && self.beam_diagrams == BeamDiagramOverlay::None
            && !self.deformed_shape
            && !self.load_path
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlabUtilisationOverlay {
    pub id: u64,
    pub label: String,
    pub polygon: Vec<Point>,
    pub centroid: Point,
    pub utilisation: f64,
    pub light: Light,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamUtilisationOverlay {
    pub beam_id: u64,
    pub label: String,
    pub start: Point,
    pub end: Point,
    pub width_m: f64,
    pub utilisation: f64,
    pub light: Light,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PointUtilisationOverlay {
    pub id: u64,
    pub label: String,
    pub center: Point,
    pub width_m: f64,
    pub depth_m: f64,
    pub angle_rad: f64,
    pub utilisation: f64,
    pub light: Light,
}

/// One beam's internal-force diagram polygon projected orthogonally along the beam axis.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanBeamDiagram {
    pub beam_id: u64,
    pub kind: BeamDiagramOverlay,
    pub start: Point,
    pub end: Point,
    /// Positive lobe polygons (sagging M > 0 or V > 0) in plan metres.
    pub positive_lobes: Vec<Vec<Point>>,
    /// Negative lobe polygons (hogging M < 0 or V < 0) in plan metres.
    pub negative_lobes: Vec<Vec<Point>>,
    /// Complete diagram boundary curve in plan metres.
    pub curve: Vec<Point>,
}

/// One beam's exaggerated deflected shape on the plan.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanDeformedBeam {
    pub beam_id: u64,
    pub start: Point,
    pub end: Point,
    pub curve: Vec<Point>,
    pub deflection_mm: f64,
    pub limit_mm: f64,
    pub light: Light,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadPathStage {
    SlabToBeam,
    BeamToColumn,
    ColumnToFooting,
}

/// A directed segment of the gravity load path from slab to soil.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoadPathArrow {
    pub from: Point,
    pub to: Point,
    pub stage: LoadPathStage,
    pub load_kn: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayLabelTone {
    Green,
    Yellow,
    Red,
    MomentPos,
    MomentNeg,
    Shear,
    LoadPath,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverlayLabel {
    pub anchor: Point,
    pub text: String,
    pub tone: OverlayLabelTone,
}

/// Full plan-space simulation overlay ready for both egui canvas and SVG/PDF export.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanSimulationOverlay {
    pub slabs: Vec<SlabUtilisationOverlay>,
    pub beams: Vec<BeamUtilisationOverlay>,
    pub columns: Vec<PointUtilisationOverlay>,
    pub footings: Vec<PointUtilisationOverlay>,
    pub beam_diagrams: Vec<PlanBeamDiagram>,
    pub deformed_beams: Vec<PlanDeformedBeam>,
    pub load_paths: Vec<LoadPathArrow>,
    pub labels: Vec<OverlayLabel>,
}

impl PlanSimulationOverlay {
    pub fn is_empty(&self) -> bool {
        self.slabs.is_empty()
            && self.beams.is_empty()
            && self.columns.is_empty()
            && self.footings.is_empty()
            && self.beam_diagrams.is_empty()
            && self.deformed_beams.is_empty()
            && self.load_paths.is_empty()
            && self.labels.is_empty()
    }
}

fn light_tone(light: Light) -> OverlayLabelTone {
    match light {
        Light::Green => OverlayLabelTone::Green,
        Light::Yellow => OverlayLabelTone::Yellow,
        Light::Red => OverlayLabelTone::Red,
    }
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// Canonical unit normal pointing consistently to one side of a beam segment.
fn beam_normal(a: Point, b: Point) -> Point {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).hypot(0.0).max(1e-6);
    // Prefer consistent downward/rightward positive sagging side in plan space.
    let nx = -dy / len;
    let ny = dx / len;
    if ny < -1e-6 || (ny.abs() <= 1e-6 && nx < 0.0) {
        Point::new(-nx, -ny)
    } else {
        Point::new(nx, ny)
    }
}

fn lerp_point(a: Point, b: Point, t: f64) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Splits a signed value curve along `[a, b]` into positive and negative filled polygons.
fn split_diagram_lobes(
    a: Point,
    b: Point,
    normal: Point,
    samples: &[(f64, f64)],
    scale_m_per_unit: f64,
) -> (Vec<Vec<Point>>, Vec<Vec<Point>>, Vec<Point>) {
    let mut curve = Vec::with_capacity(samples.len());
    for &(t, val) in samples {
        let base = lerp_point(a, b, t);
        let offset = val * scale_m_per_unit;
        curve.push(Point::new(
            base.x + normal.x * offset,
            base.y + normal.y * offset,
        ));
    }

    let mut pos_lobes: Vec<Vec<Point>> = Vec::new();
    let mut neg_lobes: Vec<Vec<Point>> = Vec::new();
    if samples.len() < 2 {
        return (pos_lobes, neg_lobes, curve);
    }

    let push_lobe = |positive: bool,
                     start_t: f64,
                     end_t: f64,
                     pts: Vec<Point>,
                     pos_lobes: &mut Vec<Vec<Point>>,
                     neg_lobes: &mut Vec<Vec<Point>>| {
        if pts.len() < 2 {
            return;
        }
        let mut poly = Vec::with_capacity(pts.len() + 2);
        poly.push(lerp_point(a, b, start_t));
        poly.extend(pts);
        poly.push(lerp_point(a, b, end_t));
        if positive {
            pos_lobes.push(poly);
        } else {
            neg_lobes.push(poly);
        }
    };

    let mut cur_sign: Option<bool> = None;
    let mut cur_start_t = 0.0;
    let mut cur_pts: Vec<Point> = Vec::new();

    for i in 0..samples.len() {
        let (t, val) = samples[i];
        let pt = curve[i];
        let sign = val >= 0.0;
        match cur_sign {
            None => {
                cur_sign = Some(sign);
                cur_start_t = t;
                cur_pts.push(pt);
            }
            Some(prev_sign) if prev_sign == sign => {
                cur_pts.push(pt);
            }
            Some(prev_sign) => {
                let (t0, v0) = samples[i - 1];
                let frac = if (val - v0).abs() > 1e-12 {
                    (-v0 / (val - v0)).clamp(0.0, 1.0)
                } else {
                    0.5
                };
                let t_zero = t0 + (t - t0) * frac;
                let p_zero = lerp_point(a, b, t_zero);
                cur_pts.push(p_zero);
                push_lobe(
                    prev_sign,
                    cur_start_t,
                    t_zero,
                    std::mem::take(&mut cur_pts),
                    &mut pos_lobes,
                    &mut neg_lobes,
                );
                cur_sign = Some(sign);
                cur_start_t = t_zero;
                cur_pts.push(p_zero);
                cur_pts.push(pt);
            }
        }
    }
    if let Some(sign) = cur_sign {
        let last_t = samples.last().map(|(t, _)| *t).unwrap_or(1.0);
        push_lobe(
            sign,
            cur_start_t,
            last_t,
            cur_pts,
            &mut pos_lobes,
            &mut neg_lobes,
        );
    }

    (pos_lobes, neg_lobes, curve)
}

/// Resolves overlay label anchors against pre-existing structural/plan labels
/// and against each other so no two labels collide (B05).
fn resolve_overlay_labels(labels: &mut [OverlayLabel], obstacles: &[Area]) {
    const GAP: f64 = 0.04;
    const RADII: [f64; 6] = [0.18, 0.34, 0.52, 0.74, 1.00, 1.32];
    const ANGLES: usize = 12;

    let mut placed: Vec<Area> = obstacles.to_vec();
    for label in labels.iter_mut() {
        let size = tag_size(&label.text);
        let preferred = Area::centered((label.anchor.x, label.anchor.y), size);
        let collides = |candidate: &Area, boxes: &[Area]| {
            boxes.iter().any(|other| candidate.intersects(other, GAP))
        };
        if !collides(&preferred, &placed) {
            placed.push(preferred);
            continue;
        }
        let mut chosen: Option<(Point, Area)> = None;
        'rings: for &radius in &RADII {
            for step in 0..ANGLES {
                let theta = std::f64::consts::TAU * (step as f64) / (ANGLES as f64);
                let pt = Point::new(
                    label.anchor.x + radius * theta.cos(),
                    label.anchor.y + radius * theta.sin(),
                );
                let candidate = Area::centered((pt.x, pt.y), size);
                if !collides(&candidate, &placed) {
                    chosen = Some((pt, candidate));
                    break 'rings;
                }
            }
        }
        if let Some((pt, area)) = chosen {
            label.anchor = pt;
            placed.push(area);
        } else {
            placed.push(preferred);
        }
    }
}

/// Builds the complete plan-space simulation overlay from the project and its
/// structural report using the active [`SimulationOverlay`] configuration.
pub fn build_plan_simulation_overlay(
    project: &Project,
    report: &StructureReport,
    config: SimulationOverlay,
) -> PlanSimulationOverlay {
    let mut out = PlanSimulationOverlay::default();
    if config.is_empty() {
        return out;
    }

    // Collect pre-existing structure text marks as label obstacles (B05).
    let mut obstacles: Vec<Area> = Vec::new();
    for mark in project.cached_structure_marks().iter() {
        if let super::conduits::PlanMark::Text { anchor, text } = mark {
            obstacles.push(Area::centered((anchor.x, anchor.y), tag_size(text)));
        }
    }

    // Map beam ids to their endpoints.
    let beam_geom = |beam_id: u64| -> Option<(Point, Point, f64)> {
        let beam = project.structure.beams.iter().find(|b| b.id == beam_id)?;
        let (a, b) = project.beam_endpoints(beam)?;
        Some((a, b, beam.width_m))
    };

    // 1. Utilisation traffic-light colours on slabs, beams, columns, footings.
    if config.utilisation_colors {
        for slab in &report.slabs {
            let Some(slab_obj) = project.structure.slabs.iter().find(|s| s.id == slab.id) else {
                continue;
            };
            let Some(room) = project.rooms.iter().find(|r| r.id == slab_obj.room_id) else {
                continue;
            };
            let Some(poly) = project.room_polygon(room) else {
                continue;
            };
            let centroid = super::geometry::polygon_centroid(&poly);
            let u_thick = if slab.thickness_m > 1e-6 {
                slab.thickness_min_m / slab.thickness_m
            } else {
                2.0
            };
            let u_defl = if slab.deflection_limit_mm > 1e-6 {
                slab.deflection_mm / slab.deflection_limit_mm
            } else {
                0.0
            };
            let util = u_thick.max(u_defl);
            let light = Light::from_utilisation(util);
            out.slabs.push(SlabUtilisationOverlay {
                id: slab.id,
                label: slab.label.clone(),
                polygon: poly,
                centroid,
                utilisation: util,
                light,
            });
            out.labels.push(OverlayLabel {
                anchor: Point::new(centroid.x, centroid.y + 0.24),
                text: format!("{} {:.0}%", slab.label, util * 100.0),
                tone: light_tone(light),
            });
        }

        for line in &report.beams {
            for span in &line.spans {
                let Some((a, b, width_m)) = beam_geom(span.beam) else {
                    continue;
                };
                let u_h = if span.height_m > 1e-6 {
                    span.height_min_m / span.height_m
                } else {
                    2.0
                };
                let u_d = if span.limit_mm > 1e-6 {
                    span.deflection_mm / span.limit_mm
                } else {
                    0.0
                };
                let u_w = 0.12 / span.width_m.max(1e-6);
                let util = u_h.max(u_d).max(u_w);
                let light = Light::from_utilisation(util);
                out.beams.push(BeamUtilisationOverlay {
                    beam_id: span.beam,
                    label: line.label.clone(),
                    start: a,
                    end: b,
                    width_m,
                    utilisation: util,
                    light,
                });
                let mid = lerp_point(a, b, 0.5);
                let n = beam_normal(a, b);
                out.labels.push(OverlayLabel {
                    anchor: Point::new(mid.x - n.x * 0.22, mid.y - n.y * 0.22),
                    text: format!("{:.0}%", util * 100.0),
                    tone: light_tone(light),
                });
            }
        }

        for col in &report.columns {
            let Some(fix) = project
                .fixtures
                .iter()
                .find(|f| f.id == col.id && f.kind == FixtureKind::Column)
            else {
                continue;
            };
            let u_area = if col.area_cm2 > 1e-6 {
                col.area_needed_cm2 / col.area_cm2
            } else {
                2.0
            };
            let min_side = col.width_m.min(col.depth_m);
            let u_side = 0.14 / min_side.max(1e-6);
            let u_slender = col.slenderness / 90.0;
            let util = u_area.max(u_side).max(u_slender);
            let light = Light::from_utilisation(util);
            out.columns.push(PointUtilisationOverlay {
                id: col.id,
                label: col.label.clone(),
                center: fix.center,
                width_m: fix.width,
                depth_m: fix.depth,
                angle_rad: fix.rotation_rad(),
                utilisation: util,
                light,
            });
            out.labels.push(OverlayLabel {
                anchor: Point::new(fix.center.x + 0.24, fix.center.y - 0.20),
                text: format!("{} {:.0}%", col.label, util * 100.0),
                tone: light_tone(light),
            });
        }

        for ft in &report.footings {
            let Some(fix) = project
                .fixtures
                .iter()
                .find(|f| f.id == ft.id && f.kind == FixtureKind::Footing)
            else {
                continue;
            };
            let u_soil = if ft.soil_kpa > 1e-6 {
                ft.max_pressure_kpa / ft.soil_kpa
            } else {
                2.0
            };
            let u_punch = ft.punching_ratio;
            let u_over = if ft.overturning_fs.is_finite() && ft.overturning_fs > 1e-6 {
                1.5 / ft.overturning_fs
            } else {
                0.0
            };
            let util = u_soil.max(u_punch).max(u_over);
            let light = Light::from_utilisation(util);
            out.footings.push(PointUtilisationOverlay {
                id: ft.id,
                label: ft.label.clone(),
                center: fix.center,
                width_m: fix.width,
                depth_m: fix.depth,
                angle_rad: fix.rotation_rad(),
                utilisation: util,
                light,
            });
        }
    }

    // 2. Internal-force diagrams directly over the beams on the plan.
    if config.beam_diagrams != BeamDiagramOverlay::None {
        let mut global_peak = 0.0_f64;
        for line in &report.beams {
            for span in &line.spans {
                let peak = match config.beam_diagrams {
                    BeamDiagramOverlay::Moment => span
                        .moment_left
                        .abs()
                        .max(span.moment_right.abs())
                        .max(span.moment_span.abs()),
                    BeamDiagramOverlay::Shear => span.shear.abs(),
                    BeamDiagramOverlay::None => 0.0,
                };
                global_peak = global_peak.max(peak);
            }
        }
        let max_height_m = 0.48;
        let scale = if global_peak > 1e-6 {
            max_height_m / global_peak
        } else {
            0.0
        };

        const SAMPLES: usize = 24;
        for line in &report.beams {
            for span in &line.spans {
                let Some((a, b, _)) = beam_geom(span.beam) else {
                    continue;
                };
                if span.length_m < 1e-4 {
                    continue;
                }
                let n = beam_normal(a, b);
                let mut pts = Vec::with_capacity(SAMPLES + 1);
                let mut max_pos = (0.5_f64, 0.0_f64);
                let mut max_neg = (0.0_f64, 0.0_f64);
                for i in 0..=SAMPLES {
                    let t = (i as f64) / (SAMPLES as f64);
                    let x = t * span.length_m;
                    let val = match config.beam_diagrams {
                        BeamDiagramOverlay::Moment => beam_moment_at(span, x),
                        BeamDiagramOverlay::Shear => beam_shear_at(span, x),
                        BeamDiagramOverlay::None => 0.0,
                    };
                    if val > max_pos.1 {
                        max_pos = (t, val);
                    }
                    if val < max_neg.1 {
                        max_neg = (t, val);
                    }
                    pts.push((t, val));
                }
                let (pos_lobes, neg_lobes, curve) = split_diagram_lobes(a, b, n, &pts, scale);
                out.beam_diagrams.push(PlanBeamDiagram {
                    beam_id: span.beam,
                    kind: config.beam_diagrams,
                    start: a,
                    end: b,
                    positive_lobes: pos_lobes,
                    negative_lobes: neg_lobes,
                    curve,
                });

                let unit = match config.beam_diagrams {
                    BeamDiagramOverlay::Moment => "kN·m",
                    BeamDiagramOverlay::Shear => "kN",
                    BeamDiagramOverlay::None => "",
                };
                if max_pos.1 > 0.1 {
                    let base = lerp_point(a, b, max_pos.0);
                    let offset = (max_pos.1 * scale) + 0.14;
                    out.labels.push(OverlayLabel {
                        anchor: Point::new(base.x + n.x * offset, base.y + n.y * offset),
                        text: format!("+{} {unit}", decimal(max_pos.1, 1)),
                        tone: if config.beam_diagrams == BeamDiagramOverlay::Moment {
                            OverlayLabelTone::MomentPos
                        } else {
                            OverlayLabelTone::Shear
                        },
                    });
                }
                if max_neg.1 < -0.1 {
                    let base = lerp_point(a, b, max_neg.0);
                    let offset = (max_neg.1 * scale) - 0.14;
                    out.labels.push(OverlayLabel {
                        anchor: Point::new(base.x + n.x * offset, base.y + n.y * offset),
                        text: format!("{} {unit}", decimal(max_neg.1, 1)),
                        tone: if config.beam_diagrams == BeamDiagramOverlay::Moment {
                            OverlayLabelTone::MomentNeg
                        } else {
                            OverlayLabelTone::Shear
                        },
                    });
                }
            }
        }
    }

    // 3. Deformed shape directly over beams on the plan.
    if config.deformed_shape {
        let max_defl = report
            .beams
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|s| s.deflection_mm.abs())
            .fold(0.0_f64, f64::max);
        let scale = if max_defl > 1e-6 {
            0.30 / max_defl
        } else {
            0.0
        };
        const SAMPLES: usize = 20;
        for line in &report.beams {
            for span in &line.spans {
                let Some((a, b, _)) = beam_geom(span.beam) else {
                    continue;
                };
                let n = beam_normal(a, b);
                let util = if span.limit_mm > 1e-6 {
                    span.deflection_mm / span.limit_mm
                } else {
                    0.0
                };
                let light = Light::from_utilisation(util);
                // Smooth clamped-to-pinned beam deflection profile peaking at midspan.
                let continuity_factor =
                    if span.moment_left.abs() > 0.1 || span.moment_right.abs() > 0.1 {
                        1.0
                    } else {
                        0.0
                    };
                let mut curve = Vec::with_capacity(SAMPLES + 1);
                for i in 0..=SAMPLES {
                    let t = (i as f64) / (SAMPLES as f64);
                    // Blend simply supported sin(pi*t) with continuous 16*t^2*(1-t)^2 shape.
                    let simple = (std::f64::consts::PI * t).sin();
                    let fixed = 16.0 * t * t * (1.0 - t) * (1.0 - t);
                    let shape =
                        (1.0 - 0.4 * continuity_factor) * simple + 0.4 * continuity_factor * fixed;
                    let base = lerp_point(a, b, t);
                    let disp = span.deflection_mm * scale * shape;
                    curve.push(Point::new(base.x + n.x * disp, base.y + n.y * disp));
                }
                let mid = lerp_point(a, b, 0.5);
                let peak_offset = span.deflection_mm * scale + 0.14;
                let ratio = if span.deflection_mm > 1e-4 {
                    (span.length_m * 1000.0 / span.deflection_mm).round()
                } else {
                    9999.0
                };
                out.labels.push(OverlayLabel {
                    anchor: Point::new(mid.x + n.x * peak_offset, mid.y + n.y * peak_offset),
                    text: format!("δ={} mm (L/{:.0})", decimal(span.deflection_mm, 1), ratio),
                    tone: light_tone(light),
                });
                out.deformed_beams.push(PlanDeformedBeam {
                    beam_id: span.beam,
                    start: a,
                    end: b,
                    curve,
                    deflection_mm: span.deflection_mm,
                    limit_mm: span.limit_mm,
                    light,
                });
            }
        }
    }

    // 4. Gravity load path from slab -> beam -> column -> footing/soil.
    if config.load_path {
        // Stage A: Slabs to their perimeter supporting beams.
        for slab in &report.slabs {
            let Some(slab_obj) = project.structure.slabs.iter().find(|s| s.id == slab.id) else {
                continue;
            };
            let Some(room) = project.rooms.iter().find(|r| r.id == slab_obj.room_id) else {
                continue;
            };
            let Some(poly) = project.room_polygon(room) else {
                continue;
            };
            let centroid = super::geometry::polygon_centroid(&poly);
            let room_area = super::geometry::polygon_area(&poly);
            let total_slab_kn = slab.load_kn_m2 * room_area + slab.point_load_kn;

            let mut edge_targets: Vec<(Point, f64)> = Vec::new();
            for beam in project.valid_beams() {
                let Some((a, b)) = project.beam_endpoints(beam) else {
                    continue;
                };
                let mid = lerp_point(a, b, 0.5);
                // Check if beam lies on one of the room's enclosing walls/edges.
                let on_edge = poly.windows(2).any(|w| {
                    let (nearest, _) = super::geometry::nearest_point_on_segment(mid, w[0], w[1]);
                    nearest.distance(mid) <= 0.35
                }) || (poly.len() >= 3 && {
                    let (nearest, _) = super::geometry::nearest_point_on_segment(
                        mid,
                        *poly.last().unwrap(),
                        poly[0],
                    );
                    nearest.distance(mid) <= 0.35
                });
                if on_edge {
                    edge_targets.push((mid, a.distance(b)));
                }
            }
            let sum_len: f64 = edge_targets.iter().map(|(_, l)| *l).sum();
            if sum_len > 1e-6 {
                for (target, len) in edge_targets {
                    let share_kn = total_slab_kn * (len / sum_len);
                    let to = lerp_point(centroid, target, 0.85);
                    out.load_paths.push(LoadPathArrow {
                        from: centroid,
                        to,
                        stage: LoadPathStage::SlabToBeam,
                        load_kn: share_kn,
                    });
                }
            }
        }

        // Stage B: Beams to their end supports (columns or host beams).
        for line in &report.beams {
            for span in &line.spans {
                let Some((a, b, _)) = beam_geom(span.beam) else {
                    continue;
                };
                let mid = lerp_point(a, b, 0.5);
                let reaction_half =
                    ((span.dead_kn_m + span.live_kn_m) * span.length_m * 0.5).max(span.shear * 0.5);
                if reaction_half > 0.1 {
                    out.load_paths.push(LoadPathArrow {
                        from: mid,
                        to: lerp_point(mid, a, 0.86),
                        stage: LoadPathStage::BeamToColumn,
                        load_kn: reaction_half,
                    });
                    out.load_paths.push(LoadPathArrow {
                        from: mid,
                        to: lerp_point(mid, b, 0.86),
                        stage: LoadPathStage::BeamToColumn,
                        load_kn: reaction_half,
                    });
                }
            }
        }

        // Stage C: Columns into footings / soil.
        for col in &report.columns {
            let Some(col_fix) = project
                .fixtures
                .iter()
                .find(|f| f.id == col.id && f.kind == FixtureKind::Column)
            else {
                continue;
            };
            let footing = project
                .fixtures
                .iter()
                .filter(|f| f.kind == FixtureKind::Footing)
                .min_by(|a, b| {
                    a.center
                        .distance(col_fix.center)
                        .total_cmp(&b.center.distance(col_fix.center))
                })
                .filter(|f| f.center.distance(col_fix.center) <= 1.2);
            let target = footing
                .map(|f| Point::new(f.center.x, f.center.y + f.depth * 0.45))
                .unwrap_or_else(|| Point::new(col_fix.center.x, col_fix.center.y + 0.35));
            out.load_paths.push(LoadPathArrow {
                from: col_fix.center,
                to: target,
                stage: LoadPathStage::ColumnToFooting,
                load_kn: col.load_kn,
            });
            out.labels.push(OverlayLabel {
                anchor: Point::new(target.x + 0.18, target.y + 0.12),
                text: format!("↓{} kN", decimal(col.load_kn, 0)),
                tone: OverlayLabelTone::LoadPath,
            });
        }
    }

    resolve_overlay_labels(&mut out.labels, &obstacles);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_simulation_overlay_builds_diagrams_deformed_shape_utilisation_and_load_path_without_label_overlap()
     {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.5, 3.5), "Sala")
            .unwrap();
        project
            .add_room(Point::new(4.5, 0.0), Point::new(8.0, 3.5), "Quarto")
            .unwrap();
        project.generate_structure();

        let report = project.cached_structure_report();
        let cfg = SimulationOverlay {
            utilisation_colors: true,
            beam_diagrams: BeamDiagramOverlay::Moment,
            deformed_shape: true,
            load_path: true,
        };
        let overlay = build_plan_simulation_overlay(&project, &report, cfg);

        assert_eq!(overlay.slabs.len(), report.slabs.len());
        assert!(!overlay.beams.is_empty());
        assert_eq!(overlay.columns.len(), report.columns.len());
        assert_eq!(overlay.footings.len(), report.footings.len());
        assert!(!overlay.beam_diagrams.is_empty());
        assert!(
            overlay
                .beam_diagrams
                .iter()
                .any(|d| !d.positive_lobes.is_empty())
        );
        assert!(!overlay.deformed_beams.is_empty());
        assert!(
            overlay
                .load_paths
                .iter()
                .any(|p| p.stage == LoadPathStage::SlabToBeam)
        );
        assert!(
            overlay
                .load_paths
                .iter()
                .any(|p| p.stage == LoadPathStage::BeamToColumn)
        );
        assert!(
            overlay
                .load_paths
                .iter()
                .any(|p| p.stage == LoadPathStage::ColumnToFooting)
        );
        assert!(!overlay.labels.is_empty());

        // Verify Shear diagram mode also emits valid shear polygons.
        let shear_overlay = build_plan_simulation_overlay(
            &project,
            &report,
            SimulationOverlay {
                beam_diagrams: BeamDiagramOverlay::Shear,
                ..SimulationOverlay::default()
            },
        );
        assert!(!shear_overlay.beam_diagrams.is_empty());

        // Check that labels were spread apart cleanly (B05 overlap avoidance).
        let mut overlapping_pairs = 0usize;
        for (i, label) in overlay.labels.iter().enumerate() {
            let a = Area::centered((label.anchor.x, label.anchor.y), tag_size(&label.text));
            for other in overlay.labels.iter().skip(i + 1) {
                let b = Area::centered((other.anchor.x, other.anchor.y), tag_size(&other.text));
                if a.intersects(&b, 0.0) {
                    overlapping_pairs += 1;
                }
            }
        }
        assert_eq!(
            overlapping_pairs, 0,
            "simulation overlay labels must avoid overlapping each other"
        );

        // Screen equals paper: both standalone plan SVG and scaled ABNT sheet SVG
        // emit the simulation overlay group when enabled.
        project.structure.overlay = cfg;
        let sheet_svg = crate::io::export::sheet::scaled_sheet_svg(&project);
        assert!(sheet_svg.contains("id=\"simulacao-estrutural\""));
        assert!(sheet_svg.contains("kN·m"));
        assert!(sheet_svg.contains("mm (L/"));
    }
}
