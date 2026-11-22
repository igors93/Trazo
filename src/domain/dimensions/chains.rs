//! Automatic associative opening dimension chains (#53).
//!
//! For each wall carrying doors, windows, gates or open spans, generates a
//! collinear chain of associative `Aligned` dimensions from `WallStart`
//! through every opening jamb in order (`OpeningLeft` → `OpeningRight`) to
//! `WallEnd`, offset cleanly to the outward side of the wall.

use crate::domain::{geometry::Point, model::Project};

use super::{DimensionAnchor, DimensionKind};

/// Minimum segment length (2 cm) worth dimensioning along a wall chain so
/// openings flush with a wall end do not create zero-length clutter.
const MIN_SEGMENT_M: f64 = 0.02;

/// Summary returned after generating opening dimension chains.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpeningChainSummary {
    pub walls_processed: usize,
    pub dimensions_added: usize,
}

/// Summary returned after generating the full set of site dimensions
/// (internal room dimensions + opening chains along walls).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SiteDimensionSummary {
    pub internal_added: usize,
    pub opening_chains: OpeningChainSummary,
}

impl SiteDimensionSummary {
    pub fn total_added(self) -> usize {
        self.internal_added + self.opening_chains.dimensions_added
    }
}

/// Signed perpendicular offset that places a wall's opening chain outside the
/// room (for perimeter walls) or toward the more spacious/exterior side (for
/// standalone and interior partition walls).
pub fn outward_offset_for_wall(project: &Project, wall_id: u64, distance_m: f64) -> f64 {
    let magnitude = distance_m.abs().max(0.15);
    let Some(wall) = project.walls.iter().find(|w| w.id == wall_id) else {
        return -magnitude;
    };
    let Some(dir) = wall.end.minus(wall.start).normalized() else {
        return -magnitude;
    };
    let left_normal = Point::new(-dir.y, dir.x);
    let mid = wall.start.lerp(wall.end, 0.5);

    // Adjacent rooms that reference this wall.
    let mut adjacent_rooms: Vec<(Point, f64)> = Vec::new();
    for room in &project.rooms {
        if room.wall_ids.contains(&wall_id)
            && let Some((min, max)) = project.room_bounds(room)
        {
            let center = Point::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5);
            let area = (max.x - min.x).abs() * (max.y - min.y).abs();
            adjacent_rooms.push((center, area));
        }
    }

    match adjacent_rooms.len() {
        // Exterior / perimeter wall of a room: offset strictly AWAY from the room interior.
        1 => {
            let room_center = adjacent_rooms[0].0;
            let outward = mid.minus(room_center);
            let dot = outward.x * left_normal.x + outward.y * left_normal.y;
            if dot >= 0.0 { magnitude } else { -magnitude }
        }
        // Interior partition between two rooms: place on the side of the larger room
        // at a modest offset so it stays clear of tight spaces (bathrooms, corridors).
        2.. => {
            let interior_dist = magnitude.min(0.35);
            let larger_center = adjacent_rooms
                .iter()
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(c, _)| *c)
                .unwrap_or(mid);
            let toward_larger = larger_center.minus(mid);
            let dot = toward_larger.x * left_normal.x + toward_larger.y * left_normal.y;
            if dot >= 0.0 {
                interior_dist
            } else {
                -interior_dist
            }
        }
        // Standalone wall (no room attached): offset away from the building bounding-box center.
        0 => {
            if project.walls.len() > 1 {
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for w in &project.walls {
                    for p in [w.start, w.end] {
                        min_x = min_x.min(p.x);
                        min_y = min_y.min(p.y);
                        max_x = max_x.max(p.x);
                        max_y = max_y.max(p.y);
                    }
                }
                let center = Point::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
                let outward = mid.minus(center);
                let dot = outward.x * left_normal.x + outward.y * left_normal.y;
                if dot.abs() > 1e-4 {
                    return if dot > 0.0 { magnitude } else { -magnitude };
                }
            }
            -magnitude
        }
    }
}

/// Ordered associative anchor pairs along a wall (`WallStart → Op1.Left →
/// Op1.Right → ... → WallEnd`), sorted along `wall.start -> wall.end`.
pub fn wall_opening_chain_pairs(
    project: &Project,
    wall_id: u64,
) -> Vec<(DimensionAnchor, DimensionAnchor)> {
    let Some(wall) = project.walls.iter().find(|w| w.id == wall_id) else {
        return Vec::new();
    };
    let wall_len = wall.length();
    if wall_len <= MIN_SEGMENT_M {
        return Vec::new();
    }
    let Some(dir) = wall.end.minus(wall.start).normalized() else {
        return Vec::new();
    };

    let mut openings: Vec<(u64, f64)> = project
        .openings
        .iter()
        .filter(|o| o.wall_id == wall_id)
        .map(|o| (o.id, o.t))
        .collect();
    if openings.is_empty() {
        return Vec::new();
    }
    openings.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

    let projection = |anchor: DimensionAnchor| -> Option<f64> {
        let p = anchor.resolve(project)?;
        let v = p.minus(wall.start);
        Some(v.x * dir.x + v.y * dir.y)
    };

    let mut pairs = Vec::with_capacity(openings.len() * 2 + 1);
    let mut cursor = DimensionAnchor::WallStart { wall_id };
    let mut cursor_s = 0.0_f64;

    for (opening_id, _) in openings {
        let left = DimensionAnchor::OpeningLeft { opening_id };
        let right = DimensionAnchor::OpeningRight { opening_id };
        let Some(left_s) = projection(left) else {
            continue;
        };
        let Some(right_s) = projection(right) else {
            continue;
        };

        // Pier between previous anchor and opening left jamb.
        if left_s - cursor_s >= MIN_SEGMENT_M {
            pairs.push((cursor, left));
        }

        // Opening clear width itself.
        if right_s - left_s >= MIN_SEGMENT_M {
            pairs.push((left, right));
            cursor = right;
            cursor_s = right_s;
        }
    }

    // Final pier from last opening right jamb to WallEnd.
    let end = DimensionAnchor::WallEnd { wall_id };
    if wall_len - cursor_s >= MIN_SEGMENT_M {
        pairs.push((cursor, end));
    }

    pairs
}

fn has_linear_dimension_between(project: &Project, a: DimensionAnchor, b: DimensionAnchor) -> bool {
    project.dimensions.iter().any(|d| match d.kind {
        DimensionKind::Aligned { a: da, b: db, .. }
        | DimensionKind::External { a: da, b: db, .. } => {
            (da == a && db == b) || (da == b && db == a)
        }
        _ => false,
    })
}

impl Project {
    /// Generates associative opening dimension chains along one wall (when
    /// `wall_id` is `Some`) or every wall that has openings (`None`).
    ///
    /// Each chain runs along `WallStart → Opening1.Left → Opening1.Right → …
    /// → WallEnd` at a smart outward offset (`offset_m` magnitude, clamped to
    /// `>= 0.15 m`) and is idempotent — existing anchor pairs are not duplicated.
    pub fn generate_opening_dimension_chains(
        &mut self,
        wall_id: Option<u64>,
        offset_m: f64,
    ) -> Result<OpeningChainSummary, String> {
        if !offset_m.is_finite() {
            return Err("Deslocamento inválido.".to_owned());
        }
        let target_walls: Vec<u64> = match wall_id {
            Some(id) => {
                if !self.walls.iter().any(|w| w.id == id) {
                    return Err("Parede não encontrada.".to_owned());
                }
                vec![id]
            }
            None => self.walls.iter().map(|w| w.id).collect(),
        };

        let mut summary = OpeningChainSummary::default();
        for wid in target_walls {
            let pairs = wall_opening_chain_pairs(self, wid);
            if pairs.is_empty() {
                continue;
            }
            summary.walls_processed += 1;
            let signed_offset = outward_offset_for_wall(self, wid, offset_m);
            for (a, b) in pairs {
                if has_linear_dimension_between(self, a, b) {
                    continue;
                }
                if self
                    .add_dimension(DimensionKind::Aligned {
                        a,
                        b,
                        offset: signed_offset,
                    })
                    .is_ok()
                {
                    summary.dimensions_added += 1;
                }
            }
        }
        Ok(summary)
    }

    /// One-click site dimensions: generates both internal room dimensions and
    /// opening dimension chains on every wall with openings.
    pub fn generate_site_dimensions(&mut self, offset_m: f64) -> SiteDimensionSummary {
        let internal_added = self.generate_room_internal_dimensions();
        let opening_chains = self
            .generate_opening_dimension_chains(None, offset_m)
            .unwrap_or_default();
        SiteDimensionSummary {
            internal_added,
            opening_chains,
        }
    }
}
