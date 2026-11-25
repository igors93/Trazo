//! Neutral review tools used for personal reminders and collaborative discussions.

use super::{
    icons::{Glyph, Icon},
    widgets::{
        danger_button, primary_button, ribbon_button, ribbon_panel, ribbon_small, ribbon_stack,
        secondary_button,
    },
};
use crate::app::{DocDialog, FloorPlanApp, Tool, theme::*};
use crate::domain::markups::MarkupKind;
use eframe::egui::{self, Color32, RichText, Ui};

impl FloorPlanApp {
    pub(super) fn markups_tab(&mut self, ui: &mut Ui) {
        self.sync_markup_context();
        ribbon_panel(ui, "Marcar", |ui| {
            self.tool_button(
                ui,
                Tool::Markup(None),
                Icon::Select,
                "Selecionar",
                "Arraste para mover; arraste os pontos azuis para redimensionar. Delete exclui a selecionada.",
            );
            for kind in MarkupKind::ALL {
                let icon = match kind {
                    MarkupKind::Rectangle => Icon::Glyph(Glyph::Rectangle),
                    MarkupKind::Circle => Icon::Glyph(Glyph::Circle),
                    MarkupKind::Arrow => Icon::Glyph(Glyph::Arrow),
                    MarkupKind::Freehand => Icon::Glyph(Glyph::Spline),
                };
                self.tool_button(
                    ui,
                    Tool::Markup(Some(kind)),
                    icon,
                    kind.label(),
                    "Marque uma área ou detalhe da planta. As marcações não alteram a construção.",
                );
            }
        });
        ribbon_panel(ui, "Cor", |ui| {
            ui.vertical(|ui| {
                ui.color_edit_button_srgb(&mut self.markup_state.color);
                ui.horizontal(|ui| {
                    for color in [
                        [220, 65, 65],
                        [231, 145, 32],
                        [35, 142, 86],
                        [38, 112, 216],
                        [143, 80, 191],
                    ] {
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(19.0, 19.0), egui::Sense::click());
                        ui.painter().circle_filled(
                            rect.center(),
                            7.0,
                            Color32::from_rgb(color[0], color[1], color[2]),
                        );
                        if response.clicked() {
                            self.markup_state.color = color;
                        }
                    }
                });
            });
        });
        ribbon_panel(ui, "Acompanhar", |ui| {
            if !self.markup_state.context_elements.is_empty() && ribbon_button(ui,Icon::Room,"Marcar seleção",false,false).on_hover_text("Cria uma marcação ao redor dos elementos selecionados antes de abrir esta aba e mantém os vínculos.").clicked() {self.mark_selection();}
            if ribbon_button(ui, Icon::TextNote, "Lista e comentários", false, false).clicked() {
                self.markup_state.edit = self
                    .project
                    .markups
                    .iter()
                    .find(|m| Some(m.id) == self.markup_state.selected)
                    .cloned();
                self.active_doc_dialog = DocDialog::Markups;
            }
            ribbon_stack(ui, |ui| {
                let mut settings = self.project.markup_settings;
                let changed = ui
                    .checkbox(&mut settings.visible, "Mostrar marcações")
                    .changed()
                    | ui.checkbox(&mut settings.show_resolved, "Mostrar resolvidas")
                        .changed()
                    | ui.checkbox(&mut settings.export, "Incluir na exportação")
                        .on_hover_text(
                            "Inclui as marcações visíveis nos arquivos de planta e prancha.",
                        )
                        .changed();
                if changed {
                    let before = self.project.clone();
                    self.project.markup_settings = settings;
                    self.record_change(before);
                }
            });
            if let Some(id) = self.markup_state.selected {
                ribbon_stack(ui, |ui| {
                    if ribbon_small(ui, Icon::TextNote, &format!("Editar M{id}"), false).clicked() {
                        self.markup_state.edit =
                            self.project.markups.iter().find(|m| m.id == id).cloned();
                        self.active_doc_dialog = DocDialog::Markups;
                    }
                    if ribbon_small(ui, Icon::Check, "Resolver / reabrir", false).clicked() {
                        let before = self.project.clone();
                        if let Some(m) = self.project.markups.iter_mut().find(|m| m.id == id) {
                            m.resolved = !m.resolved;
                            self.record_change(before);
                        }
                    }
                });
            }
        });
    }

    pub(super) fn markups_dialog(&mut self, ctx: &egui::Context) {
        self.sync_markup_context();
        if self.active_doc_dialog != DocDialog::Markups {
            return;
        }
        let mut close = false;
        let mut save = false;
        let mut delete = None;
        let mut focus = None;
        let response=egui::Modal::new(egui::Id::unique("markups_dialog"))
            .frame(super::dialogs::dialog_frame()).backdrop_color(super::dialogs::backdrop())
            .show(ctx,|ui| {
                ui.set_width(650.0_f32.min(ctx.content_rect().width()-48.0).max(280.0));
                super::dialogs::dialog_title(ui,Icon::TextNote,"Marcações","Lembretes, dúvidas e revisões deste pavimento. Selecione uma marcação para editar seu comentário.");
                ui.add(egui::TextEdit::singleline(&mut self.markup_state.search).hint_text("Buscar por identificador ou comentário").desired_width(f32::INFINITY));
                ui.add_space(8.0);
                let search=self.markup_state.search.trim().to_lowercase();
                let list:Vec<_>=self.project.markups.iter().filter(|m|search.is_empty() || m.label().to_lowercase().contains(&search) || m.comment.to_lowercase().contains(&search)).cloned().collect();
                egui::ScrollArea::vertical().id_salt("markup_list").max_height(210.0).show(ui,|ui| {
                    if list.is_empty() { ui.label("Nenhuma marcação encontrada. Use as ferramentas da aba Marcações para desenhar."); }
                    for m in list {
                        ui.horizontal(|ui| {
                            let color=Color32::from_rgb(m.color[0],m.color[1],m.color[2]);
                            ui.label(RichText::new("●").color(color));
                            let comment=if m.comment.is_empty() { "Sem comentário" } else { &m.comment };
                            let label=format!("{} · {} · {}",m.label(),if m.resolved { "Resolvida" } else { "Pendente" },comment);
                            if ui.add(egui::Button::selectable(self.markup_state.selected==Some(m.id),label).wrap()).clicked() { self.markup_state.selected=Some(m.id); self.markup_state.edit=Some(m.clone()); }
                            if secondary_button(ui,"Localizar").clicked() { focus=Some(m.id); close=true; }
                        });
                    }
                });
                ui.separator();
                if let Some(edit)=&mut self.markup_state.edit {
                    if !self.project.markups.iter().any(|m|m.id==edit.id) { ui.label("Esta marcação foi excluída."); }
                    else {
                        ui.horizontal(|ui| { ui.strong(format!("{} — {}",edit.label(),edit.kind.label())); ui.color_edit_button_srgb(&mut edit.color); ui.checkbox(&mut edit.resolved,"Resolvida"); });
                        ui.add(egui::TextEdit::multiline(&mut edit.comment).desired_rows(4).char_limit(4000).desired_width(f32::INFINITY).hint_text("O que você quer lembrar ou revisar aqui?"));
                        if edit.kind!=MarkupKind::Freehand {
                            ui.collapsing("Coordenadas (metros)",|ui| {for (i,p) in edit.points.iter_mut().enumerate() {ui.horizontal(|ui| {ui.label(format!("Ponto {}",i+1));ui.add(egui::DragValue::new(&mut p.x).speed(0.01).prefix("X: "));let mut y=-p.y;if ui.add(egui::DragValue::new(&mut y).speed(0.01).prefix("Y: ")).changed(){p.y = -y;}});}});
                        }
                        ui.horizontal(|ui| {
                            let (a,b)=edit.bounds();
                            ui.label(RichText::new(format!("Área: {:.2} × {:.2} m · {} elementos relacionados",b.x-a.x,b.y-a.y,(edit.related_elements(&self.project).len()+edit.related_resources(&self.project).len()))).size(11.0).color(TEXT_2));
                        });
                        ui.horizontal_wrapped(|ui| {
                            if primary_button(ui,"Salvar comentário").clicked() { save=true; }
                            if secondary_button(ui,"Localizar na planta").clicked() { focus=Some(edit.id); close=true; }
                            if danger_button(ui,"Excluir marcação").clicked() { delete=Some(edit.id); }
                        });
                    }
                } else {
                    ui.label(RichText::new("Selecione uma marcação da lista para editar.").color(MUTED));
                    ui.add_space(80.0);
                }
                ui.separator();
                ui.horizontal(|ui| { ui.label("Comentário das próximas marcações:"); ui.add(egui::TextEdit::singleline(&mut self.markup_state.comment).char_limit(4000).desired_width(250.0)); });
                if secondary_button(ui,"Fechar").clicked() { close=true; }
            });
        if save && let Some(edit) = self.markup_state.edit.clone() {
            let before = self.project.clone();
            match self.project.update_markup(edit) {
                Ok(()) => {
                    self.record_change(before);
                    self.message = "Marcação atualizada.".into();
                }
                Err(e) => self.message = e,
            }
        }
        if let Some(id) = delete {
            let before = self.project.clone();
            if self.project.delete_markup(id) {
                self.record_change(before);
                self.markup_state.edit = None;
                self.markup_state.selected = None;
            }
        }
        if let Some(id) = focus
            && let Some(m) = self.project.markups.iter().find(|m| m.id == id)
        {
            let (a, b) = m.bounds();
            self.zoom_target = Some((
                a.translated(crate::domain::geometry::Point::new(-1.0, -1.0)),
                b.translated(crate::domain::geometry::Point::new(1.0, 1.0)),
            ));
            self.markup_state.selected = Some(id);
            let before = self.project.clone();
            self.project.markup_settings.visible = true;
            if m.resolved {
                self.project.markup_settings.show_resolved = true;
            }
            self.record_change(before);
        }
        if close || response.should_close() {
            self.active_doc_dialog = DocDialog::None;
        }
    }
}
