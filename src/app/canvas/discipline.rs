//! Discipline overlays: electrical, structure, foundations and plumbing
//! fixtures in their colours, and the ergonomics clearances.

use eframe::egui::{self, Color32, Painter, Rect, Shape, Stroke};

use crate::app::theme;
use crate::domain::model::Project;

use super::{ViewTransform, paint_fixture};

#[derive(Clone, Copy, PartialEq)]
pub enum FixtureTone {
    Normal,
    Hovered,
    Selected,
    Ghost,
}

/// Draws a fixture's plan symbol (see `domain::fixtures`).
/// Draws only the electrical layer (devices and tags), for the electrical
/// plan view that fades everything else.
fn paint_electrical_overlay(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    selected: &dyn Fn(u64) -> bool,
) {
    if !project.category_visible(crate::domain::layers::LayerCategory::Electrical) {
        return;
    }
    for fixture in project.electrical_devices() {
        let tone = if selected(fixture.id) {
            FixtureTone::Selected
        } else {
            FixtureTone::Normal
        };
        paint_fixture(painter, view, fixture, tone);
    }
    super::labels::paint_conduits(painter, view, project);
    super::labels::paint_electrical_tags(painter, view, project);
}

/// Draws only the superstructure (`Planta de Forma`: columns, upper beams,
/// slabs, concentrated loads, roofs and timber posts/beams — no footings,
/// baldrames or balance beams) on top of the faded architectural underlay.
fn paint_structure_overlay(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    selected: &dyn Fn(u64) -> bool,
) {
    if !project.category_visible(crate::domain::layers::LayerCategory::Structure) {
        return;
    }
    for fixture in project
        .fixtures
        .iter()
        .filter(|f| crate::domain::discipline::Discipline::Structure.allows_fixture(f.kind))
    {
        let tone = if selected(fixture.id) {
            FixtureTone::Selected
        } else {
            FixtureTone::Normal
        };
        paint_fixture(painter, view, fixture, tone);
    }
    super::labels::paint_superstructure(painter, view, project);
    super::sim_overlay::paint_simulation_overlay(painter, view, project);
}

/// Draws only the foundations (`Planta de Fundações`: columns, isolated
/// footings, balance beams, baldrames, strip footings and timber post bases —
/// no upper beams, slabs, roofs or concentrated loads) on top of the faded
/// architectural underlay.
fn paint_foundations_overlay(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    selected: &dyn Fn(u64) -> bool,
) {
    if !project.category_visible(crate::domain::layers::LayerCategory::Structure) {
        return;
    }
    for fixture in project
        .fixtures
        .iter()
        .filter(|f| crate::domain::discipline::Discipline::Foundations.allows_fixture(f.kind))
    {
        let tone = if selected(fixture.id) {
            FixtureTone::Selected
        } else {
            FixtureTone::Normal
        };
        paint_fixture(painter, view, fixture, tone);
    }
    super::labels::paint_foundations(painter, view, project);
    super::sim_overlay::paint_simulation_overlay(painter, view, project);
}

/// Draws only sanitary/plumbing fixtures on top of the faded architecture
/// (prepared view for hydraulic installations #72).
fn paint_plumbing_overlay(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    selected: &dyn Fn(u64) -> bool,
) {
    if !project.category_visible(crate::domain::layers::LayerCategory::Fixtures) {
        return;
    }
    for fixture in project.fixtures.iter().filter(|f| f.kind.is_plumbing()) {
        let tone = if selected(fixture.id) {
            FixtureTone::Selected
        } else {
            FixtureTone::Normal
        };
        paint_fixture(painter, view, fixture, tone);
    }
}

/// Draws the focused discipline's overlay on top of a faded architectural veil.
pub fn paint_discipline_overlay(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    discipline: crate::domain::discipline::Discipline,
    selected: &dyn Fn(u64) -> bool,
) {
    use crate::domain::discipline::Discipline;
    match discipline {
        Discipline::Electrical => paint_electrical_overlay(painter, view, project, selected),
        Discipline::Structure => paint_structure_overlay(painter, view, project, selected),
        Discipline::Foundations => paint_foundations_overlay(painter, view, project, selected),
        Discipline::Plumbing => paint_plumbing_overlay(painter, view, project, selected),
        Discipline::All | Discipline::Architecture => {}
    }
}

/// Paints furniture functional clearance zones and circulation/interference alerts.
pub fn paint_ergonomics_overlay(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    min_passage_m: f64,
) {
    use crate::domain::ergonomics::{IssueSeverity, analyze_ergonomics};
    let report = analyze_ergonomics(project, min_passage_m);
    let zone_fill = Color32::from_rgba_unmultiplied(38, 152, 122, 34);
    let zone_border = Stroke::new(1.1, Color32::from_rgba_unmultiplied(28, 128, 102, 165));
    for zone in &report.clearance_zones {
        let rect = Rect::from_two_pos(view.to_screen(zone.min), view.to_screen(zone.max));
        painter.rect_filled(rect, 0.0, zone_fill);
        let pts = [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
            rect.left_top(),
        ];
        painter.extend(Shape::dashed_line(&pts, zone_border, 5.0, 4.0));
    }
    for issue in &report.issues {
        let pos = view.to_screen(issue.location);
        let (fill, glyph) = match issue.severity {
            IssueSeverity::Error => (theme::DANGER, "!"),
            IssueSeverity::Warning => (theme::WARNING, "▲"),
        };
        painter.circle(
            pos,
            10.0,
            fill.gamma_multiply(0.92),
            Stroke::new(1.5, Color32::WHITE),
        );
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            glyph,
            theme::strong(11.0),
            Color32::WHITE,
        );
    }
}
