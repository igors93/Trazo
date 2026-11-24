//! Result types produced by structural pre-sizing (`structure_report`).

use crate::domain::{fixtures::FixtureKind, geometry::Point};

use super::model::{BeamSupport, SlabKind};

/// A run of collinear beams through consecutive columns: one continuous
/// beam, labelled once.
#[derive(Clone, Debug, PartialEq)]
pub struct BeamLine {
    pub label: String,
    /// Beam ids, span by span.
    pub spans: Vec<u64>,
    /// Support ids (column or supporting beam id), in order (one more than the spans).
    pub joints: Vec<u64>,
    /// Detailed support specs matching `joints` 1:1.
    pub supports: Vec<BeamSupport>,
}

/// One span of a continuous beam, analysed.
#[derive(Clone, Debug, PartialEq)]
pub struct SpanResult {
    pub beam: u64,
    pub length_m: f64,
    pub width_m: f64,
    pub height_m: f64,
    /// Characteristic permanent and live load, kN/m (`dead_kn_m = g1_dead_kn_m + g2_dead_kn_m`).
    pub dead_kn_m: f64,
    /// Structural self-weight component `G1` (beam + slab self-weight), kN/m.
    pub g1_dead_kn_m: f64,
    /// Non-structural permanent component `G2` (finishes + walls), kN/m.
    pub g2_dead_kn_m: f64,
    pub live_kn_m: f64,
    /// Characteristic concentrated loads applied along this span.
    pub point_loads: Vec<SpanPointLoad>,
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
    /// Cracking moment Mr (kN·m), whether the quasi-permanent service moment
    /// exceeds Mr (Stage II), Branson equivalent inertia ratio I_eq / I_c and
    /// tension steel As (cm²) used in the stiffness check (B12, NBR 6118 17.3.2).
    pub cracking_moment_knm: f64,
    pub cracked: bool,
    pub i_eq_ratio: f64,
    pub branson_as_cm2: f64,
    /// The usual minimum height for this span (L/10 at an end, L/12
    /// inside a continuous beam), metres.
    pub height_min_m: f64,
}

/// Characteristic bending moment M(x) (sagging positive, kN·m) along a span,
/// including both uniform load and any concentrated point loads.
pub fn beam_moment_at(span: &SpanResult, x: f64) -> f64 {
    let l = span.length_m.max(1e-6);
    let x = x.clamp(0.0, l);
    let w = span.dead_kn_m + span.live_kn_m;
    let mut v_left = w * l / 2.0 + (span.moment_right - span.moment_left) / l;
    for pl in &span.point_loads {
        let a = pl.position_m.clamp(0.0, l);
        v_left += pl.load_kn * (l - a) / l;
    }
    let mut m = span.moment_left + v_left * x - 0.5 * w * x * x;
    for pl in &span.point_loads {
        let a = pl.position_m.clamp(0.0, l);
        if x > a {
            m -= pl.load_kn * (x - a);
        }
    }
    m
}

/// Characteristic shear force V(x) (kN) along a span, including concentrated loads.
pub fn beam_shear_at(span: &SpanResult, x: f64) -> f64 {
    let l = span.length_m.max(1e-6);
    let x = x.clamp(0.0, l);
    let w = span.dead_kn_m + span.live_kn_m;
    let mut v = w * l / 2.0 + (span.moment_right - span.moment_left) / l;
    for pl in &span.point_loads {
        let a = pl.position_m.clamp(0.0, l);
        v += pl.load_kn * (l - a) / l;
    }
    v -= w * x;
    for pl in &span.point_loads {
        let a = pl.position_m.clamp(0.0, l);
        if x > a {
            v -= pl.load_kn;
        }
    }
    v
}

/// A concentrated load retained with its position so downstream analyses
/// (notably the coupled 2D frame) cannot silently turn it into zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpanPointLoad {
    /// Distance from the span's left support, metres.
    pub position_m: f64,
    /// Characteristic downward load, kN.
    pub load_kn: f64,
    /// Fraction retained in the quasi-permanent service combination.
    pub quasi_factor: f64,
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
    /// Characteristic axial load at the foundation (including real upper floors), kN.
    pub load_kn: f64,
    /// Axial load arriving from stacked columns on real upper floors above, kN.
    pub upper_load_kn: f64,
    /// Concrete area needed and drawn, cm².
    pub area_needed_cm2: f64,
    pub area_cm2: f64,
    /// Free height: the tallest wall it stands in, or the storey (#145), m.
    pub height_m: f64,
    /// Slenderness, 3,46 · height / smallest side.
    pub slenderness: f64,
}

impl ColumnResult {
    pub fn ok(&self) -> bool {
        self.area_cm2 + 1e-6 >= self.area_needed_cm2.max(360.0)
            && self.width_m.min(self.depth_m) >= 0.14 - 1e-9
            && self.slender_ok()
    }

    /// Within the simplified methods (slenderness up to 90).
    pub fn slender_ok(&self) -> bool {
        self.slenderness <= crate::domain::wall_heights::MAX_SIMPLE_SLENDERNESS + 1e-9
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
    /// Column-to-footing centre eccentricity, metres.
    pub eccentricity_m: f64,
    /// Peak soil bearing pressure accounting for eccentricity (NBR 6122), kPa.
    pub max_pressure_kpa: f64,
    /// Allowable soil bearing pressure used in the check, kPa.
    pub soil_kpa: f64,
    /// Punching shear utilisation ratio (NBR 6118 19.5): <= 1.0 passes.
    pub punching_ratio: f64,
    /// Overturning safety factor (NBR 6122): >= 1.5 required (`INFINITY` when concentric).
    pub overturning_fs: f64,
    /// Estimated immediate elastic settlement under working load (NBR 6122 / B11), mm.
    pub settlement_mm: f64,
}

impl FootingResult {
    pub fn ok(&self) -> bool {
        self.width_m * self.depth_m + 1e-6 >= self.needed_width_m * self.needed_depth_m
            && self.max_pressure_kpa <= self.soil_kpa + 1e-3
            && self.punching_ratio <= 1.0 + 1e-6
            && self.overturning_fs >= 1.5 - 1e-6
            && self.settlement_mm <= crate::domain::soil::MAX_FOOTING_SETTLEMENT_MM + 1e-6
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
    /// Sum of concentrated characteristic loads supported by this slab, kN.
    pub point_load_kn: f64,
    /// Additional conservative one-metre-strip moment from concentrated loads, kN·m/m.
    pub point_moment_knm_m: f64,
    /// Governing positive moments per metre in the short and long directions.
    pub moment_x_knm_m: f64,
    pub moment_y_knm_m: f64,
    pub method_x: crate::domain::slabs::SlabTableMethod,
    pub method_y: crate::domain::slabs::SlabTableMethod,
    /// Governing long-term deflection and its L/250 limit, millimetres.
    pub deflection_mm: f64,
    pub deflection_limit_mm: f64,
    /// Rectangular panels used for an irregular or L-shaped room.
    pub panels: Vec<crate::domain::slabs::SlabPanelResult>,
    /// Whether some beam carries it.
    pub supported: bool,
}

impl SlabResult {
    pub fn ok(&self) -> bool {
        self.thickness_m + 1e-6 >= self.thickness_min_m
            && self.supported
            && (self.kind != SlabKind::Solid
                || self.deflection_mm <= self.deflection_limit_mm + 1e-9)
    }
}

/// An upper-floor column with no aligned column directly underneath it.
#[derive(Clone, Debug, PartialEq)]
pub struct MisalignedColumn {
    pub floor_name: String,
    pub column_label: String,
    pub center: Point,
    pub load_kn: f64,
    /// Label of the lower-floor beam acting as a transfer girder, or `None`
    /// if the column drops onto open space / slab without a beam beneath it.
    pub transfer_beam: Option<String>,
}

/// A water tank or heavy item and the structural element that receives it.
#[derive(Clone, Debug, PartialEq)]
pub struct ConcentratedLoadResult {
    pub id: u64,
    pub label: String,
    pub kind: FixtureKind,
    pub center: Point,
    pub load_kn: f64,
    pub support: String,
    pub slab: Option<String>,
    pub local_moment_knm_m: f64,
    /// Punching utilisation around the loaded footprint; `None` when the
    /// load is directly on a beam or the slab system needs manufacturer data.
    pub punching_ratio: Option<f64>,
    pub supported: bool,
}

/// The pre-sizing of the whole structure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StructureReport {
    pub slabs: Vec<SlabResult>,
    pub beams: Vec<BeamLineResult>,
    pub columns: Vec<ColumnResult>,
    pub footings: Vec<FootingResult>,
    pub concentrated_loads: Vec<ConcentratedLoadResult>,
    pub misaligned_columns: Vec<MisalignedColumn>,
    /// Balance beams centring the boundary footings (#144).
    pub balance_beams: Vec<crate::domain::foundations::BalanceBeamResult>,
    /// Ground beams and strip footings under the walls (#143).
    pub wall_foundations: Vec<crate::domain::wall_foundations::WallFoundationResult>,
    /// Timber roofs and where they bear (#114).
    pub roofs: Vec<crate::domain::roof::RoofResult>,
    /// Timber posts and the timber beams over them (#146).
    pub timber_posts: Vec<crate::domain::timber_posts::TimberPostResult>,
    pub eave_beams: Vec<crate::domain::timber_posts::EaveBeamResult>,
    /// Wind on the tall walls and the roofs (#147).
    pub wind: crate::domain::wind::WindReport,
    /// Lintels, sills and masonry wall-panel checks (#111).
    pub masonry: crate::domain::masonry_structure::MasonryStructureReport,
    /// Concrete stairs, balconies and cantilever slabs/beams (#115).
    pub cantilevers: crate::domain::cantilevers::CantileverReport,
    /// NBR 6120 load cases, NBR 6123 building wind and NBR 8681 combinations (#105).
    pub loads: crate::domain::loads::LoadAnalysisReport,
    /// 2D plane-frame analysis per continuous beam line (#94).
    pub frames: Vec<crate::domain::frame2d::FrameLineResult>,
    /// Eligible frame lines that failed to solve; never hidden from outputs.
    pub frame_failures: Vec<crate::domain::frame2d::FrameAnalysisFailure>,
    pub warnings: Vec<String>,
}

/// What generating the structure added.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StructureGeneration {
    pub columns: usize,
    pub beams: usize,
    pub footings: usize,
    pub slabs: usize,
    /// Columns that could not be placed (outside the lot even when moved
    /// inside it), so the caller can say so instead of staying silent.
    pub skipped: usize,
}

impl StructureGeneration {
    pub fn total(&self) -> usize {
        self.columns + self.beams + self.footings + self.slabs
    }
}
