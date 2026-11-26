//! Reading the structural tools' arguments: soil and occupancy classes,
//! SPT readings, the soil survey, roofs, centimetres and rooms.

use serde_json::Value;

use super::super::{number, required_number, text, to_model};
use crate::domain::model::Project;
use crate::domain::structure::{OccupancyClass, SoilClass};

pub(super) fn parse_soil_class(raw: &str) -> Option<SoilClass> {
    match raw.trim().to_lowercase().as_str() {
        "argila_mole" | "soft_clay" => Some(SoilClass::SoftClay),
        "argila_media" | "argila_média" | "medium_clay" => Some(SoilClass::MediumClay),
        "areia_media" | "areia_média" | "medium_sand" => Some(SoilClass::MediumSand),
        "areia_compacta" | "dense_sand" => Some(SoilClass::DenseSand),
        "rocha" | "rock" => Some(SoilClass::Rock),
        _ => None,
    }
}

pub(super) fn parse_occupancy_class(raw: &str) -> Option<OccupancyClass> {
    match raw.trim().to_lowercase().as_str() {
        "residencial" | "residential" => Some(OccupancyClass::Residential),
        "escritorio" | "escritório" | "office" => Some(OccupancyClass::Office),
        "garagem" | "garage" => Some(OccupancyClass::Garage),
        "comercial" | "comercio" | "comércio" | "retail" => Some(OccupancyClass::Retail),
        _ => None,
    }
}

pub(super) fn parse_spt_readings(
    rows: &[Value],
) -> Result<Vec<crate::domain::soil::SptReading>, String> {
    use crate::domain::soil::{SoilKind, SptReading};
    let mut readings = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let depth = number(row, "profundidade_m")?
            .ok_or_else(|| format!("Leitura {}: falta 'profundidade_m'.", i + 1))?;
        let n = number(row, "n")?.ok_or_else(|| format!("Leitura {}: falta 'n'.", i + 1))?;
        if n < 0.0 || n.fract() != 0.0 {
            return Err(format!(
                "Leitura {}: 'n' é um número inteiro de golpes.",
                i + 1
            ));
        }
        let soil = match text(row, "solo") {
            Some(raw) => SoilKind::parse(&raw).ok_or_else(|| {
                format!(
                    "Leitura {}: 'solo' deve ser argila, silte_argiloso, silte_arenoso ou areia.",
                    i + 1
                )
            })?,
            None => SoilKind::Clay,
        };
        readings.push(SptReading {
            depth_m: depth,
            n: n as u32,
            soil,
        });
    }
    Ok(readings)
}

/// The soil survey from 'furos' [{nome, nivel_agua_m, leituras}] or a single
/// borehole via 'leituras' [{profundidade_m, n, solo}], (#112).
pub(super) fn survey_arg(args: &Value) -> Result<crate::domain::soil::SoilSurvey, String> {
    use crate::domain::soil::{Borehole, SoilSurvey};
    let footing_depth = number(args, "cota_sapata_m")?.unwrap_or(1.0);
    if let Some(furos) = args.get("furos").and_then(Value::as_array) {
        if furos.is_empty() {
            return Err("Informe pelo menos um furo em 'furos'.".to_owned());
        }
        let mut boreholes = Vec::with_capacity(furos.len());
        for (idx, item) in furos.iter().enumerate() {
            let rows = item
                .get("leituras")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("Furo {}: informe 'leituras'.", idx + 1))?;
            let readings = parse_spt_readings(rows)?;
            let name = text(item, "nome").unwrap_or_else(|| format!("SP-{:02}", idx + 1));
            let water = number(item, "nivel_agua_m")?;
            boreholes.push(Borehole::new(name, readings, water)?);
        }
        return SoilSurvey::from_boreholes(boreholes, footing_depth);
    }
    let rows = args
        .get("leituras")
        .and_then(Value::as_array)
        .ok_or("Informe 'leituras': [{profundidade_m, n, solo}] ou 'furos': [{nome, leituras}].")?;
    let readings = parse_spt_readings(rows)?;
    SoilSurvey::new(
        text(args, "nome").unwrap_or_else(|| "SP-01".to_owned()),
        readings,
        number(args, "nivel_agua_m")?,
        footing_depth,
    )
}

/// The roof from the arguments of definir_telhado, starting from `base`
/// when editing one (#114).
pub(super) fn roof_arg(
    args: &Value,
    base: Option<crate::domain::roof::Roof>,
) -> Result<crate::domain::roof::Roof, String> {
    use crate::domain::roof::{Roof, RoofSide, TileKind, TimberSpecies};
    let editing = base.is_some();
    let mut roof = match base {
        Some(roof) => roof,
        None => {
            let x = required_number(args, "x")?;
            let y = required_number(args, "y")?;
            let width = required_number(args, "largura_m")?;
            let length = required_number(args, "comprimento_m")?;
            let high = required_number(args, "altura_alta_m")?;
            let low = required_number(args, "altura_baixa_m")?;
            Roof::new(
                to_model(x, y),
                to_model(x + width, y + length),
                RoofSide::West,
                high,
                low,
            )
        }
    };
    if editing && (args.get("x").is_some() || args.get("largura_m").is_some()) {
        let x = required_number(args, "x")?;
        let y = required_number(args, "y")?;
        let width = required_number(args, "largura_m")?;
        let length = required_number(args, "comprimento_m")?;
        let fresh = Roof::new(
            to_model(x, y),
            to_model(x + width, y + length),
            roof.high_side,
            2.0,
            1.0,
        );
        roof.min = fresh.min;
        roof.max = fresh.max;
    }
    if let Some(raw) = text(args, "lado_alto") {
        roof.high_side =
            RoofSide::parse(&raw).ok_or("'lado_alto' deve ser oeste, leste, norte ou sul.")?;
    }
    if let Some(v) = number(args, "altura_alta_m")? {
        roof.high_m = v;
    }
    if let Some(v) = number(args, "altura_baixa_m")? {
        roof.low_m = v;
    }
    if let Some(raw) = text(args, "telha") {
        roof.tile = TileKind::parse(&raw).ok_or(
            "'telha' deve ser ceramica, romana, americana, concreto, fibrocimento ou metalica.",
        )?;
    }
    if let Some(raw) = text(args, "madeira") {
        roof.timber = TimberSpecies::parse(&raw)
            .ok_or("'madeira' deve ser uma classe C20–C60 ou uma espécie cadastrada.")?
            .class();
    }
    if let Some(v) = number(args, "espacamento_vigas_m")? {
        roof.beam_spacing_m = v;
    }
    if let Some(value) = args.get("contraventamento_kn") {
        roof.lateral_bracing_kn = if value.is_null() {
            None
        } else {
            Some(
                value
                    .as_f64()
                    .ok_or("'contraventamento_kn' deve ser um número em kN ou null.")?,
            )
        };
    }
    if let Some(value) = args.get("secao_vigas_cm") {
        let values = value
            .as_array()
            .ok_or("'secao_vigas_cm' deve ser [largura, altura] em cm, ou [].")?;
        roof.beam_section_cm = match values.as_slice() {
            [] => None,
            [width, height] => Some((
                width
                    .as_f64()
                    .ok_or("A largura da viga deve ser numerica.")?,
                height
                    .as_f64()
                    .ok_or("A altura da viga deve ser numerica.")?,
            )),
            _ => return Err("'secao_vigas_cm' deve ter exatamente dois numeros.".to_owned()),
        };
    }
    if let Some(value) = args.get("posicoes_vigas_extras_m") {
        let values = value
            .as_array()
            .ok_or("'posicoes_vigas_extras_m' deve ser uma lista de distancias em metros.")?;
        roof.extra_beam_positions_m = values
            .iter()
            .map(|value| {
                value
                    .as_f64()
                    .ok_or("Cada posicao de viga extra deve ser numerica.")
            })
            .collect::<Result<Vec<_>, _>>()?;
        roof.extra_beam_positions_m.sort_by(f64::total_cmp);
    }
    if let Some(v) = number(args, "espacamento_tercas_m")? {
        roof.purlin_spacing_m = v;
    }
    if let Some(v) = number(args, "espacamento_caibros_m")? {
        roof.rafter_spacing_m = v;
    }
    if let Some(raw) = text(args, "estilo") {
        roof.style = crate::domain::roof::RoofStyle::parse(&raw)
            .ok_or("'estilo' deve ser meia_agua, duas_aguas ou quatro_aguas.")?;
    }
    if let Some(v) = number(args, "beiral_m")? {
        roof.overhang_m = v;
    }
    if let Some(v) = args.get("tesouras").and_then(Value::as_bool) {
        roof.use_trusses = v;
    }
    if let Some(v) = args.get("aparente").and_then(Value::as_bool) {
        roof.exposed = v;
    }
    Ok(roof)
}

/// A size argument in centimetres, as metres.
pub(super) fn centimetres(args: &Value, key: &str) -> Result<Option<f64>, String> {
    Ok(number(args, key)?.map(|cm| cm / 100.0))
}

/// The room named by 'comodo' (its name, or its id).
pub(super) fn room_arg(project: &Project, args: &Value) -> Result<u64, String> {
    if let Some(id) = args.get("comodo").and_then(Value::as_u64) {
        return project
            .rooms
            .iter()
            .find(|room| room.id == id)
            .map(|room| room.id)
            .ok_or_else(|| format!("Cômodo {id} não encontrado."));
    }
    let name = text(args, "comodo").ok_or("Informe 'comodo' (nome ou id).")?;
    project
        .rooms
        .iter()
        .find(|room| room.display_name().eq_ignore_ascii_case(name.trim()))
        .map(|room| room.id)
        .ok_or_else(|| format!("Nenhum cômodo chamado '{name}'."))
}
