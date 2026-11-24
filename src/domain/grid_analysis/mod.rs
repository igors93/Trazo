//! Coupled slab + beam grillage analysis (#109, B06): solves slabs
//! (including L-shaped and irregular polygons) and supporting concrete beams
//! together via 3-DOF/node grid analogy (`w, θx, θy`, Hambly 1991 / NBR 6118 §14.7.5).

mod builder;
mod diagrams;
mod solver;

#[cfg(test)]
mod tests;

use crate::domain::{geometry::Point, model::Project, verdict::Light};

pub use builder::analyse_project_grid;
#[cfg(test)]
pub use builder::solve_rectangular_plate_grid;
pub use diagrams::grid_analysis_svg;
#[cfg(test)]
pub use solver::GridMemberKind;

#[derive(Clone, Debug, PartialEq)]
pub struct GridSlabResult {
    pub slab_id: u64,
    pub label: String,
    pub room: String,
    pub is_irregular: bool,
    pub short_span_m: f64,
    pub w_elastic_mm: f64,
    pub w_long_term_mm: f64,
    pub limit_mm: f64,
    pub mx_pos_knm_m: f64,
    pub my_pos_knm_m: f64,
    pub mx_neg_knm_m: f64,
    pub my_neg_knm_m: f64,
    pub mxy_max_knm_m: f64,
    pub light: Light,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GridBeamResult {
    pub beam_id: u64,
    pub label: String,
    pub m_pos_knm: f64,
    pub m_neg_knm: f64,
    pub v_max_kn: f64,
    pub t_max_knm: f64,
    pub w_elastic_mm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GridReaction {
    pub label: String,
    pub point: Point,
    pub reaction_kn: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridCellHeat {
    pub min: Point,
    pub max: Point,
    pub w_elastic_mm: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridAnalysisReport {
    pub node_count: usize,
    pub member_count: usize,
    pub total_applied_kn: f64,
    pub total_reaction_kn: f64,
    pub equilibrium_error_kn: f64,
    pub max_elastic_w_mm: f64,
    pub slabs: Vec<GridSlabResult>,
    pub beams: Vec<GridBeamResult>,
    pub reactions: Vec<GridReaction>,
    pub heatmap_cells: Vec<GridCellHeat>,
    pub error: Option<String>,
}

impl Project {
    /// Runs the coupled slab + beam grillage analysis for the active floor (#109).
    pub fn grid_analysis(&self) -> GridAnalysisReport {
        analyse_project_grid(self)
    }
}
