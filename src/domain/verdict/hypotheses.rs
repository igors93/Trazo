//! Contested hypotheses in the traffic light (#169): a structural result
//! that rests on a hypothesis Trazo does not accept is never green, and the
//! contested hypotheses themselves are red.

use super::{Check, Discipline, Light, Verdict};
use crate::domain::model::Project;
use crate::domain::protection::hypotheses::{contests, evidence};

/// The element name of the verdict that lists the contested hypotheses.
pub const HYPOTHESES_ELEMENT: &str = "Hipóteses de cálculo";

/// Turns every green structural check yellow and adds a red verdict per
/// contested hypothesis, when there is any structure to judge.
pub(super) fn contest(project: &Project, verdicts: &mut Vec<Verdict>) {
    if verdicts.is_empty() {
        return;
    }
    let found = contests(project);
    if found.is_empty() {
        return;
    }
    for verdict in verdicts.iter_mut() {
        for check in &mut verdict.checks {
            if check.light == Light::Green {
                check.light = Light::Yellow;
                check.simple.push_str(
                    " Contestado: depende de uma hipótese de cálculo sem prova (veja \"Hipóteses de cálculo\").",
                );
            }
        }
    }
    let checks = found
        .into_iter()
        .map(|contested| {
            let key = contested.key;
            let record = project.protection.hypotheses.record(key);
            let proof = evidence(key, project)
                .map_or_else(|| "sem prova".to_owned(), |e| format!("prova: {e}"));
            let detail = format!(
                "{}: {} · {} · {proof}. {} Regra 10 do contrato de segurança: sem prova, sem verde, e a norma não baixa.",
                key.label(),
                key.display(project),
                record.source.label(),
                key.plain()
            );
            Check::fail(
                key.label(),
                format!("{} {}", contested.reason, contested.fix),
                detail,
            )
        })
        .collect();
    verdicts.push(Verdict {
        discipline: Discipline::Structure,
        element: HYPOTHESES_ELEMENT.to_owned(),
        checks,
    });
}
