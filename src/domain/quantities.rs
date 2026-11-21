//! Construction quantities (#56): one consolidated takeoff of the masonry,
//! the lintels and the finishes, wall by wall and room by room, ready for
//! the shopping list.
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
//!   *cinta de amarração*), which they replace, plus the lintels.
//! - **Lintels** (*vergas*) over every door and window and **sills**
//!   (*contravergas*) under every window, overlapping 30 cm into the wall on
//!   each side (usual practice).
//! - **Laying mortar**: the joint fraction of the face times the block width.
//! - **Plaster** on both faces of the net area, at the masonry's plaster
//!   thickness.
//! - **Finishes** per room: floor (+10 % waste), skirting, walls and ceiling,
//!   from `room_finish_quantities`.
//!
//! Axis lengths count the corners twice, which the waste allowance covers.
//! Only the active floor is measured.

use super::masonry::Masonry;
use super::model::{OpeningKind, Project};
use super::schedules::ScheduleTable;

/// How far a lintel or sill runs into the wall on each side, m.
pub const LINTEL_BEARING_M: f64 = 0.30;
/// Waste on mortar and plaster.
pub const MORTAR_WASTE: f64 = 0.10;

/// One wall, measured.
#[derive(Clone, Debug, PartialEq)]
pub struct WallTakeoff {
    pub wall_id: u64,
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
    pub mortar_m3: f64,
    /// Plaster area on both faces.
    pub plaster_m2: f64,
    pub plaster_m3: f64,
}

/// A lintel or sill.
#[derive(Clone, Debug, PartialEq)]
pub struct LintelTakeoff {
    pub opening_id: u64,
    /// "Verga" or "Contraverga".
    pub kind: &'static str,
    pub opening: &'static str,
    pub length_m: f64,
    /// U-blocks to cast it in (with the wall's block length).
    pub u_blocks: f64,
}

/// One room's finishes.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomTakeoff {
    pub room: String,
    pub floor_m2: f64,
    pub floor_with_waste_m2: f64,
    pub skirting_m: f64,
    pub walls_m2: f64,
    pub ceiling_m2: f64,
}

/// Totals for one kind of masonry.
#[derive(Clone, Debug, PartialEq)]
pub struct MasonryTotal {
    pub masonry: String,
    pub blocks: f64,
    pub blocks_with_waste: f64,
    pub half_blocks: f64,
    pub half_blocks_with_waste: f64,
    pub u_blocks: f64,
}

/// The whole takeoff.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConstructionTakeoff {
    pub walls: Vec<WallTakeoff>,
    pub lintels: Vec<LintelTakeoff>,
    pub rooms: Vec<RoomTakeoff>,
    pub masonry: Vec<MasonryTotal>,
    pub waste_percent: f64,
    pub net_wall_m2: f64,
    pub mortar_m3: f64,
    pub plaster_m2: f64,
    pub plaster_m3: f64,
    pub lintel_m: f64,
    pub floor_m2: f64,
    pub floor_with_waste_m2: f64,
    pub skirting_m: f64,
    pub room_walls_m2: f64,
    pub ceiling_m2: f64,
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

fn opening_label(kind: OpeningKind) -> &'static str {
    match kind {
        OpeningKind::Door => "Porta",
        OpeningKind::Window => "Janela",
        OpeningKind::SaloonDoor => "Porta vaivém",
        OpeningKind::Gap => "Vão livre",
        OpeningKind::Gate => "Portão",
    }
}

impl Project {
    /// The consolidated construction takeoff of the active floor.
    pub fn construction_takeoff(&self) -> ConstructionTakeoff {
        let waste_percent = self.schedules.waste_percent.clamp(0.0, 50.0);
        let waste = 1.0 + waste_percent / 100.0;
        let storey = self.wall_height.max(0.5);
        let mut out = ConstructionTakeoff {
            waste_percent,
            ..ConstructionTakeoff::default()
        };
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
            out.walls.push(WallTakeoff {
                wall_id: wall.id,
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
            });
            for opening in &openings {
                if !matches!(
                    opening.kind,
                    OpeningKind::Door | OpeningKind::Window | OpeningKind::SaloonDoor
                ) {
                    continue;
                }
                let length = opening.width() + 2.0 * LINTEL_BEARING_M;
                let u = (length / masonry.module() - 1e-9).ceil();
                let mut kinds = vec!["Verga"];
                if opening.kind == OpeningKind::Window {
                    kinds.push("Contraverga");
                }
                for kind in kinds {
                    out.lintels.push(LintelTakeoff {
                        opening_id: opening.id,
                        kind,
                        opening: opening_label(opening.kind),
                        length_m: length,
                        u_blocks: u,
                    });
                }
            }
        }
        for room in &self.rooms {
            if let Some(q) = self.room_finish_quantities(room) {
                out.rooms.push(RoomTakeoff {
                    room: room.display_name().to_owned(),
                    floor_m2: q.floor_area_m2,
                    floor_with_waste_m2: q.floor_with_waste_m2,
                    skirting_m: q.baseboard_linear_m,
                    walls_m2: q.wall_net_area_m2,
                    ceiling_m2: q.ceiling_area_m2,
                });
            }
        }
        // Totals, masonry by kind.
        for wall in &out.walls {
            let lintel_u: f64 = out
                .lintels
                .iter()
                .filter(|l| {
                    self.openings
                        .iter()
                        .any(|o| o.id == l.opening_id && o.wall_id == wall.wall_id)
                })
                .map(|l| l.u_blocks)
                .sum();
            match out.masonry.iter_mut().find(|m| m.masonry == wall.masonry) {
                Some(total) => {
                    total.blocks += wall.blocks;
                    total.half_blocks += wall.half_blocks;
                    total.u_blocks += wall.u_blocks + lintel_u;
                }
                None => out.masonry.push(MasonryTotal {
                    masonry: wall.masonry.clone(),
                    blocks: wall.blocks,
                    blocks_with_waste: 0.0,
                    half_blocks: wall.half_blocks,
                    half_blocks_with_waste: 0.0,
                    u_blocks: wall.u_blocks + lintel_u,
                }),
            }
            out.net_wall_m2 += wall.net_m2;
            out.mortar_m3 += wall.mortar_m3 * (1.0 + MORTAR_WASTE);
            out.plaster_m2 += wall.plaster_m2;
            out.plaster_m3 += wall.plaster_m3 * (1.0 + MORTAR_WASTE);
        }
        for total in &mut out.masonry {
            total.blocks = total.blocks.ceil();
            total.blocks_with_waste = (total.blocks * waste).ceil();
            total.half_blocks_with_waste = (total.half_blocks * waste).ceil();
        }
        out.lintel_m = out.lintels.iter().map(|l| l.length_m).sum();
        for room in &out.rooms {
            out.floor_m2 += room.floor_m2;
            out.floor_with_waste_m2 += room.floor_with_waste_m2;
            out.skirting_m += room.skirting_m;
            out.room_walls_m2 += room.walls_m2;
            out.ceiling_m2 += room.ceiling_m2;
        }
        out
    }
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// The takeoff as tables: the masonry summary, lintels and finishes.
pub fn takeoff_tables(project: &Project) -> Vec<ScheduleTable> {
    let t = project.construction_takeoff();
    let mut masonry_rows: Vec<Vec<String>> = Vec::new();
    for m in &t.masonry {
        masonry_rows.push(vec![
            format!("{} · blocos inteiros", m.masonry),
            format!("{:.0} un.", m.blocks_with_waste),
            format!("{:.0} un. + {:.0} % de perda", m.blocks, t.waste_percent),
        ]);
        masonry_rows.push(vec![
            format!("{} · meios-blocos", m.masonry),
            format!("{:.0} un.", m.half_blocks_with_waste),
            "Amarração nas pontas e nos vãos".to_owned(),
        ]);
        masonry_rows.push(vec![
            format!("{} · canaletas", m.masonry),
            format!("{:.0} un.", m.u_blocks),
            "Cinta de amarração, vergas e contravergas".to_owned(),
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
            "Reboco (duas faces)".to_owned(),
            format!(
                "{} m² · {} m³",
                decimal(t.plaster_m2, 2),
                decimal(t.plaster_m3, 3)
            ),
            format!("+{:.0} % de perda no volume", MORTAR_WASTE * 100.0),
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
    let rooms: Vec<Vec<String>> = t
        .rooms
        .iter()
        .map(|r| {
            vec![
                r.room.clone(),
                decimal(r.floor_with_waste_m2, 2),
                decimal(r.skirting_m, 2),
                decimal(r.walls_m2, 2),
                decimal(r.ceiling_m2, 2),
            ]
        })
        .collect();
    let lintels: Vec<Vec<String>> = t
        .lintels
        .iter()
        .map(|l| {
            vec![
                format!("{} ({})", l.kind, l.opening.to_lowercase()),
                format!("{} m", decimal(l.length_m, 2)),
                format!("{:.0}", l.u_blocks),
            ]
        })
        .collect();
    vec![
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
            title: "Vergas e contravergas".to_owned(),
            headers: ["Peça", "Comprimento", "Canaletas"]
                .iter()
                .map(|h| (*h).to_owned())
                .collect(),
            rows: lintels,
            footer: Some(vec![
                "Total".to_owned(),
                format!("{} m", decimal(t.lintel_m, 2)),
                String::new(),
            ]),
        },
        ScheduleTable {
            title: "Acabamentos por ambiente".to_owned(),
            headers: [
                "Ambiente",
                "Piso c/ perda (m²)",
                "Rodapé (m)",
                "Paredes (m²)",
                "Forro (m²)",
            ]
            .iter()
            .map(|h| (*h).to_owned())
            .collect(),
            rows: rooms,
            footer: Some(vec![
                "Total".to_owned(),
                decimal(t.floor_with_waste_m2, 2),
                decimal(t.skirting_m, 2),
                decimal(t.room_walls_m2, 2),
                decimal(t.ceiling_m2, 2),
            ]),
        },
    ]
}

/// The takeoff tables as CSV (semicolon-separated, decimal comma).
pub fn takeoff_csv(project: &Project) -> String {
    let mut out = String::new();
    for table in takeoff_tables(project) {
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

#[cfg(test)]
mod tests {
    use super::*;
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

        let close = |a: f64, b: f64| (a - b).abs() <= 0.02 * b.abs();
        let total = &t.masonry[0];
        assert!(close(t.net_wall_m2, net), "{} vs {net}", t.net_wall_m2);
        assert!(close(total.blocks, whole), "{} vs {whole}", total.blocks);
        assert!(close(total.half_blocks, halves), "{}", total.half_blocks);
        assert!(close(total.u_blocks, u), "{} vs {u}", total.u_blocks);
        assert!(close(total.blocks_with_waste, (whole * 1.1).ceil()));
        assert!(close(t.lintel_m, lintel_m), "{}", t.lintel_m);
        assert_eq!(t.lintels.len(), 3);
        assert!(close(t.mortar_m3, mortar), "{} vs {mortar}", t.mortar_m3);
        assert!(close(t.plaster_m2, plaster_m2));
        assert!(close(t.plaster_m3, plaster_m3));
        // Finishes come from the room: floor with 10 % waste.
        assert_eq!(t.rooms.len(), 1);
        assert!(close(t.floor_with_waste_m2, t.floor_m2 * 1.1));
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
        let csv = takeoff_csv(&project);
        assert!(csv.contains("Quantitativos de alvenaria"));
        assert!(csv.contains("Vergas e contravergas"));
        assert!(csv.contains("Acabamentos por ambiente"));
        assert!(csv.contains("Bloco de concreto 14 · canaletas"));
    }
}
