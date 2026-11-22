//! Formwork and foundation plan symbols and overlay marks.

use std::collections::{HashMap, HashSet};

use crate::domain::{
    conduits::PlanMark,
    fixtures::{Fixture, FixtureKind, SymbolShape, Weight},
    geometry::Point,
    model::Project,
};

use super::{Beam, BeamSupport, decimal, section_cm};

/// The plan symbol of a column (cut, so hatched) or a footing (below the
/// floor, so dashed, with the edges of its sloped top).
pub(in crate::domain) fn symbol(kind: FixtureKind, width: f64, depth: f64) -> Vec<SymbolShape> {
    let (hw, hd) = (width / 2.0, depth / 2.0);
    let corners = vec![
        Point::new(-hw, -hd),
        Point::new(hw, -hd),
        Point::new(hw, hd),
        Point::new(-hw, hd),
    ];
    match kind {
        FixtureKind::Column => {
            let mut shapes = vec![SymbolShape::Polyline {
                points: corners,
                closed: true,
                dashed: false,
                weight: Weight::Outline,
            }];
            // 45° hatching clipped to the section.
            let step = 0.035;
            let mut c = -hw - hd + step;
            while c < hw + hd {
                // Points with x - y = c inside the box.
                let x0 = (-hw).max(c - hd);
                let x1 = hw.min(c + hd);
                if x1 > x0 + 1e-6 {
                    shapes.push(SymbolShape::Polyline {
                        points: vec![Point::new(x0, x0 - c), Point::new(x1, x1 - c)],
                        closed: false,
                        dashed: false,
                        weight: Weight::Detail,
                    });
                }
                c += step;
            }
            shapes
        }
        FixtureKind::Footing => {
            let inner = 0.12_f64.min(hw * 0.5).min(hd * 0.5);
            let mut shapes = vec![SymbolShape::Polyline {
                points: corners.clone(),
                closed: true,
                dashed: true,
                weight: Weight::Outline,
            }];
            for corner in corners {
                let toward = Point::new(corner.x.signum() * inner, corner.y.signum() * inner);
                shapes.push(SymbolShape::Polyline {
                    points: vec![corner, toward],
                    closed: false,
                    dashed: true,
                    weight: Weight::Detail,
                });
            }
            shapes
        }
        // A round post: its section and a growth ring; a square one, its
        // section crossed.
        FixtureKind::TimberPost if (width - depth).abs() < 1e-6 => vec![
            SymbolShape::Circle {
                center: Point::new(0.0, 0.0),
                radius: hw,
                weight: Weight::Outline,
            },
            SymbolShape::Circle {
                center: Point::new(0.0, 0.0),
                radius: hw * 0.55,
                weight: Weight::Detail,
            },
        ],
        FixtureKind::TimberPost => vec![
            SymbolShape::Polyline {
                points: corners,
                closed: true,
                dashed: false,
                weight: Weight::Outline,
            },
            SymbolShape::Polyline {
                points: vec![Point::new(-hw, -hd), Point::new(hw, hd)],
                closed: false,
                dashed: false,
                weight: Weight::Detail,
            },
        ],
        FixtureKind::WaterTank => vec![
            SymbolShape::Circle {
                center: Point::new(0.0, 0.0),
                radius: hw.min(hd),
                weight: Weight::Outline,
            },
            SymbolShape::Circle {
                center: Point::new(0.0, 0.0),
                radius: hw.min(hd) * 0.72,
                weight: Weight::Detail,
            },
            SymbolShape::Polyline {
                points: vec![Point::new(-hw * 0.65, 0.0), Point::new(hw * 0.65, 0.0)],
                closed: false,
                dashed: false,
                weight: Weight::Detail,
            },
        ],
        FixtureKind::HeavyEquipment => vec![
            SymbolShape::Polyline {
                points: corners,
                closed: true,
                dashed: false,
                weight: Weight::Outline,
            },
            SymbolShape::Polyline {
                points: vec![Point::new(-hw, -hd), Point::new(hw, hd)],
                closed: false,
                dashed: false,
                weight: Weight::Detail,
            },
            SymbolShape::Polyline {
                points: vec![Point::new(hw, -hd), Point::new(-hw, hd)],
                closed: false,
                dashed: false,
                weight: Weight::Detail,
            },
        ],
        _ => Vec::new(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructureMarksScope {
    /// Combined superstructure + foundations.
    All,
    /// Superstructure (`Planta de Forma` — Estrutura tab).
    Superstructure,
    /// Infrastructure (`Planta de Fundações` — Fundações tab).
    Foundations,
}

impl Project {
    /// Beams and labels as drawn on the formwork plan: the two faces of each
    /// beam between the column faces, and the P, V, L and S labels.
    pub fn structure_marks(&self) -> Vec<PlanMark> {
        self.structure_marks_for(StructureMarksScope::All)
    }

    /// Marks for the **Estrutura** (`Planta de Forma`) discipline view:
    /// columns, upper beams, slabs, concentrated loads, roofs, timber posts and
    /// timber eave beams (no footings, balance beams, baldrames or strip footings).
    pub fn superstructure_marks(&self) -> Vec<PlanMark> {
        self.structure_marks_for(StructureMarksScope::Superstructure)
    }

    /// Marks for the **Fundações** (`Planta de Fundações`) discipline view:
    /// columns (starter bar locations), isolated footings, balance beams,
    /// baldrames, strip footings and timber post foundation bases (no upper
    /// beams, slabs, roofs, timber eave beams or concentrated loads).
    pub fn foundation_marks(&self) -> Vec<PlanMark> {
        self.structure_marks_for(StructureMarksScope::Foundations)
    }

    pub fn structure_marks_for(&self, scope: StructureMarksScope) -> Vec<PlanMark> {
        let include_super = matches!(
            scope,
            StructureMarksScope::All | StructureMarksScope::Superstructure
        );
        let include_foundations = matches!(
            scope,
            StructureMarksScope::All | StructureMarksScope::Foundations
        );
        let labels = self.structure_labels();
        let mut marks = Vec::new();
        if include_super {
            let extent = |column: &Fixture, dir: Point| {
                let (s, c) = column.rotation_rad().sin_cos();
                let u = Point::new(c, s);
                let v = Point::new(-s, c);
                dir.dot(u).abs() * column.width / 2.0 + dir.dot(v).abs() * column.depth / 2.0
            };
            let support_trim = |support: BeamSupport, dir: Point| match support {
                BeamSupport::Column { id } => self.column(id).map_or(0.0, |c| extent(c, dir)),
                BeamSupport::Beam { beam_id, .. } => {
                    self.beam_by_id(beam_id).map_or(0.0, |b| b.width_m / 2.0)
                }
            };
            let mut labelled: HashSet<String> = HashSet::new();
            let beam_map: HashMap<u64, Beam> = self.valid_beams().map(|b| (b.id, *b)).collect();
            let lines = self.beam_lines();
            for line in &lines {
                // The longest span carries the line's label.
                let mut longest: Option<(f64, Point, Point, Beam)> = None;
                for id in &line.spans {
                    let Some(&beam) = beam_map.get(id) else {
                        continue;
                    };
                    let Some((pa, pb)) = self.beam_endpoints(&beam) else {
                        continue;
                    };
                    let Some(dir) = pb.minus(pa).normalized() else {
                        continue;
                    };
                    let normal = Point::new(-dir.y, dir.x);
                    let start = pa.translated(dir.scaled(support_trim(beam.start_support(), dir)));
                    let end = pb.translated(dir.scaled(-support_trim(beam.end_support(), dir)));
                    for side in [-1.0, 1.0] {
                        let off = normal.scaled(side * beam.width_m / 2.0);
                        marks.push(PlanMark::Line {
                            points: vec![start.translated(off), end.translated(off)],
                            dashed: false,
                            heavy: true,
                        });
                    }
                    let length = pa.distance(pb);
                    if longest.is_none_or(|(l, _, _, _)| length > l) {
                        longest = Some((length, pa, pb, beam));
                    }
                }
                if let Some((_, a, b, beam)) = longest
                    && labelled.insert(line.label.clone())
                {
                    let dir = b.minus(a).normalized().unwrap_or(Point::new(1.0, 0.0));
                    let mut normal = Point::new(-dir.y, dir.x);
                    if normal.y > 0.0 || (normal.y.abs() < 1e-6 && normal.x > 0.0) {
                        normal = normal.scaled(-1.0);
                    }
                    let text = format!(
                        "{} ({})",
                        line.label,
                        section_cm(beam.width_m, beam.height_m)
                    );
                    let (w, _) = crate::domain::electrical::tag_size(&text);
                    let reach = beam.width_m / 2.0 + 0.16 + normal.x.abs() * w / 2.0;
                    marks.push(PlanMark::Text {
                        anchor: a.midpoint(b).translated(normal.scaled(reach)),
                        text,
                    });
                }
            }
        }
        // Columns appear in both Estrutura (superstructure) and Fundações (starter bar location).
        for column in self.structural_columns() {
            if let Some(text) = labels.get(&column.id) {
                let text = format!("{text} {}", section_cm(column.width, column.depth));
                let (w, _) = crate::domain::electrical::tag_size(&text);
                let (hw, hd) = match column.quarter_turns % 2 {
                    0 => (column.width / 2.0, column.depth / 2.0),
                    _ => (column.depth / 2.0, column.width / 2.0),
                };
                marks.push(PlanMark::Text {
                    anchor: column
                        .center
                        .translated(Point::new(hw + w / 2.0 + 0.05, -hd - 0.12)),
                    text,
                });
            }
        }
        if include_foundations {
            for footing in self
                .fixtures
                .iter()
                .filter(|f| f.kind == FixtureKind::Footing)
            {
                if let Some(text) = labels.get(&footing.id) {
                    let text = format!("{text} {}", section_cm(footing.width, footing.depth));
                    let (w, _) = crate::domain::electrical::tag_size(&text);
                    let (hw, hd) = match footing.quarter_turns % 2 {
                        0 => (footing.width / 2.0, footing.depth / 2.0),
                        _ => (footing.depth / 2.0, footing.width / 2.0),
                    };
                    marks.push(PlanMark::Text {
                        anchor: footing
                            .center
                            .translated(Point::new(hw - w / 2.0, hd + 0.14)),
                        text,
                    });
                }
            }
        }
        if include_super {
            for slab in &self.structure.slabs {
                let Some(room) = self.rooms.iter().find(|room| room.id == slab.room_id) else {
                    continue;
                };
                let Some((anchor, _, _)) = self.room_label_anchor(room) else {
                    continue;
                };
                if let Some(text) = labels.get(&slab.id) {
                    marks.push(PlanMark::Text {
                        anchor: anchor.translated(Point::new(0.0, 0.45)),
                        text: format!(
                            "{text} h={:.0} ({})",
                            slab.thickness_m * 100.0,
                            slab.kind.label().to_lowercase()
                        ),
                    });
                }
            }
            for load in self.fixtures.iter().filter(|fixture| {
                matches!(
                    fixture.kind,
                    FixtureKind::WaterTank | FixtureKind::HeavyEquipment
                )
            }) {
                let Some(text) = labels.get(&load.id) else {
                    continue;
                };
                let value = load.structural_load_kn.unwrap_or(0.0);
                marks.push(PlanMark::Text {
                    anchor: load
                        .center
                        .translated(Point::new(0.0, load.depth / 2.0 + 0.16)),
                    text: format!("{text} {} kN", decimal(value, 1)),
                });
            }
        }
        let report = self.cached_structure_report();
        if include_foundations {
            // Balance beams and wall foundations (baldrames / strip footings), below ground.
            marks.extend(self.balance_beam_marks(&report.balance_beams));
            marks.extend(self.wall_foundation_marks(&report.wall_foundations));
        }
        if include_super {
            marks.extend(self.roof_marks(&report.roofs));
            marks.extend(crate::domain::masonry_structure::masonry_structure_marks(
                &report.masonry,
            ));
        }
        marks.extend(self.post_marks_filtered(
            &crate::domain::timber_posts::PostEffects {
                posts: report.timber_posts.clone(),
                eave_beams: report.eave_beams.clone(),
            },
            include_foundations,
            include_super,
        ));
        crate::domain::label_layout::resolve_plan_mark_labels(&mut marks, &[]);
        marks
    }
}
