//! Building in stages in the traffic light (#158).

use super::{Check, Discipline, Verdict};
use crate::domain::model::Project;
use crate::domain::protection::help::HELP_SENTENCE;
use crate::domain::protection::stages::extension_beyond_built;

/// The element name of the staged-building verdict.
pub const STAGES_ELEMENT: &str = "Construção em etapas";

pub(super) fn stages(project: &Project, verdicts: &mut Vec<Verdict>) {
    if verdicts.is_empty() {
        return;
    }
    let plan = project.protection.stages;
    let mut checks = Vec::new();
    if let Some((built, now)) = extension_beyond_built(project) {
        checks.push(Check::fail(
            "Ampliação além da fundação feita",
            format!(
                "A fundação foi feita para {built} pavimento(s) e o projeto agora tem {now}. Não suba mais andares sobre ela sem um profissional avaliar a fundação e os pilares. {HELP_SENTENCE}"
            ),
            "Construção em etapas (#158): a ampliação passa do que a casa foi feita para aguentar.".to_owned(),
        ));
    }
    if plan.future_floors > 0 {
        checks.push(Check::new(
            "Andares futuros já considerados",
            0.5,
            format!(
                "A fundação, os pilares e a laje já foram calculados para mais {} andar(es) em cima.",
                plan.future_floors
            ),
            format!(
                "Pavimentos carregados pelos pilares: {} (térreo + {} futuro(s)).",
                project.structure.settings.floors, plan.future_floors
            ),
        ));
    }
    if checks.is_empty() {
        return;
    }
    verdicts.push(Verdict {
        discipline: Discipline::Structure,
        element: STAGES_ELEMENT.to_owned(),
        checks,
    });
}
