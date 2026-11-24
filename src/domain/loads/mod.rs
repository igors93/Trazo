//! Load cases, combinations and building wind (#105):
//!
//! - **ABNT NBR 6120:2019** (`nbr6120`): permanent structural self-weight (`G1`),
//!   non-structural permanent dead loads (`G2`: finishes, masonry walls, roofs,
//!   fixed tanks/equipment), and characteristic variable occupancy live loads (`Q`)
//!   with room-by-room classification.
//! - **ABNT NBR 6123** (`nbr6123`): wind on the whole rectangular building across
//!   four orthogonal directions (`+X`, `−X`, `+Y`, `−Y`), using Tables 1–3 (`S1`,
//!   `S2(z)`, `S3`) and Table 4 external pressure/shape coefficients (`Cpe` and
//!   net drag `Ca = Cpe,barlavento − Cpe,sotavento`), storey-by-storey wind forces,
//!   base shear, overturning moment and `ELU-EQU` rigid-body stability screening.
//! - **ABNT NBR 8681:2003 / NBR 6118:2023 §11.8** (`nbr8681`): normal ultimate
//!   limit state combinations (`ELU-1..ELU-7`), equilibrium overturning/uplift
//!   combinations (`ELU-EQU±`), and serviceability combinations (`ELS-QP`,
//!   `ELS-F1..F3`, `ELS-R1..R4`) with occupancy-dependent `ψ0, ψ1, ψ2`.

pub mod analysis;
pub mod nbr6120;
pub mod nbr6123;
pub mod nbr8681;

#[cfg(test)]
mod tests;

pub use analysis::{LoadAnalysisReport, analyze_project_loads};
pub use nbr6120::LoadCaseKind;
#[cfg(test)]
pub use nbr6120::classify_room_nbr6120;
pub use nbr6123::WindDirection;
#[cfg(test)]
pub use nbr6123::nbr6123_wall_coeffs;
pub use nbr8681::{
    CombinationCategory, CombinationFactors, GAMMA_W, PSI1_WIND, build_nbr8681_combinations,
};
#[cfg(test)]
pub use nbr8681::{GAMMA_G_EQU_FAV, GAMMA_G_FAV, GAMMA_G_UNFAV, GAMMA_Q, PSI0_WIND};
