//! Start Page (*Página Inicial*, T-06): entry screen with New / Open / Templates
//! actions, recent-project cards with embedded PNG thumbnails from `.trazo`
//! containers, user `.trazot` templates and autosave recovery actions
//! (*Recuperar / Descartar / Adiar*).

use std::path::{Path, PathBuf};

use eframe::egui::{
    self, ColorImage, CornerRadius, Margin, RichText, Sense, Stroke, StrokeKind, TextureOptions,
    Ui, vec2,
};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::icons::{self, Icon};
use super::widgets::{danger_button, icon_button_sized, primary_button, secondary_button};

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
                let avail = ui.available_width();
                let content_w = (avail - 64.0).clamp(320.0, 960.0);
                let side_margin = ((avail - content_w) * 0.5).max(16.0).round() as i8;
                egui::Frame::new()
                    .inner_margin(Margin {
                        left: side_margin,
                        right: side_margin,
                        top: 28,
                        bottom: 32,
                    })
                    .show(ui, |ui| {
                        ui.set_width(content_w);

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
                                .fill(WARNING_SOFT)
                                .stroke(Stroke::new(1.0, WARNING))
                                .corner_radius(10)
                                .inner_margin(Margin::same(14))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = vec2(12.0, 8.0);
                                        ui.vertical(|ui| {
                                            ui.set_max_width((content_w - 310.0).max(220.0));
                                            ui.label(
                                                RichText::new(
                                                    "Rascunho automático encontrado da última sessão",
                                                )
                                                .font(strong(13.5))
                                                .color(TEXT),
                                            );
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(&summary)
                                                        .size(12.0)
                                                        .color(TEXT_2),
                                                )
                                                .truncate(),
                                            )
                                            .on_hover_text(&summary);
                                        });
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = 6.0;
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
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(10.0, 8.0);
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

                        // Recent files grid with PNG thumbnails (responsive 1 / 2 / 3 columns)
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
                                    ui.set_width(ui.available_width());
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
                            let cols = if content_w >= 840.0 {
                                3
                            } else if content_w >= 540.0 {
                                2
                            } else {
                                1
                            };
                            let gap = 14.0;
                            let card_outer_w =
                                ((content_w - (cols as f32 - 1.0) * gap) / cols as f32)
                                    .floor()
                                    .max(220.0);
                            let inner_w = (card_outer_w - 22.0).max(180.0);
                            let thumb_h = (inner_w * 0.625).round();

                            egui::Grid::new("start_page_recent_grid")
                                .num_columns(cols)
                                .spacing([gap, gap])
                                .show(ui, |ui| {
                                    for (idx, entry) in recents.iter().enumerate() {
                                        let thumb_id = self.thumbnail_texture(&ctx, &entry.path);
                                        let card_id = ui.make_persistent_id(("recent_card", idx));
                                        let card_hovered = ui
                                            .ctx()
                                            .data(|d| d.get_temp::<bool>(card_id))
                                            .unwrap_or(false);
                                        let frame_resp = egui::Frame::new()
                                            .fill(if card_hovered { CONTROL } else { PANEL })
                                            .stroke(Stroke::new(
                                                1.0,
                                                if card_hovered { ACCENT_RING } else { BORDER },
                                            ))
                                            .corner_radius(10)
                                            .inner_margin(Margin::same(10))
                                            .show(ui, |ui| {
                                                ui.set_width(inner_w);
                                                let thumb_size = vec2(inner_w, thumb_h);
                                                let thumb_resp = if let Some(tex_id) = thumb_id {
                                                    ui.add(
                                                        egui::Image::new(
                                                            egui::load::SizedTexture::new(
                                                                tex_id, thumb_size,
                                                            ),
                                                        )
                                                        .corner_radius(CornerRadius::same(6))
                                                        .sense(Sense::click()),
                                                    )
                                                } else {
                                                    let (rect, resp) = ui.allocate_exact_size(
                                                        thumb_size,
                                                        Sense::click(),
                                                    );
                                                    ui.painter().rect(
                                                        rect,
                                                        6,
                                                        PANEL_ALT,
                                                        Stroke::new(1.0, BORDER),
                                                        StrokeKind::Inside,
                                                    );
                                                    icons::paint(
                                                        ui.painter(),
                                                        egui::Rect::from_center_size(
                                                            rect.center() - vec2(0.0, 10.0),
                                                            vec2(26.0, 26.0),
                                                        ),
                                                        Icon::Plan,
                                                        MUTED,
                                                        ACCENT,
                                                    );
                                                    ui.painter().text(
                                                        rect.center() + vec2(0.0, 14.0),
                                                        egui::Align2::CENTER_CENTER,
                                                        "Prévia indisponível",
                                                        regular(11.5),
                                                        MUTED,
                                                    );
                                                    resp
                                                };
                                                let thumb_resp = thumb_resp
                                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                    .on_hover_text(format!(
                                                        "Abrir {}",
                                                        entry.path.display()
                                                    ));
                                                if thumb_resp.clicked() {
                                                    action = Some(StartAction::OpenPath(
                                                        entry.path.clone(),
                                                    ));
                                                }
                                                ui.add_space(8.0);
                                                ui.horizontal(|ui| {
                                                    ui.vertical(|ui| {
                                                        ui.set_max_width((inner_w - 30.0).max(80.0));
                                                        let title_resp = ui
                                                            .add(
                                                                egui::Label::new(
                                                                    RichText::new(&entry.title)
                                                                        .font(strong(13.0))
                                                                        .color(TEXT),
                                                                )
                                                                .truncate()
                                                                .sense(Sense::click()),
                                                            )
                                                            .on_hover_cursor(
                                                                egui::CursorIcon::PointingHand,
                                                            );
                                                        if title_resp.clicked() {
                                                            action = Some(StartAction::OpenPath(
                                                                entry.path.clone(),
                                                            ));
                                                        }
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "{} parede(s) · {} cômodo(s)",
                                                                entry.wall_count, entry.room_count
                                                            ))
                                                            .size(11.0)
                                                            .color(MUTED),
                                                        );
                                                    });
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if icon_button_sized(
                                                                ui,
                                                                Icon::Close,
                                                                "Remover dos recentes",
                                                                false,
                                                                22.0,
                                                                12.0,
                                                            )
                                                            .clicked()
                                                            {
                                                                action =
                                                                    Some(StartAction::RemoveRecent(
                                                                        entry.path.clone(),
                                                                    ));
                                                            }
                                                        },
                                                    );
                                                });
                                            })
                                            .response;
                                        ui.ctx().data_mut(|d| {
                                            d.insert_temp(card_id, frame_resp.hovered());
                                        });
                                        if idx % cols == cols - 1 {
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
