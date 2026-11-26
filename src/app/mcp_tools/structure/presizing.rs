//! The pre-sizing tool: every structural result the assistant reads in
//! one call, with the forces in every beam.

use serde_json::{Value, json};

use super::super::{ToolOutput, xy};
use super::{
    beam_ultimate_envelope_json, cantilevers_json, foundation_study_json, grid_analysis_json,
    hypotheses_json, loads_analysis_json, roof_json, round1, verdicts_json, wind_json,
};
use crate::app::FloorPlanApp;
use crate::domain::structure::section_cm;

impl FloorPlanApp {
    pub(in crate::app::mcp_tools) fn tool_presizing(&mut self) -> Result<ToolOutput, String> {
        let report = self.project.cached_structure_report();
        let reinforcement = self.project.cached_reinforcement_report();
        let settings = self.project.structure.settings;
        let slabs: Vec<Value> = report
            .slabs
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "rotulo": s.label,
                    "comodo": s.room,
                    "tipo": s.kind.key(),
                    "vaos_m": [round1(s.lx_m * 10.0) / 10.0, round1(s.ly_m * 10.0) / 10.0],
                    "espessura_cm": round1(s.thickness_m * 100.0),
                    "espessura_minima_cm": round1(s.thickness_min_m * 100.0),
                    "carga_kn_m2": round1(s.load_kn_m2 * 10.0) / 10.0,
                    "cargas_concentradas_kn": round1(s.point_load_kn),
                    "momento_local_knm_m": round1(s.point_moment_knm_m),
                    "momento_x_knm_m": round1(s.moment_x_knm_m * 10.0) / 10.0,
                    "momento_y_knm_m": round1(s.moment_y_knm_m * 10.0) / 10.0,
                    "metodo_x": s.method_x.label(),
                    "metodo_y": s.method_y.label(),
                    "flecha_mm": round1(s.deflection_mm),
                    "flecha_limite_mm": round1(s.deflection_limit_mm),
                    "paineis": s.panels.iter().map(|p| json!({
                        "indice": p.index,
                        "vaos_m": [round1(p.lx_m * 100.0) / 100.0, round1(p.ly_m * 100.0) / 100.0],
                        "ly_lx": round1(p.ratio * 100.0) / 100.0,
                        "momento_x_knm_m": round1(p.moment_x_knm_m * 10.0) / 10.0,
                        "momento_y_knm_m": round1(p.moment_y_knm_m * 10.0) / 10.0,
                        "metodo_x": p.method_x.label(),
                        "metodo_y": p.method_y.label(),
                        "flecha_mm": round1(p.deflection_mm),
                        "limite_mm": round1(p.limit_mm),
                    })).collect::<Vec<_>>(),
                    "apoiada": s.supported,
                    "atende": s.ok(),
                })
            })
            .collect();
        let beams: Vec<Value> = report
            .beams
            .iter()
            .map(|line| {
                json!({
                    "rotulo": line.label,
                    "pilares": line.joints,
                    "reacoes_kn": line.reactions.iter().map(|r| round1(*r)).collect::<Vec<_>>(),
                    "vaos": line.spans.iter().map(|s| json!({
                        "id": s.beam,
                        "vao_m": round1(s.length_m * 100.0) / 100.0,
                        "secao_cm": section_cm(s.width_m, s.height_m),
                        "altura_minima_cm": round1(s.height_min_m * 100.0),
                        "carga_permanente_kn_m": round1(s.dead_kn_m),
                        "carga_variavel_kn_m": round1(s.live_kn_m),
                        "momento_apoio_esq_knm": round1(s.moment_left),
                        "momento_apoio_dir_knm": round1(s.moment_right),
                        "momento_vao_knm": round1(s.moment_span),
                        "cortante_kn": round1(s.shear),
                        "flecha_mm": round1(s.deflection_mm),
                        "flecha_limite_mm": round1(s.limit_mm),
                        "momento_fissuracao_knm": round1(s.cracking_moment_knm),
                        "estadio_ii": s.cracked,
                        "inercia_equivalente_pct": (s.i_eq_ratio * 100.0).round(),
                        "as_branson_cm2": (s.branson_as_cm2 * 100.0).round() / 100.0,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let columns: Vec<Value> = report
            .columns
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "rotulo": c.label,
                    "secao_cm": section_cm(c.width_m, c.depth_m),
                    "posicao": c.position.label(),
                    "carga_kn": round1(c.load_kn),
                    "carga_pavimentos_superiores_kn": round1(c.upper_load_kn),
                    "area_necessaria_cm2": c.area_needed_cm2.max(360.0).round(),
                    "area_cm2": c.area_cm2.round(),
                    "altura_m": (c.height_m * 100.0).round() / 100.0,
                    "esbeltez": c.slenderness.round(),
                    "atende": c.ok(),
                })
            })
            .collect();
        let footings: Vec<Value> = report
            .footings
            .iter()
            .map(|f| {
                json!({
                    "id": f.id,
                    "rotulo": f.label,
                    "pilar": f.column,
                    "dimensoes_cm": section_cm(f.width_m, f.depth_m),
                    "carga_kn": round1(f.load_kn),
                    "necessaria_cm": section_cm(f.needed_width_m, f.needed_depth_m),
                    "altura_cm": round1(f.height_m * 100.0),
                    "excentricidade_m": (f.eccentricity_m * 1000.0).round() / 1000.0,
                    "tensao_max_kpa": round1(f.max_pressure_kpa),
                    "puncao_razao": (f.punching_ratio * 100.0).round() / 100.0,
                    "fator_tombamento": f.overturning_fs.is_finite().then(|| round1(f.overturning_fs)),
                    "recalque_mm": round1(f.settlement_mm),
                    "atende": f.ok(),
                })
            })
            .collect();
        let pile_caps: Vec<Value> = report
            .pile_caps
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "rotulo": c.label,
                    "pilar": c.column,
                    "dimensoes_cm": section_cm(c.width_m, c.depth_m),
                    "altura_cm": round1(c.height_m * 100.0),
                    "diametro_broca_cm": round1(c.pile_diameter_m * 100.0),
                    "comprimento_broca_m": round1(c.pile_depth_m),
                    "quantidade_brocas": c.pile_count,
                    "arranque_m": (c.anchor_depth_m * 100.0).round() / 100.0,
                    "carga_kn": round1(c.load_kn),
                    "capacidade_admissivel_kn": round1(c.allowable_kn),
                    "atrito_ruptura_kn": round1(c.shaft_rupture_kn),
                    "ponta_ruptura_kn": round1(c.tip_rupture_kn),
                    "tracao_vento_kn": round1(c.uplift_kn),
                    "capacidade_arrancamento_kn": round1(c.uplift_capacity_kn),
                    "recalque_mm": round1(c.settlement_mm),
                    "acima_lencol": c.above_water_table,
                    "com_sondagem": c.uses_survey,
                    "atende": c.ok(),
                })
            })
            .collect();
        let misaligned: Vec<Value> = report
            .misaligned_columns
            .iter()
            .map(|m| {
                json!({
                    "pavimento": m.floor_name,
                    "pilar": m.column_label,
                    "centro": xy(m.center),
                    "carga_kn": round1(m.load_kn),
                    "viga_transicao": m.transfer_beam,
                })
            })
            .collect();
        let concentrated_loads: Vec<Value> = report
            .concentrated_loads
            .iter()
            .map(|load| {
                json!({
                    "id": load.id,
                    "rotulo": load.label,
                    "tipo": load.kind.key(),
                    "centro": xy(load.center),
                    "carga_kn": round1(load.load_kn),
                    "apoio": load.support,
                    "laje": load.slab,
                    "momento_local_knm_m": round1(load.local_moment_knm_m),
                    "puncao_razao": load.punching_ratio.map(|ratio| (ratio * 100.0).round() / 100.0),
                    "apoiada": load.supported,
                })
            })
            .collect();
        let masonry_bands: Vec<Value> = report
            .masonry
            .bands
            .iter()
            .map(|band| {
                json!({
                    "abertura_id": band.opening_id,
                    "parede_id": band.wall_id,
                    "rotulo": band.label,
                    "tipo": match band.kind {
                        crate::domain::masonry_structure::MasonryBandKind::Lintel => "verga",
                        crate::domain::masonry_structure::MasonryBandKind::Sill => "contraverga",
                    },
                    "vao_m": (band.clear_span_m * 100.0).round() / 100.0,
                    "apoio_esquerdo_cm": round1(band.bearing_left_m * 100.0),
                    "apoio_direito_cm": round1(band.bearing_right_m * 100.0),
                    "secao_cm": section_cm(band.width_m, band.height_m),
                    "carga_kn_m": round1(band.load_kn_m),
                    "momento_knm": round1(band.moment_knm),
                    "armadura": band.bars,
                    "atende": band.ok(),
                })
            })
            .collect();
        let masonry_panels: Vec<Value> = report
            .masonry
            .panels
            .iter()
            .map(|panel| {
                json!({
                    "parede_id": panel.wall_id,
                    "rotulo": panel.label,
                    "material": panel.masonry,
                    "uso": match panel.kind {
                        crate::domain::masonry_structure::MasonryWallKind::Infill => "vedacao",
                        crate::domain::masonry_structure::MasonryWallKind::LoadBearing => "estrutural",
                    },
                    "comprimento_m": (panel.length_m * 100.0).round() / 100.0,
                    "altura_m": (panel.height_m * 100.0).round() / 100.0,
                    "espessura_cm": round1(panel.thickness_m * 100.0),
                    "esbeltez": round1(panel.slenderness),
                    "esbeltez_limite": panel.slenderness_limit,
                    "trecho_sem_amarracao_m": (panel.unbraced_m * 100.0).round() / 100.0,
                    "limite_sem_amarracao_m": (panel.unbraced_limit_m * 100.0).round() / 100.0,
                    "compressao_mpa": round1(panel.compression_mpa),
                    "resistencia_reduzida_mpa": round1(panel.compression_capacity_mpa),
                    "vento_fora_plano": panel.wind.as_ref().map(|wind| json!({
                        "altura_critica_m": round1(wind.height_m),
                        "q_kpa": round1(wind.q_kpa),
                        "momento_solicitante_knm_m": round1(wind.moment_knm_m),
                        "momento_resistente_knm_m": round1(wind.capacity_knm_m),
                        "uso": (wind.ratio * 100.0).round() / 100.0,
                        "pilares_extremos": wind.end_columns,
                        "fundacao_continua": wind.base_supported,
                        "sem_aberturas": wind.openings_clear,
                        "atende": wind.ok(),
                        "cintas": wind.bond_beam.as_ref().map(|bond| json!({
                            "intermediarias": bond.intermediate_count,
                            "coroamento": true,
                            "passo_m": round1(bond.spacing_m),
                            "vao_maximo_m": round1(bond.max_span_m),
                            "secao_atravessando_cm": bond.across_m.map(|w| round1(w * 100.0)),
                            "altura_cm": round1(bond.height_m * 100.0),
                            "momento_knm": round1(bond.moment_knm),
                            "cortante_kn": round1(bond.shear_kn),
                            "aco_cm2": round1(bond.steel_cm2),
                            "barras": bond.bars,
                            "atende": bond.ok(),
                        })),
                        "jambas_aberturas": wind.opening_jambs.iter().map(|j| json!({
                            "abertura_id": j.opening_id,
                            "largura_abertura_m": (j.opening_width_m * 100.0).round() / 100.0,
                            "altura_jamba_m": (j.jamb_height_m * 100.0).round() / 100.0,
                            "momento_knm": (j.moment_knm * 100.0).round() / 100.0,
                            "secao_cm": section_cm(j.width_m, j.depth_m),
                            "aco_cm2": (j.steel_cm2 * 100.0).round() / 100.0,
                            "barras": j.bars,
                            "uso": (j.ratio * 100.0).round() / 100.0,
                            "atende": j.ok,
                        })).collect::<Vec<_>>(),
                    })),
                    "atende": panel.ok(),
                })
            })
            .collect();
        let reinforcement_beams: Vec<Value> = reinforcement
            .beams
            .iter()
            .map(|beam| {
                json!({
                    "id": beam.beam_id,
                    "rotulo": beam.label,
                    "vao": beam.span_index,
                    "comprimento_m": round1(beam.length_m * 10.0) / 10.0,
                    "inferior": beam.bottom.notation(),
                    "superior_esquerda": beam.top.left().notation(),
                    "superior_direita": beam.top.right().notation(),
                    "superior_continua": matches!(&beam.top, crate::domain::reinforcement::BeamTopBars::Continuous(_)),
                    "origem_esforcos": if beam.frame_envelope.is_some() { "portico_2d_elu" } else { "viga_continua_caracteristica" },
                    "envoltoria_elu": beam.frame_envelope.as_ref().map(beam_ultimate_envelope_json),
                    "estribos": beam.stirrups.notation(),
                    "as_inferior_necessaria_cm2": round1(beam.bottom.required_cm2),
                    "as_inferior_minima_els_cm2": beam.service_minimum_cm2,
                    "as_inferior_fornecida_cm2": round1(beam.bottom.provided_cm2),
                    "ancoragem_inf_cm": (beam.bottom_anchorage_m * 100.0).round(),
                    "ancoragem_sup_cm": (beam.top_anchorage_m * 100.0).round(),
                    "dominio_ok": beam.flexural_domain_ok,
                    "barras_cabem_na_secao": beam.bottom.fits_section
                        && beam.top.left().fits_section
                        && beam.top.right().fits_section,
                    "cortante_atende": beam.stirrups.shear_capacity_ok,
                    "massa_kg": round1(beam.mass_kg),
                })
            })
            .collect();
        let reinforcement_columns: Vec<Value> = reinforcement
            .columns
            .iter()
            .map(|column| {
                json!({
                    "id": column.column_id,
                    "rotulo": column.label,
                    "longitudinal": column.longitudinal.notation(),
                    "estribos": column.ties.notation(),
                    "as_necessaria_cm2": round1(column.longitudinal.required_cm2),
                    "as_fornecida_cm2": round1(column.longitudinal.provided_cm2),
                    "traspasse_cm": (column.lap_length_m * 100.0).round(),
                    "taxa_aco_percentual": round1(column.steel_ratio * 100.0),
                    "massa_kg": round1(column.mass_kg),
                })
            })
            .collect();
        let reinforcement_slabs: Vec<Value> = reinforcement
            .slabs
            .iter()
            .map(|slab| {
                json!({
                    "id": slab.slab_id,
                    "rotulo": slab.label,
                    "comodo": slab.room,
                    "tipo": slab.kind.key(),
                    "principal": slab.main.notation(),
                    "distribuicao": slab.distribution.notation(),
                    "as_principal_necessaria_cm2_m": round1(slab.main.required_cm2_m),
                    "as_principal_fornecida_cm2_m": round1(slab.main.provided_cm2_m),
                    "dominio_ok": slab.domain_ok,
                    "massa_kg": round1(slab.mass_kg),
                })
            })
            .collect();
        let reinforcement_footings: Vec<Value> = reinforcement
            .footings
            .iter()
            .map(|footing| {
                json!({
                    "id": footing.footing_id,
                    "rotulo": footing.label,
                    "direcao_x": footing.bars_x.notation(),
                    "direcao_y": footing.bars_y.notation(),
                    "massa_kg": round1(footing.mass_kg),
                })
            })
            .collect();
        let steel_schedule: Vec<Value> = reinforcement
            .schedule
            .iter()
            .map(|row| {
                json!({
                    "marca": row.mark,
                    "elemento": row.element,
                    "posicao": row.position,
                    "forma": row.shape,
                    "diametro_mm": row.diameter_mm,
                    "quantidade": row.quantity,
                    "comprimento_unitario_m": round1(row.unit_length_m * 10.0) / 10.0,
                    "comprimento_total_m": round1(row.total_length_m * 10.0) / 10.0,
                    "massa_kg": round1(row.mass_kg),
                })
            })
            .collect();
        let steel_by_diameter: Vec<Value> = reinforcement
            .by_diameter
            .iter()
            .map(|row| {
                json!({
                    "bitola_mm": row.diameter_mm,
                    "comprimento_total_m": round1(row.total_length_m * 10.0) / 10.0,
                    "barras_12m": row.bars_12m,
                    "massa_kg": round1(row.mass_kg),
                    "massa_com_perda_10_kg": round1(row.mass_with_waste_kg),
                })
            })
            .collect();
        let soil_survey = self.project.structure.soil_survey.as_ref().map(|survey| {
            json!({
                "nome": survey.name,
                "nivel_agua_m": survey.governing_water_table_m(),
                "cota_sapata_m": survey.footing_depth_m,
                "tensao_admissivel_kpa": survey.footing_kpa(1.0).map(|v| v.round()),
                "furo_critico": survey.governing_footing_bulb(1.0).map(|(_, _, name)| name),
                "leituras": survey.readings.iter().map(|r| json!({
                    "profundidade_m": r.depth_m,
                    "n": r.n,
                    "solo": r.soil.key(),
                })).collect::<Vec<_>>(),
                "furos": survey.boreholes().into_iter().map(|b| json!({
                    "nome": b.name,
                    "nivel_agua_m": b.water_table_m,
                    "profundidade_max_m": b.deepest_m(),
                    "leituras": b.readings.iter().map(|r| json!({
                        "profundidade_m": r.depth_m,
                        "n": r.n,
                        "solo": r.soil.key(),
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        });
        let balance_beams: Vec<Value> = report
            .balance_beams
            .iter()
            .map(|b| {
                let strip = b.strip_alternative.as_ref().map(|alt| {
                    json!({
                        "largura_cm": (alt.width_m * 100.0).round(),
                        "tensao_kpa": round1(alt.pressure_kpa),
                        "viavel": alt.viable,
                        "nota": alt.note,
                    })
                });
                json!({
                    "id": b.id,
                    "rotulo": b.label,
                    "pilar_divisa": b.column,
                    "pilar_interno": b.anchor,
                    "vao_m": round1(b.span_m * 100.0) / 100.0,
                    "excentricidade_cm": round1(b.eccentricity_m * 100.0),
                    "carga_pilar_kn": round1(b.column_load_kn),
                    "reacao_sapata_kn": round1(b.reaction_kn),
                    "alivio_kn": round1(b.relief_kn),
                    "levanta_apoio": b.uplift(),
                    "fator_tombamento_par": b.pair_overturning_fs.is_finite().then(|| round1(b.pair_overturning_fs)),
                    "tombamento_ok": b.overturning_ok(),
                    "tensao_divisa_kpa": b.boundary_pressure_kpa.map(round1),
                    "tensao_apoio_kpa": b.anchor_pressure_kpa.map(round1),
                    "solo_par_ok": b.pair_soil_ok(),
                    "momento_knm": round1(b.moment_knm),
                    "cortante_kn": round1(b.shear_kn),
                    "secao_cm": b.height_m.map(|h| section_cm(b.width_m, h)),
                    "armadura_superior_cm2": (b.top_steel_cm2 * 100.0).round() / 100.0,
                    "barras": b.bars,
                    "estribos": b.stirrups,
                    "atende": b.overturning_ok() && b.pair_soil_ok() && b.height_m.is_some(),
                    "alternativa_sapata_corrida": strip,
                })
            })
            .collect();
        let foundation_study = (!report.columns.is_empty())
            .then(|| foundation_study_json(&self.project.foundation_study()));
        let frames_2d: Vec<Value> = report
            .frames
            .iter()
            .map(|f| {
                let frame_beams: Vec<Value> = f
                    .beams
                    .iter()
                    .map(|b| {
                        json!({
                            "id": b.beam_id,
                            "rotulo": b.label,
                            "vao_m": (b.length_m * 100.0).round() / 100.0,
                            "carga_distribuida_kn_m": round1(b.load_kn_m),
                            "cargas_pontuais": b.point_loads.iter().map(|p| json!({
                                "posicao_m": (p.position_m * 100.0).round() / 100.0,
                                "carga_kn": round1(p.load_kn),
                                "fator_quase_permanente": (p.quasi_factor * 100.0).round() / 100.0,
                            })).collect::<Vec<_>>(),
                            "momento_esq_knm": round1(b.moment_left_knm),
                            "momento_vao_knm": round1(b.moment_span_knm),
                            "momento_dir_knm": round1(b.moment_right_knm),
                            "cortante_kn": round1(b.max_shear_kn),
                            "axial_kn": round1(b.axial_kn),
                            "flecha_elastica_mm": (b.elastic_deflection_mm * 100.0).round() / 100.0,
                            "envoltoria_elu": beam_ultimate_envelope_json(&b.ultimate),
                        })
                    })
                    .collect();
                let frame_cols: Vec<Value> = f
                    .columns
                    .iter()
                    .map(|c| {
                        json!({
                            "id": c.column_id,
                            "rotulo": c.label,
                            "axial_kn": round1(c.axial_kn),
                            "cortante_kn": round1(c.shear_kn),
                            "momento_base_knm": round1(c.moment_base_knm),
                            "momento_topo_knm": round1(c.moment_top_knm),
                            "momento_max_knm": round1(c.max_moment_knm),
                            "reacao_rx_kn": round1(c.reaction_rx_kn),
                            "reacao_ry_kn": round1(c.reaction_ry_kn),
                            "reacao_mz_knm": round1(c.reaction_mz_knm),
                        })
                    })
                    .collect();
                let frame_cases: Vec<Value> = f
                    .load_cases
                    .iter()
                    .map(|lc| {
                        json!({
                            "caso": lc.kind.code(),
                            "descricao": lc.kind.label(),
                            "vertical_kn": round1(lc.total_vertical_kn),
                            "horizontal_kn": round1(lc.total_horizontal_kn),
                            "momento_max_viga_knm": round1(lc.max_beam_moment_knm),
                            "momento_max_pilar_knm": round1(lc.max_column_moment_knm),
                            "deslocamento_lateral_mm": (lc.max_lateral_drift_mm * 100.0).round() / 100.0,
                        })
                    })
                    .collect();
                let frame_combos: Vec<Value> = f
                    .combinations
                    .iter()
                    .map(|c| {
                        json!({
                            "codigo": c.code,
                            "categoria": c.category.key(),
                            "formula": c.formula,
                            "momento_pos_viga_knm": round1(c.max_beam_pos_knm),
                            "momento_neg_viga_knm": round1(c.min_beam_neg_knm),
                            "cortante_viga_kn": round1(c.max_beam_shear_kn),
                            "momento_pilar_knm": round1(c.max_column_moment_knm),
                            "compressao_pilar_kn": round1(c.max_column_compression_kn),
                            "axial_max_tracao_pilar_kn": round1(c.max_column_tension_kn),
                            "flecha_viga_mm": (c.max_beam_deflection_mm * 100.0).round() / 100.0,
                            "deslocamento_lateral_mm": (c.max_lateral_drift_mm * 100.0).round() / 100.0,
                        })
                    })
                    .collect();
                json!({
                    "rotulo": f.label,
                    "nos_rigidos": f.rigid_joints,
                    "bases_engastadas": f.fixed_bases,
                    "contraventado": f.braced_sway,
                    "comprimento_total_m": (f.total_length_m * 100.0).round() / 100.0,
                    "altura_m": (f.story_height_m * 100.0).round() / 100.0,
                    "vento_lateral_kn": round1(f.lateral_wind_kn),
                    "vento_caracteristico_kn": round1(f.characteristic_wind_kn),
                    "carga_pontual_total_kn": round1(f.total_point_load_kn()),
                    "momento_max_viga_knm": round1(f.max_beam_moment_knm()),
                    "momento_max_pilar_knm": round1(f.max_column_moment_knm()),
                    "deslocamento_lateral_mm": (f.max_lateral_drift_mm * 100.0).round() / 100.0,
                    "limite_deslocamento_mm": (f.drift_limit_mm * 100.0).round() / 100.0,
                    "deslocamento_ok": f.drift_ok(),
                    "residuo_equilibrio_kn": f.equilibrium_residual_kn,
                    "envoltoria_elu": {
                        "momento_pos_viga_knm": round1(f.envelope.elu_max_beam_pos_knm),
                        "combinacao_momento_pos": f.envelope.elu_max_beam_pos_combo,
                        "momento_neg_viga_knm": round1(f.envelope.elu_min_beam_neg_knm),
                        "combinacao_momento_neg": f.envelope.elu_min_beam_neg_combo,
                        "cortante_viga_kn": round1(f.envelope.elu_max_beam_shear_kn),
                        "combinacao_cortante": f.envelope.elu_max_beam_shear_combo,
                        "momento_pilar_knm": round1(f.envelope.elu_max_column_moment_knm),
                        "combinacao_momento_pilar": f.envelope.elu_max_column_moment_combo,
                        "compressao_pilar_kn": round1(f.envelope.elu_max_column_compression_kn),
                        "combinacao_compressao_pilar": f.envelope.elu_max_column_compression_combo,
                        "axial_min_pilar_kn": round1(f.envelope.elu_min_column_axial_kn),
                        "combinacao_axial_min": f.envelope.elu_min_column_axial_combo,
                        "arrancamento_pilar": f.envelope.has_column_uplift,
                    },
                    "envoltoria_els": {
                        "flecha_viga_qp_mm": (f.envelope.els_qp_max_beam_deflection_mm * 100.0).round() / 100.0,
                        "deslocamento_lateral_f_mm": (f.envelope.els_f_max_drift_mm * 100.0).round() / 100.0,
                        "combinacao_deslocamento": f.envelope.els_f_drift_combo,
                    },
                    "casos_de_carga": frame_cases,
                    "combinacoes": frame_combos,
                    "vigas": frame_beams,
                    "pilares": frame_cols,
                })
            })
            .collect();
        let frame_failures: Vec<Value> = report
            .frame_failures
            .iter()
            .map(|failure| {
                json!({
                    "rotulo": failure.label,
                    "motivo": failure.reason,
                    "atende": false,
                })
            })
            .collect();
        let loads_json = loads_analysis_json(&report.loads);
        let grid = self.project.cached_grid_analysis();
        let grid_json = grid_analysis_json(&grid);
        let ov = self.project.structure.overlay;
        let overlay_json = json!({
            "cores_aproveitamento": ov.utilisation_colors,
            "diagrama_planta": ov.beam_diagrams.key(),
            "deformada_planta": ov.deformed_shape,
            "caminho_cargas": ov.load_path,
        });
        Ok(ToolOutput::ok(format!(
            "{} laje(s), {} viga(s), {} pórtico(s) 2D resolvido(s), {} falha(s) de pórtico, {} pilar(es), {} sapata(s), {} verga(s)/contraverga(s), {} painel(is) de alvenaria, {} kg de aço preliminar, {} alerta(s).",
            slabs.len(),
            beams.len(),
            frames_2d.len(),
            frame_failures.len(),
            columns.len(),
            footings.len(),
            masonry_bands.len(),
            masonry_panels.len(),
            round1(reinforcement.total_mass_kg()),
            report.warnings.len()
        ))
        .with_structured(json!({
            "projeto_id": self.active_project_id(),
            "materiais": {
                "fck_mpa": settings.fck_mpa,
                "solo_kpa": settings.soil_kpa,
                "pavimentos": settings.floors,
                "ocupacao": settings.occupancy.key(),
                "sobrecarga_kn_m2": settings.live_kn_m2,
                "revestimento_kn_m2": settings.finish_kn_m2,
                "peso_alvenaria_kn_m3": settings.masonry_kn_m3,
                "sobrecarga_por_comodo": settings.per_room_live_loads,
                "portico_nos_rigidos": settings.frame_rigid_joints,
                "portico_bases_engastadas": settings.frame_fixed_bases,
            },
            "cargas_e_combinacoes": loads_json,
            "sobreposicao_planta": overlay_json,
            "grelha_lajes_vigas": grid_json,
            "escadas_e_balancos": cantilevers_json(&report.cantilevers),
            "lajes": slabs,
            "vigas": beams,
            "porticos_2d": frames_2d,
            "falhas_porticos_2d": frame_failures,
            "pilares": columns,
            "sapatas": footings,
            "blocos_broca": pile_caps,
            "telhados": report.roofs.iter().map(roof_json).collect::<Vec<_>>(),
            "vento": wind_json(&self.project.structure.wind, &report.wind, &report.masonry),
            "mouroes": report.timber_posts.iter().map(|p| json!({
                "id": p.id,
                "rotulo": p.label,
                "secao": if p.round { "redonda" } else { "quadrada" },
                "diametro_cm": round1(p.size_m * 100.0),
                "altura_m": (p.height_m * 100.0).round() / 100.0,
                "carga_kn": round1(p.load_kn),
                "especie": p.species.key(),
                "nome_cientifico": p.species.scientific_name(),
                "classe": p.timber_class.key(),
                "tratada": p.treated,
                "vento_kpa": round1(p.wind_kpa * 100.0) / 100.0,
                "forca_vento_kn": round1(p.wind_force_kn),
                "momento_vento_knm": round1(p.wind_moment_knm),
                "uso_axial": (p.axial_ratio.min(9.99) * 100.0).round() / 100.0,
                "uso_vento": (p.wind_ratio.min(9.99) * 100.0).round() / 100.0,
                "esbeltez": p.slenderness.round(),
                "uso": (p.ratio.min(9.99) * 100.0).round() / 100.0,
                "atende": p.ok(),
                "diametro_necessario_cm": p.needed_m.map(|s| round1(s * 100.0)),
                "base_cm": round1(p.base_m * 100.0),
                "profundidade_base_cm": round1(p.base_depth_m * 100.0),
                "pressao_base_kpa": round1(p.base_kpa),
                "uso_estabilidade_base": (p.base_stability_ratio.min(9.99) * 100.0).round() / 100.0,
                "chumbador_kn": round1(p.anchor_kn),
            })).collect::<Vec<_>>(),
            "vigas_madeira": report.eave_beams.iter().map(|b| json!({
                "rotulo": b.label,
                "especie": b.species.key(),
                "classe": b.timber_class.key(),
                "tratada": b.treated,
                "de": b.from_post,
                "para": b.to_post,
                "vao_m": (b.span_m * 100.0).round() / 100.0,
                "secao_cm": format!("{}x{}", b.section_cm.0, b.section_cm.1),
                "uso_flexao": (b.bending_ratio.min(9.99) * 100.0).round() / 100.0,
                "uso_cisalhamento": (b.shear_ratio.min(9.99) * 100.0).round() / 100.0,
                "uso_flecha": (b.deflection_ratio.min(9.99) * 100.0).round() / 100.0,
                "atende": b.ok && b.treated,
                "cargas_kn": b.loads.iter().map(|(_, p)| round1(*p)).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "sondagem": soil_survey,
            "estudo_fundacoes": foundation_study,
            "fundacao_paredes": report.wall_foundations.iter().map(|f| json!({
                "rotulo": f.label,
                "parede": f.wall_id,
                "tipo": match f.kind {
                    crate::domain::wall_foundations::WallFoundationKind::GroundBeam => "baldrame",
                    crate::domain::wall_foundations::WallFoundationKind::StripFooting => "sapata_corrida",
                },
                "comprimento_m": round1(f.length_m * 100.0) / 100.0,
                "carga_kn_m": round1(f.load_kn_m),
                "secao_cm": f.height_m.map(|h| section_cm(f.width_m, h)),
                "largura_cm": round1(f.width_m * 100.0),
                "momento_knm": round1(f.moment_knm),
                "armadura_cm2": (f.steel_cm2 * 100.0).round() / 100.0,
                "barras": f.bars,
                "reacoes_pilares": f.reactions.iter().map(|(id, kn)| json!({"pilar_id": id, "carga_kn": round1(*kn)})).collect::<Vec<_>>(),
                "reacoes_baldrames": f.beam_reactions.iter().map(|(id, kn)| json!({"baldrame_id": id, "carga_kn": round1(*kn)})).collect::<Vec<_>>(),
                "apoios_insuficientes": f.unsupported,
                "pressao_solo_kpa": round1(f.pressure_kpa),
                "largura_necessaria_cm": round1(f.needed_width_m * 100.0),
            })).collect::<Vec<_>>(),
            "vigas_alavanca": balance_beams,
            "pilares_desalinhados": misaligned,
            "cargas_concentradas": concentrated_loads,
            "alvenaria": {
                "vergas_contravergas": masonry_bands,
                "paineis": masonry_panels,
            },
            "armaduras": {
                "carater": "estimativa preliminar para revisão; não é detalhamento executivo",
                "vigas": reinforcement_beams,
                "pilares": reinforcement_columns,
                "lajes": reinforcement_slabs,
                "sapatas": reinforcement_footings,
                "tabela_aco": steel_schedule,
                "resumo_bitolas": steel_by_diameter,
                "massa_total_kg": round1(reinforcement.total_mass_kg()),
                "massa_com_perda_10_kg": round1(reinforcement.total_mass_with_waste_kg()),
                "total_barras_12m": reinforcement.total_bars_12m(),
                "avisos": reinforcement.warnings,
            },
            "semaforo": verdicts_json(&crate::domain::verdict::structure_verdicts(
                &self.project,
                &report,
            )),
            "limites": {
                "recusado": crate::domain::limits::structural_refusal(&self.project),
                "elementos_fora": crate::domain::limits::element_violations(&report)
                    .iter()
                    .map(|v| json!({ "elemento": v.element, "motivo": v.message }))
                    .collect::<Vec<_>>(),
            },
            "premissas": crate::domain::verdict::assumptions(&self.project),
            "resumo_para_a_familia": crate::domain::protection::glossary::family_summary(
                &crate::domain::verdict::safety_summary(&self.project),
            ),
            "hipoteses": hypotheses_json(&self.project),
            "riscos_terreno": {
                "com_risco": self.project.protection.site_risks.risky().iter().map(|r| r.key()).collect::<Vec<_>>(),
                "sem_resposta": self.project.protection.site_risks.unanswered().iter().map(|r| r.key()).collect::<Vec<_>>(),
            },
            "listas_concretagem": crate::domain::protection::checklists::pour_checklists(&reinforcement)
                .iter()
                .map(|list| json!({
                    "titulo": list.title,
                    "elementos": list.elements,
                    "conferir": list.items,
                }))
                .collect::<Vec<_>>(),
            "onde_buscar_ajuda": crate::domain::protection::help::help_lines(
                &self.project.protection.help_contacts,
            ),
            "alertas": report.warnings,
        })))
    }
}
