use eframe::egui::{self, Margin, RichText, Stroke};

use super::super::{DocDialog, FloorPlanApp, theme::*};
use super::widgets::{primary_button, secondary_button, toggle_pill};
use crate::domain::{
    circulation::{CirculationKind, RailingSide, StairShape},
    geometry::Point,
};

impl FloorPlanApp {
    pub(super) fn circulation_dialog(&mut self, ctx: &egui::Context) {
        if self.active_doc_dialog != DocDialog::Circulation {
            return;
        }
        let mut open = true;
        let mut changed = false;

        egui::Window::new("Escadas, Rampas, Patamares, Guarda-corpos e Desníveis")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(680.0)
            .default_height(520.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Insira circulação vertical e transições de nível com cálculo automático da fórmula de Blondel e inclinação NBR 9050.",
                    )
                    .size(12.0)
                    .color(TEXT_2),
                );
                ui.add_space(6.0);

                let anchor = self
                    .project
                    .content_bounds()
                    .map(|(a, b)| a.midpoint(b))
                    .unwrap_or(Point::new(1.0, 1.0));

                ui.horizontal_wrapped(|ui| {
                    if primary_button(ui, "+ Escada reta").clicked() {
                        let prev = self.project.clone();
                        let rise = self.project.wall_height.max(2.6);
                        if self
                            .project
                            .add_stair(anchor, StairShape::Straight, 1.20, rise, 0.28)
                            .is_ok()
                        {
                            self.record_change(prev);
                        }
                    }
                    if secondary_button(ui, "+ Escada em L").clicked() {
                        let prev = self.project.clone();
                        let rise = self.project.wall_height.max(2.6);
                        if self
                            .project
                            .add_stair(anchor, StairShape::LShaped, 1.20, rise, 0.28)
                            .is_ok()
                        {
                            self.record_change(prev);
                        }
                    }
                    if secondary_button(ui, "+ Escada em U").clicked() {
                        let prev = self.project.clone();
                        let rise = self.project.wall_height.max(2.6);
                        if self
                            .project
                            .add_stair(anchor, StairShape::UShaped, 1.20, rise, 0.28)
                            .is_ok()
                        {
                            self.record_change(prev);
                        }
                    }
                    if secondary_button(ui, "+ Rampa NBR 9050").clicked() {
                        let prev = self.project.clone();
                        if self.project.add_ramp(anchor, 1.20, 6.00, 0.50).is_ok() {
                            self.record_change(prev);
                        }
                    }
                    if secondary_button(ui, "+ Patamar").clicked() {
                        let prev = self.project.clone();
                        if self.project.add_landing(anchor, 1.50, 1.50, 0.30).is_ok() {
                            self.record_change(prev);
                        }
                    }
                    if secondary_button(ui, "+ Guarda-corpo").clicked() {
                        let prev = self.project.clone();
                        if self
                            .project
                            .add_railing(anchor, anchor.translated(Point::new(2.5, 0.0)), 1.10)
                            .is_ok()
                        {
                            self.record_change(prev);
                        }
                    }
                    if secondary_button(ui, "+ Desnível").clicked() {
                        let prev = self.project.clone();
                        if self
                            .project
                            .add_level_change(
                                anchor,
                                anchor.translated(Point::new(2.0, 0.0)),
                                0.0,
                                -0.15,
                            )
                            .is_ok()
                        {
                            self.record_change(prev);
                        }
                    }
                });

                ui.add_space(8.0);
                ui.separator();

                if self.project.circulations.is_empty() {
                    ui.label(
                        RichText::new("Nenhum elemento de circulação vertical cadastrado ainda.")
                            .size(12.0)
                            .color(MUTED),
                    );
                    return;
                }

                let mut remove_id = None;
                let mut apply_proposal: Option<(u64, crate::domain::codes::StairProposal)> = None;

                // Precompute 26-rule stair findings and solver proposals before mutably iterating.
                let mut stair_data: std::collections::BTreeMap<
                    u64,
                    (
                        Vec<crate::domain::codes::StairFinding>,
                        Vec<crate::domain::codes::StairProposal>,
                        crate::domain::codes::StairLocks,
                    ),
                > = std::collections::BTreeMap::new();
                for elem in &self.project.circulations {
                    if matches!(elem.kind, CirculationKind::Stair { .. }) {
                        let lock_id = egui::Id::unique(("trazo_stair_locks", elem.id));
                        let locks: crate::domain::codes::StairLocks =
                            ctx.data(|d| d.get_temp(lock_id)).unwrap_or_default();
                        let findings = crate::domain::codes::stair_findings(&self.project, elem);
                        let proposals = crate::domain::codes::solve_stair(&self.project, elem, locks);
                        stair_data.insert(elem.id, (findings, proposals, locks));
                    }
                }

                egui::ScrollArea::vertical()
                    .max_height(370.0)
                    .show(ui, |ui| {
                        for elem in &mut self.project.circulations {
                            let summary = elem.summary();
                            let id = elem.id;
                            egui::Frame::new()
                                .fill(PANEL_ALT)
                                .stroke(Stroke::new(1.0, BORDER))
                                .corner_radius(8)
                                .inner_margin(Margin::same(10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(&elem.name)
                                                .font(strong(12.5))
                                                .color(TEXT),
                                        );
                                        ui.label(RichText::new(summary).size(11.5).color(ACCENT_TEXT));
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.small_button("Excluir").clicked() {
                                                    remove_id = Some(id);
                                                }
                                                if ui.small_button("Girar 90°").clicked() {
                                                    elem.rotation_deg =
                                                        (elem.rotation_deg + 90.0) % 360.0;
                                                    changed = true;
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(4.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label("X:");
                                        changed |= ui
                                            .add(
                                                egui::DragValue::new(&mut elem.origin.x)
                                                    .speed(0.1)
                                                    .suffix(" m"),
                                            )
                                            .changed();
                                        ui.label("Y:");
                                        changed |= ui
                                            .add(
                                                egui::DragValue::new(&mut elem.origin.y)
                                                    .speed(0.1)
                                                    .suffix(" m"),
                                            )
                                            .changed();
                                        match &mut elem.kind {
                                            CirculationKind::Stair {
                                                shape,
                                                width_m,
                                                total_rise_m,
                                                tread_m,
                                                ..
                                            } => {
                                                ui.label("Larg.:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(width_m)
                                                            .range(0.60..=4.0)
                                                            .speed(0.05)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                                ui.label("Desnível:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(total_rise_m)
                                                            .range(0.30..=6.0)
                                                            .speed(0.05)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                                ui.label("Piso:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(tread_m)
                                                            .range(0.22..=0.45)
                                                            .speed(0.01)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                                for st in StairShape::ALL {
                                                    if toggle_pill(
                                                        ui,
                                                        st.label(),
                                                        *shape == st,
                                                        st.label(),
                                                    )
                                                    .clicked()
                                                    {
                                                        *shape = st;
                                                        changed = true;
                                                    }
                                                }
                                            }
                                            CirculationKind::Ramp {
                                                width_m,
                                                length_m,
                                                total_rise_m,
                                                ..
                                            } => {
                                                ui.label("Larg.:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(width_m)
                                                            .range(0.80..=5.0)
                                                            .speed(0.05)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                                ui.label("Comp.:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(length_m)
                                                            .range(0.80..=30.0)
                                                            .speed(0.1)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                                ui.label("Desnível:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(total_rise_m)
                                                            .range(0.05..=4.0)
                                                            .speed(0.05)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                            }
                                            CirculationKind::Landing {
                                                width_m,
                                                depth_m,
                                                elevation_m,
                                            } => {
                                                ui.label("Larg.:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(width_m)
                                                            .range(0.5..=10.0)
                                                            .speed(0.1)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                                ui.label("Prof.:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(depth_m)
                                                            .range(0.5..=10.0)
                                                            .speed(0.1)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                                ui.label("Nível:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(elevation_m)
                                                            .range(-10.0..=20.0)
                                                            .speed(0.05)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                            }
                                            CirculationKind::Railing { height_m, .. } => {
                                                ui.label("Altura:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(height_m)
                                                            .range(0.80..=1.80)
                                                            .speed(0.05)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                            }
                                            CirculationKind::LevelChange {
                                                from_elev_m,
                                                to_elev_m,
                                                ..
                                            } => {
                                                ui.label("De:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(from_elev_m)
                                                            .speed(0.05)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                                ui.label("Para:");
                                                changed |= ui
                                                    .add(
                                                        egui::DragValue::new(to_elev_m)
                                                            .speed(0.05)
                                                            .suffix(" m"),
                                                    )
                                                    .changed();
                                            }
                                        }
                                        if matches!(
                                            elem.kind,
                                            CirculationKind::Stair { .. }
                                                | CirculationKind::Ramp { .. }
                                        ) {
                                            ui.label("Guarda-corpo:");
                                            for side in RailingSide::ALL {
                                                if toggle_pill(
                                                    ui,
                                                    side.label(),
                                                    elem.railing == side,
                                                    side.label(),
                                                )
                                                .clicked()
                                                {
                                                    elem.railing = side;
                                                    changed = true;
                                                }
                                            }
                                        }
                                    });
                                    // 26-rule normative compliance & interactive stair solver (T-10)
                                    if let Some((findings, proposals, mut locks)) =
                                        stair_data.get(&id).cloned()
                                    {
                                        ui.add_space(4.0);
                                        if findings.is_empty() {
                                            ui.label(
                                                RichText::new(
                                                    "✓ Escada conforme às 26 regras normativas ativas.",
                                                )
                                                .size(11.5)
                                                .color(SUCCESS),
                                            );
                                        } else {
                                            ui.label(
                                                RichText::new(format!(
                                                    "⚠ {} desvio(s) normativo(s) encontrado(s):",
                                                    findings.len()
                                                ))
                                                .size(11.5)
                                                .color(DANGER),
                                            );
                                            for f in findings.iter().take(4) {
                                                ui.label(
                                                    RichText::new(format!("• {}", f.message))
                                                        .size(11.0)
                                                        .color(TEXT_2),
                                                );
                                            }
                                            if findings.len() > 4 {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "… e mais {} achado(s) em Normas / Segurança.",
                                                        findings.len() - 4
                                                    ))
                                                    .size(11.0)
                                                    .color(MUTED),
                                                );
                                            }
                                        }
                                        egui::CollapsingHeader::new(
                                            RichText::new("Solver de Escadas (soluções sugeridas)")
                                                .size(11.5)
                                                .color(ACCENT_TEXT),
                                        )
                                        .id_salt(("stair_solver_hdr", id))
                                        .default_open(!findings.is_empty())
                                        .show(ui, |ui| {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label(
                                                    RichText::new("Travas:").font(strong(11.0)),
                                                );
                                                let mut lock_changed = false;
                                                lock_changed |=
                                                    ui.checkbox(&mut locks.risers, "Espelhos").changed();
                                                lock_changed |=
                                                    ui.checkbox(&mut locks.tread, "Piso").changed();
                                                lock_changed |=
                                                    ui.checkbox(&mut locks.width, "Largura").changed();
                                                lock_changed |= ui
                                                    .checkbox(&mut locks.landings, "Patamares")
                                                    .changed();
                                                if lock_changed {
                                                    let lock_id =
                                                        egui::Id::unique(("trazo_stair_locks", id));
                                                    ctx.data_mut(|d| d.insert_temp(lock_id, locks));
                                                }
                                            });
                                            if proposals.is_empty() {
                                                ui.label(
                                                    RichText::new(
                                                        "Sem ajustes necessários ou travas muito restritivas.",
                                                    )
                                                    .size(11.0)
                                                    .color(MUTED),
                                                );
                                            } else {
                                                for (p_idx, prop) in proposals.iter().enumerate() {
                                                    ui.horizontal_wrapped(|ui| {
                                                        let badge = if prop.compliant {
                                                            RichText::new("OK").color(SUCCESS).font(strong(11.0))
                                                        } else {
                                                            RichText::new(format!(
                                                                "{} pend.",
                                                                prop.remaining_findings
                                                            ))
                                                            .color(WARNING)
                                                            .font(strong(11.0))
                                                        };
                                                        ui.label(badge);
                                                        let desc = if prop.changes.is_empty() {
                                                            "Mantém medidas atuais".to_owned()
                                                        } else {
                                                            prop.changes.join(" · ")
                                                        };
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "#{} · {} (e={:.1} cm, p={:.0} cm, Blondel {:.0} cm)",
                                                                p_idx + 1,
                                                                desc,
                                                                prop.riser_height_m * 100.0,
                                                                prop.tread_m * 100.0,
                                                                prop.blondel_m * 100.0
                                                            ))
                                                            .size(11.0),
                                                        );
                                                        if !prop.changes.is_empty()
                                                            && ui.small_button("Aplicar").clicked()
                                                        {
                                                            apply_proposal = Some((id, prop.clone()));
                                                        }
                                                    });
                                                }
                                            }
                                        });
                                    }
                                });
                            ui.add_space(6.0);
                        }
                    });
                if let Some((stair_id, proposal)) = apply_proposal {
                    let prev = self.project.clone();
                    if let Some(el) = self
                        .project
                        .circulations
                        .iter_mut()
                        .find(|c| c.id == stair_id)
                        && crate::domain::codes::apply_stair_proposal(el, &proposal).is_ok()
                    {
                        self.record_change(prev);
                        self.message = format!(
                            "Proposta do Solver aplicada à escada ({} espelhos, piso {:.0} cm).",
                            proposal.risers,
                            proposal.tread_m * 100.0
                        );
                    }
                }
                if let Some(id) = remove_id {
                    let prev = self.project.clone();
                    self.project.remove_circulation(id);
                    self.record_change(prev);
                }
            });

        if changed {
            self.mark_edited_outside_history();
        }
        if !open {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
