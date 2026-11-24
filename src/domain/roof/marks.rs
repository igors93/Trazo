//! Plan marks for roofs (outline, overhangs, ridge, hip rafters, main beams/trusses and label).

use crate::domain::{conduits::PlanMark, geometry::Point, model::Project};

use super::{RoofElement, RoofResult, RoofStyle};

impl Project {
    /// The roof outlines, ridges, hip rafters, main beams/trusses and labels drawn on the plan.
    pub fn roof_marks(&self, results: &[RoofResult]) -> Vec<PlanMark> {
        let mut marks = Vec::new();
        for result in results {
            let Some(roof) = self.structure.roofs.iter().find(|r| r.id == result.id) else {
                continue;
            };
            let (a, c) = (roof.min, roof.max);
            let oh = roof.overhang_m.clamp(0.0, 1.5);
            let oa = Point::new(a.x - oh, a.y - oh);
            let oc = Point::new(c.x + oh, c.y + oh);
            // Outer eave outline (includes overhang when > 0).
            marks.push(PlanMark::Line {
                points: vec![oa, Point::new(oc.x, oa.y), oc, Point::new(oa.x, oc.y), oa],
                dashed: true,
                heavy: false,
            });
            if oh > 0.05 {
                // Inner support perimeter line.
                marks.push(PlanMark::Line {
                    points: vec![a, Point::new(c.x, a.y), c, Point::new(a.x, c.y), a],
                    dashed: true,
                    heavy: false,
                });
            }
            // Draw ridge (cumeeira) and hip rafters (espigões) for Gable & Hip roofs.
            match roof.style {
                RoofStyle::MonoPitch => {}
                RoofStyle::Gable => {
                    let (r0, r1) = if roof.along_x() {
                        let my = 0.5 * (oa.y + oc.y);
                        (Point::new(oa.x, my), Point::new(oc.x, my))
                    } else {
                        let mx = 0.5 * (oa.x + oc.x);
                        (Point::new(mx, oa.y), Point::new(mx, oc.y))
                    };
                    marks.push(PlanMark::Line {
                        points: vec![r0, r1],
                        dashed: false,
                        heavy: true,
                    });
                }
                RoofStyle::Hip => {
                    let w = oc.x - oa.x;
                    let h = oc.y - oa.y;
                    let inset = 0.5 * w.min(h);
                    let (r0, r1) = if w >= h {
                        let my = 0.5 * (oa.y + oc.y);
                        (Point::new(oa.x + inset, my), Point::new(oc.x - inset, my))
                    } else {
                        let mx = 0.5 * (oa.x + oc.x);
                        (Point::new(mx, oa.y + inset), Point::new(mx, oc.y - inset))
                    };
                    marks.push(PlanMark::Line {
                        points: vec![r0, r1],
                        dashed: false,
                        heavy: true,
                    });
                    let corners = [
                        (oa, r0),
                        (Point::new(oa.x, oc.y), if w >= h { r0 } else { r1 }),
                        (Point::new(oc.x, oa.y), if w >= h { r1 } else { r0 }),
                        (oc, r1),
                    ];
                    for (corner, apex) in corners {
                        marks.push(PlanMark::Line {
                            points: vec![corner, apex],
                            dashed: false,
                            heavy: false,
                        });
                    }
                }
            }
            for pair in result.reactions.chunks(2) {
                if let [high, low] = pair {
                    marks.push(PlanMark::Line {
                        points: vec![high.point, low.point],
                        dashed: true,
                        heavy: false,
                    });
                }
            }
            let detail = if let Some(truss) = &result.truss {
                format!(" · {} tesoura(s)", truss.count)
            } else {
                result
                    .elements
                    .iter()
                    .find(|e| e.element == RoofElement::Beams)
                    .map_or(String::new(), |e| {
                        format!(" · vigas {:.0}×{:.0}", e.section_cm.0, e.section_cm.1)
                    })
            };
            let label_text = match roof.style {
                RoofStyle::MonoPitch => format!(
                    "{} meia-água {:.0} % → {}{}",
                    result.label,
                    result.slope_percent,
                    roof.high_side.opposite().label(),
                    detail
                ),
                RoofStyle::Gable => format!(
                    "{} duas águas {:.0} %{}",
                    result.label, result.slope_percent, detail
                ),
                RoofStyle::Hip => format!(
                    "{} quatro águas {:.0} %{}",
                    result.label, result.slope_percent, detail
                ),
            };
            marks.push(PlanMark::Text {
                anchor: a.lerp(c, 0.5),
                text: label_text,
            });
        }
        marks
    }
}
