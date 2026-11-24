//! On-screen rendering of structural simulation overlays (#106):
//! utilisation traffic-light colours, internal-force diagrams over beams,
//! exaggerated deformed shapes, animated slab-to-soil load path and
//! overlap-free labels. Mirrors `io::export::conduits::write_simulation_overlay`.

use eframe::egui::{self, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use crate::domain::{
    geometry::Point,
    model::Project,
    plan_results::{LoadPathStage, OverlayLabelTone, build_plan_simulation_overlay},
    verdict::Light,
};

use super::ViewTransform;

fn light_stroke_color(light: Light) -> Color32 {
    match light {
        Light::Green => Color32::from_rgb(46, 125, 50),
        Light::Yellow => Color32::from_rgb(230, 126, 34),
        Light::Red => Color32::from_rgb(198, 40, 40),
    }
}

fn light_fill_color(light: Light, alpha: u8) -> Color32 {
    match light {
        Light::Green => Color32::from_rgba_unmultiplied(46, 125, 50, alpha),
        Light::Yellow => Color32::from_rgba_unmultiplied(239, 108, 0, alpha),
        Light::Red => Color32::from_rgba_unmultiplied(198, 40, 40, alpha),
    }
}

fn tone_color(tone: OverlayLabelTone) -> Color32 {
    match tone {
        OverlayLabelTone::Green => Color32::from_rgb(27, 94, 32),
        OverlayLabelTone::Yellow => Color32::from_rgb(191, 90, 0),
        OverlayLabelTone::Red => Color32::from_rgb(183, 28, 28),
        OverlayLabelTone::MomentPos => Color32::from_rgb(21, 101, 192),
        OverlayLabelTone::MomentNeg => Color32::from_rgb(183, 28, 28),
        OverlayLabelTone::Shear => Color32::from_rgb(106, 27, 154),
        OverlayLabelTone::LoadPath => Color32::from_rgb(0, 105, 92),
    }
}

fn paint_polygon_fill(painter: &Painter, points: &[Pos2], fill: Color32, stroke: Stroke) {
    if points.len() < 3 {
        return;
    }
    // Fan triangulation works cleanly for convex slabs & single-sign diagram lobes;
    // for general concave rooms (L-shaped) we draw a translucent outline + path.
    let shape = egui::Shape::convex_polygon(points.to_vec(), fill, stroke);
    painter.add(shape);
}

fn paint_arrowhead(painter: &Painter, from: Pos2, to: Pos2, color: Color32, size: f32) {
    let dir = to - from;
    let len = dir.length();
    if len < 4.0 {
        return;
    }
    let u = dir / len;
    let n = Vec2::new(-u.y, u.x);
    let tip = to;
    let left = tip - u * size + n * (size * 0.45);
    let right = tip - u * size - n * (size * 0.45);
    painter.add(egui::Shape::convex_polygon(
        vec![tip, left, right],
        color,
        Stroke::NONE,
    ));
}

fn paint_rotated_rect(
    painter: &Painter,
    view: ViewTransform,
    item: &crate::domain::plan_results::PointUtilisationOverlay,
    min_side_m: f64,
    fill: Color32,
    stroke: Stroke,
) {
    let (sin, cos) = item.angle_rad.sin_cos();
    let hw = item.width_m.max(min_side_m) * 0.5;
    let hd = item.depth_m.max(min_side_m) * 0.5;
    let corners = [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)]
        .into_iter()
        .map(|(lx, ly)| {
            let p = Point::new(
                item.center.x + lx * cos - ly * sin,
                item.center.y + lx * sin + ly * cos,
            );
            view.to_screen(p)
        })
        .collect();
    painter.add(egui::Shape::convex_polygon(corners, fill, stroke));
}

/// Paints the active structural simulation overlays onto the canvas, requesting
/// continuous repaints whenever the animated load-path overlay is active.
pub(super) fn paint_simulation_overlay(painter: &Painter, view: ViewTransform, project: &Project) {
    let cfg = project.structure.overlay;
    if cfg.is_empty() {
        return;
    }
    let report = project.cached_structure_report();
    let overlay = build_plan_simulation_overlay(project, &report, cfg);
    if overlay.is_empty() {
        return;
    }
    let time_sec = painter.ctx().input(|i| i.time);

    // 1. Utilisation traffic-light fills on slabs, beams, columns, footings.
    for slab in &overlay.slabs {
        let pts: Vec<Pos2> = slab.polygon.iter().map(|&p| view.to_screen(p)).collect();
        let fill = light_fill_color(slab.light, 34);
        let stroke = Stroke::new(1.8, light_stroke_color(slab.light));
        // For concave L-shaped slabs draw closed perimeter stroke + subtle fill.
        if pts.len() == 4 {
            paint_polygon_fill(painter, &pts, fill, stroke);
        } else if pts.len() >= 3 {
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
    }
    for beam in &overlay.beams {
        let a = view.to_screen(beam.start);
        let b = view.to_screen(beam.end);
        let px_w = ((beam.width_m as f32) * view.scale).clamp(4.0, 14.0);
        painter.line_segment([a, b], Stroke::new(px_w, light_fill_color(beam.light, 110)));
        painter.line_segment([a, b], Stroke::new(2.2, light_stroke_color(beam.light)));
    }
    for ft in &overlay.footings {
        paint_rotated_rect(
            painter,
            view,
            ft,
            0.0,
            light_fill_color(ft.light, 45),
            Stroke::new(2.0, light_stroke_color(ft.light)),
        );
    }
    for col in &overlay.columns {
        paint_rotated_rect(
            painter,
            view,
            col,
            0.16,
            light_fill_color(col.light, 170),
            Stroke::new(2.0, light_stroke_color(col.light)),
        );
    }

    // 2. Internal-force diagrams directly over beams.
    for diag in &overlay.beam_diagrams {
        let (pos_fill, pos_stroke, neg_fill, neg_stroke) = match diag.kind {
            crate::domain::plan_results::BeamDiagramOverlay::Moment => (
                Color32::from_rgba_unmultiplied(21, 101, 192, 64),
                Color32::from_rgb(21, 101, 192),
                Color32::from_rgba_unmultiplied(198, 40, 40, 64),
                Color32::from_rgb(198, 40, 40),
            ),
            crate::domain::plan_results::BeamDiagramOverlay::Shear => (
                Color32::from_rgba_unmultiplied(106, 27, 154, 60),
                Color32::from_rgb(106, 27, 154),
                Color32::from_rgba_unmultiplied(216, 67, 21, 60),
                Color32::from_rgb(216, 67, 21),
            ),
            crate::domain::plan_results::BeamDiagramOverlay::None => continue,
        };
        for lobe in &diag.positive_lobes {
            let pts: Vec<Pos2> = lobe.iter().map(|&p| view.to_screen(p)).collect();
            paint_polygon_fill(painter, &pts, pos_fill, Stroke::new(1.5, pos_stroke));
        }
        for lobe in &diag.negative_lobes {
            let pts: Vec<Pos2> = lobe.iter().map(|&p| view.to_screen(p)).collect();
            paint_polygon_fill(painter, &pts, neg_fill, Stroke::new(1.5, neg_stroke));
        }
        let curve_pts: Vec<Pos2> = diag.curve.iter().map(|&p| view.to_screen(p)).collect();
        if curve_pts.len() >= 2 {
            painter.add(egui::Shape::line(curve_pts, Stroke::new(1.8, pos_stroke)));
        }
    }

    // 3. Deformed shape over beams.
    for defl in &overlay.deformed_beams {
        let a = view.to_screen(defl.start);
        let b = view.to_screen(defl.end);
        painter.extend(egui::Shape::dashed_line(
            &[a, b],
            Stroke::new(1.0, Color32::from_gray(130)),
            4.0,
            4.0,
        ));
        let pts: Vec<Pos2> = defl.curve.iter().map(|&p| view.to_screen(p)).collect();
        if pts.len() >= 2 {
            painter.add(egui::Shape::line(
                pts,
                Stroke::new(2.4, light_stroke_color(defl.light)),
            ));
        }
    }

    // 4. Animated load path from slab -> beam -> column -> footing/soil.
    let animated = !overlay.load_paths.is_empty();
    let phase = (time_sec * 0.75).fract() as f32;
    for arrow in &overlay.load_paths {
        let a = view.to_screen(arrow.from);
        let b = view.to_screen(arrow.to);
        let (color, width) = match arrow.stage {
            LoadPathStage::SlabToBeam => (Color32::from_rgb(2, 119, 189), 1.8),
            LoadPathStage::BeamToColumn => (Color32::from_rgb(0, 137, 123), 2.2),
            LoadPathStage::ColumnToFooting => (Color32::from_rgb(198, 40, 40), 2.8),
        };
        painter.line_segment([a, b], Stroke::new(width, color));
        paint_arrowhead(painter, a, b, color, 7.0);
        // Animated marching flow pulse along the load path.
        let pulse = a + (b - a) * phase;
        painter.circle_filled(pulse, 3.6, Color32::WHITE);
        painter.circle_stroke(pulse, 3.6, Stroke::new(1.8, color));
    }

    // 5. Overlap-free labels (B05).
    if view.scale >= 28.0 {
        let font_size = (view.scale * 0.115).clamp(9.0, 12.0);
        for label in &overlay.labels {
            let color = tone_color(label.tone);
            let galley =
                painter.layout_no_wrap(label.text.clone(), FontId::proportional(font_size), color);
            let rect = Rect::from_center_size(view.to_screen(label.anchor), galley.size());
            painter.rect_filled(
                rect.expand2(Vec2::new(3.0, 1.0)),
                2.5,
                Color32::from_rgba_unmultiplied(255, 255, 255, 225),
            );
            painter.rect_stroke(
                rect.expand2(Vec2::new(3.0, 1.0)),
                2.5,
                Stroke::new(0.7, color.gamma_multiply(0.55)),
                egui::StrokeKind::Outside,
            );
            painter.galley(rect.min, galley, color);
        }
    }

    if animated {
        painter.ctx().request_repaint();
    }
}
