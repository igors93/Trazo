//! Tools for doors, windows and fixtures: inserting and editing them.

use serde_json::{Value, json};

use super::{
    Change, element_id, flag, from_model, number, quarter_turns, required_number, text, to_model,
    xy,
};
use crate::app::{FloorPlanApp, Selection};
use crate::{
    domain::{
        fixtures::FixtureKind,
        geometry::{Point, format_meters},
        model::OpeningKind,
    },
    mcp::ToolOutput,
};

impl FloorPlanApp {
    pub(super) fn tool_insert_opening(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let wall_id = element_id(args, "parede_id")?;
        let kind = match text(args, "tipo")
            .map(|kind| kind.to_lowercase().replace(['-', ' '], "_"))
            .as_deref()
        {
            Some("porta") => OpeningKind::Door,
            Some("janela") => OpeningKind::Window,
            Some("porta_vai_e_vem" | "vai_e_vem" | "porta_faroeste") => OpeningKind::SaloonDoor,
            Some("vao_livre" | "vao" | "vão_livre" | "vão") => OpeningKind::Gap,
            Some("portao" | "portão") => OpeningKind::Gate,
            _ => {
                return Err(
                    "Informe 'tipo': porta, janela, porta_vai_e_vem, vao_livre ou portao."
                        .to_owned(),
                );
            }
        };
        let position = number(args, "posicao")?.unwrap_or(0.5);
        if !(0.0..=1.0).contains(&position) {
            return Err(
                "'posicao' é uma fração de 0 a 1 ao longo da parede (0,5 = meio).".to_owned(),
            );
        }
        let width = number(args, "largura")?;
        if let Some(width) = width
            && !(kind.min_width()..=kind.max_width()).contains(&width)
        {
            return Err(format!(
                "A largura de {} deve estar entre {} e {}.",
                kind.label().to_lowercase(),
                format_meters(kind.min_width()),
                format_meters(kind.max_width())
            ));
        }
        self.require(
            wall_id,
            |selection| matches!(selection, Selection::Wall(_)),
            "parede",
        )?;
        self.apply_change(None, |project| {
            let id = project.add_opening_sized(wall_id, kind, position, width).ok_or(format!(
                "Não há espaço para a {} nessa posição: ela não pode sobrepor outra abertura nem ficar perto demais das pontas da parede.",
                kind.label().to_lowercase()
            ))?;
            let width = project
                .openings
                .iter()
                .find(|opening| opening.id == id)
                .map(|opening| opening.width())
                .unwrap_or_default();
            Ok(Change::new(format!(
                "{} inserida na parede {wall_id} (id {id}), {}.",
                kind.label(),
                format_meters(width)
            ))
            .select(Selection::Opening(id))
            .data(json!({ "id": id })))
        })
    }

    pub(super) fn tool_insert_fixture(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let keys: Vec<&str> = FixtureKind::ALL.iter().map(|kind| kind.key()).collect();
        let kind = text(args, "tipo")
            .and_then(|kind| FixtureKind::from_key(&kind.to_lowercase()))
            .ok_or_else(|| format!("Informe 'tipo': {}.", keys.join(", ")))?;
        let (default_width, default_depth) = kind.default_size();
        let width = number(args, "largura")?.unwrap_or(default_width);
        let depth = number(args, "profundidade")?.unwrap_or(default_depth);
        let point = to_model(required_number(args, "x")?, required_number(args, "y")?);
        let turns = quarter_turns(args)?.unwrap_or(0);
        let against_wall = flag(args, "encostar_na_parede");
        let placed = if against_wall {
            self.place_against_wall(kind, point, width, depth, turns)
        } else {
            crate::domain::fixtures::Fixture {
                id: 0,
                kind,
                center: point,
                width,
                depth,
                quarter_turns: turns,
                angle_rad: 0.0,
                electrical: kind.default_electrical(),
                structural_load_kn: kind.default_structural_load_kn(),
                pile: (kind == FixtureKind::PileCap)
                    .then(crate::domain::fixtures::PileCapSpec::default),
            }
        };
        let moved_to_wall = against_wall && placed.center != point;
        let mut output = self.apply_change(None, |project| {
            let id = project
                .add_fixture(kind, placed.center, width, depth, placed.quarter_turns)
                .ok_or(format!(
                    "Não foi possível inserir: o objeto precisa ficar dentro do terreno e medir entre {:.2} e {:.2} m.",
                    kind.min_size(),
                    crate::domain::fixtures::MAX_SIZE
                ).replace('.', ","))?;
            Ok(Change::new(format!(
                "{} inserido(a) (id {id}), {} × {}, rotação {}°.",
                kind.label(),
                format_meters(width),
                format_meters(depth),
                placed.quarter_turns as u32 * 90
            ))
            .select(Selection::Fixture(id))
            .data(json!({ "id": id, "centro": xy(placed.center), "rotacao": placed.quarter_turns as u32 * 90 })))
        })?;
        if against_wall && !moved_to_wall {
            output
                .text
                .push_str(" Nenhuma parede reta a menos de 0,8 m: ficou no ponto informado.");
        }
        Ok(output)
    }

    pub(super) fn tool_edit_fixture(&mut self, args: &Value) -> Result<ToolOutput, String> {
        let id = element_id(args, "id")?;
        self.require(
            id,
            |selection| matches!(selection, Selection::Fixture(_)),
            "objeto",
        )?;
        let current = self
            .project
            .fixtures
            .iter()
            .find(|fixture| fixture.id == id)
            .copied()
            .ok_or("Objeto não encontrado.")?;
        let x = number(args, "x")?;
        let y = number(args, "y")?;
        let width = number(args, "largura")?;
        let depth = number(args, "profundidade")?;
        let turns = quarter_turns(args)?;
        let free_angle = number(args, "angulo_graus")?;
        if x.is_none()
            && y.is_none()
            && width.is_none()
            && depth.is_none()
            && turns.is_none()
            && free_angle.is_none()
        {
            return Err("Informe x, y, rotacao, angulo_graus, largura ou profundidade.".to_owned());
        }
        let (cx, cy) = from_model(current.center);
        let center =
            (x.is_some() || y.is_some()).then(|| to_model(x.unwrap_or(cx), y.unwrap_or(cy)));
        let size = (width.is_some() || depth.is_some()).then(|| {
            (
                width.unwrap_or(current.width),
                depth.unwrap_or(current.depth),
            )
        });
        let angle_update = if let Some(deg) = free_angle {
            let norm = deg.rem_euclid(360.0);
            let q = ((norm / 90.0).floor() as u8) % 4;
            let rem_deg = norm - f64::from(q) * 90.0;
            let rem_rad = if rem_deg.abs() < 1e-6 {
                0.0
            } else {
                rem_deg.to_radians()
            };
            Some((q, rem_rad))
        } else {
            turns.map(|q| (q, 0.0))
        };
        self.apply_change(None, |project| {
            if center.is_some() || size.is_some() {
                project.update_fixture(id, center, size, None)?;
            }
            if let Some((q, rad)) = angle_update {
                project.set_fixture_angle(id, q, rad)?;
            }
            Ok(Change::new(format!("Objeto {id} atualizado.")).select(Selection::Fixture(id)))
        })
    }

    pub(super) fn tool_edit_opening(&mut self, args: &Value) -> Result<ToolOutput, String> {
        use crate::domain::openings::{DoorType, WindowType};
        let id = element_id(args, "id")?;
        self.require(
            id,
            |selection| matches!(selection, Selection::Opening(_)),
            "porta ou janela",
        )?;
        let width = number(args, "largura")?;
        let position = number(args, "posicao")?;
        let flip_side = flag(args, "inverter_lado");
        let flip_hinge = flag(args, "inverter_dobradica");
        let height = number(args, "altura")?;
        let sill = number(args, "peitoril")?;
        let door_type = match text(args, "tipo_porta") {
            Some(raw) => {
                let key = raw.to_lowercase().replace(['-', ' '], "_");
                let dt = match key.as_str() {
                    "giro" | "swing" => DoorType::Swing,
                    "dupla" | "double_swing" => DoorType::DoubleSwing,
                    "correr" | "sliding" => DoorType::Sliding,
                    "correr_embutida" | "pocket_sliding" => DoorType::PocketSliding,
                    "pivotante" | "pivot" => DoorType::Pivot,
                    "sanfonada" | "camarao" | "bifold" => DoorType::Bifold,
                    "giratoria" | "revolving" => DoorType::Revolving,
                    _ => {
                        return Err(
                            "'tipo_porta' deve ser giro, dupla, correr, correr_embutida, pivotante, sanfonada ou giratoria."
                                .to_owned(),
                        );
                    }
                };
                Some(dt)
            }
            None => None,
        };
        let window_type = match text(args, "tipo_janela") {
            Some(raw) => {
                let key = raw.to_lowercase().replace(['-', ' '], "_");
                let wt = match key.as_str() {
                    "correr" | "sliding" => WindowType::Sliding,
                    "basculante" => WindowType::Basculante,
                    "maxim_ar" | "maximar" => WindowType::MaximAr,
                    "abrir" | "casement" => WindowType::Casement,
                    "oscilo_batente" | "tilt_turn" => WindowType::TiltTurn,
                    "fixa" | "fixed" => WindowType::Fixed,
                    "bay_window" => WindowType::BayWindow,
                    _ => {
                        return Err(
                            "'tipo_janela' deve ser correr, basculante, maxim_ar, abrir, oscilo_batente, fixa ou bay_window."
                                .to_owned(),
                        );
                    }
                };
                Some(wt)
            }
            None => None,
        };
        if width.is_none()
            && position.is_none()
            && !flip_side
            && !flip_hinge
            && height.is_none()
            && sill.is_none()
            && door_type.is_none()
            && window_type.is_none()
        {
            return Err(
                "Informe largura, posicao, altura, peitoril, tipo_porta, tipo_janela, inverter_lado ou inverter_dobradica."
                    .to_owned(),
            );
        }
        if position.is_some_and(|position| !(0.0..=1.0).contains(&position)) {
            return Err("'posicao' é uma fração de 0 a 1 ao longo da parede.".to_owned());
        }
        self.apply_change(None, |project| {
            if let Some(width) = width {
                project.update_opening_width(id, width)?;
            }
            if let Some(position) = position {
                let wall = project
                    .openings
                    .iter()
                    .find(|opening| opening.id == id)
                    .and_then(|opening| {
                        project.walls.iter().find(|wall| wall.id == opening.wall_id)
                    })
                    .cloned()
                    .ok_or("A parede dessa abertura não existe.")?;
                let target = Point::new(
                    wall.start.x + (wall.end.x - wall.start.x) * position,
                    wall.start.y + (wall.end.y - wall.start.y) * position,
                );
                if !project.move_opening_along_wall(id, target) {
                    return Err("Não há espaço para a abertura nessa posição.".to_owned());
                }
            }
            if flip_side || flip_hinge {
                project.flip_opening(id, flip_side, flip_hinge);
            }
            if height.is_some() || sill.is_some() {
                project.set_opening_heights(id, height, sill)?;
            }
            if let Some(dt) = door_type {
                project.set_opening_door_type(id, dt)?;
            }
            if let Some(wt) = window_type {
                project.set_opening_window_type(id, wt)?;
            }
            Ok(Change::new(format!("Abertura {id} atualizada.")).select(Selection::Opening(id)))
        })
    }
}
