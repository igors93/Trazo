//! The "Telhado" tab of the structure window (#114): mono-pitch timber
//! roofs, their framing sized piece by piece and where they bear, and the
//! timber posts with the timber beams over them (#146).

use eframe::egui::{self, RichText, Ui};

use super::super::theme::*;
use super::widgets::{primary_button, secondary_button};
use crate::domain::geometry::Point;
use crate::domain::model::Project;
use crate::domain::roof::{
    Roof, RoofElement, RoofResult, RoofSide, RoofStyle, TileKind, TimberClass, TimberSpecies,
};
use crate::domain::structure::StructureReport;

/// What the roof tab asks for.
pub(super) enum RoofAction {
    Add(Roof),
    Update(u64, Roof),
    Remove(u64),
    FitWalls(u64),
    /// Species/catalogued class and treatment of every timber post.
    PostSpecies(TimberSpecies),
    PostTreated(bool),
    /// The site's wind (#147).
    Wind(crate::domain::wind::WindSettings),
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
    report: &StructureReport,
) -> Option<RoofAction> {
    let results: &[RoofResult] = &report.roofs;
    let mut action = None;
    ui.add_space(6.0);
    ui.label(
        RichText::new("Telhados de madeira (meia-água, duas águas ou quatro águas): telhas sobre ripas, caibros, terças, frechais, vigas/tesouras e espigões dimensionados pela NBR 7190, com beirais e sucção de vento NBR 6123.")
            .color(TEXT_2),
    );
    ui.horizontal_wrapped(|ui| {
        if primary_button(ui, "Nova meia-água").clicked()
            && let Some((min, max)) = plan_bounds(project)
        {
            let storey = project.wall_height.max(2.4);
            let run = (max.x - min.x).max(1.0);
            let roof = Roof::new(min, max, RoofSide::West, storey + 0.32 * run, storey);
            action = Some(RoofAction::Add(roof));
        }
        if secondary_button(ui, "Novo duas águas").clicked()
            && let Some((min, max)) = plan_bounds(project)
        {
            let storey = project.wall_height.max(2.4);
            let half = (max.x - min.x).max(1.0) * 0.5;
            let mut roof = Roof::new(min, max, RoofSide::West, storey + 0.32 * half, storey);
            roof.style = RoofStyle::Gable;
            roof.overhang_m = 0.60;
            roof.use_trusses = true;
            action = Some(RoofAction::Add(roof));
        }
        if secondary_button(ui, "Novo quatro águas").clicked()
            && let Some((min, max)) = plan_bounds(project)
        {
            let storey = project.wall_height.max(2.4);
            let half = (max.x - min.x).min(max.y - min.y).max(1.0) * 0.5;
            let mut roof = Roof::new(min, max, RoofSide::West, storey + 0.32 * half, storey);
            roof.style = RoofStyle::Hip;
            roof.overhang_m = 0.60;
            roof.use_trusses = true;
            action = Some(RoofAction::Add(roof));
        }
    });
    for roof in &project.structure.roofs {
        let result = results.iter().find(|r| r.id == roof.id);
        let label = result.map_or_else(|| "T".to_owned(), |r| r.label.clone());
        ui.add_space(10.0);
        ui.separator();
        ui.label(
            RichText::new(format!("{label} · {}", roof.style.label().to_lowercase()))
                .font(strong(13.5)),
        );
        let mut edited = roof.clone();
        let mut changed = false;
        egui::Grid::new(("roof_settings", roof.id))
            .spacing([14.0, 6.0])
            .num_columns(4)
            .show(ui, |ui| {
                ui.label("Tipo");
                egui::ComboBox::from_id_salt(("roof_style", roof.id))
                    .selected_text(edited.style.label())
                    .show_ui(ui, |ui| {
                        for style in RoofStyle::ALL {
                            changed |= ui
                                .selectable_value(&mut edited.style, style, style.label())
                                .changed();
                        }
                    });
                ui.label("Beiral");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut edited.overhang_m)
                            .range(0.0..=1.5)
                            .speed(0.02)
                            .max_decimals(2)
                            .suffix(" m"),
                    )
                    .changed();
                ui.end_row();
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
                ui.label("Viga principal");
                let mut fixed = edited.beam_section_cm.is_some();
                let mut section = edited.beam_section_cm.unwrap_or((10.0, 30.0));
                if ui.checkbox(&mut fixed, "secao fixa").changed() {
                    edited.beam_section_cm = fixed.then_some(section);
                    changed = true;
                }
                ui.label("Largura x altura");
                ui.add_enabled_ui(fixed, |ui| {
                    let width_changed = ui
                        .add(
                            egui::DragValue::new(&mut section.0)
                                .range(5.0..=30.0)
                                .suffix(" cm"),
                        )
                        .changed();
                    ui.label("x");
                    let height_changed = ui
                        .add(
                            egui::DragValue::new(&mut section.1)
                                .range(8.0..=50.0)
                                .suffix(" cm"),
                        )
                        .changed();
                    if width_changed || height_changed {
                        edited.beam_section_cm = Some(section);
                        changed = true;
                    }
                });
                ui.end_row();
                ui.label("Vigas extras");
                ui.label(format!("{} posicoes", edited.extra_beam_positions_m.len()));
                ui.label("Ao longo do beiral");
                ui.label(
                    edited
                        .extra_beam_positions_m
                        .iter()
                        .map(|position| format!("{} m", decimal(*position, 2)))
                        .collect::<Vec<_>>()
                        .join("; "),
                );
                ui.end_row();
                ui.label("Telhado aparente");
                changed |= ui.checkbox(&mut edited.exposed, "sem forro").changed();
                ui.label("Tesouras");
                changed |= ui
                    .checkbox(&mut edited.use_trusses, "estruturar com tesouras")
                    .changed();
                ui.end_row();
                ui.label("Contraventamento");
                let mut braced = edited.lateral_bracing_kn.is_some();
                let mut capacity = edited.lateral_bracing_kn.unwrap_or(0.0);
                if ui.checkbox(&mut braced, "caminho verificado").on_hover_text("Resistência de cálculo do caminho completo: plano da cobertura, ligações, apoios verticais e fundações.").changed() {
                    edited.lateral_bracing_kn = braced.then_some(capacity);
                    changed = true;
                }
                ui.end_row();
                ui.label("Resistência lateral");
                changed |= ui.add_enabled(braced, egui::DragValue::new(&mut capacity).range(0.0..=100_000.0).suffix(" kN")).changed();
                if braced {
                    edited.lateral_bracing_kn = Some(capacity);
                }
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
                    "Flecha / limite",
                    "Uso",
                    "Situação",
                ] {
                    ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
                }
                ui.end_row();
                for element in result
                    .elements
                    .iter()
                    .chain(result.wall_plate.iter())
                    .chain(result.hip_rafters.iter())
                {
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
                    ui.label(format!(
                        "{} / {} mm",
                        decimal(element.deflection_m * 1000.0, 1),
                        decimal(element.deflection_limit_m * 1000.0, 1)
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
        if let Some(truss) = &result.truss {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "{} · {} un. · vão {} m · altura {} m · carga de cálculo {} kN · volume {} m³",
                    truss.kind,
                    truss.count,
                    decimal(truss.span_m, 2),
                    decimal(truss.rise_m, 2),
                    decimal(truss.design_total_load_kn, 1),
                    decimal(truss.volume_m3, 2)
                ))
                .font(strong(12.0)),
            );
            egui::Grid::new(("roof_truss_members", roof.id))
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    for h in [
                        "Barra",
                        "Esforço Nd",
                        "Seção",
                        "Esbeltez λ",
                        "Uso",
                        "Situação",
                    ] {
                        ui.label(RichText::new(h).font(strong(11.5)).color(TEXT_2));
                    }
                    ui.end_row();
                    for m in &truss.members {
                        ui.label(&m.name);
                        let kind = if m.axial_design_kn < 0.0 {
                            format!("-{} kN (comp.)", decimal(m.axial_design_kn.abs(), 1))
                        } else {
                            format!("+{} kN (traç.)", decimal(m.axial_design_kn, 1))
                        };
                        ui.label(kind);
                        ui.label(format!("{:.0} × {:.0} cm", m.section_cm.0, m.section_cm.1));
                        ui.label(if m.slenderness > 0.0 {
                            decimal(m.slenderness, 0)
                        } else {
                            "—".to_owned()
                        });
                        ui.label(format!("{:.0} %", m.ratio * 100.0));
                        ui.label(
                            RichText::new(if m.ok { "Atende" } else { "Revise" }).color(if m.ok {
                                SUCCESS
                            } else {
                                DANGER
                            }),
                        );
                        ui.end_row();
                    }
                });
        }
        if result.uplift.lifts {
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "Sucção de vento (NBR 6123, V0={} m/s): líquido {} kPa → ancorar cada apoio para {} kN.",
                    decimal(result.uplift.wind_v0_ms, 0),
                    decimal(result.uplift.net_design_uplift_kpa, 2),
                    decimal(result.uplift.anchor_design_kn, 1)
                ))
                .size(11.5)
                .color(WARNING),
            );
        }
        if let Some(critical) = result
            .elements
            .iter()
            .find(|element| element.element == RoofElement::Beams)
            .or_else(|| {
                result
                    .elements
                    .iter()
                    .max_by(|a, b| a.deflection_ratio.total_cmp(&b.deflection_ratio))
            })
        {
            let ok = critical.deflection_ratio <= 1.0 + 1e-9;
            ui.label(
                RichText::new(format!(
                    "Simula\u{00e7}\u{00e3}o de embarrigamento: {} de v\u{00e3}o {} m, flecha {} mm para limite L/200 = {} mm (uso {:.0} %) - {}.",
                    critical.element.label(),
                    decimal(critical.span_m, 2),
                    decimal(critical.deflection_m * 1000.0, 1),
                    decimal(critical.deflection_limit_m * 1000.0, 1),
                    critical.deflection_ratio * 100.0,
                    if ok { "atende" } else { "risco de embarrigamento" }
                ))
                .size(11.5)
                .color(if ok { SUCCESS } else { WARNING }),
            );
        }
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
    if let Some(posts) = posts_section(ui, project, report) {
        action = Some(posts);
    }
    if let Some(wind) = wind_section(ui, project, report) {
        action = Some(wind);
    }
    action
}

/// The wind on the tall walls and on the roofs (#147).
fn wind_section(ui: &mut Ui, project: &Project, report: &StructureReport) -> Option<RoofAction> {
    let mut action = None;
    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new("Vento (NBR 6123)").font(strong(13.5)));
    let mut wind = project.structure.wind;
    ui.horizontal_wrapped(|ui| {
        ui.label("V0");
        let mut changed = ui
            .add(
                egui::DragValue::new(&mut wind.v0_ms)
                    .range(25.0..=55.0)
                    .speed(0.0)
                    .max_decimals(0)
                    .suffix(" m/s"),
            )
            .on_hover_text("Velocidade básica do vento da cidade, no mapa de isopletas da NBR 6123")
            .changed();
        ui.label("Terreno");
        egui::ComboBox::from_id_salt("wind_category")
            .selected_text(match wind.category {
                1 => "I · mar aberto",
                2 => "II · campo aberto",
                3 => "III · subúrbio, casas",
                4 => "IV · cidade",
                _ => "V · centro denso",
            })
            .show_ui(ui, |ui| {
                for (category, label) in [
                    (1, "I · mar aberto"),
                    (2, "II · campo aberto"),
                    (3, "III · subúrbio, casas"),
                    (4, "IV · cidade"),
                    (5, "V · centro denso"),
                ] {
                    changed |= ui
                        .selectable_value(&mut wind.category, category, label)
                        .changed();
                }
            });
        if changed {
            action = Some(RoofAction::Wind(wind));
        }
    });
    if report.wind.columns.is_empty()
        && report.wind.roofs.is_empty()
        && report
            .masonry
            .panels
            .iter()
            .all(|panel| panel.wind.is_none())
    {
        ui.label(
            RichText::new("Nenhuma parede mais alta que o pé-direito nem telhado para verificar.")
                .color(MUTED),
        );
        return action;
    }
    for column in &report.wind.columns {
        let ok = column.ratio <= 1.0 + 1e-9;
        ui.label(
            RichText::new(format!(
                "Pilar {} · parede de {} m, faixa de {} m, q = {} kN/m², {}: M = {} kN·m; {} cm atravessando a parede{}",
                column.label,
                decimal(column.height_m, 2),
                decimal(column.tributary_m, 2),
                decimal(column.q_kpa, 2),
                if column.braced { "topo travado pelo telhado" } else { "em balanço" },
                decimal(column.moment_knm, 1),
                decimal(column.depth_m * 100.0, 0),
                if ok {
                    format!(" atendem ({} cm² por face).", decimal(column.steel_cm2, 2))
                } else {
                    format!(
                        " não bastam: use {}.",
                        column
                            .needed_depth_m
                            .map_or("mais de 80 cm".to_owned(), |h| format!("{:.0} cm", h * 100.0))
                    )
                }
            ))
            .color(if ok { TEXT_2 } else { WARNING }),
        );
    }
    for panel in &report.masonry.panels {
        let Some(wind) = &panel.wind else { continue };
        let bands = wind
            .bond_beam
            .as_ref()
            .map_or("sem pilares nas pontas".to_owned(), |bond| {
                format!(
                    "{} cintas intermediárias + coroamento a cada {} m, seção {}×{} cm",
                    bond.intermediate_count,
                    decimal(bond.spacing_m, 2),
                    bond.across_m
                        .map_or("mais de 40".to_owned(), |w| decimal(w * 100.0, 0)),
                    decimal(bond.height_m * 100.0, 0)
                )
            });
        ui.label(
            RichText::new(format!(
                "Painel {}: vento q = {} kN/m², M = {} / {} kN·m/m; {}{}",
                panel.label,
                decimal(wind.q_kpa, 2),
                decimal(wind.moment_knm_m, 2),
                decimal(wind.capacity_knm_m, 2),
                bands,
                if wind.ok() {
                    "."
                } else {
                    "; revise apoios, aberturas e seção."
                }
            ))
            .color(if wind.ok() { TEXT_2 } else { WARNING }),
        );
    }
    for roof in &report.wind.roofs {
        ui.label(
            RichText::new(if roof.lifts() {
                format!(
                    "Telhado {}: sucção de {} kN/m²{} contra {} kN/m² de peso: ancore cada ponta de viga para {} kN e amarre as telhas.",
                    roof.label,
                    decimal(roof.uplift_kpa, 2),
                    if roof.open { " (lado aberto)" } else { "" },
                    decimal(roof.dead_kpa, 2),
                    decimal(roof.anchor_kn, 1)
                )
            } else {
                format!(
                    "Telhado {}: o peso ({} kN/m²) vence a sucção ({} kN/m²).",
                    roof.label,
                    decimal(roof.dead_kpa, 2),
                    decimal(roof.uplift_kpa, 2)
                )
            })
            .color(if roof.lifts() { WARNING } else { TEXT_2 }),
        );
    }
    for brace in &report.wind.bracing {
        ui.label(RichText::new(format!(
            "Telhado {} · contraventamento: exige {} kN; resistência verificada {}; apoios ancorados {}.",
            brace.label, decimal(brace.demand_kn, 1),
            brace.capacity_kn.map_or("não informada".to_owned(), |kn| format!("{} kN", decimal(kn, 1))),
            if brace.path_ok { "sim" } else { "não" }
        )).color(if brace.ok() { TEXT_2 } else { WARNING }));
    }
    for global in &report.wind.global {
        ui.label(RichText::new(format!(
            "Estabilidade {}: vento {} kN, α {:.2}/{:.2}, deslocamento {} / {} mm, tombamento {:.2} · {}",
            global.direction, decimal(global.wind_kn, 1), global.alpha, global.alpha_limit,
            decimal(global.drift_mm, 1), decimal(global.drift_limit_mm, 1),
            global.overturning_ratio,
            if global.ok() { "triagem atende" } else { "revisar caminho e estrutura" }
        )).color(if global.ok() { TEXT_2 } else { WARNING }));
    }
    ui.label(
        RichText::new("O apoio vertical do telhado só trava o pilar após informar resistência verificada do caminho lateral completo até as fundações. A triagem global não substitui o projeto de ligações, fundações e deslizamento.")
            .size(11.0)
            .color(MUTED),
    );
    action
}

/// The timber posts and the timber beams over them (#146).
fn posts_section(ui: &mut Ui, project: &Project, report: &StructureReport) -> Option<RoofAction> {
    let mut action = None;
    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new("Mourões e vigas de madeira").font(strong(13.5)));
    ui.horizontal(|ui| {
        ui.label("Espécie dos mourões");
        let mut species = project.structure.post_species;
        egui::ComboBox::from_id_salt("post_species")
            .selected_text(format!("{} · {}", species.label(), species.class().label()))
            .show_ui(ui, |ui| {
                for option in TimberSpecies::ALL {
                    if ui
                        .selectable_value(
                            &mut species,
                            option,
                            format!("{} · {}", option.label(), option.class().label()),
                        )
                        .changed()
                    {
                        action = Some(RoofAction::PostSpecies(option));
                    }
                }
            });
        let mut treated = project.structure.post_treated;
        if ui.checkbox(&mut treated, "tratada").changed() {
            action = Some(RoofAction::PostTreated(treated));
        }
    });
    if report.timber_posts.is_empty() {
        ui.label(
            RichText::new("Nenhum mourão: use Mourão na faixa Estrutura para pôr um na ponta de cada viga do telhado que não tem pilar embaixo.")
                .color(MUTED),
        );
        return action;
    }
    egui::Grid::new("timber_posts")
        .striped(true)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            for title in [
                "Mourão",
                "Seção",
                "Altura",
                "Carga",
                "Vento",
                "Esbeltez",
                "Uso",
                "Base",
                "Situação",
            ] {
                ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
            }
            ui.end_row();
            for post in &report.timber_posts {
                ui.label(RichText::new(&post.label).font(strong(12.5)));
                ui.label(format!(
                    "{}{} cm",
                    if post.round { "Ø " } else { "" },
                    decimal(post.size_m * 100.0, 0)
                ));
                ui.label(format!("{} m", decimal(post.height_m, 2)));
                ui.label(format!("{} kN", decimal(post.load_kn, 1)));
                ui.label(format!(
                    "{} kN · {} kN·m",
                    decimal(post.wind_force_kn, 1),
                    decimal(post.wind_moment_knm, 1)
                ));
                ui.label(format!("λ {:.0}", post.slenderness));
                ui.label(if post.ratio.is_finite() {
                    format!("{:.0} %", post.ratio * 100.0)
                } else {
                    "—".to_owned()
                });
                ui.label(format!(
                    "{} × {} cm · Ch {:.1} kN",
                    decimal(post.base_m * 100.0, 0),
                    decimal(post.base_depth_m * 100.0, 0),
                    post.anchor_kn
                ));
                if !post.treated {
                    ui.label(RichText::new("Tratar madeira").color(WARNING));
                } else if post.base_kpa > post.soil_kpa + 1e-9
                    || post.base_stability_ratio > 1.0 + 1e-9
                {
                    ui.label(RichText::new("Rever base").color(WARNING));
                } else if post.ok() {
                    ui.label(RichText::new("Atende").color(SUCCESS));
                } else {
                    ui.label(
                        RichText::new(match post.needed_m {
                            Some(size) => format!("Use {} cm", decimal(size * 100.0, 0)),
                            None => "Revise".to_owned(),
                        })
                        .color(WARNING),
                    );
                }
                ui.end_row();
            }
        });
    if !report.eave_beams.is_empty() {
        ui.add_space(4.0);
        egui::Grid::new("eave_beams")
            .striped(true)
            .spacing([14.0, 4.0])
            .show(ui, |ui| {
                for title in ["Viga", "De", "Para", "Vão", "Seção", "Cargas", "Situação"] {
                    ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
                }
                ui.end_row();
                for beam in &report.eave_beams {
                    ui.label(RichText::new(&beam.label).font(strong(12.5)));
                    ui.label(&beam.from_post);
                    ui.label(&beam.to_post);
                    ui.label(format!("{} m", decimal(beam.span_m, 2)));
                    ui.label(format!(
                        "{:.0} × {:.0} cm",
                        beam.section_cm.0, beam.section_cm.1
                    ));
                    ui.label(if beam.loads.is_empty() {
                        "só amarra".to_owned()
                    } else {
                        beam.loads
                            .iter()
                            .map(|(_, p)| format!("{} kN", decimal(*p, 1)))
                            .collect::<Vec<_>>()
                            .join(" + ")
                    });
                    if !beam.treated {
                        ui.label(RichText::new("Tratar madeira").color(WARNING));
                    } else if beam.ok {
                        ui.label(RichText::new("Atende").color(SUCCESS));
                    } else {
                        ui.label(RichText::new("Revise").color(WARNING));
                    }
                    ui.end_row();
                }
            });
    }
    ui.label(
        RichText::new("Mourões pela NBR 7190: lote classificado, madeira tratada, compressão com flambagem e flexão pelo vento (interação ≤ 100 %, λ ≤ 140), sobre bloco de concreto com chumbador para a madeira não tocar o solo.")
            .size(11.0)
            .color(MUTED),
    );
    action
}
