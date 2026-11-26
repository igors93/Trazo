//! Importing each DXF entity type: lines, polylines (walls or drawings),
//! circles, arcs, splines, texts, dimensions and hatches.

use std::{collections::HashMap, f64::consts::TAU};

use crate::domain::{
    dimensions::{DimensionAnchor, DimensionKind},
    drawing::Shape,
    geometry::Point,
    model::Project,
    notes::NoteKind,
};

use super::{
    DxfImportReport, DxfPair, ensure_layer_id, find_f64, find_str, from_dxf, is_terrain_layer,
    is_wall_layer,
};

pub(super) fn import_line(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
        find_f64(slice, 10),
        find_f64(slice, 20),
        find_f64(slice, 11),
        find_f64(slice, 21),
    ) else {
        return;
    };
    let a = from_dxf(x1, y1, scale);
    let b = from_dxf(x2, y2, scale);
    if a.distance(b) < 0.01 {
        return;
    }
    let layer_name = find_str(slice, 8).unwrap_or("0");
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);

    if is_wall_layer(layer_name)
        && let Some(wall_id) = project.add_wall(a, b)
    {
        if let Some(wall) = project.walls.iter_mut().find(|w| w.id == wall_id) {
            wall.layer_id = layer_id;
        }
        report.walls += 1;
        return;
    }

    if let Ok(id) = project.add_drawing(Shape::Line { start: a, end: b }) {
        if let Some(d) = project.drawings.iter_mut().find(|d| d.id == id) {
            d.layer_id = layer_id;
        }
        report.drawings += 1;
    }
}

pub(super) fn import_lwpolyline(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let layer_name = find_str(slice, 8).unwrap_or("0");
    let closed = slice
        .iter()
        .find(|p| p.code == 70)
        .and_then(|p| p.value.trim().parse::<i32>().ok())
        .is_some_and(|f| (f & 1) != 0);

    let mut pts = Vec::new();
    let mut pending_x: Option<f64> = None;
    for pair in slice {
        match pair.code {
            10 => {
                pending_x = pair
                    .value
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite());
            }
            20 => {
                if let (Some(x), Some(y)) = (
                    pending_x.take(),
                    pair.value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite()),
                ) {
                    pts.push(from_dxf(x, y, scale));
                }
            }
            _ => {}
        }
    }
    import_polyline_points(project, layer_name, pts, closed, layer_ids, report);
}

/// Checks whether a closed 4-vertex polygon is a thin wall rectangle and,
/// if so, returns its centreline endpoints `(start, end)`.
pub(super) fn extract_wall_centerline(pts: &[Point]) -> Option<(Point, Point)> {
    if pts.len() != 4 {
        return None;
    }
    let d01 = pts[0].distance(pts[1]);
    let d12 = pts[1].distance(pts[2]);
    let d23 = pts[2].distance(pts[3]);
    let d30 = pts[3].distance(pts[0]);

    // Opposite sides must match within tolerance.
    if (d01 - d23).abs() > 0.03 || (d12 - d30).abs() > 0.03 {
        return None;
    }

    let (short, long, start, end) = if d01 >= d12 {
        (d12, d01, pts[0].lerp(pts[3], 0.5), pts[1].lerp(pts[2], 0.5))
    } else {
        (d01, d12, pts[0].lerp(pts[1], 0.5), pts[3].lerp(pts[2], 0.5))
    };

    if (0.05..=0.60).contains(&short) && long >= 0.24 && long / short >= 1.4 {
        Some((start, end))
    } else {
        None
    }
}

pub(super) fn import_polyline_points(
    project: &mut Project,
    layer_name: &str,
    mut pts: Vec<Point>,
    closed: bool,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    // Drop trailing duplicate vertex when explicitly closed.
    if closed
        && pts.len() > 2
        && let (Some(&first), Some(&last)) = (pts.first(), pts.last())
        && first.near(last, 1e-4)
    {
        pts.pop();
    }
    if pts.len() < 2 {
        return;
    }
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);

    if is_wall_layer(layer_name) {
        if closed
            && let Some((start, end)) = extract_wall_centerline(&pts)
            && let Some(wall_id) = project.add_wall(start, end)
        {
            if let Some(wall) = project.walls.iter_mut().find(|w| w.id == wall_id) {
                wall.layer_id = layer_id;
            }
            report.walls += 1;
            return;
        }
        // Open or multi-segment polyline on wall layer -> import each segment as a wall.
        let seg_count = if closed { pts.len() } else { pts.len() - 1 };
        let mut added = 0;
        for idx in 0..seg_count {
            let a = pts[idx];
            let b = pts[(idx + 1) % pts.len()];
            if let Some(wall_id) = project.add_wall(a, b) {
                if let Some(wall) = project.walls.iter_mut().find(|w| w.id == wall_id) {
                    wall.layer_id = layer_id;
                }
                added += 1;
            }
        }
        if added > 0 {
            report.walls += added;
            return;
        }
    }

    if closed && pts.len() >= 3 && is_terrain_layer(layer_name) && project.terrain.is_none() {
        let min_x = pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let min_y = pts.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
        let max_x = pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        let max_y = pts.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
        if project
            .set_terrain(Point::new(min_x, min_y), Point::new(max_x, max_y))
            .is_some()
        {
            if let Some(terrain) = &mut project.terrain {
                terrain.vertices = pts;
            }
            report.terrain_imported = true;
            return;
        }
    }

    if let Ok(id) = project.add_drawing(Shape::Polyline {
        points: pts,
        closed,
    }) {
        if let Some(d) = project.drawings.iter_mut().find(|d| d.id == id) {
            d.layer_id = layer_id;
        }
        report.drawings += 1;
    }
}

pub(super) fn import_circle(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let (Some(cx), Some(cy), Some(r)) = (
        find_f64(slice, 10),
        find_f64(slice, 20),
        find_f64(slice, 40),
    ) else {
        return;
    };
    let radius = r * scale;
    if radius < 0.01 {
        return;
    }
    let layer_name = find_str(slice, 8).unwrap_or("0");
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);
    if let Ok(id) = project.add_drawing(Shape::Circle {
        center: from_dxf(cx, cy, scale),
        radius,
    }) {
        if let Some(d) = project.drawings.iter_mut().find(|d| d.id == id) {
            d.layer_id = layer_id;
        }
        report.drawings += 1;
    }
}

pub(super) fn import_arc(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let (Some(cx), Some(cy), Some(r), Some(a0_deg), Some(a1_deg)) = (
        find_f64(slice, 10),
        find_f64(slice, 20),
        find_f64(slice, 40),
        find_f64(slice, 50),
        find_f64(slice, 51),
    ) else {
        return;
    };
    let radius = r * scale;
    if radius < 0.01 {
        return;
    }
    let dxf_sweep_deg = (a1_deg - a0_deg).rem_euclid(360.0);
    if dxf_sweep_deg < 1e-3 {
        return;
    }
    // Inverse of Y-flip: DXF CCW from a0 to a1 corresponds to Trazo start=-a1, positive sweep.
    let start = (-a1_deg.to_radians()).rem_euclid(TAU);
    let sweep = dxf_sweep_deg.to_radians();

    let layer_name = find_str(slice, 8).unwrap_or("0");
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);
    if let Ok(id) = project.add_drawing(Shape::Arc {
        center: from_dxf(cx, cy, scale),
        radius,
        start,
        sweep,
    }) {
        if let Some(d) = project.drawings.iter_mut().find(|d| d.id == id) {
            d.layer_id = layer_id;
        }
        report.drawings += 1;
    }
}

pub(super) fn import_spline(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let closed = slice
        .iter()
        .find(|p| p.code == 70)
        .and_then(|p| p.value.trim().parse::<i32>().ok())
        .is_some_and(|f| (f & 1) != 0);

    let mut pts = Vec::new();
    let mut pending_x: Option<f64> = None;
    for pair in slice {
        match pair.code {
            10 | 11 => {
                pending_x = pair
                    .value
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite());
            }
            20 | 21 => {
                if let (Some(x), Some(y)) = (
                    pending_x.take(),
                    pair.value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite()),
                ) {
                    pts.push(from_dxf(x, y, scale));
                }
            }
            _ => {}
        }
    }
    if pts.len() < 3 {
        return;
    }
    let layer_name = find_str(slice, 8).unwrap_or("0");
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);
    if let Ok(id) = project.add_drawing(Shape::Spline {
        points: pts,
        closed,
    }) {
        if let Some(d) = project.drawings.iter_mut().find(|d| d.id == id) {
            d.layer_id = layer_id;
        }
        report.drawings += 1;
    }
}

pub(super) fn clean_mtext(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                'P' | 'p' => {
                    out.push(' ');
                    i += 2;
                    continue;
                }
                '~' => {
                    out.push(' ');
                    i += 2;
                    continue;
                }
                'f' | 'F' | 'C' | 'H' | 'W' | 'T' | 'Q' | 'A' => {
                    i += 2;
                    while i < chars.len() && chars[i] != ';' {
                        i += 1;
                    }
                    if i < chars.len() {
                        i += 1;
                    }
                    continue;
                }
                'L' | 'l' | 'O' | 'o' => {
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        if chars[i] == '{' || chars[i] == '}' {
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out.trim().to_owned()
}

pub(super) fn import_text(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    is_mtext: bool,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let (Some(x), Some(y)) = (find_f64(slice, 10), find_f64(slice, 20)) else {
        return;
    };
    let mut text = String::new();
    for pair in slice {
        if pair.code == 3 || pair.code == 1 {
            text.push_str(pair.value);
        }
    }
    let cleaned = if is_mtext {
        clean_mtext(&text)
    } else {
        text.trim().to_owned()
    };
    if cleaned.is_empty() {
        return;
    }
    let layer_name = find_str(slice, 8).unwrap_or("0");
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);
    let pos = from_dxf(x, y, scale);
    let rotation_deg = find_f64(slice, 50).map(|deg| -deg).unwrap_or(0.0);
    let height = find_f64(slice, 40).map(|h| h * scale).unwrap_or(0.16);

    if let Ok(id) = project.add_note(pos, &cleaned, NoteKind::Text) {
        if let Some(note) = project.notes.iter_mut().find(|n| n.id == id) {
            note.layer_id = layer_id;
            note.rotation_deg = rotation_deg;
            note.font_size = (height / 0.014).clamp(8.0, 32.0);
        }
        report.notes += 1;
    }
}

pub(super) fn import_dimension(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    report: &mut DxfImportReport,
) {
    let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
        find_f64(slice, 13),
        find_f64(slice, 23),
        find_f64(slice, 14),
        find_f64(slice, 24),
    ) else {
        return;
    };
    let a = from_dxf(x1, y1, scale);
    let b = from_dxf(x2, y2, scale);
    if a.distance(b) < 0.02 {
        return;
    }

    // Estimate signed perpendicular offset from definition line (10, 20).
    let offset = if let (Some(lx), Some(ly)) = (find_f64(slice, 10), find_f64(slice, 20)) {
        let line_pt = from_dxf(lx, ly, scale);
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = dx.hypot(dy).max(1e-6);
        let nx = -dy / len;
        let ny = dx / len;
        let proj = (line_pt.x - a.x) * nx + (line_pt.y - a.y) * ny;
        if proj.abs() < 0.05 { 0.40 } else { proj }
    } else {
        0.40
    };

    let kind = DimensionKind::Aligned {
        a: DimensionAnchor::from_point(a),
        b: DimensionAnchor::from_point(b),
        offset,
    };
    if let Ok(id) = project.add_dimension(kind) {
        if let Some(text) = find_str(slice, 1)
            && !text.is_empty()
            && text != "<>"
            && let Some(dim) = project.dimensions.iter_mut().find(|d| d.id == id)
        {
            dim.override_text = Some(text.to_owned());
        }
        report.dimensions += 1;
    }
}

pub(super) fn import_hatch(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let layer_name = find_str(slice, 8).unwrap_or("Hachuras");
    let mut pts = Vec::new();
    let mut pending_x: Option<f64> = None;
    let mut in_boundary = false;
    for pair in slice {
        if pair.code == 92 {
            in_boundary = true;
            continue;
        }
        if !in_boundary {
            continue;
        }
        match pair.code {
            10 => {
                pending_x = pair
                    .value
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite());
            }
            20 => {
                if let (Some(x), Some(y)) = (
                    pending_x.take(),
                    pair.value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite()),
                ) {
                    pts.push(from_dxf(x, y, scale));
                }
            }
            _ => {}
        }
    }
    if pts.len() >= 3 {
        import_polyline_points(project, layer_name, pts, true, layer_ids, report);
    }
}
