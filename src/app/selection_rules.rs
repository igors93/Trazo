//! Which element is selected and whether it may be edited: revalidation
//! after undo, the status message, discipline filters and locks.

use super::{FloorPlanApp, Selection, Tool};

impl FloorPlanApp {
    /// Drops from the selection whatever the project no longer has (after
    /// an undo, say); the last element left becomes the primary one.
    pub fn revalidate_selection(&mut self) {
        let kept: Vec<Selection> = self
            .selected()
            .into_iter()
            .filter(|item| self.project.element_exists(*item))
            .collect();
        self.set_selection_set(kept);
    }

    pub(super) fn selection_message(selection: Option<Selection>) -> &'static str {
        match selection {
            Some(Selection::Opening(_)) => {
                "Abertura selecionada. Arraste ao longo da parede ou altere a largura no painel."
            }
            Some(Selection::Wall(_)) => {
                "Parede selecionada. Arraste para mover ou altere o comprimento no painel."
            }
            Some(Selection::Room(_)) => {
                "Cômodo selecionado. Arraste para mover ou altere as medidas no painel."
            }
            Some(Selection::Terrain(_)) => {
                "Terreno selecionado. Ajuste as medidas ou arraste para mover."
            }
            Some(Selection::Fixture(_)) => {
                "Objeto selecionado. Arraste para mover, G para girar ou ajuste as medidas no painel."
            }
            Some(Selection::Drawing(_)) => {
                "Desenho selecionado. Arraste ou use as setas para mover; Delete exclui."
            }
            None => "Clique em um elemento para selecionar.",
        }
    }

    /// Selects an element from outside the canvas (the project browser),
    /// switching to the selection tool.
    pub(super) fn select(&mut self, selection: Selection) {
        if self.tool != Tool::Move {
            self.set_tool(Tool::Move);
        }
        self.cancel_creation();
        self.set_selection(Some(selection));
        self.sync_inspector_fields();
        self.message = Self::selection_message(self.selection).to_owned();
    }

    pub(crate) fn selection_allowed_in_discipline(&self, selection: Selection) -> bool {
        match selection {
            Selection::Terrain(_)
            | Selection::Room(_)
            | Selection::Wall(_)
            | Selection::Opening(_)
            | Selection::Drawing(_) => self.discipline.allows_architecture_edit(),
            Selection::Fixture(id) => self
                .project
                .fixtures
                .iter()
                .find(|fixture| fixture.id == id)
                .is_some_and(|fixture| self.discipline.allows_fixture(fixture.kind)),
        }
    }

    pub(crate) fn prune_selection_for_discipline(&mut self) {
        let kept: Vec<Selection> = self
            .selected()
            .into_iter()
            .filter(|sel| self.selection_allowed_in_discipline(*sel))
            .collect();
        if kept.len() != self.selected().len() {
            self.set_selection_set(kept);
            self.sync_inspector_fields();
        }
    }

    pub(crate) fn architecture_edit_blocked_message(&self) -> String {
        format!(
            "Na aba {} a visão geral aparece somente como referência (fundo). Troque para Visão geral ou Arquitetura para editar paredes, cômodos e aberturas.",
            self.discipline.label()
        )
    }

    /// Moves the selected element by `delta` metres as one undo step,
    /// within the terrain like a drag. Doors and windows slide along their
    /// wall, so only the part of `delta` along the wall counts.
    pub(super) fn selection_is_locked(&self, selection: Selection) -> bool {
        use crate::domain::layers::LayerCategory;
        if !self.selection_allowed_in_discipline(selection) {
            return true;
        }
        match selection {
            Selection::Terrain(_) => self.project.category_locked(LayerCategory::Terrain),
            Selection::Room(_) => self.project.category_locked(LayerCategory::Rooms),
            Selection::Opening(_) => self.project.category_locked(LayerCategory::Openings),
            Selection::Fixture(id) => self
                .project
                .fixtures
                .iter()
                .find(|fixture| fixture.id == id)
                .is_some_and(|fixture| {
                    self.project
                        .category_locked(crate::domain::model::Project::fixture_category(fixture))
                }),
            Selection::Wall(id) => self
                .project
                .walls
                .iter()
                .find(|w| w.id == id)
                .is_some_and(|w| self.project.is_wall_locked(w)),
            Selection::Drawing(id) => self
                .project
                .drawing(id)
                .is_some_and(|d| self.project.is_drawing_locked(d)),
        }
    }
}
