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
    WallPlates,
    HipRafters,
}

impl RoofElement {
    pub fn label(self) -> &'static str {
        match self {
            Self::Battens => "Ripas",
            Self::Rafters => "Caibros",
            Self::Purlins => "Terças",
            Self::Beams => "Vigas",
            Self::WallPlates => "Frechais",
            Self::HipRafters => "Espigões",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Battens => "ripas",
            Self::Rafters => "caibros",
            Self::Purlins => "tercas",
            Self::Beams => "vigas",
            Self::WallPlates => "frechais",
            Self::HipRafters => "espigoes",
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
            Self::Beams | Self::HipRafters => &[
                (6.0, 16.0),
                (8.0, 16.0),
                (8.0, 20.0),
                (10.0, 20.0),
                (10.0, 25.0),
                (12.0, 25.0),
                (12.0, 30.0),
                (15.0, 30.0),
            ],
            Self::WallPlates => &[
                (6.0, 12.0),
                (8.0, 12.0),
                (8.0, 16.0),
                (10.0, 16.0),
                (10.0, 20.0),
                (12.0, 20.0),
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

/// One member group inside a pitched timber Howe truss (*tesoura*, #114).
#[derive(Clone, Debug, PartialEq)]
pub struct RoofTrussMemberResult {
    pub name: String,
    /// Design axial force `Nd` (kN): negative = compression, positive = tension.
    pub axial_design_kn: f64,
    /// Unbraced buckling length `L_0` (m).
    pub length_m: f64,
    pub section_cm: (f64, f64),
    pub slenderness: f64,
    /// Demand/capacity utilisation ratio (NBR 7190 tension or compression+buckling).
    pub ratio: f64,
    pub ok: bool,
}

/// Complete pitched timber Howe truss sizing (*tesoura*, #114).
#[derive(Clone, Debug, PartialEq)]
pub struct RoofTrussResult {
    pub kind: String,
    pub span_m: f64,
    pub rise_m: f64,
    pub count: usize,
    pub tributary_m: f64,
    pub design_total_load_kn: f64,
    pub members: Vec<RoofTrussMemberResult>,
    pub volume_m3: f64,
    pub ok: bool,
}

/// Integrated NBR 6123 wind uplift check for the roof and its overhangs (#114).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoofUpliftSummary {
    pub wind_v0_ms: f64,
    pub suction_kpa: f64,
    pub overhang_suction_kpa: f64,
    /// Net design uplift `1.4·W_up − 0.9·G` (kPa; > 0 means anchors must resist tension).
    pub net_design_uplift_kpa: f64,
    /// Peak design uplift tension per beam/truss support (kN).
    pub anchor_design_kn: f64,
    pub lifts: bool,
}

/// A roof analysed.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofResult {
    pub id: u64,
    pub label: String,
    pub style: super::RoofStyle,
    pub overhang_m: f64,
    pub slope_percent: f64,
    pub min_slope_percent: f64,
    pub surface_m2: f64,
    /// Loads on the horizontal projection, kN/m².
    pub dead_kn_m2: f64,
    pub live_kn_m2: f64,
    pub elements: Vec<RoofElementResult>,
    /// Sized wall plates (*frechais*) along the eave supports.
    pub wall_plate: Option<RoofElementResult>,
    /// Sized pitched timber trusses (*tesouras*) for gable/hip roofs or truss mode.
    pub truss: Option<RoofTrussResult>,
    /// Sized diagonal hip rafters (*espigões*) for 4-water hip roofs.
    pub hip_rafters: Option<RoofElementResult>,
    /// Overhang cantilever bending utilisation (0.0 when `overhang_m == 0`).
    pub overhang_bending_ratio: f64,
    /// Integrated NBR 6123 wind uplift & support anchor demand.
    pub uplift: RoofUpliftSummary,
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

/// Sizes a symmetric 4-panel pitched timber Howe truss (*tesoura*, NBR 7190)
/// using exact closed-form Method of Joints equilibrium:
///
/// - Total factored vertical load `W_d = 1.4 · (g + q) · trib · (L + 2·overhang)`
/// - Panel node loads `P = W_d / 4` at `L/4`, `L/2`, `3L/4` and `W_d / 8` at eaves
/// - Top chord outer bay compression: `N_top = (3/8 · W_d) / sin(θ)`
/// - Bottom chord outer bay tension:  `N_bot = (3/8 · W_d) / tan(θ)`
/// - Diagonal strut compression:      `N_diag = (1/8 · W_d) / sin(θ)`
/// - Central king post tension:       `N_kp = (1/4 · W_d)`
pub(super) fn size_howe_truss(
    span_m: f64,
    rise_m: f64,
    tributary_m: f64,
    total_design_load_kn: f64,
    count: usize,
    timber: TimberClass,
) -> RoofTrussResult {
    let half_span = (span_m * 0.5).max(0.5);
    let rise = rise_m.max(0.25);
    let theta = (rise / half_span).atan();
    let sin_t = theta.sin().max(0.10);
    let tan_t = theta.tan().max(0.10);

    // Exact 4-panel Howe truss member axial forces (kN).
    let n_top_c = (0.375 * total_design_load_kn) / sin_t; // compression
    let n_bot_t = (0.375 * total_design_load_kn) / tan_t; // tension
    let n_diag_c = (0.125 * total_design_load_kn) / sin_t; // compression
    let n_kp_t = 0.25 * total_design_load_kn; // tension

    let top_panel_len = (half_span * 0.5).hypot(rise * 0.5);
    let bot_panel_len = half_span * 0.5;
    let diag_len = (half_span * 0.5).hypot(rise * 0.5);
    let kp_len = rise;

    let (fc0k, _, e0m, _) = timber.properties();
    let fc0d = KMOD * fc0k / 1.4 * 1000.0; // kPa
    let ft0d = KMOD * fc0k / 1.8 * 1000.0; // kPa (NBR 7190 γ_wt = 1.8)
    let e_ef = KMOD * e0m * 1000.0; // kPa

    let catalog: &[(f64, f64)] = &[
        (6.0, 12.0),
        (6.0, 16.0),
        (8.0, 16.0),
        (8.0, 20.0),
        (10.0, 20.0),
        (10.0, 25.0),
        (12.0, 25.0),
    ];

    let size_comp = |name: &str, force_kn: f64, len_m: f64| -> RoofTrussMemberResult {
        let mut best = None;
        for &(b_cm, h_cm) in catalog {
            let (b, h) = (b_cm / 100.0, h_cm / 100.0);
            let area = b * h;
            let i_min = h * b.powi(3) / 12.0;
            let r_min = (i_min / area).sqrt();
            let lambda = len_m / r_min;
            let n_euler = std::f64::consts::PI.powi(2) * e_ef * i_min / (len_m * len_m).max(1e-4);
            let n_crush = fc0d * area;
            // NBR 7190 Perry-Robertson / Euler combined capacity.
            let rel_slender = (n_crush / n_euler.max(1e-6)).sqrt();
            let k_c = if rel_slender <= 0.3 {
                1.0
            } else {
                let phi = 0.5 * (1.0 + 0.2 * (rel_slender - 0.3) + rel_slender * rel_slender);
                (1.0 / (phi + (phi * phi - rel_slender * rel_slender).max(0.0).sqrt())).min(1.0)
            };
            let n_rd = k_c * n_crush;
            let ratio = (force_kn / n_rd.max(1e-6)).max(lambda / 140.0);
            let ok = ratio <= 1.0 + 1e-9 && lambda <= 140.0;
            let res = RoofTrussMemberResult {
                name: name.to_owned(),
                axial_design_kn: -force_kn,
                length_m: len_m,
                section_cm: (b_cm, h_cm),
                slenderness: lambda,
                ratio,
                ok,
            };
            best = Some(res);
            if ok {
                break;
            }
        }
        best.unwrap_or(RoofTrussMemberResult {
            name: name.to_owned(),
            axial_design_kn: -force_kn,
            length_m: len_m,
            section_cm: (6.0, 12.0),
            slenderness: 0.0,
            ratio: 0.0,
            ok: false,
        })
    };

    let size_tens = |name: &str, force_kn: f64, len_m: f64| -> RoofTrussMemberResult {
        let mut best = None;
        for &(b_cm, h_cm) in catalog {
            let (b, h) = (b_cm / 100.0, h_cm / 100.0);
            // 15% net-section deduction for bolted/notched timber joints.
            let a_net = 0.85 * b * h;
            let ratio = force_kn / (ft0d * a_net).max(1e-6);
            let ok = ratio <= 1.0 + 1e-9;
            let res = RoofTrussMemberResult {
                name: name.to_owned(),
                axial_design_kn: force_kn,
                length_m: len_m,
                section_cm: (b_cm, h_cm),
                slenderness: 0.0,
                ratio,
                ok,
            };
            best = Some(res);
            if ok {
                break;
            }
        }
        best.unwrap_or(RoofTrussMemberResult {
            name: name.to_owned(),
            axial_design_kn: force_kn,
            length_m: len_m,
            section_cm: (6.0, 12.0),
            slenderness: 0.0,
            ratio: 0.0,
            ok: false,
        })
    };

    let top = size_comp("Banzo superior (empenas)", n_top_c, top_panel_len);
    let bot = size_tens("Banzo inferior (linha)", n_bot_t, bot_panel_len);
    let diag = size_comp("Diagonais (escoras)", n_diag_c, diag_len);
    let kp = size_tens("Pendural central", n_kp_t, kp_len);

    let area_m2 = |s: (f64, f64)| s.0 * s.1 / 10_000.0;
    let vol_one = 4.0 * top_panel_len * area_m2(top.section_cm)
        + span_m * area_m2(bot.section_cm)
        + 2.0 * diag_len * area_m2(diag.section_cm)
        + kp_len * area_m2(kp.section_cm);
    let ok = top.ok && bot.ok && diag.ok && kp.ok;

    RoofTrussResult {
        kind: "Tesoura Howe 4 painéis".to_owned(),
        span_m,
        rise_m: rise,
        count,
        tributary_m,
        design_total_load_kn: total_design_load_kn,
        members: vec![top, bot, diag, kp],
        volume_m3: vol_one * count as f64,
        ok,
    }
}
