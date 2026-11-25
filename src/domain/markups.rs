//! Coloured review marks, separate from construction geometry and quantities.
//! Geometry stays in metres so people and connected clients refer to the same area.

use serde::{Deserialize, Serialize};

use super::{
    drawing::Shape,
    element::Element,
    geometry::{Point, distance_to_segment, polygon_contains_point},
    model::Project,
};

mod related;
pub use related::MarkupResource;

pub const MAX_POINTS: usize = 2048;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkupKind {
    #[default]
    Rectangle,
    Circle,
    Arrow,
    Freehand,
}

impl MarkupKind {
    pub const ALL: [Self; 4] = [Self::Rectangle, Self::Circle, Self::Arrow, Self::Freehand];
    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangle => "Retângulo",
            Self::Circle => "Círculo",
            Self::Arrow => "Seta",
            Self::Freehand => "Traço livre",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Rectangle => "retangulo",
            Self::Circle => "circulo",
            Self::Arrow => "seta",
            Self::Freehand => "traco_livre",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Markup {
    pub id: u64,
    pub kind: MarkupKind,
    /// Opposite corners, centre/radius point, arrow ends, or the freehand path.
    pub points: Vec<Point>,
    pub color: [u8; 3],
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub resolved: bool,
    /// Explicit references stay stable when the underlying elements move.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_elements: Vec<Element>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MarkupSettings {
    pub visible: bool,
    pub show_resolved: bool,
    pub export: bool,
}

impl Default for MarkupSettings {
    fn default() -> Self {
        Self {
            visible: true,
            show_resolved: false,
            export: false,
        }
    }
}

impl Markup {
    pub fn label(&self) -> String {
        format!("M{}", self.id)
    }
    pub fn validate(&self) -> Result<(), String> {
        let count = self.points.len();
        if !(2..=MAX_POINTS).contains(&count) || (self.kind != MarkupKind::Freehand && count != 2) {
            return Err(
                "Use dois pontos para retângulo, círculo ou seta; de 2 a 2048 para traço livre."
                    .into(),
            );
        }
        if self
            .points
            .iter()
            .any(|p| !p.is_finite() || p.x.abs() > 10_000.0 || p.y.abs() > 10_000.0)
        {
            return Err("As coordenadas da marcação devem estar entre -10000 e 10000 m.".into());
        }
        if self.comment.chars().count() > 4000 || self.linked_elements.len() > 500 {
            return Err("Use até 4000 caracteres no comentário e até 500 vínculos.".into());
        }
        if self
            .comment
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err("O comentário contém caracteres de controle inválidos.".into());
        }
        if self.kind == MarkupKind::Rectangle
            && ((self.points[0].x - self.points[1].x).abs() < 0.001
                || (self.points[0].y - self.points[1].y).abs() < 0.001)
        {
            return Err("O retângulo precisa de largura e altura maiores que 1 mm.".into());
        }
        self.shape().validate()
    }
    pub fn shape(&self) -> Shape {
        let a = self.points.first().copied().unwrap_or(Point::ORIGIN);
        let b = self.points.get(1).copied().unwrap_or(a);
        match self.kind {
            MarkupKind::Rectangle => Shape::rectangle(a, b),
            MarkupKind::Circle => Shape::Circle {
                center: a,
                radius: a.distance(b),
            },
            MarkupKind::Arrow => Shape::Line { start: a, end: b },
            MarkupKind::Freehand => Shape::Polyline {
                points: self.points.clone(),
                closed: false,
            },
        }
    }
    pub fn bounds(&self) -> (Point, Point) {
        self.shape().bounds()
    }
    pub fn hit(&self, point: Point, tolerance: f64) -> bool {
        if self.validate().is_err() {
            return false;
        }
        let path = self.shape().outline();
        path.windows(2)
            .any(|s| distance_to_segment(point, s[0], s[1]) <= tolerance)
            || (matches!(self.kind, MarkupKind::Rectangle | MarkupKind::Circle)
                && polygon_contains_point(&path, point))
    }
    /// Intersect real outlines rather than bounding boxes, avoiding unrelated
    /// objects in the corners of a circle or beside a diagonal arrow.
    pub fn related_elements(&self, project: &Project) -> Vec<Element> {
        if self.validate().is_err() {
            return Vec::new();
        }
        let region = related::Region::new(self);
        project
            .elements()
            .into_iter()
            .filter(|e| {
                self.linked_elements.contains(e)
                    || project
                        .element_outline(*e)
                        .is_some_and(|outline| region.intersects(&outline))
            })
            .collect()
    }
}

impl Project {
    pub fn add_markup(&mut self, mut markup: Markup) -> Result<u64, String> {
        markup.validate()?;
        if self.markups.len() >= 5000 {
            return Err("Limite de 5000 marcações por pavimento atingido.".into());
        }
        if markup
            .linked_elements
            .iter()
            .any(|e| !self.element_exists(*e))
        {
            return Err("Um dos elementos vinculados não existe neste pavimento.".into());
        }
        // A mark always names a real floor, even in legacy single-floor plans.
        self.ensure_default_floor();
        markup.id = self.id();
        let id = markup.id;
        self.markups.push(markup);
        Ok(id)
    }
    pub fn update_markup(&mut self, markup: Markup) -> Result<(), String> {
        markup.validate()?;
        let current = self
            .markups
            .iter_mut()
            .find(|m| m.id == markup.id)
            .ok_or("Marcação não encontrada neste pavimento.")?;
        *current = markup;
        Ok(())
    }
    pub fn delete_markup(&mut self, id: u64) -> bool {
        let len = self.markups.len();
        self.markups.retain(|m| m.id != id);
        len != self.markups.len()
    }
    pub fn markup_shown(&self, markup: &Markup) -> bool {
        self.markup_settings.visible && (!markup.resolved || self.markup_settings.show_resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        fixtures::FixtureKind,
        history::{History, ProjectSnapshot},
    };
    fn mark(kind: MarkupKind) -> Markup {
        Markup {
            id: 0,
            kind,
            points: vec![Point::ORIGIN, Point::new(2.0, 2.0)],
            color: [220, 65, 65],
            comment: "Revisar bancada".into(),
            resolved: false,
            linked_elements: vec![],
        }
    }

    #[test]
    fn review_marks_are_floor_local_persistent_and_undoable() {
        let mut project = Project::default();
        let ground = project.ensure_default_floor();
        let id = project.add_markup(mark(MarkupKind::Rectangle)).unwrap();
        let physical = project.content_bounds();
        let mut history = History::new();
        let before = project.clone();
        project.markups[0].comment = "Mover a bancada".into();
        history.record(before.clone());
        let mut undone = project.clone();
        assert!(history.undo(&mut undone));
        assert_eq!(undone, before);
        assert_eq!(project.content_bounds(), physical);
        let snapshot = ProjectSnapshot::from_project_sharing(&project, None);
        assert_eq!(snapshot.to_project(), project);
        let upper = project.add_floor("Superior", true, false);
        assert!(project.markups.is_empty());
        let other = project.add_markup(mark(MarkupKind::Arrow)).unwrap();
        assert_ne!(id, other);
        project.switch_floor(ground).unwrap();
        assert_eq!(project.markups[0].id, id);
        let saved = serde_json::to_string(&project).unwrap();
        let mut loaded: Project = serde_json::from_str(&saved).unwrap();
        loaded.set_next_id(1);
        loaded.validate_and_repair().unwrap();
        assert_eq!(loaded.markups[0].comment, "Mover a bancada");
        let next = loaded.add_markup(mark(MarkupKind::Circle)).unwrap();
        assert!(next > other);
        loaded.switch_floor(upper).unwrap();
        assert_eq!(loaded.markups[0].id, other);
        let legacy: Project = serde_json::from_str(
            r#"{"version":1,"name":"Legado","walls":[],"rooms":[],"openings":[],"next_id":1}"#,
        )
        .unwrap();
        assert!(legacy.markups.is_empty());
        assert!(!legacy.markup_settings.export);
    }

    #[test]
    fn related_elements_use_real_geometry_and_explicit_links() {
        let mut p = Project::default();
        let inside = p
            .add_fixture(FixtureKind::Counter, Point::new(0.5, 0.5), 0.4, 0.4, 0)
            .unwrap();
        let corner = p
            .add_fixture(FixtureKind::Counter, Point::new(1.8, 1.8), 0.2, 0.2, 0)
            .unwrap();
        let diagonal = p
            .add_fixture(FixtureKind::Counter, Point::new(0.3, 1.5), 0.2, 0.2, 0)
            .unwrap();
        let mut m = mark(MarkupKind::Circle);
        m.points[1] = Point::new(2.0, 0.0);
        assert!(m.related_elements(&p).contains(&Element::Fixture(inside)));
        assert!(!m.related_elements(&p).contains(&Element::Fixture(corner)));
        m.kind = MarkupKind::Arrow;
        m.points[1] = Point::new(2.0, 2.0);
        let related = m.related_elements(&p);
        assert!(related.contains(&Element::Fixture(inside)));
        assert!(!related.contains(&Element::Fixture(diagonal)));
        m.linked_elements.push(Element::Fixture(diagonal));
        assert!(m.related_elements(&p).contains(&Element::Fixture(diagonal)));
    }

    #[test]
    fn invalid_marks_never_mutate_the_project() {
        let mut p = Project::default();
        for kind in MarkupKind::ALL {
            let mut m = mark(kind);
            m.points[1] = Point::ORIGIN;
            assert!(p.add_markup(m).is_err());
            let mut m = mark(kind);
            m.points[0].x = f64::NAN;
            assert!(p.add_markup(m).is_err());
            let mut m = mark(kind);
            m.comment = "a".repeat(4001);
            assert!(p.add_markup(m).is_err());
        }
        assert!(p.markups.is_empty());
    }

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(32))]
        #[test]
        fn translating_a_mark_preserves_size_and_hit_testing(x in -100.0..100.0,y in -100.0..100.0,dx in -10.0..10.0,dy in -10.0..10.0) {
            for kind in MarkupKind::ALL {
                let mut m=mark(kind);m.points=vec![Point::new(x,y),Point::new(x+2.0,y+2.0)];
                let (a,b)=m.bounds();let delta=Point::new(dx,dy);m.points.iter_mut().for_each(|p|*p=p.translated(delta));let (c,d)=m.bounds();
                proptest::prop_assert!(((b.x-a.x)-(d.x-c.x)).abs()<1e-8);
                proptest::prop_assert!(m.hit(Point::new(x+dx,y+dy),0.01));
            }
        }
    }
}
