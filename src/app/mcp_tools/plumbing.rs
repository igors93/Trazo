//! Assistant tools for water, sewage and rain (roadmap section E): the
//! plumbing report `instalacoes_hidraulicas`.

use serde_json::{Value, json};

use super::{ToolOutput, round, xy};
use crate::app::FloorPlanApp;
use crate::domain::plumbing::points::{PointKind, plumbing_points};

impl FloorPlanApp {
    /// `instalacoes_hidraulicas`: the plumbing points, their counts and
    /// the fixture schedule.
    pub(super) fn tool_plumbing_report(&self) -> Result<ToolOutput, String> {
        let project = &self.project;
        let points = plumbing_points(project);
        if points.is_empty() {
            return Ok(ToolOutput::ok(
                "Nenhum aparelho sanitário na planta: coloque vaso, lavatório, chuveiro, pia ou máquina de lavar com inserir_objeto.",
            )
            .with_structured(json!({ "pontos": [] })));
        }
        let counts = PointKind::ALL
            .iter()
            .map(|k| format!("{} {}", points.count(*k), k.label().to_lowercase()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut text = format!(
            "Pontos: {counts}. Aquecimento: {}.\nQuadro de aparelhos:",
            project.plumbing.heater.label().to_lowercase()
        );
        for row in &points.schedule {
            let rooms = if row.rooms.is_empty() {
                String::new()
            } else {
                format!(" ({})", row.rooms.join(", "))
            };
            text.push_str(&format!(
                "\n- {} × {}{rooms}: peso {}, {} UHC cada",
                row.count,
                row.kind.label(),
                round(row.weight),
                round(row.uhc)
            ));
        }
        text.push_str(&format!(
            "\nSoma dos pesos {}, soma das UHC {}.",
            round(points.total_weight),
            round(points.total_uhc)
        ));
        for note in &points.notes {
            text.push_str(&format!("\n{note}"));
        }
        let list: Vec<Value> = points
            .points
            .iter()
            .map(|p| {
                json!({
                    "aparelho_id": p.fixture_id,
                    "aparelho": p.fixture.key(),
                    "ponto": p.kind.key(),
                    "sigla": p.kind.tag(),
                    "posicao": xy(p.at),
                    "altura_m": round(p.height_m),
                    "diametro_mm": round(p.diameter_mm),
                    "comodo": p.room,
                })
            })
            .collect();
        let schedule: Vec<Value> = points
            .schedule
            .iter()
            .map(|row| {
                json!({
                    "aparelho": row.kind.key(),
                    "nome": row.kind.label(),
                    "quantidade": row.count,
                    "comodos": row.rooms,
                    "agua_fria_altura_m": row.cold_m.map(round),
                    "agua_quente_altura_m": row.hot_m.map(round),
                    "gas_altura_m": row.gas_m.map(round),
                    "esgoto": row.drain.map(|(dn, h)| json!({ "dn": dn, "altura_m": round(h) })),
                    "peso": round(row.weight),
                    "uhc": round(row.uhc),
                    "valor_de_referencia_trazo": row.trazo_reference,
                })
            })
            .collect();
        let count: serde_json::Map<String, Value> = PointKind::ALL
            .iter()
            .map(|k| (k.key().to_owned(), json!(points.count(*k))))
            .collect();
        Ok(ToolOutput::ok(text).with_structured(json!({
            "aquecimento": project.plumbing.heater.key(),
            "pontos": list,
            "contagem": count,
            "quadro_de_aparelhos": schedule,
            "soma_pesos": round(points.total_weight),
            "soma_uhc": round(points.total_uhc),
            "observacoes": points.notes,
        })))
    }
}
