//! Geometric context includes technical entities that are not CAD selections.
use super::*;
use crate::domain::{dimensions::ResolvedDimension, geometry::line_intersection};

pub struct MarkupResource {
    pub id: u64,
    pub kind: &'static str,
}

pub(super) struct Region {
    path: Vec<Point>,
    closed: bool,
    low: Point,
    high: Point,
}
impl Region {
    pub fn new(mark: &Markup) -> Self {
        let (low, high) = mark.bounds();
        Self {
            path: mark.shape().outline(),
            closed: matches!(mark.kind, MarkupKind::Rectangle | MarkupKind::Circle),
            low,
            high,
        }
    }
    pub fn intersects(&self, outline: &[Point]) -> bool {
        let low = self.low;
        let high = self.high;
        if outline.is_empty()
            || outline.iter().all(|p| p.x < low.x - 0.005)
            || outline.iter().all(|p| p.x > high.x + 0.005)
            || outline.iter().all(|p| p.y < low.y - 0.005)
            || outline.iter().all(|p| p.y > high.y + 0.005)
        {
            return false;
        }
        (self.closed
            && outline
                .iter()
                .any(|p| polygon_contains_point(&self.path, *p)))
            || (outline.len() > 2
                && outline.first() == outline.last()
                && self
                    .path
                    .iter()
                    .any(|p| polygon_contains_point(outline, *p)))
            || outline.iter().any(|p| {
                self.path
                    .windows(2)
                    .any(|s| distance_to_segment(*p, s[0], s[1]) <= 0.005)
            })
            || self.path.iter().any(|p| {
                outline
                    .windows(2)
                    .any(|s| distance_to_segment(*p, s[0], s[1]) <= 0.005)
            })
            || self.path.windows(2).any(|a| {
                outline.windows(2).any(|b| {
                    line_intersection(a[0], a[1], b[0], b[1]).is_some_and(|(_, t, u)| {
                        (-1e-8..=1.0 + 1e-8).contains(&t) && (-1e-8..=1.0 + 1e-8).contains(&u)
                    })
                })
            })
    }
}
fn closed(mut points: Vec<Point>) -> Vec<Point> {
    if let Some(first) = points.first().copied() {
        points.push(first);
    }
    points
}
fn band(a: Point, b: Point, width: f64) -> Vec<Point> {
    let Some(v) = b.minus(a).normalized() else {
        return vec![a];
    };
    let n = Point::new(-v.y, v.x).scaled(width * 0.5);
    closed(vec![
        a.translated(n),
        b.translated(n),
        b.minus(n),
        a.minus(n),
    ])
}

impl Markup {
    pub fn related_resources(&self, project: &Project) -> Vec<super::MarkupResource> {
        if self.validate().is_err() {
            return vec![];
        }
        let region = Region::new(self);
        let mut result = Vec::new();
        let mut add = |kind, id, outline: Vec<Point>| {
            if region.intersects(&outline) {
                result.push(MarkupResource { id, kind });
            }
        };
        for beam in &project.structure.beams {
            if let Some((a, b)) = project.beam_endpoints(beam) {
                add("viga", beam.id, band(a, b, beam.width_m));
            }
        }
        for f in &project.structure.wall_foundations {
            if let Some(w) = project.walls.iter().find(|w| w.id == f.wall_id) {
                add(
                    "fundacao_parede",
                    f.id,
                    band(w.start, w.end, f.width_m.unwrap_or(w.thickness())),
                );
            }
        }
        for f in &project.structure.balance_beams {
            if let (Some(a), Some(b)) = (
                project.column_by_id(f.column),
                project.column_by_id(f.anchor),
            ) {
                add("viga_alavanca", f.id, band(a.center, b.center, f.width_m));
            }
        }
        for slab in &project.structure.slabs {
            if let Some(room) = project.rooms.iter().find(|r| r.id == slab.room_id)
                && let Some(poly) = project.room_polygon(room)
            {
                add("laje", slab.id, closed(poly));
            }
        }
        for roof in &project.structure.roofs {
            let d = Point::new(roof.overhang_m, roof.overhang_m);
            add(
                "telhado",
                roof.id,
                crate::domain::drawing::Shape::rectangle(roof.min.minus(d), roof.max.translated(d))
                    .outline(),
            );
        }
        for note in &project.notes {
            let mut path = vec![note.position];
            if let crate::domain::notes::NoteKind::Callout { target } = note.kind {
                path.push(target);
            }
            add("anotacao", note.id, path);
        }
        for c in &project.circulations {
            let geometry = c.plan_geometry();
            if geometry
                .polygons
                .into_iter()
                .any(|p| region.intersects(&closed(p)))
                || geometry
                    .lines
                    .iter()
                    .any(|(a, b)| region.intersects(&[*a, *b]))
            {
                result.push(MarkupResource {
                    id: c.id,
                    kind: "circulacao",
                });
            }
        }
        for d in &project.dimensions {
            if let Some(resolved) = project.resolve_dimension(d) {
                let path = match resolved {
                    ResolvedDimension::Linear { dim_p1, dim_p2, .. } => vec![dim_p1, dim_p2],
                    ResolvedDimension::Angular {
                        vertex,
                        ray_a,
                        ray_b,
                        ..
                    } => vec![ray_a, vertex, ray_b],
                };
                if region.intersects(&path) {
                    result.push(MarkupResource {
                        id: d.id,
                        kind: "cota",
                    });
                }
            }
        }
        for conduit in &project.conduits {
            if let (Some(a), Some(b)) = (
                project.fixtures.iter().find(|f| f.id == conduit.from),
                project.fixtures.iter().find(|f| f.id == conduit.to),
            ) && region.intersects(&[a.center, b.center])
            {
                result.push(MarkupResource {
                    id: conduit.id,
                    kind: "eletroduto",
                });
            }
        }
        for lot in &project.lots {
            if region.intersects(&closed(lot.vertices.clone())) {
                result.push(MarkupResource {
                    id: lot.id,
                    kind: "lote",
                });
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_mark_between_columns_identifies_the_beam_and_technical_notes() {
        let mut p = Project::default();
        let a = p
            .add_fixture(
                crate::domain::fixtures::FixtureKind::Column,
                Point::ORIGIN,
                0.2,
                0.2,
                0,
            )
            .unwrap();
        let b = p
            .add_fixture(
                crate::domain::fixtures::FixtureKind::Column,
                Point::new(4.0, 0.0),
                0.2,
                0.2,
                0,
            )
            .unwrap();
        let beam = p.add_beam(a, b, 0.2, 0.4).unwrap();
        let note = p
            .add_note(
                Point::new(2.0, 0.0),
                "Revisar",
                crate::domain::notes::NoteKind::Text,
            )
            .unwrap();
        let m = Markup {
            id: 0,
            kind: MarkupKind::Rectangle,
            points: vec![Point::new(1.0, -0.3), Point::new(3.0, 0.3)],
            color: [220, 65, 65],
            comment: String::new(),
            resolved: false,
            linked_elements: vec![],
        };
        assert!(!m.related_elements(&p).contains(&Element::Fixture(a)));
        let refs = m.related_resources(&p);
        assert!(refs.iter().any(|r| r.kind == "viga" && r.id == beam));
        assert!(refs.iter().any(|r| r.kind == "anotacao" && r.id == note));
    }
}
