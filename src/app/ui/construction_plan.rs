//! Section H ("From model to construction") interactive tabs inside the
//! "Quantitativos e planejamento de obra" window (`DocDialog::Quantities`):
//! - Materials catalogue (#135)
//! - Stage shopping list (#136), cost estimate & SINAPI (#87), schedule (#137),
//!   safety & permits (#138) and site progress log (#139) are added incrementally.

use eframe::egui::{self, RichText};

use super::super::{FloorPlanApp, theme::*};
use super::widgets::secondary_button;
use crate::domain::construction::{
    materials::{
        MaterialCategory, MaterialOverride, materials_catalogue_csv, materials_catalogue_svg,
    },
    shopping::{ConstructionStage, stage_shopping_list_csv, stage_shopping_list_svg},
};

impl FloorPlanApp {
    /// Tab 1: Editable model-detected materials catalogue (#135).
    pub(super) fn materials_catalogue_tab(&mut self, ui: &mut egui::Ui) {
        let filter_id = egui::Id::new("trazo_materials_category_filter");
        let selected_id = egui::Id::new("trazo_materials_selected_id");
        let mut category_filter: Option<MaterialCategory> =
            ui.ctx().data(|d| d.get_temp(filter_id)).unwrap_or(None);
        let mut selected_mat: String = ui
            .ctx()
            .data(|d| d.get_temp(selected_id))
            .unwrap_or_else(|| "concrete_structural".to_owned());

        let catalogue = self.project.materials_catalogue();

        ui.label(
            RichText::new(
                "Materiais detectados automaticamente na estrutura, alvenaria, argamassas, cobertura, hidráulica e elétrica. Edite fck, densidade, perdas ou traços — as alterações sincronizam com os cálculos e ficam salvas no arquivo .trazo.",
            )
            .size(12.0)
            .color(TEXT_2),
        );
        ui.add_space(6.0);

        // Summary + export bar
        let mut export_kind: Option<&'static str> = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} materiais · {} em uso no modelo · {} editado(s)",
                    catalogue.items.len(),
                    catalogue.active_count,
                    catalogue.edited_count
                ))
                .font(strong(12.0)),
            );
            ui.separator();
            if secondary_button(ui, "Exportar CSV").clicked() {
                export_kind = Some("csv");
            }
            if secondary_button(ui, "Exportar PDF").clicked() {
                export_kind = Some("pdf");
            }
            if secondary_button(ui, "Exportar SVG").clicked() {
                export_kind = Some("svg");
            }
        });
        ui.add_space(4.0);

        // Category filter pills
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut category_filter, None, "Todos");
            for cat in MaterialCategory::ALL {
                ui.selectable_value(&mut category_filter, Some(cat), cat.label());
            }
        });
        ui.separator();

        // Main table
        egui::ScrollArea::both()
            .id_salt("materials_catalogue_table")
            .max_height(280.0)
            .show(ui, |ui| {
                egui::Grid::new("materials_catalogue_grid")
                    .striped(true)
                    .spacing([10.0, 5.0])
                    .show(ui, |ui| {
                        for header in [
                            "Categoria",
                            "Material",
                            "Norma",
                            "Densidade",
                            "Resistência",
                            "Perda",
                            "Traço (cim/cal/areia/brita)",
                            "Uso",
                        ] {
                            ui.label(RichText::new(header).font(strong(11.5)).color(TEXT_2));
                        }
                        ui.end_row();

                        for item in &catalogue.items {
                            if let Some(filter) = category_filter
                                && item.category != filter
                            {
                                continue;
                            }
                            let is_sel = selected_mat == item.id;
                            let name_text = if item.edited {
                                format!("{} *", item.name)
                            } else {
                                item.name.clone()
                            };
                            ui.label(
                                RichText::new(item.category.label())
                                    .size(11.5)
                                    .color(TEXT_2),
                            );
                            if ui.selectable_label(is_sel, name_text).clicked() {
                                selected_mat = item.id.clone();
                            }
                            ui.label(RichText::new(&item.standard).size(11.0).color(MUTED));
                            ui.label(
                                item.density_kg_m3
                                    .map_or("—".to_owned(), |d| format!("{d:.0} kg/m³")),
                            );
                            ui.label(
                                item.strength_mpa
                                    .map_or("—".to_owned(), |s| format!("{s:.1} MPa")),
                            );
                            ui.label(format!("{:.0} %", item.waste_pct));
                            let mix_str = match (
                                item.mix_cement_kg_m3,
                                item.mix_lime_kg_m3,
                                item.mix_sand_m3_per_m3,
                                item.mix_gravel_m3_per_m3,
                            ) {
                                (Some(c), Some(l), Some(s), None) => {
                                    format!("{c:.0} kg / {l:.0} kg / {s:.2} m³")
                                }
                                (Some(c), None, Some(s), Some(g)) => {
                                    format!("{c:.0} kg / {s:.2} m³ / {g:.2} m³")
                                }
                                _ => "—".to_owned(),
                            };
                            ui.label(RichText::new(mix_str).size(11.0));
                            ui.label(
                                RichText::new(if item.used_in_model {
                                    "No modelo"
                                } else {
                                    "Referência"
                                })
                                .size(11.0)
                                .color(if item.used_in_model {
                                    egui::Color32::from_rgb(27, 94, 32)
                                } else {
                                    MUTED
                                }),
                            );
                            ui.end_row();
                        }
                    });
            });

        // Inline editor for the selected material
        if let Some(entry) = catalogue.find(&selected_mat).cloned() {
            ui.add_space(6.0);
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("Editar: {} ({})", entry.name, entry.category.key()))
                        .font(strong(12.5)),
                );
                if entry.edited && secondary_button(ui, "Restaurar padrão").clicked() {
                    let before = self.project.clone();
                    if self.project.reset_catalogue_material(&entry.id) {
                        self.record_change(before);
                        self.message = format!("Material '{}' restaurado ao padrão.", entry.name);
                    }
                }
            });
            ui.label(
                RichText::new(format!(
                    "{} · {}",
                    entry.usage_summary, entry.properties_note
                ))
                .size(11.0)
                .color(MUTED),
            );
            ui.add_space(4.0);

            let mut patch = MaterialOverride::default();
            let mut changed = false;

            ui.horizontal_wrapped(|ui| {
                let mut waste = entry.waste_pct;
                ui.label("Perda (%):");
                if ui
                    .add(
                        egui::DragValue::new(&mut waste)
                            .range(0.0..=50.0)
                            .speed(0.5)
                            .max_decimals(1)
                            .suffix(" %"),
                    )
                    .changed()
                {
                    patch.waste_pct = Some(waste);
                    changed = true;
                }

                if let Some(mut density) = entry.density_kg_m3 {
                    ui.separator();
                    ui.label("Densidade:");
                    if ui
                        .add(
                            egui::DragValue::new(&mut density)
                                .range(100.0..=15_000.0)
                                .speed(10.0)
                                .max_decimals(0)
                                .suffix(" kg/m³"),
                        )
                        .changed()
                    {
                        patch.density_kg_m3 = Some(density);
                        changed = true;
                    }
                }

                if let Some(mut strength) = entry.strength_mpa {
                    ui.separator();
                    let range = if entry.id == "concrete_structural" {
                        15.0..=50.0
                    } else {
                        1.0..=1000.0
                    };
                    ui.label("Resistência:");
                    if ui
                        .add(
                            egui::DragValue::new(&mut strength)
                                .range(range)
                                .speed(1.0)
                                .max_decimals(1)
                                .suffix(" MPa"),
                        )
                        .changed()
                    {
                        patch.strength_mpa = Some(strength);
                        changed = true;
                    }
                }
            });

            if entry.mix_cement_kg_m3.is_some()
                || entry.mix_lime_kg_m3.is_some()
                || entry.mix_sand_m3_per_m3.is_some()
                || entry.mix_gravel_m3_per_m3.is_some()
            {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Traço por m³:").size(11.5).color(TEXT_2));
                    if let Some(mut c) = entry.mix_cement_kg_m3 {
                        ui.label("Cimento:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut c)
                                    .range(50.0..=800.0)
                                    .speed(5.0)
                                    .max_decimals(0)
                                    .suffix(" kg"),
                            )
                            .changed()
                        {
                            patch.mix_cement_kg_m3 = Some(c);
                            changed = true;
                        }
                    }
                    if let Some(mut l) = entry.mix_lime_kg_m3 {
                        ui.label("Cal:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut l)
                                    .range(0.0..=500.0)
                                    .speed(5.0)
                                    .max_decimals(0)
                                    .suffix(" kg"),
                            )
                            .changed()
                        {
                            patch.mix_lime_kg_m3 = Some(l);
                            changed = true;
                        }
                    }
                    if let Some(mut s) = entry.mix_sand_m3_per_m3 {
                        ui.label("Areia:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut s)
                                    .range(0.0..=2.0)
                                    .speed(0.02)
                                    .max_decimals(2)
                                    .suffix(" m³"),
                            )
                            .changed()
                        {
                            patch.mix_sand_m3_per_m3 = Some(s);
                            changed = true;
                        }
                    }
                    if let Some(mut g) = entry.mix_gravel_m3_per_m3 {
                        ui.label("Brita:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut g)
                                    .range(0.0..=2.0)
                                    .speed(0.02)
                                    .max_decimals(2)
                                    .suffix(" m³"),
                            )
                            .changed()
                        {
                            patch.mix_gravel_m3_per_m3 = Some(g);
                            changed = true;
                        }
                    }
                });
            }

            if changed {
                let before = self.project.clone();
                match self.project.edit_catalogue_material(&entry.id, patch) {
                    Ok(msg) => {
                        self.record_change(before);
                        self.message = msg;
                    }
                    Err(err) => self.message = err,
                }
            }
        }

        ui.ctx().data_mut(|d| {
            d.insert_temp(filter_id, category_filter);
            d.insert_temp(selected_id, selected_mat);
        });

        if let Some(kind) = export_kind {
            self.export_materials_catalogue(kind);
        }
    }

    fn export_materials_catalogue(&mut self, kind: &str) {
        let (label, ext, filename) = match kind {
            "csv" => ("Planilha CSV", "csv", "catalogo-materiais.csv"),
            "pdf" => ("Documento PDF", "pdf", "catalogo-materiais.pdf"),
            _ => ("Desenho SVG", "svg", "catalogo-materiais.svg"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(filename)
            .save_file()
        else {
            return;
        };
        let res = match kind {
            "csv" => std::fs::write(&path, materials_catalogue_csv(&self.project))
                .map_err(|e| e.to_string()),
            "pdf" => {
                let svg = materials_catalogue_svg(&self.project);
                crate::domain::views::export_view_pdf(&svg, &path)
            }
            _ => {
                let svg = materials_catalogue_svg(&self.project);
                crate::domain::views::export_view_svg(&svg, &path)
            }
        };
        self.message = match res {
            Ok(()) => format!("Catálogo de materiais exportado em {}", path.display()),
            Err(error) => format!("Erro ao exportar o catálogo de materiais: {error}"),
        };
    }

    /// Tab 2: Stage-by-stage shopping list across all disciplines (#136).
    pub(super) fn stage_shopping_tab(&mut self, ui: &mut egui::Ui) {
        let filter_id = egui::Id::new("trazo_shopping_stage_filter");
        let mut stage_filter: Option<ConstructionStage> =
            ui.ctx().data(|d| d.get_temp(filter_id)).unwrap_or(None);

        let list = self.project.stage_shopping_list();

        ui.label(
            RichText::new(
                "Lista de compras consolidada de todas as disciplinas (estrutura, alvenaria, cobertura, elétrica, hidráulica e acabamentos), organizada por etapa de execução com conversão para unidades comerciais (sacos de cimento 50 kg, cal 20 kg, areia/brita em m³, barras de 12 m, rolos de 100 m e tubos de 6 m).",
            )
            .size(12.0)
            .color(TEXT_2),
        );
        ui.add_space(6.0);

        let mut export_kind: Option<&'static str> = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!(
                    "Totais: {} sc cimento 50 kg · {} sc cal 20 kg · {:.1} m³ areia · {:.1} m³ brita · {:.0} kg aço CA-50 ({} barras 12 m) · {} blocos",
                    list.summary.cement_bags_50kg,
                    list.summary.lime_bags_20kg,
                    list.summary.sand_m3,
                    list.summary.gravel_m3,
                    list.summary.steel_ca50_kg,
                    list.summary.steel_bars_12m,
                    list.summary.total_blocks
                ))
                .font(strong(11.5)),
            );
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if secondary_button(ui, "Exportar CSV").clicked() {
                export_kind = Some("csv");
            }
            if secondary_button(ui, "Exportar PDF").clicked() {
                export_kind = Some("pdf");
            }
            if secondary_button(ui, "Exportar SVG").clicked() {
                export_kind = Some("svg");
            }
        });
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut stage_filter, None, "Todas as etapas");
            for st in ConstructionStage::ALL {
                ui.selectable_value(
                    &mut stage_filter,
                    Some(st),
                    format!("{}. {}", st.number(), st.short_label()),
                );
            }
        });
        ui.separator();

        egui::ScrollArea::both()
            .id_salt("stage_shopping_scroll")
            .show(ui, |ui| {
                if list.stages.is_empty() {
                    ui.label(
                        RichText::new("Desenhe cômodos e gere a estrutura/instalações para ver a lista de compras por etapa.")
                            .color(MUTED),
                    );
                }
                for group in &list.stages {
                    if let Some(f) = stage_filter
                        && group.stage != f
                    {
                        continue;
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(group.stage.label()).font(strong(13.0)));
                    egui::Grid::new(("shopping_stage_grid", group.stage.key()))
                        .striped(true)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            for h in [
                                "Disciplina",
                                "Material",
                                "Líquido",
                                "Comprar",
                                "Unid.",
                                "Observação comercial",
                            ] {
                                ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                            }
                            ui.end_row();
                            for item in &group.items {
                                ui.label(RichText::new(item.discipline).size(11.0).color(MUTED));
                                ui.label(&item.description);
                                ui.label(format!("{:.1}", item.net_quantity).replace('.', ","));
                                ui.label(
                                    RichText::new(
                                        format!("{:.1}", item.buy_quantity).replace('.', ","),
                                    )
                                    .font(strong(11.5)),
                                );
                                ui.label(&item.unit);
                                ui.label(
                                    RichText::new(&item.commercial_note)
                                        .size(11.0)
                                        .color(MUTED),
                                );
                                ui.end_row();
                            }
                        });
                }
                ui.add_space(8.0);
                for note in &list.assumptions {
                    ui.label(RichText::new(format!("• {note}")).size(11.0).color(MUTED));
                }
            });

        ui.ctx()
            .data_mut(|d| d.insert_temp(filter_id, stage_filter));

        if let Some(kind) = export_kind {
            self.export_stage_shopping(kind);
        }
    }

    fn export_stage_shopping(&mut self, kind: &str) {
        let (label, ext, filename) = match kind {
            "csv" => ("Planilha CSV", "csv", "lista-compras-etapas.csv"),
            "pdf" => ("Documento PDF", "pdf", "lista-compras-etapas.pdf"),
            _ => ("Desenho SVG", "svg", "lista-compras-etapas.svg"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(filename)
            .save_file()
        else {
            return;
        };
        let res = match kind {
            "csv" => std::fs::write(&path, stage_shopping_list_csv(&self.project))
                .map_err(|e| e.to_string()),
            "pdf" => {
                let svg = stage_shopping_list_svg(&self.project);
                crate::domain::views::export_view_pdf(&svg, &path)
            }
            _ => {
                let svg = stage_shopping_list_svg(&self.project);
                crate::domain::views::export_view_svg(&svg, &path)
            }
        };
        self.message = match res {
            Ok(()) => format!("Lista de compras por etapa exportada em {}", path.display()),
            Err(error) => format!("Erro ao exportar lista de compras: {error}"),
        };
    }
}
