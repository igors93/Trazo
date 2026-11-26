//! Low-level DXF output: group codes, the HEADER, TABLES and BLOCKS
//! sections, and one writer per entity type.

use std::{
    collections::HashMap,
    f64::consts::{FRAC_PI_2, TAU},
    fmt::Write as _,
};

use crate::domain::{geometry::Point, layers::Layer};

use super::{Extents, linetype_to_dxf, rgb_to_aci, sanitize_layer_name, to_dxf};

pub(super) fn write_pair(out: &mut String, code: i32, value: &str) {
    let _ = writeln!(out, "{code:3}\n{value}");
}

pub(super) fn write_f64(out: &mut String, code: i32, value: f64) {
    let cleaned = if value.abs() < 1e-10 { 0.0 } else { value };
    let _ = writeln!(out, "{code:3}\n{cleaned:.6}");
}

pub(super) fn write_i32(out: &mut String, code: i32, value: i32) {
    let _ = writeln!(out, "{code:3}\n{value}");
}

pub(super) fn write_header(out: &mut String, bounds: Extents) {
    write_pair(out, 0, "SECTION");
    write_pair(out, 2, "HEADER");

    write_pair(out, 9, "$ACADVER");
    write_pair(out, 1, "AC1009");

    // 6 = Metres, 1 = Metric measurement system, 2 = Decimal units.
    write_pair(out, 9, "$INSUNITS");
    write_i32(out, 70, 6);
    write_pair(out, 9, "$MEASUREMENT");
    write_i32(out, 70, 1);
    write_pair(out, 9, "$LUNITS");
    write_i32(out, 70, 2);
    write_pair(out, 9, "$LUPREC");
    write_i32(out, 70, 4);

    let min_x = bounds.min_x.unwrap_or(0.0);
    let min_y = bounds.min_y.unwrap_or(0.0);
    let max_x = bounds.max_x.unwrap_or(10.0);
    let max_y = bounds.max_y.unwrap_or(10.0);

    write_pair(out, 9, "$EXTMIN");
    write_f64(out, 10, min_x);
    write_f64(out, 20, min_y);
    write_f64(out, 30, 0.0);

    write_pair(out, 9, "$EXTMAX");
    write_f64(out, 10, max_x);
    write_f64(out, 20, max_y);
    write_f64(out, 30, 0.0);

    write_pair(out, 0, "ENDSEC");
}

pub(super) fn write_tables(out: &mut String, layers: &[Layer], include_hatches_layer: bool) {
    write_pair(out, 0, "SECTION");
    write_pair(out, 2, "TABLES");

    // LTYPE table.
    write_pair(out, 0, "TABLE");
    write_pair(out, 2, "LTYPE");
    write_i32(out, 70, 4);
    write_ltype(out, "CONTINUOUS", "Solid line", &[]);
    write_ltype(out, "DASHED", "Dashed __ __ __", &[0.25, -0.15]);
    write_ltype(
        out,
        "DASHDOT",
        "Dash dot __ . __ .",
        &[0.35, -0.10, 0.0, -0.10],
    );
    write_ltype(out, "DOTTED", "Dotted . . . .", &[0.0, -0.12]);
    write_pair(out, 0, "ENDTAB");

    // LAYER table.
    let mut seen = HashMap::<String, ()>::new();
    let mut layer_entries: Vec<(String, i32, &str, i32)> = Vec::new();
    for layer in layers {
        let name = sanitize_layer_name(&layer.name);
        if seen.insert(name.to_uppercase(), ()).is_some() {
            continue;
        }
        let aci = i32::from(rgb_to_aci(layer.color));
        let signed_color = if layer.visible { aci } else { -aci };
        let flags = if layer.locked { 4 } else { 0 };
        layer_entries.push((name, signed_color, linetype_to_dxf(layer.line_type), flags));
    }
    if include_hatches_layer && seen.insert("HACHURAS".to_owned(), ()).is_none() {
        layer_entries.push(("Hachuras".to_owned(), 9, "CONTINUOUS", 0));
    }

    write_pair(out, 0, "TABLE");
    write_pair(out, 2, "LAYER");
    write_i32(out, 70, layer_entries.len() as i32);
    for (name, color, ltype, flags) in layer_entries {
        write_pair(out, 0, "LAYER");
        write_pair(out, 2, &name);
        write_i32(out, 70, flags);
        write_i32(out, 62, color);
        write_pair(out, 6, ltype);
    }
    write_pair(out, 0, "ENDTAB");

    // STYLE table.
    write_pair(out, 0, "TABLE");
    write_pair(out, 2, "STYLE");
    write_i32(out, 70, 1);
    write_pair(out, 0, "STYLE");
    write_pair(out, 2, "STANDARD");
    write_i32(out, 70, 0);
    write_f64(out, 40, 0.0);
    write_f64(out, 41, 1.0);
    write_f64(out, 50, 0.0);
    write_i32(out, 71, 0);
    write_f64(out, 42, 0.20);
    write_pair(out, 3, "txt");
    write_pair(out, 4, "");
    write_pair(out, 0, "ENDTAB");

    write_pair(out, 0, "ENDSEC");
}

pub(super) fn write_ltype(out: &mut String, name: &str, desc: &str, pattern: &[f64]) {
    write_pair(out, 0, "LTYPE");
    write_pair(out, 2, name);
    write_i32(out, 70, 0);
    write_pair(out, 3, desc);
    write_i32(out, 72, 65);
    write_i32(out, 73, pattern.len() as i32);
    let total_len: f64 = pattern.iter().map(|v| v.abs()).sum();
    write_f64(out, 40, total_len);
    for &seg in pattern {
        write_f64(out, 49, seg);
    }
}

pub(super) fn write_blocks(out: &mut String, blocks_buf: &str) {
    write_pair(out, 0, "SECTION");
    write_pair(out, 2, "BLOCKS");
    out.push_str(blocks_buf);
    write_pair(out, 0, "ENDSEC");
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_linear_dimension_block(
    out: &mut String,
    block_name: &str,
    layer: &str,
    p1: Point,
    p2: Point,
    dim_p1: Point,
    dim_p2: Point,
    label: &str,
) {
    write_pair(out, 0, "BLOCK");
    write_pair(out, 8, layer);
    write_pair(out, 2, block_name);
    write_i32(out, 70, 1);
    write_f64(out, 10, 0.0);
    write_f64(out, 20, 0.0);
    write_f64(out, 30, 0.0);

    // Extension lines + dimension line.
    write_line(out, layer, p1, dim_p1, None);
    write_line(out, layer, p2, dim_p2, None);
    write_line(out, layer, dim_p1, dim_p2, None);

    // Architectural 45° tick marks at both ends.
    let dx = dim_p2.x - dim_p1.x;
    let dy = dim_p2.y - dim_p1.y;
    let angle = dy.atan2(dx);
    let tick_angle = angle + FRAC_PI_2 * 0.5;
    let tx = 0.06 * tick_angle.cos();
    let ty = 0.06 * tick_angle.sin();
    for end in [dim_p1, dim_p2] {
        write_line(
            out,
            layer,
            Point::new(end.x - tx, end.y - ty),
            Point::new(end.x + tx, end.y + ty),
            None,
        );
    }

    let mid = dim_p1.lerp(dim_p2, 0.5);
    let rot_deg = (-angle).to_degrees();
    write_text(out, layer, mid, 0.14, rot_deg, label, None);

    write_pair(out, 0, "ENDBLK");
    write_pair(out, 8, layer);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_linear_dimension_entity(
    out: &mut String,
    layer: &str,
    block_name: &str,
    p1: Point,
    p2: Point,
    dim_p1: Point,
    dim_p2: Point,
    label: &str,
) {
    let (d1x, d1y) = to_dxf(p1);
    let (d2x, d2y) = to_dxf(p2);
    let (lx, ly) = to_dxf(dim_p2);
    let (mx, my) = to_dxf(dim_p1.lerp(dim_p2, 0.5));

    write_pair(out, 0, "DIMENSION");
    write_pair(out, 8, layer);
    write_pair(out, 2, block_name);
    write_f64(out, 10, lx);
    write_f64(out, 20, ly);
    write_f64(out, 30, 0.0);
    write_f64(out, 11, mx);
    write_f64(out, 21, my);
    write_f64(out, 31, 0.0);
    // 1 = aligned linear dimension.
    write_i32(out, 70, 1);
    write_pair(out, 1, label);
    write_f64(out, 13, d1x);
    write_f64(out, 23, d1y);
    write_f64(out, 33, 0.0);
    write_f64(out, 14, d2x);
    write_f64(out, 24, d2y);
    write_f64(out, 34, 0.0);
}

pub(super) fn write_line(out: &mut String, layer: &str, a: Point, b: Point, aci: Option<u8>) {
    let (ax, ay) = to_dxf(a);
    let (bx, by) = to_dxf(b);
    write_pair(out, 0, "LINE");
    write_pair(out, 8, layer);
    if let Some(color) = aci {
        write_i32(out, 62, i32::from(color));
    }
    write_f64(out, 10, ax);
    write_f64(out, 20, ay);
    write_f64(out, 30, 0.0);
    write_f64(out, 11, bx);
    write_f64(out, 21, by);
    write_f64(out, 31, 0.0);
}

pub(super) fn write_polyline(
    out: &mut String,
    layer: &str,
    pts: &[Point],
    closed: bool,
    aci: Option<u8>,
) {
    write_polyline_styled(out, layer, pts, closed, aci, None);
}

pub(super) fn write_polyline_styled(
    out: &mut String,
    layer: &str,
    pts: &[Point],
    closed: bool,
    aci: Option<u8>,
    ltype: Option<&str>,
) {
    if pts.len() < 2 {
        return;
    }
    write_pair(out, 0, "POLYLINE");
    write_pair(out, 8, layer);
    if let Some(lt) = ltype {
        write_pair(out, 6, lt);
    }
    if let Some(color) = aci {
        write_i32(out, 62, i32::from(color));
    }
    write_i32(out, 66, 1);
    write_f64(out, 10, 0.0);
    write_f64(out, 20, 0.0);
    write_f64(out, 30, 0.0);
    write_i32(out, 70, if closed { 1 } else { 0 });

    for &pt in pts {
        let (x, y) = to_dxf(pt);
        write_pair(out, 0, "VERTEX");
        write_pair(out, 8, layer);
        write_f64(out, 10, x);
        write_f64(out, 20, y);
        write_f64(out, 30, 0.0);
    }
    write_pair(out, 0, "SEQEND");
    write_pair(out, 8, layer);
}

pub(super) fn write_circle(
    out: &mut String,
    layer: &str,
    center: Point,
    radius: f64,
    aci: Option<u8>,
) {
    let (cx, cy) = to_dxf(center);
    write_pair(out, 0, "CIRCLE");
    write_pair(out, 8, layer);
    if let Some(color) = aci {
        write_i32(out, 62, i32::from(color));
    }
    write_f64(out, 10, cx);
    write_f64(out, 20, cy);
    write_f64(out, 30, 0.0);
    write_f64(out, 40, radius);
}

pub(super) fn write_arc(
    out: &mut String,
    layer: &str,
    center: Point,
    radius: f64,
    start_rad: f64,
    sweep_rad: f64,
    aci: Option<u8>,
) {
    let (cx, cy) = to_dxf(center);
    // Flipping Y reverses orientation: Trazo positive sweep (CW in Y-up) becomes
    // negative CCW in DXF, so swap start and end when sweep > 0.
    let end_rad = start_rad + sweep_rad;
    let (dxf_start, dxf_end) = if sweep_rad >= 0.0 {
        ((-end_rad).rem_euclid(TAU), (-start_rad).rem_euclid(TAU))
    } else {
        ((-start_rad).rem_euclid(TAU), (-end_rad).rem_euclid(TAU))
    };
    write_pair(out, 0, "ARC");
    write_pair(out, 8, layer);
    if let Some(color) = aci {
        write_i32(out, 62, i32::from(color));
    }
    write_f64(out, 10, cx);
    write_f64(out, 20, cy);
    write_f64(out, 30, 0.0);
    write_f64(out, 40, radius);
    write_f64(out, 50, dxf_start.to_degrees());
    write_f64(out, 51, dxf_end.to_degrees());
}

pub(super) fn write_text(
    out: &mut String,
    layer: &str,
    position: Point,
    height: f64,
    rotation_deg: f64,
    text: &str,
    aci: Option<u8>,
) {
    let cleaned = text.replace(['\r', '\n'], " ");
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return;
    }
    let (x, y) = to_dxf(position);
    write_pair(out, 0, "TEXT");
    write_pair(out, 8, layer);
    if let Some(color) = aci {
        write_i32(out, 62, i32::from(color));
    }
    write_f64(out, 10, x);
    write_f64(out, 20, y);
    write_f64(out, 30, 0.0);
    write_f64(out, 40, height);
    write_pair(out, 1, trimmed);
    if rotation_deg.abs() > 1e-4 {
        write_f64(out, 50, rotation_deg.rem_euclid(360.0));
    }
    write_pair(out, 7, "STANDARD");
}
