//! Electrical design in the interface: the device section of the properties
//! panel, the "Projeto elétrico" window (NBR 5410 load forecast and load
//! schedule) and the design commands.

use eframe::egui::{self, Color32, RichText, Ui};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{hint, primary_button, property_label, secondary_button, section};
use crate::domain::electrical::{CircuitKind, DeviceRole};
use crate::domain::fixtures::Fixture;
use crate::domain::layers::LayerCategory;

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

impl FloorPlanApp {
    /// Adds the points NBR 5410 asks for and each room still lacks.
    pub(crate) fn generate_electrical_points(&mut self) {
        let previous = self.project.clone();
        let report = self.project.generate_electrical_points();
        if report.total() == 0 {
            self.message = "Todos os cômodos já atendem ao mínimo da NBR 5410.".to_owned();
            return;
        }
        if self.record_change(previous) {
            self.message = format!(
                "Pontos gerados: {} luz(es), {} tomada(s), {} interruptor(es), {} TUE. Use Circuitos para numerá-los.",
                report.lights, report.outlets, report.switches, report.specific
            );
        }
    }

    /// Numbers every circuit again, following NBR 5410 good practice.
    pub(crate) fn assign_electrical_circuits(&mut self) {
        let previous = self.project.clone();
        let circuits = self.project.assign_circuits();
        if circuits == 0 {
            self.message = "Não há pontos de luz ou tomadas para distribuir.".to_owned();
            return;
        }
        if self.record_change(previous) {
            self.message =
                format!("{circuits} circuito(s) distribuído(s). Veja o Quadro de cargas.");
        }
    }

    pub(crate) fn set_electrical_voltage(&mut self, voltage: f64) {
        if (self.project.electrical.voltage_v - voltage).abs() < 1.0 {
            return;
        }
        let previous = self.project.clone();
        self.project.electrical.voltage_v = voltage;
        if self.record_change(previous) {
            self.message = format!("Tensão dos circuitos: {voltage:.0} V.");
        }
    }

    pub(crate) fn toggle_electrical_layer(&mut self) {
        let previous = self.project.clone();
        self.project.ensure_default_layers();
        if let Some(layer) = self
            .project
            .layers
            .iter_mut()
            .find(|layer| layer.category == Some(LayerCategory::Electrical))
        {
            layer.visible = !layer.visible;
        }
        self.record_change(previous);
    }

    /// The device's circuit, power and height in the properties panel.
    pub(super) fn electrical_device_section(&mut self, ui: &mut Ui, fixture: &Fixture) {
        let Some(role) = fixture.kind.role() else {
            return;
        };
        let data = fixture
            .electrical
            .or_else(|| fixture.kind.default_electrical());
        let Some(data) = data else {
            return;
        };
        let mut circuit = data.circuit.unwrap_or(0);
        let mut power = data.power_va;
        let mut height = data.height_m;
        let mut changed = false;
        section(ui, "electrical_device", "Instalação elétrica", |ui| {
            egui::Grid::new("electrical_device_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .min_col_width(96.0)
                .show(ui, |ui| {
                    if !matches!(role, DeviceRole::Panel | DeviceRole::Data) {
                        property_label(ui, "Circuito");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut circuit)
                                    .range(0..=99)
                                    .speed(0.0)
                                    .custom_formatter(|n, _| {
                                        if n < 0.5 {
                                            "sem circuito".to_owned()
                                        } else {
                                            format!("C{n:.0}")
                                        }
                                    }),
                            )
                            .changed();
                        ui.end_row();
                    }
                    if !matches!(
                        role,
                        DeviceRole::Switch | DeviceRole::Panel | DeviceRole::Data
                    ) {
                        property_label(ui, "Potência");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut power)
                                    .range(0.0..=50_000.0)
                                    .speed(0.0)
                                    .suffix(" VA"),
                            )
                            .changed();
                        ui.end_row();
                    }
                    if fixture.kind.mounts_on_wall() {
                        property_label(ui, "Altura");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut height)
                                    .range(0.0..=6.0)
                                    .speed(0.0)
                                    .max_decimals(2)
                                    .suffix(" m"),
                            )
                            .changed();
                        ui.end_row();
                    }
                    if let Some(room) = self.project.room_holding(fixture.center) {
                        property_label(ui, "Cômodo");
                        ui.label(room.display_name());
                        ui.end_row();
                    }
                });
            ui.add_space(6.0);
            hint(
                ui,
                Icon::Info,
                "Digite e tecle Enter. O Quadro de cargas (aba Elétrica) dimensiona fio e disjuntor de cada circuito.",
                ACCENT,
            );
        });
        if changed {
            let previous = self.project.clone();
            let result = self.project.update_electrical(
                fixture.id,
                Some((circuit > 0).then_some(circuit)),
                Some(power),
                Some(height),
            );
            match result {
                Ok(()) => {
                    self.record_change(previous);
                }
                Err(error) => {
                    self.project = previous;
                    self.message = error;
                }
            }
        }
    }

    /// The "Projeto elétrico" window: forecast per room and load schedule.
    pub(super) fn electrical_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Electrical {
            return;
        }
        let mut open = true;
        let mut generate = false;
        let mut distribute = false;
        let mut voltage = None;
        let forecast = self.project.electrical_forecast();
        let schedule = self.project.load_schedule();
        egui::Window::new("Projeto elétrico · Quadro de cargas (NBR 5410)")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(760.0)
            .default_height(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Previsão mínima de cargas por cômodo, circuitos com corrente, seção do fio e disjuntor. Confira sempre o projeto com um profissional habilitado.",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    generate = primary_button(ui, "Gerar pontos mínimos").clicked();
                    distribute = secondary_button(ui, "Distribuir circuitos").clicked();
                    ui.separator();
                    ui.label("Tensão:");
                    for value in [127.0, 220.0] {
                        let selected = (self.project.electrical.voltage_v - value).abs() < 1.0;
                        if ui.selectable_label(selected, format!("{value:.0} V")).clicked() {
                            voltage = Some(value);
                        }
                    }
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new("Previsão por cômodo").font(strong(13.0)));
                    egui::Grid::new("electrical_forecast")
                        .striped(true)
                        .spacing([14.0, 6.0])
                        .show(ui, |ui| {
                            for title in [
                                "Cômodo", "Tipo", "Área", "Perímetro", "Luz mín.", "Luz", "Tomadas mín.", "Tomadas", "Situação",
                            ] {
                                ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
                            }
                            ui.end_row();
                            for room in &forecast {
                                let missing = room.missing();
                                ui.label(&room.name);
                                ui.label(room.class.label());
                                ui.label(format!("{} m²", decimal(room.area_m2, 2)));
                                ui.label(format!("{} m", decimal(room.perimeter_m, 2)));
                                ui.label(format!("{:.0} VA", room.min_lighting_va));
                                ui.label(format!("{} · {:.0} VA", room.lights, room.lighting_va));
                                ui.label(format!("{} · {:.0} VA", room.min_outlets, room.min_outlet_va));
                                ui.label(format!("{} · {:.0} VA", room.outlets, room.outlet_va));
                                if missing.is_empty() {
                                    ui.label(RichText::new("Atende").color(SUCCESS));
                                } else {
                                    ui.label(RichText::new(missing.join("; ")).color(WARNING));
                                }
                                ui.end_row();
                            }
                        });
                    ui.add_space(12.0);
                    ui.label(RichText::new("Quadro de cargas").font(strong(13.0)));
                    if schedule.circuits.is_empty() {
                        ui.label(
                            RichText::new("Nenhum circuito ainda: gere os pontos e distribua os circuitos.")
                                .color(MUTED),
                        );
                    } else {
                        egui::Grid::new("electrical_schedule")
                            .striped(true)
                            .spacing([14.0, 6.0])
                            .show(ui, |ui| {
                                for title in [
                                    "Circuito", "Tipo", "Cômodos", "Pontos", "Potência", "Tensão", "Corrente", "Fio", "Disjuntor", "DR",
                                ] {
                                    ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
                                }
                                ui.end_row();
                                for row in &schedule.circuits {
                                    let tone = if row.kind == CircuitKind::Mixed { WARNING } else { TEXT };
                                    ui.label(RichText::new(format!("C{}", row.number)).font(strong(12.5)));
                                    ui.label(RichText::new(row.kind.label()).color(tone));
                                    ui.label(row.rooms.join(", "));
                                    ui.label(row.points.to_string());
                                    ui.label(format!("{:.0} VA", row.power_va));
                                    ui.label(format!("{:.0} V", row.voltage_v));
                                    ui.label(format!("{} A", decimal(row.current_a, 1)));
                                    ui.label(row.wire_mm2.map_or("—".to_owned(), |w| {
                                        format!("{} mm²", decimal(w, if w < 2.0 || w == 2.5 { 1 } else { 0 }))
                                    }));
                                    ui.label(row.breaker_a.map_or("—".to_owned(), |b| format!("{b:.0} A")));
                                    ui.label(if row.needs_rcd { "Sim" } else { "—" });
                                    ui.end_row();
                                }
                            });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!(
                                "Carga instalada: {:.0} VA ({} kVA) em {} circuito(s).",
                                schedule.total_va,
                                decimal(schedule.total_va / 1000.0, 2),
                                schedule.circuits.len()
                            ))
                            .font(strong(12.5))
                            .color(ACCENT_TEXT),
                        );
                    }
                    if !schedule.warnings.is_empty() {
                        ui.add_space(10.0);
                        ui.label(RichText::new("Atenção").font(strong(13.0)).color(WARNING));
                        for warning in &schedule.warnings {
                            ui.label(RichText::new(format!("• {warning}")).color(Color32::from_rgb(150, 90, 10)));
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Fios de cobre com isolação de PVC, método B1, 2 condutores carregados a 30 °C. Seções mínimas: 1,5 mm² para iluminação e 2,5 mm² para tomadas. DR de 30 mA em áreas molhadas e externas.",
                        )
                        .size(11.0)
                        .color(MUTED),
                    );
                });
            });
        if generate {
            self.generate_electrical_points();
        }
        if distribute {
            self.assign_electrical_circuits();
        }
        if let Some(value) = voltage {
            self.set_electrical_voltage(value);
        }
        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
