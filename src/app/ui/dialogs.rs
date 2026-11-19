use super::super::{FloorPlanApp, theme::*};
use super::icons::{self, Icon};
use super::widgets::{icon_button_sized, key_cap, primary_button, secondary_button};
use eframe::egui::{self, Key, Margin, RichText, Sense, Stroke, Ui, vec2};

pub(super) fn dialog_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(12)
        .inner_margin(Margin::same(22))
        .shadow(egui::Shadow {
            offset: [0, 10],
            blur: 34,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(22, 50, 92, 52),
        })
}

pub(super) fn backdrop() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(24, 46, 78, 70)
}

/// A dialog's title row: an icon tile, a title and a line under it.
pub(super) fn dialog_title(ui: &mut Ui, icon: Icon, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let (tile, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
        ui.painter().rect_filled(tile, 10, ACCENT_SOFT);
        icons::paint(
            ui.painter(),
            egui::Rect::from_center_size(tile.center(), vec2(24.0, 24.0)),
            icon,
            ACCENT_TEXT,
            ACCENT,
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(title).font(strong(17.0)).color(TEXT));
            ui.label(RichText::new(subtitle).color(MUTED));
        });
    });
    ui.add_space(14.0);
}

impl FloorPlanApp {
    pub(super) fn room_name_dialog(&mut self, ui: &Ui) {
        if self.pending_room.is_none() {
            return;
        }
        let mut confirm = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("room_name_dialog"))
            .frame(dialog_frame())
            .backdrop_color(backdrop())
            .show(ui.ctx(), |ui| {
                ui.set_width(360.0);
                let size = self
                    .pending_room
                    .map(|(a, b)| {
                        format!(
                            "{:.2} × {:.2} m · {:.2} m²",
                            (b.x - a.x).abs(),
                            (b.y - a.y).abs(),
                            (b.x - a.x).abs() * (b.y - a.y).abs()
                        )
                        .replace('.', ",")
                    })
                    .unwrap_or_default();
                dialog_title(ui, Icon::Room, "Novo cômodo", &size);
                ui.label(RichText::new("Nome do cômodo").color(TEXT_2));
                ui.add_space(4.0);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.room_name)
                        .hint_text("Ex.: Suíte, escritório, sala de estar")
                        .margin(Margin::symmetric(8, 6))
                        .desired_width(f32::INFINITY),
                );
                if self.room_dialog_focus {
                    edit.request_focus();
                    self.room_dialog_focus = false;
                }
                ui.add_space(18.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled_ui(!self.room_name.trim().is_empty(), |ui| {
                            primary_button(ui, "Criar cômodo")
                        })
                        .inner
                        .clicked()
                    {
                        confirm = true;
                    }
                    if secondary_button(ui, "Cancelar").clicked() {
                        cancel = true;
                    }
                });
                if ui.input(|input| input.key_pressed(Key::Enter))
                    && !self.room_name.trim().is_empty()
                {
                    confirm = true;
                }
            });
        if cancel {
            self.pending_room = None;
            self.message = "Criação do cômodo cancelada.".to_owned();
        } else if confirm && let Some((start, end)) = self.pending_room.take() {
            let previous = self.project.clone();
            if self
                .project
                .add_room(start, end, self.room_name.trim())
                .is_some()
            {
                self.record_change(previous);
                self.message = format!("Cômodo {} criado.", self.room_name.trim());
            }
        }
    }

    pub(super) fn unsaved_changes_dialog(&mut self, ctx: &egui::Context) {
        if self.pending_action.is_none() {
            return;
        }
        let mut save = false;
        let mut discard = false;
        let mut cancel = false;

        egui::Modal::new(egui::Id::unique("unsaved_changes_dialog"))
            .frame(dialog_frame())
            .backdrop_color(backdrop())
            .show(ctx, |ui| {
                ui.set_width(420.0);
                dialog_title(
                    ui,
                    Icon::Save,
                    "Salvar alterações?",
                    "O projeto possui alterações que ainda não foram salvas.",
                );
                ui.label(
                    RichText::new("Deseja salvar antes de continuar? As alterações não salvas serão perdidas se você descartá-las.")
                        .color(TEXT_2),
                );
                ui.add_space(18.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if primary_button(ui, "Salvar").clicked() {
                        save = true;
                    }
                    if secondary_button(ui, "Descartar").clicked() {
                        discard = true;
                    }
                    if secondary_button(ui, "Cancelar").clicked() {
                        cancel = true;
                    }
                });
                if ui.input(|input| input.key_pressed(Key::Escape)) {
                    cancel = true;
                }
            });

        if save {
            if self.save() {
                self.execute_pending_action(ctx);
            }
        } else if discard {
            self.discard_changes_and_proceed(ctx);
        } else if cancel {
            self.cancel_pending_action();
        }
    }

    /// Keyboard shortcuts and a short guide (F1).
    pub(super) fn help_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let mut close = false;
        let response = egui::Modal::new(egui::Id::unique("help_dialog"))
            .frame(dialog_frame())
            .backdrop_color(backdrop())
            .show(ctx, |ui| {
                ui.set_width(640.0);
                let close_rect = egui::Rect::from_min_size(
                    egui::pos2(ui.max_rect().right() - 26.0, ui.max_rect().top()),
                    vec2(26.0, 26.0),
                );
                if ui
                    .put(close_rect, |ui: &mut Ui| {
                        icon_button_sized(ui, Icon::Close, "Fechar (Esc)", false, 26.0, 14.0)
                    })
                    .clicked()
                {
                    close = true;
                }
                dialog_title(
                    ui,
                    Icon::Keyboard,
                    "Atalhos e guia rápido",
                    "Trabalhe mais rápido com o teclado.",
                );
                ui.columns(2, |columns| {
                    shortcut_group(
                        &mut columns[0],
                        "Ferramentas",
                        &[
                            (&["V"], "Selecionar e mover"),
                            (&["T"], "Terreno"),
                            (&["P"], "Parede"),
                            (&["C"], "Cômodo"),
                            (&["R"], "Porta"),
                            (&["J"], "Janela"),
                            (&["M"], "Medir (régua)"),
                        ],
                    );
                    shortcut_group(
                        &mut columns[0],
                        "Arquivo",
                        &[
                            (&["Ctrl", "N"], "Nova planta"),
                            (&["Ctrl", "O"], "Abrir projeto"),
                            (&["Ctrl", "S"], "Salvar"),
                        ],
                    );
                    shortcut_group(
                        &mut columns[1],
                        "Edição",
                        &[
                            (&["Ctrl", "Z"], "Desfazer"),
                            (&["Ctrl", "Y"], "Refazer"),
                            (&["Delete"], "Excluir seleção"),
                            (&["Setas"], "Mover seleção 0,25 m"),
                            (&["Shift", "Setas"], "Mover seleção 1 m"),
                            (&["G"], "Girar objeto 90°"),
                            (&["I"], "Porta: abrir para o outro lado"),
                            (&["Shift", "I"], "Porta: trocar a dobradiça"),
                            (&["Esc"], "Cancelar ação ou limpar seleção"),
                        ],
                    );
                    shortcut_group(
                        &mut columns[1],
                        "Vista e precisão",
                        &[
                            (&["F"], "Enquadrar tudo"),
                            (&["F3"], "Encaixe magnético"),
                            (&["F7"], "Mostrar grade"),
                            (&["F8"], "Modo orto"),
                            (&["F1"], "Esta ajuda"),
                        ],
                    );
                });
                ui.add_space(8.0);
                egui::Frame::new()
                    .fill(PANEL_ALT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(8)
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("Como usar").font(strong(13.0)).color(TEXT));
                        ui.add_space(4.0);
                        for tip in [
                            "Terreno, Parede e Cômodo: arraste, ou clique no primeiro e no segundo ponto.",
                            "Porta e Janela: clique numa parede, ou arraste o botão do ribbon até ela.",
                            "Roda do mouse: zoom no cursor · botão do meio: mover a vista · botão direito: menu de contexto.",
                            "Navegador do projeto: clique para selecionar, duplo clique para aproximar.",
                            "Valores numéricos aceitam vírgula decimal (ex.: 3,50).",
                        ] {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("•").color(ACCENT));
                                ui.label(RichText::new(tip).color(TEXT_2));
                            });
                        }
                    });
            });
        if close || response.should_close() {
            self.show_help = false;
        }
    }
}

fn shortcut_group(ui: &mut Ui, title: &str, entries: &[(&[&str], &str)]) {
    ui.label(RichText::new(title).font(strong(13.0)).color(TEXT));
    ui.add_space(4.0);
    egui::Grid::new(("shortcut_group", title))
        .num_columns(2)
        .spacing([14.0, 6.0])
        .min_col_width(90.0)
        .show(ui, |ui| {
            for (keys, action) in entries {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    for (index, key) in keys.iter().enumerate() {
                        if index > 0 {
                            ui.label(RichText::new("+").color(FAINT).size(11.0));
                        }
                        key_cap(ui, key);
                    }
                });
                ui.label(RichText::new(*action).color(TEXT_2));
                ui.end_row();
            }
        });
    ui.add_space(12.0);
}
