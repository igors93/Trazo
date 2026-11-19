//! The drawing tools: line, polyline, rectangle, arc, circle and spline.
//! Points are clicked one after another; lines chain from their end,
//! polylines and splines end with Enter or a double click (or close on
//! their first point).

use eframe::egui::{self, Key};

use crate::app::ui::Glyph;
use crate::app::{FloorPlanApp, Tool};
use crate::domain::drawing::Shape;
use crate::domain::geometry::{POINT_TOLERANCE, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum DrawKind {
    Line,
    Polyline,
    Rectangle,
    Arc,
    Circle,
    Spline,
}

impl DrawKind {
    pub(in crate::app) const ALL: [DrawKind; 6] = [
        DrawKind::Line,
        DrawKind::Polyline,
        DrawKind::Rectangle,
        DrawKind::Arc,
        DrawKind::Circle,
        DrawKind::Spline,
    ];

    pub(in crate::app) fn label(self) -> &'static str {
        match self {
            DrawKind::Line => "Linha",
            DrawKind::Polyline => "Polilinha",
            DrawKind::Rectangle => "Retângulo",
            DrawKind::Arc => "Arco",
            DrawKind::Circle => "Círculo",
            DrawKind::Spline => "Spline",
        }
    }

    pub(in crate::app) fn glyph(self) -> Glyph {
        match self {
            DrawKind::Line => Glyph::Line,
            DrawKind::Polyline => Glyph::Polyline,
            DrawKind::Rectangle => Glyph::Rectangle,
            DrawKind::Arc => Glyph::Arc,
            DrawKind::Circle => Glyph::Circle,
            DrawKind::Spline => Glyph::Spline,
        }
    }

    /// The key that, with Shift, picks the tool.
    pub(in crate::app) fn key(self) -> (Key, &'static str) {
        match self {
            DrawKind::Line => (Key::L, "Shift+L"),
            DrawKind::Polyline => (Key::P, "Shift+P"),
            DrawKind::Rectangle => (Key::R, "Shift+R"),
            DrawKind::Arc => (Key::A, "Shift+A"),
            DrawKind::Circle => (Key::C, "Shift+C"),
            DrawKind::Spline => (Key::S, "Shift+S"),
        }
    }

    /// What to do, shown when the tool is picked.
    pub(in crate::app) fn hint(self) -> &'static str {
        match self {
            DrawKind::Line => {
                "Clique no início e no fim da linha; ela continua do fim. Esc encerra."
            }
            DrawKind::Polyline => {
                "Clique em cada ponto; Enter ou duplo clique termina, o primeiro ponto fecha."
            }
            DrawKind::Rectangle => "Clique em dois cantos opostos do retângulo.",
            DrawKind::Arc => "Clique no início, num ponto do arco e no fim.",
            DrawKind::Circle => "Clique no centro e depois num ponto da circunferência.",
            DrawKind::Spline => {
                "Clique nos pontos por onde a curva passa; Enter ou duplo clique termina."
            }
        }
    }

    /// How many points make the shape, for those with a fixed count.
    fn points_needed(self) -> Option<usize> {
        match self {
            DrawKind::Line | DrawKind::Rectangle | DrawKind::Circle => Some(2),
            DrawKind::Arc => Some(3),
            DrawKind::Polyline | DrawKind::Spline => None,
        }
    }
}

impl FloorPlanApp {
    /// A click with a drawing tool.
    pub(in crate::app) fn add_draw_point(&mut self, kind: DrawKind, point: Point) {
        if self
            .draw_points
            .last()
            .is_some_and(|last| last.near(point, POINT_TOLERANCE))
        {
            return;
        }
        let Some(needed) = kind.points_needed() else {
            let closes = self.draw_points.len() >= 2
                && self
                    .draw_points
                    .first()
                    .is_some_and(|first| first.near(point, self.screen_metres(10.0).max(0.02)));
            if closes {
                self.finish_drawing(kind, true);
            } else {
                self.draw_points.push(point);
                self.message = format!(
                    "{} ponto(s). Enter ou duplo clique termina; clique no primeiro ponto para fechar.",
                    self.draw_points.len()
                );
            }
            return;
        };
        self.draw_points.push(point);
        if self.draw_points.len() < needed {
            self.message = match (kind, self.draw_points.len()) {
                (DrawKind::Arc, 1) => "Clique num ponto por onde o arco passa.",
                (DrawKind::Arc, _) => "Clique no fim do arco.",
                (DrawKind::Circle, _) => "Clique num ponto da circunferência (define o raio).",
                (DrawKind::Rectangle, _) => "Clique no canto oposto.",
                _ => "Clique no próximo ponto.",
            }
            .to_owned();
            return;
        }
        let shape = Self::shape_from(kind, &self.draw_points, None, false);
        self.draw_points.clear();
        if let Some(shape) = shape {
            self.create_drawing(shape);
        } else {
            self.message = "Os três pontos estão em linha: o arco não existe.".to_owned();
        }
        // Lines chain: the next one starts where this one ended.
        if kind == DrawKind::Line {
            self.draw_points.push(point);
        }
    }

    /// The shape the clicked `points` (and the pointer, while drawing) make.
    pub(in crate::app) fn shape_from(
        kind: DrawKind,
        points: &[Point],
        cursor: Option<Point>,
        closed: bool,
    ) -> Option<Shape> {
        let mut all = points.to_vec();
        if let Some(cursor) = cursor
            && all
                .last()
                .is_none_or(|last| !last.near(cursor, POINT_TOLERANCE))
        {
            all.push(cursor);
        }
        if all.len() < 2 {
            return None;
        }
        Some(match kind {
            DrawKind::Line => Shape::Line {
                start: all[0],
                end: all[1],
            },
            DrawKind::Rectangle => Shape::rectangle(all[0], all[1]),
            DrawKind::Circle => Shape::Circle {
                center: all[0],
                radius: all[0].distance(all[1]),
            },
            // With two points, the chord shows until the third is placed.
            DrawKind::Arc if all.len() == 2 => Shape::Line {
                start: all[0],
                end: all[1],
            },
            DrawKind::Arc => Shape::arc_through(all[0], all[1], all[2])?,
            DrawKind::Polyline => Shape::Polyline {
                points: all,
                closed,
            },
            DrawKind::Spline => Shape::Spline {
                points: all,
                closed,
            },
        })
    }

    /// Enter or a double click: ends the polyline or spline being drawn
    /// (`closed` joins it back to its first point), or stops a line chain.
    pub(in crate::app) fn finish_drawing(&mut self, kind: DrawKind, closed: bool) {
        match kind {
            DrawKind::Polyline | DrawKind::Spline => {
                let needed = if closed { 3 } else { 2 };
                if self.draw_points.len() < needed {
                    self.message = format!("São necessários pelo menos {needed} pontos.");
                    return;
                }
                let points = std::mem::take(&mut self.draw_points);
                if let Some(shape) = Self::shape_from(kind, &points, None, closed) {
                    self.create_drawing(shape);
                }
            }
            DrawKind::Line if !self.draw_points.is_empty() => {
                self.draw_points.clear();
                self.message = "Sequência de linhas encerrada.".to_owned();
            }
            _ => {}
        }
    }

    /// Backspace: takes back the last point clicked.
    pub(in crate::app) fn undo_draw_point(&mut self) {
        if self.draw_points.pop().is_some() {
            self.message = format!("Ponto desfeito; restam {}.", self.draw_points.len());
        }
    }

    fn create_drawing(&mut self, shape: Shape) {
        let previous = self.project.clone();
        let label = shape.label();
        match self.project.add_drawing(shape) {
            Ok(_) => {
                self.record_change(previous);
                self.message = format!("{label} criado(a). Continue desenhando ou Esc.");
            }
            Err(error) => self.message = error,
        }
    }

    /// The drawing tool picked by Shift and a letter, if any.
    pub(in crate::app) fn drafting_shortcut(input: &egui::InputState) -> Option<Tool> {
        DrawKind::ALL.into_iter().find_map(|kind| {
            let (key, _) = kind.key();
            input
                .events
                .iter()
                .any(|event| {
                    matches!(event, egui::Event::Key { key: pressed, pressed: true, repeat: false, modifiers, .. }
                        if *pressed == key && modifiers.shift && !modifiers.ctrl && !modifiers.command && !modifiers.alt)
                })
                .then_some(Tool::Draw(kind))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::drawing::Shape;

    fn app() -> FloorPlanApp {
        FloorPlanApp::new(&egui::Context::default())
    }

    #[test]
    fn lines_chain_from_their_end_until_escape() {
        let mut app = app();
        app.set_tool(Tool::Draw(DrawKind::Line));
        app.add_draw_point(DrawKind::Line, Point::new(0.0, 0.0));
        app.add_draw_point(DrawKind::Line, Point::new(3.0, 0.0));
        app.add_draw_point(DrawKind::Line, Point::new(3.0, 2.0));
        assert_eq!(app.project.drawings.len(), 2);
        assert_eq!(app.draft_start(), Some(Point::new(3.0, 2.0)));
        app.cancel_creation();
        assert!(app.draft_start().is_none());
        assert!(app.history.can_undo());
    }

    #[test]
    fn each_tool_builds_its_shape() {
        let mut app = app();
        app.add_draw_point(DrawKind::Rectangle, Point::new(0.0, 0.0));
        app.add_draw_point(DrawKind::Rectangle, Point::new(4.0, 3.0));
        app.add_draw_point(DrawKind::Circle, Point::new(10.0, 0.0));
        app.add_draw_point(DrawKind::Circle, Point::new(11.5, 0.0));
        for point in [
            Point::new(-2.0, 10.0),
            Point::new(0.0, 8.0),
            Point::new(2.0, 10.0),
        ] {
            app.add_draw_point(DrawKind::Arc, point);
        }
        let labels: Vec<&str> = app
            .project
            .drawings
            .iter()
            .map(|drawing| drawing.shape.label())
            .collect();
        assert_eq!(labels, ["Retângulo", "Círculo", "Arco"]);
        assert!(matches!(
            app.project.drawings[1].shape,
            Shape::Circle { radius, .. } if (radius - 1.5).abs() < 1e-9
        ));

        // A polyline closes on its first point; a spline ends with Enter.
        for point in [
            Point::new(0.0, 20.0),
            Point::new(4.0, 20.0),
            Point::new(4.0, 24.0),
            Point::new(0.0, 20.0),
        ] {
            app.add_draw_point(DrawKind::Polyline, point);
        }
        assert!(app.project.drawings[3].shape.is_closed());
        app.add_draw_point(DrawKind::Spline, Point::new(0.0, 30.0));
        app.add_draw_point(DrawKind::Spline, Point::new(2.0, 31.0));
        app.add_draw_point(DrawKind::Spline, Point::new(4.0, 30.0));
        app.undo_draw_point();
        app.finish_drawing(DrawKind::Spline, false);
        assert_eq!(app.project.drawings[4].shape.key_points().len(), 2);
        assert_eq!(app.project.drawings.len(), 5);
    }
}
