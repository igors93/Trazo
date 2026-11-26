//! In-ground swimming pools (*piscinas*): a reinforced-concrete box in the
//! ground that holds water when full and holds back the earth when empty.
//!
//! A room named "Piscina" (or one with an explicit [`PoolSpec`]) is a pool.
//! Its walls are concrete below the ground, not masonry: they leave the
//! masonry-panel, lintel, wall-foundation and global-stability checks and are
//! checked here instead.
//!
//! Pre-sizing, per metre of wall, as a cantilever fixed in the floor slab
//! (conservative for long walls, NBR 6118 §14.7):
//! - full pool before backfilling: water `M = γw·H³/6`;
//! - empty pool: earth at rest `K0 = 1 − sin φ'` plus the edge surcharge,
//!   `M = K0·γ·H³/6 + K0·q·H²/2` (with groundwater, the submerged weight
//!   plus the water);
//! - ULS `γf = 1.4`, rectangular stress block, CA-50, cover 3 cm;
//! - shear without stirrups, `V_Rd1` of NBR 6118 §19.4.1;
//! - watertightness: steel stress in service limited to 160 MPa, minimum
//!   steel 0.15 % on each face (NBR 6118 Table 17.3).
//!
//! Also checked: the floor's pressure on the soil, flotation of the empty
//! pool over groundwater, foundations inside the influence zone of the
//! excavation (NBR 6122, foundations at different levels: 60° from the
//! vertical in weak soil, 45° otherwise), the lot boundary, collapsible soil
//! and the concrete class for durability (NBR 6118 Table 7.1, CAA II → C25).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::domain::fixtures::FixtureKind;
use crate::domain::geometry::{Point, distance_to_segment, polygon_area, polygon_contains_point};
use crate::domain::model::{Project, Room};
use crate::domain::protection::site_risks::SiteRisk;
use crate::domain::structure::CONCRETE_KN_M3;

pub const DEFAULT_DEPTH_M: f64 = 1.40;
pub const DEFAULT_WALL_M: f64 = 0.15;
pub const DEFAULT_FLOOR_M: f64 = 0.15;
/// People, furniture and deck at the edge, kPa.
pub const DEFAULT_EDGE_KPA: f64 = 2.5;
/// Cover on faces against water and soil, m.
pub const COVER_M: f64 = 0.03;
/// Least concrete class for a pool (NBR 6118 Table 7.1, CAA II).
pub const DURABLE_FCK_MPA: f64 = 25.0;
/// Steel stress in service that keeps cracks fine enough to hold water, MPa.
pub const WATERTIGHT_STEEL_MPA: f64 = 160.0;
/// Thinnest concrete wall or floor Trazo accepts for a pool, m.
pub const MIN_THICKNESS_M: f64 = 0.12;
/// Lean concrete or gravel under the floor slab, m.
const BLINDING_M: f64 = 0.05;
/// A neighbour's foundation or wall near the excavation, as a surcharge, kPa.
const NEIGHBOUR_SURCHARGE_KPA: f64 = 10.0;
const WATER_KN_M3: f64 = 10.0;
/// Base depth Trazo assumes for a footing without a survey, m.
const FOOTING_BASE_M: f64 = 1.0;
/// Base depth of a strip footing under a wall, m.
const STRIP_BASE_M: f64 = 0.5;
/// Hours to filter the whole pool once (usual practice for houses).
const TURNOVER_H: f64 = 6.0;

fn default_depth() -> f64 {
    DEFAULT_DEPTH_M
}
fn default_wall() -> f64 {
    DEFAULT_WALL_M
}
fn default_floor() -> f64 {
    DEFAULT_FLOOR_M
}
fn default_edge() -> f64 {
    DEFAULT_EDGE_KPA
}

/// What the designer says about one pool.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PoolSpec {
    pub room_id: u64,
    /// Inner depth from the coping to the floor at the deepest point, m.
    #[serde(default = "default_depth")]
    pub depth_m: f64,
    #[serde(default = "default_wall")]
    pub wall_thickness_m: f64,
    #[serde(default = "default_floor")]
    pub floor_thickness_m: f64,
    /// Surcharge at the edge (deck, people), kPa.
    #[serde(default = "default_edge")]
    pub edge_surcharge_kpa: f64,
}

impl PoolSpec {
    pub fn new(room_id: u64) -> Self {
        Self {
            room_id,
            depth_m: DEFAULT_DEPTH_M,
            wall_thickness_m: DEFAULT_WALL_M,
            floor_thickness_m: DEFAULT_FLOOR_M,
            edge_surcharge_kpa: DEFAULT_EDGE_KPA,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let within = |value: f64, low: f64, high: f64, what: &str| {
            if value.is_finite() && (low..=high).contains(&value) {
                Ok(())
            } else {
                Err(format!(
                    "{what} deve ficar entre {} e {}.",
                    decimal(low, 2),
                    decimal(high, 2)
                ))
            }
        };
        within(self.depth_m, 0.40, 3.00, "A profundidade da piscina (m)")?;
        within(
            self.wall_thickness_m,
            0.10,
            0.40,
            "A espessura da parede da piscina (m)",
        )?;
        within(
            self.floor_thickness_m,
            0.10,
            0.50,
            "A espessura do fundo da piscina (m)",
        )?;
        within(
            self.edge_surcharge_kpa,
            0.0,
            20.0,
            "A sobrecarga na borda (kN/m²)",
        )?;
        Ok(())
    }
}

/// Whether a room's name makes it a pool.
pub fn is_pool_name(name: &str) -> bool {
    let name = name.to_lowercase();
    name.contains("piscina") || name.contains("pool")
}

/// The wall of a pool, per metre.
#[derive(Clone, Debug, PartialEq)]
pub struct PoolWallDesign {
    pub height_m: f64,
    pub thickness_m: f64,
    /// Characteristic moments at the base, kN·m/m.
    pub moment_water_knm_m: f64,
    pub moment_soil_knm_m: f64,
    /// Design moment and shear, kN·m/m and kN/m.
    pub md_knm_m: f64,
    pub vd_kn_m: f64,
    /// Shear resistance without stirrups, kN/m.
    pub vrd1_kn_m: f64,
    /// Bending demand over the ductility limit (μ/0.295).
    pub bending_ratio: f64,
    pub shear_ratio: f64,
    /// Steel on each face, cm²/m: governing, ULS, service and minimum.
    pub steel_cm2_m: f64,
    pub steel_uls_cm2_m: f64,
    pub steel_service_cm2_m: f64,
    pub steel_min_cm2_m: f64,
    pub bars: String,
    /// The thinnest wall that passes, m.
    pub needed_thickness_m: f64,
    pub utilisation: f64,
}

/// The empty pool over groundwater.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlotationCheck {
    pub water_depth_m: f64,
    pub uplift_kn: f64,
    pub weight_kn: f64,
    /// `1.1·U / 0.9·G` (≤ 1 holds down).
    pub ratio: f64,
}

/// One pool, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct PoolResult {
    pub room_id: u64,
    pub label: String,
    pub room: String,
    pub spec: PoolSpec,
    pub inner_area_m2: f64,
    pub perimeter_m: f64,
    pub water_m3: f64,
    pub excavation_depth_m: f64,
    pub unit_weight_kn_m3: f64,
    pub friction_deg: f64,
    pub k0: f64,
    /// Edge surcharge plus neighbouring foundations, kPa.
    pub surcharge_kpa: f64,
    /// Water in the ground: `Some(depth)` known, `None` none expected.
    pub groundwater_m: Option<f64>,
    /// Nobody answered whether water springs from the ground.
    pub groundwater_unknown: bool,
    pub wall: PoolWallDesign,
    pub floor_pressure_kpa: f64,
    pub allowable_kpa: f64,
    pub floor_ratio: f64,
    pub floor_bars: String,
    pub flotation: Option<FlotationCheck>,
    /// Foundations inside the excavation's influence zone: (label, clearance m, needed m).
    pub foundations_in_zone: Vec<(String, f64, f64)>,
    /// Clearance from the outer face to the lot boundary, m.
    pub boundary_clearance_m: Option<f64>,
    /// Clearance the boundary needs, m.
    pub boundary_needed_m: f64,
    pub collapsible: bool,
    pub fck_mpa: f64,
    /// Filter flow for a full turnover in six hours, m³/h.
    pub filter_m3_h: f64,
    /// Execution and safety, in plain words.
    pub care: Vec<String>,
}

impl PoolResult {
    pub fn boundary_in_zone(&self) -> bool {
        self.boundary_clearance_m
            .is_some_and(|c| c + 1e-9 < self.boundary_needed_m)
    }
}

impl Project {
    /// Every pool: rooms with an explicit spec and rooms named "Piscina".
    pub fn pool_specs(&self) -> Vec<(&Room, PoolSpec)> {
        self.rooms
            .iter()
            .filter_map(|room| {
                if let Some(spec) = self.structure.pools.iter().find(|p| p.room_id == room.id) {
                    return Some((room, *spec));
                }
                is_pool_name(room.display_name()).then(|| (room, PoolSpec::new(room.id)))
            })
            .collect()
    }

    /// Walls that only pools use: concrete in the ground, not masonry.
    pub fn pool_wall_ids(&self) -> HashSet<u64> {
        let pools: HashSet<u64> = self.pool_specs().iter().map(|(room, _)| room.id).collect();
        let mut ids = HashSet::new();
        if pools.is_empty() {
            return ids;
        }
        for room in self.rooms.iter().filter(|r| pools.contains(&r.id)) {
            for wall in &room.wall_ids {
                let shared = self
                    .rooms
                    .iter()
                    .any(|other| !pools.contains(&other.id) && other.wall_ids.contains(wall));
                if !shared {
                    ids.insert(*wall);
                }
            }
        }
        ids
    }

    /// Keeps the designer's spec for the pool in `spec.room_id`.
    pub fn set_pool_spec(&mut self, spec: PoolSpec) -> Result<(), String> {
        spec.validate()?;
        if !self.rooms.iter().any(|r| r.id == spec.room_id) {
            return Err(format!("Cômodo {} não encontrado.", spec.room_id));
        }
        if let Some(existing) = self
            .structure
            .pools
            .iter_mut()
            .find(|p| p.room_id == spec.room_id)
        {
            *existing = spec;
        } else {
            self.structure.pools.push(spec);
        }
        Ok(())
    }

    /// Every pool, checked.
    pub fn pool_report(&self) -> Vec<PoolResult> {
        self.pool_specs()
            .into_iter()
            .enumerate()
            .filter_map(|(index, (room, spec))| self.check_pool(index, room, spec))
            .collect()
    }

    fn check_pool(&self, index: usize, room: &Room, spec: PoolSpec) -> Option<PoolResult> {
        let polygon = self.room_polygon(room)?;
        let axis_area = polygon_area(&polygon);
        let perimeter: f64 = (0..polygon.len())
            .map(|i| polygon[i].distance(polygon[(i + 1) % polygon.len()]))
            .sum();
        let t_wall = spec.wall_thickness_m;
        let inner_area = (axis_area - perimeter * t_wall / 2.0).max(0.1);
        let outer_area = axis_area + perimeter * t_wall / 2.0;
        let height = spec.depth_m;
        let water_m3 = inner_area * (height - 0.10).max(0.1);
        let excavation = height + spec.floor_thickness_m + BLINDING_M;

        let presumed = self
            .structure
            .presumed_soil
            .as_ref()
            .map(crate::domain::soil::PresumedSoil::analysis);
        let unit_weight = presumed.as_ref().map_or(18.0, |a| a.unit_weight_kn_m3);
        let friction = presumed.as_ref().map_or(25.0, |a| a.friction_deg);
        let collapsible = presumed.as_ref().is_some_and(|a| a.collapsible);
        let weak = presumed.as_ref().is_none_or(|a| a.governing_n < 10)
            && self
                .structure
                .soil_survey
                .as_ref()
                .and_then(|s| s.footing_n(1.0))
                .is_none_or(|n| n < 10.0);
        // NBR 6122, foundations at different levels: 60° from the vertical in
        // weak soil (horizontal = 1.73 × vertical), 45° otherwise.
        let spread = if weak { 3.0_f64.sqrt() } else { 1.0 };
        let k0 = 1.0 - friction.to_radians().sin();

        let survey_water = self
            .structure
            .soil_survey
            .as_ref()
            .and_then(crate::domain::soil::SoilSurvey::governing_water_table_m);
        let springs = self.protection.site_risks.effective(SiteRisk::Groundwater);
        let groundwater_m = match (survey_water, springs) {
            (Some(depth), _) => Some(depth),
            (None, Some(true)) => Some(0.0),
            _ => None,
        };
        let groundwater_unknown = survey_water.is_none() && springs.is_none();

        // Foundations near the excavation.
        let outer_clearance = |point: Point, half: f64| -> f64 {
            if polygon_contains_point(&polygon, point) {
                return 0.0;
            }
            let edge = (0..polygon.len())
                .map(|i| distance_to_segment(point, polygon[i], polygon[(i + 1) % polygon.len()]))
                .fold(f64::INFINITY, f64::min);
            (edge - t_wall / 2.0 - half).max(0.0)
        };
        let labels = self.structure_labels();
        let mut in_zone = Vec::new();
        for fixture in &self.fixtures {
            let base = match fixture.kind {
                FixtureKind::Footing => FOOTING_BASE_M,
                FixtureKind::TimberPost => 0.60,
                FixtureKind::PileCap => {
                    let pile = fixture.pile.unwrap_or_default();
                    // Piles that reach below the pool carry their load past it.
                    if pile.depth_m >= excavation + 0.50 {
                        continue;
                    }
                    pile.depth_m
                }
                _ => continue,
            };
            let drop = excavation - base;
            if drop <= 0.0 {
                continue;
            }
            let needed = drop * spread;
            let clearance = outer_clearance(fixture.center, fixture.width.max(fixture.depth) / 2.0);
            if clearance + 1e-9 < needed {
                let label = labels
                    .get(&fixture.id)
                    .cloned()
                    .unwrap_or_else(|| format!("{} #{}", fixture.kind.label(), fixture.id));
                in_zone.push((label, clearance, needed));
            }
        }
        let pool_walls = self.pool_wall_ids();
        for foundation in &self.structure.wall_foundations {
            if foundation.kind != crate::domain::wall_foundations::WallFoundationKind::StripFooting
                || pool_walls.contains(&foundation.wall_id)
            {
                continue;
            }
            let Some(wall) = self.walls.iter().find(|w| w.id == foundation.wall_id) else {
                continue;
            };
            let drop = excavation - STRIP_BASE_M;
            let needed = drop * spread;
            let samples = 12;
            let clearance = (0..=samples)
                .map(|i| wall.start.lerp(wall.end, f64::from(i) / f64::from(samples)))
                .map(|p| outer_clearance(p, 0.20))
                .fold(f64::INFINITY, f64::min);
            if clearance + 1e-9 < needed {
                in_zone.push((
                    format!("sapata corrida da parede {}", wall.id),
                    clearance,
                    needed,
                ));
            }
        }
        let boundary_needed = excavation * spread;
        let boundary_clearance = self.terrain.as_ref().map(|terrain| {
            let outline = terrain.outline();
            polygon
                .iter()
                .map(|corner| {
                    (0..outline.len())
                        .map(|i| {
                            distance_to_segment(
                                *corner,
                                outline[i],
                                outline[(i + 1) % outline.len()],
                            )
                        })
                        .fold(f64::INFINITY, f64::min)
                })
                .chain((0..polygon.len()).map(|i| {
                    let mid = polygon[i].midpoint(polygon[(i + 1) % polygon.len()]);
                    (0..outline.len())
                        .map(|j| {
                            distance_to_segment(mid, outline[j], outline[(j + 1) % outline.len()])
                        })
                        .fold(f64::INFINITY, f64::min)
                }))
                .fold(f64::INFINITY, f64::min)
                - t_wall / 2.0
        });
        let boundary_in_zone = boundary_clearance.is_some_and(|c| c + 1e-9 < boundary_needed);
        let surcharge = spec.edge_surcharge_kpa
            + if in_zone.is_empty() && !boundary_in_zone {
                0.0
            } else {
                NEIGHBOUR_SURCHARGE_KPA
            };

        let fck = self.structure.settings.fck_mpa;
        let wall = design_wall(
            height,
            t_wall,
            unit_weight,
            k0,
            surcharge,
            groundwater_m.is_some_and(|d| d < height),
            fck,
        );

        // The floor on the soil: water, slab and walls spread over the base.
        let walls_kn = CONCRETE_KN_M3 * t_wall * height * perimeter;
        let floor_pressure = WATER_KN_M3 * (height - 0.10).max(0.0)
            + CONCRETE_KN_M3 * spec.floor_thickness_m
            + walls_kn / outer_area.max(0.1);
        let allowable = self.structure.settings.soil_kpa;
        let floor_min = 0.0015 * spec.floor_thickness_m * 10_000.0;

        let flotation = groundwater_m.map(|depth| {
            let water_depth = (excavation - depth).max(0.0);
            let uplift = WATER_KN_M3 * water_depth * outer_area;
            let weight = walls_kn + CONCRETE_KN_M3 * spec.floor_thickness_m * outer_area;
            FlotationCheck {
                water_depth_m: water_depth,
                uplift_kn: uplift,
                weight_kn: weight,
                ratio: 1.1 * uplift / (0.9 * weight).max(1e-9),
            }
        });

        let mut care = vec![
            "Impermeabilize por dentro (manta ou argamassa polimérica) e faça o teste de estanqueidade: encha e observe 72 horas antes de assentar o revestimento.".to_owned(),
            "Concrete o fundo e as paredes com junta de concretagem tratada (mata-junta ou fita hidroexpansiva) e vibre bem o concreto: é por ali que a piscina vaza.".to_owned(),
            "Ralo de fundo com tampa antiaprisionamento e sucção protegida (veja a NBR 10339): um ralo de sucção sem proteção pode prender uma criança no fundo.".to_owned(),
            "Cerque a piscina com grade ou barreira de pelo menos 1,20 m e portão que fecha sozinho: afogamento de criança é rápido e silencioso.".to_owned(),
            "Borda e entorno com piso antiderrapante; marque a profundidade na borda.".to_owned(),
            "Elétrica da piscina (NBR 5410, seção 9.2): iluminação subaquática em extra-baixa tensão (12 V), DR de 30 mA nos circuitos e ligação equipotencial de todas as partes metálicas.".to_owned(),
        ];
        if collapsible {
            care.push("Solo colapsível: troque e compacte a base sob o fundo (solo-cimento), faça um dreno testemunha sob a piscina para mostrar vazamento cedo, e conserte qualquer vazamento na hora: água no solo poroso afunda as fundações vizinhas.".to_owned());
        }
        if !in_zone.is_empty() {
            care.push("NBR 6122: a fundação mais funda (a piscina) é feita primeiro; as fundações vizinhas vêm depois, sobre reaterro compactado em camadas, ou em brocas com a ponta abaixo do fundo da piscina.".to_owned());
        }
        if boundary_in_zone {
            care.push("Escavação junto à divisa: escore a vala enquanto ela estiver aberta, não escave em dia de chuva, avise o vizinho antes e fotografe as paredes dele.".to_owned());
        }
        if groundwater_m.is_some() {
            care.push("Com água no solo, não esvazie a piscina sem rebaixar o lençol: vazia, ela pode subir.".to_owned());
        }

        Some(PoolResult {
            room_id: room.id,
            label: format!("PI{}", index + 1),
            room: room.display_name().to_owned(),
            spec,
            inner_area_m2: inner_area,
            perimeter_m: perimeter,
            water_m3,
            excavation_depth_m: excavation,
            unit_weight_kn_m3: unit_weight,
            friction_deg: friction,
            k0,
            surcharge_kpa: surcharge,
            groundwater_m,
            groundwater_unknown,
            wall,
            floor_pressure_kpa: floor_pressure,
            allowable_kpa: allowable,
            floor_ratio: floor_pressure / allowable.max(1.0),
            floor_bars: bars_for(floor_min),
            flotation,
            foundations_in_zone: in_zone,
            boundary_clearance_m: boundary_clearance,
            boundary_needed_m: boundary_needed,
            collapsible,
            fck_mpa: fck,
            filter_m3_h: water_m3 / TURNOVER_H,
            care,
        })
    }
}

/// Designs one metre of pool wall and finds the thinnest one that passes.
fn design_wall(
    height: f64,
    thickness: f64,
    unit_weight: f64,
    k0: f64,
    surcharge: f64,
    groundwater: bool,
    fck: f64,
) -> PoolWallDesign {
    let water = WATER_KN_M3 * height.powi(3) / 6.0;
    let water_shear = WATER_KN_M3 * height.powi(2) / 2.0;
    let (soil, soil_shear) = if groundwater {
        let submerged = (unit_weight - WATER_KN_M3).max(0.0);
        (
            k0 * submerged * height.powi(3) / 6.0
                + WATER_KN_M3 * height.powi(3) / 6.0
                + k0 * surcharge * height.powi(2) / 2.0,
            k0 * submerged * height.powi(2) / 2.0
                + WATER_KN_M3 * height.powi(2) / 2.0
                + k0 * surcharge * height,
        )
    } else {
        (
            k0 * unit_weight * height.powi(3) / 6.0 + k0 * surcharge * height.powi(2) / 2.0,
            k0 * unit_weight * height.powi(2) / 2.0 + k0 * surcharge * height,
        )
    };
    let mk = water.max(soil);
    let vk = water_shear.max(soil_shear);
    let section = |t: f64| section_check(mk, vk, t, fck);
    let here = section(thickness);
    let mut needed = MIN_THICKNESS_M;
    while needed < 0.60 && section(needed).utilisation > 1.0 + 1e-9 {
        needed += 0.01;
    }
    PoolWallDesign {
        height_m: height,
        thickness_m: thickness,
        moment_water_knm_m: water,
        moment_soil_knm_m: soil,
        md_knm_m: 1.4 * mk,
        vd_kn_m: 1.4 * vk,
        vrd1_kn_m: here.vrd1,
        bending_ratio: here.bending_ratio,
        shear_ratio: here.shear_ratio,
        steel_cm2_m: here.steel,
        steel_uls_cm2_m: here.steel_uls,
        steel_service_cm2_m: here.steel_service,
        steel_min_cm2_m: here.steel_min,
        bars: bars_for(here.steel),
        needed_thickness_m: needed,
        utilisation: here.utilisation,
    }
}

struct Section {
    vrd1: f64,
    bending_ratio: f64,
    shear_ratio: f64,
    steel: f64,
    steel_uls: f64,
    steel_service: f64,
    steel_min: f64,
    utilisation: f64,
}

/// One metre of a slab-like section `t` thick under `mk` and `vk` (characteristic).
fn section_check(mk: f64, vk: f64, t: f64, fck: f64) -> Section {
    let d = (t - COVER_M - 0.005).max(0.03);
    let fcd = fck.max(10.0) / 1.4 * 1000.0;
    let fyd = 500.0 / 1.15 * 1000.0;
    let md = 1.4 * mk;
    let mu = md / (0.85 * fcd * d * d);
    let xi = ((1.0 - (1.0 - 2.0 * mu.min(0.295)).max(0.0).sqrt()) / 0.8).min(0.45);
    let z = d * (1.0 - 0.4 * xi);
    let steel_uls = md / (z * fyd) * 10_000.0;
    let steel_service = mk / (z * WATERTIGHT_STEEL_MPA * 1000.0) * 10_000.0;
    let steel_min = 0.0015 * t * 10_000.0;
    let steel = steel_uls.max(steel_service).max(steel_min);
    // NBR 6118 §19.4.1: V_Rd1 = τRd·k·(1.2 + 40ρ1)·b·d.
    let fctd = 0.21 * fck.max(10.0).powf(2.0 / 3.0) / 1.4;
    let tau_rd = 0.25 * fctd * 1000.0;
    let k = (1.6 - d).max(1.0);
    let rho = (steel / 10_000.0 / d).min(0.02);
    let vrd1 = tau_rd * k * (1.2 + 40.0 * rho) * d;
    let bending_ratio = mu / 0.295;
    let shear_ratio = 1.4 * vk / vrd1.max(1e-9);
    let thickness_ratio = MIN_THICKNESS_M / t.max(1e-9);
    Section {
        vrd1,
        bending_ratio,
        shear_ratio,
        steel,
        steel_uls,
        steel_service,
        steel_min,
        utilisation: bending_ratio.max(shear_ratio).max(thickness_ratio),
    }
}

/// "Ø10 c/ 15 cm" for `cm2_m` of steel per metre.
pub fn bars_for(cm2_m: f64) -> String {
    for (diameter, area) in [(8.0, 0.503), (10.0, 0.785), (12.5, 1.227), (16.0, 2.011)] {
        let spacing_cm = ((area / cm2_m.max(1e-9) * 100.0) / 2.5).floor() * 2.5;
        if spacing_cm >= 10.0 {
            let spacing_cm = spacing_cm.min(20.0);
            let places = |v: f64| if v.fract().abs() > 1e-9 { 1 } else { 0 };
            return format!(
                "Ø{} c/ {} cm",
                decimal(diameter, places(diameter)),
                decimal(spacing_cm, places(spacing_cm))
            );
        }
    }
    "Ø16 c/ 10 cm não basta: engrosse a parede".to_owned()
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::soil::{Likelihood, PresumedLayer, PresumedSoil, PresumedSoilType};

    fn pool_project() -> (Project, u64) {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        let room = project
            .add_room(Point::new(3.0, 5.0), Point::new(6.0, 13.0), "Piscina")
            .unwrap();
        (project, room)
    }

    #[test]
    fn a_room_named_piscina_is_a_pool_and_its_walls_leave_the_masonry() {
        let (project, room) = pool_project();
        let pools = project.pool_specs();
        assert_eq!(pools.len(), 1);
        assert_eq!(pools[0].0.id, room);
        assert_eq!(project.pool_wall_ids().len(), 4);
    }

    #[test]
    fn the_wall_matches_a_hand_calculation() {
        // H = 1.40 m, water: M = 10·1.4³/6 = 4.573 kN·m/m.
        // Soil γ = 18, φ = 25° → K0 = 0.5774; q = 2.5:
        // M = 0.5774·18·1.4³/6 + 0.5774·2.5·1.4²/2 = 4.753 + 1.415 = 6.168 kN·m/m.
        let wall = design_wall(
            1.40,
            0.15,
            18.0,
            1.0 - 25f64.to_radians().sin(),
            2.5,
            false,
            20.0,
        );
        assert!((wall.moment_water_knm_m - 4.5733).abs() < 1e-3);
        assert!(
            (wall.moment_soil_knm_m - 6.168).abs() < 2e-3,
            "{}",
            wall.moment_soil_knm_m
        );
        assert!((wall.md_knm_m - 1.4 * 6.168).abs() < 3e-3);
        // d = 0.115; μ = 8.635/(0.85·14286·0.115²) = 0.0538 → passes.
        assert!(wall.bending_ratio < 0.2);
        // Service stress governs: As = 6.168/(z·160 MPa) ≈ 3.5 cm²/m.
        assert!(wall.steel_service_cm2_m > wall.steel_uls_cm2_m);
        assert!((wall.steel_cm2_m - wall.steel_service_cm2_m).abs() < 1e-9);
        assert!(wall.utilisation <= 1.0);
        assert!(wall.bars.starts_with('Ø'));
    }

    #[test]
    fn a_thin_deep_wall_fails_and_names_the_thickness_it_needs() {
        let wall = design_wall(2.5, 0.10, 18.0, 0.58, 10.0, true, 20.0);
        assert!(wall.utilisation > 1.0);
        assert!(wall.needed_thickness_m > 0.10);
        let thicker = design_wall(2.5, wall.needed_thickness_m, 18.0, 0.58, 10.0, true, 20.0);
        assert!(thicker.utilisation <= 1.0 + 1e-9);
    }

    #[test]
    fn groundwater_makes_an_empty_pool_float_and_a_footing_nearby_is_in_the_zone() {
        let (mut project, _) = pool_project();
        project.protection.site_risks.set(
            SiteRisk::Groundwater,
            Some(true),
            crate::domain::protection::hypotheses::HypothesisSource::Person,
        );
        project
            .add_fixture(FixtureKind::Footing, Point::new(7.0, 9.0), 0.6, 0.6, 0)
            .unwrap();
        let report = project.pool_report();
        let pool = &report[0];
        let flotation = pool.flotation.unwrap();
        // Uplift over 1.60 m of water beats a 15 cm floor and walls.
        assert!(flotation.ratio > 1.0, "{flotation:?}");
        assert_eq!(pool.foundations_in_zone.len(), 1);
        assert!(pool.surcharge_kpa > DEFAULT_EDGE_KPA);
    }

    #[test]
    fn a_collapsible_presumed_soil_adds_its_care_and_weak_soil_widens_the_zone() {
        let (mut project, _) = pool_project();
        project.structure.presumed_soil = Some(
            PresumedSoil::new(
                vec![PresumedLayer {
                    soil: PresumedSoilType::RedSandyClay,
                    likelihood: Likelihood::Probable,
                }],
                None,
            )
            .unwrap(),
        );
        let pool = &project.pool_report()[0];
        assert!(pool.collapsible);
        assert!(pool.care.iter().any(|c| c.contains("colapsível")));
        // 1.60 m excavation × tan 60° = 2.77 m from the boundary.
        assert!((pool.boundary_needed_m - 1.60 * 3f64.sqrt()).abs() < 1e-6);
        assert!(pool.groundwater_unknown);
    }

    #[test]
    fn bar_spacing_reads_like_a_drawing() {
        assert_eq!(bars_for(3.5), "Ø8 c/ 12,5 cm");
        assert_eq!(bars_for(2.25), "Ø8 c/ 20 cm");
        assert_eq!(bars_for(6.0), "Ø10 c/ 12,5 cm");
        assert_eq!(bars_for(9.0), "Ø12,5 c/ 12,5 cm");
    }
}
