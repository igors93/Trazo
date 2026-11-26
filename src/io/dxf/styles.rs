//! Line types and AutoCAD Color Index (ACI) colours, both ways.

use crate::domain::layers::LineType;

pub(super) fn linetype_to_dxf(line_type: LineType) -> &'static str {
    match line_type {
        LineType::Solid => "CONTINUOUS",
        LineType::Dashed => "DASHED",
        LineType::DashDot => "DASHDOT",
        LineType::Dotted => "DOTTED",
    }
}

pub(super) fn linetype_from_dxf(name: &str) -> LineType {
    let upper = name.trim().to_ascii_uppercase();
    if upper.contains("DASHDOT") || upper.contains("CENTER") || upper.contains("PHANTOM") {
        LineType::DashDot
    } else if upper.contains("DOT") {
        LineType::Dotted
    } else if upper.contains("DASH") || upper.contains("HIDDEN") {
        LineType::Dashed
    } else {
        LineType::Solid
    }
}

/// Curated AutoCAD Color Index (ACI) table covering standard architectural colours.
pub(super) const ACI_PALETTE: &[(u8, [u8; 3])] = &[
    (1, [255, 0, 0]),       // Red
    (2, [255, 255, 0]),     // Yellow
    (3, [0, 255, 0]),       // Green
    (4, [0, 255, 255]),     // Cyan
    (5, [0, 0, 255]),       // Blue
    (6, [255, 0, 255]),     // Magenta
    (7, [35, 38, 46]),      // Default white/black
    (8, [128, 128, 128]),   // Dark grey
    (9, [192, 192, 192]),   // Light grey
    (10, [220, 60, 50]),    // Brick / red-orange
    (30, [215, 130, 50]),   // Orange / wood
    (40, [195, 160, 75]),   // Warm ochre
    (52, [160, 145, 90]),   // Sand / beige
    (82, [85, 150, 95]),    // Architectural green
    (132, [75, 155, 165]),  // Teal
    (150, [94, 139, 191]),  // Terrain blue
    (170, [65, 105, 180]),  // Cobalt blue
    (210, [145, 95, 175]),  // Purple
    (251, [90, 96, 108]),   // Charcoal
    (253, [165, 174, 188]), // Silver
    (254, [210, 215, 222]), // Pale grey
];

/// Converts an RGB colour to the nearest AutoCAD Color Index (1..=255).
pub fn rgb_to_aci(rgb: [u8; 3]) -> u8 {
    let mut best_aci = 7u8;
    let mut best_dist = u32::MAX;
    for &(aci, [pr, pg, pb]) in ACI_PALETTE {
        let dr = i32::from(rgb[0]) - i32::from(pr);
        let dg = i32::from(rgb[1]) - i32::from(pg);
        let db = i32::from(rgb[2]) - i32::from(pb);
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best_aci = aci;
        }
    }
    best_aci
}

/// Converts an AutoCAD Color Index (positive or negative) to RGB.
pub fn aci_to_rgb(aci: i32) -> [u8; 3] {
    let abs = aci.unsigned_abs().min(255) as u8;
    if let Some(&(_, rgb)) = ACI_PALETTE.iter().find(|&&(code, _)| code == abs) {
        return rgb;
    }
    match abs {
        0 | 7 => [35, 38, 46],
        10..=29 => [210, 75, 65],
        30..=49 => [205, 140, 60],
        50..=79 => [150, 165, 65],
        80..=119 => [75, 155, 95],
        120..=149 => [70, 155, 170],
        150..=189 => [80, 120, 185],
        190..=229 => [145, 95, 175],
        230..=249 => [185, 85, 130],
        250..=255 => {
            let v = 60u8.saturating_add((abs - 250).saturating_mul(36));
            [v, v, v]
        }
        _ => [90, 96, 108],
    }
}
