//! Documentation graphic styles based on Brazilian architectural standards
//! (ABNT NBR 6492:2021 and ABNT NBR 16752), plus visual and monochrome
//! construction-site presets.

use serde::{Deserialize, Serialize};

use super::{annotations::centimetres, geometry::format_meters};

/// Explicit responsibility notice required whenever applying NBR 6492 presets.
pub const NBR_6492_DISCLAIMER: &str = "Padrão baseado na ABNT NBR 6492:2021. Conferir as exigências normativas e legais aplicáveis continua sendo responsabilidade de quem projeta.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocStylePreset {
    #[default]
    AbntNbr6492,
    TrazoVisual,
    ObraMonocromatico,
}

impl DocStylePreset {
    pub const ALL: [DocStylePreset; 3] = [
        DocStylePreset::AbntNbr6492,
        DocStylePreset::TrazoVisual,
        DocStylePreset::ObraMonocromatico,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DocStylePreset::AbntNbr6492 => "ABNT NBR 6492:2021 (Técnico)",
            DocStylePreset::TrazoVisual => "Trazo Visual (Apresentação)",
            DocStylePreset::ObraMonocromatico => "Obra Monocromático (P&B)",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionTick {
    #[default]
    Oblique45,
    Arrow,
    Dot,
}

impl DimensionTick {
    pub const ALL: [DimensionTick; 3] = [
        DimensionTick::Oblique45,
        DimensionTick::Arrow,
        DimensionTick::Dot,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DimensionTick::Oblique45 => "Traço oblíquo 45° (NBR 6492)",
            DimensionTick::Arrow => "Seta técnica",
            DimensionTick::Dot => "Ponto cheio",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionUnit {
    #[default]
    Centimetres,
    Metres,
}

impl DimensionUnit {
    pub fn label(self) -> &'static str {
        match self {
            DimensionUnit::Centimetres => "Centímetros (cm)",
            DimensionUnit::Metres => "Metros (m)",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DocStyle {
    pub preset: DocStylePreset,
    pub monochrome: bool,
    pub tick: DimensionTick,
    pub unit: DimensionUnit,
    pub wall_pen_mm: f64,
    pub opening_pen_mm: f64,
    pub dimension_pen_mm: f64,
    pub hatch_pen_mm: f64,
    pub font_family: String,
}

impl Default for DocStyle {
    fn default() -> Self {
        Self::from_preset(DocStylePreset::AbntNbr6492)
    }
}

impl DocStyle {
    pub fn from_preset(preset: DocStylePreset) -> Self {
        match preset {
            DocStylePreset::AbntNbr6492 => Self {
                preset,
                monochrome: false,
                tick: DimensionTick::Oblique45,
                unit: DimensionUnit::Centimetres,
                wall_pen_mm: 0.35,
                opening_pen_mm: 0.20,
                dimension_pen_mm: 0.13,
                hatch_pen_mm: 0.13,
                font_family: "Arial, Helvetica, sans-serif".to_owned(),
            },
            DocStylePreset::TrazoVisual => Self {
                preset,
                monochrome: false,
                tick: DimensionTick::Oblique45,
                unit: DimensionUnit::Centimetres,
                wall_pen_mm: 0.35,
                opening_pen_mm: 0.22,
                dimension_pen_mm: 0.15,
                hatch_pen_mm: 0.15,
                font_family: "Arial, sans-serif".to_owned(),
            },
            DocStylePreset::ObraMonocromatico => Self {
                preset,
                monochrome: true,
                tick: DimensionTick::Oblique45,
                unit: DimensionUnit::Centimetres,
                wall_pen_mm: 0.40,
                opening_pen_mm: 0.25,
                dimension_pen_mm: 0.15,
                hatch_pen_mm: 0.13,
                font_family: "Arial, Helvetica, sans-serif".to_owned(),
            },
        }
    }

    pub fn format_length(&self, metres: f64) -> String {
        match self.unit {
            DimensionUnit::Centimetres => centimetres(metres),
            DimensionUnit::Metres => format_meters(metres),
        }
    }

    pub fn wall_color(&self) -> &'static str {
        if self.monochrome {
            "#000000"
        } else {
            "#1d2836"
        }
    }

    pub fn opening_color(&self) -> &'static str {
        if self.monochrome {
            "#111111"
        } else {
            "#2878d2"
        }
    }

    pub fn dimension_color(&self) -> &'static str {
        if self.monochrome {
            "#222222"
        } else {
            "#3b4e63"
        }
    }

    pub fn room_fill(&self) -> &'static str {
        match self.preset {
            DocStylePreset::TrazoVisual => "#eef5fd",
            DocStylePreset::AbntNbr6492 => "#f9fbfd",
            DocStylePreset::ObraMonocromatico => "#ffffff",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nbr_6492_preset_configures_standard_pens_units_and_disclaimer() {
        let nbr = DocStyle::from_preset(DocStylePreset::AbntNbr6492);
        assert_eq!(nbr.tick, DimensionTick::Oblique45);
        assert_eq!(nbr.unit, DimensionUnit::Centimetres);
        assert!((nbr.wall_pen_mm - 0.35).abs() < 1e-6);
        assert!((nbr.dimension_pen_mm - 0.13).abs() < 1e-6);
        assert_eq!(nbr.format_length(3.45), "345");
        assert!(NBR_6492_DISCLAIMER.contains("NBR 6492:2021"));

        let mut metres_style = nbr.clone();
        metres_style.unit = DimensionUnit::Metres;
        assert_eq!(metres_style.format_length(3.45), "3,45 m");

        let mono = DocStyle::from_preset(DocStylePreset::ObraMonocromatico);
        assert!(mono.monochrome);
        assert_eq!(mono.wall_color(), "#000000");
        assert_eq!(mono.room_fill(), "#ffffff");
    }
}
