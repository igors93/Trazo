//! Additional tabs inside the "Projeto elétrico" window for Section F
//! (Electrical, beyond the basics):
//! - #124 Electrical shopping list (wires by section & colour, conduits, boxes,
//!   breakers, RCDs and DPS).

use eframe::egui::{self, Color32, RichText, Ui};

use super::super::{FloorPlanApp, theme::*};
use super::widgets::secondary_button;
use crate::domain::distribution::section_label;
use crate::domain::electrical::lighting::{
    SurfaceReflectance, lighting_report_csv, lighting_report_svg,
};
use crate::domain::electrical::shopping_list::{
    DEFAULT_ELECTRICAL_WASTE_PCT, electrical_shopping_list_csv, electrical_shopping_list_svg,
};
use crate::domain::verdict::Light;

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

impl FloorPlanApp {
    /// Tab 3 (#124): Electrical shopping list.
    pub(super) fn electrical_shopping_list_tab(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let waste_id = egui::Id::unique("trazo_elec_waste_pct");
        let mut waste_pct: f64 = ctx
            .data(|d| d.get_temp(waste_id))
            .unwrap_or(DEFAULT_ELECTRICAL_WASTE_PCT);

        let mut export_kind: Option<&'static str> = None;
        let list = self.project.electrical_shopping_list(waste_pct);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Lista de compras elétrica (NBR 5410): cabos por bitola e cor, eletrodutos, caixas, quadros, disjuntores, DR de 30 mA, DPS e acabamentos.",
                )
                .size(12.0)
                .color(TEXT_2),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Perda / sobra:");
                ui.add(
                    egui::DragValue::new(&mut waste_pct)
                        .range(0.0..=30.0)
                        .speed(0.5)
                        .suffix(" %"),
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

            for warning in &list.warnings {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("• {warning}"))
                        .size(11.5)
                        .color(Color32::from_rgb(150, 90, 10)),
                );
            }

            ui.add_space(8.0);
            ui.label(RichText::new("1. Cabos flexíveis de cobre 750 V (por bitola e cor)").font(strong(13.0)));
            if list.wires.is_empty() {
                ui.label(
                    RichText::new("Nenhum circuito lançado ainda: gere pontos, circuitos e eletrodutos.")
                        .color(MUTED),
                );
            } else {
                egui::Grid::new("elec_shopping_wires")
                    .striped(true)
                    .spacing([14.0, 5.0])
                    .show(ui, |ui| {
                        for h in ["Bitola", "Função / Cor (NBR 5410)", "Líquido", "Com perda", "Rolos 100 m"] {
                            ui.label(RichText::new(h).font(strong(12.0)).color(TEXT_2));
                        }
                        ui.end_row();
                        for w in &list.wires {
                            ui.label(RichText::new(format!("{} mm²", section_label(w.section_mm2))).font(strong(12.0)));
                            ui.label(w.color.label());
                            ui.label(format!("{} m", decimal(w.net_length_m, 1)));
                            ui.label(RichText::new(format!("{} m", decimal(w.total_length_m, 1))).font(strong(12.0)));
                            ui.label(format!("{} rolo(s)", w.rolls_100m));
                            ui.end_row();
                        }
                    });
                ui.label(
                    RichText::new(format!(
                        "Total de cabos: {} m líquido → {} m com {:.0} % de perda.",
                        decimal(list.total_wire_net_m, 1),
                        decimal(list.total_wire_m, 1),
                        list.waste_pct
                    ))
                    .size(12.0)
                    .color(ACCENT_TEXT),
                );
            }

            ui.add_space(10.0);
            ui.label(RichText::new("2. Eletrodutos").font(strong(13.0)));
            if list.conduits.is_empty() {
                ui.label(RichText::new("Nenhum eletroduto lançado.").color(MUTED));
            } else {
                egui::Grid::new("elec_shopping_conduits")
                    .striped(true)
                    .spacing([14.0, 5.0])
                    .show(ui, |ui| {
                        for h in ["Diâmetro", "Trecho", "Líquido", "Com perda", "Rolos 50 m"] {
                            ui.label(RichText::new(h).font(strong(12.0)).color(TEXT_2));
                        }
                        ui.end_row();
                        for c in &list.conduits {
                            ui.label(format!("Ø{} mm", c.diameter_mm));
                            ui.label(c.run.label());
                            ui.label(format!("{} m", decimal(c.net_length_m, 1)));
                            ui.label(RichText::new(format!("{} m", decimal(c.total_length_m, 1))).font(strong(12.0)));
                            ui.label(format!("{} rolo(s)", c.rolls_50m));
                            ui.end_row();
                        }
                    });
            }

            ui.add_space(10.0);
            ui.label(RichText::new("3. Caixas e quadros de distribuição").font(strong(13.0)));
            if list.boxes.is_empty() {
                ui.label(RichText::new("Nenhum ponto inserido.").color(MUTED));
            } else {
                egui::Grid::new("elec_shopping_boxes")
                    .striped(true)
                    .spacing([14.0, 5.0])
                    .show(ui, |ui| {
                        for h in ["Qtd.", "Item", "Especificação"] {
                            ui.label(RichText::new(h).font(strong(12.0)).color(TEXT_2));
                        }
                        ui.end_row();
                        for b in &list.boxes {
                            ui.label(RichText::new(format!("{} un", b.quantity)).font(strong(12.0)));
                            ui.label(&b.kind);
                            ui.label(RichText::new(&b.specification).color(TEXT_2));
                            ui.end_row();
                        }
                    });
            }

            ui.add_space(10.0);
            ui.label(RichText::new("4. Disjuntores, DR (30 mA) e DPS").font(strong(13.0)));
            if list.protections.is_empty() {
                ui.label(RichText::new("Nenhum circuito dimensionado.").color(MUTED));
            } else {
                egui::Grid::new("elec_shopping_protections")
                    .striped(true)
                    .spacing([14.0, 5.0])
                    .show(ui, |ui| {
                        for h in ["Qtd.", "Dispositivo", "Aplicação"] {
                            ui.label(RichText::new(h).font(strong(12.0)).color(TEXT_2));
                        }
                        ui.end_row();
                        for p in &list.protections {
                            ui.label(RichText::new(format!("{} un", p.quantity)).font(strong(12.0)));
                            ui.label(&p.specification);
                            ui.label(RichText::new(&p.notes).color(TEXT_2));
                            ui.end_row();
                        }
                    });
            }

            if !list.devices.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new("5. Acabamentos (tomadas, interruptores e pontos de luz)").font(strong(13.0)));
                egui::Grid::new("elec_shopping_devices")
                    .striped(true)
                    .spacing([14.0, 5.0])
                    .show(ui, |ui| {
                        for h in ["Qtd.", "Item"] {
                            ui.label(RichText::new(h).font(strong(12.0)).color(TEXT_2));
                        }
                        ui.end_row();
                        for d in &list.devices {
                            ui.label(RichText::new(format!("{} un", d.quantity)).font(strong(12.0)));
                            ui.label(&d.specification);
                            ui.end_row();
                        }
                    });
            }

            ui.add_space(8.0);
            for note in &list.assumptions {
                ui.label(RichText::new(note).size(11.0).color(MUTED));
            }
        });

        ctx.data_mut(|d| d.insert_temp(waste_id, waste_pct));

        if let Some(kind) = export_kind {
            self.export_electrical_shopping_list(kind, waste_pct);
        }
    }

    fn export_electrical_shopping_list(&mut self, kind: &str, waste_pct: f64) {
        let (label, ext) = match kind {
            "csv" => ("Planilha CSV", "csv"),
            "pdf" => ("Documento PDF", "pdf"),
            _ => ("Desenho SVG", "svg"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(format!("lista-materiais-eletricos.{ext}"))
            .save_file()
        else {
            return;
        };
        let result = match kind {
            "csv" => std::fs::write(&path, electrical_shopping_list_csv(&self.project, waste_pct))
                .map_err(|e| e.to_string()),
            "pdf" => {
                let markup = electrical_shopping_list_svg(&self.project, waste_pct);
                crate::domain::views::export_view_pdf(&markup, &path)
            }
            _ => {
                let markup = electrical_shopping_list_svg(&self.project, waste_pct);
                crate::domain::views::export_view_svg(&markup, &path)
            }
        };
        self.message = match result {
            Ok(()) => format!("Lista de materiais elétricos exportada em {}", path.display()),
            Err(error) => format!("Erro ao exportar lista elétrica: {error}"),
        };
    }

    /// Tab 4 (#125): Room lighting calculation (Lumen Method, NBR ISO/CIE 8995-1).
    pub(super) fn electrical_lighting_tab(&mut self, ui: &mut Ui) {
        let mut settings = self.project.electrical.lighting;
        let mut changed = false;
        let mut export_kind: Option<&'static str> = None;
        let report = self.project.lighting_report();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Cálculo luminotécnico pelo Método dos Lúmens (ABNT NBR ISO/CIE 8995-1): compara a iluminância média calculada (lux) de cada cômodo com a meta normativa.",
                )
                .size(12.0)
                .color(TEXT_2),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Luminária teto:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut settings.ceiling_lumens)
                            .range(200.0..=20_000.0)
                            .speed(50.0)
                            .suffix(" lm"),
                    )
                    .changed();
                ui.label("Arandela:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut settings.wall_lumens)
                            .range(100.0..=10_000.0)
                            .speed(50.0)
                            .suffix(" lm"),
                    )
                    .changed();
                ui.label("Fd:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut settings.maintenance_factor)
                            .range(0.50..=0.95)
                            .speed(0.01)
                            .max_decimals(2),
                    )
                    .changed();
                ui.label("Plano:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut settings.work_plane_m)
                            .range(0.0..=1.5)
                            .speed(0.05)
                            .max_decimals(2)
                            .suffix(" m"),
                    )
                    .changed();
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Refletâncias:");
                for option in SurfaceReflectance::ALL {
                    if ui
                        .selectable_label(settings.reflectance == option, option.short())
                        .clicked()
                    {
                        settings.reflectance = option;
                        changed = true;
                    }
                }
                ui.separator();
                changed |= ui
                    .checkbox(&mut settings.show_plan_overlay, "Mostrar lux na planta")
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

            ui.add_space(8.0);
            if report.rooms.is_empty() {
                ui.label(RichText::new("Desenhe os cômodos e insira pontos de luz para calcular.").color(MUTED));
            } else {
                egui::Grid::new("elec_lighting_grid")
                    .striped(true)
                    .spacing([12.0, 5.0])
                    .show(ui, |ui| {
                        for h in [
                            "Cômodo",
                            "Área",
                            "K",
                            "Fu",
                            "Pontos",
                            "Fluxo (lm)",
                            "Calculado",
                            "Meta",
                            "Rec.",
                            "Situação",
                        ] {
                            ui.label(RichText::new(h).font(strong(12.0)).color(TEXT_2));
                        }
                        ui.end_row();
                        for r in &report.rooms {
                            let color = match r.light {
                                Light::Green => SUCCESS,
                                Light::Yellow => WARNING,
                                Light::Red => DANGER,
                            };
                            ui.label(RichText::new(&r.room_name).font(strong(12.0)));
                            ui.label(format!("{} m²", decimal(r.area_m2, 1)));
                            ui.label(decimal(r.room_index_k, 2));
                            ui.label(decimal(r.utilization_factor, 2));
                            ui.label(r.luminaires.to_string());
                            ui.label(format!("{:.0}", r.installed_lumens));
                            ui.label(RichText::new(format!("{:.0} lx", r.calculated_lux)).font(strong(12.0)).color(color));
                            ui.label(format!("{:.0} lx", r.target_lux));
                            ui.label(format!("{} un", r.recommended_luminaires));
                            ui.label(RichText::new(r.light.label()).color(color));
                            ui.end_row();
                        }
                    });

                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "Resumo: {} de {} cômodo(s) atendem à meta · Fluxo instalado: {:.0} lm (~{:.0} W em LED).",
                        report.rooms_passing,
                        report.rooms.len(),
                        report.total_installed_lumens,
                        report.total_estimated_watts
                    ))
                    .font(strong(12.0))
                    .color(ACCENT_TEXT),
                );
                for r in report.rooms.iter().filter(|r| r.light == Light::Red) {
                    ui.label(RichText::new(format!("• {}", r.plain_note)).size(11.5).color(DANGER));
                }
            }

            ui.add_space(8.0);
            for note in &report.assumptions {
                ui.label(RichText::new(note).size(11.0).color(MUTED));
            }
        });

        if changed {
            let previous = self.project.clone();
            self.project.electrical.lighting = settings;
            self.record_change(previous);
        }
        if let Some(kind) = export_kind {
            self.export_electrical_lighting(kind);
        }
    }

    fn export_electrical_lighting(&mut self, kind: &str) {
        let (label, ext) = match kind {
            "csv" => ("Planilha CSV", "csv"),
            "pdf" => ("Documento PDF", "pdf"),
            _ => ("Desenho SVG", "svg"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(format!("calculo-luminotecnico.{ext}"))
            .save_file()
        else {
            return;
        };
        let result = match kind {
            "csv" => std::fs::write(&path, lighting_report_csv(&self.project))
                .map_err(|e| e.to_string()),
            "pdf" => {
                let markup = lighting_report_svg(&self.project);
                crate::domain::views::export_view_pdf(&markup, &path)
            }
            _ => {
                let markup = lighting_report_svg(&self.project);
                crate::domain::views::export_view_svg(&markup, &path)
            }
        };
        self.message = match result {
            Ok(()) => format!("Cálculo luminotécnico exportado em {}", path.display()),
            Err(error) => format!("Erro ao exportar luminotécnico: {error}"),
        };
    }
}
