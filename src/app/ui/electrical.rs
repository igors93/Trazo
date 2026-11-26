//! Electrical design in the interface: the device section of the properties
//! panel, the "Projeto elétrico" window (NBR 5410 load forecast and load
//! schedule, the distribution board and its single-line diagram) and the
//! design commands, conduits included.

use eframe::egui::{self, Color32, RichText, Ui};

use super::super::{DocDialog, FloorPlanApp, Selection, theme::*};
use super::icons::Icon;
use super::widgets::{hint, primary_button, property_label, secondary_button, section};
use crate::domain::conduits::ConduitRun;
use crate::domain::distribution::{
    Supply, UtilityProfile, phase_label, section_label, single_line_svg,
};
use crate::domain::electrical::{CircuitKind, DeviceRole};
use crate::domain::fixtures::Fixture;
use crate::domain::layers::LayerCategory;

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// Width and height of an SVG drawing from its `viewBox`.
fn view_box(markup: &str) -> Option<(f32, f32)> {
    let start = markup.find("viewBox=\"")? + 9;
    let end = start + markup[start..].find('"')?;
    let numbers: Vec<f32> = markup[start..end]
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    (numbers.len() == 4).then(|| (numbers[2], numbers[3]))
}

/// An SVG drawing as a texture, rendered again only when it changes; `name`
/// keeps drawings of different windows apart.
pub(super) fn svg_preview(
    ctx: &egui::Context,
    name: &str,
    markup: &str,
) -> Option<(egui::TextureHandle, egui::Vec2)> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    markup.hash(&mut hasher);
    let key = hasher.finish();
    let id = egui::Id::unique(name);
    let (w_mm, h_mm) = view_box(markup)?;
    let width = 1400_u32;
    let height = ((width as f32) * h_mm / w_mm.max(1.0)).round().max(1.0) as u32;
    let size = egui::vec2(w_mm, h_mm);
    if let Some((cached, texture)) =
        ctx.data(|data| data.get_temp::<(u64, egui::TextureHandle)>(id))
        && cached == key
    {
        return Some((texture, size));
    }
    let rgba = crate::io::print::svg_to_rgba(markup, width, height).ok()?;
    let image = egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
    let texture = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, (key, texture.clone())));
    Some((texture, size))
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

    /// Lays conduits from the board to every device not yet reached.
    pub(crate) fn generate_conduits(&mut self) {
        let previous = self.project.clone();
        match self.project.generate_conduits() {
            Ok(0) => {
                self.message = "Todos os pontos já chegam ao quadro por eletrodutos.".to_owned();
            }
            Ok(added) => {
                if self.record_change(previous) {
                    self.message = format!(
                        "{added} eletroduto(s) lançado(s) a partir do quadro, com a fiação de cada trecho."
                    );
                }
            }
            Err(error) => {
                self.project = previous;
                self.message = error;
            }
        }
    }

    /// The selected electrical devices, in the order they were picked.
    fn selected_devices(&self) -> Vec<u64> {
        self.selected()
            .iter()
            .filter_map(|item| match item {
                Selection::Fixture(id) => Some(*id),
                _ => None,
            })
            .filter(|id| {
                self.project
                    .fixtures
                    .iter()
                    .any(|fixture| fixture.id == *id && fixture.kind.is_electrical())
            })
            .collect()
    }

    /// Conduits between the selected devices, or touching the one selected.
    fn selected_conduits(&self) -> Vec<u64> {
        let devices = self.selected_devices();
        if devices.len() >= 2 {
            self.project
                .valid_conduits()
                .filter(|c| devices.contains(&c.from) && devices.contains(&c.to))
                .map(|c| c.id)
                .collect()
        } else {
            self.project.conduits_of(&devices)
        }
    }

    /// Joins the selected devices with conduits, one after the other.
    pub(crate) fn connect_selected_devices(&mut self) {
        let devices = self.selected_devices();
        if devices.len() < 2 {
            self.message =
                "Selecione dois ou mais pontos elétricos (Shift+clique) para ligá-los por eletrodutos."
                    .to_owned();
            return;
        }
        let previous = self.project.clone();
        let mut added = 0;
        let mut refusal = None;
        for pair in devices.windows(2) {
            match self
                .project
                .add_conduit(pair[0], pair[1], ConduitRun::Ceiling)
            {
                Ok(_) => added += 1,
                Err(error) => refusal = Some(error),
            }
        }
        if added == 0 {
            self.project = previous;
            self.message = refusal.unwrap_or_default();
            return;
        }
        if self.record_change(previous) {
            self.message = format!("{added} eletroduto(s) ligando os pontos selecionados.");
        }
    }

    /// Moves the selected conduits between the ceiling and the floor.
    pub(crate) fn toggle_selected_conduit_run(&mut self) {
        let ids = self.selected_conduits();
        if ids.is_empty() {
            self.message =
                "Selecione os pontos nas pontas do eletroduto (ou um ponto para todos os seus)."
                    .to_owned();
            return;
        }
        let to_floor = self
            .project
            .conduits
            .iter()
            .any(|c| ids.contains(&c.id) && c.run == ConduitRun::Ceiling);
        let run = if to_floor {
            ConduitRun::Floor
        } else {
            ConduitRun::Ceiling
        };
        let previous = self.project.clone();
        for id in &ids {
            let _ = self.project.set_conduit_run(*id, run);
        }
        if self.record_change(previous) {
            self.message = format!(
                "{} eletroduto(s) agora no {}.",
                ids.len(),
                if to_floor { "piso" } else { "teto ou parede" }
            );
        }
    }

    /// Removes the selected conduits.
    pub(crate) fn remove_selected_conduits(&mut self) {
        let ids = self.selected_conduits();
        if ids.is_empty() {
            self.message =
                "Selecione os pontos nas pontas do eletroduto (ou um ponto para todos os seus)."
                    .to_owned();
            return;
        }
        let previous = self.project.clone();
        for id in &ids {
            self.project.delete_conduit(*id);
        }
        if self.record_change(previous) {
            self.message = format!("{} eletroduto(s) removido(s).", ids.len());
        }
    }

    pub(crate) fn set_electrical_supply(&mut self, supply: Supply) {
        if self.project.electrical.supply == supply {
            return;
        }
        let previous = self.project.clone();
        self.project.electrical.supply = supply;
        if self.record_change(previous) {
            self.message = format!("Alimentação: {}.", supply.label().to_lowercase());
        }
    }

    /// Saves the single-line diagram as PDF or SVG.
    fn export_single_line(&mut self, pdf: bool) {
        let markup = single_line_svg(&self.project);
        let (label, ext) = if pdf {
            ("Documento PDF", "pdf")
        } else {
            ("Desenho SVG", "svg")
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(format!("diagrama-unifilar.{ext}"))
            .save_file()
        else {
            return;
        };
        let result = if pdf {
            crate::domain::views::export_view_pdf(&markup, &path)
        } else {
            crate::domain::views::export_view_svg(&markup, &path)
        };
        self.message = match result {
            Ok(()) => format!("Diagrama unifilar exportado em {}", path.display()),
            Err(error) => format!("Erro ao exportar o diagrama unifilar: {error}"),
        };
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
                    if let Some(room) = self.project.room_for_fixture(fixture) {
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
        let mut conduits = false;
        let mut voltage = None;
        let mut supply = None;
        let mut utility = None;
        let mut export = None;
        let tab_id = egui::Id::unique("trazo_electrical_tab");
        let mut tab: u8 = ctx.data(|data| data.get_temp(tab_id)).unwrap_or(0);
        let forecast = self.project.electrical_forecast();
        let (schedule, schedule_stale) = self.background.load_schedule_or_schedule(&self.project);
        let _ = schedule_stale;
        let preview = if tab == 1 {
            let markup = single_line_svg(&self.project);
            svg_preview(ctx, "trazo_single_line_preview", &markup)
        } else {
            None
        };
        let wiring = if tab == 1 {
            self.project.conduit_wiring()
        } else {
            Vec::new()
        };
        let unreached = self.project.conduit_tree().unreached.len();
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
                    conduits = secondary_button(ui, "Gerar eletrodutos").clicked();
                    ui.separator();
                    ui.label("Tensão:");
                    for value in [127.0, 220.0] {
                        let selected = (self.project.electrical.voltage_v - value).abs() < 1.0;
                        if ui.selectable_label(selected, format!("{value:.0} V")).clicked() {
                            voltage = Some(value);
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for (index, title) in [
                        "Cargas e circuitos",
                        "Quadro e diagrama unifilar",
                        "Demanda coletiva (Multifamiliar)",
                        "Lista de materiais",
                    ]
                    .iter()
                    .enumerate()
                    {
                        if ui
                            .selectable_label(tab == index as u8, RichText::new(*title).font(strong(12.5)))
                            .clicked()
                        {
                            tab = index as u8;
                        }
                    }
                });
                ui.separator();
                if tab == 3 {
                    self.electrical_shopping_list_tab(ui, ctx);
                } else if tab == 2 {
                    self.collective_demand_tab(ui, ctx);
                } else if tab == 1 {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Alimentação:");
                            for option in Supply::ALL {
                                let selected = self.project.electrical.supply == option;
                                if ui.selectable_label(selected, option.label()).clicked() {
                                    supply = Some(option);
                                }
                            }
                            ui.separator();
                            ui.label("Distribuidora:");
                            egui::ComboBox::from_id_salt("electrical_utility_profile")
                                .selected_text(self.project.electrical.utility.label())
                                .show_ui(ui, |ui| {
                                    for profile in UtilityProfile::ALL {
                                        if ui
                                            .selectable_label(
                                                self.project.electrical.utility == profile,
                                                profile.label(),
                                            )
                                            .clicked()
                                        {
                                            utility = Some(profile);
                                        }
                                    }
                                });
                            ui.separator();
                            if secondary_button(ui, "Exportar PDF").clicked() {
                                export = Some(true);
                            }
                            if secondary_button(ui, "Exportar SVG").clicked() {
                                export = Some(false);
                            }
                        });
                        let rules = self
                            .project
                            .electrical
                            .utility
                            .rules_from_codes(Some(&self.project.codes))
                            .with_overrides(self.project.electrical.custom_rules);
                        let cite = rules.citation;
                        ui.label(
                            RichText::new(format!(
                                "Referência: {} ({}) — {}. Limites: mono ≤ {:.0} kVA, bi ≤ {:.0} kVA, mín. {:.0} A / {:.0} mm². {}",
                                cite.standard,
                                cite.edition,
                                cite.tables,
                                rules.single_phase_max_kva,
                                rules.two_phase_max_kva,
                                rules.min_breaker_a,
                                rules.min_wire_mm2,
                                cite.verification_note
                            ))
                            .size(11.0)
                            .color(MUTED),
                        );
                        let members = self.project.electrical.utility.members();
                        if !members.is_empty() {
                            let names: Vec<&str> =
                                members.iter().map(|(id, _)| id.as_str()).collect();
                            ui.label(
                                RichText::new(format!(
                                    "Distribuidoras/cooperativas cobertas ({}): {}",
                                    members.len(),
                                    names.join(", ")
                                ))
                                .size(10.5)
                                .color(MUTED),
                            );
                        }
                        if schedule.total_va > 0.0
                            && let Some(cat_line) = self
                                .project
                                .electrical
                                .utility
                                .entrance_category_summary(
                                    Some(&self.project.codes),
                                    schedule.total_va / 1000.0,
                                    schedule.demand.demand_va / 1000.0,
                                    self.project.electrical.voltage_v,
                                    schedule.supply.phases().max(1),
                                    None,
                                )
                        {
                            ui.label(
                                RichText::new(format!("Tabela oficial da concessionária: {cat_line}"))
                                    .size(11.5)
                                    .color(ACCENT_TEXT),
                            );
                        }
                        ui.add_space(6.0);
                        let d = schedule.demand;
                        egui::Grid::new("electrical_board")
                            .num_columns(2)
                            .spacing([16.0, 6.0])
                            .show(ui, |ui| {
                                let row = |ui: &mut Ui, label: &str, value: String| {
                                    ui.label(RichText::new(label).color(TEXT_2));
                                    ui.label(value);
                                    ui.end_row();
                                };
                                row(ui, "Ligação em uso", schedule.supply.label().to_owned());
                                row(
                                    ui,
                                    "Demanda provável",
                                    format!(
                                        "{:.0} VA × {} (iluminação e TUG) + {:.0} VA × {} (TUE) = {} kVA",
                                        d.general_va,
                                        decimal(d.general_factor, 2),
                                        d.specific_va,
                                        decimal(d.specific_factor, 2),
                                        decimal(d.demand_va / 1000.0, 2)
                                    ),
                                );
                                let phases = usize::from(schedule.supply.phases().max(1));
                                row(
                                    ui,
                                    "Carga por fase",
                                    (0..phases)
                                        .map(|i| {
                                            format!(
                                                "{} {:.0} VA",
                                                crate::domain::distribution::PHASE_NAMES[i],
                                                schedule.phase_va[i]
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" · "),
                                );
                                if let Some(e) = schedule.entrance {
                                    row(
                                        ui,
                                        "Disjuntor geral",
                                        e.breaker_a.map_or("—".to_owned(), |b| {
                                            format!("{}P {b:.0} A ({} A na fase mais carregada)", phases, decimal(e.current_a, 1))
                                        }),
                                    );
                                    row(
                                        ui,
                                        "Ramal de entrada",
                                        match (e.wire_mm2, e.earth_mm2) {
                                            (Some(w), Some(t)) => format!(
                                                "{phases} fase(s) e neutro de {} mm², terra de {} mm², eletroduto Ø{} mm",
                                                section_label(w),
                                                section_label(t),
                                                e.conduit_mm.map_or("—".to_owned(), |c| c.to_string())
                                            ),
                                            _ => "consulte a distribuidora".to_owned(),
                                        },
                                    );
                                }
                                row(
                                    ui,
                                    "Eletrodutos",
                                    if self.project.conduits.is_empty() {
                                        "nenhum ainda: insira o quadro (QD) e use Gerar eletrodutos".to_owned()
                                    } else {
                                        format!(
                                            "{} trecho(s), {} ponto(s) sem ligação ao quadro",
                                            wiring.len(),
                                            unreached
                                        )
                                    },
                                );
                            });
                        if let Some((texture, size)) = &preview {
                            ui.add_space(8.0);
                            let width = ui.available_width().max(200.0);
                            let height = width * size.y / size.x.max(1.0);
                            ui.add(
                                egui::Image::new(texture)
                                    .fit_to_exact_size(egui::vec2(width, height)),
                            );
                        }
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(
                                "Fatores de demanda usuais das distribuidoras para residências e queda de tensão máxima de 4 % nos circuitos terminais (NBR 5410 6.2.7). Cada distribuidora tem a sua tabela de padrão de entrada: confira a norma local.",
                            )
                            .size(11.0)
                            .color(MUTED),
                        );
                    });
                } else {
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
                                    "Circuito", "Tipo", "Cômodos", "Pontos", "Potência", "Tensão", "Corrente", "Fases", "Fio", "Disjuntor", "DR", "Queda",
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
                                    ui.label(phase_label(&row.phases));
                                    ui.label(row.wire_mm2.map_or("—".to_owned(), |w| {
                                        format!("{} mm²{}", section_label(w), if row.wire_by_drop { " *" } else { "" })
                                    }));
                                    ui.label(row.breaker_a.map_or("—".to_owned(), |b| {
                                        format!("{}{b:.0} A", if row.poles >= 2 { "2P " } else { "" })
                                    }));
                                    ui.label(if row.needs_rcd { "Sim" } else { "—" });
                                    ui.label(row.drop_percent.map_or("—".to_owned(), |d| format!("{} %", decimal(d, 1))));
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
                            "Fios de cobre com isolação de PVC, método B1, 2 condutores carregados a 30 °C. Seções mínimas: 1,5 mm² para iluminação e 2,5 mm² para tomadas. DR de 30 mA em áreas molhadas e externas. * fio aumentado para manter a queda de tensão até 4 % (com o quadro inserido).",
                        )
                        .size(11.0)
                        .color(MUTED),
                    );
                });
                }
            });
        ctx.data_mut(|data| data.insert_temp(tab_id, tab));
        if conduits {
            self.generate_conduits();
        }
        if let Some(option) = supply {
            self.set_electrical_supply(option);
        }
        if let Some(profile) = utility
            && self.project.electrical.utility != profile
        {
            let previous = self.project.clone();
            self.project.electrical.utility = profile;
            let _ = self.project.codes.set_base(
                crate::domain::codes::CodeDiscipline::Electrical,
                if profile == UtilityProfile::Generic {
                    "generic"
                } else {
                    "nbr-5410"
                },
            );
            if profile == UtilityProfile::Generic {
                let _ = self
                    .project
                    .codes
                    .set_amendments(crate::domain::codes::CodeDiscipline::Electrical, Vec::new());
            } else {
                let _ = self.project.codes.set_amendments(
                    crate::domain::codes::CodeDiscipline::Electrical,
                    vec![profile.code_key().to_owned()],
                );
            }
            self.record_change(previous);
            self.message = format!("Distribuidora: {}.", profile.label());
        }
        if let Some(pdf) = export {
            self.export_single_line(pdf);
        }
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

    /// Tab 2: Collective residential building demand (CODI / CEMIG ND-5.2 / Celesc N-321.0003 / Fecoergs / Hidropan).
    fn collective_demand_tab(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let count_id = egui::Id::unique("trazo_coll_count");
        let area_id = egui::Id::unique("trazo_coll_area");
        let light_id = egui::Id::unique("trazo_coll_light");
        let spec_id = egui::Id::unique("trazo_coll_spec");
        let printed_id = egui::Id::unique("trazo_coll_printed");
        let util_id = egui::Id::unique("trazo_coll_util");

        let mut count: usize = ctx.data(|d| d.get_temp(count_id)).unwrap_or(6);
        let mut area_m2: f64 = ctx.data(|d| d.get_temp(area_id)).unwrap_or(70.0);
        let mut common_light_kw: f64 = ctx.data(|d| d.get_temp(light_id)).unwrap_or(2.0);
        let mut special_kva: f64 = ctx.data(|d| d.get_temp(spec_id)).unwrap_or(0.0);
        let mut as_printed: bool = ctx.data(|d| d.get_temp(printed_id)).unwrap_or(false);
        let default_util = if self
            .project
            .electrical
            .utility
            .building_demand_method(Some(&self.project.codes))
            .is_some()
        {
            self.project.electrical.utility
        } else {
            UtilityProfile::Celesc
        };
        let mut chosen_util: UtilityProfile =
            ctx.data(|d| d.get_temp(util_id)).unwrap_or(default_util);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Cálculo de demanda coletiva para pequenos edifícios residenciais (até 3 pavimentos) com tabelas oficiais CODI / CEMIG ND-5.2 / Celesc N-321.0003 / Fecoergs / Hidropan.",
                )
                .size(12.0)
                .color(TEXT_2),
            );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Norma coletiva:").font(strong(12.0)));
                for prof in [
                    UtilityProfile::Celesc,
                    UtilityProfile::Cemig,
                    UtilityProfile::Fecoergs,
                    UtilityProfile::Hidropan,
                ] {
                    if ui
                        .selectable_label(chosen_util == prof, prof.label())
                        .clicked()
                    {
                        chosen_util = prof;
                    }
                }
            });
            ui.add_space(6.0);
            egui::Grid::new("collective_demand_inputs")
                .num_columns(2)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Número de unidades residenciais:");
                    ui.add(egui::DragValue::new(&mut count).range(2..=200));
                    ui.end_row();

                    ui.label("Área média por unidade:");
                    ui.add(
                        egui::DragValue::new(&mut area_m2)
                            .range(15.0..=800.0)
                            .speed(1.0)
                            .suffix(" m²"),
                    );
                    ui.end_row();

                    ui.label("Iluminação de área comum (instalada):");
                    ui.add(
                        egui::DragValue::new(&mut common_light_kw)
                            .range(0.0..=100.0)
                            .speed(0.5)
                            .suffix(" kW"),
                    );
                    ui.end_row();

                    ui.label("Cargas especiais (bombas, elevador, etc.):");
                    ui.add(
                        egui::DragValue::new(&mut special_kva)
                            .range(0.0..=200.0)
                            .speed(0.5)
                            .suffix(" kVA"),
                    );
                    ui.end_row();

                    ui.label("Erratas publicadas em tabelas oficiais:");
                    ui.checkbox(&mut as_printed, "Usar valor impresso literal (comparação)");
                    ui.end_row();
                });

            ui.add_space(8.0);
            if let Some(method) = chosen_util.building_demand_method(Some(&self.project.codes)) {
                let mut common_kw = std::collections::BTreeMap::new();
                if common_light_kw > 0.0 {
                    common_kw.insert("lighting".to_owned(), common_light_kw);
                }
                let input = crate::domain::codes::BuildingDemandInput {
                    dwellings: vec![crate::domain::codes::DwellingGroup { count, area_m2 }],
                    common_installed_kw: common_kw,
                    special_demand: special_kva,
                    as_printed,
                    ..Default::default()
                };
                match crate::domain::codes::calculate_building_demand(&method, &input) {
                    Ok(res) => {
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "Demanda total coletiva: {:.2} {}  ({} · {})",
                                res.total_demand, res.unit, res.standard, chosen_util.label()
                            ))
                            .font(strong(14.0))
                            .color(ACCENT_TEXT),
                        );
                        ui.add_space(4.0);
                        egui::Grid::new("collective_demand_results")
                            .num_columns(2)
                            .spacing([16.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("Demanda unitária por área (d):");
                                ui.label(
                                    format!("{:.2} {}", res.dwelling_demand_d, res.unit)
                                        .replace('.', ","),
                                );
                                ui.end_row();
                                ui.label("Fator de diversidade F(n):");
                                ui.label(
                                    format!("{:.2}", res.diversity_factor_f).replace('.', ","),
                                );
                                ui.end_row();
                                ui.label("Fator de segurança / majoração:");
                                ui.label(format!("{:.2}", res.safety_factor).replace('.', ","));
                                ui.end_row();
                                ui.label("Parcela das unidades:");
                                ui.label(
                                    format!("{:.2} {}", res.dwellings_total_demand, res.unit)
                                        .replace('.', ","),
                                );
                                ui.end_row();
                                ui.label("Parcela de área comum:");
                                ui.label(
                                    format!("{:.2} {}", res.common_demand, res.unit)
                                        .replace('.', ","),
                                );
                                ui.end_row();
                                if let Some(brk) = res.entrance_breaker_a {
                                    ui.label("Disjuntor geral da entrada coletiva:");
                                    ui.label(
                                        RichText::new(format!("{brk:.0} A")).font(strong(12.5)),
                                    );
                                    ui.end_row();
                                }
                            });
                        if let Some(erratum) = &res.erratum_applied {
                            ui.add_space(4.0);
                            ui.label(RichText::new(erratum).size(11.0).color(WARNING));
                        }
                        for w in &res.warnings {
                            ui.label(RichText::new(format!("• {w}")).size(11.0).color(WARNING));
                        }
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("Fonte: {}", res.source))
                                .size(11.0)
                                .color(MUTED),
                        );
                    }
                    Err(err) => {
                        ui.label(RichText::new(err).color(DANGER));
                    }
                }
            }
        });

        ctx.data_mut(|d| {
            d.insert_temp(count_id, count);
            d.insert_temp(area_id, area_m2);
            d.insert_temp(light_id, common_light_kw);
            d.insert_temp(spec_id, special_kva);
            d.insert_temp(printed_id, as_printed);
            d.insert_temp(util_id, chosen_util);
        });
    }
}
