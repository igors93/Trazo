//! The tab strip above the plan, and a second tab's plan shown beside the
//! active one.

use eframe::egui::{self, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, vec2};

use super::super::canvas::{self, PaintOptions, ViewTransform};
use super::super::{FloorPlanApp, theme::*};
use super::icons::{self, Icon};
use super::widgets::{icon_button_sized, tip};

/// Height of the tab strip.
const STRIP: f32 = 31.0;

/// What a click in the tab strip asks for, applied after drawing it.
enum TabAction {
    Switch(usize),
    Close(usize),
    New,
    Duplicate(usize),
    SideBySide(Option<usize>),
}

impl FloorPlanApp {
    /// The plan area: the tab strip, then one plan or two side by side.
    pub(super) fn plan_area(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        let strip = Rect::from_min_size(full.min, vec2(full.width(), STRIP));
        let body = Rect::from_min_max(Pos2::new(full.left(), strip.bottom()), full.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
            self.tab_strip(ui)
        });
        let side = self
            .split_tab
            .filter(|index| *index != self.active_tab() && self.tab_document(*index).is_some());
        match side {
            None => {
                ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| self.canvas(ui));
            }
            Some(index) => {
                let middle = body.center().x.round();
                let left = Rect::from_min_max(body.min, Pos2::new(middle - 1.0, body.bottom()));
                let right = Rect::from_min_max(Pos2::new(middle + 1.0, body.top()), body.max);
                ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| self.canvas(ui));
                self.side_plan(ui, right, index);
                ui.painter().line_segment(
                    [
                        Pos2::new(middle, body.top()),
                        Pos2::new(middle, body.bottom()),
                    ],
                    Stroke::new(2.0, BORDER_STRONG),
                );
            }
        }
    }

    fn tab_strip(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, PANEL);
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, BORDER),
        );
        let infos = self.tab_infos();
        let mut action = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
            ui.add_space(6.0);
            for (index, info) in infos.iter().enumerate() {
                let font = if info.active { strong(12.5) } else { regular(12.5) };
                let ink = if info.active { TEXT } else { TEXT_2 };
                let label = ui.painter().layout_no_wrap(info.title.clone(), font, ink);
                let text_width = label.size().x.min(220.0);
                let width = 14.0 + if info.dirty { 12.0 } else { 0.0 } + text_width + 30.0;
                let (tab, response) =
                    ui.allocate_exact_size(vec2(width, STRIP - 4.0), Sense::click());
                let painter = ui.painter();
                let fill = if info.active {
                    CANVAS
                } else if response.hovered() {
                    ACCENT_SOFTER
                } else {
                    PANEL
                };
                painter.rect(
                    tab.translate(vec2(0.0, 4.0)),
                    egui::CornerRadius {
                        nw: 7,
                        ne: 7,
                        sw: 0,
                        se: 0,
                    },
                    fill,
                    Stroke::new(1.0, if info.active { BORDER } else { PANEL }),
                    egui::StrokeKind::Inside,
                );
                if info.active {
                    painter.rect_filled(
                        Rect::from_min_size(tab.min + vec2(0.0, 4.0), vec2(tab.width(), 2.0)),
                        0.0,
                        ACCENT,
                    );
                }
                let mut x = tab.left() + 14.0;
                let y = tab.center().y + 2.0;
                if info.dirty {
                    painter.circle_filled(Pos2::new(x + 3.0, y), 3.0, WARNING);
                    x += 12.0;
                }
                let clip = Rect::from_min_max(
                    Pos2::new(x, tab.top()),
                    Pos2::new(x + text_width, tab.bottom()),
                );
                painter.with_clip_rect(clip).galley(
                    Pos2::new(x, y - label.size().y / 2.0),
                    label,
                    ink,
                );
                // The close button.
                let close = Rect::from_center_size(
                    Pos2::new(tab.right() - 14.0, y),
                    vec2(18.0, 18.0),
                );
                let over_close = response
                    .hover_pos()
                    .is_some_and(|position| close.contains(position));
                if over_close {
                    painter.rect_filled(close, 4.0, ACCENT_SOFT);
                }
                if response.hovered() || info.active {
                    icons::paint(
                        painter,
                        close.shrink(4.0),
                        Icon::Close,
                        if over_close { TEXT } else { MUTED },
                        MUTED,
                    );
                }
                let response = tip(
                    response.on_hover_cursor(CursorIcon::PointingHand),
                    &format!(
                        "{}{} · clique com o botão do meio ou Ctrl+W para fechar",
                        info.title,
                        if info.dirty { " (alterações não salvas)" } else { "" }
                    ),
                );
                if response.clicked() {
                    action = Some(if over_close {
                        TabAction::Close(index)
                    } else {
                        TabAction::Switch(index)
                    });
                }
                if response.middle_clicked() {
                    action = Some(TabAction::Close(index));
                }
                response.context_menu(|ui| {
                    if ui.button("Duplicar em nova aba").clicked() {
                        action = Some(TabAction::Duplicate(index));
                        ui.close();
                    }
                    if !info.active && ui.button("Mostrar ao lado").clicked() {
                        action = Some(TabAction::SideBySide(Some(index)));
                        ui.close();
                    }
                    if ui.button("Fechar aba").clicked() {
                        action = Some(TabAction::Close(index));
                        ui.close();
                    }
                });
            }
            ui.add_space(4.0);
            if icon_button_sized(ui, Icon::New, "Nova aba (Ctrl+T ou Ctrl+N)", false, 24.0, 15.0)
                .clicked()
            {
                action = Some(TabAction::New);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let split = self.split_tab.is_some();
                let side = small_toggle(ui, "Lado a lado", split);
                if tip(
                    side,
                    "Mostra outra aba ao lado, com a mesma vista: compare a planta elétrica com a estrutural do mesmo imóvel",
                )
                .clicked()
                {
                    let other = (0..infos.len()).find(|index| *index != self.active_tab());
                    action = Some(TabAction::SideBySide(if split { None } else { other }));
                }
                if tip(
                    small_toggle(ui, "Duplicar aba", false),
                    "Copia este projeto numa nova aba, como base para outra disciplina (elétrica, estrutural)",
                )
                .clicked()
                {
                    action = Some(TabAction::Duplicate(self.active_tab()));
                }
            });
        });
        match action {
            Some(TabAction::Switch(index)) => self.switch_tab(index),
            Some(TabAction::Close(index)) => self.request_close_tab(index),
            Some(TabAction::New) => self.new_tab(),
            Some(TabAction::Duplicate(index)) => {
                self.switch_tab(index);
                self.duplicate_tab();
            }
            Some(TabAction::SideBySide(Some(index))) => self.toggle_side_by_side(Some(index)),
            Some(TabAction::SideBySide(None)) => {
                if self.split_tab.is_none() && self.tab_count() < 2 {
                    self.message =
                        "Abra ou duplique outro projeto numa aba para ver lado a lado.".to_owned();
                } else {
                    self.toggle_side_by_side(None);
                }
            }
            None => {}
        }
    }

    /// Another tab's plan beside the active one, with the same view, so the
    /// same part of the building shows in both. A click makes it the one
    /// being edited.
    fn side_plan(&mut self, ui: &mut Ui, rect: Rect, index: usize) {
        let response = ui.allocate_rect(rect, Sense::click());
        let Some(document) = self.tab_document(index) else {
            return;
        };
        let painter = ui.painter_at(rect);
        let view = ViewTransform {
            rect,
            scale: self.scale,
            pan: self.pan,
        };
        canvas::paint(
            &painter,
            view,
            &document.project,
            PaintOptions {
                selection: None,
                extra_selection: &[],
                hover: None,
                pending: &[],
                preview: None,
                show_grid: self.show_grid,
                ghost: None,
                dimensions: self.show_dimensions,
                wall_reference: self.wall_reference,
            },
        );
        let title = self
            .tab_infos()
            .get(index)
            .map(|info| info.title.clone())
            .unwrap_or_default();
        let label = format!("{title} · clique para editar");
        let galley = painter.layout_no_wrap(label, regular(12.0), TEXT_2);
        let plate = Rect::from_min_size(
            rect.min + vec2(12.0, 10.0),
            galley.size() + vec2(20.0, 10.0),
        );
        painter.rect(
            plate,
            8.0,
            PANEL,
            Stroke::new(1.0, BORDER),
            egui::StrokeKind::Inside,
        );
        painter.galley(plate.min + vec2(10.0, 5.0), galley, TEXT_2);
        if response.hovered() {
            painter.rect_stroke(
                rect.shrink(1.0),
                0.0,
                Stroke::new(2.0, ACCENT_SOFT),
                egui::StrokeKind::Inside,
            );
        }
        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            self.switch_tab(index);
        }
    }
}

/// A compact text toggle for the tab strip.
fn small_toggle(ui: &mut Ui, label: &str, on: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        regular(12.0),
        if on { ACCENT_TEXT } else { TEXT_2 },
    );
    let (rect, response) =
        ui.allocate_exact_size(vec2(galley.size().x + 20.0, STRIP - 8.0), Sense::click());
    let painter = ui.painter();
    let fill = if on {
        ACCENT_SOFT
    } else if response.hovered() {
        ACCENT_SOFTER
    } else {
        PANEL
    };
    painter.rect(
        rect,
        6.0,
        fill,
        Stroke::new(1.0, if on { ACCENT } else { BORDER }),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        if on { ACCENT_TEXT } else { TEXT_2 },
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}
