use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use crate::app::theme;
use crate::domain::{
    annotations::{self, centimetres},
    fixtures::{Fixture, SymbolShape, Weight},
    geometry::{GRID_METERS, Point, WALL_THICKNESS_METERS, format_meters},
    model::{OpeningKind, Project, Wall},
    wall_reference::WallReference,
};

use super::Selection;

mod drafts;
mod drawings;
mod labels;
pub mod overlay;
mod sim_overlay;
pub use drafts::{paint_fragment_ghost, paint_modify_guide, paint_polygon_draft};
pub use drawings::paint_draw_draft;

pub fn paint_markup(
    painter: &Painter,
    view: ViewTransform,
    markup: &crate::domain::markups::Markup,
    label: bool,
) {
    use crate::domain::markups::MarkupKind;
    if markup.validate().is_err() {
        return;
    }
    let [r, g, b] = markup.color;
    let ink = Color32::from_rgb(r, g, b).gamma_multiply(if markup.resolved { 0.55 } else { 1.0 });
    let shape = markup.shape();
    let path: Vec<_> = shape
        .outline()
        .into_iter()
        .map(|p| view.to_screen(p))
        .collect();
    if matches!(markup.kind, MarkupKind::Rectangle | MarkupKind::Circle) {
        painter.add(Shape::convex_polygon(
            path.clone(),
            ink.gamma_multiply(0.10),
            Stroke::NONE,
        ));
    }
    painter.add(Shape::line(path, Stroke::new(2.2, ink)));
    if markup.kind == MarkupKind::Arrow {
        let end = view.to_screen(markup.points[1]);
        let start = view.to_screen(markup.points[0]);
        let along = (end - start).normalized();
        let across = egui::vec2(-along.y, along.x);
        painter.add(Shape::convex_polygon(
            vec![
                end,
                end - along * 13.0 + across * 5.5,
                end - along * 13.0 - across * 5.5,
            ],
            ink,
            Stroke::NONE,
        ));
    }
    if label {
        let anchor = view.to_screen(markup.points[0]);
        let text = format!(
            "{}{}",
            markup.label(),
            if markup.resolved { " ✓" } else { "" }
        );
        let galley = painter.layout_no_wrap(text, theme::strong(12.0), Color32::WHITE);
        let rect = Rect::from_min_size(
            anchor + egui::vec2(3.0, -23.0),
            galley.size() + egui::vec2(10.0, 6.0),
        );
        painter.rect_filled(rect, 4.0, ink);
        painter.galley(rect.min + egui::vec2(5.0, 3.0), galley, Color32::WHITE);
    }
}

pub fn paint_markups(painter: &Painter, view: ViewTransform, project: &Project) {
    for m in &project.markups {
        if project.markup_shown(m) {
            paint_markup(painter, view, m, true);
        }
    }
}
use labels::{paint_preview_measure, paint_room_labels, paint_terrain_label, paint_wall_labels};

#[derive(Clone, Copy)]
pub struct ViewTransform {
    pub rect: Rect,
    pub scale: f32,
    pub pan: Vec2,
}

impl ViewTransform {
    pub fn to_screen(self, point: Point) -> Pos2 {
        let scale = f64::from(self.scale);
        self.rect.center()
            + self.pan
            + egui::vec2((point.x * scale) as f32, (point.y * scale) as f32)
    }

    pub fn to_world(self, position: Pos2) -> Point {
        let offset = position - self.rect.center() - self.pan;
        Point::new(self.metres(offset.x), self.metres(offset.y))
    }

    /// A length on the plan, in screen pixels.
    pub fn pixels(self, metres: f64) -> f32 {
        (metres * f64::from(self.scale)) as f32
    }

    /// A length on screen, in metres on the plan.
    pub fn metres(self, pixels: f32) -> f64 {
        f64::from(pixels) / f64::from(self.scale)
    }
}

#[derive(Clone, Copy)]
pub enum Preview {
    Wall(Point, Point),
    Room(Point, Point),
    Terrain(Point, Point),
}

/// What the canvas shows besides the project itself.
#[derive(Clone, Copy, Default)]
pub struct PaintOptions<'a> {
    pub selection: Option<Selection>,
    /// The other selected elements, when several are.
    pub extra_selection: &'a [Selection],
    /// The element under the pointer, highlighted before it is picked.
    pub hover: Option<Selection>,
    /// What a selection box being drawn would pick, highlighted like hover.
    pub pending: &'a [Selection],
    pub preview: Option<Preview>,
    pub show_grid: bool,
    /// The fixture about to be placed, drawn under the pointer.
    pub ghost: Option<Fixture>,
    /// Dimension chains around the building.
    pub dimensions: bool,
    /// Active reference line when previewing walls or rooms.
    pub wall_reference: WallReference,
    /// Loaded GPU textures for visible external image/PDF underlays.
    pub underlay_textures: &'a [(u64, egui::TextureId)],
}

/// Door and window type tags ("P1 80/210") beside each opening: on the side
/// a door does not swing to, or past its leaf when that side is taken. A tag
/// that would cover another label is left out until the view is closer.
fn paint_frame_tags(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    occupied: &mut Vec<Rect>,
) {
    if view.scale < 24.0 {
        return;
    }
    let types = annotations::frame_types(project);
    for opening in &project.openings {
        let (Some(frame), Some(wall)) = (
            annotations::frame_of(&types, opening),
            project.walls.iter().find(|wall| wall.id == opening.wall_id),
        ) else {
            continue;
        };
        let length = wall.length().max(1e-4);
        let along = Point::new(
            (wall.end.x - wall.start.x) / length,
            (wall.end.y - wall.start.y) / length,
        );
        let mut normal = Point::new(-along.y, along.x);
        if opening.kind.is_door() && !opening.flip_side {
            normal = Point::new(-normal.x, -normal.y);
        }
        let center = Point::new(
            wall.start.x + (wall.end.x - wall.start.x) * opening.t,
            wall.start.y + (wall.end.y - wall.start.y) * opening.t,
        );
        let galley = painter.layout_no_wrap(frame.tag(), theme::strong(10.0), theme::ACCENT_TEXT);
        let size = galley.size() + egui::vec2(8.0, 2.0);
        let across = |direction: Point| {
            view.metres(
                direction.x.abs() as f32 * size.x / 2.0 + direction.y.abs() as f32 * size.y / 2.0,
            )
        };
        let slide = opening.width() / 2.0 + across(along) + view.metres(4.0);
        let rect_at = |side: f64, beyond: f64, shift: f64| {
            let offset = wall.thickness() / 2.0 + beyond + view.metres(4.0) + across(normal);
            Rect::from_center_size(
                view.to_screen(Point::new(
                    center.x + normal.x * offset * side + along.x * shift,
                    center.y + normal.y * offset * side + along.y * shift,
                )),
                size,
            )
        };
        let leaf = if opening.kind.is_door() {
            opening.width()
        } else {
            0.0
        };
        let candidates = [
            rect_at(1.0, 0.0, 0.0),
            rect_at(-1.0, leaf, 0.0),
            rect_at(1.0, 0.0, slide),
            rect_at(1.0, 0.0, -slide),
            rect_at(-1.0, 0.0, slide),
            rect_at(-1.0, 0.0, -slide),
        ];
        let Some(rect) = candidates
            .into_iter()
            .find(|rect| !occupied.iter().any(|taken| taken.intersects(*rect)))
        else {
            continue;
        };
        occupied.push(rect.expand(2.0));
        painter.rect(
            rect,
            3.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 230),
            Stroke::new(0.8, theme::ACCENT_RING),
            egui::StrokeKind::Inside,
        );
        painter.galley(rect.min + egui::vec2(4.0, 1.0), galley, theme::ACCENT_TEXT);
    }
}

/// Dimension chains in centimetres above and to the left of the building:
/// one with every wall face, one with the total. Returns the bands they
/// take, so labels keep clear of them.
fn paint_dimensions(painter: &Painter, view: ViewTransform, project: &Project) -> Vec<Rect> {
    let Some(dims) = annotations::building_dimensions(project) else {
        return Vec::new();
    };
    let stroke = Stroke::new(1.0, theme::MUTED);
    let tick = Stroke::new(1.4, theme::TEXT_2);
    let font = theme::regular(10.5);
    let corner = view.to_screen(dims.min);
    let total = [dims.min, dims.max];

    // Along the top, reading left to right.
    for (row, stations) in [
        (corner.y - 24.0, dims.xs.clone()),
        (corner.y - 46.0, vec![total[0].x, total[1].x]),
    ] {
        let xs: Vec<f32> = stations
            .iter()
            .map(|x| view.to_screen(Point::new(*x, 0.0)).x)
            .collect();
        if let (Some(&first), Some(&last)) = (xs.first(), xs.last()) {
            painter.line_segment([Pos2::new(first, row), Pos2::new(last, row)], stroke);
        }
        for x in &xs {
            painter.line_segment([Pos2::new(*x, row - 5.0), Pos2::new(*x, row + 8.0)], stroke);
            painter.line_segment(
                [Pos2::new(x - 3.5, row + 3.5), Pos2::new(x + 3.5, row - 3.5)],
                tick,
            );
        }
        for (pair, span) in stations.windows(2).zip(xs.windows(2)) {
            let galley =
                painter.layout_no_wrap(centimetres(pair[1] - pair[0]), font.clone(), theme::TEXT_2);
            if galley.size().x + 4.0 <= span[1] - span[0] {
                let at = Pos2::new((span[0] + span[1]) / 2.0, row - 3.0);
                painter.galley(
                    at - egui::vec2(galley.size().x / 2.0, galley.size().y),
                    galley,
                    theme::TEXT_2,
                );
            }
        }
    }

    // Down the left side, reading bottom to top.
    for (column, stations) in [
        (corner.x - 24.0, dims.ys.clone()),
        (corner.x - 46.0, vec![total[0].y, total[1].y]),
    ] {
        let ys: Vec<f32> = stations
            .iter()
            .map(|y| view.to_screen(Point::new(0.0, *y)).y)
            .collect();
        if let (Some(&first), Some(&last)) = (ys.first(), ys.last()) {
            painter.line_segment([Pos2::new(column, first), Pos2::new(column, last)], stroke);
        }
        for y in &ys {
            painter.line_segment(
                [Pos2::new(column - 5.0, *y), Pos2::new(column + 8.0, *y)],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(column - 3.5, y + 3.5),
                    Pos2::new(column + 3.5, y - 3.5),
                ],
                tick,
            );
        }
        for (pair, span) in stations.windows(2).zip(ys.windows(2)) {
            let galley =
                painter.layout_no_wrap(centimetres(pair[1] - pair[0]), font.clone(), theme::TEXT_2);
            let (width, height) = (galley.size().x, galley.size().y);
            if width + 4.0 <= span[1] - span[0] {
                let middle = (span[0] + span[1]) / 2.0;
                let at = Pos2::new(column - 3.0 - height, middle + width / 2.0);
                painter.add(
                    egui::epaint::TextShape::new(at, galley, theme::TEXT_2)
                        .with_angle(-std::f32::consts::FRAC_PI_2),
                );
            }
        }
    }
    let far = view.to_screen(dims.max);
    vec![
        Rect::from_min_max(
            Pos2::new(corner.x, corner.y - 62.0),
            Pos2::new(far.x, corner.y - 16.0),
        ),
        Rect::from_min_max(
            Pos2::new(corner.x - 62.0, corner.y),
            Pos2::new(corner.x - 16.0, far.y),
        ),
    ]
}

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
pub fn paint_electrical_overlay(
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
    labels::paint_conduits(painter, view, project);
    labels::paint_electrical_tags(painter, view, project);
}

/// Draws only the superstructure (`Planta de Forma`: columns, upper beams,
/// slabs, concentrated loads, roofs and timber posts/beams — no footings,
/// baldrames or balance beams) on top of the faded architectural underlay.
pub fn paint_structure_overlay(
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
    labels::paint_superstructure(painter, view, project);
    sim_overlay::paint_simulation_overlay(painter, view, project);
}

/// Draws only the foundations (`Planta de Fundações`: columns, isolated
/// footings, balance beams, baldrames, strip footings and timber post bases —
/// no upper beams, slabs, roofs or concentrated loads) on top of the faded
/// architectural underlay.
pub fn paint_foundations_overlay(
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
    labels::paint_foundations(painter, view, project);
    sim_overlay::paint_simulation_overlay(painter, view, project);
}

/// Draws only sanitary/plumbing fixtures on top of the faded architecture
/// (prepared view for hydraulic installations #72).
pub fn paint_plumbing_overlay(
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
        crate::domain::fixtures::FixtureKind::Footing if tone == FixtureTone::Normal => {
            (theme::STRUCTURE, theme::STRUCTURE, Color32::TRANSPARENT)
        }
        crate::domain::fixtures::FixtureKind::Footing => (ink, detail, Color32::TRANSPARENT),
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

/// A measured distance: a dimension line with architectural ticks and its
/// length (plus Δx and Δy when it runs at an angle).
pub fn paint_measurement(painter: &Painter, view: ViewTransform, start: Point, end: Point) {
    let (a, b) = (view.to_screen(start), view.to_screen(end));
    let length = a.distance(b);
    let color = theme::ACCENT_STRONG;
    painter.line_segment([a, b], Stroke::new(1.6, color));
    for point in [a, b] {
        painter.circle_filled(point, 2.6, color);
    }
    if length < 1.0 {
        return;
    }
    let along = (b - a) / length;
    let normal = egui::vec2(-along.y, along.x);
    let slash = (along + normal).normalized() * 6.0;
    for point in [a, b] {
        painter.line_segment([point - slash, point + slash], Stroke::new(2.0, color));
    }
    let (dx, dy) = ((end.x - start.x).abs(), (end.y - start.y).abs());
    let mut text = format_meters(start.distance(end));
    if dx > 0.005 && dy > 0.005 {
        text.push_str(&format!("\nΔx {dx:.2} · Δy {dy:.2}").replace('.', ","));
    }
    let galley = painter.layout(text, theme::strong(12.0), color, f32::INFINITY);
    let center = a.lerp(b, 0.5) - normal * (galley.size().y / 2.0 + 10.0);
    let badge = Rect::from_center_size(center, galley.size() + egui::vec2(12.0, 6.0));
    painter.rect(
        badge,
        4.0,
        Color32::WHITE,
        Stroke::new(1.0, theme::ACCENT_RING),
        egui::StrokeKind::Inside,
    );
    painter.galley(badge.min + egui::vec2(6.0, 3.0), galley, color);
}

/// A property line: long dash, short dash.
fn dash_dot(painter: &Painter, points: &[Pos2], stroke: Stroke) {
    painter.extend(Shape::dashed_line_with_offset(
        points,
        stroke,
        &[14.0, 2.5],
        &[4.0, 4.0],
        0.0,
    ));
}

/// Which elements are drawn as selected and which as hovered.
#[derive(Clone, Copy)]
pub struct Marks<'a> {
    selection: Option<Selection>,
    extra: &'a [Selection],
    hover: Option<Selection>,
    pending: &'a [Selection],
}

impl Marks<'_> {
    pub fn selected(&self, item: Selection) -> bool {
        self.selection == Some(item) || self.extra.contains(&item)
    }

    pub fn hovered(&self, item: Selection) -> bool {
        self.hover == Some(item) || self.pending.contains(&item)
    }
}

pub fn paint(painter: &Painter, view: ViewTransform, project: &Project, options: PaintOptions<'_>) {
    let PaintOptions {
        selection,
        extra_selection,
        hover,
        pending,
        preview,
        show_grid,
        ghost,
        dimensions,
        wall_reference,
        underlay_textures,
    } = options;
    let marks = Marks {
        selection,
        extra: extra_selection,
        hover,
        pending,
    };
    painter.rect_filled(view.rect, 0.0, theme::CANVAS);
    if show_grid {
        paint_grid(painter, view);
    }

    if project.category_visible(crate::domain::layers::LayerCategory::Terrain)
        && let Some(terrain) = &project.terrain
    {
        let outline = terrain.outline();
        let screen_outline: Vec<Pos2> =
            outline.iter().map(|point| view.to_screen(*point)).collect();
        let selected = marks.selected(Selection::Terrain(terrain.id));
        let hovered = marks.hovered(Selection::Terrain(terrain.id));
        let fill = if selected {
            theme::TERRAIN_FILL_SELECTED
        } else {
            theme::TERRAIN_FILL
        };
        for triangle in crate::domain::geometry::triangulate_polygon(&outline) {
            painter.add(Shape::convex_polygon(
                triangle
                    .into_iter()
                    .map(|point| view.to_screen(point))
                    .collect(),
                fill,
                Stroke::NONE,
            ));
        }
        let (width, color) = if selected {
            (2.2, theme::SELECTION)
        } else if hovered {
            (2.0, theme::HOVER)
        } else {
            (1.6, theme::TERRAIN_LINE)
        };
        let mut closed = screen_outline.clone();
        if let Some(first) = closed.first().copied() {
            closed.push(first);
        }
        dash_dot(painter, &closed, Stroke::new(width, color));
        for corner in screen_outline {
            painter.circle_filled(corner, 3.0, color);
        }
        for lot in &project.lots {
            let mut points: Vec<Pos2> = lot
                .vertices
                .iter()
                .map(|point| view.to_screen(*point))
                .collect();
            if let Some(first) = points.first().copied() {
                points.push(first);
            }
            painter.extend(Shape::dashed_line(
                &points,
                Stroke::new(1.2, theme::TERRAIN_LINE),
                7.0,
                4.0,
            ));
        }
        for (a, b) in terrain.setback_segments(project.setbacks) {
            painter.extend(Shape::dashed_line(
                &[view.to_screen(a), view.to_screen(b)],
                Stroke::new(1.5, theme::WARNING),
                9.0,
                5.0,
            ));
        }
    }

    super::ui::paint_image_underlays(painter, view, project, underlay_textures);
    paint_floor_underlay(painter, view, project);

    if project.category_visible(crate::domain::layers::LayerCategory::Rooms) {
        for room in &project.rooms {
            let selected = marks.selected(Selection::Room(room.id));
            let hovered = marks.hovered(Selection::Room(room.id));
            let fill_color = if selected {
                theme::ROOM_FILL_SELECTED
            } else if hovered {
                theme::ROOM_FILL_HOVER
            } else {
                theme::ROOM_FILL
            };

            if let Some(poly) = project.room_polygon(room) {
                let fill = match room.hatch {
                    Some(style)
                        if !selected
                            && !hovered
                            && style.pattern == crate::domain::hatch::HatchPattern::Solid =>
                    {
                        let [r, g, b] = style.color;
                        Color32::from_rgba_unmultiplied(r, g, b, 175)
                    }
                    _ => fill_color,
                };
                let triangles = crate::domain::geometry::triangulate_polygon(&poly);
                for tri in triangles {
                    let p0 = view.to_screen(tri[0]);
                    let p1 = view.to_screen(tri[1]);
                    let p2 = view.to_screen(tri[2]);
                    painter.add(egui::Shape::convex_polygon(
                        vec![p0, p1, p2],
                        fill,
                        Stroke::NONE,
                    ));
                }
                if let Some(style) = room.hatch {
                    let [r, g, b] = style.color;
                    let stroke = Stroke::new(0.9, Color32::from_rgba_unmultiplied(r, g, b, 180));
                    for (a, b) in crate::domain::hatch::hatch_segments_in_polygon(&poly, &style) {
                        painter.line_segment([view.to_screen(a), view.to_screen(b)], stroke);
                    }
                }
            } else if let Some((min, max)) = project.room_bounds(room) {
                let rect = Rect::from_two_pos(view.to_screen(min), view.to_screen(max));
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(204, 63, 51, 120)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    // Paint walls as architectural double-line (hollow) walls in four passes so
    // L-, T-, X-junctions and collinear continuations merge cleanly without
    // crossing lines inside the wall cavity:
    //   1) outer dark contour (junction patches + wall runs),
    //   2) inner white hollow core (inset junction patches + inset wall runs),
    //   3) hovered / selected wall highlights (outer + inner),
    //   4) openings (jambs, doors, windows, passages) on top.
    if project.category_visible(crate::domain::layers::LayerCategory::Walls) {
        let base_wall_w = wall_screen_width(view.scale, project.masonry.thickness());
        let border_px = wall_border_width(base_wall_w);
        let draw_hollow = base_wall_w >= 2.0 * border_px + 1.0;

        struct WallRenderEntry<'a> {
            wall: &'a Wall,
            tone: WallTone,
            runs: Vec<(Pos2, Pos2)>,
            width: f32,
        }

        let visible_walls: Vec<WallRenderEntry<'_>> = project
            .walls
            .iter()
            .filter(|wall| project.is_wall_visible(wall))
            .map(|wall| {
                let in_room = |marked: &dyn Fn(Selection) -> bool| {
                    project.rooms.iter().any(|room| {
                        marked(Selection::Room(room.id)) && room.wall_ids.contains(&wall.id)
                    })
                };
                let tone = if marks.selected(Selection::Wall(wall.id))
                    || in_room(&|item| marks.selected(item))
                {
                    WallTone::Selected
                } else if marks.hovered(Selection::Wall(wall.id))
                    || in_room(&|item| marks.hovered(item))
                {
                    WallTone::Hovered
                } else {
                    WallTone::Normal
                };
                let width = wall_screen_width(view.scale, wall.thickness());
                let runs = wall_solid_runs(wall, project, view, width);
                WallRenderEntry {
                    wall,
                    tone,
                    runs,
                    width,
                }
            })
            .collect();

        // Pass 1: outer wall contour (junctions + segments).
        for junction in project.wall_junctions() {
            painter.add(Shape::convex_polygon(
                junction
                    .outline
                    .into_iter()
                    .map(|point| view.to_screen(point))
                    .collect(),
                theme::WALL,
                Stroke::NONE,
            ));
        }
        for entry in &visible_walls {
            for &(from, to) in &entry.runs {
                wall_run(painter, from, to, entry.width, theme::WALL);
            }
        }

        // Pass 2: inner hollow white core (inset junctions + inset segments)
        // and multilayer wall assemblies.
        if draw_hollow && view.scale > 1e-3 {
            let border_m = (border_px / view.scale) as f64;
            for junction in project.wall_junctions_with_inset(border_m) {
                painter.add(Shape::convex_polygon(
                    junction
                        .outline
                        .into_iter()
                        .map(|point| view.to_screen(point))
                        .collect(),
                    theme::WALL_FILL,
                    Stroke::NONE,
                ));
            }
        }
        for entry in &visible_walls {
            if let Some(assembly) = &entry.wall.assembly
                && entry.tone == WallTone::Normal
                && entry.width >= 6.0
                && assembly.total_thickness() > 1e-6
            {
                let a = view.to_screen(entry.wall.start);
                let b = view.to_screen(entry.wall.end);
                let direction = (b - a).normalized();
                let normal = egui::vec2(-direction.y, direction.x);
                for &(from, to) in &entry.runs {
                    paint_wall_assembly_segment(painter, from, to, normal, entry.width, assembly);
                }
            } else {
                for &(from, to) in &entry.runs {
                    wall_core_run(painter, from, to, entry.width, border_px, theme::WALL_FILL);
                }
            }
        }

        // Pass 3: hovered and selected walls keep the same double-line look
        // with blue contours and a soft tinted interior.
        for highlight_tone in [WallTone::Hovered, WallTone::Selected] {
            let (outer_color, inner_color) = match highlight_tone {
                WallTone::Selected => (theme::SELECTION, theme::WALL_FILL_SELECTED),
                WallTone::Hovered => (theme::HOVER, theme::WALL_FILL_HOVER),
                WallTone::Normal => continue,
            };
            let any_highlighted = visible_walls
                .iter()
                .any(|entry| entry.tone == highlight_tone);
            if !any_highlighted {
                continue;
            }
            for entry in &visible_walls {
                if entry.tone == highlight_tone {
                    for &(from, to) in &entry.runs {
                        wall_run(painter, from, to, entry.width, outer_color);
                    }
                }
            }
            for entry in &visible_walls {
                if entry.tone == highlight_tone {
                    for &(from, to) in &entry.runs {
                        wall_core_run(painter, from, to, entry.width, border_px, inner_color);
                    }
                }
            }
        }

        // Pass 4: openings (doors, windows, gates, passages and jambs).
        for entry in &visible_walls {
            paint_wall_openings(
                painter, view, entry.wall, project, entry.tone, marks, border_px,
            );
        }
    }

    {
        for fixture in &project.fixtures {
            if !project.category_visible(Project::fixture_category(fixture)) {
                continue;
            }
            let tone = if marks.selected(Selection::Fixture(fixture.id)) {
                FixtureTone::Selected
            } else if marks.hovered(Selection::Fixture(fixture.id)) {
                FixtureTone::Hovered
            } else {
                FixtureTone::Normal
            };
            paint_fixture(painter, view, fixture, tone);
        }
    }
    if project.category_visible(crate::domain::layers::LayerCategory::Structure) {
        labels::paint_structure(painter, view, project);
        sim_overlay::paint_simulation_overlay(painter, view, project);
    }
    if project.category_visible(crate::domain::layers::LayerCategory::Electrical) {
        labels::paint_conduits(painter, view, project);
        labels::paint_electrical_tags(painter, view, project);
    }
    drawings::paint_drawings(painter, view, project, marks);
    paint_markups(painter, view, project);

    let mut occupied = Vec::new();
    let show_dims =
        dimensions && project.category_visible(crate::domain::layers::LayerCategory::Dimensions);
    if show_dims {
        occupied.extend(paint_dimensions(painter, view, project));
        paint_associative_dimensions(painter, view, project);
    }

    if project.category_visible(crate::domain::layers::LayerCategory::Rooms) {
        paint_room_labels(painter, view, project, &mut occupied);
    }
    if show_dims && project.category_visible(crate::domain::layers::LayerCategory::Openings) {
        paint_frame_tags(painter, view, project, &mut occupied);
    }
    if project.category_visible(crate::domain::layers::LayerCategory::Walls) {
        paint_wall_labels(painter, view, project, selection, &mut occupied);
    }
    if project.category_visible(crate::domain::layers::LayerCategory::Annotations) {
        paint_notes(painter, view, project);
        paint_views(painter, view, project);
    }
    paint_circulations(painter, view, project);
    paint_terrain_label(painter, view, project, &mut occupied);

    if project.walls.is_empty()
        && project.rooms.is_empty()
        && project.terrain.is_none()
        && preview.is_none()
    {
        paint_empty_state(painter, view.rect);
    }

    if let Some(preview) = preview {
        paint_preview(painter, view, project, preview, wall_reference, &occupied);
    }
    if let Some(ghost) = &ghost {
        paint_fixture(painter, view, ghost, FixtureTone::Ghost);
    }
}

fn paint_empty_state(painter: &Painter, area: Rect) {
    let card = Rect::from_center_size(area.center(), egui::vec2(460.0, 132.0));
    if !area.contains_rect(card) {
        return;
    }
    let shadow = egui::Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(22, 50, 92, 30),
    };
    painter.add(shadow.as_shape(card, 10));
    painter.rect(
        card,
        10.0,
        theme::PANEL,
        Stroke::new(1.0, theme::BORDER),
        egui::StrokeKind::Inside,
    );
    let icon = Rect::from_center_size(
        Pos2::new(card.left() + 44.0, card.center().y - 14.0),
        egui::vec2(34.0, 34.0),
    );
    painter.rect_filled(icon.expand(6.0), 9.0, theme::ACCENT_SOFT);
    crate::app::ui::icons::paint(
        painter,
        icon,
        crate::app::ui::icons::Icon::Plan,
        theme::ACCENT_TEXT,
        theme::ACCENT,
    );
    let left = card.left() + 84.0;
    painter.text(
        Pos2::new(left, card.top() + 30.0),
        egui::Align2::LEFT_CENTER,
        "Comece seu projeto",
        theme::strong(16.0),
        theme::TEXT,
    );
    painter.text(
        Pos2::new(left, card.top() + 56.0),
        egui::Align2::LEFT_CENTER,
        "Crie um terreno com medidas exatas ou desenhe",
        theme::regular(13.0),
        theme::TEXT_2,
    );
    painter.text(
        Pos2::new(left, card.top() + 74.0),
        egui::Align2::LEFT_CENTER,
        "paredes e cômodos diretamente na grade.",
        theme::regular(13.0),
        theme::TEXT_2,
    );
    painter.text(
        Pos2::new(left, card.top() + 104.0),
        egui::Align2::LEFT_CENTER,
        "Terreno  T    ·    Parede  P    ·    Cômodo  C    ·    Ajuda  F1",
        theme::regular(11.5),
        theme::MUTED,
    );
}

fn paint_notes(painter: &Painter, view: ViewTransform, project: &Project) {
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

fn paint_views(painter: &Painter, view: ViewTransform, project: &Project) {
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

fn paint_circulations(painter: &Painter, view: ViewTransform, project: &Project) {
    let fill = Color32::from_rgba_unmultiplied(238, 244, 250, 210);
    let outline = Stroke::new(1.4, theme::WALL);
    let step_stroke = Stroke::new(1.0, theme::TEXT_2);
    let rail_stroke = Stroke::new(2.2, theme::ACCENT_TEXT);
    for elem in &project.circulations {
        let geom = elem.plan_geometry();
        for poly in &geom.polygons {
            let pts: Vec<Pos2> = poly.iter().map(|p| view.to_screen(*p)).collect();
            if pts.len() >= 3 {
                painter.add(Shape::convex_polygon(pts.clone(), fill, outline));
            }
        }
        for (a, b) in &geom.lines {
            painter.line_segment([view.to_screen(*a), view.to_screen(*b)], step_stroke);
        }
        for (a, b) in &geom.railing_lines {
            painter.line_segment([view.to_screen(*a), view.to_screen(*b)], rail_stroke);
        }
        if let Some((a, b)) = geom.arrow {
            let sa = view.to_screen(a);
            let sb = view.to_screen(b);
            painter.line_segment([sa, sb], Stroke::new(1.4, theme::ACCENT));
            let dir = (sb - sa).normalized();
            let perp = egui::vec2(-dir.y, dir.x);
            painter.add(Shape::convex_polygon(
                vec![sb, sb - dir * 8.0 + perp * 4.0, sb - dir * 8.0 - perp * 4.0],
                theme::ACCENT,
                Stroke::NONE,
            ));
        }
        if !geom.label.is_empty() {
            painter.text(
                view.to_screen(geom.label_pos),
                egui::Align2::CENTER_CENTER,
                &geom.label,
                theme::strong(10.0),
                theme::TEXT,
            );
        }
    }
}

/// Paints the floor directly below as a translucent architectural underlay reference.
fn paint_floor_underlay(painter: &Painter, view: ViewTransform, project: &Project) {
    let Some(underlay) = project.underlay_floor() else {
        return;
    };
    let ghost_wall = Color32::from_rgba_unmultiplied(110, 140, 175, 78);
    let ghost_line = Stroke::new(1.1, Color32::from_rgba_unmultiplied(100, 130, 165, 115));
    for wall in &underlay.data.walls {
        let a = view.to_screen(wall.start);
        let b = view.to_screen(wall.end);
        let w = wall_screen_width(view.scale, wall.thickness());
        wall_run(painter, a, b, w, ghost_wall);
        painter.extend(Shape::dashed_line(&[a, b], ghost_line, 6.0, 4.0));
    }
    for elem in &underlay.data.circulations {
        let geom = elem.plan_geometry();
        for poly in &geom.polygons {
            let mut pts: Vec<Pos2> = poly.iter().map(|p| view.to_screen(*p)).collect();
            if let Some(first) = pts.first().copied() {
                pts.push(first);
                painter.extend(Shape::dashed_line(&pts, ghost_line, 5.0, 4.0));
            }
        }
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

fn paint_associative_dimensions(painter: &Painter, view: ViewTransform, project: &Project) {
    use crate::domain::dimensions::ResolvedDimension;
    let line_stroke = Stroke::new(1.0, theme::TEXT_2);
    let tick_stroke = Stroke::new(1.5, theme::WALL);
    for dim in &project.dimensions {
        if !dim.visible {
            continue;
        }
        let Some(resolved) = project.resolve_dimension(dim) else {
            continue;
        };
        match &resolved {
            ResolvedDimension::Linear {
                p1,
                p2,
                dim_p1,
                dim_p2,
                label,
                ..
            } => {
                let s1 = view.to_screen(*p1);
                let s2 = view.to_screen(*p2);
                let d1 = view.to_screen(*dim_p1);
                let d2 = view.to_screen(*dim_p2);
                if s1.distance(d1) > 1.5 {
                    painter.line_segment([s1, d1], line_stroke);
                }
                if s2.distance(d2) > 1.5 {
                    painter.line_segment([s2, d2], line_stroke);
                }
                painter.line_segment([d1, d2], line_stroke);
                for end in [d1, d2] {
                    painter.line_segment(
                        [end + egui::vec2(-3.5, 3.5), end + egui::vec2(3.5, -3.5)],
                        tick_stroke,
                    );
                }
                let mid = d1.lerp(d2, 0.5);
                let galley =
                    painter.layout_no_wrap(label.clone(), theme::regular(11.0), theme::TEXT);
                let pill = Rect::from_center_size(
                    mid + egui::vec2(0.0, -9.0),
                    galley.size() + egui::vec2(6.0, 2.0),
                );
                painter.rect_filled(
                    pill,
                    3.0,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 220),
                );
                painter.galley(pill.min + egui::vec2(3.0, 1.0), galley, theme::TEXT);
            }
            ResolvedDimension::Angular {
                vertex,
                start_angle,
                sweep,
                radius,
                label,
                ..
            } => {
                let arc_pts: Vec<Pos2> = resolved
                    .arc_points(20)
                    .into_iter()
                    .map(|p| view.to_screen(p))
                    .collect();
                if arc_pts.len() >= 2 {
                    painter.add(egui::Shape::line(arc_pts, line_stroke));
                }
                let mid_angle = start_angle + sweep * 0.5;
                let text_world =
                    crate::domain::drawing::point_at_angle(*vertex, radius + 0.18, mid_angle);
                let text_pos = view.to_screen(text_world);
                painter.text(
                    text_pos,
                    egui::Align2::CENTER_CENTER,
                    label,
                    theme::regular(11.0),
                    theme::TEXT,
                );
            }
        }
    }
}

fn paint_preview(
    painter: &Painter,
    view: ViewTransform,
    project: &Project,
    preview: Preview,
    reference: WallReference,
    occupied: &[Rect],
) {
    let valid_color = theme::ACCENT;
    let invalid_color = theme::DANGER;
    let thickness = project.masonry.thickness();
    match preview {
        Preview::Wall(ref_a, ref_b) => {
            let (a, b) = reference.offset_segment(ref_a, ref_b, thickness);
            let valid = a.distance(b) >= 0.24
                && project
                    .terrain
                    .as_ref()
                    .is_none_or(|t| t.contains(a) && t.contains(b));
            let color = if valid { valid_color } else { invalid_color };
            painter.line_segment(
                [view.to_screen(a), view.to_screen(b)],
                Stroke::new(
                    wall_screen_width(view.scale, thickness),
                    color.gamma_multiply(0.85),
                ),
            );
            if reference != WallReference::Center {
                overlay::paint_snap_guide(painter, view.to_screen(ref_a), view.to_screen(ref_b));
            }
            painter.circle_filled(view.to_screen(ref_a), 3.5, color);
            painter.circle_filled(view.to_screen(ref_b), 4.5, color);
            let midpoint = view.to_screen(ref_a).lerp(view.to_screen(ref_b), 0.5);
            let label = if reference == WallReference::Center {
                format_meters(ref_a.distance(ref_b))
            } else {
                format!(
                    "{} ({})",
                    format_meters(ref_a.distance(ref_b)),
                    reference.short_label()
                )
            };
            paint_preview_measure(
                painter,
                midpoint + egui::vec2(0.0, -22.0),
                view.rect,
                view.to_screen(ref_b),
                &label,
                view.rect,
                occupied,
            );
        }
        Preview::Room(ref_a, ref_b) => {
            let w = (ref_b.x - ref_a.x).abs();
            let h = (ref_b.y - ref_a.y).abs();
            let shifted = reference.offset_rect(ref_a, ref_b, thickness);
            let valid = w >= 0.5
                && h >= 0.5
                && shifted.is_some_and(|(min, max)| {
                    project
                        .terrain
                        .as_ref()
                        .is_none_or(|t| t.contains(min) && t.contains(max))
                });
            let color = if valid { valid_color } else { invalid_color };
            let (draw_a, draw_b) = shifted.unwrap_or((ref_a, ref_b));
            let rect = Rect::from_two_pos(view.to_screen(draw_a), view.to_screen(draw_b));
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.10));
            let corners = [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ];
            for index in 0..4 {
                painter.line_segment(
                    [corners[index], corners[(index + 1) % 4]],
                    Stroke::new(
                        wall_screen_width(view.scale, thickness),
                        color.gamma_multiply(0.85),
                    ),
                );
            }
            if reference != WallReference::Center {
                let ref_rect = Rect::from_two_pos(view.to_screen(ref_a), view.to_screen(ref_b));
                let ref_corners = [
                    ref_rect.left_top(),
                    ref_rect.right_top(),
                    ref_rect.right_bottom(),
                    ref_rect.left_bottom(),
                ];
                for index in 0..4 {
                    overlay::paint_snap_guide(
                        painter,
                        ref_corners[index],
                        ref_corners[(index + 1) % 4],
                    );
                }
            }
            let room_label = if reference == WallReference::Center {
                format!(
                    "{} × {} · {:.2} m²",
                    format_meters(w),
                    format_meters(h),
                    w * h
                )
            } else {
                format!(
                    "{} × {} ({}) · {:.2} m²",
                    format_meters(w),
                    format_meters(h),
                    reference.short_label(),
                    w * h
                )
            }
            .replace('.', ",");
            paint_preview_measure(
                painter,
                rect.center(),
                rect.shrink(wall_stroke_width(view.scale) / 2.0 + 4.0),
                view.to_screen(ref_b),
                &room_label,
                view.rect,
                occupied,
            );
        }
        Preview::Terrain(a, b) => {
            let w = (b.x - a.x).abs();
            let h = (b.y - a.y).abs();
            let valid = w >= 0.25 && h >= 0.25;
            let color = if valid {
                theme::TERRAIN_LINE
            } else {
                invalid_color
            };
            let rect = Rect::from_two_pos(view.to_screen(a), view.to_screen(b));
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.10));
            dash_dot(
                painter,
                &[
                    rect.left_top(),
                    rect.right_top(),
                    rect.right_bottom(),
                    rect.left_bottom(),
                    rect.left_top(),
                ],
                Stroke::new(2.0, color),
            );
            paint_preview_measure(
                painter,
                rect.center(),
                rect.shrink(4.0),
                view.to_screen(b),
                &format!(
                    "{} × {} · {:.2} m²",
                    format_meters(w),
                    format_meters(h),
                    w * h
                )
                .replace('.', ","),
                view.rect,
                occupied,
            );
        }
    }
}

fn paint_grid(painter: &Painter, view: ViewTransform) {
    let step = if view.scale < 8.0 {
        5.0
    } else if view.scale < 20.0 {
        1.0
    } else {
        GRID_METERS
    };
    let major_every = if step >= 5.0 { 25.0 } else { 1.0 };
    let first = view.to_world(view.rect.left_top());
    let last = view.to_world(view.rect.right_bottom());
    let start_x = (first.x / step).floor() as i32;
    let end_x = (last.x / step).ceil() as i32;
    let start_y = (first.y / step).floor() as i32;
    let end_y = (last.y / step).ceil() as i32;
    let color = |value: f64| {
        if (value / major_every).fract().abs() < 0.001
            || (value / major_every).fract().abs() > 0.999
        {
            theme::GRID_MAJOR
        } else {
            theme::GRID_MINOR
        }
    };
    for index in start_x..=end_x {
        let value = f64::from(index) * step;
        let x = view.to_screen(Point::new(value, 0.0)).x;
        painter.line_segment(
            [
                Pos2::new(x, view.rect.top()),
                Pos2::new(x, view.rect.bottom()),
            ],
            Stroke::new(1.0, color(value)),
        );
    }
    for index in start_y..=end_y {
        let value = f64::from(index) * step;
        let y = view.to_screen(Point::new(0.0, value)).y;
        painter.line_segment(
            [
                Pos2::new(view.rect.left(), y),
                Pos2::new(view.rect.right(), y),
            ],
            Stroke::new(1.0, color(value)),
        );
    }
}

#[derive(Clone, Copy, PartialEq)]
enum WallTone {
    Normal,
    Hovered,
    Selected,
}

/// Paints a straight run of wall from `from` to `to` with square ends, so
/// walls meeting at a corner close it without a notch.
fn wall_run(painter: &Painter, from: Pos2, to: Pos2, width: f32, color: Color32) {
    let along = to - from;
    if along.length() < 0.01 {
        return;
    }
    let direction = along.normalized();
    let normal = egui::vec2(-direction.y, direction.x) * (width / 2.0);
    painter.add(Shape::convex_polygon(
        vec![from + normal, to + normal, to - normal, from - normal],
        color,
        Stroke::NONE,
    ));
}

/// Paints the hollow architectural interior core of a wall segment inset by
/// `border` on all four sides, leaving crisp double-line outer contours and
/// clean end caps / jambs.
fn wall_core_run(painter: &Painter, from: Pos2, to: Pos2, width: f32, border: f32, fill: Color32) {
    let inner_width = width - 2.0 * border;
    if inner_width < 1.0 {
        return;
    }
    let along = to - from;
    let len = along.length();
    if len <= 2.0 * border + 0.5 {
        return;
    }
    let dir = along / len;
    wall_run(
        painter,
        from + dir * border,
        to - dir * border,
        inner_width,
        fill,
    );
}

/// Computes the screen-space solid masonry runs of a wall between its openings,
/// extending by half the wall thickness at wall endpoints so corners close.
fn wall_solid_runs(
    wall: &Wall,
    project: &Project,
    view: ViewTransform,
    wall_width: f32,
) -> Vec<(Pos2, Pos2)> {
    let a = view.to_screen(wall.start);
    let b = view.to_screen(wall.end);
    let screen_length = a.distance(b);
    if screen_length < 0.01 {
        return Vec::new();
    }
    let direction = (b - a) / screen_length;
    let cap = direction * (wall_width / 2.0);
    let mut openings: Vec<_> = project
        .openings
        .iter()
        .filter(|opening| opening.wall_id == wall.id)
        .collect();
    openings.sort_by(|left, right| left.t.total_cmp(&right.t));

    let mut runs = Vec::with_capacity(openings.len() + 1);
    let mut cursor = 0.0;
    let mut run_start = a - cap;
    for opening in openings {
        let half = view.pixels(opening.width()) / 2.0;
        let center = opening.t as f32 * screen_length;
        let left = center - half;
        let right = center + half;
        if left > cursor {
            runs.push((run_start, a + direction * left));
        }
        cursor = right;
        run_start = a + direction * right;
    }
    if cursor < screen_length {
        runs.push((run_start, b + cap));
    }
    runs
}

fn paint_wall_openings(
    painter: &Painter,
    view: ViewTransform,
    wall: &Wall,
    project: &Project,
    tone: WallTone,
    marks: Marks<'_>,
    border_px: f32,
) {
    let a = view.to_screen(wall.start);
    let b = view.to_screen(wall.end);
    let screen_length = a.distance(b);
    if screen_length < 0.01 {
        return;
    }
    let direction = (b - a) / screen_length;
    let normal = egui::vec2(-direction.y, direction.x);
    let wall_width = wall_screen_width(view.scale, wall.thickness());
    let color = match tone {
        WallTone::Selected => theme::SELECTION,
        WallTone::Hovered => theme::HOVER,
        WallTone::Normal => theme::WALL,
    };
    let mut openings: Vec<_> = project
        .openings
        .iter()
        .filter(|opening| opening.wall_id == wall.id)
        .collect();
    openings.sort_by(|left, right| left.t.total_cmp(&right.t));
    for opening in openings {
        let op_selected = marks.selected(Selection::Opening(opening.id));
        let op_hovered = marks.hovered(Selection::Opening(opening.id));
        let half = view.pixels(opening.width()) / 2.0;
        let center = opening.t as f32 * screen_length;
        let left = center - half;
        let right = center + half;
        let start = a + direction * left;
        let end = a + direction * right;
        let op_color = if op_selected {
            theme::SELECTION
        } else if op_hovered {
            theme::HOVER
        } else {
            Color32::TRANSPARENT
        };
        let jamb = Stroke::new(
            border_px.max(1.2),
            if op_color == Color32::TRANSPARENT {
                color
            } else {
                op_color
            },
        );
        let half_wall = wall_width / 2.0;
        for point in [start, end] {
            painter.line_segment(
                [point - normal * half_wall, point + normal * half_wall],
                jamb,
            );
        }
        match opening.kind {
            OpeningKind::Door | OpeningKind::Gate => {
                use crate::domain::openings::DoorType;
                let leaf_color = if op_color == Color32::TRANSPARENT {
                    theme::WALL
                } else {
                    op_color
                };
                let arc_color = if op_color == Color32::TRANSPARENT {
                    theme::DOOR_ARC
                } else {
                    op_color
                };
                let leaf = half * 2.0;
                let side = if opening.flip_side { -normal } else { normal };
                let (hinge, along) = if opening.flip_hinge {
                    (end, -direction)
                } else {
                    (start, direction)
                };
                let mut outline_extent = leaf + 4.0;
                match opening.effective_door_type() {
                    DoorType::Swing => {
                        painter.line_segment(
                            [hinge, hinge + side * leaf],
                            Stroke::new(2.2, leaf_color),
                        );
                        let arc: Vec<Pos2> = (0..=20)
                            .map(|step| {
                                let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 20.0;
                                hinge + along * (theta.sin() * leaf) + side * (theta.cos() * leaf)
                            })
                            .collect();
                        painter.add(Shape::line(arc, Stroke::new(1.0, arc_color)));
                    }
                    DoorType::DoubleSwing => {
                        outline_extent = half + 4.0;
                        for (h, dir_h) in [(start, direction), (end, -direction)] {
                            painter
                                .line_segment([h, h + side * half], Stroke::new(2.0, leaf_color));
                            let arc: Vec<Pos2> = (0..=14)
                                .map(|step| {
                                    let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 14.0;
                                    h + dir_h * (theta.sin() * half) + side * (theta.cos() * half)
                                })
                                .collect();
                            painter.add(Shape::line(arc, Stroke::new(1.0, arc_color)));
                        }
                    }
                    DoorType::Sliding => {
                        outline_extent = half_wall + 8.0;
                        let track_off = side * (half_wall + 2.5);
                        let leaf_off = side * (half_wall + 5.0);
                        painter.line_segment(
                            [start + track_off, end + track_off],
                            Stroke::new(1.0, arc_color),
                        );
                        let slide_end = hinge + along * leaf;
                        painter.line_segment(
                            [hinge + leaf_off, slide_end + leaf_off],
                            Stroke::new(2.4, leaf_color),
                        );
                        // Sliding direction arrow.
                        let mid = start.lerp(end, 0.5) + side * (half_wall + 9.0);
                        let tip = mid + along * (half * 0.45);
                        painter.line_segment(
                            [mid - along * (half * 0.45), tip],
                            Stroke::new(1.1, arc_color),
                        );
                        painter.line_segment(
                            [tip, tip - along * 4.5 + side * 2.5],
                            Stroke::new(1.1, arc_color),
                        );
                        painter.line_segment(
                            [tip, tip - along * 4.5 - side * 2.5],
                            Stroke::new(1.1, arc_color),
                        );
                    }
                    DoorType::PocketSliding => {
                        outline_extent = half_wall + 6.0;
                        let pocket_end = hinge - along * leaf;
                        let p_off = normal * (half_wall * 0.45);
                        painter.extend(Shape::dashed_line(
                            &[
                                hinge + p_off,
                                pocket_end + p_off,
                                pocket_end - p_off,
                                hinge - p_off,
                            ],
                            Stroke::new(1.0, arc_color),
                            4.0,
                            3.0,
                        ));
                        painter.line_segment(
                            [hinge, hinge + along * (leaf * 0.65)],
                            Stroke::new(2.4, leaf_color),
                        );
                    }
                    DoorType::Pivot => {
                        let pivot = hinge + along * (leaf * 0.20);
                        let short_r = leaf * 0.20;
                        let long_r = leaf * 0.80;
                        outline_extent = long_r + 4.0;
                        painter.circle_filled(pivot, 2.5, leaf_color);
                        painter.line_segment(
                            [pivot - side * short_r, pivot + side * long_r],
                            Stroke::new(2.2, leaf_color),
                        );
                        let big_arc: Vec<Pos2> = (0..=16)
                            .map(|step| {
                                let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 16.0;
                                pivot
                                    + along * (theta.sin() * long_r)
                                    + side * (theta.cos() * long_r)
                            })
                            .collect();
                        let small_arc: Vec<Pos2> = (0..=10)
                            .map(|step| {
                                let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 10.0;
                                pivot
                                    - along * (theta.sin() * short_r)
                                    - side * (theta.cos() * short_r)
                            })
                            .collect();
                        painter.add(Shape::line(big_arc, Stroke::new(1.0, arc_color)));
                        painter.add(Shape::line(small_arc, Stroke::new(1.0, arc_color)));
                    }
                    DoorType::Bifold => {
                        outline_extent = half * 0.65 + 4.0;
                        let segs = 4;
                        let step_along = leaf * 0.72 / segs as f32;
                        let peak = half * 0.55;
                        let pts: Vec<Pos2> = (0..=segs)
                            .map(|i| {
                                let s = if i % 2 == 1 {
                                    side * peak
                                } else {
                                    egui::Vec2::ZERO
                                };
                                hinge + along * (step_along * i as f32) + s
                            })
                            .collect();
                        painter.add(Shape::line(pts, Stroke::new(2.0, leaf_color)));
                        painter.extend(Shape::dashed_line(
                            &[start, end],
                            Stroke::new(1.0, arc_color),
                            4.0,
                            3.0,
                        ));
                    }
                    DoorType::Revolving => {
                        outline_extent = half + 4.0;
                        let mid = start.lerp(end, 0.5);
                        painter.circle_stroke(mid, half, Stroke::new(1.2, arc_color));
                        let diag1 = (direction + normal).normalized() * half;
                        let diag2 = (direction - normal).normalized() * half;
                        painter
                            .line_segment([mid - diag1, mid + diag1], Stroke::new(2.0, leaf_color));
                        painter
                            .line_segment([mid - diag2, mid + diag2], Stroke::new(2.0, leaf_color));
                    }
                }
                if op_color != Color32::TRANSPARENT {
                    outline_opening(
                        painter,
                        start,
                        end,
                        direction,
                        side,
                        -(half_wall + 4.0),
                        outline_extent,
                        op_color,
                    );
                }
            }
            OpeningKind::SaloonDoor => {
                let leaf_color = if op_color == Color32::TRANSPARENT {
                    theme::WALL
                } else {
                    op_color
                };
                let arc_color = if op_color == Color32::TRANSPARENT {
                    theme::DOOR_ARC
                } else {
                    op_color
                };
                // Two half leaves, hinged at each jamb, swinging both ways.
                for (hinge, toward) in [(start, 1.0), (end, -1.0)] {
                    painter
                        .line_segment([hinge, hinge + normal * half], Stroke::new(2.0, leaf_color));
                    let flip = if opening.flip_side { -1.0 } else { 1.0 };
                    for side in [flip, -flip] {
                        let arc: Vec<Pos2> = (0..=12)
                            .map(|step| {
                                let theta = std::f32::consts::FRAC_PI_2 * step as f32 / 12.0;
                                hinge
                                    + direction * (toward * theta.sin() * half)
                                    + normal * (side * theta.cos() * half)
                            })
                            .collect();
                        if side == flip {
                            painter.add(Shape::line(arc, Stroke::new(1.0, arc_color)));
                        } else {
                            painter.extend(Shape::dashed_line(
                                &arc,
                                Stroke::new(1.0, arc_color),
                                3.0,
                                3.0,
                            ));
                        }
                    }
                }
                if op_color != Color32::TRANSPARENT {
                    outline_opening(
                        painter,
                        start,
                        end,
                        direction,
                        normal,
                        -(half + 4.0),
                        half + 4.0,
                        op_color,
                    );
                }
            }
            OpeningKind::Gap => {
                // No leaf: a dashed line marks the open span.
                let gap_color = if op_color == Color32::TRANSPARENT {
                    theme::DOOR_ARC
                } else {
                    op_color
                };
                painter.extend(Shape::dashed_line(
                    &[start, end],
                    Stroke::new(1.2, gap_color),
                    6.0,
                    4.0,
                ));
                if op_color != Color32::TRANSPARENT {
                    outline_opening(
                        painter,
                        start,
                        end,
                        direction,
                        normal,
                        -(half_wall + 4.0),
                        half_wall + 4.0,
                        op_color,
                    );
                }
            }
            OpeningKind::Window => {
                use crate::domain::openings::WindowType;
                let window_color = if op_color == Color32::TRANSPARENT {
                    theme::WINDOW
                } else {
                    op_color
                };
                let offset = (half_wall - 0.5).max(2.5);
                let side = if opening.flip_side { -normal } else { normal };
                painter.add(Shape::convex_polygon(
                    vec![
                        start + normal * offset,
                        end + normal * offset,
                        end - normal * offset,
                        start - normal * offset,
                    ],
                    window_color.gamma_multiply(0.12),
                    Stroke::NONE,
                ));
                for s in [-1.0, 1.0] {
                    painter.line_segment(
                        [start + normal * offset * s, end + normal * offset * s],
                        Stroke::new(1.5, window_color),
                    );
                }
                let mid = start.lerp(end, 0.5);
                let overlap = (half * 0.14).min(6.0);
                match opening.effective_window_type() {
                    WindowType::Sliding => {
                        // Two slightly offset overlapping sliding sashes.
                        let off = offset * 0.35;
                        painter.line_segment(
                            [
                                start + normal * off,
                                mid + direction * overlap + normal * off,
                            ],
                            Stroke::new(1.3, window_color),
                        );
                        painter.line_segment(
                            [mid - direction * overlap - normal * off, end - normal * off],
                            Stroke::new(1.3, window_color),
                        );
                    }
                    WindowType::Basculante => {
                        // High-sill awning window: dashed horizontal projection on both sides + central axis.
                        let proj = offset + 4.0;
                        painter.extend(Shape::dashed_line(
                            &[start + side * proj, end + side * proj],
                            Stroke::new(1.0, window_color),
                            4.0,
                            3.0,
                        ));
                        painter.line_segment([start, end], Stroke::new(1.0, window_color));
                        painter.circle_filled(mid, 2.0, window_color);
                    }
                    WindowType::MaximAr => {
                        // Outward-projecting maxim-ar sash triangle/trapezoid in dashed lines.
                        let proj = offset + 7.0;
                        painter.line_segment([start, end], Stroke::new(1.0, window_color));
                        painter.extend(Shape::dashed_line(
                            &[start, mid + side * proj, end],
                            Stroke::new(1.1, window_color),
                            4.0,
                            3.0,
                        ));
                    }
                    WindowType::Casement | WindowType::TiltTurn => {
                        // Two opening leaves with technical quarter arcs.
                        let leaf_r = half * 0.75;
                        for (h, dir_h) in [(start, direction), (end, -direction)] {
                            painter.line_segment(
                                [h, h + side * leaf_r],
                                Stroke::new(1.4, window_color),
                            );
                            let arc: Vec<Pos2> = (0..=10)
                                .map(|step| {
                                    let th = std::f32::consts::FRAC_PI_2 * step as f32 / 10.0;
                                    h + dir_h * (th.sin() * leaf_r) + side * (th.cos() * leaf_r)
                                })
                                .collect();
                            painter.extend(Shape::dashed_line(
                                &arc,
                                Stroke::new(0.9, window_color),
                                3.5,
                                2.5,
                            ));
                        }
                        if opening.effective_window_type() == WindowType::TiltTurn {
                            painter.extend(Shape::dashed_line(
                                &[start, mid + side * (offset + 5.0), end],
                                Stroke::new(0.9, window_color),
                                3.0,
                                2.5,
                            ));
                        }
                    }
                    WindowType::Fixed => {
                        // Single fixed glass pane with small central cross mark.
                        painter.line_segment([start, end], Stroke::new(1.3, window_color));
                        let cross = 3.0;
                        painter.line_segment(
                            [
                                mid - direction * cross - normal * cross,
                                mid + direction * cross + normal * cross,
                            ],
                            Stroke::new(1.0, window_color),
                        );
                        painter.line_segment(
                            [
                                mid - direction * cross + normal * cross,
                                mid + direction * cross - normal * cross,
                            ],
                            Stroke::new(1.0, window_color),
                        );
                    }
                    WindowType::BayWindow => {
                        // Outward-projecting trapezoidal bay window silhouette.
                        let depth = (offset + 12.0).max(half * 0.45);
                        let inset = half * 0.35;
                        let p1 = start + direction * inset + side * depth;
                        let p2 = end - direction * inset + side * depth;
                        painter.add(Shape::line(
                            vec![start, p1, p2, end],
                            Stroke::new(1.8, window_color),
                        ));
                        painter.line_segment([start, end], Stroke::new(0.9, window_color));
                    }
                }
                if op_color != Color32::TRANSPARENT {
                    outline_opening(
                        painter,
                        start,
                        end,
                        direction,
                        normal,
                        -(half_wall + 4.0),
                        half_wall + 4.0,
                        op_color,
                    );
                }
            }
        }
    }
}

fn paint_wall_assembly_segment(
    painter: &Painter,
    from: Pos2,
    to: Pos2,
    normal: egui::Vec2,
    wall_width: f32,
    assembly: &crate::domain::wall_layers::WallAssembly,
) {
    let total = assembly.total_thickness() as f32;
    for (start_m, end_m, layer) in assembly.layer_offsets() {
        let s0 = (start_m as f32 / total) * wall_width;
        let s1 = (end_m as f32 / total) * wall_width;
        let [r, g, b] = layer.color;
        painter.add(Shape::convex_polygon(
            vec![
                from + normal * s0,
                to + normal * s0,
                to + normal * s1,
                from + normal * s1,
            ],
            Color32::from_rgb(r, g, b),
            Stroke::NONE,
        ));
    }
    let half = wall_width * 0.5;
    let outline = Stroke::new(0.8, theme::WALL);
    painter.line_segment([from - normal * half, to - normal * half], outline);
    painter.line_segment([from + normal * half, to + normal * half], outline);
}

/// A dashed frame around a selected or hovered opening.
#[allow(clippy::too_many_arguments)]
fn outline_opening(
    painter: &Painter,
    start: Pos2,
    end: Pos2,
    direction: Vec2,
    normal: Vec2,
    near: f32,
    far: f32,
    color: Color32,
) {
    let start = start - direction * 4.0;
    let end = end + direction * 4.0;
    let corners = [
        start + normal * near,
        end + normal * near,
        end + normal * far,
        start + normal * far,
        start + normal * near,
    ];
    painter.extend(Shape::dashed_line(
        &corners,
        Stroke::new(1.2, color),
        4.0,
        3.0,
    ));
}

fn wall_stroke_width(scale: f32) -> f32 {
    (scale * WALL_THICKNESS_METERS as f32).clamp(1.5, 10.0)
}

/// A wall drawn at its real thickness, never thinner than a hairline.
fn wall_screen_width(scale: f32, thickness: f64) -> f32 {
    (scale * thickness as f32).max(1.5)
}

/// Face outline weight for hollow architectural double-line walls.
fn wall_border_width(wall_width: f32) -> f32 {
    (wall_width * 0.20).clamp(1.0, 1.6)
}

#[cfg(test)]
mod tests {
    use super::{wall_border_width, wall_stroke_width};

    #[test]
    fn wall_lines_get_thinner_when_zoomed_out() {
        assert!(wall_stroke_width(18.0) < 3.0);
        assert!(wall_stroke_width(60.0) > 6.0);
    }

    #[test]
    fn hollow_wall_border_leaves_clear_interior_at_working_zoom() {
        let w = 9.0; // 15 cm at 60 px/m
        let b = wall_border_width(w);
        assert!((1.0..=1.6).contains(&b));
        assert!(w - 2.0 * b >= 5.0);
    }
}
