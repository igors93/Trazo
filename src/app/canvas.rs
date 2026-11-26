//! Painting the plan on the canvas: the view transform, the paint options
//! and the main `paint` pass, which draws each layer through the
//! submodules (walls, fixtures, annotations, overlays, previews...).

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use crate::app::theme;
use crate::domain::{
    fixtures::Fixture,
    geometry::{Point, WALL_THICKNESS_METERS},
    model::{Project, Wall},
    wall_reference::WallReference,
};

use super::Selection;

mod background;
mod circulation;
mod dimensions;
mod discipline;
mod drafts;
mod drawings;
mod fixtures;
mod labels;
mod markups;
mod notes;
pub mod overlay;
mod preview;
mod sim_overlay;
mod views;
mod walls;

pub use drafts::{paint_fragment_ghost, paint_modify_guide, paint_polygon_draft};
pub use drawings::paint_draw_draft;

use labels::{paint_room_labels, paint_terrain_label, paint_wall_labels};

use background::{paint_empty_state, paint_floor_underlay, paint_grid};
use circulation::paint_circulations;
pub use dimensions::paint_measurement;
use dimensions::{paint_associative_dimensions, paint_dimensions, paint_frame_tags};
pub use discipline::{FixtureTone, paint_discipline_overlay, paint_ergonomics_overlay};
pub use fixtures::paint_fixture;
pub use markups::{paint_markup, paint_markups};
use notes::{dash_dot, paint_notes};
use preview::paint_preview;
use views::paint_views;
use walls::{
    WallTone, paint_wall_assembly_segment, paint_wall_openings, wall_core_run, wall_run,
    wall_solid_runs,
};

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
