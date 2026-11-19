//! Dividing and joining walls and drawings, aligning and distributing
//! elements, and repeating them in rows and columns or around a centre.

use std::f64::consts::TAU;

use super::modify::{
    END_TOLERANCE, MIN_PIECE, MIN_WALL_LENGTH, point_along, position_on, running_lengths, sub_path,
};
use super::{Project, Wall};
use crate::domain::drawing::{Shape, angle_of};
use crate::domain::element::Element;
use crate::domain::geometry::{JOIN_TOLERANCE, POINT_TOLERANCE, Point};
use crate::domain::transform::{Transform, normalize_angle};

/// The most copies one repeat makes.
pub const MAX_COPIES: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Left,
    CenterX,
    Right,
    Top,
    CenterY,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// The piece of a polyline path from `from` to `to`, as a shape.
fn polyline(points: Vec<Point>) -> Shape {
    Shape::Polyline {
        points,
        closed: false,
    }
}

/// `shape` cut in two at the point nearest `at`.
fn split_shape(shape: &Shape, at: Point) -> Result<Vec<Shape>, String> {
    let too_close = || "Divida longe das pontas do desenho.".to_owned();
    match shape {
        Shape::Line { start, end } => {
            let points = [*start, *end];
            let lengths = running_lengths(&points);
            let s = position_on(&points, &lengths, at);
            if s <= MIN_PIECE || s >= lengths[1] - MIN_PIECE {
                return Err(too_close());
            }
            let middle = point_along(&points, &lengths, s).rounded();
            Ok(vec![
                Shape::Line {
                    start: *start,
                    end: middle,
                },
                Shape::Line {
                    start: middle,
                    end: *end,
                },
            ])
        }
        Shape::Polyline { points, closed } => {
            let mut path = points.clone();
            if *closed {
                path.push(points[0]);
            }
            let lengths = running_lengths(&path);
            let total = *lengths.last().unwrap_or(&0.0);
            let s = position_on(&path, &lengths, at);
            if *closed {
                // A loop opens where it is cut.
                let mut open = sub_path(&path, &lengths, s, total);
                open.pop();
                open.extend(sub_path(&path, &lengths, 0.0, s));
                return Ok(vec![polyline(open)]);
            }
            if s <= MIN_PIECE || s >= total - MIN_PIECE {
                return Err(too_close());
            }
            Ok(vec![
                polyline(sub_path(&path, &lengths, 0.0, s)),
                polyline(sub_path(&path, &lengths, s, total)),
            ])
        }
        Shape::Arc {
            center,
            radius,
            start,
            sweep,
        } => {
            let sign = sweep.signum();
            let turned = ((angle_of(*center, at) - start) * sign).rem_euclid(TAU);
            if turned * radius <= MIN_PIECE || (sweep.abs() - turned) * radius <= MIN_PIECE {
                return Err(too_close());
            }
            Ok(vec![
                Shape::Arc {
                    center: *center,
                    radius: *radius,
                    start: *start,
                    sweep: sign * turned,
                },
                Shape::Arc {
                    center: *center,
                    radius: *radius,
                    start: normalize_angle(start + sign * turned),
                    sweep: sign * (sweep.abs() - turned),
                },
            ])
        }
        Shape::Circle { .. } => Err(
            "Um círculo precisa de dois cortes: use Aparar, ou divida-o em partes iguais."
                .to_owned(),
        ),
        Shape::Spline { .. } => Err("Splines não podem ser divididas.".to_owned()),
    }
}

/// `shape` cut into `parts` pieces of equal length.
fn divide_shape(shape: &Shape, parts: usize) -> Result<Vec<Shape>, String> {
    let n = parts as f64;
    match shape {
        Shape::Line { .. } | Shape::Polyline { .. } => {
            let (mut path, closed) = match shape {
                Shape::Line { start, end } => (vec![*start, *end], false),
                Shape::Polyline { points, closed } => (points.clone(), *closed),
                _ => unreachable!(),
            };
            if closed {
                path.push(path[0]);
            }
            let lengths = running_lengths(&path);
            let total = *lengths.last().unwrap_or(&0.0);
            if total / n < MIN_PIECE {
                return Err("Partes pequenas demais.".to_owned());
            }
            Ok((0..parts)
                .map(|k| {
                    let piece = sub_path(
                        &path,
                        &lengths,
                        total * k as f64 / n,
                        total * (k + 1) as f64 / n,
                    );
                    if piece.len() == 2 {
                        Shape::Line {
                            start: piece[0],
                            end: piece[1],
                        }
                    } else {
                        polyline(piece)
                    }
                })
                .collect())
        }
        Shape::Arc {
            center,
            radius,
            start,
            sweep,
        } => Ok((0..parts)
            .map(|k| Shape::Arc {
                center: *center,
                radius: *radius,
                start: normalize_angle(start + sweep * k as f64 / n),
                sweep: sweep / n,
            })
            .collect()),
        Shape::Circle { center, radius } => Ok((0..parts)
            .map(|k| Shape::Arc {
                center: *center,
                radius: *radius,
                start: normalize_angle(TAU * k as f64 / n),
                sweep: TAU / n,
            })
            .collect()),
        Shape::Spline { .. } => Err("Splines não podem ser divididas.".to_owned()),
    }
}

impl Project {
    /// Cuts a wall or drawing in two at the point nearest `at`; returns
    /// the pieces. A wall keeps its rooms, doors and windows.
    pub fn split_element(&mut self, element: Element, at: Point) -> Result<Vec<Element>, String> {
        match element {
            Element::Wall(id) => {
                let wall = self
                    .walls
                    .iter()
                    .find(|wall| wall.id == id)
                    .cloned()
                    .ok_or("Parede não encontrada.")?;
                let points = [wall.start, wall.end];
                let lengths = running_lengths(&points);
                let s = position_on(&points, &lengths, at);
                if s < MIN_WALL_LENGTH - POINT_TOLERANCE
                    || lengths[1] - s < MIN_WALL_LENGTH - POINT_TOLERANCE
                {
                    return Err("Cada parte da parede precisa de pelo menos 0,25 m.".to_owned());
                }
                let cut = point_along(&points, &lengths, s).rounded();
                let mut tx = self.clone();
                let second = tx.split_wall_at(id, cut).ok_or(
                    "O corte não pode passar por uma porta ou janela (nem a menos de 4 cm dela).",
                )?;
                *self = tx;
                Ok(vec![Element::Wall(id), Element::Wall(second)])
            }
            Element::Drawing(id) => {
                let shape = self
                    .drawing(id)
                    .ok_or("Desenho não encontrado.")?
                    .shape
                    .clone();
                self.replace_drawing(id, split_shape(&shape, at)?)
            }
            _ => Err("Só paredes e desenhos 2D podem ser divididos.".to_owned()),
        }
    }

    /// Cuts a wall or drawing into `parts` pieces of equal length.
    pub fn divide_element(
        &mut self,
        element: Element,
        parts: usize,
    ) -> Result<Vec<Element>, String> {
        if !(2..=100).contains(&parts) {
            return Err("Divida em 2 a 100 partes.".to_owned());
        }
        match element {
            Element::Wall(id) => {
                let wall = self
                    .walls
                    .iter()
                    .find(|wall| wall.id == id)
                    .cloned()
                    .ok_or("Parede não encontrada.")?;
                let length = wall.length();
                if length / (parts as f64) < MIN_WALL_LENGTH - POINT_TOLERANCE {
                    return Err(format!(
                        "Em {parts} partes, cada uma teria menos de 0,25 m."
                    ));
                }
                let mut tx = self.clone();
                let mut pieces = vec![Element::Wall(id)];
                let mut current = id;
                for k in 1..parts {
                    let cut = wall.start.lerp(wall.end, k as f64 / parts as f64).rounded();
                    current = tx.split_wall_at(current, cut).ok_or(
                        "Um dos cortes passaria por uma porta ou janela (ou a menos de 4 cm dela).",
                    )?;
                    pieces.push(Element::Wall(current));
                }
                *self = tx;
                Ok(pieces)
            }
            Element::Drawing(id) => {
                let shape = self
                    .drawing(id)
                    .ok_or("Desenho não encontrado.")?
                    .shape
                    .clone();
                self.replace_drawing(id, divide_shape(&shape, parts)?)
            }
            _ => Err("Só paredes e desenhos 2D podem ser divididos.".to_owned()),
        }
    }

    /// Puts `pieces` in the place of drawing `id`: the first keeps its id.
    fn replace_drawing(&mut self, id: u64, pieces: Vec<Shape>) -> Result<Vec<Element>, String> {
        let mut tx = self.clone();
        let mut created = Vec::with_capacity(pieces.len());
        for (index, piece) in pieces.into_iter().enumerate() {
            if index == 0 {
                tx.update_drawing(id, piece)?;
                created.push(Element::Drawing(id));
            } else {
                created.push(Element::Drawing(tx.add_drawing(piece)?));
            }
        }
        *self = tx;
        Ok(created)
    }

    /// Joins walls in line that meet end to end into one wall, or lines,
    /// open polylines and arcs of one circle that meet end to end into one
    /// drawing; returns the joined element.
    pub fn join_elements(&mut self, elements: &[Element]) -> Result<Element, String> {
        let walls: Vec<u64> = elements
            .iter()
            .filter_map(|element| match element {
                Element::Wall(id) => Some(*id),
                _ => None,
            })
            .collect();
        let drawings: Vec<u64> = elements
            .iter()
            .filter_map(|element| match element {
                Element::Drawing(id) => Some(*id),
                _ => None,
            })
            .collect();
        match (walls.len(), drawings.len()) {
            (0, 2..) => self.join_drawings(&drawings),
            (2.., 0) => self.join_walls(&walls),
            (1.., 1..) => Err("Una paredes com paredes, ou desenhos com desenhos.".to_owned()),
            _ => Err(
                "Selecione pelo menos duas paredes ou dois desenhos que se tocam pelas pontas."
                    .to_owned(),
            ),
        }
    }

    fn join_walls(&mut self, ids: &[u64]) -> Result<Element, String> {
        let walls: Vec<Wall> = ids
            .iter()
            .filter_map(|id| self.walls.iter().find(|wall| wall.id == *id).cloned())
            .collect();
        let first = walls.first().ok_or("Parede não encontrada.")?;
        let origin = first.start;
        let along = first
            .end
            .minus(first.start)
            .normalized()
            .ok_or("Parede sem comprimento.")?;
        let normal = Point::new(-along.y, along.x);
        let in_line = walls.iter().all(|wall| {
            wall.start.minus(origin).dot(normal).abs() <= JOIN_TOLERANCE
                && wall.end.minus(origin).dot(normal).abs() <= JOIN_TOLERANCE
        });
        if !in_line {
            return Err(
                "As paredes precisam estar alinhadas, uma em continuação da outra.".to_owned(),
            );
        }
        if walls.iter().any(|wall| wall.masonry != first.masonry) {
            return Err(
                "As paredes são de materiais diferentes; iguale-os antes de unir.".to_owned(),
            );
        }
        // Each wall as a stretch along the line, in order.
        let mut spans: Vec<(f64, f64, &Wall)> = walls
            .iter()
            .map(|wall| {
                let a = wall.start.minus(origin).dot(along);
                let b = wall.end.minus(origin).dot(along);
                (a.min(b), a.max(b), wall)
            })
            .collect();
        spans.sort_by(|x, y| x.0.total_cmp(&y.0));
        if spans
            .windows(2)
            .any(|pair| (pair[1].0 - pair[0].1).abs() > JOIN_TOLERANCE)
        {
            return Err(
                "As paredes precisam se tocar pelas pontas, sem vãos nem sobreposições.".to_owned(),
            );
        }
        let from = spans[0].0;
        let to = spans[spans.len() - 1].1;
        let kept = spans[0].2.id;
        let start = origin.translated(along.scaled(from)).rounded();
        let end = origin.translated(along.scaled(to)).rounded();
        let length = to - from;

        let mut tx = self.clone();
        // Doors and windows keep their place and their look on site.
        for opening in tx.openings.iter_mut() {
            let Some(wall) = walls.iter().find(|wall| wall.id == opening.wall_id) else {
                continue;
            };
            let center = wall.start.lerp(wall.end, opening.t);
            opening.t = (center.minus(origin).dot(along) - from) / length;
            opening.wall_id = kept;
            if wall.end.minus(wall.start).dot(along) < 0.0 {
                opening.flip_side = !opening.flip_side;
                opening.flip_hinge = !opening.flip_hinge;
            }
        }
        tx.walls
            .retain(|wall| wall.id == kept || !ids.contains(&wall.id));
        if let Some(wall) = tx.walls.iter_mut().find(|wall| wall.id == kept) {
            wall.start = start;
            wall.end = end;
        }
        for room in &mut tx.rooms {
            if room.wall_ids.iter().any(|id| ids.contains(id)) {
                let mut merged = Vec::with_capacity(room.wall_ids.len());
                for id in &room.wall_ids {
                    let id = if ids.contains(id) { kept } else { *id };
                    if !merged.contains(&id) {
                        merged.push(id);
                    }
                }
                room.wall_ids = merged;
            }
        }
        tx.revalidate_openings_on_walls(&[kept])?;
        tx.validate_affected_rooms(&[kept])?;
        *self = tx;
        Ok(Element::Wall(kept))
    }

    fn join_drawings(&mut self, ids: &[u64]) -> Result<Element, String> {
        let shapes: Vec<Shape> = ids
            .iter()
            .filter_map(|id| self.drawing(*id).map(|drawing| drawing.shape.clone()))
            .collect();
        let joined = if shapes
            .iter()
            .all(|shape| matches!(shape, Shape::Arc { .. }))
        {
            join_arcs(&shapes)?
        } else {
            join_paths(&shapes)?
        };
        joined.validate()?;
        let mut tx = self.clone();
        tx.update_drawing(ids[0], joined)?;
        for id in &ids[1..] {
            tx.delete_drawing(*id);
        }
        *self = tx;
        Ok(Element::Drawing(ids[0]))
    }

    /// Rooms that share walls move as one block; every other element moves
    /// on its own. Doors, windows and the terrain stay out.
    fn arrange_units(&self, elements: &[Element]) -> Vec<Vec<Element>> {
        let mut units: Vec<Vec<Element>> = Vec::new();
        let rooms: Vec<u64> = elements
            .iter()
            .filter_map(|element| match element {
                Element::Room(id) => Some(*id),
                _ => None,
            })
            .collect();
        let walls_of = |id: u64| {
            self.rooms
                .iter()
                .find(|room| room.id == id)
                .map(|room| room.wall_ids.clone())
                .unwrap_or_default()
        };
        for room in &rooms {
            let mine = walls_of(*room);
            let touching: Vec<usize> = units
                .iter()
                .enumerate()
                .filter(|(_, unit)| {
                    unit.iter().any(|element| match element {
                        Element::Room(other) => {
                            walls_of(*other).iter().any(|wall| mine.contains(wall))
                        }
                        _ => false,
                    })
                })
                .map(|(index, _)| index)
                .collect();
            let mut unit = vec![Element::Room(*room)];
            for index in touching.into_iter().rev() {
                unit.extend(units.remove(index));
            }
            units.push(unit);
        }
        let room_walls: Vec<u64> = rooms.iter().flat_map(|id| walls_of(*id)).collect();
        for element in elements {
            match element {
                Element::Wall(id) if !room_walls.contains(id) => units.push(vec![*element]),
                Element::Fixture(_) | Element::Drawing(_) => units.push(vec![*element]),
                _ => {}
            }
        }
        units
    }

    /// The box a unit takes on the plan, rooms by their walls' faces.
    fn unit_bounds(&self, unit: &[Element]) -> Option<(Point, Point)> {
        let parts: Vec<Element> = unit
            .iter()
            .flat_map(|element| match element {
                Element::Room(id) => self
                    .rooms
                    .iter()
                    .find(|room| room.id == *id)
                    .map(|room| room.wall_ids.iter().map(|w| Element::Wall(*w)).collect())
                    .unwrap_or_default(),
                _ => vec![*element],
            })
            .collect();
        self.elements_bounds(&parts)
    }

    /// Lines up `elements` on one edge or the middle of the box around them
    /// all; returns how many moved.
    pub fn align_elements(
        &mut self,
        elements: &[Element],
        alignment: Alignment,
    ) -> Result<usize, String> {
        let units = self.arrange_units(elements);
        if units.len() < 2 {
            return Err("Selecione pelo menos dois elementos para alinhar.".to_owned());
        }
        let boxes: Vec<(Point, Point)> = units
            .iter()
            .map(|unit| self.unit_bounds(unit).ok_or("Elemento sem posição."))
            .collect::<Result<_, _>>()?;
        let low = boxes
            .iter()
            .fold(Point::new(f64::INFINITY, f64::INFINITY), |acc, (lo, _)| {
                Point::new(acc.x.min(lo.x), acc.y.min(lo.y))
            });
        let high = boxes.iter().fold(
            Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
            |acc, (_, hi)| Point::new(acc.x.max(hi.x), acc.y.max(hi.y)),
        );
        let middle = low.midpoint(high);
        let mut tx = self.clone();
        let mut moved = 0;
        for (unit, (lo, hi)) in units.iter().zip(&boxes) {
            let delta = match alignment {
                Alignment::Left => Point::new(low.x - lo.x, 0.0),
                Alignment::Right => Point::new(high.x - hi.x, 0.0),
                Alignment::CenterX => Point::new(middle.x - (lo.x + hi.x) / 2.0, 0.0),
                Alignment::Top => Point::new(0.0, low.y - lo.y),
                Alignment::Bottom => Point::new(0.0, high.y - hi.y),
                Alignment::CenterY => Point::new(0.0, middle.y - (lo.y + hi.y) / 2.0),
            };
            if delta.x.abs() > 1e-9 || delta.y.abs() > 1e-9 {
                tx.move_elements(unit, delta)?;
                moved += 1;
            }
        }
        *self = tx;
        Ok(moved)
    }

    /// Spaces `elements` (at least three) evenly between the first and the
    /// last along `axis`; returns how many moved.
    pub fn distribute_elements(
        &mut self,
        elements: &[Element],
        axis: Axis,
    ) -> Result<usize, String> {
        let units = self.arrange_units(elements);
        if units.len() < 3 {
            return Err("Selecione pelo menos três elementos para distribuir.".to_owned());
        }
        let along = |point: Point| match axis {
            Axis::Horizontal => point.x,
            Axis::Vertical => point.y,
        };
        let mut placed: Vec<(f64, f64, &Vec<Element>)> = units
            .iter()
            .map(|unit| {
                self.unit_bounds(unit)
                    .map(|(lo, hi)| (along(lo), along(hi), unit))
                    .ok_or("Elemento sem posição.")
            })
            .collect::<Result<_, _>>()?;
        placed.sort_by(|a, b| a.0.total_cmp(&b.0));
        let span = placed[placed.len() - 1].1 - placed[0].0;
        let sizes: f64 = placed.iter().map(|(lo, hi, _)| hi - lo).sum();
        let gap = (span - sizes) / (placed.len() - 1) as f64;
        if gap < -POINT_TOLERANCE {
            return Err(
                "Não há espaço para distribuir: os elementos se sobrepõem nessa direção."
                    .to_owned(),
            );
        }
        let mut tx = self.clone();
        let mut moved = 0;
        let mut cursor = placed[0].1 + gap;
        for (lo, hi, unit) in &placed[1..placed.len() - 1] {
            let shift = cursor - lo;
            if shift.abs() > 1e-9 {
                let delta = match axis {
                    Axis::Horizontal => Point::new(shift, 0.0),
                    Axis::Vertical => Point::new(0.0, shift),
                };
                tx.move_elements(unit, delta)?;
                moved += 1;
            }
            cursor += hi - lo + gap;
        }
        *self = tx;
        Ok(moved)
    }

    /// Copies `elements` into a grid of `columns` × `rows` (the original
    /// is the first cell), `spacing` apart; returns the copies.
    pub fn repeat_grid(
        &mut self,
        elements: &[Element],
        columns: usize,
        rows: usize,
        spacing: Point,
    ) -> Result<Vec<Element>, String> {
        let cells = columns.saturating_mul(rows);
        if columns == 0 || rows == 0 || cells < 2 {
            return Err("Use pelo menos 2 colunas ou 2 linhas.".to_owned());
        }
        if cells - 1 > MAX_COPIES {
            return Err(format!("No máximo {MAX_COPIES} cópias de uma vez."));
        }
        if !spacing.is_finite() {
            return Err("Espaçamento inválido.".to_owned());
        }
        let transforms: Vec<Transform> = (0..rows)
            .flat_map(|row| (0..columns).map(move |column| (row, column)))
            .skip(1)
            .map(|(row, column)| {
                Transform::Translate(Point::new(
                    spacing.x * column as f64,
                    spacing.y * row as f64,
                ))
            })
            .collect();
        self.repeat(elements, &transforms)
    }

    /// Copies `elements` around `center`: `count` in all (the original is
    /// the first) over `degrees` (a full turn spreads them evenly).
    pub fn repeat_around(
        &mut self,
        elements: &[Element],
        center: Point,
        count: usize,
        degrees: f64,
    ) -> Result<Vec<Element>, String> {
        if !(2..=MAX_COPIES + 1).contains(&count) {
            return Err(format!("Use de 2 a {} itens.", MAX_COPIES + 1));
        }
        if !degrees.is_finite() || degrees.abs() < 1e-6 || degrees.abs() > 360.0 {
            return Err("O ângulo total deve estar entre 0 e 360°.".to_owned());
        }
        if !center.is_finite() {
            return Err("Centro inválido.".to_owned());
        }
        let step = if (degrees.abs() - 360.0).abs() < 1e-6 {
            degrees / count as f64
        } else {
            degrees / (count - 1) as f64
        };
        let transforms: Vec<Transform> = (1..count)
            .map(|k| Transform::rotate_degrees(center, step * k as f64))
            .collect();
        self.repeat(elements, &transforms)
    }

    fn repeat(
        &mut self,
        elements: &[Element],
        transforms: &[Transform],
    ) -> Result<Vec<Element>, String> {
        let fragment = self.fragment(elements);
        if fragment.is_empty() {
            return Err(
                "Nada para repetir: portas e janelas seguem as suas paredes, e o terreno não se repete."
                    .to_owned(),
            );
        }
        let mut tx = self.clone();
        let mut created = Vec::new();
        for transform in transforms {
            let copy = fragment.transformed(transform)?;
            created.extend(tx.insert_fragment(&copy)?);
        }
        *self = tx;
        Ok(created)
    }
}

/// Lines and open polylines that meet end to end, as one path.
fn join_paths(shapes: &[Shape]) -> Result<Shape, String> {
    let mut pieces: Vec<Vec<Point>> = Vec::with_capacity(shapes.len());
    for shape in shapes {
        match shape {
            Shape::Line { start, end } => pieces.push(vec![*start, *end]),
            Shape::Polyline {
                points,
                closed: false,
            } => pieces.push(points.clone()),
            _ => {
                return Err(
                    "Só linhas e polilinhas abertas se unem, ou arcos do mesmo círculo.".to_owned(),
                );
            }
        }
    }
    let touch = |a: Point, b: Point| a.near(b, POINT_TOLERANCE * 2.0);
    let mut chain = pieces.remove(0);
    while !pieces.is_empty() {
        let (head, tail) = (chain[0], chain[chain.len() - 1]);
        let found = pieces.iter().position(|piece| {
            let (first, last) = (piece[0], piece[piece.len() - 1]);
            touch(tail, first) || touch(tail, last) || touch(head, first) || touch(head, last)
        });
        let Some(index) = found else {
            return Err("Os desenhos precisam se tocar pelas pontas para serem unidos.".to_owned());
        };
        let mut piece = pieces.remove(index);
        let (first, last) = (piece[0], piece[piece.len() - 1]);
        if touch(tail, first) {
            chain.extend(piece.into_iter().skip(1));
        } else if touch(tail, last) {
            piece.reverse();
            chain.extend(piece.into_iter().skip(1));
        } else if touch(head, last) {
            piece.pop();
            piece.extend(chain);
            chain = piece;
        } else {
            piece.reverse();
            piece.pop();
            piece.extend(chain);
            chain = piece;
        }
    }
    let n = chain.len();
    if n >= 4 && touch(chain[0], chain[n - 1]) {
        chain.pop();
        return Ok(Shape::Polyline {
            points: chain,
            closed: true,
        });
    }
    let length: f64 = chain.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
    let (first, last) = (chain[0], chain[n - 1]);
    if (length - first.distance(last)).abs() <= END_TOLERANCE {
        return Ok(Shape::Line {
            start: first,
            end: last,
        });
    }
    Ok(polyline(chain))
}

/// Arcs of one circle that meet end to end, as one arc (or the circle).
fn join_arcs(shapes: &[Shape]) -> Result<Shape, String> {
    let mut spans: Vec<(f64, f64)> = Vec::new();
    let (mut center, mut radius) = (Point::ORIGIN, 0.0);
    for (index, shape) in shapes.iter().enumerate() {
        let Shape::Arc {
            center: c,
            radius: r,
            start,
            sweep,
        } = shape
        else {
            unreachable!();
        };
        if index == 0 {
            (center, radius) = (*c, *r);
        } else if !c.near(center, POINT_TOLERANCE) || (r - radius).abs() > POINT_TOLERANCE {
            return Err("Só arcos do mesmo círculo (mesmo centro e raio) se unem.".to_owned());
        }
        // Counter-clockwise in plan angles: from the lower angle.
        let (from, turn) = if *sweep >= 0.0 {
            (*start, *sweep)
        } else {
            (start + sweep, -sweep)
        };
        spans.push((from, turn));
    }
    let tolerance = END_TOLERANCE / radius.max(1e-6);
    let meets = |a: f64, b: f64| {
        let gap = (a - b).rem_euclid(TAU);
        gap <= tolerance || TAU - gap <= tolerance
    };
    let (mut from, mut turn) = spans.remove(0);
    while !spans.is_empty() {
        let found = spans
            .iter()
            .position(|(start, sweep)| meets(*start, from + turn) || meets(start + sweep, from));
        let Some(index) = found else {
            return Err("Os arcos precisam se tocar pelas pontas para serem unidos.".to_owned());
        };
        let (start, sweep) = spans.remove(index);
        if meets(start, from + turn) {
            turn += sweep;
        } else {
            from = start;
            turn += sweep;
        }
    }
    if turn >= TAU - tolerance {
        return Ok(Shape::Circle { center, radius });
    }
    Ok(Shape::Arc {
        center,
        radius,
        start: normalize_angle(from),
        sweep: turn,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;
    use crate::domain::model::OpeningKind;

    #[test]
    fn walls_split_divide_and_join_back_keeping_doors() {
        let mut project = Project::default();
        let room = project
            .add_room(Point::new(0.0, 0.0), Point::new(6.0, 3.0), "Sala")
            .unwrap();
        let bottom = project.rooms[0].wall_ids[0];
        let door = project
            .add_opening(bottom, OpeningKind::Door, 0.75)
            .unwrap();
        let before = project.element_bounds(Element::Opening(door)).unwrap();

        // Cutting through the door is refused; beside it, it splits.
        assert!(
            project
                .split_element(Element::Wall(bottom), Point::new(4.5, 0.0))
                .is_err()
        );
        let pieces = project
            .split_element(Element::Wall(bottom), Point::new(2.0, 0.0))
            .unwrap();
        assert_eq!(pieces.len(), 2);
        let room_ref = project.rooms.iter().find(|r| r.id == room).unwrap();
        assert_eq!(project.room_area(room_ref), Some(18.0));
        let moved = project.element_bounds(Element::Opening(door)).unwrap();
        assert!(moved.0.near(before.0, 1e-6) && moved.1.near(before.1, 1e-6));

        // Joined again, the wall is whole and the door where it was.
        let joined = project.join_elements(&pieces).unwrap();
        assert_eq!(joined, Element::Wall(bottom));
        assert_eq!(project.walls.len(), 4);
        let wall = project.walls.iter().find(|w| w.id == bottom).unwrap();
        assert!((wall.length() - 6.0).abs() < 1e-9);
        let after = project.element_bounds(Element::Opening(door)).unwrap();
        assert!(after.0.near(before.0, 1e-6) && after.1.near(before.1, 1e-6));
        let room_ref = project.rooms.iter().find(|r| r.id == room).unwrap();
        assert_eq!(room_ref.wall_ids.len(), 4);

        // The top wall, 6 m, in three equal parts.
        let top = project.rooms[0].wall_ids[2];
        let thirds = project.divide_element(Element::Wall(top), 3).unwrap();
        assert_eq!(thirds.len(), 3);
        for piece in &thirds {
            let Element::Wall(id) = piece else { panic!() };
            let wall = project.walls.iter().find(|w| w.id == *id).unwrap();
            assert!((wall.length() - 2.0).abs() < 1e-9);
        }
        assert!(project.divide_element(Element::Wall(top), 30).is_err());
        // Walls not in line do not join.
        let side = project.rooms[0].wall_ids.last().copied().unwrap();
        assert!(
            project
                .join_elements(&[Element::Wall(bottom), Element::Wall(side)])
                .is_err()
        );
    }

    #[test]
    fn drawings_split_divide_and_join() {
        let mut project = Project::default();
        let line = project
            .add_drawing(Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(4.0, 0.0),
            })
            .unwrap();
        let halves = project
            .split_element(Element::Drawing(line), Point::new(1.0, 0.3))
            .unwrap();
        assert_eq!(project.drawing(line).unwrap().shape.length(), 1.0);
        // Joined back into one straight line.
        project.join_elements(&halves).unwrap();
        assert_eq!(project.drawings.len(), 1);
        assert_eq!(
            project.drawing(line).unwrap().shape,
            Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(4.0, 0.0)
            }
        );

        // Three lines around a corner make a polyline; closing a loop
        // makes a closed one.
        let up = project
            .add_drawing(Shape::Line {
                start: Point::new(4.0, 0.0),
                end: Point::new(4.0, 3.0),
            })
            .unwrap();
        let back = project
            .add_drawing(Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(4.0, 3.0),
            })
            .unwrap();
        project
            .join_elements(&[
                Element::Drawing(up),
                Element::Drawing(line),
                Element::Drawing(back),
            ])
            .unwrap();
        let shape = project.drawing(up).unwrap().shape.clone();
        assert!(shape.is_closed());
        assert!((shape.length() - 12.0).abs() < 1e-9);

        // A circle in four arcs, and the arcs joined into the circle again.
        let circle = project
            .add_drawing(Shape::Circle {
                center: Point::new(10.0, 0.0),
                radius: 1.0,
            })
            .unwrap();
        let quarters = project.divide_element(Element::Drawing(circle), 4).unwrap();
        assert_eq!(quarters.len(), 4);
        project.join_elements(&quarters[..3]).unwrap();
        let Shape::Arc { sweep, .. } = project.drawing(circle).unwrap().shape else {
            panic!("an arc");
        };
        assert!((sweep - 1.5 * std::f64::consts::PI).abs() < 1e-9);
        project
            .join_elements(&[Element::Drawing(circle), quarters[3]])
            .unwrap();
        assert!(matches!(
            project.drawing(circle).unwrap().shape,
            Shape::Circle { .. }
        ));
        assert!(
            project
                .join_elements(&[Element::Drawing(circle), Element::Drawing(up)])
                .is_err()
        );
    }

    #[test]
    fn align_and_distribute_by_the_edges_of_the_selection() {
        let mut project = Project::default();
        let ids: Vec<u64> = [(0.0, 0.0), (3.0, 1.0), (8.0, 0.5)]
            .into_iter()
            .map(|(x, y)| {
                project
                    .add_fixture(FixtureKind::Table, Point::new(x, y), 1.0, 1.0, 0)
                    .unwrap()
            })
            .collect();
        let elements: Vec<Element> = ids.iter().map(|id| Element::Fixture(*id)).collect();
        assert_eq!(
            project.align_elements(&elements, Alignment::Top).unwrap(),
            2
        );
        assert!(
            project
                .fixtures
                .iter()
                .all(|f| (f.center.y - 0.0).abs() < 1e-9)
        );
        assert_eq!(
            project
                .distribute_elements(&elements, Axis::Horizontal)
                .unwrap(),
            1
        );
        let middle = project.fixtures.iter().find(|f| f.id == ids[1]).unwrap();
        assert!((middle.center.x - 4.0).abs() < 1e-9);
        assert!(
            project
                .align_elements(&elements[..1], Alignment::Left)
                .is_err()
        );
        assert!(
            project
                .distribute_elements(&elements[..2], Axis::Vertical)
                .is_err()
        );
    }

    #[test]
    fn repeats_make_grids_and_rings_of_copies() {
        let mut project = Project::default();
        let chair = project
            .add_fixture(FixtureKind::Table, Point::new(0.0, -2.0), 0.5, 0.5, 0)
            .unwrap();
        let copies = project
            .repeat_grid(&[Element::Fixture(chair)], 3, 2, Point::new(1.0, 1.0))
            .unwrap();
        assert_eq!(copies.len(), 5);
        assert_eq!(project.fixtures.len(), 6);
        assert!(
            project
                .fixtures
                .iter()
                .any(|f| f.center == Point::new(2.0, -1.0))
        );
        let ring = project
            .repeat_around(&[Element::Fixture(chair)], Point::ORIGIN, 4, 360.0)
            .unwrap();
        assert_eq!(ring.len(), 3);
        assert!(
            project
                .fixtures
                .iter()
                .any(|f| f.center == Point::new(2.0, 0.0))
        );
        // Fixtures only turn in quarter turns.
        assert!(
            project
                .repeat_around(&[Element::Fixture(chair)], Point::ORIGIN, 6, 360.0)
                .is_err()
        );
        assert!(
            project
                .repeat_grid(&[Element::Fixture(chair)], 1, 1, Point::new(1.0, 1.0))
                .is_err()
        );
    }
}
