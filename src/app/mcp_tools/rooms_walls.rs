//! Tools for walls and rooms: creating them, editing a room, changing the
//! masonry and moving elements.

use serde_json::{Value, json};

use super::{Change, element_id, flag, number, required_number, text, to_model};
use crate::app::{FloorPlanApp, Selection};
use crate::{
    domain::geometry::{Point, format_meters},
    mcp::ToolOutput,
};

impl FloorPlanApp {
    pub(super) fn tool_create_room(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let name = text(args, "nome").ok_or("Informe 'nome' do cômodo.")?;
        if let Some(corners) = args.get("pontos").filter(|value| !value.is_null()) {
            return self.create_polygon_room(&name, corners);
        }
        let x = required_number(args, "x")?;
        let y = required_number(args, "y")?;
        let width = required_number(args, "largura")?;
        let length = required_number(args, "comprimento")?;
        if width < 0.5 || length < 0.5 {
            return Err(
                "O cômodo precisa ter pelo menos 0,50 m de largura e de comprimento.".to_owned(),
            );
        }
        self.apply_change(None, |project| {
            let id = project
                .add_room(to_model(x, y), to_model(x + width, y + length), &name)
                .ok_or(
                    "Não foi possível criar o cômodo: ele precisa ficar inteiro dentro do terreno.",
                )?;
            let walls = project
                .rooms
                .iter()
                .find(|room| room.id == id)
                .map(|room| room.wall_ids.clone())
                .unwrap_or_default();
            Ok(Change::new(format!(
                "Cômodo \"{name}\" criado (id {id}), {} × {}.",
                format_meters(width),
                format_meters(length)
            ))
            .select(Selection::Room(id))
            .data(json!({ "id": id, "paredes": walls })))
        })
    }

    pub(super) fn tool_create_wall(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let start = to_model(required_number(args, "x1")?, required_number(args, "y1")?);
        let end = to_model(required_number(args, "x2")?, required_number(args, "y2")?);
        self.apply_change(None, |project| {
            let id = project.add_wall(start, end).ok_or(
                "Não foi possível criar a parede: ela precisa ter pelo menos 0,25 m e ficar dentro do terreno.",
            )?;
            Ok(Change::new(format!(
                "Parede criada (id {id}), {}.",
                format_meters(start.distance(end))
            ))
            .select(Selection::Wall(id))
            .data(json!({ "id": id })))
        })
    }

    pub(super) fn tool_masonry(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let mut material = self.project.masonry.clone();
        if let Some(name) = text(args, "material") {
            material = crate::domain::masonry::Masonry::presets()
                .into_iter()
                .find(|preset| preset.name.to_lowercase() == name.to_lowercase())
                .unwrap_or(crate::domain::masonry::Masonry { name, ..material });
        }
        let cm = |key: &str| number(args, key).map(|value| value.map(|cm| cm / 100.0));
        if let Some(width) = cm("largura_bloco_cm")? {
            material.block_width = width;
        }
        if let Some(length) = cm("comprimento_bloco_cm")? {
            material.block_length = length;
        }
        if let Some(joint) = cm("junta_cm")? {
            material.joint = joint;
        }
        if let Some(plaster) = cm("reboco_cm")? {
            material.plaster = plaster;
        }
        if !material.valid() {
            return Err("Medidas de bloco inválidas: largura de 4 a 50 cm, comprimento de 5 a 100 cm, junta até 5 cm e reboco até 8 cm.".to_owned());
        }
        let height = number(args, "pe_direito")?;
        if height.is_some_and(|height| !(1.5..=10.0).contains(&height)) {
            return Err("pe_direito deve estar entre 1,50 e 10,00 m.".to_owned());
        }
        let all = flag(args, "aplicar_em_todas");
        let label = material.label();
        let thickness = material.thickness();
        let output = self.apply_change(None, |project| {
            project.masonry = material.clone();
            if all {
                project.set_all_walls_masonry(&material);
            }
            if let Some(height) = height {
                project.wall_height = height;
            }
            Ok(Change::new(format!(
                "Alvenaria: {label}, parede acabada de {}{}.",
                format_meters(thickness),
                if all {
                    ", aplicada a todas as paredes"
                } else {
                    " para as novas paredes"
                }
            )))
        })?;
        self.sync_masonry_fields();
        Ok(output)
    }

    pub(super) fn tool_edit_room(&mut self, args: &Value) -> Result<ToolOutput, String> {
        use crate::domain::finishes::{RoomFinishPreset, RoomFinishes};
        let id = element_id(args, "id")?;
        self.require(
            id,
            |selection| matches!(selection, Selection::Room(_)),
            "cômodo",
        )?;
        let name = text(args, "nome");
        let width = number(args, "largura")?;
        let length = number(args, "comprimento")?;
        let finish_preset = match text(args, "preset_acabamento") {
            Some(raw) => {
                let key = raw.to_lowercase().replace(['-', ' '], "_");
                let preset = match key.as_str() {
                    "social_porcelanato" | "social" => RoomFinishPreset::SocialPorcelanato,
                    "dormitorio_madeira" | "dormitorio" | "quarto" => {
                        RoomFinishPreset::DormitorioMadeira
                    }
                    "area_molhada_banheiro" | "banheiro" => RoomFinishPreset::AreaMolhadaBanheiro,
                    "cozinha_area_servico" | "cozinha" => RoomFinishPreset::CozinhaAreaServico,
                    "area_externa_varanda" | "varanda" | "externa" => {
                        RoomFinishPreset::AreaExternaVaranda
                    }
                    _ => {
                        return Err(
                            "'preset_acabamento' deve ser social_porcelanato, dormitorio_madeira, banheiro, cozinha ou varanda."
                                .to_owned(),
                        );
                    }
                };
                Some(preset)
            }
            None => None,
        };
        let floor_fin = text(args, "piso");
        let wall_fin = text(args, "parede");
        let ceiling_fin = text(args, "teto");
        let baseboard_fin = text(args, "rodape");
        let baseboard_cm = number(args, "rodape_cm")?;
        let waterproofing = args.get("impermeabilizacao").and_then(Value::as_bool);
        let finish_notes = text(args, "notas_acabamento");
        let has_finish_fields = floor_fin.is_some()
            || wall_fin.is_some()
            || ceiling_fin.is_some()
            || baseboard_fin.is_some()
            || baseboard_cm.is_some()
            || waterproofing.is_some()
            || finish_notes.is_some();

        if name.is_none()
            && width.is_none()
            && length.is_none()
            && finish_preset.is_none()
            && !has_finish_fields
        {
            return Err(
                "Informe nome, largura, comprimento, preset_acabamento ou campos de acabamento (piso, parede, teto, rodape, rodape_cm, impermeabilizacao)."
                    .to_owned(),
            );
        }
        if let Some(cm) = baseboard_cm
            && !(0.0..=50.0).contains(&cm)
        {
            return Err("'rodape_cm' deve ficar entre 0 e 50 cm.".to_owned());
        }
        let current_bounds = self
            .project
            .rooms
            .iter()
            .find(|room| room.id == id)
            .and_then(|room| self.project.room_bounds(room))
            .map(|(min, max)| (max.x - min.x, max.y - min.y));
        if (width.is_some() || length.is_some()) && current_bounds.is_none() {
            return Err("Cômodo sem paredes válidas para medir.".to_owned());
        }
        self.apply_change(None, |project| {
            if let Some(name) = &name {
                project.rename_room(id, name)?;
            }
            if (width.is_some() || length.is_some())
                && let Some((current_width, current_length)) = current_bounds
            {
                project.resize_room_dimensions(
                    id,
                    width.unwrap_or(current_width),
                    length.unwrap_or(current_length),
                )?;
            }
            if let Some(preset) = finish_preset {
                project.apply_room_finish_preset(id, preset)?;
            }
            if has_finish_fields {
                let mut finishes = project
                    .rooms
                    .iter()
                    .find(|r| r.id == id)
                    .and_then(|r| r.finishes.clone())
                    .unwrap_or_else(|| RoomFinishes {
                        baseboard_height_cm: 10.0,
                        ..RoomFinishes::default()
                    });
                if let Some(v) = &floor_fin {
                    finishes.floor = v.clone();
                }
                if let Some(v) = &wall_fin {
                    finishes.wall = v.clone();
                }
                if let Some(v) = &ceiling_fin {
                    finishes.ceiling = v.clone();
                }
                if let Some(v) = &baseboard_fin {
                    finishes.baseboard = v.clone();
                }
                if let Some(cm) = baseboard_cm {
                    finishes.baseboard_height_cm = cm;
                }
                if let Some(w) = waterproofing {
                    finishes.waterproofing = w;
                }
                if let Some(v) = &finish_notes {
                    finishes.notes = v.clone();
                }
                project.set_room_finishes(id, Some(finishes))?;
            }
            let room = project.rooms.iter().find(|room| room.id == id);
            let summary = room
                .and_then(|room| {
                    let (min, max) = project.room_bounds(room)?;
                    Some(format!(
                        "\"{}\", {} × {}",
                        room.display_name(),
                        format_meters(max.x - min.x),
                        format_meters(max.y - min.y)
                    ))
                })
                .unwrap_or_default();
            Ok(Change::new(format!("Cômodo {id} atualizado: {summary}."))
                .select(Selection::Room(id)))
        })
    }

    pub(super) fn tool_move(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let id = element_id(args, "id")?;
        let dx = required_number(args, "dx")?;
        let dy = required_number(args, "dy")?;
        let selection = self.find_element(id)?;
        let delta = Point::new(dx, -dy);
        let opening_target = self
            .opening_center(id)
            .map(|center| Point::new(center.x + delta.x, center.y + delta.y));
        let label = Self::element_label(selection);
        self.apply_change(
            Some("Não foi possível mover: o terreno ou as paredes vizinhas não deixam espaço nessa direção."),
            |project| {
                match selection {
                    Selection::Wall(id) => project.move_wall(id, delta),
                    Selection::Room(id) => {
                        if !project.move_room(id, delta) {
                            return Err("Esse cômodo compartilha paredes com outro e não pode ser movido sozinho; mova as paredes individualmente.".to_owned());
                        }
                    }
                    Selection::Terrain(id) => project.move_terrain(id, delta),
                    Selection::Fixture(id) => project.move_fixture(id, delta),
                    Selection::Drawing(id) => project.move_drawing(id, delta),
                    Selection::Opening(id) => {
                        if let Some(target) = opening_target {
                            project.move_opening_along_wall(id, target);
                        }
                    }
                }
                Ok(Change::new(format!("{label} {id} movido(a).")).select(selection))
            },
        )
    }
}
