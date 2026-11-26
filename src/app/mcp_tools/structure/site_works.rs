//! The presumed soil profile and the in-ground pools, for the assistant:
//! reading `definir_solo_provavel` / `configurar_piscina` and reporting them
//! in `pre_dimensionamento`.

use serde_json::{Value, json};

use super::super::text;
use crate::domain::model::Project;
use crate::domain::soil::{Likelihood, PresumedLayer, PresumedSoil, PresumedSoilType};

fn r2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// The profile from `camadas: [{solo, expectativa}]` and an optional `fonte`.
pub(super) fn presumed_soil_arg(args: &Value) -> Result<PresumedSoil, String> {
    let rows = args
        .get("camadas")
        .and_then(Value::as_array)
        .ok_or("Informe 'camadas': [{solo, expectativa}].")?;
    let soils = PresumedSoilType::ALL
        .iter()
        .map(|s| s.key())
        .collect::<Vec<_>>()
        .join(", ");
    let mut layers = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let raw = text(row, "solo").ok_or_else(|| format!("Camada {}: falta 'solo'.", i + 1))?;
        let soil = PresumedSoilType::parse(&raw)
            .ok_or_else(|| format!("Camada {}: 'solo' deve ser um de: {soils}.", i + 1))?;
        let likelihood = match text(row, "expectativa") {
            Some(raw) => Likelihood::parse(&raw).ok_or_else(|| {
                format!(
                    "Camada {}: 'expectativa' deve ser provavel, possivel ou possivel_em_profundidade.",
                    i + 1
                )
            })?,
            None => Likelihood::Probable,
        };
        layers.push(PresumedLayer { soil, likelihood });
    }
    PresumedSoil::new(layers, text(args, "fonte"))
}

/// The presumed profile and what it means for the calculations.
pub(super) fn presumed_soil_json(project: &Project) -> Value {
    let Some(profile) = &project.structure.presumed_soil else {
        return Value::Null;
    };
    let analysis = profile.analysis();
    json!({
        "camadas": profile.layers.iter().map(|layer| {
            let p = layer.soil.properties();
            json!({
                "solo": layer.soil.key(),
                "nome": layer.soil.label(),
                "expectativa": layer.likelihood.key(),
                "expectativa_nome": layer.likelihood.label(),
                "n_spt_provavel": [p.n_range.0, p.n_range.1],
                "peso_especifico_kn_m3": p.unit_weight_kn_m3,
                "atrito_graus": p.friction_deg,
                "infiltracao_l_m2_dia": p.absorption_l_m2_day,
                "colapsivel": p.collapsible,
            })
        }).collect::<Vec<_>>(),
        "fonte": profile.source,
        "n_spt_governante": analysis.governing_n,
        "solo_governante": analysis.governing_soil.key(),
        "tensao_admissivel_provavel_kpa": analysis.allowable_kpa,
        "tensao_em_uso_kpa": project.structure.settings.soil_kpa,
        "sondagem_prevalece": project.structure.soil_survey.is_some(),
        "colapsivel": analysis.collapsible,
        "colapsivel_provavel": analysis.collapsible_probable,
        "peso_especifico_empuxo_kn_m3": analysis.unit_weight_kn_m3,
        "atrito_empuxo_graus": analysis.friction_deg,
        "infiltracao_l_m2_dia": analysis.absorption_l_m2_day,
        "infiltracao_pior_l_m2_dia": analysis.worst_absorption_l_m2_day,
        "camada_firme_em_profundidade": analysis.firm_layer_at_depth,
        "escavacao_dificil": analysis.hard_digging,
        "observacoes": analysis.notes,
        "carater": "estimativa sem sondagem: só deixa os cálculos mais seguros; não é prova",
    })
}

/// Every pool, checked.
pub(super) fn pools_json(project: &Project) -> Vec<Value> {
    project
        .pool_report()
        .iter()
        .map(|pool| {
            let wall = &pool.wall;
            json!({
                "rotulo": pool.label,
                "comodo_id": pool.room_id,
                "comodo": pool.room,
                "explicita": project.structure.pools.iter().any(|p| p.room_id == pool.room_id),
                "profundidade_m": r2(pool.spec.depth_m),
                "espessura_parede_cm": r2(pool.spec.wall_thickness_m * 100.0),
                "espessura_fundo_cm": r2(pool.spec.floor_thickness_m * 100.0),
                "sobrecarga_borda_kn_m2": r2(pool.spec.edge_surcharge_kpa),
                "area_interna_m2": r2(pool.inner_area_m2),
                "perimetro_m": r2(pool.perimeter_m),
                "volume_agua_m3": r2(pool.water_m3),
                "escavacao_m": r2(pool.excavation_depth_m),
                "solo": {
                    "peso_especifico_kn_m3": r2(pool.unit_weight_kn_m3),
                    "atrito_graus": r2(pool.friction_deg),
                    "k0": r2(pool.k0),
                    "tensao_admissivel_kpa": r2(pool.allowable_kpa),
                    "colapsivel": pool.collapsible,
                },
                "sobrecarga_total_kn_m2": r2(pool.surcharge_kpa),
                "lencol_m": pool.groundwater_m.map(r2),
                "lencol_desconhecido": pool.groundwater_unknown,
                "parede": {
                    "altura_m": r2(wall.height_m),
                    "espessura_cm": r2(wall.thickness_m * 100.0),
                    "momento_agua_knm_m": r2(wall.moment_water_knm_m),
                    "momento_terra_knm_m": r2(wall.moment_soil_knm_m),
                    "md_knm_m": r2(wall.md_knm_m),
                    "vd_kn_m": r2(wall.vd_kn_m),
                    "vrd1_kn_m": r2(wall.vrd1_kn_m),
                    "uso_flexao": r2(wall.bending_ratio),
                    "uso_cortante": r2(wall.shear_ratio),
                    "aco_por_face_cm2_m": r2(wall.steel_cm2_m),
                    "aco_elu_cm2_m": r2(wall.steel_uls_cm2_m),
                    "aco_servico_cm2_m": r2(wall.steel_service_cm2_m),
                    "aco_minimo_cm2_m": r2(wall.steel_min_cm2_m),
                    "barras_por_face": wall.bars,
                    "espessura_necessaria_cm": r2(wall.needed_thickness_m * 100.0),
                    "uso": r2(wall.utilisation.min(9.99)),
                    "atende": wall.utilisation <= 1.0 + 1e-9,
                },
                "fundo": {
                    "pressao_kpa": r2(pool.floor_pressure_kpa),
                    "uso": r2(pool.floor_ratio.min(9.99)),
                    "barras_cada_face": pool.floor_bars,
                },
                "flutuacao": pool.flotation.map(|f| json!({
                    "altura_agua_m": r2(f.water_depth_m),
                    "empuxo_kn": r2(f.uplift_kn),
                    "peso_kn": r2(f.weight_kn),
                    "uso": r2(f.ratio.min(9.99)),
                    "atende": f.ratio <= 1.0 + 1e-9,
                })),
                "fundacoes_na_zona": pool.foundations_in_zone.iter().map(|(label, clearance, needed)| json!({
                    "elemento": label,
                    "folga_m": r2(*clearance),
                    "pede_m": r2(*needed),
                })).collect::<Vec<_>>(),
                "divisa": {
                    "folga_m": pool.boundary_clearance_m.map(r2),
                    "pede_m": r2(pool.boundary_needed_m),
                    "na_zona": pool.boundary_in_zone(),
                },
                "fck_mpa": pool.fck_mpa,
                "filtro_m3_h": r2(pool.filter_m3_h),
                "cuidados": pool.care,
            })
        })
        .collect()
}
