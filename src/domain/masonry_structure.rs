//! Structural masonry panels, lintels (*vergas*) and window sills
//! (*contravergas*) derived from the architectural model (#111).
//!
//! The elements are derived rather than stored: moving or resizing an opening
//! immediately regenerates its lintel/sill and the wall-panel checks. This is a
//! conservative pre-sizing aid under the engineering safety contract, not an
//! executive masonry design.

use super::conduits::PlanMark;
use super::geometry::{Point, distance_to_segment};
use super::masonry::Masonry;
use super::model::{OpeningKind, Project};
use super::quantities::LINTEL_BEARING_M;

/// Minimum bearing used by the check. The generated takeoff still requests
/// the preferred 30 cm on each side whenever the wall has room for it.
pub const MIN_LINTEL_BEARING_M: f64 = 0.20;
/// The simplified lintel method is deliberately limited to ordinary openings.
pub const MAX_LINTEL_SPAN_M: f64 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MasonryWallKind {
    Infill,
    LoadBearing,
}

impl MasonryWallKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Infill => "Vedação",
            Self::LoadBearing => "Estrutural",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MasonryBandKind {
    Lintel,
    Sill,
}

impl MasonryBandKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lintel => "Verga",
            Self::Sill => "Contraverga",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MasonryBandResult {
    pub opening_id: u64,
    pub wall_id: u64,
    pub label: String,
    pub kind: MasonryBandKind,
    pub opening_kind: OpeningKind,
    pub start: Point,
    pub end: Point,
    pub clear_span_m: f64,
    pub bearing_left_m: f64,
    pub bearing_right_m: f64,
    pub width_m: f64,
    pub height_m: f64,
    pub load_kn_m: f64,
    pub moment_knm: f64,
    pub bars: String,
    pub flexure_ratio: f64,
}

impl MasonryBandResult {
    pub fn utilisation(&self) -> f64 {
        let bearing =
            MIN_LINTEL_BEARING_M / self.bearing_left_m.min(self.bearing_right_m).max(1e-9);
        let span = self.clear_span_m / MAX_LINTEL_SPAN_M;
        bearing.max(span).max(self.flexure_ratio)
    }

    pub fn ok(&self) -> bool {
        self.utilisation() <= 1.0 + 1e-9
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MasonryPanelResult {
    pub wall_id: u64,
    pub label: String,
    pub masonry: String,
    pub kind: MasonryWallKind,
    pub length_m: f64,
    pub unbraced_m: f64,
    pub unbraced_limit_m: f64,
    pub height_m: f64,
    pub thickness_m: f64,
    pub slenderness: f64,
    pub slenderness_limit: f64,
    /// Design compression in load-bearing masonry, MPa; zero for infill.
    pub compression_mpa: f64,
    /// Reduced design capacity, MPa; zero for infill.
    pub compression_capacity_mpa: f64,
    pub compression_ratio: f64,
}

impl MasonryPanelResult {
    pub fn utilisation(&self) -> f64 {
        (self.slenderness / self.slenderness_limit.max(1e-9))
            .max(self.unbraced_m / self.unbraced_limit_m.max(1e-9))
            .max(self.compression_ratio)
    }

    pub fn ok(&self) -> bool {
        self.utilisation() <= 1.0 + 1e-9
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MasonryStructureReport {
    pub bands: Vec<MasonryBandResult>,
    pub panels: Vec<MasonryPanelResult>,
    pub warnings: Vec<String>,
}

fn load_bearing(masonry: &Masonry) -> bool {
    masonry.name.to_lowercase().contains("estrutural")
}

/// Conservative characteristic prism strength used only by the simplified
/// load-bearing check. A custom structural material gets the lower value and
/// is explicitly reported as an assumption in the engineering documentation.
fn prism_strength_mpa(masonry: &Masonry) -> f64 {
    let name = masonry.name.to_lowercase();
    if name.contains("concreto") {
        4.0
    } else if name.contains("cerâm") || name.contains("ceram") {
        3.0
    } else {
        2.0
    }
}

fn next_course(required: f64, masonry: &Masonry) -> f64 {
    let course = (masonry.block_height + masonry.joint).max(0.05);
    (required / course - 1e-9).ceil().max(1.0) * course
}

fn bar_area_m2(diameter_mm: f64, count: u32) -> f64 {
    f64::from(count) * std::f64::consts::PI * (diameter_mm / 1000.0).powi(2) / 4.0
}

fn lintel_bars(moment_knm: f64, height_m: f64) -> (String, f64) {
    let d = (height_m - 0.035).max(0.06);
    let fyd_kn_m2 = 500_000.0 / 1.15;
    let required = 1.4 * moment_knm / (0.9 * d * fyd_kn_m2).max(1e-9);
    for diameter in [6.3, 8.0, 10.0, 12.5, 16.0] {
        let provided = bar_area_m2(diameter, 2);
        if provided + 1e-12 >= required.max(bar_area_m2(6.3, 2)) {
            return (
                format!("2Ø{}", format!("{diameter:.1}").replace(".0", "")),
                required / provided.max(1e-12),
            );
        }
    }
    ("acima de 2Ø16".to_owned(), f64::INFINITY)
}

impl Project {
    /// Derives every lintel/sill and checks every masonry wall panel.
    pub fn masonry_structure_report(&self) -> MasonryStructureReport {
        let mut report = MasonryStructureReport::default();
        let storey = self.wall_height.max(0.5);
        let settings = self.structure.settings;
        let mut lintel_no = 0usize;
        let mut sill_no = 0usize;

        for (wall_index, wall) in self.walls.iter().enumerate() {
            let masonry = wall.masonry.as_ref().unwrap_or(&self.masonry);
            let kind = if load_bearing(masonry) {
                MasonryWallKind::LoadBearing
            } else {
                MasonryWallKind::Infill
            };
            let wall_dir = wall
                .end
                .minus(wall.start)
                .normalized()
                .unwrap_or(Point::new(1.0, 0.0));
            let wall_length = wall.length();
            let thickness = masonry.block_width.max(0.04);
            let height = wall.mean_height(storey);

            // Intersections and concrete columns brace a panel. End points are
            // also treated as returns/supports; the longest interval governs.
            let mut braces = vec![0.0, wall_length];
            for other in &self.walls {
                if other.id == wall.id {
                    continue;
                }
                for point in [other.start, other.end] {
                    if distance_to_segment(point, wall.start, wall.end) <= wall.thickness() {
                        braces.push(
                            point
                                .minus(wall.start)
                                .dot(wall_dir)
                                .clamp(0.0, wall_length),
                        );
                    }
                }
            }
            for column in self.structural_columns() {
                if distance_to_segment(column.center, wall.start, wall.end)
                    <= column.width.max(column.depth) * 0.5 + 0.08
                {
                    braces.push(
                        column
                            .center
                            .minus(wall.start)
                            .dot(wall_dir)
                            .clamp(0.0, wall_length),
                    );
                }
            }
            braces.sort_by(f64::total_cmp);
            braces.dedup_by(|a, b| (*a - *b).abs() < 0.05);
            let unbraced = braces
                .windows(2)
                .map(|pair| pair[1] - pair[0])
                .fold(0.0, f64::max);
            let slenderness = height / thickness;
            let slenderness_limit = if kind == MasonryWallKind::LoadBearing {
                24.0
            } else {
                35.0
            };
            let unbraced_limit = if kind == MasonryWallKind::LoadBearing {
                (30.0 * thickness).clamp(2.5, 4.5)
            } else {
                4.5
            };

            let openings_width: f64 = self
                .openings
                .iter()
                .filter(|opening| opening.wall_id == wall.id)
                .map(|opening| opening.width())
                .sum();
            let net_fraction = ((wall_length - openings_width).max(thickness)
                / wall_length.max(thickness))
            .clamp(0.10, 1.0);
            let (compression, capacity, compression_ratio) = if kind == MasonryWallKind::LoadBearing
            {
                let floors = f64::from(settings.floors.max(1));
                // Load-bearing ground-floor walls carry all `floors` storeys of wall dead weight.
                let wall_kn_m = settings.masonry_kn_m3 * thickness * height * floors;
                // Conservative 1.0 m tributary slab strip per storey (declared preliminary assumption).
                let floor_kn_m = (2.5 + settings.finish_kn_m2 + settings.live_kn_m2) * floors;
                let compression =
                    1.4 * (wall_kn_m + floor_kn_m) / (thickness * 1000.0 * net_fraction);
                // NBR 16868-1:2020 §6.2.2 (fk = 0.70·fpk, γm = 2.0) and §11.2.2 (R = 1 - (λ/40)³).
                let reduction = (1.0 - (slenderness / 40.0).powi(3)).clamp(0.0, 1.0);
                let capacity = 0.70 * prism_strength_mpa(masonry) / 2.0 * reduction;
                (compression, capacity, compression / capacity.max(1e-9))
            } else {
                (0.0, 0.0, 0.0)
            };

            let panel = MasonryPanelResult {
                wall_id: wall.id,
                label: format!("AL{}", wall_index + 1),
                masonry: masonry.name.clone(),
                kind,
                length_m: wall_length,
                unbraced_m: unbraced,
                unbraced_limit_m: unbraced_limit,
                height_m: height,
                thickness_m: thickness,
                slenderness,
                slenderness_limit,
                compression_mpa: compression,
                compression_capacity_mpa: capacity,
                compression_ratio,
            };
            if !panel.ok() {
                report.warnings.push(format!(
                    "{}: painel de alvenaria {} não atende (uso {:.0} %); aumente a espessura ou acrescente amarração/pilar intermediário.",
                    panel.label,
                    panel.kind.label().to_lowercase(),
                    panel.utilisation() * 100.0
                ));
            }
            report.panels.push(panel);

            for opening in self.openings.iter().filter(|o| o.wall_id == wall.id) {
                let centre_along = opening.t.clamp(0.0, 1.0) * wall_length;
                let half = opening.width() * 0.5;
                let available_left = (centre_along - half).max(0.0);
                let available_right = (wall_length - centre_along - half).max(0.0);
                let bearing_left = available_left.min(LINTEL_BEARING_M);
                let bearing_right = available_right.min(LINTEL_BEARING_M);
                let centre = wall.start.lerp(wall.end, opening.t);
                let start = centre.translated(wall_dir.scaled(-(half + bearing_left)));
                let end = centre.translated(wall_dir.scaled(half + bearing_right));
                let top = wall.height_over(storey, opening);
                let opening_top = opening.sill() + opening.height().unwrap_or(top);
                let clear_above = (top - opening_top).max(0.0);

                // A full-height opening has no wall above it and therefore no
                // lintel. Every other opening, including gates, gets one.
                if clear_above > (masonry.block_height + masonry.joint) * 0.5 {
                    lintel_no += 1;
                    let l0 = opening.width();
                    let depth = next_course((l0 / 12.0).max(0.10), masonry);
                    // NBR 6118 §14.6.2.4 / NBR 16868-1: effective span includes bearing half-widths.
                    let l_ef = l0
                        + (bearing_left * 0.5).min(depth * 0.5)
                        + (bearing_right * 0.5).min(depth * 0.5);
                    let supported_floor = if kind == MasonryWallKind::LoadBearing {
                        2.5 + settings.finish_kn_m2 + settings.live_kn_m2
                    } else {
                        0.0
                    };
                    let own = 25.0 * thickness * depth;
                    // NBR 16868-1 §10.4.2: a 45° triangular masonry arch forms only when
                    // clear_above >= L/2 (triangular moment M = q_avg·Lef²/6); below L/2
                    // the full rectangular wall strip loads the lintel (M = q·Lef²/8).
                    let (wall_load, moment) = if clear_above >= l0 * 0.5 {
                        let q_avg = 0.5 * (l0 * 0.5) * thickness * settings.masonry_kn_m3;
                        let m_tri = q_avg * l_ef.powi(2) / 6.0;
                        let m_uni = (supported_floor + own) * l_ef.powi(2) / 8.0;
                        (q_avg, m_tri + m_uni)
                    } else {
                        let q_rect = clear_above * thickness * settings.masonry_kn_m3;
                        let m_tot = (q_rect + supported_floor + own) * l_ef.powi(2) / 8.0;
                        (q_rect, m_tot)
                    };
                    let load = wall_load + supported_floor + own;
                    let (bars, flexure_ratio) = lintel_bars(moment, depth);
                    let result = MasonryBandResult {
                        opening_id: opening.id,
                        wall_id: wall.id,
                        label: format!("VA{lintel_no}"),
                        kind: MasonryBandKind::Lintel,
                        opening_kind: opening.kind,
                        start,
                        end,
                        clear_span_m: opening.width(),
                        bearing_left_m: bearing_left,
                        bearing_right_m: bearing_right,
                        width_m: thickness,
                        height_m: depth,
                        load_kn_m: load,
                        moment_knm: moment,
                        bars,
                        flexure_ratio,
                    };
                    if !result.ok() {
                        report.warnings.push(format!(
                            "{}: verga do vão {} não atende ao método simplificado (apoios {:.0}/{:.0} cm, vão {:.2} m).",
                            result.label,
                            opening.id,
                            bearing_left * 100.0,
                            bearing_right * 100.0,
                            opening.width()
                        ));
                    }
                    report.bands.push(result);
                }

                if opening.kind == OpeningKind::Window && opening.sill() > 0.05 {
                    sill_no += 1;
                    let depth = next_course(0.10, masonry);
                    let result = MasonryBandResult {
                        opening_id: opening.id,
                        wall_id: wall.id,
                        label: format!("CV{sill_no}"),
                        kind: MasonryBandKind::Sill,
                        opening_kind: opening.kind,
                        start,
                        end,
                        clear_span_m: opening.width(),
                        bearing_left_m: bearing_left,
                        bearing_right_m: bearing_right,
                        width_m: thickness,
                        height_m: depth,
                        load_kn_m: 0.0,
                        moment_knm: 0.0,
                        bars: "2Ø6,3".to_owned(),
                        flexure_ratio: 0.0,
                    };
                    if !result.ok() {
                        let reason = if opening.width() > MAX_LINTEL_SPAN_M + 1e-9 {
                            format!(
                                "vão de {:.2} m excede o limite de {:.1} m do método simplificado",
                                opening.width(),
                                MAX_LINTEL_SPAN_M
                            )
                        } else {
                            "não consegue os 20 cm mínimos de apoio".to_owned()
                        };
                        report.warnings.push(format!(
                            "{}: contraverga do vão {} {reason}.",
                            result.label, opening.id
                        ));
                    }
                    report.bands.push(result);
                }
            }
        }
        report
    }
}

/// Plan marks shared by screen, SVG/PDF and DXF structural views.
pub fn masonry_structure_marks(report: &MasonryStructureReport) -> Vec<PlanMark> {
    let mut marks = Vec::new();
    for band in &report.bands {
        let Some(dir) = band.end.minus(band.start).normalized() else {
            continue;
        };
        let normal = Point::new(-dir.y, dir.x);
        let side = if band.kind == MasonryBandKind::Lintel {
            -1.0
        } else {
            1.0
        };
        let offset = normal.scaled(side * 0.07);
        let a = band.start.translated(offset);
        let b = band.end.translated(offset);
        marks.push(PlanMark::Line {
            points: vec![a, b],
            dashed: band.kind == MasonryBandKind::Sill,
            heavy: false,
        });
        marks.push(PlanMark::Text {
            anchor: a.midpoint(b).translated(normal.scaled(side * 0.14)),
            text: format!(
                "{} {}×{:.0} {}",
                band.label,
                (band.width_m * 100.0).round(),
                band.height_m * 100.0,
                band.bars
            ),
        });
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;
    use crate::domain::model::OpeningKind;

    #[test]
    fn every_built_opening_gets_a_lintel_and_windows_get_a_sill() {
        let mut project = Project::default();
        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(6.0, 0.0))
            .unwrap();
        let door = project
            .add_opening_sized(wall, OpeningKind::Door, 0.25, Some(0.90))
            .unwrap();
        project
            .add_opening_sized(wall, OpeningKind::Window, 0.65, Some(1.20))
            .unwrap();

        let report = project.masonry_structure_report();
        assert_eq!(
            report
                .bands
                .iter()
                .filter(|b| b.kind == MasonryBandKind::Lintel)
                .count(),
            2
        );
        assert_eq!(
            report
                .bands
                .iter()
                .filter(|b| b.kind == MasonryBandKind::Sill)
                .count(),
            1
        );
        assert!(report.bands.iter().all(MasonryBandResult::ok));
        assert_eq!(masonry_structure_marks(&report).len(), 6);

        // Hand calculation for the 0.90 m door in the default 9 cm ceramic
        // masonry: ha = min(0.70, 0.45) m, h = one 15 cm course.
        let lintel = report
            .bands
            .iter()
            .find(|band| band.opening_id == door && band.kind == MasonryBandKind::Lintel)
            .unwrap();
        let q_wall = 0.5 * 0.45 * 0.09 * 13.0;
        let q_self = 25.0 * 0.09 * 0.15;
        let expected_moment = (q_wall + q_self) * 0.90_f64.powi(2) / 8.0;
        assert!((lintel.height_m - 0.15).abs() < 1e-9);
        assert!((lintel.moment_knm - expected_moment).abs() < 1e-9);
    }

    #[test]
    fn slender_infill_and_loaded_structural_masonry_are_checked_separately() {
        let mut project = Project::default();
        let infill = project
            .add_wall(Point::new(0.0, 0.0), Point::new(3.0, 0.0))
            .unwrap();
        let structural = project
            .add_wall(Point::new(0.0, 2.0), Point::new(3.0, 2.0))
            .unwrap();
        project.set_wall_height(infill, Some(4.0), None).unwrap();
        let material = Masonry::presets()
            .into_iter()
            .find(|m| m.name.contains("estrutural"))
            .unwrap();
        project
            .walls
            .iter_mut()
            .find(|wall| wall.id == structural)
            .unwrap()
            .masonry = Some(material);

        let report = project.masonry_structure_report();
        let infill = report.panels.iter().find(|p| p.wall_id == infill).unwrap();
        let structural = report
            .panels
            .iter()
            .find(|p| p.wall_id == structural)
            .unwrap();
        assert_eq!(infill.kind, MasonryWallKind::Infill);
        assert!(infill.slenderness > infill.slenderness_limit);
        assert!(!infill.ok());
        assert_eq!(structural.kind, MasonryWallKind::LoadBearing);
        assert!(structural.compression_mpa > 0.0);
        assert!(structural.compression_capacity_mpa > structural.compression_mpa);
    }

    #[test]
    fn an_opening_too_close_to_the_wall_end_fails_the_bearing_check() {
        let mut project = Project::default();
        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        let opening = project
            .add_opening_sized(wall, OpeningKind::Window, 0.12, Some(0.80))
            .unwrap();
        let report = project.masonry_structure_report();
        let lintel = report
            .bands
            .iter()
            .find(|band| band.opening_id == opening && band.kind == MasonryBandKind::Lintel)
            .unwrap();
        assert!(lintel.bearing_left_m < MIN_LINTEL_BEARING_M);
        assert!(!lintel.ok());
    }
}
