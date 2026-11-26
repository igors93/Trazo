//! Block paging (#54): how each wall is laid, course by course. Whole
//! blocks, half blocks and cut pieces in running bond (every other course
//! starts with a half block), the gaps of doors and windows, and the
//! concrete lintels and sills that take the place of blocks around them.
//! The first course is the one the bricklayer marks on the floor.

use crate::domain::annotations::frame_types;
use crate::domain::geometry::Point;
use crate::domain::masonry::Masonry;
use crate::domain::masonry_structure::MasonryBandKind;
use crate::domain::model::{OpeningKind, Project, Wall};

/// What a piece of a course is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PieceKind {
    Whole,
    Half,
    /// A block cut to fit the end of a run.
    Cut,
}

/// One block or piece, measured along the wall from its start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub start_m: f64,
    pub length_m: f64,
    pub kind: PieceKind,
}

/// One course: its height band and its pieces.
#[derive(Clone, Debug, PartialEq)]
pub struct Course {
    pub bottom_m: f64,
    pub height_m: f64,
    pub pieces: Vec<Piece>,
}

/// A door, window or gap in the wall's face.
#[derive(Clone, Debug, PartialEq)]
pub struct OpeningSpan {
    pub code: String,
    pub from_m: f64,
    pub to_m: f64,
    pub bottom_m: f64,
    pub top_m: f64,
}

/// A concrete lintel or sill band in the wall's face.
#[derive(Clone, Debug, PartialEq)]
pub struct BandSpan {
    pub label: String,
    pub kind: MasonryBandKind,
    pub from_m: f64,
    pub to_m: f64,
    pub bottom_m: f64,
    pub top_m: f64,
}

/// How many pieces of each kind, and the cut lengths.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PieceCounts {
    pub whole: usize,
    pub half: usize,
    pub cut: usize,
}

impl PieceCounts {
    fn add(&mut self, piece: &Piece) {
        match piece.kind {
            PieceKind::Whole => self.whole += 1,
            PieceKind::Half => self.half += 1,
            PieceKind::Cut => self.cut += 1,
        }
    }

    fn of<'a>(pieces: impl IntoIterator<Item = &'a Piece>) -> Self {
        let mut counts = Self::default();
        for piece in pieces {
            counts.add(piece);
        }
        counts
    }

    /// "12 inteiros, 1 meio-bloco, 2 cortados".
    pub fn summary(&self) -> String {
        format!(
            "{} inteiro(s), {} meio(s)-bloco(s), {} cortado(s)",
            self.whole, self.half, self.cut
        )
    }
}

/// The paging of one wall.
#[derive(Clone, Debug, PartialEq)]
pub struct WallPaging {
    pub wall_id: u64,
    pub label: String,
    pub start: Point,
    pub end: Point,
    pub length_m: f64,
    /// Heights of the top at the start and at the end of the wall.
    pub top_m: (f64, f64),
    pub masonry: Masonry,
    pub courses: Vec<Course>,
    pub openings: Vec<OpeningSpan>,
    pub bands: Vec<BandSpan>,
    /// Pieces of the first course only.
    pub first: PieceCounts,
    /// Pieces of the whole wall.
    pub total: PieceCounts,
}

impl WallPaging {
    pub fn first_course(&self) -> &[Piece] {
        self.courses.first().map_or(&[], |c| c.pieces.as_slice())
    }
}

/// Lays the pieces of one course along `segments`, starting with a half
/// block on odd courses (running bond).
fn lay(segments: &[(f64, f64)], masonry: &Masonry, odd: bool) -> Vec<Piece> {
    let joint = masonry.joint;
    let whole = masonry.block_length;
    let half = ((whole - joint) / 2.0).max(0.01);
    let mut pieces = Vec::new();
    for &(from, to) in segments {
        let mut pos = from;
        let mut first = true;
        while to - pos > 0.005 {
            let want = if first && odd { half } else { whole };
            first = false;
            let available = to - pos;
            let (length, kind) = if available + 1e-6 >= want {
                (
                    want,
                    if (want - half).abs() < 1e-9 {
                        PieceKind::Half
                    } else {
                        PieceKind::Whole
                    },
                )
            } else if (available - half).abs() < 0.01 {
                (available, PieceKind::Half)
            } else {
                (available, PieceKind::Cut)
            };
            pieces.push(Piece {
                start_m: pos,
                length_m: length,
                kind,
            });
            pos += length + joint;
        }
    }
    pieces
}

/// `[0, length]` without the blocked intervals, merged and sorted.
fn free_segments(length: f64, mut blocked: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut segments = Vec::new();
    let mut pos = 0.0;
    for (from, to) in blocked {
        let (from, to) = (from.max(0.0), to.min(length));
        if to <= from {
            continue;
        }
        if from - pos > 0.01 {
            segments.push((pos, from));
        }
        pos = pos.max(to);
    }
    if length - pos > 0.01 {
        segments.push((pos, length));
    }
    segments
}

/// Where along the wall the top stays at or above `height`.
fn under_top(length: f64, top: (f64, f64), height: f64) -> Option<(f64, f64)> {
    let (a, b) = top;
    if a >= height && b >= height {
        return Some((0.0, length));
    }
    if a < height && b < height {
        return None;
    }
    let u = (height - a) / (b - a) * length;
    if a >= height {
        Some((0.0, u))
    } else {
        Some((u, length))
    }
}

/// The paging of every wall of the active floor.
pub fn wall_paging(project: &Project) -> Vec<WallPaging> {
    let codes: std::collections::HashMap<u64, String> = frame_types(project)
        .into_iter()
        .flat_map(|frame| {
            frame
                .ids
                .into_iter()
                .map(move |id| (id, frame.code.clone()))
        })
        .collect();
    let bands = project.masonry_structure_report().bands;
    let storey = project.wall_height.max(0.5);
    project
        .walls
        .iter()
        .map(|wall| {
            let masonry = wall
                .masonry
                .clone()
                .unwrap_or_else(|| project.masonry.clone());
            paging_of(project, wall, masonry, storey, &codes, &bands)
        })
        .collect()
}

fn paging_of(
    project: &Project,
    wall: &Wall,
    masonry: Masonry,
    storey: f64,
    codes: &std::collections::HashMap<u64, String>,
    bands: &[crate::domain::masonry_structure::MasonryBandResult],
) -> WallPaging {
    let length = wall.length();
    let top = wall.heights(storey);
    let direction = if length > 1e-9 {
        Point::new(
            (wall.end.x - wall.start.x) / length,
            (wall.end.y - wall.start.y) / length,
        )
    } else {
        Point::new(1.0, 0.0)
    };
    let along = |p: Point| (p.x - wall.start.x) * direction.x + (p.y - wall.start.y) * direction.y;
    let openings: Vec<OpeningSpan> = project
        .openings
        .iter()
        .filter(|o| o.wall_id == wall.id)
        .map(|o| {
            let centre = o.t * length;
            let half = o.width() / 2.0;
            let bottom = o.sill();
            let top_m = match o.kind {
                OpeningKind::Gap => top.0.min(top.1),
                _ => bottom + o.height().unwrap_or(storey - bottom),
            };
            OpeningSpan {
                code: codes.get(&o.id).cloned().unwrap_or_default(),
                from_m: centre - half,
                to_m: centre + half,
                bottom_m: bottom,
                top_m,
            }
        })
        .collect();
    let band_spans: Vec<BandSpan> = bands
        .iter()
        .filter(|b| b.wall_id == wall.id)
        .filter_map(|b| {
            let opening = project.openings.iter().find(|o| o.id == b.opening_id)?;
            let sill = opening.sill();
            let opening_top = sill + opening.height().unwrap_or(storey - sill);
            let (bottom, top_m) = match b.kind {
                MasonryBandKind::Lintel => (opening_top, opening_top + b.height_m),
                MasonryBandKind::Sill => ((sill - b.height_m).max(0.0), sill),
            };
            let (from, to) = (along(b.start), along(b.end));
            Some(BandSpan {
                label: b.label.clone(),
                kind: b.kind,
                from_m: from.min(to),
                to_m: from.max(to),
                bottom_m: bottom,
                top_m,
            })
        })
        .collect();
    let course_h = (masonry.block_height + masonry.joint).max(0.02);
    let tallest = top.0.max(top.1);
    let mut courses = Vec::new();
    let mut index = 0;
    loop {
        let bottom = index as f64 * course_h;
        let course_top = bottom + course_h;
        if course_top > tallest + 1e-6 {
            break;
        }
        let Some(span) = under_top(length, top, course_top - masonry.joint) else {
            break;
        };
        let mut blocked: Vec<(f64, f64)> = openings
            .iter()
            .filter(|o| o.bottom_m < course_top - 1e-6 && o.top_m > bottom + 1e-6)
            .map(|o| (o.from_m, o.to_m))
            .chain(
                band_spans
                    .iter()
                    .filter(|b| b.bottom_m < course_top - 1e-6 && b.top_m > bottom + 1e-6)
                    .map(|b| (b.from_m, b.to_m)),
            )
            .collect();
        if span.0 > 0.0 {
            blocked.push((0.0, span.0));
        }
        if span.1 < length {
            blocked.push((span.1, length));
        }
        let segments = free_segments(length, blocked);
        courses.push(Course {
            bottom_m: bottom,
            height_m: masonry.block_height,
            pieces: lay(&segments, &masonry, index % 2 == 1),
        });
        index += 1;
        if index > 400 {
            break;
        }
    }
    let first = courses
        .first()
        .map(|c| PieceCounts::of(&c.pieces))
        .unwrap_or_default();
    let total = PieceCounts::of(courses.iter().flat_map(|c| &c.pieces));
    WallPaging {
        wall_id: wall.id,
        label: format!("Parede {}", wall.id),
        start: wall.start,
        end: wall.end,
        length_m: length,
        top_m: top,
        masonry,
        courses,
        openings,
        bands: band_spans,
        first,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block() -> Masonry {
        Masonry {
            name: "Bloco de concreto 14".to_owned(),
            block_width: 0.14,
            block_length: 0.39,
            joint: 0.01,
            plaster: 0.015,
            block_height: 0.19,
        }
    }

    #[test]
    fn a_course_lays_whole_blocks_and_a_cut_end_and_odd_courses_start_half() {
        let masonry = block();
        let even = lay(&[(0.0, 2.60)], &masonry, false);
        assert_eq!(PieceCounts::of(&even).whole, 6);
        assert_eq!(even.last().map(|p| p.kind), Some(PieceKind::Cut));
        let odd = lay(&[(0.0, 2.60)], &masonry, true);
        assert_eq!(odd[0].kind, PieceKind::Half);
        assert!((odd[0].length_m - 0.19).abs() < 1e-9);
    }

    #[test]
    fn doors_interrupt_every_course_and_windows_only_their_own() {
        use crate::domain::geometry::Point;
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let wall = project.walls[0].id;
        project.add_opening(wall, OpeningKind::Door, 0.5).unwrap();
        let paging = wall_paging(&project);
        let door_wall = paging.iter().find(|p| p.wall_id == wall).unwrap();
        let door = &door_wall.openings[0];
        let crosses_door = |c: &Course| {
            c.pieces.iter().any(|p| {
                p.start_m < door.to_m - 0.02 && p.start_m + p.length_m > door.from_m + 0.02
            })
        };
        assert!(!crosses_door(&door_wall.courses[0]));
        assert!(door_wall.courses.len() > 10);
        assert!(door_wall.total.whole > door_wall.first.whole);
    }
}
