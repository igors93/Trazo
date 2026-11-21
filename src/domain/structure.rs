//! Structure on the floor plan: columns (*pilares*) and footings (*sapatas*)
//! as fixtures, beams (*vigas*) between columns and slabs (*lajes*) over the
//! rooms, drawn as a formwork plan (*planta de forma*) on a layer of their
//! own (#92); pre-sizing by the usual rules of thumb, with warnings (#93);
//! and the analysis of each continuous beam: loads, bending moments, shear,
//! reactions and deflection (#94).
//!
//! Columns and footings are fixtures, so they select, move, copy, rotate,
//! undo and export like any other object. Beams name the two columns they
//! join, so they follow them, and slabs name their room.
//!
//! Everything here is pre-sizing for a first layout: the final design is a
//! structural engineer's job (NBR 6118).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::conduits::PlanMark;
use super::fixtures::{Fixture, FixtureKind, SymbolShape, Weight};
use super::geometry::Point;
use super::model::Project;

/// Reinforced concrete unit weight, kN/m³.
pub const CONCRETE_KN_M3: f64 = 25.0;

/// A beam between two columns.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Beam {
    pub id: u64,
    pub from: u64,
    pub to: u64,
    /// Section width (b), metres.
    pub width_m: f64,
    /// Section height (h), metres.
    pub height_m: f64,
}

/// How a slab is built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlabKind {
    /// Precast lattice joists with blocks (*laje treliçada*).
    #[default]
    Ribbed,
    /// Solid cast-in-place slab (*laje maciça*).
    Solid,
}

impl SlabKind {
    pub const ALL: [SlabKind; 2] = [SlabKind::Ribbed, SlabKind::Solid];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ribbed => "Treliçada",
            Self::Solid => "Maciça",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Ribbed => "trelicada",
            Self::Solid => "macica",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "trelicada" | "treliçada" | "nervurada" => Some(Self::Ribbed),
            "macica" | "maciça" => Some(Self::Solid),
            _ => None,
        }
    }

    /// Self weight of a slab of thickness `h`, kN/m² (a lattice slab with
    /// fillers weighs about 55 % of a solid one).
    pub fn self_weight(self, h: f64) -> f64 {
        match self {
            Self::Ribbed => CONCRETE_KN_M3 * h * 0.55,
            Self::Solid => CONCRETE_KN_M3 * h,
        }
    }

    /// The usual minimum thickness for the smaller span `lx`, metres.
    pub fn minimum_thickness(self, lx: f64) -> f64 {
        match self {
            Self::Solid => round_up(lx / 40.0, 0.01).max(0.08),
            Self::Ribbed => {
                if lx <= 3.5 {
                    0.12
                } else if lx <= 4.5 {
                    0.16
                } else if lx <= 5.5 {
                    0.20
                } else if lx <= 6.5 {
                    0.25
                } else {
                    0.30
                }
            }
        }
    }
}

/// A slab over a room.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slab {
    pub id: u64,
    pub room_id: u64,
    #[serde(default)]
    pub kind: SlabKind,
    pub thickness_m: f64,
}

/// Materials and loads for pre-sizing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StructureSettings {
    /// Concrete strength, MPa.
    pub fck_mpa: f64,
    /// Allowable soil pressure, kPa (200 kPa = 0.2 MPa).
    pub soil_kpa: f64,
    /// Floors the columns carry (each with its slab, beams and walls).
    pub floors: u32,
    /// Live load on the slabs, kN/m² (NBR 6120: 1.5 for dwellings).
    pub live_kn_m2: f64,
    /// Floor finishes and screed, kN/m².
    pub finish_kn_m2: f64,
    /// Masonry unit weight, kN/m³ (about 13 for hollow ceramic blocks).
    pub masonry_kn_m3: f64,
}

impl Default for StructureSettings {
    fn default() -> Self {
        Self {
            fck_mpa: 25.0,
            soil_kpa: 200.0,
            floors: 1,
            live_kn_m2: 1.5,
            finish_kn_m2: 1.0,
            masonry_kn_m3: 13.0,
        }
    }
}

impl StructureSettings {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Secant modulus of the concrete, kN/m² (NBR 6118: 0.85 · 5600 √fck).
    pub fn modulus_kn_m2(&self) -> f64 {
        0.85 * 5600.0 * self.fck_mpa.max(10.0).sqrt() * 1000.0
    }
}

/// The structural elements that are not fixtures, and their settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectStructure {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beams: Vec<Beam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slabs: Vec<Slab>,
    #[serde(default, skip_serializing_if = "StructureSettings::is_default")]
    pub settings: StructureSettings,
}

impl ProjectStructure {
    pub fn is_empty(&self) -> bool {
        self.beams.is_empty() && self.slabs.is_empty() && self.settings.is_default()
    }
}

fn round_up(value: f64, step: f64) -> f64 {
    (value / step - 1e-9).ceil() * step
}

/// "14×30" from metres.
pub fn section_cm(b: f64, h: f64) -> String {
    format!("{:.0}×{:.0}", b * 100.0, h * 100.0)
}

/// Reading order on the plan: top to bottom, then left to right.
fn reading(a: Point, b: Point) -> Ordering {
    let row = |p: Point| (p.y / 0.3).round() as i64;
    row(a).cmp(&row(b)).then(a.x.total_cmp(&b.x))
}

impl FixtureKind {
    pub const STRUCTURAL: [FixtureKind; 2] = [FixtureKind::Column, FixtureKind::Footing];

    pub fn is_structural(self) -> bool {
        matches!(self, Self::Column | Self::Footing)
    }
}

/// The plan symbol of a column (cut, so hatched) or a footing (below the
/// floor, so dashed, with the edges of its sloped top).
pub(super) fn symbol(kind: FixtureKind, width: f64, depth: f64) -> Vec<SymbolShape> {
    let (hw, hd) = (width / 2.0, depth / 2.0);
    let corners = vec![
        Point::new(-hw, -hd),
        Point::new(hw, -hd),
        Point::new(hw, hd),
        Point::new(-hw, hd),
    ];
    match kind {
        FixtureKind::Column => {
            let mut shapes = vec![SymbolShape::Polyline {
                points: corners,
                closed: true,
                dashed: false,
                weight: Weight::Outline,
            }];
            // 45° hatching clipped to the section.
            let step = 0.035;
            let mut c = -hw - hd + step;
            while c < hw + hd {
                // Points with x - y = c inside the box.
                let x0 = (-hw).max(c - hd);
                let x1 = hw.min(c + hd);
                if x1 > x0 + 1e-6 {
                    shapes.push(SymbolShape::Polyline {
                        points: vec![Point::new(x0, x0 - c), Point::new(x1, x1 - c)],
                        closed: false,
                        dashed: false,
                        weight: Weight::Detail,
                    });
                }
                c += step;
            }
            shapes
        }
        FixtureKind::Footing => {
            let inner = 0.12_f64.min(hw * 0.5).min(hd * 0.5);
            let mut shapes = vec![SymbolShape::Polyline {
                points: corners.clone(),
                closed: true,
                dashed: true,
                weight: Weight::Outline,
            }];
            for corner in corners {
                let toward = Point::new(corner.x.signum() * inner, corner.y.signum() * inner);
                shapes.push(SymbolShape::Polyline {
                    points: vec![corner, toward],
                    closed: false,
                    dashed: true,
                    weight: Weight::Detail,
                });
            }
            shapes
        }
        _ => Vec::new(),
    }
}

/// A run of collinear beams through consecutive columns: one continuous
/// beam, labelled once.
#[derive(Clone, Debug, PartialEq)]
pub struct BeamLine {
    pub label: String,
    /// Beam ids, span by span.
    pub spans: Vec<u64>,
    /// Column ids at the supports, in order (one more than the spans).
    pub joints: Vec<u64>,
}

/// One span of a continuous beam, analysed.
#[derive(Clone, Debug, PartialEq)]
pub struct SpanResult {
    pub beam: u64,
    pub length_m: f64,
    pub width_m: f64,
    pub height_m: f64,
    /// Characteristic permanent and live load, kN/m.
    pub dead_kn_m: f64,
    pub live_kn_m: f64,
    /// Bending moments (sagging positive), kN·m: at each end and the
    /// largest along the span.
    pub moment_left: f64,
    pub moment_right: f64,
    pub moment_span: f64,
    /// Largest shear, kN.
    pub shear: f64,
    /// Long-term deflection and the L/250 limit, mm.
    pub deflection_mm: f64,
    pub limit_mm: f64,
    /// The usual minimum height for this span (L/10 at an end, L/12
    /// inside a continuous beam), metres.
    pub height_min_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamLineResult {
    pub label: String,
    pub joints: Vec<u64>,
    pub spans: Vec<SpanResult>,
    /// Support reactions, kN, one per joint.
    pub reactions: Vec<f64>,
}

/// Where a column stands in the frame, for the pre-sizing factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnPosition {
    Interior,
    Edge,
    Corner,
    /// No beam reaches it.
    Isolated,
}

impl ColumnPosition {
    pub fn label(self) -> &'static str {
        match self {
            Self::Interior => "Intermediário",
            Self::Edge => "Extremidade",
            Self::Corner => "Canto",
            Self::Isolated => "Isolado",
        }
    }

    /// Pre-sizing factor for the bending a column takes at its position.
    pub fn factor(self) -> f64 {
        match self {
            Self::Interior => 1.3,
            Self::Edge => 1.5,
            Self::Corner | Self::Isolated => 1.8,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnResult {
    pub id: u64,
    pub label: String,
    pub width_m: f64,
    pub depth_m: f64,
    pub position: ColumnPosition,
    /// Characteristic axial load at the foundation, kN.
    pub load_kn: f64,
    /// Concrete area needed and drawn, cm².
    pub area_needed_cm2: f64,
    pub area_cm2: f64,
}

impl ColumnResult {
    pub fn ok(&self) -> bool {
        self.area_cm2 + 1e-6 >= self.area_needed_cm2.max(360.0)
            && self.width_m.min(self.depth_m) >= 0.14 - 1e-9
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FootingResult {
    pub id: u64,
    pub label: String,
    pub column: Option<String>,
    pub width_m: f64,
    pub depth_m: f64,
    pub load_kn: f64,
    /// Plan size needed for the soil (along the fixture's width and depth).
    pub needed_width_m: f64,
    pub needed_depth_m: f64,
    /// Height for a rigid footing, metres.
    pub height_m: f64,
}

impl FootingResult {
    pub fn ok(&self) -> bool {
        self.width_m * self.depth_m + 1e-6 >= self.needed_width_m * self.needed_depth_m
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlabResult {
    pub id: u64,
    pub label: String,
    pub room: String,
    pub kind: SlabKind,
    pub lx_m: f64,
    pub ly_m: f64,
    pub thickness_m: f64,
    pub thickness_min_m: f64,
    /// Total characteristic load, kN/m².
    pub load_kn_m2: f64,
    /// Whether some beam carries it.
    pub supported: bool,
}

impl SlabResult {
    pub fn ok(&self) -> bool {
        self.thickness_m + 1e-6 >= self.thickness_min_m && self.supported
    }
}

/// The pre-sizing of the whole structure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StructureReport {
    pub slabs: Vec<SlabResult>,
    pub beams: Vec<BeamLineResult>,
    pub columns: Vec<ColumnResult>,
    pub footings: Vec<FootingResult>,
    pub warnings: Vec<String>,
}

/// What generating the structure added.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StructureGeneration {
    pub columns: usize,
    pub beams: usize,
    pub footings: usize,
    pub slabs: usize,
}

impl StructureGeneration {
    pub fn total(&self) -> usize {
        self.columns + self.beams + self.footings + self.slabs
    }
}

/// Solves `a · x = b` by Gaussian elimination with partial pivoting.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|i, j| a[*i][col].abs().total_cmp(&a[*j][col].abs()))?;
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let factor = a[row][col] / a[col][col];
            let (above, below) = a.split_at_mut(row);
            for (target, source) in below[0][col..].iter_mut().zip(&above[col][col..]) {
                *target -= factor * source;
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row][row];
    }
    Some(x)
}

/// End moments (sagging positive, kN·m) of a continuous beam on pinned
/// supports, by the slope-deflection (stiffness) method: `spans` gives each
/// span's length, uniform load and EI.
pub fn continuous_beam(spans: &[(f64, f64, f64)]) -> Vec<(f64, f64)> {
    let n = spans.len() + 1;
    let mut k = vec![vec![0.0; n]; n];
    let mut f = vec![0.0; n];
    for (i, (length, load, ei)) in spans.iter().enumerate() {
        let s = 2.0 * ei / length.max(1e-6);
        k[i][i] += 2.0 * s;
        k[i][i + 1] += s;
        k[i + 1][i] += s;
        k[i + 1][i + 1] += 2.0 * s;
        // Fixed-end moments, clockwise positive: -wL²/12 and +wL²/12.
        let fem = load * length * length / 12.0;
        f[i] += fem;
        f[i + 1] -= fem;
    }
    let theta = solve(k, f).unwrap_or_else(|| vec![0.0; n]);
    spans
        .iter()
        .enumerate()
        .map(|(i, (length, load, ei))| {
            let s = 2.0 * ei / length.max(1e-6);
            let fem = load * length * length / 12.0;
            let m_ab = s * (2.0 * theta[i] + theta[i + 1]) - fem;
            let m_ba = s * (theta[i] + 2.0 * theta[i + 1]) + fem;
            (m_ab, -m_ba)
        })
        .collect()
}

/// Largest deflection of a span with end moments and a uniform load, by
/// integrating the curvature twice; same units as `ei`.
pub fn span_deflection(length: f64, load: f64, left: f64, right: f64, ei: f64) -> f64 {
    let steps = 100;
    let dx = length / steps as f64;
    let shear = load * length / 2.0 + (right - left) / length.max(1e-6);
    let moment = |x: f64| left + shear * x - load * x * x / 2.0;
    let mut slope = 0.0;
    let mut deflection = vec![0.0; steps + 1];
    for i in 0..steps {
        let (x0, x1) = (i as f64 * dx, (i + 1) as f64 * dx);
        let next = slope + (moment(x0) + moment(x1)) / 2.0 / ei * dx;
        deflection[i + 1] = deflection[i] + (slope + next) / 2.0 * dx;
        slope = next;
    }
    let end = deflection[steps];
    (0..=steps)
        .map(|i| (deflection[i] - end * i as f64 / steps as f64).abs())
        .fold(0.0, f64::max)
}

impl Project {
    pub fn structural_columns(&self) -> impl Iterator<Item = &Fixture> {
        self.fixtures
            .iter()
            .filter(|fixture| fixture.kind == FixtureKind::Column)
    }

    fn column(&self, id: u64) -> Option<&Fixture> {
        self.structural_columns().find(|column| column.id == id)
    }

    /// The beams whose two ends are columns of the plan.
    pub fn valid_beams(&self) -> impl Iterator<Item = &Beam> {
        self.structure.beams.iter().filter(|beam| {
            beam.from != beam.to
                && self.column(beam.from).is_some()
                && self.column(beam.to).is_some()
        })
    }

    /// Drops beams left without a column and slabs left without a room.
    pub fn prune_structure(&mut self) -> usize {
        let before = self.structure.beams.len() + self.structure.slabs.len();
        let valid: HashSet<u64> = self.valid_beams().map(|beam| beam.id).collect();
        self.structure.beams.retain(|beam| valid.contains(&beam.id));
        let rooms: HashSet<u64> = self.rooms.iter().map(|room| room.id).collect();
        self.structure
            .slabs
            .retain(|slab| rooms.contains(&slab.room_id));
        before - self.structure.beams.len() - self.structure.slabs.len()
    }

    /// Where a column or footing lands for the pointer at `point`: on the
    /// nearest wall junction within 0.5 m, else on the nearest wall's
    /// centre line, turned along the wall.
    pub fn structural_placement(&self, point: Point) -> (Point, u8) {
        let mut best: Option<(f64, Point, u8)> = None;
        for wall in &self.walls {
            let along = wall.end.minus(wall.start);
            let turns = if along.x.abs() >= along.y.abs() { 0 } else { 1 };
            // Junctions win over a plain point on the wall.
            for end in [wall.start, wall.end] {
                let d = end.distance(point);
                let score = d - 0.2;
                if d < 0.5 && best.is_none_or(|(b, _, _)| score < b) {
                    best = Some((score, end, turns));
                }
            }
            let (on, _) = super::geometry::nearest_point_on_segment(point, wall.start, wall.end);
            let d = on.distance(point);
            if d < 0.5 && best.is_none_or(|(b, _, _)| d < b) {
                best = Some((d, on, turns));
            }
        }
        best.map_or((point, 0), |(_, at, turns)| (at, turns))
    }

    /// Joins two columns with a beam. Returns its id.
    pub fn add_beam(&mut self, from: u64, to: u64, width: f64, height: f64) -> Result<u64, String> {
        if from == to {
            return Err("Escolha dois pilares diferentes.".to_owned());
        }
        for id in [from, to] {
            if self.column(id).is_none() {
                return Err(format!("O id {id} não é um pilar."));
            }
        }
        validate_section(width, height)?;
        if self.valid_beams().any(|beam| {
            (beam.from == from && beam.to == to) || (beam.from == to && beam.to == from)
        }) {
            return Err("Esses pilares já estão ligados por uma viga.".to_owned());
        }
        let id = self.id();
        self.structure.beams.push(Beam {
            id,
            from,
            to,
            width_m: width,
            height_m: height,
        });
        Ok(id)
    }

    pub fn delete_beam(&mut self, id: u64) -> bool {
        let before = self.structure.beams.len();
        self.structure.beams.retain(|beam| beam.id != id);
        self.structure.beams.len() < before
    }

    pub fn update_beam(
        &mut self,
        id: u64,
        width: Option<f64>,
        height: Option<f64>,
    ) -> Result<(), String> {
        let beam = self
            .structure
            .beams
            .iter_mut()
            .find(|beam| beam.id == id)
            .ok_or("Viga não encontrada.")?;
        let (w, h) = (
            width.unwrap_or(beam.width_m),
            height.unwrap_or(beam.height_m),
        );
        validate_section(w, h)?;
        beam.width_m = w;
        beam.height_m = h;
        Ok(())
    }

    /// Beams touching any of `columns`.
    pub fn beams_of(&self, columns: &[u64]) -> Vec<u64> {
        self.valid_beams()
            .filter(|beam| columns.contains(&beam.from) || columns.contains(&beam.to))
            .map(|beam| beam.id)
            .collect()
    }

    /// Sets (or adds) the slab over a room.
    pub fn set_slab(
        &mut self,
        room_id: u64,
        kind: SlabKind,
        thickness: Option<f64>,
    ) -> Result<u64, String> {
        let room = self
            .rooms
            .iter()
            .find(|room| room.id == room_id)
            .ok_or("Cômodo não encontrado.")?
            .clone();
        let lx = self.slab_spans(&room).0;
        let thickness = thickness.unwrap_or_else(|| kind.minimum_thickness(lx));
        if !thickness.is_finite() || !(0.05..=0.60).contains(&thickness) {
            return Err("A espessura da laje deve ficar entre 5 e 60 cm.".to_owned());
        }
        if let Some(slab) = self
            .structure
            .slabs
            .iter_mut()
            .find(|s| s.room_id == room_id)
        {
            slab.kind = kind;
            slab.thickness_m = thickness;
            return Ok(slab.id);
        }
        let id = self.id();
        self.structure.slabs.push(Slab {
            id,
            room_id,
            kind,
            thickness_m: thickness,
        });
        Ok(id)
    }

    pub fn remove_slab(&mut self, room_id: u64) -> bool {
        let before = self.structure.slabs.len();
        self.structure.slabs.retain(|slab| slab.room_id != room_id);
        self.structure.slabs.len() < before
    }

    /// The smaller and larger span of the slab over `room` (its outline's
    /// bounding box, axis to axis), metres.
    fn slab_spans(&self, room: &super::model::Room) -> (f64, f64) {
        let Some(outline) = self.room_outline(room) else {
            return (0.0, 0.0);
        };
        let (mut min, mut max) = (outline[0], outline[0]);
        for p in &outline {
            min = Point::new(min.x.min(p.x), min.y.min(p.y));
            max = Point::new(max.x.max(p.x), max.y.max(p.y));
        }
        let (w, h) = (max.x - min.x, max.y - min.y);
        (w.min(h), w.max(h))
    }

    /// Column, beam, slab and footing labels: P1, V1, L1, S1… in reading
    /// order, footings after the column they carry.
    pub fn structure_labels(&self) -> HashMap<u64, String> {
        let mut labels = HashMap::new();
        let mut columns: Vec<&Fixture> = self.structural_columns().collect();
        columns.sort_by(|a, b| reading(a.center, b.center));
        for (i, column) in columns.iter().enumerate() {
            labels.insert(column.id, format!("P{}", i + 1));
        }
        let mut footings: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
            .collect();
        footings.sort_by(|a, b| reading(a.center, b.center));
        let mut next = columns.len() + 1;
        for footing in footings {
            let under = columns
                .iter()
                .position(|column| footing.contains(column.center))
                .map(|i| i + 1);
            let number = under.unwrap_or_else(|| {
                next += 1;
                next - 1
            });
            labels.insert(footing.id, format!("S{number}"));
        }
        for line in self.beam_lines() {
            for span in &line.spans {
                labels.insert(*span, line.label.clone());
            }
        }
        let mut slabs: Vec<(Point, u64)> = self
            .structure
            .slabs
            .iter()
            .filter_map(|slab| {
                let room = self.rooms.iter().find(|room| room.id == slab.room_id)?;
                Some((self.room_label_anchor(room)?.0, slab.id))
            })
            .collect();
        slabs.sort_by(|a, b| reading(a.0, b.0));
        for (i, (_, id)) in slabs.iter().enumerate() {
            labels.insert(*id, format!("L{}", i + 1));
        }
        labels
    }

    /// Collinear beams through shared columns, as continuous beams.
    pub fn beam_lines(&self) -> Vec<BeamLine> {
        let beams: Vec<(Beam, Point, Point)> = self
            .valid_beams()
            .filter_map(|beam| {
                Some((
                    *beam,
                    self.column(beam.from)?.center,
                    self.column(beam.to)?.center,
                ))
            })
            .collect();
        let direction = |a: Point, b: Point| {
            let d = b.minus(a).normalized().unwrap_or(Point::new(1.0, 0.0));
            if d.x > 1e-6 || (d.x.abs() <= 1e-6 && d.y > 0.0) {
                d
            } else {
                d.scaled(-1.0)
            }
        };
        // Union-find over beams joined end to end in a straight line.
        let mut parent: Vec<usize> = (0..beams.len()).collect();
        fn root(parent: &mut [usize], i: usize) -> usize {
            let mut i = i;
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for i in 0..beams.len() {
            for j in i + 1..beams.len() {
                let (a, b) = (&beams[i], &beams[j]);
                let shared = [a.0.from, a.0.to]
                    .iter()
                    .any(|id| *id == b.0.from || *id == b.0.to);
                let (da, db) = (direction(a.1, a.2), direction(b.1, b.2));
                if shared && da.cross(db).abs() < 0.02 {
                    let (ra, rb) = (root(&mut parent, i), root(&mut parent, j));
                    parent[ra] = rb;
                }
            }
        }
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..beams.len() {
            let r = root(&mut parent, i);
            groups.entry(r).or_default().push(i);
        }
        let mut lines: Vec<(u8, f64, f64, BeamLine)> = groups
            .into_values()
            .map(|members| {
                let d = direction(beams[members[0]].1, beams[members[0]].2);
                let along = |p: Point| p.dot(d);
                let mut spans: Vec<&(Beam, Point, Point)> =
                    members.iter().map(|i| &beams[*i]).collect();
                spans.sort_by(|a, b| along(a.1.midpoint(a.2)).total_cmp(&along(b.1.midpoint(b.2))));
                let mut joints: Vec<(f64, u64)> = Vec::new();
                for (beam, a, b) in &spans {
                    for (id, p) in [(beam.from, *a), (beam.to, *b)] {
                        if !joints.iter().any(|(_, j)| *j == id) {
                            joints.push((along(p), id));
                        }
                    }
                }
                joints.sort_by(|a, b| a.0.total_cmp(&b.0));
                let points: Vec<Point> = spans.iter().flat_map(|(_, a, b)| [*a, *b]).collect();
                let (sum_x, sum_y) = points
                    .iter()
                    .fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
                let n = points.len().max(1) as f64;
                let min_x = points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
                let min_y = points.iter().map(|p| p.y).fold(f64::MAX, f64::min);
                let (group, key1, key2) = if d.y.abs() < 0.1 {
                    (0, sum_y / n, min_x)
                } else if d.x.abs() < 0.1 {
                    (1, sum_x / n, min_y)
                } else {
                    (2, min_y, min_x)
                };
                (
                    group,
                    key1,
                    key2,
                    BeamLine {
                        label: String::new(),
                        spans: spans.iter().map(|(beam, _, _)| beam.id).collect(),
                        joints: joints.into_iter().map(|(_, id)| id).collect(),
                    },
                )
            })
            .collect();
        lines.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(((a.1 / 0.3).round() as i64).cmp(&((b.1 / 0.3).round() as i64)))
                .then(a.2.total_cmp(&b.2))
        });
        lines
            .into_iter()
            .enumerate()
            .map(|(i, (_, _, _, mut line))| {
                line.label = format!("V{}", i + 1);
                line
            })
            .collect()
    }

    /// Lays out a first structure over the walls: a column at every wall
    /// junction and end, more along walls longer than 4.5 m (clear of
    /// doors and windows), a beam along every wall between consecutive
    /// columns, a lattice slab over every room and a footing under every
    /// column, each sized by the pre-sizing. What exists stays.
    pub fn generate_structure(&mut self) -> StructureGeneration {
        let mut report = StructureGeneration::default();
        let max_span = 4.5;
        let near = |project: &Project, p: Point| {
            project
                .structural_columns()
                .any(|column| column.center.distance(p) < 0.3)
        };
        let walls = self.walls.clone();
        // Columns at junctions and ends.
        for wall in &walls {
            let along = wall.end.minus(wall.start);
            let turns = if along.x.abs() >= along.y.abs() { 0 } else { 1 };
            let b = round_up(wall.thickness().max(0.14), 0.01).min(0.30);
            for end in [wall.start, wall.end] {
                if !near(self, end)
                    && self
                        .add_fixture(FixtureKind::Column, end, 0.30, b, turns)
                        .is_some()
                {
                    report.columns += 1;
                }
            }
        }
        // Intermediate columns on long walls, clear of openings.
        for wall in &walls {
            let length = wall.start.distance(wall.end);
            if length <= max_span {
                continue;
            }
            let along = wall.end.minus(wall.start);
            let turns = if along.x.abs() >= along.y.abs() { 0 } else { 1 };
            let b = round_up(wall.thickness().max(0.14), 0.01).min(0.30);
            let blocked: Vec<(f64, f64)> = self
                .openings
                .iter()
                .filter(|opening| opening.wall_id == wall.id)
                .map(|opening| {
                    let s = opening.t * length;
                    let half = opening.width() / 2.0 + 0.2;
                    (s - half, s + half)
                })
                .collect();
            let pieces = (length / max_span).ceil() as usize;
            for k in 1..pieces {
                let target = length * k as f64 / pieces as f64;
                let spot = (0..30)
                    .flat_map(|i| [f64::from(i) * 0.1, -f64::from(i) * 0.1])
                    .map(|delta| target + delta)
                    .find(|s| {
                        *s > 0.5
                            && *s < length - 0.5
                            && !blocked.iter().any(|(a, z)| s > a && s < z)
                    });
                if let Some(s) = spot {
                    let at = wall.start.lerp(wall.end, s / length);
                    if !near(self, at)
                        && self
                            .add_fixture(FixtureKind::Column, at, 0.30, b, turns)
                            .is_some()
                    {
                        report.columns += 1;
                    }
                }
            }
        }
        // Beams along each wall between consecutive columns.
        for wall in &walls {
            let length = wall.start.distance(wall.end);
            let mut on: Vec<(f64, u64)> = self
                .structural_columns()
                .filter(|column| {
                    super::geometry::distance_to_segment(column.center, wall.start, wall.end) < 0.06
                })
                .map(|column| (column.center.distance(wall.start), column.id))
                .filter(|(s, _)| *s <= length + 0.06)
                .collect();
            on.sort_by(|a, b| a.0.total_cmp(&b.0));
            let b = round_up(wall.thickness().max(0.12), 0.01).min(0.30);
            for pair in on.windows(2) {
                // Columns this close act together: a beam between them
                // would only lift one off its support.
                let span = pair[1].0 - pair[0].0;
                if span < 0.6 {
                    continue;
                }
                let h = round_up(span / 10.0, 0.05).max(0.30);
                if self.add_beam(pair[0].1, pair[1].1, b, h).is_ok() {
                    report.beams += 1;
                }
            }
        }
        // Slabs.
        let rooms: Vec<u64> = self.rooms.iter().map(|room| room.id).collect();
        for room in rooms {
            if !self.structure.slabs.iter().any(|slab| slab.room_id == room)
                && self.set_slab(room, SlabKind::Ribbed, None).is_ok()
            {
                report.slabs += 1;
            }
        }
        // Footings, sized by the loads.
        let columns: Vec<Fixture> = self.structural_columns().copied().collect();
        for column in columns {
            let has = self
                .fixtures
                .iter()
                .any(|f| f.kind == FixtureKind::Footing && f.contains(column.center));
            if !has
                && self
                    .add_fixture(
                        FixtureKind::Footing,
                        column.center,
                        0.60,
                        0.60,
                        column.quarter_turns,
                    )
                    .is_some()
            {
                report.footings += 1;
            }
        }
        self.size_footings();
        report
    }

    /// Resizes every footing to what the soil asks for (never smaller than
    /// 60 cm). Returns how many changed.
    pub fn size_footings(&mut self) -> usize {
        let report = self.structure_report();
        let mut changed = 0;
        for result in report.footings {
            if let Some(footing) = self.fixtures.iter_mut().find(|f| f.id == result.id)
                && ((footing.width - result.needed_width_m).abs() > 1e-6
                    || (footing.depth - result.needed_depth_m).abs() > 1e-6)
            {
                footing.width = result.needed_width_m;
                footing.depth = result.needed_depth_m;
                changed += 1;
            }
        }
        changed
    }

    /// The load path from the slabs down to the soil, and the pre-sizing
    /// checks of every element.
    pub fn structure_report(&self) -> StructureReport {
        let settings = self.structure.settings;
        let labels = self.structure_labels();
        let label = |id: u64| labels.get(&id).cloned().unwrap_or_else(|| "?".to_owned());
        let mut report = StructureReport::default();
        let floors = f64::from(settings.floors.max(1));
        let beams: Vec<(Beam, Point, Point)> = self
            .valid_beams()
            .filter_map(|beam| {
                Some((
                    *beam,
                    self.column(beam.from)?.center,
                    self.column(beam.to)?.center,
                ))
            })
            .collect();
        // Slab loads shared by the beams along the slab's sides.
        let mut beam_dead: HashMap<u64, f64> = HashMap::new();
        let mut beam_live: HashMap<u64, f64> = HashMap::new();
        for slab in &self.structure.slabs {
            let Some(room) = self.rooms.iter().find(|room| room.id == slab.room_id) else {
                continue;
            };
            let (lx, ly) = self.slab_spans(room);
            let area = self.room_area(room).unwrap_or(lx * ly);
            let dead = slab.kind.self_weight(slab.thickness_m) + settings.finish_kn_m2;
            let live = settings.live_kn_m2;
            let mut shares: Vec<(u64, f64)> = Vec::new();
            if let Some(outline) = self.room_outline(room) {
                for i in 0..outline.len() {
                    let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
                    let Some(dir) = b.minus(a).normalized() else {
                        continue;
                    };
                    for (beam, p, q) in &beams {
                        let off = |x: Point| x.minus(a).cross(dir).abs();
                        if off(*p) > 0.12 || off(*q) > 0.12 {
                            continue;
                        }
                        let (s0, s1) = (p.minus(a).dot(dir), q.minus(a).dot(dir));
                        let (lo, hi) = (s0.min(s1), s0.max(s1));
                        let overlap = hi.min(a.distance(b)) - lo.max(0.0);
                        if overlap > 0.05 {
                            shares.push((beam.id, overlap));
                        }
                    }
                }
            }
            let total: f64 = shares.iter().map(|(_, s)| s).sum();
            for (id, share) in &shares {
                *beam_dead.entry(*id).or_default() += dead * area * share / total;
                *beam_live.entry(*id).or_default() += live * area * share / total;
            }
            report.slabs.push(SlabResult {
                id: slab.id,
                label: label(slab.id),
                room: room.display_name().to_owned(),
                kind: slab.kind,
                lx_m: lx,
                ly_m: ly,
                thickness_m: slab.thickness_m,
                thickness_min_m: slab.kind.minimum_thickness(lx),
                load_kn_m2: dead + live,
                supported: total > 0.0,
            });
        }

        // Continuous beams.
        let modulus = settings.modulus_kn_m2();
        let mut column_load: HashMap<u64, f64> = HashMap::new();
        let mut incident: HashMap<u64, Vec<Point>> = HashMap::new();
        for line in self.beam_lines() {
            let mut spans = Vec::new();
            let mut stiff = Vec::new();
            for (i, id) in line.spans.iter().enumerate() {
                let Some((beam, _, _)) = beams.iter().find(|(b, _, _)| b.id == *id) else {
                    continue;
                };
                let (a, b) = (
                    self.column(line.joints[i]).map(|c| c.center),
                    self.column(line.joints[i + 1]).map(|c| c.center),
                );
                let (Some(a), Some(b)) = (a, b) else {
                    continue;
                };
                let length = a.distance(b).max(0.1);
                for (from, to) in [(line.joints[i], b), (line.joints[i + 1], a)] {
                    if let Some(c) = self.column(from)
                        && let Some(d) = to.minus(c.center).normalized()
                    {
                        incident.entry(from).or_default().push(d);
                    }
                }
                // Walls of the floors above stand on the beams.
                let walls_kn_m: f64 = if floors > 1.0 {
                    self.walls
                        .iter()
                        .filter(|wall| {
                            super::geometry::distance_to_segment(
                                a.midpoint(b),
                                wall.start,
                                wall.end,
                            ) < 0.06
                        })
                        .map(|wall| wall.thickness() * self.wall_height * settings.masonry_kn_m3)
                        .fold(0.0, f64::max)
                        * (floors - 1.0)
                } else {
                    0.0
                };
                let own = CONCRETE_KN_M3 * beam.width_m * beam.height_m;
                let dead = own + walls_kn_m + beam_dead.get(id).copied().unwrap_or(0.0) / length;
                let live = beam_live.get(id).copied().unwrap_or(0.0) / length;
                let inertia = beam.width_m * beam.height_m.powi(3) / 12.0;
                spans.push((beam.id, length, dead, live, beam.width_m, beam.height_m));
                stiff.push((length, dead + live, modulus * inertia));
            }
            if spans.len() != line.spans.len() {
                continue;
            }
            let moments = continuous_beam(&stiff);
            let mut reactions = vec![0.0; line.joints.len()];
            let mut results = Vec::new();
            let count = spans.len();
            for (i, ((id, length, dead, live, b, h), (left, right))) in
                spans.iter().zip(&moments).enumerate()
            {
                let w = dead + live;
                let v_left = w * length / 2.0 + (right - left) / length;
                let v_right = v_left - w * length;
                reactions[i] += v_left;
                reactions[i + 1] -= v_right;
                let x = (v_left / w.max(1e-9)).clamp(0.0, *length);
                let span_moment = left + v_left * x - w * x * x / 2.0;
                // Quasi-permanent load (ψ2 = 0.3), cracked inertia about half
                // the gross, creep factor 1 + 1.32 (NBR 6118).
                let w_qp = dead + 0.3 * live;
                let ratio = if w > 0.0 { w_qp / w } else { 1.0 };
                let ei = modulus * b * h.powi(3) / 12.0 * 0.5;
                let deflection =
                    span_deflection(*length, w_qp, left * ratio, right * ratio, ei) * (1.0 + 1.32);
                let end_span = i == 0 || i + 1 == count;
                let divisor = if count > 1 && !end_span { 12.0 } else { 10.0 };
                results.push(SpanResult {
                    beam: *id,
                    length_m: *length,
                    width_m: *b,
                    height_m: *h,
                    dead_kn_m: *dead,
                    live_kn_m: *live,
                    moment_left: *left,
                    moment_right: *right,
                    moment_span: span_moment.max(0.0),
                    shear: v_left.abs().max(v_right.abs()),
                    deflection_mm: deflection * 1000.0,
                    limit_mm: length / 250.0 * 1000.0,
                    height_min_m: round_up(length / divisor, 0.05).max(0.25),
                });
            }
            for (joint, reaction) in line.joints.iter().zip(&reactions) {
                // An upward reaction does not relieve the column below.
                *column_load.entry(*joint).or_default() += reaction.max(0.0);
                if *reaction < -0.5 {
                    report.warnings.push(format!(
                        "{}: o apoio {} é puxado para cima ({} kN); evite um vão muito curto ao lado de um longo.",
                        line.label,
                        label(*joint),
                        decimal(-reaction, 1)
                    ));
                }
            }
            for span in &results {
                if span.height_m + 1e-6 < span.height_min_m {
                    report.warnings.push(format!(
                        "{}: vão de {} m pede altura de pelo menos {:.0} cm (tem {:.0} cm).",
                        line.label,
                        decimal(span.length_m, 2),
                        span.height_min_m * 100.0,
                        span.height_m * 100.0
                    ));
                }
                if span.deflection_mm > span.limit_mm {
                    report.warnings.push(format!(
                        "{}: flecha de {} mm passa do limite L/250 ({} mm); aumente a altura.",
                        line.label,
                        decimal(span.deflection_mm, 1),
                        decimal(span.limit_mm, 1)
                    ));
                }
                if span.width_m < 0.12 - 1e-9 {
                    report.warnings.push(format!(
                        "{}: largura abaixo de 12 cm (NBR 6118 13.2.2).",
                        line.label
                    ));
                }
            }
            report.beams.push(BeamLineResult {
                label: line.label.clone(),
                joints: line.joints.clone(),
                spans: results,
                reactions,
            });
        }

        // Columns.
        let fcd = settings.fck_mpa / 1.4 / 10.0; // kN/cm²
        let capacity = 0.85 * fcd + 0.015 * 42.0;
        let mut columns: Vec<&Fixture> = self.structural_columns().collect();
        columns.sort_by(|a, b| reading(a.center, b.center));
        for column in columns {
            let directions = incident.get(&column.id).cloned().unwrap_or_default();
            let mut distinct: Vec<Point> = Vec::new();
            for d in directions {
                if !distinct.iter().any(|e| e.dot(d) > 0.98) {
                    distinct.push(d);
                }
            }
            let position = match distinct.len() {
                0 => ColumnPosition::Isolated,
                1 => ColumnPosition::Corner,
                2 if distinct[0].dot(distinct[1]) < -0.98 => ColumnPosition::Edge,
                2 => ColumnPosition::Corner,
                3 => ColumnPosition::Edge,
                _ => ColumnPosition::Interior,
            };
            let own = CONCRETE_KN_M3 * column.width * column.depth * self.wall_height;
            let load = (column_load.get(&column.id).copied().unwrap_or(0.0) + own) * floors;
            let needed = 1.4 * position.factor() * load / capacity;
            let result = ColumnResult {
                id: column.id,
                label: label(column.id),
                width_m: column.width,
                depth_m: column.depth,
                position,
                load_kn: load,
                area_needed_cm2: needed,
                area_cm2: column.width * column.depth * 10_000.0,
            };
            if position == ColumnPosition::Isolated {
                report
                    .warnings
                    .push(format!("{}: nenhuma viga chega ao pilar.", result.label));
            } else if !result.ok() {
                report.warnings.push(format!(
                    "{}: seção de {} cm² abaixo do necessário ({:.0} cm², mínimo 360 cm² e 14 cm de lado).",
                    result.label,
                    decimal(result.area_cm2, 0),
                    needed.max(360.0)
                ));
            }
            report.columns.push(result);
        }

        // Footings.
        let mut footings: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
            .collect();
        footings.sort_by(|a, b| reading(a.center, b.center));
        for footing in footings {
            let column = report.columns.iter().find(|c| {
                self.column(c.id)
                    .is_some_and(|col| footing.contains(col.center))
            });
            let load = column.map_or(0.0, |c| c.load_kn);
            let area = 1.05 * load / settings.soil_kpa.max(10.0);
            // Same overhang on all sides of the column.
            let (cw, cd) = column
                .and_then(|c| self.column(c.id))
                .map_or((0.0, 0.0), |col| {
                    if col.quarter_turns % 2 == footing.quarter_turns % 2 {
                        (col.width, col.depth)
                    } else {
                        (col.depth, col.width)
                    }
                });
            let diff = cw - cd;
            let depth = ((-diff + (diff * diff + 4.0 * area).sqrt()) / 2.0).max(0.0);
            let needed_depth = round_up(depth, 0.05).max(0.60);
            let needed_width = round_up(depth + diff, 0.05).max(0.60);
            let overhang = ((footing.width - cw).max(footing.depth - cd) / 2.0).max(0.0);
            let result = FootingResult {
                id: footing.id,
                label: label(footing.id),
                column: column.map(|c| c.label.clone()),
                width_m: footing.width,
                depth_m: footing.depth,
                load_kn: load,
                needed_width_m: needed_width,
                needed_depth_m: needed_depth,
                height_m: round_up((overhang * 2.0 / 3.0).max(0.25), 0.05),
            };
            if result.column.is_none() {
                report
                    .warnings
                    .push(format!("{}: sapata sem pilar em cima.", result.label));
            } else if !result.ok() {
                report.warnings.push(format!(
                    "{}: {} m² é pouco para {:.0} kN no solo de {:.0} kPa; use {}.",
                    result.label,
                    decimal(result.width_m * result.depth_m, 2),
                    load,
                    settings.soil_kpa,
                    section_cm(needed_width, needed_depth)
                ));
            }
            report.footings.push(result);
        }
        for slab in &report.slabs {
            if !slab.supported {
                report.warnings.push(format!(
                    "{} ({}): nenhuma viga ao longo das bordas da laje.",
                    slab.label, slab.room
                ));
            } else if slab.thickness_m + 1e-6 < slab.thickness_min_m {
                report.warnings.push(format!(
                    "{} ({}): {:.0} cm é pouco para o vão de {} m; use {:.0} cm.",
                    slab.label,
                    slab.room,
                    slab.thickness_m * 100.0,
                    decimal(slab.lx_m, 2),
                    slab.thickness_min_m * 100.0
                ));
            }
        }
        report.slabs.sort_by(|a, b| natural(&a.label, &b.label));
        report.beams.sort_by(|a, b| natural(&a.label, &b.label));
        report
    }

    /// Beams and labels as drawn on the formwork plan: the two faces of each
    /// beam between the column faces, and the P, V, L and S labels.
    pub fn structure_marks(&self) -> Vec<PlanMark> {
        let labels = self.structure_labels();
        let mut marks = Vec::new();
        let extent = |column: &Fixture, dir: Point| {
            let (u, v) = match column.quarter_turns % 2 {
                0 => (Point::new(1.0, 0.0), Point::new(0.0, 1.0)),
                _ => (Point::new(0.0, 1.0), Point::new(1.0, 0.0)),
            };
            dir.dot(u).abs() * column.width / 2.0 + dir.dot(v).abs() * column.depth / 2.0
        };
        let mut labelled: HashSet<String> = HashSet::new();
        let lines = self.beam_lines();
        for line in &lines {
            // The longest span carries the line's label.
            let mut longest: Option<(f64, Point, Point, Beam)> = None;
            for id in &line.spans {
                let Some(beam) = self.valid_beams().find(|b| b.id == *id).copied() else {
                    continue;
                };
                let (Some(a), Some(b)) = (self.column(beam.from), self.column(beam.to)) else {
                    continue;
                };
                let Some(dir) = b.center.minus(a.center).normalized() else {
                    continue;
                };
                let normal = Point::new(-dir.y, dir.x);
                let start = a.center.translated(dir.scaled(extent(a, dir)));
                let end = b.center.translated(dir.scaled(-extent(b, dir)));
                for side in [-1.0, 1.0] {
                    let off = normal.scaled(side * beam.width_m / 2.0);
                    marks.push(PlanMark::Line {
                        points: vec![start.translated(off), end.translated(off)],
                        dashed: false,
                        heavy: true,
                    });
                }
                let length = a.center.distance(b.center);
                if longest.is_none_or(|(l, _, _, _)| length > l) {
                    longest = Some((length, a.center, b.center, beam));
                }
            }
            if let Some((_, a, b, beam)) = longest
                && labelled.insert(line.label.clone())
            {
                let dir = b.minus(a).normalized().unwrap_or(Point::new(1.0, 0.0));
                let mut normal = Point::new(-dir.y, dir.x);
                if normal.y > 0.0 || (normal.y.abs() < 1e-6 && normal.x > 0.0) {
                    normal = normal.scaled(-1.0);
                }
                let text = format!(
                    "{} ({})",
                    line.label,
                    section_cm(beam.width_m, beam.height_m)
                );
                let (w, _) = super::electrical::tag_size(&text);
                let reach = beam.width_m / 2.0 + 0.16 + normal.x.abs() * w / 2.0;
                marks.push(PlanMark::Text {
                    anchor: a.midpoint(b).translated(normal.scaled(reach)),
                    text,
                });
            }
        }
        for column in self.structural_columns() {
            if let Some(text) = labels.get(&column.id) {
                let text = format!("{text} {}", section_cm(column.width, column.depth));
                let (w, _) = super::electrical::tag_size(&text);
                let (hw, hd) = match column.quarter_turns % 2 {
                    0 => (column.width / 2.0, column.depth / 2.0),
                    _ => (column.depth / 2.0, column.width / 2.0),
                };
                marks.push(PlanMark::Text {
                    anchor: column
                        .center
                        .translated(Point::new(hw + w / 2.0 + 0.05, -hd - 0.12)),
                    text,
                });
            }
        }
        for footing in self
            .fixtures
            .iter()
            .filter(|f| f.kind == FixtureKind::Footing)
        {
            if let Some(text) = labels.get(&footing.id) {
                let text = format!("{text} {}", section_cm(footing.width, footing.depth));
                let (w, _) = super::electrical::tag_size(&text);
                let (hw, hd) = match footing.quarter_turns % 2 {
                    0 => (footing.width / 2.0, footing.depth / 2.0),
                    _ => (footing.depth / 2.0, footing.width / 2.0),
                };
                marks.push(PlanMark::Text {
                    anchor: footing
                        .center
                        .translated(Point::new(hw - w / 2.0, hd + 0.14)),
                    text,
                });
            }
        }
        for slab in &self.structure.slabs {
            let Some(room) = self.rooms.iter().find(|room| room.id == slab.room_id) else {
                continue;
            };
            let Some((anchor, _, _)) = self.room_label_anchor(room) else {
                continue;
            };
            if let Some(text) = labels.get(&slab.id) {
                marks.push(PlanMark::Text {
                    anchor: anchor.translated(Point::new(0.0, 0.45)),
                    text: format!(
                        "{text} h={:.0} ({})",
                        slab.thickness_m * 100.0,
                        slab.kind.label().to_lowercase()
                    ),
                });
            }
        }
        marks
    }
}

fn validate_section(width: f64, height: f64) -> Result<(), String> {
    if !width.is_finite() || !(0.08..=1.0).contains(&width) {
        return Err("A largura da viga deve ficar entre 8 e 100 cm.".to_owned());
    }
    if !height.is_finite() || !(0.15..=2.0).contains(&height) {
        return Err("A altura da viga deve ficar entre 15 e 200 cm.".to_owned());
    }
    Ok(())
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// "V2" before "V10".
fn natural(a: &str, b: &str) -> Ordering {
    let split = |s: &str| {
        let digits: String = s.chars().filter(char::is_ascii_digit).collect();
        let letters: String = s.chars().filter(|c| !c.is_ascii_digit()).collect();
        (letters, digits.parse::<u64>().unwrap_or(0))
    };
    split(a).cmp(&split(b))
}

/// The columns with their section, load and footing, for drawing sheets.
pub fn build_columns_schedule(project: &Project) -> super::schedules::ScheduleTable {
    let report = project.structure_report();
    let rows = report
        .columns
        .iter()
        .map(|column| {
            let footing = report
                .footings
                .iter()
                .find(|f| f.column.as_deref() == Some(column.label.as_str()));
            vec![
                column.label.clone(),
                section_cm(column.width_m, column.depth_m),
                format!("{:.0}", column.load_kn),
                footing.map_or("—".to_owned(), |f| {
                    format!("{} {}", f.label, section_cm(f.width_m, f.depth_m))
                }),
            ]
        })
        .collect();
    let s = project.structure.settings;
    super::schedules::ScheduleTable {
        title: format!(
            "Quadro de pilares e sapatas · fck {:.0} MPa · solo {:.0} kPa",
            s.fck_mpa, s.soil_kpa
        ),
        headers: ["Pilar", "Seção (cm)", "Carga (kN)", "Sapata (cm)"]
            .iter()
            .map(|h| (*h).to_owned())
            .collect(),
        rows,
        footer: None,
    }
}

/// The internal forces of every beam as diagrams: each continuous beam with
/// its supports, the bending moment diagram (hogging up, sagging down) and
/// the values at supports and spans, plus the deflection of each span.
pub fn beam_diagrams_svg(project: &Project) -> String {
    let report = project.structure_report();
    let width = 420.0;
    let row = 62.0;
    let height = (40.0 + report.beams.len() as f64 * row + 20.0).max(297.0);
    let ink = "#1d2836";
    let blue = "#1565c0";
    let red = "#c62828";
    let font = "font-family=\"Arial, sans-serif\"";
    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height:.0}mm\" viewBox=\"0 0 {width} {height:.0}\">\n\
         <rect width=\"{width}\" height=\"{height:.0}\" fill=\"#ffffff\"/>\n\
         <rect x=\"10\" y=\"10\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>\n",
        width - 20.0,
        height - 20.0
    );
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let text = |svg: &mut String,
                x: f64,
                y: f64,
                size: f64,
                bold: bool,
                anchor: &str,
                color: &str,
                value: &str| {
        svg.push_str(&format!(
            "<text x=\"{x:.2}\" y=\"{y:.2}\" {font} font-size=\"{size}\"{} fill=\"{color}\" text-anchor=\"{anchor}\">{}</text>\n",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        ));
    };
    text(
        &mut svg,
        20.0,
        24.0,
        5.0,
        true,
        "start",
        ink,
        "ESFORÇOS NAS VIGAS · MOMENTO FLETOR, CORTANTE E FLECHA",
    );
    let s = project.structure.settings;
    text(
        &mut svg,
        20.0,
        31.0,
        3.0,
        false,
        "start",
        ink,
        &format!(
            "{} · fck {:.0} MPa · sobrecarga {} kN/m² · revestimento {} kN/m² · {} pavimento(s) · valores característicos",
            project.name,
            s.fck_mpa,
            decimal(s.live_kn_m2, 1),
            decimal(s.finish_kn_m2, 1),
            s.floors.max(1)
        ),
    );
    if report.beams.is_empty() {
        text(
            &mut svg,
            20.0,
            48.0,
            3.6,
            false,
            "start",
            ink,
            "Nenhuma viga ainda: gere a estrutura ou ligue os pilares.",
        );
        svg.push_str("</svg>\n");
        return svg;
    }
    let left = 40.0;
    let usable = width - 40.0 - 120.0;
    for (index, line) in report.beams.iter().enumerate() {
        let top = 44.0 + index as f64 * row;
        let total: f64 = line.spans.iter().map(|s| s.length_m).sum::<f64>().max(0.1);
        let scale = usable / total;
        let axis = top + 22.0;
        text(
            &mut svg,
            20.0,
            axis + 1.2,
            3.4,
            true,
            "start",
            ink,
            &line.label,
        );
        svg.push_str(&format!(
            "<line x1=\"{left:.2}\" y1=\"{axis:.2}\" x2=\"{:.2}\" y2=\"{axis:.2}\" stroke=\"{ink}\" stroke-width=\"0.6\"/>\n",
            left + total * scale
        ));
        let peak = line
            .spans
            .iter()
            .flat_map(|s| {
                [
                    s.moment_left.abs(),
                    s.moment_right.abs(),
                    s.moment_span.abs(),
                ]
            })
            .fold(0.1, f64::max);
        let mscale = 14.0 / peak;
        let mut x0 = left;
        let mut path = format!("M{left:.2} {axis:.2}");
        for (i, span) in line.spans.iter().enumerate() {
            let w = span.dead_kn_m + span.live_kn_m;
            let length = span.length_m;
            let v_left = w * length / 2.0 + (span.moment_right - span.moment_left) / length;
            for k in 0..=24 {
                let x = length * f64::from(k) / 24.0;
                let m = span.moment_left + v_left * x - w * x * x / 2.0;
                // Hogging drawn above the axis, sagging below (tension side).
                path.push_str(&format!(" L{:.2} {:.2}", x0 + x * scale, axis + m * mscale));
            }
            let mid = x0 + length * scale / 2.0;
            text(
                &mut svg,
                mid,
                axis + span.moment_span * mscale + 4.0,
                2.4,
                false,
                "middle",
                blue,
                &format!("M+ {}", decimal(span.moment_span, 1)),
            );
            // Short spans only get their length, so the texts do not meet.
            let caption = if length * scale < 75.0 {
                format!("{} m", decimal(length, 2))
            } else {
                format!(
                    "{} m · {} kN/m · {} · V {} kN · flecha {} mm",
                    decimal(length, 2),
                    decimal(w, 1),
                    section_cm(span.width_m, span.height_m),
                    decimal(span.shear, 1),
                    decimal(span.deflection_mm, 1)
                )
            };
            text(
                &mut svg,
                mid,
                top + 4.0,
                2.4,
                false,
                "middle",
                ink,
                &caption,
            );
            if i == 0 && span.moment_left.abs() > 0.05 {
                text(
                    &mut svg,
                    x0,
                    axis + span.moment_left * mscale - 1.5,
                    2.4,
                    false,
                    "middle",
                    red,
                    &decimal(span.moment_left, 1),
                );
            }
            if span.moment_right.abs() > 0.05 {
                text(
                    &mut svg,
                    x0 + length * scale,
                    axis + span.moment_right * mscale - 1.5,
                    2.4,
                    false,
                    "middle",
                    red,
                    &decimal(span.moment_right, 1),
                );
            }
            x0 += length * scale;
        }
        path.push_str(&format!(" L{x0:.2} {axis:.2} Z"));
        svg.push_str(&format!(
            "<path d=\"{path}\" fill=\"{blue}\" fill-opacity=\"0.15\" stroke=\"{blue}\" stroke-width=\"0.35\"/>\n"
        ));
        // Supports with their column label and reaction.
        let labels = project.structure_labels();
        let mut x = left;
        for (j, joint) in line.joints.iter().enumerate() {
            svg.push_str(&format!(
                "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.4\"/>\n",
                x,
                axis,
                x - 2.0,
                axis + 3.0,
                x + 2.0,
                axis + 3.0
            ));
            text(
                &mut svg,
                x,
                axis + 7.0,
                2.3,
                true,
                "middle",
                ink,
                labels.get(joint).map_or("?", String::as_str),
            );
            if let Some(r) = line.reactions.get(j) {
                text(
                    &mut svg,
                    x,
                    axis + 10.5,
                    2.2,
                    false,
                    "middle",
                    ink,
                    &format!("R {} kN", decimal(*r, 1)),
                );
            }
            if let Some(span) = line.spans.get(j) {
                x += span.length_m * scale;
            }
        }
        // Summary on the right.
        let sx = width - 112.0;
        let worst = line
            .spans
            .iter()
            .map(|s| s.moment_left.min(s.moment_right))
            .fold(0.0, f64::min);
        let best = line.spans.iter().map(|s| s.moment_span).fold(0.0, f64::max);
        let shear = line.spans.iter().map(|s| s.shear).fold(0.0, f64::max);
        let ok = line
            .spans
            .iter()
            .all(|s| s.deflection_mm <= s.limit_mm && s.height_m + 1e-6 >= s.height_min_m);
        for (k, item) in [
            format!("M− máx: {} kN·m", decimal(worst, 1)),
            format!("M+ máx: {} kN·m", decimal(best, 1)),
            format!("V máx: {} kN", decimal(shear, 1)),
            if ok {
                "Altura e flecha: atendem".to_owned()
            } else {
                "Altura ou flecha: revise".to_owned()
            },
        ]
        .iter()
        .enumerate()
        {
            text(
                &mut svg,
                sx,
                top + 12.0 + k as f64 * 5.0,
                2.7,
                k == 3,
                "start",
                if k == 3 && !ok { red } else { ink },
                item,
            );
        }
    }
    let notes_y = height - 22.0;
    for (i, note) in [
        "Vigas contínuas sobre apoios simples (pilares), carga uniforme por vão: peso próprio, lajes repartidas pelas bordas e paredes dos pavimentos de cima.",
        "Flecha com carga quase permanente (g + 0,3q), inércia fissurada ≈ 50 % da bruta e fluência (1 + 1,32), comparada a L/250 (NBR 6118).",
        "Pré-dimensionamento para estudo: o projeto estrutural final é de um engenheiro habilitado (ART).",
    ]
    .iter()
    .enumerate()
    {
        text(&mut svg, 20.0, notes_y + i as f64 * 4.0, 2.3, false, "start", ink, note);
    }
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::OpeningKind;

    fn house() -> Project {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project
            .add_room(Point::new(4.0, 0.0), Point::new(7.0, 4.0), "Cozinha")
            .unwrap();
        project
    }

    #[test]
    fn a_continuous_beam_matches_the_textbook() {
        // Two equal spans with a uniform load: -wL²/8 over the middle
        // support, 3wL/8 at the ends and 10wL/8 in the middle.
        let (w, l) = (10.0, 4.0);
        let moments = continuous_beam(&[(l, w, 1.0), (l, w, 1.0)]);
        assert!(moments[0].0.abs() < 1e-9);
        assert!((moments[0].1 + w * l * l / 8.0).abs() < 1e-9);
        assert!((moments[1].0 - moments[0].1).abs() < 1e-9);
        // One simply supported span: 5wL⁴/384EI at mid-span.
        let d = span_deflection(l, w, 0.0, 0.0, 1000.0);
        assert!(
            (d - 5.0 * w * l.powi(4) / 384.0 / 1000.0).abs() < 1e-4,
            "{d}"
        );
        assert_eq!(SlabKind::Ribbed.minimum_thickness(3.2), 0.12);
        assert_eq!(SlabKind::Solid.minimum_thickness(4.4), 0.11);
        assert_eq!(natural("V2", "V10"), Ordering::Less);
    }

    #[test]
    fn generating_lays_columns_beams_slabs_and_footings() {
        let mut project = house();
        let wall = project
            .walls
            .iter()
            .find(|w| (w.start.y - w.end.y).abs() < 1e-6 && w.start.y.abs() < 1e-6)
            .unwrap()
            .id;
        project.add_opening(wall, OpeningKind::Door, 0.5).unwrap();
        let report = project.generate_structure();
        // Six junctions of the two rooms.
        assert_eq!(report.columns, 6, "{report:?}");
        assert_eq!(report.beams, 7);
        assert_eq!(report.slabs, 2);
        assert_eq!(report.footings, 6);
        // Again, nothing new.
        assert_eq!(project.generate_structure().total(), 0);
        let labels = project.structure_labels();
        let first = project
            .structural_columns()
            .min_by(|a, b| reading(a.center, b.center))
            .unwrap();
        assert_eq!(labels[&first.id], "P1");
        // Two horizontal continuous beams (V1, V2) and three vertical ones.
        let lines = project.beam_lines();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0].label, "V1");
        assert_eq!(lines[0].spans.len(), 2);

        let structure = project.structure_report();
        assert!(
            structure.slabs.iter().all(SlabResult::ok),
            "{:?}",
            structure.slabs
        );
        assert!(structure.columns.iter().all(|c| c.load_kn > 0.0));
        assert!(structure.footings.iter().all(FootingResult::ok));
        // The slab loads all reach the columns: the reactions add up to the
        // slabs, the beams and nothing else (one floor, no walls above).
        let slabs: f64 = project
            .structure
            .slabs
            .iter()
            .map(|slab| {
                let room = project.rooms.iter().find(|r| r.id == slab.room_id).unwrap();
                let result = structure.slabs.iter().find(|r| r.id == slab.id).unwrap();
                result.load_kn_m2 * project.room_area(room).unwrap()
            })
            .sum();
        let beams: f64 = structure
            .beams
            .iter()
            .flat_map(|l| &l.spans)
            .map(|s| CONCRETE_KN_M3 * s.width_m * s.height_m * s.length_m)
            .sum();
        let reactions: f64 = structure.beams.iter().flat_map(|l| &l.reactions).sum();
        assert!(
            ((slabs + beams) - reactions).abs() < 0.01 * reactions,
            "{slabs} + {beams} vs {reactions}"
        );
        assert!(structure.warnings.is_empty(), "{:?}", structure.warnings);
        // The plan shows the beams and every label.
        let marks = project.structure_marks();
        for label in ["P1", "V1", "L1", "S1"] {
            assert!(
                marks
                    .iter()
                    .any(|m| matches!(m, PlanMark::Text { text, .. } if text.starts_with(&format!("{label} ")))),
                "{label}"
            );
        }
        let svg = beam_diagrams_svg(&project);
        assert!(svg2pdf::usvg::Tree::from_str(&svg, &svg2pdf::usvg::Options::default()).is_ok());
        assert!(svg.contains(">V1<"));
    }

    #[test]
    fn undersized_elements_are_flagged() {
        let mut project = house();
        project.generate_structure();
        let slab = project.structure.slabs[0];
        project
            .set_slab(slab.room_id, SlabKind::Solid, Some(0.06))
            .unwrap();
        let beam = project.structure.beams[0].id;
        project.update_beam(beam, None, Some(0.15)).unwrap();
        project.structure.settings.floors = 6;
        let report = project.structure_report();
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("é pouco para o vão"))
        );
        assert!(report.warnings.iter().any(|w| w.contains("pede altura")));
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("seção") || w.contains("m² é pouco"))
        );
        // Bad sections are refused.
        assert!(project.update_beam(beam, Some(0.02), None).is_err());
        assert!(
            project
                .set_slab(slab.room_id, SlabKind::Solid, Some(2.0))
                .is_err()
        );
    }

    #[test]
    fn each_floor_keeps_its_beams_slabs_and_conduits() {
        // Regression: switching floors left these behind, and the pruning
        // on the next edit deleted them.
        let mut project = house();
        project.generate_structure();
        let (w, d) = FixtureKind::Panel.default_size();
        let panel = project
            .add_fixture(FixtureKind::Panel, Point::new(0.5, 0.2), w, d, 0)
            .unwrap();
        let (w, d) = FixtureKind::CeilingLight.default_size();
        let light = project
            .add_fixture(FixtureKind::CeilingLight, Point::new(2.0, 2.0), w, d, 0)
            .unwrap();
        project
            .add_conduit(panel, light, crate::domain::conduits::ConduitRun::Ceiling)
            .unwrap();
        let (beams, slabs) = (
            project.structure.beams.clone(),
            project.structure.slabs.clone(),
        );
        let conduits = project.conduits.clone();
        let ground = project.ensure_default_floor();
        project.add_floor("Superior", true, false);
        assert!(project.structure.beams.is_empty() && project.conduits.is_empty());
        // An edit upstairs prunes, and must not touch the floor below.
        project.prune_conduits();
        project.prune_structure();
        project.switch_floor(ground).unwrap();
        assert_eq!(project.structure.beams, beams);
        assert_eq!(project.structure.slabs, slabs);
        assert_eq!(project.conduits, conduits);
    }

    #[test]
    fn beams_go_with_their_columns() {
        let mut project = house();
        project.generate_structure();
        let column = project.structural_columns().next().unwrap().id;
        let attached = project.beams_of(&[column]).len();
        assert!(attached >= 2);
        let before = project.structure.beams.len();
        project.delete_fixture(column);
        assert!(project.beams_of(&[column]).is_empty());
        assert_eq!(project.structure.beams.len(), before - attached);
        // Removed any other way, the pruning on each edit catches it.
        let other = project.structural_columns().next().unwrap().id;
        let left = project.beams_of(&[other]).len();
        project.fixtures.retain(|f| f.id != other);
        assert_eq!(project.prune_structure(), left);
        // A column placed near a junction snaps onto it.
        let (at, _) = project.structural_placement(Point::new(4.2, 0.1));
        assert!(at.distance(Point::new(4.0, 0.0)) < 1e-6, "{at:?}");
    }
}
