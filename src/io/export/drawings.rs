//! Drawing entities (lines, arcs, circles...) in the exported plan, as
//! SVG paths in the same thin dark ink as on screen.

use std::fmt::Write as _;

use crate::domain::geometry::Point;
use crate::domain::model::Project;

pub(super) fn write_drawings(
    svg: &mut String,
    project: &Project,
    point: &impl Fn(Point) -> (f64, f64),
) {
    for drawing in &project.drawings {
        let outline = drawing.shape.outline();
        let Some((first, rest)) = outline.split_first() else {
            continue;
        };
        let (x, y) = point(*first);
        let mut path = format!("M{x:.2} {y:.2}");
        for corner in rest {
            let (x, y) = point(*corner);
            let _ = write!(path, " L{x:.2} {y:.2}");
        }
        let _ = writeln!(
            svg,
            "<path d=\"{path}\" fill=\"none\" stroke=\"#1d2836\" stroke-width=\"1.4\" stroke-linejoin=\"round\" stroke-linecap=\"round\"/>"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::drawing::Shape;

    #[test]
    fn every_drawing_becomes_a_path() {
        let mut project = Project::default();
        project
            .add_drawing(Shape::rectangle(Point::ORIGIN, Point::new(2.0, 1.0)))
            .unwrap();
        project
            .add_drawing(Shape::Circle {
                center: Point::new(5.0, 0.0),
                radius: 1.0,
            })
            .unwrap();
        let mut svg = String::new();
        write_drawings(&mut svg, &project, &|p: Point| (p.x * 100.0, p.y * 100.0));
        assert_eq!(svg.matches("<path").count(), 2);
        // The rectangle closes back on its first corner.
        assert!(svg.starts_with(
            "<path d=\"M0.00 0.00 L200.00 0.00 L200.00 100.00 L0.00 100.00 L0.00 0.00\""
        ));
    }
}
