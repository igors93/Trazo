//! How structural results are reported to the assistant as JSON: beam
//! envelopes, the foundation study, roofs, wind, verdicts, cantilevers,
//! the grid analysis and the load cases.

use serde_json::{Value, json};

use super::super::xy;
use crate::domain::structure::section_cm;

pub(super) fn beam_ultimate_envelope_json(
    e: &crate::domain::frame2d::builder::FrameBeamEnvelope,
) -> Value {
    json!({
        "momento_positivo_knm": e.max_positive_moment_knm,
        "combinacao_momento_positivo": e.positive_combination,
        "momento_negativo_knm": e.min_negative_moment_knm,
        "combinacao_momento_negativo": e.negative_combination,
        "cortante_kn": e.max_shear_kn,
        "combinacao_cortante": e.shear_combination,
    })
}

/// The foundation study as tool output (#112).
pub(super) fn foundation_study_json(study: &crate::domain::soil::FoundationStudy) -> Value {
    let round1 = |v: f64| (v * 10.0).round() / 10.0;
    let pile_json = |p: crate::domain::soil::PileCapacity, count: u32| {
        json!({
            "tipo": p.kind.label(),
            "diametro_cm": (p.diameter_m * 100.0).round(),
            "comprimento_m": p.length_m,
            "quantidade": count,
            "carga_admissivel_kn": round1(p.allowable_kn),
            "ponta_ruptura_kn": round1(p.tip_kn),
            "atrito_ruptura_kn": round1(p.shaft_kn),
            "limite_concreto_kn": round1(p.structural_kn),
            "n_ponta": round1(p.tip_n),
            "n_fuste": round1(p.shaft_n),
            "recalque_mm": round1(p.settlement_mm),
        })
    };
    json!({
        "com_sondagem": study.survey,
        "furos": study.borehole_count,
        "furo_critico": study.governing_borehole,
        "carga_total_kn": round1(study.total_load_kn),
        "projecao_m2": round1(study.footprint_m2),
        "solo_kpa_sondagem": study.survey_footing_kpa.map(|v| v.round()),
        "recomendada": study.recommended.map(|o| o.key()),
        "opcoes": study.options.iter().map(|o| json!({
            "tipo": o.option.key(),
            "nome": o.option.label(),
            "viavel": o.viable,
            "resumo": o.summary,
            "observacoes": o.notes,
            "concreto_m3": (o.concrete_m3 * 100.0).round() / 100.0,
            "solo_kpa": o.soil_kpa.map(|v| v.round()),
            "recalque_mm": o.settlement_mm.map(round1),
            "recalque_diferencial_mm": o.differential_settlement_mm.map(round1),
        })).collect::<Vec<_>>(),
        "broca": study.pile.map(|p| pile_json(p, study.piles)),
        "estaca_profunda": study.deep_pile.map(|p| pile_json(p, study.deep_piles)),
    })
}

/// A roof result as tool output (#114).
pub(super) fn roof_json(result: &crate::domain::roof::RoofResult) -> Value {
    let round2 = |v: f64| (v * 100.0).round() / 100.0;
    let piece_json = |e: &crate::domain::roof::RoofElementResult| {
        json!({
            "peca": e.element.key(),
            "secao_cm": format!("{}x{}", e.section_cm.0, e.section_cm.1),
            "atende": e.ok,
            "vao_m": round2(e.span_m),
            "espacamento_m": round2(e.spacing_m),
            "carga_kn_m": round2(e.load_kn_m),
            "flexao": round2(e.bending_ratio),
            "cisalhamento": round2(e.shear_ratio),
            "flecha": round2(e.deflection_ratio),
            "flecha_mm": round2(e.deflection_m * 1000.0),
            "flecha_limite_mm": round2(e.deflection_limit_m * 1000.0),
            "risco_embarrigamento": e.deflection_ratio > 1.0 + 1e-9,
            "quantidade": e.count,
            "comprimento_total_m": round2(e.total_length_m),
            "volume_m3": round2(e.volume_m3),
        })
    };
    let truss_json = result.truss.as_ref().map(|t| {
        let members: Vec<Value> = t
            .members
            .iter()
            .map(|m| {
                json!({
                    "barra": m.name,
                    "esforco_nd_kn": round2(m.axial_design_kn),
                    "comprimento_m": round2(m.length_m),
                    "secao_cm": format!("{}x{}", m.section_cm.0, m.section_cm.1),
                    "esbeltez": round2(m.slenderness),
                    "uso": round2(m.ratio),
                    "atende": m.ok,
                })
            })
            .collect();
        json!({
            "tipo": t.kind,
            "vao_m": round2(t.span_m),
            "altura_m": round2(t.rise_m),
            "quantidade": t.count,
            "carga_calculo_kn": round2(t.design_total_load_kn),
            "volume_m3": round2(t.volume_m3),
            "atende": t.ok,
            "barras": members,
        })
    });
    let uplift_json = json!({
        "v0_ms": result.uplift.wind_v0_ms,
        "succao_kpa": round2(result.uplift.suction_kpa),
        "succao_beiral_kpa": round2(result.uplift.overhang_suction_kpa),
        "succao_liquida_calculo_kpa": round2(result.uplift.net_design_uplift_kpa),
        "ancoragem_apoio_kn": round2(result.uplift.anchor_design_kn),
        "levanta": result.uplift.lifts,
    });
    json!({
        "id": result.id,
        "rotulo": result.label,
        "estilo": result.style.key(),
        "beiral_m": round2(result.overhang_m),
        "uso_flexao_beiral": round2(result.overhang_bending_ratio),
        "inclinacao_pct": round2(result.slope_percent),
        "inclinacao_minima_pct": result.min_slope_percent,
        "carga_permanente_kn_m2": round2(result.dead_kn_m2),
        "sobrecarga_kn_m2": result.live_kn_m2,
        "area_telhado_m2": round2(result.surface_m2),
        "telhas": result.tiles,
        "madeira_m3": round2(result.timber_m3),
        "pecas": result.elements.iter().map(&piece_json).collect::<Vec<_>>(),
        "frechal": result.wall_plate.as_ref().map(piece_json),
        "espigoes": result.hip_rafters.as_ref().map(piece_json),
        "tesoura": truss_json,
        "succao_vento": uplift_json,
        "apoios": result.reactions.iter().map(|r| json!({
            "ponto": xy(r.point),
            "lado": if r.high { "alto" } else { "baixo" },
            "carga_kn": round2(r.load_kn),
            "permanente_kn": round2(r.dead_kn),
            "apoio": r.support,
        })).collect::<Vec<_>>(),
    })
}

/// The wind settings and its effects as tool output (#147).
pub(super) fn wind_json(
    wind: &crate::domain::wind::WindSettings,
    effects: &crate::domain::wind::WindReport,
    masonry: &crate::domain::masonry_structure::MasonryStructureReport,
) -> Value {
    let round2 = |v: f64| (v * 100.0).round() / 100.0;
    json!({
        "v0_ms": wind.v0_ms,
        "categoria_terreno": wind.category,
        "s1": wind.s1,
        "s3": wind.s3,
        "pilares": effects.columns.iter().map(|c| json!({
            "rotulo": c.label,
            "altura_m": round2(c.height_m),
            "faixa_m": round2(c.tributary_m),
            "q_kn_m2": round2(c.q_kpa),
            "topo_travado": c.braced,
            "momento_knm": round2(c.moment_knm),
            "secao_atravessando_cm": round2(c.depth_m * 100.0),
            "uso": round2(c.ratio.min(9.99)),
            "armadura_por_face_cm2": round2(c.steel_cm2),
            "secao_necessaria_cm": c.needed_depth_m.map(|h| round2(h * 100.0)),
            "sapata_excentricidade_m": c.footing_eccentricity_m.map(round2),
            "sapata_pressao_max_kpa": c.footing_max_kpa.map(round2),
            "sapata_vento_uso": c.footing_wind_ratio.map(|r| round2(r.min(9.99))),
        })).collect::<Vec<_>>(),
        "telhados": effects.roofs.iter().map(|r| json!({
            "rotulo": r.label,
            "q_kn_m2": round2(r.q_kpa),
            "lado_aberto": r.open,
            "succao_kn_m2": round2(r.uplift_kpa),
            "peso_kn_m2": round2(r.dead_kpa),
            "levanta": r.lifts(),
            "ancoragem_por_ponta_kn": round2(r.anchor_kn),
            "chumbador_mm": r.anchor_bolt_mm,
            "chumbadores_qtd": r.anchor_bolt_count,
            "chumbador_interacao": round2(r.anchor_interaction),
            "chumbadores_sugeridos": r.anchor_spec,
        })).collect::<Vec<_>>(),
        "contraventamento": effects.bracing.iter().map(|brace| json!({
            "telhado": brace.label,
            "demanda_kn": round2(brace.demand_kn),
            "resistencia_informada_kn": brace.capacity_kn.map(round2),
            "apoios_ancorados": brace.path_ok,
            "tirante_mm": brace.tie_bar_mm,
            "tirante_as_cm2": round2(brace.tie_as_cm2),
            "tirante_sugerido": brace.tie_spec,
            "atende": brace.ok(),
        })).collect::<Vec<_>>(),
        "estabilidade_global": effects.global.iter().map(|global| json!({
            "direcao": global.direction,
            "altura_m": round2(global.height_m),
            "fachada_m": round2(global.facade_m),
            "vento_kn": round2(global.wind_kn),
            "momento_vento_knm": round2(global.overturning_knm),
            "peso_permanente_kn": round2(global.gravity_kn),
            "succao_cobertura_kn": round2(global.roof_uplift_kn),
            "momento_estabilizante_knm": round2(global.stabilizing_knm),
            "razao_tombamento": round2(global.overturning_ratio),
            "atrito_mu": round2(global.friction_mu),
            "resistencia_deslizamento_kn": round2(global.sliding_resistance_kn),
            "deslizamento_fs": round2(global.sliding_fs),
            "alfa": global.alpha.is_finite().then(|| round2(global.alpha)),
            "alfa_limite": round2(global.alpha_limit),
            "deslocamento_mm": global.drift_mm.is_finite().then(|| round2(global.drift_mm)),
            "deslocamento_limite_mm": round2(global.drift_limit_mm),
            "apoios_ok": global.supports_ok,
            "contraventamento_ok": global.bracing_ok,
            "atende": global.ok(),
        })).collect::<Vec<_>>(),
        "paineis_alvenaria": masonry.panels.iter().filter_map(|panel| {
            let effect = panel.wind.as_ref()?;
            Some(json!({
                "rotulo": panel.label,
                "q_kn_m2": round2(effect.q_kpa),
                "momento_knm_m": round2(effect.moment_knm_m),
                "resistencia_knm_m": round2(effect.capacity_knm_m),
                "uso": round2(effect.ratio),
                "apoios_ok": effect.end_columns && effect.base_supported,
                "aberturas_livres": effect.openings_clear,
                "cintas_intermediarias": effect.bond_beam.as_ref().map(|tie| tie.intermediate_count),
                "espacamento_cintas_m": effect.bond_beam.as_ref().map(|tie| round2(tie.spacing_m)),
            }))
        }).collect::<Vec<_>>(),
    })
}

/// Every calculation hypothesis with its origin and evidence (#168).
pub(in crate::app::mcp_tools) fn hypotheses_json(project: &crate::domain::model::Project) -> Value {
    use crate::domain::protection::hypotheses::{HypothesisKey, evidence};
    Value::Array(
        HypothesisKey::ALL
            .into_iter()
            .map(|key| {
                let record = project.protection.hypotheses.record(key);
                json!({
                    "chave": key.key(),
                    "nome": key.label(),
                    "valor": key.display(project),
                    "origem": record.source.key(),
                    "prova": evidence(key, project),
                    "o_que_e": key.plain(),
                })
            })
            .collect(),
    )
}

/// Traffic-light verdicts as tool output (#101).
pub(in crate::app::mcp_tools) fn verdicts_json(
    verdicts: &[crate::domain::verdict::Verdict],
) -> Value {
    Value::Array(
        verdicts
            .iter()
            .map(|verdict| {
                json!({
                    "elemento": verdict.element,
                    "disciplina": verdict.discipline.label(),
                    "semaforo": verdict.light().key(),
                    "verificacoes": verdict.checks.iter().map(|check| json!({
                        "nome": check.name,
                        "semaforo": check.light.key(),
                        "uso_pct": check.utilisation.is_finite().then(|| (check.utilisation * 1000.0).round() / 10.0),
                        "folga_pct": check.utilisation.is_finite().then(|| (check.margin_percent() * 10.0).round() / 10.0),
                        "o_que_fazer": check.simple,
                        "detalhe": check.detail,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

pub(super) fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

pub(super) fn cantilevers_json(rep: &crate::domain::cantilevers::CantileverReport) -> Value {
    let round2 = |v: f64| (v * 100.0).round() / 100.0;
    let stairs: Vec<Value> = rep
        .stairs
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "rotulo": s.label,
                "nome": s.name,
                "lance_m": round2(s.flight_span_m),
                "espessura_cm": round1(s.waist_m * 100.0),
                "md_knm_m": round2(s.moment_design_knm_m),
                "as_inferior_cm2_m": round2(s.as_bottom_cm2_m),
                "as_superior_cm2_m": round2(s.as_top_cm2_m),
                "flecha_mm": round1(s.deflection_mm),
                "limite_mm": round1(s.limit_mm),
                "reacao_total_kn": round1(s.reaction_total_kn),
                "semaforo": s.light.key(),
            })
        })
        .collect();
    let balconies: Vec<Value> = rep
        .balconies
        .iter()
        .map(|b| {
            json!({
                "id": b.id,
                "rotulo": b.label,
                "origem": b.source,
                "balanco_m": round2(b.span_m),
                "espessura_cm": round1(b.thickness_m * 100.0),
                "espessura_minima_cm": round1(b.thickness_min_m * 100.0),
                "gamma_n": round2(b.gamma_n),
                "mk_neg_knm_m": round2(b.moment_k_neg_knm_m),
                "md_neg_knm_m": round2(b.moment_d_neg_knm_m),
                "as_superior_cm2_m": round2(b.as_top_cm2_m),
                "flecha_mm": round1(b.deflection_mm),
                "limite_l125_mm": round1(b.limit_mm),
                "contravao_m": round2(b.back_span_m),
                "contravao_minimo_m": round2(b.required_back_span_m),
                "contravao_ok": b.back_span_ok,
                "alerta_trelicada": b.is_ribbed_warning,
                "semaforo": b.light.key(),
            })
        })
        .collect();
    let beams: Vec<Value> = rep
        .beams
        .iter()
        .map(|cb| {
            json!({
                "id": cb.id,
                "rotulo": cb.label,
                "balanco_m": round2(cb.span_m),
                "secao_cm": section_cm(cb.width_m, cb.height_m),
                "altura_minima_cm": round1(cb.height_min_m * 100.0),
                "mk_neg_knm": round2(cb.moment_k_neg_knm),
                "md_neg_knm": round2(cb.moment_d_neg_knm),
                "cortante_kn": round1(cb.shear_k_kn),
                "as_superior_cm2": round2(cb.as_top_cm2),
                "flecha_mm": round1(cb.deflection_mm),
                "limite_l125_mm": round1(cb.limit_mm),
                "contravao_ok": cb.back_span_ok,
                "semaforo": cb.light.key(),
            })
        })
        .collect();
    json!({
        "escadas": stairs,
        "sacadas_lajes_balanco": balconies,
        "vigas_balanco": beams,
    })
}

pub(super) fn grid_analysis_json(grid: &crate::domain::grid_analysis::GridAnalysisReport) -> Value {
    let slabs: Vec<Value> = grid
        .slabs
        .iter()
        .map(|s| {
            json!({
                "id": s.slab_id,
                "rotulo": s.label,
                "comodo": s.room,
                "irregular": s.is_irregular,
                "vao_menor_m": (s.short_span_m * 100.0).round() / 100.0,
                "w_elastico_mm": (s.w_elastic_mm * 100.0).round() / 100.0,
                "w_longo_prazo_mm": round1(s.w_long_term_mm),
                "limite_mm": round1(s.limit_mm),
                "mx_pos_knm_m": (s.mx_pos_knm_m * 100.0).round() / 100.0,
                "my_pos_knm_m": (s.my_pos_knm_m * 100.0).round() / 100.0,
                "mx_neg_knm_m": (s.mx_neg_knm_m * 100.0).round() / 100.0,
                "my_neg_knm_m": (s.my_neg_knm_m * 100.0).round() / 100.0,
                "mxy_max_knm_m": (s.mxy_max_knm_m * 100.0).round() / 100.0,
                "semaforo": s.light.key(),
            })
        })
        .collect();
    let beams: Vec<Value> = grid
        .beams
        .iter()
        .map(|b| {
            json!({
                "id": b.beam_id,
                "rotulo": b.label,
                "m_pos_knm": round1(b.m_pos_knm),
                "m_neg_knm": round1(b.m_neg_knm),
                "v_max_kn": round1(b.v_max_kn),
                "torcao_max_knm": (b.t_max_knm * 100.0).round() / 100.0,
                "w_elastico_mm": (b.w_elastic_mm * 100.0).round() / 100.0,
            })
        })
        .collect();
    let reactions: Vec<Value> = grid
        .reactions
        .iter()
        .map(|r| {
            json!({
                "rotulo": r.label,
                "reacao_kn": round1(r.reaction_kn),
            })
        })
        .collect();
    json!({
        "nos": grid.node_count,
        "barras": grid.member_count,
        "carga_total_kn": round1(grid.total_applied_kn),
        "reacao_total_kn": round1(grid.total_reaction_kn),
        "erro_equilibrio_kn": grid.equilibrium_error_kn,
        "w_max_elastico_mm": (grid.max_elastic_w_mm * 100.0).round() / 100.0,
        "lajes": slabs,
        "vigas": beams,
        "reacoes": reactions,
        "erro": grid.error,
    })
}

pub(super) fn loads_analysis_json(loads: &crate::domain::loads::LoadAnalysisReport) -> Value {
    let n6120 = &loads.nbr6120;
    let bw = &loads.building_wind;
    let round2 = |v: f64| (v * 100.0).round() / 100.0;
    let rooms: Vec<Value> = n6120
        .rooms
        .iter()
        .map(|r| {
            json!({
                "comodo": r.room_name,
                "area_m2": round2(r.area_m2),
                "categoria": r.category,
                "g1_kn_m2": round2(r.g1_kn_m2),
                "g2_kn_m2": round2(r.g2_kn_m2),
                "q_min_tabela10_kn_m2": round2(r.table_min_q_kn_m2),
                "q_adotada_kn_m2": round2(r.active_q_kn_m2),
            })
        })
        .collect();
    let directions: Vec<Value> = bw
        .directions
        .iter()
        .map(|d| {
            let storeys: Vec<Value> = d
                .storeys
                .iter()
                .map(|st| {
                    json!({
                        "pavimento": st.label,
                        "z_topo_m": round2(st.z_top_m),
                        "s2": (st.s2 * 1000.0).round() / 1000.0,
                        "vk_ms": round1(st.vk_ms),
                        "q_kpa": (st.q_kpa * 1000.0).round() / 1000.0,
                        "area_m2": round2(st.facade_area_m2),
                        "forca_kn": round1(st.force_kn),
                    })
                })
                .collect();
            json!({
                "direcao": d.direction.short(),
                "rotulo": d.direction.label(),
                "fachada_m": round2(d.facade_width_m),
                "profundidade_m": round2(d.depth_m),
                "h_sobre_b": round2(d.coeffs.h_over_b),
                "a_sobre_b": round2(d.coeffs.a_over_b),
                "dentro_tabela4": d.coeffs.within_table4,
                "cpe_barlavento": round2(d.coeffs.cpe_windward),
                "cpe_sotavento": round2(d.coeffs.cpe_leeward),
                "cpe_lateral_borda": round2(d.coeffs.cpe_side_edge),
                "ca_arrasto": round2(d.coeffs.ca_drag),
                "cpi_pos": round2(d.coeffs.cpi_pos),
                "cpi_neg": round2(d.coeffs.cpi_neg),
                "pressao_max_parede_kpa": round2(d.max_wall_pressure_kpa),
                "succao_max_parede_kpa": round2(d.max_wall_suction_kpa),
                "cortante_base_kn": round1(d.base_shear_kn),
                "cortante_diafragma_kn": round1(d.diaphragm_shear_kn),
                "momento_tombamento_knm": round1(d.overturning_moment_knm),
                "momento_estabilizante_elu_equ_knm": round1(d.stabilizing_moment_knm),
                "razao_tombamento_elu_equ": d.overturning_safety_ratio.is_finite().then(|| round2(d.overturning_safety_ratio)),
                "atende_tombamento": d.overturning_ok(),
                "pavimentos": storeys,
            })
        })
        .collect();
    let combos: Vec<Value> = loads
        .combinations
        .iter()
        .map(|c| {
            json!({
                "codigo": c.factors.code,
                "nome": c.factors.name,
                "categoria": c.factors.category.key(),
                "norma": c.factors.clause,
                "formula": c.factors.formula,
                "gama_g1": round2(c.factors.gamma_g1),
                "gama_g2": round2(c.factors.gamma_g2),
                "gama_q": round2(c.factors.gamma_q),
                "gama_w": round2(c.factors.gamma_w),
                "vertical_kn": round1(c.vertical_kn),
                "horizontal_x_kn": round1(c.horizontal_x_kn),
                "horizontal_y_kn": round1(c.horizontal_y_kn),
            })
        })
        .collect();
    json!({
        "nbr_6120": {
            "ocupacao": n6120.occupancy.key(),
            "categoria_nbr_8681": n6120.occupancy.nbr8681_category(),
            "pavimentos": n6120.storeys,
            "concreto_kn_m3": n6120.concrete_kn_m3,
            "aco_kn_m3": n6120.steel_kn_m3,
            "alvenaria_kn_m3": n6120.masonry_kn_m3,
            "revestimento_kn_m2": n6120.finish_kn_m2,
            "sobrecarga_base_kn_m2": n6120.base_live_kn_m2,
            "sobrecarga_por_comodo": n6120.per_room_live_loads,
            "psi0_q": n6120.psi0_q,
            "psi1_q": n6120.psi1_q,
            "psi2_q": n6120.psi2_q,
            "psi0_w": n6120.psi0_w,
            "psi1_w": n6120.psi1_w,
            "psi2_w": n6120.psi2_w,
            "g1_total_kn": round1(n6120.total_g1_kn),
            "g2_total_kn": round1(n6120.total_g2_kn),
            "g_total_kn": round1(n6120.total_g_kn()),
            "q_total_kn": round1(n6120.total_q_kn),
            "nota_reducao": n6120.live_reduction_note,
            "comodos": rooms,
        },
        "vento_edificacao_nbr_6123": {
            "v0_ms": bw.settings.v0_ms,
            "categoria_terreno": bw.settings.category,
            "s1": bw.settings.s1,
            "s3": bw.settings.s3,
            "classe": bw.building_class,
            "fechada": bw.enclosed,
            "planta_x_m": round2(bw.plan_width_x_m),
            "planta_y_m": round2(bw.plan_depth_y_m),
            "altura_total_m": round2(bw.total_height_m),
            "direcoes": directions,
        },
        "combinacoes_nbr_8681": combos,
        "resumo": {
            "elu_vertical_max_kn": round1(loads.max_elu_vertical_kn()),
            "elu_horizontal_max_kn": round1(loads.max_elu_horizontal_kn()),
            "els_qp_vertical_kn": round1(loads.els_qp_vertical_kn()),
            "els_frequente_horizontal_kn": round1(loads.els_frequent_horizontal_kn()),
        },
    })
}
