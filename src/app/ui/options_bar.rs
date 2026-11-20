use super::super::{FloorPlanApp, Selection, Tool, theme::*};
use super::icons::{self, Icon};
use super::widgets::{danger_button, primary_button, secondary_button, tip, toggle_pill};
use crate::domain::{geometry::format_meters, model::OpeningKind};
use eframe::egui::{self, Key, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2};

mod drafting;
mod modify;

pub(super) fn tool_icon(tool: Tool) -> Icon {
    match tool {
        Tool::Move => Icon::Select,
        Tool::Terrain => Icon::Terrain,
        Tool::Wall => Icon::Wall,
        Tool::Room => Icon::Room,
        Tool::RoomPolygon => Icon::RoomPolygon,
        Tool::Opening(OpeningKind::Door) => Icon::Door,
        Tool::Opening(OpeningKind::Window) => Icon::Window,
        Tool::Opening(OpeningKind::SaloonDoor) => Icon::SaloonDoor,
        Tool::Opening(OpeningKind::Gap) => Icon::Gap,
        Tool::Opening(OpeningKind::Gate) => Icon::Gate,
        Tool::Fixture(kind) => Icon::Fixture(kind),
        Tool::Measure => Icon::Ruler,
        Tool::Draw(kind) => Icon::Glyph(kind.glyph()),
        Tool::Modify(kind) => Icon::Glyph(kind.glyph()),
        Tool::Constrain(kind) => Icon::Glyph(kind.glyph()),
    }
}

pub(super) fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Move => "Selecionar",
        Tool::Terrain => "Terreno",
        Tool::Wall => "Parede",
        Tool::Room => "Cômodo",
        Tool::RoomPolygon => "Cômodo livre",
        Tool::Opening(OpeningKind::Door) => "Porta",
        Tool::Opening(OpeningKind::Window) => "Janela",
        Tool::Opening(OpeningKind::SaloonDoor) => "Porta vai-e-vem",
        Tool::Opening(OpeningKind::Gap) => "Vão livre",
        Tool::Opening(OpeningKind::Gate) => "Portão",
        Tool::Fixture(kind) => kind.label(),
        Tool::Measure => "Medir",
        Tool::Draw(kind) => kind.label(),
        Tool::Modify(kind) => kind.label(),
        Tool::Constrain(kind) => kind.label(),
    }
}

fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(13.0, 22.0), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top()),
            Pos2::new(rect.center().x, rect.bottom()),
        ],
        Stroke::new(1.0, BORDER),
    );
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(MUTED).size(12.0));
}

fn chip(ui: &mut Ui, text: &str) -> egui::Response {
    egui::Frame::new()
        .fill(PANEL_ALT)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(TEXT_2).size(12.0));
        })
        .response
}

impl FloorPlanApp {
    pub(super) fn options_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.set_min_height(26.0);
            ui.spacing_mut().item_spacing.x = 6.0;

            // The active tool, as in CAD option bars.
            let tool = self.tool;
            let name = tool_name(tool);
            let galley = ui
                .painter()
                .layout_no_wrap(name.to_owned(), strong(13.0), ACCENT_TEXT);
            let (rect, _) =
                ui.allocate_exact_size(vec2(galley.size().x + 32.0, 24.0), Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(rect, 5, ACCENT_SOFT);
            icons::paint(
                painter,
                Rect::from_center_size(Pos2::new(rect.left() + 13.0, rect.center().y), vec2(16.0, 16.0)),
                tool_icon(tool),
                ACCENT_TEXT,
                ACCENT,
            );
            painter.galley(
                Pos2::new(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0),
                galley,
                ACCENT_TEXT,
            );
            divider(ui);

            if self.project.floors.levels.len() > 1 {
                let mut switch_to = None;
                let active_id = self.project.floors.active_floor_id;
                for level in &self.project.floors.levels {
                    if toggle_pill(
                        ui,
                        &level.name,
                        Some(level.id) == active_id,
                        &level.label_with_elevation(),
                    )
                    .clicked()
                    {
                        switch_to = Some(level.id);
                    }
                }
                if let Some(id) = switch_to {
                    let prev = self.project.clone();
                    if self.project.switch_floor(id).is_ok() {
                        self.clear_selection();
                        self.record_change(prev);
                    }
                }
                divider(ui);
            }

            match tool {
                Tool::Terrain => self.terrain_options(ui),
                Tool::Wall | Tool::Room => self.masonry_options(ui),
                Tool::Draw(kind) => self.draw_options(ui, kind),
                Tool::Modify(kind) => self.modify_options(ui, kind),
                Tool::Constrain(kind) => self.constrain_options(ui, kind),
                Tool::RoomPolygon => {
                    self.masonry_options(ui);
                    divider(ui);
                    let corners = self.polygon_draft.len();
                    chip(
                        ui,
                        &match corners {
                            0 => "nenhum canto".to_owned(),
                            1 => "1 canto".to_owned(),
                            count => format!("{count} cantos"),
                        },
                    );
                    caption(
                        ui,
                        "Feche no 1º canto, duplo clique ou Enter · Backspace desfaz o último canto",
                    );
                }
                Tool::Opening(kind) => {
                    caption(ui, "Largura padrão");
                    chip(ui, &format_meters(kind.default_width()));
                    caption(ui, "Limites");
                    chip(
                        ui,
                        &format!(
                            "{} – {}",
                            format_meters(kind.min_width()),
                            format_meters(kind.max_width())
                        ),
                    );
                    divider(ui);
                    caption(ui, "Clique numa parede para inserir.");
                }
                Tool::Fixture(kind) => {
                    let (width, depth) = kind.default_size();
                    caption(ui, "Tamanho");
                    chip(
                        ui,
                        &format!("{width:.2} × {depth:.2} m").replace('.', ","),
                    );
                    caption(ui, "Giro livre");
                    chip(ui, &format!("{}°", self.fixture_turns as u32 * 90));
                    if tip(
                        secondary_button(ui, "Girar (G)"),
                        "Gira o próximo objeto colocado longe das paredes",
                    )
                    .clicked()
                    {
                        self.fixture_turns = (self.fixture_turns + 1) % 4;
                    }
                    divider(ui);
                    caption(
                        ui,
                        "Perto de uma parede, ele encosta na face dos tijolos (parede de 12 cm).",
                    );
                }
                Tool::Move => self.selection_options(ui),
                Tool::Measure => {
                    caption(ui, "Medidas");
                    chip(ui, &self.measurements.len().to_string());
                    if ui
                        .add_enabled_ui(!self.measurements.is_empty(), |ui| {
                            secondary_button(ui, "Limpar medidas")
                        })
                        .inner
                        .clicked()
                    {
                        self.measurements.clear();
                    }
                    divider(ui);
                    caption(
                        ui,
                        "Clique em dois pontos. Encaixa em cantos, faces e batentes. F8 trava na horizontal/vertical; Esc limpa.",
                    );
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let aided = matches!(
                    tool,
                    Tool::Wall
                        | Tool::Room
                        | Tool::RoomPolygon
                        | Tool::Terrain
                        | Tool::Measure
                        | Tool::Draw(_)
                ) || matches!(tool, Tool::Modify(kind) if kind.uses_selection());
                if aided {
                    if matches!(tool, Tool::Wall | Tool::Room | Tool::RoomPolygon)
                        && toggle_pill(
                            ui,
                            "Modulação",
                            self.modulation_enabled,
                            "Comprimentos em múltiplos de meio bloco (bloco + junta)",
                        )
                        .clicked()
                    {
                        self.modulation_enabled = !self.modulation_enabled;
                    }
                    if matches!(
                        tool,
                        Tool::Wall
                            | Tool::RoomPolygon
                            | Tool::Measure
                            | Tool::Draw(_)
                            | Tool::Modify(_)
                    )
                        && toggle_pill(
                            ui,
                            "Orto",
                            self.ortho_enabled,
                            "Restringe a parede à horizontal ou vertical (F8)",
                        )
                        .clicked()
                    {
                        self.toggle_ortho();
                    }
                    if self.snap_enabled {
                        let osnap_btn = toggle_pill(
                            ui,
                            "OSNAP ▾",
                            self.osnap.any_enabled(),
                            "Configurar encaixes avançados (ponta, meio, interseção, centro, perpendicular, tangente e prolongamento)",
                        );
                        egui::Popup::menu(&osnap_btn).show(|ui| self.osnap_menu(ui));
                    }
                    if toggle_pill(
                        ui,
                        "Encaixe",
                        self.snap_enabled,
                        "Encaixe magnético: grade e encaixes geométricos OSNAP (F3)",
                    )
                    .clicked()
                    {
                        self.toggle_snap();
                    }
                    caption(ui, "Auxílios:");
                }
            });
        });
    }

    /// The brick or block walls are drawn with, chosen while drawing.
    fn masonry_options(&mut self, ui: &mut Ui) {
        let current = self.project.masonry.clone();
        caption(ui, "Material");
        let mut chosen = None;
        egui::ComboBox::from_id_salt("masonry_preset")
            .width(200.0)
            .selected_text(RichText::new(&current.name).size(12.0))
            .show_ui(ui, |ui| {
                for preset in crate::domain::masonry::Masonry::presets() {
                    let selected = preset.name == current.name;
                    if ui.selectable_label(selected, preset.label()).clicked() {
                        chosen = Some(preset);
                    }
                }
            });
        if let Some(preset) = chosen {
            self.set_masonry(preset);
        }
        let field = |ui: &mut Ui, text: &mut String, hint: &str| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .desired_width(34.0)
                    .align(egui::Align2::RIGHT_CENTER),
            )
            .on_hover_text(hint)
        };
        caption(ui, "Bloco");
        let width = field(
            ui,
            &mut self.masonry_width_text,
            "Largura do bloco (cm): a espessura bruta",
        );
        caption(ui, "×");
        let length = field(
            ui,
            &mut self.masonry_length_text,
            "Comprimento do bloco (cm): o módulo da parede",
        );
        caption(ui, "cm  Junta");
        let joint = field(ui, &mut self.masonry_joint_text, "Junta de argamassa (cm)");
        caption(ui, "Reboco");
        let plaster = field(
            ui,
            &mut self.masonry_plaster_text,
            "Reboco em cada face (cm)",
        );
        let typed = [width, length, joint, plaster]
            .iter()
            .any(|response| response.lost_focus());
        if typed {
            self.apply_masonry_fields();
        }
        caption(ui, "=");
        chip(
            ui,
            &format!("parede {:.1} cm", current.thickness() * 100.0)
                .replace(".0 ", " ")
                .replace('.', ","),
        )
        .on_hover_text("Espessura acabada: bloco + reboco nas duas faces");
        divider(ui);
        caption(ui, "Referência");
        egui::ComboBox::from_id_salt("wall_reference")
            .width(104.0)
            .selected_text(RichText::new(self.wall_reference.label()).size(12.0))
            .show_ui(ui, |ui| {
                for reference in crate::domain::wall_reference::WallReference::ALL {
                    let selected = self.wall_reference == reference;
                    if ui.selectable_label(selected, reference.label()).clicked() {
                        self.wall_reference = reference;
                        self.message =
                            format!("Referência da parede: {} (Tab alterna).", reference.label());
                    }
                }
            })
            .response
            .on_hover_text(
                "Alinha a parede pelo eixo, pela face externa ou pela face interna (Tab alterna)",
            );
    }

    fn terrain_options(&mut self, ui: &mut Ui) {
        caption(ui, "Largura");
        let width = ui.add(
            egui::TextEdit::singleline(&mut self.terrain_width_text)
                .desired_width(64.0)
                .align(egui::Align2::RIGHT_CENTER),
        );
        caption(ui, "m");
        caption(ui, "Comprimento");
        let height = ui.add(
            egui::TextEdit::singleline(&mut self.terrain_height_text)
                .desired_width(64.0)
                .align(egui::Align2::RIGHT_CENTER),
        );
        caption(ui, "m");
        let valid = self.terrain_dimensions().is_some();
        let label = if self.project.terrain.is_some() {
            "Aplicar"
        } else {
            "Criar terreno"
        };
        let clicked = ui
            .add_enabled_ui(valid, |ui| primary_button(ui, label))
            .inner
            .clicked();
        let enter = ui.input(|input| input.key_pressed(Key::Enter));
        if valid && (clicked || ((width.lost_focus() || height.lost_focus()) && enter)) {
            self.create_terrain_from_dimensions();
        }
        if let Some((w, h)) = self.terrain_dimensions() {
            divider(ui);
            caption(ui, "Área");
            chip(ui, &format!("{:.2} m²", w * h).replace('.', ","));
        } else {
            ui.label(
                RichText::new("Use valores entre 0,25 e 10.000 m.")
                    .color(DANGER)
                    .size(12.0),
            );
        }
    }

    /// The selection filter as a pill with its menu.
    fn selection_filter_pill(&mut self, ui: &mut Ui) {
        let label = if self.selection_filter.is_open() {
            "Filtro ▾"
        } else {
            "Filtro ativo ▾"
        };
        let pill = toggle_pill(
            ui,
            label,
            !self.selection_filter.is_open(),
            "Tipos de elemento que a seleção por janela, por cruzamento e Ctrl+A pegam",
        );
        egui::Popup::menu(&pill).show(|ui| self.selection_filter_menu(ui));
    }

    fn selection_options(&mut self, ui: &mut Ui) {
        if self.is_multi_selection() {
            caption(ui, "Selecionados:");
            chip(ui, &self.selection_summary());
            divider(ui);
            if tip(
                secondary_button(ui, "Aproximar"),
                "Aproximar a vista de todos os selecionados",
            )
            .clicked()
                && let Some(primary) = self.selection
            {
                self.zoom_to_selection(primary);
            }
            if tip(
                danger_button(ui, "Excluir"),
                "Excluir todos os selecionados (Delete)",
            )
            .clicked()
            {
                self.delete_selection();
            }
            divider(ui);
            self.selection_filter_pill(ui);
            return;
        }
        let Some(selection) = self.selection else {
            caption(
                ui,
                "Clique para selecionar · Shift+clique soma · arraste no vazio: → janela, ← cruzamento.",
            );
            if tip(
                secondary_button(ui, "Selecionar tudo"),
                "Seleciona todos os elementos que o filtro deixa passar (Ctrl+A)",
            )
            .clicked()
            {
                self.select_all();
            }
            self.selection_filter_pill(ui);
            return;
        };
        let (label, detail) = match selection {
            Selection::Wall(id) => (
                "Parede",
                self.project
                    .walls
                    .iter()
                    .find(|wall| wall.id == id)
                    .map(|wall| format_meters(wall.length()))
                    .unwrap_or_default(),
            ),
            Selection::Room(id) => (
                "Cômodo",
                self.project
                    .rooms
                    .iter()
                    .find(|room| room.id == id)
                    .map(|room| {
                        let area = self
                            .project
                            .room_area(room)
                            .map(|area| format!(" · {area:.2} m²").replace('.', ","))
                            .unwrap_or_default();
                        format!("{}{area}", room.display_name())
                    })
                    .unwrap_or_default(),
            ),
            Selection::Terrain(_) => (
                "Terreno",
                self.project
                    .terrain
                    .as_ref()
                    .map(|terrain| {
                        format!(
                            "{} × {} · {:.2} m²",
                            format_meters(terrain.width()),
                            format_meters(terrain.height()),
                            terrain.area()
                        )
                        .replace('.', ",")
                        .replace(" m ×", " ×")
                    })
                    .unwrap_or_default(),
            ),
            Selection::Drawing(id) => {
                let shape = self.project.drawing(id).map(|drawing| &drawing.shape);
                (
                    shape.map(|shape| shape.label()).unwrap_or("Desenho"),
                    shape
                        .map(|shape| format_meters(shape.length()))
                        .unwrap_or_default(),
                )
            }
            Selection::Fixture(id) => {
                let fixture = self.project.fixtures.iter().find(|f| f.id == id);
                (
                    fixture.map(|f| f.kind.label()).unwrap_or("Objeto"),
                    fixture
                        .map(|f| format!("{:.2} × {:.2} m", f.width, f.depth).replace('.', ","))
                        .unwrap_or_default(),
                )
            }
            Selection::Opening(id) => {
                let opening = self.project.openings.iter().find(|o| o.id == id);
                (
                    opening.map(|o| o.kind.label()).unwrap_or("Abertura"),
                    opening
                        .map(|o| format_meters(o.width()))
                        .unwrap_or_default(),
                )
            }
        };
        caption(ui, "Selecionado:");
        ui.label(RichText::new(label).font(strong(13.0)).color(TEXT));
        chip(ui, &detail);
        divider(ui);
        if tip(
            secondary_button(ui, "Aproximar"),
            "Aproximar a vista deste elemento",
        )
        .clicked()
        {
            self.zoom_to_selection(selection);
        }
        if tip(
            danger_button(ui, "Excluir"),
            "Excluir o elemento selecionado (Delete)",
        )
        .clicked()
        {
            self.delete_selection();
        }
    }
}
