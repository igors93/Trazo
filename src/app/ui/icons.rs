use eframe::egui::{
    self, Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, Ui,
};

#[derive(Clone, Copy)]
pub enum ToolIcon {
    Wall,
    Room,
    Terrain,
    Move,
    Door,
    Window,
    Delete,
}

pub fn tool_tile(
    ui: &mut Ui,
    label: &str,
    icon: ToolIcon,
    selected: bool,
    draggable: bool,
) -> Response {
    let sense = if draggable {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(70.0, 46.0), sense);
    let background = if !ui.is_enabled() {
        Color32::from_rgb(243, 246, 247)
    } else if selected {
        Color32::from_rgb(218, 245, 239)
    } else if response.hovered() {
        Color32::from_rgb(235, 246, 245)
    } else {
        Color32::from_rgb(255, 255, 255)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, background);
    if selected {
        painter.rect_stroke(
            rect,
            6.0,
            Stroke::new(1.5, Color32::from_rgb(27, 155, 140)),
            egui::StrokeKind::Inside,
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left() + 8.0, rect.bottom() - 2.5),
                Pos2::new(rect.right() - 8.0, rect.bottom()),
            ),
            1.5,
            Color32::from_rgb(27, 155, 140),
        );
    } else if response.hovered() && ui.is_enabled() {
        painter.rect_stroke(
            rect,
            6.0,
            Stroke::new(1.0, Color32::from_rgb(180, 210, 210)),
            egui::StrokeKind::Inside,
        );
    } else {
        painter.rect_stroke(
            rect,
            6.0,
            Stroke::new(0.8, Color32::from_rgb(222, 230, 233)),
            egui::StrokeKind::Inside,
        );
    }
    let color = if !ui.is_enabled() {
        Color32::from_rgb(157, 170, 175)
    } else if selected {
        Color32::from_rgb(20, 122, 112)
    } else {
        Color32::from_rgb(45, 66, 78)
    };
    let center = Pos2::new(rect.center().x, rect.top() + 16.0);
    paint_icon(painter, center, icon, color);
    painter.text(
        Pos2::new(rect.center().x, rect.bottom() - 9.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        color,
    );
    if ui.is_enabled() {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

fn paint_icon(painter: &Painter, center: Pos2, icon: ToolIcon, color: Color32) {
    let p = |x: f32, y: f32| center + egui::vec2(x, y);
    let line = |a: (f32, f32), b: (f32, f32), width: f32| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], Stroke::new(width, color));
    };
    match icon {
        ToolIcon::Wall => {
            line((-12.0, 9.0), (10.0, -9.0), 4.0);
            line((-8.0, 12.0), (14.0, -6.0), 2.0);
            painter.circle_filled(p(-12.0, 9.0), 2.5, color);
            painter.circle_filled(p(10.0, -9.0), 2.5, color);
        }
        ToolIcon::Room => {
            line((-11.0, -10.0), (11.0, -10.0), 2.5);
            line((-11.0, -10.0), (-11.0, 10.0), 2.5);
            line((11.0, -10.0), (11.0, 10.0), 2.5);
            line((-11.0, 10.0), (2.0, 10.0), 2.5);
            line((8.0, 10.0), (11.0, 10.0), 2.5);
            line((2.0, 10.0), (2.0, 3.0), 1.5);
        }
        ToolIcon::Terrain => {
            line((-12.0, -9.0), (12.0, -9.0), 2.0);
            line((12.0, -9.0), (12.0, 9.0), 2.0);
            line((12.0, 9.0), (-12.0, 9.0), 2.0);
            line((-12.0, 9.0), (-12.0, -9.0), 2.0);
            painter.circle_filled(p(-12.0, -9.0), 2.5, color);
            painter.circle_filled(p(12.0, 9.0), 2.5, color);
        }
        ToolIcon::Move => {
            line((-11.0, 0.0), (11.0, 0.0), 2.0);
            line((0.0, -11.0), (0.0, 11.0), 2.0);
            for (a, b) in [
                ((-11.0, 0.0), (-7.0, -4.0)),
                ((-11.0, 0.0), (-7.0, 4.0)),
                ((11.0, 0.0), (7.0, -4.0)),
                ((11.0, 0.0), (7.0, 4.0)),
                ((0.0, -11.0), (-4.0, -7.0)),
                ((0.0, -11.0), (4.0, -7.0)),
                ((0.0, 11.0), (-4.0, 7.0)),
                ((0.0, 11.0), (4.0, 7.0)),
            ] {
                line(a, b, 1.7);
            }
        }
        ToolIcon::Door => {
            line((-12.0, 10.0), (-4.0, 10.0), 3.0);
            line((8.0, 10.0), (12.0, 10.0), 3.0);
            line((-4.0, 10.0), (-4.0, -5.0), 2.0);
            let arc: Vec<_> = (0..=12)
                .map(|step| {
                    let angle = std::f32::consts::FRAC_PI_2 * step as f32 / 12.0;
                    p(-4.0 + 15.0 * angle.sin(), 10.0 - 15.0 * angle.cos())
                })
                .collect();
            painter.add(egui::Shape::line(arc, Stroke::new(1.3, color)));
        }
        ToolIcon::Window => {
            line((-13.0, 8.0), (-10.0, 8.0), 3.0);
            line((10.0, 8.0), (13.0, 8.0), 3.0);
            line((-10.0, -7.0), (-10.0, 8.0), 2.0);
            line((10.0, -7.0), (10.0, 8.0), 2.0);
            line((-10.0, -5.0), (10.0, -5.0), 2.0);
            line((-10.0, 3.0), (10.0, 3.0), 2.0);
        }
        ToolIcon::Delete => {
            line((-10.0, -8.0), (10.0, -8.0), 2.0);
            line((-5.0, -12.0), (5.0, -12.0), 2.0);
            line((-8.0, -5.0), (-7.0, 11.0), 2.0);
            line((8.0, -5.0), (7.0, 11.0), 2.0);
            line((-7.0, 11.0), (7.0, 11.0), 2.0);
            line((-3.0, -3.0), (-3.0, 7.0), 1.5);
            line((3.0, -3.0), (3.0, 7.0), 1.5);
        }
    }
}
