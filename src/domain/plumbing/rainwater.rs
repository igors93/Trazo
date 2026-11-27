//! Rainwater (#122, NBR 10844 and NBR 15527): the gutters and downpipes of
//! every roof from its area and the city's rainfall, a cistern to reuse the
//! water, and two warnings: a roof that drains onto the neighbour (Código
//! Civil, art. 1.300) and one that drains into a pool.
//!
//! - Design flow `Q = I·A/60` (L/min), with the city's rainfall intensity
//!   `I` (mm/h) or, for roofs up to 100 m², the standard's 150 mm/h; the
//!   area of a sloping roof is `A = a·b·(1 + s/2)` (NBR 10844 5.2).
//! - Gutters: semicircular, falling 0.5 %, Manning with the pack's `n`;
//!   each carries the flow towards one of its downpipes.
//! - Downpipes: the inner diameter from NBR 10844's Figure 3 (digitised in
//!   the pack: the water depth in the gutter `H` and the downpipe's length
//!   `L`), at least the standard's 70 mm, rounded to a commercial pipe.
//! - Cistern: NBR 15527's practical German method, 6 % of the lesser of the
//!   year's rain on the roof (runoff 0.8) and the year's non-drinking use.

use serde::Deserialize;

use crate::domain::construction::roof_plan::roof_plans;
use crate::domain::geometry::{Point, distance_to_segment};
use crate::domain::model::Project;

use super::cold_water::{PEOPLE_PER_BEDROOM, bedroom_count, decimal};
use super::rules::section;

/// Commercial semicircular gutters, mm.
const GUTTERS_MM: [f64; 4] = [100.0, 125.0, 150.0, 200.0];
/// Commercial downpipes (PVC), mm.
const DOWNPIPES_MM: [f64; 3] = [75.0, 100.0, 150.0];
/// Rain of a year when the city's is not given, mm (a conservative
/// Brazilian figure).
pub const DEFAULT_ANNUAL_RAIN_MM: f64 = 1200.0;
/// Share of the rain that runs off a roof (NBR 15527).
const RUNOFF: f64 = 0.8;
/// Water that need not be drinkable, per person and day: flushing,
/// laundry, cleaning and the garden, L.
const NON_DRINKING_L: f64 = 50.0;
/// First rain thrown away, L per m² of roof (NBR 15527: 2 mm).
const FIRST_FLUSH_L_M2: f64 = 2.0;
/// Commercial cisterns, L.
const CISTERNS_L: [f64; 9] = [
    500.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0, 7500.0, 10_000.0, 15_000.0,
];
/// An eave closer than this to the lot's boundary sheds onto the
/// neighbour's side unless its gutter takes every drop, m.
const NEAR_BOUNDARY_M: f64 = 0.50;
/// A gutter or downpipe this close to a pool drains towards it, m.
const NEAR_POOL_M: f64 = 2.0;

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct Roughness {
    material: String,
    n: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct LengthCurve {
    length: Option<f64>,
    c: [f64; 3],
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct Abacus {
    depths: Vec<[f64; 3]>,
    lengths: Vec<LengthCurve>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct Curves {
    edge: Abacus,
}

/// The storm-water rules of the pack (NBR 10844).
#[derive(Clone, Debug, PartialEq, Deserialize)]
struct StormRules {
    default_rain: f64,
    default_rain_max_area: f64,
    min_downpipe: f64,
    min_slope: f64,
    roughness: Vec<Roughness>,
    diameter_axis: [f64; 2],
    depth_range: [f64; 2],
    min_length: f64,
    downpipe_curves: Curves,
}

impl StormRules {
    fn n(&self) -> f64 {
        self.roughness
            .iter()
            .find(|r| r.material == "pvc")
            .map_or(0.011, |r| r.n)
    }
}

/// `ln Q` on a length curve at diameter `d`.
fn ln_q(c: &[f64; 3], d: f64) -> f64 {
    let x = d.ln();
    c[0] + c[1] * x + c[2] * x * x
}

/// The diameter at which a length curve carries `q`, by bisection on the
/// chart's diameter axis; `None` above the chart.
fn d_on_curve(c: &[f64; 3], q: f64, [lo, hi]: [f64; 2]) -> Option<f64> {
    let target = q.max(1e-9).ln();
    if ln_q(c, hi) < target {
        return None;
    }
    if ln_q(c, lo) >= target {
        return Some(lo);
    }
    let (mut a, mut b) = (lo, hi);
    for _ in 0..60 {
        let m = (a + b) / 2.0;
        if ln_q(c, m) < target {
            a = m;
        } else {
            b = m;
        }
    }
    Some(b)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// The inner diameter a downpipe needs (NBR 10844 Figure 3, gutter outlet
/// with a sharp edge), mm; `None` when the flow is above the chart.
fn downpipe_inner_mm(rules: &StormRules, q: f64, h: f64, l: f64) -> Option<f64> {
    let abacus = &rules.downpipe_curves.edge;
    let [h_lo, h_hi] = rules.depth_range;
    let h = h.clamp(h_lo, h_hi);
    let at = |row: &[f64; 3]| row[1] * q + row[2];
    let hi = abacus.depths.iter().position(|r| r[0] >= h - 1e-9)?;
    let lo = hi.saturating_sub(1);
    let (r0, r1) = (abacus.depths[lo], abacus.depths[hi]);
    let by_depth = if hi == lo || (r1[0] - r0[0]).abs() < 1e-9 {
        at(&r1)
    } else {
        lerp(at(&r0), at(&r1), (h - r0[0]) / (r1[0] - r0[0]))
    };
    let l = l.max(rules.min_length);
    let length = |c: &LengthCurve| c.length.unwrap_or(f64::INFINITY);
    let j = abacus
        .lengths
        .iter()
        .position(|c| length(c) >= l - 1e-9)
        .unwrap_or(abacus.lengths.len() - 1);
    let i = j.saturating_sub(1);
    let dj = d_on_curve(&abacus.lengths[j].c, q, rules.diameter_axis)?;
    let by_length = if i == j || !length(&abacus.lengths[j]).is_finite() {
        dj
    } else {
        let di = d_on_curve(&abacus.lengths[i].c, q, rules.diameter_axis)?;
        let (li, lj) = (length(&abacus.lengths[i]), length(&abacus.lengths[j]));
        lerp(di, dj, ((l - li) / (lj - li)).clamp(0.0, 1.0))
    };
    Some(by_depth.max(by_length).max(rules.min_downpipe))
}

/// Flow a full semicircular gutter carries at `slope` (Manning), L/min.
fn gutter_capacity_l_min(diameter_mm: f64, slope: f64, n: f64) -> f64 {
    let d = diameter_mm / 1000.0;
    let area = std::f64::consts::PI * d * d / 8.0;
    let radius = d / 4.0;
    60_000.0 * area / n * radius.powf(2.0 / 3.0) * slope.sqrt()
}

/// One gutter, sized.
#[derive(Clone, Debug, PartialEq)]
pub struct GutterDesign {
    pub roof: String,
    pub from: Point,
    pub to: Point,
    /// Area that drains into it, with the slope's allowance, m².
    pub area_m2: f64,
    /// Design flow, L/min.
    pub flow_l_min: f64,
    pub gutter_mm: f64,
    /// Downpipes drawn, and the downpipes it needs.
    pub downpipes: Vec<Point>,
    pub downpipes_needed: usize,
    pub downpipe_mm: f64,
}

/// The rainwater design.
#[derive(Clone, Debug, PartialEq)]
pub struct Rainwater {
    /// Rainfall intensity, mm/h, and whether it is the city's.
    pub intensity_mm_h: f64,
    pub city_intensity: bool,
    pub gutters: Vec<GutterDesign>,
    /// No roof drawn: the slab of the whole house, drained at its corners.
    pub flat_slab: bool,
    pub roof_area_m2: f64,
    pub total_flow_l_min: f64,
    /// Cistern for reuse: the year's rain on the roof and the year's
    /// non-drinking use, L, and the cistern.
    pub annual_rain_mm: f64,
    pub annual_rain_given: bool,
    pub yearly_yield_l: f64,
    pub yearly_use_l: f64,
    pub cistern_l: f64,
    pub first_flush_l: f64,
    /// Gutters or downpipes over or next to the neighbour's lot.
    pub onto_neighbour: Vec<String>,
    /// … and over the neighbour's lot itself (the law forbids it).
    pub over_boundary: Vec<String>,
    /// Gutters or downpipes draining towards a pool.
    pub into_pool: Vec<String>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

fn next(sizes: &[f64], value: f64) -> Option<f64> {
    sizes.iter().copied().find(|s| *s >= value - 1e-9)
}

/// The rainwater design, or `None` without a roof or walls.
pub fn rainwater(project: &Project) -> Option<Rainwater> {
    let rules: StormRules = section(project, "storm")?;
    let n = rules.n();
    let plans = roof_plans(project);
    let wall_points: Vec<Point> = project
        .walls
        .iter()
        .flat_map(|w| [w.start, w.end])
        .collect();
    if plans.is_empty() && wall_points.is_empty() {
        return None;
    }

    // The gutters to size: the roofs', or the slab's edges.
    let mut runs: Vec<(String, Point, Point, f64, Vec<Point>)> = Vec::new();
    let flat_slab = plans.is_empty();
    if flat_slab {
        let min = wall_points.iter().fold(wall_points[0], |m, p| {
            Point::new(m.x.min(p.x), m.y.min(p.y))
        });
        let max = wall_points.iter().fold(wall_points[0], |m, p| {
            Point::new(m.x.max(p.x), m.y.max(p.y))
        });
        let area = (max.x - min.x) * (max.y - min.y);
        // A flat slab falls to two opposite edges.
        let (a, b, c, d) = (
            Point::new(min.x, min.y),
            Point::new(max.x, min.y),
            Point::new(min.x, max.y),
            Point::new(max.x, max.y),
        );
        runs.push(("Laje".to_owned(), a, b, area / 2.0, vec![a, b]));
        runs.push(("Laje".to_owned(), c, d, area / 2.0, vec![c, d]));
    } else {
        for plan in &plans {
            let allowance = 1.0 + plan.slope_percent / 100.0 / 2.0;
            for gutter in &plan.gutters {
                runs.push((
                    plan.label.clone(),
                    gutter.from,
                    gutter.to,
                    gutter.area_m2 * allowance,
                    gutter.downpipes.clone(),
                ));
            }
        }
    }
    let roof_area_m2: f64 = runs.iter().map(|r| r.3).sum();
    let city = project.codes.site.rain_intensity_mm_h;
    let intensity_mm_h = city.unwrap_or(rules.default_rain);

    let eave_m = project.wall_height.max(rules.min_length);
    let h = rules.depth_range[0];
    let capacity_100 = {
        // The flow a 100 mm downpipe takes, found on the chart.
        let (mut a, mut b) = (1.0, 5000.0);
        for _ in 0..60 {
            let m = (a + b) / 2.0;
            match downpipe_inner_mm(&rules, m, h, eave_m) {
                Some(d) if d <= 100.0 => a = m,
                _ => b = m,
            }
        }
        a
    };
    let mut gutters = Vec::new();
    for (roof, from, to, area, downpipes) in runs {
        let flow = intensity_mm_h * area / 60.0;
        let needed = ((flow / capacity_100).ceil() as usize)
            .max(downpipes.len())
            .max(1);
        let per_outlet = flow / needed as f64;
        let gutter_mm = GUTTERS_MM
            .iter()
            .copied()
            .find(|d| gutter_capacity_l_min(*d, rules.min_slope, n) >= per_outlet)
            .unwrap_or(GUTTERS_MM[GUTTERS_MM.len() - 1]);
        let inner = downpipe_inner_mm(&rules, per_outlet, h, eave_m).unwrap_or(100.0);
        let downpipe_mm = next(&DOWNPIPES_MM, inner).unwrap_or(150.0);
        gutters.push(GutterDesign {
            roof,
            from,
            to,
            area_m2: area,
            flow_l_min: flow,
            gutter_mm,
            downpipes,
            downpipes_needed: needed,
            downpipe_mm,
        });
    }
    let total_flow_l_min: f64 = gutters.iter().map(|g| g.flow_l_min).sum();

    // The cistern.
    let settings = &project.plumbing;
    let residents = settings
        .residents
        .filter(|n| *n > 0)
        .unwrap_or((PEOPLE_PER_BEDROOM * bedroom_count(project)).max(PEOPLE_PER_BEDROOM));
    let annual_rain_mm = settings
        .annual_rain_mm
        .filter(|r| *r > 0.0)
        .unwrap_or(DEFAULT_ANNUAL_RAIN_MM);
    let yearly_yield_l = annual_rain_mm * roof_area_m2 * RUNOFF;
    let yearly_use_l = f64::from(residents) * NON_DRINKING_L * 365.0;
    let cistern_l = next(&CISTERNS_L, 0.06 * yearly_yield_l.min(yearly_use_l))
        .unwrap_or(CISTERNS_L[CISTERNS_L.len() - 1]);
    let first_flush_l = FIRST_FLUSH_L_M2 * roof_area_m2;

    // Where the water falls.
    let boundary: Vec<Point> = project
        .terrain
        .as_ref()
        .map(crate::domain::model::Terrain::outline)
        .unwrap_or_default();
    let inside = |p: Point| project.terrain.as_ref().is_none_or(|t| t.contains(p));
    let to_boundary = |p: Point| {
        (0..boundary.len())
            .map(|i| distance_to_segment(p, boundary[i], boundary[(i + 1) % boundary.len()]))
            .fold(f64::INFINITY, f64::min)
    };
    let mut onto_neighbour = Vec::new();
    let mut over_boundary = Vec::new();
    if boundary.len() >= 3 && !flat_slab {
        for gutter in &gutters {
            let name = format!("Calha de {}", gutter.roof);
            if !inside(gutter.from)
                || !inside(gutter.to)
                || gutter.downpipes.iter().any(|p| !inside(*p))
            {
                over_boundary.push(name);
            } else if to_boundary(gutter.from).min(to_boundary(gutter.to)) < NEAR_BOUNDARY_M {
                onto_neighbour.push(name);
            }
        }
    }
    let pools: Vec<Vec<Point>> = project
        .pool_specs()
        .iter()
        .filter_map(|(room, _)| project.room_polygon(room))
        .collect();
    let to_pool = |p: Point, pool: &[Point]| {
        if crate::domain::geometry::polygon_contains_point(pool, p) {
            return 0.0;
        }
        (0..pool.len())
            .map(|i| distance_to_segment(p, pool[i], pool[(i + 1) % pool.len()]))
            .fold(f64::INFINITY, f64::min)
    };
    let mut into_pool = Vec::new();
    for gutter in &gutters {
        let near = pools.iter().any(|pool| {
            let along = (0..=10).map(|k| gutter.from.lerp(gutter.to, f64::from(k) / 10.0));
            along
                .chain(gutter.downpipes.iter().copied())
                .any(|p| to_pool(p, pool) < NEAR_POOL_M)
        });
        if near {
            into_pool.push(format!("Calha de {}", gutter.roof));
        }
    }

    let mut warnings = Vec::new();
    for name in &over_boundary {
        warnings.push(format!(
            "{name} fica sobre o lote do vizinho: a água do telhado não pode cair no terreno dele (Código Civil, art. 1.300). Recue o beiral para dentro do seu lote."
        ));
    }
    for name in &onto_neighbour {
        warnings.push(format!(
            "{name} fica a menos de {} m da divisa: a calha é obrigatória e a descida tem de despejar no seu lote, nunca no do vizinho (Código Civil, art. 1.300).",
            decimal(NEAR_BOUNDARY_M, 2)
        ));
    }
    for name in &into_pool {
        warnings.push(format!(
            "{name} despeja perto da piscina: leve a descida para longe dela; a água do telhado traz folhas e sujeira e desequilibra o cloro."
        ));
    }
    for gutter in gutters
        .iter()
        .filter(|g| g.downpipes_needed > g.downpipes.len())
    {
        warnings.push(format!(
            "Calha de {} ({} m²): precisa de {} descida(s) de {} mm para {} mm/h; a planta de cobertura tem {}.",
            gutter.roof,
            decimal(gutter.area_m2, 1),
            gutter.downpipes_needed,
            decimal(gutter.downpipe_mm, 0),
            decimal(intensity_mm_h, 0),
            gutter.downpipes.len()
        ));
    }
    if city.is_none() && roof_area_m2 > rules.default_rain_max_area {
        warnings.push(format!(
            "A chuva de {} mm/h só vale para coberturas até {} m² (NBR 10844); esta tem {} m². Informe a cidade (local_clima) para usar a chuva dela.",
            decimal(rules.default_rain, 0),
            decimal(rules.default_rain_max_area, 0),
            decimal(roof_area_m2, 0)
        ));
    }

    let mut notes = vec![format!(
        "Q = I·A/60 com I = {} mm/h{}: {} L/min de {} m² de cobertura; calhas semicirculares com {} % de caimento.",
        decimal(intensity_mm_h, 0),
        if city.is_some() {
            " (da cidade)"
        } else {
            " (NBR 10844, coberturas até 100 m²)"
        },
        decimal(total_flow_l_min, 0),
        decimal(roof_area_m2, 1),
        decimal(rules.min_slope * 100.0, 1)
    )];
    if flat_slab {
        notes.push(
            "Sem telhado desenhado: considerei a laje da casa inteira, caindo para duas bordas, com descidas nos cantos. Desenhe o telhado para o cálculo dele."
                .to_owned(),
        );
    }
    notes.push(format!(
        "Cisterna para reúso (NBR 15527, método prático alemão): 6 % do menor entre a chuva do ano no telhado ({} L, com {} mm/ano) e o uso não potável do ano ({} L, {} L por pessoa por dia): cisterna de {} L, descartando os primeiros {} L de cada chuva.",
        decimal(yearly_yield_l, 0),
        decimal(annual_rain_mm, 0),
        decimal(yearly_use_l, 0),
        decimal(NON_DRINKING_L, 0),
        decimal(cistern_l, 0),
        decimal(first_flush_l, 0)
    ));
    if settings.annual_rain_mm.is_none() {
        notes.push(format!(
            "Chuva anual suposta em {} mm; informe a da cidade para refazer a cisterna.",
            decimal(DEFAULT_ANNUAL_RAIN_MM, 0)
        ));
    }
    notes.push(
        "Água da cisterna só para descarga, lavar roupa e quintal: tubos separados, nunca ligados à água de beber, e cisterna tampada e telada contra o mosquito da dengue."
            .to_owned(),
    );
    notes.push(
        "A chuva vai para a sarjeta ou para a cisterna, nunca para o esgoto nem para o lote do vizinho."
            .to_owned(),
    );

    Some(Rainwater {
        intensity_mm_h,
        city_intensity: city.is_some(),
        gutters,
        flat_slab,
        roof_area_m2,
        total_flow_l_min,
        annual_rain_mm,
        annual_rain_given: settings.annual_rain_mm.is_some(),
        yearly_yield_l,
        yearly_use_l,
        cistern_l,
        first_flush_l,
        onto_neighbour,
        over_boundary,
        into_pool,
        warnings,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> StormRules {
        section(&Project::default(), "storm").unwrap()
    }

    #[test]
    fn a_100_mm_gutter_carries_130_l_min_at_half_a_percent() {
        let q = gutter_capacity_l_min(100.0, 0.005, 0.011);
        assert!((q - 130.0).abs() < 2.0, "{q}");
    }

    #[test]
    fn the_downpipe_chart_grows_with_the_flow_and_keeps_the_least() {
        let r = rules();
        let small = downpipe_inner_mm(&r, 50.0, 50.0, 3.0).unwrap();
        let large = downpipe_inner_mm(&r, 600.0, 50.0, 3.0).unwrap();
        assert!(small >= r.min_downpipe - 1e-9);
        assert!(large > small);
    }

    #[test]
    fn a_house_without_a_roof_drains_its_slab_and_gets_a_cistern() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        project
            .add_room(Point::new(2.0, 5.0), Point::new(8.0, 13.0), "Casa")
            .unwrap();
        let rain = rainwater(&project).unwrap();
        assert!(rain.flat_slab);
        assert_eq!(rain.gutters.len(), 2);
        assert!(rain.cistern_l >= 500.0);
        assert!(rain.notes.iter().any(|n| n.contains("dengue")));
    }
}
