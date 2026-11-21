//! Conduits (*eletrodutos*) between electrical devices, and the wires each
//! run carries (#88).
//!
//! A conduit joins two devices. Together the conduits form a tree rooted at
//! the distribution board: walking it tells which circuits pass through each
//! run, and with which conductors (phase, neutral, switch returns, earth).
//! On the plan a conduit is drawn, as in Brazilian practice, as a gentle arc
//! between the two points (solid in the ceiling or wall, dashed in the
//! floor), with one tick per conductor and the circuit number beside them.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use super::electrical::DeviceRole;
use super::fixtures::{Fixture, FixtureKind};
use super::geometry::Point;
use super::model::Project;

/// Where a conduit runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConduitRun {
    /// Embedded in the ceiling slab or down a wall: drawn solid.
    #[default]
    Ceiling,
    /// Embedded in the floor: drawn dashed.
    Floor,
}

impl ConduitRun {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ceiling => "Teto/parede",
            Self::Floor => "Piso",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Ceiling => "teto",
            Self::Floor => "piso",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "teto" | "parede" | "teto_parede" => Some(Self::Ceiling),
            "piso" => Some(Self::Floor),
            _ => None,
        }
    }

    fn is_ceiling(&self) -> bool {
        *self == Self::Ceiling
    }
}

/// A conduit between two electrical devices.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conduit {
    pub id: u64,
    pub from: u64,
    pub to: u64,
    #[serde(default, skip_serializing_if = "ConduitRun::is_ceiling")]
    pub run: ConduitRun,
}

/// A conductor in a conduit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conductor {
    Phase,
    Neutral,
    /// A switch return (or a traveller between two-way switches).
    Return,
    /// The protective conductor (PE).
    Earth,
}

impl Conductor {
    pub const ALL: [Conductor; 4] = [
        Conductor::Phase,
        Conductor::Neutral,
        Conductor::Return,
        Conductor::Earth,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Phase => "Fase",
            Self::Neutral => "Neutro",
            Self::Return => "Retorno",
            Self::Earth => "Terra (proteção)",
        }
    }
}

/// The wires one circuit has in one conduit.
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitWires {
    pub circuit: u32,
    pub phases: u8,
    pub neutral: bool,
    pub returns: u8,
    pub earth: bool,
    pub section_mm2: f64,
}

impl CircuitWires {
    /// The conductors in drawing order: phases, neutral, returns, earth.
    pub fn conductors(&self) -> Vec<Conductor> {
        let mut out = vec![Conductor::Phase; usize::from(self.phases)];
        if self.neutral {
            out.push(Conductor::Neutral);
        }
        out.extend(std::iter::repeat_n(
            Conductor::Return,
            usize::from(self.returns),
        ));
        if self.earth {
            out.push(Conductor::Earth);
        }
        out
    }

    pub fn count(&self) -> usize {
        self.conductors().len()
    }

    /// "F N T", "F R"...
    pub fn short(&self) -> String {
        self.conductors()
            .iter()
            .map(|c| match c {
                Conductor::Phase => "F",
                Conductor::Neutral => "N",
                Conductor::Return => "R",
                Conductor::Earth => "T",
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// What runs through one conduit.
#[derive(Clone, Debug, PartialEq)]
pub struct ConduitWiring {
    pub conduit: u64,
    /// The device on the board's side, when the conduit reaches the board.
    pub upstream: u64,
    pub downstream: u64,
    pub run: ConduitRun,
    /// Straight length between the two points, in metres.
    pub length_m: f64,
    pub circuits: Vec<CircuitWires>,
    /// Nominal diameter in millimetres; `None` when even the largest is
    /// too small.
    pub diameter_mm: Option<u32>,
}

impl ConduitWiring {
    pub fn conductor_count(&self) -> usize {
        self.circuits.iter().map(CircuitWires::count).sum()
    }
}

/// The conduits as a tree from the board.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConduitTree {
    /// The main board the tree starts from.
    pub panel: Option<u64>,
    /// Secondary boards (QDC) present in the plan, in placement order.
    pub sub_panels: Vec<u64>,
    /// Serving board (main or sub-board) for each reached device.
    pub board_of: HashMap<u64, u64>,
    /// Each reached device with its parent (towards the board) and the
    /// conduit to it.
    pub parent: HashMap<u64, (u64, u64)>,
    /// Horizontal length along the conduits from the board, in metres.
    pub distance: HashMap<u64, f64>,
    /// Devices of the installation with no conduit path to the board.
    pub unreached: Vec<u64>,
    /// Conduits that close a ring (left out of the tree, no wires).
    pub rings: Vec<u64>,
}

/// Typical external diameter of a 750 V PVC-insulated copper wire, in mm.
pub fn wire_diameter_mm(section_mm2: f64) -> f64 {
    const TABLE: [(f64, f64); 9] = [
        (1.5, 3.0),
        (2.5, 3.6),
        (4.0, 4.2),
        (6.0, 4.8),
        (10.0, 6.1),
        (16.0, 7.2),
        (25.0, 8.9),
        (35.0, 10.1),
        (50.0, 11.9),
    ];
    TABLE
        .iter()
        .find(|(section, _)| *section + 1e-9 >= section_mm2)
        .map_or(13.5, |(_, diameter)| *diameter)
}

/// Nominal sizes of corrugated PVC conduit with their usual internal
/// diameter, in mm. 20 mm (¾″) is the usual minimum for embedded runs.
pub const CONDUIT_SIZES: [(u32, f64); 5] =
    [(20, 15.4), (25, 19.0), (32, 24.6), (40, 31.4), (50, 40.0)];

/// The smallest conduit for these wire sections, keeping the NBR 5410
/// (6.2.11.1.6) fill limits: 53 % for one conductor, 31 % for two, 40 % for
/// three or more.
pub fn conduit_size(sections_mm2: &[f64]) -> Option<u32> {
    if sections_mm2.is_empty() {
        return Some(CONDUIT_SIZES[0].0);
    }
    let fill = match sections_mm2.len() {
        1 => 0.53,
        2 => 0.31,
        _ => 0.40,
    };
    let area: f64 = sections_mm2
        .iter()
        .map(|section| {
            let d = wire_diameter_mm(*section);
            std::f64::consts::PI * d * d / 4.0
        })
        .sum();
    CONDUIT_SIZES
        .iter()
        .find(|(_, inner)| std::f64::consts::PI * inner * inner / 4.0 * fill + 1e-9 >= area)
        .map(|(nominal, _)| *nominal)
}

/// A primitive the conduit drawing is made of, in plan coordinates (m).
#[derive(Clone, Debug, PartialEq)]
pub enum PlanMark {
    Line {
        points: Vec<Point>,
        dashed: bool,
        /// The conduit itself (heavier) rather than a wire tick.
        heavy: bool,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    /// A short label centred on `anchor`, drawn like the device tags.
    Text {
        anchor: Point,
        text: String,
    },
}

/// Whether a device is part of the power installation (data points have
/// conduits of their own, outside the board).
fn is_power_device(fixture: &Fixture) -> bool {
    fixture
        .kind
        .role()
        .is_some_and(|role| role != DeviceRole::Data)
}

/// Points along the arc drawn for a conduit from `a` to `b`.
fn arc_points(a: Point, b: Point) -> Vec<Point> {
    let length = a.distance(b);
    let Some(dir) = b.minus(a).normalized() else {
        return vec![a, b];
    };
    // Start and end just off the symbols.
    let trim = (length * 0.2).min(0.12);
    let (a, b) = (
        a.translated(dir.scaled(trim)),
        b.translated(dir.scaled(-trim)),
    );
    let normal = Point::new(-dir.y, dir.x);
    let sagitta = (length * 0.12).min(0.3);
    let control = a.midpoint(b).translated(normal.scaled(2.0 * sagitta));
    (0..=16)
        .map(|i| {
            let t = f64::from(i) / 16.0;
            let u = 1.0 - t;
            Point::new(
                u * u * a.x + 2.0 * u * t * control.x + t * t * b.x,
                u * u * a.y + 2.0 * u * t * control.y + t * t * b.y,
            )
        })
        .collect()
}

/// The point at arc length `s` along `points`, with the unit tangent there.
fn along(points: &[Point], s: f64) -> (Point, Point) {
    let mut left = s.max(0.0);
    let mut last = (points[0], Point::new(1.0, 0.0));
    for pair in points.windows(2) {
        let step = pair[0].distance(pair[1]);
        let tangent = pair[1]
            .minus(pair[0])
            .normalized()
            .unwrap_or(Point::new(1.0, 0.0));
        if left <= step {
            return (pair[0].lerp(pair[1], left / step.max(1e-9)), tangent);
        }
        left -= step;
        last = (pair[1], tangent);
    }
    last
}

/// "1–4, 7, 9–10" for a sorted list of circuit numbers.
fn circuit_ranges(numbers: &[u32]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut index = 0;
    while index < numbers.len() {
        let start = numbers[index];
        let mut end = start;
        while index + 1 < numbers.len() && numbers[index + 1] == end + 1 {
            index += 1;
            end = numbers[index];
        }
        parts.push(if end > start + 1 {
            format!("{start}–{end}")
        } else if end == start + 1 {
            format!("{start}, {end}")
        } else {
            start.to_string()
        });
        index += 1;
    }
    parts.join(", ")
}

fn polyline_length(points: &[Point]) -> f64 {
    points
        .windows(2)
        .map(|pair| pair[0].distance(pair[1]))
        .sum()
}

/// One wire tick across the conduit at `p`, with its end mark.
fn tick(marks: &mut Vec<PlanMark>, p: Point, tangent: Point, conductor: Conductor) {
    let normal = Point::new(-tangent.y, tangent.x);
    let (h, k) = (0.08, 0.035);
    let low = p
        .translated(normal.scaled(-h))
        .translated(tangent.scaled(-k));
    let top = p.translated(normal.scaled(h)).translated(tangent.scaled(k));
    marks.push(PlanMark::Line {
        points: vec![low, top],
        dashed: false,
        heavy: false,
    });
    match conductor {
        // A flag at the top.
        Conductor::Phase => marks.push(PlanMark::Line {
            points: vec![top, top.translated(tangent.scaled(0.05))],
            dashed: false,
            heavy: false,
        }),
        Conductor::Neutral => {}
        // A small ring at the top.
        Conductor::Return => marks.push(PlanMark::Circle {
            center: top.translated(normal.scaled(0.02)),
            radius: 0.02,
        }),
        // A bar across the top, like a T.
        Conductor::Earth => marks.push(PlanMark::Line {
            points: vec![
                top.translated(tangent.scaled(-0.035)),
                top.translated(tangent.scaled(0.035)),
            ],
            dashed: false,
            heavy: false,
        }),
    }
}

impl Project {
    /// The conduits whose two ends are electrical devices of the plan.
    pub fn valid_conduits(&self) -> impl Iterator<Item = &Conduit> {
        self.conduits.iter().filter(|conduit| {
            conduit.from != conduit.to
                && [conduit.from, conduit.to].iter().all(|id| {
                    self.fixtures
                        .iter()
                        .any(|fixture| fixture.id == *id && fixture.kind.is_electrical())
                })
        })
    }

    /// Drops conduits left without one of their devices.
    pub fn prune_conduits(&mut self) -> usize {
        let before = self.conduits.len();
        let valid: HashSet<u64> = self.valid_conduits().map(|conduit| conduit.id).collect();
        self.conduits.retain(|conduit| valid.contains(&conduit.id));
        before - self.conduits.len()
    }

    fn device(&self, id: u64) -> Option<&Fixture> {
        self.fixtures.iter().find(|fixture| fixture.id == id)
    }

    /// The distribution board the installation starts from.
    pub fn main_panel(&self) -> Option<&Fixture> {
        self.fixtures
            .iter()
            .find(|fixture| fixture.kind == FixtureKind::Panel)
    }

    /// All distribution boards in the plan: the first is the main board (QDG)
    /// and any subsequent ones are sub-boards (QDC1, QDC2…).
    pub fn panels(&self) -> Vec<&Fixture> {
        self.fixtures
            .iter()
            .filter(|fixture| fixture.kind == FixtureKind::Panel)
            .collect()
    }

    /// Label of a distribution board (`QD` when there is only one; `QDG` and
    /// `QDC1`, `QDC2`, … when sub-boards exist).
    pub fn panel_label(&self, panel_id: u64) -> Option<String> {
        let panels = self.panels();
        let index = panels.iter().position(|p| p.id == panel_id)?;
        if panels.len() == 1 {
            Some("QD".to_owned())
        } else if index == 0 {
            Some("QDG".to_owned())
        } else {
            Some(format!("QDC{index}"))
        }
    }

    /// Joins two devices with a conduit. Returns its id.
    pub fn add_conduit(&mut self, from: u64, to: u64, run: ConduitRun) -> Result<u64, String> {
        if from == to {
            return Err("Escolha dois pontos diferentes.".to_owned());
        }
        for id in [from, to] {
            if !self
                .device(id)
                .is_some_and(|fixture| fixture.kind.is_electrical())
            {
                return Err(format!("O id {id} não é um ponto elétrico."));
            }
        }
        if self.valid_conduits().any(|conduit| {
            (conduit.from == from && conduit.to == to) || (conduit.from == to && conduit.to == from)
        }) {
            return Err("Esses pontos já estão ligados por um eletroduto.".to_owned());
        }
        let id = self.id();
        self.conduits.push(Conduit { id, from, to, run });
        Ok(id)
    }

    pub fn delete_conduit(&mut self, id: u64) -> bool {
        let before = self.conduits.len();
        self.conduits.retain(|conduit| conduit.id != id);
        self.conduits.len() < before
    }

    pub fn set_conduit_run(&mut self, id: u64, run: ConduitRun) -> Result<(), String> {
        let conduit = self
            .conduits
            .iter_mut()
            .find(|conduit| conduit.id == id)
            .ok_or("Eletroduto não encontrado.")?;
        conduit.run = run;
        Ok(())
    }

    /// Conduits touching any of `devices`.
    pub fn conduits_of(&self, devices: &[u64]) -> Vec<u64> {
        self.valid_conduits()
            .filter(|conduit| devices.contains(&conduit.from) || devices.contains(&conduit.to))
            .map(|conduit| conduit.id)
            .collect()
    }

    /// Walks the conduits from the main board, tracking sub-boards (QDC) so
    /// each device knows which board serves it.
    pub fn conduit_tree(&self) -> ConduitTree {
        let panels = self.panels();
        let main_id = panels.first().map(|panel| panel.id);
        let sub_panels: Vec<u64> = panels.iter().skip(1).map(|panel| panel.id).collect();
        let mut tree = ConduitTree {
            panel: main_id,
            sub_panels,
            ..ConduitTree::default()
        };
        let mut links: HashMap<u64, Vec<(u64, u64)>> = HashMap::new();
        for conduit in self.valid_conduits() {
            links
                .entry(conduit.from)
                .or_default()
                .push((conduit.to, conduit.id));
            links
                .entry(conduit.to)
                .or_default()
                .push((conduit.from, conduit.id));
        }
        let devices: HashMap<u64, &Fixture> = self
            .fixtures
            .iter()
            .filter(|f| f.kind.is_electrical())
            .map(|f| (f.id, f))
            .collect();
        let mut used: HashSet<u64> = HashSet::new();
        if let Some(panel) = tree.panel {
            tree.distance.insert(panel, 0.0);
            tree.board_of.insert(panel, panel);
            let mut queue = VecDeque::from([panel]);
            while let Some(node) = queue.pop_front() {
                let device = devices.get(&node).copied();
                let here = device.map(|d| d.center);
                let base = tree.distance[&node];
                let child_board = if device.is_some_and(|d| d.kind == FixtureKind::Panel) {
                    node
                } else {
                    tree.board_of.get(&node).copied().unwrap_or(panel)
                };
                for &(next, conduit) in links.get(&node).map(Vec::as_slice).unwrap_or(&[]) {
                    if used.contains(&conduit) {
                        continue;
                    }
                    used.insert(conduit);
                    if tree.distance.contains_key(&next) {
                        tree.rings.push(conduit);
                        continue;
                    }
                    let step = match (here, devices.get(&next)) {
                        (Some(a), Some(b)) => a.distance(b.center),
                        _ => 0.0,
                    };
                    tree.distance.insert(next, base + step);
                    tree.parent.insert(next, (node, conduit));
                    tree.board_of.insert(next, child_board);
                    queue.push_back(next);
                }
            }
        }
        tree.unreached = self
            .fixtures
            .iter()
            .filter(|fixture| {
                is_power_device(fixture)
                    && Some(fixture.id) != main_id
                    && !tree.distance.contains_key(&fixture.id)
            })
            .map(|fixture| fixture.id)
            .collect();
        tree
    }

    /// Serving board id for `device`: its upstream board in `tree`, or the
    /// closest board in the plan when no conduits reach it yet.
    pub fn serving_board(&self, tree: &ConduitTree, device: &Fixture) -> Option<u64> {
        if device.kind == FixtureKind::Panel {
            return Some(device.id);
        }
        if let Some(&board) = tree.board_of.get(&device.id) {
            return Some(board);
        }
        self.panels()
            .into_iter()
            .min_by(|a, b| {
                a.center
                    .distance(device.center)
                    .total_cmp(&b.center.distance(device.center))
            })
            .map(|panel| panel.id)
    }

    /// Cable length from the serving board (main or sub-board) to a device,
    /// in metres: along the conduits (or square to the axes when it has none),
    /// plus the drops from the ceiling to the board and to the device. `None`
    /// without a board.
    pub fn cable_length(&self, tree: &ConduitTree, device: &Fixture) -> Option<f64> {
        let board_id = if device.kind == FixtureKind::Panel {
            // Feeder length to a sub-board comes from its upstream board.
            tree.board_of.get(&device.id).copied().or(tree.panel)?
        } else {
            self.serving_board(tree, device)?
        };
        let panel = self.device(board_id)?;
        let drop = |fixture: &Fixture| {
            if fixture.kind.mounts_on_wall() {
                let height = fixture.electrical.map_or(0.0, |data| data.height_m);
                (self.wall_height - height).abs()
            } else {
                0.0
            }
        };
        let horizontal = match (tree.distance.get(&device.id), tree.distance.get(&board_id)) {
            (Some(&dev_d), Some(&board_d)) => (dev_d - board_d).max(0.0),
            _ => {
                (device.center.x - panel.center.x).abs() + (device.center.y - panel.center.y).abs()
            }
        };
        Some(horizontal + drop(panel) + drop(device))
    }

    /// The wires in every conduit, given the section of each circuit's wire
    /// and how many phases it takes (2 for 220 V between phases).
    pub fn conduit_wiring_with(
        &self,
        tree: &ConduitTree,
        sections: &HashMap<u32, f64>,
        poles: &HashMap<u32, u8>,
    ) -> Vec<ConduitWiring> {
        // Subtree of each device: itself and everything fed through it.
        let mut below: HashMap<u64, HashSet<u64>> = HashMap::new();
        for node in tree.distance.keys() {
            let mut current = *node;
            below.entry(current).or_default().insert(*node);
            while let Some((parent, _)) = tree.parent.get(&current) {
                below.entry(*parent).or_default().insert(*node);
                current = *parent;
            }
        }
        let devices: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|fixture| is_power_device(fixture))
            .collect();
        let circuit_of = |fixture: &Fixture| fixture.electrical.and_then(|data| data.circuit);
        let room_of: HashMap<u64, Option<u64>> = devices
            .iter()
            .map(|fixture| {
                (
                    fixture.id,
                    self.room_for_fixture(fixture).map(|room| room.id),
                )
            })
            .collect();

        // Who commands each light: switches and lights of the same circuit
        // and room, each light on its nearest control. Two-way (three-way)
        // and intermediate (four-way) switches form a switching chain:
        // phase enters `chain[0]`, each consecutive link carries two
        // travellers, and `chain.last()` returns to the commanded lights.
        struct Control {
            chain: Vec<u64>,
            lights: Vec<u64>,
        }
        let mut controls: Vec<(u32, Control)> = Vec::new();
        let mut commanded: HashSet<u64> = HashSet::new();
        let mut groups: Vec<(u32, Option<u64>)> = devices
            .iter()
            .filter(|d| d.kind.role() == Some(DeviceRole::Switch))
            .filter_map(|d| Some((circuit_of(d)?, room_of[&d.id])))
            .collect();
        groups.sort_unstable();
        groups.dedup();
        let build_chain = |start: &Fixture, mids: &[&Fixture], end: &Fixture| -> Vec<u64> {
            let mut chain = Vec::with_capacity(mids.len() + 2);
            chain.push(start.id);
            let mut remaining: Vec<&Fixture> = mids.to_vec();
            let mut current = start.center;
            while !remaining.is_empty() {
                let next_idx = remaining
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        a.center
                            .distance(current)
                            .total_cmp(&b.center.distance(current))
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let picked = remaining.remove(next_idx);
                chain.push(picked.id);
                current = picked.center;
            }
            chain.push(end.id);
            chain
        };
        for (circuit, room) in groups {
            let in_group = |d: &&&Fixture| circuit_of(d) == Some(circuit) && room_of[&d.id] == room;
            let switches: Vec<&Fixture> = devices
                .iter()
                .filter(in_group)
                .filter(|d| d.kind.role() == Some(DeviceRole::Switch))
                .copied()
                .collect();
            let lights: Vec<&Fixture> = devices
                .iter()
                .filter(in_group)
                .filter(|d| d.kind.role() == Some(DeviceRole::Lighting))
                .copied()
                .collect();
            let two_way: Vec<&Fixture> = switches
                .iter()
                .filter(|d| d.kind == FixtureKind::SwitchTwoWay)
                .copied()
                .collect();
            let intermediates: Vec<&Fixture> = switches
                .iter()
                .filter(|d| d.kind == FixtureKind::SwitchIntermediate)
                .copied()
                .collect();
            let one_way: Vec<&Fixture> = switches
                .iter()
                .filter(|d| d.kind == FixtureKind::Switch)
                .copied()
                .collect();
            let mut group: Vec<Control> = Vec::new();
            if two_way.len() >= 2 {
                if intermediates.is_empty()
                    && two_way.len().is_multiple_of(2)
                    && lights.len() >= two_way.len() / 2
                {
                    // Multiple independent two-way pairs for separate lighting groups.
                    for pair in two_way.as_chunks::<2>().0 {
                        group.push(Control {
                            chain: vec![pair[0].id, pair[1].id],
                            lights: Vec::new(),
                        });
                    }
                } else {
                    // Two-way ends with any intermediate switches (plus any extra
                    // two-way switches in the same group) wired in series.
                    let mut mids = intermediates;
                    if two_way.len() > 2 {
                        mids.extend_from_slice(&two_way[2..]);
                    }
                    group.push(Control {
                        chain: build_chain(two_way[0], &mids, two_way[1]),
                        lights: Vec::new(),
                    });
                }
            } else {
                let mut multi: Vec<&Fixture> =
                    Vec::with_capacity(two_way.len() + intermediates.len());
                multi.extend_from_slice(&two_way);
                multi.extend_from_slice(&intermediates);
                if multi.len() >= 2 {
                    let first = multi[0];
                    let last = *multi.last().unwrap_or(&first);
                    let mids = &multi[1..multi.len() - 1];
                    group.push(Control {
                        chain: build_chain(first, mids, last),
                        lights: Vec::new(),
                    });
                } else if let Some(&lone) = multi.first() {
                    group.push(Control {
                        chain: vec![lone.id],
                        lights: Vec::new(),
                    });
                }
            }
            group.extend(one_way.iter().map(|d| Control {
                chain: vec![d.id],
                lights: Vec::new(),
            }));
            for light in lights {
                let nearest = group.iter_mut().min_by(|a, b| {
                    let at = |control: &Control| {
                        let out_id = *control.chain.last().unwrap_or(&control.chain[0]);
                        self.device(out_id)
                            .map_or(f64::MAX, |d| d.center.distance(light.center))
                    };
                    at(a).total_cmp(&at(b))
                });
                if let Some(control) = nearest {
                    control.lights.push(light.id);
                    commanded.insert(light.id);
                }
            }
            controls.extend(group.into_iter().map(|control| (circuit, control)));
        }

        let mut circuits: Vec<u32> = devices.iter().filter_map(|d| circuit_of(d)).collect();
        circuits.sort_unstable();
        circuits.dedup();

        let mut out = Vec::new();
        for conduit in self.valid_conduits() {
            let a = self.device(conduit.from).map(|d| d.center);
            let b = self.device(conduit.to).map(|d| d.center);
            let length_m = match (a, b) {
                (Some(a), Some(b)) => a.distance(b),
                _ => 0.0,
            };
            // Oriented from the board's side, when the tree reaches it.
            let (upstream, downstream) =
                match (tree.parent.get(&conduit.to), tree.parent.get(&conduit.from)) {
                    (Some((parent, id)), _) if *id == conduit.id && *parent == conduit.from => {
                        (conduit.from, conduit.to)
                    }
                    (_, Some((parent, id))) if *id == conduit.id && *parent == conduit.to => {
                        (conduit.to, conduit.from)
                    }
                    _ => (conduit.from, conduit.to),
                };
            let in_tree = tree.parent.get(&downstream).map(|(_, id)| *id) == Some(conduit.id);
            let empty = HashSet::new();
            let side = if in_tree {
                below.get(&downstream).unwrap_or(&empty)
            } else {
                &empty
            };
            let inside = |id: u64| side.contains(&id);
            let mut wires = Vec::new();
            for circuit in &circuits {
                let members: Vec<&&Fixture> = devices
                    .iter()
                    .filter(|d| circuit_of(d) == Some(*circuit))
                    .collect();
                let load_inside = members.iter().any(|d| {
                    matches!(
                        d.kind.role(),
                        Some(
                            DeviceRole::Lighting
                                | DeviceRole::GeneralOutlet
                                | DeviceRole::SpecificOutlet
                        )
                    ) && inside(d.id)
                });
                let mut phase = members.iter().any(|d| {
                    inside(d.id)
                        && match d.kind.role() {
                            Some(DeviceRole::GeneralOutlet | DeviceRole::SpecificOutlet) => true,
                            Some(DeviceRole::Lighting) => !commanded.contains(&d.id),
                            _ => false,
                        }
                });
                let mut returns = 0u8;
                for (_, control) in controls.iter().filter(|(c, _)| c == circuit) {
                    if control.lights.is_empty() || control.chain.is_empty() {
                        continue;
                    }
                    let feed = control.chain[0];
                    let out = *control.chain.last().unwrap_or(&feed);
                    phase |= inside(feed);
                    for pair in control.chain.windows(2) {
                        if inside(pair[0]) != inside(pair[1]) {
                            returns += 2;
                        }
                    }
                    if control.lights.iter().any(|l| inside(*l) != inside(out)) {
                        returns += 1;
                    }
                }
                if !(phase || load_inside || returns > 0) {
                    continue;
                }
                let two_pole = poles.get(circuit).copied().unwrap_or(1) >= 2;
                wires.push(CircuitWires {
                    circuit: *circuit,
                    phases: if phase || load_inside {
                        if two_pole { 2 } else { 1 }
                    } else {
                        0
                    },
                    neutral: load_inside && !two_pole,
                    returns,
                    earth: load_inside,
                    section_mm2: sections.get(circuit).copied().unwrap_or(2.5),
                });
            }
            let all_sections: Vec<f64> = wires
                .iter()
                .flat_map(|w| std::iter::repeat_n(w.section_mm2, w.count()))
                .collect();
            out.push(ConduitWiring {
                conduit: conduit.id,
                upstream,
                downstream,
                run: conduit.run,
                length_m,
                diameter_mm: conduit_size(&all_sections),
                circuits: wires,
            });
        }
        out
    }

    /// The wires in every conduit, with the circuits sized by the load
    /// schedule.
    pub fn conduit_wiring(&self) -> Vec<ConduitWiring> {
        let schedule = self.cached_load_schedule();
        let tree = self.conduit_tree();
        let sections = schedule
            .circuits
            .iter()
            .map(|row| (row.number, row.wire_mm2.unwrap_or(2.5)))
            .collect();
        let poles = schedule
            .circuits
            .iter()
            .map(|row| (row.number, row.poles))
            .collect();
        self.conduit_wiring_with(&tree, &sections, &poles)
    }

    /// Lays conduits to every device not yet reached from the board: the
    /// board and the ceiling lights are joined by the shortest ceiling runs
    /// (a minimum spanning tree), then each other device takes a run from
    /// the nearest light of its room (or the nearest light or board
    /// anywhere). Existing conduits stay. Returns how many were added.
    pub fn generate_conduits(&mut self) -> Result<usize, String> {
        self.prune_conduits();
        let panel = self
            .main_panel()
            .map(|panel| panel.id)
            .ok_or("Insira o quadro de distribuição (QD) antes de gerar os eletrodutos.")?;
        let tree = self.conduit_tree();
        let mut reached: HashSet<u64> = tree.distance.keys().copied().collect();
        let centre = |project: &Project, id: u64| project.device(id).map(|d| d.center);
        let room = |project: &Project, id: u64| {
            project
                .device(id)
                .and_then(|d| project.room_for_fixture(d))
                .map(|room| room.id)
        };
        let is_hub = |project: &Project, id: u64| {
            id == panel
                || project.device(id).is_some_and(|d| {
                    matches!(d.kind, FixtureKind::CeilingLight | FixtureKind::Panel)
                })
        };
        let mut added = 0;
        // Hubs: board and ceiling lights, joined by a tree that grows from
        // the board. Each step weighs the new run plus half the way back to
        // the board, so lights far along a chain do not keep feeding others:
        // several branches leave the board instead of one heavy trunk, and
        // the cables stay shorter.
        let mut depth: HashMap<u64, f64> = tree.distance.clone();
        let mut hubs: Vec<u64> = tree
            .unreached
            .iter()
            .copied()
            .filter(|id| is_hub(self, *id))
            .collect();
        while !hubs.is_empty() {
            let mut best: Option<(f64, usize, u64, f64)> = None;
            for (index, hub) in hubs.iter().enumerate() {
                let Some(at) = centre(self, *hub) else {
                    continue;
                };
                for other in reached.iter().filter(|id| is_hub(self, **id)) {
                    let Some(there) = centre(self, *other) else {
                        continue;
                    };
                    let distance = at.distance(there);
                    let back = depth.get(other).copied().unwrap_or(0.0);
                    let cost = distance + 0.5 * back;
                    if best.is_none_or(|(c, _, _, _)| cost < c) {
                        best = Some((cost, index, *other, back + distance));
                    }
                }
            }
            let Some((_, index, from, reach)) = best else {
                break;
            };
            let hub = hubs.remove(index);
            self.add_conduit(from, hub, ConduitRun::Ceiling)?;
            reached.insert(hub);
            depth.insert(hub, reach);
            added += 1;
        }
        // Everything else from the nearest hub, preferring its own room.
        let others: Vec<u64> = tree
            .unreached
            .iter()
            .copied()
            .filter(|id| !reached.contains(id))
            .collect();
        for device in others {
            let Some(at) = centre(self, device) else {
                continue;
            };
            let own_room = room(self, device);
            let candidates: Vec<u64> = reached
                .iter()
                .copied()
                .filter(|id| is_hub(self, *id))
                .collect();
            let nearest = |pool: &mut dyn Iterator<Item = u64>| {
                pool.filter_map(|id| Some((centre(self, id)?.distance(at), id)))
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, id)| id)
            };
            // A light of the device's own room, else the nearest hub.
            let in_room =
                nearest(&mut candidates.iter().copied().filter(|id| {
                    own_room.is_some() && *id != panel && room(self, *id) == own_room
                }));
            let Some(from) = in_room.or_else(|| nearest(&mut candidates.iter().copied())) else {
                continue;
            };
            self.add_conduit(from, device, ConduitRun::Ceiling)?;
            reached.insert(device);
            added += 1;
        }
        Ok(added)
    }

    /// The conduits as drawn on the plan: arcs, wire ticks, circuit numbers
    /// and the diameter where it is above the minimum.
    pub fn conduit_marks(&self) -> Vec<PlanMark> {
        let wiring = self.conduit_wiring();
        let device_by_id: HashMap<u64, &Fixture> =
            self.fixtures.iter().map(|f| (f.id, f)).collect();
        let mut marks = Vec::new();
        for run in &wiring {
            let (Some(&a), Some(&b)) = (
                device_by_id.get(&run.upstream),
                device_by_id.get(&run.downstream),
            ) else {
                continue;
            };
            let points = arc_points(a.center, b.center);
            let length = polyline_length(&points);
            marks.push(PlanMark::Line {
                points: points.clone(),
                dashed: run.run == ConduitRun::Floor,
                heavy: true,
            });
            let ticks: usize = run.conductor_count();
            let spacing = 0.06;
            let gap = 0.16;
            let needed = ticks as f64 * spacing + run.circuits.len().saturating_sub(1) as f64 * gap;
            let (middle, tangent) = along(&points, length / 2.0);
            let normal = Point::new(-tangent.y, tangent.x);
            let diameter = match run.diameter_mm {
                Some(20) => None,
                Some(d) => Some(format!("Ø{d}")),
                None => Some("Ø>50".to_owned()),
            };
            // A label beside the arc: pushed out by half its width when the
            // arc runs up and down, so the text does not cover it.
            let beside = |text: String, side: f64| {
                let (width, _) = super::electrical::tag_size(&text);
                let reach = 0.2 + normal.x.abs() * width / 2.0;
                PlanMark::Text {
                    anchor: middle.translated(normal.scaled(side * reach)),
                    text,
                }
            };
            if ticks > 0 && needed <= length * 0.75 {
                let mut s = (length - needed) / 2.0;
                for wires in &run.circuits {
                    let start = s;
                    for conductor in wires.conductors() {
                        let (p, t) = along(&points, s + spacing / 2.0);
                        tick(&mut marks, p, t, conductor);
                        s += spacing;
                    }
                    let (p, t) = along(&points, (start + s) / 2.0);
                    let n = Point::new(-t.y, t.x);
                    marks.push(PlanMark::Text {
                        anchor: p.translated(n.scaled(0.22)),
                        text: wires.circuit.to_string(),
                    });
                    s += gap;
                }
                if let Some(diameter) = diameter {
                    marks.push(beside(diameter, -1.0));
                }
            } else if ticks > 0 {
                // Too many wires to tick: circuits, count and size in one label.
                let numbers: Vec<u32> = run.circuits.iter().map(|w| w.circuit).collect();
                let mut text = format!("{} · {ticks} fios", circuit_ranges(&numbers));
                if let Some(diameter) = diameter {
                    text = format!("{text} · {diameter}");
                }
                marks.push(beside(text, 1.0));
            } else if let Some(diameter) = diameter {
                marks.push(beside(diameter, -1.0));
            }
        }
        let tag_obstacles: Vec<super::label_layout::Area> = self
            .cached_electrical_tags()
            .iter()
            .map(|(anchor, text)| {
                super::label_layout::Area::centered(
                    (anchor.x, anchor.y),
                    super::electrical::tag_size(text),
                )
            })
            .collect();
        super::label_layout::resolve_plan_mark_labels(&mut marks, &tag_obstacles);
        marks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::OpeningKind;

    /// A room with a board, a light, a switch by the door and two outlets.
    fn wired_room() -> (Project, [u64; 5]) {
        let mut project = Project::default();
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala")
            .unwrap();
        let add = |kind: FixtureKind, x: f64, y: f64, project: &mut Project| {
            let (w, d) = kind.default_size();
            project
                .add_fixture(kind, Point::new(x, y), w, d, 0)
                .unwrap()
        };
        let panel = add(FixtureKind::Panel, 0.5, 0.2, &mut project);
        let light = add(FixtureKind::CeilingLight, 2.5, 2.0, &mut project);
        let switch = add(FixtureKind::Switch, 1.2, 0.2, &mut project);
        let outlet = add(FixtureKind::OutletLow, 4.0, 0.2, &mut project);
        let outlet2 = add(FixtureKind::OutletLow, 4.5, 3.8, &mut project);
        for (id, circuit) in [(light, 1), (switch, 1), (outlet, 2), (outlet2, 2)] {
            project
                .update_electrical(id, Some(Some(circuit)), None, None)
                .unwrap();
        }
        (project, [panel, light, switch, outlet, outlet2])
    }

    #[test]
    fn crowded_runs_list_their_circuits_as_ranges() {
        assert_eq!(circuit_ranges(&[1, 2, 3, 4, 7, 9, 10]), "1–4, 7, 9, 10");
        assert_eq!(circuit_ranges(&[5]), "5");
    }

    #[test]
    fn conduits_are_sized_by_the_fill_limits() {
        // Three 2.5 mm² wires fit a 20 mm conduit.
        assert_eq!(conduit_size(&[2.5, 2.5, 2.5]), Some(20));
        // Twelve of them need more.
        assert!(conduit_size(&[2.5; 12]).unwrap() > 20);
        // Far too many for any size.
        assert_eq!(conduit_size(&[50.0; 12]), None);
    }

    #[test]
    fn generated_conduits_reach_every_device_from_the_board() {
        let (mut project, [panel, light, switch, outlet, _]) = wired_room();
        assert!(project.conduit_tree().unreached.len() == 4);
        let added = project.generate_conduits().unwrap();
        assert_eq!(added, 4);
        let tree = project.conduit_tree();
        assert!(tree.unreached.is_empty());
        assert_eq!(tree.parent[&light].0, panel);
        assert_eq!(tree.parent[&switch].0, light);
        assert_eq!(tree.parent[&outlet].0, light);
        // Running again adds nothing.
        assert_eq!(project.generate_conduits().unwrap(), 0);
        // Without a board there is nothing to start from.
        let mut empty = Project::default();
        assert!(empty.generate_conduits().is_err());
    }

    #[test]
    fn each_run_carries_the_wires_of_the_circuits_beyond_it() {
        let (mut project, [panel, light, switch, outlet, _]) = wired_room();
        project.generate_conduits().unwrap();
        let wiring = project.conduit_wiring();
        let run = |a: u64, b: u64| {
            wiring
                .iter()
                .find(|w| w.upstream == a && w.downstream == b)
                .unwrap()
        };
        let wires = |w: &ConduitWiring, c: u32| {
            w.circuits
                .iter()
                .find(|x| x.circuit == c)
                .map(CircuitWires::short)
        };
        // Board to light: the lighting circuit (phase for the switch,
        // neutral and earth for the light) and the outlet circuit.
        let feed = run(panel, light);
        assert_eq!(wires(feed, 1).as_deref(), Some("F N T"));
        assert_eq!(wires(feed, 2).as_deref(), Some("F N T"));
        // Light to switch: phase down, return up.
        let to_switch = run(light, switch);
        assert_eq!(wires(to_switch, 1).as_deref(), Some("F R"));
        assert_eq!(wires(to_switch, 2), None);
        // Light to an outlet: only that circuit.
        let to_outlet = run(light, outlet);
        assert_eq!(wires(to_outlet, 2).as_deref(), Some("F N T"));
        assert_eq!(wires(to_outlet, 1), None);
        assert_eq!(to_outlet.diameter_mm, Some(20));
        // The plan shows the arcs, the ticks and the circuit numbers.
        let marks = project.conduit_marks();
        let heavy = marks
            .iter()
            .filter(|m| matches!(m, PlanMark::Line { heavy: true, .. }))
            .count();
        assert_eq!(heavy, 4);
        assert!(
            marks
                .iter()
                .any(|m| matches!(m, PlanMark::Text { text, .. } if text == "1"))
        );
    }

    #[test]
    fn deleting_a_device_drops_its_conduits_and_floor_runs_are_kept() {
        let (mut project, [_, light, _, outlet, _]) = wired_room();
        project.generate_conduits().unwrap();
        let id = project.conduits_of(&[outlet])[0];
        project.set_conduit_run(id, ConduitRun::Floor).unwrap();
        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("\"run\":\"floor\""));
        assert!(
            project
                .add_conduit(light, outlet, ConduitRun::Ceiling)
                .is_err()
        );
        // Deleting a device takes its conduit with it.
        project.delete_fixture(outlet);
        assert_eq!(project.conduits.len(), 3);
        // Removed any other way, the pruning on each edit catches it.
        let switch = project
            .fixtures
            .iter()
            .find(|f| f.kind == FixtureKind::Switch)
            .unwrap()
            .id;
        project.fixtures.retain(|f| f.id != switch);
        assert_eq!(project.prune_conduits(), 1);
        assert_eq!(project.conduits.len(), 2);
    }

    #[test]
    fn two_way_switches_carry_travellers_between_them() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(6.0, 3.0), "Corredor")
            .unwrap();
        let wall = project.walls[0].id;
        let _ = project.add_opening(wall, OpeningKind::Door, 0.5);
        let mut add = |kind: FixtureKind, x: f64, y: f64| {
            let (w, d) = kind.default_size();
            let id = project
                .add_fixture(kind, Point::new(x, y), w, d, 0)
                .unwrap();
            if kind != FixtureKind::Panel {
                project
                    .update_electrical(id, Some(Some(1)), None, None)
                    .unwrap();
            }
            id
        };
        let panel = add(FixtureKind::Panel, 0.3, 0.2);
        let light = add(FixtureKind::CeilingLight, 3.0, 1.5);
        let a = add(FixtureKind::SwitchTwoWay, 0.8, 0.2);
        let b = add(FixtureKind::SwitchTwoWay, 5.5, 0.2);
        for (from, to) in [(panel, light), (light, a), (light, b)] {
            project.add_conduit(from, to, ConduitRun::Ceiling).unwrap();
        }
        let wiring = project.conduit_wiring();
        let to = |id: u64| wiring.iter().find(|w| w.downstream == id).unwrap();
        // Phase down to the first, two travellers back up; travellers down
        // to the second and the return up to the light.
        assert_eq!(to(a).circuits[0].short(), "F R R");
        assert_eq!(to(b).circuits[0].short(), "R R R");
    }

    #[test]
    fn intermediate_switches_carry_four_travellers_and_chain_multiple_points() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(8.0, 3.0), "Corredor")
            .unwrap();
        let mut add = |kind: FixtureKind, x: f64, y: f64| {
            let (w, d) = kind.default_size();
            let id = project
                .add_fixture(kind, Point::new(x, y), w, d, 0)
                .unwrap();
            if kind != FixtureKind::Panel {
                project
                    .update_electrical(id, Some(Some(1)), None, None)
                    .unwrap();
            }
            id
        };
        let panel = add(FixtureKind::Panel, 0.3, 0.2);
        let light = add(FixtureKind::CeilingLight, 4.0, 1.5);
        let a = add(FixtureKind::SwitchTwoWay, 0.8, 0.2);
        let mid = add(FixtureKind::SwitchIntermediate, 4.0, 0.2);
        let b = add(FixtureKind::SwitchTwoWay, 7.4, 0.2);
        for (from, to) in [(panel, light), (light, a), (light, mid), (light, b)] {
            project.add_conduit(from, to, ConduitRun::Ceiling).unwrap();
        }
        let wiring = project.conduit_wiring();
        let to = |id: u64| wiring.iter().find(|w| w.downstream == id).unwrap();
        // Feed two-way gets phase + 2 travellers, intermediate gets 4 travellers
        // (two in, two out), exit two-way gets 2 travellers + 1 return to the light.
        assert_eq!(to(a).circuits[0].short(), "F R R");
        assert_eq!(to(mid).circuits[0].short(), "R R R R");
        assert_eq!(to(b).circuits[0].short(), "R R R");
    }
}
