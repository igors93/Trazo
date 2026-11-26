//! The annotations tool: texts, leaders and technical symbols.

use serde_json::{Value, json};

use super::super::{Change, element_id, number, required_number, text, to_model, xy};
use super::round2;
use crate::{
    app::FloorPlanApp,
    domain::{
        geometry::Point,
        notes::{NoteKind, TechnicalSymbol},
    },
    mcp::ToolOutput,
};

impl FloorPlanApp {
    /// Texts, technical notes, leader callouts and symbols (`anotacoes`).
    pub(in crate::app::mcp_tools) fn tool_notes(
        &mut self,
        args: &Value,
    ) -> Result<ToolOutput, String> {
        let action = text(args, "acao")
            .unwrap_or_else(|| "listar".to_owned())
            .to_lowercase();
        match action.as_str() {
            "listar" => {
                let notes: Vec<Value> = self
                    .project
                    .notes
                    .iter()
                    .map(|n| {
                        let (kind, extra) = match n.kind {
                            NoteKind::Text => ("texto", json!({})),
                            NoteKind::Note => ("nota", json!({})),
                            NoteKind::Callout { target } => {
                                ("chamada", json!({ "alvo": xy(target) }))
                            }
                            NoteKind::Symbol(TechnicalSymbol::NorthArrow) => ("norte", json!({})),
                            NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m }) => {
                                ("cota_nivel", json!({ "nivel_m": round2(elevation_m) }))
                            }
                            NoteKind::Symbol(TechnicalSymbol::SlopeArrow { percent }) => {
                                ("caimento", json!({ "percentual": round2(percent) }))
                            }
                            NoteKind::Symbol(TechnicalSymbol::EntryArrow) => ("acesso", json!({})),
                        };
                        json!({
                            "id": n.id,
                            "tipo": kind,
                            "texto": n.text,
                            "posicao": xy(n.position),
                            "tamanho_fonte": round2(n.font_size),
                            "rotacao_graus": round2(n.rotation_deg),
                            "detalhes": extra,
                        })
                    })
                    .collect();
                Ok(
                    ToolOutput::ok(format!("{} anotação(ões) na planta.", notes.len()))
                        .with_structured(json!({
                            "projeto_id": self.active_project_id(),
                            "anotacoes": notes,
                        })),
                )
            }
            "adicionar" => {
                let kind_key = text(args, "tipo")
                    .unwrap_or_else(|| "texto".to_owned())
                    .to_lowercase()
                    .replace(['-', ' '], "_");
                let pos = to_model(required_number(args, "x")?, required_number(args, "y")?);
                let content = text(args, "texto").unwrap_or_default();
                let kind = match kind_key.as_str() {
                    "texto" | "text" => NoteKind::Text,
                    "nota" | "note" => NoteKind::Note,
                    "chamada" | "callout" => {
                        let target = to_model(
                            required_number(args, "alvo_x")?,
                            required_number(args, "alvo_y")?,
                        );
                        NoteKind::Callout { target }
                    }
                    "norte" | "north" => NoteKind::Symbol(TechnicalSymbol::NorthArrow),
                    "cota_nivel" | "nivel" | "level" => {
                        let elevation_m = number(args, "nivel_m")?.unwrap_or(0.0);
                        NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m })
                    }
                    "caimento" | "slope" => {
                        let percent = number(args, "percentual")?.unwrap_or(1.0);
                        NoteKind::Symbol(TechnicalSymbol::SlopeArrow { percent })
                    }
                    "acesso" | "entry" => NoteKind::Symbol(TechnicalSymbol::EntryArrow),
                    _ => {
                        return Err(
                            "'tipo' deve ser texto, nota, chamada, norte, cota_nivel, caimento ou acesso."
                                .to_owned(),
                        );
                    }
                };
                let font_size = number(args, "tamanho_fonte")?;
                let rotation_deg = number(args, "rotacao_graus")?;
                self.apply_change(None, |project| {
                    let id = project.add_note(pos, &content, kind)?;
                    if let Some(note) = project.notes.iter_mut().find(|n| n.id == id) {
                        if let Some(fs) = font_size {
                            note.font_size = fs.clamp(6.0, 72.0);
                        }
                        if let Some(rot) = rotation_deg {
                            note.rotation_deg = rot;
                        }
                    }
                    let saved = project
                        .notes
                        .iter()
                        .find(|n| n.id == id)
                        .ok_or("Anotação não encontrada após inserção.")?;
                    Ok(Change::new(format!(
                        "{} adicionado(a) (id {id}): \"{}\".",
                        kind.label(),
                        saved.text
                    ))
                    .data(json!({ "id": id, "texto": saved.text })))
                })
            }
            "editar" => {
                let id = element_id(args, "id")?;
                let new_text = text(args, "texto");
                let dx = number(args, "dx")?;
                let dy = number(args, "dy")?;
                let font_size = number(args, "tamanho_fonte")?;
                let rotation_deg = number(args, "rotacao_graus")?;
                if new_text.is_none()
                    && dx.is_none()
                    && dy.is_none()
                    && font_size.is_none()
                    && rotation_deg.is_none()
                {
                    return Err(
                        "Informe 'texto', 'dx'/'dy', 'tamanho_fonte' ou 'rotacao_graus'."
                            .to_owned(),
                    );
                }
                self.apply_change(Some("A anotação não sofreu alterações."), |project| {
                    if !project.notes.iter().any(|n| n.id == id) {
                        return Err("Anotação não encontrada.".to_owned());
                    }
                    if let Some(t) = &new_text {
                        project.update_note_text(id, t)?;
                    }
                    if dx.is_some() || dy.is_some() {
                        project.move_note(id, Point::new(dx.unwrap_or(0.0), -dy.unwrap_or(0.0)));
                    }
                    if let Some(note) = project.notes.iter_mut().find(|n| n.id == id) {
                        if let Some(fs) = font_size {
                            note.font_size = fs.clamp(6.0, 72.0);
                        }
                        if let Some(rot) = rotation_deg {
                            note.rotation_deg = rot;
                        }
                    }
                    Ok(Change::new(format!("Anotação {id} atualizada.")))
                })
            }
            "remover" => {
                let id = element_id(args, "id")?;
                self.apply_change(None, |project| {
                    if !project.delete_note(id) {
                        return Err("Anotação não encontrada.".to_owned());
                    }
                    Ok(Change::new(format!("Anotação {id} removida.")))
                })
            }
            _ => Err("Informe 'acao': listar, adicionar, editar ou remover.".to_owned()),
        }
    }
}
