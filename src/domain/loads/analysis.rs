//! Top-level orchestrator for NBR 6120 load cases, NBR 6123 building wind
//! and NBR 8681 / NBR 6118 ultimate & service load combinations (#105).

use crate::domain::{model::Project, structure::StructureReport};

use super::{
    nbr6120::{Nbr6120Summary, analyze_nbr6120},
    nbr6123::{BuildingWindReport, analyze_building_wind},
    nbr8681::{BuildingCombinationResult, CombinationCategory, build_nbr8681_combinations},
};

/// Complete building-level load-case, wind and combination analysis report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadAnalysisReport {
    pub nbr6120: Nbr6120Summary,
    pub building_wind: BuildingWindReport,
    pub combinations: Vec<BuildingCombinationResult>,
}

impl LoadAnalysisReport {
    pub fn is_empty(&self) -> bool {
        self.nbr6120.total_gravity_kn() <= 1e-9 && self.building_wind.directions.is_empty()
    }

    /// Maximum ELU combined vertical gravity resultant (`Fz,d`), kN.
    pub fn max_elu_vertical_kn(&self) -> f64 {
        self.combinations
            .iter()
            .filter(|c| c.factors.category == CombinationCategory::EluNormal)
            .map(|c| c.vertical_kn)
            .fold(0.0_f64, f64::max)
    }

    /// Maximum ELU combined horizontal shear magnitude along X or Y (`Fw,d`), kN.
    pub fn max_elu_horizontal_kn(&self) -> f64 {
        self.combinations
            .iter()
            .filter(|c| c.factors.category.is_ultimate())
            .map(|c| c.horizontal_x_kn.abs().max(c.horizontal_y_kn.abs()))
            .fold(0.0_f64, f64::max)
    }

    /// Quasi-permanent service vertical load (`ELS-QP`), kN.
    pub fn els_qp_vertical_kn(&self) -> f64 {
        self.combinations
            .iter()
            .find(|c| c.factors.category == CombinationCategory::ElsQuasiPermanent)
            .map_or(0.0, |c| c.vertical_kn)
    }

    /// Frequent service wind shear magnitude (`ELS-F2/F3`), kN.
    pub fn els_frequent_horizontal_kn(&self) -> f64 {
        self.combinations
            .iter()
            .filter(|c| c.factors.category == CombinationCategory::ElsFrequent)
            .map(|c| c.horizontal_x_kn.abs().max(c.horizontal_y_kn.abs()))
            .fold(0.0_f64, f64::max)
    }
}

/// Runs the full NBR 6120 + NBR 6123 + NBR 8681 load analysis on `project`.
pub fn analyze_project_loads(project: &Project, report: &StructureReport) -> LoadAnalysisReport {
    let nbr6120 = analyze_nbr6120(project, report);
    let building_wind = analyze_building_wind(project, report, nbr6120.total_g_kn());
    let g1 = nbr6120.total_g1_kn;
    let g2 = nbr6120.total_g2_kn;
    let q = nbr6120.total_q_kn;
    let v_wx = building_wind.base_shear_x_kn();
    let v_wy = building_wind.base_shear_y_kn();

    let combinations = build_nbr8681_combinations(nbr6120.occupancy)
        .into_iter()
        .map(|factors| {
            let vertical_kn = factors.gamma_g1 * g1 + factors.gamma_g2 * g2 + factors.gamma_q * q;
            let horizontal_x_kn = factors.gamma_w * v_wx;
            let horizontal_y_kn = factors.gamma_w * v_wy;
            BuildingCombinationResult {
                factors,
                vertical_kn,
                horizontal_x_kn,
                horizontal_y_kn,
            }
        })
        .collect();

    LoadAnalysisReport {
        nbr6120,
        building_wind,
        combinations,
    }
}
