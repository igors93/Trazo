//! Architectural door typologies (swing, double swing, sliding, pocket sliding,
//! pivot, bifold/accordion, revolving) and helpers for technical documentation.

use serde::{Deserialize, Serialize};

use super::model::{OpeningKind, Project};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum DoorType {
    /// Traditional single-leaf hinged door (90° arc).
    #[default]
    Swing,
    /// Two symmetric hinged leaves meeting at the centre.
    DoubleSwing,
    /// Surface-mounted sliding leaf on a parallel track.
    Sliding,
    /// Sliding leaf that retracts into a wall pocket cavity.
    PocketSliding,
    /// Offset-axis pivot door (~20% / 80% swing).
    Pivot,
    /// Multi-panel accordion / bifold door.
    Bifold,
    /// Four-wing revolving entrance door.
    Revolving,
}

impl DoorType {
    pub const ALL: [DoorType; 7] = [
        DoorType::Swing,
        DoorType::DoubleSwing,
        DoorType::Sliding,
        DoorType::PocketSliding,
        DoorType::Pivot,
        DoorType::Bifold,
        DoorType::Revolving,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Swing => "Porta de giro (1 folha)",
            Self::DoubleSwing => "Porta dupla (2 folhas)",
            Self::Sliding => "Porta de correr",
            Self::PocketSliding => "Porta de correr embutida",
            Self::Pivot => "Porta pivotante",
            Self::Bifold => "Porta sanfonada / camarão",
            Self::Revolving => "Porta giratória",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Swing => "Giro",
            Self::DoubleSwing => "Dupla",
            Self::Sliding => "Correr",
            Self::PocketSliding => "Correr embutida",
            Self::Pivot => "Pivotante",
            Self::Bifold => "Sanfonada",
            Self::Revolving => "Giratória",
        }
    }

    /// Suggested typical opening width in metres when switching typology.
    pub fn typical_width(self) -> f64 {
        match self {
            Self::Swing | Self::Sliding | Self::PocketSliding | Self::Bifold => 0.80,
            Self::Pivot => 1.00,
            Self::DoubleSwing => 1.40,
            Self::Revolving => 1.80,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum WindowType {
    /// Two overlapping sliding sashes with opposing arrows.
    #[default]
    Sliding,
    /// High-sill awning / basculante window (e.g. bathrooms, laundry).
    Basculante,
    /// Top-hung outward-projecting maxim-ar window.
    MaximAr,
    /// Side-hung casement window (2 leaves).
    Casement,
    /// European tilt-and-turn window (oscilo-batente).
    TiltTurn,
    /// Non-operable fixed glass panel.
    Fixed,
    /// Outward-projecting trapezoidal bay window.
    BayWindow,
}

impl WindowType {
    pub const ALL: [WindowType; 7] = [
        WindowType::Sliding,
        WindowType::Basculante,
        WindowType::MaximAr,
        WindowType::Casement,
        WindowType::TiltTurn,
        WindowType::Fixed,
        WindowType::BayWindow,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sliding => "Janela de correr",
            Self::Basculante => "Janela basculante",
            Self::MaximAr => "Janela maxim-ar",
            Self::Casement => "Janela de abrir (2 folhas)",
            Self::TiltTurn => "Janela oscilo-batente",
            Self::Fixed => "Vidro fixo",
            Self::BayWindow => "Bay window (projetada)",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Sliding => "Correr",
            Self::Basculante => "Basculante",
            Self::MaximAr => "Maxim-ar",
            Self::Casement => "Abrir",
            Self::TiltTurn => "Oscilo-batente",
            Self::Fixed => "Fixa",
            Self::BayWindow => "Bay window",
        }
    }

    pub fn typical_sill_and_height(self) -> (f64, f64) {
        match self {
            Self::Basculante | Self::MaximAr => (1.50, 0.60),
            Self::Fixed => (0.90, 1.20),
            Self::BayWindow => (0.80, 1.30),
            _ => (1.10, 1.00),
        }
    }
}

impl Project {
    pub fn set_opening_door_type(&mut self, id: u64, door_type: DoorType) -> Result<(), String> {
        let opening = self
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .ok_or_else(|| "Abertura não encontrada.".to_owned())?;
        if opening.kind != OpeningKind::Door {
            return Err("Apenas portas aceitam tipologia de porta.".to_owned());
        }
        opening.door_type = if door_type == DoorType::Swing {
            None
        } else {
            Some(door_type)
        };
        Ok(())
    }

    pub fn set_opening_window_type(
        &mut self,
        id: u64,
        window_type: WindowType,
    ) -> Result<(), String> {
        let opening = self
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .ok_or_else(|| "Abertura não encontrada.".to_owned())?;
        if opening.kind != OpeningKind::Window {
            return Err("Apenas janelas aceitam tipologia de janela.".to_owned());
        }
        opening.window_type = if window_type == WindowType::Sliding {
            None
        } else {
            Some(window_type)
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{annotations::frame_types, geometry::Point};

    #[test]
    fn door_typologies_differentiate_schedule_codes_and_round_trip() {
        for dt in DoorType::ALL {
            assert!(!dt.label().is_empty());
            assert!(!dt.short_label().is_empty());
            assert!(dt.typical_width() >= 0.60);
        }

        let mut project = Project::default();
        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(8.0, 0.0))
            .unwrap();
        let d1 = project.add_opening(wall, OpeningKind::Door, 0.25).unwrap();
        let d2 = project.add_opening(wall, OpeningKind::Door, 0.55).unwrap();
        let d3 = project.add_opening(wall, OpeningKind::Door, 0.82).unwrap();

        project
            .set_opening_door_type(d2, DoorType::Sliding)
            .unwrap();
        project.set_opening_door_type(d3, DoorType::Pivot).unwrap();

        // Same width (0.90 m), three distinct door types => three distinct P1/P2/P3 codes.
        let types = frame_types(&project);
        assert_eq!(types.len(), 3);
        assert_eq!(types[0].code, "P1");
        assert_eq!(types[1].code, "P2");
        assert_eq!(types[2].code, "P3");

        // Rejects setting door type on a window.
        let win = project.add_opening(wall, OpeningKind::Window, 0.10);
        if let Some(win_id) = win {
            assert!(
                project
                    .set_opening_door_type(win_id, DoorType::Bifold)
                    .is_err()
            );
        }

        // JSON round-trip preserves door_type.
        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(
            loaded
                .openings
                .iter()
                .find(|o| o.id == d1)
                .unwrap()
                .effective_door_type(),
            DoorType::Swing
        );
        assert_eq!(
            loaded
                .openings
                .iter()
                .find(|o| o.id == d2)
                .unwrap()
                .effective_door_type(),
            DoorType::Sliding
        );
        assert_eq!(
            loaded
                .openings
                .iter()
                .find(|o| o.id == d3)
                .unwrap()
                .effective_door_type(),
            DoorType::Pivot
        );
    }

    #[test]
    fn window_typologies_differentiate_schedule_codes_and_round_trip() {
        for wt in WindowType::ALL {
            assert!(!wt.label().is_empty());
            assert!(!wt.short_label().is_empty());
            let (sill, height) = wt.typical_sill_and_height();
            assert!(sill > 0.0 && height > 0.0);
        }

        let mut project = Project::default();
        let wall = project
            .add_wall(Point::new(0.0, 0.0), Point::new(8.0, 0.0))
            .unwrap();
        let w1 = project
            .add_opening(wall, OpeningKind::Window, 0.20)
            .unwrap();
        let w2 = project
            .add_opening(wall, OpeningKind::Window, 0.50)
            .unwrap();
        let w3 = project
            .add_opening(wall, OpeningKind::Window, 0.80)
            .unwrap();
        project
            .set_opening_window_type(w2, WindowType::Basculante)
            .unwrap();
        project
            .set_opening_window_type(w3, WindowType::MaximAr)
            .unwrap();

        let types = frame_types(&project);
        assert_eq!(types.len(), 3);
        assert_eq!(types[0].code, "J1");
        assert_eq!(types[1].code, "J2");
        assert!(types[1].description().contains("Basculante"));
        assert_eq!(types[2].code, "J3");
        assert!(types[2].description().contains("Maxim-ar"));

        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(
            loaded
                .openings
                .iter()
                .find(|o| o.id == w1)
                .unwrap()
                .effective_window_type(),
            WindowType::Sliding
        );
        assert_eq!(
            loaded
                .openings
                .iter()
                .find(|o| o.id == w2)
                .unwrap()
                .effective_window_type(),
            WindowType::Basculante
        );
        assert_eq!(
            loaded
                .openings
                .iter()
                .find(|o| o.id == w3)
                .unwrap()
                .effective_window_type(),
            WindowType::MaximAr
        );
    }
}
