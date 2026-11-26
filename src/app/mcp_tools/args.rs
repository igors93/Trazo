//! Reading the assistant's arguments, and converting coordinates between
//! the assistant's view (Y up) and the model (Y down).

use serde_json::{Value, json};

use crate::domain::geometry::Point;

/// Largest coordinate or length accepted, in metres.
const LIMIT: f64 = 10_000.0;

pub(super) fn to_model(x: f64, y: f64) -> Point {
    Point::new(x, -y)
}

pub(super) fn from_model(point: Point) -> (f64, f64) {
    // Adding 0.0 turns -0.0 into 0.0.
    (point.x + 0.0, -point.y + 0.0)
}

pub(super) fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0 + 0.0
}

pub(super) fn xy(point: Point) -> Value {
    let (x, y) = from_model(point);
    json!({ "x": round(x), "y": round(y) })
}

pub(super) fn square_metres(value: f64) -> String {
    format!("{value:.2} m²").replace('.', ",")
}

/// An optional number argument; strings such as "3,5" are accepted too.
pub(super) fn number(args: &Value, key: &str) -> Result<Option<f64>, String> {
    let value = match args.get(key) {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => text.trim().replace(',', ".").parse::<f64>().ok(),
        Some(_) => None,
    };
    match value {
        Some(value) if value.is_finite() && value.abs() <= LIMIT => Ok(Some(value)),
        _ => Err(format!(
            "'{key}' deve ser um número entre -10000 e 10000 (metros)."
        )),
    }
}

/// A whole number from 0 up given as `key`, if any.
pub(in crate::app) fn optional_count(args: &Value, key: &str) -> Result<Option<u64>, String> {
    let wrong = || format!("'{key}' deve ser um número inteiro de 0 em diante.");
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .or_else(|| {
                number
                    .as_f64()
                    .filter(|value| *value >= 0.0 && value.fract() == 0.0)
                    .map(|value| value as u64)
            })
            .map(Some)
            .ok_or_else(wrong),
        Some(Value::String(text)) => text.trim().parse::<u64>().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

pub(super) fn required_number(args: &Value, key: &str) -> Result<f64, String> {
    number(args, key)?.ok_or_else(|| format!("Informe '{key}' (número, em metros)."))
}

pub(super) fn element_id(args: &Value, key: &str) -> Result<u64, String> {
    let id = match args.get(key) {
        Some(Value::Number(number)) => number.as_u64().or_else(|| {
            number
                .as_f64()
                .filter(|v| v.fract() == 0.0 && *v >= 0.0)
                .map(|v| v as u64)
        }),
        Some(Value::String(text)) => text.trim().parse::<u64>().ok(),
        _ => None,
    };
    id.ok_or_else(|| format!("Informe '{key}': o id numérico do elemento (veja obter_planta)."))
}

/// `rotacao` in degrees (a multiple of 90) as clockwise quarter turns.
pub(super) fn quarter_turns(args: &Value) -> Result<Option<u8>, String> {
    let Some(degrees) = number(args, "rotacao")? else {
        return Ok(None);
    };
    let turns = degrees / 90.0;
    if (turns - turns.round()).abs() > 0.01 {
        return Err("'rotacao' deve ser 0, 90, 180 ou 270 graus.".to_owned());
    }
    Ok(Some((turns.round() as i32).rem_euclid(4) as u8))
}

pub(super) fn text(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

pub(super) fn flag(args: &Value, key: &str) -> bool {
    match args.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(text)) => {
            matches!(text.trim().to_lowercase().as_str(), "true" | "sim" | "1")
        }
        _ => false,
    }
}
