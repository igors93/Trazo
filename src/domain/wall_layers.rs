//! Multilayer wall assemblies: structural masonry, thermal/acoustic insulation,
//! plaster/render, air gap and interior/exterior finishes.

use serde::{Deserialize, Serialize};

use super::{masonry::Masonry, model::Project};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallLayerRole {
    Structure,
    Plaster,
    Insulation,
    Finish,
    AirGap,
}

impl WallLayerRole {
    pub const ALL: [WallLayerRole; 5] = [
        WallLayerRole::Structure,
        WallLayerRole::Plaster,
        WallLayerRole::Insulation,
        WallLayerRole::Finish,
        WallLayerRole::AirGap,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Structure => "Alvenaria / Estrutura",
            Self::Plaster => "Reboco / Emboço",
            Self::Insulation => "Isolamento",
            Self::Finish => "Acabamento / Revestimento",
            Self::AirGap => "Câmara de ar",
        }
    }

    pub fn default_color(self) -> [u8; 3] {
        match self {
            Self::Structure => [40, 54, 70],
            Self::Plaster => [162, 174, 188],
            Self::Insulation => [216, 176, 84],
            Self::Finish => [122, 166, 206],
            Self::AirGap => [236, 241, 246],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WallLayer {
    pub name: String,
    pub role: WallLayerRole,
    /// Layer thickness in metres.
    pub thickness_m: f64,
    #[serde(default = "default_layer_color")]
    pub color: [u8; 3],
}

fn default_layer_color() -> [u8; 3] {
    [40, 54, 70]
}

impl WallLayer {
    pub fn new(name: &str, role: WallLayerRole, thickness_cm: f64) -> Self {
        Self {
            name: name.to_owned(),
            role,
            thickness_m: (thickness_cm / 100.0).max(0.002),
            color: role.default_color(),
        }
    }

    pub fn hex_color(&self) -> String {
        let [r, g, b] = self.color;
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WallAssembly {
    pub name: String,
    pub layers: Vec<WallLayer>,
}

impl WallAssembly {
    pub fn total_thickness(&self) -> f64 {
        self.layers.iter().map(|l| l.thickness_m).sum()
    }

    pub fn valid(&self) -> bool {
        if self.layers.is_empty() {
            return false;
        }
        let all_valid = self
            .layers
            .iter()
            .all(|l| l.thickness_m.is_finite() && (0.002..=0.60).contains(&l.thickness_m));
        let total = self.total_thickness();
        all_valid && total.is_finite() && (0.04..=1.0).contains(&total)
    }

    /// Returns `(start_offset, end_offset, layer)` in metres relative to wall centreline.
    pub fn layer_offsets(&self) -> Vec<(f64, f64, &WallLayer)> {
        let mut cursor = -self.total_thickness() * 0.5;
        let mut out = Vec::with_capacity(self.layers.len());
        for layer in &self.layers {
            let next = cursor + layer.thickness_m;
            out.push((cursor, next, layer));
            cursor = next;
        }
        out
    }

    /// Derives a standard 3-layer assembly from a legacy single `Masonry` definition.
    pub fn from_masonry(masonry: &Masonry) -> Self {
        let mut layers = Vec::new();
        if masonry.plaster > 0.001 {
            layers.push(WallLayer::new(
                "Reboco externo",
                WallLayerRole::Plaster,
                masonry.plaster * 100.0,
            ));
        }
        layers.push(WallLayer::new(
            &masonry.name,
            WallLayerRole::Structure,
            masonry.block_width * 100.0,
        ));
        if masonry.plaster > 0.001 {
            layers.push(WallLayer::new(
                "Reboco interno",
                WallLayerRole::Plaster,
                masonry.plaster * 100.0,
            ));
        }
        Self {
            name: masonry.name.clone(),
            layers,
        }
    }

    /// Standard Brazilian architectural multilayer wall presets.
    pub fn presets() -> Vec<WallAssembly> {
        vec![
            WallAssembly {
                name: "Alvenaria rebocada (14 cm)".to_owned(),
                layers: vec![
                    WallLayer::new("Reboco externo", WallLayerRole::Plaster, 1.5),
                    WallLayer::new("Bloco cerâmico 11 cm", WallLayerRole::Structure, 11.0),
                    WallLayer::new("Reboco interno", WallLayerRole::Plaster, 1.5),
                ],
            },
            WallAssembly {
                name: "Parede externa com cerâmica (16,5 cm)".to_owned(),
                layers: vec![
                    WallLayer::new("Revestimento cerâmico", WallLayerRole::Finish, 1.0),
                    WallLayer::new("Emboço externo", WallLayerRole::Plaster, 1.5),
                    WallLayer::new("Bloco cerâmico 11,5 cm", WallLayerRole::Structure, 11.5),
                    WallLayer::new("Emboço interno", WallLayerRole::Plaster, 1.5),
                    WallLayer::new("Massa corrida e pintura", WallLayerRole::Finish, 1.0),
                ],
            },
            WallAssembly {
                name: "Termoacústica com lã de rocha (21 cm)".to_owned(),
                layers: vec![
                    WallLayer::new("Reboco externo", WallLayerRole::Plaster, 1.5),
                    WallLayer::new("Alvenaria estrutural 9 cm", WallLayerRole::Structure, 9.0),
                    WallLayer::new("Lã de rocha", WallLayerRole::Insulation, 3.0),
                    WallLayer::new("Alvenaria interna 6 cm", WallLayerRole::Structure, 6.0),
                    WallLayer::new("Gesso liso", WallLayerRole::Finish, 1.5),
                ],
            },
            WallAssembly {
                name: "Drywall acústico (11,5 cm)".to_owned(),
                layers: vec![
                    WallLayer::new("Chapa gesso acartonado", WallLayerRole::Finish, 1.25),
                    WallLayer::new("Montante + lã de vidro", WallLayerRole::Insulation, 9.0),
                    WallLayer::new("Chapa gesso acartonado", WallLayerRole::Finish, 1.25),
                ],
            },
            WallAssembly {
                name: "Parede dupla com câmara de ar (24 cm)".to_owned(),
                layers: vec![
                    WallLayer::new("Reboco externo", WallLayerRole::Plaster, 1.5),
                    WallLayer::new("Bloco externo 9 cm", WallLayerRole::Structure, 9.0),
                    WallLayer::new("Câmara de ar", WallLayerRole::AirGap, 3.0),
                    WallLayer::new("Bloco interno 9 cm", WallLayerRole::Structure, 9.0),
                    WallLayer::new("Reboco interno", WallLayerRole::Plaster, 1.5),
                ],
            },
        ]
    }
}

impl Project {
    /// Assigns (or clears) a multilayer wall assembly on a specific wall.
    pub fn set_wall_assembly(
        &mut self,
        wall_id: u64,
        assembly: Option<WallAssembly>,
    ) -> Result<(), String> {
        if let Some(ref asm) = assembly
            && !asm.valid()
        {
            return Err(
                "A composição multicamadas precisa ter espessura total entre 4 cm e 100 cm."
                    .to_owned(),
            );
        }
        let wall = self
            .walls
            .iter_mut()
            .find(|w| w.id == wall_id)
            .ok_or_else(|| "Parede não encontrada.".to_owned())?;
        wall.assembly = assembly;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn multilayer_presets_are_valid_and_compute_centered_offsets() {
        for role in WallLayerRole::ALL {
            assert!(!role.label().is_empty());
        }
        let presets = WallAssembly::presets();
        assert!(presets.len() >= 5);
        for preset in &presets {
            assert!(preset.valid(), "preset {} should be valid", preset.name);
            let offsets = preset.layer_offsets();
            assert_eq!(offsets.len(), preset.layers.len());
            let first = offsets.first().unwrap().0;
            let last = offsets.last().unwrap().1;
            let total = preset.total_thickness();
            assert!((first + total * 0.5).abs() < 1e-9);
            assert!((last - total * 0.5).abs() < 1e-9);
        }
    }

    #[test]
    fn assigning_multilayer_assembly_updates_wall_thickness_and_round_trips() {
        let mut project = Project::default();
        let wall_id = project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        let acoustic = WallAssembly::presets()[2].clone();
        assert!((acoustic.total_thickness() - 0.21).abs() < 1e-6);

        project
            .set_wall_assembly(wall_id, Some(acoustic.clone()))
            .unwrap();
        assert!((project.walls[0].thickness() - 0.21).abs() < 1e-6);

        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.walls[0].assembly, Some(acoustic));

        // Clearing returns to masonry thickness.
        project.set_wall_assembly(wall_id, None).unwrap();
        assert!((project.walls[0].thickness() - project.masonry.thickness()).abs() < 1e-6);
    }

    #[test]
    fn from_masonry_preserves_total_wall_thickness() {
        let m = Masonry::default();
        let asm = WallAssembly::from_masonry(&m);
        assert!(asm.valid());
        assert!((asm.total_thickness() - m.thickness()).abs() < 1e-6);
        assert_eq!(asm.layers.len(), 3);
    }
}
