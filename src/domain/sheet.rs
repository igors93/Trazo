//! Technical drawing sheets (ISO A4–A0 and custom sizes), orientations and
//! ABNT NBR 16752 margins.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "paper", rename_all = "snake_case")]
pub enum PaperSize {
    A4,
    #[default]
    A3,
    A2,
    A1,
    A0,
    Custom {
        width_mm: f64,
        height_mm: f64,
    },
}

impl PaperSize {
    pub const STANDARD: [PaperSize; 5] = [
        PaperSize::A4,
        PaperSize::A3,
        PaperSize::A2,
        PaperSize::A1,
        PaperSize::A0,
    ];

    /// Portrait `(short_mm, long_mm)` for ISO sizes, or `(width_mm, height_mm)` for Custom.
    pub fn base_mm(self) -> (f64, f64) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A2 => (420.0, 594.0),
            PaperSize::A1 => (594.0, 841.0),
            PaperSize::A0 => (841.0, 1189.0),
            PaperSize::Custom {
                width_mm,
                height_mm,
            } => (
                width_mm.clamp(100.0, 5000.0),
                height_mm.clamp(100.0, 5000.0),
            ),
        }
    }

    pub fn label(self) -> String {
        match self {
            PaperSize::A4 => "A4 (210 × 297 mm)".to_owned(),
            PaperSize::A3 => "A3 (297 × 420 mm)".to_owned(),
            PaperSize::A2 => "A2 (420 × 594 mm)".to_owned(),
            PaperSize::A1 => "A1 (594 × 841 mm)".to_owned(),
            PaperSize::A0 => "A0 (841 × 1189 mm)".to_owned(),
            PaperSize::Custom {
                width_mm,
                height_mm,
            } => format!("Personalizado ({width_mm:.0} × {height_mm:.0} mm)"),
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            PaperSize::A4 => "A4",
            PaperSize::A3 => "A3",
            PaperSize::A2 => "A2",
            PaperSize::A1 => "A1",
            PaperSize::A0 => "A0",
            PaperSize::Custom { .. } => "Personalizado",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Landscape,
    Portrait,
}

impl Orientation {
    pub fn label(self) -> &'static str {
        match self {
            Orientation::Landscape => "Paisagem",
            Orientation::Portrait => "Retrato",
        }
    }
}

/// Sheet border margins in millimetres (ABNT NBR 16752: 25 mm on the left for
/// binding, 7 mm on A4–A2 and 10 mm on A1–A0 on the remaining three sides).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetMargins {
    pub left_mm: f64,
    pub right_mm: f64,
    pub top_mm: f64,
    pub bottom_mm: f64,
}

impl Default for SheetMargins {
    fn default() -> Self {
        Self::abnt_for(PaperSize::A3)
    }
}

impl SheetMargins {
    pub fn abnt_for(size: PaperSize) -> Self {
        let other = match size {
            PaperSize::A1 | PaperSize::A0 => 10.0,
            _ => 7.0,
        };
        Self {
            left_mm: 25.0,
            right_mm: other,
            top_mm: other,
            bottom_mm: other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetLayout {
    pub size: PaperSize,
    pub orientation: Orientation,
    pub margins: SheetMargins,
}

impl Default for SheetLayout {
    fn default() -> Self {
        Self::standard(PaperSize::A3, Orientation::Landscape)
    }
}

impl SheetLayout {
    pub fn standard(size: PaperSize, orientation: Orientation) -> Self {
        Self {
            size,
            orientation,
            margins: SheetMargins::abnt_for(size),
        }
    }

    /// Physical `(width_mm, height_mm)` of the full sheet.
    pub fn sheet_mm(self) -> (f64, f64) {
        let (a, b) = self.size.base_mm();
        match self.size {
            PaperSize::Custom { .. } => match self.orientation {
                Orientation::Landscape => (a.max(b), a.min(b)),
                Orientation::Portrait => (a.min(b), a.max(b)),
            },
            _ => match self.orientation {
                Orientation::Landscape => (b, a),
                Orientation::Portrait => (a, b),
            },
        }
    }

    /// Inner drawing border `(x_mm, y_mm, width_mm, height_mm)` after margins.
    pub fn printable_rect_mm(self) -> (f64, f64, f64, f64) {
        let (w, h) = self.sheet_mm();
        let inner_w = (w - self.margins.left_mm - self.margins.right_mm).max(20.0);
        let inner_h = (h - self.margins.top_mm - self.margins.bottom_mm).max(20.0);
        (self.margins.left_mm, self.margins.top_mm, inner_w, inner_h)
    }

    pub fn validate(self) -> Result<(), String> {
        let (w, h) = self.sheet_mm();
        if !w.is_finite() || !h.is_finite() || w < 100.0 || h < 100.0 {
            return Err("O tamanho da prancha deve ser de pelo menos 100 × 100 mm.".to_owned());
        }
        if self.margins.left_mm + self.margins.right_mm >= w - 20.0
            || self.margins.top_mm + self.margins.bottom_mm >= h - 20.0
        {
            return Err("As margens são grandes demais para o formato escolhido.".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_paper_sizes_orientations_and_abnt_margins_are_exact() {
        let a3 = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
        assert_eq!(a3.sheet_mm(), (420.0, 297.0));
        assert_eq!(a3.printable_rect_mm(), (25.0, 7.0, 388.0, 283.0));
        assert!(a3.validate().is_ok());

        let a4p = SheetLayout::standard(PaperSize::A4, Orientation::Portrait);
        assert_eq!(a4p.sheet_mm(), (210.0, 297.0));
        assert_eq!(a4p.printable_rect_mm(), (25.0, 7.0, 178.0, 283.0));

        let a0 = SheetLayout::standard(PaperSize::A0, Orientation::Landscape);
        assert_eq!(a0.sheet_mm(), (1189.0, 841.0));
        assert_eq!(a0.margins.right_mm, 10.0);

        let custom = SheetLayout::standard(
            PaperSize::Custom {
                width_mm: 600.0,
                height_mm: 400.0,
            },
            Orientation::Landscape,
        );
        assert_eq!(custom.sheet_mm(), (600.0, 400.0));
        assert!(custom.validate().is_ok());
    }
}
