//! The quantities takeoff and ergonomics check tools.

use serde_json::{Value, json};

use super::super::{number, round, xy};
use super::round2;
use crate::{
    app::FloorPlanApp,
    domain::{
        ergonomics::{ErgonomicsIssueKind, IssueSeverity, analyze_ergonomics},
        geometry::Point,
    },
    mcp::ToolOutput,
};

impl FloorPlanApp {
    /// Read-only consolidated construction quantities takeoff (#56).
    pub(in crate::app::mcp_tools) fn tool_quantities(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let all_floors = args
            .get("todos_pavimentos")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let waste_override = number(args, "perda_blocos_pct")?;
        if let Some(w) = waste_override
            && !(0.0..=50.0).contains(&w)
        {
            return Err("'perda_blocos_pct' deve ficar entre 0 e 50.".to_owned());
        }
        let takeoff = self
            .project
            .construction_takeoff_with_options(all_floors, waste_override);
        let structure = self.project.cached_structure_report();
        let reinforcement = self.project.cached_reinforcement_report();
        let roof_tiles: u64 = structure
            .roofs
            .iter()
            .filter_map(|r| r.tiles)
            .map(|t| t.ceil() as u64)
            .sum();
        let roof_timber_m3: f64 = structure.roofs.iter().map(|r| r.timber_m3).sum();
        let roof_area_m2: f64 = structure.roofs.iter().map(|r| r.surface_m2).sum();
        let steel_kg = reinforcement.total_mass_kg();

        let masonry_json: Vec<Value> = takeoff
            .masonry
            .iter()
            .map(|m| {
                json!({
                    "material": m.masonry,
                    "area_liquida_m2": round2(m.net_area_m2),
                    "blocos": m.blocks.ceil() as u64,
                    "blocos_com_perda": m.blocks_with_waste.ceil() as u64,
                    "meios_blocos": m.half_blocks.ceil() as u64,
                    "meios_blocos_com_perda": m.half_blocks_with_waste.ceil() as u64,
                    "canaletas_cinta": m.bond_beam_u_blocks.ceil() as u64,
                    "canaletas_vergas": m.lintel_u_blocks.ceil() as u64,
                    "canaletas": m.u_blocks.ceil() as u64,
                    "canaletas_com_perda": m.u_blocks_with_waste.ceil() as u64,
                })
            })
            .collect();
        let walls_json: Vec<Value> = takeoff
            .walls
            .iter()
            .map(|w| {
                json!({
                    "parede_id": w.wall_id,
                    "pavimento": w.floor_name,
                    "material": w.masonry,
                    "comprimento_m": round2(w.length_m),
                    "altura_m": round2(w.height_m),
                    "area_bruta_m2": round2(w.gross_m2),
                    "aberturas_m2": round2(w.openings_m2),
                    "area_liquida_m2": round2(w.net_m2),
                    "blocos": w.blocks.ceil() as u64,
                    "meios_blocos": w.half_blocks.ceil() as u64,
                    "canaletas": w.u_blocks.ceil() as u64,
                    "argamassa_m3": round(w.mortar_m3),
                    "reboco_m2": round2(w.plaster_m2),
                    "reboco_m3": round(w.plaster_m3),
                    "graute_m3": round(w.grout_m3),
                })
            })
            .collect();
        let lintels_json: Vec<Value> = takeoff
            .lintels
            .iter()
            .map(|l| {
                json!({
                    "abertura_id": l.opening_id,
                    "parede_id": l.wall_id,
                    "pavimento": l.floor_name,
                    "codigo": l.code,
                    "tipo": l.kind,
                    "abertura": l.opening,
                    "vao_m": round2(l.opening_width_m),
                    "transpasse_lado_m": round2(l.bearing_each_side_m),
                    "comprimento_m": round2(l.length_m),
                    "canaletas": l.u_blocks.ceil() as u64,
                    "graute_m3": round(l.grout_m3),
                })
            })
            .collect();
        let rooms_json: Vec<Value> = takeoff
            .rooms
            .iter()
            .map(|r| {
                json!({
                    "pavimento": r.floor_name,
                    "comodo": r.room,
                    "piso_m2": round2(r.floor_m2),
                    "piso_com_perda_m2": round2(r.floor_with_waste_m2),
                    "rodape_m": round2(r.skirting_m),
                    "paredes_m2": round2(r.walls_m2),
                    "paredes_com_perda_m2": round2(r.walls_with_waste_m2),
                    "teto_m2": round2(r.ceiling_m2),
                    "impermeabilizacao_m2": round2(r.waterproofing_m2),
                    "acabamento_piso": r.floor_spec,
                    "acabamento_parede": r.wall_spec,
                    "acabamento_teto": r.ceiling_spec,
                    "acabamento_rodape": r.baseboard_spec,
                    "impermeabilizado": r.waterproofing,
                })
            })
            .collect();
        let floors_json: Vec<Value> = takeoff
            .floors
            .iter()
            .map(|f| {
                json!({
                    "pavimento": f.floor_name,
                    "alvenaria_liquida_m2": round2(f.net_wall_m2),
                    "blocos_com_perda": f.blocks_with_waste.ceil() as u64,
                    "meios_blocos_com_perda": f.half_blocks_with_waste.ceil() as u64,
                    "canaletas_com_perda": f.u_blocks_with_waste.ceil() as u64,
                    "argamassa_m3": round(f.mortar_m3),
                    "reboco_m2": round2(f.plaster_m2),
                    "reboco_m3": round(f.plaster_m3),
                    "graute_m3": round(f.grout_m3),
                    "vergas_qtd": f.lintel_count,
                    "vergas_m": round2(f.lintel_m),
                    "piso_com_perda_m2": round2(f.floor_with_waste_m2),
                    "paredes_internas_m2": round2(f.room_walls_m2),
                    "teto_m2": round2(f.ceiling_m2),
                })
            })
            .collect();

        let data = json!({
            "projeto_id": self.active_project_id(),
            "todos_pavimentos": takeoff.all_floors,
            "perda_percentual": round2(takeoff.waste_percent),
            "alvenaria": masonry_json,
            "paredes": walls_json,
            "vergas_contravergas": lintels_json,
            "acabamentos": rooms_json,
            "por_pavimento": floors_json,
            "telhados": {
                "quantidade": structure.roofs.len(),
                "area_m2": round2(roof_area_m2),
                "telhas": roof_tiles,
                "madeira_m3": round2(roof_timber_m3),
            },
            "totais": {
                "alvenaria_liquida_m2": round2(takeoff.net_wall_m2),
                "argamassa_m3": round(takeoff.mortar_m3),
                "reboco_m2": round2(takeoff.plaster_m2),
                "reboco_m3": round(takeoff.plaster_m3),
                "graute_canaletas_m3": round(takeoff.grout_m3),
                "vergas_m": round2(takeoff.lintel_m),
                "piso_m2": round2(takeoff.floor_m2),
                "piso_com_perda_m2": round2(takeoff.floor_with_waste_m2),
                "rodape_m": round2(takeoff.skirting_m),
                "paredes_internas_m2": round2(takeoff.room_walls_m2),
                "paredes_internas_com_perda_m2": round2(takeoff.room_walls_with_waste_m2),
                "teto_m2": round2(takeoff.ceiling_m2),
                "impermeabilizacao_m2": round2(takeoff.waterproofing_m2),
                "aco_preliminar_kg": round2(steel_kg),
                "madeira_telhado_m3": round2(roof_timber_m3),
            },
        });
        Ok(ToolOutput::ok(format!(
            "Quantitativos: {:.2} m² de alvenaria líquida, {:.2} m² de piso ({:.2} m² com perda), {:.1} kg de aço preliminar.",
            takeoff.net_wall_m2,
            takeoff.floor_m2,
            takeoff.floor_with_waste_m2,
            steel_kg
        )
        .replace('.', ","))
        .with_structured(data))
    }

    /// Read-only residential ergonomics and ABNT NBR 9050 clearance analysis.
    pub(in crate::app::mcp_tools) fn tool_ergonomics(
        &self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let min_passage = number(args, "passagem_minima_m")?.unwrap_or(0.80);
        if !(0.40..=2.50).contains(&min_passage) {
            return Err("'passagem_minima_m' deve ficar entre 0,40 e 2,50 m.".to_owned());
        }
        let report = analyze_ergonomics(&self.project, min_passage);
        let issues: Vec<Value> = report
            .issues
            .iter()
            .map(|issue| {
                let kind = match issue.kind {
                    ErgonomicsIssueKind::FixtureCollision => "colisao_objetos",
                    ErgonomicsIssueKind::WallCollision => "colisao_parede",
                    ErgonomicsIssueKind::DoorSwingObstruction => "abertura_porta_obstruida",
                    ErgonomicsIssueKind::ClearanceBlocked => "area_uso_obstruida",
                    ErgonomicsIssueKind::NarrowPassage => "passagem_estreita",
                };
                let severity = match issue.severity {
                    IssueSeverity::Error => "erro",
                    IssueSeverity::Warning => "alerta",
                };
                json!({
                    "severidade": severity,
                    "tipo": kind,
                    "posicao": xy(issue.location),
                    "mensagem": issue.message,
                })
            })
            .collect();
        let zones: Vec<Value> = report
            .clearance_zones
            .iter()
            .map(|z| {
                json!({
                    "objeto_id": z.fixture_id,
                    "rotulo": z.label,
                    "profundidade_m": round2(z.depth_m),
                    "min": xy(Point::new(z.min.x, z.max.y)),
                    "max": xy(Point::new(z.max.x, z.min.y)),
                })
            })
            .collect();
        Ok(ToolOutput::ok(
            format!(
                "Ergonomia (passagem mínima {:.2} m): {} erro(s), {} alerta(s).",
                report.min_passage_m,
                report.error_count(),
                report.warning_count()
            )
            .replace('.', ","),
        )
        .with_structured(json!({
            "projeto_id": self.active_project_id(),
            "passagem_minima_m": round2(report.min_passage_m),
            "erros": report.error_count(),
            "alertas": report.warning_count(),
            "ocorrencias": issues,
            "zonas_de_uso": zones,
        })))
    }
}
