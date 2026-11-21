use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::widgets::{primary_button, secondary_button};
use crate::domain::notes::format_elevation;

impl FloorPlanApp {
    pub(super) fn floors_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Floors {
            return;
        }
        let mut open = true;

        egui::Window::new("Pavimentos, Níveis e Referência (Underlay)")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(640.0)
            .default_height(480.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Gerencie múltiplos pavimentos (subsolo, térreo, superior) com nível, pé-direito e referência translúcida do pavimento inferior.",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
                ui.add_space(6.0);

                ui.horizontal_wrapped(|ui| {
                    if primary_button(ui, "+ Pavimento superior").clicked() {
                        let prev = self.project.clone();
                        self.project.add_floor("", true, false);
                        self.clear_selection();
                        self.record_change(prev);
                    }
                    if secondary_button(ui, "+ Superior (copiar paredes)").clicked() {
                        let prev = self.project.clone();
                        self.project.add_floor("", true, true);
                        self.clear_selection();
                        self.record_change(prev);
                    }
                    if secondary_button(ui, "+ Subsolo").clicked() {
                        let prev = self.project.clone();
                        self.project.add_floor("", false, false);
                        self.clear_selection();
                        self.record_change(prev);
                    }
                });

                ui.add_space(6.0);
                if ui
                    .checkbox(
                        &mut self.project.floors.show_underlay,
                        "Mostrar referência do pavimento de baixo (underlay fantasma)",
                    )
                    .changed()
                {
                    self.mark_edited_outside_history();
                }

                let total_area = self.project.total_multistorey_constructed_area();
                ui.label(
                    RichText::new(
                        format!(
                            "Área construída total (todos os pavimentos): {total_area:.2} m²"
                        )
                        .replace('.', ","),
                    )
                    .size(12.0)
                    .color(ACCENT_TEXT),
                );
                ui.add_space(6.0);
                ui.separator();

                if self.project.floors.levels.is_empty() {
                    self.project.ensure_default_floor();
                }

                let mut switch_to = None;
                let mut remove_id = None;
                let mut meta_update: Option<(u64, String, f64, f64, f64)> = None;
                let active_id = self.project.floors.active_floor_id;
                let count = self.project.floors.levels.len();

                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        // Display highest floor on top (architectural section order).
                        let snapshot: Vec<_> = self
                            .project
                            .floors
                            .levels
                            .iter()
                            .rev()
                            .cloned()
                            .collect();
                        for level in snapshot {
                            let is_active = Some(level.id) == active_id;
                            egui::Frame::new()
                                .fill(if is_active { ACCENT_SOFTER } else { PANEL_ALT })
                                .stroke(Stroke::new(
                                    1.0,
                                    if is_active { ACCENT } else { BORDER },
                                ))
                                .corner_radius(8)
                                .inner_margin(Margin::same(10))
                                .show(ui, |ui| {
                                    let mut name = level.name.clone();
                                    let mut elev = level.elevation_m;
                                    let mut ceiling = level.ceiling_height_m;
                                    let mut slab = level.slab_thickness_m;
                                    let mut edited = false;

                                    ui.horizontal(|ui| {
                                        if ui
                                            .selectable_label(
                                                is_active,
                                                RichText::new(format!(
                                                    "{} · Nível {}",
                                                    level.name,
                                                    format_elevation(level.elevation_m)
                                                ))
                                                .font(strong(12.5)),
                                            )
                                            .clicked()
                                            && !is_active
                                        {
                                            switch_to = Some(level.id);
                                        }
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if count > 1 && ui.small_button("Excluir").clicked()
                                                {
                                                    remove_id = Some(level.id);
                                                }
                                                if !is_active
                                                    && ui.small_button("Ativar").clicked()
                                                {
                                                    switch_to = Some(level.id);
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(4.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label("Nome:");
                                        edited |= ui
                                            .add(
                                                egui::TextEdit::singleline(&mut name)
                                                    .desired_width(130.0),
                                            )
                                            .changed();
                                        ui.label("Nível:");
                                        edited |= ui
                                            .add(
                                                egui::DragValue::new(&mut elev)
                                                    .range(-20.0..=100.0)
                                                    .speed(0.05)
                                                    .suffix(" m"),
                                            )
                                            .changed();
                                        ui.label("Pé-direito:");
                                        edited |= ui
                                            .add(
                                                egui::DragValue::new(&mut ceiling)
                                                    .range(2.0..=12.0)
                                                    .speed(0.05)
                                                    .suffix(" m"),
                                            )
                                            .changed();
                                        ui.label("Laje:");
                                        edited |= ui
                                            .add(
                                                egui::DragValue::new(&mut slab)
                                                    .range(0.08..=0.80)
                                                    .speed(0.01)
                                                    .suffix(" m"),
                                            )
                                            .changed();
                                    });
                                    if edited {
                                        meta_update =
                                            Some((level.id, name, elev, ceiling, slab));
                                    }
                                });
                            ui.add_space(6.0);
                        }
                    });

                if let Some((id, name, elev, ceiling, slab)) = meta_update
                    && self
                        .project
                        .update_floor_metadata(id, &name, elev, ceiling, slab)
                        .is_ok()
                {
                    self.mark_edited_outside_history();
                }
                if let Some(id) = switch_to
                    && self.project.switch_floor(id).is_ok()
                {
                    self.active_tab_floor_id = self.project.floors.active_floor_id;
                    self.clear_selection();
                    self.sync_inspector_fields();
                    if let Some(level) = self.project.active_floor() {
                        self.message = format!("Pavimento ativo: {}", level.label_with_elevation());
                    }
                }
                if let Some(id) = remove_id {
                    let prev = self.project.clone();
                    if self.project.remove_floor(id).is_ok() {
                        self.clear_selection();
                        self.record_change(prev);
                    }
                }
            });

        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
