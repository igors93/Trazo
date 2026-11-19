//! Direct vertex and grip manipulation for walls, polygonal rooms, CAD
//! drawing entities and terrain corners.

use super::Project;
use crate::domain::{
    drawing::{Shape, point_at_angle},
    geometry::{self, JOIN_TOLERANCE, POINT_TOLERANCE, Point},
};

const MIN_WALL_LENGTH: f64 = 0.25;
const OPENING_END_MARGIN: f64 = 0.05;
const OPENING_GAP: f64 = 0.05;

impl Project {
    /// Moves every wall endpoint joined at `anchor` (within `JOIN_TOLERANCE`)
    /// to `to`, keeping connected corners, polygonal rooms and openings
    /// consistent transactionally.
    pub fn move_wall_corner(&mut self, anchor: Point, to: Point) -> Result<(), String> {
        if !to.is_finite() {
            return Err("Coordenadas inválidas.".to_owned());
        }
        let target = to.rounded();
        if let Some(terrain) = &self.terrain
            && !terrain.contains(target)
        {
            return Err("O vértice precisa permanecer dentro do terreno.".to_owned());
        }

        let mut tx = self.clone();
        let mut affected_walls = Vec::new();
        for wall in &mut tx.walls {
            let mut changed = false;
            if wall.start.near(anchor, JOIN_TOLERANCE) {
                wall.start = target;
                changed = true;
            }
            if wall.end.near(anchor, JOIN_TOLERANCE) {
                wall.end = target;
                changed = true;
            }
            if changed {
                if wall.length() < MIN_WALL_LENGTH - POINT_TOLERANCE {
                    return Err("A parede ficaria menor que 0,25 m.".to_owned());
                }
                affected_walls.push(wall.id);
            }
        }
        if affected_walls.is_empty() {
            return Err("Nenhum vértice de parede encontrado nessa posição.".to_owned());
        }

        tx.revalidate_openings_on_walls(&affected_walls)?;
        tx.validate_affected_rooms(&affected_walls)?;
        *self = tx;
        Ok(())
    }

    /// Re-clamps and checks openings on every wall in `wall_ids`.
    pub(super) fn revalidate_openings_on_walls(&mut self, wall_ids: &[u64]) -> Result<(), String> {
        for &wall_id in wall_ids {
            let Some(wall) = self.walls.iter().find(|w| w.id == wall_id) else {
                continue;
            };
            let len = wall.length();
            let mut spans: Vec<(f64, f64)> = Vec::new();
            for op in &mut self.openings {
                if op.wall_id != wall_id {
                    continue;
                }
                let width = op.width();
                if len < width + OPENING_END_MARGIN * 2.0 - 1e-4 {
                    return Err(
                        "A parede ficaria curta demais para manter as aberturas existentes."
                            .to_owned(),
                    );
                }
                let margin_t = (width * 0.5 + OPENING_END_MARGIN) / len;
                op.t = op.t.clamp(margin_t, 1.0 - margin_t);
                let center = op.t * len;
                spans.push((center - width * 0.5, center + width * 0.5));
            }
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            for pair in spans.windows(2) {
                if pair[0].1 + OPENING_GAP - 1e-4 > pair[1].0 {
                    return Err("As aberturas da parede se sobreporiam.".to_owned());
                }
            }
        }
        Ok(())
    }

    /// Ensures every room touching any wall in `wall_ids` stays a valid,
    /// simple polygon with area at least 0.25 m².
    pub(super) fn validate_affected_rooms(&self, wall_ids: &[u64]) -> Result<(), String> {
        for room in &self.rooms {
            if !room.wall_ids.iter().any(|id| wall_ids.contains(id)) {
                continue;
            }
            let Some(poly) = self.room_polygon(room) else {
                return Err(format!("O cômodo \"{}\" perderia o fechamento.", room.name));
            };
            let outline = geometry::simplify_polygon(&poly);
            if outline.len() < 3 || !geometry::polygon_is_simple(&outline) {
                return Err(format!(
                    "Os lados do cômodo \"{}\" não podem se cruzar.",
                    room.name
                ));
            }
            if geometry::polygon_area(&outline) < 0.25 - 1e-4 {
                return Err(format!(
                    "O cômodo \"{}\" ficaria menor que 0,25 m².",
                    room.name
                ));
            }
        }
        Ok(())
    }

    /// Draggable vertex/control handles for a 2D CAD drawing shape.
    pub fn drawing_handles(shape: &Shape) -> Vec<Point> {
        match shape {
            Shape::Line { start, end } => vec![*start, *end],
            Shape::Polyline { points, .. } | Shape::Spline { points, .. } => points.clone(),
            Shape::Circle { center, radius } => {
                vec![*center, Point::new(center.x + *radius, center.y)]
            }
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => vec![
                point_at_angle(*center, *radius, *start),
                point_at_angle(*center, *radius, *start + *sweep * 0.5),
                point_at_angle(*center, *radius, *start + *sweep),
            ],
        }
    }

    /// Moves handle `handle` of CAD drawing `id` to `to`, validating the
    /// resulting shape before committing.
    pub fn move_drawing_handle(&mut self, id: u64, handle: usize, to: Point) -> Result<(), String> {
        if !to.is_finite() {
            return Err("Coordenadas inválidas.".to_owned());
        }
        let target = to.rounded();
        let current = self
            .drawing(id)
            .ok_or("Desenho não encontrado.")?
            .shape
            .clone();

        let next = match current {
            Shape::Line { start, end } => match handle {
                0 => Shape::Line { start: target, end },
                1 => Shape::Line { start, end: target },
                _ => return Err("Vértice inválido.".to_owned()),
            },
            Shape::Polyline { mut points, closed } => {
                let slot = points.get_mut(handle).ok_or("Vértice inválido.")?;
                *slot = target;
                Shape::Polyline { points, closed }
            }
            Shape::Spline { mut points, closed } => {
                let slot = points.get_mut(handle).ok_or("Vértice inválido.")?;
                *slot = target;
                Shape::Spline { points, closed }
            }
            Shape::Circle { center, radius } => match handle {
                0 => Shape::Circle {
                    center: target,
                    radius,
                },
                1 => Shape::Circle {
                    center,
                    radius: center.distance(target),
                },
                _ => return Err("Vértice inválido.".to_owned()),
            },
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let mut pts = [
                    point_at_angle(center, radius, start),
                    point_at_angle(center, radius, start + sweep * 0.5),
                    point_at_angle(center, radius, start + sweep),
                ];
                let slot = pts.get_mut(handle).ok_or("Vértice inválido.")?;
                *slot = target;
                Shape::arc_through(pts[0], pts[1], pts[2])
                    .ok_or("Os três pontos do arco não podem ficar em linha reta.")?
            }
        };

        self.update_drawing(id, next)
    }

    /// Four corners of the terrain in clockwise order: TL, TR, BR, BL.
    pub fn terrain_corners(&self) -> Option<[Point; 4]> {
        let t = self.terrain.as_ref()?;
        Some([
            t.min,
            Point::new(t.max.x, t.min.y),
            t.max,
            Point::new(t.min.x, t.max.y),
        ])
    }

    /// Moves corner `corner` (0=TL, 1=TR, 2=BR, 3=BL) of the terrain to `to`,
    /// ensuring all existing walls and fixtures remain inside the terrain.
    pub fn move_terrain_corner(&mut self, corner: usize, to: Point) -> Result<(), String> {
        if !to.is_finite() {
            return Err("Coordenadas inválidas.".to_owned());
        }
        let target = to.rounded();
        let Some(current) = self.terrain.as_ref() else {
            return Err("Nenhum terreno definido.".to_owned());
        };
        let (mut min_x, mut min_y, mut max_x, mut max_y) =
            (current.min.x, current.min.y, current.max.x, current.max.y);
        match corner {
            0 => {
                min_x = target.x;
                min_y = target.y;
            }
            1 => {
                max_x = target.x;
                min_y = target.y;
            }
            2 => {
                max_x = target.x;
                max_y = target.y;
            }
            3 => {
                min_x = target.x;
                max_y = target.y;
            }
            _ => return Err("Canto do terreno inválido.".to_owned()),
        }
        if max_x - min_x < MIN_WALL_LENGTH - POINT_TOLERANCE
            || max_y - min_y < MIN_WALL_LENGTH - POINT_TOLERANCE
        {
            return Err("O terreno precisa ter pelo menos 0,25 m em cada lado.".to_owned());
        }
        let mut next = current.clone();
        next.min = Point::new(min_x, min_y).rounded();
        next.max = Point::new(max_x, max_y).rounded();

        for wall in &self.walls {
            if !next.contains(wall.start) || !next.contains(wall.end) {
                return Err("As paredes precisam continuar dentro do terreno.".to_owned());
            }
        }
        for fixture in &self.fixtures {
            if !next.contains(fixture.center) {
                return Err("Os objetos precisam continuar dentro do terreno.".to_owned());
            }
        }
        self.terrain = Some(next);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::OpeningKind;

    #[test]
    fn moving_room_corner_updates_connected_walls_and_area() {
        let mut project = Project::default();
        // L-shaped room: 4x4 minus 2x2 notch (area = 12 m²).
        let room_id = project
            .add_polygon_room(
                &[
                    Point::new(0.0, 0.0),
                    Point::new(4.0, 0.0),
                    Point::new(4.0, 2.0),
                    Point::new(2.0, 2.0),
                    Point::new(2.0, 4.0),
                    Point::new(0.0, 4.0),
                ],
                "Sala em L",
            )
            .unwrap();
        // Drag the inner notch corner (2, 2) to (3, 2) -> notch becomes 1x2, area grows from 12 to 13 m².
        // Wait: (4,2)-(3,2)-(2,4) forms a diagonal; moving (2,2)->(3,2) adds triangle of base 1, height 2 -> +1 m²!
        project
            .move_wall_corner(Point::new(2.0, 2.0), Point::new(3.0, 2.0))
            .unwrap();
        let room = project.rooms.iter().find(|r| r.id == room_id).unwrap();
        assert!((project.room_area(room).unwrap() - 13.0).abs() < 1e-4);

        // Self-intersecting drag (pulling (3, 2) past (0, 0) to (-2, -2)) is rejected cleanly.
        assert!(
            project
                .move_wall_corner(Point::new(3.0, 2.0), Point::new(-2.0, -2.0))
                .is_err()
        );
        let room = project.rooms.iter().find(|r| r.id == room_id).unwrap();
        assert!((project.room_area(room).unwrap() - 13.0).abs() < 1e-4);
    }

    #[test]
    fn moving_wall_corner_protects_openings_and_terrain_bounds() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let wall_id = project
            .add_wall(Point::new(1.0, 1.0), Point::new(4.0, 1.0))
            .unwrap();
        project
            .add_opening(wall_id, OpeningKind::Door, 0.5)
            .unwrap();

        // Shrinking the wall below door width + margins (0.90 + 0.10 = 1.00 m) is rejected.
        assert!(
            project
                .move_wall_corner(Point::new(4.0, 1.0), Point::new(1.5, 1.0))
                .is_err()
        );
        // Moving outside the terrain is rejected.
        assert!(
            project
                .move_wall_corner(Point::new(4.0, 1.0), Point::new(12.0, 1.0))
                .is_err()
        );
        // Valid stretch to (6, 1) succeeds and keeps the door.
        project
            .move_wall_corner(Point::new(4.0, 1.0), Point::new(6.0, 1.0))
            .unwrap();
        assert!((project.walls[0].length() - 5.0).abs() < 1e-4);
        assert_eq!(project.openings.len(), 1);
    }

    #[test]
    fn moving_drawing_handles_and_terrain_corners_edits_geometry_directly() {
        let mut project = Project::default();
        let line_id = project
            .add_drawing(Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(3.0, 0.0),
            })
            .unwrap();
        project
            .move_drawing_handle(line_id, 1, Point::new(3.0, 4.0))
            .unwrap();
        assert!((project.drawing(line_id).unwrap().shape.length() - 5.0).abs() < 1e-4);

        let circle_id = project
            .add_drawing(Shape::Circle {
                center: Point::new(5.0, 5.0),
                radius: 1.0,
            })
            .unwrap();
        project
            .move_drawing_handle(circle_id, 1, Point::new(8.0, 5.0))
            .unwrap();
        match project.drawing(circle_id).unwrap().shape {
            Shape::Circle { radius, .. } => assert!((radius - 3.0).abs() < 1e-4),
            ref other => panic!("expected circle, got {other:?}"),
        }

        let arc_id = project
            .add_drawing(
                Shape::arc_through(
                    Point::new(0.0, 0.0),
                    Point::new(1.0, 1.0),
                    Point::new(2.0, 0.0),
                )
                .unwrap(),
            )
            .unwrap();
        let handles = Project::drawing_handles(&project.drawing(arc_id).unwrap().shape);
        assert_eq!(handles.len(), 3);
        project
            .move_drawing_handle(arc_id, 1, Point::new(1.0, 2.0))
            .unwrap();
        // Collinear arc handle drag is rejected cleanly.
        assert!(
            project
                .move_drawing_handle(arc_id, 1, Point::new(1.0, 0.0))
                .is_err()
        );

        // Terrain corner resize respects walls inside it.
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        project
            .add_wall(Point::new(2.0, 2.0), Point::new(6.0, 2.0))
            .unwrap();
        assert!(
            project
                .move_terrain_corner(2, Point::new(4.0, 10.0))
                .is_err()
        );
        project
            .move_terrain_corner(2, Point::new(8.0, 9.0))
            .unwrap();
        let terrain = project.terrain.unwrap();
        assert!((terrain.width() - 8.0).abs() < 1e-4);
        assert!((terrain.height() - 9.0).abs() < 1e-4);
    }
}
