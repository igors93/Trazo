//! ABNT NBR 6120:2019 — Actions for the calculation of building structures:
//! fundamental load cases (`G1`, `G2`, `Q`) and room-by-room live loads.

use crate::domain::{
    model::Project,
    structure::{CONCRETE_KN_M3, OccupancyClass, StructureReport, room_live_load_kn_m2},
};

/// Unit weight of structural / reinforcing steel, kN/m³ (NBR 6120:2019, Table 1).
pub const STEEL_KN_M3: f64 = 78.5;

/// Fundamental structural load cases used across building analysis and 2D frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoadCaseKind {
    /// Structural self-weight: reinforced concrete slabs, beams and columns.
    G1,
    /// Non-structural permanent dead loads: floor finishes/screed, masonry walls,
    /// roof dead weight and fixed concentrated loads (water tanks, equipment).
    G2,
    /// Characteristic variable occupancy / roof live loads (NBR 6120:2019).
    Q,
    /// Characteristic wind acting in the positive axis direction (`+X` or `+Y`, NBR 6123).
    WindPos,
    /// Characteristic wind acting in the negative axis direction (`−X` or `−Y`, NBR 6123).
    WindNeg,
}

impl LoadCaseKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::G1 => "G1",
            Self::G2 => "G2",
            Self::Q => "Q",
            Self::WindPos => "W+",
            Self::WindNeg => "W−",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::G1 => "Peso próprio estrutural (G1)",
            Self::G2 => "Permanente não estrutural (G2)",
            Self::Q => "Sobrecarga de utilização (Q)",
            Self::WindPos => "Vento característico (+)",
            Self::WindNeg => "Vento característico (−)",
        }
    }
}

/// NBR 6120:2019 Table 10 room-use classification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nbr6120RoomCategory {
    pub label: &'static str,
    pub table_ref: &'static str,
    pub min_live_kn_m2: f64,
}

/// Classifies a room name against ABNT NBR 6120:2019 Table 10 minimum live loads.
pub fn classify_room_nbr6120(room_name: &str, occupancy: OccupancyClass) -> Nbr6120RoomCategory {
    let lower = room_name.to_lowercase();
    if lower.contains("garagem") || lower.contains("estacionamento") {
        Nbr6120RoomCategory {
            label: "Garagem / estacionamento (veículos ≤ 25 kN)",
            table_ref: "NBR 6120:2019, Tabela 10",
            min_live_kn_m2: 3.0,
        }
    } else if lower.contains("escada") {
        Nbr6120RoomCategory {
            label: "Escada sem acesso ao público",
            table_ref: "NBR 6120:2019, Tabela 10",
            min_live_kn_m2: 2.5,
        }
    } else if lower.contains("varanda")
        || lower.contains("sacada")
        || lower.contains("terraço")
        || lower.contains("terraco")
    {
        Nbr6120RoomCategory {
            label: "Varanda / sacada / terraço",
            table_ref: "NBR 6120:2019, Tabela 10",
            min_live_kn_m2: 2.5,
        }
    } else if lower.contains("corredor")
        || lower.contains("circulação")
        || lower.contains("circulacao")
        || lower.contains("hall")
    {
        Nbr6120RoomCategory {
            label: "Corredor / circulação privativa",
            table_ref: "NBR 6120:2019, Tabela 10",
            min_live_kn_m2: 2.0,
        }
    } else if lower.contains("serviço")
        || lower.contains("servico")
        || lower.contains("lavanderia")
        || lower.contains("despensa")
    {
        Nbr6120RoomCategory {
            label: "Área de serviço / lavanderia / despensa",
            table_ref: "NBR 6120:2019, Tabela 10",
            min_live_kn_m2: 2.0,
        }
    } else {
        match occupancy {
            OccupancyClass::Residential => Nbr6120RoomCategory {
                label: "Residencial — sala / quarto / cozinha / banheiro",
                table_ref: "NBR 6120:2019, Tabela 10",
                min_live_kn_m2: 1.5,
            },
            OccupancyClass::Office => Nbr6120RoomCategory {
                label: "Escritório / uso administrativo",
                table_ref: "NBR 6120:2019, Tabela 10",
                min_live_kn_m2: 2.0,
            },
            OccupancyClass::Garage => Nbr6120RoomCategory {
                label: "Garagem / estacionamento leve",
                table_ref: "NBR 6120:2019, Tabela 10",
                min_live_kn_m2: 3.0,
            },
            OccupancyClass::Retail => Nbr6120RoomCategory {
                label: "Comércio / loja",
                table_ref: "NBR 6120:2019, Tabela 10",
                min_live_kn_m2: 4.0,
            },
        }
    }
}

/// Characteristic area loads of one room/slab per NBR 6120:2019.
#[derive(Clone, Debug, PartialEq)]
pub struct Nbr6120RoomLoad {
    pub room_name: String,
    pub area_m2: f64,
    pub category: &'static str,
    /// Structural slab self-weight `g1`, kN/m² (0 when the room has no slab).
    pub g1_kn_m2: f64,
    /// Floor finishes & screed `g2`, kN/m².
    pub g2_kn_m2: f64,
    /// NBR 6120 Table 10 minimum for this room name, kN/m².
    pub table_min_q_kn_m2: f64,
    /// Active live load used in calculation `q`, kN/m².
    pub active_q_kn_m2: f64,
}

/// Consolidated NBR 6120:2019 characteristic building load summary.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nbr6120Summary {
    pub occupancy: OccupancyClass,
    pub storeys: u32,
    pub concrete_kn_m3: f64,
    pub steel_kn_m3: f64,
    pub masonry_kn_m3: f64,
    pub finish_kn_m2: f64,
    pub base_live_kn_m2: f64,
    pub per_room_live_loads: bool,
    pub psi0_q: f64,
    pub psi1_q: f64,
    pub psi2_q: f64,
    pub psi0_w: f64,
    pub psi1_w: f64,
    pub psi2_w: f64,
    pub rooms: Vec<Nbr6120RoomLoad>,
    /// Total characteristic structural self-weight `ΣG1` (slabs + beams + columns), kN.
    pub total_g1_kn: f64,
    /// Total characteristic non-structural permanent load `ΣG2` (finishes + walls + roof dead + fixed concentrated loads), kN.
    pub total_g2_kn: f64,
    /// Total characteristic live load `ΣQ` (slabs live + roof live), kN.
    pub total_q_kn: f64,
    /// Note regarding NBR 6120:2019 §6.3 live-load reduction by storeys.
    pub live_reduction_note: String,
}

impl Nbr6120Summary {
    /// Total characteristic permanent load `ΣG = ΣG1 + ΣG2`, kN.
    pub fn total_g_kn(&self) -> f64 {
        self.total_g1_kn + self.total_g2_kn
    }

    /// Total characteristic gravity load `Σ(G1 + G2 + Q)`, kN.
    pub fn total_gravity_kn(&self) -> f64 {
        self.total_g1_kn + self.total_g2_kn + self.total_q_kn
    }
}

/// Computes the consolidated NBR 6120:2019 characteristic load breakdown (`G1`, `G2`, `Q`)
/// for the project's active structure (including multi-storey multiplier or real floors).
pub fn analyze_nbr6120(project: &Project, report: &StructureReport) -> Nbr6120Summary {
    let settings = project.structure.settings;
    let storeys = crate::domain::limits::storeys(project).max(1);
    let active_id = project.floors.active_floor_id;
    let has_real_upper = project.floors.levels.iter().any(|level| {
        Some(level.id) != active_id
            && (!level.data.beams.is_empty()
                || !level.data.slabs.is_empty()
                || level
                    .data
                    .fixtures
                    .iter()
                    .any(|f| f.kind == crate::domain::fixtures::FixtureKind::Column))
    });
    let floor_mult = if has_real_upper {
        1.0
    } else {
        f64::from(settings.floors.max(1))
    };

    let mut rooms = Vec::new();
    let mut slab_g1_kn = 0.0;
    let mut slab_g2_kn = 0.0;
    let mut slab_q_kn = 0.0;

    for room in &project.rooms {
        let area = project.room_area(room).unwrap_or(0.0).max(0.0);
        let cat = classify_room_nbr6120(room.display_name(), settings.occupancy);
        let slab = project
            .structure
            .slabs
            .iter()
            .find(|s| s.room_id == room.id);
        let g1 = slab.map_or(0.0, |s| s.kind.self_weight(s.thickness_m));
        let g2 = if slab.is_some() {
            settings.finish_kn_m2
        } else {
            0.0
        };
        let active_q = if settings.per_room_live_loads {
            room_live_load_kn_m2(room.display_name(), settings.live_kn_m2)
        } else {
            settings.live_kn_m2
        };
        if slab.is_some() {
            slab_g1_kn += g1 * area;
            slab_g2_kn += g2 * area;
            slab_q_kn += active_q * area;
        }
        rooms.push(Nbr6120RoomLoad {
            room_name: room.display_name().to_owned(),
            area_m2: area,
            category: cat.label,
            g1_kn_m2: g1,
            g2_kn_m2: g2,
            table_min_q_kn_m2: cat.min_live_kn_m2,
            active_q_kn_m2: active_q,
        });
    }

    // Structural self-weight of beams and columns on this floor.
    let beams_g1_kn: f64 = report
        .beams
        .iter()
        .flat_map(|line| line.spans.iter())
        .map(|s| CONCRETE_KN_M3 * s.width_m * s.height_m * s.length_m)
        .sum();
    let columns_g1_kn: f64 = report
        .columns
        .iter()
        .map(|c| CONCRETE_KN_M3 * c.width_m * c.depth_m * c.height_m)
        .sum();

    // Masonry walls on this floor.
    let walls_g2_kn: f64 = project
        .walls
        .iter()
        .map(|w| settings.masonry_kn_m3 * w.thickness() * project.wall_net_area(w))
        .sum();

    // Fixed concentrated loads (water tanks / heavy equipment) treated as G2.
    let concentrated_g2_kn: f64 = report
        .concentrated_loads
        .iter()
        .filter(|c| c.supported)
        .map(|c| c.load_kn)
        .sum();

    // Timber roofs (never multiplied by storeys).
    let mut roof_g2_kn = 0.0;
    let mut roof_q_kn = 0.0;
    for roof in &report.roofs {
        if let Some(model) = project.structure.roofs.iter().find(|r| r.id == roof.id) {
            let area =
                ((model.max.x - model.min.x).abs() * (model.max.y - model.min.y).abs()).max(0.0);
            roof_g2_kn += roof.dead_kn_m2 * area;
            roof_q_kn += roof.live_kn_m2 * area;
        }
    }

    // Real upper floors (when present) add their exact G1/G2/Q totals.
    let mut upper_g1_kn = 0.0;
    let mut upper_g2_kn = 0.0;
    let mut upper_q_kn = 0.0;
    if has_real_upper {
        for level in &project.floors.levels {
            if Some(level.id) == active_id {
                continue;
            }
            let h_level = level.ceiling_height_m.max(2.0);
            for slab in &level.data.slabs {
                if let Some(room) = level.data.rooms.iter().find(|r| r.id == slab.room_id) {
                    // Build lightweight polygon area via snapshot helper.
                    let area = room_area_in_walls(&level.data.walls, room);
                    upper_g1_kn += slab.kind.self_weight(slab.thickness_m) * area;
                    upper_g2_kn += settings.finish_kn_m2 * area;
                    let q = if settings.per_room_live_loads {
                        room_live_load_kn_m2(room.display_name(), settings.live_kn_m2)
                    } else {
                        settings.live_kn_m2
                    };
                    upper_q_kn += q * area;
                }
            }
            for wall in &level.data.walls {
                let (h0, h1) = wall.heights(h_level);
                let mean_h = 0.5 * (h0 + h1);
                upper_g2_kn += settings.masonry_kn_m3 * wall.thickness() * wall.length() * mean_h;
            }
            for fixture in &level.data.fixtures {
                match fixture.kind {
                    crate::domain::fixtures::FixtureKind::Column => {
                        upper_g1_kn += CONCRETE_KN_M3 * fixture.width * fixture.depth * h_level;
                    }
                    crate::domain::fixtures::FixtureKind::WaterTank
                    | crate::domain::fixtures::FixtureKind::HeavyEquipment => {
                        upper_g2_kn += fixture.structural_load_kn.unwrap_or(0.0).max(0.0);
                    }
                    _ => {}
                }
            }
            for beam in &level.data.beams {
                let p0 = level
                    .data
                    .fixtures
                    .iter()
                    .find(|f| f.id == beam.from)
                    .map(|f| f.center);
                let p1 = level
                    .data
                    .fixtures
                    .iter()
                    .find(|f| f.id == beam.to)
                    .map(|f| f.center);
                if let (Some(a), Some(b)) = (p0, p1) {
                    upper_g1_kn += CONCRETE_KN_M3 * beam.width_m * beam.height_m * a.distance(b);
                }
            }
        }
    }

    let total_g1_kn = (slab_g1_kn + beams_g1_kn + columns_g1_kn) * floor_mult + upper_g1_kn;
    let total_g2_kn =
        (slab_g2_kn + walls_g2_kn) * floor_mult + concentrated_g2_kn + roof_g2_kn + upper_g2_kn;
    let total_q_kn = slab_q_kn * floor_mult + roof_q_kn + upper_q_kn;

    let live_reduction_note = if storeys <= 3 {
        format!(
            "Sem redução de sobrecarga por pavimentos ({storeys} pav. ≤ 3; NBR 6120:2019 §6.3 só reduz a partir de 4 pavimentos)."
        )
    } else {
        "Acima de 3 pavimentos: projeto fora dos limites de pré-dimensionamento do Trazo (#102)."
            .to_owned()
    };

    Nbr6120Summary {
        occupancy: settings.occupancy,
        storeys,
        concrete_kn_m3: CONCRETE_KN_M3,
        steel_kn_m3: STEEL_KN_M3,
        masonry_kn_m3: settings.masonry_kn_m3,
        finish_kn_m2: settings.finish_kn_m2,
        base_live_kn_m2: settings.live_kn_m2,
        per_room_live_loads: settings.per_room_live_loads,
        psi0_q: settings.occupancy.psi0(),
        psi1_q: settings.occupancy.psi1(),
        psi2_q: settings.occupancy.psi2(),
        psi0_w: super::nbr8681::PSI0_WIND,
        psi1_w: super::nbr8681::PSI1_WIND,
        psi2_w: super::nbr8681::PSI2_WIND,
        rooms,
        total_g1_kn,
        total_g2_kn,
        total_q_kn,
        live_reduction_note,
    }
}

/// Estimates room area from its enclosing walls on an inactive upper floor.
fn room_area_in_walls(
    walls: &[crate::domain::model::Wall],
    room: &crate::domain::model::Room,
) -> f64 {
    let mut pts = Vec::new();
    for wid in &room.wall_ids {
        if let Some(w) = walls.iter().find(|w| w.id == *wid) {
            pts.push(w.start);
            pts.push(w.end);
        }
    }
    let Some(first) = pts.first().copied() else {
        return 0.0;
    };
    let (lo, hi) = pts.into_iter().fold((first, first), |(lo, hi), p| {
        (
            crate::domain::geometry::Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            crate::domain::geometry::Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    });
    ((hi.x - lo.x) * (hi.y - lo.y)).max(0.0)
}
