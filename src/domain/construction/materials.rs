//! Editable materials catalogue (#135): detects every material used across
//! structure, masonry, mortars, roofs, plumbing and electrical calculations,
//! exposes its physical/normative properties and mix ratios, persists user
//! overrides on `Project::construction`, propagates engineering property edits
//! back to the model and exports CSV / A3 SVG.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::domain::{
    model::Project,
    plumbing::cold_water::cold_water,
    reinforcement::{COMMERCIAL_BAR_LENGTH_M, STEEL_DENSITY_KG_M3, STEEL_FYK_MPA},
};

/// Material discipline/family in the catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialCategory {
    Concrete,
    Steel,
    Block,
    Mortar,
    Timber,
    Tile,
    Pipe,
    Wire,
}

impl MaterialCategory {
    pub const ALL: [Self; 8] = [
        Self::Concrete,
        Self::Steel,
        Self::Block,
        Self::Mortar,
        Self::Timber,
        Self::Tile,
        Self::Pipe,
        Self::Wire,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Concrete => "Concreto",
            Self::Steel => "Aço",
            Self::Block => "Blocos e tijolos",
            Self::Mortar => "Argamassas e graute",
            Self::Timber => "Madeira",
            Self::Tile => "Telhas",
            Self::Pipe => "Tubulações",
            Self::Wire => "Fios e eletrodutos",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Concrete => "concreto",
            Self::Steel => "aco",
            Self::Block => "bloco",
            Self::Mortar => "argamassa",
            Self::Timber => "madeira",
            Self::Tile => "telha",
            Self::Pipe => "tubo",
            Self::Wire => "cabo",
        }
    }
}

/// Persisted user override for one material in `Project::construction`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub density_kg_m3: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strength_mpa: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waste_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mix_cement_kg_m3: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mix_lime_kg_m3: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mix_sand_m3_per_m3: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mix_gravel_m3_per_m3: Option<f64>,
}

impl MaterialOverride {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.density_kg_m3.is_none()
            && self.strength_mpa.is_none()
            && self.waste_pct.is_none()
            && self.mix_cement_kg_m3.is_none()
            && self.mix_lime_kg_m3.is_none()
            && self.mix_sand_m3_per_m3.is_none()
            && self.mix_gravel_m3_per_m3.is_none()
    }
}

/// One resolved material entry detected from the model (with overrides applied).
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialEntry {
    pub id: String,
    pub category: MaterialCategory,
    pub name: String,
    pub standard: String,
    pub unit: String,
    pub density_kg_m3: Option<f64>,
    pub strength_mpa: Option<f64>,
    pub waste_pct: f64,
    /// Mix consumption per m³ of final concrete/mortar: cement (kg/m³).
    pub mix_cement_kg_m3: Option<f64>,
    /// Mix consumption per m³ of final mortar: hydrated lime (kg/m³).
    pub mix_lime_kg_m3: Option<f64>,
    /// Mix consumption per m³ of final concrete/mortar: washed sand (m³/m³).
    pub mix_sand_m3_per_m3: Option<f64>,
    /// Mix consumption per m³ of final concrete: crushed gravel (m³/m³).
    pub mix_gravel_m3_per_m3: Option<f64>,
    pub properties_note: String,
    pub used_in_model: bool,
    pub usage_summary: String,
    pub edited: bool,
}

/// Full materials catalogue for a project (#135).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialsCatalogue {
    pub items: Vec<MaterialEntry>,
    pub active_count: usize,
    pub edited_count: usize,
}

impl MaterialsCatalogue {
    pub fn find(&self, id: &str) -> Option<&MaterialEntry> {
        self.items.iter().find(|m| m.id == id)
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

/// Standard Brazilian concrete mix recipe `(cement_kg/m³, sand_m³/m³, gravel_m³/m³, ratio_label)`
/// by characteristic strength `fck` (MPa, ABNT NBR 12655 / TCPO).
pub fn concrete_mix_for_fck(fck_mpa: f64) -> (f64, f64, f64, &'static str) {
    if fck_mpa >= 30.0 - 1e-6 {
        (410.0, 0.56, 0.80, "1 : 1,8 : 2,5 (C30)")
    } else if fck_mpa >= 25.0 - 1e-6 {
        (360.0, 0.58, 0.82, "1 : 2,0 : 3,0 (C25)")
    } else {
        (315.0, 0.62, 0.84, "1 : 2,5 : 3,5 (C20)")
    }
}

fn apply_override(entry: &mut MaterialEntry, ov: &MaterialOverride) {
    if let Some(name) = &ov.name
        && !name.trim().is_empty()
    {
        entry.name = name.trim().to_owned();
    }
    if let Some(d) = ov.density_kg_m3 {
        entry.density_kg_m3 = Some(d);
    }
    if let Some(s) = ov.strength_mpa {
        entry.strength_mpa = Some(s);
    }
    if let Some(w) = ov.waste_pct {
        entry.waste_pct = w;
    }
    if let Some(c) = ov.mix_cement_kg_m3 {
        entry.mix_cement_kg_m3 = Some(c);
    }
    if let Some(l) = ov.mix_lime_kg_m3 {
        entry.mix_lime_kg_m3 = Some(l);
    }
    if let Some(s) = ov.mix_sand_m3_per_m3 {
        entry.mix_sand_m3_per_m3 = Some(s);
    }
    if let Some(g) = ov.mix_gravel_m3_per_m3 {
        entry.mix_gravel_m3_per_m3 = Some(g);
    }
    entry.edited = !ov.is_empty();
}

/// Detects every material active in `project` across all disciplines and applies
/// any persisted overrides from `project.construction.material_overrides`.
pub fn detect_materials_catalogue(project: &Project) -> MaterialsCatalogue {
    let mut items = Vec::new();
    let s = project.structure.settings;
    let has_structure = !project.structure.beams.is_empty()
        || !project.structure.slabs.is_empty()
        || !project.structure.wall_foundations.is_empty()
        || project.fixtures.iter().any(|f| f.kind.is_structural());

    // 1. Structural concrete (fck from ProjectStructure).
    let (cem, sand, gravel, ratio_lbl) = concrete_mix_for_fck(s.fck_mpa);
    let e_ci = 5600.0 * s.fck_mpa.max(15.0).sqrt();
    items.push(MaterialEntry {
        id: "concrete_structural".to_owned(),
        category: MaterialCategory::Concrete,
        name: format!(
            "Concreto estrutural C{:.0} (fck {:.0} MPa)",
            s.fck_mpa, s.fck_mpa
        ),
        standard: "ABNT NBR 6118:2023 · NBR 8953".to_owned(),
        unit: "m³".to_owned(),
        density_kg_m3: Some(2500.0),
        strength_mpa: Some(s.fck_mpa),
        waste_pct: 5.0,
        mix_cement_kg_m3: Some(cem),
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: Some(sand),
        mix_gravel_m3_per_m3: Some(gravel),
        properties_note: format!(
            "Traço ref. {ratio_lbl} · Eci ≈ {:.0} MPa · brita 1 · slump 10±2 cm",
            e_ci
        ),
        used_in_model: has_structure,
        usage_summary: if has_structure {
            "Fundações, pilares, vigas, baldrames e lajes".to_owned()
        } else {
            "Configurado na estrutura (sem elementos lançados ainda)".to_owned()
        },
        edited: false,
    });

    // 2. Lean concrete (*concreto magro* / lastro).
    items.push(MaterialEntry {
        id: "concrete_lean".to_owned(),
        category: MaterialCategory::Concrete,
        name: "Concreto magro / lastro (10 MPa · e = 5 cm)".to_owned(),
        standard: "ABNT NBR 6122 · TCPO".to_owned(),
        unit: "m³".to_owned(),
        density_kg_m3: Some(2300.0),
        strength_mpa: Some(10.0),
        waste_pct: 5.0,
        mix_cement_kg_m3: Some(210.0),
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: Some(0.68),
        mix_gravel_m3_per_m3: Some(0.86),
        properties_note: "Traço ref. 1 : 3,5 : 4,5 · regularização de fundo de valas e contrapiso"
            .to_owned(),
        used_in_model: has_structure || !project.rooms.is_empty(),
        usage_summary: "Lastro de fundações e base de contrapiso".to_owned(),
        edited: false,
    });

    // 3. Reinforcing steel CA-50.
    items.push(MaterialEntry {
        id: "steel_ca50".to_owned(),
        category: MaterialCategory::Steel,
        name: "Aço nervurado CA-50 (fyk 500 MPa)".to_owned(),
        standard: "ABNT NBR 7480 · NBR 6118".to_owned(),
        unit: "kg".to_owned(),
        density_kg_m3: Some(STEEL_DENSITY_KG_M3),
        strength_mpa: Some(STEEL_FYK_MPA),
        waste_pct: 10.0,
        mix_cement_kg_m3: None,
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: None,
        mix_gravel_m3_per_m3: None,
        properties_note: format!(
            "Es = 210 GPa · barras comerciais de {:.0} m (Ø6,3 a Ø25 mm)",
            COMMERCIAL_BAR_LENGTH_M
        ),
        used_in_model: has_structure,
        usage_summary: "Armaduras de sapatas, baldrames, pilares, vigas e lajes".to_owned(),
        edited: false,
    });

    // 4. Masonry blocks (project default + any per-wall custom masonry).
    let masonry_density = s.masonry_kn_m3 * 1000.0 / 9.81;
    let m = &project.masonry;
    items.push(MaterialEntry {
        id: "block_default".to_owned(),
        category: MaterialCategory::Block,
        name: m.name.clone(),
        standard: "ABNT NBR 15270 / NBR 6136 / NBR 16868".to_owned(),
        unit: "un".to_owned(),
        density_kg_m3: Some(masonry_density.round()),
        strength_mpa: Some(3.0),
        waste_pct: project.schedules.waste_percent,
        mix_cement_kg_m3: None,
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: None,
        mix_gravel_m3_per_m3: None,
        properties_note: format!(
            "{:.0}×{:.0}×{:.0} cm · junta {:.1} cm · reboco {:.1} cm · {} un/m²",
            m.block_length * 100.0,
            m.block_width * 100.0,
            m.block_height * 100.0,
            m.joint * 100.0,
            m.plaster * 100.0,
            decimal(m.blocks_per_square_metre(), 1)
        ),
        used_in_model: !project.walls.is_empty(),
        usage_summary: format!("Alvenaria padrão ({} parede(s))", project.walls.len()),
        edited: false,
    });

    // 5. Mortars: laying mortar, plaster/stucco, and U-block grout.
    items.push(MaterialEntry {
        id: "mortar_laying".to_owned(),
        category: MaterialCategory::Mortar,
        name: "Argamassa de assentamento (traço 1:2:8)".to_owned(),
        standard: "ABNT NBR 13281 · SINAPI".to_owned(),
        unit: "m³".to_owned(),
        density_kg_m3: Some(1900.0),
        strength_mpa: Some(4.0),
        waste_pct: 10.0,
        mix_cement_kg_m3: Some(165.0),
        mix_lime_kg_m3: Some(170.0),
        mix_sand_m3_per_m3: Some(1.05),
        mix_gravel_m3_per_m3: None,
        properties_note: "Cimento CP II + cal hidratada CH-III + areia média lavada".to_owned(),
        used_in_model: !project.walls.is_empty(),
        usage_summary: "Assentamento de blocos e tijolos das paredes".to_owned(),
        edited: false,
    });

    items.push(MaterialEntry {
        id: "mortar_plaster".to_owned(),
        category: MaterialCategory::Mortar,
        name: "Argamassa de emboço / reboco (traço 1:2:6)".to_owned(),
        standard: "ABNT NBR 7200 · NBR 13749 · SINAPI".to_owned(),
        unit: "m³".to_owned(),
        density_kg_m3: Some(1900.0),
        strength_mpa: Some(3.5),
        waste_pct: 10.0,
        mix_cement_kg_m3: Some(205.0),
        mix_lime_kg_m3: Some(205.0),
        mix_sand_m3_per_m3: Some(1.02),
        mix_gravel_m3_per_m3: None,
        properties_note: "Revestimento interno e externo nas duas faces da alvenaria".to_owned(),
        used_in_model: !project.walls.is_empty(),
        usage_summary: "Chapisco e reboco de paredes".to_owned(),
        edited: false,
    });

    items.push(MaterialEntry {
        id: "mortar_grout".to_owned(),
        category: MaterialCategory::Mortar,
        name: "Graute / microconcreto para canaletas (traço 1:2:2)".to_owned(),
        standard: "ABNT NBR 16868 · NBR 8953".to_owned(),
        unit: "m³".to_owned(),
        density_kg_m3: Some(2200.0),
        strength_mpa: Some(20.0),
        waste_pct: 10.0,
        mix_cement_kg_m3: Some(400.0),
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: Some(0.62),
        mix_gravel_m3_per_m3: Some(0.62),
        properties_note:
            "Cimento + areia grossa + pedrisco (brita 0) para cintas, vergas e contravergas"
                .to_owned(),
        used_in_model: !project.walls.is_empty(),
        usage_summary: "Preenchimento de canaletas de cinta, vergas e contravergas".to_owned(),
        edited: false,
    });

    // 6. Roof timber & tiles (detected from `project.structure.roofs`).
    let first_roof = project.structure.roofs.first();
    let (timber_name, timber_density, timber_fc0k) = match first_roof {
        Some(r) => {
            let (fc0k, _, _, density) = r.timber.properties();
            (
                format!("Madeira estrutural serrada {}", r.timber.label()),
                density,
                fc0k,
            )
        }
        None => (
            "Madeira estrutural serrada Dicotiledônea C30 (ref.)".to_owned(),
            800.0,
            30.0,
        ),
    };
    items.push(MaterialEntry {
        id: "timber_roof".to_owned(),
        category: MaterialCategory::Timber,
        name: timber_name,
        standard: "ABNT NBR 7190:2022".to_owned(),
        unit: "m³".to_owned(),
        density_kg_m3: Some(timber_density),
        strength_mpa: Some(timber_fc0k),
        waste_pct: 10.0,
        mix_cement_kg_m3: None,
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: None,
        mix_gravel_m3_per_m3: None,
        properties_note: format!(
            "fc0k = {:.0} MPa · densidade {:.0} kg/m³ · ripas, caibros, terças, vigas e tesouras",
            timber_fc0k, timber_density
        ),
        used_in_model: !project.structure.roofs.is_empty()
            || project
                .fixtures
                .iter()
                .any(|f| f.kind == crate::domain::fixtures::FixtureKind::TimberPost),
        usage_summary: if project.structure.roofs.is_empty() {
            "Padrão estrutural (sem telhado modelado)".to_owned()
        } else {
            format!("{} telhado(s) modelado(s)", project.structure.roofs.len())
        },
        edited: false,
    });

    let (tile_name, tile_note) = match first_roof {
        Some(r) => (
            format!("Telha {}", r.tile.label()),
            format!(
                "Peso {} kN/m² · inclinação mínima {:.0} %",
                decimal(r.tile.weight_kn_m2(), 2),
                r.tile.min_slope_percent()
            ),
        ),
        None => (
            "Telha cerâmica romana / colonial (ref.)".to_owned(),
            "Peso 0,45 kN/m² · inclinação mínima 30 % · ~16 un/m²".to_owned(),
        ),
    };
    items.push(MaterialEntry {
        id: "tile_roof".to_owned(),
        category: MaterialCategory::Tile,
        name: tile_name,
        standard: "ABNT NBR 15310 / NBR 15210".to_owned(),
        unit: "m²".to_owned(),
        density_kg_m3: None,
        strength_mpa: None,
        waste_pct: 5.0,
        mix_cement_kg_m3: None,
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: None,
        mix_gravel_m3_per_m3: None,
        properties_note: tile_note,
        used_in_model: !project.structure.roofs.is_empty(),
        usage_summary: if project.structure.roofs.is_empty() {
            "Padrão de cobertura (sem telhado modelado)".to_owned()
        } else {
            "Cobertura dos telhados modelados".to_owned()
        },
        edited: false,
    });

    // 7. Plumbing pipes (cold water & sewage, detected from plumbing points/network).
    let cw = cold_water(project);
    let has_plumbing =
        cw.is_some() || !crate::domain::plumbing::points::plumbing_points(project).is_empty();
    let pipe_dns: String = match &cw {
        Some(design) => {
            let mut dns: Vec<u32> = design
                .network
                .pipes
                .iter()
                .map(|p| p.dn.round() as u32)
                .collect();
            dns.sort_unstable();
            dns.dedup();
            if dns.is_empty() {
                "DN 20 a DN 50".to_owned()
            } else {
                dns.iter()
                    .map(|d| format!("DN {d}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        None => "DN 20 a DN 50".to_owned(),
    };
    items.push(MaterialEntry {
        id: "pipe_cold_water".to_owned(),
        category: MaterialCategory::Pipe,
        name: "Tubo PVC rígido soldável (água fria)".to_owned(),
        standard: "ABNT NBR 5648 · NBR 5626".to_owned(),
        unit: "m".to_owned(),
        density_kg_m3: Some(1400.0),
        strength_mpa: None,
        waste_pct: 10.0,
        mix_cement_kg_m3: None,
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: None,
        mix_gravel_m3_per_m3: None,
        properties_note: format!("Diâmetros no projeto: {pipe_dns} · barras 6 m · PN 750 kPa"),
        used_in_model: has_plumbing,
        usage_summary: if has_plumbing {
            "Barrilete, colunas, ramais e sub-ramais de água fria".to_owned()
        } else {
            "Padrão hidráulico (sem aparelhos sanitários)".to_owned()
        },
        edited: false,
    });

    items.push(MaterialEntry {
        id: "pipe_sewage".to_owned(),
        category: MaterialCategory::Pipe,
        name: "Tubo PVC série normal (esgoto sanitário)".to_owned(),
        standard: "ABNT NBR 5688 · NBR 8160".to_owned(),
        unit: "m".to_owned(),
        density_kg_m3: Some(1400.0),
        strength_mpa: None,
        waste_pct: 10.0,
        mix_cement_kg_m3: None,
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: None,
        mix_gravel_m3_per_m3: None,
        properties_note: "DN 40, DN 50, DN 75 e DN 100 · declividade mín. 1 % a 2 % · barras 6 m"
            .to_owned(),
        used_in_model: has_plumbing,
        usage_summary: "Ramais de descarga, esgoto e ventilação".to_owned(),
        edited: false,
    });

    // 8. Electrical copper wires & PVC conduits (detected from electrical design).
    let has_electrical = project.electrical_devices().next().is_some();
    let schedule = project.cached_load_schedule();
    let wire_sections: String = {
        let mut secs: Vec<u32> = schedule
            .circuits
            .iter()
            .filter_map(|c| c.wire_mm2.map(|w| (w * 10.0).round() as u32))
            .collect();
        secs.sort_unstable();
        secs.dedup();
        if secs.is_empty() {
            "1,5 a 10 mm²".to_owned()
        } else {
            secs.iter()
                .map(|s| {
                    let v = f64::from(*s) / 10.0;
                    let txt = if v.fract().abs() < 1e-6 {
                        format!("{v:.0}")
                    } else {
                        decimal(v, 1)
                    };
                    format!("{txt} mm²")
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    items.push(MaterialEntry {
        id: "wire_copper_pvc".to_owned(),
        category: MaterialCategory::Wire,
        name: "Cabo flexível de cobre isolação PVC 750 V (70 °C)".to_owned(),
        standard: "ABNT NBR NM 247-3 · NBR 5410".to_owned(),
        unit: "m".to_owned(),
        density_kg_m3: Some(8960.0),
        strength_mpa: None,
        waste_pct: 10.0,
        mix_cement_kg_m3: None,
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: None,
        mix_gravel_m3_per_m3: None,
        properties_note: format!(
            "ρ = 1/56 Ω·mm²/m · bitolas no projeto: {wire_sections} · rolos 100 m"
        ),
        used_in_model: has_electrical,
        usage_summary: if has_electrical {
            format!("{} circuito(s) elétrico(s)", schedule.circuits.len())
        } else {
            "Padrão elétrico (sem pontos lançados)".to_owned()
        },
        edited: false,
    });

    items.push(MaterialEntry {
        id: "conduit_pvc".to_owned(),
        category: MaterialCategory::Wire,
        name: "Eletroduto corrugado flexível PVC (anti-chama)".to_owned(),
        standard: "ABNT NBR 15465 · NBR 5410".to_owned(),
        unit: "m".to_owned(),
        density_kg_m3: Some(1400.0),
        strength_mpa: None,
        waste_pct: 10.0,
        mix_cement_kg_m3: None,
        mix_lime_kg_m3: None,
        mix_sand_m3_per_m3: None,
        mix_gravel_m3_per_m3: None,
        properties_note: "Ø20 a Ø50 mm · taxa de ocupação máx. 40 % (≥ 3 fios) · rolos 50 m"
            .to_owned(),
        used_in_model: !project.conduits.is_empty() || has_electrical,
        usage_summary: format!("{} trecho(s) de eletroduto", project.conduits.len()),
        edited: false,
    });

    // Apply persisted user overrides.
    for item in &mut items {
        if let Some(ov) = project.construction.material_overrides.get(&item.id) {
            apply_override(item, ov);
        }
    }

    let active_count = items.iter().filter(|m| m.used_in_model).count();
    let edited_count = items.iter().filter(|m| m.edited).count();
    MaterialsCatalogue {
        items,
        active_count,
        edited_count,
    }
}

/// Persists an edit to a material in the catalogue and propagates relevant
/// engineering properties back to the project's calculation settings.
pub fn edit_catalogue_material(
    project: &mut Project,
    id: &str,
    patch: MaterialOverride,
) -> Result<String, String> {
    let catalogue = detect_materials_catalogue(project);
    let existing = catalogue
        .find(id)
        .ok_or_else(|| format!("Material '{id}' não encontrado no catálogo."))?;

    if let Some(d) = patch.density_kg_m3
        && (!d.is_finite() || !(100.0..=15_000.0).contains(&d))
    {
        return Err("A densidade deve ficar entre 100 e 15000 kg/m³.".to_owned());
    }
    if let Some(s) = patch.strength_mpa
        && (!s.is_finite() || !(1.0..=1000.0).contains(&s))
    {
        return Err("A resistência deve ficar entre 1 e 1000 MPa.".to_owned());
    }
    if let Some(w) = patch.waste_pct
        && (!w.is_finite() || !(0.0..=50.0).contains(&w))
    {
        return Err("A perda deve ficar entre 0 e 50 %.".to_owned());
    }
    for (label, val) in [
        ("Cimento", patch.mix_cement_kg_m3),
        ("Cal", patch.mix_lime_kg_m3),
        ("Areia", patch.mix_sand_m3_per_m3),
        ("Brita", patch.mix_gravel_m3_per_m3),
    ] {
        if let Some(v) = val
            && (!v.is_finite() || !(0.0..=1500.0).contains(&v))
        {
            return Err(format!("{label} no traço deve ser um número válido ≥ 0."));
        }
    }

    // Propagate core engineering properties back to live calculations!
    if id == "concrete_structural"
        && let Some(fck) = patch.strength_mpa
    {
        if !(15.0..=50.0).contains(&fck) {
            return Err("O fck do concreto estrutural deve ficar entre 15 e 50 MPa.".to_owned());
        }
        project.structure.settings.fck_mpa = fck;
    }
    if id == "block_default" {
        if let Some(d) = patch.density_kg_m3 {
            let kn_m3 = (d * 9.81 / 1000.0).clamp(6.0, 24.0);
            project.structure.settings.masonry_kn_m3 = kn_m3;
        }
        if let Some(w) = patch.waste_pct {
            project.schedules.waste_percent = w;
        }
    }

    let entry = project
        .construction
        .material_overrides
        .entry(id.to_owned())
        .or_default();
    if let Some(v) = patch.name {
        entry.name = (!v.trim().is_empty()).then(|| v.trim().to_owned());
    }
    if let Some(v) = patch.density_kg_m3 {
        entry.density_kg_m3 = Some(v);
    }
    if let Some(v) = patch.strength_mpa {
        entry.strength_mpa = Some(v);
    }
    if let Some(v) = patch.waste_pct {
        entry.waste_pct = Some(v);
    }
    if let Some(v) = patch.mix_cement_kg_m3 {
        entry.mix_cement_kg_m3 = Some(v);
    }
    if let Some(v) = patch.mix_lime_kg_m3 {
        entry.mix_lime_kg_m3 = Some(v);
    }
    if let Some(v) = patch.mix_sand_m3_per_m3 {
        entry.mix_sand_m3_per_m3 = Some(v);
    }
    if let Some(v) = patch.mix_gravel_m3_per_m3 {
        entry.mix_gravel_m3_per_m3 = Some(v);
    }
    if entry.is_empty() {
        project.construction.material_overrides.remove(id);
    }

    Ok(format!(
        "Material '{}' atualizado no catálogo.",
        existing.name
    ))
}

/// Resets a material override back to the model's detected defaults.
pub fn reset_catalogue_material(project: &mut Project, id: &str) -> bool {
    project.construction.material_overrides.remove(id).is_some()
}

/// UTF-8 BOM CSV export of the materials catalogue.
pub fn materials_catalogue_csv(project: &Project) -> String {
    let cat = detect_materials_catalogue(project);
    let mut csv = String::from(
        "\u{feff}id,categoria,material,norma,unidade,densidade_kg_m3,resistencia_mpa,perda_pct,cimento_kg_m3,cal_kg_m3,areia_m3,brita_m3,em_uso,editado\r\n",
    );
    let opt = |v: Option<f64>| v.map_or(String::new(), |x| format!("{x:.2}"));
    for m in &cat.items {
        let _ = writeln!(
            csv,
            "{},{},\"{}\",\"{}\",{},{},{},{:.1},{},{},{},{},{},{}\r",
            m.id,
            m.category.key(),
            m.name.replace('"', "\"\""),
            m.standard.replace('"', "\"\""),
            m.unit,
            opt(m.density_kg_m3),
            opt(m.strength_mpa),
            m.waste_pct,
            opt(m.mix_cement_kg_m3),
            opt(m.mix_lime_kg_m3),
            opt(m.mix_sand_m3_per_m3),
            opt(m.mix_gravel_m3_per_m3),
            if m.used_in_model { "sim" } else { "nao" },
            if m.edited { "sim" } else { "nao" }
        );
    }
    csv
}

/// Printable A3 landscape SVG sheet of the materials catalogue (#135).
pub fn materials_catalogue_svg(project: &Project) -> String {
    let cat = detect_materials_catalogue(project);
    let width = 420.0;
    let height = (85.0 + cat.items.len() as f64 * 9.0 + 35.0).max(297.0);
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
        "CATÁLOGO DE MATERIAIS DA OBRA (DETECTADO DO MODELO)",
    );
    text(
        &mut svg,
        18.0,
        28.5,
        3.0,
        false,
        muted,
        &format!(
            "{} · {} materiais ({} em uso no modelo · {} editado(s))",
            project.name,
            cat.items.len(),
            cat.active_count,
            cat.edited_count
        ),
    );

    let mut y = 40.0;
    let _ = writeln!(
        svg,
        "<rect x=\"16\" y=\"{:.1}\" width=\"388\" height=\"7.0\" fill=\"#edf2f7\" stroke=\"{ink}\" stroke-width=\"0.3\"/>",
        y - 4.8
    );
    for (title, x) in [
        ("Categoria", 18.0),
        ("Material / Especificação", 54.0),
        ("Norma", 162.0),
        ("Densidade", 232.0),
        ("Resist.", 262.0),
        ("Perda", 288.0),
        ("Uso no modelo", 310.0),
    ] {
        text(&mut svg, x, y, 2.6, true, ink, title);
    }
    y += 7.5;

    for m in &cat.items {
        let use_color = if m.used_in_model { "#1b5e20" } else { muted };
        text(&mut svg, 18.0, y, 2.4, true, ink, m.category.label());
        text(&mut svg, 54.0, y, 2.5, true, ink, &m.name);
        text(&mut svg, 162.0, y, 2.2, false, muted, &m.standard);
        text(
            &mut svg,
            232.0,
            y,
            2.4,
            false,
            ink,
            &m.density_kg_m3
                .map_or("—".to_owned(), |d| format!("{d:.0} kg/m³")),
        );
        text(
            &mut svg,
            262.0,
            y,
            2.4,
            false,
            ink,
            &m.strength_mpa
                .map_or("—".to_owned(), |s| format!("{s:.0} MPa")),
        );
        text(
            &mut svg,
            288.0,
            y,
            2.4,
            false,
            ink,
            &format!("{:.0} %", m.waste_pct),
        );
        text(&mut svg, 310.0, y, 2.3, false, use_color, &m.usage_summary);
        text(
            &mut svg,
            54.0,
            y + 3.6,
            2.1,
            false,
            muted,
            &m.properties_note,
        );
        let _ = writeln!(
            svg,
            "<line x1=\"16\" y1=\"{:.1}\" x2=\"404\" y2=\"{:.1}\" stroke=\"#d0dae5\" stroke-width=\"0.2\"/>",
            y + 5.2,
            y + 5.2
        );
        y += 9.0;
    }

    svg.push_str("</svg>\n");
    svg
}

/// Persisted construction planning data attached to `Project::construction`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConstructionPlan {
    /// Editable material overrides keyed by catalogue material id (#135).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub material_overrides: BTreeMap<String, MaterialOverride>,
    /// Editable unit prices, BDI and SINAPI import settings (#87).
    #[serde(skip_serializing_if = "super::cost::CostSettings::is_default")]
    pub cost: super::cost::CostSettings,
    /// Crew size and calendar settings for the construction schedule (#137).
    #[serde(skip_serializing_if = "super::schedule::ScheduleSettings::is_default")]
    pub schedule: super::schedule::ScheduleSettings,
    /// Checked NR-18 site-safety item ids (#138).
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub checked_safety_items: BTreeSet<String>,
    /// Current elapsed workday used for planned-vs-actual comparison (#139).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub progress_reference_day: u32,
    /// Site progress diary entries with photos, notes and actual costs (#139).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub progress_log: Vec<super::progress::ProgressEntry>,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl ConstructionPlan {
    pub fn is_default(&self) -> bool {
        self.material_overrides.is_empty()
            && self.cost.is_default()
            && self.schedule.is_default()
            && self.checked_safety_items.is_empty()
            && self.progress_reference_day == 0
            && self.progress_log.is_empty()
    }
}

impl Project {
    /// Detects every material used in the project across all disciplines (#135).
    pub fn materials_catalogue(&self) -> MaterialsCatalogue {
        detect_materials_catalogue(self)
    }

    /// Edits one material in the catalogue and synchronizes engineering settings (#135).
    pub fn edit_catalogue_material(
        &mut self,
        id: &str,
        patch: MaterialOverride,
    ) -> Result<String, String> {
        edit_catalogue_material(self, id, patch)
    }

    /// Restores one material in the catalogue back to model defaults (#135).
    pub fn reset_catalogue_material(&mut self, id: &str) -> bool {
        reset_catalogue_material(self, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::samples::SampleProject;

    #[test]
    fn catalogue_detects_all_eight_categories_from_sample_house() {
        let mut project = SampleProject::CasaTerrea.build();
        project.generate_structure();
        project.generate_electrical_points();
        project.assign_circuits();

        let cat = project.materials_catalogue();
        for category in MaterialCategory::ALL {
            assert!(
                cat.items.iter().any(|m| m.category == category),
                "missing category {category:?}"
            );
        }
        assert!(cat.find("concrete_structural").unwrap().used_in_model);
        assert!(cat.find("steel_ca50").unwrap().used_in_model);
        assert!(cat.find("block_default").unwrap().used_in_model);
        assert!(cat.find("wire_copper_pvc").unwrap().used_in_model);
        assert!(cat.find("pipe_cold_water").unwrap().used_in_model);

        let csv = materials_catalogue_csv(&project);
        assert!(csv.starts_with("\u{feff}id,categoria"));
        assert!(csv.contains("concrete_structural"));
        let svg = materials_catalogue_svg(&project);
        assert!(svg.contains("CATÁLOGO DE MATERIAIS"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn editing_catalogue_material_persists_and_updates_calculations() {
        let mut project = SampleProject::CasaTerrea.build();
        project.generate_structure();
        assert!((project.structure.settings.fck_mpa - 20.0).abs() < 1e-6);

        project
            .edit_catalogue_material(
                "concrete_structural",
                MaterialOverride {
                    strength_mpa: Some(25.0),
                    waste_pct: Some(7.0),
                    ..MaterialOverride::default()
                },
            )
            .unwrap();
        assert!((project.structure.settings.fck_mpa - 25.0).abs() < 1e-6);
        let cat = project.materials_catalogue();
        let conc = cat.find("concrete_structural").unwrap();
        assert!(conc.edited);
        assert!((conc.strength_mpa.unwrap() - 25.0).abs() < 1e-6);
        assert!((conc.waste_pct - 7.0).abs() < 1e-6);

        // JSON round-trip preserves catalogue edits.
        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.construction, project.construction);
        assert!(
            loaded
                .materials_catalogue()
                .find("concrete_structural")
                .unwrap()
                .edited
        );

        assert!(project.reset_catalogue_material("concrete_structural"));
        assert!(
            !project
                .materials_catalogue()
                .find("concrete_structural")
                .unwrap()
                .edited
        );
    }
}
