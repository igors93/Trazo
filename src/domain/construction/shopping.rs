//! Stage-by-stage shopping list across all disciplines (#136): converts raw
//! model quantities (concrete m³, mortar/plaster/grout m³, steel kg, blocks,
//! roof timber/tiles, electrical wires/conduits/protection, plumbing pipes and
//! room finishes) into commercial purchase units (50 kg cement bags, 20 kg lime
//! bags, m³ of sand and gravel by catalogue mix, 12 m steel bars, 100 m wire
//! rolls, 50 m conduit coils, 6 m PVC pipes, 18 L paint cans) grouped by
//! construction stage so nothing spoils on site.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::domain::{
    construction::{materials::MaterialsCatalogue, roof_plan::roof_plans},
    model::Project,
    plumbing::{
        cold_water::cold_water,
        points::{PointKind, plumbing_points},
    },
    reinforcement::COMMERCIAL_BAR_LENGTH_M,
    structure::SlabKind,
    wall_foundations::WallFoundationKind,
};

/// Ordered construction stages shared by the shopping list (#136), cost
/// estimate (#87), schedule (#137), NR-18 safety checklists (#138) and site
/// progress log (#139).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstructionStage {
    SiteAndSettingOut,
    Foundations,
    Structure,
    Masonry,
    Roof,
    Installations,
    PlasterAndWaterproofing,
    Finishes,
}

impl ConstructionStage {
    pub const ALL: [Self; 8] = [
        Self::SiteAndSettingOut,
        Self::Foundations,
        Self::Structure,
        Self::Masonry,
        Self::Roof,
        Self::Installations,
        Self::PlasterAndWaterproofing,
        Self::Finishes,
    ];

    pub fn number(self) -> u8 {
        match self {
            Self::SiteAndSettingOut => 1,
            Self::Foundations => 2,
            Self::Structure => 3,
            Self::Masonry => 4,
            Self::Roof => 5,
            Self::Installations => 6,
            Self::PlasterAndWaterproofing => 7,
            Self::Finishes => 8,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SiteAndSettingOut => "1. Serviços preliminares e locação",
            Self::Foundations => "2. Fundações e baldrames",
            Self::Structure => "3. Estrutura (pilares, vigas e lajes)",
            Self::Masonry => "4. Alvenaria, vergas e cintas",
            Self::Roof => "5. Cobertura e telhado",
            Self::Installations => "6. Instalações elétricas e hidráulicas",
            Self::PlasterAndWaterproofing => "7. Chapisco, reboco e impermeabilização",
            Self::Finishes => "8. Acabamentos e pintura",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::SiteAndSettingOut => "Preliminares e locação",
            Self::Foundations => "Fundações e baldrames",
            Self::Structure => "Estrutura",
            Self::Masonry => "Alvenaria",
            Self::Roof => "Cobertura",
            Self::Installations => "Instalações",
            Self::PlasterAndWaterproofing => "Reboco e imperm.",
            Self::Finishes => "Acabamentos",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::SiteAndSettingOut => "preliminares",
            Self::Foundations => "fundacoes",
            Self::Structure => "estrutura",
            Self::Masonry => "alvenaria",
            Self::Roof => "cobertura",
            Self::Installations => "instalacoes",
            Self::PlasterAndWaterproofing => "reboco",
            Self::Finishes => "acabamentos",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let key = raw.trim().to_lowercase();
        let norm = key
            .replace('ç', "c")
            .replace('õ', "o")
            .replace(['ã', 'á'], "a")
            .replace('é', "e")
            .replace('í', "i");
        match norm.as_str() {
            "1" | "preliminares" | "locacao" | "site_and_setting_out" => {
                Some(Self::SiteAndSettingOut)
            }
            "2" | "fundacoes" | "fundacao" | "baldrames" | "foundations" => Some(Self::Foundations),
            "3" | "estrutura" | "concreto" | "structure" => Some(Self::Structure),
            "4" | "alvenaria" | "paredes" | "masonry" => Some(Self::Masonry),
            "5" | "cobertura" | "telhado" | "roof" => Some(Self::Roof),
            "6" | "instalacoes" | "eletrica" | "hidraulica" | "installations" => {
                Some(Self::Installations)
            }
            "7" | "reboco" | "revestimento" | "impermeabilizacao" | "plaster_and_waterproofing" => {
                Some(Self::PlasterAndWaterproofing)
            }
            "8" | "acabamentos" | "acabamento" | "pintura" | "finishes" => Some(Self::Finishes),
            _ => None,
        }
    }
}

/// One commercial purchase item inside a construction stage.
#[derive(Clone, Debug, PartialEq)]
pub struct ShoppingItem {
    /// Stable price/SINAPI lookup code shared with `#87`.
    pub item_code: String,
    pub stage: ConstructionStage,
    pub discipline: &'static str,
    pub description: String,
    /// Net quantity before waste allowance.
    pub net_quantity: f64,
    /// Quantity to buy (including waste & commercial rounding where applicable).
    pub buy_quantity: f64,
    /// Waste percentage applied.
    pub waste_pct: f64,
    /// Purchase unit (`sc 50 kg`, `sc 20 kg`, `m³`, `kg`, `un`, `m²`, `m`, `lata 18 L`).
    pub unit: String,
    /// Commercial packaging note (e.g. `12 barras de 12 m`, `2 rolos de 100 m`, `traço C20`).
    pub commercial_note: String,
}

/// All items belonging to one construction stage.
#[derive(Clone, Debug, PartialEq)]
pub struct StageShoppingGroup {
    pub stage: ConstructionStage,
    pub items: Vec<ShoppingItem>,
}

/// Consolidated summary of key bulk consumables across all stages.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BulkShoppingSummary {
    pub cement_bags_50kg: u32,
    pub lime_bags_20kg: u32,
    pub sand_m3: f64,
    pub gravel_m3: f64,
    pub structural_concrete_m3: f64,
    pub lean_concrete_m3: f64,
    pub steel_ca50_kg: f64,
    pub steel_bars_12m: u32,
    pub total_blocks: u32,
    pub wire_total_m: f64,
    pub conduit_total_m: f64,
    pub pvc_pipe_total_m: f64,
}

/// Complete stage-by-stage shopping list for the whole project (#136).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StageShoppingList {
    pub stages: Vec<StageShoppingGroup>,
    pub summary: BulkShoppingSummary,
    pub assumptions: Vec<String>,
}

impl StageShoppingList {
    pub fn total_items(&self) -> usize {
        self.stages.iter().map(|g| g.items.len()).sum()
    }

    pub fn all_items(&self) -> impl Iterator<Item = &ShoppingItem> {
        self.stages.iter().flat_map(|g| g.items.iter())
    }
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

/// Converts a concrete or mortar volume (`m³`) into commercial raw materials
/// using the material's mix recipe and waste % from `#135`.
pub fn expand_mix_volume(
    volume_net_m3: f64,
    catalogue: &MaterialsCatalogue,
    material_id: &str,
) -> (f64, u32, u32, f64, f64, f64) {
    let entry = catalogue.find(material_id);
    let waste_pct = entry.map_or(10.0, |m| m.waste_pct);
    let vol_with_waste = volume_net_m3 * (1.0 + waste_pct / 100.0);
    let cem_kg = vol_with_waste * entry.and_then(|m| m.mix_cement_kg_m3).unwrap_or(0.0);
    let lime_kg = vol_with_waste * entry.and_then(|m| m.mix_lime_kg_m3).unwrap_or(0.0);
    let sand_m3 = vol_with_waste * entry.and_then(|m| m.mix_sand_m3_per_m3).unwrap_or(0.0);
    let gravel_m3 = vol_with_waste * entry.and_then(|m| m.mix_gravel_m3_per_m3).unwrap_or(0.0);
    let cement_bags = if cem_kg > 1e-6 {
        (cem_kg / 50.0).ceil() as u32
    } else {
        0
    };
    let lime_bags = if lime_kg > 1e-6 {
        (lime_kg / 20.0).ceil() as u32
    } else {
        0
    };
    (
        vol_with_waste,
        cement_bags,
        lime_bags,
        sand_m3,
        gravel_m3,
        waste_pct,
    )
}

struct MixSpec<'a> {
    stage: ConstructionStage,
    discipline: &'static str,
    context_label: &'a str,
    volume_net_m3: f64,
    material_id: &'a str,
}

fn push_mix_items(
    items: &mut Vec<ShoppingItem>,
    summary: &mut BulkShoppingSummary,
    catalogue: &MaterialsCatalogue,
    spec: MixSpec<'_>,
) {
    let MixSpec {
        stage,
        discipline,
        context_label,
        volume_net_m3,
        material_id,
    } = spec;
    if volume_net_m3 <= 1e-6 {
        return;
    }
    let (vol_gross, cem_bags, lime_bags, sand_m3, gravel_m3, waste_pct) =
        expand_mix_volume(volume_net_m3, catalogue, material_id);
    let mix_note = format!(
        "{context_label}: {} m³ líq. → {} m³ c/ perda",
        decimal(volume_net_m3, 2),
        decimal(vol_gross, 2)
    );
    if cem_bags > 0 {
        summary.cement_bags_50kg += cem_bags;
        items.push(ShoppingItem {
            item_code: "cimento_50kg".to_owned(),
            stage,
            discipline,
            description: format!("Cimento CP II-32 — {context_label}"),
            net_quantity: (volume_net_m3
                * catalogue
                    .find(material_id)
                    .and_then(|m| m.mix_cement_kg_m3)
                    .unwrap_or(0.0)
                / 50.0),
            buy_quantity: f64::from(cem_bags),
            waste_pct,
            unit: "sc 50 kg".to_owned(),
            commercial_note: mix_note.clone(),
        });
    }
    if lime_bags > 0 {
        summary.lime_bags_20kg += lime_bags;
        items.push(ShoppingItem {
            item_code: "cal_20kg".to_owned(),
            stage,
            discipline,
            description: format!("Cal hidratada CH-III — {context_label}"),
            net_quantity: (volume_net_m3
                * catalogue
                    .find(material_id)
                    .and_then(|m| m.mix_lime_kg_m3)
                    .unwrap_or(0.0)
                / 20.0),
            buy_quantity: f64::from(lime_bags),
            waste_pct,
            unit: "sc 20 kg".to_owned(),
            commercial_note: mix_note.clone(),
        });
    }
    if sand_m3 > 0.01 {
        let buy_sand = (sand_m3 * 10.0).ceil() / 10.0;
        summary.sand_m3 += buy_sand;
        items.push(ShoppingItem {
            item_code: "areia_m3".to_owned(),
            stage,
            discipline,
            description: format!("Areia lavada média — {context_label}"),
            net_quantity: sand_m3 / (1.0 + waste_pct / 100.0),
            buy_quantity: buy_sand,
            waste_pct,
            unit: "m³".to_owned(),
            commercial_note: format!(
                "{mix_note} (~{} carriolas de 60 L)",
                (buy_sand / 0.06).ceil() as u32
            ),
        });
    }
    if gravel_m3 > 0.01 {
        let buy_gravel = (gravel_m3 * 10.0).ceil() / 10.0;
        summary.gravel_m3 += buy_gravel;
        let gravel_name = if material_id == "mortar_grout" {
            format!("Pedrisco (brita 0) — {context_label}")
        } else {
            format!("Brita nº 1 — {context_label}")
        };
        items.push(ShoppingItem {
            item_code: "brita_m3".to_owned(),
            stage,
            discipline,
            description: gravel_name,
            net_quantity: gravel_m3 / (1.0 + waste_pct / 100.0),
            buy_quantity: buy_gravel,
            waste_pct,
            unit: "m³".to_owned(),
            commercial_note: mix_note,
        });
    }
}

/// Builds the complete stage-by-stage shopping list across all disciplines (#136).
pub fn build_stage_shopping_list(project: &Project) -> StageShoppingList {
    let catalogue = project.materials_catalogue();
    let all_floors = project.floors.levels.len() > 1;
    let takeoff = project.construction_takeoff_with_options(all_floors, None);
    let structure = project.cached_structure_report();
    let reinforcement = project.cached_reinforcement_report();
    let wire_waste = catalogue
        .find("wire_copper_pvc")
        .map_or(10.0, |m| m.waste_pct);
    let elec = project.electrical_shopping_list(wire_waste);
    let cw = cold_water(project);
    let plumbing_pts = plumbing_points(project);

    let mut summary = BulkShoppingSummary::default();
    let mut by_stage: BTreeMap<ConstructionStage, Vec<ShoppingItem>> = BTreeMap::new();
    for st in ConstructionStage::ALL {
        by_stage.insert(st, Vec::new());
    }

    // -------------------------------------------------------------------------
    // Stage 1: Site & setting-out (Preliminares e locação)
    // -------------------------------------------------------------------------
    let built_perimeter_m = {
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for w in &project.walls {
            for p in [w.start, w.end] {
                min_x = min_x.min(p.x);
                max_x = max_x.max(p.x);
                min_y = min_y.min(p.y);
                max_y = max_y.max(p.y);
            }
        }
        if min_x.is_finite() {
            2.0 * ((max_x - min_x + 2.0) + (max_y - min_y + 2.0))
        } else {
            0.0
        }
    };
    if built_perimeter_m > 0.5 {
        let gabarito_m = (built_perimeter_m * 1.15).ceil();
        let stakes = (built_perimeter_m / 1.5).ceil().max(4.0);
        let stage1 = by_stage
            .entry(ConstructionStage::SiteAndSettingOut)
            .or_default();
        stage1.push(ShoppingItem {
            item_code: "tabua_gabarito_m".to_owned(),
            stage: ConstructionStage::SiteAndSettingOut,
            discipline: "Locação",
            description: "Tábuas de pinus 15 cm para gabarito de locação".to_owned(),
            net_quantity: built_perimeter_m,
            buy_quantity: gabarito_m,
            waste_pct: 15.0,
            unit: "m".to_owned(),
            commercial_note: format!(
                "~{} peças de 3 m ao redor da obra (+1 m de afastamento)",
                (gabarito_m / 3.0).ceil() as u32
            ),
        });
        stage1.push(ShoppingItem {
            item_code: "pontalete_gabarito_un".to_owned(),
            stage: ConstructionStage::SiteAndSettingOut,
            discipline: "Locação",
            description: "Pontaletes / estacas de madeira 7×7 cm para gabarito".to_owned(),
            net_quantity: stakes,
            buy_quantity: stakes,
            waste_pct: 0.0,
            unit: "un".to_owned(),
            commercial_note: "Espaçados a cada ~1,5 m + linha de nylon e pregos 17×21".to_owned(),
        });
    }

    // -------------------------------------------------------------------------
    // Stage 2: Foundations & ground beams (Fundações e baldrames)
    // -------------------------------------------------------------------------
    let mut found_concrete_m3 = 0.0;
    let mut lean_concrete_m3 = 0.0;
    let mut found_formwork_m2 = 0.0;
    let mut baldrame_waterproof_m2 = 0.0;

    for f in &structure.footings {
        found_concrete_m3 += f.width_m * f.depth_m * f.height_m;
        lean_concrete_m3 += (f.width_m + 0.10) * (f.depth_m + 0.10) * 0.05;
        found_formwork_m2 += 2.0 * (f.width_m + f.depth_m) * f.height_m;
    }
    for pc in &structure.pile_caps {
        let cap_vol = pc.width_m * pc.depth_m * pc.height_m;
        let pile_area = std::f64::consts::PI * (pc.pile_diameter_m / 2.0).powi(2);
        let piles_vol = f64::from(pc.pile_count) * pile_area * pc.pile_depth_m;
        found_concrete_m3 += cap_vol + piles_vol;
        lean_concrete_m3 += pc.width_m * pc.depth_m * 0.05;
        found_formwork_m2 += 2.0 * (pc.width_m + pc.depth_m) * pc.height_m;
    }
    for wf in &structure.wall_foundations {
        let h = wf.height_m.unwrap_or(0.30);
        found_concrete_m3 += wf.length_m * wf.width_m * h;
        lean_concrete_m3 += wf.length_m * (wf.width_m + 0.10) * 0.05;
        if wf.kind == WallFoundationKind::GroundBeam {
            found_formwork_m2 += 2.0 * wf.length_m * h;
            baldrame_waterproof_m2 += wf.length_m * (wf.width_m + 2.0 * h.min(0.30));
        }
    }
    for bb in &structure.balance_beams {
        let h = bb.height_m.unwrap_or(0.50);
        found_concrete_m3 += bb.span_m * bb.width_m * h;
        found_formwork_m2 += 2.0 * bb.span_m * h;
    }

    // Foundation steel (footings from reinforcement report + ~80 kg/m³ for wall foundations/pile caps).
    let footing_steel_net_kg: f64 = reinforcement.footings.iter().map(|f| f.mass_kg).sum();
    let wf_concrete_m3: f64 = structure
        .wall_foundations
        .iter()
        .map(|wf| wf.length_m * wf.width_m * wf.height_m.unwrap_or(0.30))
        .sum();
    let found_steel_net_kg = footing_steel_net_kg + wf_concrete_m3 * 65.0;
    let steel_waste_pct = catalogue.find("steel_ca50").map_or(10.0, |m| m.waste_pct);

    summary.structural_concrete_m3 += found_concrete_m3;
    summary.lean_concrete_m3 += lean_concrete_m3;

    {
        let stage2 = by_stage.entry(ConstructionStage::Foundations).or_default();
        push_mix_items(
            stage2,
            &mut summary,
            &catalogue,
            MixSpec {
                stage: ConstructionStage::Foundations,
                discipline: "Estrutura",
                context_label: "concreto estrutural de fundações e baldrames",
                volume_net_m3: found_concrete_m3,
                material_id: "concrete_structural",
            },
        );
        push_mix_items(
            stage2,
            &mut summary,
            &catalogue,
            MixSpec {
                stage: ConstructionStage::Foundations,
                discipline: "Estrutura",
                context_label: "concreto magro / lastro (5 cm)",
                volume_net_m3: lean_concrete_m3,
                material_id: "concrete_lean",
            },
        );
        if found_steel_net_kg > 0.5 {
            let buy_kg = (found_steel_net_kg * (1.0 + steel_waste_pct / 100.0)).ceil();
            let bars_est = (buy_kg / (0.617 * COMMERCIAL_BAR_LENGTH_M)).ceil() as u32;
            summary.steel_ca50_kg += buy_kg;
            summary.steel_bars_12m += bars_est;
            stage2.push(ShoppingItem {
                item_code: "aco_ca50_kg".to_owned(),
                stage: ConstructionStage::Foundations,
                discipline: "Estrutura",
                description: "Aço nervurado CA-50 para fundações e baldrames".to_owned(),
                net_quantity: found_steel_net_kg,
                buy_quantity: buy_kg,
                waste_pct: steel_waste_pct,
                unit: "kg".to_owned(),
                commercial_note: format!(
                    "~{bars_est} barras eq. 12 m (Ø10 mm ref.; confira bitolas na tabela de aço)"
                ),
            });
            let wire_kg = (buy_kg * 0.015).ceil().max(1.0);
            stage2.push(ShoppingItem {
                item_code: "arame_recozido_kg".to_owned(),
                stage: ConstructionStage::Foundations,
                discipline: "Estrutura",
                description: "Arame recozido nº 18 para amarração das armaduras".to_owned(),
                net_quantity: buy_kg * 0.015,
                buy_quantity: wire_kg,
                waste_pct: 0.0,
                unit: "kg".to_owned(),
                commercial_note: "Estimativa 1,5 % do peso de aço".to_owned(),
            });
        }
        if found_formwork_m2 > 0.5 {
            let buy_fw = (found_formwork_m2 * 1.10).ceil();
            stage2.push(ShoppingItem {
                item_code: "forma_madeira_m2".to_owned(),
                stage: ConstructionStage::Foundations,
                discipline: "Estrutura",
                description: "Fôrma de madeira / compensado para baldrames e blocos".to_owned(),
                net_quantity: found_formwork_m2,
                buy_quantity: buy_fw,
                waste_pct: 10.0,
                unit: "m²".to_owned(),
                commercial_note: format!(
                    "~{} chapas de compensado 2,20×1,10 m (com reaproveitamento)",
                    (buy_fw / 2.42).ceil() as u32
                ),
            });
        }
        if baldrame_waterproof_m2 > 0.5 {
            let paint_l = (baldrame_waterproof_m2 * 0.50 * 1.10).ceil();
            stage2.push(ShoppingItem {
                item_code: "impermeabilizante_baldrame_l".to_owned(),
                stage: ConstructionStage::Foundations,
                discipline: "Impermeabilização",
                description: "Tinta asfáltica impermeabilizante para vigas baldrame".to_owned(),
                net_quantity: baldrame_waterproof_m2 * 0.50,
                buy_quantity: paint_l,
                waste_pct: 10.0,
                unit: "L".to_owned(),
                commercial_note: format!(
                    "2 demãos em {} m² de topo e laterais de baldrames (evita umidade ascendente)",
                    decimal(baldrame_waterproof_m2, 1)
                ),
            });
        }
    }

    // -------------------------------------------------------------------------
    // Stage 3: Superstructure (Pilares, vigas e lajes)
    // -------------------------------------------------------------------------
    let mut super_concrete_m3 = 0.0;
    let mut super_formwork_m2 = 0.0;
    let mut slab_area_m2 = 0.0;
    for col in &structure.columns {
        super_concrete_m3 += col.width_m * col.depth_m * col.height_m;
        super_formwork_m2 += 2.0 * (col.width_m + col.depth_m) * col.height_m;
    }
    for line in &structure.beams {
        for span in &line.spans {
            super_concrete_m3 += span.length_m * span.width_m * span.height_m;
            super_formwork_m2 += span.length_m * (span.width_m + 2.0 * span.height_m);
        }
    }
    for slab in &structure.slabs {
        let area = slab.lx_m * slab.ly_m;
        slab_area_m2 += area;
        let eff_h = match slab.kind {
            SlabKind::Solid => slab.thickness_m,
            // Ribbed lattice slab: concrete topping + ribs ≈ 55% of total thickness.
            SlabKind::Ribbed => slab.thickness_m * 0.55,
        };
        super_concrete_m3 += area * eff_h;
        if slab.kind == SlabKind::Solid {
            super_formwork_m2 += area;
        }
    }
    summary.structural_concrete_m3 += super_concrete_m3;

    // Superstructure steel: columns + beams + slabs from reinforcement report.
    let super_steel_net_kg: f64 = reinforcement.columns.iter().map(|c| c.mass_kg).sum::<f64>()
        + reinforcement.beams.iter().map(|b| b.mass_kg).sum::<f64>()
        + reinforcement.slabs.iter().map(|s| s.mass_kg).sum::<f64>();

    {
        let stage3 = by_stage.entry(ConstructionStage::Structure).or_default();
        push_mix_items(
            stage3,
            &mut summary,
            &catalogue,
            MixSpec {
                stage: ConstructionStage::Structure,
                discipline: "Estrutura",
                context_label: "concreto estrutural de pilares, vigas e lajes",
                volume_net_m3: super_concrete_m3,
                material_id: "concrete_structural",
            },
        );
        if super_steel_net_kg > 0.5 {
            let buy_kg = (super_steel_net_kg * (1.0 + steel_waste_pct / 100.0)).ceil();
            let total_bars = reinforcement
                .total_bars_12m()
                .max((buy_kg / (0.617 * COMMERCIAL_BAR_LENGTH_M)).ceil() as u32);
            summary.steel_ca50_kg += buy_kg;
            summary.steel_bars_12m += total_bars;
            stage3.push(ShoppingItem {
                item_code: "aco_ca50_kg".to_owned(),
                stage: ConstructionStage::Structure,
                discipline: "Estrutura",
                description: "Aço nervurado CA-50 para pilares, vigas e lajes".to_owned(),
                net_quantity: super_steel_net_kg,
                buy_quantity: buy_kg,
                waste_pct: steel_waste_pct,
                unit: "kg".to_owned(),
                commercial_note: format!(
                    "{total_bars} barras comerciais de 12 m ({} bitola(s) na tabela de aço)",
                    reinforcement.by_diameter.len().max(1)
                ),
            });
            let wire_kg = (buy_kg * 0.015).ceil().max(1.0);
            stage3.push(ShoppingItem {
                item_code: "arame_recozido_kg".to_owned(),
                stage: ConstructionStage::Structure,
                discipline: "Estrutura",
                description: "Arame recozido nº 18 para armaduras da superestrutura".to_owned(),
                net_quantity: buy_kg * 0.015,
                buy_quantity: wire_kg,
                waste_pct: 0.0,
                unit: "kg".to_owned(),
                commercial_note: "Estimativa 1,5 % do peso de aço".to_owned(),
            });
        }
        if super_formwork_m2 > 0.5 {
            let buy_fw = (super_formwork_m2 * 0.60).ceil(); // ~2x reuse across pillars/beams
            stage3.push(ShoppingItem {
                item_code: "forma_madeira_m2".to_owned(),
                stage: ConstructionStage::Structure,
                discipline: "Estrutura",
                description: "Compensado plastificado / madeira para fôrmas de pilares e vigas"
                    .to_owned(),
                net_quantity: super_formwork_m2,
                buy_quantity: buy_fw,
                waste_pct: 10.0,
                unit: "m²".to_owned(),
                commercial_note: format!(
                    "Área total {} m² (estimando ~2 reaproveitamentos)",
                    decimal(super_formwork_m2, 1)
                ),
            });
        }
        if slab_area_m2 > 0.5 {
            let props = (slab_area_m2 / 1.2).ceil();
            stage3.push(ShoppingItem {
                item_code: "escora_eucalipto_un".to_owned(),
                stage: ConstructionStage::Structure,
                discipline: "Estrutura",
                description: "Escoras de eucalipto / metálicas para escoramento de lajes e vigas"
                    .to_owned(),
                net_quantity: props,
                buy_quantity: props,
                waste_pct: 0.0,
                unit: "un".to_owned(),
                commercial_note: format!(
                    "1 escora a cada ~1,2 m² em {} m² de laje · manter ≥ 21 dias (NBR 14931)",
                    decimal(slab_area_m2, 1)
                ),
            });
        }
    }

    // -------------------------------------------------------------------------
    // Stage 4: Masonry, lintels & bond beams (Alvenaria, vergas e cintas)
    // -------------------------------------------------------------------------
    let block_waste_pct = catalogue
        .find("block_default")
        .map_or(takeoff.waste_percent, |m| m.waste_pct);
    {
        let stage4 = by_stage.entry(ConstructionStage::Masonry).or_default();
        for mt in &takeoff.masonry {
            let whole = (mt.blocks * (1.0 + block_waste_pct / 100.0)).ceil();
            let half = (mt.half_blocks * (1.0 + block_waste_pct / 100.0)).ceil();
            let u_bl = (mt.u_blocks * (1.0 + block_waste_pct / 100.0)).ceil();
            summary.total_blocks += whole as u32 + half as u32 + u_bl as u32;
            if whole > 0.0 {
                stage4.push(ShoppingItem {
                    item_code: "bloco_alvenaria_un".to_owned(),
                    stage: ConstructionStage::Masonry,
                    discipline: "Alvenaria",
                    description: format!("Bloco inteiro — {}", mt.masonry),
                    net_quantity: mt.blocks,
                    buy_quantity: whole,
                    waste_pct: block_waste_pct,
                    unit: "un".to_owned(),
                    commercial_note: format!("Área líquida {} m²", decimal(mt.net_area_m2, 1)),
                });
            }
            if half > 0.0 {
                stage4.push(ShoppingItem {
                    item_code: "meio_bloco_un".to_owned(),
                    stage: ConstructionStage::Masonry,
                    discipline: "Alvenaria",
                    description: format!("Meio-bloco (amarração) — {}", mt.masonry),
                    net_quantity: mt.half_blocks,
                    buy_quantity: half,
                    waste_pct: block_waste_pct,
                    unit: "un".to_owned(),
                    commercial_note: "Pontas de paredes e ombreiras de vãos".to_owned(),
                });
            }
            if u_bl > 0.0 {
                stage4.push(ShoppingItem {
                    item_code: "canaleta_un".to_owned(),
                    stage: ConstructionStage::Masonry,
                    discipline: "Alvenaria",
                    description: format!(
                        "Bloco canaleta (cintas, vergas e contravergas) — {}",
                        mt.masonry
                    ),
                    net_quantity: mt.u_blocks,
                    buy_quantity: u_bl,
                    waste_pct: block_waste_pct,
                    unit: "un".to_owned(),
                    commercial_note: format!(
                        "{} cinta + {} vergas/contravergas",
                        mt.bond_beam_u_blocks.ceil() as u32,
                        mt.lintel_u_blocks.ceil() as u32
                    ),
                });
            }
        }
        // Laying mortar (takeoff.mortar_m3 already had 10% baked in; recover net).
        let mortar_net = takeoff.mortar_m3 / 1.10;
        push_mix_items(
            stage4,
            &mut summary,
            &catalogue,
            MixSpec {
                stage: ConstructionStage::Masonry,
                discipline: "Alvenaria",
                context_label: "argamassa de assentamento (traço 1:2:8)",
                volume_net_m3: mortar_net,
                material_id: "mortar_laying",
            },
        );
        // U-block grout fill.
        let grout_net = takeoff.grout_m3 / 1.10;
        push_mix_items(
            stage4,
            &mut summary,
            &catalogue,
            MixSpec {
                stage: ConstructionStage::Masonry,
                discipline: "Alvenaria",
                context_label: "graute / microconcreto de canaletas e vergas",
                volume_net_m3: grout_net,
                material_id: "mortar_grout",
            },
        );
    }

    // -------------------------------------------------------------------------
    // Stage 5: Roof (Cobertura e telhado)
    // -------------------------------------------------------------------------
    if !structure.roofs.is_empty() {
        let timber_waste = catalogue.find("timber_roof").map_or(10.0, |m| m.waste_pct);
        let tile_waste = catalogue.find("tile_roof").map_or(5.0, |m| m.waste_pct);
        let total_timber_net: f64 = structure.roofs.iter().map(|r| r.timber_m3).sum();
        let total_surface_m2: f64 = structure.roofs.iter().map(|r| r.surface_m2).sum();
        let total_tiles: f64 = structure.roofs.iter().filter_map(|r| r.tiles).sum();
        let rplans = roof_plans(project);
        let total_gutter_m: f64 = rplans
            .iter()
            .flat_map(|rp| rp.gutters.iter())
            .map(|g| g.length_m())
            .sum();
        let total_downpipes: usize = rplans
            .iter()
            .flat_map(|rp| rp.gutters.iter())
            .map(|g| g.downpipes.len())
            .sum();

        let stage5 = by_stage.entry(ConstructionStage::Roof).or_default();
        if total_timber_net > 0.01 {
            let buy_timber =
                ((total_timber_net * (1.0 + timber_waste / 100.0)) * 100.0).ceil() / 100.0;
            stage5.push(ShoppingItem {
                item_code: "madeira_telhado_m3".to_owned(),
                stage: ConstructionStage::Roof,
                discipline: "Cobertura",
                description:
                    "Madeira estrutural serrada para telhado (ripas, caibros, terças e vigas)"
                        .to_owned(),
                net_quantity: total_timber_net,
                buy_quantity: buy_timber,
                waste_pct: timber_waste,
                unit: "m³".to_owned(),
                commercial_note: format!(
                    "{} telhado(s) · conferir bitolas na aba Telhado",
                    structure.roofs.len()
                ),
            });
        }
        if total_tiles > 1.0 {
            let buy_tiles = total_tiles.ceil();
            stage5.push(ShoppingItem {
                item_code: "telha_un".to_owned(),
                stage: ConstructionStage::Roof,
                discipline: "Cobertura",
                description: "Telhas de cobertura (cerâmica / concreto)".to_owned(),
                net_quantity: total_tiles / 1.05,
                buy_quantity: buy_tiles,
                waste_pct: tile_waste,
                unit: "un".to_owned(),
                commercial_note: format!(
                    "Superfície inclinada {} m² (já com reserva)",
                    decimal(total_surface_m2, 1)
                ),
            });
        } else if total_surface_m2 > 0.5 {
            let buy_m2 = (total_surface_m2 * (1.0 + tile_waste / 100.0)).ceil();
            stage5.push(ShoppingItem {
                item_code: "telha_m2".to_owned(),
                stage: ConstructionStage::Roof,
                discipline: "Cobertura",
                description: "Telhas em chapa (fibrocimento / metálica)".to_owned(),
                net_quantity: total_surface_m2,
                buy_quantity: buy_m2,
                waste_pct: tile_waste,
                unit: "m²".to_owned(),
                commercial_note: "Conferir transpasse longitudinal e fixadores".to_owned(),
            });
        }
        if total_gutter_m > 0.5 {
            let buy_gutter = (total_gutter_m * 1.05).ceil();
            stage5.push(ShoppingItem {
                item_code: "calha_m".to_owned(),
                stage: ConstructionStage::Roof,
                discipline: "Cobertura",
                description: "Calha galvanizada nº 24 com suportes e rufos".to_owned(),
                net_quantity: total_gutter_m,
                buy_quantity: buy_gutter,
                waste_pct: 5.0,
                unit: "m".to_owned(),
                commercial_note: format!("{total_downpipes} condutor(es) pluvial(is) Ø100 mm"),
            });
        }
    }

    // -------------------------------------------------------------------------
    // Stage 6: Installations (Elétrica + Hidrossanitária)
    // -------------------------------------------------------------------------
    {
        let stage6 = by_stage
            .entry(ConstructionStage::Installations)
            .or_default();

        // Electrical wires aggregated by section
        let mut wire_by_sec: BTreeMap<u32, (f64, f64, u32)> = BTreeMap::new();
        for w in &elec.wires {
            let key = (w.section_mm2 * 10.0).round() as u32;
            let entry = wire_by_sec.entry(key).or_insert((0.0, 0.0, 0));
            entry.0 += w.net_length_m;
            entry.1 += w.total_length_m;
            entry.2 += w.rolls_100m;
        }
        for (sec10, (net_m, total_m, rolls)) in wire_by_sec {
            let sec = f64::from(sec10) / 10.0;
            let sec_str = if sec.fract().abs() < 1e-6 {
                format!("{sec:.0}")
            } else {
                decimal(sec, 1)
            };
            summary.wire_total_m += total_m;
            stage6.push(ShoppingItem {
                item_code: format!("cabo_cobre_{sec10}_m"),
                stage: ConstructionStage::Installations,
                discipline: "Elétrica",
                description: format!(
                    "Cabo flexível de cobre 750 V — {sec_str} mm² (cores NBR 5410)"
                ),
                net_quantity: net_m,
                buy_quantity: total_m.ceil(),
                waste_pct: elec.waste_pct,
                unit: "m".to_owned(),
                commercial_note: format!("{rolls} rolo(s) de 100 m (ver cores na aba Elétrica)"),
            });
        }

        // Conduits
        if elec.total_conduit_m > 0.5 {
            summary.conduit_total_m += elec.total_conduit_m;
            let coils = elec
                .conduits
                .iter()
                .map(|c| c.rolls_50m)
                .sum::<u32>()
                .max(1);
            stage6.push(ShoppingItem {
                item_code: "eletroduto_m".to_owned(),
                stage: ConstructionStage::Installations,
                discipline: "Elétrica",
                description: "Eletroduto corrugado flexível PVC anti-chama (Ø20–Ø32 mm)".to_owned(),
                net_quantity: elec.total_conduit_net_m,
                buy_quantity: elec.total_conduit_m.ceil(),
                waste_pct: elec.waste_pct,
                unit: "m".to_owned(),
                commercial_note: format!("{coils} rolo(s) de 50 m"),
            });
        }

        // Electrical boxes & DIN boards
        let box_total: usize = elec.boxes.iter().map(|b| b.quantity).sum();
        if box_total > 0 {
            stage6.push(ShoppingItem {
                item_code: "caixa_eletrica_un".to_owned(),
                stage: ConstructionStage::Installations,
                discipline: "Elétrica",
                description: "Caixas de passagem (4×2\", 4×4\" octogonais) e quadros DIN"
                    .to_owned(),
                net_quantity: box_total as f64,
                buy_quantity: box_total as f64,
                waste_pct: 0.0,
                unit: "un".to_owned(),
                commercial_note: format!(
                    "{} tipo(s) na lista elétrica detalhada",
                    elec.boxes.len()
                ),
            });
        }

        // Breakers, RCDs, SPDs
        let mut breakers_qty = 0usize;
        let mut dr_dps_qty = 0usize;
        for p in &elec.protections {
            match p.category {
                crate::domain::electrical::shopping_list::ProtectionCategory::Rcd
                | crate::domain::electrical::shopping_list::ProtectionCategory::Dps => {
                    dr_dps_qty += p.quantity;
                }
                _ => breakers_qty += p.quantity,
            }
        }
        if breakers_qty > 0 {
            stage6.push(ShoppingItem {
                item_code: "disjuntor_un".to_owned(),
                stage: ConstructionStage::Installations,
                discipline: "Elétrica",
                description: "Disjuntores termomagnéticos DIN (geral e terminais)".to_owned(),
                net_quantity: breakers_qty as f64,
                buy_quantity: breakers_qty as f64,
                waste_pct: 0.0,
                unit: "un".to_owned(),
                commercial_note: "Curvas B/C conforme quadro de cargas".to_owned(),
            });
        }
        if dr_dps_qty > 0 {
            stage6.push(ShoppingItem {
                item_code: "dr_dps_un".to_owned(),
                stage: ConstructionStage::Installations,
                discipline: "Elétrica",
                description: "Interruptores DR (30 mA) e protetores DPS Classe II".to_owned(),
                net_quantity: dr_dps_qty as f64,
                buy_quantity: dr_dps_qty as f64,
                waste_pct: 0.0,
                unit: "un".to_owned(),
                commercial_note: "Proteção obrigatória NBR 5410 §5.1.3.2".to_owned(),
            });
        }

        let dev_qty: usize = elec.devices.iter().map(|d| d.quantity).sum();
        if dev_qty > 0 {
            stage6.push(ShoppingItem {
                item_code: "ponto_acabamento_un".to_owned(),
                stage: ConstructionStage::Installations,
                discipline: "Elétrica",
                description: "Conjuntos de tomadas 10 A / 20 A, interruptores e receptáculos"
                    .to_owned(),
                net_quantity: dev_qty as f64,
                buy_quantity: dev_qty as f64,
                waste_pct: 0.0,
                unit: "un".to_owned(),
                commercial_note: "Instalar após pintura final".to_owned(),
            });
        }

        // Plumbing cold water & sewage
        let pipe_waste = catalogue
            .find("pipe_cold_water")
            .map_or(10.0, |m| m.waste_pct);
        if let Some(design) = &cw {
            stage6.push(ShoppingItem {
                item_code: "caixa_dagua_un".to_owned(),
                stage: ConstructionStage::Installations,
                discipline: "Hidráulica",
                description: format!(
                    "Caixa-d'água polietileno com tampa ({:.0} L)",
                    design.tank_l
                ),
                net_quantity: 1.0,
                buy_quantity: 1.0,
                waste_pct: 0.0,
                unit: "un".to_owned(),
                commercial_note: format!(
                    "Reserva para {} morador(es) · peso cheia {} kN",
                    design.residents,
                    decimal(design.tank_full_kn, 1)
                ),
            });

            let mut af_by_dn: BTreeMap<u32, f64> = BTreeMap::new();
            let mut fittings_count = 0usize;
            for pipe in &design.network.pipes {
                let dn = pipe.dn.round() as u32;
                *af_by_dn.entry(dn).or_insert(0.0) += pipe.length_m;
                fittings_count += pipe.fittings.len();
            }
            for (dn, net_m) in af_by_dn {
                let total_m = (net_m * (1.0 + pipe_waste / 100.0)).ceil();
                let bars_6m = (total_m / 6.0).ceil() as u32;
                summary.pvc_pipe_total_m += total_m;
                stage6.push(ShoppingItem {
                    item_code: format!("tubo_pvc_af_{dn}_m"),
                    stage: ConstructionStage::Installations,
                    discipline: "Hidráulica",
                    description: format!("Tubo PVC soldável água fria DN {dn} mm"),
                    net_quantity: net_m,
                    buy_quantity: total_m,
                    waste_pct: pipe_waste,
                    unit: "m".to_owned(),
                    commercial_note: format!("{bars_6m} barra(s) comercial(is) de 6 m"),
                });
            }
            if fittings_count > 0 {
                stage6.push(ShoppingItem {
                    item_code: "conexao_pvc_un".to_owned(),
                    stage: ConstructionStage::Installations,
                    discipline: "Hidráulica",
                    description: "Conexões PVC soldável (joelhos, tês, registros e adesivo)"
                        .to_owned(),
                    net_quantity: fittings_count as f64,
                    buy_quantity: (fittings_count as f64 * 1.15).ceil(),
                    waste_pct: 15.0,
                    unit: "un".to_owned(),
                    commercial_note: "Inclui 15 % de folga para desvios em obra".to_owned(),
                });
            }
        }

        let drain_pts = plumbing_pts.count(PointKind::Drain);
        if drain_pts > 0 {
            let sewage_net_m = (drain_pts as f64 * 4.5).max(12.0);
            let sewage_buy_m = (sewage_net_m * (1.0 + pipe_waste / 100.0)).ceil();
            let bars_6m = (sewage_buy_m / 6.0).ceil() as u32;
            summary.pvc_pipe_total_m += sewage_buy_m;
            stage6.push(ShoppingItem {
                item_code: "tubo_pvc_esgoto_m".to_owned(),
                stage: ConstructionStage::Installations,
                discipline: "Hidráulica",
                description: "Tubo PVC esgoto série normal (DN 40 / 50 / 100 mm) e caixas"
                    .to_owned(),
                net_quantity: sewage_net_m,
                buy_quantity: sewage_buy_m,
                waste_pct: pipe_waste,
                unit: "m".to_owned(),
                commercial_note: format!(
                    "{bars_6m} barra(s) de 6 m estimadas para {drain_pts} ponto(s) de esgoto"
                ),
            });
        }
    }

    // -------------------------------------------------------------------------
    // Stage 7: Plaster & waterproofing (Chapisco, reboco e impermeabilização)
    // -------------------------------------------------------------------------
    {
        let stage7 = by_stage
            .entry(ConstructionStage::PlasterAndWaterproofing)
            .or_default();
        let plaster_net = takeoff.plaster_m3 / 1.10;
        push_mix_items(
            stage7,
            &mut summary,
            &catalogue,
            MixSpec {
                stage: ConstructionStage::PlasterAndWaterproofing,
                discipline: "Revestimento",
                context_label: "chapisco e reboco de paredes (traço 1:2:6)",
                volume_net_m3: plaster_net,
                material_id: "mortar_plaster",
            },
        );
        if takeoff.waterproofing_m2 > 0.5 {
            let kg = (takeoff.waterproofing_m2 * 3.5 * 1.10).ceil();
            stage7.push(ShoppingItem {
                item_code: "impermeabilizante_kg".to_owned(),
                stage: ConstructionStage::PlasterAndWaterproofing,
                discipline: "Impermeabilização",
                description: "Argamassa polimérica impermeabilizante para áreas molhadas"
                    .to_owned(),
                net_quantity: takeoff.waterproofing_m2 * 3.5,
                buy_quantity: kg,
                waste_pct: 10.0,
                unit: "kg".to_owned(),
                commercial_note: format!(
                    "~{} cx de 18 kg para {} m² de banheiros/cozinha/serviço",
                    (kg / 18.0).ceil() as u32,
                    decimal(takeoff.waterproofing_m2, 1)
                ),
            });
        }
    }

    // -------------------------------------------------------------------------
    // Stage 8: Finishes & paint (Acabamentos e pintura)
    // -------------------------------------------------------------------------
    {
        let stage8 = by_stage.entry(ConstructionStage::Finishes).or_default();
        if takeoff.floor_m2 > 0.5 {
            let floor_buy = takeoff.floor_with_waste_m2.ceil();
            stage8.push(ShoppingItem {
                item_code: "piso_m2".to_owned(),
                stage: ConstructionStage::Finishes,
                discipline: "Acabamentos",
                description: "Revestimento de piso (cerâmica / porcelanato conforme cômodos)"
                    .to_owned(),
                net_quantity: takeoff.floor_m2,
                buy_quantity: floor_buy,
                waste_pct: 10.0,
                unit: "m²".to_owned(),
                commercial_note: format!("{} cômodo(s) · +10 % para recortes", takeoff.rooms.len()),
            });
            let adhesive_bags = ((floor_buy * 5.0) / 20.0).ceil();
            stage8.push(ShoppingItem {
                item_code: "argamassa_colante_20kg".to_owned(),
                stage: ConstructionStage::Finishes,
                discipline: "Acabamentos",
                description: "Argamassa colante AC-II / AC-III (saco 20 kg)".to_owned(),
                net_quantity: (takeoff.floor_m2 * 5.0) / 20.0,
                buy_quantity: adhesive_bags,
                waste_pct: 10.0,
                unit: "sc 20 kg".to_owned(),
                commercial_note: "Consumo ref. 5 kg/m² (dupla colagem)".to_owned(),
            });
            let grout_kg = (floor_buy * 0.4).ceil().max(1.0);
            stage8.push(ShoppingItem {
                item_code: "rejunte_kg".to_owned(),
                stage: ConstructionStage::Finishes,
                discipline: "Acabamentos",
                description: "Rejunte flexível para pisos".to_owned(),
                net_quantity: takeoff.floor_m2 * 0.4,
                buy_quantity: grout_kg,
                waste_pct: 10.0,
                unit: "kg".to_owned(),
                commercial_note: "Consumo ref. 0,4 kg/m²".to_owned(),
            });
        }
        if takeoff.skirting_m > 0.5 {
            let buy_sk = (takeoff.skirting_m * 1.10).ceil();
            stage8.push(ShoppingItem {
                item_code: "rodape_m".to_owned(),
                stage: ConstructionStage::Finishes,
                discipline: "Acabamentos",
                description: "Rodapé cerâmico / madeira / poliestireno".to_owned(),
                net_quantity: takeoff.skirting_m,
                buy_quantity: buy_sk,
                waste_pct: 10.0,
                unit: "m".to_owned(),
                commercial_note: "Perímetro dos cômodos descontando vãos de portas".to_owned(),
            });
        }
        let paint_area_m2 = takeoff.room_walls_m2 + takeoff.ceiling_m2;
        if paint_area_m2 > 1.0 {
            // 2 coats + sealer ≈ 10 m²/L per coat → 5 m²/L effective.
            let liters = (paint_area_m2 / 5.0) * 1.10;
            let cans_18l = (liters / 18.0).ceil().max(1.0);
            stage8.push(ShoppingItem {
                item_code: "tinta_18l".to_owned(),
                stage: ConstructionStage::Finishes,
                discipline: "Acabamentos",
                description: "Tinta acrílica fosca/semibrilho (lata 18 L — paredes e tetos)"
                    .to_owned(),
                net_quantity: liters / 18.0,
                buy_quantity: cans_18l,
                waste_pct: 10.0,
                unit: "lata 18 L".to_owned(),
                commercial_note: format!(
                    "Área {} m² (2 demãos, rendimento ref. 10 m²/L·demão)",
                    decimal(paint_area_m2, 1)
                ),
            });
        }
    }

    let stages = ConstructionStage::ALL
        .into_iter()
        .map(|stage| StageShoppingGroup {
            stage,
            items: by_stage.remove(&stage).unwrap_or_default(),
        })
        .filter(|g| !g.items.is_empty())
        .collect();

    StageShoppingList {
        stages,
        summary,
        assumptions: vec![
            "Compre os materiais por etapa (principalmente cimento e cal) para evitar empedramento no canteiro.".to_owned(),
            "Conversões de concreto e argamassas usam os traços e perdas definidos no Catálogo de materiais (#135).".to_owned(),
            "Armaduras em barras de 12 m, cabos em rolos de 100 m, eletrodutos em rolos de 50 m e tubos PVC em barras de 6 m.".to_owned(),
            "Quantidades de pré-dimensionamento: confirme com o profissional responsável (ART/RRT) e com o projeto executivo antes de grandes compras.".to_owned(),
        ],
    }
}

/// UTF-8 BOM CSV export of the stage-by-stage shopping list (#136).
pub fn stage_shopping_list_csv(project: &Project) -> String {
    let list = build_stage_shopping_list(project);
    let mut csv = String::from(
        "\u{feff}etapa,disciplina,codigo,item,liquido,comprar,perda_pct,unidade,observacao_comercial\r\n",
    );
    for group in &list.stages {
        for item in &group.items {
            let _ = writeln!(
                csv,
                "\"{}\",\"{}\",{},\"{}\",{:.2},{:.2},{:.1},{},\"{}\"\r",
                group.stage.label(),
                item.discipline,
                item.item_code,
                item.description.replace('"', "\"\""),
                item.net_quantity,
                item.buy_quantity,
                item.waste_pct,
                item.unit,
                item.commercial_note.replace('"', "\"\"")
            );
        }
    }
    csv
}

/// Printable A3 landscape SVG sheet of the stage-by-stage shopping list (#136).
pub fn stage_shopping_list_svg(project: &Project) -> String {
    let list = build_stage_shopping_list(project);
    let rows = list.total_items() + list.stages.len() * 2;
    let width = 420.0;
    let height = (90.0 + rows as f64 * 6.8 + 25.0).max(297.0);
    let ink = "#1d2836";
    let muted = "#4a5d73";
    let font = "font-family=\"Arial, sans-serif\"";

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height:.0}mm\" viewBox=\"0 0 {width} {height:.0}\">\n\
         <rect width=\"{width}\" height=\"{height:.0}\" fill=\"#ffffff\"/>\n\
         <rect x=\"10\" y=\"10\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>\n",
        width - 20.0,
        height - 20.0
    );
    let text = |svg: &mut String,
                x: f64,
                y: f64,
                size: f64,
                bold: bool,
                color: &str,
                value: &str| {
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
        "LISTA DE COMPRAS POR ETAPA DA OBRA (TODAS AS DISCIPLINAS)",
    );
    text(
        &mut svg,
        18.0,
        28.5,
        2.9,
        false,
        muted,
        &format!(
            "{} · {} sc cimento 50 kg · {} sc cal 20 kg · {} m³ areia · {} m³ brita · {:.0} kg aço CA-50 · {} blocos",
            project.name,
            list.summary.cement_bags_50kg,
            list.summary.lime_bags_20kg,
            decimal(list.summary.sand_m3, 1),
            decimal(list.summary.gravel_m3, 1),
            list.summary.steel_ca50_kg,
            list.summary.total_blocks
        ),
    );

    let mut y = 38.0;
    for group in &list.stages {
        let _ = writeln!(
            svg,
            "<rect x=\"16\" y=\"{:.1}\" width=\"388\" height=\"6.5\" fill=\"#e8eef5\" stroke=\"{ink}\" stroke-width=\"0.25\"/>",
            y - 4.4
        );
        text(&mut svg, 18.0, y, 2.9, true, ink, group.stage.label());
        y += 7.0;

        for (title, x) in [
            ("Disciplina", 18.0),
            ("Material", 48.0),
            ("Líquido", 190.0),
            ("Comprar", 215.0),
            ("Unid.", 240.0),
            ("Observação comercial", 262.0),
        ] {
            text(&mut svg, x, y, 2.2, true, muted, title);
        }
        y += 5.2;

        for item in &group.items {
            text(&mut svg, 18.0, y, 2.3, false, muted, item.discipline);
            text(&mut svg, 48.0, y, 2.4, true, ink, &item.description);
            text(
                &mut svg,
                190.0,
                y,
                2.3,
                false,
                muted,
                &decimal(item.net_quantity, 1),
            );
            text(
                &mut svg,
                215.0,
                y,
                2.5,
                true,
                ink,
                &decimal(item.buy_quantity, 1),
            );
            text(&mut svg, 240.0, y, 2.3, false, ink, &item.unit);
            text(&mut svg, 262.0, y, 2.2, false, muted, &item.commercial_note);
            let _ = writeln!(
                svg,
                "<line x1=\"16\" y1=\"{:.1}\" x2=\"404\" y2=\"{:.1}\" stroke=\"#dce4ec\" stroke-width=\"0.15\"/>",
                y + 1.8,
                y + 1.8
            );
            y += 6.2;
        }
        y += 3.5;
    }

    svg.push_str("</svg>\n");
    svg
}

impl Project {
    /// Complete shopping list organized by construction stage across all disciplines (#136).
    pub fn stage_shopping_list(&self) -> StageShoppingList {
        build_stage_shopping_list(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::samples::SampleProject;

    #[test]
    fn expand_mix_volume_matches_hand_calculation_for_c20_concrete() {
        let project = Project::default();
        let cat = project.materials_catalogue();
        // 2.0 m³ net C20 concrete with 5% waste => 2.10 m³ gross.
        // Cement: 2.10 * 315 = 661.5 kg => ceil(661.5 / 50) = 14 bags of 50 kg.
        // Sand: 2.10 * 0.62 = 1.302 m³; Gravel: 2.10 * 0.84 = 1.764 m³.
        let (vol, cem_bags, lime_bags, sand, gravel, waste) =
            expand_mix_volume(2.0, &cat, "concrete_structural");
        assert!((waste - 5.0).abs() < 1e-6);
        assert!((vol - 2.10).abs() < 1e-6);
        assert_eq!(cem_bags, 14);
        assert_eq!(lime_bags, 0);
        assert!((sand - 1.302).abs() < 1e-6);
        assert!((gravel - 1.764).abs() < 1e-6);
    }

    #[test]
    fn sample_house_produces_complete_stage_shopping_list_and_exports() {
        let mut project = SampleProject::CasaTerrea.build();
        project.generate_structure();
        project.generate_electrical_points();
        project.assign_circuits();

        let list = project.stage_shopping_list();
        assert!(list.stages.len() >= 6, "expected stages across disciplines");
        assert!(list.summary.cement_bags_50kg > 20);
        assert!(list.summary.sand_m3 > 1.0);
        assert!(list.summary.gravel_m3 > 1.0);
        assert!(list.summary.steel_ca50_kg > 50.0);
        assert!(list.summary.total_blocks > 100);
        assert!(list.summary.pvc_pipe_total_m > 5.0);

        let csv = stage_shopping_list_csv(&project);
        assert!(csv.starts_with("\u{feff}etapa,disciplina"));
        assert!(csv.contains("cimento_50kg"));
        assert!(csv.contains("aco_ca50_kg"));

        let svg = stage_shopping_list_svg(&project);
        assert!(svg.contains("LISTA DE COMPRAS POR ETAPA"));
        assert!(svg.contains("</svg>"));
    }
}
