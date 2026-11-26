//! Building in stages (#158). Self-built houses grow over years: a room
//! first, the slab later, a floor on top for a child who married. The
//! first stage must already carry what comes next, and an extension beyond
//! what the house was built for must not pass in silence.

use serde::{Deserialize, Serialize};

use crate::domain::limits::{MAX_STOREYS, storeys};
use crate::domain::model::Project;

/// The plan for the floors to come.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StagePlan {
    /// Floors the family means to build on top later.
    pub future_floors: u32,
    /// The storeys the foundation was built for, once it is built.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub built_for: Option<u32>,
}

impl StagePlan {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Storeys drawn with walls or structure, counting the active one.
pub fn drawn_storeys(project: &Project) -> u32 {
    let active = project.floors.active_floor_id;
    let others = project
        .floors
        .levels
        .iter()
        .filter(|level| {
            Some(level.id) != active
                && (!level.data.walls.is_empty() || !level.data.beams.is_empty())
        })
        .count() as u32;
    1 + others
}

/// Plans `future` floors on top: the columns carry them from now on, so the
/// first stage's footings, columns and slab are sized for the whole house.
pub fn plan_future_floors(project: &mut Project, future: u32) -> Result<(), String> {
    if drawn_storeys(project) > 1 {
        return Err(
            "Este projeto já tem andares desenhados: desenhe também o andar futuro como um pavimento, para o Trazo calcular o peso real dele."
                .to_owned(),
        );
    }
    if 1 + future > MAX_STOREYS {
        return Err(format!(
            "O pré-dimensionamento do Trazo vale até {MAX_STOREYS} pavimentos: com mais andares, procure um profissional."
        ));
    }
    project.protection.stages.future_floors = future;
    project.structure.settings.floors = 1 + future;
    Ok(())
}

/// Marks the foundation as built, for the storeys the structure carries now.
pub fn mark_foundation_built(project: &mut Project, built: bool) {
    project.protection.stages.built_for = built.then(|| storeys(project));
}

/// When the project now carries more storeys than the built foundation was
/// made for: (built for, carried now).
pub fn extension_beyond_built(project: &Project) -> Option<(u32, u32)> {
    let built = project.protection.stages.built_for?;
    let now = storeys(project);
    (now > built).then_some((built, now))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn future_floors_are_carried_and_an_extension_beyond_the_built_base_is_caught() {
        let mut project = Project::default();
        plan_future_floors(&mut project, 1).unwrap();
        assert_eq!(project.structure.settings.floors, 2);
        mark_foundation_built(&mut project, true);
        assert_eq!(project.protection.stages.built_for, Some(2));
        assert_eq!(extension_beyond_built(&project), None);
        plan_future_floors(&mut project, 2).unwrap();
        assert_eq!(extension_beyond_built(&project), Some((2, 3)));
        assert!(plan_future_floors(&mut project, 5).is_err());
    }
}
