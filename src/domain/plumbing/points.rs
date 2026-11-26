//! Plumbing points (#72): where each fixture takes cold and hot water,
//! where it drains and where the stove takes gas, at the usual heights;
//! their counts; and the fixture schedule (*quadro de aparelhos*) with the
//! weights and drainage units the water and sewage design add up.

use crate::domain::fixtures::{Fixture, FixtureKind};
use crate::domain::geometry::Point;
use crate::domain::model::Project;

use super::rules::FixtureTable;

/// What a point carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointKind {
    /// Água fria.
    Cold,
    /// Água quente.
    Hot,
    /// Esgoto.
    Drain,
    /// Gás (GLP).
    Gas,
}

impl PointKind {
    pub const ALL: [PointKind; 4] = [
        PointKind::Cold,
        PointKind::Hot,
        PointKind::Drain,
        PointKind::Gas,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Cold => "Água fria",
            Self::Hot => "Água quente",
            Self::Drain => "Esgoto",
            Self::Gas => "Gás (GLP)",
        }
    }

    /// The tag written next to the point on the plan.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Cold => "AF",
            Self::Hot => "AQ",
            Self::Drain => "ES",
            Self::Gas => "GÁS",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Cold => "agua_fria",
            Self::Hot => "agua_quente",
            Self::Drain => "esgoto",
            Self::Gas => "gas",
        }
    }

    /// The colour of the point and its pipes, on screen and on the sheets.
    pub fn color(self) -> [u8; 3] {
        match self {
            Self::Cold => [31, 111, 209],
            Self::Hot => [209, 52, 47],
            Self::Drain => [122, 82, 48],
            Self::Gas => [196, 140, 0],
        }
    }

    pub fn hex(self) -> String {
        let [r, g, b] = self.color();
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

/// One point: a fixture's connection to a pipe.
#[derive(Clone, Debug, PartialEq)]
pub struct PlumbingPoint {
    pub fixture_id: u64,
    pub fixture: FixtureKind,
    pub kind: PointKind,
    /// Where the pipe comes out of the wall or floor, metres.
    pub at: Point,
    /// Height of the outlet above the finished floor, metres.
    pub height_m: f64,
    /// Nominal size of the connection: DN of the water or gas pipe, DN of
    /// the drain.
    pub diameter_mm: f64,
    /// The room the fixture stands in.
    pub room: Option<String>,
}

/// Nominal size of a water point (PVC *soldável* DN 20, ½").
pub const WATER_POINT_DN: f64 = 20.0;
/// Nominal size of a gas point (copper or PEX 15 mm).
pub const GAS_POINT_DN: f64 = 15.0;

/// Where a kind of fixture takes its points, in the fixture's own frame
/// (x along its width, y from its back at `-depth/2`), and at what height.
/// The usual heights from the finished floor; the maker's manual has the
/// last word.
fn layout(kind: FixtureKind, width: f64, depth: f64, hot: bool) -> Vec<(PointKind, Point, f64)> {
    let back = -depth / 2.0;
    let mut points = match kind {
        // Toilet with a close-coupled cistern: water 0.20 m high beside it,
        // the DN 100 drain under it, 0.30 m from the wall.
        FixtureKind::Toilet => vec![
            (PointKind::Cold, Point::new(-0.15, back), 0.20),
            (PointKind::Drain, Point::new(0.0, back + 0.30), 0.0),
        ],
        FixtureKind::Washbasin => vec![
            (PointKind::Cold, Point::new(0.10, back), 0.60),
            (PointKind::Drain, Point::new(0.0, back), 0.50),
        ],
        // Wall tap over the sink.
        FixtureKind::KitchenSink => vec![
            (PointKind::Cold, Point::new(0.10, back), 1.10),
            (PointKind::Drain, Point::new(0.0, back), 0.55),
        ],
        // Shower head at 2.10 m (its valve at 1.10 m), floor drain in the
        // middle of the box.
        FixtureKind::Shower => vec![
            (PointKind::Cold, Point::new(0.0, back), 2.10),
            (PointKind::Drain, Point::new(0.0, 0.0), 0.0),
        ],
        FixtureKind::Bathtub => {
            let end = -width / 2.0 + 0.20;
            vec![
                (PointKind::Cold, Point::new(end, back), 0.55),
                (PointKind::Drain, Point::new(end, 0.0), 0.0),
            ]
        }
        FixtureKind::Washer => vec![
            (PointKind::Cold, Point::new(0.0, back), 0.90),
            (PointKind::Drain, Point::new(0.20, back), 0.80),
        ],
        FixtureKind::Stove => vec![(PointKind::Gas, Point::new(width / 2.0 - 0.10, back), 0.80)],
        _ => Vec::new(),
    };
    let takes_hot = match kind {
        // A bathtub needs hot water whatever heats it.
        FixtureKind::Bathtub => true,
        FixtureKind::Washbasin | FixtureKind::KitchenSink | FixtureKind::Shower => hot,
        _ => false,
    };
    if takes_hot && let Some(&(_, cold, height)) = points.iter().find(|p| p.0 == PointKind::Cold) {
        // Hot water on the left of the cold, as Brazilian taps have it.
        points.insert(
            1,
            (PointKind::Hot, Point::new(cold.x - 0.20, cold.y), height),
        );
    }
    points
}

/// The points of one fixture.
pub fn fixture_points(
    project: &Project,
    fixture: &Fixture,
    table: &FixtureTable,
) -> Vec<PlumbingPoint> {
    let hot = project.plumbing.heater.pipes_hot_water();
    let room = project
        .room_at(fixture.center)
        .and_then(|id| project.rooms.iter().find(|r| r.id == id))
        .map(|r| r.display_name().to_owned());
    let drain_dn = table.figures(fixture.kind).map_or(40.0, |f| f.drain_dn);
    layout(fixture.kind, fixture.width, fixture.depth, hot)
        .into_iter()
        .map(|(kind, local, height_m)| PlumbingPoint {
            fixture_id: fixture.id,
            fixture: fixture.kind,
            kind,
            at: fixture.to_world(local),
            height_m,
            diameter_mm: match kind {
                PointKind::Drain => drain_dn,
                PointKind::Gas => GAS_POINT_DN,
                PointKind::Cold | PointKind::Hot => WATER_POINT_DN,
            },
            room: room.clone(),
        })
        .collect()
}

/// Whether a fixture has plumbing points.
pub fn has_points(kind: FixtureKind) -> bool {
    kind.is_plumbing() || kind == FixtureKind::Stove
}

/// One line of the fixture schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleRow {
    pub kind: FixtureKind,
    pub count: usize,
    /// The rooms they stand in, each once.
    pub rooms: Vec<String>,
    /// Height of the cold, hot and gas points, metres.
    pub cold_m: Option<f64>,
    pub hot_m: Option<f64>,
    pub gas_m: Option<f64>,
    /// The drain: DN and height, metres.
    pub drain: Option<(f64, f64)>,
    /// Weight and drainage units of one fixture.
    pub weight: f64,
    pub uhc: f64,
    pub trazo_reference: bool,
}

/// The points of the whole plan, their counts and the fixture schedule.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlumbingPoints {
    pub points: Vec<PlumbingPoint>,
    pub schedule: Vec<ScheduleRow>,
    /// Sum of the weights of the cold-water points.
    pub total_weight: f64,
    /// Sum of the drainage fixture units.
    pub total_uhc: f64,
    pub notes: Vec<String>,
}

impl PlumbingPoints {
    pub fn count(&self, kind: PointKind) -> usize {
        self.points.iter().filter(|p| p.kind == kind).count()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// The points, counts and schedule of the plan's fixtures.
pub fn plumbing_points(project: &Project) -> PlumbingPoints {
    let table = FixtureTable::for_project(project);
    let mut out = PlumbingPoints::default();
    for fixture in project.fixtures.iter().filter(|f| has_points(f.kind)) {
        let points = fixture_points(project, fixture, &table);
        if points.is_empty() {
            continue;
        }
        let figures = table.figures(fixture.kind);
        let (weight, uhc) = figures.as_ref().map_or((0.0, 0.0), |f| (f.weight, f.uhc));
        out.total_weight += weight;
        out.total_uhc += uhc;
        let height = |kind: PointKind| points.iter().find(|p| p.kind == kind).map(|p| p.height_m);
        let room = points.first().and_then(|p| p.room.clone());
        let drain = points
            .iter()
            .find(|p| p.kind == PointKind::Drain)
            .map(|p| (p.diameter_mm, p.height_m));
        match out.schedule.iter_mut().find(|row| row.kind == fixture.kind) {
            Some(row) => {
                row.count += 1;
                if let Some(room) = room
                    && !row.rooms.contains(&room)
                {
                    row.rooms.push(room);
                }
            }
            None => out.schedule.push(ScheduleRow {
                kind: fixture.kind,
                count: 1,
                rooms: room.into_iter().collect(),
                cold_m: height(PointKind::Cold),
                hot_m: height(PointKind::Hot),
                gas_m: height(PointKind::Gas),
                drain,
                weight,
                uhc,
                trazo_reference: figures.is_some_and(|f| f.trazo_reference),
            }),
        }
        out.points.extend(points);
    }
    if out.is_empty() {
        return out;
    }
    out.notes.push(
        "Alturas usuais a partir do piso acabado; o manual do aparelho tem a palavra final."
            .to_owned(),
    );
    if project.plumbing.heater.pipes_hot_water() {
        out.notes.push(format!(
            "{}: água quente chega à pia, ao lavatório e ao chuveiro.",
            project.plumbing.heater.label()
        ));
    } else {
        out.notes.push(
            "Chuveiro elétrico: sem tubos de água quente; cada chuveiro precisa da sua tomada de uso específico no projeto elétrico."
                .to_owned(),
        );
    }
    if out.count(PointKind::Gas) > 0 {
        out.notes.push(
            "Gás: botijão fora da cozinha, num abrigo ventilado, quando der; mangueira com validade e de no máximo 1,25 m."
                .to_owned(),
        );
    }
    if out.schedule.iter().any(|row| row.trazo_reference) {
        out.notes.push(
            "Máquina de lavar e banheira: peso e UHC de referência do Trazo (NBR 5626:1998 Anexo A, NBR 8160 Tabela 3); o pacote de normas não os traz."
                .to_owned(),
        );
    }
    out
}
