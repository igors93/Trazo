//! Plan marks for balance beams on formwork and foundation sheets.

use super::BalanceBeamResult;
use crate::domain::conduits::PlanMark;
use crate::domain::geometry::Point;
use crate::domain::model::Project;

impl Project {
    /// Balance beams as drawn on the formwork/foundation plan: the two faces
    /// from the boundary footing to the interior column, dashed (below ground),
    /// and the section label.
    pub fn balance_beam_marks(&self, results: &[BalanceBeamResult]) -> Vec<PlanMark> {
        let mut marks = Vec::new();
        for result in results {
            let Some(beam) = self
                .structure
                .balance_beams
                .iter()
                .find(|b| b.id == result.id)
            else {
                continue;
            };
            let (Some(column), Some(anchor)) = (
                self.column_by_id(beam.column),
                self.column_by_id(beam.anchor),
            ) else {
                continue;
            };
            let Some(direction) = anchor.center.minus(column.center).normalized() else {
                continue;
            };
            let normal = Point::new(-direction.y, direction.x);
            for side in [-1.0, 1.0] {
                let off = normal.scaled(side * beam.width_m / 2.0);
                marks.push(PlanMark::Line {
                    points: vec![column.center.translated(off), anchor.center.translated(off)],
                    dashed: true,
                    heavy: true,
                });
            }
            let text = match result.height_m {
                Some(h) => format!(
                    "{} {:.0}×{:.0}",
                    result.label,
                    beam.width_m * 100.0,
                    h * 100.0
                ),
                None => format!("{} (seção insuficiente)", result.label),
            };
            marks.push(PlanMark::Text {
                anchor: column
                    .center
                    .lerp(anchor.center, 0.5)
                    .translated(normal.scaled(beam.width_m / 2.0 + 0.18)),
                text,
            });
        }
        marks
    }
}
