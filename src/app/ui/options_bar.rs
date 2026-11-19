use super::super::{FloorPlanApp, Selection, Tool, theme::*};
use super::icons::{self, Icon};
use super::widgets::{danger_button, primary_button, secondary_button, tip, toggle_pill};
use crate::domain::{geometry::format_meters, model::OpeningKind};
use eframe::egui::{self, Key, Pos2, Rect, RichText, Sense, Stroke, Ui, vec2};

pub(super) fn tool_icon(tool: Tool) -> Icon {
    match tool {
        Tool::Move => Icon::Select,
        Tool::Terrain => Icon::Terrain,
        Tool::Wall => Icon::Wall,
        Tool::Room => Icon::Room,
        Tool::Opening(OpeningKind::Door) => Icon::Door,
        Tool::Opening(OpeningKind::Window) => Icon::Window,
        Tool::Opening(OpeningKind::SaloonDoor) => Icon::SaloonDoor,
        Tool::Opening(OpeningKind::Gap) => Icon::Gap,
        Tool::Fixture(kind) => Icon::Fixture(kind),
        Tool::Measure => Icon::Ruler,
    }
}

pub(super) fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Move => "Selecionar",
        Tool::Terrain => "Terreno",
        Tool::Wall => "Parede",
        Tool::Room => "Cômodo",
        Tool::Opening(OpeningKind::Door) => "Porta",
        Tool::Opening(OpeningKind::Window) => "Janela",
        Tool::Opening(OpeningKind::SaloonDoor) => "Porta vai-e-vem",
        Tool::Opening(OpeningKind::Gap) => "Vão livre",
        Tool::Fixture(kind) => kind.label(),
        Tool::Measure => "Medir",
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

            match tool {
                Tool::Terrain => self.terrain_options(ui),
                Tool::Wall | Tool::Room => self.masonry_options(ui),
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
                if matches!(tool, Tool::Wall | Tool::Room | Tool::Terrain | Tool::Measure) {
                    if matches!(tool, Tool::Wall | Tool::Room)
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
                    if matches!(tool, Tool::Wall | Tool::Measure)
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
                    if toggle_pill(
                        ui,
                        "Encaixe",
                        self.snap_enabled,
                        "Encaixe na grade de 0,25 m e nas extremidades das paredes (F3)",
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

    fn selection_options(&mut self, ui: &mut Ui) {
        let Some(selection) = self.selection else {
            caption(
                ui,
                "Clique num elemento para selecioná-lo; arraste para mover. Roda: zoom · botão do meio: mover vista.",
            );
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
