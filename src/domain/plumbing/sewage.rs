//! Sewage (#120, NBR 8160): from every fixture's drain to the street sewer
//! (or the septic tank).
//!
//! In each wet room the washbasin, shower, bathtub and washing machine
//! drain to a floor trap (*caixa sifonada*); the water closet drains on its
//! own, DN 100; the kitchen sink drains to a grease trap (*caixa de
//! gordura*) outside. Each room's branch (*ramal de esgoto*) takes the
//! drainage units (UHC) of its fixtures and leaves the building to an
//! inspection box (*caixa de inspeção*); a buried main (*subcoletor*,
//! *coletor predial*) joins the boxes and runs to the outfall, with a box at
//! every change of direction and at most 25 m apart. Branches of DN 75 or
//! less fall 2 %, larger ones 1 % (the standard's least). A room with a
//! water closet has a vent (*tubo ventilador*) rising above the roof.

use crate::domain::fixtures::FixtureKind;
use crate::domain::geometry::Point;
use crate::domain::model::Project;

use super::cold_water::decimal;
use super::points::{PlumbingPoint, PointKind, plumbing_points};
use super::rules::{DrainageRules, FixtureTable};

/// Fall of branches of DN 75 or less, and of larger pipes (NBR 8160).
pub const SMALL_SLOPE: f64 = 0.02;
pub const LARGE_SLOPE: f64 = 0.01;
/// Depth of the first box's outlet under the ground, m (cover over the pipe).
const FIRST_DEPTH_M: f64 = 0.40;
/// The boxes stand this far outside the walls, m.
const OUTSIDE_M: f64 = 0.60;
/// Most distance between two inspection devices, m (NBR 8160).
pub const MAX_BOX_SPACING_M: f64 = 25.0;
/// An inspection box deeper than this is a manhole (*poço de visita*), m.
pub const MAX_BOX_DEPTH_M: f64 = 1.00;

/// A device of the sewage system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    FloorTrap,
    GreaseTrap,
    InspectionBox,
    Vent,
}

impl DeviceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::FloorTrap => "Caixa sifonada",
            Self::GreaseTrap => "Caixa de gordura",
            Self::InspectionBox => "Caixa de inspeção",
            Self::Vent => "Tubo ventilador",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub kind: DeviceKind,
    /// "CS-1", "CI-3"…
    pub label: String,
    pub at: Point,
    pub room: Option<String>,
    /// Its outlet (or the vent's) DN.
    pub dn: f64,
    /// Depth of the outlet under the ground, for the buried boxes, m.
    pub depth_m: Option<f64>,
}

/// What a drain pipe is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrainRole {
    /// Ramal de descarga: from one fixture.
    Fixture,
    /// Ramal de esgoto: a room's branch.
    Branch,
    /// Subcoletor / coletor predial: buried, between boxes.
    Main,
}

impl DrainRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fixture => "Ramal de descarga",
            Self::Branch => "Ramal de esgoto",
            Self::Main => "Coletor",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drain {
    pub role: DrainRole,
    pub path: Vec<Point>,
    pub dn: f64,
    /// Fall, as a fraction.
    pub slope: f64,
    /// Drainage units it carries.
    pub uhc: f64,
    pub length_m: f64,
}

/// Where the sewage goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outfall {
    PublicSewer,
    Septic,
}

/// The sewage design.
#[derive(Clone, Debug, PartialEq)]
pub struct Sewage {
    pub drains: Vec<Drain>,
    pub devices: Vec<Device>,
    pub total_uhc: f64,
    pub outfall: Outfall,
    /// Where the main leaves the lot (or enters the septic tank).
    pub exit_at: Point,
    pub exit_given: bool,
    /// Depth of the main where it leaves, m.
    pub exit_depth_m: f64,
    /// The main at the outfall: DN and the units it takes at its slope.
    pub main_dn: f64,
    pub main_capacity_uhc: f64,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

fn slope_for(dn: f64) -> f64 {
    if dn <= 75.0 { SMALL_SLOPE } else { LARGE_SLOPE }
}

/// An L-shaped run, along X and then Y.
fn l_path(from: Point, to: Point) -> Vec<Point> {
    let corner = Point::new(to.x, from.y);
    let mut path = vec![from];
    if corner.distance(from) > 1e-6 && corner.distance(to) > 1e-6 {
        path.push(corner);
    }
    if to.distance(from) > 1e-6 {
        path.push(to);
    }
    path
}

fn length(path: &[Point]) -> f64 {
    path.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// The point just outside the building's box, on its side nearest `from`.
fn outside(min: Point, max: Point, from: Point) -> Point {
    let sides = [
        (from.x - min.x, Point::new(min.x - OUTSIDE_M, from.y)),
        (max.x - from.x, Point::new(max.x + OUTSIDE_M, from.y)),
        (from.y - min.y, Point::new(from.x, min.y - OUTSIDE_M)),
        (max.y - from.y, Point::new(from.x, max.y + OUTSIDE_M)),
    ];
    sides
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(from, |(_, p)| p)
}

/// Where the main meets the street sewer when nobody says: on the lot's
/// front (the edge the front setback is measured from), in line with the
/// building.
fn default_exit(project: &Project, min: Point, max: Point) -> Point {
    let centre_x = (min.x + max.x) / 2.0;
    match &project.terrain {
        Some(terrain) => Point::new(
            centre_x.clamp(terrain.min.x + 0.5, terrain.max.x - 0.5),
            terrain.min.y,
        ),
        None => Point::new(centre_x, min.y - 3.0),
    }
}

/// A node of the buried main: a box and the pipe to the box after it.
struct MainNode {
    at: Point,
    device: Option<usize>,
    uhc: f64,
    parent: Option<usize>,
    path: Vec<Point>,
}

/// The sewage design, or `None` when no fixture drains.
pub fn sewage(project: &Project) -> Option<Sewage> {
    let points = plumbing_points(project);
    let drains: Vec<&PlumbingPoint> = points
        .points
        .iter()
        .filter(|p| p.kind == PointKind::Drain)
        .collect();
    if drains.is_empty() {
        return None;
    }
    let rules = DrainageRules::for_project(project);
    let table = FixtureTable::for_project(project);
    let uhc_of = |p: &PlumbingPoint| table.figures(p.fixture).map_or(1.0, |f| f.uhc);

    let corners: Vec<Point> = project
        .walls
        .iter()
        .flat_map(|w| [w.start, w.end])
        .chain(drains.iter().map(|p| p.at))
        .collect();
    let min = corners
        .iter()
        .fold(corners[0], |m, p| Point::new(m.x.min(p.x), m.y.min(p.y)));
    let max = corners
        .iter()
        .fold(corners[0], |m, p| Point::new(m.x.max(p.x), m.y.max(p.y)));

    // One group per room.
    let mut groups: Vec<(Option<String>, Vec<&PlumbingPoint>)> = Vec::new();
    for point in &drains {
        let key = point
            .room
            .clone()
            .or_else(|| Some(format!("#{}", point.fixture_id)));
        match groups.iter_mut().find(|(room, _)| *room == key) {
            Some(group) => group.1.push(point),
            None => groups.push((key, vec![point])),
        }
    }

    let mut out_drains: Vec<Drain> = Vec::new();
    let mut devices: Vec<Device> = Vec::new();
    let mut main: Vec<MainNode> = Vec::new();
    let count = |kind: DeviceKind, devices: &Vec<Device>| {
        devices.iter().filter(|d| d.kind == kind).count() + 1
    };
    let push_drain = |drains: &mut Vec<Drain>, role, path: Vec<Point>, dn: f64, uhc: f64| {
        let length_m = length(&path);
        if length_m > 1e-6 {
            drains.push(Drain {
                role,
                path,
                dn,
                slope: slope_for(dn),
                uhc,
                length_m,
            });
        }
    };

    for (room, pts) in &groups {
        let room_name = room.clone().filter(|r| !r.starts_with('#'));
        let toilet = pts.iter().find(|p| p.fixture == FixtureKind::Toilet);
        let sinks: Vec<_> = pts
            .iter()
            .filter(|p| p.fixture == FixtureKind::KitchenSink)
            .collect();
        let trapped: Vec<_> = pts
            .iter()
            .filter(|p| !matches!(p.fixture, FixtureKind::Toilet | FixtureKind::KitchenSink))
            .collect();
        let group_uhc: f64 = pts.iter().map(|p| uhc_of(p)).sum();

        // The floor trap and the drains to it.
        let mut trap = None;
        if !trapped.is_empty() {
            let at = trapped
                .iter()
                .find(|p| p.fixture == FixtureKind::Shower)
                .map(|p| p.at)
                .unwrap_or_else(|| {
                    let n = trapped.len() as f64;
                    let s = trapped.iter().fold(Point::new(0.0, 0.0), |a, p| {
                        Point::new(a.x + p.at.x, a.y + p.at.y)
                    });
                    Point::new(s.x / n, s.y / n)
                });
            let trap_uhc: f64 = trapped.iter().map(|p| uhc_of(p)).sum();
            let dn = rules.branch_dn(trap_uhc).max(50.0);
            for p in &trapped {
                push_drain(
                    &mut out_drains,
                    DrainRole::Fixture,
                    l_path(p.at, at),
                    p.diameter_mm,
                    uhc_of(p),
                );
            }
            let n = count(DeviceKind::FloorTrap, &devices);
            devices.push(Device {
                kind: DeviceKind::FloorTrap,
                label: format!("CS-{n}"),
                at,
                room: room_name.clone(),
                dn,
                depth_m: None,
            });
            trap = Some((at, trap_uhc, dn));
        }

        // The room's way out.
        let anchor = toilet
            .map(|p| p.at)
            .or(trap.map(|t| t.0))
            .unwrap_or(pts[0].at);
        let exit = outside(min, max, anchor);
        let wet_uhc: f64 = group_uhc - sinks.iter().map(|p| uhc_of(p)).sum::<f64>();
        if let Some(toilet) = toilet {
            if let Some((at, uhc, dn)) = trap {
                push_drain(
                    &mut out_drains,
                    DrainRole::Branch,
                    l_path(at, toilet.at),
                    dn,
                    uhc,
                );
            }
            let dn = rules.branch_dn(wet_uhc).max(100.0);
            push_drain(
                &mut out_drains,
                DrainRole::Branch,
                l_path(toilet.at, exit),
                dn,
                wet_uhc,
            );
            let n = count(DeviceKind::Vent, &devices);
            devices.push(Device {
                kind: DeviceKind::Vent,
                label: format!("TV-{n}"),
                at: toilet.at,
                room: room_name.clone(),
                dn: rules.vent_dn(wet_uhc, true),
                depth_m: None,
            });
        } else if let Some((at, uhc, dn)) = trap {
            push_drain(
                &mut out_drains,
                DrainRole::Branch,
                l_path(at, exit),
                dn,
                uhc,
            );
        }
        if wet_uhc > 0.0 {
            let n = count(DeviceKind::InspectionBox, &devices);
            devices.push(Device {
                kind: DeviceKind::InspectionBox,
                label: format!("CI-{n}"),
                at: exit,
                room: room_name.clone(),
                dn: rules.main_min_dn,
                depth_m: None,
            });
            main.push(MainNode {
                at: exit,
                device: Some(devices.len() - 1),
                uhc: wet_uhc,
                parent: None,
                path: Vec::new(),
            });
        }
        // The kitchen sink: to a grease trap outside.
        if !sinks.is_empty() {
            let sink_uhc: f64 = sinks.iter().map(|p| uhc_of(p)).sum();
            let grease_at = outside(min, max, sinks[0].at);
            let grease_at = if wet_uhc > 0.0 && grease_at.distance(exit) < 0.8 {
                Point::new(grease_at.x + 0.9, grease_at.y)
            } else {
                grease_at
            };
            for p in &sinks {
                push_drain(
                    &mut out_drains,
                    DrainRole::Fixture,
                    l_path(p.at, grease_at),
                    p.diameter_mm,
                    uhc_of(p),
                );
            }
            devices.push(Device {
                kind: DeviceKind::GreaseTrap,
                label: format!("CG-{}", count(DeviceKind::GreaseTrap, &devices)),
                at: grease_at,
                room: room_name.clone(),
                dn: 75.0,
                depth_m: None,
            });
            main.push(MainNode {
                at: grease_at,
                device: Some(devices.len() - 1),
                uhc: sink_uhc,
                parent: None,
                path: Vec::new(),
            });
        }
    }
    let total_uhc: f64 = drains.iter().map(|p| uhc_of(p)).sum();

    // The outfall.
    let septic_at = project.plumbing.septic.as_ref().and_then(|s| s.tank_at);
    let outfall = if project.plumbing.septic.is_some() {
        Outfall::Septic
    } else {
        Outfall::PublicSewer
    };
    let exit_at = match (outfall, septic_at) {
        (Outfall::Septic, Some(at)) => at,
        _ => project
            .plumbing
            .sewer_exit
            .unwrap_or_else(|| default_exit(project, min, max)),
    };
    // The last box, a metre inside the lot from the street (or the tank).
    let towards = Point::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0);
    let last_at = match exit_at.minus(towards).normalized() {
        Some(dir) if outfall == Outfall::PublicSewer => {
            Point::new(exit_at.x - dir.x * 1.0, exit_at.y - dir.y * 1.0)
        }
        _ => exit_at,
    };
    let mut nodes: Vec<MainNode> = vec![MainNode {
        at: last_at,
        device: None,
        uhc: 0.0,
        parent: None,
        path: Vec::new(),
    }];
    if outfall == Outfall::PublicSewer {
        let n = devices
            .iter()
            .filter(|d| d.kind == DeviceKind::InspectionBox)
            .count()
            + 1;
        devices.push(Device {
            kind: DeviceKind::InspectionBox,
            label: format!("CI-{n}"),
            at: last_at,
            room: None,
            dn: rules.main_min_dn,
            depth_m: None,
        });
        nodes[0].device = Some(devices.len() - 1);
    }
    main.sort_by(|a, b| a.at.distance(last_at).total_cmp(&b.at.distance(last_at)));
    for node in main {
        let target = (0..nodes.len())
            .min_by(|a, b| {
                nodes[*a]
                    .at
                    .distance(node.at)
                    .total_cmp(&nodes[*b].at.distance(node.at))
            })
            .unwrap_or(0);
        let path = l_path(node.at, nodes[target].at);
        // A box at the turn of a buried pipe.
        let mut parent = target;
        if path.len() == 3 {
            let corner = path[1];
            let n = devices
                .iter()
                .filter(|d| d.kind == DeviceKind::InspectionBox)
                .count()
                + 1;
            devices.push(Device {
                kind: DeviceKind::InspectionBox,
                label: format!("CI-{n}"),
                at: corner,
                room: None,
                dn: rules.main_min_dn,
                depth_m: None,
            });
            nodes.push(MainNode {
                at: corner,
                device: Some(devices.len() - 1),
                uhc: 0.0,
                parent: Some(target),
                path: vec![corner, nodes[target].at],
            });
            parent = nodes.len() - 1;
        }
        let at = node.at;
        nodes.push(MainNode {
            parent: Some(parent),
            path: vec![at, nodes[parent].at],
            ..node
        });
    }
    // Boxes along runs longer than the standard allows.
    let mut index = 1;
    while index < nodes.len() {
        let run = length(&nodes[index].path);
        if run > MAX_BOX_SPACING_M + 1e-6
            && let Some(parent) = nodes[index].parent
        {
            let (from, to) = (nodes[index].at, nodes[parent].at);
            let mid = from.lerp(to, 0.5);
            let n = devices
                .iter()
                .filter(|d| d.kind == DeviceKind::InspectionBox)
                .count()
                + 1;
            devices.push(Device {
                kind: DeviceKind::InspectionBox,
                label: format!("CI-{n}"),
                at: mid,
                room: None,
                dn: rules.main_min_dn,
                depth_m: None,
            });
            nodes.push(MainNode {
                at: mid,
                device: Some(devices.len() - 1),
                uhc: 0.0,
                parent: Some(parent),
                path: vec![mid, to],
            });
            let new = nodes.len() - 1;
            nodes[index].parent = Some(new);
            nodes[index].path = vec![from, mid];
            continue;
        }
        index += 1;
    }

    // Units, sizes and depths, from the furthest boxes down.
    let n = nodes.len();
    let mut carried: Vec<f64> = nodes.iter().map(|node| node.uhc).collect();
    let mut order: Vec<usize> = (0..n).collect();
    let depth_of = |i: usize, nodes: &Vec<MainNode>| {
        let mut d = 0;
        let mut at = i;
        while let Some(p) = nodes[at].parent {
            d += 1;
            at = p;
            if d > n {
                break;
            }
        }
        d
    };
    order.sort_by_key(|i| std::cmp::Reverse(depth_of(*i, &nodes)));
    for &i in &order {
        if let Some(p) = nodes[i].parent {
            carried[p] += carried[i];
        }
    }
    let mut depth = vec![FIRST_DEPTH_M; n];
    let mut main_dn = rules.main_min_dn;
    let mut main_capacity = 0.0;
    for &i in &order {
        let Some(p) = nodes[i].parent else {
            continue;
        };
        let (dn, capacity) = rules.main_dn(carried[i], LARGE_SLOPE);
        let run = length(&nodes[i].path);
        depth[p] = depth[p].max(depth[i] + LARGE_SLOPE * run);
        if p == 0 {
            main_dn = main_dn.max(dn);
            main_capacity = capacity;
        }
        out_drains.push(Drain {
            role: DrainRole::Main,
            path: nodes[i].path.clone(),
            dn,
            slope: LARGE_SLOPE,
            uhc: carried[i],
            length_m: run,
        });
    }
    for (i, node) in nodes.iter().enumerate() {
        if let Some(device) = node.device {
            devices[device].depth_m = Some(depth[i]);
        }
    }
    let exit_depth_m = depth[0]
        + if outfall == Outfall::PublicSewer {
            LARGE_SLOPE * last_at.distance(exit_at)
        } else {
            0.0
        };
    if outfall == Outfall::PublicSewer && last_at.distance(exit_at) > 1e-6 {
        out_drains.push(Drain {
            role: DrainRole::Main,
            path: vec![last_at, exit_at],
            dn: main_dn,
            slope: LARGE_SLOPE,
            uhc: total_uhc,
            length_m: last_at.distance(exit_at),
        });
    }

    let mut warnings = Vec::new();
    for device in devices
        .iter()
        .filter(|d| d.depth_m.is_some_and(|z| z > MAX_BOX_DEPTH_M + 1e-6))
    {
        warnings.push(format!(
            "{} fica a {} m de profundidade: acima de {} m use poço de visita ou dê menos caimento onde a norma deixar.",
            device.label,
            decimal(device.depth_m.unwrap_or_default(), 2),
            decimal(MAX_BOX_DEPTH_M, 2)
        ));
    }
    if outfall == Outfall::Septic && septic_at.is_none() {
        warnings.push(
            "Sem rede de esgoto: posicione a fossa séptica (projeto_hidraulico → configurar_fossa) para o coletor chegar até ela."
                .to_owned(),
        );
    }
    let mut notes = vec![
        format!(
            "{} UHC no total; coletor DN {} com {} % de caimento ({} UHC de capacidade, NBR 8160).",
            decimal(total_uhc, 0),
            decimal(main_dn, 0),
            decimal(LARGE_SLOPE * 100.0, 0),
            decimal(main_capacity, 0)
        ),
        "Ramais até DN 75 com 2 % de caimento e os maiores com 1 %; caixas de inspeção de 60 × 60 cm a cada mudança de direção e no máximo a 25 m uma da outra.".to_owned(),
        "A água da chuva nunca vai para o esgoto (NBR 8160): calhas e ralos do quintal vão para a sarjeta ou para a cisterna.".to_owned(),
        "Caixa de gordura limpa a cada três meses; caixa sifonada e ralos sempre com água, para o cheiro não voltar.".to_owned(),
    ];
    if devices.iter().any(|d| d.kind == DeviceKind::Vent) {
        notes.push(
            "Tubo ventilador de cada banheiro sobe até 30 cm acima do telhado, longe de janelas."
                .to_owned(),
        );
    }
    match outfall {
        Outfall::PublicSewer => {
            notes.push(format!(
                "O coletor chega à rua a cerca de {} m de profundidade: a rede pública precisa estar mais funda. Confirme com a concessionária antes de fazer as caixas.",
                decimal(exit_depth_m, 2)
            ));
            if project.plumbing.sewer_exit.is_none() {
                notes.push(
                    "Ligação à rede escolhida pelo Trazo na frente do lote; ajuste-a ao ponto que a concessionária indicar."
                        .to_owned(),
                );
            }
        }
        Outfall::Septic => notes.push(format!(
            "O coletor chega à fossa séptica a cerca de {} m de profundidade.",
            decimal(exit_depth_m, 2)
        )),
    }

    Some(Sewage {
        drains: out_drains,
        devices,
        total_uhc,
        outfall,
        exit_at,
        exit_given: project.plumbing.sewer_exit.is_some(),
        exit_depth_m,
        main_dn,
        main_capacity_uhc: main_capacity,
        warnings,
        notes,
    })
}
