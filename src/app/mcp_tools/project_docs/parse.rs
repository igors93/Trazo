//! Parsing the documentation tools' arguments: colours, line types, scales,
//! elevation sides, railings and stair shapes.

use crate::domain::{
    circulation::{RailingSide, StairShape},
    layers::LineType,
    sheet::DrawingScale,
    views::ElevationSide,
};

pub(super) fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0 + 0.0
}

pub(in crate::app::mcp_tools) fn parse_hex_color(raw: &str) -> Result<[u8; 3], String> {
    let hex = raw.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Cor inválida: use hexadecimal #RRGGBB (ex.: #c62828).".to_owned());
    }
    let r = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| "Cor hexadecimal inválida (use #RRGGBB).".to_owned())?;
    let g = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| "Cor hexadecimal inválida (use #RRGGBB).".to_owned())?;
    let b = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| "Cor hexadecimal inválida (use #RRGGBB).".to_owned())?;
    Ok([r, g, b])
}

pub(super) fn parse_line_type(raw: &str) -> Option<LineType> {
    match raw.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
        "continua" | "contínua" | "solid" => Some(LineType::Solid),
        "tracejada" | "dashed" => Some(LineType::Dashed),
        "traco_ponto" | "traço_ponto" | "dash_dot" | "dashdot" => Some(LineType::DashDot),
        "pontilhada" | "dotted" => Some(LineType::Dotted),
        _ => None,
    }
}

pub(super) fn line_type_key(lt: LineType) -> &'static str {
    match lt {
        LineType::Solid => "continua",
        LineType::Dashed => "tracejada",
        LineType::DashDot => "traco_ponto",
        LineType::Dotted => "pontilhada",
    }
}

pub(in crate::app::mcp_tools) fn parse_scale(raw: &str) -> Option<DrawingScale> {
    let clean = raw.trim().to_lowercase().replace(' ', "");
    match clean.as_str() {
        "1:25" | "25" => Some(DrawingScale::S1_25),
        "1:50" | "50" => Some(DrawingScale::S1_50),
        "1:75" | "75" => Some(DrawingScale::S1_75),
        "1:100" | "100" => Some(DrawingScale::S1_100),
        "1:200" | "200" => Some(DrawingScale::S1_200),
        other => {
            let denom = other.strip_prefix("1:").unwrap_or(other);
            denom
                .replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|d| d.is_finite() && (1.0..=10_000.0).contains(d))
                .map(|denominator| DrawingScale::Custom { denominator })
        }
    }
}

pub(super) fn parse_elevation_side(raw: &str) -> Option<ElevationSide> {
    match raw.trim().to_lowercase().as_str() {
        "norte" | "north" => Some(ElevationSide::North),
        "sul" | "south" => Some(ElevationSide::South),
        "leste" | "east" => Some(ElevationSide::East),
        "oeste" | "west" => Some(ElevationSide::West),
        _ => None,
    }
}

pub(in crate::app::mcp_tools) fn parse_railing_side(raw: &str) -> Option<RailingSide> {
    match raw.trim().to_lowercase().as_str() {
        "nenhum" | "none" | "sem" => Some(RailingSide::None),
        "esquerda" | "left" => Some(RailingSide::Left),
        "direita" | "right" => Some(RailingSide::Right),
        "ambos" | "both" => Some(RailingSide::Both),
        _ => None,
    }
}

pub(in crate::app::mcp_tools) fn parse_stair_shape(raw: &str) -> Option<StairShape> {
    match raw.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
        "reta" | "lance_reto" | "straight" => Some(StairShape::Straight),
        "l" | "em_l" | "l_shaped" => Some(StairShape::LShaped),
        "u" | "em_u" | "u_shaped" => Some(StairShape::UShaped),
        _ => None,
    }
}
