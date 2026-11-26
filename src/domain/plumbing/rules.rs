//! The NBR tables the plumbing design reads from the project's plumbing
//! building code, or from the bundled NBR pack when the project's code has
//! no such section.

use serde::Deserialize;

use crate::domain::codes::{CodeDiscipline, builtin_codes};
use crate::domain::fixtures::FixtureKind;
use crate::domain::model::Project;

/// One fixture's water and drain figures.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct FixtureFigures {
    /// The pack's name for the fixture (`wc`, `lav`, `sink`, `shower`).
    pub kind: String,
    /// Design flow of cold water, L/s.
    pub cold_flow: f64,
    /// Design flow of hot water, L/s, when the pack gives one.
    #[serde(default)]
    pub hot_flow: Option<f64>,
    /// Relative weight (*peso relativo*) for the probable flow, NBR 5626.
    pub weight: f64,
    /// Drainage fixture units (*UHC*), NBR 8160.
    pub uhc: f64,
    /// The least drain (*ramal de descarga*), DN.
    pub drain_dn: f64,
    /// True when the pack has no row for the fixture and the figures are
    /// Trazo's reference values.
    #[serde(skip)]
    pub trazo_reference: bool,
}

#[derive(Deserialize)]
struct FixturesSection {
    fixtures: Vec<FixtureFigures>,
}

/// The pack's name for a fixture kind.
fn pack_kind(kind: FixtureKind) -> Option<&'static str> {
    match kind {
        FixtureKind::Toilet => Some("wc"),
        FixtureKind::Washbasin => Some("lav"),
        FixtureKind::KitchenSink => Some("sink"),
        FixtureKind::Shower => Some("shower"),
        _ => None,
    }
}

/// Trazo's reference figures, for the fixtures the pack leaves out: the
/// weights and flows of NBR 5626:1998 Annex A and the UHC and least drains
/// of NBR 8160 Table 3, as they are usually published.
fn reference_figures(kind: FixtureKind) -> Option<FixtureFigures> {
    let (key, flow, weight, uhc, dn) = match kind {
        FixtureKind::Toilet => ("wc", 0.15, 0.3, 6.0, 100.0),
        FixtureKind::Washbasin => ("lav", 0.15, 0.3, 1.0, 40.0),
        FixtureKind::KitchenSink => ("sink", 0.25, 0.7, 3.0, 50.0),
        FixtureKind::Shower => ("shower", 0.20, 0.4, 2.0, 40.0),
        FixtureKind::Washer => ("washer", 0.30, 1.0, 3.0, 50.0),
        FixtureKind::Bathtub => ("bathtub", 0.30, 1.0, 2.0, 40.0),
        _ => return None,
    };
    Some(FixtureFigures {
        kind: key.to_owned(),
        cold_flow: flow,
        hot_flow: None,
        weight,
        uhc,
        drain_dn: dn,
        trazo_reference: true,
    })
}

/// A section of the project's plumbing code; the bundled NBR pack's when
/// the project's code has none.
pub(crate) fn section<T>(project: &Project, name: &str) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    project
        .codes
        .resolve_typed::<T>(CodeDiscipline::Plumbing, name)
        .or_else(|| {
            builtin_codes()
                .iter()
                .find(|c| c.discipline == CodeDiscipline::Plumbing && c.code == "nbr")
                .and_then(|c| c.sections.get(name))
                .and_then(|value| serde_json::from_value(value.clone()).ok())
        })
}

/// The figures of every plumbing fixture kind, read once.
#[derive(Clone, Debug, Default)]
pub struct FixtureTable {
    rows: Vec<FixtureFigures>,
}

impl FixtureTable {
    pub fn for_project(project: &Project) -> Self {
        let rows = section::<FixturesSection>(project, "fixtures")
            .map(|s| s.fixtures)
            .unwrap_or_default();
        Self { rows }
    }

    /// The figures of `kind`, or `None` when it takes no water.
    pub fn figures(&self, kind: FixtureKind) -> Option<FixtureFigures> {
        if !kind.is_plumbing() {
            return None;
        }
        pack_kind(kind)
            .and_then(|key| self.rows.iter().find(|row| row.kind == key).cloned())
            .or_else(|| reference_figures(kind))
    }
}
