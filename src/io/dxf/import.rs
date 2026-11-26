//! Reading a DXF file: tokenizing group codes, the drawing units, and
//! mapping layers and entities into the project.

use std::collections::HashMap;

use crate::domain::{layers::LineType, model::Project};

use super::{
    DxfImportReport, aci_to_rgb, from_dxf, import_arc, import_circle, import_dimension,
    import_hatch, import_line, import_lwpolyline, import_polyline_points, import_spline,
    import_text, linetype_from_dxf, sanitize_layer_name,
};

pub(super) struct DxfPair<'a> {
    pub(super) code: i32,
    pub(super) value: &'a str,
}

pub(super) fn tokenize_dxf(content: &str) -> Result<Vec<DxfPair<'_>>, String> {
    let mut lines = content.lines();
    let mut pairs = Vec::new();
    while let Some(code_line) = lines.next() {
        let trimmed_code = code_line.trim();
        if trimmed_code.is_empty() {
            continue;
        }
        let code = trimmed_code
            .parse::<i32>()
            .map_err(|_| format!("Código DXF inválido: '{trimmed_code}'"))?;
        let value = lines.next().unwrap_or("").trim_end_matches('\r');
        pairs.push(DxfPair { code, value });
    }
    if pairs.is_empty() {
        return Err("Arquivo DXF vazio.".to_owned());
    }
    Ok(pairs)
}

pub(super) fn insunits_scale(code: i32) -> (f64, &'static str) {
    match code {
        1 => (0.0254, "polegadas"),
        2 => (0.3048, "pés"),
        4 => (0.001, "milímetros"),
        5 => (0.01, "centímetros"),
        6 => (1.0, "metros"),
        _ => (1.0, "metros"),
    }
}

pub(super) fn is_wall_layer(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    lower.contains("parede")
        || lower.contains("wall")
        || lower.contains("alvenaria")
        || lower.contains("muro")
}

pub(super) fn is_terrain_layer(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    lower.contains("terreno") || lower == "lote" || lower.contains("site") || lower.contains("lot")
}

/// Parses ASCII DXF `content` and merges its layers and entities into `project`.
pub fn import_str(project: &mut Project, content: &str) -> Result<DxfImportReport, String> {
    let pairs = tokenize_dxf(content)?;
    let mut report = DxfImportReport {
        units_label: "metros",
        ..Default::default()
    };

    let mut scale = 1.0_f64;
    let mut layer_ids = HashMap::<String, u64>::new();
    for layer in &project.layers {
        layer_ids.insert(sanitize_layer_name(&layer.name).to_uppercase(), layer.id);
        layer_ids.insert(layer.name.trim().to_uppercase(), layer.id);
    }

    let mut i = 0usize;
    let mut section = String::new();

    while i < pairs.len() {
        let pair = &pairs[i];
        if pair.code == 0 && pair.value.trim().eq_ignore_ascii_case("SECTION") {
            if i + 1 < pairs.len() && pairs[i + 1].code == 2 {
                section = pairs[i + 1].value.trim().to_ascii_uppercase();
                i += 2;
                continue;
            }
        } else if pair.code == 0 && pair.value.trim().eq_ignore_ascii_case("ENDSEC") {
            section.clear();
            i += 1;
            continue;
        }

        match section.as_str() {
            "HEADER" => {
                if pair.code == 9 && pair.value.trim().eq_ignore_ascii_case("$INSUNITS") {
                    if i + 1 < pairs.len()
                        && pairs[i + 1].code == 70
                        && let Ok(code) = pairs[i + 1].value.trim().parse::<i32>()
                    {
                        let (s, label) = insunits_scale(code);
                        scale = s;
                        report.units_label = label;
                    }
                    i += 2;
                    continue;
                }
                i += 1;
            }
            "TABLES" => {
                if pair.code == 0 && pair.value.trim().eq_ignore_ascii_case("LAYER") {
                    i += 1;
                    let mut name = String::new();
                    let mut color_aci = 7i32;
                    let mut flags = 0i32;
                    let mut ltype = LineType::Solid;
                    while i < pairs.len() && pairs[i].code != 0 {
                        match pairs[i].code {
                            2 => name = pairs[i].value.trim().to_owned(),
                            62 => {
                                if let Ok(c) = pairs[i].value.trim().parse::<i32>() {
                                    color_aci = c;
                                }
                            }
                            70 => {
                                if let Ok(f) = pairs[i].value.trim().parse::<i32>() {
                                    flags = f;
                                }
                            }
                            6 => ltype = linetype_from_dxf(pairs[i].value),
                            _ => {}
                        }
                        i += 1;
                    }
                    if !name.is_empty() && name != "0" {
                        let visible = color_aci >= 0;
                        let locked = (flags & 4) != 0;
                        let rgb = aci_to_rgb(color_aci);
                        let key = name.to_uppercase();
                        if let Some(&existing_id) = layer_ids.get(&key) {
                            if let Some(layer) =
                                project.layers.iter_mut().find(|l| l.id == existing_id)
                            {
                                layer.color = rgb;
                                layer.line_type = ltype;
                                layer.visible = visible;
                                layer.locked = locked;
                            }
                        } else if let Ok(new_id) = project.add_layer(&name, rgb, 0.25, ltype) {
                            if let Some(layer) = project.layers.iter_mut().find(|l| l.id == new_id)
                            {
                                layer.visible = visible;
                                layer.locked = locked;
                            }
                            layer_ids.insert(key, new_id);
                            layer_ids.insert(sanitize_layer_name(&name).to_uppercase(), new_id);
                            report.layers += 1;
                        }
                    }
                    continue;
                }
                i += 1;
            }
            "ENTITIES" => {
                if pair.code == 0 {
                    let entity_type = pair.value.trim().to_ascii_uppercase();
                    i += 1;
                    let start_idx = i;
                    while i < pairs.len() && pairs[i].code != 0 {
                        i += 1;
                    }
                    let slice = &pairs[start_idx..i];

                    match entity_type.as_str() {
                        "LINE" => {
                            import_line(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "LWPOLYLINE" => {
                            import_lwpolyline(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "POLYLINE" => {
                            let closed = slice
                                .iter()
                                .find(|p| p.code == 70)
                                .and_then(|p| p.value.trim().parse::<i32>().ok())
                                .is_some_and(|f| (f & 1) != 0);
                            let layer_name = find_str(slice, 8).unwrap_or("0");
                            let mut pts = Vec::new();
                            while i < pairs.len() {
                                if pairs[i].code == 0
                                    && pairs[i].value.trim().eq_ignore_ascii_case("VERTEX")
                                {
                                    i += 1;
                                    let v_start = i;
                                    while i < pairs.len() && pairs[i].code != 0 {
                                        i += 1;
                                    }
                                    let v_slice = &pairs[v_start..i];
                                    if let (Some(x), Some(y)) =
                                        (find_f64(v_slice, 10), find_f64(v_slice, 20))
                                    {
                                        pts.push(from_dxf(x, y, scale));
                                    }
                                } else if pairs[i].code == 0
                                    && pairs[i].value.trim().eq_ignore_ascii_case("SEQEND")
                                {
                                    i += 1;
                                    while i < pairs.len() && pairs[i].code != 0 {
                                        i += 1;
                                    }
                                    break;
                                } else {
                                    break;
                                }
                            }
                            import_polyline_points(
                                project,
                                layer_name,
                                pts,
                                closed,
                                &mut layer_ids,
                                &mut report,
                            );
                        }
                        "CIRCLE" => {
                            import_circle(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "ARC" => {
                            import_arc(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "SPLINE" => {
                            import_spline(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "TEXT" | "MTEXT" => {
                            import_text(
                                project,
                                slice,
                                scale,
                                entity_type == "MTEXT",
                                &mut layer_ids,
                                &mut report,
                            );
                        }
                        "DIMENSION" => {
                            import_dimension(project, slice, scale, &mut report);
                        }
                        "HATCH" => {
                            import_hatch(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "ENDSEC" | "EOF" | "SEQEND" | "VERTEX" => {}
                        _ => {
                            report.skipped_entities += 1;
                        }
                    }
                    continue;
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    Ok(report)
}

pub(super) fn ensure_layer_id(
    project: &mut Project,
    layer_name: &str,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) -> Option<u64> {
    let trimmed = layer_name.trim();
    if trimmed.is_empty() || trimmed == "0" {
        return None;
    }
    let key = trimmed.to_uppercase();
    if let Some(&id) = layer_ids.get(&key) {
        return Some(id);
    }
    let clean_key = sanitize_layer_name(trimmed).to_uppercase();
    if let Some(&id) = layer_ids.get(&clean_key) {
        return Some(id);
    }
    if let Ok(id) = project.add_layer(trimmed, [90, 96, 108], 0.25, LineType::Solid) {
        layer_ids.insert(key, id);
        layer_ids.insert(clean_key, id);
        report.layers += 1;
        return Some(id);
    }
    None
}

pub(super) fn find_str<'a>(pairs: &'a [DxfPair<'a>], code: i32) -> Option<&'a str> {
    pairs
        .iter()
        .find(|p| p.code == code)
        .map(|p| p.value.trim())
}

pub(super) fn find_f64(pairs: &[DxfPair<'_>], code: i32) -> Option<f64> {
    pairs
        .iter()
        .find(|p| p.code == code)
        .and_then(|p| p.value.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}
