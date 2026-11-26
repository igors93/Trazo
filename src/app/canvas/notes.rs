//! Texts, leaders and technical symbols placed on the plan.

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke};

use crate::app::theme;
use crate::domain::model::Project;

use super::ViewTransform;

/// A property line: long dash, short dash.
pub(super) fn dash_dot(painter: &Painter, points: &[Pos2], stroke: Stroke) {
    painter.extend(Shape::dashed_line_with_offset(
        points,
        stroke,
        &[14.0, 2.5],
        &[4.0, 4.0],
        0.0,
    ));
}

pub(super) fn paint_notes(painter: &Painter, view: ViewTransform, project: &Project) {
    use crate::domain::notes::{NoteKind, TechnicalSymbol};
    let ink = Stroke::new(1.2, theme::TEXT);
    for note in &project.notes {
        let pos = view.to_screen(note.position);
        let font = theme::regular((note.font_size as f32).clamp(9.0, 22.0));
        match note.kind {
            NoteKind::Text => {
                painter.text(
                    pos,
                    egui::Align2::LEFT_CENTER,
                    &note.text,
                    font,
                    theme::TEXT,
                );
            }
            NoteKind::Note => {
                let galley = painter.layout_no_wrap(note.text.clone(), font, theme::TEXT);
                let box_rect = Rect::from_min_size(pos, galley.size() + egui::vec2(10.0, 6.0));
                painter.rect(
                    box_rect,
                    3.0,
                    Color32::from_rgba_unmultiplied(255, 253, 242, 240),
                    Stroke::new(1.0, theme::BORDER_STRONG),
                    egui::StrokeKind::Inside,
                );
                painter.galley(box_rect.min + egui::vec2(5.0, 3.0), galley, theme::TEXT);
            }
            NoteKind::Callout { target } => {
                let dst = view.to_screen(target);
                painter.line_segment([pos, dst], Stroke::new(1.1, theme::ACCENT_TEXT));
                painter.circle_filled(dst, 3.2, theme::ACCENT);
                let galley = painter.layout_no_wrap(note.text.clone(), font, theme::TEXT);
                let pill = Rect::from_min_size(
                    pos + egui::vec2(4.0, -galley.size().y * 0.5 - 3.0),
                    galley.size() + egui::vec2(8.0, 6.0),
                );
                painter.rect(
                    pill,
                    3.0,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 235),
                    Stroke::new(1.0, theme::ACCENT_RING),
                    egui::StrokeKind::Inside,
                );
                painter.galley(pill.min + egui::vec2(4.0, 3.0), galley, theme::TEXT);
            }
            NoteKind::Symbol(TechnicalSymbol::LevelMark { .. }) => {
                painter.circle(pos, 7.0, Color32::WHITE, ink);
                painter.line_segment(
                    [pos + egui::vec2(-10.0, 0.0), pos + egui::vec2(10.0, 0.0)],
                    ink,
                );
                painter.line_segment(
                    [pos + egui::vec2(0.0, -10.0), pos + egui::vec2(0.0, 10.0)],
                    ink,
                );
                painter.text(
                    pos + egui::vec2(12.0, -2.0),
                    egui::Align2::LEFT_CENTER,
                    &note.text,
                    theme::strong(11.0),
                    theme::TEXT,
                );
            }
            NoteKind::Symbol(TechnicalSymbol::SlopeArrow { .. }) => {
                let tip = pos + egui::vec2(32.0, 0.0);
                painter.line_segment([pos, tip], ink);
                painter.line_segment([tip, tip + egui::vec2(-6.0, -4.0)], ink);
                painter.line_segment([tip, tip + egui::vec2(-6.0, 4.0)], ink);
                painter.text(
                    pos + egui::vec2(16.0, -9.0),
                    egui::Align2::CENTER_CENTER,
                    &note.text,
                    theme::regular(10.5),
                    theme::TEXT_2,
                );
            }
            NoteKind::Symbol(TechnicalSymbol::NorthArrow) => {
                painter.circle(pos, 11.0, Color32::WHITE, ink);
                painter.add(Shape::convex_polygon(
                    vec![
                        pos + egui::vec2(0.0, -9.0),
                        pos + egui::vec2(5.0, 6.0),
                        pos + egui::vec2(-5.0, 6.0),
                    ],
                    theme::WALL,
                    Stroke::NONE,
                ));
                painter.text(
                    pos + egui::vec2(0.0, -17.0),
                    egui::Align2::CENTER_CENTER,
                    &note.text,
                    theme::strong(11.0),
                    theme::TEXT,
                );
            }
            NoteKind::Symbol(TechnicalSymbol::EntryArrow) => {
                painter.add(Shape::convex_polygon(
                    vec![
                        pos + egui::vec2(0.0, -8.0),
                        pos + egui::vec2(7.0, 5.0),
                        pos + egui::vec2(-7.0, 5.0),
                    ],
                    theme::ACCENT,
                    Stroke::NONE,
                ));
                painter.text(
                    pos + egui::vec2(0.0, 14.0),
                    egui::Align2::CENTER_CENTER,
                    &note.text,
                    theme::strong(10.0),
                    theme::ACCENT_TEXT,
                );
            }
        }
    }
}
