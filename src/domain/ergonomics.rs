//! Architectural circulation, interference and furniture functional clearance
//! verification (ABNT NBR 9050 & residential ergonomics).

use super::{
    fixtures::Fixture,
    geometry::{Point, distance_to_segment},
    model::{OpeningKind, Project},
    openings::DoorType,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueSeverity {
    Error,
    Warning,
}

impl IssueSeverity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "Erro",
            Self::Warning => "Alerta",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErgonomicsIssueKind {
    FixtureCollision,
    WallCollision,
    DoorSwingObstruction,
    ClearanceBlocked,
    NarrowPassage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ErgonomicsIssue {
    pub severity: IssueSeverity,
    pub kind: ErgonomicsIssueKind,
    pub location: Point,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClearanceZone {
    pub fixture_id: u64,
    pub label: String,
    pub min: Point,
    pub max: Point,
    pub depth_m: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ErgonomicsReport {
    pub min_passage_m: f64,
    pub clearance_zones: Vec<ClearanceZone>,
    pub issues: Vec<ErgonomicsIssue>,
}

impl ErgonomicsReport {
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Warning)
            .count()
    }
}

fn rect_overlap_area(a_min: Point, a_max: Point, b_min: Point, b_max: Point) -> f64 {
    let ox = (a_max.x.min(b_max.x) - a_min.x.max(b_min.x)).max(0.0);
    let oy = (a_max.y.min(b_max.y) - a_min.y.max(b_min.y)).max(0.0);
    ox * oy
}

fn rect_overlap_center(a_min: Point, a_max: Point, b_min: Point, b_max: Point) -> Point {
    Point::new(
        (a_min.x.max(b_min.x) + a_max.x.min(b_max.x)) * 0.5,
        (a_min.y.max(b_min.y) + a_max.y.min(b_max.y)) * 0.5,
    )
}

/// Minimum Euclidean gap between two axis-aligned rectangles (0 if touching/overlapping).
fn rect_gap(a_min: Point, a_max: Point, b_min: Point, b_max: Point) -> f64 {
    let dx = (b_min.x - a_max.x).max(a_min.x - b_max.x).max(0.0);
    let dy = (b_min.y - a_max.y).max(a_min.y - b_max.y).max(0.0);
    dx.hypot(dy)
}

/// Computes the axis-aligned functional clearance rectangle directly in front
/// of `fixture` (in local coordinates: `y` from `+depth/2` to `+depth/2 + clearance`).
pub fn fixture_clearance_zone(fixture: Fixture) -> ClearanceZone {
    let clearance = fixture.kind.front_clearance_m();
    let hw = fixture.width * 0.5;
    let hd = fixture.depth * 0.5;
    let corners = [
        fixture.to_world(Point::new(-hw, hd)),
        fixture.to_world(Point::new(hw, hd)),
        fixture.to_world(Point::new(hw, hd + clearance)),
        fixture.to_world(Point::new(-hw, hd + clearance)),
    ];
    let min = Point::new(
        corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
        corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
    );
    let max = Point::new(
        corners
            .iter()
            .map(|p| p.x)
            .fold(f64::NEG_INFINITY, f64::max),
        corners
            .iter()
            .map(|p| p.y)
            .fold(f64::NEG_INFINITY, f64::max),
    );
    ClearanceZone {
        fixture_id: fixture.id,
        label: format!("Área de uso ({:.0} cm)", clearance * 100.0),
        min,
        max,
        depth_m: clearance,
    }
}

/// Checks if a segment `s1 -> s2` intrudes into the interior of AABB `(min, max)`
/// expanded by `pad` (used to detect walls cutting through a fixture or its usage zone).
fn segment_intrudes_rect(
    s1: Point,
    s2: Point,
    min: Point,
    max: Point,
    margin: f64,
) -> Option<Point> {
    let inner_min = Point::new(min.x + margin, min.y + margin);
    let inner_max = Point::new(max.x - margin, max.y - margin);
    if inner_min.x >= inner_max.x || inner_min.y >= inner_max.y {
        return None;
    }
    let samples = 12;
    for i in 0..=samples {
        let t = i as f64 / samples as f64;
        let p = s1.lerp(s2, t);
        if p.x > inner_min.x && p.x < inner_max.x && p.y > inner_min.y && p.y < inner_max.y {
            return Some(p);
        }
    }
    None
}

/// Runs circulation, collision, door-swing and furniture-clearance checks on the active floor.
pub fn analyze_ergonomics(project: &Project, min_passage_m: f64) -> ErgonomicsReport {
    let min_passage = min_passage_m.clamp(0.45, 1.50);
    let mut clearance_zones = Vec::with_capacity(project.fixtures.len());
    let mut issues = Vec::new();

    for &fixture in &project.fixtures {
        clearance_zones.push(fixture_clearance_zone(fixture));
    }

    // 1. Fixture-to-fixture physical collisions & narrow passages.
    for i in 0..project.fixtures.len() {
        let f1 = project.fixtures[i];
        let (a_min, a_max) = f1.bounds();
        for f2 in project.fixtures.iter().skip(i + 1) {
            let (b_min, b_max) = f2.bounds();
            let overlap = rect_overlap_area(a_min, a_max, b_min, b_max);
            if overlap > 0.004 {
                issues.push(ErgonomicsIssue {
                    severity: IssueSeverity::Error,
                    kind: ErgonomicsIssueKind::FixtureCollision,
                    location: rect_overlap_center(a_min, a_max, b_min, b_max),
                    message: format!(
                        "Interferência física entre {} e {}.",
                        f1.kind.label(),
                        f2.kind.label()
                    ),
                });
                continue;
            }
            // Check narrow circulation gap between nearby non-adjacent furniture (> 5 cm, < min_passage).
            let gap = rect_gap(a_min, a_max, b_min, b_max);
            let x_faces = a_min.x.max(b_min.x) < a_max.x.min(b_max.x) - 0.10;
            let y_faces = a_min.y.max(b_min.y) < a_max.y.min(b_max.y) - 0.10;
            if (x_faces || y_faces) && gap > 0.08 && gap < min_passage - 0.01 {
                issues.push(ErgonomicsIssue {
                    severity: IssueSeverity::Warning,
                    kind: ErgonomicsIssueKind::NarrowPassage,
                    location: f1.center.midpoint(f2.center),
                    message: format!(
                        "Passagem estreita ({:.0} cm < {:.0} cm) entre {} e {}.",
                        gap * 100.0,
                        min_passage * 100.0,
                        f1.kind.label(),
                        f2.kind.label()
                    ),
                });
            }
        }
    }

    // 2. Fixture-to-wall intrusion (allowing flush placement against wall faces).
    for &fixture in &project.fixtures {
        let (f_min, f_max) = fixture.bounds();
        for wall in &project.walls {
            let half_t = wall.thickness() * 0.5;
            // Margin accounts for wall thickness + 3 cm snapping tolerance.
            if let Some(hit) = segment_intrudes_rect(wall.start, wall.end, f_min, f_max, 0.04) {
                if distance_to_segment(fixture.center, wall.start, wall.end)
                    < fixture.width.min(fixture.depth) * 0.5 + half_t - 0.04
                {
                    issues.push(ErgonomicsIssue {
                        severity: IssueSeverity::Error,
                        kind: ErgonomicsIssueKind::WallCollision,
                        location: hit,
                        message: format!("{} invade uma parede.", fixture.kind.label()),
                    });
                    break;
                }
            }
        }
    }

    // 3. Furniture functional clearance zone blocked by another fixture or opposing wall.
    for zone in &clearance_zones {
        let owner = project
            .fixtures
            .iter()
            .find(|f| f.id == zone.fixture_id)
            .unwrap();
        for other in &project.fixtures {
            if other.id == zone.fixture_id {
                continue;
            }
            let (o_min, o_max) = other.bounds();
            let overlap = rect_overlap_area(zone.min, zone.max, o_min, o_max);
            if overlap > 0.02 {
                issues.push(ErgonomicsIssue {
                    severity: IssueSeverity::Warning,
                    kind: ErgonomicsIssueKind::ClearanceBlocked,
                    location: rect_overlap_center(zone.min, zone.max, o_min, o_max),
                    message: format!(
                        "Área de uso de {} ({:.0} cm) obstruída por {}.",
                        owner.kind.label(),
                        zone.depth_m * 100.0,
                        other.kind.label()
                    ),
                });
            }
        }
        for wall in &project.walls {
            if let Some(hit) = segment_intrudes_rect(wall.start, wall.end, zone.min, zone.max, 0.08)
            {
                issues.push(ErgonomicsIssue {
                    severity: IssueSeverity::Warning,
                    kind: ErgonomicsIssueKind::ClearanceBlocked,
                    location: hit,
                    message: format!(
                        "Área de uso frontal de {} ({:.0} cm) bloqueada por parede.",
                        owner.kind.label(),
                        zone.depth_m * 100.0
                    ),
                });
                break;
            }
        }
    }

    // 4. Door swing obstruction by furniture.
    for opening in &project.openings {
        if opening.kind != OpeningKind::Door {
            continue;
        }
        let door_type = opening.effective_door_type();
        if matches!(door_type, DoorType::Sliding | DoorType::PocketSliding) {
            continue;
        }
        let Some(wall) = project.walls.iter().find(|w| w.id == opening.wall_id) else {
            continue;
        };
        let Some(dir) = wall.end.minus(wall.start).normalized() else {
            continue;
        };
        let normal = Point::new(-dir.y, dir.x);
        let side = if opening.flip_side {
            Point::new(-normal.x, -normal.y)
        } else {
            normal
        };
        let center = wall.start.lerp(wall.end, opening.t);
        let half_w = opening.width() * 0.5;
        let reach = match door_type {
            DoorType::DoubleSwing => half_w,
            DoorType::Pivot => opening.width() * 0.80,
            DoorType::Bifold => opening.width() * 0.35,
            _ => opening.width(),
        };
        let p0 = center.translated(dir.scaled(-half_w));
        let p1 = center.translated(dir.scaled(half_w));
        let p2 = p1.translated(side.scaled(reach));
        let p3 = p0.translated(side.scaled(reach));
        let sweep_min = Point::new(
            p0.x.min(p1.x).min(p2.x).min(p3.x),
            p0.y.min(p1.y).min(p2.y).min(p3.y),
        );
        let sweep_max = Point::new(
            p0.x.max(p1.x).max(p2.x).max(p3.x),
            p0.y.max(p1.y).max(p2.y).max(p3.y),
        );
        for fixture in &project.fixtures {
            let (f_min, f_max) = fixture.bounds();
            if rect_overlap_area(sweep_min, sweep_max, f_min, f_max) > 0.02 {
                issues.push(ErgonomicsIssue {
                    severity: IssueSeverity::Error,
                    kind: ErgonomicsIssueKind::DoorSwingObstruction,
                    location: rect_overlap_center(sweep_min, sweep_max, f_min, f_max),
                    message: format!(
                        "{} bloqueia o raio de abertura da porta ({}).",
                        fixture.kind.label(),
                        door_type.short_label()
                    ),
                });
            }
        }
    }

    ErgonomicsReport {
        min_passage_m: min_passage,
        clearance_zones,
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;

    #[test]
    fn clean_room_with_well_spaced_furniture_has_no_issues() {
        let mut project = Project::default();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Quarto")
            .unwrap();
        // Bed along top wall, desk far away along bottom wall (> 1.3 m gap).
        project
            .add_fixture(FixtureKind::BedSingle, Point::new(1.0, 1.1), 0.90, 1.95, 0)
            .unwrap();
        project
            .add_fixture(FixtureKind::Desk, Point::new(4.0, 3.5), 1.40, 0.65, 2)
            .unwrap();

        let report = analyze_ergonomics(&project, 0.70);
        assert_eq!(report.clearance_zones.len(), 2);
        assert!(
            report.issues.is_empty(),
            "expected no issues, got: {:?}",
            report.issues
        );
    }

    #[test]
    fn detects_collision_narrow_passage_clearance_block_and_door_swing_obstruction() {
        let mut project = Project::default();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala")
            .unwrap();
        let top_wall = project
            .walls
            .iter()
            .find(|w| w.start.y.abs() < 1e-4 && w.end.y.abs() < 1e-4)
            .unwrap()
            .id;
        // Door on top wall swinging downwards into room (normal = +Y).
        project
            .add_opening(top_wall, OpeningKind::Door, 0.5)
            .unwrap();
        // Place a sofa right inside the door swing!
        project
            .add_fixture(FixtureKind::Sofa, Point::new(2.5, 0.55), 1.80, 0.80, 0)
            .unwrap();
        // Place a second overlapping armchair right on top of the sofa.
        project
            .add_fixture(FixtureKind::Armchair, Point::new(2.6, 0.60), 0.85, 0.85, 0)
            .unwrap();
        // Place a table facing the sofa with a narrow 35 cm gap (< 70 cm).
        project
            .add_fixture(FixtureKind::Table, Point::new(2.5, 1.80), 1.40, 0.90, 2)
            .unwrap();

        let report = analyze_ergonomics(&project, 0.70);
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.kind == ErgonomicsIssueKind::DoorSwingObstruction)
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.kind == ErgonomicsIssueKind::FixtureCollision)
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.kind == ErgonomicsIssueKind::NarrowPassage)
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.kind == ErgonomicsIssueKind::ClearanceBlocked)
        );
        assert!(report.error_count() >= 2);
        assert!(report.warning_count() >= 2);
    }
}
