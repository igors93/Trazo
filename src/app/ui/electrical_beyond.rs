//! Additional tabs inside the "Projeto elétrico" window for Section F
//! (Electrical, beyond the basics):
//! - #124 Electrical shopping list (wires by section & colour, conduits, boxes,
//!   breakers, RCDs and DPS).

use eframe::egui::{self, Color32, RichText, Ui};

use super::super::{FloorPlanApp, theme::*};
use super::widgets::secondary_button;
use crate::domain::distribution::section_label;
use crate::domain::distribution::selectivity::{short_circuit_csv, short_circuit_sheet_svg};
use crate::domain::electrical::earthing::{
    EarthingScheme, SpdaSurroundings, earthing_spda_sheet_svg,
};
use crate::domain::electrical::lighting::{
    SurfaceReflectance, lighting_report_csv, lighting_report_svg,
};
use crate::domain::electrical::photovoltaic::{photovoltaic_csv, photovoltaic_sheet_svg};
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

    /// Tab 5 (#126): Photovoltaic pre-sizing (consumption, kWp, roof planes & monthly generation).
    pub(super) fn electrical_pv_tab(&mut self, ui: &mut Ui) {
        let mut settings = self.project.electrical.pv;
        let mut changed = false;
        let mut export_kind: Option<&'static str> = None;
        let report = self.project.photovoltaic_report();
        let mut consumption_val = settings
            .monthly_consumption_kwh
            .unwrap_or(report.monthly_consumption_kwh);
        let mut consumption_edited = false;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Pré-dimensionamento solar fotovoltaico (INPE/CRESESB · NBR 16690 · Lei 14.300): potência em kWp, alocação de módulos nos planos do telhado e geração mensal para a cidade do projeto.",
                )
                .size(12.0)
                .color(TEXT_2),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                changed |= ui
                    .checkbox(&mut settings.enabled, "Ativar fotovoltaico no projeto")
                    .changed();
                if settings.enabled {
                    changed |= ui
                        .checkbox(&mut settings.show_on_plan, "Mostrar placas na planta")
                        .changed();
                }
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
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Consumo mensal:");
                consumption_edited |= ui
                    .add(
                        egui::DragValue::new(&mut consumption_val)
                            .range(30.0..=50_000.0)
                            .speed(10.0)
                            .suffix(" kWh/mês"),
                    )
                    .changed();
                ui.label("Módulo:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut settings.module_wp)
                            .range(250.0..=800.0)
                            .speed(10.0)
                            .suffix(" Wp"),
                    )
                    .changed();
                ui.label("PR:");
                let mut pr_pct = settings.performance_ratio * 100.0;
                if ui
                    .add(
                        egui::DragValue::new(&mut pr_pct)
                            .range(60.0..=88.0)
                            .speed(1.0)
                            .suffix(" %"),
                    )
                    .changed()
                {
                    settings.performance_ratio = pr_pct / 100.0;
                    changed = true;
                }
                ui.label("Compensação:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut settings.target_offset_pct)
                            .range(10.0..=100.0)
                            .speed(5.0)
                            .suffix(" %"),
                    )
                    .changed();
            });
            ui.add_space(6.0);

            let tone = match report.light {
                Light::Green => SUCCESS,
                Light::Yellow => WARNING,
                Light::Red => DANGER,
            };
            ui.label(RichText::new(&report.summary_sentence).font(strong(12.5)).color(tone));
            ui.add_space(6.0);

            egui::Grid::new("elec_pv_summary")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    let row = |ui: &mut Ui, label: &str, value: String| {
                        ui.label(RichText::new(label).color(TEXT_2));
                        ui.label(value);
                        ui.end_row();
                    };
                    row(
                        ui,
                        "Cidade / HSP média",
                        format!(
                            "{} · {} kWh/m²·dia",
                            report.city_label,
                            decimal(report.mean_hsp_kwh_m2_day, 2)
                        ),
                    );
                    row(
                        ui,
                        "Potência / módulos",
                        format!(
                            "{} kWp · {} módulo(s) de {:.0} Wp ({} alocados no telhado)",
                            decimal(report.installed_kwp, 2),
                            report.modules_required,
                            report.settings.module_wp,
                            report.modules_placed
                        ),
                    );
                    row(
                        ui,
                        "Inversor / proteção CA",
                        format!(
                            "{} kW (FDI {}) · Disjuntor {} · Cabo {} mm²",
                            decimal(report.inverter_kw, 1),
                            decimal(report.dc_ac_ratio, 2),
                            report
                                .ac_breaker_a
                                .map_or("—".to_owned(), |b| format!("{b:.0} A")),
                            report
                                .ac_wire_mm2
                                .map_or("—".to_owned(), section_label)
                        ),
                    );
                    row(
                        ui,
                        "Geração anual / mensal",
                        format!(
                            "{:.0} kWh/ano (média {:.0} kWh/mês · {:.0} % do consumo)",
                            report.annual_generation_kwh,
                            report.mean_monthly_generation_kwh,
                            report.offset_percent
                        ),
                    );
                    row(
                        ui,
                        "Área e carga no telhado",
                        format!(
                            "{} m² estimados · sobrecarga {} kN/m² (~15 kg/m²)",
                            decimal(report.required_roof_area_m2, 1),
                            decimal(report.roof_load_kn_m2, 2)
                        ),
                    );
                });

            if !report.planes.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new("Alocação nos planos de telhado").font(strong(12.5)));
                egui::Grid::new("elec_pv_planes")
                    .striped(true)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        for h in ["Telhado", "Orientação", "Incl.", "Área", "Fator", "Cabem", "Alocados"] {
                            ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                        }
                        ui.end_row();
                        for p in &report.planes {
                            ui.label(format!("{} · água {}", p.roof_label, p.plane_index));
                            ui.label(&p.orientation_label);
                            ui.label(format!("{:.0} %", p.slope_percent));
                            ui.label(format!("{} m²", decimal(p.area_m2, 1)));
                            ui.label(decimal(p.orientation_factor, 2));
                            ui.label(format!("{} un", p.capacity_panels));
                            ui.label(RichText::new(format!("{} un", p.placed_panels)).font(strong(11.5)));
                            ui.end_row();
                        }
                    });
            }

            ui.add_space(8.0);
            ui.label(RichText::new("Geração mensal estimada (Jan–Dez)").font(strong(12.5)));
            egui::Grid::new("elec_pv_months")
                .striped(true)
                .spacing([14.0, 4.0])
                .show(ui, |ui| {
                    for h in ["Mês", "HSP (kWh/m²·d)", "Geração (kWh)", "Consumo (kWh)", "Saldo (kWh)"] {
                        ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                    }
                    ui.end_row();
                    for m in &report.months {
                        let bal_color = if m.balance_kwh >= 0.0 { SUCCESS } else { WARNING };
                        ui.label(RichText::new(&m.month).font(strong(11.5)));
                        ui.label(decimal(m.hsp_kwh_m2_day, 2));
                        ui.label(format!("{:.0}", m.generation_kwh));
                        ui.label(format!("{:.0}", m.consumption_kwh));
                        ui.label(RichText::new(format!("{:+.0}", m.balance_kwh)).color(bal_color));
                        ui.end_row();
                    }
                });

            if !report.warnings.is_empty() {
                ui.add_space(6.0);
                for w in &report.warnings {
                    ui.label(
                        RichText::new(format!("• {w}"))
                            .size(11.5)
                            .color(Color32::from_rgb(150, 90, 10)),
                    );
                }
            }
            ui.add_space(6.0);
            for note in &report.assumptions {
                ui.label(RichText::new(note).size(11.0).color(MUTED));
            }
        });

        if consumption_edited {
            settings.monthly_consumption_kwh = Some(consumption_val);
            changed = true;
        }
        if changed {
            let previous = self.project.clone();
            self.project.electrical.pv = settings;
            self.record_change(previous);
        }
        if let Some(kind) = export_kind {
            self.export_electrical_pv(kind);
        }
    }

    fn export_electrical_pv(&mut self, kind: &str) {
        let (label, ext) = match kind {
            "csv" => ("Planilha CSV", "csv"),
            "pdf" => ("Documento PDF", "pdf"),
            _ => ("Desenho SVG", "svg"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(format!("pre-dimensionamento-fotovoltaico.{ext}"))
            .save_file()
        else {
            return;
        };
        let result = match kind {
            "csv" => std::fs::write(&path, photovoltaic_csv(&self.project))
                .map_err(|e| e.to_string()),
            "pdf" => {
                let markup = photovoltaic_sheet_svg(&self.project);
                crate::domain::views::export_view_pdf(&markup, &path)
            }
            _ => {
                let markup = photovoltaic_sheet_svg(&self.project);
                crate::domain::views::export_view_svg(&markup, &path)
            }
        };
        self.message = match result {
            Ok(()) => format!("Estudo fotovoltaico exportado em {}", path.display()),
            Err(error) => format!("Erro ao exportar estudo fotovoltaico: {error}"),
        };
    }

    /// Tab 6 (#127): Multi-board hierarchy, short-circuit estimate (Icc) and breaker selectivity.
    pub(super) fn electrical_selectivity_tab(&mut self, ui: &mut Ui) {
        let mut sc = self.project.electrical.short_circuit;
        let mut changed = false;
        let mut export_kind: Option<&'static str> = None;
        let report = self.project.short_circuit_report();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Quadros principal e secundários (QDG/QDC), estimativa de corrente de curto-circuito (IEC 60909 / NBR 5410 §5.3.5) e seletividade entre disjuntores (§5.3.6).",
                )
                .size(12.0)
                .color(TEXT_2),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Trafo:");
                for kva in [45.0, 75.0, 112.5, 150.0, 225.0, 300.0] {
                    if ui
                        .selectable_label((sc.transformer_kva - kva).abs() < 0.5, format!("{kva:.0} kVA"))
                        .clicked()
                    {
                        sc.transformer_kva = kva;
                        changed = true;
                    }
                }
                ui.separator();
                ui.label("Icn disj.:");
                for icn in [3.0, 4.5, 6.0, 10.0] {
                    if ui
                        .selectable_label((sc.breaker_icn_ka - icn).abs() < 0.05, format!("{} kA", decimal(icn, 1)))
                        .clicked()
                    {
                        sc.breaker_icn_ka = icn;
                        changed = true;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Z% trafo:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut sc.transformer_z_pct)
                            .range(1.5..=8.0)
                            .speed(0.1)
                            .max_decimals(1)
                            .suffix(" %"),
                    )
                    .changed();
                ui.label("Ramal poste:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut sc.service_drop_length_m)
                            .range(2.0..=200.0)
                            .speed(1.0)
                            .suffix(" m"),
                    )
                    .changed();
                ui.label("Medidor→QDG:");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut sc.entrance_length_m)
                            .range(1.0..=150.0)
                            .speed(1.0)
                            .suffix(" m"),
                    )
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
            if report.boards.is_empty() {
                ui.label(
                    RichText::new("Gere os circuitos elétricos primeiro para estimar curto-circuito e seletividade.")
                        .color(MUTED),
                );
            } else {
                ui.label(RichText::new("1. Quadros e níveis de curto-circuito (Icc)").font(strong(13.0)));
                egui::Grid::new("elec_sc_boards")
                    .striped(true)
                    .spacing([12.0, 5.0])
                    .show(ui, |ui| {
                        for h in ["Quadro", "Montante", "Alim.", "Disj.", "Icc 3φ", "Icc 1φ", "Icc máx", "Icn mín.", "Situação"] {
                            ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                        }
                        ui.end_row();
                        for b in &report.boards {
                            let color = match b.light {
                                Light::Green => SUCCESS,
                                Light::Yellow => WARNING,
                                Light::Red => DANGER,
                            };
                            ui.label(RichText::new(&b.label).font(strong(12.0)));
                            ui.label(b.upstream_board.as_deref().unwrap_or("—"));
                            ui.label(format!(
                                "{} mm² · {:.0} m",
                                b.feeder_wire_mm2.map_or("—".to_owned(), section_label),
                                b.feeder_length_m
                            ));
                            ui.label(
                                b.main_breaker_a
                                    .map_or("—".to_owned(), |a| format!("{a:.0} A")),
                            );
                            ui.label(format!("{} kA", decimal(b.icc_3p_ka, 2)));
                            ui.label(format!("{} kA", decimal(b.icc_1p_ka, 2)));
                            ui.label(RichText::new(format!("{} kA", decimal(b.icc_max_ka, 2))).font(strong(12.0)).color(color));
                            ui.label(format!("{} kA", decimal(b.recommended_icn_ka, 1)));
                            ui.label(RichText::new(b.light.label()).color(color));
                            ui.end_row();
                        }
                    });

                ui.add_space(10.0);
                ui.label(RichText::new("2. Seletividade entre disjuntores (Montante → Jusante)").font(strong(13.0)));
                egui::Grid::new("elec_sc_selectivity")
                    .striped(true)
                    .spacing([12.0, 5.0])
                    .show(ui, |ui| {
                        for h in ["Montante", "Jusante", "Razão", "Limite Is", "Curto local", "Resultado"] {
                            ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                        }
                        ui.end_row();
                        for s in &report.selectivity {
                            let color = match s.light {
                                Light::Green => SUCCESS,
                                Light::Yellow => WARNING,
                                Light::Red => DANGER,
                            };
                            ui.label(format!("{} ({:.0} A)", s.upstream_label, s.upstream_in_a));
                            ui.label(format!(
                                "{} ({:.0} A {})",
                                s.downstream_label,
                                s.downstream_in_a,
                                s.downstream_curve.label()
                            ));
                            ui.label(decimal(s.ratio, 2));
                            ui.label(format!("{} kA", decimal(s.selectivity_limit_ka, 2)));
                            ui.label(format!("{} kA", decimal(s.icc_downstream_ka, 2)));
                            ui.label(RichText::new(s.status.label()).color(color));
                            ui.end_row();
                        }
                    });

                ui.add_space(10.0);
                ui.label(RichText::new("3. Fim de linha, suporte térmico dos cabos e queda acumulada").font(strong(13.0)));
                egui::Grid::new("elec_sc_circuits")
                    .striped(true)
                    .spacing([12.0, 5.0])
                    .show(ui, |ui| {
                        for h in ["Circuito", "Quadro", "ΔV total", "Icc mín. fim", "Disparo mag.", "Suporte cabo", "Situação"] {
                            ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                        }
                        ui.end_row();
                        for c in &report.circuits {
                            let color = match c.light {
                                Light::Green => SUCCESS,
                                Light::Yellow => WARNING,
                                Light::Red => DANGER,
                            };
                            ui.label(RichText::new(format!("C{}", c.circuit)).font(strong(12.0)));
                            ui.label(&c.board);
                            ui.label(
                                c.total_drop_percent
                                    .map_or("—".to_owned(), |d| format!("{} %", decimal(d, 2))),
                            );
                            ui.label(format!("{:.0} A", c.icc_min_end_a));
                            ui.label(format!("{:.0} A ({})", c.mag_trip_max_a, c.curve.label()));
                            ui.label(format!("{} kA", decimal(c.cable_thermal_ka, 2)));
                            ui.label(RichText::new(c.light.label()).color(color));
                            ui.end_row();
                        }
                    });
            }

            if !report.warnings.is_empty() {
                ui.add_space(6.0);
                for w in &report.warnings {
                    ui.label(
                        RichText::new(format!("• {w}"))
                            .size(11.5)
                            .color(DANGER),
                    );
                }
            }
            ui.add_space(6.0);
            for note in &report.assumptions {
                ui.label(RichText::new(note).size(11.0).color(MUTED));
            }
        });

        if changed {
            let previous = self.project.clone();
            self.project.electrical.short_circuit = sc;
            self.record_change(previous);
        }
        if let Some(kind) = export_kind {
            self.export_electrical_selectivity(kind);
        }
    }

    fn export_electrical_selectivity(&mut self, kind: &str) {
        let (label, ext) = match kind {
            "csv" => ("Planilha CSV", "csv"),
            "pdf" => ("Documento PDF", "pdf"),
            _ => ("Desenho SVG", "svg"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(format!("curto-circuito-seletividade.{ext}"))
            .save_file()
        else {
            return;
        };
        let result = match kind {
            "csv" => std::fs::write(&path, short_circuit_csv(&self.project))
                .map_err(|e| e.to_string()),
            "pdf" => {
                let markup = short_circuit_sheet_svg(&self.project);
                crate::domain::views::export_view_pdf(&markup, &path)
            }
            _ => {
                let markup = short_circuit_sheet_svg(&self.project);
                crate::domain::views::export_view_svg(&markup, &path)
            }
        };
        self.message = match result {
            Ok(()) => format!("Estudo de curto-circuito exportado em {}", path.display()),
            Err(error) => format!("Erro ao exportar curto-circuito: {error}"),
        };
    }

    /// Tab 7 (#129): Earthing layout (NBR 5410 §6.4) and simplified SPDA risk check (NBR 5419-2).
    pub(super) fn electrical_earthing_tab(&mut self, ui: &mut Ui) {
        let mut cfg = self.project.electrical.earthing;
        let mut changed = false;
        let mut export_pdf: Option<bool> = None;
        let report = self.project.earthing_and_spda_report();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Malha de aterramento e BEP (NBR 5410 §6.4) e triagem simplificada de risco de descargas atmosféricas — SPDA (NBR 5419-2).",
                )
                .size(12.0)
                .color(TEXT_2),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Esquema:");
                for option in EarthingScheme::ALL {
                    if ui
                        .selectable_label(cfg.scheme == option, option.code())
                        .clicked()
                    {
                        cfg.scheme = option;
                        changed = true;
                    }
                }
                ui.separator();
                ui.label("Resistividade do solo (ρ):");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut cfg.soil_resistivity_ohm_m)
                            .range(20.0..=5000.0)
                            .speed(10.0)
                            .suffix(" Ω·m"),
                    )
                    .changed();
                for (lbl, val) in [("Úmido 100", 100.0), ("Comum 200", 200.0), ("Seco 500", 500.0)] {
                    if ui
                        .selectable_label((cfg.soil_resistivity_ohm_m - val).abs() < 5.0, lbl)
                        .clicked()
                    {
                        cfg.soil_resistivity_ohm_m = val;
                        changed = true;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Entorno (Cd):");
                egui::ComboBox::from_id_salt("elec_spda_surroundings")
                    .selected_text(cfg.surroundings.label())
                    .show_ui(ui, |ui| {
                        for option in SpdaSurroundings::ALL {
                            if ui
                                .selectable_label(cfg.surroundings == option, option.label())
                                .clicked()
                            {
                                cfg.surroundings = option;
                                changed = true;
                            }
                        }
                    });
                changed |= ui
                    .checkbox(&mut cfg.overhead_power_line, "Rede aérea")
                    .changed();
                changed |= ui
                    .checkbox(&mut cfg.has_dps_at_entrance, "DPS na entrada")
                    .changed();
                changed |= ui
                    .checkbox(&mut cfg.show_on_plan, "Mostrar BEP/hastes na planta")
                    .changed();
                ui.separator();
                if secondary_button(ui, "Exportar PDF").clicked() {
                    export_pdf = Some(true);
                }
                if secondary_button(ui, "Exportar SVG").clicked() {
                    export_pdf = Some(false);
                }
            });

            ui.add_space(8.0);
            ui.label(RichText::new("1. Aterramento e equipotencialização (NBR 5410 §6.4)").font(strong(13.0)));
            let e_color = match report.earthing_light {
                Light::Green => SUCCESS,
                Light::Yellow => WARNING,
                Light::Red => DANGER,
            };
            ui.label(RichText::new(&report.earthing_note).font(strong(12.0)).color(e_color));
            ui.add_space(4.0);
            egui::Grid::new("elec_earthing_grid")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    let row = |ui: &mut Ui, label: &str, value: String| {
                        ui.label(RichText::new(label).color(TEXT_2));
                        ui.label(value);
                        ui.end_row();
                    };
                    row(
                        ui,
                        "Haste individual (Dwight)",
                        format!(
                            "Copperweld Ø{:.0} mm × {} m → R1 = {} Ω",
                            report.settings.rod_diameter_mm,
                            decimal(report.settings.rod_length_m, 2),
                            decimal(report.single_rod_ohm, 1)
                        ),
                    );
                    row(
                        ui,
                        "Malha dimensionada",
                        format!(
                            "{} hastes espaçadas em {} m → Ra = {} Ω (meta ≤ 25 Ω)",
                            report.rod_count,
                            decimal(report.settings.rod_spacing_m, 1),
                            decimal(report.array_resistance_ohm, 1)
                        ),
                    );
                    row(
                        ui,
                        "Condutores e caixas",
                        format!(
                            "Enterrado #{} mm² nu (ou #{} mm² em eletroduto) · BEP #{} mm² · {} caixas de inspeção Ø300 mm",
                            decimal(report.buried_conductor_mm2, 0),
                            decimal(report.conduit_conductor_mm2, 0),
                            decimal(report.bonding_conductor_mm2, 0),
                            report.inspection_boxes
                        ),
                    );
                });

            ui.add_space(10.0);
            ui.label(RichText::new("2. Triagem de risco de descargas atmosféricas — SPDA (NBR 5419-2)").font(strong(13.0)));
            let s_color = match report.spda_light {
                Light::Green => SUCCESS,
                Light::Yellow => WARNING,
                Light::Red => DANGER,
            };
            ui.label(RichText::new(&report.spda_note).font(strong(12.0)).color(s_color));
            ui.add_space(4.0);
            egui::Grid::new("elec_spda_grid")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    let row = |ui: &mut Ui, label: &str, value: String| {
                        ui.label(RichText::new(label).color(TEXT_2));
                        ui.label(value);
                        ui.end_row();
                    };
                    row(
                        ui,
                        "Local / densidade de raios",
                        format!(
                            "{} · Td = {:.0} dias/ano → Ng = {} descargas/km²·ano",
                            report.city_label,
                            report.thunderstorm_days_year,
                            decimal(report.flash_density_ng, 2)
                        ),
                    );
                    row(
                        ui,
                        "Edificação / área Ad",
                        format!(
                            "{} × {} × {} m → Ad = {:.0} m² · Nd = {:.4}/ano · Nl = {:.4}/ano",
                            decimal(report.building_length_m, 1),
                            decimal(report.building_width_m, 1),
                            decimal(report.building_height_m, 1),
                            report.collection_area_ad_m2,
                            report.annual_direct_flashes_nd,
                            report.annual_line_flashes_nl
                        ),
                    );
                    row(
                        ui,
                        "Risco R1 vs. tolerável RT",
                        format!(
                            "R1 = {:.2e}/ano (limite tolerável RT = 1,00e-5/ano)",
                            report.risk_r1
                        ),
                    );
                    row(
                        ui,
                        "Conclusão da triagem",
                        report
                            .recommended_np
                            .clone()
                            .unwrap_or_else(|| "SPDA estrutural dispensado na triagem · manter DPS Classe II e aterramento".to_owned()),
                    );
                });

            ui.add_space(6.0);
            for note in &report.assumptions {
                ui.label(RichText::new(note).size(11.0).color(MUTED));
            }
        });

        if changed {
            let previous = self.project.clone();
            self.project.electrical.earthing = cfg;
            self.record_change(previous);
        }
        if let Some(pdf) = export_pdf {
            self.export_electrical_earthing(pdf);
        }
    }

    fn export_electrical_earthing(&mut self, pdf: bool) {
        let (label, ext) = if pdf {
            ("Documento PDF", "pdf")
        } else {
            ("Desenho SVG", "svg")
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(format!("aterramento-spda.{ext}"))
            .save_file()
        else {
            return;
        };
        let markup = earthing_spda_sheet_svg(&self.project);
        let result = if pdf {
            crate::domain::views::export_view_pdf(&markup, &path)
        } else {
            crate::domain::views::export_view_svg(&markup, &path)
        };
        self.message = match result {
            Ok(()) => format!("Estudo de aterramento/SPDA exportado em {}", path.display()),
            Err(error) => format!("Erro ao exportar aterramento/SPDA: {error}"),
        };
    }
}
