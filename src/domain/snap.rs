//! CAD object snaps (OSNAP): endpoint, midpoint, intersection, centre,
//! perpendicular, tangent, extension and grid.
//!
//! Pure geometry over the project and active draft points, with no UI
//! dependencies.

use serde::{Deserialize, Serialize};

use super::{
    drawing::{Shape, angle_of, angle_within, point_at_angle},
    geometry::{EPSILON, GRID_METERS, POINT_TOLERANCE, Point, line_intersection},
    model::Project,
};

/// Which object snap modes are active. All are enabled by default so
/// beginners benefit automatically while professionals can fine-tune them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OsnapSettings {
    pub endpoint: bool,
    pub midpoint: bool,
    pub intersection: bool,
    pub center: bool,
    pub perpendicular: bool,
    pub tangent: bool,
    pub extension: bool,
}

impl Default for OsnapSettings {
    fn default() -> Self {
        Self {
            endpoint: true,
            midpoint: true,
            intersection: true,
            center: true,
            perpendicular: true,
            tangent: true,
            extension: true,
        }
    }
}

impl OsnapSettings {
    pub fn any_enabled(self) -> bool {
        self.endpoint
            || self.midpoint
            || self.intersection
            || self.center
            || self.perpendicular
            || self.tangent
            || self.extension
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    /// End of a wall, corner of a room/terrain, or vertex of a CAD shape.
    Endpoint,
    /// Midpoint of a wall, terrain side, CAD segment or arc.
    Midpoint,
    /// Crossing between two segments, or between a segment/circle and a circle/arc.
    Intersection,
    /// Centre of a circle, arc, rectangle or room.
    Center,
    /// Foot of the perpendicular from the active draft start onto a segment or circle.
    Perpendicular,
    /// Point of tangency from the active draft start onto a circle or arc.
    Tangent,
    /// Alignment along the extension of an existing segment or the draft start axes.
    Extension,
    /// A point on the face of a wall (used in measure mode).
    Face,
    /// A grid intersection.
    Grid,
}

impl SnapKind {
    pub fn label(self) -> &'static str {
        match self {
            SnapKind::Endpoint => "Ponta",
            SnapKind::Midpoint => "Meio",
            SnapKind::Intersection => "Interseção",
            SnapKind::Center => "Centro",
            SnapKind::Perpendicular => "Perpendicular",
            SnapKind::Tangent => "Tangente",
            SnapKind::Extension => "Prolongamento",
            SnapKind::Face => "Face",
            SnapKind::Grid => "Grade",
        }
    }

    fn priority_bias(self, threshold: f64) -> f64 {
        match self {
            SnapKind::Endpoint => 0.0,
            SnapKind::Intersection => 0.05 * threshold,
            SnapKind::Midpoint | SnapKind::Center => 0.10 * threshold,
            SnapKind::Perpendicular | SnapKind::Tangent => 0.15 * threshold,
            SnapKind::Extension => 0.35 * threshold,
            SnapKind::Face => 0.20 * threshold,
            SnapKind::Grid => threshold,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapResult {
    pub point: Point,
    pub kind: SnapKind,
    /// Optional reference segment rendered as a thin dashed tracking line.
    pub guide: Option<(Point, Point)>,
}

struct CircleRef {
    center: Point,
    radius: f64,
    /// `(start_angle, sweep)` when this is an arc; `None` for a full circle.
    arc: Option<(f64, f64)>,
}

impl CircleRef {
    fn contains_angle(&self, angle: f64) -> bool {
        match self.arc {
            Some((start, sweep)) => angle_within(angle, start, sweep),
            None => true,
        }
    }
}

/// Finds the best object snap for `cursor` within `threshold` metres, falling
/// back to the 0.25 m grid when no object snap is in range.
pub fn find_snap(
    project: &Project,
    draft_points: &[Point],
    cursor: Point,
    start: Option<Point>,
    threshold: f64,
    settings: OsnapSettings,
) -> SnapResult {
    let mut segments: Vec<(Point, Point)> = Vec::new();
    let mut endpoints: Vec<Point> = Vec::new();
    let mut midpoints: Vec<Point> = Vec::new();
    let mut centers: Vec<Point> = Vec::new();
    let mut circles: Vec<CircleRef> = Vec::new();

    collect_geometry(
        project,
        draft_points,
        &mut segments,
        &mut endpoints,
        &mut midpoints,
        &mut centers,
        &mut circles,
    );

    let mut best: Option<(f64, SnapResult)> = None;
    let mut consider = |point: Point, kind: SnapKind, guide: Option<(Point, Point)>| {
        if !point.is_finite() {
            return;
        }
        let distance = cursor.distance(point);
        if distance >= threshold {
            return;
        }
        let score = distance + kind.priority_bias(threshold);
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score < *best_score)
        {
            best = Some((score, SnapResult { point, kind, guide }));
        }
    };

    if settings.endpoint {
        for &pt in &endpoints {
            consider(pt, SnapKind::Endpoint, None);
        }
    }

    if settings.midpoint {
        for &pt in &midpoints {
            consider(pt, SnapKind::Midpoint, None);
        }
    }

    if settings.center {
        for &pt in &centers {
            consider(pt, SnapKind::Center, None);
        }
    }

    if settings.intersection {
        collect_intersections(&segments, &circles, |pt| {
            consider(pt, SnapKind::Intersection, None);
        });
    }

    if settings.perpendicular
        && let Some(from) = start
    {
        for &(a, b) in &segments {
            if let Some(foot) = perpendicular_foot_on_segment(from, a, b)
                && foot.distance(from) > POINT_TOLERANCE
            {
                consider(foot, SnapKind::Perpendicular, Some((from, foot)));
            }
        }
        for circle in &circles {
            if let Some(dir) = from.minus(circle.center).normalized() {
                for sign in [1.0, -1.0] {
                    let pt = circle.center.translated(dir.scaled(sign * circle.radius));
                    if circle.contains_angle(angle_of(circle.center, pt))
                        && pt.distance(from) > POINT_TOLERANCE
                    {
                        consider(pt, SnapKind::Perpendicular, Some((from, pt)));
                    }
                }
            }
        }
    }

    if settings.tangent
        && let Some(from) = start
    {
        for circle in &circles {
            for pt in tangent_points(from, circle) {
                consider(pt, SnapKind::Tangent, Some((from, pt)));
            }
        }
    }

    if settings.extension {
        let ext_threshold = threshold * 0.85;
        for &(a, b) in &segments {
            if let Some((anchor, projected)) = segment_extension_point(cursor, a, b)
                && cursor.distance(projected) < ext_threshold
            {
                consider(projected, SnapKind::Extension, Some((anchor, projected)));
            }
        }
    }

    if let Some((_, result)) = best {
        return result;
    }

    // Fallback: grid snap, with optional axis alignment (extension) to `start`.
    let mut grid_point = cursor.snapped_to(GRID_METERS);
    if settings.extension
        && let Some(from) = start
    {
        let mut aligned = false;
        if (cursor.x - from.x).abs() < threshold {
            grid_point.x = from.x;
            aligned = true;
        }
        if (cursor.y - from.y).abs() < threshold {
            grid_point.y = from.y;
            aligned = true;
        }
        if aligned && grid_point.distance(from) > POINT_TOLERANCE {
            return SnapResult {
                point: grid_point,
                kind: SnapKind::Extension,
                guide: Some((from, grid_point)),
            };
        }
    }

    SnapResult {
        point: grid_point,
        kind: SnapKind::Grid,
        guide: None,
    }
}

fn collect_geometry(
    project: &Project,
    draft_points: &[Point],
    segments: &mut Vec<(Point, Point)>,
    endpoints: &mut Vec<Point>,
    midpoints: &mut Vec<Point>,
    centers: &mut Vec<Point>,
    circles: &mut Vec<CircleRef>,
) {
    for wall in &project.walls {
        segments.push((wall.start, wall.end));
        endpoints.push(wall.start);
        endpoints.push(wall.end);
        midpoints.push(wall.start.midpoint(wall.end));
    }

    if let Some(terrain) = &project.terrain {
        let corners = [
            terrain.min,
            Point::new(terrain.max.x, terrain.min.y),
            terrain.max,
            Point::new(terrain.min.x, terrain.max.y),
        ];
        endpoints.extend_from_slice(&corners);
        for i in 0..4 {
            let a = corners[i];
            let b = corners[(i + 1) % 4];
            segments.push((a, b));
            midpoints.push(a.midpoint(b));
        }
    }

    for room in &project.rooms {
        if let Some(poly) = project.room_polygon(room)
            && !poly.is_empty()
        {
            let (mut sx, mut sy) = (0.0, 0.0);
            for pt in &poly {
                sx += pt.x;
                sy += pt.y;
            }
            centers.push(Point::new(sx / poly.len() as f64, sy / poly.len() as f64));
        }
    }

    for drawing in &project.drawings {
        match &drawing.shape {
            Shape::Line { start, end } => {
                segments.push((*start, *end));
                endpoints.push(*start);
                endpoints.push(*end);
                midpoints.push(start.midpoint(*end));
            }
            Shape::Polyline { points, closed } => {
                endpoints.extend(points.iter().copied());
                for pair in points.windows(2) {
                    segments.push((pair[0], pair[1]));
                    midpoints.push(pair[0].midpoint(pair[1]));
                }
                if *closed && points.len() >= 2 {
                    let a = *points.last().unwrap();
                    let b = points[0];
                    segments.push((a, b));
                    midpoints.push(a.midpoint(b));
                }
                if drawing.shape.is_rectangle() && points.len() == 4 {
                    centers.push(points[0].midpoint(points[2]));
                }
            }
            Shape::Circle { center, radius } => {
                centers.push(*center);
                circles.push(CircleRef {
                    center: *center,
                    radius: *radius,
                    arc: None,
                });
            }
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                centers.push(*center);
                endpoints.push(point_at_angle(*center, *radius, *start));
                endpoints.push(point_at_angle(*center, *radius, *start + *sweep));
                midpoints.push(point_at_angle(*center, *radius, *start + *sweep * 0.5));
                circles.push(CircleRef {
                    center: *center,
                    radius: *radius,
                    arc: Some((*start, *sweep)),
                });
            }
            Shape::Spline { points, .. } => {
                endpoints.extend(points.iter().copied());
            }
        }
    }

    endpoints.extend(draft_points.iter().copied());
    for pair in draft_points.windows(2) {
        segments.push((pair[0], pair[1]));
        midpoints.push(pair[0].midpoint(pair[1]));
    }
}

fn collect_intersections(
    segments: &[(Point, Point)],
    circles: &[CircleRef],
    mut emit: impl FnMut(Point),
) {
    const PARAM_TOL: f64 = 1e-6;
    for i in 0..segments.len() {
        let (a, b) = segments[i];
        for &(c, d) in &segments[(i + 1)..] {
            if let Some((pt, t, u)) = line_intersection(a, b, c, d)
                && (-PARAM_TOL..=1.0 + PARAM_TOL).contains(&t)
                && (-PARAM_TOL..=1.0 + PARAM_TOL).contains(&u)
            {
                emit(pt);
            }
        }
        for circle in circles {
            for pt in segment_circle_intersections(a, b, circle) {
                emit(pt);
            }
        }
    }
    for i in 0..circles.len() {
        for other in &circles[(i + 1)..] {
            for pt in circle_circle_intersections(&circles[i], other) {
                emit(pt);
            }
        }
    }
}

fn segment_circle_intersections(a: Point, b: Point, circle: &CircleRef) -> Vec<Point> {
    let d = b.minus(a);
    let f = a.minus(circle.center);
    let qa = d.dot(d);
    if qa <= EPSILON * EPSILON {
        return Vec::new();
    }
    let qb = 2.0 * f.dot(d);
    let qc = f.dot(f) - circle.radius * circle.radius;
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < -EPSILON {
        return Vec::new();
    }
    let root = disc.max(0.0).sqrt();
    let mut out = Vec::new();
    for sign in [-1.0, 1.0] {
        let t = (-qb + sign * root) / (2.0 * qa);
        if (-1e-6..=1.0 + 1e-6).contains(&t) {
            let pt = a.lerp(b, t.clamp(0.0, 1.0));
            if circle.contains_angle(angle_of(circle.center, pt)) {
                out.push(pt);
            }
        }
        if root <= EPSILON {
            break;
        }
    }
    out
}

fn circle_circle_intersections(c1: &CircleRef, c2: &CircleRef) -> Vec<Point> {
    let d = c1.center.distance(c2.center);
    if d <= EPSILON
        || d > c1.radius + c2.radius + EPSILON
        || d < (c1.radius - c2.radius).abs() - EPSILON
    {
        return Vec::new();
    }
    let a = (c1.radius * c1.radius - c2.radius * c2.radius + d * d) / (2.0 * d);
    let h = (c1.radius * c1.radius - a * a).max(0.0).sqrt();
    let dir = c2.center.minus(c1.center).scaled(1.0 / d);
    let mid = c1.center.translated(dir.scaled(a));
    let perp = Point::new(-dir.y, dir.x);
    let mut out = Vec::new();
    for sign in [-1.0, 1.0] {
        let pt = mid.translated(perp.scaled(sign * h));
        if c1.contains_angle(angle_of(c1.center, pt)) && c2.contains_angle(angle_of(c2.center, pt))
        {
            out.push(pt);
        }
        if h <= EPSILON {
            break;
        }
    }
    out
}

fn perpendicular_foot_on_segment(from: Point, a: Point, b: Point) -> Option<Point> {
    let d = b.minus(a);
    let len_sq = d.dot(d);
    if len_sq <= EPSILON * EPSILON {
        return None;
    }
    let t = from.minus(a).dot(d) / len_sq;
    (0.0..=1.0).contains(&t).then(|| a.lerp(b, t))
}

fn tangent_points(from: Point, circle: &CircleRef) -> Vec<Point> {
    let d = from.distance(circle.center);
    if d <= circle.radius + POINT_TOLERANCE {
        return Vec::new();
    }
    let base = angle_of(circle.center, from);
    let alpha = (circle.radius / d).clamp(-1.0, 1.0).acos();
    let mut out = Vec::new();
    for angle in [base + alpha, base - alpha] {
        if circle.contains_angle(angle) {
            out.push(point_at_angle(circle.center, circle.radius, angle));
        }
    }
    out
}

/// Projects `cursor` onto the extension of segment `a`-`b` beyond its endpoints.
/// Returns `(nearest_endpoint, projected_point)`.
fn segment_extension_point(cursor: Point, a: Point, b: Point) -> Option<(Point, Point)> {
    let d = b.minus(a);
    let len_sq = d.dot(d);
    if len_sq <= EPSILON * EPSILON {
        return None;
    }
    let t = cursor.minus(a).dot(d) / len_sq;
    if (-1e-4..=1.0 + 1e-4).contains(&t) {
        return None;
    }
    let anchor = if t < 0.0 { a } else { b };
    Some((anchor, a.lerp(b, t)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::drawing::Shape;

    #[test]
    fn snaps_to_midpoint_intersection_center_perpendicular_tangent_and_extension() {
        let mut project = Project::default();
        // Two crossing walls: (0,0)-(4,0) and (2,-2)-(2,2).
        project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        project
            .add_wall(Point::new(2.0, -2.0), Point::new(2.0, 2.0))
            .unwrap();
        // A circle centred at (10, 0) with radius 2.
        project
            .add_drawing(Shape::Circle {
                center: Point::new(10.0, 0.0),
                radius: 2.0,
            })
            .unwrap();
        let all = OsnapSettings::default();

        // Intersection at (2, 0) (also midpoint of both walls; intersection has higher priority).
        let hit = find_snap(&project, &[], Point::new(2.02, 0.01), None, 0.2, all);
        assert_eq!(hit.kind, SnapKind::Intersection);
        assert!(hit.point.near(Point::new(2.0, 0.0), 1e-4));

        // Midpoint of a non-intersecting wall (0, 6)-(4, 6) -> (2, 6).
        project
            .add_wall(Point::new(0.0, 6.0), Point::new(4.0, 6.0))
            .unwrap();
        let mid = find_snap(&project, &[], Point::new(2.03, 5.98), None, 0.2, all);
        assert_eq!(mid.kind, SnapKind::Midpoint);
        assert!(mid.point.near(Point::new(2.0, 6.0), 1e-4));

        // Circle centre at (10, 0).
        let center = find_snap(&project, &[], Point::new(10.04, 0.02), None, 0.2, all);
        assert_eq!(center.kind, SnapKind::Center);
        assert!(center.point.near(Point::new(10.0, 0.0), 1e-4));

        // Perpendicular from (1, 4) onto wall (0, 6)-(4, 6) -> (1, 6).
        let perp = find_snap(
            &project,
            &[],
            Point::new(1.02, 6.03),
            Some(Point::new(1.0, 4.0)),
            0.2,
            all,
        );
        assert_eq!(perp.kind, SnapKind::Perpendicular);
        assert!(perp.point.near(Point::new(1.0, 6.0), 1e-4));
        assert!(perp.guide.is_some());

        // Tangent from (6, 0) to circle (10, 0), r = 2:
        // cos(alpha) = 2/4 = 0.5 -> alpha = 60°, x = 10 - 1 = 9, y = ±sqrt(3).
        let tan = find_snap(
            &project,
            &[],
            Point::new(9.02, 1.72),
            Some(Point::new(6.0, 0.0)),
            0.2,
            all,
        );
        assert_eq!(tan.kind, SnapKind::Tangent);
        assert!(tan.point.near(Point::new(9.0, 3.0_f64.sqrt()), 1e-3));

        // Extension beyond wall (0, 6)-(4, 6) at (5.3, 6.04) -> (5.3, 6.0).
        let ext = find_snap(&project, &[], Point::new(5.3, 6.04), None, 0.2, all);
        assert_eq!(ext.kind, SnapKind::Extension);
        assert!((ext.point.y - 6.0).abs() < 1e-4);
        assert!(ext.guide.is_some());
    }

    #[test]
    fn snaps_circle_and_arc_intersections_and_respects_arc_span() {
        let mut project = Project::default();
        // Horizontal line y = 0 from x = -5 to x = 5.
        project
            .add_drawing(Shape::Line {
                start: Point::new(-5.0, 0.0),
                end: Point::new(5.0, 0.0),
            })
            .unwrap();
        // Upper-half arc around (0, 0) with radius 2: angles 0..=PI (in plan coords: +y side).
        project
            .add_drawing(Shape::Arc {
                center: Point::new(0.0, 0.0),
                radius: 2.0,
                start: 0.0,
                sweep: std::f64::consts::PI,
            })
            .unwrap();
        // Full circle centred at (3, 0) with radius 2 (intersects arc at x = 1.5, y = +sqrt(1.75)).
        project
            .add_drawing(Shape::Circle {
                center: Point::new(3.0, 0.0),
                radius: 2.0,
            })
            .unwrap();

        let all = OsnapSettings::default();
        // Arc midpoint at angle PI/2 -> (0, 2).
        let arc_mid = find_snap(&project, &[], Point::new(0.02, 2.01), None, 0.2, all);
        assert_eq!(arc_mid.kind, SnapKind::Midpoint);
        assert!(arc_mid.point.near(Point::new(0.0, 2.0), 1e-4));

        // Circle-arc intersection on the arc's valid half (y > 0).
        let y = (4.0_f64 - 2.25).sqrt();
        let hit_valid = find_snap(&project, &[], Point::new(1.51, y + 0.01), None, 0.2, all);
        assert_eq!(hit_valid.kind, SnapKind::Intersection);
        assert!(hit_valid.point.near(Point::new(1.5, y), 1e-3));

        // Symmetric point (1.5, -y) is NOT on the arc, so it must not snap as Intersection.
        let hit_outside = find_snap(&project, &[], Point::new(1.51, -y - 0.01), None, 0.2, all);
        assert_ne!(hit_outside.kind, SnapKind::Intersection);

        // Segment-circle intersection at (5, 0) is also the line endpoint -> Endpoint wins priority.
        let corner = find_snap(&project, &[], Point::new(5.01, 0.01), None, 0.2, all);
        assert_eq!(corner.kind, SnapKind::Endpoint);
        assert!(corner.point.near(Point::new(5.0, 0.0), 1e-4));

        // Segment-circle intersection at (1, 0) is strictly interior -> Intersection.
        let seg_circ = find_snap(&project, &[], Point::new(1.01, 0.02), None, 0.2, all);
        assert_eq!(seg_circ.kind, SnapKind::Intersection);
        assert!(seg_circ.point.near(Point::new(1.0, 0.0), 1e-4));
    }

    #[test]
    fn disabling_individual_osnap_modes_and_snapping_to_draft_points() {
        let mut project = Project::default();
        project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        let no_mid = OsnapSettings {
            midpoint: false,
            extension: false,
            ..Default::default()
        };
        // Midpoint at (2, 0) is ignored when midpoint OSNAP is disabled; falls back to grid.
        let hit = find_snap(&project, &[], Point::new(2.03, 0.02), None, 0.2, no_mid);
        assert_eq!(hit.kind, SnapKind::Grid);

        // Active polygon/polyline draft points expose endpoints and midpoints.
        let draft = [Point::new(1.0, 3.0), Point::new(5.0, 3.0)];
        let draft_end = find_snap(
            &project,
            &draft,
            Point::new(1.02, 3.01),
            None,
            0.2,
            OsnapSettings::default(),
        );
        assert_eq!(draft_end.kind, SnapKind::Endpoint);
        assert!(draft_end.point.near(Point::new(1.0, 3.0), 1e-4));

        let draft_mid = find_snap(
            &project,
            &draft,
            Point::new(3.01, 3.02),
            None,
            0.2,
            OsnapSettings::default(),
        );
        assert_eq!(draft_mid.kind, SnapKind::Midpoint);
        assert!(draft_mid.point.near(Point::new(3.0, 3.0), 1e-4));
    }
}
