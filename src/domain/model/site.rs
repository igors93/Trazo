use super::{Lot, Project, Terrain};
use crate::domain::geometry::{Point, polygon_area, polygon_is_simple};

fn bounds(points: &[Point]) -> Option<(Point, Point)> {
    let first = *points.first()?;
    Some(
        points
            .iter()
            .skip(1)
            .fold((first, first), |(min, max), point| {
                (
                    Point::new(min.x.min(point.x), min.y.min(point.y)),
                    Point::new(max.x.max(point.x), max.y.max(point.y)),
                )
            }),
    )
}

fn clip_half_plane(polygon: &[Point], a: Point, b: Point, keep_left: bool) -> Vec<Point> {
    let side = |point: Point| b.minus(a).cross(point.minus(a));
    let inside = |value: f64| {
        if keep_left {
            value >= -1e-8
        } else {
            value <= 1e-8
        }
    };
    let mut output = Vec::new();
    for index in 0..polygon.len() {
        let current = polygon[index];
        let next = polygon[(index + 1) % polygon.len()];
        let current_side = side(current);
        let next_side = side(next);
        let current_inside = inside(current_side);
        let next_inside = inside(next_side);
        if current_inside {
            output.push(current);
        }
        if current_inside != next_inside {
            let ratio = current_side / (current_side - next_side);
            output.push(current.lerp(next, ratio).rounded());
        }
    }
    output.dedup_by(|left, right| left.near(*right, 1e-7));
    output
}

impl Project {
    pub fn set_polygon_terrain(&mut self, corners: &[Point]) -> Result<u64, String> {
        let corners: Vec<Point> = corners.iter().map(|point| point.rounded()).collect();
        if corners.len() < 3 || !polygon_is_simple(&corners) || polygon_area(&corners) < 0.25 {
            return Err(
                "A divisa precisa ser um polígono simples com pelo menos 0,25 m².".to_owned(),
            );
        }
        let (min, max) = bounds(&corners).unwrap();
        let id = self
            .terrain
            .as_ref()
            .map(|terrain| terrain.id)
            .unwrap_or_else(|| self.id());
        let terrain = Terrain {
            id,
            min,
            max,
            vertices: corners,
        };
        if self
            .walls
            .iter()
            .any(|wall| !terrain.contains(wall.start) || !terrain.contains(wall.end))
        {
            return Err("A nova divisa precisa conter todas as paredes.".to_owned());
        }
        self.terrain = Some(terrain);
        self.lots.clear();
        Ok(id)
    }

    /// Splits the terrain, or an existing lot crossed by the line, into two
    /// polygonal lots. Areas are conserved within numeric tolerance.
    pub fn subdivide_terrain(&mut self, a: Point, b: Point) -> Result<(u64, u64), String> {
        if a.distance(b) < 0.01 {
            return Err("A linha de desmembramento precisa de dois pontos distintos.".to_owned());
        }
        let sources: Vec<(Option<usize>, Vec<Point>)> = if self.lots.is_empty() {
            vec![(
                None,
                self.terrain
                    .as_ref()
                    .ok_or("Defina o terreno primeiro.")?
                    .outline(),
            )]
        } else {
            self.lots
                .iter()
                .enumerate()
                .map(|(index, lot)| (Some(index), lot.vertices.clone()))
                .collect()
        };
        let Some((source_index, source, left, right)) =
            sources.into_iter().find_map(|(index, source)| {
                let left = clip_half_plane(&source, a, b, true);
                let right = clip_half_plane(&source, a, b, false);
                (left.len() >= 3
                    && right.len() >= 3
                    && polygon_area(&left) >= 0.25
                    && polygon_area(&right) >= 0.25)
                    .then_some((index, source, left, right))
            })
        else {
            return Err(
                "A linha precisa atravessar um lote e deixar pelo menos 0,25 m² de cada lado."
                    .to_owned(),
            );
        };
        let original_area = polygon_area(&source);
        if (polygon_area(&left) + polygon_area(&right) - original_area).abs() > 0.01 {
            return Err("Não foi possível conservar a área no desmembramento.".to_owned());
        }
        if let Some(index) = source_index {
            self.lots.remove(index);
        } else {
            self.lots.clear();
        }
        let first_id = self.id();
        let second_id = self.id();
        let base = self.lots.len() + 1;
        self.lots.push(Lot {
            id: first_id,
            name: format!("Lote {base}"),
            vertices: left,
        });
        self.lots.push(Lot {
            id: second_id,
            name: format!("Lote {}", base + 1),
            vertices: right,
        });
        Ok((first_id, second_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn irregular_terrain_and_subdivision_preserve_geometry_and_area() {
        let mut project = Project::default();
        project
            .set_polygon_terrain(&[
                Point::new(0.0, 0.0),
                Point::new(10.0, 1.0),
                Point::new(8.0, 8.0),
                Point::new(1.0, 6.0),
            ])
            .unwrap();
        let terrain = project.terrain.as_ref().unwrap();
        assert!(terrain.contains(Point::new(4.0, 4.0)));
        assert!(!terrain.contains(Point::new(9.5, 7.5)));
        let area = terrain.area();

        project
            .subdivide_terrain(Point::new(5.0, -2.0), Point::new(5.0, 10.0))
            .unwrap();
        assert_eq!(project.lots.len(), 2);
        let lots_area: f64 = project.lots.iter().map(Lot::area).sum();
        assert!((lots_area - area).abs() < 0.01);
    }
}
