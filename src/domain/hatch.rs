//! Graphic hatch patterns by material, with exact polygon clipping for both
//! interactive canvas rendering and vector SVG/PDF export.

use serde::{Deserialize, Serialize};

use super::{
    geometry::{EPSILON, Point},
    masonry::Masonry,
    model::Project,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HatchPattern {
    #[default]
    None,
    /// Orthogonal tile grid (30 × 30 cm — bathrooms, kitchens, wet areas).
    CeramicGrid,
    /// Large stone/porcelain grid (60 × 60 cm).
    StoneGrid,
    /// Parallel planks (wood / laminate flooring).
    WoodPlank,
    /// 45° parallel lines (standard masonry / fill).
    Diagonal45,
    /// 45° + 135° crossed lines (concrete / structural masonry).
    CrossHatch,
    /// Running-bond brick pattern.
    Brick,
    /// Solid translucent tint.
    Solid,
}

impl HatchPattern {
    pub const ALL: [HatchPattern; 8] = [
        HatchPattern::None,
        HatchPattern::CeramicGrid,
        HatchPattern::StoneGrid,
        HatchPattern::WoodPlank,
        HatchPattern::Diagonal45,
        HatchPattern::CrossHatch,
        HatchPattern::Brick,
        HatchPattern::Solid,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HatchPattern::None => "Sem hachura",
            HatchPattern::CeramicGrid => "Piso cerâmico (30×30)",
            HatchPattern::StoneGrid => "Porcelanato / Pedra (60×60)",
            HatchPattern::WoodPlank => "Madeira / Laminado",
            HatchPattern::Diagonal45 => "Alvenaria (45°)",
            HatchPattern::CrossHatch => "Concreto (cruzada)",
            HatchPattern::Brick => "Tijolo aparente",
            HatchPattern::Solid => "Preenchimento sólido",
        }
    }

    /// Suggested technical hatch pattern for a given masonry material.
    pub fn for_masonry(masonry: &Masonry) -> Self {
        let lower = masonry.name.to_lowercase();
        if lower.contains("concreto") {
            HatchPattern::CrossHatch
        } else if lower.contains("tijolo") {
            HatchPattern::Brick
        } else {
            HatchPattern::Diagonal45
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HatchStyle {
    pub pattern: HatchPattern,
    /// Spacing between parallel hatch lines in metres.
    pub spacing_m: f64,
    /// Primary hatch angle in degrees.
    pub angle_deg: f64,
    pub color: [u8; 3],
}

impl Default for HatchStyle {
    fn default() -> Self {
        Self::for_pattern(HatchPattern::None)
    }
}

impl HatchStyle {
    pub fn for_pattern(pattern: HatchPattern) -> Self {
        match pattern {
            HatchPattern::None => Self {
                pattern,
                spacing_m: 0.30,
                angle_deg: 0.0,
                color: [165, 186, 208],
            },
            HatchPattern::CeramicGrid => Self {
                pattern,
                spacing_m: 0.30,
                angle_deg: 0.0,
                color: [152, 178, 204],
            },
            HatchPattern::StoneGrid => Self {
                pattern,
                spacing_m: 0.60,
                angle_deg: 0.0,
                color: [148, 166, 184],
            },
            HatchPattern::WoodPlank => Self {
                pattern,
                spacing_m: 0.18,
                angle_deg: 0.0,
                color: [184, 152, 118],
            },
            HatchPattern::Diagonal45 => Self {
                pattern,
                spacing_m: 0.25,
                angle_deg: 45.0,
                color: [140, 158, 176],
            },
            HatchPattern::CrossHatch => Self {
                pattern,
                spacing_m: 0.25,
                angle_deg: 45.0,
                color: [130, 148, 166],
            },
            HatchPattern::Brick => Self {
                pattern,
                spacing_m: 0.20,
                angle_deg: 0.0,
                color: [182, 130, 108],
            },
            HatchPattern::Solid => Self {
                pattern,
                spacing_m: 0.30,
                angle_deg: 0.0,
                color: [222, 234, 246],
            },
        }
    }

    pub fn hex_color(&self) -> String {
        let [r, g, b] = self.color;
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

fn rotate(pt: Point, cos_a: f64, sin_a: f64) -> Point {
    Point::new(pt.x * cos_a - pt.y * sin_a, pt.x * sin_a + pt.y * cos_a)
}

/// Clips parallel scanlines separated by `spacing` at `angle_deg` to `poly`.
fn parallel_segments(poly: &[Point], spacing: f64, angle_deg: f64) -> Vec<(Point, Point)> {
    if poly.len() < 3 || !spacing.is_finite() || spacing < 0.02 {
        return Vec::new();
    }
    let rad = angle_deg.to_radians();
    let (cos_m, sin_m) = ((-rad).cos(), (-rad).sin());
    let (cos_p, sin_p) = (rad.cos(), rad.sin());

    let rotated: Vec<Point> = poly.iter().map(|&p| rotate(p, cos_m, sin_m)).collect();
    let min_y = rotated.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_y = rotated
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);
    if !min_y.is_finite() || !max_y.is_finite() || max_y - min_y <= EPSILON {
        return Vec::new();
    }

    let first_k = (min_y / spacing).floor() as i64 + 1;
    let last_k = (max_y / spacing).ceil() as i64 - 1;
    let max_lines = 400_i64;
    let step_k = ((last_k - first_k).max(0) / max_lines).max(1);

    let mut out = Vec::new();
    let n = rotated.len();
    let mut k = first_k;
    while k <= last_k {
        let y = k as f64 * spacing;
        let mut xs = Vec::new();
        for i in 0..n {
            let a = rotated[i];
            let b = rotated[(i + 1) % n];
            if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
                let t = (y - a.y) / (b.y - a.y);
                xs.push(a.x + t * (b.x - a.x));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks_exact(2) {
            if pair[1] - pair[0] > 1e-4 {
                let p0 = rotate(Point::new(pair[0], y), cos_p, sin_p);
                let p1 = rotate(Point::new(pair[1], y), cos_p, sin_p);
                out.push((p0, p1));
            }
        }
        k += step_k;
    }
    out
}

/// Generates all world-space hatch line segments clipped strictly inside `poly`.
pub fn hatch_segments_in_polygon(poly: &[Point], style: &HatchStyle) -> Vec<(Point, Point)> {
    let spacing = style.spacing_m.clamp(0.05, 5.0);
    match style.pattern {
        HatchPattern::None | HatchPattern::Solid => Vec::new(),
        HatchPattern::WoodPlank | HatchPattern::Diagonal45 => {
            parallel_segments(poly, spacing, style.angle_deg)
        }
        HatchPattern::CeramicGrid | HatchPattern::StoneGrid | HatchPattern::CrossHatch => {
            let mut segs = parallel_segments(poly, spacing, style.angle_deg);
            segs.extend(parallel_segments(poly, spacing, style.angle_deg + 90.0));
            segs
        }
        HatchPattern::Brick => {
            // Horizontal courses + staggered short vertical brick joints.
            let mut segs = parallel_segments(poly, spacing, style.angle_deg);
            let rad = style.angle_deg.to_radians();
            let (cos_m, sin_m) = ((-rad).cos(), (-rad).sin());
            let (cos_p, sin_p) = (rad.cos(), rad.sin());
            let rotated: Vec<Point> = poly.iter().map(|&p| rotate(p, cos_m, sin_m)).collect();
            let min_y = rotated.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
            let max_y = rotated
                .iter()
                .map(|p| p.y)
                .fold(f64::NEG_INFINITY, f64::max);
            let first_row = (min_y / spacing).floor() as i64;
            let last_row = (max_y / spacing).ceil() as i64;
            let brick_len = spacing * 2.0;
            let n = rotated.len();
            if last_row - first_row <= 200 {
                for row in first_row..last_row {
                    let y0 = row as f64 * spacing;
                    let y1 = (row + 1) as f64 * spacing;
                    let ym = (y0 + y1) * 0.5;
                    let mut xs = Vec::new();
                    for i in 0..n {
                        let a = rotated[i];
                        let b = rotated[(i + 1) % n];
                        if (a.y <= ym && b.y > ym) || (b.y <= ym && a.y > ym) {
                            let t = (ym - a.y) / (b.y - a.y);
                            xs.push(a.x + t * (b.x - a.x));
                        }
                    }
                    xs.sort_by(f64::total_cmp);
                    let stagger = if row.rem_euclid(2) == 0 {
                        0.0
                    } else {
                        brick_len * 0.5
                    };
                    for pair in xs.chunks_exact(2) {
                        let start_col = ((pair[0] - stagger) / brick_len).ceil() as i64;
                        let end_col = ((pair[1] - stagger) / brick_len).floor() as i64;
                        for col in start_col..=end_col {
                            let x = col as f64 * brick_len + stagger;
                            if x > pair[0] + 1e-3 && x < pair[1] - 1e-3 {
                                let p0 = rotate(Point::new(x, y0), cos_p, sin_p);
                                let p1 = rotate(Point::new(x, y1), cos_p, sin_p);
                                segs.push((p0, p1));
                            }
                        }
                    }
                }
            }
            segs
        }
    }
}

impl Project {
    pub fn set_room_hatch(
        &mut self,
        room_id: u64,
        hatch: Option<HatchStyle>,
    ) -> Result<(), String> {
        let room = self
            .rooms
            .iter_mut()
            .find(|r| r.id == room_id)
            .ok_or("Cômodo não encontrado.")?;
        room.hatch = hatch.filter(|h| h.pattern != HatchPattern::None);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::polygon_contains_point;

    #[test]
    fn hatch_segments_clip_cleanly_inside_rectangular_and_l_polygons() {
        let rect = [
            Point::new(0.0, 0.0),
            Point::new(3.0, 0.0),
            Point::new(3.0, 2.0),
            Point::new(0.0, 2.0),
        ];
        let grid =
            hatch_segments_in_polygon(&rect, &HatchStyle::for_pattern(HatchPattern::CeramicGrid));
        assert!(!grid.is_empty());
        for (a, b) in &grid {
            let mid = a.midpoint(*b);
            assert!(polygon_contains_point(&rect, mid));
        }

        // L-shaped room: diagonal 45° and brick hatches must stay strictly inside.
        let l_room = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 2.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 4.0),
            Point::new(0.0, 4.0),
        ];
        for pattern in [
            HatchPattern::Diagonal45,
            HatchPattern::CrossHatch,
            HatchPattern::Brick,
            HatchPattern::WoodPlank,
        ] {
            let segs = hatch_segments_in_polygon(&l_room, &HatchStyle::for_pattern(pattern));
            assert!(!segs.is_empty(), "expected segments for {pattern:?}");
            for (a, b) in &segs {
                assert!(polygon_contains_point(&l_room, a.midpoint(*b)));
            }
        }
    }

    #[test]
    fn material_mapping_and_room_hatch_assignment_work() {
        let presets = Masonry::presets();
        for preset in &presets {
            assert_ne!(HatchPattern::for_masonry(preset), HatchPattern::None);
        }
        let mut project = Project::default();
        let room_id = project
            .add_room(Point::new(0.0, 0.0), Point::new(3.0, 3.0), "Banheiro")
            .unwrap();
        project
            .set_room_hatch(
                room_id,
                Some(HatchStyle::for_pattern(HatchPattern::CeramicGrid)),
            )
            .unwrap();
        assert_eq!(
            project.rooms[0].hatch.map(|h| h.pattern),
            Some(HatchPattern::CeramicGrid)
        );
        project
            .set_room_hatch(room_id, Some(HatchStyle::for_pattern(HatchPattern::None)))
            .unwrap();
        assert_eq!(project.rooms[0].hatch, None);
    }
}
