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

/// Cold-water rules (NBR 5626): the probable flow, the speed limit, the
/// least pressure at a point of use, the pipes and their fittings.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ColdWaterRules {
    /// `c` of the probable flow `Q = c·√ΣP` (L/s).
    pub flow_coefficient: f64,
    /// Fastest the water may run, m/s.
    pub max_speed: f64,
    /// Least dynamic pressure at a point of use, kPa.
    pub min_use_kpa: f64,
    /// Roughness of the pipe wall, mm.
    pub roughness: f64,
    /// PVC pipes: outside diameter (DN) and wall, mm.
    pub pipes: Vec<[f64; 2]>,
    /// The fittings of the equivalent-length table, in its column order.
    pub fittings: Vec<String>,
    /// Equivalent lengths, m: DN, then one column per fitting.
    pub equivalent_lengths: Vec<Vec<f64>>,
}

impl Default for ColdWaterRules {
    /// The bundled pack's values, should a custom code lack the section.
    fn default() -> Self {
        Self {
            flow_coefficient: 0.3,
            max_speed: 3.0,
            min_use_kpa: 10.0,
            roughness: 0.0015,
            pipes: vec![
                [20.0, 1.5],
                [25.0, 1.7],
                [32.0, 2.1],
                [40.0, 2.4],
                [50.0, 3.0],
                [60.0, 3.3],
            ],
            fittings: [
                "elbow90",
                "elbow45",
                "tee_through",
                "tee_side",
                "gate",
                "globe",
                "check",
            ]
            .map(str::to_owned)
            .to_vec(),
            equivalent_lengths: vec![
                vec![20.0, 1.1, 0.4, 0.7, 2.3, 0.1, 11.1, 2.5],
                vec![25.0, 1.2, 0.5, 0.8, 2.4, 0.2, 11.4, 2.7],
                vec![32.0, 1.5, 0.7, 0.9, 3.1, 0.3, 15.0, 3.8],
                vec![40.0, 2.0, 1.0, 1.5, 4.6, 0.4, 22.0, 4.9],
                vec![50.0, 3.2, 1.3, 2.2, 7.3, 0.7, 35.8, 6.8],
                vec![60.0, 3.4, 1.5, 2.3, 7.6, 0.8, 37.9, 7.1],
            ],
        }
    }
}

impl ColdWaterRules {
    pub fn for_project(project: &Project) -> Self {
        section(project, "cold_water").unwrap_or_default()
    }

    /// Inside diameter of a PVC pipe, mm.
    pub fn inner_mm(&self, dn: f64) -> f64 {
        self.pipes
            .iter()
            .find(|[d, _]| (*d - dn).abs() < 1e-9)
            .map_or(dn * 0.85, |[d, wall]| d - 2.0 * wall)
    }

    /// The commercial DNs, smallest first.
    pub fn dns(&self) -> Vec<f64> {
        let mut dns: Vec<f64> = self.pipes.iter().map(|[d, _]| *d).collect();
        dns.sort_by(f64::total_cmp);
        dns
    }

    /// Equivalent length of a fitting on a pipe of `dn`, m; the nearest
    /// larger row of the table when `dn` has none.
    pub fn equivalent_m(&self, dn: f64, fitting: &str) -> f64 {
        let Some(column) = self.fittings.iter().position(|f| f == fitting) else {
            return 0.0;
        };
        self.equivalent_lengths
            .iter()
            .find(|row| row.first().is_some_and(|d| *d >= dn - 1e-9))
            .or_else(|| self.equivalent_lengths.last())
            .and_then(|row| row.get(column + 1).copied())
            .unwrap_or(0.0)
    }

    /// Least pressure at a point of use, metres of water (*mca*).
    pub fn min_use_m(&self) -> f64 {
        self.min_use_kpa / super::network::GRAVITY
    }
}

/// How long the water tank must last (NBR 5626), days.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ReservoirRules {
    pub min_days: f64,
    pub max_days: f64,
}

impl Default for ReservoirRules {
    fn default() -> Self {
        Self {
            min_days: 1.0,
            max_days: 3.0,
        }
    }
}

impl ReservoirRules {
    pub fn for_project(project: &Project) -> Self {
        section(project, "reservoir").unwrap_or_default()
    }
}

/// Sewage rules (NBR 8160): the pipes, the branches' and mains' capacities
/// in drainage units, and the vents.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct DrainageRules {
    /// Commercial DNs, mm.
    pub dns: Vec<f64>,
    /// The least DN of a main (*subcoletor*, *coletor predial*).
    pub main_min_dn: f64,
    /// Branches (*ramais de esgoto*): DN and the most units it takes.
    pub branch: Vec<[f64; 2]>,
    /// The slopes of the mains' table, as fractions.
    pub main_slopes: Vec<f64>,
    /// Mains: DN, then the most units at each slope (0 where not allowed).
    pub main: Vec<Vec<f64>>,
    /// Branch vents without a water closet: most units, DN.
    pub branch_vent_no_wc: Vec<[f64; 2]>,
    /// Branch vents with a water closet: most units, DN.
    pub branch_vent_wc: Vec<[f64; 2]>,
}

impl Default for DrainageRules {
    fn default() -> Self {
        Self {
            dns: vec![40.0, 50.0, 75.0, 100.0, 150.0, 200.0],
            main_min_dn: 100.0,
            branch: vec![[40.0, 3.0], [50.0, 6.0], [75.0, 20.0], [100.0, 160.0]],
            main_slopes: vec![0.005, 0.01, 0.02, 0.04],
            main: vec![
                vec![100.0, 0.0, 180.0, 216.0, 250.0],
                vec![150.0, 0.0, 700.0, 840.0, 1000.0],
                vec![200.0, 1400.0, 1600.0, 1920.0, 2300.0],
            ],
            branch_vent_no_wc: vec![[12.0, 40.0], [18.0, 50.0], [36.0, 75.0]],
            branch_vent_wc: vec![[17.0, 50.0], [60.0, 75.0]],
        }
    }
}

impl DrainageRules {
    pub fn for_project(project: &Project) -> Self {
        section(project, "drainage").unwrap_or_default()
    }

    /// The least branch that takes `uhc` units, DN.
    pub fn branch_dn(&self, uhc: f64) -> f64 {
        self.branch
            .iter()
            .find(|[_, most]| *most >= uhc - 1e-9)
            .map_or(self.main_min_dn, |[dn, _]| *dn)
    }

    /// The least main that takes `uhc` units at `slope`: DN and its capacity.
    pub fn main_dn(&self, uhc: f64, slope: f64) -> (f64, f64) {
        let column = self
            .main_slopes
            .iter()
            .rposition(|s| *s <= slope + 1e-9)
            .map_or(1, |i| i + 1);
        self.main
            .iter()
            .filter(|row| row.first().is_some_and(|dn| *dn >= self.main_min_dn - 1e-9))
            .find_map(|row| {
                let most = row.get(column).copied().unwrap_or(0.0);
                (most >= uhc - 1e-9 && most > 0.0).then(|| (row[0], most))
            })
            .unwrap_or_else(|| {
                let last = self.main.last().cloned().unwrap_or_default();
                (
                    last.first().copied().unwrap_or(self.main_min_dn),
                    last.get(column).copied().unwrap_or(0.0),
                )
            })
    }

    /// The vent of a branch of `uhc` units, DN.
    pub fn vent_dn(&self, uhc: f64, with_wc: bool) -> f64 {
        let table = if with_wc {
            &self.branch_vent_wc
        } else {
            &self.branch_vent_no_wc
        };
        table
            .iter()
            .find(|[most, _]| *most >= uhc - 1e-9)
            .or(table.last())
            .map_or(50.0, |[_, dn]| *dn)
    }
}
