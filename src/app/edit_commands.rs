//! Keyboard editing commands on the selection: nudging with the arrows,
//! rotating fixtures, flipping doors and deleting.

use super::{FloorPlanApp, Selection, Tool};
use crate::domain::geometry::Point;

impl FloorPlanApp {
    pub(super) fn nudge_selection(&mut self, delta: Point) {
        let Some(selection) = self.selection else {
            return;
        };
        if self
            .selected()
            .into_iter()
            .any(|selected| self.selection_is_locked(selected))
        {
            self.message = "A camada deste elemento está bloqueada.".to_owned();
            return;
        }
        if self.item_drag.is_some() || self.draft_start().is_some() {
            return;
        }
        let previous = self.project.clone();
        if self.is_multi_selection() {
            if let Err(error) = self.project.move_elements(&self.selected(), delta) {
                self.message = error;
                return;
            }
            if self.project == previous {
                self.message =
                    "Não há espaço para mover a seleção nessa direção dentro do terreno."
                        .to_owned();
                return;
            }
            if !self.record_change(previous) {
                return;
            }
            self.sync_inspector_fields();
            self.message = format!(
                "{} movidos {}. Setas: 0,25 m · Shift+setas: 1 m.",
                self.selection_summary(),
                crate::domain::geometry::format_meters(delta.x.abs().max(delta.y.abs()))
            );
            return;
        }
        match selection {
            Selection::Wall(id) => {
                if let Err(error) = self.project.move_wall_checked(id, delta) {
                    self.message = error;
                    return;
                }
            }
            Selection::Room(id) => {
                if !self.project.move_room(id, delta) {
                    self.message = "Cômodo possui paredes compartilhadas. Mova as paredes individualmente para ajustar a planta.".to_owned();
                    return;
                }
            }
            Selection::Terrain(id) => self.project.move_terrain(id, delta),
            Selection::Fixture(id) => self.project.move_fixture(id, delta),
            Selection::Drawing(id) => self.project.move_drawing(id, delta),
            Selection::Opening(id) => {
                if let Some(center) = self.opening_center(id) {
                    self.project.move_opening_along_wall(
                        id,
                        Point::new(center.x + delta.x, center.y + delta.y),
                    );
                }
            }
        }
        if self.project == previous {
            self.message = match selection {
                Selection::Opening(_) => {
                    "A abertura só se move ao longo da parede, e não há espaço nessa direção."
                }
                _ => "Não há espaço para mover nessa direção dentro do terreno.",
            }
            .to_owned();
            return;
        }
        if !self.record_change(previous) {
            return;
        }
        self.sync_inspector_fields();
        self.message = format!(
            "Seleção movida {}. Setas: 0,25 m · Shift+setas: 1 m.",
            crate::domain::geometry::format_meters(delta.x.abs().max(delta.y.abs()))
        );
    }

    /// G: turns the selected fixture, or the next one to be placed; any
    /// other selection turns 90° clockwise about its centre.
    pub(super) fn rotate_fixture_or_next(&mut self) {
        if matches!(self.tool, Tool::Fixture(_)) {
            self.fixture_turns = (self.fixture_turns + 1) % 4;
            self.message = format!(
                "Próximo objeto girado para {}° (longe das paredes).",
                self.fixture_turns as u32 * 90
            );
        } else if let Some(Selection::Fixture(id)) = self.selection
            && !self.is_multi_selection()
        {
            let previous = self.project.clone();
            match self.project.rotate_fixture(id) {
                Ok(()) => {
                    if !self.record_change(previous) {
                        return;
                    }
                    self.sync_inspector_fields();
                    self.message = "Objeto girado 90°.".to_owned();
                }
                Err(error) => self.message = error,
            }
        } else if self.selection.is_some() {
            self.rotate_selection_quarter(true);
        }
    }

    /// I / Shift+I: the selected door opens to the other side of its wall
    /// or turns on the other jamb.
    pub(super) fn flip_selected_door(&mut self, side: bool, hinge: bool) {
        let Some(Selection::Opening(id)) = self.selection else {
            self.message = "Selecione uma porta para inverter.".to_owned();
            return;
        };
        let is_door = self
            .project
            .openings
            .iter()
            .any(|opening| opening.id == id && opening.kind.is_door());
        if !is_door {
            self.message = "Só portas mudam de lado.".to_owned();
            return;
        }
        let previous = self.project.clone();
        self.project.flip_opening(id, side, hinge);
        if !self.record_change(previous) {
            return;
        }
        self.message = match (side, hinge) {
            (true, false) => "Porta abrindo para o outro lado da parede.",
            (false, true) => "Dobradiça da porta trocada de lado.",
            _ => "Porta invertida.",
        }
        .to_owned();
    }

    pub(super) fn delete_selection(&mut self) {
        self.commit_item_drag();
        if self
            .selected()
            .into_iter()
            .any(|selected| self.selection_is_locked(selected))
        {
            self.message = "A camada deste elemento está bloqueada.".to_owned();
            return;
        }
        if self.is_multi_selection() {
            let items = self.selected();
            let summary = self.selection_summary();
            let previous = self.project.clone();
            self.project.delete_elements(&items);
            self.clear_selection();
            if !self.record_change(previous) {
                return;
            }
            self.message = format!("{summary} removidos.");
            self.sync_inspector_fields();
            return;
        }
        if let Some(selection) = self.selection.take() {
            let previous = self.project.clone();
            match selection {
                Selection::Wall(id) => self.project.delete_wall(id),
                Selection::Room(id) => {
                    self.project.delete_room(id);
                }
                Selection::Terrain(id) => {
                    self.project.delete_terrain(id);
                }
                Selection::Opening(id) => {
                    self.project.delete_opening(id);
                }
                Selection::Fixture(id) => {
                    self.project.delete_fixture(id);
                }
                Selection::Drawing(id) => {
                    self.project.delete_drawing(id);
                }
            }
            if !self.record_change(previous) {
                return;
            }
            self.message = match selection {
                Selection::Wall(_) => "Parede removida.",
                Selection::Room(_) => "Cômodo removido.",
                Selection::Terrain(_) => "Terreno removido.",
                Selection::Opening(_) => "Abertura removida.",
                Selection::Fixture(_) => "Objeto removido.",
                Selection::Drawing(_) => "Desenho removido.",
            }
            .to_owned();
            self.sync_inspector_fields();
        }
    }
}
