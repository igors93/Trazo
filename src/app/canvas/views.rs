//! Section and elevation markers on the plan.

use eframe::egui::{self, Painter, Pos2, Shape, Stroke};

use crate::app::theme;
use crate::domain::model::Project;

use super::{ViewTransform, dash_dot};

pub(super) fn paint_views(painter: &Painter, view: ViewTransform, project: &Project) {
    let section_color = theme::ACCENT_TEXT;
    let stroke = Stroke::new(1.6, section_color);
    for section in &project.views.sections {
        let a = view.to_screen(section.start);
        let b = view.to_screen(section.end);
        dash_dot(painter, &[a, b], stroke);
        let dir = (b - a).normalized();
        let look = if section.look_left {
            egui::vec2(-dir.y, dir.x)
        } else {
            egui::vec2(dir.y, -dir.x)
        };
        let short_tag = section
            .label
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_else(|| section.label.clone());
        for end in [a, b] {
            let tip = end + look * 14.0;
            let side = dir * 5.0;
            painter.add(Shape::convex_polygon(
                vec![tip, end + side, end - side],
                section_color,
                Stroke::NONE,
            ));
            let badge_center = end - look * 12.0;
            painter.circle(
                badge_center,
                10.0,
                theme::PANEL,
                Stroke::new(1.4, section_color),
            );
            painter.text(
                badge_center,
                egui::Align2::CENTER_CENTER,
                &short_tag,
                theme::strong(10.5),
                section_color,
            );
        }
        if let Some(sheet) = &section.target_sheet {
            let mid = a.lerp(b, 0.5) - look * 12.0;
            painter.text(
                mid,
                egui::Align2::CENTER_CENTER,
                format!("Corte {} ({sheet})", section.label),
                theme::regular(10.0),
                section_color,
            );
        }
    }

    for detail in &project.views.details {
        let c = view.to_screen(detail.center);
        let r = (detail.radius as f32 * view.scale).max(16.0);
        let steps = 36;
        let pts: Vec<Pos2> = (0..=steps)
            .map(|i| {
                let ang = std::f32::consts::TAU * i as f32 / steps as f32;
                c + egui::vec2(ang.cos() * r, ang.sin() * r)
            })
            .collect();
        painter.extend(Shape::dashed_line(
            &pts,
            Stroke::new(1.4, theme::WARNING),
            7.0,
            4.0,
        ));
        let tag = match &detail.target_sheet {
            Some(sheet) => format!("{} ({sheet})", detail.label),
            None => detail.label.clone(),
        };
        painter.text(
            c + egui::vec2(0.0, -r - 10.0),
            egui::Align2::CENTER_CENTER,
            tag,
            theme::strong(10.5),
            theme::WARNING,
        );
    }
}
