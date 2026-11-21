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
use crate::domain::structure::{SlabKind, beam_diagrams_svg, section_cm};

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
}

impl FloorPlanApp {
    /// Lays out columns, beams, slabs and footings over the walls.
    pub(crate) fn generate_structure(&mut self) {
        if self.project.walls.is_empty() {
            self.message = "Desenhe as paredes antes de gerar a estrutura.".to_owned();
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
                "Estrutura gerada: {} pilar(es), {} viga(s), {} laje(s) e {} sapata(s). Veja o pré-dimensionamento.",
                report.columns, report.beams, report.slabs, report.footings
            );
        }
    }

    /// Resizes the footings to what the soil asks for.
    pub(crate) fn size_footings(&mut self) {
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

    /// Shows one discipline (or everything) on the plan.
    pub(crate) fn set_discipline(&mut self, discipline: Discipline) {
        self.discipline = discipline;
        self.message = match discipline {
            Discipline::All => "Mostrando todas as disciplinas.".to_owned(),
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

    /// The "Estrutura" window: pre-sizing and the forces in the beams.
    pub(super) fn structure_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Structure {
            return;
        }
        let mut open = true;
        let mut generate = false;
        let mut footings = false;
        let mut export = None;
        let mut edits: Vec<StructureEdit> = Vec::new();
        let tab_id = egui::Id::unique("trazo_structure_tab");
        let mut tab: u8 = ctx.data(|data| data.get_temp(tab_id)).unwrap_or(0);
        let report = self.project.cached_structure_report();
        let labels = self.project.structure_labels();
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
                        "Lajes, vigas, pilares e sapatas pré-dimensionados por regras práticas, com as cargas descendo das lajes às vigas, dos pilares ao solo. É um estudo inicial: o projeto estrutural é de um engenheiro habilitado (NBR 6118).",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
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
                    for (index, title) in ["Pré-dimensionamento", "Esforços nas vigas"].iter().enumerate() {
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
                            export = Some(true);
                        }
                        if secondary_button(ui, "Exportar SVG").clicked() {
                            export = Some(false);
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
                    if report.slabs.is_empty() && report.beams.is_empty() && report.columns.is_empty() {
                        ui.label(
                            RichText::new("Nenhum elemento estrutural ainda: use Gerar estrutura, ou insira pilares e ligue-os por vigas.")
                                .color(MUTED),
                        );
                    }
                    if !report.slabs.is_empty() {
                        heading(ui, "Lajes");
                        egui::Grid::new("structure_slabs").striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
                            titles(ui, &["Laje", "Cômodo", "Tipo", "Vãos", "Espessura", "Mínima", "Carga", "Situação"]);
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
                                let mut cm = slab.thickness_m * 100.0;
                                ui.add(egui::DragValue::new(&mut cm).range(5.0..=60.0).speed(0.0).max_decimals(0).suffix(" cm"));
                                ui.label(format!("{:.0} cm", slab.thickness_min_m * 100.0));
                                ui.label(format!("{} kN/m²", decimal(slab.load_kn_m2, 2)));
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
                            titles(ui, &["Pilar", "Seção", "Posição", "Carga", "Área necessária", "Área", "Situação"]);
                            for column in &report.columns {
                                ui.label(RichText::new(&column.label).font(strong(12.5)));
                                ui.label(format!("{} cm", section_cm(column.width_m, column.depth_m)));
                                ui.label(column.position.label());
                                ui.label(format!("{} kN", decimal(column.load_kn, 1)));
                                ui.label(format!("{:.0} cm²", column.area_needed_cm2.max(360.0)));
                                ui.label(format!("{:.0} cm²", column.area_cm2));
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
        if generate {
            self.generate_structure();
        }
        if footings {
            self.size_footings();
        }
        if let Some(pdf) = export {
            self.export_beam_diagrams(pdf);
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
