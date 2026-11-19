//! Texts, technical notes, leader callouts and architectural symbols (north
//! arrow, floor level marks, slope arrows and entry arrows).

use serde::{Deserialize, Serialize};

use super::{
    geometry::{Point, distance_to_segment},
    model::Project,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "symbol", rename_all = "snake_case")]
pub enum TechnicalSymbol {
    NorthArrow,
    LevelMark { elevation_m: f64 },
    SlopeArrow { percent: f64 },
    EntryArrow,
}

impl TechnicalSymbol {
    pub fn label(self) -> &'static str {
        match self {
            TechnicalSymbol::NorthArrow => "Símbolo de Norte",
            TechnicalSymbol::LevelMark { .. } => "Cota de nível",
            TechnicalSymbol::SlopeArrow { .. } => "Caimento de piso",
            TechnicalSymbol::EntryArrow => "Seta de acesso",
        }
    }

    pub fn display_text(self) -> String {
        match self {
            TechnicalSymbol::NorthArrow => "N".to_owned(),
            TechnicalSymbol::LevelMark { elevation_m } => format_elevation(elevation_m),
            TechnicalSymbol::SlopeArrow { percent } => format!("i = {:.1}%", percent)
                .replace(".0%", "%")
                .replace('.', ","),
            TechnicalSymbol::EntryArrow => "ACESSO".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoteKind {
    Text,
    Note,
    Callout { target: Point },
    Symbol(TechnicalSymbol),
}

impl NoteKind {
    pub fn label(self) -> &'static str {
        match self {
            NoteKind::Text => "Texto",
            NoteKind::Note => "Nota",
            NoteKind::Callout { .. } => "Chamada",
            NoteKind::Symbol(sym) => sym.label(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnnotationNote {
    pub id: u64,
    pub position: Point,
    pub text: String,
    pub kind: NoteKind,
    #[serde(default = "default_font_size")]
    pub font_size: f64,
    #[serde(default)]
    pub rotation_deg: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_id: Option<u64>,
}

fn default_font_size() -> f64 {
    12.0
}

pub fn format_elevation(elevation_m: f64) -> String {
    if elevation_m.abs() < 0.0005 {
        "±0,00".to_owned()
    } else if elevation_m > 0.0 {
        format!("+{elevation_m:.2}").replace('.', ",")
    } else {
        format!("{elevation_m:.2}").replace('.', ",")
    }
}

impl Project {
    pub fn add_note(&mut self, position: Point, text: &str, kind: NoteKind) -> Result<u64, String> {
        if !position.is_finite() {
            return Err("Posição da anotação inválida.".to_owned());
        }
        let content = match kind {
            NoteKind::Symbol(sym) if text.trim().is_empty() => sym.display_text(),
            _ => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    return Err("Digite um texto para a anotação.".to_owned());
                }
                trimmed.to_owned()
            }
        };
        if let NoteKind::Callout { target } = kind
            && !target.is_finite()
        {
            return Err("Alvo da chamada inválido.".to_owned());
        }
        let id = self.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        self.notes.push(AnnotationNote {
            id,
            position: position.rounded(),
            text: content,
            kind,
            font_size: default_font_size(),
            rotation_deg: 0.0,
            layer_id: self.active_layer_id,
        });
        Ok(id)
    }

    pub fn update_note_text(&mut self, id: u64, text: &str) -> Result<(), String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("O texto não pode ficar vazio.".to_owned());
        }
        let note = self
            .notes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or("Anotação não encontrada.")?;
        note.text = trimmed.to_owned();
        Ok(())
    }

    pub fn move_note(&mut self, id: u64, delta: Point) -> bool {
        let Some(note) = self.notes.iter_mut().find(|n| n.id == id) else {
            return false;
        };
        note.position = note.position.translated(delta).rounded();
        if let NoteKind::Callout { target } = &mut note.kind {
            *target = target.translated(delta).rounded();
        }
        true
    }

    pub fn delete_note(&mut self, id: u64) -> bool {
        let before = self.notes.len();
        self.notes.retain(|n| n.id != id);
        self.notes.len() < before
    }

    #[allow(dead_code)]
    pub fn note_at(&self, point: Point, tolerance: f64) -> Option<u64> {
        self.notes.iter().rev().find_map(|note| {
            let near_pos = note.position.distance(point) <= tolerance.max(0.35);
            let near_leader = match note.kind {
                NoteKind::Callout { target } => {
                    distance_to_segment(point, note.position, target) <= tolerance
                }
                _ => false,
            };
            (near_pos || near_leader).then_some(note.id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_callouts_and_technical_symbols_are_created_edited_moved_and_deleted() {
        assert_eq!(format_elevation(0.0), "±0,00");
        assert_eq!(format_elevation(0.15), "+0,15");
        assert_eq!(format_elevation(-0.05), "-0,05");

        let mut project = Project::default();
        let text_id = project
            .add_note(Point::new(1.0, 1.0), "Piso porcelanato", NoteKind::Text)
            .unwrap();
        let callout_id = project
            .add_note(
                Point::new(3.0, 2.0),
                "Ralo linear",
                NoteKind::Callout {
                    target: Point::new(2.0, 2.0),
                },
            )
            .unwrap();
        let level_id = project
            .add_note(
                Point::new(4.0, 4.0),
                "",
                NoteKind::Symbol(TechnicalSymbol::LevelMark { elevation_m: 0.15 }),
            )
            .unwrap();
        let slope_id = project
            .add_note(
                Point::new(5.0, 4.0),
                "",
                NoteKind::Symbol(TechnicalSymbol::SlopeArrow { percent: 1.5 }),
            )
            .unwrap();

        assert_eq!(
            project
                .notes
                .iter()
                .find(|n| n.id == level_id)
                .unwrap()
                .text,
            "+0,15"
        );
        assert_eq!(
            project
                .notes
                .iter()
                .find(|n| n.id == slope_id)
                .unwrap()
                .text,
            "i = 1,5%"
        );

        assert_eq!(
            project.note_at(Point::new(2.5, 2.0), 0.15),
            Some(callout_id)
        );
        project
            .update_note_text(text_id, "Porcelanato acetinado")
            .unwrap();
        assert!(project.move_note(callout_id, Point::new(0.5, 0.0)));
        assert!(project.delete_note(text_id));
        assert_eq!(project.notes.len(), 3);
    }
}
