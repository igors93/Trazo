//! The "Telhado" tab of the structure window (#114): mono-pitch timber
//! roofs, their framing sized piece by piece and where they bear.

use eframe::egui::{self, RichText, Ui};

use super::super::theme::*;
use super::widgets::{primary_button, secondary_button};
use crate::domain::geometry::Point;
use crate::domain::model::Project;
use crate::domain::roof::{Roof, RoofResult, RoofSide, TileKind, TimberClass};

/// What the roof tab asks for.
pub(super) enum RoofAction {
    Add(Roof),
    Update(u64, Roof),
    Remove(u64),
    FitWalls(u64),
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// The plan rectangle of the walls, or of the terrain.
fn plan_bounds(project: &Project) -> Option<(Point, Point)> {
    let mut points = project.walls.iter().flat_map(|w| [w.start, w.end]);
    let first = points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}

pub(super) fn roof_tab(
    ui: &mut Ui,
    project: &Project,
    results: &[RoofResult],
) -> Option<RoofAction> {
    let mut action = None;
    ui.add_space(6.0);
    ui.label(
        RichText::new("Meia-água de madeira: telhas sobre ripas, caibros, terças e vigas, dimensionados pela NBR 7190; o peso desce para os pilares e vigas embaixo de cada ponta das vigas do telhado.")
            .color(TEXT_2),
    );
    if primary_button(ui, "Nova meia-água sobre a planta").clicked()
        && let Some((min, max)) = plan_bounds(project)
    {
        let storey = project.wall_height.max(2.4);
        let run = (max.x - min.x).max(1.0);
        // 32 % clears the slope of ceramic tiles.
        let roof = Roof::new(min, max, RoofSide::West, storey + 0.32 * run, storey);
        action = Some(RoofAction::Add(roof));
    }
    for roof in &project.structure.roofs {
        let result = results.iter().find(|r| r.id == roof.id);
        let label = result.map_or_else(|| "T".to_owned(), |r| r.label.clone());
        ui.add_space(10.0);
        ui.separator();
        ui.label(RichText::new(format!("{label} · meia-água")).font(strong(13.5)));
        let mut edited = *roof;
        let mut changed = false;
        egui::Grid::new(("roof_settings", roof.id))
            .spacing([14.0, 6.0])
            .num_columns(4)
            .show(ui, |ui| {
                ui.label("Lado alto");
                egui::ComboBox::from_id_salt(("roof_side", roof.id))
                    .selected_text(edited.high_side.label())
                    .show_ui(ui, |ui| {
                        for side in RoofSide::ALL {
                            changed |= ui
                                .selectable_value(&mut edited.high_side, side, side.label())
                                .changed();
                        }
                    });
                ui.label("Telha");
                egui::ComboBox::from_id_salt(("roof_tile", roof.id))
                    .selected_text(edited.tile.label())
                    .show_ui(ui, |ui| {
                        for tile in TileKind::ALL {
                            changed |= ui
                                .selectable_value(&mut edited.tile, tile, tile.label())
                                .changed();
                        }
                    });
                ui.end_row();
                ui.label("Altura alta");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut edited.high_m)
                            .range(1.5..=12.0)
                            .speed(0.0)
                            .max_decimals(2)
                            .suffix(" m"),
                    )
                    .changed();
                ui.label("Madeira");
                egui::ComboBox::from_id_salt(("roof_timber", roof.id))
                    .selected_text(edited.timber.label())
                    .show_ui(ui, |ui| {
                        for class in TimberClass::ALL {
                            changed |= ui
                                .selectable_value(&mut edited.timber, class, class.label())
                                .changed();
                        }
                    });
                ui.end_row();
                ui.label("Altura baixa");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut edited.low_m)
                            .range(1.5..=12.0)
                            .speed(0.0)
                            .max_decimals(2)
                            .suffix(" m"),
                    )
                    .changed();
                ui.label("Vigas a cada");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut edited.beam_spacing_m)
                            .range(1.0..=6.0)
                            .speed(0.0)
                            .max_decimals(2)
                            .suffix(" m"),
                    )
                    .changed();
                ui.end_row();
                ui.label("Terças a cada");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut edited.purlin_spacing_m)
                            .range(0.5..=3.0)
                            .speed(0.0)
                            .max_decimals(2)
                            .suffix(" m"),
                    )
                    .changed();
                ui.label("Caibros a cada");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut edited.rafter_spacing_m)
                            .range(0.3..=1.0)
                            .speed(0.0)
                            .max_decimals(2)
                            .suffix(" m"),
                    )
                    .changed();
                ui.end_row();
                ui.label("Telhado aparente");
                changed |= ui.checkbox(&mut edited.exposed, "sem forro").changed();
                ui.label("Área");
                ui.label(format!(
                    "{} × {} m",
                    decimal(roof.max.x - roof.min.x, 2),
                    decimal(roof.max.y - roof.min.y, 2)
                ));
                ui.end_row();
            });
        if changed {
            action = Some(RoofAction::Update(roof.id, edited));
        }
        ui.horizontal_wrapped(|ui| {
            if secondary_button(ui, "Ajustar as paredes ao telhado")
                .on_hover_text("As paredes sob o telhado ganham a altura dele em cada ponta: a do lado alto sobe, os oitões ficam inclinados")
                .clicked()
            {
                action = Some(RoofAction::FitWalls(roof.id));
            }
            if secondary_button(ui, "Remover telhado").clicked() {
                action = Some(RoofAction::Remove(roof.id));
            }
        });
        let Some(result) = result else {
            continue;
        };
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "Inclinação {:.0} % (mínimo {:.0} %), cai para {}; carga {} kN/m² + {} kN/m² de sobrecarga na projeção; {} m² de telhado{}; {} m³ de madeira.",
                result.slope_percent,
                result.min_slope_percent,
                roof.high_side.opposite().label(),
                decimal(result.dead_kn_m2, 2),
                decimal(result.live_kn_m2, 2),
                decimal(result.surface_m2, 1),
                result
                    .tiles
                    .map_or(String::new(), |n| format!(", {n:.0} telhas")),
                decimal(result.timber_m3, 2)
            ))
            .color(if result.slope_ok() { TEXT_2 } else { DANGER }),
        );
        egui::Grid::new(("roof_elements", roof.id))
            .striped(true)
            .spacing([14.0, 4.0])
            .show(ui, |ui| {
                for title in [
                    "Peça",
                    "Seção",
                    "Vão",
                    "Espaçamento",
                    "Quantidade",
                    "Uso",
                    "Situação",
                ] {
                    ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
                }
                ui.end_row();
                for element in &result.elements {
                    ui.label(RichText::new(element.element.label()).font(strong(12.5)));
                    ui.label(format!(
                        "{} × {} cm",
                        decimal(element.section_cm.0, 1).replace(",0", ""),
                        decimal(element.section_cm.1, 1).replace(",0", "")
                    ));
                    ui.label(format!("{} m", decimal(element.span_m, 2)));
                    ui.label(format!("{} m", decimal(element.spacing_m, 2)));
                    ui.label(format!(
                        "{} × {} m",
                        element.count,
                        decimal(element.total_length_m / element.count.max(1) as f64, 2)
                    ));
                    let worst = element
                        .bending_ratio
                        .max(element.shear_ratio)
                        .max(element.deflection_ratio);
                    ui.label(format!("{:.0} %", worst * 100.0));
                    if element.ok {
                        ui.label(RichText::new("Atende").color(SUCCESS));
                    } else {
                        ui.label(RichText::new("Revise").color(WARNING));
                    }
                    ui.end_row();
                }
            });
        let loose = result.unsupported();
        let carried: Vec<String> = result
            .reactions
            .iter()
            .filter_map(|r| r.support.clone())
            .collect();
        ui.label(
            RichText::new(format!(
                "{} ponta(s) de viga: {} sobre {}{}.",
                result.reactions.len(),
                carried.len(),
                if carried.is_empty() {
                    "nada".to_owned()
                } else {
                    let mut names = carried.clone();
                    names.sort();
                    names.dedup();
                    names.join(", ")
                },
                if loose > 0 {
                    format!("; {loose} sem apoio embaixo (ponha pilares ou mourões)")
                } else {
                    String::new()
                }
            ))
            .size(11.5)
            .color(if loose > 0 { WARNING } else { MUTED }),
        );
    }
    action
}
