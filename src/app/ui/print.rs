//! Exact-scale Print dialog with live sheet preview and native system print
//! dialog dispatch (#70).

use std::{
    hash::{Hash, Hasher},
    sync::mpsc,
    thread,
};

use eframe::egui::{
    self, ColorImage, Margin, RichText, Stroke, TextureHandle, TextureOptions, vec2,
};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button, toggle_pill};
use crate::{
    domain::sheet::{DrawingScale, Orientation, PaperSize, SheetMargins},
    io::print::{self, PrintOptions},
};

#[derive(Default)]
pub(in crate::app) struct PrintDialogState {
    pub options: PrintOptions,
    pub printers: Vec<String>,
    pub printers_loaded: bool,
    pub preview_texture: Option<TextureHandle>,
    pub preview_key: u64,
    pub preview_meta: Option<(f64, f64, String, f64, bool)>,
    pub preview_error: Option<String>,
    pub job_receiver: Option<mpsc::Receiver<Result<String, String>>>,
    pub status: Option<(bool, String)>,
}

impl FloorPlanApp {
    pub(in crate::app) fn open_print_dialog(&mut self) {
        self.print_state.options.monochrome = self.project.doc_style.monochrome;
        self.print_state.status = None;
        if !self.print_state.printers_loaded {
            self.print_state.printers = print::list_printers();
            self.print_state.printers_loaded = true;
        }
        self.active_doc_dialog = DocDialog::Print;
    }

    fn compute_print_preview_key(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.plan_log.revision.hash(&mut h);
        self.project.name.hash(&mut h);
        self.project.walls.len().hash(&mut h);
        self.project.rooms.len().hash(&mut h);
        self.project.openings.len().hash(&mut h);
        self.project.fixtures.len().hash(&mut h);
        let (sw, sh) = self.project.sheet.sheet_mm();
        sw.to_bits().hash(&mut h);
        sh.to_bits().hash(&mut h);
        self.project
            .sheet
            .scale
            .denominator()
            .to_bits()
            .hash(&mut h);
        self.print_state.options.monochrome.hash(&mut h);
        self.project.doc_style.monochrome.hash(&mut h);
        (self.project.doc_style.preset as u8).hash(&mut h);
        h.finish()
    }

    fn ensure_print_preview(&mut self, ctx: &egui::Context) {
        let key = self.compute_print_preview_key();
        if self.print_state.preview_texture.is_some() && self.print_state.preview_key == key {
            return;
        }
        match print::render_sheet_preview(&self.project, self.print_state.options.monochrome, 560) {
            Ok(preview) => {
                let image = ColorImage::from_rgba_unmultiplied(
                    [preview.width, preview.height],
                    &preview.rgba,
                );
                let handle = ctx.load_texture("trazo_print_preview", image, TextureOptions::LINEAR);
                self.print_state.preview_texture = Some(handle);
                self.print_state.preview_meta = Some((
                    preview.sheet_mm.0,
                    preview.sheet_mm.1,
                    preview.scale_label,
                    preview.mm_per_meter,
                    preview.fits,
                ));
                self.print_state.preview_error = None;
                self.print_state.preview_key = key;
            }
            Err(error) => {
                self.print_state.preview_error = Some(error);
                self.print_state.preview_key = key;
            }
        }
    }

    fn start_print_job(&mut self, ctx: &egui::Context) {
        if self.print_state.job_receiver.is_some() {
            return;
        }
        let project = self.project.clone();
        let options = self.print_state.options.clone();
        let wake = ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.print_state.job_receiver = Some(rx);
        self.print_state.status = Some((true, "Preparando prancha em escala exata...".to_owned()));
        let _ = thread::Builder::new()
            .name("trazo-print-job".to_owned())
            .spawn(move || {
                let res = print::dispatch_print(&project, &options);
                let _ = tx.send(res);
                wake.request_repaint();
            });
    }

    pub(super) fn print_dialog(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.print_state.job_receiver
            && let Ok(result) = rx.try_recv()
        {
            self.print_state.job_receiver = None;
            match result {
                Ok(msg) => {
                    self.message = msg.clone();
                    self.print_state.status = Some((true, msg));
                }
                Err(err) => {
                    self.message = err.clone();
                    self.print_state.status = Some((false, err));
                }
            }
        }

        if self.active_doc_dialog != DocDialog::Print {
            return;
        }
        self.ensure_print_preview(ctx);

        let mut close = false;
        let mut open_sheet_setup = false;
        let mut trigger_print = false;
        let mut export_pdf = false;
        let busy = self.print_state.job_receiver.is_some();

        let response = egui::Modal::new(egui::Id::unique("print_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(780.0);
                super::dialogs::dialog_title(
                    ui,
                    Icon::Print,
                    "Imprimir em escala exata",
                    "Pré-visualize a prancha e envie para a impressora sem distorção de escala (1:1 em milímetros).",
                );

                ui.columns(2, |cols| {
                    // Left column: live exact-scale sheet preview.
                    cols[0].vertical(|ui| {
                        egui::Frame::new()
                            .fill(BAND)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.set_min_size(vec2(360.0, 320.0));
                                ui.vertical_centered(|ui| {
                                    if let Some(tex) = &self.print_state.preview_texture {
                                        let tex_size = tex.size_vec2();
                                        let max_w = 340.0;
                                        let max_h = 280.0;
                                        let scale = (max_w / tex_size.x.max(1.0))
                                            .min(max_h / tex_size.y.max(1.0));
                                        let draw_size = tex_size * scale;
                                        egui::Frame::new()
                                            .fill(PANEL)
                                            .stroke(Stroke::new(1.0, BORDER_STRONG))
                                            .shadow(egui::Shadow {
                                                offset: [0, 3],
                                                blur: 10,
                                                spread: 0,
                                                color: egui::Color32::from_black_alpha(28),
                                            })
                                            .show(ui, |ui| {
                                                ui.image((tex.id(), draw_size));
                                            });
                                    } else if let Some(err) = &self.print_state.preview_error {
                                        ui.label(RichText::new(err).size(12.0).color(DANGER));
                                    } else {
                                        ui.label(
                                            RichText::new("Gerando prévia da prancha...")
                                                .size(12.0)
                                                .color(MUTED),
                                        );
                                    }
                                });

                                if let Some((sw, sh, ref scale_label, mm_per_m, fits)) =
                                    self.print_state.preview_meta
                                {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(format!(
                                            "Folha {} ({sw:.0} × {sh:.0} mm) · Escala {scale_label} (1,00 m = {mm_per_m:.1} mm no papel)",
                                            self.project.sheet.size.short_name()
                                        ).replace('.', ","))
                                        .size(11.5)
                                        .color(TEXT_2),
                                    );
                                    let (fit_text, fit_color) = if fits {
                                        (
                                            "A planta cabe integralmente na área útil da prancha.",
                                            SUCCESS,
                                        )
                                    } else {
                                        (
                                            "Atenção: a planta excede a folha atual. Escolha uma folha maior ou clique em Melhor escala.",
                                            WARNING,
                                        )
                                    };
                                    ui.label(RichText::new(fit_text).size(11.5).color(fit_color));
                                }
                            });
                    });

                    // Right column: paper, exact scale, printer and print options.
                    cols[1].vertical(|ui| {
                        egui::Frame::new()
                            .fill(PANEL_ALT)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Papel e escala")
                                        .font(strong(13.0))
                                        .color(TEXT),
                                );
                                ui.add_space(6.0);
                                ui.horizontal_wrapped(|ui| {
                                    for standard in PaperSize::STANDARD {
                                        if toggle_pill(
                                            ui,
                                            standard.short_name(),
                                            self.project.sheet.size == standard,
                                            &standard.label(),
                                        )
                                        .clicked()
                                        {
                                            let prev = self.project.clone();
                                            self.project.sheet.size = standard;
                                            self.project.sheet.margins =
                                                SheetMargins::abnt_for(standard);
                                            self.record_change(prev);
                                        }
                                    }
                                });
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    for orient in [Orientation::Landscape, Orientation::Portrait] {
                                        if toggle_pill(
                                            ui,
                                            orient.label(),
                                            self.project.sheet.orientation == orient,
                                            orient.label(),
                                        )
                                        .clicked()
                                        {
                                            let prev = self.project.clone();
                                            self.project.sheet.orientation = orient;
                                            self.record_change(prev);
                                        }
                                    }
                                    if let Some((min, max)) = self.project.bounds()
                                        && secondary_button(ui, "Melhor escala").clicked()
                                    {
                                        let (_, _, pw, ph) = self.project.sheet.printable_rect_mm();
                                        let best = DrawingScale::best_fit(
                                            (max.x - min.x).max(0.5),
                                            (max.y - min.y).max(0.5),
                                            pw,
                                            (ph - 16.0).max(20.0),
                                        );
                                        let prev = self.project.clone();
                                        self.project.sheet.scale = best;
                                        self.record_change(prev);
                                    }
                                });
                                ui.add_space(6.0);
                                ui.horizontal_wrapped(|ui| {
                                    for scale in DrawingScale::STANDARD {
                                        if toggle_pill(
                                            ui,
                                            &scale.label(),
                                            self.project.sheet.scale == scale,
                                            &format!(
                                                "1,00 m real = {:.1} mm no papel",
                                                scale.mm_per_meter()
                                            ),
                                        )
                                        .clicked()
                                        {
                                            let prev = self.project.clone();
                                            self.project.sheet.scale = scale;
                                            self.record_change(prev);
                                        }
                                    }
                                });
                            });

                        ui.add_space(8.0);
                        egui::Frame::new()
                            .fill(PANEL_ALT)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Impressora e qualidade")
                                        .font(strong(13.0))
                                        .color(TEXT),
                                );
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Impressora:").size(12.0).color(TEXT_2));
                                    let current_label = self
                                        .print_state
                                        .options
                                        .printer
                                        .as_deref()
                                        .unwrap_or("Padrão do sistema");
                                    egui::ComboBox::from_id_salt("print_printer_combo")
                                        .width(210.0)
                                        .selected_text(current_label)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut self.print_state.options.printer,
                                                None,
                                                "Padrão do sistema",
                                            );
                                            for name in &self.print_state.printers {
                                                ui.selectable_value(
                                                    &mut self.print_state.options.printer,
                                                    Some(name.clone()),
                                                    name,
                                                );
                                            }
                                        });
                                });
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Cópias:").size(12.0).color(TEXT_2));
                                    ui.add(
                                        egui::DragValue::new(&mut self.print_state.options.copies)
                                            .range(1..=99),
                                    );
                                    ui.add_space(12.0);
                                    ui.label(RichText::new("Resolução:").size(12.0).color(TEXT_2));
                                    for dpi in [150u32, 300u32] {
                                        if toggle_pill(
                                            ui,
                                            &format!("{dpi} DPI"),
                                            self.print_state.options.dpi == dpi,
                                            "Qualidade de rasterização",
                                        )
                                        .clicked()
                                        {
                                            self.print_state.options.dpi = dpi;
                                        }
                                    }
                                });
                                ui.add_space(6.0);
                                ui.checkbox(
                                    &mut self.print_state.options.monochrome,
                                    "Impressão monocromática (P&B técnico)",
                                );
                                ui.checkbox(
                                    &mut self.print_state.options.use_system_dialog,
                                    "Abrir diálogo de impressão do sistema antes de imprimir",
                                );
                            });
                    });
                });

                if let Some((ok, ref msg)) = self.print_state.status {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(msg)
                            .size(12.0)
                            .color(if ok { ACCENT_TEXT } else { DANGER }),
                    );
                }

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let print_label = if busy {
                        "Imprimindo..."
                    } else if self.print_state.options.use_system_dialog {
                        "Imprimir (diálogo do sistema)..."
                    } else {
                        "Imprimir agora"
                    };
                    if ui
                        .add_enabled_ui(!busy, |ui| primary_button(ui, print_label))
                        .inner
                        .clicked()
                    {
                        trigger_print = true;
                    }
                    if secondary_button(ui, "Salvar PDF em escala...").clicked() {
                        export_pdf = true;
                    }
                    if secondary_button(ui, "Carimbo e margens...").clicked() {
                        open_sheet_setup = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary_button(ui, "Fechar").clicked() {
                            close = true;
                        }
                    });
                });
            });

        if trigger_print {
            self.start_print_job(ctx);
        }
        if export_pdf {
            self.export_sheet_pdf();
        }
        if open_sheet_setup {
            self.active_doc_dialog = DocDialog::Sheet;
        } else if close || response.should_close() {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
