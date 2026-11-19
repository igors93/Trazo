//! The free-shape room tool: corners are clicked one after another and the
//! room closes on the first corner, a double click or Enter.

use crate::app::FloorPlanApp;
use crate::domain::geometry::{
    POINT_TOLERANCE, Point, polygon_area, polygon_is_simple, simplify_polygon,
};

impl FloorPlanApp {
    /// Adds a corner to the free-shape room; a click back on the first
    /// corner closes it.
    pub(in crate::app) fn add_polygon_point(&mut self, point: Point) {
        if self.polygon_draft.len() >= 3
            && self
                .polygon_draft
                .first()
                .is_some_and(|first| first.near(point, self.screen_metres(10.0).max(0.02)))
        {
            self.close_polygon_draft();
            return;
        }
        if self
            .polygon_draft
            .last()
            .is_some_and(|last| last.near(point, POINT_TOLERANCE))
        {
            return;
        }
        self.polygon_draft.push(point);
        self.message = if self.polygon_draft.len() < 3 {
            "Clique no próximo canto. Backspace desfaz o último; Esc cancela.".to_owned()
        } else {
            "Clique no primeiro canto, dê duplo clique ou Enter para fechar o cômodo.".to_owned()
        };
    }

    /// Ends the free-shape room and asks for its name.
    pub(in crate::app) fn close_polygon_draft(&mut self) {
        let corners = simplify_polygon(&self.polygon_draft);
        if corners.len() < 3 {
            self.message = "Um cômodo precisa de pelo menos 3 cantos.".to_owned();
        } else if !polygon_is_simple(&corners) {
            self.message =
                "Os lados do cômodo se cruzam; desfaça o último canto (Backspace).".to_owned();
        } else if polygon_area(&corners) < 0.25 {
            self.message = "O cômodo precisa de pelo menos 0,25 m².".to_owned();
        } else {
            self.polygon_draft.clear();
            self.pending_room = Some(corners);
            self.room_name.clear();
            self.room_dialog_focus = true;
            self.message = "Digite o nome do cômodo para concluir.".to_owned();
        }
    }

    /// Backspace: takes back the last corner of the free-shape room.
    pub(in crate::app) fn undo_polygon_point(&mut self) {
        if self.polygon_draft.pop().is_some() {
            self.message = format!(
                "Canto desfeito; {} no desenho.",
                match self.polygon_draft.len() {
                    0 => "nenhum canto".to_owned(),
                    1 => "1 canto".to_owned(),
                    count => format!("{count} cantos"),
                }
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{FloorPlanApp, Tool};
    use crate::domain::geometry::Point;
    use eframe::egui;

    #[test]
    fn free_shape_rooms_are_drawn_corner_by_corner() {
        let context = egui::Context::default();
        let mut app = FloorPlanApp::new(&context);
        app.set_tool(Tool::RoomPolygon);
        for corner in [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 2.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 4.0),
            Point::new(9.0, 9.0),
        ] {
            app.add_polygon_point(corner);
        }
        // Backspace takes back the stray corner; the draft goes on from
        // the last one.
        app.undo_polygon_point();
        assert_eq!(app.draft_start(), Some(Point::new(2.0, 4.0)));
        app.add_polygon_point(Point::new(0.0, 4.0));
        // Clicking the first corner closes the room and asks for its name.
        app.add_polygon_point(Point::new(0.0, 0.0));
        assert!(app.polygon_draft.is_empty());
        let corners = app.pending_room.clone().unwrap();
        assert_eq!(corners.len(), 6);
        assert_eq!(crate::domain::geometry::polygon_area(&corners), 12.0);

        // Crossing sides are refused before the name is asked.
        app.pending_room = None;
        for corner in [
            Point::new(10.0, 0.0),
            Point::new(14.0, 4.0),
            Point::new(14.0, 0.0),
            Point::new(10.0, 4.0),
        ] {
            app.add_polygon_point(corner);
        }
        app.close_polygon_draft();
        assert!(app.pending_room.is_none());
        assert_eq!(app.polygon_draft.len(), 4);
        app.cancel_creation();
        assert!(app.draft_start().is_none());
    }
}
