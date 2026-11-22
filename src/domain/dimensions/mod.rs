//! Persistent associative dimensions: internal room dimensions, external
//! dimensions, aligned dimensions (any angle), angular dimensions and
//! automatic opening dimension chains (#53), each editable with custom
//! override text and live-linked to walls/rooms/openings.

pub mod chains;

#[cfg(test)]
mod tests;

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use super::{
    annotations::centimetres,
    drawing::{angle_of, point_at_angle},
    geometry::{EPSILON, Point},
    model::Project,
};

fn default_true() -> bool {
    true
}

/// An endpoint that can follow a wall or opening as the plan changes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "anchor", rename_all = "snake_case")]
pub enum DimensionAnchor {
    Point { x: f64, y: f64 },
    WallStart { wall_id: u64 },
    WallEnd { wall_id: u64 },
    OpeningLeft { opening_id: u64 },
    OpeningRight { opening_id: u64 },
}

impl DimensionAnchor {
    #[allow(dead_code)]
    pub fn from_point(p: Point) -> Self {
        Self::Point { x: p.x, y: p.y }
    }

    pub fn resolve(self, project: &Project) -> Option<Point> {
        match self {
            DimensionAnchor::Point { x, y } => Some(Point::new(x, y)),
            DimensionAnchor::WallStart { wall_id } => project
                .walls
                .iter()
                .find(|w| w.id == wall_id)
                .map(|w| w.start),
            DimensionAnchor::WallEnd { wall_id } => project
                .walls
                .iter()
                .find(|w| w.id == wall_id)
                .map(|w| w.end),
            DimensionAnchor::OpeningLeft { opening_id }
            | DimensionAnchor::OpeningRight { opening_id } => {
                let op = project.openings.iter().find(|o| o.id == opening_id)?;
                let wall = project.walls.iter().find(|w| w.id == op.wall_id)?;
                let dir = wall.end.minus(wall.start).normalized()?;
                let center = wall.start.lerp(wall.end, op.t);
                let half = op.width() * 0.5;
                let sign = if matches!(self, DimensionAnchor::OpeningLeft { .. }) {
                    -1.0
                } else {
                    1.0
                };
                Some(center.translated(dir.scaled(sign * half)))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DimensionKind {
    /// Clear internal span of a room (discounts half wall thickness on each side).
    Internal {
        room_id: u64,
        horizontal: bool,
        offset: f64,
    },
    /// External linear dimension between two anchors.
    External {
        a: DimensionAnchor,
        b: DimensionAnchor,
        offset: f64,
    },
    /// Dimension parallel to the line joining `a` and `b` at any angle.
    Aligned {
        a: DimensionAnchor,
        b: DimensionAnchor,
        offset: f64,
    },
    /// Angle in degrees between `vertex -> ray_a` and `vertex -> ray_b`.
    Angular {
        vertex: DimensionAnchor,
        ray_a: DimensionAnchor,
        ray_b: DimensionAnchor,
        radius: f64,
    },
}

impl DimensionKind {
    pub fn label(&self) -> &'static str {
        match self {
            DimensionKind::Internal { .. } => "Interna",
            DimensionKind::External { .. } => "Externa",
            DimensionKind::Aligned { .. } => "Alinhada",
            DimensionKind::Angular { .. } => "Angular",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DimensionEntity {
    pub id: u64,
    pub kind: DimensionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_text: Option<String>,
    #[serde(default = "default_true")]
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedDimension {
    Linear {
        p1: Point,
        p2: Point,
        dim_p1: Point,
        dim_p2: Point,
        value_m: f64,
        label: String,
    },
    Angular {
        vertex: Point,
        ray_a: Point,
        ray_b: Point,
        start_angle: f64,
        sweep: f64,
        radius: f64,
        angle_deg: f64,
        label: String,
    },
}

impl ResolvedDimension {
    pub fn label(&self) -> &str {
        match self {
            ResolvedDimension::Linear { label, .. } | ResolvedDimension::Angular { label, .. } => {
                label
            }
        }
    }

    pub fn arc_points(&self, steps: usize) -> Vec<Point> {
        let ResolvedDimension::Angular {
            vertex,
            start_angle,
            sweep,
            radius,
            ..
        } = self
        else {
            return Vec::new();
        };
        let n = steps.max(4);
        (0..=n)
            .map(|i| {
                let angle = start_angle + sweep * (i as f64 / n as f64);
                point_at_angle(*vertex, *radius, angle)
            })
            .collect()
    }
}

fn format_degrees(deg: f64) -> String {
    if (deg - deg.round()).abs() < 0.05 {
        format!("{:.0}°", deg.round())
    } else {
        format!("{deg:.1}°").replace('.', ",")
    }
}

impl Project {
    pub fn add_dimension(&mut self, kind: DimensionKind) -> Result<u64, String> {
        let probe = DimensionEntity {
            id: 0,
            kind,
            override_text: None,
            visible: true,
        };
        if self.resolve_dimension(&probe).is_none() {
            return Err(
                "Os pontos de referência da cota são inválidos ou coincidentes.".to_owned(),
            );
        }
        let id = self.dimensions.iter().map(|d| d.id).max().unwrap_or(0) + 1;
        self.dimensions.push(DimensionEntity {
            id,
            kind,
            override_text: None,
            visible: true,
        });
        Ok(id)
    }

    pub fn add_wall_aligned_dimension(&mut self, wall_id: u64, offset: f64) -> Result<u64, String> {
        if !self.walls.iter().any(|w| w.id == wall_id) {
            return Err("Parede não encontrada.".to_owned());
        }
        self.add_dimension(DimensionKind::Aligned {
            a: DimensionAnchor::WallStart { wall_id },
            b: DimensionAnchor::WallEnd { wall_id },
            offset,
        })
    }

    pub fn update_dimension_override(
        &mut self,
        id: u64,
        text: Option<String>,
    ) -> Result<(), String> {
        let dim = self
            .dimensions
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or("Cota não encontrada.")?;
        dim.override_text = text.and_then(|t| {
            let trimmed = t.trim().to_owned();
            (!trimmed.is_empty()).then_some(trimmed)
        });
        Ok(())
    }

    pub fn update_dimension_offset(&mut self, id: u64, offset: f64) -> Result<(), String> {
        if !offset.is_finite() {
            return Err("Deslocamento inválido.".to_owned());
        }
        let dim = self
            .dimensions
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or("Cota não encontrada.")?;
        match &mut dim.kind {
            DimensionKind::Internal { offset: slot, .. }
            | DimensionKind::External { offset: slot, .. }
            | DimensionKind::Aligned { offset: slot, .. } => *slot = offset,
            DimensionKind::Angular { radius, .. } => *radius = offset.abs().max(0.25),
        }
        Ok(())
    }

    pub fn delete_dimension(&mut self, id: u64) -> bool {
        let before = self.dimensions.len();
        self.dimensions.retain(|d| d.id != id);
        self.dimensions.len() < before
    }

    /// Generates associative internal horizontal & vertical dimensions for all
    /// rooms that do not already have them; returns how many were added.
    pub fn generate_room_internal_dimensions(&mut self) -> usize {
        let room_ids: Vec<u64> = self.rooms.iter().map(|r| r.id).collect();
        let mut added = 0;
        for room_id in room_ids {
            for horizontal in [true, false] {
                let exists = self.dimensions.iter().any(|d| {
                    matches!(
                        d.kind,
                        DimensionKind::Internal {
                            room_id: rid,
                            horizontal: h,
                            ..
                        } if rid == room_id && h == horizontal
                    )
                });
                if !exists
                    && self
                        .add_dimension(DimensionKind::Internal {
                            room_id,
                            horizontal,
                            offset: 0.0,
                        })
                        .is_ok()
                {
                    added += 1;
                }
            }
        }
        added
    }

    /// Resolves a persistent associative dimension against current live geometry.
    pub fn resolve_dimension(&self, dim: &DimensionEntity) -> Option<ResolvedDimension> {
        match dim.kind {
            DimensionKind::Internal {
                room_id,
                horizontal,
                offset,
            } => {
                let room = self.rooms.iter().find(|r| r.id == room_id)?;
                let (min, max) = self.room_bounds(room)?;
                let half_t = room
                    .wall_ids
                    .first()
                    .and_then(|wid| self.walls.iter().find(|w| w.id == *wid))
                    .map(|w| w.thickness() * 0.5)
                    .unwrap_or(self.masonry.thickness() * 0.5);
                let (p1, p2) = if horizontal {
                    let y = (min.y + max.y) * 0.5 + offset;
                    (Point::new(min.x + half_t, y), Point::new(max.x - half_t, y))
                } else {
                    let x = (min.x + max.x) * 0.5 + offset;
                    (Point::new(x, min.y + half_t), Point::new(x, max.y - half_t))
                };
                let value_m = p1.distance(p2);
                if value_m <= 0.05 {
                    return None;
                }
                let label = dim
                    .override_text
                    .clone()
                    .unwrap_or_else(|| centimetres(value_m));
                Some(ResolvedDimension::Linear {
                    p1,
                    p2,
                    dim_p1: p1,
                    dim_p2: p2,
                    value_m,
                    label,
                })
            }
            DimensionKind::External { a, b, offset } | DimensionKind::Aligned { a, b, offset } => {
                let p1 = a.resolve(self)?;
                let p2 = b.resolve(self)?;
                let dir = p2.minus(p1).normalized()?;
                let value_m = p1.distance(p2);
                if value_m <= EPSILON {
                    return None;
                }
                let normal = Point::new(-dir.y, dir.x).scaled(offset);
                let dim_p1 = p1.translated(normal);
                let dim_p2 = p2.translated(normal);
                let label = dim
                    .override_text
                    .clone()
                    .unwrap_or_else(|| centimetres(value_m));
                Some(ResolvedDimension::Linear {
                    p1,
                    p2,
                    dim_p1,
                    dim_p2,
                    value_m,
                    label,
                })
            }
            DimensionKind::Angular {
                vertex,
                ray_a,
                ray_b,
                radius,
            } => {
                let v = vertex.resolve(self)?;
                let a = ray_a.resolve(self)?;
                let b = ray_b.resolve(self)?;
                if v.distance(a) <= EPSILON || v.distance(b) <= EPSILON || radius <= 0.05 {
                    return None;
                }
                let start_angle = angle_of(v, a);
                let end_angle = angle_of(v, b);
                let mut sweep = (end_angle - start_angle).rem_euclid(TAU);
                if sweep > std::f64::consts::PI {
                    sweep -= TAU;
                }
                let angle_deg = sweep.abs().to_degrees();
                if angle_deg <= 0.1 {
                    return None;
                }
                let label = dim
                    .override_text
                    .clone()
                    .unwrap_or_else(|| format_degrees(angle_deg));
                Some(ResolvedDimension::Angular {
                    vertex: v,
                    ray_a: a,
                    ray_b: b,
                    start_angle,
                    sweep,
                    radius,
                    angle_deg,
                    label,
                })
            }
        }
    }
}
