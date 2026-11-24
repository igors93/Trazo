//! Headless CLI performance benchmark (`trazo --bench`, T-07).
//!
//! Generates a deterministic 3-storey, 500+ wall synthetic building and measures
//! core architectural, spatial, undo/redo, structural, quantities, parallel ZIP
//! I/O and vector SVG export timings so regressions are caught objectively.

use std::fs;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::domain::{
    discipline::Discipline,
    fixtures::FixtureKind,
    geometry::Point,
    history::History,
    model::{OpeningKind, Project},
    snap::{OsnapSettings, find_snap},
    spatial::SpatialIndex,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchReport {
    pub storeys: usize,
    pub active_floor_walls: usize,
    pub total_walls_all_floors: usize,
    pub rooms: usize,
    pub openings: usize,
    pub fixtures: usize,
    pub build_ms: f64,
    pub spatial_index_ms: f64,
    pub snap_and_pick_20_ms: f64,
    pub incremental_edit_ms: f64,
    pub undo_redo_ms: f64,
    pub structure_report_ms: f64,
    pub quantities_ms: f64,
    pub zip_save_ms: f64,
    pub zip_load_ms: f64,
    pub json_bytes: usize,
    pub zip_bytes: usize,
    pub compression_ratio: f64,
    pub svg_export_ms: f64,
}

/// Generates a deterministic 3-storey building with 500+ walls across storeys
/// (≥ 180 walls per storey, rooms, doors/windows, structural columns & beams).
pub fn build_stress_project() -> Project {
    let mut project = Project::default();
    project.name = "Edifício Benchmark 3 Pavimentos".to_owned();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(70.0, 36.0));

    // 8 × 11 grid: 195 walls per storey, 585 across three storeys.
    for row in 0..8usize {
        for col in 0..11usize {
            let x0 = col as f64 * 6.0;
            let y0 = row as f64 * 4.0;
            let w_horiz = project.add_wall(Point::new(x0, y0), Point::new(x0 + 6.0, y0));
            let _w_vert = project.add_wall(Point::new(x0, y0), Point::new(x0, y0 + 4.0));
            if let Some(wid) = w_horiz {
                if (row + col) % 2 == 0 {
                    project.add_opening(wid, OpeningKind::Door, 0.5);
                } else {
                    project.add_opening(wid, OpeningKind::Window, 0.5);
                }
            }
            if row % 2 == 0 && col % 2 == 0 {
                project.add_fixture(
                    FixtureKind::Column,
                    Point::new(x0 + 0.15, y0 + 0.15),
                    0.20,
                    0.20,
                    0,
                );
            }
        }
        // Close right-hand vertical walls and bottom horizontal walls
        let y0 = row as f64 * 4.0;
        let _ = project.add_wall(Point::new(66.0, y0), Point::new(66.0, y0 + 4.0));
    }
    for col in 0..11usize {
        let x0 = col as f64 * 6.0;
        let _ = project.add_wall(Point::new(x0, 32.0), Point::new(x0 + 6.0, 32.0));
    }
    // Add 8 interior rooms with names for area/quantities/structural slab checks.
    for col in 0..4usize {
        let x0 = col as f64 * 6.0;
        let _ = project.add_room(
            Point::new(x0, 0.0),
            Point::new(x0 + 6.0, 4.0),
            format!("Sala {}", col + 1),
        );
    }
    // Add 2 upper storeys copying the wall layout → 3 storeys, 500+ total walls.
    project.add_floor("2º Pavimento", true, true);
    project.add_floor("3º Pavimento", true, true);
    // Switch back to ground floor (index 0) where rooms/columns live.
    if let Some(ground) = project.floors.levels.first().map(|f| f.id) {
        let _ = project.switch_floor(ground);
    }
    project.generate_structure();
    project.ensure_all_guids();
    project
}

fn elapsed_ms(t0: Instant) -> f64 {
    t0.elapsed().as_secs_f64() * 1000.0
}

/// Runs the complete headless benchmark suite and returns structured timings.
pub fn run_bench() -> Result<BenchReport, String> {
    let t_build = Instant::now();
    let mut project = build_stress_project();
    let build_ms = elapsed_ms(t_build);

    let storeys = project.floors.levels.len().max(1);
    let active_floor_walls = project.walls.len();
    let inactive_walls: usize = project
        .floors
        .levels
        .iter()
        .filter(|f| Some(f.id) != project.floors.active_floor_id)
        .map(|f| f.data.walls.len())
        .sum();
    let total_walls_all_floors = active_floor_walls + inactive_walls;

    let t_spatial = Instant::now();
    let _index = SpatialIndex::build(&project);
    let spatial_index_ms = elapsed_ms(t_spatial);

    // Warm spatial index cache, then measure 20 OSNAP + pick queries.
    let _ = project.cached_spatial_index();
    let settings = OsnapSettings::default();
    let t_snap = Instant::now();
    for k in 0..20usize {
        let p = Point::new((k as f64) * 3.0 + 0.05, 4.02);
        let _ = find_snap(&project, &[], p, None, 0.25, settings);
        let _ = project.fixture_at_in_discipline(p, Discipline::All);
        let _ = project.room_at(p);
    }
    let snap_and_pick_20_ms = elapsed_ms(t_snap);

    // Incremental edit + O(N) diff + incremental validation.
    let before = project.clone();
    let t_edit = Instant::now();
    if let Some(wall) = project.walls.first_mut() {
        wall.end.x += 0.15;
    }
    let changes = project.changes_since(&before);
    let _validation = project.validate_incremental(&changes);
    let incremental_edit_ms = elapsed_ms(t_edit);

    // Structurally-shared History record + undo + redo.
    let mut history = History::new();
    history.record(before);
    let t_undo = Instant::now();
    history.undo(&mut project);
    history.redo(&mut project);
    let undo_redo_ms = elapsed_ms(t_undo);

    // Structural pre-sizing report.
    let t_struct = Instant::now();
    let _structure = project.structure_report();
    let structure_report_ms = elapsed_ms(t_struct);

    // Multi-floor quantities takeoff.
    let t_qty = Instant::now();
    let _takeoff = project.construction_takeoff_with_options(true, None);
    let quantities_ms = elapsed_ms(t_qty);

    // Parallel ZIP `.trazo` save & load + compression ratio.
    let json_bytes = serde_json::to_vec(&project)
        .map_err(|e| e.to_string())?
        .len();
    let tmp_path = std::env::temp_dir().join(format!("trazo-bench-{}.trazo", std::process::id()));
    let t_save = Instant::now();
    crate::io::project_file::save(&project, &tmp_path)?;
    let zip_save_ms = elapsed_ms(t_save);
    let zip_bytes = fs::metadata(&tmp_path)
        .map(|m| m.len() as usize)
        .unwrap_or(1);
    let t_load = Instant::now();
    let _loaded = crate::io::project_file::load(&tmp_path)?;
    let zip_load_ms = elapsed_ms(t_load);
    let _ = fs::remove_file(&tmp_path);

    // Vector SVG export.
    let svg_path = std::env::temp_dir().join(format!("trazo-bench-{}.svg", std::process::id()));
    let view = crate::io::export::ExportView::for_discipline(&project, Discipline::All);
    let t_svg = Instant::now();
    crate::io::export::svg_for_view(&project, &view, &svg_path)?;
    let svg_export_ms = elapsed_ms(t_svg);
    let _ = fs::remove_file(&svg_path);

    let compression_ratio = if zip_bytes > 0 {
        json_bytes as f64 / zip_bytes as f64
    } else {
        1.0
    };

    Ok(BenchReport {
        storeys,
        active_floor_walls,
        total_walls_all_floors,
        rooms: project.rooms.len(),
        openings: project.openings.len(),
        fixtures: project.fixtures.len(),
        build_ms,
        spatial_index_ms,
        snap_and_pick_20_ms,
        incremental_edit_ms,
        undo_redo_ms,
        structure_report_ms,
        quantities_ms,
        zip_save_ms,
        zip_load_ms,
        json_bytes,
        zip_bytes,
        compression_ratio,
        svg_export_ms,
    })
}

/// Entrypoint for `trazo --bench [--bench-out FILE]`.
pub fn run_cli(json_out: Option<&Path>) -> Result<(), String> {
    let report = run_bench()?;
    println!("Trazo Benchmark (--bench)");
    println!(
        "  Modelo: {} pavimentos · {} paredes totais ({} no ativo) · {} cômodos · {} aberturas · {} objetos",
        report.storeys,
        report.total_walls_all_floors,
        report.active_floor_walls,
        report.rooms,
        report.openings,
        report.fixtures
    );
    println!("  Construção do modelo : {:7.2} ms", report.build_ms);
    println!(
        "  Índice espacial XY   : {:7.2} ms",
        report.spatial_index_ms
    );
    println!(
        "  OSNAP + pick (20x)   : {:7.2} ms ({:.2} ms/consulta)",
        report.snap_and_pick_20_ms,
        report.snap_and_pick_20_ms / 20.0
    );
    println!(
        "  Edição incremental   : {:7.2} ms",
        report.incremental_edit_ms
    );
    println!("  Undo + Redo CoW      : {:7.2} ms", report.undo_redo_ms);
    println!(
        "  Análise estrutural   : {:7.2} ms",
        report.structure_report_ms
    );
    println!("  Quantitativos (3 pav): {:7.2} ms", report.quantities_ms);
    println!(
        "  ZIP .trazo salvar    : {:7.2} ms ({} KB → {} KB, {:.1}× menor)",
        report.zip_save_ms,
        report.json_bytes / 1024,
        report.zip_bytes / 1024,
        report.compression_ratio
    );
    println!("  ZIP .trazo abrir     : {:7.2} ms", report.zip_load_ms);
    println!("  Exportação SVG       : {:7.2} ms", report.svg_export_ms);

    if let Some(path) = json_out {
        let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        fs::write(path, json)
            .map_err(|e| format!("Não foi possível gravar {}: {e}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stress_benchmark_runs_cleanly_on_3_storey_500_wall_building() {
        let report = run_bench().unwrap();
        assert_eq!(report.storeys, 3);
        assert!(
            report.total_walls_all_floors >= 500,
            "expected >=500 total walls, got {}",
            report.total_walls_all_floors
        );
        assert!(report.compression_ratio > 2.5);
        assert!(report.incremental_edit_ms.is_finite());
    }
}
