//! What an edit made worse (#170): the checks whose light got worse, the
//! supporting elements removed and the hypotheses changed. Every AI action
//! reports it, so neither the assistant nor the person can miss an edit that
//! makes the house less safe.

use std::collections::{BTreeMap, BTreeSet};

use super::hypotheses::HypothesisKey;
use crate::domain::fixtures::FixtureKind;
use crate::domain::model::Project;
use crate::domain::verdict::{Light, safety_summary};

/// One check that got worse.
#[derive(Clone, Debug, PartialEq)]
pub struct Worsened {
    pub element: String,
    pub check: String,
    /// `None` when the check did not exist before the edit.
    pub before: Option<Light>,
    pub after: Light,
    /// What to do, in plain words.
    pub simple: String,
}

/// What an edit made worse, in safety terms.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionReport {
    pub worsened: Vec<Worsened>,
    /// Supporting elements removed: "pilar 12", "viga 30"…
    pub removed: Vec<String>,
    /// Hypotheses changed, as "Tensão admissível do solo: 100 kPa → 150 kPa".
    pub hypotheses: Vec<String>,
}

impl ActionReport {
    pub fn is_empty(&self) -> bool {
        self.worsened.is_empty() && self.removed.is_empty() && self.hypotheses.is_empty()
    }

    /// One plain-language paragraph for the person and the assistant.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.worsened.is_empty() {
            let shown: Vec<String> = self
                .worsened
                .iter()
                .take(6)
                .map(|w| {
                    format!(
                        "{} · {}: {} → {}",
                        w.element,
                        w.check.to_lowercase(),
                        w.before.map_or("novo", |l| l.label()).to_lowercase(),
                        w.after.label().to_lowercase()
                    )
                })
                .collect();
            let more = self.worsened.len().saturating_sub(shown.len());
            parts.push(format!(
                "{} verificação(ões) piorou(aram): {}{}",
                self.worsened.len(),
                shown.join("; "),
                if more > 0 {
                    format!(" e mais {more}")
                } else {
                    String::new()
                }
            ));
        }
        if !self.removed.is_empty() {
            parts.push(format!(
                "elementos que sustentam a casa foram removidos: {}",
                self.removed.join(", ")
            ));
        }
        if !self.hypotheses.is_empty() {
            parts.push(format!(
                "hipóteses de cálculo mudaram: {}",
                self.hypotheses.join("; ")
            ));
        }
        format!(
            "Atenção, esta ação piorou a segurança ou mudou o que o cálculo supõe: {}.",
            parts.join("; ")
        )
    }
}

/// Compares the project before and after an edit.
pub fn action_report(before: &Project, after: &Project) -> ActionReport {
    if before == after {
        return ActionReport::default();
    }
    ActionReport {
        worsened: worsened_checks(before, after),
        removed: removed_supports(before, after),
        hypotheses: changed_hypotheses(before, after),
    }
}

fn worsened_checks(before: &Project, after: &Project) -> Vec<Worsened> {
    if !has_checks(before) && !has_checks(after) {
        return Vec::new();
    }
    let lights: BTreeMap<(String, String), Light> = safety_summary(before)
        .verdicts
        .into_iter()
        .flat_map(|verdict| {
            let element = verdict.element;
            verdict
                .checks
                .into_iter()
                .map(move |check| ((element.clone(), check.name), check.light))
        })
        .collect();
    let mut worsened = Vec::new();
    for verdict in safety_summary(after).verdicts {
        for check in verdict.checks {
            let key = (verdict.element.clone(), check.name.clone());
            let earlier = lights.get(&key).copied();
            let worse = match earlier {
                Some(light) => check.light > light,
                None => check.light != Light::Green,
            };
            if worse {
                worsened.push(Worsened {
                    element: verdict.element.clone(),
                    check: check.name,
                    before: earlier,
                    after: check.light,
                    simple: check.simple,
                });
            }
        }
    }
    worsened.sort_by_key(|w| std::cmp::Reverse(w.after));
    worsened
}

/// Whether the project has anything the traffic light judges.
fn has_checks(project: &Project) -> bool {
    !project.structure.beams.is_empty()
        || !project.structure.slabs.is_empty()
        || !project.conduits.is_empty()
        || project
            .fixtures
            .iter()
            .any(|f| f.kind.is_structural() || f.kind.is_electrical())
}

fn supports(project: &Project) -> BTreeSet<(String, u64)> {
    let mut found = BTreeSet::new();
    for fixture in &project.fixtures {
        let name = match fixture.kind {
            FixtureKind::Column => "pilar",
            FixtureKind::Footing => "sapata",
            FixtureKind::PileCap => "bloco sobre broca",
            FixtureKind::TimberPost => "mourão",
            _ => continue,
        };
        found.insert((name.to_owned(), fixture.id));
    }
    for beam in &project.structure.beams {
        found.insert(("viga".to_owned(), beam.id));
    }
    for slab in &project.structure.slabs {
        found.insert(("laje".to_owned(), slab.id));
    }
    for foundation in &project.structure.wall_foundations {
        found.insert(("fundação de parede".to_owned(), foundation.id));
    }
    for beam in &project.structure.balance_beams {
        found.insert(("viga alavanca".to_owned(), beam.id));
    }
    found
}

fn removed_supports(before: &Project, after: &Project) -> Vec<String> {
    let now = supports(after);
    supports(before)
        .difference(&now)
        .map(|(name, id)| format!("{name} {id}"))
        .collect()
}

fn changed_hypotheses(before: &Project, after: &Project) -> Vec<String> {
    HypothesisKey::ALL
        .into_iter()
        .filter(|key| key.display(before) != key.display(after))
        .map(|key| {
            format!(
                "{}: {} → {}",
                key.label(),
                key.display(before),
                key.display(after)
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    fn house() -> Project {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project.generate_structure();
        project
    }

    #[test]
    fn removing_a_column_and_changing_a_hypothesis_are_reported() {
        let before = house();
        let mut after = before.clone();
        let column = after
            .fixtures
            .iter()
            .position(|f| f.kind == FixtureKind::Column);
        assert!(column.is_some());
        if let Some(index) = column {
            after.fixtures.remove(index);
        }
        after.structure.settings.fck_mpa = 30.0;
        let report = action_report(&before, &after);
        assert!(report.removed.iter().any(|r| r.starts_with("pilar")));
        assert!(report.hypotheses.iter().any(|h| h.contains("fck")));
        assert!(report.summary().contains("Atenção"));
    }

    #[test]
    fn an_unchanged_project_reports_nothing() {
        let project = house();
        assert!(action_report(&project, &project).is_empty());
    }
}
