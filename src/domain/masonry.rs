//! The bricks or blocks walls are built with. In a plan only two of their
//! dimensions matter: the width, which with the plaster gives the wall's
//! thickness, and the length, which with the mortar joint gives the module
//! that wall lengths should follow. The height is kept for estimates.

use serde::{Deserialize, Serialize};

/// Thickness of walls drawn before materials existed.
pub const LEGACY_THICKNESS: f32 = 0.12;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Masonry {
    pub name: String,
    /// Width of the block: the raw wall, in metres.
    pub block_width: f32,
    /// Length of the block along the wall, in metres.
    pub block_length: f32,
    /// Mortar joint between blocks, in metres.
    #[serde(default = "default_joint")]
    pub joint: f32,
    /// Plaster (reboco) on each face, in metres.
    #[serde(default)]
    pub plaster: f32,
    /// Height of the block, only for estimating quantities.
    #[serde(default = "default_height")]
    pub block_height: f32,
}

fn default_joint() -> f32 {
    0.01
}

fn default_height() -> f32 {
    0.19
}

impl Default for Masonry {
    fn default() -> Self {
        Self::presets().remove(0)
    }
}

fn preset(name: &str, width_cm: f32, length_cm: f32, height_cm: f32) -> Masonry {
    Masonry {
        name: name.to_owned(),
        block_width: width_cm / 100.0,
        block_length: length_cm / 100.0,
        joint: 0.01,
        plaster: 0.015,
        block_height: height_cm / 100.0,
    }
}

impl Masonry {
    /// Common Brazilian bricks and blocks, laid as usual.
    pub fn presets() -> Vec<Masonry> {
        vec![
            preset("Tijolo cerâmico 6 furos (em pé)", 9.0, 19.0, 14.0),
            preset("Tijolo cerâmico 6 furos (deitado)", 14.0, 19.0, 9.0),
            preset("Tijolo cerâmico 8 furos", 11.5, 19.0, 19.0),
            preset("Tijolo maciço", 10.0, 20.0, 5.0),
            preset("Bloco cerâmico estrutural 14", 14.0, 29.0, 19.0),
            preset("Bloco de concreto 9", 9.0, 39.0, 19.0),
            preset("Bloco de concreto 14", 14.0, 39.0, 19.0),
            preset("Bloco de concreto 19", 19.0, 39.0, 19.0),
        ]
    }

    /// The finished wall: block plus plaster on both faces.
    pub fn thickness(&self) -> f32 {
        self.block_width + 2.0 * self.plaster
    }

    /// One block and its joint: the step wall lengths follow.
    pub fn module(&self) -> f32 {
        self.block_length + self.joint
    }

    pub fn valid(&self) -> bool {
        let ok = |value: f32, low: f32, high: f32| value.is_finite() && (low..=high).contains(&value);
        ok(self.block_width, 0.04, 0.5)
            && ok(self.block_length, 0.05, 1.0)
            && ok(self.joint, 0.0, 0.05)
            && ok(self.plaster, 0.0, 0.08)
            && ok(self.block_height, 0.03, 1.0)
    }

    /// `length` rounded to whole half blocks (half modules), never below one.
    pub fn modular_length(&self, length: f32) -> f32 {
        let step = self.module() / 2.0;
        (length / step).round().max(1.0) * step
    }

    /// Blocks in one course of a wall `length` long: whole blocks and the
    /// length of the cut piece that completes it (0 when none is needed).
    pub fn course(&self, length: f32) -> (u32, f32) {
        let module = self.module();
        let whole = ((length + self.joint) / module + 1e-4).floor().max(0.0);
        // What is left after the whole blocks holds one more joint and the cut piece.
        let rest = length - whole * module;
        let cut = if rest > self.joint + 0.005 { rest - self.joint } else { 0.0 };
        (whole as u32, cut)
    }

    /// Blocks per square metre of wall face, joints included.
    pub fn blocks_per_square_metre(&self) -> f32 {
        1.0 / (self.module() * (self.block_height + self.joint))
    }

    /// "Tijolo cerâmico 6 furos (em pé) · 9 × 19 cm".
    pub fn label(&self) -> String {
        let cm = |value: f32| {
            let value = value * 100.0;
            if (value - value.round()).abs() < 0.05 {
                format!("{value:.0}")
            } else {
                format!("{value:.1}").replace('.', ",")
            }
        };
        format!("{} · {} × {} cm", self.name, cm(self.block_width), cm(self.block_length))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_brick_keeps_the_old_wall_thickness() {
        let brick = Masonry::default();
        assert!((brick.thickness() - LEGACY_THICKNESS).abs() < 1e-6);
        assert!((brick.module() - 0.20).abs() < 1e-6);
        assert!(Masonry::presets().iter().all(Masonry::valid));
    }

    #[test]
    fn lengths_follow_half_blocks() {
        let block = preset("Bloco de concreto 14", 14.0, 39.0, 19.0);
        assert!((block.modular_length(2.07) - 2.0).abs() < 1e-5);
        assert!((block.modular_length(2.13) - 2.2).abs() < 1e-5);
        assert!((block.modular_length(0.01) - 0.2).abs() < 1e-5);
    }

    #[test]
    fn a_course_counts_whole_blocks_and_the_cut() {
        let block = preset("Bloco de concreto 14", 14.0, 39.0, 19.0);
        // 2,39 m: six blocks of 39 cm with five joints, no cut.
        assert_eq!(block.course(2.39), (6, 0.0));
        let (whole, cut) = block.course(2.60);
        assert_eq!(whole, 6);
        assert!((cut - 0.19).abs() < 1e-4);
        // 12,5 blocks per m² for 39 × 19 blocks with 1 cm joints.
        assert!((block.blocks_per_square_metre() - 12.5).abs() < 1e-3);
    }
}
