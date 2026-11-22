//! Plan marks for roofs (outline, main beams and slope label).

use crate::domain::{conduits::PlanMark, geometry::Point, model::Project};

use super::{RoofElement, RoofResult};

impl Project {
    /// The roof outlines, main beams and labels drawn on the plan.
    pub fn roof_marks(&self, results: &[RoofResult]) -> Vec<PlanMark> {
        let mut marks = Vec::new();
        for result in results {
            let Some(roof) = self.structure.roofs.iter().find(|r| r.id == result.id) else {
                continue;
            };
            let (a, c) = (roof.min, roof.max);
            marks.push(PlanMark::Line {
                points: vec![a, Point::new(c.x, a.y), c, Point::new(a.x, c.y), a],
                dashed: true,
                heavy: false,
            });
            for pair in result.reactions.chunks(2) {
                if let [high, low] = pair {
                    marks.push(PlanMark::Line {
                        points: vec![high.point, low.point],
                        dashed: true,
                        heavy: false,
                    });
                }
            }
            let beams = result
                .elements
                .iter()
                .find(|e| e.element == RoofElement::Beams)
                .map_or(String::new(), |e| {
                    format!(" · vigas {:.0}×{:.0}", e.section_cm.0, e.section_cm.1)
                });
            marks.push(PlanMark::Text {
                anchor: a.lerp(c, 0.5),
                text: format!(
                    "{} meia-água {:.0} % → {}{}",
                    result.label,
                    result.slope_percent,
                    roof.high_side.opposite().label(),
                    beams
                ),
            });
        }
        marks
    }
}
