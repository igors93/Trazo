//! Offsetting, trimming and extending walls and drawings, CAD style: an
//! offset is a parallel copy on the picked side; trimming removes the
//! piece between the nearest crossings around the click; extending runs an
//! open end up to the next wall, drawing or terrain edge.

use std::f64::consts::TAU;

use super::{Project, Wall};
use crate::domain::curves::{Curve, circle_hits, line_hits};
use crate::domain::drawing::{Shape, angle_of, point_at_angle};
use crate::domain::element::Element;
use crate::domain::geometry::{
    JOIN_TOLERANCE, POINT_TOLERANCE, Point, distance_to_segment, nearest_point_on_segment,
};
use crate::domain::offset::offset_shape;

/// How close to an end a crossing still counts as the end (0,1 mm).
pub(super) const END_TOLERANCE: f64 = 1e-4;
/// The shortest piece of drawing kept after trimming (a millimetre).
pub(super) const MIN_PIECE: f64 = 1e-3;
pub(super) const MIN_WALL_LENGTH: f64 = 0.25;

/// The stretch between the crossings `cuts` (positions along a path of
/// length `total`) around `at`.
fn span_around(cuts: &[f64], at: f64, total: f64) -> (f64, f64) {
    let before = cuts
        .iter()
        .copied()
        .filter(|cut| *cut < at)
        .fold(0.0, f64::max);
    let after = cuts
        .iter()
        .copied()
        .filter(|cut| *cut > at)
        .fold(total, f64::min);
    (before, after)
}

/// Positions along a path of its points, from 0 at the first.
pub(super) fn running_lengths(points: &[Point]) -> Vec<f64> {
    let mut lengths = Vec::with_capacity(points.len());
    let mut total = 0.0;
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            total += points[index - 1].distance(*point);
        }
        lengths.push(total);
    }
    lengths
}

/// The point at position `s` along the path.
pub(super) fn point_along(points: &[Point], lengths: &[f64], s: f64) -> Point {
    for i in 1..points.len() {
        if s <= lengths[i] || i == points.len() - 1 {
            let span = lengths[i] - lengths[i - 1];
            let t = if span > 0.0 {
                ((s - lengths[i - 1]) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return points[i - 1].lerp(points[i], t);
        }
    }
    points[0]
}

/// The part of the path from position `from` to `to`.
pub(super) fn sub_path(points: &[Point], lengths: &[f64], from: f64, to: f64) -> Vec<Point> {
    let mut piece = vec![point_along(points, lengths, from).rounded()];
    for (point, length) in points.iter().zip(lengths) {
        if *length > from + END_TOLERANCE && *length < to - END_TOLERANCE {
            piece.push(*point);
        }
    }
    piece.push(point_along(points, lengths, to).rounded());
    piece
}

/// Where along the path its nearest point to `at` lies.
pub(super) fn position_on(points: &[Point], lengths: &[f64], at: Point) -> f64 {
    let mut best = (f64::INFINITY, 0.0);
    for i in 1..points.len() {
        let (foot, t) = nearest_point_on_segment(at, points[i - 1], points[i]);
        let distance = foot.distance(at);
        if distance < best.0 {
            best = (distance, lengths[i - 1] + t * (lengths[i] - lengths[i - 1]));
        }
    }
    best.1
}

/// Where the path crosses `edges`, as positions along it, ends left out.
fn path_cuts(points: &[Point], lengths: &[f64], edges: &[Curve]) -> Vec<f64> {
    let total = *lengths.last().unwrap_or(&0.0);
    let mut cuts = Vec::new();
    for i in 1..points.len() {
        let span = lengths[i] - lengths[i - 1];
        if span <= 0.0 {
            continue;
        }
        for edge in edges {
            for t in line_hits(points[i - 1], points[i], edge) {
                if (-1e-9..=1.0 + 1e-9).contains(&t) {
                    let s = lengths[i - 1] + t.clamp(0.0, 1.0) * span;
                    if s > END_TOLERANCE && s < total - END_TOLERANCE {
                        cuts.push(s);
                    }
                }
            }
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    cuts
}

/// The crossings of the circle around `center` with `edges`, as angles
/// turned from `start` in the direction of `sign`, within `[0, limit)`.
fn arc_cuts(center: Point, radius: f64, start: f64, sign: f64, edges: &[Curve]) -> Vec<f64> {
    let mut cuts: Vec<f64> = edges
        .iter()
        .flat_map(|edge| circle_hits(center, radius, edge))
        .map(|angle| ((angle - start) * sign).rem_euclid(TAU))
        .collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    cuts
}

impl Project {
    /// The edges that cut or stop `except`: wall centre lines, drawings and
    /// the terrain's sides.
    fn edges_except(&self, except: Element) -> Vec<Curve> {
        let mut edges: Vec<Curve> = self
            .walls
            .iter()
            .filter(|wall| except != Element::Wall(wall.id))
            .map(|wall| Curve::Segment(wall.start, wall.end))
            .collect();
        for drawing in &self.drawings {
            if except != Element::Drawing(drawing.id) {
                edges.extend(Curve::of_shape(&drawing.shape));
            }
        }
        // The terrain's sides, straight or slanted.
        if let Some(corners) = self.terrain_corners() {
            let count = corners.len();
            for i in 0..count {
                edges.push(Curve::Segment(corners[i], corners[(i + 1) % count]));
            }
        }
        edges
    }

    /// Offsets a wall (a parallel wall `distance` away, centre line to
    /// centre line, of the same material) or a drawing towards `toward`;
    /// returns the new element.
    pub fn offset_element(
        &mut self,
        element: Element,
        distance: f64,
        toward: Point,
    ) -> Result<Element, String> {
        match element {
            Element::Wall(id) => {
                let wall = self
                    .walls
                    .iter()
                    .find(|wall| wall.id == id)
                    .cloned()
                    .ok_or("Parede não encontrada.")?;
                let line = Shape::Line {
                    start: wall.start,
                    end: wall.end,
                };
                let Shape::Line { start, end } = offset_shape(&line, distance, toward)? else {
                    return Err("Deslocamento inválido.".to_owned());
                };
                let new_id = self
                    .add_wall(start, end)
                    .ok_or("A parede deslocada precisa caber dentro do terreno.")?;
                if let Some(added) = self.walls.iter_mut().find(|w| w.id == new_id) {
                    added.masonry = wall.masonry;
                }
                Ok(Element::Wall(new_id))
            }
            Element::Drawing(id) => {
                let shape = self
                    .drawing(id)
                    .ok_or("Desenho não encontrado.")?
                    .shape
                    .clone();
                let offset = offset_shape(&shape, distance, toward)?;
                Ok(Element::Drawing(self.add_drawing(offset)?))
            }
            _ => Err("Só paredes e desenhos 2D podem ser deslocados (offset).".to_owned()),
        }
    }

    /// Trims the piece of a wall or drawing around `at` back to the
    /// nearest crossings with other walls, drawings or the terrain's edge.
    pub fn trim_element(&mut self, element: Element, at: Point) -> Result<(), String> {
        let edges = self.edges_except(element);
        match element {
            Element::Wall(id) => self.trim_wall(id, at, &edges),
            Element::Drawing(id) => {
                let shape = self
                    .drawing(id)
                    .ok_or("Desenho não encontrado.")?
                    .shape
                    .clone();
                let pieces = trim_shape(&shape, at, &edges)?;
                let mut tx = self.clone();
                let mut rest = pieces.into_iter();
                let first = rest.next().ok_or("Nada sobraria do desenho.")?;
                tx.update_drawing(id, first)?;
                for piece in rest {
                    tx.add_drawing(piece)?;
                }
                *self = tx;
                Ok(())
            }
            _ => Err("Só paredes e desenhos 2D podem ser aparados.".to_owned()),
        }
    }

    fn trim_wall(&mut self, id: u64, at: Point, edges: &[Curve]) -> Result<(), String> {
        let wall = self
            .walls
            .iter()
            .find(|wall| wall.id == id)
            .cloned()
            .ok_or("Parede não encontrada.")?;
        let points = [wall.start, wall.end];
        let lengths = running_lengths(&points);
        let length = lengths[1];
        let cuts = path_cuts(&points, &lengths, edges);
        let click = position_on(&points, &lengths, at);
        let (from, to) = span_around(&cuts, click, length);
        if from <= END_TOLERANCE && to >= length - END_TOLERANCE {
            return Err(
                "Nenhuma parede ou desenho corta este trecho; use Delete para apagar a parede."
                    .to_owned(),
            );
        }
        for opening in self.openings.iter().filter(|o| o.wall_id == id) {
            let center = opening.t * length;
            let half = opening.width() / 2.0;
            if center + half > from && center - half < to {
                return Err(
                    "Há uma porta ou janela no trecho a aparar; mova-a ou exclua-a antes."
                        .to_owned(),
                );
            }
        }
        let keep_start = from > END_TOLERANCE;
        let keep_end = to < length - END_TOLERANCE;
        if (keep_start && from < MIN_WALL_LENGTH - POINT_TOLERANCE)
            || (keep_end && length - to < MIN_WALL_LENGTH - POINT_TOLERANCE)
        {
            return Err("O trecho que sobra da parede ficaria menor que 0,25 m.".to_owned());
        }
        let cut_from = wall.start.lerp(wall.end, from / length).rounded();
        let cut_to = wall.start.lerp(wall.end, to / length).rounded();
        let mut tx = self.clone();
        let mut touched = vec![id];
        let moved: Vec<(u64, f64)> = tx
            .openings
            .iter()
            .filter(|o| o.wall_id == id)
            .map(|o| (o.id, o.t * length))
            .collect();
        if keep_start {
            if let Some(kept) = tx.walls.iter_mut().find(|w| w.id == id) {
                kept.end = cut_from;
            }
            let second = keep_end.then(|| tx.id());
            if let Some(new_id) = second {
                tx.walls.push(Wall {
                    id: new_id,
                    start: cut_to,
                    end: wall.end,
                    masonry: wall.masonry.clone(),
                    assembly: wall.assembly.clone(),
                    layer_id: wall.layer_id,
                });
                touched.push(new_id);
            }
            for (opening_id, center) in moved {
                let Some(opening) = tx.openings.iter_mut().find(|o| o.id == opening_id) else {
                    continue;
                };
                if center < from {
                    opening.t = center / from;
                } else if let Some(new_id) = second {
                    opening.wall_id = new_id;
                    opening.t = (center - to) / (length - to);
                }
            }
        } else {
            if let Some(kept) = tx.walls.iter_mut().find(|w| w.id == id) {
                kept.start = cut_to;
            }
            for (opening_id, center) in moved {
                if let Some(opening) = tx.openings.iter_mut().find(|o| o.id == opening_id) {
                    opening.t = (center - to) / (length - to);
                }
            }
        }
        tx.revalidate_openings_on_walls(&touched)?;
        tx.validate_affected_rooms(&touched)?;
        *self = tx;
        Ok(())
    }

    /// Extends the open end of a wall or drawing nearest `at` up to the
    /// next wall, drawing or terrain edge.
    pub fn extend_element(&mut self, element: Element, at: Point) -> Result<(), String> {
        let edges = self.edges_except(element);
        match element {
            Element::Wall(id) => self.extend_wall(id, at, &edges),
            Element::Drawing(id) => {
                let shape = self
                    .drawing(id)
                    .ok_or("Desenho não encontrado.")?
                    .shape
                    .clone();
                let longer = extend_shape(&shape, at, &edges)?;
                self.update_drawing(id, longer)
            }
            _ => Err("Só paredes e desenhos 2D podem ser estendidos.".to_owned()),
        }
    }

    fn extend_wall(&mut self, id: u64, at: Point, edges: &[Curve]) -> Result<(), String> {
        let wall = self
            .walls
            .iter()
            .find(|wall| wall.id == id)
            .cloned()
            .ok_or("Parede não encontrada.")?;
        let at_end = at.distance(wall.end) < at.distance(wall.start);
        let tip = if at_end { wall.end } else { wall.start };
        // An end that already meets a wall (an L, or a T on its side) stays.
        let joined = self.walls.iter().any(|other| {
            other.id != id && distance_to_segment(tip, other.start, other.end) <= JOIN_TOLERANCE
        });
        if joined {
            return Err("Esta ponta já encontra outra parede.".to_owned());
        }
        let reached = reach_along(wall.start, wall.end, at_end, edges)
            .ok_or("Nada para alcançar nessa direção: nenhuma parede, desenho ou divisa.")?
            .rounded();
        let other_end = if at_end { wall.start } else { wall.end };
        if !self.point_inside_buildable_limit(reached)
            || !self.point_inside_buildable_limit(reached.midpoint(other_end))
        {
            return Err(
                "A parede estendida sairia do limite de implantação do terreno.".to_owned(),
            );
        }
        let old_length = wall.length();
        let mut tx = self.clone();
        let Some(longer) = tx.walls.iter_mut().find(|w| w.id == id) else {
            return Err("Parede não encontrada.".to_owned());
        };
        if at_end {
            longer.end = reached;
        } else {
            longer.start = reached;
        }
        let new_length = longer.length();
        let added = new_length - old_length;
        // Doors and windows stay where they are on site.
        for opening in tx.openings.iter_mut().filter(|o| o.wall_id == id) {
            let center = opening.t * old_length + if at_end { 0.0 } else { added };
            opening.t = center / new_length;
        }
        tx.revalidate_openings_on_walls(&[id])?;
        tx.validate_affected_rooms(&[id])?;
        *self = tx;
        Ok(())
    }
}

/// The point the line `start`–`end` reaches beyond its end (`at_end`) or
/// its start: the nearest crossing with `edges` in that direction.
fn reach_along(start: Point, end: Point, at_end: bool, edges: &[Curve]) -> Option<Point> {
    let length = start.distance(end);
    if length <= 0.0 {
        return None;
    }
    let tolerance = END_TOLERANCE / length;
    let hits = edges.iter().flat_map(|edge| line_hits(start, end, edge));
    let t = if at_end {
        hits.filter(|t| *t > 1.0 + tolerance)
            .fold(None, |best: Option<f64>, t| {
                Some(best.map_or(t, |b| b.min(t)))
            })?
    } else {
        hits.filter(|t| *t < -tolerance)
            .fold(None, |best: Option<f64>, t| {
                Some(best.map_or(t, |b| b.max(t)))
            })?
    };
    Some(start.lerp(end, t))
}

/// What is left of `shape` after removing the piece around `at` between
/// its nearest crossings with `edges`.
fn trim_shape(shape: &Shape, at: Point, edges: &[Curve]) -> Result<Vec<Shape>, String> {
    let nothing = || "Nada corta este trecho; use Delete para apagar o desenho inteiro.".to_owned();
    let pieces = match shape {
        Shape::Line { start, end } => {
            let points = [*start, *end];
            let lengths = running_lengths(&points);
            let total = lengths[1];
            let cuts = path_cuts(&points, &lengths, edges);
            let (from, to) = span_around(&cuts, position_on(&points, &lengths, at), total);
            if from <= END_TOLERANCE && to >= total - END_TOLERANCE {
                return Err(nothing());
            }
            let mut pieces = Vec::new();
            if from > MIN_PIECE {
                pieces.push(Shape::Line {
                    start: *start,
                    end: point_along(&points, &lengths, from).rounded(),
                });
            }
            if total - to > MIN_PIECE {
                pieces.push(Shape::Line {
                    start: point_along(&points, &lengths, to).rounded(),
                    end: *end,
                });
            }
            pieces
        }
        Shape::Polyline {
            points,
            closed: false,
        } => {
            let lengths = running_lengths(points);
            let total = *lengths.last().unwrap_or(&0.0);
            let cuts = path_cuts(points, &lengths, edges);
            let (from, to) = span_around(&cuts, position_on(points, &lengths, at), total);
            if from <= END_TOLERANCE && to >= total - END_TOLERANCE {
                return Err(nothing());
            }
            let mut pieces = Vec::new();
            if from > MIN_PIECE {
                pieces.push(Shape::Polyline {
                    points: sub_path(points, &lengths, 0.0, from),
                    closed: false,
                });
            }
            if total - to > MIN_PIECE {
                pieces.push(Shape::Polyline {
                    points: sub_path(points, &lengths, to, total),
                    closed: false,
                });
            }
            pieces
        }
        Shape::Polyline {
            points,
            closed: true,
        } => {
            let mut path = points.clone();
            path.push(points[0]);
            let lengths = running_lengths(&path);
            let total = *lengths.last().unwrap_or(&0.0);
            let mut cuts = path_cuts(&path, &lengths, edges);
            // The closing corner is a place like any other on a loop.
            for edge in edges {
                if line_hits(path[0], path[1], edge)
                    .iter()
                    .any(|t| t.abs() * lengths[1] <= END_TOLERANCE)
                {
                    cuts.insert(0, 0.0);
                    break;
                }
            }
            if cuts.len() < 2 {
                return Err("Uma forma fechada precisa de dois cortes para ser aparada.".to_owned());
            }
            let click = position_on(&path, &lengths, at);
            let after = cuts
                .iter()
                .copied()
                .find(|cut| *cut > click)
                .unwrap_or(cuts[0]);
            let before = cuts
                .iter()
                .copied()
                .rev()
                .find(|cut| *cut < click)
                .unwrap_or(cuts[cuts.len() - 1]);
            let kept = if after < before {
                sub_path(&path, &lengths, after, before)
            } else {
                let mut joined = sub_path(&path, &lengths, after, total);
                joined.pop();
                joined.extend(sub_path(&path, &lengths, 0.0, before));
                joined
            };
            vec![Shape::Polyline {
                points: kept,
                closed: false,
            }]
        }
        Shape::Arc {
            center,
            radius,
            start,
            sweep,
        } => {
            let sign = sweep.signum();
            let total = sweep.abs();
            let tolerance = END_TOLERANCE / radius;
            let cuts: Vec<f64> = arc_cuts(*center, *radius, *start, sign, edges)
                .into_iter()
                .filter(|cut| *cut > tolerance && *cut < total - tolerance)
                .collect();
            let click = ((angle_of(*center, at) - start) * sign).rem_euclid(TAU);
            // A click beyond the arc's ends counts at the nearer end.
            let click = if click > total {
                if click - total < TAU - click {
                    total
                } else {
                    0.0
                }
            } else {
                click
            };
            let (from, to) = span_around(&cuts, click, total);
            if from <= tolerance && to >= total - tolerance {
                return Err(nothing());
            }
            let mut pieces = Vec::new();
            if from * radius > MIN_PIECE {
                pieces.push(Shape::Arc {
                    center: *center,
                    radius: *radius,
                    start: *start,
                    sweep: sign * from,
                });
            }
            if (total - to) * radius > MIN_PIECE {
                pieces.push(Shape::Arc {
                    center: *center,
                    radius: *radius,
                    start: start + sign * to,
                    sweep: sign * (total - to),
                });
            }
            pieces
        }
        Shape::Circle { center, radius } => {
            let cuts = arc_cuts(*center, *radius, 0.0, 1.0, edges);
            if cuts.len() < 2 {
                return Err("Um círculo precisa de dois cortes para ser aparado.".to_owned());
            }
            let click = angle_of(*center, at).rem_euclid(TAU);
            let after = cuts
                .iter()
                .copied()
                .find(|cut| *cut > click)
                .unwrap_or(cuts[0]);
            let before = cuts
                .iter()
                .copied()
                .rev()
                .find(|cut| *cut < click)
                .unwrap_or(cuts[cuts.len() - 1]);
            vec![Shape::Arc {
                center: *center,
                radius: *radius,
                start: after,
                sweep: (before - after).rem_euclid(TAU),
            }]
        }
        Shape::Spline { .. } => {
            return Err("Splines não podem ser aparadas.".to_owned());
        }
    };
    if pieces.is_empty() {
        return Err(nothing());
    }
    for piece in &pieces {
        piece.validate()?;
    }
    Ok(pieces)
}

/// `shape` with its open end nearest `at` run up to the next of `edges`.
fn extend_shape(shape: &Shape, at: Point, edges: &[Curve]) -> Result<Shape, String> {
    let nothing = || "Nada para alcançar nessa direção.".to_owned();
    let longer = match shape {
        Shape::Line { start, end } => {
            let at_end = at.distance(*end) < at.distance(*start);
            let reached = reach_along(*start, *end, at_end, edges)
                .ok_or_else(nothing)?
                .rounded();
            if at_end {
                Shape::Line {
                    start: *start,
                    end: reached,
                }
            } else {
                Shape::Line {
                    start: reached,
                    end: *end,
                }
            }
        }
        Shape::Polyline {
            points,
            closed: false,
        } => {
            let n = points.len();
            let mut points = points.clone();
            if at.distance(points[n - 1]) < at.distance(points[0]) {
                points[n - 1] = reach_along(points[n - 2], points[n - 1], true, edges)
                    .ok_or_else(nothing)?
                    .rounded();
            } else {
                points[0] = reach_along(points[0], points[1], false, edges)
                    .ok_or_else(nothing)?
                    .rounded();
            }
            Shape::Polyline {
                points,
                closed: false,
            }
        }
        Shape::Arc {
            center,
            radius,
            start,
            sweep,
        } => {
            let sign = sweep.signum();
            let end = start + sweep;
            let at_end = at.distance(point_at_angle(*center, *radius, end))
                < at.distance(point_at_angle(*center, *radius, *start));
            let tolerance = END_TOLERANCE / radius;
            let room = TAU - sweep.abs() - tolerance;
            let reach = if at_end {
                arc_cuts(*center, *radius, end, sign, edges)
            } else {
                arc_cuts(*center, *radius, *start, -sign, edges)
            }
            .into_iter()
            .find(|turn| *turn > tolerance && *turn < room)
            .ok_or_else(nothing)?;
            if at_end {
                Shape::Arc {
                    center: *center,
                    radius: *radius,
                    start: *start,
                    sweep: sweep + sign * reach,
                }
            } else {
                Shape::Arc {
                    center: *center,
                    radius: *radius,
                    start: start - sign * reach,
                    sweep: sweep + sign * reach,
                }
            }
        }
        _ => {
            return Err(
                "Só linhas, polilinhas abertas, arcos e paredes têm pontas para estender."
                    .to_owned(),
            );
        }
    };
    longer.validate()?;
    Ok(longer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::OpeningKind;

    /// A cross of two lines and a wall across them.
    fn cross() -> (Project, u64, u64) {
        let mut project = Project::default();
        let horizontal = project
            .add_drawing(Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(6.0, 0.0),
            })
            .unwrap();
        let vertical = project
            .add_drawing(Shape::Line {
                start: Point::new(3.0, -2.0),
                end: Point::new(3.0, 2.0),
            })
            .unwrap();
        (project, horizontal, vertical)
    }

    #[test]
    fn trimming_a_line_removes_the_clicked_side_of_the_crossing() {
        let (mut project, horizontal, _) = cross();
        project
            .trim_element(Element::Drawing(horizontal), Point::new(5.0, 0.0))
            .unwrap();
        assert_eq!(
            project.drawing(horizontal).unwrap().shape,
            Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(3.0, 0.0)
            }
        );
        // With a second cutter the middle piece goes and the line splits.
        let mut project = cross().0;
        project
            .add_drawing(Shape::Line {
                start: Point::new(5.0, -2.0),
                end: Point::new(5.0, 2.0),
            })
            .unwrap();
        project
            .trim_element(Element::Drawing(horizontal), Point::new(4.0, 0.0))
            .unwrap();
        let lines: Vec<f64> = project
            .drawings
            .iter()
            .filter(
                |d| matches!(d.shape, Shape::Line { start, end } if start.y == 0.0 && end.y == 0.0),
            )
            .map(|d| d.shape.length())
            .collect();
        assert_eq!(lines, vec![3.0, 1.0]);
        // With nothing crossing it, a line has nothing to be trimmed back to.
        let (mut lonely, horizontal, _) = cross();
        lonely.drawings.retain(|d| d.id == horizontal);
        assert!(
            lonely
                .trim_element(Element::Drawing(horizontal), Point::new(1.0, 0.0))
                .is_err()
        );
    }

    #[test]
    fn extending_runs_to_the_next_edge() {
        let (mut project, horizontal, vertical) = cross();
        project
            .update_drawing(
                horizontal,
                Shape::Line {
                    start: Point::new(0.0, 0.0),
                    end: Point::new(1.0, 0.0),
                },
            )
            .unwrap();
        project
            .extend_element(Element::Drawing(horizontal), Point::new(0.9, 0.1))
            .unwrap();
        assert_eq!(project.drawing(horizontal).unwrap().shape.length(), 3.0);
        // Nothing lies beyond the other end.
        assert!(
            project
                .extend_element(Element::Drawing(horizontal), Point::new(0.1, 0.0))
                .is_err()
        );
        // An arc runs along its circle up to the vertical line.
        let arc = project
            .add_drawing(Shape::Arc {
                center: Point::new(3.0, 5.0),
                radius: 1.0,
                start: 0.0,
                sweep: 0.5,
            })
            .unwrap();
        project
            .update_drawing(
                vertical,
                Shape::Line {
                    start: Point::new(1.0, 3.0),
                    end: Point::new(1.0, 8.0),
                },
            )
            .unwrap();
        assert!(
            project
                .extend_element(Element::Drawing(arc), Point::new(3.5, 5.9))
                .is_err()
        );
        project
            .update_drawing(
                vertical,
                Shape::Line {
                    start: Point::new(2.5, 3.0),
                    end: Point::new(2.5, 8.0),
                },
            )
            .unwrap();
        project
            .extend_element(Element::Drawing(arc), Point::new(3.5, 5.9))
            .unwrap();
        let Shape::Arc { sweep, .. } = project.drawing(arc).unwrap().shape else {
            panic!("an arc");
        };
        // It reaches x = 2,5 at 120° on its way round.
        assert!(
            (sweep - 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-6,
            "{sweep}"
        );
    }

    #[test]
    fn circles_and_rectangles_trim_between_two_cuts() {
        let mut project = Project::default();
        let circle = project
            .add_drawing(Shape::Circle {
                center: Point::ORIGIN,
                radius: 2.0,
            })
            .unwrap();
        project
            .add_drawing(Shape::Line {
                start: Point::new(-3.0, 0.0),
                end: Point::new(3.0, 0.0),
            })
            .unwrap();
        // Remove the half below the line on screen (y > 0).
        project
            .trim_element(Element::Drawing(circle), Point::new(0.0, 2.0))
            .unwrap();
        let shape = project.drawing(circle).unwrap().shape.clone();
        assert!(matches!(shape, Shape::Arc { .. }));
        assert!(shape.distance_to(Point::new(0.0, -2.0)) < 1e-6);
        assert!(shape.distance_to(Point::new(0.0, 2.0)) > 1.0);

        let rectangle = project
            .add_drawing(Shape::rectangle(
                Point::new(-1.0, -1.0),
                Point::new(1.0, 1.0),
            ))
            .unwrap();
        project
            .trim_element(Element::Drawing(rectangle), Point::new(0.0, 1.0))
            .unwrap();
        let open = project.drawing(rectangle).unwrap().shape.clone();
        assert!(!open.is_closed());
        // The top side between the two cuts is gone: 4 m of the 8 m remain.
        assert!((open.length() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn walls_trim_and_extend_keeping_doors_in_place() {
        let mut project = Project::default();
        let long = project
            .add_wall(Point::new(0.0, 0.0), Point::new(6.0, 0.0))
            .unwrap();
        project
            .add_wall(Point::new(4.0, -2.0), Point::new(4.0, 2.0))
            .unwrap();
        let door = project
            .add_opening_sized(long, OpeningKind::Door, 1.0 / 6.0, Some(0.8))
            .unwrap();
        project
            .trim_element(Element::Wall(long), Point::new(5.0, 0.0))
            .unwrap();
        let wall = project.walls.iter().find(|w| w.id == long).unwrap();
        assert_eq!(wall.end, Point::new(4.0, 0.0));
        let center = project.element_bounds(Element::Opening(door)).unwrap();
        assert!((center.0.midpoint(center.1).x - 1.0).abs() < 1e-6);
        // The door is in the way when trimming the left piece.
        project
            .add_wall(Point::new(2.0, -2.0), Point::new(2.0, 2.0))
            .unwrap();
        assert!(
            project
                .trim_element(Element::Wall(long), Point::new(1.0, 0.0))
                .is_err()
        );

        // A short wall runs up to the nearest wall's centre line.
        let short = project
            .add_wall(Point::new(-3.0, 1.0), Point::new(-1.0, 1.0))
            .unwrap();
        project
            .extend_element(Element::Wall(short), Point::new(-1.1, 1.0))
            .unwrap();
        let wall = project.walls.iter().find(|w| w.id == short).unwrap();
        assert_eq!(wall.end, Point::new(2.0, 1.0));
        // An end joined to another wall stays.
        assert!(
            project
                .extend_element(Element::Wall(short), Point::new(1.9, 1.0))
                .is_err()
        );
    }

    #[test]
    fn walls_and_drawings_offset_to_the_side_clicked() {
        let mut project = Project::default();
        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        let Element::Wall(copy) = project
            .offset_element(Element::Wall(wall), 1.2, Point::new(2.0, -3.0))
            .unwrap()
        else {
            panic!("a wall");
        };
        let copy = project.walls.iter().find(|w| w.id == copy).unwrap();
        assert_eq!(copy.start, Point::new(0.0, -1.2));
        assert_eq!(copy.thickness(), project.walls[0].thickness());
        project.set_terrain(Point::new(-1.0, -1.5), Point::new(5.0, 1.0));
        assert!(
            project
                .offset_element(Element::Wall(wall), 2.0, Point::new(2.0, 3.0))
                .is_err()
        );
        assert!(
            project
                .offset_element(Element::Terrain(0), 1.0, Point::ORIGIN)
                .is_err()
        );
    }
}
