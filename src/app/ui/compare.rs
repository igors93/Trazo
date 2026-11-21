//! Visual version comparison dialog and interactive canvas diff overlay (#45).

use std::fs;

use eframe::egui::{self, Color32, Margin, RichText, Shape, Stroke, vec2};

use super::super::{DocDialog, FloorPlanApp, canvas::ViewTransform, theme::*};
use super::icons::Icon;
use super::widgets::{primary_button, secondary_button};
use crate::{
    domain::{
        compare::{self, ProjectDiff},
        model::Project,
    },
    io::project_file,
};

const DIFF_ADDED: Color32 = Color32::from_rgb(30, 158, 90);
const DIFF_REMOVED: Color32 = Color32::from_rgb(214, 59, 48);
const DIFF_MODIFIED: Color32 = Color32::from_rgb(213, 132, 27);

pub(in crate::app) struct CompareUiState {
    pub baseline: Option<Project>,
    pub baseline_label: String,
    pub diff: Option<ProjectDiff>,
    pub overlay_active: bool,
    pub show_added: bool,
    pub show_removed: bool,
    pub show_modified: bool,
    pub last_revision: u64,
    pub status: Option<(bool, String)>,
}

impl Default for CompareUiState {
    fn default() -> Self {
        Self {
            baseline: None,
            baseline_label: String::new(),
            diff: None,
            overlay_active: false,
            show_added: true,
            show_removed: true,
            show_modified: true,
            last_revision: u64::MAX,
            status: None,
        }
    }
}

impl FloorPlanApp {
    fn refresh_compare_diff(&mut self) {
        if let Some(ref baseline) = self.compare_state.baseline {
            self.compare_state.diff = Some(compare::compare_projects(
                baseline,
                &self.project,
                self.compare_state.baseline_label.clone(),
            ));
            self.compare_state.last_revision = self.plan_log.revision;
        }
    }

    pub(in crate::app) fn pick_compare_baseline_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Selecionar versão anterior do projeto para comparar")
            .add_filter("Projetos Trazo", &["trazo", "json"])
            .pick_file()
        else {
            return;
        };
        match project_file::load(&path) {
            Ok((loaded, _)) => {
                let label = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("versão anterior")
                    .to_owned();
                self.compare_state.baseline = Some(loaded);
                self.compare_state.baseline_label = label.clone();
                self.compare_state.overlay_active = true;
                self.refresh_compare_diff();
                let msg = format!("Comparando projeto atual com '{label}'.");
                self.message = msg.clone();
                self.compare_state.status = Some((true, msg));
            }
            Err(err) => {
                let msg = format!("Não foi possível abrir a versão para comparação: {err}");
                self.message = msg.clone();
                self.compare_state.status = Some((false, msg));
            }
        }
    }

    pub(in crate::app) fn compare_with_tab(&mut self, tab_index: usize) {
        if let Some(Some(doc)) = self.tabs.get(tab_index) {
            let label = if doc.project.name.trim().is_empty() {
                format!("Aba {}", tab_index + 1)
            } else {
                doc.project.name.clone()
            };
            let baseline = doc.project.clone();
            self.compare_state.baseline = Some(baseline);
            self.compare_state.baseline_label = label.clone();
            self.compare_state.overlay_active = true;
            self.refresh_compare_diff();
            let msg = format!("Comparando projeto atual com a aba '{label}'.");
            self.message = msg.clone();
            self.compare_state.status = Some((true, msg));
        }
    }

    fn export_compare_svg(&mut self) {
        let (Some(baseline), Some(diff)) = (&self.compare_state.baseline, &self.compare_state.diff)
        else {
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .set_title("Exportar comparação visual (SVG)")
            .add_filter("Desenho vetorial SVG", &["svg"])
            .set_file_name("comparacao-versoes.svg")
            .save_file()
        else {
            return;
        };
        let svg = compare::export_diff_svg(baseline, &self.project, diff);
        match fs::write(&path, svg) {
            Ok(()) => {
                let msg = format!("Comparação visual exportada em {}.", path.display());
                self.message = msg.clone();
                self.compare_state.status = Some((true, msg));
            }
            Err(err) => {
                let msg = format!("Não foi possível salvar a comparação: {err}");
                self.message = msg.clone();
                self.compare_state.status = Some((false, msg));
            }
        }
    }

    /// Paints the live color-coded comparison overlay (green added, dashed red
    /// removed, amber modified) on top of the active canvas.
    pub(in crate::app) fn paint_compare_overlay(
        &mut self,
        painter: &egui::Painter,
        view: ViewTransform,
    ) {
        if !self.compare_state.overlay_active || self.compare_state.baseline.is_none() {
            return;
        }
        if self.compare_state.last_revision != self.plan_log.revision {
            self.refresh_compare_diff();
        }
        let Some(ref diff) = self.compare_state.diff else {
            return;
        };

        if self.compare_state.show_removed {
            for wall in &diff.walls_removed {
                let a = view.to_screen(wall.start);
                let b = view.to_screen(wall.end);
                let sw = (view.scale * wall.thickness() as f32).clamp(3.0, 12.0);
                painter.extend(Shape::dashed_line(
                    &[a, b],
                    Stroke::new(sw * 0.55, DIFF_REMOVED),
                    8.0,
                    5.0,
                ));
            }
            for fix in &diff.fixtures_removed {
                let (lo, hi) = fix.bounds();
                let r = egui::Rect::from_two_pos(view.to_screen(lo), view.to_screen(hi));
                painter.rect_filled(r, 0.0, DIFF_REMOVED.gamma_multiply(0.16));
                let pts = [
                    r.left_top(),
                    r.right_top(),
                    r.right_bottom(),
                    r.left_bottom(),
                    r.left_top(),
                ];
                painter.extend(Shape::dashed_line(
                    &pts,
                    Stroke::new(1.8, DIFF_REMOVED),
                    5.0,
                    3.0,
                ));
            }
            for drawing in &diff.drawings_removed {
                let pts: Vec<_> = drawing
                    .shape
                    .outline()
                    .into_iter()
                    .map(|p| view.to_screen(p))
                    .collect();
                if pts.len() >= 2 {
                    painter.extend(Shape::dashed_line(
                        &pts,
                        Stroke::new(1.8, DIFF_REMOVED),
                        6.0,
                        4.0,
                    ));
                }
            }
        }

        if self.compare_state.show_modified {
            for (old, new) in &diff.walls_modified {
                let oa = view.to_screen(old.start);
                let ob = view.to_screen(old.end);
                let na = view.to_screen(new.start);
                let nb = view.to_screen(new.end);
                let sw = (view.scale * new.thickness() as f32).clamp(3.0, 12.0);
                painter.extend(Shape::dashed_line(
                    &[oa, ob],
                    Stroke::new(sw * 0.45, DIFF_MODIFIED.gamma_multiply(0.6)),
                    5.0,
                    4.0,
                ));
                painter.line_segment([na, nb], Stroke::new(sw * 0.55, DIFF_MODIFIED));
            }
            for (_, fix) in &diff.fixtures_modified {
                let (lo, hi) = fix.bounds();
                let r = egui::Rect::from_two_pos(view.to_screen(lo), view.to_screen(hi));
                painter.rect(
                    r,
                    0.0,
                    DIFF_MODIFIED.gamma_multiply(0.16),
                    Stroke::new(1.8, DIFF_MODIFIED),
                    egui::StrokeKind::Middle,
                );
            }
        }

        if self.compare_state.show_added {
            for wall in &diff.walls_added {
                let a = view.to_screen(wall.start);
                let b = view.to_screen(wall.end);
                let sw = (view.scale * wall.thickness() as f32).clamp(3.0, 12.0);
                painter.line_segment([a, b], Stroke::new(sw * 0.55, DIFF_ADDED));
            }
            for fix in &diff.fixtures_added {
                let (lo, hi) = fix.bounds();
                let r = egui::Rect::from_two_pos(view.to_screen(lo), view.to_screen(hi));
                painter.rect(
                    r,
                    0.0,
                    DIFF_ADDED.gamma_multiply(0.16),
                    Stroke::new(1.8, DIFF_ADDED),
                    egui::StrokeKind::Middle,
                );
            }
            for drawing in &diff.drawings_added {
                let pts: Vec<_> = drawing
                    .shape
                    .outline()
                    .into_iter()
                    .map(|p| view.to_screen(p))
                    .collect();
                if pts.len() >= 2 {
                    painter.add(Shape::line(pts, Stroke::new(1.8, DIFF_ADDED)));
                }
            }
        }

        // Compact legend badge at top-right of the canvas while comparing.
        let counts = diff.counts();
        let text = format!(
            "Comparação ({}) · +{}  -{}  ~{}",
            diff.baseline_label, counts.added, counts.removed, counts.modified
        );
        let galley = painter.layout_no_wrap(text, strong(11.5), TEXT);
        let pill = egui::Rect::from_min_size(
            egui::pos2(
                view.rect.right() - galley.size().x - 32.0,
                view.rect.top() + 12.0,
            ),
            galley.size() + vec2(20.0, 10.0),
        );
        if view.rect.contains_rect(pill) {
            painter.rect(
                pill,
                6.0,
                PANEL,
                Stroke::new(1.0, ACCENT_RING),
                egui::StrokeKind::Inside,
            );
            painter.galley(pill.min + vec2(10.0, 5.0), galley, TEXT);
        }
    }

    pub(super) fn compare_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Compare {
            return;
        }
        if self.compare_state.baseline.is_some()
            && self.compare_state.last_revision != self.plan_log.revision
        {
            self.refresh_compare_diff();
        }

        let mut close = false;
        let mut pick_file = false;
        let mut pick_tab: Option<usize> = None;
        let mut clear_compare = false;
        let mut export_svg = false;

        let other_tabs: Vec<(usize, String)> = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| {
                if i == self.active_tab {
                    None
                } else {
                    slot.as_ref().map(|doc| {
                        let name = if doc.project.name.trim().is_empty() {
                            format!("Aba {}", i + 1)
                        } else {
                            doc.project.name.clone()
                        };
                        (i, name)
                    })
                }
            })
            .collect();

        let response = egui::Modal::new(egui::Id::unique("compare_dialog"))
            .frame(super::dialogs::dialog_frame())
            .backdrop_color(super::dialogs::backdrop())
            .show(ctx, |ui| {
                ui.set_width(640.0);
                super::dialogs::dialog_title(
                    ui,
                    Icon::SplitView,
                    "Comparação visual de versões",
                    "Compare a planta atual com uma versão anterior (.trazo / .json) ou outra aba aberta, destacando adições, remoções e alterações.",
                );

                ui.horizontal_wrapped(|ui| {
                    if primary_button(ui, "Escolher versão anterior (.trazo)...").clicked() {
                        pick_file = true;
                    }
                    if !other_tabs.is_empty() {
                        egui::ComboBox::from_id_salt("compare_tab_combo")
                            .selected_text("Comparar com aba aberta...")
                            .show_ui(ui, |ui| {
                                for (idx, title) in &other_tabs {
                                    if ui.selectable_label(false, title).clicked() {
                                        pick_tab = Some(*idx);
                                    }
                                }
                            });
                    }
                    if self.compare_state.baseline.is_some()
                        && secondary_button(ui, "Limpar comparação").clicked()
                    {
                        clear_compare = true;
                    }
                });

                if let Some((ok, ref msg)) = self.compare_state.status {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(msg)
                            .size(12.0)
                            .color(if ok { ACCENT_TEXT } else { DANGER }),
                    );
                }

                ui.add_space(10.0);
                if let Some(ref diff) = self.compare_state.diff {
                    let counts = diff.counts();
                    egui::Frame::new()
                        .fill(PANEL_ALT)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(8)
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(
                                    &mut self.compare_state.overlay_active,
                                    "Mostrar sobreposição visual no desenho",
                                );
                            });
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.checkbox(
                                    &mut self.compare_state.show_added,
                                    RichText::new(format!("Adicionados ({})", counts.added))
                                        .color(DIFF_ADDED),
                                );
                                ui.add_space(8.0);
                                ui.checkbox(
                                    &mut self.compare_state.show_removed,
                                    RichText::new(format!("Removidos ({})", counts.removed))
                                        .color(DIFF_REMOVED),
                                );
                                ui.add_space(8.0);
                                ui.checkbox(
                                    &mut self.compare_state.show_modified,
                                    RichText::new(format!("Alterados ({})", counts.modified))
                                        .color(DIFF_MODIFIED),
                                );
                            });
                            ui.add_space(6.0);
                            let delta = diff.total_area_after - diff.total_area_before;
                            let sign = if delta >= 0.0 { "+" } else { "" };
                            ui.label(
                                RichText::new(format!(
                                    "Área total: {:.2} m² → {:.2} m² ({sign}{:.2} m²)",
                                    diff.total_area_before, diff.total_area_after, delta
                                )
                                .replace('.', ","))
                                .font(strong(12.5))
                                .color(TEXT),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "Paredes: +{} / -{} / ~{} · Aberturas: +{} / -{} / ~{} · Objetos: +{} / -{} / ~{}",
                                    diff.walls_added.len(),
                                    diff.walls_removed.len(),
                                    diff.walls_modified.len(),
                                    diff.openings_added,
                                    diff.openings_removed,
                                    diff.openings_modified,
                                    diff.fixtures_added.len(),
                                    diff.fixtures_removed.len(),
                                    diff.fixtures_modified.len(),
                                ))
                                .size(11.5)
                                .color(TEXT_2),
                            );
                        });

                    if !diff.rooms_added.is_empty()
                        || !diff.rooms_removed.is_empty()
                        || !diff.rooms_modified.is_empty()
                    {
                        ui.add_space(8.0);
                        egui::ScrollArea::vertical()
                            .max_height(180.0)
                            .show(ui, |ui| {
                                for r in &diff.rooms_added {
                                    ui.label(
                                        RichText::new(format!(
                                            "+ Cômodo '{}' ({:.2} m²)",
                                            r.name,
                                            r.area_after.unwrap_or(0.0)
                                        )
                                        .replace('.', ","))
                                        .size(12.0)
                                        .color(DIFF_ADDED),
                                    );
                                }
                                for r in &diff.rooms_removed {
                                    ui.label(
                                        RichText::new(format!(
                                            "- Cômodo '{}' ({:.2} m²)",
                                            r.name,
                                            r.area_before.unwrap_or(0.0)
                                        )
                                        .replace('.', ","))
                                        .size(12.0)
                                        .color(DIFF_REMOVED),
                                    );
                                }
                                for r in &diff.rooms_modified {
                                    ui.label(
                                        RichText::new(format!(
                                            "~ Cômodo '{}': {:.2} m² → {:.2} m²",
                                            r.name,
                                            r.area_before.unwrap_or(0.0),
                                            r.area_after.unwrap_or(0.0)
                                        )
                                        .replace('.', ","))
                                        .size(12.0)
                                        .color(DIFF_MODIFIED),
                                    );
                                }
                            });
                    } else if diff.is_identical() {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("As duas versões são geometricamente idênticas.")
                                .size(12.5)
                                .color(SUCCESS),
                        );
                    }
                } else {
                    egui::Frame::new()
                        .fill(PANEL_ALT)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(8)
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Escolha uma versão anterior acima para visualizar as diferenças diretamente sobre a planta.")
                                    .size(12.5)
                                    .color(MUTED),
                            );
                        });
                }

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if self.compare_state.diff.is_some()
                        && secondary_button(ui, "Exportar comparação (SVG)...").clicked()
                    {
                        export_svg = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary_button(ui, "Fechar").clicked() {
                            close = true;
                        }
                    });
                });
            });

        if pick_file {
            self.pick_compare_baseline_file();
        }
        if let Some(idx) = pick_tab {
            self.compare_with_tab(idx);
        }
        if clear_compare {
            self.compare_state = CompareUiState::default();
            self.message = "Comparação visual desativada.".to_owned();
        }
        if export_svg {
            self.export_compare_svg();
        }
        if close || response.should_close() {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
