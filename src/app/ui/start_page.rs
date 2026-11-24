//! Start Page (*Página Inicial*, T-06): entry screen with New / Open / Templates
//! actions, recent-project cards with embedded PNG thumbnails from `.trazo`
//! containers, user `.trazot` templates and autosave recovery actions
//! (*Recuperar / Descartar / Adiar*).

use std::path::{Path, PathBuf};

use eframe::egui::{self, ColorImage, Margin, RichText, Stroke, TextureOptions, Ui, vec2};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::widgets::{danger_button, primary_button, secondary_button};

enum StartAction {
    NewBlank,
    OpenDialog,
    OpenTemplates,
    OpenPath(PathBuf),
    RemoveRecent(PathBuf),
    RecoverDraft,
    DiscardDraft,
    DeferDraft,
    Dismiss,
}

impl FloorPlanApp {
    /// Loads and caches the embedded `thumbnail.png` of `path` as an `egui` texture.
    fn thumbnail_texture(&mut self, ctx: &egui::Context, path: &Path) -> Option<egui::TextureId> {
        if let Some(cached) = self.thumbnail_cache.get(path) {
            return cached.as_ref().map(|t| t.id());
        }
        let handle = crate::io::container::read_thumbnail(path).and_then(|png_bytes| {
            let decoded = image::load_from_memory(&png_bytes).ok()?.to_rgba8();
            let (w, h) = decoded.dimensions();
            let color_image =
                ColorImage::from_rgba_unmultiplied([w as usize, h as usize], decoded.as_raw());
            Some(ctx.load_texture(
                format!("trazo_thumb_{}", path.display()),
                color_image,
                TextureOptions::LINEAR,
            ))
        });
        let id = handle.as_ref().map(|t| t.id());
        self.thumbnail_cache.insert(path.to_path_buf(), handle);
        id
    }

    pub(super) fn start_page_view(&mut self, ui: &mut Ui) {
        let mut action: Option<StartAction> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(Margin::symmetric(36, 28))
                    .show(ui, |ui| {
                        ui.set_max_width(940.0);

                        // Header
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Trazo")
                                        .font(strong(26.0))
                                        .color(TEXT),
                                );
                                ui.label(
                                    RichText::new(
                                        "Planta baixa, documentação técnica ABNT e pré-dimensionamento estrutural/elétrico",
                                    )
                                    .size(13.0)
                                    .color(MUTED),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if secondary_button(ui, "Ir para a planta").clicked() {
                                        action = Some(StartAction::Dismiss);
                                    }
                                },
                            );
                        });

                        ui.add_space(18.0);

                        // Autosave recovery banner (Recuperar / Descartar / Adiar)
                        if let Some(draft) = &self.pending_recovery {
                            let name = if draft.name.trim().is_empty() {
                                "Planta sem título"
                            } else {
                                draft.name.trim()
                            };
                            let summary = format!(
                                "\"{name}\" · {} parede(s), {} cômodo(s)",
                                draft.walls.len(),
                                draft.rooms.len()
                            );
                            egui::Frame::new()
                                .fill(egui::Color32::from_rgb(255, 248, 232))
                                .stroke(Stroke::new(1.0, WARNING))
                                .corner_radius(10)
                                .inner_margin(Margin::same(14))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(
                                                    "Rascunho automático encontrado da última sessão",
                                                )
                                                .font(strong(13.5))
                                                .color(TEXT),
                                            );
                                            ui.label(
                                                RichText::new(summary).size(12.0).color(TEXT_2),
                                            );
                                        });
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if secondary_button(ui, "Adiar").clicked() {
                                                    action = Some(StartAction::DeferDraft);
                                                }
                                                if danger_button(ui, "Descartar").clicked() {
                                                    action = Some(StartAction::DiscardDraft);
                                                }
                                                if primary_button(ui, "Recuperar").clicked() {
                                                    action = Some(StartAction::RecoverDraft);
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(16.0);
                        }

                        // Primary action bar
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            if primary_button(ui, "Nova planta").clicked() {
                                action = Some(StartAction::NewBlank);
                            }
                            if secondary_button(ui, "Abrir projeto...").clicked() {
                                action = Some(StartAction::OpenDialog);
                            }
                            if secondary_button(ui, "Modelos e exemplos...").clicked() {
                                action = Some(StartAction::OpenTemplates);
                            }
                        });

                        // User `.trazot` templates strip
                        let shown_templates: Vec<PathBuf> =
                            self.user_templates.shown().to_vec();
                        if !shown_templates.is_empty() {
                            ui.add_space(20.0);
                            ui.label(
                                RichText::new("MEUS MODELOS (.TRAZOT)")
                                    .font(strong(11.0))
                                    .color(MUTED),
                            );
                            ui.add_space(8.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                                for tpl_path in shown_templates {
                                    let label = tpl_path
                                        .file_stem()
                                        .map(|s| s.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| "Modelo".to_owned());
                                    if secondary_button(ui, &label)
                                        .on_hover_text(format!(
                                            "Criar novo projeto a partir de {}",
                                            tpl_path.display()
                                        ))
                                        .clicked()
                                    {
                                        action = Some(StartAction::OpenPath(tpl_path));
                                    }
                                }
                            });
                        }

                        // Recent files grid with PNG thumbnails
                        ui.add_space(22.0);
                        ui.label(
                            RichText::new("PROJETOS RECENTES")
                                .font(strong(11.0))
                                .color(MUTED),
                        );
                        ui.add_space(10.0);

                        let recents = self.recent_files.entries.clone();
                        if recents.is_empty() {
                            egui::Frame::new()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .corner_radius(10)
                                .inner_margin(Margin::same(18))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(
                                            "Nenhum projeto recente ainda. Crie uma nova planta ou abra um projeto pronto em \"Modelos e exemplos...\".",
                                        )
                                        .size(12.5)
                                        .color(MUTED),
                                    );
                                });
                        } else {
                            let ctx = ui.ctx().clone();
                            egui::Grid::new("start_page_recent_grid")
                                .num_columns(3)
                                .spacing([14.0, 14.0])
                                .show(ui, |ui| {
                                    for (idx, entry) in recents.iter().enumerate() {
                                        let thumb_id = self.thumbnail_texture(&ctx, &entry.path);
                                        egui::Frame::new()
                                            .fill(PANEL)
                                            .stroke(Stroke::new(1.0, BORDER))
                                            .corner_radius(10)
                                            .inner_margin(Margin::same(10))
                                            .show(ui, |ui| {
                                                ui.set_width(270.0);
                                                if let Some(tex_id) = thumb_id {
                                                    ui.image(egui::load::SizedTexture::new(
                                                        tex_id,
                                                        vec2(250.0, 150.0),
                                                    ));
                                                } else {
                                                    let (rect, _) = ui.allocate_exact_size(
                                                        vec2(250.0, 150.0),
                                                        egui::Sense::hover(),
                                                    );
                                                    ui.painter().rect_filled(rect, 6, PANEL_ALT);
                                                    ui.painter().text(
                                                        rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "Prévia indisponível",
                                                        regular(11.5),
                                                        MUTED,
                                                    );
                                                }
                                                ui.add_space(8.0);
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(&entry.title)
                                                            .font(strong(13.0))
                                                            .color(TEXT),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} parede(s) · {} cômodo(s)",
                                                        entry.wall_count, entry.room_count
                                                    ))
                                                    .size(11.0)
                                                    .color(MUTED),
                                                );
                                                ui.add_space(6.0);
                                                ui.horizontal(|ui| {
                                                    if primary_button(ui, "Abrir").clicked() {
                                                        action = Some(StartAction::OpenPath(
                                                            entry.path.clone(),
                                                        ));
                                                    }
                                                    if secondary_button(ui, "Remover").clicked() {
                                                        action = Some(StartAction::RemoveRecent(
                                                            entry.path.clone(),
                                                        ));
                                                    }
                                                });
                                            });
                                        if idx % 3 == 2 {
                                            ui.end_row();
                                        }
                                    }
                                });
                        }
                    });
            });

        if let Some(act) = action {
            match act {
                StartAction::NewBlank => {
                    self.show_start_page = false;
                    self.new_project();
                }
                StartAction::OpenDialog => {
                    self.request_open();
                }
                StartAction::OpenTemplates => {
                    self.active_doc_dialog = DocDialog::Templates;
                }
                StartAction::OpenPath(path) => {
                    let _ = self.open_in_tab(&path);
                }
                StartAction::RemoveRecent(path) => {
                    self.recent_files.remove(&path);
                    self.thumbnail_cache.remove(&path);
                    if !cfg!(test) {
                        let _ = self.recent_files.save();
                    }
                }
                StartAction::RecoverDraft => self.recover_pending_draft(),
                StartAction::DiscardDraft => self.discard_pending_draft(),
                StartAction::DeferDraft => self.defer_pending_draft(),
                StartAction::Dismiss => {
                    self.show_start_page = false;
                }
            }
        }
    }
}
