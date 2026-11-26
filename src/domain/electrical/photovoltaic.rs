//! Residential photovoltaic (PV) pre-sizing (#126) following INPE/CRESESB
//! Brazilian Solar Atlas irradiation data, ABNT NBR 16690 and Lei 14.300
//! (microgeração distribuída up to 75 kW): consumption estimate or user bill,
//! kWp, module count, inverter & AC protection, actual panel placement on roof
//! planes, 12-month generation profile, plan marks, CSV and A3 SVG sheet.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::domain::{
    conduits::PlanMark,
    distribution::Supply,
    electrical::{BREAKERS, WIRE_CAPACITY, WIRE_CAPACITY_3_LOADED},
    geometry::{self, Point},
    model::Project,
    verdict::Light,
};

/// Microgeneration applicability limit (Lei 14.300 / BT), kW AC.
pub const MAX_PV_MICROGEN_KW: f64 = 75.0;
/// Default commercial PV module peak power, Wp.
pub const DEFAULT_MODULE_WP: f64 = 550.0;
/// Default commercial 550 Wp module length, m.
pub const DEFAULT_MODULE_LENGTH_M: f64 = 2.28;
/// Default commercial 550 Wp module width, m.
pub const DEFAULT_MODULE_WIDTH_M: f64 = 1.13;
/// Conservative residential Performance Ratio (PR).
pub const DEFAULT_PERFORMANCE_RATIO: f64 = 0.78;
/// Additional roof dead load from PV modules + aluminium rails, kN/m² (~15 kg/m²).
pub const PV_ROOF_LOAD_KN_M2: f64 = 0.15;
/// Edge setback from roof borders for wind uplift & maintenance, m.
const ROOF_EDGE_SETBACK_M: f64 = 0.35;
/// Inter-module clamp spacing, m.
const MODULE_GAP_M: f64 = 0.025;

/// Month metadata: short PT-BR label and days in a non-leap year.
pub const MONTHS: [(&str, u32); 12] = [
    ("Jan", 31),
    ("Fev", 28),
    ("Mar", 31),
    ("Abr", 30),
    ("Mai", 31),
    ("Jun", 30),
    ("Jul", 31),
    ("Ago", 31),
    ("Set", 30),
    ("Out", 31),
    ("Nov", 30),
    ("Dez", 31),
];

/// Monthly horizontal global irradiation (`HSP`, kWh/m²·dia) for Brazilian cities
/// (INPE Atlas Brasileiro de Energia Solar 2ª ed. / CRESESB SunData).
#[derive(Clone, Copy, Debug)]
struct CitySolarData {
    preset_id: &'static str,
    city: &'static str,
    uf: &'static str,
    latitude: f64,
    monthly_hsp: [f64; 12],
}

const BRAZIL_SOLAR_DB: [CitySolarData; 27] = [
    CitySolarData { preset_id: "sao_paulo_sp", city: "São Paulo", uf: "SP", latitude: -23.55, monthly_hsp: [5.15, 5.10, 4.65, 4.15, 3.60, 3.35, 3.55, 4.15, 4.30, 4.75, 5.20, 5.30] },
    CitySolarData { preset_id: "campinas_sp", city: "Campinas", uf: "SP", latitude: -22.91, monthly_hsp: [5.55, 5.50, 5.10, 4.65, 4.05, 3.80, 4.00, 4.65, 4.85, 5.30, 5.65, 5.60] },
    CitySolarData { preset_id: "rio_de_janeiro_rj", city: "Rio de Janeiro", uf: "RJ", latitude: -22.91, monthly_hsp: [5.75, 5.80, 5.20, 4.55, 3.90, 3.65, 3.80, 4.45, 4.65, 5.15, 5.55, 5.80] },
    CitySolarData { preset_id: "belo_horizonte_mg", city: "Belo Horizonte", uf: "MG", latitude: -19.92, monthly_hsp: [5.65, 5.75, 5.35, 4.95, 4.45, 4.25, 4.45, 5.15, 5.35, 5.55, 5.50, 5.45] },
    CitySolarData { preset_id: "curitiba_pr", city: "Curitiba", uf: "PR", latitude: -25.43, monthly_hsp: [5.10, 4.95, 4.45, 3.85, 3.25, 2.95, 3.15, 3.80, 4.00, 4.50, 5.15, 5.35] },
    CitySolarData { preset_id: "florianopolis_sc", city: "Florianópolis", uf: "SC", latitude: -27.60, monthly_hsp: [5.50, 5.25, 4.65, 3.95, 3.25, 2.85, 3.05, 3.65, 3.95, 4.60, 5.40, 5.70] },
    CitySolarData { preset_id: "porto_alegre_rs", city: "Porto Alegre", uf: "RS", latitude: -30.03, monthly_hsp: [5.95, 5.50, 4.75, 3.85, 3.00, 2.55, 2.75, 3.45, 4.05, 4.85, 5.75, 6.10] },
    CitySolarData { preset_id: "brasilia_df", city: "Brasília", uf: "DF", latitude: -15.79, monthly_hsp: [5.40, 5.55, 5.35, 5.25, 5.05, 4.95, 5.15, 5.75, 5.85, 5.70, 5.35, 5.25] },
    CitySolarData { preset_id: "goiania_go", city: "Goiânia", uf: "GO", latitude: -16.68, monthly_hsp: [5.35, 5.45, 5.30, 5.20, 4.95, 4.85, 5.05, 5.65, 5.70, 5.60, 5.35, 5.20] },
    CitySolarData { preset_id: "salvador_ba", city: "Salvador", uf: "BA", latitude: -12.97, monthly_hsp: [6.05, 6.00, 5.65, 4.95, 4.35, 4.15, 4.35, 4.95, 5.55, 5.95, 6.05, 6.05] },
    CitySolarData { preset_id: "recife_pe", city: "Recife", uf: "PE", latitude: -8.05, monthly_hsp: [6.05, 5.95, 5.70, 5.10, 4.60, 4.35, 4.55, 5.20, 5.80, 6.20, 6.30, 6.15] },
    CitySolarData { preset_id: "fortaleza_ce", city: "Fortaleza", uf: "CE", latitude: -3.73, monthly_hsp: [5.65, 5.50, 5.25, 5.15, 5.35, 5.45, 5.70, 6.15, 6.45, 6.50, 6.35, 6.05] },
    CitySolarData { preset_id: "manaus_am", city: "Manaus", uf: "AM", latitude: -3.12, monthly_hsp: [4.35, 4.35, 4.45, 4.40, 4.45, 4.75, 5.00, 5.35, 5.35, 5.15, 4.85, 4.55] },
    CitySolarData { preset_id: "belem_pa", city: "Belém", uf: "PA", latitude: -1.46, monthly_hsp: [4.55, 4.50, 4.55, 4.65, 4.85, 5.10, 5.30, 5.55, 5.60, 5.55, 5.35, 4.90] },
    CitySolarData { preset_id: "vitoria_es", city: "Vitória", uf: "ES", latitude: -20.32, monthly_hsp: [5.85, 5.95, 5.40, 4.75, 4.20, 3.95, 4.10, 4.75, 5.05, 5.40, 5.55, 5.75] },
    CitySolarData { preset_id: "campo_grande_ms", city: "Campo Grande", uf: "MS", latitude: -20.47, monthly_hsp: [5.65, 5.55, 5.30, 4.85, 4.25, 4.00, 4.25, 4.95, 5.20, 5.65, 5.90, 5.85] },
    CitySolarData { preset_id: "cuiaba_mt", city: "Cuiabá", uf: "MT", latitude: -15.60, monthly_hsp: [5.25, 5.25, 5.20, 5.15, 4.85, 4.75, 5.00, 5.45, 5.45, 5.65, 5.60, 5.35] },
    CitySolarData { preset_id: "natal_rn", city: "Natal", uf: "RN", latitude: -5.79, monthly_hsp: [6.05, 5.95, 5.75, 5.35, 5.10, 4.95, 5.15, 5.75, 6.25, 6.50, 6.50, 6.25] },
    CitySolarData { preset_id: "joao_pessoa_pb", city: "João Pessoa", uf: "PB", latitude: -7.12, monthly_hsp: [6.05, 5.95, 5.75, 5.20, 4.80, 4.60, 4.80, 5.45, 6.00, 6.35, 6.40, 6.20] },
    CitySolarData { preset_id: "maceio_al", city: "Maceió", uf: "AL", latitude: -9.67, monthly_hsp: [6.05, 5.95, 5.70, 5.10, 4.55, 4.30, 4.50, 5.15, 5.75, 6.15, 6.25, 6.15] },
    CitySolarData { preset_id: "aracaju_se", city: "Aracaju", uf: "SE", latitude: -10.95, monthly_hsp: [6.15, 6.05, 5.80, 5.20, 4.65, 4.40, 4.60, 5.25, 5.85, 6.20, 6.30, 6.20] },
    CitySolarData { preset_id: "teresina_pi", city: "Teresina", uf: "PI", latitude: -5.09, monthly_hsp: [5.40, 5.30, 5.25, 5.25, 5.40, 5.55, 5.80, 6.20, 6.40, 6.35, 6.10, 5.75] },
    CitySolarData { preset_id: "sao_luis_ma", city: "São Luís", uf: "MA", latitude: -2.53, monthly_hsp: [5.05, 4.85, 4.80, 4.85, 5.10, 5.35, 5.55, 5.90, 6.10, 6.05, 5.85, 5.45] },
    CitySolarData { preset_id: "palmas_to", city: "Palmas", uf: "TO", latitude: -10.18, monthly_hsp: [5.15, 5.15, 5.15, 5.25, 5.30, 5.40, 5.60, 5.95, 5.85, 5.65, 5.40, 5.15] },
    CitySolarData { preset_id: "londrina_pr", city: "Londrina", uf: "PR", latitude: -23.31, monthly_hsp: [5.65, 5.50, 5.05, 4.50, 3.85, 3.55, 3.80, 4.50, 4.75, 5.25, 5.75, 5.85] },
    CitySolarData { preset_id: "joinville_sc", city: "Joinville", uf: "SC", latitude: -26.30, monthly_hsp: [4.95, 4.75, 4.25, 3.65, 3.05, 2.75, 2.90, 3.45, 3.70, 4.25, 4.95, 5.15] },
    CitySolarData { preset_id: "caxias_do_sul_rs", city: "Caxias do Sul", uf: "RS", latitude: -29.17, monthly_hsp: [5.75, 5.35, 4.65, 3.75, 2.95, 2.55, 2.75, 3.45, 3.95, 4.75, 5.60, 5.95] },
];

/// Conservative fallback HSP profile (mean 4.20 kWh/m²·dia) when no city is set.
const CONSERVATIVE_FALLBACK_HSP: [f64; 12] = [
    4.95, 4.80, 4.40, 3.90, 3.40, 3.15, 3.30, 3.85, 4.20, 4.60, 4.90, 4.95,
];

fn default_module_wp() -> f64 {
    DEFAULT_MODULE_WP
}
fn default_module_length() -> f64 {
    DEFAULT_MODULE_LENGTH_M
}
fn default_module_width() -> f64 {
    DEFAULT_MODULE_WIDTH_M
}
fn default_pr() -> f64 {
    DEFAULT_PERFORMANCE_RATIO
}
fn default_offset() -> f64 {
    100.0
}
fn default_true() -> bool {
    true
}

/// Photovoltaic pre-sizing settings persisted inside `ElectricalSettings` (`Copy`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PvSettings {
    /// Whether PV panels are drawn on the plan and checked in Safety verdicts.
    #[serde(default)]
    pub enabled: bool,
    /// Monthly electricity bill consumption (kWh/month); `None` estimates from the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monthly_consumption_kwh: Option<f64>,
    #[serde(default = "default_module_wp")]
    pub module_wp: f64,
    #[serde(default = "default_module_length")]
    pub module_length_m: f64,
    #[serde(default = "default_module_width")]
    pub module_width_m: f64,
    #[serde(default = "default_pr")]
    pub performance_ratio: f64,
    /// Target percentage of offsettable consumption (10..=100 %).
    #[serde(default = "default_offset")]
    pub target_offset_pct: f64,
    #[serde(default = "default_true")]
    pub show_on_plan: bool,
}

impl Default for PvSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            monthly_consumption_kwh: None,
            module_wp: DEFAULT_MODULE_WP,
            module_length_m: DEFAULT_MODULE_LENGTH_M,
            module_width_m: DEFAULT_MODULE_WIDTH_M,
            performance_ratio: DEFAULT_PERFORMANCE_RATIO,
            target_offset_pct: 100.0,
            show_on_plan: true,
        }
    }
}

impl PvSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// One placed PV module footprint on a roof plane, in model coordinates (m).
#[derive(Clone, Debug, PartialEq)]
pub struct PvPanelPlacement {
    pub roof_label: String,
    pub plane_index: usize,
    pub orientation_label: String,
    pub corners: [Point; 4],
}

/// Allocation summary for one roof plane.
#[derive(Clone, Debug, PartialEq)]
pub struct PvPlaneAllocation {
    pub roof_label: String,
    pub plane_index: usize,
    pub orientation_label: String,
    pub slope_percent: f64,
    pub area_m2: f64,
    pub orientation_factor: f64,
    pub capacity_panels: usize,
    pub placed_panels: usize,
}

/// One month of expected PV energy generation.
#[derive(Clone, Debug, PartialEq)]
pub struct PvMonthGeneration {
    pub month: String,
    pub days: u32,
    pub hsp_kwh_m2_day: f64,
    pub generation_kwh: f64,
    pub consumption_kwh: f64,
    pub balance_kwh: f64,
}

/// Complete photovoltaic pre-sizing report (#126).
#[derive(Clone, Debug, PartialEq)]
pub struct PhotovoltaicReport {
    pub settings: PvSettings,
    pub city_label: String,
    pub city_is_fallback: bool,
    pub latitude: f64,
    pub mean_hsp_kwh_m2_day: f64,
    pub monthly_consumption_kwh: f64,
    pub consumption_is_estimated: bool,
    pub availability_cost_kwh: f64,
    pub target_generation_kwh_month: f64,
    pub effective_orientation_factor: f64,
    pub required_kwp: f64,
    pub modules_required: usize,
    pub modules_placed: usize,
    pub installed_kwp: f64,
    pub inverter_kw: f64,
    pub dc_ac_ratio: f64,
    pub ac_current_a: f64,
    pub ac_breaker_a: Option<f64>,
    pub ac_wire_mm2: Option<f64>,
    pub required_roof_area_m2: f64,
    pub roof_load_kn_m2: f64,
    pub annual_generation_kwh: f64,
    pub mean_monthly_generation_kwh: f64,
    pub offset_percent: f64,
    pub refused_above_limit: bool,
    pub light: Light,
    pub summary_sentence: String,
    pub planes: Vec<PvPlaneAllocation>,
    pub placements: Vec<PvPanelPlacement>,
    pub months: Vec<PvMonthGeneration>,
    pub warnings: Vec<String>,
    pub assumptions: Vec<String>,
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn normalize_name(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .replace(['á', 'à', 'â', 'ã'], "a")
        .replace(['é', 'ê'], "e")
        .replace('í', "i")
        .replace(['ó', 'ô', 'õ'], "o")
        .replace('ú', "u")
        .replace('ç', "c")
}

/// Resolves monthly HSP `(city_label, is_fallback, latitude, [hsp; 12])` from
/// `project.codes.site`.
fn resolve_city_solar(project: &Project) -> (String, bool, f64, [f64; 12]) {
    let site = &project.codes.site;
    if let Some(id) = site.preset_id.as_deref()
        && let Some(entry) = BRAZIL_SOLAR_DB
            .iter()
            .find(|c| c.preset_id.eq_ignore_ascii_case(id))
    {
        return (
            format!("{} / {}", entry.city, entry.uf),
            false,
            site.latitude.unwrap_or(entry.latitude),
            entry.monthly_hsp,
        );
    }
    if let Some(city) = site.city.as_deref() {
        let needle = normalize_name(city);
        if let Some(entry) = BRAZIL_SOLAR_DB
            .iter()
            .find(|c| normalize_name(c.city) == needle)
        {
            return (
                format!("{} / {}", entry.city, entry.uf),
                false,
                site.latitude.unwrap_or(entry.latitude),
                entry.monthly_hsp,
            );
        }
    }
    // Fallback by closest latitude if coordinates were provided.
    if let Some(lat) = site.latitude
        && let Some(entry) = BRAZIL_SOLAR_DB
            .iter()
            .min_by(|a, b| (a.latitude - lat).abs().total_cmp(&(b.latitude - lat).abs()))
    {
        let label = site
            .location_label()
            .unwrap_or_else(|| format!("Lat. {:.2}° (ref. {})", lat, entry.city));
        return (label, false, lat, entry.monthly_hsp);
    }
    (
        "Cidade não informada (padrão conservador)".to_owned(),
        true,
        -23.5,
        CONSERVATIVE_FALLBACK_HSP,
    )
}

/// Estimates monthly consumption (kWh/month) from the project when the user
/// has not entered their electricity bill.
fn estimate_monthly_consumption_kwh(project: &Project) -> f64 {
    let schedule = project.cached_load_schedule();
    let rooms = project.rooms.len().max(1) as f64;
    let specific_count = schedule.demand.specific_count as f64;
    if schedule.total_va > 0.0 {
        // Typical Brazilian residential estimate: ~45 kWh per room + 70 kWh per TUE (shower/AC).
        (rooms * 45.0 + specific_count * 70.0).clamp(120.0, 2500.0)
    } else {
        (rooms * 55.0).clamp(150.0, 1500.0)
    }
}

/// Cardinal orientation label and transposition factor for a roof plane in the
/// Southern Hemisphere (`downhill` is a unit vector in Trazo model coords, Y down = South).
fn plane_orientation(downhill: Point, slope_percent: f64, latitude: f64) -> (&'static str, f64) {
    let tilt_deg = (slope_percent / 100.0).atan().to_degrees();
    if tilt_deg < 2.0 {
        return ("Plano", 1.00);
    }
    // Model coords: -Y is North, +Y is South, +X is East, -X is West.
    let north_component = -downhill.y;
    let east_component = downhill.x;
    let label = if north_component >= 0.70 {
        "Norte"
    } else if north_component <= -0.70 {
        "Sul"
    } else if east_component >= 0.70 {
        "Leste"
    } else if east_component <= -0.70 {
        "Oeste"
    } else if north_component > 0.0 {
        if east_component > 0.0 {
            "Nordeste"
        } else {
            "Noroeste"
        }
    } else if east_component > 0.0 {
        "Sudeste"
    } else {
        "Sudoeste"
    };

    // Transposition efficiency relative to horizontal irradiation (Southern Hemisphere, lat <= 0).
    let optimal_tilt = latitude.abs().clamp(8.0, 30.0);
    let factor = if north_component >= 0.5 {
        let penalty = ((tilt_deg - optimal_tilt).abs() / 35.0).min(1.0);
        1.03 - 0.04 * penalty
    } else if north_component <= -0.5 {
        (1.0 - 0.011 * tilt_deg).clamp(0.68, 0.96)
    } else {
        (1.0 - 0.004 * tilt_deg).clamp(0.84, 0.98)
    };
    (label, factor)
}

/// Packs non-overlapping axis-aligned PV modules inside a roof plane polygon,
/// choosing whichever module orientation (portrait or landscape) yields more modules.
fn pack_panels_on_plane(
    polygon: &[Point],
    mod_len: f64,
    mod_wid: f64,
) -> Vec<[Point; 4]> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let min_x = polygon.iter().map(|p| p.x).fold(f64::INFINITY, f64::min) + ROOF_EDGE_SETBACK_M;
    let max_x = polygon
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max)
        - ROOF_EDGE_SETBACK_M;
    let min_y = polygon.iter().map(|p| p.y).fold(f64::INFINITY, f64::min) + ROOF_EDGE_SETBACK_M;
    let max_y = polygon
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max)
        - ROOF_EDGE_SETBACK_M;
    if max_x <= min_x || max_y <= min_y {
        return Vec::new();
    }

    let try_pack = |w: f64, h: f64| -> Vec<[Point; 4]> {
        let mut placed = Vec::new();
        let step_x = w + MODULE_GAP_M;
        let step_y = h + MODULE_GAP_M;
        let nx = ((max_x - min_x + MODULE_GAP_M) / step_x).floor().max(0.0) as usize;
        let ny = ((max_y - min_y + MODULE_GAP_M) / step_y).floor().max(0.0) as usize;
        // Center the grid within the usable bounding rectangle.
        let used_w = if nx > 0 { nx as f64 * step_x - MODULE_GAP_M } else { 0.0 };
        let used_h = if ny > 0 { ny as f64 * step_y - MODULE_GAP_M } else { 0.0 };
        let ox = min_x + ((max_x - min_x) - used_w) * 0.5;
        let oy = min_y + ((max_y - min_y) - used_h) * 0.5;
        for row in 0..ny {
            for col in 0..nx {
                let x0 = ox + col as f64 * step_x;
                let y0 = oy + row as f64 * step_y;
                let x1 = x0 + w;
                let y1 = y0 + h;
                let corners = [
                    Point::new(x0, y0),
                    Point::new(x1, y0),
                    Point::new(x1, y1),
                    Point::new(x0, y1),
                ];
                // Keep all 4 corners strictly inside the roof plane polygon (handles hip trapezoids/triangles).
                let inside_all = corners.iter().all(|&c| {
                    geometry::polygon_contains_point(polygon, c)
                        && (0..polygon.len()).all(|i| {
                            geometry::distance_to_segment(
                                c,
                                polygon[i],
                                polygon[(i + 1) % polygon.len()],
                            ) >= ROOF_EDGE_SETBACK_M * 0.5
                        })
                });
                if inside_all {
                    placed.push(corners);
                }
            }
        }
        placed
    };

    let a = try_pack(mod_wid, mod_len);
    let b = try_pack(mod_len, mod_wid);
    if b.len() > a.len() { b } else { a }
}

/// Picks the smallest commercial inverter (kW) respecting DC/AC ratio ≤ 1.25.
fn select_inverter_kw(dc_kwp: f64) -> f64 {
    const SIZES: [f64; 17] = [
        1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 60.0,
        75.0,
    ];
    let min_ac = dc_kwp / 1.25;
    SIZES
        .iter()
        .copied()
        .find(|&kw| kw + 1e-9 >= min_ac)
        .unwrap_or_else(|| (min_ac / 5.0).ceil() * 5.0)
}

/// Computes the complete photovoltaic pre-sizing report for `project` (#126).
pub fn photovoltaic_report(project: &Project) -> PhotovoltaicReport {
    let settings = project.electrical.pv;
    let schedule = project.cached_load_schedule();
    let (city_label, city_is_fallback, latitude, monthly_hsp) = resolve_city_solar(project);
    let total_days: f64 = MONTHS.iter().map(|(_, d)| f64::from(*d)).sum();
    let mean_hsp: f64 = MONTHS
        .iter()
        .zip(monthly_hsp.iter())
        .map(|((_, d), &h)| f64::from(*d) * h)
        .sum::<f64>()
        / total_days;

    let (monthly_consumption, consumption_is_estimated) = match settings.monthly_consumption_kwh {
        Some(v) if v.is_finite() && v > 0.0 => (v, false),
        _ => (estimate_monthly_consumption_kwh(project), true),
    };

    // ANEEL REN 1000 / Lei 14.300 availability cost (kWh/month).
    let supply = schedule.supply;
    let availability_cost_kwh = match supply {
        Supply::Three => 100.0,
        Supply::Two => 50.0,
        Supply::Single | Supply::Auto => 30.0,
    };
    let offset_frac = (settings.target_offset_pct / 100.0).clamp(0.10, 1.0);
    let target_gen_month = ((monthly_consumption - availability_cost_kwh).max(monthly_consumption * 0.25))
        * offset_frac;
    let pr = settings.performance_ratio.clamp(0.60, 0.88);
    let mod_wp = settings.module_wp.clamp(250.0, 800.0);
    let mod_len = settings.module_length_m.clamp(1.0, 3.0);
    let mod_wid = settings.module_width_m.clamp(0.6, 1.8);

    // Inspect available roof planes and sort best-oriented first.
    let roof_plans = crate::domain::construction::roof_plan::roof_plans(project);
    struct CandidatePlane {
        roof_label: String,
        plane_index: usize,
        orientation_label: String,
        slope_percent: f64,
        area_m2: f64,
        orientation_factor: f64,
        candidates: Vec<[Point; 4]>,
    }
    let mut plane_pool: Vec<CandidatePlane> = Vec::new();
    for rp in &roof_plans {
        for (idx, plane) in rp.planes.iter().enumerate() {
            let (orient_label, factor) =
                plane_orientation(plane.downhill, rp.slope_percent, latitude);
            let candidates = pack_panels_on_plane(&plane.polygon, mod_len, mod_wid);
            plane_pool.push(CandidatePlane {
                roof_label: rp.label.clone(),
                plane_index: idx + 1,
                orientation_label: orient_label.to_owned(),
                slope_percent: rp.slope_percent,
                area_m2: plane.area_m2(),
                orientation_factor: factor,
                candidates,
            });
        }
    }
    plane_pool.sort_by(|a, b| b.orientation_factor.total_cmp(&a.orientation_factor));

    let best_orient = plane_pool
        .first()
        .map_or(1.0, |p| p.orientation_factor);

    // Initial required kWp and module count using best orientation.
    let daily_target = target_gen_month / 30.4167;
    let initial_kwp = daily_target / (mean_hsp * pr * best_orient).max(0.1);
    let modules_required = ((initial_kwp * 1000.0) / mod_wp).ceil().max(1.0) as usize;

    // Place required modules onto ranked roof planes.
    let mut remaining = modules_required;
    let mut placements: Vec<PvPanelPlacement> = Vec::new();
    let mut planes: Vec<PvPlaneAllocation> = Vec::new();
    let mut weighted_orient_sum = 0.0;
    let mut used_south = false;

    for cp in &plane_pool {
        let take = remaining.min(cp.candidates.len());
        remaining = remaining.saturating_sub(take);
        for corners in cp.candidates.iter().take(take) {
            placements.push(PvPanelPlacement {
                roof_label: cp.roof_label.clone(),
                plane_index: cp.plane_index,
                orientation_label: cp.orientation_label.clone(),
                corners: *corners,
            });
        }
        if take > 0 {
            weighted_orient_sum += take as f64 * cp.orientation_factor;
            if cp.orientation_label.contains("Sul") || cp.orientation_label.contains("Sud") {
                used_south = true;
            }
        }
        planes.push(PvPlaneAllocation {
            roof_label: cp.roof_label.clone(),
            plane_index: cp.plane_index,
            orientation_label: cp.orientation_label.clone(),
            slope_percent: cp.slope_percent,
            area_m2: cp.area_m2,
            orientation_factor: cp.orientation_factor,
            capacity_panels: cp.candidates.len(),
            placed_panels: take,
        });
    }

    let modules_placed = placements.len();
    let effective_modules = if !plane_pool.is_empty() && modules_placed > 0 {
        modules_placed
    } else {
        modules_required
    };
    let effective_orient = if modules_placed > 0 {
        weighted_orient_sum / modules_placed as f64
    } else {
        best_orient
    };
    let required_kwp = daily_target / (mean_hsp * pr * effective_orient).max(0.1);
    let installed_kwp = effective_modules as f64 * mod_wp / 1000.0;
    let inverter_kw = select_inverter_kw(installed_kwp);
    let dc_ac_ratio = installed_kwp / inverter_kw.max(0.1);
    let refused_above_limit = inverter_kw > MAX_PV_MICROGEN_KW + 1e-6;

    // AC connection sizing (NBR 5410 / NBR 16690).
    let general_v = project.electrical.voltage_v.max(1.0);
    let (ac_current_a, use_3_loaded) = if supply == Supply::Three || inverter_kw > 8.0 {
        let v_ll = if general_v < 170.0 { 220.0 } else { 380.0 };
        (inverter_kw * 1000.0 / (3.0_f64.sqrt() * v_ll), true)
    } else {
        let v_ac = if general_v < 170.0 { 220.0 } else { general_v };
        (inverter_kw * 1000.0 / v_ac, false)
    };
    let ac_breaker_a = BREAKERS
        .iter()
        .copied()
        .find(|&b| b + 1e-9 >= ac_current_a * 1.15);
    let table = if use_3_loaded {
        &WIRE_CAPACITY_3_LOADED
    } else {
        &WIRE_CAPACITY
    };
    let ac_wire_mm2 = ac_breaker_a.and_then(|brk| {
        table
            .iter()
            .find(|(s, cap)| *s >= 4.0 && *cap + 1e-9 >= brk)
            .map(|(s, _)| *s)
    });

    // 12-month generation breakdown.
    let mut months = Vec::with_capacity(12);
    let mut annual_gen = 0.0;
    for (&(name, days), &hsp) in MONTHS.iter().zip(monthly_hsp.iter()) {
        let gen_kwh = installed_kwp * hsp * f64::from(days) * pr * effective_orient;
        annual_gen += gen_kwh;
        months.push(PvMonthGeneration {
            month: name.to_owned(),
            days,
            hsp_kwh_m2_day: hsp,
            generation_kwh: gen_kwh,
            consumption_kwh: monthly_consumption,
            balance_kwh: gen_kwh - monthly_consumption,
        });
    }
    let mean_monthly = annual_gen / 12.0;
    let offset_percent = (mean_monthly / monthly_consumption.max(1.0)) * 100.0;
    let required_roof_area_m2 = modules_required as f64 * mod_len * mod_wid * 1.25;

    let mut warnings = Vec::new();
    if refused_above_limit {
        warnings.push(format!(
            "Inversor de {:.1} kW excede o limite de {:.0} kW de microgeração distribuída (Lei 14.300): exige projeto de minigeração por engenheiro eletricista.",
            inverter_kw, MAX_PV_MICROGEN_KW
        ));
    }
    if city_is_fallback {
        warnings.push(
            "Cidade não configurada em Códigos/Local: usado valor conservador de 4,20 kWh/m²·dia. Selecione sua cidade para a geração real."
                .to_owned(),
        );
    }
    if consumption_is_estimated {
        warnings.push(format!(
            "Consumo de {:.0} kWh/mês estimado pelos cômodos e circuitos: informe o consumo médio real da sua conta de luz.",
            monthly_consumption
        ));
    }
    if plane_pool.is_empty() {
        warnings.push(format!(
            "Nenhum telhado modelado ainda: são necessários cerca de {} m² livres de telhado voltado ao Norte (ou Leste/Oeste) para {} módulo(s).",
            decimal(required_roof_area_m2, 1),
            modules_required
        ));
    } else if modules_placed < modules_required {
        warnings.push(format!(
            "Cabem apenas {} de {} módulo(s) necessários nos planos de telhado modelados ({:.0} % do consumo): avalie módulos mais potentes ou ampliar a cobertura.",
            modules_placed, modules_required, offset_percent
        ));
    }
    if used_south {
        warnings.push(
            "Parte dos módulos foi alocada em água voltada para o Sul (menor insolação no Hemisfério Sul): prefira águas Norte, Leste ou Oeste quando possível."
                .to_owned(),
        );
    }
    if inverter_kw > 8.0 && supply != Supply::Three {
        warnings.push(format!(
            "Inversor de {:.1} kW normalmente exige ligação trifásica na concessionária (atual: {}).",
            inverter_kw,
            supply.label().to_lowercase()
        ));
    }
    warnings.push(format!(
        "Sobrecarga permanente no telhado de {:.2} kN/m² (~15 kg/m² entre módulos e trilhos): confira caibros, terças e fixação contra vento antes de instalar.",
        PV_ROOF_LOAD_KN_M2
    ));

    let light = if refused_above_limit || (!plane_pool.is_empty() && modules_placed < modules_required)
    {
        Light::Red
    } else if used_south || city_is_fallback || dc_ac_ratio > 1.25 {
        Light::Yellow
    } else {
        Light::Green
    };

    let summary_sentence = if refused_above_limit {
        format!(
            "Sistema de {} kWp ({:.1} kW CA) ultrapassa o limite residencial de 75 kW: procure um engenheiro eletricista.",
            decimal(installed_kwp, 2),
            inverter_kw
        )
    } else if !plane_pool.is_empty() && modules_placed < modules_required {
        format!(
            "Falta área no telhado: cabem {} de {} módulos ({} kWp, geração de {:.0} kWh/mês para {:.0} kWh/mês de consumo).",
            modules_placed,
            modules_required,
            decimal(installed_kwp, 2),
            mean_monthly,
            monthly_consumption
        )
    } else {
        format!(
            "Sistema de {} kWp ({} módulos de {:.0} Wp + inversor de {:.1} kW) gera em média {:.0} kWh/mês em {} ({:.0} % do consumo).",
            decimal(installed_kwp, 2),
            effective_modules,
            mod_wp,
            inverter_kw,
            mean_monthly,
            city_label,
            offset_percent
        )
    };

    let assumptions = vec![
        format!(
            "Irradiação solar (INPE Atlas Solar / CRESESB): {} · HSP média = {} kWh/m²·dia.",
            city_label,
            decimal(mean_hsp, 2)
        ),
        format!(
            "Taxa de desempenho (PR) = {:.0} % · fator médio de orientação/inclinação = {} · custo de disponibilidade ANEEL = {:.0} kWh/mês.",
            pr * 100.0,
            decimal(effective_orient, 2),
            availability_cost_kwh
        ),
        "Pré-dimensionamento preliminar (ABNT NBR 16690 / Lei 14.300): a homologação junto à distribuidora exige projeto e ART/TRT de profissional habilitado e verificação de sombreamento no local.".to_owned(),
    ];

    PhotovoltaicReport {
        settings,
        city_label,
        city_is_fallback,
        latitude,
        mean_hsp_kwh_m2_day: mean_hsp,
        monthly_consumption_kwh: monthly_consumption,
        consumption_is_estimated,
        availability_cost_kwh,
        target_generation_kwh_month: target_gen_month,
        effective_orientation_factor: effective_orient,
        required_kwp,
        modules_required,
        modules_placed,
        installed_kwp,
        inverter_kw,
        dc_ac_ratio,
        ac_current_a,
        ac_breaker_a,
        ac_wire_mm2,
        required_roof_area_m2,
        roof_load_kn_m2: PV_ROOF_LOAD_KN_M2,
        annual_generation_kwh: annual_gen,
        mean_monthly_generation_kwh: mean_monthly,
        offset_percent,
        refused_above_limit,
        light,
        summary_sentence,
        planes,
        placements,
        months,
        warnings,
        assumptions,
    }
}

/// Plan marks drawing the placed PV modules on the roof planes (*screen equals paper*).
pub fn pv_plan_marks(project: &Project) -> Vec<PlanMark> {
    if !project.electrical.pv.enabled || !project.electrical.pv.show_on_plan {
        return Vec::new();
    }
    let report = photovoltaic_report(project);
    if report.placements.is_empty() {
        return Vec::new();
    }
    let mut marks = Vec::new();
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for p in &report.placements {
        let mut loop_pts = p.corners.to_vec();
        loop_pts.push(p.corners[0]);
        marks.push(PlanMark::Line {
            points: loop_pts,
            dashed: false,
            heavy: true,
        });
        // Subtle center cell divider line so the rectangle reads immediately as a PV panel.
        let mid_a = p.corners[0].midpoint(p.corners[1]);
        let mid_b = p.corners[3].midpoint(p.corners[2]);
        marks.push(PlanMark::Line {
            points: vec![mid_a, mid_b],
            dashed: false,
            heavy: false,
        });
        let center = p.corners[0].midpoint(p.corners[2]);
        sum_x += center.x;
        sum_y += center.y;
    }
    let n = report.placements.len() as f64;
    marks.push(PlanMark::Text {
        anchor: Point::new(sum_x / n, sum_y / n),
        text: format!(
            "FV {} kWp ({}x)",
            decimal(report.installed_kwp, 2),
            report.placements.len()
        ),
    });
    marks
}

/// UTF-8 BOM CSV export of the monthly PV generation and system summary.
pub fn photovoltaic_csv(project: &Project) -> String {
    let r = photovoltaic_report(project);
    let mut csv = String::from(
        "\u{feff}mes,dias,hsp_kwh_m2_dia,geracao_kwh,consumo_kwh,saldo_kwh\r\n",
    );
    for m in &r.months {
        let _ = writeln!(
            csv,
            "{},{},{:.2},{:.1},{:.1},{:.1}\r",
            m.month, m.days, m.hsp_kwh_m2_day, m.generation_kwh, m.consumption_kwh, m.balance_kwh
        );
    }
    let _ = writeln!(
        csv,
        "ANUAL,365,{:.2},{:.1},{:.1},{:.1}\r",
        r.mean_hsp_kwh_m2_day,
        r.annual_generation_kwh,
        r.monthly_consumption_kwh * 12.0,
        r.annual_generation_kwh - r.monthly_consumption_kwh * 12.0
    );
    csv
}

/// Printable A3 landscape SVG sheet of the photovoltaic pre-sizing (#126):
/// system card, 12-month bar chart vs. consumption, roof plane allocation and notes.
pub fn photovoltaic_sheet_svg(project: &Project) -> String {
    let r = photovoltaic_report(project);
    let width = 420.0;
    let height = 297.0;
    let ink = "#1d2836";
    let muted = "#4a5d73";
    let blue = "#1565c0";
    let font = "font-family=\"Arial, sans-serif\"";

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height}mm\" viewBox=\"0 0 {width} {height}\">\n\
         <rect width=\"{width}\" height=\"{height}\" fill=\"#ffffff\"/>\n\
         <rect x=\"10\" y=\"10\" width=\"400\" height=\"277\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>\n"
    );
    let text = |svg: &mut String, x: f64, y: f64, size: f64, bold: bool, color: &str, value: &str| {
        let _ = writeln!(
            svg,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" {font} font-size=\"{size}\"{} fill=\"{color}\">{}</text>",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        );
    };

    text(
        &mut svg,
        18.0,
        22.0,
        5.0,
        true,
        ink,
        "PRÉ-DIMENSIONAMENTO FOTOVOLTAICO (ABNT NBR 16690 · LEI 14.300)",
    );
    text(
        &mut svg,
        18.0,
        28.5,
        3.0,
        false,
        muted,
        &format!("{} · {} · {}", project.name, r.city_label, r.summary_sentence),
    );

    // Left panel: technical specification summary.
    let mut y = 40.0;
    text(&mut svg, 18.0, y, 3.5, true, ink, "1. Resumo do sistema");
    y += 5.5;
    let specs = [
        ("Cidade / irradiação média", format!("{} · {} kWh/m²·dia", r.city_label, decimal(r.mean_hsp_kwh_m2_day, 2))),
        ("Consumo mensal", format!("{:.0} kWh/mês ({})", r.monthly_consumption_kwh, if r.consumption_is_estimated { "estimado" } else { "informado" })),
        ("Potência instalada", format!("{} kWp ({} módulos de {:.0} Wp)", decimal(r.installed_kwp, 2), r.placements.len().max(r.modules_required), r.settings.module_wp)),
        ("Inversor sugerido", format!("{} kW CA (FDI = {})", decimal(r.inverter_kw, 1), decimal(r.dc_ac_ratio, 2))),
        ("Proteção CA", format!("Disjuntor {} · cabo {} mm² · I = {} A",
            r.ac_breaker_a.map_or("—".to_owned(), |b| format!("{b:.0} A")),
            r.ac_wire_mm2.map_or("—".to_owned(), |w| decimal(w, 1)),
            decimal(r.ac_current_a, 1))),
        ("Geração estimada", format!("{:.0} kWh/mês ({:.0} kWh/ano · {:.0} % do consumo)", r.mean_monthly_generation_kwh, r.annual_generation_kwh, r.offset_percent)),
        ("Área e peso no telhado", format!("{} m² · sobrecarga {:.2} kN/m² (~15 kg/m²)", decimal(r.required_roof_area_m2, 1), r.roof_load_kn_m2)),
    ];
    for (k, v) in specs {
        text(&mut svg, 18.0, y, 2.6, true, muted, k);
        text(&mut svg, 68.0, y, 2.7, false, ink, &v);
        y += 5.5;
    }

    // Roof planes allocation table.
    y += 5.0;
    text(&mut svg, 18.0, y, 3.5, true, ink, "2. Planos de telhado");
    y += 5.5;
    if r.planes.is_empty() {
        text(&mut svg, 18.0, y, 2.6, false, muted, "Nenhum telhado modelado; área estimada pela planta.");
    } else {
        for p in &r.planes {
            text(
                &mut svg,
                18.0,
                y,
                2.5,
                false,
                ink,
                &format!(
                    "{} · água {} ({}, incl. {:.0} %, área {} m², f={}): {} de {} módulos cabem",
                    p.roof_label,
                    p.plane_index,
                    p.orientation_label,
                    p.slope_percent,
                    decimal(p.area_m2, 1),
                    decimal(p.orientation_factor, 2),
                    p.placed_panels,
                    p.capacity_panels
                ),
            );
            y += 5.0;
        }
    }

    // Right panel: 12-month bar chart.
    let cx = 215.0;
    let cy = 40.0;
    text(&mut svg, cx, cy, 3.5, true, ink, "3. Geração mensal estimada (kWh/mês)");
    let chart_x = cx + 10.0;
    let chart_y = cy + 8.0;
    let chart_w = 176.0;
    let chart_h = 85.0;
    let _ = writeln!(
        svg,
        "<rect x=\"{chart_x}\" y=\"{chart_y}\" width=\"{chart_w}\" height=\"{chart_h}\" fill=\"#f8fafc\" stroke=\"#cbd5e1\" stroke-width=\"0.3\"/>"
    );
    let max_val = r
        .months
        .iter()
        .map(|m| m.generation_kwh)
        .fold(r.monthly_consumption_kwh, f64::max)
        .max(10.0)
        * 1.15;
    let bar_slot = chart_w / 12.0;
    for (i, m) in r.months.iter().enumerate() {
        let bh = (m.generation_kwh / max_val) * (chart_h - 10.0);
        let bx = chart_x + i as f64 * bar_slot + 2.2;
        let by = chart_y + chart_h - bh;
        let _ = writeln!(
            svg,
            "<rect x=\"{bx:.2}\" y=\"{by:.2}\" width=\"{:.2}\" height=\"{bh:.2}\" fill=\"{blue}\" fill-opacity=\"0.82\"/>",
            bar_slot - 4.4
        );
        text(&mut svg, bx + 1.0, by - 1.5, 2.0, false, ink, &format!("{:.0}", m.generation_kwh));
        text(&mut svg, bx + 1.5, chart_y + chart_h + 4.2, 2.2, true, muted, &m.month);
    }
    // Dashed line for monthly consumption.
    let cons_y = chart_y + chart_h - (r.monthly_consumption_kwh / max_val) * (chart_h - 10.0);
    let _ = writeln!(
        svg,
        "<line x1=\"{chart_x}\" y1=\"{cons_y:.2}\" x2=\"{:.2}\" y2=\"{cons_y:.2}\" stroke=\"#c62828\" stroke-width=\"0.5\" stroke-dasharray=\"3 2\"/>",
        chart_x + chart_w
    );
    text(
        &mut svg,
        chart_x + chart_w - 42.0,
        cons_y - 1.8,
        2.2,
        true,
        "#c62828",
        &format!("Consumo: {:.0} kWh", r.monthly_consumption_kwh),
    );

    // Monthly compact table below chart.
    let mut ty = chart_y + chart_h + 14.0;
    text(&mut svg, cx, ty, 3.0, true, ink, "Tabela mensal (HSP · geração · saldo)");
    ty += 5.0;
    for pair in r.months.chunks(2) {
        let left = &pair[0];
        text(
            &mut svg,
            cx,
            ty,
            2.3,
            false,
            ink,
            &format!(
                "{}: HSP {} · {:.0} kWh (saldo {:+.0})",
                left.month,
                decimal(left.hsp_kwh_m2_day, 2),
                left.generation_kwh,
                left.balance_kwh
            ),
        );
        if let Some(right) = pair.get(1) {
            text(
                &mut svg,
                cx + 95.0,
                ty,
                2.3,
                false,
                ink,
                &format!(
                    "{}: HSP {} · {:.0} kWh (saldo {:+.0})",
                    right.month,
                    decimal(right.hsp_kwh_m2_day, 2),
                    right.generation_kwh,
                    right.balance_kwh
                ),
            );
        }
        ty += 4.6;
    }

    // Bottom warnings & assumptions.
    let mut fy = 232.0;
    for w in r.warnings.iter().take(4) {
        text(&mut svg, 18.0, fy, 2.3, false, "#9a5b00", &format!("• {w}"));
        fy += 4.2;
    }
    fy += 2.0;
    for note in &r.assumptions {
        text(&mut svg, 18.0, fy, 2.2, false, muted, note);
        fy += 4.0;
    }

    svg.push_str("</svg>\n");
    svg
}

impl Project {
    /// Photovoltaic system pre-sizing report (#126).
    pub fn photovoltaic_report(&self) -> PhotovoltaicReport {
        photovoltaic_report(self)
    }

    /// Placed PV module plan marks on roof planes (#126).
    pub fn pv_plan_marks(&self) -> Vec<PlanMark> {
        pv_plan_marks(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        codes::SiteClimate,
        roof::{Roof, RoofSide, RoofStyle},
        samples::SampleProject,
    };

    /// Worked example 1: São Paulo / SP, 400 kWh/month, 550 Wp modules, PR = 0.78, single-phase (30 kWh availability).
    /// Target gen = 370 kWh/month = 12.164 kWh/day; mean HSP SP ≈ 4.44 kWh/m²·day.
    /// Required kWp ≈ 12.164 / (4.44 * 0.78) ≈ 3.51 kWp -> 7 modules = 3.85 kWp -> 3.0 or 4.0 kW inverter!
    #[test]
    fn worked_example_sao_paulo_400kwh_matches_cresesb_hand_calculation() {
        let mut project = SampleProject::CasaTerrea.build();
        project.codes.site = SiteClimate::find_preset("sao_paulo_sp")
            .unwrap()
            .to_site_climate();
        project.electrical.pv.enabled = true;
        project.electrical.pv.monthly_consumption_kwh = Some(400.0);

        let report = project.photovoltaic_report();
        assert!(!report.city_is_fallback);
        assert_eq!(report.city_label, "São Paulo / SP");
        assert!((report.mean_hsp_kwh_m2_day - 4.44).abs() < 0.08);
        assert!(
            (report.required_kwp - 3.51).abs() < 0.25,
            "required_kwp={}",
            report.required_kwp
        );
        assert!((6..=8).contains(&report.modules_required));
        assert!(report.inverter_kw >= 3.0 && report.inverter_kw <= 5.0);
        assert_eq!(report.months.len(), 12);
        let sum_months: f64 = report.months.iter().map(|m| m.generation_kwh).sum();
        assert!((sum_months - report.annual_generation_kwh).abs() < 1e-6);
        assert!(report.months[0].generation_kwh > report.months[5].generation_kwh);
        let svg = photovoltaic_sheet_svg(&project);
        assert!(svg.contains("PRÉ-DIMENSIONAMENTO FOTOVOLTAICO"));
        assert!(svg.contains("</svg>"));
        let csv = photovoltaic_csv(&project);
        assert!(csv.contains("ANUAL,365"));
    }

    /// Worked example 2: Fortaleza / CE (high solar irradiation ~5.75 kWh/m²·day) produces
    /// significantly more energy per kWp than Curitiba / PR (~4.22 kWh/m²·day), and a gable
    /// roof places panels on its North-facing plane first before using the South plane.
    #[test]
    fn fortaleza_generates_more_than_curitiba_and_gable_roof_prefers_north_plane() {
        let mut proj = Project::default();
        proj.set_terrain(Point::new(0.0, 0.0), Point::new(20.0, 20.0));
        // Gable roof with ridge along X (high_side = North -> planes drain North and South).
        let mut roof = Roof::new(Point::new(2.0, 2.0), Point::new(14.0, 12.0), RoofSide::North, 4.4, 2.8);
        roof.style = RoofStyle::Gable;
        roof.id = 1;
        proj.structure.roofs.push(roof);
        proj.electrical.pv.enabled = true;
        proj.electrical.pv.monthly_consumption_kwh = Some(500.0);

        proj.codes.site = SiteClimate::find_preset("fortaleza_ce")
            .unwrap()
            .to_site_climate();
        let r_for = proj.photovoltaic_report();

        proj.codes.site = SiteClimate::find_preset("curitiba_pr")
            .unwrap()
            .to_site_climate();
        let r_cwb = proj.photovoltaic_report();

        assert!(r_for.mean_hsp_kwh_m2_day > r_cwb.mean_hsp_kwh_m2_day + 1.2);
        assert!(r_for.modules_required < r_cwb.modules_required);
        // On a 12x10 m gable roof, North plane is 12x5 m (60 m²) -> easily fits all modules on North!
        assert_eq!(r_for.modules_placed, r_for.modules_required);
        assert!(
            r_for
                .placements
                .iter()
                .all(|p| p.orientation_label == "Norte"),
            "placements: {:?}",
            r_for.placements
        );
        // Plan marks are generated for placed panels.
        assert!(!proj.pv_plan_marks().is_empty());
    }
}
