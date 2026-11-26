//! AutoCAD DXF import and export (#38).
//!
//! Exports a Trazo [`Project`] as standard ASCII DXF (`AC1009` / R12 with
//! `$INSUNITS = 6` metres and `$MEASUREMENT = 1` metric) so plans open cleanly
//! in AutoCAD, QCAD, LibreCAD and other CAD tools with layers, colours,
//! linetypes, walls, openings, fixtures, drawings, associative dimensions,
//! texts and hatches intact.
//!
//! Imports ASCII DXF files (R12 through modern AutoCAD with `LWPOLYLINE`,
//! `MTEXT`, `SPLINE`, `DIMENSION` and `$INSUNITS` scaling from mm, cm, m,
//! inches or feet), mapping wall layers to Trazo walls and preserving layers,
//! colours, drawings, texts and dimensions.

mod export;
mod import;
mod import_entities;
mod styles;
#[cfg(test)]
mod tests;
mod writer;

use std::{fs, path::Path};

use crate::domain::{geometry::Point, model::Project};

pub use export::to_dxf_string;
use export::{Extents, sanitize_layer_name};
pub use import::import_str;
use import::{DxfPair, ensure_layer_id, find_f64, find_str, is_terrain_layer, is_wall_layer};
use import_entities::{
    import_arc, import_circle, import_dimension, import_hatch, import_line, import_lwpolyline,
    import_polyline_points, import_spline, import_text,
};
pub use styles::{aci_to_rgb, rgb_to_aci};
use styles::{linetype_from_dxf, linetype_to_dxf};
use writer::{
    write_arc, write_blocks, write_circle, write_header, write_line, write_linear_dimension_block,
    write_linear_dimension_entity, write_pair, write_polyline, write_polyline_styled, write_tables,
    write_text,
};

/// Summary returned after importing a DXF file into a [`Project`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DxfImportReport {
    pub layers: usize,
    pub walls: usize,
    pub drawings: usize,
    pub notes: usize,
    pub dimensions: usize,
    pub terrain_imported: bool,
    pub skipped_entities: usize,
    pub units_label: &'static str,
}

impl DxfImportReport {
    pub fn total_imported(&self) -> usize {
        self.walls
            + self.drawings
            + self.notes
            + self.dimensions
            + usize::from(self.terrain_imported)
    }

    pub fn summary_pt(&self) -> String {
        let mut parts = Vec::new();
        if self.walls > 0 {
            parts.push(format!("{} paredes", self.walls));
        }
        if self.drawings > 0 {
            parts.push(format!("{} entidades 2D", self.drawings));
        }
        if self.notes > 0 {
            parts.push(format!("{} textos", self.notes));
        }
        if self.dimensions > 0 {
            parts.push(format!("{} cotas", self.dimensions));
        }
        if self.terrain_imported {
            parts.push("terreno".to_owned());
        }
        if parts.is_empty() {
            format!(
                "DXF lido sem entidades geométricas importáveis (unidades: {}).",
                self.units_label
            )
        } else {
            format!(
                "DXF importado: {} ({} camadas, unidades: {}).",
                parts.join(", "),
                self.layers,
                self.units_label
            )
        }
    }
}

/// Exports `project` to an ASCII DXF file at `path`.
pub fn export(project: &Project, path: &Path) -> Result<(), String> {
    let content = to_dxf_string(project);
    fs::write(path, content).map_err(|error| format!("Não foi possível gravar o DXF: {error}"))
}

/// Imports an ASCII DXF file from `path` into `project`.
pub fn import_file(project: &mut Project, path: &Path) -> Result<DxfImportReport, String> {
    let budget = crate::io::memory::MemoryBudget::for_file(
        path,
        crate::io::memory::DXF_FACTOR,
        crate::io::memory::MAX_DXF_FALLBACK_BYTES,
    )?;
    budget.check()?;
    let bytes =
        fs::read(path).map_err(|error| format!("Não foi possível ler o arquivo DXF: {error}"))?;
    budget.check()?;
    let content = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) => err
            .into_bytes()
            .into_iter()
            .map(|b| b as char)
            .collect::<String>(),
    };
    budget.check()?;
    import_str(project, &content)
}

fn to_dxf(p: Point) -> (f64, f64) {
    (p.x, -p.y)
}

fn from_dxf(x: f64, y: f64, scale: f64) -> Point {
    Point::new(x * scale, -y * scale)
}
