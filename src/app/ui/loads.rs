//! UI tab for NBR 6120 load cases, NBR 6123 building wind and NBR 8681 /
//! NBR 6118 ultimate (ELU) & service (ELS) load combinations (#105).

use eframe::egui::{self, RichText, Ui};

use super::super::theme::*;
use crate::domain::{
    loads::{CombinationCategory, LoadAnalysisReport, WindDirection},
    structure::{OccupancyClass, StructureSettings},
    wind::{WindImportanceGroup, WindSettings},
};

fn dec(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// Renders the "Cargas e combinações" tab inside the Structure window.
/// Mutates `settings` and `wind` in-place when edited.
pub(super) fn loads_tab(
    ui: &mut Ui,
    loads: &LoadAnalysisReport,
    settings: &mut StructureSettings,
    wind: &mut WindSettings,
) {
    ui.label(
        RichText::new(
            "Casos de carga (NBR 6120:2019), vento na edificação (NBR 6123) e combinações ELU / ELS (NBR 8681:2003 · NBR 6118 §11.8).",
        )
        .size(12.0)
        .color(TEXT_2),
    );
    ui.add_space(6.0);

    // --- Interactive NBR 6120 & NBR 6123 configuration bar ---
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Ocupação (NBR 6120 / 8681)").font(strong(12.0)));
        egui::ComboBox::from_id_salt("loads_occupancy_combo")
            .selected_text(settings.occupancy.label())
            .width(190.0)
            .show_ui(ui, |ui| {
                for occ in OccupancyClass::ALL {
                    if ui
                        .selectable_label(settings.occupancy == occ, occ.label())
                        .clicked()
                    {
                        settings.apply_occupancy(occ);
                    }
                }
            });
        ui.checkbox(&mut settings.per_room_live_loads, "NBR 6120 por cômodo");
        ui.separator();
        ui.label(
            RichText::new(format!(
                "Q: ψ0 = {} · ψ1 = {} · ψ2 = {}  |  Vento: ψ0 = 0,60 · ψ1 = 0,30 · ψ2 = 0,00",
                dec(settings.occupancy.psi0(), 2),
                dec(settings.occupancy.psi1(), 2),
                dec(settings.occupancy.psi2(), 2),
            ))
            .size(11.5)
            .color(ACCENT_TEXT),
        );
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Vento (NBR 6123)").font(strong(12.0)));
        ui.label("V0");
        ui.add(
            egui::DragValue::new(&mut wind.v0_ms)
                .range(25.0..=55.0)
                .speed(0.0)
                .max_decimals(0)
                .suffix(" m/s"),
        )
        .on_hover_text("Velocidade básica do vento (mapa de isopletas da NBR 6123)");
        ui.label("Terreno");
        egui::ComboBox::from_id_salt("loads_wind_category")
            .selected_text(WindSettings::category_label(wind.category))
            .width(195.0)
            .show_ui(ui, |ui| {
                for cat in 1..=5u8 {
                    ui.selectable_value(&mut wind.category, cat, WindSettings::category_label(cat));
                }
            });
        ui.label("S1");
        ui.add(
            egui::DragValue::new(&mut wind.s1)
                .range(0.5..=1.5)
                .speed(0.01)
                .max_decimals(2),
        )
        .on_hover_text(
            "Fator topográfico S1 (1,00 terreno plano; 0,90 vale protegido; >1,00 morro/talude)",
        );
        ui.label("S3");
        ui.add(
            egui::DragValue::new(&mut wind.s3)
                .range(0.8..=1.2)
                .speed(0.01)
                .max_decimals(2),
        )
        .on_hover_text("Fator estatístico S3 (NBR 6123 Tabela 3)");
        egui::ComboBox::from_id_salt("loads_wind_s3_group")
            .selected_text("Grupo S3")
            .width(86.0)
            .show_ui(ui, |ui| {
                for group in WindImportanceGroup::ALL {
                    if ui.selectable_label(false, group.label()).clicked() {
                        wind.s3 = group.s3();
                    }
                }
            });
    });

    ui.add_space(8.0);
    ui.separator();

    // --- 1. NBR 6120 Load Cases Summary ---
    let n6120 = &loads.nbr6120;
    ui.label(
        RichText::new("1. Casos de carga característicos — ABNT NBR 6120:2019").font(strong(13.0)),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!(
            "Peso próprio estrutural G1 = {} kN  ·  Permanente não estrutural G2 = {} kN  ·  Sobrecarga Q = {} kN  ·  Total gravitacional = {} kN ({} pav.)",
            dec(n6120.total_g1_kn, 1),
            dec(n6120.total_g2_kn, 1),
            dec(n6120.total_q_kn, 1),
            dec(n6120.total_gravity_kn(), 1),
            n6120.storeys
        ))
        .size(12.0)
        .color(TEXT_2),
    );
    ui.label(
        RichText::new(&n6120.live_reduction_note)
            .size(11.0)
            .color(MUTED),
    );
    if !n6120.rooms.is_empty() {
        ui.add_space(4.0);
        egui::Grid::new("loads_nbr6120_rooms")
            .striped(true)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for title in [
                    "Cômodo",
                    "Área",
                    "Categoria NBR 6120",
                    "g1 laje",
                    "g2 revest.",
                    "q mín. Tab. 10",
                    "q adotada",
                ] {
                    ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
                }
                ui.end_row();
                for r in &n6120.rooms {
                    ui.label(RichText::new(&r.room_name).font(strong(12.0)));
                    ui.label(format!("{} m²", dec(r.area_m2, 2)));
                    ui.label(r.category);
                    ui.label(format!("{} kN/m²", dec(r.g1_kn_m2, 2)));
                    ui.label(format!("{} kN/m²", dec(r.g2_kn_m2, 2)));
                    ui.label(format!("{} kN/m²", dec(r.table_min_q_kn_m2, 2)));
                    let color = if r.active_q_kn_m2 + 1e-6 < r.table_min_q_kn_m2 {
                        WARNING
                    } else {
                        SUCCESS
                    };
                    ui.label(
                        RichText::new(format!("{} kN/m²", dec(r.active_q_kn_m2, 2))).color(color),
                    );
                    ui.end_row();
                }
            });
    }

    // --- 2. NBR 6123 Building Wind ---
    ui.add_space(10.0);
    ui.separator();
    let bw = &loads.building_wind;
    ui.label(
        RichText::new("2. Vento na edificação — ABNT NBR 6123 (Tabelas 1–4)").font(strong(13.0)),
    );
    if bw.directions.is_empty() {
        ui.label(
            RichText::new(
                "Desenhe paredes ou gere a estrutura para calcular o vento na edificação.",
            )
            .color(MUTED),
        );
    } else {
        ui.label(
            RichText::new(format!(
                "Planta {} × {} m · Altura total {} m · Classe {} · Edificação {} · V0 = {:.0} m/s · Cat. {} · S1 = {} · S3 = {}",
                dec(bw.plan_width_x_m, 2),
                dec(bw.plan_depth_y_m, 2),
                dec(bw.total_height_m, 2),
                bw.building_class,
                if bw.enclosed {
                    "fechada (Cpi +0,2 / −0,3)"
                } else {
                    "abertura dominante (Cpi +0,8 / −0,6)"
                },
                bw.settings.v0_ms,
                bw.settings.category,
                dec(bw.settings.s1, 2),
                dec(bw.settings.s3, 2)
            ))
            .size(11.5)
            .color(TEXT_2),
        );
        ui.add_space(4.0);
        egui::Grid::new("loads_nbr6123_dirs")
            .striped(true)
            .spacing([11.0, 4.0])
            .show(ui, |ui| {
                for title in [
                    "Direção",
                    "Fachada × Prof.",
                    "Cpe (barl/sot/lat)",
                    "Ca",
                    "Pressão / Sucção",
                    "Cortante Vw",
                    "Tomb. Mw",
                    "ELU-EQU",
                    "Situação",
                ] {
                    ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
                }
                ui.end_row();
                for d in &bw.directions {
                    ui.label(RichText::new(d.direction.label()).font(strong(12.0)));
                    ui.label(format!(
                        "{} × {} m",
                        dec(d.facade_width_m, 2),
                        dec(d.depth_m, 2)
                    ));
                    ui.label(format!(
                        "+{} / {} / {}",
                        dec(d.coeffs.cpe_windward, 2),
                        dec(d.coeffs.cpe_leeward, 2),
                        dec(d.coeffs.cpe_side_edge, 2)
                    ));
                    ui.label(dec(d.coeffs.ca_drag, 2));
                    ui.label(format!(
                        "{} / {} kPa",
                        dec(d.max_wall_pressure_kpa, 2),
                        dec(d.max_wall_suction_kpa, 2)
                    ));
                    ui.label(format!("{} kN", dec(d.base_shear_kn, 1)));
                    ui.label(format!("{} kN·m", dec(d.overturning_moment_knm, 1)));
                    let ratio_txt = if d.overturning_safety_ratio.is_finite() {
                        format!("{}×", dec(d.overturning_safety_ratio.min(999.9), 1))
                    } else {
                        "∞".to_owned()
                    };
                    ui.label(ratio_txt);
                    if d.overturning_ok() && d.coeffs.within_table4 {
                        ui.label(RichText::new("Atende").color(SUCCESS));
                    } else if d.overturning_ok() {
                        ui.label(RichText::new("Fora Tab. 4").color(WARNING));
                    } else {
                        ui.label(RichText::new("Revise").color(DANGER));
                    }
                    ui.end_row();
                }
            });

        // Storey-by-storey wind forces for +X and +Y.
        if let Some(pos_x) = bw
            .directions
            .iter()
            .find(|d| d.direction == WindDirection::PosX)
            && !pos_x.storeys.is_empty()
        {
            ui.add_space(6.0);
            ui.label(
                RichText::new("Forças de vento por pavimento (eixo +X / +Y)").font(strong(12.0)),
            );
            let pos_y = bw
                .directions
                .iter()
                .find(|d| d.direction == WindDirection::PosY);
            egui::Grid::new("loads_nbr6123_storeys")
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    for title in [
                        "Pavimento",
                        "Cota z",
                        "S2(z)",
                        "Vk (m/s)",
                        "q (kPa)",
                        "Fw,X (kN)",
                        "Fw,Y (kN)",
                    ] {
                        ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
                    }
                    ui.end_row();
                    for (idx, st) in pos_x.storeys.iter().enumerate() {
                        let fy = pos_y
                            .and_then(|d| d.storeys.get(idx))
                            .map_or(0.0, |s| s.force_kn);
                        ui.label(&st.label);
                        ui.label(format!("{} m", dec(st.z_top_m, 2)));
                        ui.label(dec(st.s2, 3));
                        ui.label(dec(st.vk_ms, 1));
                        ui.label(dec(st.q_kpa, 3));
                        ui.label(dec(st.force_kn, 1));
                        ui.label(dec(fy, 1));
                        ui.end_row();
                    }
                });
        }
    }

    // --- 3. NBR 8681 / NBR 6118 Combinations ---
    ui.add_space(10.0);
    ui.separator();
    ui.label(
        RichText::new("3. Combinações de ações — ABNT NBR 8681:2003 / NBR 6118 §11.8")
            .font(strong(13.0)),
    );
    ui.add_space(4.0);
    egui::Grid::new("loads_nbr8681_combos")
        .striped(true)
        .spacing([11.0, 4.0])
        .show(ui, |ui| {
            for title in [
                "Código",
                "Tipo",
                "Expressão",
                "Fz (kN)",
                "Fx (kN)",
                "Fy (kN)",
            ] {
                ui.label(RichText::new(title).font(strong(12.0)).color(TEXT_2));
            }
            ui.end_row();
            for c in &loads.combinations {
                let code_color = match c.factors.category {
                    CombinationCategory::EluNormal | CombinationCategory::EluEquilibrium => {
                        ACCENT_TEXT
                    }
                    _ => TEXT_2,
                };
                ui.label(
                    RichText::new(c.factors.code)
                        .font(strong(12.0))
                        .color(code_color),
                );
                ui.label(c.factors.category.label());
                ui.label(&c.factors.formula);
                ui.label(dec(c.vertical_kn, 1));
                ui.label(dec(c.horizontal_x_kn, 1));
                ui.label(dec(c.horizontal_y_kn, 1));
                ui.end_row();
            }
        });

    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Combinações aplicadas automaticamente por superposição linear exata na análise de pórticos 2D (aba Pórticos 2D). Pré-dimensionamento preliminar — revisar com engenheiro habilitado (ART/RRT).",
        )
        .size(11.0)
        .color(MUTED),
    );
}
