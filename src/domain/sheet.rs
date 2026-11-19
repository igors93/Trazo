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

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scale", rename_all = "snake_case")]
pub enum DrawingScale {
    S1_25,
    #[default]
    S1_50,
    S1_75,
    S1_100,
    S1_200,
    Custom {
        denominator: f64,
    },
}

impl DrawingScale {
    pub const STANDARD: [DrawingScale; 5] = [
        DrawingScale::S1_25,
        DrawingScale::S1_50,
        DrawingScale::S1_75,
        DrawingScale::S1_100,
        DrawingScale::S1_200,
    ];

    pub fn denominator(self) -> f64 {
        match self {
            DrawingScale::S1_25 => 25.0,
            DrawingScale::S1_50 => 50.0,
            DrawingScale::S1_75 => 75.0,
            DrawingScale::S1_100 => 100.0,
            DrawingScale::S1_200 => 200.0,
            DrawingScale::Custom { denominator } => denominator.clamp(1.0, 10_000.0),
        }
    }

    /// Paper millimetres representing 1.00 real metre (e.g. 20 mm/m at 1:50).
    pub fn mm_per_meter(self) -> f64 {
        1000.0 / self.denominator()
    }

    pub fn label(self) -> String {
        let d = self.denominator();
        if (d - d.round()).abs() < 1e-3 {
            format!("1:{:.0}", d.round())
        } else {
            format!("1:{d:.1}").replace('.', ",")
        }
    }

    pub fn fits(self, plan_w_m: f64, plan_h_m: f64, area_w_mm: f64, area_h_mm: f64) -> bool {
        let s = self.mm_per_meter();
        plan_w_m * s <= area_w_mm + 1e-6 && plan_h_m * s <= area_h_mm + 1e-6
    }

    /// Picks the most detailed standard scale that fits `plan_w_m × plan_h_m`
    /// inside `area_w_mm × area_h_mm` (falling back to 1:200).
    pub fn best_fit(plan_w_m: f64, plan_h_m: f64, area_w_mm: f64, area_h_mm: f64) -> Self {
        Self::STANDARD
            .into_iter()
            .find(|scale| scale.fits(plan_w_m, plan_h_m, area_w_mm, area_h_mm))
            .unwrap_or(DrawingScale::S1_200)
    }
}

fn default_true() -> bool {
    true
}

/// Editable ABNT title block (carimbo), legend and pagination metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TitleBlock {
    pub client: String,
    pub address: String,
    pub author: String,
    /// CAU or CREA professional registration.
    pub registration: String,
    pub drawing_title: String,
    pub date: String,
    /// Sheet pagination, e.g. `"01/03"`.
    pub sheet_number: String,
    pub revision: String,
    pub notes: String,
    #[serde(default = "default_true")]
    pub show_title_block: bool,
    #[serde(default = "default_true")]
    pub show_legend: bool,
    #[serde(default = "default_true")]
    pub show_north: bool,
    #[serde(default = "default_true")]
    pub show_scale_bar: bool,
}

impl Default for TitleBlock {
    fn default() -> Self {
        Self {
            client: String::new(),
            address: String::new(),
            author: String::new(),
            registration: String::new(),
            drawing_title: "Planta Baixa".to_owned(),
            date: String::new(),
            sheet_number: "01/01".to_owned(),
            revision: "R00".to_owned(),
            notes: String::new(),
            show_title_block: true,
            show_legend: true,
            show_north: true,
            show_scale_bar: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetLayout {
    pub size: PaperSize,
    pub orientation: Orientation,
    pub margins: SheetMargins,
    pub scale: DrawingScale,
    pub title_block: TitleBlock,
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
            scale: DrawingScale::S1_50,
            title_block: TitleBlock::default(),
        }
    }

    /// Physical `(width_mm, height_mm)` of the full sheet.
    pub fn sheet_mm(&self) -> (f64, f64) {
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
    pub fn printable_rect_mm(&self) -> (f64, f64, f64, f64) {
        let (w, h) = self.sheet_mm();
        let inner_w = (w - self.margins.left_mm - self.margins.right_mm).max(20.0);
        let inner_h = (h - self.margins.top_mm - self.margins.bottom_mm).max(20.0);
        (self.margins.left_mm, self.margins.top_mm, inner_w, inner_h)
    }

    pub fn validate(&self) -> Result<(), String> {
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

    #[test]
    fn exact_drawing_scales_compute_physical_mm_and_best_fit() {
        assert!((DrawingScale::S1_25.mm_per_meter() - 40.0).abs() < 1e-6);
        assert!((DrawingScale::S1_50.mm_per_meter() - 20.0).abs() < 1e-6);
        assert!((DrawingScale::S1_75.mm_per_meter() - 1000.0 / 75.0).abs() < 1e-6);
        assert!((DrawingScale::S1_100.mm_per_meter() - 10.0).abs() < 1e-6);
        assert!((DrawingScale::S1_200.mm_per_meter() - 5.0).abs() < 1e-6);
        assert_eq!(DrawingScale::S1_50.label(), "1:50");

        // 12 m × 8 m plan in A3 printable area (388 × 283 mm):
        // 1:25 needs 480 × 320 mm (doesn't fit); 1:50 needs 240 × 160 mm (fits!).
        assert!(!DrawingScale::S1_25.fits(12.0, 8.0, 388.0, 283.0));
        assert!(DrawingScale::S1_50.fits(12.0, 8.0, 388.0, 283.0));
        assert_eq!(
            DrawingScale::best_fit(12.0, 8.0, 388.0, 283.0),
            DrawingScale::S1_50
        );
        // 25 m × 18 m plan in A3 fits at 1:75 (333.3 × 240.0 mm <= 388 × 283 mm).
        assert_eq!(
            DrawingScale::best_fit(25.0, 18.0, 388.0, 283.0),
            DrawingScale::S1_75
        );
        // 30 m × 22 m plan in A3 exceeds 1:75 (400 × 293.3 mm) and needs 1:100 (300 × 220 mm).
        assert_eq!(
            DrawingScale::best_fit(30.0, 22.0, 388.0, 283.0),
            DrawingScale::S1_100
        );
    }
}
