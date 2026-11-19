use super::super::{FloorPlanApp, Selection, theme::*};
use super::icons::{self, Icon};
use super::widgets::{icon_button_sized, panel_header};
use crate::domain::{geometry::format_meters, model::OpeningKind};
use eframe::egui::{self, Align2, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2};

/// One element row of the browser.
struct Row {
    selection: Selection,
    label: String,
    value: String,
}

struct Group {
    key: &'static str,
    title: &'static str,
    icon: Icon,
    rows: Vec<Row>,
}

impl FloorPlanApp {
    fn browser_groups(&self) -> Vec<Group> {
        let project = &self.project;
        let terrain = project
            .terrain
            .iter()
            .map(|terrain| Row {
                selection: Selection::Terrain(terrain.id),
                label: "Terreno".to_owned(),
                value: format!("{:.2} m²", terrain.area()).replace('.', ","),
            })
            .collect();
        let rooms = project
            .rooms
            .iter()
            .map(|room| Row {
                selection: Selection::Room(room.id),
                label: room.display_name().to_owned(),
                value: project
                    .room_area(room)
                    .map(|area| format!("{area:.2} m²").replace('.', ","))
                    .unwrap_or_else(|| "—".to_owned()),
            })
            .collect();
        let walls = project
            .walls
            .iter()
            .enumerate()
            .map(|(index, wall)| Row {
                selection: Selection::Wall(wall.id),
                label: format!("Parede {:02}", index + 1),
                value: format_meters(wall.length()),
            })
            .collect();
        let openings = |accept: fn(OpeningKind) -> bool| {
            project
                .openings
                .iter()
                .filter(|opening| accept(opening.kind))
                .enumerate()
                .map(|(index, opening)| Row {
                    selection: Selection::Opening(opening.id),
                    label: format!("{} {:02}", opening.kind.label(), index + 1),
                    value: format_meters(opening.width()),
                })
                .collect()
        };
        let fixtures = project
            .fixtures
            .iter()
            .map(|fixture| Row {
                selection: Selection::Fixture(fixture.id),
                label: fixture.kind.label().to_owned(),
                value: format!("{:.2} × {:.2}", fixture.width, fixture.depth).replace('.', ","),
            })
            .collect();
        vec![
            Group {
                key: "terrain",
                title: "Terreno",
                icon: Icon::Terrain,
                rows: terrain,
            },
            Group {
                key: "rooms",
                title: "Cômodos",
                icon: Icon::Room,
                rows: rooms,
            },
            Group {
                key: "walls",
                title: "Paredes",
                icon: Icon::Wall,
                rows: walls,
            },
            Group {
                key: "doors",
                title: "Portas",
                icon: Icon::Door,
                rows: openings(OpeningKind::is_door),
            },
            Group {
                key: "windows",
                title: "Janelas",
                icon: Icon::Window,
                rows: openings(|kind| kind == OpeningKind::Window),
            },
            Group {
                key: "gaps",
                title: "Vãos livres",
                icon: Icon::Gap,
                rows: openings(|kind| kind == OpeningKind::Gap),
            },
            Group {
                key: "fixtures",
                title: "Objetos",
                icon: Icon::Fixture(crate::domain::fixtures::FixtureKind::Counter),
                rows: fixtures,
            },
            Group {
                key: "drawings",
                title: "Desenho 2D",
                icon: Icon::Glyph(super::Glyph::Polyline),
                rows: project
                    .drawings
                    .iter()
                    .map(|drawing| Row {
                        selection: Selection::Drawing(drawing.id),
                        label: drawing.shape.label().to_owned(),
                        value: format_meters(drawing.shape.length()),
                    })
                    .collect(),
            },
        ]
    }

    pub(super) fn project_browser(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let header = panel_header(ui, "Navegador do projeto", "");
        let close = Rect::from_center_size(
            Pos2::new(header.right() - 20.0, header.center().y),
            vec2(24.0, 24.0),
        );
        if ui
            .put(close, |ui: &mut Ui| {
                icon_button_small(ui, Icon::Close, "Ocultar o navegador")
            })
            .clicked()
        {
            self.show_browser = false;
        }

        egui::Frame::new()
            .inner_margin(Margin {
                left: 12,
                right: 12,
                top: 0,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.browser_search(ui);
            });

        let filter = self.browser_filter.trim().to_lowercase();
        let groups = self.browser_groups();
        if !filter.is_empty() {
            let found: Vec<Selection> = groups
                .iter()
                .flat_map(|group| {
                    group
                        .rows
                        .iter()
                        .filter(|row| {
                            row.label.to_lowercase().contains(&filter)
                                || group.title.to_lowercase().contains(&filter)
                        })
                        .map(|row| row.selection)
                })
                .collect();
            egui::Frame::new()
                .inner_margin(Margin {
                    left: 12,
                    right: 12,
                    top: 0,
                    bottom: 8,
                })
                .show(ui, |ui| {
                    let label = format!("Selecionar os {} encontrados", found.len());
                    if ui
                        .add_enabled_ui(!found.is_empty(), |ui| {
                            super::widgets::secondary_button(ui, &label)
                        })
                        .inner
                        .on_hover_text("Seleciona juntos todos os elementos que o filtro mostra")
                        .clicked()
                    {
                        self.select_many(found, false);
                    }
                });
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                self.views_group(ui);
                for group in groups {
                    let rows: Vec<&Row> = group
                        .rows
                        .iter()
                        .filter(|row| {
                            filter.is_empty()
                                || row.label.to_lowercase().contains(&filter)
                                || group.title.to_lowercase().contains(&filter)
                        })
                        .collect();
                    if !filter.is_empty() && rows.is_empty() {
                        continue;
                    }
                    let open =
                        self.group_header(ui, group.key, group.title, group.icon, rows.len());
                    if open {
                        if rows.is_empty() {
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.add_space(46.0);
                                ui.label(RichText::new("Nenhum item").color(FAINT).size(12.0));
                            });
                            ui.add_space(4.0);
                        }
                        for row in rows {
                            self.browser_row(ui, group.icon, row);
                        }
                    }
                    ui.add_space(2.0);
                }
                ui.add_space(12.0);
            });
    }

    fn browser_search(&mut self, ui: &mut Ui) {
        let frame = egui::Frame::new()
            .fill(PANEL_ALT)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(6)
            .inner_margin(Margin::symmetric(8, 3));
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (rect, _) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::hover());
                icons::paint(ui.painter(), rect, Icon::Search, MUTED, MUTED);
                let has_text = !self.browser_filter.is_empty();
                let width = ui.available_width() - if has_text { 22.0 } else { 0.0 };
                ui.add(
                    egui::TextEdit::singleline(&mut self.browser_filter)
                        .hint_text("Filtrar elementos")
                        .frame(egui::Frame::NONE)
                        .desired_width(width),
                );
                if has_text && icon_button_small(ui, Icon::Close, "Limpar filtro").clicked() {
                    self.browser_filter.clear();
                }
            });
        });
    }

    /// The views of the project: one floor plan for now.
    fn views_group(&mut self, ui: &mut Ui) {
        if self.group_header(ui, "views", "Vistas", Icon::Plan, 1) {
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(rect.left() + 34.0, rect.top()), rect.max),
                0,
                PANEL_ALT,
            );
            icons::paint(
                painter,
                Rect::from_center_size(
                    Pos2::new(rect.left() + 46.0, rect.center().y),
                    vec2(14.0, 14.0),
                ),
                Icon::Plan,
                TEXT_2,
                ACCENT,
            );
            painter.text(
                Pos2::new(rect.left() + 60.0, rect.center().y),
                Align2::LEFT_CENTER,
                "Planta baixa · Térreo",
                strong(12.5),
                TEXT,
            );
            painter.text(
                Pos2::new(rect.right() - 12.0, rect.center().y),
                Align2::RIGHT_CENTER,
                "aberta",
                regular(11.0),
                FAINT,
            );
        }
        ui.add_space(2.0);
    }

    /// A group's title row; returns whether the group is open.
    fn group_header(
        &mut self,
        ui: &mut Ui,
        key: &str,
        title: &str,
        icon: Icon,
        count: usize,
    ) -> bool {
        let id = ui.make_persistent_id(("browser_group", key));
        // Walls are the longest list; they start folded.
        let mut open = ui.data_mut(|data| *data.get_temp_mut_or(id, key != "walls"));
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
        if response.clicked() {
            open = !open;
            ui.data_mut(|data| data.insert_temp(id, open));
        }
        let painter = ui.painter();
        if response.hovered() {
            painter.rect_filled(rect, 0, ACCENT_SOFTER);
        }
        icons::paint(
            painter,
            Rect::from_center_size(
                Pos2::new(rect.left() + 14.0, rect.center().y),
                vec2(12.0, 12.0),
            ),
            if open {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            },
            MUTED,
            MUTED,
        );
        icons::paint(
            painter,
            Rect::from_center_size(
                Pos2::new(rect.left() + 32.0, rect.center().y),
                vec2(16.0, 16.0),
            ),
            icon,
            TEXT_2,
            ACCENT,
        );
        painter.text(
            Pos2::new(rect.left() + 46.0, rect.center().y),
            Align2::LEFT_CENTER,
            title,
            strong(12.5),
            TEXT,
        );
        let count_text = count.to_string();
        let galley = painter.layout_no_wrap(count_text, regular(11.0), MUTED);
        let pill = Rect::from_center_size(
            Pos2::new(rect.right() - 14.0 - galley.size().x / 2.0, rect.center().y),
            vec2(galley.size().x + 12.0, 17.0),
        );
        painter.rect_filled(pill, 8, BAND);
        painter.galley(
            Pos2::new(
                pill.center().x - galley.size().x / 2.0,
                pill.center().y - galley.size().y / 2.0,
            ),
            galley,
            MUTED,
        );
        response.on_hover_cursor(egui::CursorIcon::PointingHand);
        open
    }

    fn browser_row(&mut self, ui: &mut Ui, icon: Icon, row: &Row) {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
        let selected = self.is_selected(row.selection);
        let painter = ui.painter();
        if selected {
            painter.rect_filled(rect, 0, ACCENT_SOFT);
            painter.rect_filled(
                Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
                0,
                ACCENT,
            );
        } else if response.hovered() {
            painter.rect_filled(rect, 0, ACCENT_SOFTER);
        }
        if response.hovered() {
            self.browser_hover = Some(row.selection);
        }
        icons::paint(
            painter,
            Rect::from_center_size(
                Pos2::new(rect.left() + 46.0, rect.center().y),
                vec2(14.0, 14.0),
            ),
            icon,
            if selected { ACCENT_TEXT } else { MUTED },
            if selected { ACCENT } else { FAINT },
        );
        let value = painter.layout_no_wrap(row.value.clone(), regular(11.5), MUTED);
        let value_width = value.size().x;
        painter.galley(
            Pos2::new(
                rect.right() - 12.0 - value_width,
                rect.center().y - value.size().y / 2.0,
            ),
            value,
            MUTED,
        );
        let label_room = (rect.width() - 60.0 - value_width - 20.0).max(20.0);
        let label = truncate(painter, &row.label, label_room);
        painter.text(
            Pos2::new(rect.left() + 60.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            regular(12.5),
            if selected { ACCENT_TEXT } else { TEXT },
        );
        let response = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(
                "Clique para selecionar · Shift ou Ctrl+clique soma ou retira · duplo clique para aproximar",
            );
        let adding = ui.input(|input| input.modifiers.shift || input.modifiers.command);
        if response.double_clicked() {
            if !self.is_selected(row.selection) {
                self.select(row.selection);
            }
            self.zoom_to_selection(row.selection);
        } else if response.clicked() && adding {
            if self.tool != super::super::Tool::Move {
                self.set_tool(super::super::Tool::Move);
            }
            self.toggle_selection(row.selection);
        } else if response.clicked() {
            self.select(row.selection);
        }
    }
}

fn icon_button_small(ui: &mut Ui, icon: Icon, tooltip: &str) -> egui::Response {
    icon_button_sized(ui, icon, tooltip, false, 22.0, 13.0)
}

/// `text` shortened with an ellipsis to fit `width` pixels.
fn truncate(painter: &egui::Painter, text: &str, width: f32) -> String {
    let fits = |value: &str| {
        painter
            .layout_no_wrap(value.to_owned(), regular(12.5), TEXT)
            .size()
            .x
            <= width
    };
    if fits(text) {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    for count in (1..chars.len()).rev() {
        let candidate = format!("{}…", chars[..count].iter().collect::<String>().trim_end());
        if fits(&candidate) {
            return candidate;
        }
    }
    "…".to_owned()
}
