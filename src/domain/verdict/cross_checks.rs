//! Independent cross-checks in the traffic light (#172): when the simple
//! weighing disagrees with the detailed simulation, the cross-check is red
//! and no structural result stays green.

use super::{Check, Discipline, Light, Verdict};
use crate::domain::model::Project;
use crate::domain::protection::cross_checks::cross_checks;
use crate::domain::structure::StructureReport;

/// The element name of the cross-check verdict.
pub const CROSS_CHECK_ELEMENT: &str = "Conferência independente";

/// Adds the cross-check verdict and, when it fails, turns every green
/// structural check yellow.
pub(super) fn cross_check(
    project: &Project,
    report: &StructureReport,
    verdicts: &mut Vec<Verdict>,
) {
    if verdicts.is_empty() {
        return;
    }
    let checks: Vec<Check> = cross_checks(project, report)
        .into_iter()
        .map(|c| Check::new(c.name, c.utilisation, c.simple, c.detail))
        .collect();
    if checks.is_empty() {
        return;
    }
    if checks.iter().any(|check| check.light == Light::Red) {
        for verdict in verdicts.iter_mut() {
            for check in &mut verdict.checks {
                if check.light == Light::Green {
                    check.light = Light::Yellow;
                    check.simple.push_str(
                        " Contestado: a conferência independente não bate com a simulação (veja \"Conferência independente\").",
                    );
                }
            }
        }
    }
    verdicts.push(Verdict {
        discipline: Discipline::Structure,
        element: CROSS_CHECK_ELEMENT.to_owned(),
        checks,
    });
}
