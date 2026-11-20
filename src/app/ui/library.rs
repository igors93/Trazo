use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, Tool, theme::*};
use super::widgets::{primary_button, secondary_button, toggle_pill};
use crate::domain::{
    fixtures::FixtureKind,
    geometry::Point,
    library::{LibraryCategory, LibraryItem},
};

impl FloorPlanApp {
    pub(super) fn library_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Library {
            return;
        }
        let mut open = true;
        let mut close_clicked = false;
        let mut place_item: Option<LibraryItem> = None;
        let mut pick_tool: Option<FixtureKind> = None;
        let mut remove_custom_id: Option<u64> = None;

        egui::Window::new("Biblioteca de Móveis, Equipamentos e Símbolos")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(700.0)
            .default_height(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Escolha móveis e símbolos técnicos prontos ou crie itens personalizados com medidas próprias.",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label("Buscar:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.library_search)
                            .desired_width(220.0)
                            .hint_text("Ex.: cama, sofá, tomada..."),
                    );
                    if !self.library_search.is_empty() && ui.small_button("Limpar").clicked() {
                        self.library_search.clear();
                    }
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if toggle_pill(
                        ui,
                        "Todos",
                        self.library_category_filter.is_none(),
                        "Mostrar todas as categorias",
                    )
                    .clicked()
                    {
                        self.library_category_filter = None;
                    }
                    for cat in LibraryCategory::ALL {
                        if toggle_pill(
                            ui,
                            cat.label(),
                            self.library_category_filter == Some(cat),
                            cat.label(),
                        )
                        .clicked()
                        {
                            self.library_category_filter = Some(cat);
                        }
                    }
                });

                ui.add_space(6.0);
                ui.separator();

                // Catalog + Custom items list.
                let query = self.library_search.trim().to_lowercase();
                let mut all_items: Vec<(LibraryItem, bool)> = self
                    .project
                    .library
                    .iter()
                    .cloned()
                    .map(|i| (i, true))
                    .collect();
                all_items.extend(
                    LibraryItem::builtin_catalog()
                        .into_iter()
                        .map(|i| (i, false)),
                );

                egui::ScrollArea::vertical()
                    .id_salt("library_catalog_scroll")
                    .max_height(280.0)
                    .show(ui, |ui| {
                        let mut shown = 0usize;
                        for (item, is_custom) in all_items {
                            if let Some(cat) = self.library_category_filter
                                && item.category != cat
                            {
                                continue;
                            }
                            if !query.is_empty()
                                && !item.name.to_lowercase().contains(&query)
                                && !item.category.label().to_lowercase().contains(&query)
                            {
                                continue;
                            }
                            shown += 1;
                            egui::Frame::new()
                                .fill(PANEL_ALT)
                                .stroke(Stroke::new(1.0, BORDER))
                                .corner_radius(6)
                                .inner_margin(Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(&item.name)
                                                .font(strong(12.0))
                                                .color(TEXT),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "{} · {:.2} × {:.2} m",
                                                item.category.label(),
                                                item.width_m,
                                                item.depth_m
                                            )
                                            .replace('.', ","))
                                            .size(11.0)
                                            .color(MUTED),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if is_custom && ui.small_button("Excluir").clicked()
                                                {
                                                    remove_custom_id = Some(item.id);
                                                }
                                                if ui.small_button("Inserir na planta").clicked() {
                                                    place_item = Some(item.clone());
                                                }
                                                if let Some(kind) = item.base_kind
                                                    && ui.small_button("Posicionar com mouse").clicked()
                                                {
                                                    pick_tool = Some(kind);
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }
                        if shown == 0 {
                            ui.label(
                                RichText::new("Nenhum item encontrado para o filtro atual.")
                                    .size(12.0)
                                    .color(MUTED),
                            );
                        }
                    });

                ui.add_space(6.0);
                ui.separator();

                // Create custom library item form.
                egui::Frame::new()
                    .fill(PANEL_ALT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(8)
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Criar item personalizado na biblioteca")
                                .font(strong(12.5))
                                .color(TEXT),
                        );
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Nome:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_library_name)
                                    .desired_width(170.0)
                                    .hint_text("Ex.: Ilha gourmet"),
                            );
                            ui.label("Larg.:");
                            ui.add(
                                egui::DragValue::new(&mut self.new_library_width)
                                    .range(0.20..=8.0)
                                    .speed(0.05)
                                    .suffix(" m"),
                            );
                            ui.label("Prof.:");
                            ui.add(
                                egui::DragValue::new(&mut self.new_library_depth)
                                    .range(0.20..=8.0)
                                    .speed(0.05)
                                    .suffix(" m"),
                            );
                            egui::ComboBox::from_id_salt("new_library_base_kind")
                                .selected_text(
                                    self.new_library_base
                                        .map(|k| k.label())
                                        .unwrap_or("Símbolo de texto"),
                                )
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(
                                            self.new_library_base.is_none(),
                                            "Símbolo de texto",
                                        )
                                        .clicked()
                                    {
                                        self.new_library_base = None;
                                    }
                                    for kind in FixtureKind::ALL {
                                        if ui
                                            .selectable_label(
                                                self.new_library_base == Some(kind),
                                                kind.label(),
                                            )
                                            .clicked()
                                        {
                                            self.new_library_base = Some(kind);
                                        }
                                    }
                                });
                            if primary_button(ui, "+ Salvar na biblioteca").clicked() {
                                let name = self.new_library_name.clone();
                                let w = self.new_library_width;
                                let d = self.new_library_depth;
                                let base = self.new_library_base;
                                let tag = base.is_none().then(|| name.clone());
                                let prev = self.project.clone();
                                match self.project.add_custom_library_item(
                                    &name,
                                    LibraryCategory::Custom,
                                    w,
                                    d,
                                    base,
                                    tag,
                                ) {
                                    Ok(_) => {
                                        self.new_library_name.clear();
                                        self.record_change(prev);
                                        self.message =
                                            format!("Item '{name}' adicionado à biblioteca.");
                                    }
                                    Err(err) => self.message = err,
                                }
                            }
                        });
                    });
                ui.add_space(4.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if secondary_button(ui, "Fechar").clicked() {
                        close_clicked = true;
                    }
                });
            });

        if let Some(id) = remove_custom_id {
            let prev = self.project.clone();
            self.project.remove_library_item(id);
            self.record_change(prev);
        }
        if let Some(item) = place_item {
            let center = self
                .project
                .content_bounds()
                .map(|(a, b)| a.midpoint(b))
                .unwrap_or(Point::new(2.0, 2.0));
            let prev = self.project.clone();
            match self.project.place_library_item(&item, center, 0) {
                Ok(_) => {
                    self.record_change(prev);
                    self.message = format!("Item '{}' inserido na planta.", item.name);
                }
                Err(err) => self.message = err,
            }
        }
        if let Some(kind) = pick_tool {
            self.set_tool(Tool::Fixture(kind));
            self.active_doc_dialog = DocDialog::None;
            return;
        }
        if !open || close_clicked {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
