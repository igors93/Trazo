//! 2D plane-frame structural analysis (#94) using the Direct Stiffness Method
//! (Euler–Bernoulli frame elements with axial + bending + shear DOFs, rigid
//! and pinned end releases, fixed/pinned/roller supports, nodal & member loads,
//! exact Hermite deformed shape, global equilibrium verification, and A3
//! vector diagrams).

pub mod builder;
pub mod diagrams;
pub mod model;
pub mod solver;

#[cfg(test)]
mod tests;

pub use builder::{FrameLineResult, analyze_project_frames};
pub use diagrams::frame_diagrams_svg;
