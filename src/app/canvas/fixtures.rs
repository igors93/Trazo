//! Fixture symbols: furniture, fittings and devices in plan.

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke};

use crate::app::theme;
use crate::domain::{
    fixtures::{Fixture, SymbolShape, Weight},
    geometry::Point,
};

use super::{FixtureTone, ViewTransform};

pub fn paint_fixture(painter: &Painter, view: ViewTransform, fixture: &Fixture, tone: FixtureTone) {
    let (ink, detail, fill) = match tone {
        FixtureTone::Normal => (theme::TEXT_2, theme::MUTED, Color32::WHITE),
        FixtureTone::Hovered => (theme::HOVER, theme::HOVER, Color32::WHITE),
        FixtureTone::Selected => (theme::SELECTION, theme::SELECTION, theme::ACCENT_SOFTER),
        FixtureTone::Ghost => (
            theme::ACCENT,
            theme::ACCENT.gamma_multiply(0.8),
            theme::ACCENT.gamma_multiply(0.08),
        ),
    };
    // Electrical devices are drawn in the electrical colour, with no
    // backing, so walls and furniture show through.
    let electrical = fixture.kind.is_electrical();
    let (ink, detail, fill) = if electrical && tone == FixtureTone::Normal {
        (theme::ELECTRICAL, theme::ELECTRICAL, Color32::TRANSPARENT)
    } else if electrical {
        (ink, detail, Color32::TRANSPARENT)
    } else {
        (ink, detail, fill)
    };
    // Structure in its own colour; footings lie below, so nothing covers.
    let (ink, detail, fill) = match fixture.kind {
        crate::domain::fixtures::FixtureKind::Footing
        | crate::domain::fixtures::FixtureKind::PileCap
            if tone == FixtureTone::Normal =>
        {
            (theme::STRUCTURE, theme::STRUCTURE, Color32::TRANSPARENT)
        }
        crate::domain::fixtures::FixtureKind::Footing
        | crate::domain::fixtures::FixtureKind::PileCap => (ink, detail, Color32::TRANSPARENT),
        crate::domain::fixtures::FixtureKind::Column
        | crate::domain::fixtures::FixtureKind::TimberPost
            if tone == FixtureTone::Normal =>
        {
            (theme::STRUCTURE, theme::STRUCTURE, Color32::WHITE)
        }
        crate::domain::fixtures::FixtureKind::WaterTank
        | crate::domain::fixtures::FixtureKind::HeavyEquipment
            if tone == FixtureTone::Normal =>
        {
            (theme::STRUCTURE, theme::STRUCTURE, Color32::TRANSPARENT)
        }
        _ => (ink, detail, fill),
    };
    let screen = |local: Point| view.to_screen(fixture.to_world(local));
    let (hw, hd) = (fixture.width / 2.0, fixture.depth / 2.0);
    let interior = !electrical && !fixture.kind.is_structural();
    if !interior {
        painter.add(Shape::convex_polygon(
            [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)]
                .iter()
                .map(|(x, y)| screen(Point::new(*x, *y)))
                .collect(),
            fill,
            Stroke::NONE,
        ));
    }
    let fine = view.pixels(fixture.width.min(fixture.depth)) >= 72.0;
    for shape in crate::domain::fixtures::symbol_with_detail(
        fixture.kind,
        fixture.width,
        fixture.depth,
        fine,
    ) {
        match shape {
            SymbolShape::Polyline {
                points,
                closed,
                dashed,
                weight,
            } => {
                let stroke = match weight {
                    Weight::Outline => Stroke::new(1.4, ink),
                    Weight::Detail => Stroke::new(0.9, detail),
                    Weight::Fine => Stroke::new(0.55, detail.gamma_multiply(0.75)),
                };
                let mut points: Vec<Pos2> = points.into_iter().map(screen).collect();
                if interior && closed && weight == Weight::Outline {
                    painter.add(Shape::convex_polygon(points.clone(), fill, Stroke::NONE));
                }
                if dashed {
                    if closed {
                        points.push(points[0]);
                    }
                    painter.extend(Shape::dashed_line(&points, stroke, 4.0, 3.0));
                } else if closed {
                    painter.add(Shape::closed_line(points, stroke));
                } else {
                    painter.add(Shape::line(points, stroke));
                }
            }
            SymbolShape::Circle {
                center,
                radius,
                weight,
            } => {
                let stroke = match weight {
                    Weight::Outline => Stroke::new(1.4, ink),
                    Weight::Detail => Stroke::new(0.9, detail),
                    Weight::Fine => Stroke::new(0.55, detail.gamma_multiply(0.75)),
                };
                if interior && weight == Weight::Outline {
                    painter.circle_filled(screen(center), view.pixels(radius), fill);
                }
                painter.circle_stroke(screen(center), view.pixels(radius), stroke);
            }
        }
    }
    if fixture.kind.needs_label() {
        let (x, y) = fixture.extent();
        let galley =
            painter.layout_no_wrap(fixture.kind.label().to_owned(), theme::regular(10.5), ink);
        let room = egui::vec2(view.pixels(x) - 6.0, view.pixels(y) - 4.0);
        if galley.size().x <= room.x && galley.size().y <= room.y {
            let center = view.to_screen(fixture.center);
            let backing = Rect::from_center_size(center, galley.size() + egui::vec2(6.0, 2.0));
            painter.rect_filled(
                backing,
                2.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 215),
            );
            painter.galley(center - galley.size() / 2.0, galley, ink);
        }
    }
}
