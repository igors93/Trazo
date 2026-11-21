//! The tab strip above the plan, and a second tab's plan shown beside the
//! active one with its own tab strip, draggable divider and navigation bar.

use eframe::egui::{self, CursorIcon, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};

use super::super::canvas::{self, PaintOptions, ViewTransform, overlay};
use super::super::tabs::{TabInfo, TabPane};
use super::super::{FloorPlanApp, theme::*};
use super::icons::Icon;
use super::widgets::{icon_button_sized, tip};

/// Height of the tab strip.
const STRIP: f32 = 31.0;

/// What a click in a tab strip asks for, applied after drawing it.
enum TabAction {
    Switch(usize),
    Close(usize),
    New(TabPane),
    NewLinked(usize, crate::domain::discipline::Discipline, TabPane),
    SetDiscipline(usize, crate::domain::discipline::Discipline),
    Duplicate(usize, TabPane),
    MoveToPane(usize, TabPane),
    SideBySide(Option<usize>),
}

impl FloorPlanApp {
    /// The plan area: one tab strip + canvas, or two resizable side-by-side
    /// panes each with its own tab strip and canvas controls.
    pub(super) fn plan_area(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        let side = self
            .split_tab
            .filter(|index| *index != self.active_tab() && self.tab_view(*index).is_some());

        match side {
            None => {
                let strip = Rect::from_min_size(full.min, vec2(full.width(), STRIP));
                let body = Rect::from_min_max(Pos2::new(full.left(), strip.bottom()), full.max);
                let action = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
                        self.render_tab_strip(ui, None, true)
                    })
                    .inner;
                self.apply_tab_action(action);
                ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| self.canvas(ui));
            }
            Some(side_index) => {
                let width = full.width().max(1.0);
                let min_pane = (width * 0.20).min(180.0);
                let min_x = full.left() + min_pane;
                let max_x = (full.right() - min_pane).max(min_x);
                let middle = (full.left() + width * self.split_ratio())
                    .clamp(min_x, max_x)
                    .round();

                let left_strip =
                    Rect::from_min_max(full.min, Pos2::new(middle - 1.0, full.top() + STRIP));
                let right_strip = Rect::from_min_max(
                    Pos2::new(middle + 1.0, full.top()),
                    Pos2::new(full.right(), full.top() + STRIP),
                );
                let left_body = Rect::from_min_max(
                    Pos2::new(full.left(), left_strip.bottom()),
                    Pos2::new(middle - 1.0, full.bottom()),
                );
                let right_body =
                    Rect::from_min_max(Pos2::new(middle + 1.0, right_strip.bottom()), full.max);

                // Render both tab strips before applying any click action.
                let left_action = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(left_strip), |ui| {
                        self.render_tab_strip(ui, Some(TabPane::Left), false)
                    })
                    .inner;
                let right_action = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(right_strip), |ui| {
                        self.render_tab_strip(ui, Some(TabPane::Right), true)
                    })
                    .inner;

                // Render the active editable canvas on its own side and the
                // matching companion canvas on the opposite side.
                let active_pane = self.active_pane();
                let mut switch_to_side = false;
                if active_pane == TabPane::Left {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(left_body), |ui| {
                        self.canvas(ui)
                    });
                    if self.side_plan(ui, right_body, side_index) {
                        switch_to_side = true;
                    }
                } else {
                    if self.side_plan(ui, left_body, side_index) {
                        switch_to_side = true;
                    }
                    ui.scope_builder(egui::UiBuilder::new().max_rect(right_body), |ui| {
                        self.canvas(ui)
                    });
                }

                // Draggable vertical divider spanning tab strips and canvases.
                self.paint_split_divider(ui, full, middle);

                if let Some(action) = left_action.or(right_action) {
                    self.apply_tab_action(Some(action));
                } else if switch_to_side {
                    self.switch_tab(side_index);
                }
            }
        }
    }

    fn paint_split_divider(&mut self, ui: &mut Ui, full: Rect, middle: f32) {
        let hit = Rect::from_center_size(
            Pos2::new(middle, full.center().y),
            vec2(10.0, full.height()),
        );
        let response = ui
            .interact(
                hit,
                ui.scope_id().with("split_divider"),
                Sense::click_and_drag(),
            )
            .on_hover_cursor(CursorIcon::ResizeHorizontal);

        if response.dragged()
            && let Some(pointer) = ui.ctx().pointer_interact_pos()
            && full.width() > 1.0
        {
            self.set_split_ratio((pointer.x - full.left()) / full.width());
        }
        if response.double_clicked() {
            self.set_split_ratio(0.5);
        }

        let active = response.hovered() || response.dragged();
        let line_color = if active { ACCENT } else { BORDER_STRONG };
        let line_width = if active { 2.5 } else { 1.5 };
        let painter = ui.painter();
        painter.line_segment(
            [
                Pos2::new(middle, full.top()),
                Pos2::new(middle, full.bottom()),
            ],
            Stroke::new(line_width, line_color),
        );

        // Tactile center grip pill along the canvas portion of the divider.
        let canvas_mid_y = (full.top() + STRIP + full.bottom()) * 0.5;
        let grip = Rect::from_center_size(Pos2::new(middle, canvas_mid_y), vec2(8.0, 28.0));
        painter.rect(
            grip,
            4.0,
            if active { ACCENT_SOFT } else { PANEL },
            Stroke::new(1.0, if active { ACCENT } else { BORDER_STRONG }),
            StrokeKind::Inside,
        );
        let dot_color = if active { ACCENT_TEXT } else { MUTED };
        for dy in [-6.0_f32, 0.0, 6.0] {
            painter.circle_filled(Pos2::new(middle, canvas_mid_y + dy), 1.15, dot_color);
        }

        let _ = tip(
            response,
            "Arraste para redimensionar os lados · duplo clique volta para 50/50",
        );
    }

    fn render_tab_strip(
        &self,
        ui: &mut Ui,
        pane_filter: Option<TabPane>,
        show_split_button: bool,
    ) -> Option<TabAction> {
        let strip = ui.max_rect();
        let painter = ui.painter();
        painter.rect_filled(strip, 0.0, BAND);
        let infos = self.tab_infos();
        let split = self.split_tab.is_some();
        let target_pane = pane_filter.unwrap_or(TabPane::Left);

        let visible_tabs: Vec<(usize, &TabInfo)> = infos
            .iter()
            .enumerate()
            .filter(|(_, info)| pane_filter.is_none_or(|pane| info.pane == pane))
            .collect();

        // Reserve room on the right for the compact action icons so tabs never
        // overlap them even when a pane is narrowed.
        let right_actions_w = if show_split_button { 62.0 } else { 34.0 };
        let tabs_rect = Rect::from_min_max(
            strip.min,
            Pos2::new(
                (strip.right() - right_actions_w).max(strip.left()),
                strip.bottom(),
            ),
        );
        let actions_rect = Rect::from_min_max(Pos2::new(tabs_rect.right(), strip.top()), strip.max);

        let mut action = None;
        let mut shown_span: Option<(f32, f32)> = None;
        let top_round = egui::CornerRadius {
            nw: 6,
            ne: 6,
            sw: 0,
            se: 0,
        };

        let pane_focused = pane_filter.is_none_or(|pane| pane == self.active_pane());

        // Left side of the strip: tabs + '+' button (horizontally scrollable if narrow).
        ui.scope_builder(egui::UiBuilder::new().max_rect(tabs_rect), |ui| {
            egui::ScrollArea::horizontal()
                .id_salt(("tab_strip_scroll", pane_filter))
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(1.0, 0.0);
                        ui.add_space(6.0);
                        let max_title_w = if split { 145.0 } else { 200.0 };

                        for (pos, &(index, info)) in visible_tabs.iter().enumerate() {
                            let is_shown = if split { info.shown } else { info.active };
                            let font = if is_shown {
                                strong(12.5)
                            } else {
                                regular(12.5)
                            };
                            let ink = if info.active {
                                TEXT
                            } else if is_shown {
                                TEXT_2
                            } else {
                                MUTED
                            };
                            let label = ui.painter().layout_no_wrap(info.title.clone(), font, ink);
                            let text_width = label.size().x.min(max_title_w);
                            let (slot, response) = ui.allocate_exact_size(
                                vec2(text_width + 44.0, STRIP),
                                Sense::click(),
                            );
                            let tab = Rect::from_min_max(
                                Pos2::new(slot.left(), slot.top() + 5.0),
                                slot.max,
                            );
                            let hovered = response.hovered();
                            let painter = ui.painter();

                            if is_shown {
                                painter.rect(
                                    tab,
                                    top_round,
                                    CANVAS,
                                    Stroke::new(1.0, BORDER),
                                    StrokeKind::Inside,
                                );
                                painter.line_segment(
                                    [
                                        Pos2::new(tab.left() + 1.0, tab.bottom() - 0.5),
                                        Pos2::new(tab.right() - 1.0, tab.bottom() - 0.5),
                                    ],
                                    Stroke::new(2.0, CANVAS),
                                );
                                // Subtle accent top indicator on the focused pane's active tab.
                                if info.active && pane_focused && split {
                                    painter.line_segment(
                                        [
                                            Pos2::new(tab.left() + 6.0, tab.top() + 1.0),
                                            Pos2::new(tab.right() - 6.0, tab.top() + 1.0),
                                        ],
                                        Stroke::new(2.0, ACCENT),
                                    );
                                }
                                shown_span = Some((
                                    tab.left().max(tabs_rect.left()),
                                    tab.right().min(tabs_rect.right()),
                                ));
                            } else if hovered {
                                painter.rect_filled(tab, top_round, ACCENT_SOFTER);
                            }

                            let y = tab.center().y;
                            let x = tab.left() + 13.0;
                            let clip = Rect::from_min_max(
                                Pos2::new(x, tab.top()),
                                Pos2::new(x + text_width, tab.bottom()),
                            );
                            painter.with_clip_rect(clip).galley(
                                Pos2::new(x, y - label.size().y / 2.0),
                                label,
                                ink,
                            );

                            let close = Rect::from_center_size(
                                Pos2::new(tab.right() - 15.0, y),
                                vec2(18.0, 18.0),
                            );
                            let over_close = response
                                .hover_pos()
                                .is_some_and(|position| close.contains(position));
                            if hovered || (is_shown && !info.dirty) {
                                if over_close {
                                    painter.circle_filled(close.center(), 9.0, ACCENT_SOFT);
                                }
                                let arm = 3.5;
                                let stroke =
                                    Stroke::new(1.4, if over_close { TEXT } else { MUTED });
                                let c = close.center();
                                painter.line_segment(
                                    [c + vec2(-arm, -arm), c + vec2(arm, arm)],
                                    stroke,
                                );
                                painter.line_segment(
                                    [c + vec2(-arm, arm), c + vec2(arm, -arm)],
                                    stroke,
                                );
                            } else if info.dirty {
                                painter.circle_filled(
                                    close.center(),
                                    3.5,
                                    if is_shown { TEXT_2 } else { MUTED },
                                );
                            }

                            let next_shown = visible_tabs.get(pos + 1).is_some_and(|(_, next)| {
                                if split { next.shown } else { next.active }
                            });
                            if !is_shown && !next_shown && !hovered {
                                painter.line_segment(
                                    [
                                        Pos2::new(tab.right() + 0.5, tab.top() + 6.0),
                                        Pos2::new(tab.right() + 0.5, tab.bottom() - 6.0),
                                    ],
                                    Stroke::new(1.0, BORDER_STRONG),
                                );
                            }

                            let response = tip(
                                response.on_hover_cursor(CursorIcon::PointingHand),
                                &format!(
                                    "{}{}{} · clique direito para disciplinas · Ctrl+W fecha",
                                    info.title,
                                    if info.linked {
                                        " [visão sincronizada]"
                                    } else {
                                        ""
                                    },
                                    if info.dirty {
                                        " (alterações não salvas)"
                                    } else {
                                        ""
                                    }
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
                                use crate::domain::discipline::Discipline;
                                ui.menu_button("Nova visão vinculada (mesmo projeto)", |ui| {
                                    for discipline in Discipline::ALL {
                                        if ui.button(discipline.label()).clicked() {
                                            action = Some(TabAction::NewLinked(
                                                index,
                                                discipline,
                                                target_pane,
                                            ));
                                            ui.close();
                                        }
                                    }
                                });
                                ui.menu_button("Disciplina desta aba", |ui| {
                                    for discipline in Discipline::ALL {
                                        let checked = info.discipline == discipline;
                                        if ui
                                            .selectable_label(checked, discipline.label())
                                            .clicked()
                                        {
                                            action =
                                                Some(TabAction::SetDiscipline(index, discipline));
                                            ui.close();
                                        }
                                    }
                                });
                                ui.separator();
                                if split {
                                    let move_label = match target_pane {
                                        TabPane::Left => "Mover para a direita",
                                        TabPane::Right => "Mover para a esquerda",
                                    };
                                    if ui.button(move_label).clicked() {
                                        action = Some(TabAction::MoveToPane(
                                            index,
                                            target_pane.opposite(),
                                        ));
                                        ui.close();
                                    }
                                } else if !info.active && ui.button("Mostrar ao lado").clicked() {
                                    action = Some(TabAction::SideBySide(Some(index)));
                                    ui.close();
                                }
                                if ui.button("Duplicar como cópia independente").clicked() {
                                    action = Some(TabAction::Duplicate(index, target_pane));
                                    ui.close();
                                }
                                if ui.button("Fechar aba").clicked() {
                                    action = Some(TabAction::Close(index));
                                    ui.close();
                                }
                            });
                        }

                        // New tab button ('+') right after the last tab of this strip.
                        ui.add_space(4.0);
                        let (slot, response) =
                            ui.allocate_exact_size(vec2(26.0, STRIP), Sense::click());
                        let button = Rect::from_center_size(
                            slot.center() + vec2(0.0, 2.5),
                            vec2(22.0, 22.0),
                        );
                        let painter = ui.painter();
                        if response.hovered() {
                            painter.rect_filled(button, 6.0, ACCENT_SOFT);
                        }
                        let c = button.center();
                        let stroke =
                            Stroke::new(1.6, if response.hovered() { TEXT } else { TEXT_2 });
                        painter.line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], stroke);
                        painter.line_segment([c + vec2(0.0, -5.0), c + vec2(0.0, 5.0)], stroke);
                        if tip(
                            response.on_hover_cursor(CursorIcon::PointingHand),
                            "Nova aba (Ctrl+T)",
                        )
                        .clicked()
                        {
                            action = Some(TabAction::New(target_pane));
                        }
                    });
                });
        });

        // Right edge of the strip: sleek icon buttons for Duplicate Tab & Split View.
        let source_for_duplicate = visible_tabs
            .iter()
            .find(|(_, info)| if split { info.shown } else { info.active })
            .map(|(index, _)| *index)
            .unwrap_or_else(|| self.active_tab());

        ui.scope_builder(egui::UiBuilder::new().max_rect(actions_rect), |ui| {
            ui.push_id(("tab_strip_actions", pane_filter), |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                    ui.add_space(6.0);
                    if show_split_button {
                        let split_tip = if split {
                            "Fechar vista lado a lado (voltar para vista única)"
                        } else {
                            "Ver lado a lado: compara duas abas simultaneamente"
                        };
                        if icon_button_sized(ui, Icon::SplitView, split_tip, split, 24.0, 16.0)
                            .clicked()
                        {
                            let other = (0..infos.len()).find(|index| *index != self.active_tab());
                            action = Some(TabAction::SideBySide(if split { None } else { other }));
                        }
                    }
                    if icon_button_sized(
                        ui,
                        Icon::DuplicateTab,
                        "Nova visão vinculada do mesmo projeto (outra disciplina sincronizada)",
                        false,
                        24.0,
                        16.0,
                    )
                    .clicked()
                    {
                        use crate::domain::discipline::Discipline;
                        let current = infos
                            .get(source_for_duplicate)
                            .map(|i| i.discipline)
                            .unwrap_or(Discipline::All);
                        let next_discipline = match current {
                            Discipline::All | Discipline::Architecture => Discipline::Electrical,
                            Discipline::Electrical => Discipline::Plumbing,
                            Discipline::Plumbing => Discipline::Structure,
                            Discipline::Structure => Discipline::Foundations,
                            Discipline::Foundations => Discipline::Architecture,
                        };
                        action = Some(TabAction::NewLinked(
                            source_for_duplicate,
                            next_discipline,
                            target_pane,
                        ));
                    }
                });
            });
        });

        // Bottom edge of this tab strip, open under the active/shown tab.
        let y = strip.bottom() - 0.5;
        let edge = Stroke::new(1.0, BORDER);
        match shown_span {
            Some((left, right)) if right > left => {
                ui.painter()
                    .line_segment([Pos2::new(strip.left(), y), Pos2::new(left, y)], edge);
                ui.painter()
                    .line_segment([Pos2::new(right, y), Pos2::new(strip.right(), y)], edge);
            }
            _ => {
                ui.painter().line_segment(
                    [Pos2::new(strip.left(), y), Pos2::new(strip.right(), y)],
                    edge,
                );
            }
        }

        action
    }

    fn apply_tab_action(&mut self, action: Option<TabAction>) {
        match action {
            Some(TabAction::Switch(index)) => self.switch_tab(index),
            Some(TabAction::Close(index)) => self.request_close_tab(index),
            Some(TabAction::New(pane)) => self.new_tab_in_pane(pane),
            Some(TabAction::NewLinked(index, discipline, pane)) => {
                self.new_linked_tab_in_pane(index, discipline, None, None, pane);
            }
            Some(TabAction::SetDiscipline(index, discipline)) => {
                let _ = self.configure_view_tab(index, Some(discipline), None, None);
            }
            Some(TabAction::Duplicate(index, pane)) => self.duplicate_tab_in_pane(index, pane),
            Some(TabAction::MoveToPane(index, pane)) => self.move_tab_to_pane(index, pane),
            Some(TabAction::SideBySide(Some(index))) => self.toggle_side_by_side(Some(index)),
            Some(TabAction::SideBySide(None)) => {
                if self.split_tab.is_none() && self.tab_count() < 2 {
                    self.message =
                        "Abra outra aba ou crie uma visão vinculada para ver lado a lado."
                            .to_owned();
                } else {
                    self.toggle_side_by_side(None);
                }
            }
            None => {}
        }
    }

    /// Companion tab's plan shown beside the active one, with matching floating
    /// zoom/grid navigation, view plate, scale bar and north arrow. Returns
    /// `true` when the user clicks the canvas area to switch editing focus.
    fn side_plan(&mut self, ui: &mut Ui, rect: Rect, index: usize) -> bool {
        let navigation = overlay::navigation_rect(rect);

        // Allocate the canvas hit region BEFORE the floating navigation buttons
        // so the buttons on top receive hover, tooltips and clicks first.
        let response = ui.interact(
            rect,
            ui.scope_id().with(("side_plan_canvas", index)),
            Sense::click(),
        );
        let pointer_over_nav = response
            .hover_pos()
            .or_else(|| response.interact_pointer_pos())
            .is_some_and(|pos| navigation.contains(pos));

        // Paint plan and static overlays directly from the live shared project.
        {
            let Some(side) = self.side_plan_view(index) else {
                return false;
            };
            let painter = ui.painter_at(rect);
            let view = ViewTransform {
                rect,
                scale: self.scale,
                pan: self.pan,
            };
            let bg_project = if side.discipline.focus().is_some() {
                side.base_project
                    .for_view(crate::domain::discipline::Discipline::All, &[])
            } else {
                std::borrow::Cow::Borrowed(side.shown_project.as_ref())
            };
            canvas::paint(
                &painter,
                view,
                &bg_project,
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
                    underlay_textures: &[],
                },
            );
            if side.discipline.focus().is_some() {
                painter.rect_filled(rect, 0.0, egui::Color32::from_white_alpha(195));
                let none_selected = |_: u64| false;
                canvas::paint_discipline_overlay(
                    &painter,
                    view,
                    &side.base_project,
                    side.discipline,
                    &none_selected,
                );
            }
            self.paint_side_view_label(&painter, rect, &side.floor_name, side.discipline);
            overlay::paint_scale_bar(&painter, view);
            overlay::paint_north_arrow(&painter, rect);
        }

        if response.hovered() && !pointer_over_nav {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll.abs() > 0.5 {
                let factor = (1.0 + scroll * 0.0025).clamp(0.75, 1.35);
                self.zoom_by(factor);
            }
            ui.painter_at(rect).rect_stroke(
                rect.shrink(1.0),
                0.0,
                Stroke::new(2.0, ACCENT_SOFT),
                StrokeKind::Inside,
            );
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

        // Floating zoom / fit / grid toolbar on top, identical to the active canvas.
        ui.push_id(("side_plan_navigation", index), |ui| {
            self.canvas_navigation(ui, navigation);
        });

        response.clicked() && !pointer_over_nav
    }
}
