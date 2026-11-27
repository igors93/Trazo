//! A water network laid from its source (the water tank, or the heater) to
//! every point: a header (*barrilete*) and a riser (*coluna*) for each wet
//! room, a branch (*ramal*) running from fixture to fixture and a drop
//! (*sub-ramal*) to each outlet. Each pipe carries the probable flow of the
//! fixtures after it (NBR 5626, `Q = c·√ΣP`), is the smallest PVC pipe that
//! keeps the water under the speed limit, and loses head by Darcy-Weisbach
//! with the fittings' equivalent lengths. The pressure left at each outlet
//! is the height of the source over it, less those losses.

use crate::domain::fixtures::FixtureKind;
use crate::domain::geometry::Point;

use super::points::PlumbingPoint;
use super::rules::{ColdWaterRules, FixtureTable};

/// Standard gravity, m/s²: kPa = m of water × 9.80665.
pub const GRAVITY: f64 = 9.80665;
/// Kinematic viscosity of water at 20 °C, m²/s.
const WATER_VISCOSITY: f64 = 1.01e-6;
/// Pressure an electric shower needs for a good bath, metres of water.
/// Below it the shower runs, but weakly.
pub const SHOWER_COMFORT_M: f64 = 2.0;
/// The branches run this far above the highest outlet of their room, m.
const BRANCH_ABOVE_OUTLET_M: f64 = 0.10;

/// A point of the network: plan position and height above the floor, m.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub at: Point,
    pub z: f64,
}

impl Node {
    pub fn new(at: Point, z: f64) -> Self {
        Self { at, z }
    }
}

/// What a pipe does in the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipeRole {
    /// Barrilete: from the source along the top to a riser.
    Header,
    /// Coluna: down (or up) to the branches of a room.
    Riser,
    /// Ramal: along the walls from fixture to fixture.
    Branch,
    /// Sub-ramal: from the branch to one outlet.
    Drop,
}

impl PipeRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Header => "Barrilete",
            Self::Riser => "Coluna",
            Self::Branch => "Ramal",
            Self::Drop => "Sub-ramal",
        }
    }

    /// The least pipe of the role, DN.
    fn least_dn(self) -> f64 {
        match self {
            Self::Header | Self::Riser => 25.0,
            Self::Branch | Self::Drop => 20.0,
        }
    }

    /// The largest pipe the sizing will try when it fights for pressure.
    fn largest_dn(self) -> f64 {
        match self {
            Self::Header | Self::Riser => 50.0,
            Self::Branch | Self::Drop => 32.0,
        }
    }
}

/// A fitting, by the column the equivalent-length table gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fitting {
    Elbow90,
    TeeThrough,
    TeeSide,
    /// Registro de gaveta.
    Gate,
    /// Registro de pressão (globe valve), at the shower.
    Globe,
}

impl Fitting {
    fn key(self) -> &'static str {
        match self {
            Self::Elbow90 => "elbow90",
            Self::TeeThrough => "tee_through",
            Self::TeeSide => "tee_side",
            Self::Gate => "gate",
            Self::Globe => "globe",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Elbow90 => "joelho 90°",
            Self::TeeThrough => "tê de passagem direta",
            Self::TeeSide => "tê de saída lateral",
            Self::Gate => "registro de gaveta",
            Self::Globe => "registro de pressão",
        }
    }
}

/// One pipe, sized.
#[derive(Clone, Debug, PartialEq)]
pub struct Pipe {
    pub role: PipeRole,
    /// The room (group) it serves, an index into `Network::groups`.
    pub group: usize,
    pub path: Vec<Node>,
    /// Sum of the weights of the fixtures after it.
    pub weight: f64,
    /// Probable flow, L/s.
    pub flow_l_s: f64,
    pub dn: f64,
    pub inner_mm: f64,
    pub speed_m_s: f64,
    /// Real length, m.
    pub length_m: f64,
    pub fittings: Vec<Fitting>,
    /// Length the fittings add, m.
    pub equivalent_m: f64,
    /// Head lost along it, m of water.
    pub loss_m: f64,
}

impl Pipe {
    fn new(
        role: PipeRole,
        group: usize,
        path: Vec<Node>,
        weight: f64,
        fittings: Vec<Fitting>,
    ) -> Self {
        let length_m = path
            .windows(2)
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                (a.at.distance(b.at).powi(2) + (a.z - b.z).powi(2)).sqrt()
            })
            .sum();
        Self {
            role,
            group,
            path,
            weight,
            flow_l_s: 0.0,
            dn: role.least_dn(),
            inner_mm: 0.0,
            speed_m_s: 0.0,
            length_m,
            fittings,
            equivalent_m: 0.0,
            loss_m: 0.0,
        }
    }

    /// Flow, speed and loss for its current DN.
    fn compute(&mut self, rules: &ColdWaterRules) {
        self.flow_l_s = rules.flow_coefficient * self.weight.max(0.0).sqrt();
        self.inner_mm = rules.inner_mm(self.dn);
        let d = self.inner_mm / 1000.0;
        let area = std::f64::consts::PI * d * d / 4.0;
        self.speed_m_s = if area > 0.0 {
            self.flow_l_s / 1000.0 / area
        } else {
            0.0
        };
        self.equivalent_m = self
            .fittings
            .iter()
            .map(|f| rules.equivalent_m(self.dn, f.key()))
            .sum();
        self.loss_m = loss_per_m(self.speed_m_s, d, rules.roughness / 1000.0)
            * (self.length_m + self.equivalent_m);
    }

    /// The smallest DN that keeps the speed under the limit.
    fn size(&mut self, rules: &ColdWaterRules) {
        let dns = rules.dns();
        let least = self.role.least_dn();
        for dn in dns.iter().copied().filter(|d| *d >= least - 1e-9) {
            self.dn = dn;
            self.compute(rules);
            if self.speed_m_s <= rules.max_speed + 1e-9 {
                return;
            }
        }
    }

    /// One commercial size up, if the role allows it.
    fn grow(&mut self, rules: &ColdWaterRules) -> bool {
        let Some(next) = rules
            .dns()
            .into_iter()
            .find(|d| *d > self.dn + 1e-9 && *d <= self.role.largest_dn() + 1e-9)
        else {
            return false;
        };
        self.dn = next;
        self.compute(rules);
        true
    }
}

/// Head loss per metre of pipe (Darcy-Weisbach, Swamee-Jain friction).
fn loss_per_m(speed: f64, d: f64, roughness: f64) -> f64 {
    if speed <= 0.0 || d <= 0.0 {
        return 0.0;
    }
    let re = speed * d / WATER_VISCOSITY;
    let f = if re < 2300.0 {
        64.0 / re
    } else {
        0.25 / (roughness / (3.7 * d) + 5.74 / re.powf(0.9))
            .log10()
            .powi(2)
    };
    f / d * speed * speed / (2.0 * GRAVITY)
}

/// How much pressure an outlet has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressure {
    Good,
    /// A shower below the comfort pressure: it runs, but weakly.
    Weak,
    /// Below the least pressure of NBR 5626.
    Low,
}

impl Pressure {
    pub fn label(self) -> &'static str {
        match self {
            Self::Good => "boa",
            Self::Weak => "fraca",
            Self::Low => "insuficiente",
        }
    }
}

/// One outlet and the pressure left there.
#[derive(Clone, Debug, PartialEq)]
pub struct Outlet {
    pub fixture_id: u64,
    pub fixture: FixtureKind,
    pub room: Option<String>,
    pub node: Node,
    /// The pipes from the source, in order.
    pub path: Vec<usize>,
    /// Height of the source over the outlet, m.
    pub static_m: f64,
    pub loss_m: f64,
    /// Dynamic pressure left, m of water.
    pub pressure_m: f64,
    pub state: Pressure,
}

impl Outlet {
    /// The pressure it should have: the least, or the shower's comfort.
    pub fn target_m(&self, min_m: f64) -> f64 {
        if self.fixture == FixtureKind::Shower {
            min_m.max(SHOWER_COMFORT_M)
        } else {
            min_m
        }
    }
}

/// The pipes of one room.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// "AF-1", "AQ-2"…
    pub label: String,
    pub room: Option<String>,
    pub riser_at: Point,
}

/// The network from one source.
#[derive(Clone, Debug, PartialEq)]
pub struct Network {
    pub source: Node,
    pub pipes: Vec<Pipe>,
    pub outlets: Vec<Outlet>,
    pub groups: Vec<Group>,
    /// Least pressure at a point of use, m of water.
    pub min_m: f64,
}

impl Network {
    /// The height the source needs so that every outlet gets the least
    /// pressure, and every shower its comfort when `comfort` is set.
    pub fn needed_source_z(&self, comfort: bool) -> f64 {
        self.outlets
            .iter()
            .map(|o| {
                let target = if comfort {
                    o.target_m(self.min_m)
                } else {
                    self.min_m
                };
                o.node.z + target + o.loss_m
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Pipe length by DN, m, smallest DN first.
    pub fn lengths_by_dn(&self) -> Vec<(f64, f64)> {
        let mut out: Vec<(f64, f64)> = Vec::new();
        for pipe in &self.pipes {
            match out.iter_mut().find(|(dn, _)| (*dn - pipe.dn).abs() < 1e-9) {
                Some(entry) => entry.1 += pipe.length_m,
                None => out.push((pipe.dn, pipe.length_m)),
            }
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out
    }

    fn update_outlets(&mut self) {
        for outlet in &mut self.outlets {
            outlet.loss_m = outlet.path.iter().map(|i| self.pipes[*i].loss_m).sum();
            outlet.pressure_m = outlet.static_m - outlet.loss_m;
            outlet.state = if outlet.pressure_m < self.min_m - 1e-9 {
                Pressure::Low
            } else if outlet.pressure_m < outlet.target_m(self.min_m) - 1e-9 {
                Pressure::Weak
            } else {
                Pressure::Good
            };
        }
    }
}

fn manhattan(a: Point, b: Point) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// An L-shaped run along X, then along Y, at height `z`; the turns it makes.
fn l_path(from: Point, to: Point, z: f64) -> (Vec<Node>, usize) {
    let corner = Point::new(to.x, from.y);
    let mut path = vec![Node::new(from, z)];
    let mut turns = 0;
    if corner.distance(from) > 1e-6 && corner.distance(to) > 1e-6 {
        path.push(Node::new(corner, z));
        turns = 1;
    }
    if to.distance(from) > 1e-6 {
        path.push(Node::new(to, z));
    }
    (path, turns)
}

/// Lays and sizes the network from `source` to the `points` (one per
/// outlet), labelling each room's riser `prefix-1`, `prefix-2`… The water
/// leaves the source with the pressure of a free surface at `head_z` (the
/// tank's base for cold water; the tank, less the heater's loss, for a gas
/// heater on a wall).
pub fn build_network(
    points: &[&PlumbingPoint],
    source: Node,
    head_z: f64,
    prefix: &str,
    table: &FixtureTable,
    rules: &ColdWaterRules,
) -> Network {
    let weight = |p: &PlumbingPoint| table.figures(p.fixture).map_or(0.3, |f| f.weight);
    // One group per room; a fixture outside every room stands alone.
    let mut rooms: Vec<(Option<String>, Option<u64>, Vec<&PlumbingPoint>)> = Vec::new();
    for point in points {
        let key = (
            point.room.clone(),
            point.room.is_none().then_some(point.fixture_id),
        );
        match rooms
            .iter_mut()
            .find(|(room, id, _)| (room.clone(), *id) == key)
        {
            Some(group) => group.2.push(point),
            None => rooms.push((key.0, key.1, vec![point])),
        }
    }
    let nearest = |pts: &[&PlumbingPoint]| {
        pts.iter()
            .map(|p| manhattan(p.at, source.at))
            .fold(f64::INFINITY, f64::min)
    };
    rooms.sort_by(|a, b| nearest(&a.2).total_cmp(&nearest(&b.2)));

    let mut network = Network {
        source,
        pipes: Vec::new(),
        outlets: Vec::new(),
        groups: Vec::new(),
        min_m: rules.min_use_m(),
    };
    let several = rooms.len() > 1;
    for (index, (room, _, pts)) in rooms.into_iter().enumerate() {
        let riser = pts
            .iter()
            .min_by(|a, b| manhattan(a.at, source.at).total_cmp(&manhattan(b.at, source.at)))
            .map_or(source.at, |p| p.at);
        let z_branch = pts.iter().map(|p| p.height_m).fold(0.3, f64::max) + BRANCH_ABOVE_OUTLET_M;
        let room_weight: f64 = pts.iter().map(|p| weight(p)).sum();
        network.groups.push(Group {
            label: format!("{prefix}-{}", index + 1),
            room: room.clone(),
            riser_at: riser,
        });

        let (header, turns) = l_path(source.at, riser, source.z);
        let mut fittings = vec![
            if several {
                Fitting::TeeSide
            } else {
                Fitting::Elbow90
            },
            Fitting::Gate,
        ];
        fittings.extend(std::iter::repeat_n(Fitting::Elbow90, turns));
        let mut trunk = Vec::new();
        if header.len() > 1 {
            network.pipes.push(Pipe::new(
                PipeRole::Header,
                index,
                header,
                room_weight,
                fittings,
            ));
            trunk.push(network.pipes.len() - 1);
        }
        network.pipes.push(Pipe::new(
            PipeRole::Riser,
            index,
            vec![Node::new(riser, source.z), Node::new(riser, z_branch)],
            room_weight,
            vec![Fitting::Elbow90],
        ));
        trunk.push(network.pipes.len() - 1);

        let mut ordered = pts.clone();
        ordered.sort_by(|a, b| manhattan(a.at, riser).total_cmp(&manhattan(b.at, riser)));
        let mut previous = riser;
        let count = ordered.len();
        for (k, point) in ordered.iter().enumerate() {
            let downstream: f64 = ordered[k..].iter().map(|p| weight(p)).sum();
            let (branch, turns) = l_path(previous, point.at, z_branch);
            let mut branch_fittings = vec![if k + 1 < count {
                Fitting::TeeThrough
            } else {
                Fitting::Elbow90
            }];
            if k == 0 {
                // The room's own valve.
                branch_fittings.push(Fitting::Gate);
            }
            branch_fittings.extend(std::iter::repeat_n(Fitting::Elbow90, turns));
            let mut drop_fittings = vec![Fitting::Elbow90];
            if k + 1 < count {
                drop_fittings.push(Fitting::TeeSide);
            }
            if point.fixture == FixtureKind::Shower {
                drop_fittings.push(Fitting::Globe);
            }
            if branch.len() > 1 {
                network.pipes.push(Pipe::new(
                    PipeRole::Branch,
                    index,
                    branch,
                    downstream,
                    branch_fittings,
                ));
                trunk.push(network.pipes.len() - 1);
            } else {
                drop_fittings.extend(branch_fittings);
            }
            let node = Node::new(point.at, point.height_m);
            network.pipes.push(Pipe::new(
                PipeRole::Drop,
                index,
                vec![Node::new(point.at, z_branch), node],
                weight(point),
                drop_fittings,
            ));
            let mut path = trunk.clone();
            path.push(network.pipes.len() - 1);
            network.outlets.push(Outlet {
                fixture_id: point.fixture_id,
                fixture: point.fixture,
                room: point.room.clone(),
                node,
                path,
                static_m: head_z - point.height_m,
                loss_m: 0.0,
                pressure_m: 0.0,
                state: Pressure::Good,
            });
            previous = point.at;
        }
    }

    for pipe in &mut network.pipes {
        pipe.size(rules);
    }
    network.update_outlets();
    improve_pressures(&mut network, rules);
    network
}

/// Grows the pipe that loses the most on the path of the weakest outlet
/// that a larger pipe can still help, until every outlet that can reach
/// its pressure does.
fn improve_pressures(network: &mut Network, rules: &ColdWaterRules) {
    let min_m = network.min_m;
    let mut stuck: Vec<usize> = Vec::new();
    for _ in 0..60 {
        let Some(outlet) = network
            .outlets
            .iter()
            .enumerate()
            .filter(|(i, o)| {
                !stuck.contains(i)
                    && o.pressure_m < o.target_m(min_m) - 1e-9
                    && o.static_m > o.target_m(min_m)
            })
            .min_by(|a, b| a.1.pressure_m.total_cmp(&b.1.pressure_m))
            .map(|(i, _)| i)
        else {
            return;
        };
        let path = network.outlets[outlet].path.clone();
        let mut candidates: Vec<usize> = path.clone();
        candidates.sort_by(|a, b| {
            network.pipes[*b]
                .loss_m
                .total_cmp(&network.pipes[*a].loss_m)
        });
        let grown = candidates.into_iter().any(|i| network.pipes[i].grow(rules));
        if !grown {
            stuck.push(outlet);
        }
        network.update_outlets();
    }
}
