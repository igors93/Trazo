//! Construction quantities (#56): consolidated takeoff of masonry (whole blocks
//! with waste, half-blocks for running bond, U-blocks for bond beams and lintels),
//! lintels and sills per opening, laying mortar, plaster/stucco, U-block grout
//! concrete, and per-room floor/wall/ceiling/baseboard/waterproofing finishes,
//! for either the active floor or all floors combined.
//!
//! Method (also in `docs/engineering.md`):
//!
//! - **Blocks** per wall: net area (axis length × the wall's mean height —
//!   the storey height unless the wall has its own, possibly sloped, top —
//!   less the openings) × blocks per m², with 1/m² = (block length + joint) ×
//!   (block height + joint), using each wall's own masonry.
//! - **Half blocks** for the running bond: one every other course at each
//!   wall end and at each jamb of an opening. Each half block stands in for
//!   half a whole one.
//! - **U-blocks** (*canaletas*): the top course of every wall (the bond beam,
//!   *cinta de amarração*), which they replace, plus the lintels and sills.
//! - **Lintels** (*vergas*) over every door and window and **sills**
//!   (*contravergas*) under every window, overlapping 30 cm into the wall on
//!   each side (usual practice), tagged with the opening's schedule code (`P1`, `J1`, …).
//! - **Laying mortar**: the joint fraction of the face times the block width,
//!   plus 10 % waste.
//! - **Plaster / stucco** (*reboco / emboço*) on both faces of the net area,
//!   at the masonry's plaster thickness, plus 10 % waste on the volume.
//! - **U-block grout / concrete fill**: ~55 % of the gross block section along
//!   the bond beam, lintels and sills, plus 10 % waste.
//! - **Finishes** per room: floor (+10 % waste), skirting, net walls (+10 %
//!   waste), ceiling, waterproofing and room finish specifications, from
//!   `room_finish_quantities`.
//!
//! Axis lengths count the corners twice, which the waste allowance covers.

use std::collections::HashMap;

use super::annotations::frame_types;
use super::masonry::Masonry;
use super::model::{OpeningKind, Project};
use super::schedules::ScheduleTable;

/// How far a lintel or sill runs into the wall on each side, m.
pub const LINTEL_BEARING_M: f64 = 0.30;
/// Waste on mortar, plaster and U-block grout.
pub const MORTAR_WASTE: f64 = 0.10;
/// Internal channel volume fraction of a U-block filled with concrete/grout.
pub const GROUT_FILL_FRACTION: f64 = 0.55;

/// One wall, measured.
#[derive(Clone, Debug, PartialEq)]
pub struct WallTakeoff {
    pub wall_id: u64,
    pub floor_name: String,
    pub masonry: String,
    pub length_m: f64,
    pub height_m: f64,
    pub gross_m2: f64,
    pub openings_m2: f64,
    pub net_m2: f64,
    /// Whole blocks, without waste.
    pub blocks: f64,
    pub half_blocks: f64,
    /// U-blocks of the top course (bond beam).
    pub u_blocks: f64,
    /// Laying mortar (net, without waste), m³.
    pub mortar_m3: f64,
    /// Plaster / stucco area on both faces, m².
    pub plaster_m2: f64,
    /// Plaster / stucco volume (net, without waste), m³.
    pub plaster_m3: f64,
    /// Bond-beam U-block channel fill concrete (net, without waste), m³.
    pub grout_m3: f64,
}

/// A lintel or sill for one opening.
#[derive(Clone, Debug, PartialEq)]
pub struct LintelTakeoff {
    pub opening_id: u64,
    pub wall_id: u64,
    pub floor_name: String,
    /// Opening schedule code (`P1`, `J1`, …).
    pub code: String,
    /// "Verga" or "Contraverga".
    pub kind: &'static str,
    pub opening: &'static str,
    pub opening_width_m: f64,
    pub bearing_each_side_m: f64,
    pub length_m: f64,
    /// U-blocks to cast it in (with the wall's block length).
    pub u_blocks: f64,
    /// Concrete/grout fill volume (net, without waste), m³.
    pub grout_m3: f64,
}

/// One room's finishes.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomTakeoff {
    pub floor_name: String,
    pub room: String,
    pub floor_m2: f64,
    pub floor_with_waste_m2: f64,
    pub skirting_m: f64,
    pub walls_m2: f64,
    pub walls_with_waste_m2: f64,
    pub ceiling_m2: f64,
    pub waterproofing_m2: f64,
    pub floor_spec: String,
    pub wall_spec: String,
    pub ceiling_spec: String,
    pub baseboard_spec: String,
    pub waterproofing: bool,
}

/// Totals for one kind of masonry.
#[derive(Clone, Debug, PartialEq)]
pub struct MasonryTotal {
    pub masonry: String,
    pub net_area_m2: f64,
    pub blocks: f64,
    pub blocks_with_waste: f64,
    pub half_blocks: f64,
    pub half_blocks_with_waste: f64,
    /// U-blocks for the top bond-beam course.
    pub bond_beam_u_blocks: f64,
    /// U-blocks for lintels and sills.
    pub lintel_u_blocks: f64,
    /// Total U-blocks (`bond_beam_u_blocks + lintel_u_blocks`), without waste.
    pub u_blocks: f64,
    /// Total U-blocks with block waste applied.
    pub u_blocks_with_waste: f64,
}

/// Summary of one floor inside a multi-floor takeoff.
#[derive(Clone, Debug, PartialEq)]
pub struct FloorTakeoffSummary {
    pub floor_name: String,
    pub net_wall_m2: f64,
    pub blocks_with_waste: f64,
    pub half_blocks_with_waste: f64,
    pub u_blocks_with_waste: f64,
    pub mortar_m3: f64,
    pub plaster_m2: f64,
    pub plaster_m3: f64,
    pub grout_m3: f64,
    pub lintel_count: usize,
    pub lintel_m: f64,
    pub floor_with_waste_m2: f64,
    pub room_walls_m2: f64,
    pub ceiling_m2: f64,
}

/// The whole takeoff.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConstructionTakeoff {
    pub all_floors: bool,
    pub walls: Vec<WallTakeoff>,
    pub lintels: Vec<LintelTakeoff>,
    pub rooms: Vec<RoomTakeoff>,
    pub masonry: Vec<MasonryTotal>,
    pub floors: Vec<FloorTakeoffSummary>,
    pub waste_percent: f64,
    pub net_wall_m2: f64,
    /// Total laying mortar including 10 % waste, m³.
    pub mortar_m3: f64,
    /// Total plaster/stucco area on both wall faces, m².
    pub plaster_m2: f64,
    /// Total plaster/stucco volume including 10 % waste, m³.
    pub plaster_m3: f64,
    /// Total U-block fill concrete/grout (bond beams + lintels/sills) including 10 % waste, m³.
    pub grout_m3: f64,
    pub lintel_m: f64,
    pub floor_m2: f64,
    pub floor_with_waste_m2: f64,
    pub skirting_m: f64,
    pub room_walls_m2: f64,
    pub room_walls_with_waste_m2: f64,
    pub ceiling_m2: f64,
    pub waterproofing_m2: f64,
}

fn course_height(masonry: &Masonry) -> f64 {
    (masonry.block_height + masonry.joint).max(0.01)
}

/// Courses along a height `h` (whole courses, nearest).
fn courses(masonry: &Masonry, h: f64) -> f64 {
    (h / course_height(masonry)).round().max(0.0)
}

/// Half blocks along one vertical edge `h` high: every other course.
fn half_blocks_on_edge(masonry: &Masonry, h: f64) -> f64 {
    (courses(masonry, h) / 2.0).ceil()
}

/// Laying mortar per m² of wall: the joint fraction of the face times the
/// block width.
pub fn mortar_per_m2(masonry: &Masonry) -> f64 {
    let face = masonry.blocks_per_square_metre() * masonry.block_length * masonry.block_height;
    (1.0 - face).max(0.0) * masonry.block_width
}

/// Internal grout/concrete fill volume per linear metre of a U-block channel.
pub fn grout_per_m(masonry: &Masonry) -> f64 {
    masonry.block_width * masonry.block_height * GROUT_FILL_FRACTION
}

fn opening_label(kind: OpeningKind) -> &'static str {
    match kind {
        OpeningKind::Door => "Porta",
        OpeningKind::Window => "Janela",
        OpeningKind::SaloonDoor => "Porta vaivém",
        OpeningKind::Gap => "Vão livre",
        OpeningKind::Gate => "Portão",
    }
}

fn opening_codes_map(project: &Project) -> HashMap<u64, String> {
    let mut map = HashMap::new();
    for frame in frame_types(project) {
        for id in frame.ids {
            map.insert(id, frame.code.clone());
        }
    }
    map
}

impl Project {
    fn takeoff_floor_snapshot(&self, level: &super::floors::FloorLevel) -> Project {
        let mut snap = Project::default();
        snap.wall_height = level.ceiling_height_m.max(0.5);
        snap.masonry = self.masonry.clone();
        snap.schedules = self.schedules.clone();
        snap.walls = level.data.walls.clone();
        snap.rooms = level.data.rooms.clone();
        snap.openings = level.data.openings.clone();
        snap.fixtures = level.data.fixtures.clone();
        snap
    }

    fn measure_single_floor(
        &self,
        floor_name: &str,
        waste_percent: f64,
    ) -> (
        Vec<WallTakeoff>,
        Vec<LintelTakeoff>,
        Vec<RoomTakeoff>,
        FloorTakeoffSummary,
    ) {
        let waste = 1.0 + waste_percent / 100.0;
        let storey = self.wall_height.max(0.5);
        let codes = opening_codes_map(self);
        let mut walls_out = Vec::new();
        let mut lintels_out = Vec::new();
        let mut rooms_out = Vec::new();

        for wall in &self.walls {
            let masonry = wall.masonry.clone().unwrap_or_else(|| self.masonry.clone());
            let length = wall.length();
            let openings: Vec<_> = self
                .openings
                .iter()
                .filter(|o| o.wall_id == wall.id)
                .collect();
            // Each wall's own top (#145): sloped tops average over the length.
            let (start_h, end_h) = wall.heights(storey);
            let height = wall.mean_height(storey);
            let opening_height = |o: &&super::model::Opening| {
                let top = wall.height_over(storey, o);
                o.height().unwrap_or(top).min(top)
            };
            let openings_m2: f64 = openings.iter().map(|o| o.width() * opening_height(o)).sum();
            let gross = length * height;
            let net = (gross - openings_m2).max(0.0);
            let equivalent = net * masonry.blocks_per_square_metre();
            let mut half =
                half_blocks_on_edge(&masonry, start_h) + half_blocks_on_edge(&masonry, end_h);
            for opening in &openings {
                half += 2.0 * half_blocks_on_edge(&masonry, opening_height(opening));
            }
            // The bond beam follows the slope of the top.
            let top_length = length.hypot(end_h - start_h);
            let u_blocks = (top_length / masonry.module() - 1e-9).ceil().max(0.0);
            let blocks = (equivalent - half / 2.0 - u_blocks).max(0.0);
            let plaster_m2 = 2.0 * net;
            let grout_rate = grout_per_m(&masonry);
            walls_out.push(WallTakeoff {
                wall_id: wall.id,
                floor_name: floor_name.to_owned(),
                masonry: masonry.name.clone(),
                length_m: length,
                height_m: height,
                gross_m2: gross,
                openings_m2,
                net_m2: net,
                blocks,
                half_blocks: half,
                u_blocks,
                mortar_m3: net * mortar_per_m2(&masonry),
                plaster_m2,
                plaster_m3: plaster_m2 * masonry.plaster,
                grout_m3: top_length * grout_rate,
            });
            for opening in &openings {
                let top = wall.height_over(storey, opening);
                let opening_top = opening.sill() + opening_height(opening);
                // A full-height free opening has no masonry over it. Every
                // opening with wall above it (including gates) needs a lintel.
                if opening_top >= top - course_height(&masonry) * 0.5 {
                    continue;
                }
                let opening_width_m = opening.width();
                let lintel_len = opening_width_m + 2.0 * LINTEL_BEARING_M;
                let u = (lintel_len / masonry.module() - 1e-9).ceil();
                let code = codes
                    .get(&opening.id)
                    .cloned()
                    .unwrap_or_else(|| format!("AB{}", opening.id));
                let mut kinds = vec!["Verga"];
                if opening.kind == OpeningKind::Window {
                    kinds.push("Contraverga");
                }
                for kind in kinds {
                    lintels_out.push(LintelTakeoff {
                        opening_id: opening.id,
                        wall_id: wall.id,
                        floor_name: floor_name.to_owned(),
                        code: code.clone(),
                        kind,
                        opening: opening_label(opening.kind),
                        opening_width_m,
                        bearing_each_side_m: LINTEL_BEARING_M,
                        length_m: lintel_len,
                        u_blocks: u,
                        grout_m3: lintel_len * grout_rate,
                    });
                }
            }
        }

        for room in &self.rooms {
            if let Some(q) = self.room_finish_quantities(room) {
                let fin = room.finishes.as_ref();
                rooms_out.push(RoomTakeoff {
                    floor_name: floor_name.to_owned(),
                    room: room.display_name().to_owned(),
                    floor_m2: q.floor_area_m2,
                    floor_with_waste_m2: q.floor_with_waste_m2,
                    skirting_m: q.baseboard_linear_m,
                    walls_m2: q.wall_net_area_m2,
                    walls_with_waste_m2: q.wall_with_waste_m2,
                    ceiling_m2: q.ceiling_area_m2,
                    waterproofing_m2: q.waterproofing_m2,
                    floor_spec: fin.map(|f| f.floor.trim().to_owned()).unwrap_or_default(),
                    wall_spec: fin.map(|f| f.wall.trim().to_owned()).unwrap_or_default(),
                    ceiling_spec: fin.map(|f| f.ceiling.trim().to_owned()).unwrap_or_default(),
                    baseboard_spec: fin
                        .map(|f| f.baseboard.trim().to_owned())
                        .unwrap_or_default(),
                    waterproofing: fin.is_some_and(|f| f.waterproofing),
                });
            }
        }

        let net_wall_m2: f64 = walls_out.iter().map(|w| w.net_m2).sum();
        let blocks_raw: f64 = walls_out.iter().map(|w| w.blocks).sum();
        let half_raw: f64 = walls_out.iter().map(|w| w.half_blocks).sum();
        let u_raw: f64 = walls_out.iter().map(|w| w.u_blocks).sum::<f64>()
            + lintels_out.iter().map(|l| l.u_blocks).sum::<f64>();
        let mortar_m3: f64 = walls_out
            .iter()
            .map(|w| w.mortar_m3 * (1.0 + MORTAR_WASTE))
            .sum();
        let plaster_m2: f64 = walls_out.iter().map(|w| w.plaster_m2).sum();
        let plaster_m3: f64 = walls_out
            .iter()
            .map(|w| w.plaster_m3 * (1.0 + MORTAR_WASTE))
            .sum();
        let grout_m3: f64 = (walls_out.iter().map(|w| w.grout_m3).sum::<f64>()
            + lintels_out.iter().map(|l| l.grout_m3).sum::<f64>())
            * (1.0 + MORTAR_WASTE);
        let lintel_m: f64 = lintels_out.iter().map(|l| l.length_m).sum();
        let floor_with_waste_m2: f64 = rooms_out.iter().map(|r| r.floor_with_waste_m2).sum();
        let room_walls_m2: f64 = rooms_out.iter().map(|r| r.walls_m2).sum();
        let ceiling_m2: f64 = rooms_out.iter().map(|r| r.ceiling_m2).sum();

        let summary = FloorTakeoffSummary {
            floor_name: floor_name.to_owned(),
            net_wall_m2,
            blocks_with_waste: (blocks_raw.ceil() * waste).ceil(),
            half_blocks_with_waste: (half_raw * waste).ceil(),
            u_blocks_with_waste: (u_raw * waste).ceil(),
            mortar_m3,
            plaster_m2,
            plaster_m3,
            grout_m3,
            lintel_count: lintels_out.len(),
            lintel_m,
            floor_with_waste_m2,
            room_walls_m2,
            ceiling_m2,
        };

        (walls_out, lintels_out, rooms_out, summary)
    }

    /// The consolidated construction takeoff of the active floor.
    pub fn construction_takeoff(&self) -> ConstructionTakeoff {
        self.construction_takeoff_with_options(false, None)
    }

    /// The consolidated construction takeoff across all floors of the project.
    pub fn construction_takeoff_all_floors(&self) -> ConstructionTakeoff {
        self.construction_takeoff_with_options(true, None)
    }

    /// Construction takeoff with explicit floor scope and optional waste % override.
    pub fn construction_takeoff_with_options(
        &self,
        all_floors: bool,
        waste_override: Option<f64>,
    ) -> ConstructionTakeoff {
        let waste_percent = waste_override
            .unwrap_or(self.schedules.waste_percent)
            .clamp(0.0, 50.0);
        let waste = 1.0 + waste_percent / 100.0;
        let mut out = ConstructionTakeoff {
            all_floors,
            waste_percent,
            ..ConstructionTakeoff::default()
        };

        let active_id = self.floors.active_floor_id;
        let active_name = self
            .active_floor()
            .map(|l| l.name.clone())
            .unwrap_or_else(|| "Térreo".to_owned());

        let has_multiple_floors = all_floors && self.floors.levels.len() > 1;
        if has_multiple_floors {
            let mut levels: Vec<_> = self.floors.levels.iter().collect();
            levels.sort_by(|a, b| a.elevation_m.total_cmp(&b.elevation_m));
            for level in levels {
                let (w, l, r, summary) = if Some(level.id) == active_id {
                    self.measure_single_floor(&level.name, waste_percent)
                } else {
                    let snap = self.takeoff_floor_snapshot(level);
                    snap.measure_single_floor(&level.name, waste_percent)
                };
                out.walls.extend(w);
                out.lintels.extend(l);
                out.rooms.extend(r);
                out.floors.push(summary);
            }
        } else {
            let (w, l, r, summary) = self.measure_single_floor(&active_name, waste_percent);
            out.walls = w;
            out.lintels = l;
            out.rooms = r;
            out.floors.push(summary);
        }

        // Totals, masonry grouped by kind.
        for wall in &out.walls {
            let lintel_u: f64 = out
                .lintels
                .iter()
                .filter(|l| l.wall_id == wall.wall_id && l.floor_name == wall.floor_name)
                .map(|l| l.u_blocks)
                .sum();
            match out.masonry.iter_mut().find(|m| m.masonry == wall.masonry) {
                Some(total) => {
                    total.net_area_m2 += wall.net_m2;
                    total.blocks += wall.blocks;
                    total.half_blocks += wall.half_blocks;
                    total.bond_beam_u_blocks += wall.u_blocks;
                    total.lintel_u_blocks += lintel_u;
                    total.u_blocks += wall.u_blocks + lintel_u;
                }
                None => out.masonry.push(MasonryTotal {
                    masonry: wall.masonry.clone(),
                    net_area_m2: wall.net_m2,
                    blocks: wall.blocks,
                    blocks_with_waste: 0.0,
                    half_blocks: wall.half_blocks,
                    half_blocks_with_waste: 0.0,
                    bond_beam_u_blocks: wall.u_blocks,
                    lintel_u_blocks: lintel_u,
                    u_blocks: wall.u_blocks + lintel_u,
                    u_blocks_with_waste: 0.0,
                }),
            }
            out.net_wall_m2 += wall.net_m2;
            out.mortar_m3 += wall.mortar_m3 * (1.0 + MORTAR_WASTE);
            out.plaster_m2 += wall.plaster_m2;
            out.plaster_m3 += wall.plaster_m3 * (1.0 + MORTAR_WASTE);
            out.grout_m3 += wall.grout_m3 * (1.0 + MORTAR_WASTE);
        }
        for lintel in &out.lintels {
            out.grout_m3 += lintel.grout_m3 * (1.0 + MORTAR_WASTE);
        }
        for total in &mut out.masonry {
            total.blocks = total.blocks.ceil();
            total.blocks_with_waste = (total.blocks * waste).ceil();
            total.half_blocks_with_waste = (total.half_blocks * waste).ceil();
            total.u_blocks_with_waste = (total.u_blocks * waste).ceil();
        }
        out.lintel_m = out.lintels.iter().map(|l| l.length_m).sum();
        for room in &out.rooms {
            out.floor_m2 += room.floor_m2;
            out.floor_with_waste_m2 += room.floor_with_waste_m2;
            out.skirting_m += room.skirting_m;
            out.room_walls_m2 += room.walls_m2;
            out.room_walls_with_waste_m2 += room.walls_with_waste_m2;
            out.ceiling_m2 += room.ceiling_m2;
            out.waterproofing_m2 += room.waterproofing_m2;
        }
        out
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

/// Structured tables for any [`ConstructionTakeoff`] (active floor or all floors).
pub fn takeoff_tables_for(t: &ConstructionTakeoff) -> Vec<ScheduleTable> {
    let multi = t.floors.len() > 1;
    let mut masonry_rows: Vec<Vec<String>> = Vec::new();
    for m in &t.masonry {
        masonry_rows.push(vec![
            format!("{} · blocos inteiros", m.masonry),
            format!("{:.0} un.", m.blocks_with_waste),
            format!(
                "{:.0} un. + {:.0} % de perda ({} m²)",
                m.blocks,
                t.waste_percent,
                decimal(m.net_area_m2, 2)
            ),
        ]);
        masonry_rows.push(vec![
            format!("{} · meios-blocos", m.masonry),
            format!("{:.0} un.", m.half_blocks_with_waste),
            format!(
                "{:.0} un. + {:.0} % (amarração nas pontas e nos vãos)",
                m.half_blocks, t.waste_percent
            ),
        ]);
        masonry_rows.push(vec![
            format!("{} · canaletas", m.masonry),
            format!("{:.0} un.", m.u_blocks_with_waste),
            format!(
                "{:.0} un. base ({:.0} cinta + {:.0} vergas/contrav.) + {:.0} %",
                m.u_blocks, m.bond_beam_u_blocks, m.lintel_u_blocks, t.waste_percent
            ),
        ]);
    }
    masonry_rows.extend([
        vec![
            "Área líquida de alvenaria".to_owned(),
            format!("{} m²", decimal(t.net_wall_m2, 2)),
            "Descontando os vãos".to_owned(),
        ],
        vec![
            "Argamassa de assentamento".to_owned(),
            format!("{} m³", decimal(t.mortar_m3, 3)),
            format!("+{:.0} % de perda", MORTAR_WASTE * 100.0),
        ],
        vec![
            "Reboco / estuco (duas faces)".to_owned(),
            format!(
                "{} m² · {} m³",
                decimal(t.plaster_m2, 2),
                decimal(t.plaster_m3, 3)
            ),
            format!("+{:.0} % de perda no volume", MORTAR_WASTE * 100.0),
        ],
        vec![
            "Graute / concreto nas canaletas".to_owned(),
            format!("{} m³", decimal(t.grout_m3, 3)),
            format!(
                "Cinta, vergas e contravergas (+{:.0} % de perda)",
                MORTAR_WASTE * 100.0
            ),
        ],
        vec![
            "Vergas e contravergas".to_owned(),
            format!("{} un. · {} m", t.lintels.len(), decimal(t.lintel_m, 2)),
            format!(
                "Transpasse de {:.0} cm de cada lado",
                LINTEL_BEARING_M * 100.0
            ),
        ],
    ]);

    let walls_rows: Vec<Vec<String>> = t
        .walls
        .iter()
        .map(|w| {
            let id_label = if multi {
                format!("P{} ({})", w.wall_id, w.floor_name)
            } else {
                format!("P{}", w.wall_id)
            };
            vec![
                id_label,
                w.masonry.clone(),
                format!("{} × {} m", decimal(w.length_m, 2), decimal(w.height_m, 2)),
                decimal(w.net_m2, 2),
                format!("{:.0}", w.blocks.ceil()),
                format!("{:.0}", w.half_blocks.ceil()),
                format!("{:.0}", w.u_blocks.ceil()),
                decimal(w.mortar_m3 * (1.0 + MORTAR_WASTE), 3),
                decimal(w.plaster_m2, 2),
            ]
        })
        .collect();

    let lintels: Vec<Vec<String>> = t
        .lintels
        .iter()
        .map(|l| {
            let code = if multi {
                format!("{} ({})", l.code, l.floor_name)
            } else {
                l.code.clone()
            };
            vec![
                code,
                format!("{} ({})", l.kind, l.opening.to_lowercase()),
                format!("{} m", decimal(l.opening_width_m, 2)),
                format!("{} m", decimal(l.length_m, 2)),
                format!("{:.0}", l.u_blocks),
                format!("{} m³", decimal(l.grout_m3 * (1.0 + MORTAR_WASTE), 3)),
            ]
        })
        .collect();

    let rooms: Vec<Vec<String>> = t
        .rooms
        .iter()
        .map(|r| {
            let name = if multi {
                format!("{} ({})", r.room, r.floor_name)
            } else {
                r.room.clone()
            };
            let specs = match (r.floor_spec.is_empty(), r.wall_spec.is_empty()) {
                (true, true) => "—".to_owned(),
                (false, true) => format!("Piso: {}", r.floor_spec),
                (true, false) => format!("Parede: {}", r.wall_spec),
                (false, false) => format!("Piso: {} · Parede: {}", r.floor_spec, r.wall_spec),
            };
            vec![
                name,
                decimal(r.floor_m2, 2),
                decimal(r.floor_with_waste_m2, 2),
                decimal(r.skirting_m, 2),
                decimal(r.walls_m2, 2),
                decimal(r.walls_with_waste_m2, 2),
                decimal(r.ceiling_m2, 2),
                if r.waterproofing_m2 > 1e-6 {
                    decimal(r.waterproofing_m2, 2)
                } else {
                    "—".to_owned()
                },
                specs,
            ]
        })
        .collect();

    let total_lintel_u: f64 = t.lintels.iter().map(|l| l.u_blocks).sum();
    let total_lintel_grout: f64 = t
        .lintels
        .iter()
        .map(|l| l.grout_m3 * (1.0 + MORTAR_WASTE))
        .sum();

    let mut tables = vec![
        ScheduleTable {
            title: "Quantitativos de alvenaria".to_owned(),
            headers: ["Item", "Quantidade", "Observação"]
                .iter()
                .map(|h| (*h).to_owned())
                .collect(),
            rows: masonry_rows,
            footer: None,
        },
        ScheduleTable {
            title: "Alvenaria por parede".to_owned(),
            headers: [
                "Parede",
                "Alvenaria",
                "L × H",
                "Líq. (m²)",
                "Blocos",
                "Meios",
                "Canaletas",
                "Argam. (m³)",
                "Reboco (m²)",
            ]
            .iter()
            .map(|h| (*h).to_owned())
            .collect(),
            rows: walls_rows,
            footer: Some(vec![
                "Total".to_owned(),
                format!("{} parede(s)", t.walls.len()),
                String::new(),
                decimal(t.net_wall_m2, 2),
                format!(
                    "{:.0}",
                    t.masonry.iter().map(|m| m.blocks).sum::<f64>().ceil()
                ),
                format!(
                    "{:.0}",
                    t.masonry.iter().map(|m| m.half_blocks).sum::<f64>().ceil()
                ),
                format!(
                    "{:.0}",
                    t.masonry
                        .iter()
                        .map(|m| m.bond_beam_u_blocks)
                        .sum::<f64>()
                        .ceil()
                ),
                decimal(t.mortar_m3, 3),
                decimal(t.plaster_m2, 2),
            ]),
        },
        ScheduleTable {
            title: "Vergas e contravergas".to_owned(),
            headers: [
                "Código",
                "Peça",
                "Vão",
                "Comprimento",
                "Canaletas",
                "Graute (+10%)",
            ]
            .iter()
            .map(|h| (*h).to_owned())
            .collect(),
            rows: lintels,
            footer: Some(vec![
                "Total".to_owned(),
                format!("{} peça(s)", t.lintels.len()),
                String::new(),
                format!("{} m", decimal(t.lintel_m, 2)),
                format!("{:.0}", total_lintel_u),
                format!("{} m³", decimal(total_lintel_grout, 3)),
            ]),
        },
        ScheduleTable {
            title: "Acabamentos por ambiente".to_owned(),
            headers: [
                "Ambiente",
                "Piso (m²)",
                "Piso +10% (m²)",
                "Rodapé (m)",
                "Paredes (m²)",
                "Paredes +10%",
                "Forro (m²)",
                "Imperm. (m²)",
                "Especificação",
            ]
            .iter()
            .map(|h| (*h).to_owned())
            .collect(),
            rows: rooms,
            footer: Some(vec![
                "Total".to_owned(),
                decimal(t.floor_m2, 2),
                decimal(t.floor_with_waste_m2, 2),
                decimal(t.skirting_m, 2),
                decimal(t.room_walls_m2, 2),
                decimal(t.room_walls_with_waste_m2, 2),
                decimal(t.ceiling_m2, 2),
                if t.waterproofing_m2 > 1e-6 {
                    decimal(t.waterproofing_m2, 2)
                } else {
                    "—".to_owned()
                },
                format!("{} ambiente(s)", t.rooms.len()),
            ]),
        },
    ];

    if multi {
        let floor_rows: Vec<Vec<String>> = t
            .floors
            .iter()
            .map(|f| {
                vec![
                    f.floor_name.clone(),
                    decimal(f.net_wall_m2, 2),
                    format!("{:.0}", f.blocks_with_waste),
                    format!("{:.0}", f.half_blocks_with_waste),
                    format!("{:.0}", f.u_blocks_with_waste),
                    decimal(f.mortar_m3, 3),
                    decimal(f.plaster_m2, 2),
                    decimal(f.floor_with_waste_m2, 2),
                ]
            })
            .collect();
        tables.push(ScheduleTable {
            title: "Resumo por pavimento".to_owned(),
            headers: [
                "Pavimento",
                "Alv. líq. (m²)",
                "Blocos c/ perda",
                "Meios c/ perda",
                "Canaletas c/ perda",
                "Argam. (m³)",
                "Reboco (m²)",
                "Piso +10% (m²)",
            ]
            .iter()
            .map(|h| (*h).to_owned())
            .collect(),
            rows: floor_rows,
            footer: Some(vec![
                "Total".to_owned(),
                decimal(t.net_wall_m2, 2),
                format!(
                    "{:.0}",
                    t.masonry.iter().map(|m| m.blocks_with_waste).sum::<f64>()
                ),
                format!(
                    "{:.0}",
                    t.masonry
                        .iter()
                        .map(|m| m.half_blocks_with_waste)
                        .sum::<f64>()
                ),
                format!(
                    "{:.0}",
                    t.masonry.iter().map(|m| m.u_blocks_with_waste).sum::<f64>()
                ),
                decimal(t.mortar_m3, 3),
                decimal(t.plaster_m2, 2),
                decimal(t.floor_with_waste_m2, 2),
            ]),
        });
    }

    tables
}

/// Compact sheet-ready *Quadro de Quantitativos de Obra* placed on exported
/// ABNT sheets (SVG/PDF) when `ScheduleConfig::show_quantities_schedule` is enabled.
pub fn build_quantities_schedule(project: &Project) -> ScheduleTable {
    let t = project.construction_takeoff();
    let mut rows = Vec::new();
    for m in &t.masonry {
        rows.push(vec![
            format!("{} · blocos (+{:.0}%)", m.masonry, t.waste_percent),
            format!("{:.0} un.", m.blocks_with_waste),
            format!(
                "Meios: {:.0} · Canaletas: {:.0}",
                m.half_blocks_with_waste, m.u_blocks_with_waste
            ),
        ]);
    }
    rows.push(vec![
        "Vergas e contravergas".to_owned(),
        format!("{} un. · {} m", t.lintels.len(), decimal(t.lintel_m, 2)),
        format!(
            "Transpasse {:.0} cm · graute {} m³",
            LINTEL_BEARING_M * 100.0,
            decimal(t.grout_m3, 3)
        ),
    ]);
    rows.push(vec![
        "Argamassa / Reboco (+10%)".to_owned(),
        format!(
            "{} / {} m³",
            decimal(t.mortar_m3, 3),
            decimal(t.plaster_m3, 3)
        ),
        format!("Reboco 2 faces: {} m²", decimal(t.plaster_m2, 2)),
    ]);
    if !t.rooms.is_empty() {
        rows.push(vec![
            "Acabamentos (piso +10% / paredes)".to_owned(),
            format!(
                "{} / {} m²",
                decimal(t.floor_with_waste_m2, 2),
                decimal(t.room_walls_m2, 2)
            ),
            format!(
                "Rodapé {} m · Forro {} m²",
                decimal(t.skirting_m, 1),
                decimal(t.ceiling_m2, 1)
            ),
        ]);
    }
    ScheduleTable {
        title: "Quadro de Quantitativos de Obra".to_owned(),
        headers: vec![
            "Item".to_owned(),
            "Quantidade".to_owned(),
            "Detalhe".to_owned(),
        ],
        rows,
        footer: None,
    }
}

/// Any [`ConstructionTakeoff`] as CSV (semicolon-separated, decimal comma).
pub fn takeoff_csv_for(t: &ConstructionTakeoff) -> String {
    let mut out = String::new();
    for table in takeoff_tables_for(t) {
        out.push_str(&table.title);
        out.push('\n');
        out.push_str(&table.headers.join(";"));
        out.push('\n');
        for row in &table.rows {
            out.push_str(&row.join(";"));
            out.push('\n');
        }
        if let Some(footer) = &table.footer {
            out.push_str(&footer.join(";"));
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

mod sheet;
pub use sheet::{takeoff_sheet_pages, takeoff_sheet_svg};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::finishes::RoomFinishPreset;
    use crate::domain::geometry::Point;

    /// A 4 × 3 m room (axes) with a 0.90 × 2.10 m door and a 1.20 × 1.00 m
    /// window, built with the default brick (9 × 19 × 14 cm, 1 cm joints,
    /// 1.5 cm plaster) and a 2.80 m storey.
    fn room() -> Project {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Quarto")
            .unwrap();
        let long = project
            .walls
            .iter()
            .find(|w| (w.length() - 4.0).abs() < 1e-6)
            .unwrap()
            .id;
        let short = project
            .walls
            .iter()
            .find(|w| (w.length() - 3.0).abs() < 1e-6)
            .unwrap()
            .id;
        project.add_opening(long, OpeningKind::Door, 0.5).unwrap();
        project
            .add_opening(short, OpeningKind::Window, 0.5)
            .unwrap();
        project
    }

    #[test]
    fn the_takeoff_matches_a_manual_takeoff_within_2_percent() {
        let project = room();
        let m = &project.masonry;
        assert_eq!(
            (m.block_length, m.block_height, m.joint),
            (0.19, 0.14, 0.01)
        );
        assert_eq!(project.wall_height, 2.8);
        let t = project.construction_takeoff();

        // Manual takeoff, worked out by hand:
        // perimeter on the axes 14 m, gross 14 × 2.8 = 39.2 m²;
        // openings 0.9 × 2.1 + 1.2 × 1.0 = 3.09 m², net 36.11 m².
        let net: f64 = 39.2 - 3.09;
        // 1 / (0.20 × 0.15) = 33.33 blocks/m² → 1203.67 equivalent blocks.
        let equivalent = net / (0.20 * 0.15);
        // 19 courses: 4 walls × 2 ends × 10 = 80 halves; door jambs
        // 2 × 7 (14 courses), window jambs 2 × 4 (7 courses) → 102 halves.
        let halves: f64 = 80.0 + 14.0 + 8.0;
        // Top course: 4/0.20 + 3/0.20 twice = 70 U-blocks.
        let top: f64 = 70.0;
        let whole = (equivalent - halves / 2.0 - top).ceil();
        // Lintels: door 0.9 + 0.6 = 1.5 m (8 U-blocks); window lintel and
        // sill 1.8 m each (9 U-blocks each).
        let lintel_m = 1.5 + 1.8 + 1.8;
        let u = top + 8.0 + 9.0 + 9.0;
        // Mortar: net × (1 − 33.33 × 0.19 × 0.14) × 0.09, + 10 %.
        let mortar = net * (1.0 - 0.19 * 0.14 / (0.20 * 0.15)) * 0.09 * 1.1;
        // Plaster: both faces, 1.5 cm, + 10 % on the volume.
        let plaster_m2 = 2.0 * net;
        let plaster_m3 = plaster_m2 * 0.015 * 1.1;
        // Grout concrete in bond beam (14 m) + lintels/sills (5.1 m), + 10%.
        let grout = (14.0 + lintel_m) * (0.09 * 0.14 * GROUT_FILL_FRACTION) * 1.1;

        let close = |a: f64, b: f64| (a - b).abs() <= 0.02 * b.abs();
        let total = &t.masonry[0];
        assert!(close(t.net_wall_m2, net), "{} vs {net}", t.net_wall_m2);
        assert!(close(total.blocks, whole), "{} vs {whole}", total.blocks);
        assert!(close(total.half_blocks, halves), "{}", total.half_blocks);
        assert!(close(total.bond_beam_u_blocks, top));
        assert!(close(total.lintel_u_blocks, 26.0));
        assert!(close(total.u_blocks, u), "{} vs {u}", total.u_blocks);
        assert!(close(total.u_blocks_with_waste, (u * 1.1).ceil()));
        assert!(close(total.blocks_with_waste, (whole * 1.1).ceil()));
        assert!(close(t.lintel_m, lintel_m), "{}", t.lintel_m);
        assert_eq!(t.lintels.len(), 3);
        assert!(
            t.lintels
                .iter()
                .any(|l| l.code == "P1" && l.kind == "Verga")
        );
        assert!(
            t.lintels
                .iter()
                .any(|l| l.code == "J1" && l.kind == "Contraverga")
        );
        assert!(close(t.mortar_m3, mortar), "{} vs {mortar}", t.mortar_m3);
        assert!(close(t.plaster_m2, plaster_m2));
        assert!(close(t.plaster_m3, plaster_m3));
        assert!(close(t.grout_m3, grout), "{} vs {grout}", t.grout_m3);
        // Finishes come from the room: floor & walls with 10 % waste.
        assert_eq!(t.rooms.len(), 1);
        assert!(close(t.floor_with_waste_m2, t.floor_m2 * 1.1));
        assert!(close(t.room_walls_with_waste_m2, t.room_walls_m2 * 1.1));
    }

    #[test]
    fn each_kind_of_masonry_is_totalled_on_its_own_and_exports() {
        let mut project = room();
        let wall = project.walls[0].id;
        let concrete = Masonry::presets()
            .into_iter()
            .find(|m| m.name.contains("concreto 14"))
            .unwrap();
        project
            .walls
            .iter_mut()
            .find(|w| w.id == wall)
            .unwrap()
            .masonry = Some(concrete);
        let t = project.construction_takeoff();
        assert_eq!(t.masonry.len(), 2);
        let csv = takeoff_csv_for(&t);
        assert!(csv.contains("Quantitativos de alvenaria"));
        assert!(csv.contains("Alvenaria por parede"));
        assert!(csv.contains("Vergas e contravergas"));
        assert!(csv.contains("Acabamentos por ambiente"));
        assert!(csv.contains("Bloco de concreto 14 · canaletas"));
    }

    #[test]
    fn multi_floor_takeoff_consolidates_all_floors_finishes_and_renders_sheet_svg() {
        let mut project = room();
        let room_id = project.rooms[0].id;
        project
            .apply_room_finish_preset(room_id, RoomFinishPreset::AreaMolhadaBanheiro)
            .unwrap();
        let ground_takeoff = project.construction_takeoff();
        assert!(ground_takeoff.waterproofing_m2 > ground_takeoff.floor_m2);
        assert!(ground_takeoff.rooms[0].floor_spec.contains("Porcelanato"));

        let ground = project.ensure_default_floor();
        project.add_floor("Superior", true, true);
        let upper_takeoff = project.construction_takeoff();
        project.switch_floor(ground).unwrap();

        let combined = project.construction_takeoff_all_floors();
        assert!(combined.all_floors);
        assert_eq!(combined.floors.len(), 2);
        assert!(
            (combined.net_wall_m2 - (ground_takeoff.net_wall_m2 + upper_takeoff.net_wall_m2)).abs()
                < 1e-6
        );
        assert!(
            (combined.mortar_m3 - (ground_takeoff.mortar_m3 + upper_takeoff.mortar_m3)).abs()
                < 1e-6
        );
        assert!(
            (combined.floor_with_waste_m2
                - (ground_takeoff.floor_with_waste_m2 + upper_takeoff.floor_with_waste_m2))
                .abs()
                < 1e-6
        );

        // Printable A3 SVG sheet renders cleanly and includes multi-floor summary.
        let svg = takeoff_sheet_svg(&project, true);
        assert!(svg.starts_with("<svg"));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains("Quantitativos de Obra"));
        assert!(svg.contains("Resumo por pavimento"));

        // Optional sheet schedule integrates into ABNT exported sheets.
        project.schedules.show_quantities_schedule = true;
        let sheet_svg = crate::io::export::sheet::scaled_sheet_svg(&project);
        assert!(sheet_svg.contains("Quadro de Quantitativos de Obra"));
    }
}
