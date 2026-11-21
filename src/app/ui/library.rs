use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, Tool, theme::*};
use super::widgets::{primary_button, secondary_button, toggle_pill};
use crate::{
    domain::{
        blocks::{BlockAttribute, BlockDefinition},
        fixtures::FixtureKind,
        geometry::Point,
        library::{LibraryCategory, LibraryItem},
    },
    io::personal_library,
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
        let mut remove_personal_item_id: Option<u64> = None;
        let mut promote_item_to_personal: Option<LibraryItem> = None;
        let mut remove_project_block_id: Option<u64> = None;
        let mut remove_personal_block_id: Option<u64> = None;
        let mut promote_block_to_personal: Option<BlockDefinition> = None;
        let mut insert_block_now: Option<(BlockDefinition, Vec<(String, String)>)> = None;

        egui::Window::new("Biblioteca, Blocos e Biblioteca Pessoal")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(760.0)
            .default_height(600.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if toggle_pill(
                        ui,
                        "Catálogo e Biblioteca Pessoal",
                        self.library_tab == 0,
                        "Móveis prontos, itens do projeto e biblioteca pessoal compartilhada entre projetos",
                    )
                    .clicked()
                    {
                        self.library_tab = 0;
                    }
                    if toggle_pill(
                        ui,
                        &format!(
                            "Blocos reutilizáveis ({})",
                            self.project.blocks.len() + self.personal_library.blocks.len()
                        ),
                        self.library_tab == 1,
                        "Criar e inserir blocos compostos com atributos editáveis por instância",
                    )
                    .clicked()
                    {
                        self.library_tab = 1;
                    }
                    if toggle_pill(
                        ui,
                        &format!("Instâncias ({})", self.project.block_instances.len()),
                        self.library_tab == 2,
                        "Editar atributos de blocos já inseridos neste projeto",
                    )
                    .clicked()
                    {
                        self.library_tab = 2;
                    }
                });
                ui.add_space(6.0);
                ui.separator();

                if self.library_tab == 0 {
                    ui.label(
                        RichText::new(
                            "Itens salvos na biblioteca pessoal ficam disponíveis automaticamente em todos os projetos.",
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

                    #[derive(Clone, Copy, PartialEq, Eq)]
                    enum ItemOrigin {
                        Personal,
                        Project,
                        Builtin,
                    }

                    let query = self.library_search.trim().to_lowercase();
                    let mut all_items: Vec<(LibraryItem, ItemOrigin)> = self
                        .personal_library
                        .items
                        .iter()
                        .cloned()
                        .map(|i| (i, ItemOrigin::Personal))
                        .collect();
                    all_items.extend(
                        self.project
                            .library
                            .iter()
                            .cloned()
                            .map(|i| (i, ItemOrigin::Project)),
                    );
                    all_items.extend(
                        LibraryItem::builtin_catalog()
                            .into_iter()
                            .map(|i| (i, ItemOrigin::Builtin)),
                    );

                    egui::ScrollArea::vertical()
                        .id_salt("library_catalog_scroll")
                        .max_height(280.0)
                        .show(ui, |ui| {
                            let mut shown = 0usize;
                            for (item, origin) in all_items {
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
                                            if origin == ItemOrigin::Personal {
                                                ui.label(
                                                    RichText::new("Pessoal")
                                                        .size(10.5)
                                                        .color(ACCENT_TEXT),
                                                );
                                            }
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
                                                    if origin == ItemOrigin::Personal
                                                        && ui.small_button("Excluir").clicked()
                                                    {
                                                        remove_personal_item_id = Some(item.id);
                                                    } else if origin == ItemOrigin::Project {
                                                        if ui.small_button("Excluir").clicked() {
                                                            remove_custom_id = Some(item.id);
                                                        }
                                                        if ui
                                                            .small_button("Compartilhar")
                                                            .clicked()
                                                        {
                                                            promote_item_to_personal =
                                                                Some(item.clone());
                                                        }
                                                    }
                                                    if ui.small_button("Inserir na planta").clicked()
                                                    {
                                                        place_item = Some(item.clone());
                                                    }
                                                    if let Some(kind) = item.base_kind
                                                        && ui
                                                            .small_button("Posicionar com mouse")
                                                            .clicked()
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
                                RichText::new("Criar item personalizado")
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
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.checkbox(
                                    &mut self.save_item_to_personal,
                                    "Salvar na biblioteca pessoal (compartilhada entre projetos)",
                                );
                                if primary_button(ui, "+ Salvar item").clicked() {
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
                                        tag.clone(),
                                    ) {
                                        Ok(id) => {
                                            if self.save_item_to_personal
                                                && let Some(item) =
                                                    self.project.library.iter().find(|i| i.id == id)
                                            {
                                                self.personal_library.add_item(item.clone());
                                                let _ = personal_library::save(
                                                    &self.personal_library,
                                                );
                                            }
                                            self.new_library_name.clear();
                                            self.record_change(prev);
                                            self.message = if self.save_item_to_personal {
                                                format!(
                                                    "Item '{name}' salvo no projeto e na biblioteca pessoal."
                                                )
                                            } else {
                                                format!("Item '{name}' adicionado à biblioteca.")
                                            };
                                        }
                                        Err(err) => self.message = err,
                                    }
                                }
                            });
                        });
                } else if self.library_tab == 1 {
                    // Reusable blocks with attributes tab.
                    let selected_elements = self.selected();
                    egui::Frame::new()
                        .fill(PANEL_ALT)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(8)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "Criar bloco reutilizável a partir da seleção atual ({} elemento(s))",
                                    selected_elements.len()
                                ))
                                .font(strong(12.5))
                                .color(TEXT),
                            );
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Nome:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_block_name)
                                        .desired_width(180.0)
                                        .hint_text("Ex.: Kit lavabo padrão"),
                                );
                                ui.label("Categoria:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_block_category)
                                        .desired_width(120.0)
                                        .hint_text("Banheiro"),
                                );
                            });
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("Atributos:").size(12.0).color(TEXT_2));
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_block_attr_key)
                                        .desired_width(100.0)
                                        .hint_text("CHAVE"),
                                );
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_block_attr_default)
                                        .desired_width(130.0)
                                        .hint_text("Valor padrão"),
                                );
                                if secondary_button(ui, "+ Atributo").clicked()
                                    && !self.new_block_attr_key.trim().is_empty()
                                {
                                    let attr = BlockAttribute::new(
                                        self.new_block_attr_key.clone(),
                                        self.new_block_attr_key.clone(),
                                        self.new_block_attr_default.clone(),
                                    );
                                    self.new_block_attrs
                                        .retain(|a| !a.key.eq_ignore_ascii_case(&attr.key));
                                    self.new_block_attrs.push(attr);
                                    self.new_block_attr_key.clear();
                                    self.new_block_attr_default.clear();
                                }
                            });
                            if !self.new_block_attrs.is_empty() {
                                ui.add_space(4.0);
                                let mut remove_attr: Option<usize> = None;
                                ui.horizontal_wrapped(|ui| {
                                    for (idx, attr) in self.new_block_attrs.iter().enumerate() {
                                        let label = if attr.default_value.is_empty() {
                                            format!("{} ×", attr.key)
                                        } else {
                                            format!("{}={} ×", attr.key, attr.default_value)
                                        };
                                        if ui.small_button(label).clicked() {
                                            remove_attr = Some(idx);
                                        }
                                    }
                                });
                                if let Some(idx) = remove_attr {
                                    self.new_block_attrs.remove(idx);
                                }
                            }
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.checkbox(
                                    &mut self.save_block_to_personal,
                                    "Salvar na biblioteca pessoal (compartilhada entre projetos)",
                                );
                                if ui
                                    .add_enabled_ui(!selected_elements.is_empty(), |ui| {
                                        primary_button(ui, "Criar bloco da seleção")
                                    })
                                    .inner
                                    .clicked()
                                {
                                    let prev = self.project.clone();
                                    match self.project.define_block_from_elements(
                                        &self.new_block_name.clone(),
                                        &self.new_block_category.clone(),
                                        "",
                                        &selected_elements,
                                        self.new_block_attrs.clone(),
                                    ) {
                                        Ok(block) => {
                                            if self.save_block_to_personal {
                                                self.personal_library.add_block(block.clone());
                                                let _ = personal_library::save(
                                                    &self.personal_library,
                                                );
                                            }
                                            self.record_change(prev);
                                            self.message = format!(
                                                "Bloco '{}' criado com {} elemento(s) e {} atributo(s).",
                                                block.name,
                                                block.element_count(),
                                                block.attributes.len()
                                            );
                                            self.new_block_name.clear();
                                        }
                                        Err(err) => self.message = err,
                                    }
                                }
                            });
                        });

                    ui.add_space(8.0);
                    // Optional attribute-override panel when inserting a block.
                    if let Some(target_block) = self.block_insert_target.clone() {
                        egui::Frame::new()
                            .fill(ACCENT_SOFTER)
                            .stroke(Stroke::new(1.0, ACCENT_RING))
                            .corner_radius(8)
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "Preencher atributos para inserir '{}'",
                                        target_block.name
                                    ))
                                    .font(strong(12.5))
                                    .color(ACCENT_TEXT),
                                );
                                for (k, v) in &mut self.block_insert_overrides {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("{k}:")).size(12.0).color(TEXT));
                                        ui.add(
                                            egui::TextEdit::singleline(v).desired_width(220.0),
                                        );
                                    });
                                }
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    if primary_button(ui, "Confirmar inserção na planta").clicked()
                                    {
                                        insert_block_now = Some((
                                            target_block.clone(),
                                            self.block_insert_overrides.clone(),
                                        ));
                                        self.block_insert_target = None;
                                    }
                                    if secondary_button(ui, "Cancelar").clicked() {
                                        self.block_insert_target = None;
                                    }
                                });
                            });
                        ui.add_space(6.0);
                    }

                    // List personal shared blocks + project blocks.
                    egui::ScrollArea::vertical()
                        .id_salt("blocks_list_scroll")
                        .max_height(260.0)
                        .show(ui, |ui| {
                            let personal_blocks: Vec<(BlockDefinition, bool)> = self
                                .personal_library
                                .blocks
                                .iter()
                                .cloned()
                                .map(|b| (b, true))
                                .collect();
                            let project_blocks: Vec<(BlockDefinition, bool)> = self
                                .project
                                .blocks
                                .iter()
                                .cloned()
                                .map(|b| (b, false))
                                .collect();
                            let all_blocks = personal_blocks
                                .into_iter()
                                .chain(project_blocks)
                                .collect::<Vec<_>>();
                            if all_blocks.is_empty() {
                                ui.label(
                                    RichText::new(
                                        "Nenhum bloco criado ainda. Selecione elementos na planta e crie um bloco reutilizável acima.",
                                    )
                                    .size(12.0)
                                    .color(MUTED),
                                );
                            }
                            for (block, is_personal) in all_blocks {
                                let (w, h) = block.size_m();
                                egui::Frame::new()
                                    .fill(PANEL_ALT)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .corner_radius(6)
                                    .inner_margin(Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(&block.name)
                                                    .font(strong(12.0))
                                                    .color(TEXT),
                                            );
                                            if is_personal {
                                                ui.label(
                                                    RichText::new("Pessoal")
                                                        .size(10.5)
                                                        .color(ACCENT_TEXT),
                                                );
                                            }
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} · {:.2} × {:.2} m · {} elem. · {} atrib.",
                                                    block.category,
                                                    w,
                                                    h,
                                                    block.element_count(),
                                                    block.attributes.len()
                                                )
                                                .replace('.', ","))
                                                .size(11.0)
                                                .color(MUTED),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if is_personal {
                                                        if ui.small_button("Excluir").clicked() {
                                                            remove_personal_block_id =
                                                                Some(block.id);
                                                        }
                                                    } else {
                                                        if ui.small_button("Excluir").clicked() {
                                                            remove_project_block_id =
                                                                Some(block.id);
                                                        }
                                                        if ui
                                                            .small_button("Compartilhar")
                                                            .clicked()
                                                        {
                                                            promote_block_to_personal =
                                                                Some(block.clone());
                                                        }
                                                    }
                                                    if ui.small_button("Inserir bloco").clicked() {
                                                        if block.attributes.is_empty() {
                                                            insert_block_now =
                                                                Some((block.clone(), Vec::new()));
                                                        } else {
                                                            self.block_insert_overrides = block
                                                                .attributes
                                                                .iter()
                                                                .map(|a| {
                                                                    (
                                                                        a.key.clone(),
                                                                        a.default_value.clone(),
                                                                    )
                                                                })
                                                                .collect();
                                                            self.block_insert_target =
                                                                Some(block.clone());
                                                        }
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(4.0);
                            }
                        });
                } else {
                    // Tab 2: Placed block instances with editable per-instance attributes.
                    egui::ScrollArea::vertical()
                        .id_salt("block_instances_scroll")
                        .max_height(430.0)
                        .show(ui, |ui| {
                            if self.project.block_instances.is_empty() {
                                ui.label(
                                    RichText::new(
                                        "Nenhuma instância de bloco inserida neste projeto ainda.",
                                    )
                                    .size(12.0)
                                    .color(MUTED),
                                );
                            }
                            let mut edited = false;
                            for instance in &mut self.project.block_instances {
                                egui::Frame::new()
                                    .fill(PANEL_ALT)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .corner_radius(6)
                                    .inner_margin(Margin::same(8))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "#{} · {} ({:.2}, {:.2} m)",
                                                instance.id,
                                                instance.block_name,
                                                instance.origin.x,
                                                instance.origin.y
                                            )
                                            .replace('.', ","))
                                            .font(strong(12.0))
                                            .color(TEXT),
                                        );
                                        for (key, value) in &mut instance.attributes {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(format!("{key}:"))
                                                        .size(11.5)
                                                        .color(TEXT_2),
                                                );
                                                if ui
                                                    .add(
                                                        egui::TextEdit::singleline(value)
                                                            .desired_width(220.0),
                                                    )
                                                    .changed()
                                                {
                                                    edited = true;
                                                }
                                            });
                                        }
                                    });
                                ui.add_space(4.0);
                            }
                            if edited {
                                self.mark_edited_outside_history();
                            }
                        });
                }

                ui.add_space(6.0);
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
        if let Some(id) = remove_personal_item_id {
            self.personal_library.remove_item(id);
            let _ = personal_library::save(&self.personal_library);
        }
        if let Some(item) = promote_item_to_personal {
            self.personal_library.add_item(item.clone());
            let _ = personal_library::save(&self.personal_library);
            self.message = format!(
                "Item '{}' salvo na biblioteca pessoal compartilhada.",
                item.name
            );
        }
        if let Some(id) = remove_project_block_id {
            let prev = self.project.clone();
            self.project.remove_block_definition(id);
            self.record_change(prev);
        }
        if let Some(id) = remove_personal_block_id {
            self.personal_library.remove_block(id);
            let _ = personal_library::save(&self.personal_library);
        }
        if let Some(block) = promote_block_to_personal {
            self.personal_library.add_block(block.clone());
            let _ = personal_library::save(&self.personal_library);
            self.message = format!(
                "Bloco '{}' salvo na biblioteca pessoal compartilhada.",
                block.name
            );
        }
        if let Some((block, overrides)) = insert_block_now {
            let center = self
                .project
                .content_bounds()
                .map(|(a, b)| a.midpoint(b))
                .unwrap_or(Point::new(2.0, 2.0));
            let prev = self.project.clone();
            match self.project.insert_block(&block, center, &overrides) {
                Ok(_) => {
                    self.record_change(prev);
                    self.message = format!("Bloco '{}' inserido na planta.", block.name);
                }
                Err(err) => self.message = err,
            }
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
