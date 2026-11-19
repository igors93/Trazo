//! The selection commands of the interface: select all, by kind, similar
//! or inverted, the selection filter, and the properties panel of several
//! selected elements.

use eframe::egui::{Margin, RichText, Ui};

use super::super::{FloorPlanApp, Selection, theme::*};
use super::icons::Icon;
use super::widgets::{
    MENU_WIDTH, danger_button, hint, menu_check, menu_item, menu_separator, property_label,
    property_value, secondary_button, section,
};
use crate::domain::element::{ElementKind, SelectionFilter};
use crate::domain::model::OpeningKind;

/// The kinds "select by kind" offers, with what picks them.
const BY_KIND: [(&str, Icon); 7] = [
    ("Cômodos", Icon::Room),
    ("Paredes", Icon::Wall),
    ("Portas", Icon::Door),
    ("Janelas", Icon::Window),
    ("Vãos livres", Icon::Gap),
    (
        "Objetos",
        Icon::Fixture(crate::domain::fixtures::FixtureKind::Counter),
    ),
    ("Desenhos 2D", Icon::Glyph(super::Glyph::Polyline)),
];

impl FloorPlanApp {
    /// The elements of the kind named in `BY_KIND`.
    fn elements_by_kind(&self, label: &str) -> Vec<Selection> {
        let project = &self.project;
        let openings = |accept: fn(OpeningKind) -> bool| -> Vec<Selection> {
            project
                .openings
                .iter()
                .filter(|opening| accept(opening.kind))
                .map(|opening| Selection::Opening(opening.id))
                .collect()
        };
        match label {
            "Cômodos" => project
                .rooms
                .iter()
                .map(|r| Selection::Room(r.id))
                .collect(),
            "Paredes" => project
                .walls
                .iter()
                .map(|w| Selection::Wall(w.id))
                .collect(),
            "Portas" => openings(OpeningKind::is_door),
            "Janelas" => openings(|kind| kind == OpeningKind::Window),
            "Vãos livres" => openings(|kind| kind == OpeningKind::Gap),
            "Objetos" => project
                .fixtures
                .iter()
                .map(|f| Selection::Fixture(f.id))
                .collect(),
            _ => project
                .drawings
                .iter()
                .map(|d| Selection::Drawing(d.id))
                .collect(),
        }
    }

    /// Selects every element of one kind (`BY_KIND`).
    pub(in crate::app) fn select_by_kind(&mut self, label: &str) {
        let items = self.elements_by_kind(label);
        if items.is_empty() {
            self.message = format!("Não há {} na planta.", label.to_lowercase());
            return;
        }
        self.select_many(items, false);
    }

    /// Select all, by kind, similar, inverted; the filter.
    pub(in crate::app) fn selection_menu_items(&mut self, ui: &mut Ui) {
        if menu_item(ui, Some(Icon::Select), "Selecionar tudo", "Ctrl+A").clicked() {
            self.select_all();
            ui.close();
        }
        ui.menu_button("Selecionar por tipo", |ui| {
            ui.set_width(MENU_WIDTH);
            for (label, icon) in BY_KIND {
                let count = self.elements_by_kind(label).len();
                if ui
                    .add_enabled_ui(count > 0, |ui| {
                        menu_item(ui, Some(icon), label, &count.to_string())
                    })
                    .inner
                    .clicked()
                {
                    self.select_by_kind(label);
                    ui.close();
                }
            }
        });
        if ui
            .add_enabled_ui(self.selection.is_some(), |ui| {
                menu_item(ui, None, "Selecionar semelhantes", "")
            })
            .inner
            .on_hover_text("Mesmo tipo do elemento selecionado: portas da mesma medida, objetos iguais, paredes da mesma espessura...")
            .clicked()
        {
            self.select_similar();
            ui.close();
        }
        if menu_item(ui, None, "Inverter seleção", "").clicked() {
            self.invert_selection();
            ui.close();
        }
        ui.menu_button("Filtro de seleção", |ui| self.selection_filter_menu(ui));
    }

    /// Which kinds a box or "select all" picks.
    pub(in crate::app) fn selection_filter_menu(&mut self, ui: &mut Ui) {
        ui.set_width(MENU_WIDTH);
        for kind in ElementKind::ALL {
            let on = self.selection_filter.accepts(kind);
            if menu_check(ui, kind.plural(), "", on).clicked() {
                self.selection_filter.set(kind, !on);
                self.message = format!(
                    "Filtro de seleção: {} {}.",
                    kind.plural().to_lowercase(),
                    if on { "ignorados" } else { "incluídos" }
                );
            }
        }
        menu_separator(ui);
        if ui
            .add_enabled_ui(!self.selection_filter.is_open(), |ui| {
                menu_item(ui, None, "Incluir todos os tipos", "")
            })
            .inner
            .clicked()
        {
            self.selection_filter = SelectionFilter::default();
            self.message = "Filtro de seleção: todos os tipos.".to_owned();
        }
    }

    /// The properties panel when several elements are selected.
    pub(in crate::app) fn multi_selection_inspector(&mut self, ui: &mut Ui) {
        let all = self.selected();
        super::inspector::type_card(
            ui,
            Icon::Select,
            &format!("{} elementos", all.len()),
            "Seleção múltipla",
        );
        section(ui, "multi_selection_counts", "Na seleção", |ui| {
            eframe::egui::Grid::new("multi_selection_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .min_col_width(96.0)
                .show(ui, |ui| {
                    for kind in ElementKind::ALL {
                        let count = all.iter().filter(|item| item.kind() == kind).count();
                        if count > 0 {
                            property_label(ui, kind.plural());
                            property_value(ui, &count.to_string());
                            ui.end_row();
                        }
                    }
                    if let Some((low, high)) = self.project.elements_bounds(&all) {
                        property_label(ui, "Extensão");
                        property_value(
                            ui,
                            &format!("{:.2} × {:.2} m", high.x - low.x, high.y - low.y)
                                .replace('.', ","),
                        );
                        ui.end_row();
                    }
                });
            ui.add_space(6.0);
            hint(
                ui,
                Icon::Info,
                "Arraste qualquer um deles para mover todos; setas deslocam 0,25 m (Shift: 1 m). Shift+clique adiciona ou retira.",
                ACCENT,
            );
        });
        // The group the selection is, with its name.
        if let Some(group) = self.selected_group() {
            let mut rename = false;
            let mut ungroup = false;
            section(ui, "multi_selection_group", "Grupo", |ui| {
                ui.horizontal(|ui| {
                    let field = ui.add(
                        eframe::egui::TextEdit::singleline(&mut self.group_name_text)
                            .desired_width(150.0)
                            .hint_text("Nome do grupo"),
                    );
                    rename = field.lost_focus()
                        && ui.input(|input| input.key_pressed(eframe::egui::Key::Enter));
                    ungroup = secondary_button(ui, "Desagrupar").clicked();
                });
                ui.add_space(4.0);
                hint(
                    ui,
                    Icon::Info,
                    "Um clique seleciona o grupo todo; Ctrl+clique seleciona só um elemento.",
                    ACCENT,
                );
            });
            if rename {
                let previous = self.project.clone();
                match self.project.rename_group(group, &self.group_name_text) {
                    Ok(()) => {
                        if !self.record_change(previous) {
                            return;
                        }
                        self.message =
                            format!("Grupo renomeado para \"{}\".", self.group_name_text.trim());
                    }
                    Err(error) => self.message = error,
                }
            }
            if ungroup {
                self.ungroup_selection();
            }
        }
        let mut arrange = None;
        section(ui, "multi_selection_arrange", "Organizar", |ui| {
            ui.horizontal_wrapped(|ui| {
                for (index, (label, hint)) in [
                    (
                        "Unir",
                        "Paredes em linha ou desenhos que se tocam pelas pontas (U)",
                    ),
                    ("Agrupar", "Um clique passa a pegar todos juntos (Ctrl+G)"),
                    (
                        "Repetir...",
                        "Cópias em linhas e colunas, ou em volta de um centro",
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    if secondary_button(ui, label).on_hover_text(hint).clicked() {
                        arrange = Some(index);
                    }
                }
                let align = secondary_button(ui, "Alinhar ▾");
                eframe::egui::Popup::menu(&align).show(|ui| self.align_menu_items(ui));
                let distribute = secondary_button(ui, "Distribuir ▾");
                eframe::egui::Popup::menu(&distribute).show(|ui| self.distribute_menu_items(ui));
            });
        });
        match arrange {
            Some(0) => self.join_selection(),
            Some(1) => self.group_selection(),
            Some(2) => self.open_repeat_dialog(),
            _ => {}
        }
        let mut action = None;
        section(ui, "multi_selection_modify", "Modificar", |ui| {
            ui.horizontal_wrapped(|ui| {
                for (index, (label, hint)) in [
                    ("Copiar", "Ctrl+C; Ctrl+V cola no cursor"),
                    ("Duplicar", "Uma cópia 1 m abaixo e à direita (Ctrl+D)"),
                    ("Girar 90° ↻", "Em torno do centro da seleção (G)"),
                    ("Girar 90° ↺", "Em torno do centro da seleção (Shift+G)"),
                    ("Espelhar ↔", "Esquerda e direita, no lugar (H)"),
                    ("Espelhar ↕", "Cima e baixo, no lugar (Shift+H)"),
                ]
                .into_iter()
                .enumerate()
                {
                    if secondary_button(ui, label).on_hover_text(hint).clicked() {
                        action = Some(index);
                    }
                }
            });
        });
        match action {
            Some(0) => {
                self.copy_selection();
            }
            Some(1) => self.duplicate_selection(),
            Some(2) => self.rotate_selection_quarter(true),
            Some(3) => self.rotate_selection_quarter(false),
            Some(4) => self.mirror_selection(true),
            Some(5) => self.mirror_selection(false),
            _ => {}
        }
        let mut zoom = false;
        let mut clear = false;
        let mut delete = false;
        eframe::egui::Frame::new()
            .inner_margin(Margin {
                left: 14,
                right: 12,
                top: 12,
                bottom: 12,
            })
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    zoom = secondary_button(ui, "Aproximar").clicked();
                    clear = secondary_button(ui, "Limpar seleção").clicked();
                    delete = danger_button(ui, "Excluir todos").clicked();
                });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "A seleção por caixa e Ctrl+A seguem o filtro de seleção (menu Editar).",
                    )
                    .size(11.5)
                    .color(MUTED),
                );
            });
        if zoom && let Some(primary) = self.selection {
            self.zoom_to_selection(primary);
        }
        if clear {
            self.clear_selection();
            self.after_selection_change();
        }
        if delete {
            self.delete_selection();
        }
    }
}
