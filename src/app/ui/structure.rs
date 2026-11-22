//! Structure in the interface: the "Estrutura" window (pre-sizing of slabs,
//! beams, columns and footings, and the internal forces of the beams) and
//! the structural commands.

use eframe::egui::{self, Color32, RichText, Ui};

use super::super::{DocDialog, FloorPlanApp, Selection, theme::*};
use super::electrical::svg_preview;
use super::widgets::{primary_button, secondary_button};
use crate::domain::discipline::Discipline;
use crate::domain::fixtures::FixtureKind;
use crate::domain::layers::LayerCategory;
use crate::domain::reinforcement::{ReinforcementReport, steel_schedule_csv};
use crate::domain::structure::{BeamSupport, SlabKind, beam_diagrams_svg, section_cm};

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

fn status(ui: &mut Ui, ok: bool) {
    if ok {
        ui.label(RichText::new("Atende").color(SUCCESS));
    } else {
        ui.label(RichText::new("Revise").color(WARNING));
    }
}

/// An edit made in the window, applied once it is drawn.
enum StructureEdit {
    Slab(u64, SlabKind, f64),
    Beam(u64, f64, f64),
    Settings(crate::domain::structure::StructureSettings),
    Soil(super::foundations::SoilAction),
    Roof(super::roof::RoofAction),
}

#[derive(Clone, Copy, Debug)]
struct SecondaryBeamDraft {
    from_beam: u64,
    to_beam: u64,
    from_t: f64,
    to_t: f64,
    width_cm: f64,
    height_cm: f64,
}

impl SecondaryBeamDraft {
    fn between(from_beam: u64, to_beam: u64) -> Self {
        Self {
            from_beam,
            to_beam,
            from_t: 0.5,
            to_t: 0.5,
            width_cm: 14.0,
            height_cm: 35.0,
        }
    }
}

fn reinforcement_tab(ui: &mut Ui, report: &ReinforcementReport) {
    let heading = |ui: &mut Ui, text: &str| {
        ui.add_space(8.0);
        ui.label(RichText::new(text).font(strong(13.0)));
    };
    let titles = |ui: &mut Ui, titles: &[&str]| {
        for title in titles {
            ui.label(RichText::new(*title).font(strong(12.0)).color(TEXT_2));
        }
        ui.end_row();
    };
    let bars = |set: &crate::domain::reinforcement::BarSet| {
        format!(
            "{} · {} / {} cm²",
            set.notation(),
            decimal(set.required_cm2, 2),
            decimal(set.provided_cm2, 2)
        )
    };
    let mesh = |set: &crate::domain::reinforcement::MeshBars| {
        format!(
            "{} · {} / {} cm²/m",
            set.notation(),
            decimal(set.required_cm2_m, 2),
            decimal(set.provided_cm2_m, 2)
        )
    };

    if report.is_empty() {
        ui.label(
            RichText::new(
                "Nenhuma armadura emitida. Gere a estrutura e resolva os limites indicados.",
            )
            .color(MUTED),
        );
    }
    if !report.beams.is_empty() {
        heading(ui, "Vigas · CA-50");
        egui::Grid::new("reinforcement_beams")
            .striped(true)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                titles(
                    ui,
                    &[
                        "Viga",
                        "Inferior",
                        "Sup. esq.",
                        "Sup. dir.",
                        "Estribos",
                        "Aço",
                    ],
                );
                for beam in &report.beams {
                    ui.label(RichText::new(&beam.label).font(strong(12.5)));
                    ui.label(bars(&beam.bottom));
                    ui.label(bars(&beam.top_left));
                    ui.label(bars(&beam.top_right));
                    ui.label(beam.stirrups.notation());
                    ui.label(format!("{} kg", decimal(beam.mass_kg, 1)));
                    ui.end_row();
                }
            });
    }
    if !report.columns.is_empty() {
        heading(ui, "Pilares · CA-50");
        egui::Grid::new("reinforcement_columns")
            .striped(true)
            .spacing([14.0, 6.0])
            .show(ui, |ui| {
                titles(ui, &["Pilar", "Longitudinal", "Taxa", "Estribos", "Aço"]);
                for column in &report.columns {
                    ui.label(RichText::new(&column.label).font(strong(12.5)));
                    ui.label(bars(&column.longitudinal));
                    ui.label(format!("{} %", decimal(column.steel_ratio * 100.0, 2)));
                    ui.label(column.ties.notation());
                    ui.label(format!("{} kg", decimal(column.mass_kg, 1)));
                    ui.end_row();
                }
            });
    }
    if !report.slabs.is_empty() {
        heading(ui, "Lajes · armadura equivalente por metro");
        egui::Grid::new("reinforcement_slabs")
            .striped(true)
            .spacing([14.0, 6.0])
            .show(ui, |ui| {
                titles(ui, &["Laje", "Cômodo", "Principal", "Distribuição", "Aço"]);
                for slab in &report.slabs {
                    ui.label(RichText::new(&slab.label).font(strong(12.5)));
                    ui.label(&slab.room);
                    ui.label(mesh(&slab.main));
                    ui.label(mesh(&slab.distribution));
                    ui.label(format!("{} kg", decimal(slab.mass_kg, 1)));
                    ui.end_row();
                }
            });
    }
    if !report.footings.is_empty() {
        heading(ui, "Sapatas · malha inferior CA-50");
        egui::Grid::new("reinforcement_footings")
            .striped(true)
            .spacing([14.0, 6.0])
            .show(ui, |ui| {
                titles(ui, &["Sapata", "Direção X", "Direção Y", "Aço"]);
                for footing in &report.footings {
                    ui.label(RichText::new(&footing.label).font(strong(12.5)));
                    ui.label(mesh(&footing.bars_x));
                    ui.label(mesh(&footing.bars_y));
                    ui.label(format!("{} kg", decimal(footing.mass_kg, 1)));
                    ui.end_row();
                }
            });
    }
    if !report.schedule.is_empty() {
        heading(ui, "Tabela de aço");
        egui::Grid::new("steel_schedule")
            .striped(true)
            .spacing([12.0, 5.0])
            .show(ui, |ui| {
                titles(
                    ui,
                    &[
                        "Marca",
                        "Elemento",
                        "Posição",
                        "Ø",
                        "Qtd.",
                        "Unit.",
                        "Total",
                        "Massa",
                    ],
                );
                for row in &report.schedule {
                    ui.label(RichText::new(&row.mark).font(strong(12.0)));
                    ui.label(&row.element);
                    ui.label(&row.position);
                    ui.label(format!("{} mm", decimal(row.diameter_mm, 1)));
                    ui.label(row.quantity.to_string());
                    ui.label(format!("{} m", decimal(row.unit_length_m, 2)));
                    ui.label(format!("{} m", decimal(row.total_length_m, 2)));
                    ui.label(format!("{} kg", decimal(row.mass_kg, 1)));
                    ui.end_row();
                }
            });
        ui.label(
            RichText::new(format!(
                "Massa total estimada: {} kg",
                decimal(report.total_mass_kg(), 1)
            ))
            .font(strong(13.0))
            .color(ACCENT_TEXT),
        );
    }
    if !report.warnings.is_empty() {
        ui.add_space(10.0);
        ui.label(
            RichText::new("Revisões necessárias")
                .font(strong(13.0))
                .color(WARNING),
        );
        for warning in &report.warnings {
            ui.label(RichText::new(format!("• {warning}")).color(WARNING));
        }
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new(
            "Estimativa preliminar pelo método já documentado para a NBR 6118:2023. A edição 2026 ainda exige auditoria; confirme cobrimentos, ancoragens, emendas e todo o detalhamento com engenheiro habilitado.",
        )
        .size(11.0)
        .color(MUTED),
    );
}

impl FloorPlanApp {
    /// Lays out columns, beams, slabs and footings over the walls.
    pub(crate) fn generate_structure(&mut self) {
        if self.project.walls.is_empty() {
            self.message = "Desenhe as paredes antes de gerar a estrutura.".to_owned();
            return;
        }
        if let Some(refusal) = crate::domain::limits::structural_refusal(&self.project) {
            self.message = refusal;
            return;
        }
        let previous = self.project.clone();
        let report = self.project.generate_structure();
        if report.total() == 0 {
            self.message = "A estrutura já cobre todas as paredes e cômodos.".to_owned();
            return;
        }
        self.project.ensure_default_layers();
        if self.record_change(previous) {
            self.message = format!(
                "Estrutura gerada: {} pilar(es), {} viga(s), {} laje(s) e {} sapata(s). Veja o pré-dimensionamento.{}",
                report.columns,
                report.beams,
                report.slabs,
                report.footings,
                if report.skipped > 0 {
                    format!(" {} pilar(es) não couberam no terreno.", report.skipped)
                } else {
                    String::new()
                }
            );
        }
    }

    /// Resizes the footings to what the soil asks for.
    pub(crate) fn size_footings(&mut self) {
        if let Some(refusal) = crate::domain::limits::structural_refusal(&self.project) {
            self.message = refusal;
            return;
        }
        let previous = self.project.clone();
        let changed = self.project.size_footings();
        if changed == 0 {
            self.message = "As sapatas já têm o tamanho que o solo pede.".to_owned();
            return;
        }
        if self.record_change(previous) {
            self.message = format!("{changed} sapata(s) redimensionada(s) pelas cargas.");
        }
    }

    fn selected_columns(&self) -> Vec<u64> {
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
                    .any(|f| f.id == *id && f.kind == FixtureKind::Column)
            })
            .collect()
    }

    /// Joins the selected columns with beams, one after the other.
    pub(crate) fn connect_selected_columns(&mut self) {
        let columns = self.selected_columns();
        if columns.len() < 2 {
            self.message =
                "Selecione dois ou mais pilares (Shift+clique) para ligá-los por vigas.".to_owned();
            return;
        }
        let previous = self.project.clone();
        let mut added = 0;
        let mut refusal = None;
        for pair in columns.windows(2) {
            let centre = |id: u64| {
                self.project
                    .fixtures
                    .iter()
                    .find(|f| f.id == id)
                    .map(|f| (f.center, f.width.min(f.depth)))
            };
            let (Some((a, b)), Some((c, _))) = (centre(pair[0]), centre(pair[1])) else {
                continue;
            };
            let span = a.distance(c);
            let height = ((span / 10.0 / 0.05).ceil() * 0.05).max(0.30);
            match self.project.add_beam(pair[0], pair[1], b.max(0.12), height) {
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
            self.message = format!("{added} viga(s) ligando os pilares selecionados.");
        }
    }

    /// Adds a secondary beam whose ends bear at explicit positions along two
    /// primary girders. The domain transfers both end reactions into those
    /// girders as point loads (#117).
    fn add_secondary_beam(&mut self, draft: SecondaryBeamDraft) {
        if draft.from_beam == draft.to_beam {
            self.message = "Escolha duas vigas principais diferentes.".to_owned();
            return;
        }
        let previous = self.project.clone();
        let result = self.project.add_beam_on_supports(
            BeamSupport::Beam {
                beam_id: draft.from_beam,
                t: draft.from_t,
            },
            BeamSupport::Beam {
                beam_id: draft.to_beam,
                t: draft.to_t,
            },
            draft.width_cm / 100.0,
            draft.height_cm / 100.0,
        );
        match result {
            Ok(id) => {
                if self.record_change(previous) {
                    self.message = format!(
                        "Viga secundária {id} apoiada nas vigas {} e {}; as reações entram nelas como cargas concentradas.",
                        draft.from_beam, draft.to_beam
                    );
                }
            }
            Err(error) => {
                self.project = previous;
                self.message = error;
            }
        }
    }

    /// Removes the beams of the selected columns (between them, when two or
    /// more are selected).
    pub(crate) fn remove_selected_beams(&mut self) {
        let columns = self.selected_columns();
        let ids: Vec<u64> = if columns.len() >= 2 {
            self.project
                .valid_beams()
                .filter(|b| columns.contains(&b.from) && columns.contains(&b.to))
                .map(|b| b.id)
                .collect()
        } else {
            self.project.beams_of(&columns)
        };
        if ids.is_empty() {
            self.message =
                "Selecione os pilares nas pontas da viga (ou um pilar para todas as suas)."
                    .to_owned();
            return;
        }
        let previous = self.project.clone();
        for id in &ids {
            self.project.delete_beam(*id);
        }
        if self.record_change(previous) {
            self.message = format!("{} viga(s) removida(s).", ids.len());
        }
    }

    pub(crate) fn toggle_structure_layer(&mut self) {
        let previous = self.project.clone();
        self.project.ensure_default_layers();
        if let Some(layer) = self
            .project
            .layers
            .iter_mut()
            .find(|layer| layer.category == Some(LayerCategory::Structure))
        {
            layer.visible = !layer.visible;
        }
        self.record_change(previous);
    }

    /// Shows one discipline (or general architectural view) on the plan.
    pub(crate) fn set_discipline(&mut self, discipline: Discipline) {
        self.discipline = discipline;
        self.prune_selection_for_discipline();
        self.message = match discipline {
            Discipline::All => {
                "Visão geral da arquitetura (estrutura e elétrica ocultas).".to_owned()
            }
            other => format!("Vista {}: o resto fica esmaecido ou oculto.", other.label()),
        };
    }

    fn export_beam_diagrams(&mut self, pdf: bool) {
        let markup = beam_diagrams_svg(&self.project);
        let (label, ext) = if pdf {
            ("Documento PDF", "pdf")
        } else {
            ("Desenho SVG", "svg")
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(format!("esforcos-vigas.{ext}"))
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
            Ok(()) => format!("Esforços nas vigas exportados em {}", path.display()),
            Err(error) => format!("Erro ao exportar os esforços: {error}"),
        };
    }

    fn export_steel_schedule(&mut self) {
        let report = self.project.cached_reinforcement_report();
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Planilha CSV", &["csv"])
            .set_file_name("tabela-de-aco.csv")
            .save_file()
        else {
            return;
        };
        let content = steel_schedule_csv(&report);
        self.message = match std::fs::write(&path, content) {
            Ok(()) => format!("Tabela de aço salva em {}", path.display()),
            Err(error) => format!("Erro ao salvar a tabela de aço: {error}"),
        };
    }

    /// The "Estrutura" window: pre-sizing and the forces in the beams.
    pub(super) fn structure_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Structure {
            return;
        }
        let mut open = true;
        let mut generate = false;
        let mut footings = false;
        let mut export_forces = None;
        let mut export_steel = false;
        let mut export_memorial = false;
        let mut edits: Vec<StructureEdit> = Vec::new();
        let tab_id = egui::Id::unique("trazo_structure_tab");
        let mut tab: u8 = ctx.data(|data| data.get_temp(tab_id)).unwrap_or(0);
        let report = self.project.cached_structure_report();
        let reinforcement = self.project.cached_reinforcement_report();
        let labels = self.project.structure_labels();
        let primary_beams: Vec<(u64, String)> = self
            .project
            .valid_beams()
            .filter(|beam| {
                matches!(beam.start_support(), BeamSupport::Column { .. })
                    && matches!(beam.end_support(), BeamSupport::Column { .. })
            })
            .map(|beam| {
                (
                    beam.id,
                    labels
                        .get(&beam.id)
                        .cloned()
                        .unwrap_or_else(|| format!("Viga {}", beam.id)),
                )
            })
            .collect();
        let secondary_id = egui::Id::unique("trazo_secondary_beam_draft");
        let default_hosts = (
            primary_beams.first().map(|(id, _)| *id).unwrap_or(0),
            primary_beams.get(1).map(|(id, _)| *id).unwrap_or(0),
        );
        let valid_host = |id: u64| primary_beams.iter().any(|(beam, _)| *beam == id);
        let mut secondary_draft: SecondaryBeamDraft = ctx
            .data(|data| data.get_temp(secondary_id))
            .filter(|draft: &SecondaryBeamDraft| {
                valid_host(draft.from_beam) && valid_host(draft.to_beam)
            })
            .unwrap_or_else(|| SecondaryBeamDraft::between(default_hosts.0, default_hosts.1));
        let mut create_secondary = false;
        let preview = if tab == 1 {
            svg_preview(
                ctx,
                "trazo_beam_diagrams",
                &beam_diagrams_svg(&self.project),
            )
        } else {
            None
        };
        let mut settings = self.project.structure.settings;
        // Beyond the applicability limits (#102) Trazo refuses to size.
        let refusal = crate::domain::limits::structural_refusal(&self.project);
        let refused = crate::domain::limits::element_violations(&report);
        egui::Window::new("Estrutura · pré-dimensionamento e esforços")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(860.0)
            .default_height(600.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Lajes, vigas, pilares e sapatas pré-dimensionados por regras práticas, com as cargas descendo das lajes às vigas, dos pilares ao solo.",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
                if let Some(refusal) = &refusal {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Fora dos limites do Trazo: {refusal}"))
                            .font(strong(12.5))
                            .color(DANGER),
                    );
                }
                for violation in &refused {
                    ui.label(
                        RichText::new(format!("{}: {}", violation.element, violation.message))
                            .color(DANGER),
                    );
                }
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    generate = primary_button(ui, "Gerar estrutura").clicked();
                    footings = secondary_button(ui, "Ajustar sapatas às cargas").clicked();
                    ui.separator();
                    ui.label("fck");
                    ui.add(egui::DragValue::new(&mut settings.fck_mpa).range(20.0..=50.0).speed(0.0).suffix(" MPa"));
                    ui.label("Solo");
                    ui.add(egui::DragValue::new(&mut settings.soil_kpa).range(50.0..=600.0).speed(0.0).suffix(" kPa"));
                    egui::ComboBox::from_id_salt("structure_soil_class")
                        .selected_text("Tipo")
                        .width(68.0)
                        .show_ui(ui, |ui| {
                            for soil in crate::domain::structure::SoilClass::ALL {
                                if ui.selectable_label(false, soil.label()).clicked() {
                                    settings.apply_soil_class(soil);
                                }
                            }
                        });
                    ui.label("Pavimentos");
                    ui.add(egui::DragValue::new(&mut settings.floors).range(1..=10).speed(0.0));
                    ui.label("Sobrecarga");
                    ui.add(egui::DragValue::new(&mut settings.live_kn_m2).range(0.5..=10.0).speed(0.0).max_decimals(1).suffix(" kN/m²"));
                    egui::ComboBox::from_id_salt("structure_occupancy_class")
                        .selected_text("Uso")
                        .width(62.0)
                        .show_ui(ui, |ui| {
                            for occ in crate::domain::structure::OccupancyClass::ALL {
                                if ui.selectable_label(false, occ.label()).clicked() {
                                    settings.apply_occupancy(occ);
                                }
                            }
                        });
                    ui.checkbox(&mut settings.per_room_live_loads, "NBR 6120/cômodo");
                    ui.label("Revestimento");
                    ui.add(egui::DragValue::new(&mut settings.finish_kn_m2).range(0.0..=5.0).speed(0.0).max_decimals(1).suffix(" kN/m²"));
                });
                if settings != self.project.structure.settings {
                    edits.push(StructureEdit::Settings(settings));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    for (index, title) in ["Pré-dimensionamento", "Esforços nas vigas", "Armaduras", "Solo e fundações", "Telhado"].iter().enumerate() {
                        if ui
                            .selectable_label(tab == index as u8, RichText::new(*title).font(strong(12.5)))
                            .clicked()
                        {
                            tab = index as u8;
                        }
                    }
                    if tab == 1 {
                        ui.separator();
                        if secondary_button(ui, "Exportar PDF").clicked() {
                            export_forces = Some(true);
                        }
                        if secondary_button(ui, "Exportar SVG").clicked() {
                            export_forces = Some(false);
                        }
                    } else if tab == 2 {
                        ui.separator();
                        if secondary_button(ui, "Exportar tabela CSV").clicked() {
                            export_steel = true;
                        }
                        if secondary_button(ui, "Exportar memorial PDF").clicked() {
                            export_memorial = true;
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if tab == 1 {
                        match &preview {
                            Some((texture, size)) => {
                                let width = ui.available_width().max(200.0);
                                let height = width * size.y / size.x.max(1.0);
                                ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(width, height)));
                            }
                            None => {
                                ui.label(RichText::new("Nenhuma viga ainda.").color(MUTED));
                            }
                        }
                        return;
                    }
                    if tab == 2 {
                        reinforcement_tab(ui, &reinforcement);
                        return;
                    }
                    if tab == 3 {
                        if let Some(action) = super::foundations::soil_tab(ui, &self.project) {
                            edits.push(StructureEdit::Soil(action));
                        }
                        return;
                    }
                    if tab == 4 {
                        if let Some(action) = super::roof::roof_tab(ui, &self.project, &report) {
                            edits.push(StructureEdit::Roof(action));
                        }
                        return;
                    }
                    let heading = |ui: &mut Ui, text: &str| {
                        ui.add_space(8.0);
                        ui.label(RichText::new(text).font(strong(13.0)));
                    };
                    let titles = |ui: &mut Ui, titles: &[&str]| {
                        for title in titles {
                            ui.label(RichText::new(*title).font(strong(12.0)).color(TEXT_2));
                        }
                        ui.end_row();
                    };
                    if report.slabs.is_empty()
                        && report.beams.is_empty()
                        && report.columns.is_empty()
                        && report.masonry.panels.is_empty()
                    {
                        ui.label(
                            RichText::new("Nenhum elemento estrutural ainda: use Gerar estrutura, ou insira pilares e ligue-os por vigas.")
                                .color(MUTED),
                        );
                    }
                    if !report.masonry.bands.is_empty() {
                        heading(ui, "Vergas e contravergas · NBR 16868");
                        egui::Grid::new("structure_masonry_bands")
                            .striped(true)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                titles(ui, &["Marca", "Tipo", "Vão", "Apoios", "Seção", "Armadura", "Situação"]);
                                for band in &report.masonry.bands {
                                    ui.label(RichText::new(&band.label).font(strong(12.5)));
                                    ui.label(band.kind.label());
                                    ui.label(format!("{} m", decimal(band.clear_span_m, 2)));
                                    ui.label(format!(
                                        "{}/{} cm",
                                        decimal(band.bearing_left_m * 100.0, 0),
                                        decimal(band.bearing_right_m * 100.0, 0)
                                    ));
                                    ui.label(section_cm(band.width_m, band.height_m));
                                    ui.label(&band.bars);
                                    status(ui, band.ok());
                                    ui.end_row();
                                }
                            });
                    }
                    if !report.masonry.panels.is_empty() {
                        heading(ui, "Painéis de alvenaria");
                        egui::Grid::new("structure_masonry_panels")
                            .striped(true)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                titles(ui, &["Painel", "Uso", "Material", "h/t", "Sem amarração", "Compressão", "Situação"]);
                                for panel in &report.masonry.panels {
                                    ui.label(RichText::new(&panel.label).font(strong(12.5)));
                                    ui.label(panel.kind.label());
                                    ui.label(&panel.masonry);
                                    ui.label(format!(
                                        "{} / {}",
                                        decimal(panel.slenderness, 1),
                                        decimal(panel.slenderness_limit, 0)
                                    ));
                                    ui.label(format!(
                                        "{} / {} m",
                                        decimal(panel.unbraced_m, 2),
                                        decimal(panel.unbraced_limit_m, 2)
                                    ));
                                    ui.label(if panel.compression_capacity_mpa > 0.0 {
                                        format!(
                                            "{} / {} MPa",
                                            decimal(panel.compression_mpa, 2),
                                            decimal(panel.compression_capacity_mpa, 2)
                                        )
                                    } else {
                                        "— (vedação)".to_owned()
                                    });
                                    status(ui, panel.ok());
                                    ui.end_row();
                                }
                            });
                    }
                    if !report.slabs.is_empty() {
                        heading(ui, "Lajes");
                        egui::Grid::new("structure_slabs").striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
                            titles(ui, &["Laje", "Cômodo", "Tipo", "Vãos", "Painéis", "Espessura", "Mínima", "Carga", "Mx / My", "Flecha", "Situação"]);
                            for slab in &report.slabs {
                                ui.label(RichText::new(&slab.label).font(strong(12.5)));
                                ui.label(&slab.room);
                                let mut kind = slab.kind;
                                egui::ComboBox::from_id_salt(("slab_kind", slab.id))
                                    .selected_text(kind.label())
                                    .show_ui(ui, |ui| {
                                        for option in SlabKind::ALL {
                                            ui.selectable_value(&mut kind, option, option.label());
                                        }
                                    });
                                ui.label(format!("{} × {} m", decimal(slab.lx_m, 2), decimal(slab.ly_m, 2)));
                                ui.label(slab.panels.len().to_string());
                                let mut cm = slab.thickness_m * 100.0;
                                ui.add(egui::DragValue::new(&mut cm).range(5.0..=60.0).speed(0.0).max_decimals(0).suffix(" cm"));
                                ui.label(format!("{:.0} cm", slab.thickness_min_m * 100.0));
                                ui.label(if slab.point_load_kn > 0.0 {
                                    format!(
                                        "{} kN/m² + {} kN",
                                        decimal(slab.load_kn_m2, 2),
                                        decimal(slab.point_load_kn, 1)
                                    )
                                } else {
                                    format!("{} kN/m²", decimal(slab.load_kn_m2, 2))
                                });
                                if slab.kind == SlabKind::Solid {
                                    ui.label(format!(
                                        "{} / {} kN·m/m",
                                        decimal(slab.moment_x_knm_m, 2),
                                        decimal(slab.moment_y_knm_m, 2)
                                    ));
                                    ui.label(format!(
                                        "{} / {} mm",
                                        decimal(slab.deflection_mm, 1),
                                        decimal(slab.deflection_limit_mm, 1)
                                    ));
                                } else {
                                    ui.label("—");
                                    ui.label("fabricante");
                                }
                                status(ui, slab.ok());
                                ui.end_row();
                                if kind != slab.kind {
                                    // A new kind takes its own minimum thickness.
                                    edits.push(StructureEdit::Slab(slab.id, kind, kind.minimum_thickness(slab.lx_m)));
                                } else if (cm / 100.0 - slab.thickness_m).abs() > 1e-6 {
                                    edits.push(StructureEdit::Slab(slab.id, kind, cm / 100.0));
                                }
                            }
                        });
                    }
                    if !report.concentrated_loads.is_empty() {
                        heading(ui, "Cargas concentradas");
                        egui::Grid::new("structure_concentrated_loads")
                            .striped(true)
                            .spacing([14.0, 6.0])
                            .show(ui, |ui| {
                                titles(ui, &["Carga", "Tipo", "Valor", "Caminho", "Momento local", "Situação"]);
                                for load in &report.concentrated_loads {
                                    ui.label(RichText::new(&load.label).font(strong(12.5)));
                                    ui.label(load.kind.label());
                                    ui.label(format!("{} kN", decimal(load.load_kn, 1)));
                                    ui.label(&load.support);
                                    ui.label(if load.local_moment_knm_m > 0.0 {
                                        format!("{} kN·m/m", decimal(load.local_moment_knm_m, 1))
                                    } else {
                                        "—".to_owned()
                                    });
                                    status(
                                        ui,
                                        load.supported
                                            && (load.slab.is_none()
                                                || load
                                                    .punching_ratio
                                                    .is_some_and(|ratio| ratio <= 1.0)),
                                    );
                                    ui.end_row();
                                }
                            });
                    }
                    if primary_beams.len() >= 2 {
                        heading(ui, "Viga secundária apoiada em vigas");
                        egui::Grid::new("secondary_beam_supports")
                            .num_columns(4)
                            .spacing([10.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("Apoio inicial");
                                egui::ComboBox::from_id_salt("secondary_from_beam")
                                    .selected_text(
                                        primary_beams
                                            .iter()
                                            .find(|(id, _)| *id == secondary_draft.from_beam)
                                            .map(|(_, label)| label.as_str())
                                            .unwrap_or("Escolha"),
                                    )
                                    .show_ui(ui, |ui| {
                                        for (id, label) in &primary_beams {
                                            ui.selectable_value(
                                                &mut secondary_draft.from_beam,
                                                *id,
                                                label,
                                            );
                                        }
                                    });
                                ui.label("Posição");
                                ui.add(
                                    egui::DragValue::new(&mut secondary_draft.from_t)
                                        .range(0.05..=0.95)
                                        .speed(0.0)
                                        .max_decimals(2)
                                        .suffix(" do vão"),
                                );
                                ui.end_row();

                                ui.label("Apoio final");
                                egui::ComboBox::from_id_salt("secondary_to_beam")
                                    .selected_text(
                                        primary_beams
                                            .iter()
                                            .find(|(id, _)| *id == secondary_draft.to_beam)
                                            .map(|(_, label)| label.as_str())
                                            .unwrap_or("Escolha"),
                                    )
                                    .show_ui(ui, |ui| {
                                        for (id, label) in &primary_beams {
                                            ui.selectable_value(
                                                &mut secondary_draft.to_beam,
                                                *id,
                                                label,
                                            );
                                        }
                                    });
                                ui.label("Posição");
                                ui.add(
                                    egui::DragValue::new(&mut secondary_draft.to_t)
                                        .range(0.05..=0.95)
                                        .speed(0.0)
                                        .max_decimals(2)
                                        .suffix(" do vão"),
                                );
                                ui.end_row();

                                ui.label("Seção");
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::DragValue::new(&mut secondary_draft.width_cm)
                                            .range(8.0..=100.0)
                                            .speed(0.0)
                                            .suffix(" cm"),
                                    );
                                    ui.label("×");
                                    ui.add(
                                        egui::DragValue::new(&mut secondary_draft.height_cm)
                                            .range(15.0..=200.0)
                                            .speed(0.0)
                                            .suffix(" cm"),
                                    );
                                });
                                create_secondary = primary_button(ui, "Criar viga").clicked();
                                ui.label(
                                    RichText::new(
                                        "As reações das pontas são transferidas às vigas principais.",
                                    )
                                    .size(11.0)
                                    .color(MUTED),
                                );
                                ui.end_row();
                            });
                    }
                    if !report.beams.is_empty() {
                        heading(ui, "Vigas (contínuas, valores característicos)");
                        egui::Grid::new("structure_beams").striped(true).spacing([12.0, 6.0]).show(ui, |ui| {
                            titles(ui, &["Viga", "Vão", "Largura", "Altura", "Mín.", "Carga", "M−", "M+", "V", "Flecha", "Situação"]);
                            for line in &report.beams {
                                for (i, span) in line.spans.iter().enumerate() {
                                    let name = if line.spans.len() > 1 {
                                        format!("{} · vão {}", line.label, i + 1)
                                    } else {
                                        line.label.clone()
                                    };
                                    ui.label(RichText::new(name).font(strong(12.5)));
                                    ui.label(format!("{} m", decimal(span.length_m, 2)));
                                    let (mut b, mut h) = (span.width_m * 100.0, span.height_m * 100.0);
                                    ui.add(egui::DragValue::new(&mut b).range(8.0..=100.0).speed(0.0).max_decimals(0).suffix(" cm"));
                                    ui.add(egui::DragValue::new(&mut h).range(15.0..=200.0).speed(0.0).max_decimals(0).suffix(" cm"));
                                    ui.label(format!("{:.0} cm", span.height_min_m * 100.0));
                                    ui.label(format!("{} kN/m", decimal(span.dead_kn_m + span.live_kn_m, 1)));
                                    ui.label(decimal(span.moment_left.min(span.moment_right), 1));
                                    ui.label(decimal(span.moment_span, 1));
                                    ui.label(decimal(span.shear, 1));
                                    ui.label(format!("{} / {} mm", decimal(span.deflection_mm, 1), decimal(span.limit_mm, 0)));
                                    status(
                                        ui,
                                        span.deflection_mm <= span.limit_mm && span.height_m + 1e-6 >= span.height_min_m,
                                    );
                                    ui.end_row();
                                    if (b / 100.0 - span.width_m).abs() > 1e-6 || (h / 100.0 - span.height_m).abs() > 1e-6 {
                                        edits.push(StructureEdit::Beam(span.beam, b / 100.0, h / 100.0));
                                    }
                                }
                            }
                        });
                        ui.label(RichText::new("Momentos em kN·m (M− nos apoios, M+ no vão), cortante em kN.").size(11.0).color(MUTED));
                    }
                    if !report.columns.is_empty() {
                        heading(ui, "Pilares");
                        egui::Grid::new("structure_columns").striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
                            titles(ui, &["Pilar", "Seção", "Posição", "Carga", "Área necessária", "Área", "Altura", "Esbeltez", "Situação"]);
                            for column in &report.columns {
                                ui.label(RichText::new(&column.label).font(strong(12.5)));
                                ui.label(format!("{} cm", section_cm(column.width_m, column.depth_m)));
                                ui.label(column.position.label());
                                ui.label(format!("{} kN", decimal(column.load_kn, 1)));
                                ui.label(format!("{:.0} cm²", column.area_needed_cm2.max(360.0)));
                                ui.label(format!("{:.0} cm²", column.area_cm2));
                                ui.label(format!("{} m", decimal(column.height_m, 2)));
                                ui.label(format!("λ {:.0}", column.slenderness));
                                status(ui, column.ok());
                                ui.end_row();
                            }
                        });
                        ui.label(RichText::new("Seleciona o pilar na planta para mudar a seção (largura e profundidade nas Propriedades).").size(11.0).color(MUTED));
                    }
                    if !report.footings.is_empty() {
                        heading(ui, "Sapatas");
                        egui::Grid::new("structure_footings").striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
                            titles(ui, &["Sapata", "Pilar", "Dimensões", "Carga", "Necessária", "Altura", "Situação"]);
                            for footing in &report.footings {
                                ui.label(RichText::new(&footing.label).font(strong(12.5)));
                                ui.label(footing.column.clone().unwrap_or_else(|| "—".to_owned()));
                                ui.label(format!("{} cm", section_cm(footing.width_m, footing.depth_m)));
                                ui.label(format!("{} kN", decimal(footing.load_kn, 1)));
                                ui.label(format!("{} cm", section_cm(footing.needed_width_m, footing.needed_depth_m)));
                                ui.label(format!("{:.0} cm", footing.height_m * 100.0));
                                status(ui, footing.ok());
                                ui.end_row();
                            }
                        });
                    }
                    if !report.warnings.is_empty() {
                        ui.add_space(10.0);
                        ui.label(RichText::new("Atenção").font(strong(13.0)).color(WARNING));
                        for warning in &report.warnings {
                            ui.label(RichText::new(format!("• {warning}")).color(Color32::from_rgb(150, 90, 10)));
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "Concreto {} kN/m³; lajes treliçadas pesam cerca de 55 % de uma maciça. Pilares: área = 1,4 · α · N / (0,85 fcd + 1,5 % · 420 MPa), α de 1,3 (intermediário), 1,5 (extremidade) e 1,8 (canto), mínimo 360 cm² e 14 cm. Sapatas: área = 1,05 · N / tensão do solo. {} rótulos na planta.",
                            crate::domain::structure::CONCRETE_KN_M3,
                            labels.len()
                        ))
                        .size(11.0)
                        .color(MUTED),
                    );
                });
            });
        ctx.data_mut(|data| data.insert_temp(tab_id, tab));
        ctx.data_mut(|data| data.insert_temp(secondary_id, secondary_draft));
        if create_secondary {
            self.add_secondary_beam(secondary_draft);
        }
        if generate {
            self.generate_structure();
        }
        if footings {
            self.size_footings();
        }
        if let Some(pdf) = export_forces {
            self.export_beam_diagrams(pdf);
        }
        if export_steel {
            self.export_steel_schedule();
        }
        if export_memorial {
            self.export_presizing_report();
        }
        if !edits.is_empty() {
            let previous = self.project.clone();
            let mut error = None;
            for edit in edits {
                let result = match edit {
                    StructureEdit::Slab(id, kind, thickness) => {
                        let room = self
                            .project
                            .structure
                            .slabs
                            .iter()
                            .find(|s| s.id == id)
                            .map(|s| s.room_id);
                        match room {
                            Some(room) => self
                                .project
                                .set_slab(room, kind, Some(thickness))
                                .map(|_| ()),
                            None => Err("Laje não encontrada.".to_owned()),
                        }
                    }
                    StructureEdit::Beam(id, b, h) => self.project.update_beam(id, Some(b), Some(h)),
                    StructureEdit::Settings(settings) => {
                        self.project.structure.settings = settings;
                        Ok(())
                    }
                    StructureEdit::Soil(super::foundations::SoilAction::Survey(survey)) => {
                        self.project.set_soil_survey(survey);
                        Ok(())
                    }
                    StructureEdit::Roof(super::roof::RoofAction::Add(roof)) => {
                        self.project.add_roof(roof).map(|_| ())
                    }
                    StructureEdit::Roof(super::roof::RoofAction::Update(id, roof)) => {
                        self.project.update_roof(id, roof)
                    }
                    StructureEdit::Roof(super::roof::RoofAction::Wind(wind)) => {
                        self.project.set_wind(wind)
                    }
                    StructureEdit::Roof(super::roof::RoofAction::PostSpecies(species)) => {
                        self.project.structure.post_species = species;
                        Ok(())
                    }
                    StructureEdit::Roof(super::roof::RoofAction::PostTreated(treated)) => {
                        self.project.structure.post_treated = treated;
                        Ok(())
                    }
                    StructureEdit::Roof(super::roof::RoofAction::Remove(id)) => {
                        self.project.remove_roof(id);
                        Ok(())
                    }
                    StructureEdit::Roof(super::roof::RoofAction::FitWalls(id)) => {
                        self.project.fit_walls_to_roof(id).map(|n| {
                            self.message = format!("{n} parede(s) ajustada(s) ao telhado.");
                        })
                    }
                    StructureEdit::Soil(super::foundations::SoilAction::ApplyToSoil) => self
                        .project
                        .apply_survey_to_soil()
                        .map(|kpa| {
                            self.message = format!("Tensão do solo: {kpa:.0} kPa, da sondagem.");
                        })
                        .ok_or_else(|| "A sondagem não chega à cota das sapatas.".to_owned()),
                };
                if let Err(e) = result {
                    error = Some(e);
                }
            }
            match error {
                Some(e) => {
                    self.project = previous;
                    self.message = e;
                }
                None => {
                    self.record_change(previous);
                }
            }
        }
        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn the_interface_adds_a_secondary_beam_between_primary_girders() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        let column = |app: &mut FloorPlanApp, x, y| {
            app.project
                .add_fixture(FixtureKind::Column, Point::new(x, y), 0.20, 0.30, 0)
                .unwrap()
        };
        let c1 = column(&mut app, 0.0, 0.0);
        let c2 = column(&mut app, 4.0, 0.0);
        let c3 = column(&mut app, 0.0, 3.0);
        let c4 = column(&mut app, 4.0, 3.0);
        let first = app.project.add_beam(c1, c2, 0.14, 0.40).unwrap();
        let second = app.project.add_beam(c3, c4, 0.14, 0.40).unwrap();

        app.add_secondary_beam(SecondaryBeamDraft::between(first, second));

        assert_eq!(app.project.structure.beams.len(), 3);
        let beam = app.project.structure.beams.last().unwrap();
        assert_eq!(
            beam.start_support(),
            BeamSupport::Beam {
                beam_id: first,
                t: 0.5
            }
        );
        assert_eq!(
            beam.end_support(),
            BeamSupport::Beam {
                beam_id: second,
                t: 0.5
            }
        );
        assert!(app.message.contains("reações entram nelas"));
    }
}
