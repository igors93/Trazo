//! Timber piece sizing (NBR 7190 bending, shear and L/200 deflection) and
//! main-beam positioning along the eaves.

use crate::domain::geometry::Point;

use super::{TimberClass, materials::KMOD};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoofElement {
    Battens,
    Rafters,
    Purlins,
    Beams,
}

impl RoofElement {
    pub fn label(self) -> &'static str {
        match self {
            Self::Battens => "Ripas",
            Self::Rafters => "Caibros",
            Self::Purlins => "Terças",
            Self::Beams => "Vigas",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Battens => "ripas",
            Self::Rafters => "caibros",
            Self::Purlins => "tercas",
            Self::Beams => "vigas",
        }
    }

    /// Commercial sections, width × height in cm, lightest first.
    fn catalog(self) -> &'static [(f64, f64)] {
        match self {
            Self::Battens => &[(5.0, 1.5), (5.0, 2.5)],
            Self::Rafters => &[(5.0, 6.0), (5.0, 7.0), (6.0, 8.0), (6.0, 12.0)],
            Self::Purlins => &[
                (6.0, 12.0),
                (6.0, 16.0),
                (8.0, 16.0),
                (8.0, 20.0),
                (10.0, 20.0),
                (10.0, 25.0),
            ],
            Self::Beams => &[
                (6.0, 16.0),
                (8.0, 16.0),
                (8.0, 20.0),
                (10.0, 20.0),
                (10.0, 25.0),
                (12.0, 25.0),
                (12.0, 30.0),
                (15.0, 30.0),
            ],
        }
    }
}

/// One kind of piece, sized.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofElementResult {
    pub element: RoofElement,
    /// Width × height chosen, cm (the largest tried when none passes).
    pub section_cm: (f64, f64),
    pub ok: bool,
    /// Horizontal span and spacing (or tributary width), metres.
    pub span_m: f64,
    pub spacing_m: f64,
    /// Characteristic load per metre of piece (g + q, horizontal), kN/m.
    pub load_kn_m: f64,
    pub bending_ratio: f64,
    pub shear_ratio: f64,
    /// Service deflection and its L/200 limit, metres.
    pub deflection_m: f64,
    pub deflection_limit_m: f64,
    pub deflection_ratio: f64,
    pub count: usize,
    pub total_length_m: f64,
    pub volume_m3: f64,
}

/// One end of a main beam bearing on the structure.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofReaction {
    pub point: Point,
    pub high: bool,
    /// Characteristic load, and its permanent part, kN.
    pub load_kn: f64,
    pub dead_kn: f64,
    /// What carries it ("P3", "V2"); `None` when nothing does.
    pub support: Option<String>,
}

/// A roof analysed.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofResult {
    pub id: u64,
    pub label: String,
    pub slope_percent: f64,
    pub min_slope_percent: f64,
    pub surface_m2: f64,
    /// Loads on the horizontal projection, kN/m².
    pub dead_kn_m2: f64,
    pub live_kn_m2: f64,
    pub elements: Vec<RoofElementResult>,
    pub reactions: Vec<RoofReaction>,
    /// Tiles to buy (5 % extra), or `None` for sheets.
    pub tiles: Option<f64>,
    pub timber_m3: f64,
    /// Sheets spanning more than they can between purlins.
    pub sheet_span_exceeded: bool,
}

impl RoofResult {
    pub fn slope_ok(&self) -> bool {
        self.slope_percent + 1e-6 >= self.min_slope_percent
    }

    pub fn unsupported(&self) -> usize {
        self.reactions
            .iter()
            .filter(|r| r.support.is_none())
            .count()
    }
}

pub(super) const LIVE_KN_M2: f64 = 0.25;
pub(super) const FRAMING_KN_M2: f64 = 0.10;
pub(super) const CEILING_KN_M2: f64 = 0.15;

/// A simply supported piece to size: `g` and `q` in kN per horizontal
/// metre (own weight added later), over the horizontal span `span`.
/// `inclined` pieces run down the slope; `normal` scales the load to the
/// component normal to the roof (battens lying on it).
#[derive(Clone, Copy)]
pub(super) struct Piece {
    pub(super) element: RoofElement,
    pub(super) g: f64,
    pub(super) q: f64,
    pub(super) span: f64,
    pub(super) spacing: f64,
    pub(super) inclined: bool,
    pub(super) normal: f64,
    pub(super) count: usize,
    pub(super) each_m: f64,
    pub(super) section_cm: Option<(f64, f64)>,
}

/// The lightest section of the catalog of the piece that passes bending,
/// shear and deflection: (section, ok, bending, shear, deflection ratios).
pub(super) fn size_piece(
    piece: &Piece,
    timber: TimberClass,
    cos: f64,
) -> ((f64, f64), bool, f64, f64, f64) {
    let Piece {
        element,
        g,
        q,
        span,
        inclined,
        normal,
        section_cm,
        ..
    } = *piece;
    let (fc0k, fv0k, e0m, density) = timber.properties();
    let fc0d = KMOD * fc0k / 1.4 * 1000.0; // kPa
    let fv0d = KMOD * fv0k / 1.8 * 1000.0;
    let e_ef = KMOD * e0m * 1000.0;
    let catalog: Vec<(f64, f64)> =
        section_cm.map_or_else(|| element.catalog().to_vec(), |section| vec![section]);
    let mut last = (
        (0.0, 0.0),
        false,
        f64::INFINITY,
        f64::INFINITY,
        f64::INFINITY,
    );
    for (b_cm, h_cm) in catalog {
        let (b, h) = (b_cm / 100.0, h_cm / 100.0);
        let own = density * 9.81 / 1000.0 * b * h / if inclined { cos } else { 1.0 };
        let gk = g + own;
        let wd = 1.4 * (gk + q) * normal;
        let moment = wd * span * span / 8.0;
        let shear = wd * span / 2.0;
        let ws = (gk + 0.2 * q) * normal;
        let ix = b * h.powi(3) / 12.0;
        let iy = h * b.powi(3) / 12.0;
        let (bending, shear_ratio, deflection_ratio) = if element == RoofElement::Purlins {
            // Purlins rest tilted on the roof slope: biaxial bending (*flexão
            // oblíqua*, NBR 7190 §7.3.4, k_M = 0.5 for rectangular sections).
            let sin = (1.0 - cos * cos).max(0.0).sqrt();
            let sigma_x = (moment * cos) / (b * h * h / 6.0);
            let sigma_y = (moment * sin) / (h * b * b / 6.0);
            let b_ratio = (sigma_x + 0.5 * sigma_y).max(0.5 * sigma_x + sigma_y) / fc0d;
            let v_ratio = 1.5 * shear * (cos + sin) / (b * h) / fv0d;
            let def_x = 5.0 * (ws * cos) * span.powi(4) / (384.0 * e_ef * ix);
            let def_y = 5.0 * (ws * sin) * span.powi(4) / (384.0 * e_ef * iy);
            let limit = span / 200.0;
            (b_ratio, v_ratio, def_x.hypot(def_y) / limit)
        } else {
            let b_ratio = moment / (b * h * h / 6.0) / fc0d;
            let v_ratio = 1.5 * shear / (b * h) / fv0d;
            let factor = if inclined { cos * cos } else { 1.0 };
            let deflection = 5.0 * ws * span.powi(4) / (384.0 * e_ef * ix * factor);
            let limit = span / if inclined { cos } else { 1.0 } / 200.0;
            (b_ratio, v_ratio, deflection / limit)
        };
        let ok =
            bending <= 1.0 + 1e-9 && shear_ratio <= 1.0 + 1e-9 && deflection_ratio <= 1.0 + 1e-9;
        last = ((b_cm, h_cm), ok, bending, shear_ratio, deflection_ratio);
        if ok {
            break;
        }
    }
    last
}

/// Main-beam positions along the eaves: the ends, the supports found, and
/// more where a gap is wider than `spacing`.
pub fn beam_positions(eaves: f64, spacing: f64, anchors: &[f64]) -> Vec<f64> {
    let mut fixed: Vec<f64> = vec![0.0, eaves];
    fixed.extend(
        anchors
            .iter()
            .copied()
            .filter(|a| *a > 0.15 && *a < eaves - 0.15),
    );
    fixed.sort_by(f64::total_cmp);
    fixed.dedup_by(|a, b| (*a - *b).abs() < 0.15);
    let mut out = Vec::new();
    for pair in fixed.windows(2) {
        let gap = pair[1] - pair[0];
        let n = ((gap / spacing.max(0.5)) - 1e-9).ceil().max(1.0) as usize;
        for k in 0..n {
            out.push(pair[0] + gap * k as f64 / n as f64);
        }
    }
    out.push(eaves);
    out
}
