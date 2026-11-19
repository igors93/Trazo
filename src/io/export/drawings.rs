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
        if !project.is_drawing_visible(drawing) {
            continue;
        }
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
        let layer = project.effective_drawing_layer(drawing);
        let stroke_color = layer
            .map(|l| l.hex_color())
            .unwrap_or_else(|| "#1d2836".to_owned());
        let stroke_width = layer
            .map(|l| (l.line_weight_mm * 7.0).clamp(0.8, 5.0))
            .unwrap_or(1.4);
        let dash_attr = layer
            .and_then(|l| l.line_type.svg_dasharray())
            .map(|dash| format!(" stroke-dasharray=\"{dash}\""))
            .unwrap_or_default();
        let _ = writeln!(
            svg,
            "<path d=\"{path}\" fill=\"none\" stroke=\"{stroke_color}\" stroke-width=\"{stroke_width:.1}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"{dash_attr}/>"
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
