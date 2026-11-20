//! Room finishes, wall/floor/ceiling coverings, baseboards (rodapés) and
//! per-room architectural specifications with automatic material quantities.

use serde::{Deserialize, Serialize};

use super::{
    geometry::polygon_perimeter,
    hatch::{HatchPattern, HatchStyle},
    model::{OpeningKind, Project, Room},
    schedules::ScheduleTable,
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomFinishes {
    pub floor: String,
    pub wall: String,
    pub ceiling: String,
    pub baseboard: String,
    #[serde(default = "default_baseboard_cm")]
    pub baseboard_height_cm: f64,
    #[serde(default)]
    pub waterproofing: bool,
    pub notes: String,
}

fn default_baseboard_cm() -> f64 {
    10.0
}

impl RoomFinishes {
    pub fn is_empty(&self) -> bool {
        self.floor.trim().is_empty()
            && self.wall.trim().is_empty()
            && self.ceiling.trim().is_empty()
            && self.baseboard.trim().is_empty()
            && self.notes.trim().is_empty()
            && !self.waterproofing
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomFinishPreset {
    SocialPorcelanato,
    DormitorioMadeira,
    AreaMolhadaBanheiro,
    CozinhaAreaServico,
    AreaExternaVaranda,
}

impl RoomFinishPreset {
    pub const ALL: [RoomFinishPreset; 5] = [
        RoomFinishPreset::SocialPorcelanato,
        RoomFinishPreset::DormitorioMadeira,
        RoomFinishPreset::AreaMolhadaBanheiro,
        RoomFinishPreset::CozinhaAreaServico,
        RoomFinishPreset::AreaExternaVaranda,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SocialPorcelanato => "Área social (Porcelanato 60×60)",
            Self::DormitorioMadeira => "Dormitório (Piso vinílico / madeira)",
            Self::AreaMolhadaBanheiro => "Banheiro (Área molhada)",
            Self::CozinhaAreaServico => "Cozinha / Área de serviço",
            Self::AreaExternaVaranda => "Varanda / Área externa",
        }
    }

    pub fn build(self) -> (RoomFinishes, HatchStyle) {
        match self {
            Self::SocialPorcelanato => (
                RoomFinishes {
                    floor: "Porcelanato acetinado 60×60 cm".to_owned(),
                    wall: "Pintura acrílica fosca sobre massa corrida".to_owned(),
                    ceiling: "Forro de gesso acartonado com pintura PVA".to_owned(),
                    baseboard: "Poliestireno branco".to_owned(),
                    baseboard_height_cm: 10.0,
                    waterproofing: false,
                    notes: "Argamassa AC-II, junta 2 mm".to_owned(),
                },
                HatchStyle::for_pattern(HatchPattern::StoneGrid),
            ),
            Self::DormitorioMadeira => (
                RoomFinishes {
                    floor: "Piso vinílico / laminado amadeirado".to_owned(),
                    wall: "Pintura acrílica fosca".to_owned(),
                    ceiling: "Forro de gesso liso".to_owned(),
                    baseboard: "MDF / Poliestireno branco".to_owned(),
                    baseboard_height_cm: 10.0,
                    waterproofing: false,
                    notes: "Contrapiso autonivelante".to_owned(),
                },
                HatchStyle::for_pattern(HatchPattern::WoodPlank),
            ),
            Self::AreaMolhadaBanheiro => (
                RoomFinishes {
                    floor: "Porcelanato antiderrapante 60×60 cm".to_owned(),
                    wall: "Revestimento cerâmico até o teto".to_owned(),
                    ceiling: "Forro de gesso resistente à umidade (RU)".to_owned(),
                    baseboard: "Sem rodapé (parede azulejada)".to_owned(),
                    baseboard_height_cm: 0.0,
                    waterproofing: true,
                    notes: "Impermeabilização piso + h=1,80 m no box; rejunte epóxi".to_owned(),
                },
                HatchStyle::for_pattern(HatchPattern::CeramicGrid),
            ),
            Self::CozinhaAreaServico => (
                RoomFinishes {
                    floor: "Porcelanato / cerâmica 60×60 cm".to_owned(),
                    wall: "Revestimento cerâmico nas paredes hidráulicas".to_owned(),
                    ceiling: "Forro de gesso acartonado".to_owned(),
                    baseboard: "Porcelanato do piso".to_owned(),
                    baseboard_height_cm: 7.0,
                    waterproofing: true,
                    notes: "Argamassa AC-III; caimento para ralo na área de serviço".to_owned(),
                },
                HatchStyle::for_pattern(HatchPattern::CeramicGrid),
            ),
            Self::AreaExternaVaranda => (
                RoomFinishes {
                    floor: "Porcelanato externo antiderrapante (EXT)".to_owned(),
                    wall: "Textura acrílica hidrorrepelente".to_owned(),
                    ceiling: "Laje aparente com selador e pintura acrílica".to_owned(),
                    baseboard: "Porcelanato externo".to_owned(),
                    baseboard_height_cm: 10.0,
                    waterproofing: true,
                    notes: "Caimento mínimo 1% para dreno".to_owned(),
                },
                HatchStyle::for_pattern(HatchPattern::StoneGrid),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomFinishQuantities {
    pub floor_area_m2: f64,
    pub floor_with_waste_m2: f64,
    pub wall_net_area_m2: f64,
    pub ceiling_area_m2: f64,
    pub baseboard_linear_m: f64,
}

impl Project {
    pub fn set_room_finishes(
        &mut self,
        room_id: u64,
        finishes: Option<RoomFinishes>,
    ) -> Result<(), String> {
        let room = self
            .rooms
            .iter_mut()
            .find(|r| r.id == room_id)
            .ok_or_else(|| "Cômodo não encontrado.".to_owned())?;
        room.finishes = finishes.filter(|f| !f.is_empty());
        Ok(())
    }

    pub fn apply_room_finish_preset(
        &mut self,
        room_id: u64,
        preset: RoomFinishPreset,
    ) -> Result<(), String> {
        let (finishes, hatch) = preset.build();
        let room = self
            .rooms
            .iter_mut()
            .find(|r| r.id == room_id)
            .ok_or_else(|| "Cômodo não encontrado.".to_owned())?;
        room.finishes = Some(finishes);
        room.hatch = Some(hatch);
        Ok(())
    }

    /// Computes practical finish quantities (floor + 10% waste, net wall area,
    /// ceiling area, and linear baseboard subtracting door/gap spans).
    pub fn room_finish_quantities(&self, room: &Room) -> Option<RoomFinishQuantities> {
        let floor_area_m2 = self
            .room_usable_area(room)
            .or_else(|| self.room_area(room))?;
        let outline = self.room_outline(room)?;
        let perimeter = polygon_perimeter(&outline);
        let wall_h = self.wall_height.max(2.0);

        let mut openings_area = 0.0;
        let mut floor_openings_width = 0.0;
        for op in self
            .openings
            .iter()
            .filter(|o| room.wall_ids.contains(&o.wall_id))
        {
            let w = op.width();
            let h = op.height().unwrap_or(wall_h);
            openings_area += w * h;
            if matches!(
                op.kind,
                OpeningKind::Door | OpeningKind::SaloonDoor | OpeningKind::Gap
            ) {
                floor_openings_width += w;
            }
        }

        let baseboard_linear_m = if room
            .finishes
            .as_ref()
            .is_some_and(|f| f.baseboard_height_cm <= 0.05)
        {
            0.0
        } else {
            (perimeter - floor_openings_width).max(0.0)
        };

        Some(RoomFinishQuantities {
            floor_area_m2,
            floor_with_waste_m2: floor_area_m2 * 1.10,
            wall_net_area_m2: (perimeter * wall_h - openings_area).max(0.0),
            ceiling_area_m2: floor_area_m2,
            baseboard_linear_m,
        })
    }
}

/// Builds the per-room finishes & specifications schedule (`Quadro de Acabamentos`).
pub fn build_finishes_schedule(project: &Project) -> ScheduleTable {
    let mut rows = Vec::new();
    let mut total_baseboard = 0.0;
    for room in &project.rooms {
        let Some(fin) = &room.finishes else {
            continue;
        };
        let qty = project.room_finish_quantities(room);
        let bb = qty.map(|q| q.baseboard_linear_m).unwrap_or(0.0);
        total_baseboard += bb;
        let bb_label = if fin.baseboard_height_cm <= 0.05 {
            "—".to_owned()
        } else {
            format!(
                "{} ({:.0} cm · {:.1} m)",
                fin.baseboard, fin.baseboard_height_cm, bb
            )
            .replace('.', ",")
        };
        rows.push(vec![
            room.display_name().to_owned(),
            if fin.floor.trim().is_empty() {
                "—".to_owned()
            } else {
                fin.floor.clone()
            },
            if fin.wall.trim().is_empty() {
                "—".to_owned()
            } else {
                fin.wall.clone()
            },
            if fin.ceiling.trim().is_empty() {
                "—".to_owned()
            } else {
                fin.ceiling.clone()
            },
            bb_label,
        ]);
    }
    let footer = (!rows.is_empty()).then(|| {
        vec![
            "Total".to_owned(),
            format!("{} ambiente(s)", rows.len()),
            String::new(),
            String::new(),
            format!("{total_baseboard:.1} m").replace('.', ","),
        ]
    });
    ScheduleTable {
        title: "Quadro de Acabamentos e Especificações".to_owned(),
        headers: vec![
            "Ambiente".to_owned(),
            "Piso".to_owned(),
            "Parede".to_owned(),
            "Teto".to_owned(),
            "Rodapé".to_owned(),
        ],
        rows,
        footer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn room_finishes_presets_compute_baseboard_and_net_wall_quantities_and_round_trip() {
        let mut project = Project::default();
        let sala_id = project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let wall_id = project.walls[0].id;
        // 0.90 m door subtracts from baseboard linear length (14.0 - 0.9 = 13.1 m).
        project
            .add_opening(wall_id, OpeningKind::Door, 0.5)
            .unwrap();

        project
            .apply_room_finish_preset(sala_id, RoomFinishPreset::SocialPorcelanato)
            .unwrap();
        let sala = project.rooms.iter().find(|r| r.id == sala_id).unwrap();
        let qty = project.room_finish_quantities(sala).unwrap();
        assert!((qty.baseboard_linear_m - 13.1).abs() < 0.05);
        assert!(qty.floor_with_waste_m2 > qty.floor_area_m2);
        assert!(qty.wall_net_area_m2 > 30.0 && qty.wall_net_area_m2 < 40.0);

        // Bathroom preset has 0 cm baseboard -> linear baseboard = 0.0 m + waterproofing true.
        let banho_id = project
            .add_room(Point::new(4.0, 0.0), Point::new(6.0, 2.0), "Banheiro")
            .unwrap();
        project
            .apply_room_finish_preset(banho_id, RoomFinishPreset::AreaMolhadaBanheiro)
            .unwrap();
        let banho = project.rooms.iter().find(|r| r.id == banho_id).unwrap();
        assert!(banho.finishes.as_ref().unwrap().waterproofing);
        assert_eq!(
            project
                .room_finish_quantities(banho)
                .unwrap()
                .baseboard_linear_m,
            0.0
        );

        let schedule = build_finishes_schedule(&project);
        assert_eq!(schedule.rows.len(), 2);
        assert!(schedule.rows[0][1].contains("Porcelanato"));

        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.rooms[0].finishes, project.rooms[0].finishes);
    }
}
