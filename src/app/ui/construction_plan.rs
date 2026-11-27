//! Section H ("From model to construction") interactive tabs inside the
//! "Quantitativos e planejamento de obra" window (`DocDialog::Quantities`):
//! - Materials catalogue (#135)
//! - Stage shopping list (#136), cost estimate & SINAPI (#87), schedule (#137),
//!   safety & permits (#138) and site progress log (#139) are added incrementally.

use eframe::egui::{self, RichText};

use super::super::{FloorPlanApp, theme::*};
use super::widgets::secondary_button;
use crate::domain::construction::{
    cost::{cost_estimate_csv, cost_estimate_svg, edit_unit_price, format_brl},
    materials::{
        MaterialCategory, MaterialOverride, materials_catalogue_csv, materials_catalogue_svg,
    },
    progress::{
        StageProgressStatus, add_progress_entry, progress_log_csv, progress_log_svg,
        remove_progress_entry,
    },
    safety_guide::{
        SafetyPriority, export_construction_bundle_pdf, safety_permits_svg, set_safety_check,
    },
    schedule::{ScheduleTaskKind, construction_schedule_csv, construction_schedule_gantt_svg},
    shopping::{ConstructionStage, stage_shopping_list_csv, stage_shopping_list_svg},
};

impl FloorPlanApp {
    /// Tab 1: Editable model-detected materials catalogue (#135).
    pub(super) fn materials_catalogue_tab(&mut self, ui: &mut egui::Ui) {
        let filter_id = egui::Id::unique("trazo_materials_category_filter");
        let selected_id = egui::Id::unique("trazo_materials_selected_id");
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
        let filter_id = egui::Id::unique("trazo_shopping_stage_filter");
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

    /// Tab 3: Cost estimate from quantities with editable price tables & SINAPI import (#87).
    pub(super) fn cost_estimate_tab(&mut self, ui: &mut egui::Ui) {
        let sel_id = egui::Id::unique("trazo_cost_selected_code");
        let mut selected_code: String = ui
            .ctx()
            .data(|d| d.get_temp(sel_id))
            .unwrap_or_else(|| "cimento_50kg".to_owned());

        let report = self.project.cost_estimate();

        ui.label(
            RichText::new(&report.plain_summary)
                .font(strong(12.5))
                .color(TEXT),
        );
        ui.label(
            RichText::new(format!(
                "Materiais: {} · Mão de obra: {} · BDI ({:.0} %): {} · Base: {}",
                format_brl(report.material_total_brl),
                format_brl(report.labor_total_brl),
                report.bdi_pct,
                format_brl(report.bdi_total_brl),
                report.reference_label
            ))
            .size(11.5)
            .color(TEXT_2),
        );
        ui.add_space(6.0);

        let mut include_labor = self.project.construction.cost.include_labor;
        let mut bdi_pct = self.project.construction.cost.bdi_pct;
        let mut settings_changed = false;
        let mut do_import_sinapi = false;
        let mut do_reset_prices = false;
        let mut export_kind: Option<&'static str> = None;

        ui.horizontal_wrapped(|ui| {
            settings_changed |= ui
                .checkbox(&mut include_labor, "Incluir mão de obra estimada")
                .changed();
            ui.separator();
            ui.label("BDI:");
            settings_changed |= ui
                .add(
                    egui::DragValue::new(&mut bdi_pct)
                        .range(0.0..=40.0)
                        .speed(0.5)
                        .max_decimals(1)
                        .suffix(" %"),
                )
                .changed();
            ui.separator();
            if secondary_button(ui, "Importar SINAPI (CSV)...").clicked() {
                do_import_sinapi = true;
            }
            if !self.project.construction.cost.unit_prices.is_empty()
                && secondary_button(ui, "Restaurar preços padrão").clicked()
            {
                do_reset_prices = true;
            }
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
        ui.separator();

        egui::ScrollArea::both()
            .id_salt("cost_estimate_scroll")
            .max_height(330.0)
            .show(ui, |ui| {
                ui.label(RichText::new("1. Resumo por etapa").font(strong(13.0)));
                egui::Grid::new("cost_stages_grid")
                    .striped(true)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        for h in ["Etapa", "Materiais", "Mão de obra", "BDI", "Total", "%"] {
                            ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                        }
                        ui.end_row();
                        for st in &report.stages {
                            ui.label(RichText::new(st.stage.label()).font(strong(11.5)));
                            ui.label(format_brl(st.material_brl));
                            ui.label(format_brl(st.labor_brl));
                            ui.label(format_brl(st.bdi_brl));
                            ui.label(RichText::new(format_brl(st.total_brl)).font(strong(11.5)));
                            ui.label(format!("{:.1} %", st.share_pct));
                            ui.end_row();
                        }
                    });

                ui.add_space(10.0);
                ui.label(
                    RichText::new(
                        "2. Curva ABC (clique em um insumo para editar seu preço unitário)",
                    )
                    .font(strong(13.0)),
                );
                egui::Grid::new("cost_abc_grid")
                    .striped(true)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        for h in ["Classe", "Insumo", "Qtd.", "Unid.", "Total", "% Acum."] {
                            ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                        }
                        ui.end_row();
                        for row in &report.abc_curve {
                            let cls_color = match row.class {
                                'A' => egui::Color32::from_rgb(183, 28, 28),
                                'B' => egui::Color32::from_rgb(178, 106, 0),
                                _ => egui::Color32::from_rgb(27, 94, 32),
                            };
                            ui.label(
                                RichText::new(row.class.to_string())
                                    .font(strong(11.5))
                                    .color(cls_color),
                            );
                            if ui
                                .selectable_label(selected_code == row.item_code, &row.description)
                                .clicked()
                            {
                                selected_code = row.item_code.clone();
                            }
                            ui.label(format!("{:.1}", row.total_quantity).replace('.', ","));
                            ui.label(&row.unit);
                            ui.label(RichText::new(format_brl(row.total_brl)).font(strong(11.5)));
                            ui.label(format!("{:.1} %", row.cumulative_pct));
                            ui.end_row();
                        }
                    });

                ui.add_space(6.0);
                for note in &report.assumptions {
                    ui.label(RichText::new(format!("• {note}")).size(11.0).color(MUTED));
                }
            });

        // Inline unit-price editor for the selected item_code
        if let Some(sample_item) = report.items.iter().find(|i| i.item_code == selected_code) {
            ui.add_space(6.0);
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "Editar preço unitário: {} (código {} · SINAPI {} · fonte: {})",
                    sample_item.description,
                    sample_item.item_code,
                    sample_item.sinapi_code,
                    sample_item.source
                ))
                .font(strong(12.0)),
            );
            let mut mat_u = sample_item.material_unit_brl;
            let mut lab_u = sample_item.labor_unit_brl;
            let mut price_changed = false;
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Material (R$/{}):", sample_item.unit));
                price_changed |= ui
                    .add(
                        egui::DragValue::new(&mut mat_u)
                            .range(0.0..=50_000.0)
                            .speed(0.5)
                            .max_decimals(2),
                    )
                    .changed();
                ui.separator();
                ui.label(format!("Mão de obra (R$/{}):", sample_item.unit));
                price_changed |= ui
                    .add(
                        egui::DragValue::new(&mut lab_u)
                            .range(0.0..=50_000.0)
                            .speed(0.5)
                            .max_decimals(2),
                    )
                    .changed();
            });
            if price_changed {
                let before = self.project.clone();
                if let Ok(msg) = edit_unit_price(
                    &mut self.project,
                    &sample_item.item_code,
                    Some(mat_u),
                    Some(lab_u),
                ) {
                    self.record_change(before);
                    self.message = msg;
                }
            }
        }

        if settings_changed {
            let before = self.project.clone();
            self.project.construction.cost.include_labor = include_labor;
            self.project.construction.cost.bdi_pct = bdi_pct;
            self.record_change(before);
        }
        if do_reset_prices {
            let before = self.project.clone();
            self.project.construction.cost = Default::default();
            self.record_change(before);
            self.message =
                "Preços unitários restaurados para a referência SINAPI padrão.".to_owned();
        }
        if do_import_sinapi
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Tabela SINAPI (CSV/TSV/TXT)", &["csv", "tsv", "txt"])
                .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let ref_name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("SINAPI importado");
                    let before = self.project.clone();
                    match self.project.import_sinapi_table(&content, Some(ref_name)) {
                        Ok(res) => {
                            self.record_change(before);
                            self.message = format!(
                                "Tabela SINAPI '{}' importada: {} preços atualizados ({} ignorados).",
                                res.reference_label, res.imported, res.skipped
                            );
                        }
                        Err(err) => self.message = format!("Erro ao importar SINAPI: {err}"),
                    }
                }
                Err(err) => self.message = format!("Erro ao ler arquivo SINAPI: {err}"),
            }
        }

        ui.ctx().data_mut(|d| d.insert_temp(sel_id, selected_code));

        if let Some(kind) = export_kind {
            self.export_cost_estimate(kind);
        }
    }

    fn export_cost_estimate(&mut self, kind: &str) {
        let (label, ext, filename) = match kind {
            "csv" => ("Planilha CSV", "csv", "orcamento-obra.csv"),
            "pdf" => ("Documento PDF", "pdf", "orcamento-obra.pdf"),
            _ => ("Desenho SVG", "svg", "orcamento-obra.svg"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(filename)
            .save_file()
        else {
            return;
        };
        let res = match kind {
            "csv" => {
                std::fs::write(&path, cost_estimate_csv(&self.project)).map_err(|e| e.to_string())
            }
            "pdf" => {
                let svg = cost_estimate_svg(&self.project);
                crate::domain::views::export_view_pdf(&svg, &path)
            }
            _ => {
                let svg = cost_estimate_svg(&self.project);
                crate::domain::views::export_view_svg(&svg, &path)
            }
        };
        self.message = match res {
            Ok(()) => format!("Orçamento exportado em {}", path.display()),
            Err(error) => format!("Erro ao exportar orçamento: {error}"),
        };
    }

    /// Tab 4: Construction schedule, curing/shoring waits and physical-financial timeline (#137).
    pub(super) fn construction_schedule_tab(&mut self, ui: &mut egui::Ui) {
        let report = self.project.construction_schedule();

        ui.label(
            RichText::new(&report.plain_summary)
                .font(strong(12.5))
                .color(TEXT),
        );
        ui.add_space(4.0);

        let mut cfg = self.project.construction.schedule.clone();
        let mut changed = false;
        let mut export_kind: Option<&'static str> = None;

        ui.horizontal_wrapped(|ui| {
            ui.label("Dias/sem:");
            changed |= ui
                .selectable_value(&mut cfg.workdays_per_week, 5, "5 d (seg–sex)")
                .changed();
            changed |= ui
                .selectable_value(&mut cfg.workdays_per_week, 6, "6 d (seg–sáb)")
                .changed();
            ui.separator();
            ui.label("Pedreiros:");
            changed |= ui
                .add(egui::DragValue::new(&mut cfg.masons).range(1..=10))
                .changed();
            ui.label("Serventes:");
            changed |= ui
                .add(egui::DragValue::new(&mut cfg.helpers).range(1..=12))
                .changed();
            ui.label("Eletric.:");
            changed |= ui
                .add(egui::DragValue::new(&mut cfg.electricians).range(1..=4))
                .changed();
            ui.label("Encan.:");
            changed |= ui
                .add(egui::DragValue::new(&mut cfg.plumbers).range(1..=4))
                .changed();
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if secondary_button(ui, "Exportar Gantt PDF").clicked() {
                export_kind = Some("pdf");
            }
            if secondary_button(ui, "Exportar Gantt SVG").clicked() {
                export_kind = Some("svg");
            }
            if secondary_button(ui, "Exportar CSV").clicked() {
                export_kind = Some("csv");
            }
        });
        ui.separator();

        let total_days = f32::from(report.total_workdays.max(1) as u16);

        egui::ScrollArea::both()
            .id_salt("construction_schedule_scroll")
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!(
                        "1. Etapas, caminho crítico ({}) e tempos de cura/escoramento",
                        report.critical_path_ids.join(" → ")
                    ))
                    .font(strong(13.0)),
                );
                egui::Grid::new("schedule_tasks_grid")
                    .striped(true)
                    .spacing([10.0, 5.0])
                    .show(ui, |ui| {
                        for h in [
                            "ID",
                            "Atividade",
                            "Tipo",
                            "Dias",
                            "Início–Fim",
                            "Linha do tempo (Gantt)",
                            "Custo etapa",
                        ] {
                            ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                        }
                        ui.end_row();

                        for t in &report.tasks {
                            let badge_color = match t.kind {
                                ScheduleTaskKind::Curing | ScheduleTaskKind::Shoring => {
                                    egui::Color32::from_rgb(180, 110, 0)
                                }
                                ScheduleTaskKind::Work if t.is_critical => {
                                    egui::Color32::from_rgb(183, 28, 28)
                                }
                                ScheduleTaskKind::Work => egui::Color32::from_rgb(25, 118, 210),
                            };
                            ui.label(RichText::new(&t.id).font(strong(11.5)).color(badge_color));
                            ui.label(&t.title).on_hover_text(&t.productivity_note);
                            ui.label(RichText::new(t.kind.label()).size(11.0).color(badge_color));
                            ui.label(format!("{} d", t.duration_days));
                            ui.label(format!("D{}–D{}", t.early_start + 1, t.early_finish));

                            // Compact inline Gantt bar
                            let bar_w = 180.0_f32;
                            let bar_h = 12.0_f32;
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(bar_w, bar_h),
                                egui::Sense::hover(),
                            );
                            let painter = ui.painter();
                            painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(238, 242, 247));
                            let x0 = rect.left() + (t.early_start as f32 / total_days) * bar_w;
                            let w = ((t.duration_days as f32 / total_days) * bar_w).max(4.0);
                            painter.rect_filled(
                                egui::Rect::from_min_size(
                                    egui::pos2(x0, rect.top() + 1.0),
                                    egui::vec2(w, bar_h - 2.0),
                                ),
                                2.0,
                                badge_color,
                            );

                            ui.label(if t.cost_brl > 0.01 {
                                format_brl(t.cost_brl)
                            } else {
                                "—".to_owned()
                            });
                            ui.end_row();
                        }
                    });

                ui.add_space(10.0);
                ui.label(
                    RichText::new("2. Cronograma físico-financeiro semanal (desembolso e Curva S)")
                        .font(strong(13.0)),
                );
                egui::Grid::new("schedule_financial_grid")
                    .striped(true)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        for h in [
                            "Semana",
                            "Desembolso",
                            "Acumulado",
                            "% Sem.",
                            "% Acum.",
                            "Etapas em andamento",
                        ] {
                            ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                        }
                        ui.end_row();
                        for p in &report.periods {
                            ui.label(RichText::new(&p.label).font(strong(11.5)));
                            ui.label(format_brl(p.cost_brl));
                            ui.label(
                                RichText::new(format_brl(p.cumulative_cost_brl)).font(strong(11.5)),
                            );
                            ui.label(format!("{:.1} %", p.physical_pct));
                            ui.label(format!("{:.1} %", p.cumulative_pct));
                            ui.label(
                                RichText::new(p.active_stages.join(" · "))
                                    .size(11.0)
                                    .color(MUTED),
                            );
                            ui.end_row();
                        }
                    });

                ui.add_space(6.0);
                for note in &report.assumptions {
                    ui.label(RichText::new(format!("• {note}")).size(11.0).color(MUTED));
                }
            });

        if changed {
            let before = self.project.clone();
            self.project.construction.schedule = cfg;
            self.record_change(before);
        }
        if let Some(kind) = export_kind {
            self.export_construction_schedule(kind);
        }
    }

    fn export_construction_schedule(&mut self, kind: &str) {
        let (label, ext, filename) = match kind {
            "csv" => ("Planilha CSV", "csv", "cronograma-obra.csv"),
            "pdf" => ("Documento PDF", "pdf", "cronograma-gantt.pdf"),
            _ => ("Desenho SVG", "svg", "cronograma-gantt.svg"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(filename)
            .save_file()
        else {
            return;
        };
        let res = match kind {
            "csv" => std::fs::write(&path, construction_schedule_csv(&self.project))
                .map_err(|e| e.to_string()),
            "pdf" => {
                let svg = construction_schedule_gantt_svg(&self.project);
                crate::domain::views::export_view_pdf(&svg, &path)
            }
            _ => {
                let svg = construction_schedule_gantt_svg(&self.project);
                crate::domain::views::export_view_svg(&svg, &path)
            }
        };
        self.message = match res {
            Ok(()) => format!("Cronograma exportado em {}", path.display()),
            Err(error) => format!("Erro ao exportar cronograma: {error}"),
        };
    }

    /// One-click M9 bundle PDF export (catalogue + stage shopping + cost + Gantt + NR-18 safety).
    pub(super) fn export_construction_bundle_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Documento PDF", &["pdf"])
            .set_file_name("pacote-planejamento-obra.pdf")
            .save_file()
        else {
            return;
        };
        self.message = match export_construction_bundle_pdf(&self.project, &path) {
            Ok(pages) => format!(
                "Pacote completo de obra ({pages} páginas A3: catálogo, compras, orçamento, cronograma e segurança NR-18) salvo em {}",
                path.display()
            ),
            Err(error) => format!("Erro ao exportar pacote de obra: {error}"),
        };
    }

    /// Tab 5: Site safety (NR-18 / NR-10 / NR-35) checklists & Brazilian permits guide (#138).
    pub(super) fn safety_and_permits_tab(&mut self, ui: &mut egui::Ui) {
        let guide = self.project.site_safety_and_permits();
        let mut toggle: Option<(String, bool)> = None;
        let mut export_kind: Option<&'static str> = None;

        ui.label(
            RichText::new(format!(
                "Guia de segurança no canteiro (NR-18, NR-10, NR-35 e EPIs) e licenças obrigatórias (ART/RRT, alvará, CNO e habite-se) · {}/{} itens conferidos.",
                guide.completed_checks, guide.total_checks
            ))
            .font(strong(12.0)),
        );
        ui.label(RichText::new(&guide.help_guidance).size(11.5).color(TEXT_2));
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            if secondary_button(ui, "Exportar guia (PDF)").clicked() {
                export_kind = Some("pdf");
            }
            if secondary_button(ui, "Exportar guia (SVG)").clicked() {
                export_kind = Some("svg");
            }
            ui.separator();
            if secondary_button(ui, "Pacote completo de obra (PDF — 5 pranchas A3)").clicked() {
                export_kind = Some("bundle");
            }
        });
        ui.separator();

        egui::ScrollArea::both()
            .id_salt("safety_permits_scroll")
            .show(ui, |ui| {
                for alert in &guide.site_alerts {
                    ui.label(
                        RichText::new(format!("⚠ {alert}"))
                            .font(strong(11.5))
                            .color(egui::Color32::from_rgb(183, 28, 28)),
                    );
                    ui.add_space(3.0);
                }

                ui.label(
                    RichText::new(
                        "1. Checklists de segurança e EPIs por etapa (NR-18 / NR-10 / NR-35)",
                    )
                    .font(strong(13.0)),
                );
                for st in &guide.stages {
                    ui.add_space(6.0);
                    ui.label(RichText::new(st.stage.label()).font(strong(12.0)));
                    ui.label(
                        RichText::new(format!("EPIs recomendados: {}", st.ppe.join(" · ")))
                            .size(11.0)
                            .color(TEXT_2),
                    );
                    for item in &st.items {
                        let mut val = item.checked;
                        let badge_col = if item.priority == SafetyPriority::Critical {
                            egui::Color32::from_rgb(183, 28, 28)
                        } else {
                            TEXT_2
                        };
                        ui.horizontal_wrapped(|ui| {
                            if ui.checkbox(&mut val, "").changed() {
                                toggle = Some((item.id.clone(), val));
                            }
                            ui.label(
                                RichText::new(format!(
                                    "[{} · {}]",
                                    item.norm,
                                    item.priority.label()
                                ))
                                .size(11.0)
                                .font(strong(11.0))
                                .color(badge_col),
                            );
                            ui.label(&item.text);
                        });
                    }
                }

                ui.add_space(10.0);
                ui.separator();
                ui.label(
                    RichText::new("2. Profissional habilitado, licenças e regularização da obra")
                        .font(strong(13.0)),
                );
                for p in &guide.permits {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("{} — {} ({})", p.title, p.authority, p.when_needed))
                            .font(strong(12.0))
                            .color(if p.mandatory {
                                TEXT
                            } else {
                                egui::Color32::from_rgb(27, 94, 32)
                            }),
                    );
                    ui.label(RichText::new(&p.why_it_matters).size(11.5).color(TEXT_2));
                }
            });

        if let Some((id, val)) = toggle {
            let before = self.project.clone();
            if let Ok(msg) = set_safety_check(&mut self.project, &id, val) {
                self.record_change(before);
                self.message = msg;
            }
        }
        if let Some(kind) = export_kind {
            if kind == "bundle" {
                self.export_construction_bundle_dialog();
            } else {
                let (label, ext, filename) = if kind == "pdf" {
                    ("Documento PDF", "pdf", "seguranca-nr18-licencas.pdf")
                } else {
                    ("Desenho SVG", "svg", "seguranca-nr18-licencas.svg")
                };
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(label, &[ext])
                    .set_file_name(filename)
                    .save_file()
                {
                    let svg = safety_permits_svg(&self.project);
                    let res = if kind == "pdf" {
                        crate::domain::views::export_view_pdf(&svg, &path)
                    } else {
                        crate::domain::views::export_view_svg(&svg, &path)
                    };
                    self.message = match res {
                        Ok(()) => format!(
                            "Guia de segurança e licenças exportado em {}",
                            path.display()
                        ),
                        Err(error) => format!("Erro ao exportar guia de segurança: {error}"),
                    };
                }
            }
        }
    }

    /// Tab 6: Site progress log with photos, notes and plan-vs-actual comparison (#139).
    pub(super) fn site_progress_tab(&mut self, ui: &mut egui::Ui) {
        let stage_id = egui::Id::unique("trazo_progress_form_stage");
        let date_id = egui::Id::unique("trazo_progress_form_date");
        let pct_id = egui::Id::unique("trazo_progress_form_pct");
        let cost_id = egui::Id::unique("trazo_progress_form_cost");
        let note_id = egui::Id::unique("trazo_progress_form_note");
        let photos_id = egui::Id::unique("trazo_progress_form_photos");

        let mut form_stage: ConstructionStage = ui
            .ctx()
            .data(|d| d.get_temp(stage_id))
            .unwrap_or(ConstructionStage::SiteAndSettingOut);
        let mut form_date: String = ui
            .ctx()
            .data(|d| d.get_temp(date_id))
            .unwrap_or_else(|| "2026-11-27".to_owned());
        let mut form_pct: f64 = ui.ctx().data(|d| d.get_temp(pct_id)).unwrap_or(50.0);
        let mut form_cost: f64 = ui.ctx().data(|d| d.get_temp(cost_id)).unwrap_or(0.0);
        let mut form_note: String = ui.ctx().data(|d| d.get_temp(note_id)).unwrap_or_default();
        let mut form_photos: Vec<String> =
            ui.ctx().data(|d| d.get_temp(photos_id)).unwrap_or_default();

        let report = self.project.progress_report();
        let mut ref_day = self.project.construction.progress_reference_day;
        let max_day = report.total_planned_workdays.max(1);
        let mut ref_day_changed = false;
        let mut add_clicked = false;
        let mut pick_photos = false;
        let mut remove_id: Option<u64> = None;
        let mut export_kind: Option<&'static str> = None;

        ui.label(
            RichText::new(&report.plain_summary)
                .font(strong(12.0))
                .color(TEXT),
        );
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            ui.label("Dia útil de referência (cronograma):");
            ref_day_changed |= ui
                .add(egui::Slider::new(&mut ref_day, 0..=max_day).suffix(" d"))
                .changed();
            ui.separator();
            if secondary_button(ui, "Exportar PDF").clicked() {
                export_kind = Some("pdf");
            }
            if secondary_button(ui, "Exportar SVG").clicked() {
                export_kind = Some("svg");
            }
            if secondary_button(ui, "Exportar CSV").clicked() {
                export_kind = Some("csv");
            }
        });
        ui.separator();

        // Compact form to add a new progress entry
        ui.label(RichText::new("Novo registro no diário de obra").font(strong(12.5)));
        ui.horizontal_wrapped(|ui| {
            ui.label("Etapa:");
            egui::ComboBox::from_id_salt("progress_stage_combo")
                .selected_text(form_stage.short_label())
                .show_ui(ui, |ui| {
                    for st in ConstructionStage::ALL {
                        ui.selectable_value(&mut form_stage, st, st.label());
                    }
                });
            ui.label("Data:");
            ui.add(egui::TextEdit::singleline(&mut form_date).desired_width(88.0));
            ui.label("Avanço:");
            ui.add(
                egui::DragValue::new(&mut form_pct)
                    .range(0.0..=100.0)
                    .speed(1.0)
                    .max_decimals(0)
                    .suffix(" %"),
            );
            ui.label("Gasto (R$):");
            ui.add(
                egui::DragValue::new(&mut form_cost)
                    .range(0.0..=5_000_000.0)
                    .speed(50.0)
                    .max_decimals(2),
            );
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Nota:");
            ui.add(
                egui::TextEdit::singleline(&mut form_note)
                    .hint_text(
                        "Ex.: Sapatas concretadas, clima bom, entrega de 40 sacos de cimento",
                    )
                    .desired_width(360.0),
            );
            if secondary_button(ui, &format!("Anexar fotos ({})", form_photos.len())).clicked() {
                pick_photos = true;
            }
            if secondary_button(ui, "Registrar avanço").clicked() {
                add_clicked = true;
            }
        });
        if !form_photos.is_empty() {
            ui.label(
                RichText::new(format!("Fotos anexadas: {}", form_photos.join(" · ")))
                    .size(11.0)
                    .color(MUTED),
            );
        }
        ui.separator();

        egui::ScrollArea::both()
            .id_salt("site_progress_scroll")
            .show(ui, |ui| {
                ui.label(RichText::new("1. Previsto × Realizado por etapa").font(strong(13.0)));
                egui::Grid::new("progress_stages_grid")
                    .striped(true)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        for h in [
                            "Etapa",
                            "Previsto",
                            "Realizado",
                            "Desvio",
                            "Situação",
                            "Gasto / Orçado",
                            "Fotos",
                        ] {
                            ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                        }
                        ui.end_row();
                        for st in &report.stages {
                            let col = match st.status {
                                StageProgressStatus::Completed | StageProgressStatus::Ahead => {
                                    egui::Color32::from_rgb(27, 94, 32)
                                }
                                StageProgressStatus::Behind => {
                                    egui::Color32::from_rgb(183, 28, 28)
                                }
                                StageProgressStatus::OnTrack => {
                                    egui::Color32::from_rgb(21, 101, 192)
                                }
                                StageProgressStatus::NotStarted => MUTED,
                            };
                            ui.label(RichText::new(st.stage.short_label()).font(strong(11.5)));
                            ui.label(format!("{:.0} %", st.planned_pct));
                            ui.label(
                                RichText::new(format!("{:.0} %", st.actual_pct)).font(strong(11.5)),
                            );
                            ui.label(format!("{:+.0} pp", st.variance_pct));
                            ui.label(RichText::new(st.status.label()).color(col));
                            ui.label(format!(
                                "{} / {}",
                                format_brl(st.actual_cost_brl),
                                format_brl(st.budget_brl)
                            ));
                            ui.label(st.photo_count.to_string());
                            ui.end_row();
                        }
                    });

                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!("2. Registros do diário ({})", report.entries.len()))
                        .font(strong(13.0)),
                );
                if report.entries.is_empty() {
                    ui.label(
                        RichText::new(
                            "Nenhum registro ainda. Use o formulário acima para registrar avanço, gastos e fotos do canteiro.",
                        )
                        .color(MUTED),
                    );
                } else {
                    for e in report.entries.iter().rev() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "#{} · {} · {} ({:.0} %)",
                                    e.id,
                                    e.date,
                                    e.stage.short_label(),
                                    e.reported_pct
                                ))
                                .font(strong(11.5)),
                            );
                            if let Some(c) = e.actual_cost_brl {
                                ui.label(RichText::new(format_brl(c)).color(TEXT_2));
                            }
                            if !e.photo_paths.is_empty() {
                                ui.label(
                                    RichText::new(format!("📷 {} foto(s)", e.photo_paths.len()))
                                        .size(11.0)
                                        .color(TEXT_2),
                                )
                                .on_hover_text(e.photo_paths.join("\n"));
                            }
                            if ui.small_button("Remover").clicked() {
                                remove_id = Some(e.id);
                            }
                        });
                        if !e.note.is_empty() {
                            ui.label(RichText::new(&e.note).size(11.5).color(TEXT_2));
                        }
                        ui.add_space(3.0);
                    }
                }
            });

        if pick_photos
            && let Some(paths) = rfd::FileDialog::new()
                .add_filter("Imagens", &["jpg", "jpeg", "png", "webp"])
                .pick_files()
        {
            for p in paths {
                let s = p.display().to_string();
                if !form_photos.contains(&s) {
                    form_photos.push(s);
                }
            }
        }
        if ref_day_changed {
            let before = self.project.clone();
            self.project.construction.progress_reference_day = ref_day;
            self.record_change(before);
        }
        if add_clicked {
            let before = self.project.clone();
            let cost_opt = (form_cost > 0.0).then_some(form_cost);
            match add_progress_entry(
                &mut self.project,
                form_date.clone(),
                form_stage,
                form_pct,
                form_note.clone(),
                form_photos.clone(),
                cost_opt,
            ) {
                Ok(id) => {
                    self.record_change(before);
                    self.message = format!("Registro #{id} adicionado ao diário de obra.");
                    form_note.clear();
                    form_photos.clear();
                    form_cost = 0.0;
                }
                Err(err) => self.message = err,
            }
        }
        if let Some(id) = remove_id {
            let before = self.project.clone();
            if remove_progress_entry(&mut self.project, id) {
                self.record_change(before);
                self.message = format!("Registro #{id} removido do diário de obra.");
            }
        }
        if let Some(kind) = export_kind {
            let (label, ext, filename) = match kind {
                "csv" => ("Planilha CSV", "csv", "diario-obra.csv"),
                "pdf" => ("Documento PDF", "pdf", "diario-obra.pdf"),
                _ => ("Desenho SVG", "svg", "diario-obra.svg"),
            };
            if let Some(path) = rfd::FileDialog::new()
                .add_filter(label, &[ext])
                .set_file_name(filename)
                .save_file()
            {
                let res = match kind {
                    "csv" => std::fs::write(&path, progress_log_csv(&self.project))
                        .map_err(|e| e.to_string()),
                    "pdf" => {
                        let svg = progress_log_svg(&self.project);
                        crate::domain::views::export_view_pdf(&svg, &path)
                    }
                    _ => {
                        let svg = progress_log_svg(&self.project);
                        crate::domain::views::export_view_svg(&svg, &path)
                    }
                };
                self.message = match res {
                    Ok(()) => format!("Diário de obra exportado em {}", path.display()),
                    Err(error) => format!("Erro ao exportar diário de obra: {error}"),
                };
            }
        }

        ui.ctx().data_mut(|d| {
            d.insert_temp(stage_id, form_stage);
            d.insert_temp(date_id, form_date);
            d.insert_temp(pct_id, form_pct);
            d.insert_temp(cost_id, form_cost);
            d.insert_temp(note_id, form_note);
            d.insert_temp(photos_id, form_photos);
        });
    }
}
