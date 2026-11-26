//! Writing a project as DXF: layers, walls, openings, fixtures, drawings,
//! dimensions, texts and hatches, entity by entity.

use std::f64::consts::{PI, TAU};

use crate::domain::{
    dimensions::ResolvedDimension,
    drawing::Shape,
    fixtures::SymbolShape,
    geometry::{Point, polygon_centroid},
    hatch::hatch_segments_in_polygon,
    layers::{Layer, LayerCategory, LineType},
    model::{OpeningKind, Project, Wall},
    notes::NoteKind,
};

use super::{
    rgb_to_aci, to_dxf, write_arc, write_blocks, write_circle, write_header, write_line,
    write_linear_dimension_block, write_linear_dimension_entity, write_pair, write_polyline,
    write_polyline_styled, write_tables, write_text,
};

/// Serializes `project` into a complete ASCII DXF document (`AC1009`, metres).
pub fn to_dxf_string(project: &Project) -> String {
    let mut blocks_buf = String::new();
    let mut entities_buf = String::new();
    let mut bounds = Extents::default();

    let layer_for_cat = |cat: LayerCategory| -> String {
        project
            .layer_for_category(cat)
            .map(|layer| sanitize_layer_name(&layer.name))
            .unwrap_or_else(|| sanitize_layer_name(default_category_name(cat)))
    };

    let layer_name_by_id = |id: Option<u64>, fallback: LayerCategory| -> String {
        id.and_then(|lid| project.layers.iter().find(|l| l.id == lid))
            .map(|layer| sanitize_layer_name(&layer.name))
            .unwrap_or_else(|| layer_for_cat(fallback))
    };

    // 1. Terrain & subdivided lots.
    let terrain_layer = layer_for_cat(LayerCategory::Terrain);
    if let Some(terrain) = &project.terrain {
        let outline = terrain.outline();
        write_polyline(&mut entities_buf, &terrain_layer, &outline, true, None);
        bounds.include_slice(&outline);
    }
    for lot in &project.lots {
        if lot.vertices.len() >= 3 {
            write_polyline(&mut entities_buf, &terrain_layer, &lot.vertices, true, None);
            bounds.include_slice(&lot.vertices);
        }
    }

    // 2. Room floor hatches (before walls so walls sit cleanly on top in viewers).
    let mut has_hatches = false;
    for room in &project.rooms {
        let Some(hatch) = &room.hatch else {
            continue;
        };
        let Some(poly) = project.room_polygon(room) else {
            continue;
        };
        let segments = hatch_segments_in_polygon(&poly, hatch);
        if segments.is_empty() {
            continue;
        }
        has_hatches = true;
        let aci = Some(rgb_to_aci(hatch.color));
        for (a, b) in segments {
            write_line(&mut entities_buf, "Hachuras", a, b, aci);
            bounds.include(a);
            bounds.include(b);
        }
    }

    // 3. Walls (exported as closed architectural double-line polylines on wall layer).
    for wall in &project.walls {
        let layer = layer_name_by_id(wall.layer_id, LayerCategory::Walls);
        let corners = wall_rectangle(wall);
        write_polyline(&mut entities_buf, &layer, &corners, true, None);
        bounds.include_slice(&corners);
    }

    // 4. Openings (doors, windows, saloon doors, gates, gaps).
    let openings_layer = layer_for_cat(LayerCategory::Openings);
    for (index, opening) in project.openings.iter().enumerate() {
        let Some(wall) = project.walls.iter().find(|w| w.id == opening.wall_id) else {
            continue;
        };
        export_opening(
            &mut entities_buf,
            &mut bounds,
            &openings_layer,
            wall,
            opening,
            index + 1,
        );
    }

    // 5. Room labels & areas.
    let rooms_layer = layer_for_cat(LayerCategory::Rooms);
    for room in &project.rooms {
        let Some(poly) = project.room_polygon(room) else {
            continue;
        };
        let center = polygon_centroid(&poly);
        let name = room.display_name();
        write_text(
            &mut entities_buf,
            &rooms_layer,
            Point::new(center.x, center.y - 0.10),
            0.16,
            0.0,
            name,
            None,
        );
        if let Some(area) = project.room_area(room) {
            let area_label = format!("{area:.2} m2");
            write_text(
                &mut entities_buf,
                &rooms_layer,
                Point::new(center.x, center.y + 0.14),
                0.12,
                0.0,
                &area_label,
                None,
            );
        }
        bounds.include(center);
    }

    // 6. Fixtures & electrical points.
    for fixture in &project.fixtures {
        let category = Project::fixture_category(fixture);
        let layer = layer_for_cat(category);
        // Export the shared symbol, not a placeholder rectangle. Keeping the
        // same local-to-world transform preserves free rotation and all parts.
        for shape in fixture.symbol() {
            match shape {
                SymbolShape::Polyline {
                    points,
                    closed,
                    dashed,
                    ..
                } => {
                    let points: Vec<_> = points.into_iter().map(|p| fixture.to_world(p)).collect();
                    write_polyline_styled(
                        &mut entities_buf,
                        &layer,
                        &points,
                        closed,
                        None,
                        dashed.then_some("DASHED"),
                    );
                    bounds.include_slice(&points);
                }
                SymbolShape::Circle { center, radius, .. } => {
                    let center = fixture.to_world(center);
                    write_circle(&mut entities_buf, &layer, center, radius, None);
                    bounds.include(Point::new(center.x - radius, center.y - radius));
                    bounds.include(Point::new(center.x + radius, center.y + radius));
                }
            }
        }
        if fixture.kind.is_electrical() {
            let r = (fixture.width.min(fixture.depth) * 0.35).max(0.06);
            if let Some(elec) = &fixture.electrical
                && let Some(circuit) = elec.circuit
            {
                let label = format!("C{circuit}");
                write_text(
                    &mut entities_buf,
                    &layer,
                    Point::new(fixture.center.x + r + 0.05, fixture.center.y),
                    0.10,
                    0.0,
                    &label,
                    None,
                );
            }
        }
    }

    // 6b. Conduits (eletrodutos) with their wire ticks and circuit numbers.
    if project.category_visible(LayerCategory::Electrical) {
        use crate::domain::conduits::PlanMark;
        let layer = layer_for_cat(LayerCategory::Electrical);
        for mark in project.conduit_marks() {
            match mark {
                PlanMark::Line { points, dashed, .. } => {
                    write_polyline_styled(
                        &mut entities_buf,
                        &layer,
                        &points,
                        false,
                        None,
                        dashed.then_some("DASHED"),
                    );
                    bounds.include_slice(&points);
                }
                PlanMark::Circle { center, radius } => {
                    write_circle(&mut entities_buf, &layer, center, radius, None);
                }
                PlanMark::Text { anchor, text } => {
                    write_text(&mut entities_buf, &layer, anchor, 0.10, 0.0, &text, None);
                }
            }
        }
    }

    // 6c. Beams and the labels of the formwork plan.
    if project.category_visible(LayerCategory::Structure) {
        use crate::domain::conduits::PlanMark;
        let layer = layer_for_cat(LayerCategory::Structure);
        // Scale structure label text height to ~2.2 mm on paper at the sheet's
        // drawing scale (e.g. 0.11 m at 1:50, 0.22 m at 1:100).
        let text_height = (2.2 / project.sheet.scale.mm_per_meter()).clamp(0.06, 0.50);
        for mark in project.structure_marks() {
            match mark {
                PlanMark::Line { points, dashed, .. } => {
                    write_polyline_styled(
                        &mut entities_buf,
                        &layer,
                        &points,
                        false,
                        None,
                        dashed.then_some("DASHED"),
                    );
                    bounds.include_slice(&points);
                }
                PlanMark::Circle { center, radius } => {
                    write_circle(&mut entities_buf, &layer, center, radius, None);
                }
                PlanMark::Text { anchor, text } => {
                    write_text(
                        &mut entities_buf,
                        &layer,
                        anchor,
                        text_height,
                        0.0,
                        &text,
                        None,
                    );
                }
            }
        }
    }

    // 7. 2D drafting entities (lines, polylines, circles, arcs, splines).
    for drawing in &project.drawings {
        let layer = layer_name_by_id(drawing.layer_id, LayerCategory::Drawings);
        match &drawing.shape {
            Shape::Line { start, end } => {
                write_line(&mut entities_buf, &layer, *start, *end, None);
                bounds.include(*start);
                bounds.include(*end);
            }
            Shape::Polyline { points, closed } => {
                write_polyline(&mut entities_buf, &layer, points, *closed, None);
                bounds.include_slice(points);
            }
            Shape::Circle { center, radius } => {
                write_circle(&mut entities_buf, &layer, *center, *radius, None);
                bounds.include(Point::new(center.x - radius, center.y - radius));
                bounds.include(Point::new(center.x + radius, center.y + radius));
            }
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                write_arc(
                    &mut entities_buf,
                    &layer,
                    *center,
                    *radius,
                    *start,
                    *sweep,
                    None,
                );
                bounds.include(Point::new(center.x - radius, center.y - radius));
                bounds.include(Point::new(center.x + radius, center.y + radius));
            }
            Shape::Spline { points, closed } => {
                let sampled = sample_spline(points, *closed, 8);
                write_polyline(&mut entities_buf, &layer, &sampled, *closed, None);
                bounds.include_slice(&sampled);
            }
        }
    }

    // 8. Associative dimensions (real DXF DIMENSION entities backed by *D<n> blocks).
    let dims_layer = layer_for_cat(LayerCategory::Dimensions);
    let mut dim_block_index = 1usize;
    for dim in &project.dimensions {
        if !dim.visible {
            continue;
        }
        let Some(resolved) = project.resolve_dimension(dim) else {
            continue;
        };
        match resolved {
            ResolvedDimension::Linear {
                p1,
                p2,
                dim_p1,
                dim_p2,
                label,
                ..
            } => {
                let block_name = format!("*D{dim_block_index}");
                dim_block_index += 1;
                write_linear_dimension_block(
                    &mut blocks_buf,
                    &block_name,
                    &dims_layer,
                    p1,
                    p2,
                    dim_p1,
                    dim_p2,
                    &label,
                );
                write_linear_dimension_entity(
                    &mut entities_buf,
                    &dims_layer,
                    &block_name,
                    p1,
                    p2,
                    dim_p1,
                    dim_p2,
                    &label,
                );
                bounds.include(p1);
                bounds.include(p2);
                bounds.include(dim_p1);
                bounds.include(dim_p2);
            }
            ResolvedDimension::Angular {
                vertex,
                start_angle,
                sweep,
                radius,
                label,
                ..
            } => {
                write_arc(
                    &mut entities_buf,
                    &dims_layer,
                    vertex,
                    radius,
                    start_angle,
                    sweep,
                    None,
                );
                let mid = start_angle + sweep * 0.5;
                let text_pos = Point::new(
                    vertex.x + (radius + 0.16) * mid.cos(),
                    vertex.y + (radius + 0.16) * mid.sin(),
                );
                write_text(
                    &mut entities_buf,
                    &dims_layer,
                    text_pos,
                    0.14,
                    0.0,
                    &label,
                    None,
                );
                bounds.include(text_pos);
            }
        }
    }

    // 9. Texts, notes, leader callouts and symbols.
    for note in &project.notes {
        let layer = layer_name_by_id(note.layer_id, LayerCategory::Annotations);
        let text_height = (note.font_size * 0.014).clamp(0.10, 0.80);
        if let NoteKind::Callout { target } = note.kind {
            write_line(&mut entities_buf, &layer, target, note.position, None);
            bounds.include(target);
        }
        write_text(
            &mut entities_buf,
            &layer,
            note.position,
            text_height,
            -note.rotation_deg,
            &note.text,
            None,
        );
        bounds.include(note.position);
    }

    let mut export_layers = project.layers.clone();
    if project.markup_settings.export {
        for m in &project.markups {
            if !project.markup_shown(m) || m.validate().is_err() {
                continue;
            }
            let name = format!(
                "Marcacoes_{:02X}{:02X}{:02X}",
                m.color[0], m.color[1], m.color[2]
            );
            if !export_layers.iter().any(|l| l.name == name) {
                export_layers.push(Layer {
                    id: 0,
                    name: name.clone(),
                    color: m.color,
                    line_weight_mm: 0.25,
                    line_type: LineType::Solid,
                    visible: true,
                    locked: false,
                    category: None,
                });
            }
            let path = m.shape().outline();
            write_polyline(&mut entities_buf, &name, &path, m.shape().is_closed(), None);
            bounds.include_slice(&path);
            if m.kind == crate::domain::markups::MarkupKind::Arrow {
                let a = m.points[0];
                let b = m.points[1];
                let along = b.minus(a).normalized().unwrap_or(Point::ORIGIN);
                let normal = Point::new(-along.y, along.x);
                let base = b.translated(along.scaled(-0.20));
                write_polyline(
                    &mut entities_buf,
                    &name,
                    &[
                        base.translated(normal.scaled(0.08)),
                        b,
                        base.translated(normal.scaled(-0.08)),
                    ],
                    false,
                    None,
                );
            }
            let label = if m.comment.is_empty() {
                m.label()
            } else {
                format!("{}: {}", m.label(), m.comment)
            };
            write_text(
                &mut entities_buf,
                &name,
                m.points[0],
                0.16,
                0.0,
                &label,
                None,
            );
        }
    }

    // Assemble complete DXF document.
    let mut out = String::with_capacity(blocks_buf.len() + entities_buf.len() + 4096);
    write_header(&mut out, bounds);
    write_tables(&mut out, &export_layers, has_hatches);
    write_blocks(&mut out, &blocks_buf);
    write_pair(&mut out, 0, "SECTION");
    write_pair(&mut out, 2, "ENTITIES");
    out.push_str(&entities_buf);
    write_pair(&mut out, 0, "ENDSEC");
    write_pair(&mut out, 0, "EOF");
    out
}

pub(super) fn default_category_name(cat: LayerCategory) -> &'static str {
    match cat {
        LayerCategory::Terrain => "Terreno",
        LayerCategory::Walls => "Paredes",
        LayerCategory::Openings => "Esquadrias",
        LayerCategory::Rooms => "Comodos",
        LayerCategory::Fixtures => "Mobiliario",
        LayerCategory::Drawings => "Desenho",
        LayerCategory::Dimensions => "Cotas",
        LayerCategory::Annotations => "Anotacoes",
        LayerCategory::Electrical => "Eletrica",
        LayerCategory::Structure => "Estrutura",
        LayerCategory::Plumbing => "Hidraulica",
    }
}

pub(super) fn sanitize_layer_name(name: &str) -> String {
    let mut clean = String::with_capacity(name.len());
    for ch in name.trim().chars() {
        let mapped = match ch {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'Á' | 'À' | 'Â' | 'Ã' => 'A',
            'é' | 'ê' => 'e',
            'É' | 'Ê' => 'E',
            'í' => 'i',
            'Í' => 'I',
            'ó' | 'ô' | 'õ' => 'o',
            'Ó' | 'Ô' | 'Õ' => 'O',
            'ú' => 'u',
            'Ú' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            '<' | '>' | '/' | '\\' | '"' | ':' | ';' | '?' | '*' | '|' | ',' | '=' | '`' => '_',
            c if c.is_ascii_graphic() => c,
            ' ' => '_',
            _ => continue,
        };
        clean.push(mapped);
    }
    if clean.is_empty() {
        "0".to_owned()
    } else {
        clean
    }
}

pub(super) fn wall_rectangle(wall: &Wall) -> [Point; 4] {
    let dx = wall.end.x - wall.start.x;
    let dy = wall.end.y - wall.start.y;
    let len = dx.hypot(dy).max(1e-9);
    let nx = -dy / len * (wall.thickness() * 0.5);
    let ny = dx / len * (wall.thickness() * 0.5);
    [
        Point::new(wall.start.x + nx, wall.start.y + ny),
        Point::new(wall.end.x + nx, wall.end.y + ny),
        Point::new(wall.end.x - nx, wall.end.y - ny),
        Point::new(wall.start.x - nx, wall.start.y - ny),
    ]
}

pub(super) fn export_opening(
    out: &mut String,
    bounds: &mut Extents,
    layer: &str,
    wall: &Wall,
    opening: &crate::domain::model::Opening,
    seq: usize,
) {
    let dx = wall.end.x - wall.start.x;
    let dy = wall.end.y - wall.start.y;
    let len = dx.hypot(dy);
    if len < 1e-6 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    let side_sign = if opening.flip_side { -1.0 } else { 1.0 };
    let nx = -uy * side_sign;
    let ny = ux * side_sign;

    let half_w = opening.width() * 0.5;
    let half_t = wall.thickness() * 0.5;
    let center = Point::new(wall.start.x + dx * opening.t, wall.start.y + dy * opening.t);
    let p_a = Point::new(center.x - ux * half_w, center.y - uy * half_w);
    let p_b = Point::new(center.x + ux * half_w, center.y + uy * half_w);

    // Jamb lines across wall thickness.
    for jamb in [p_a, p_b] {
        let j1 = Point::new(jamb.x + nx * half_t, jamb.y + ny * half_t);
        let j2 = Point::new(jamb.x - nx * half_t, jamb.y - ny * half_t);
        write_line(out, layer, j1, j2, None);
        bounds.include(j1);
        bounds.include(j2);
    }

    match opening.kind {
        OpeningKind::Door => {
            let (hinge, latch) = if opening.flip_hinge {
                (p_b, p_a)
            } else {
                (p_a, p_b)
            };
            let w = opening.width();
            let leaf_tip = Point::new(hinge.x + nx * w, hinge.y + ny * w);
            write_line(out, layer, hinge, leaf_tip, None);
            let a_closed = (latch.y - hinge.y).atan2(latch.x - hinge.x);
            let a_open = (leaf_tip.y - hinge.y).atan2(leaf_tip.x - hinge.x);
            let mut sweep = (a_open - a_closed).rem_euclid(TAU);
            if sweep > PI {
                sweep -= TAU;
            }
            write_arc(out, layer, hinge, w, a_closed, sweep, None);
            bounds.include(leaf_tip);
            let tag = format!("P{seq}");
            write_text(
                out,
                layer,
                Point::new(
                    center.x + nx * (half_t + 0.14),
                    center.y + ny * (half_t + 0.14),
                ),
                0.11,
                0.0,
                &tag,
                None,
            );
        }
        OpeningKind::SaloonDoor => {
            let hw = opening.width() * 0.5;
            for (hinge, other) in [(p_a, p_b), (p_b, p_a)] {
                let leaf_tip = Point::new(hinge.x + nx * hw, hinge.y + ny * hw);
                write_line(out, layer, hinge, leaf_tip, None);
                let a_closed = (other.y - hinge.y).atan2(other.x - hinge.x);
                let a_open = (leaf_tip.y - hinge.y).atan2(leaf_tip.x - hinge.x);
                let mut sweep = (a_open - a_closed).rem_euclid(TAU);
                if sweep > PI {
                    sweep -= TAU;
                }
                write_arc(out, layer, hinge, hw, a_closed, sweep, None);
                bounds.include(leaf_tip);
            }
        }
        OpeningKind::Window => {
            for offset in [-half_t, 0.0, half_t] {
                let s = Point::new(p_a.x + nx * offset, p_a.y + ny * offset);
                let e = Point::new(p_b.x + nx * offset, p_b.y + ny * offset);
                write_line(out, layer, s, e, None);
                bounds.include(s);
                bounds.include(e);
            }
            let tag = format!("J{seq}");
            write_text(
                out,
                layer,
                Point::new(
                    center.x + nx * (half_t + 0.14),
                    center.y + ny * (half_t + 0.14),
                ),
                0.11,
                0.0,
                &tag,
                None,
            );
        }
        OpeningKind::Gate => {
            for offset in [-half_t * 0.5, half_t * 0.5] {
                let s = Point::new(p_a.x + nx * offset, p_a.y + ny * offset);
                let e = Point::new(p_b.x + nx * offset, p_b.y + ny * offset);
                write_line(out, layer, s, e, None);
                bounds.include(s);
                bounds.include(e);
            }
            write_text(
                out,
                layer,
                Point::new(
                    center.x + nx * (half_t + 0.18),
                    center.y + ny * (half_t + 0.18),
                ),
                0.12,
                0.0,
                "PT",
                None,
            );
        }
        OpeningKind::Gap => {
            write_line(out, layer, p_a, p_b, None);
        }
    }
}

pub(super) fn sample_spline(points: &[Point], closed: bool, steps_per_seg: usize) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let n = points.len();
    let seg_count = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(seg_count * steps_per_seg + 1);
    for i in 0..seg_count {
        let p0 = if closed {
            points[(i + n - 1) % n]
        } else {
            points[i.saturating_sub(1)]
        };
        let p1 = points[i % n];
        let p2 = points[(i + 1) % n];
        let p3 = if closed {
            points[(i + 2) % n]
        } else {
            points[(i + 2).min(n - 1)]
        };
        for s in 0..steps_per_seg {
            let t = s as f64 / steps_per_seg as f64;
            let t2 = t * t;
            let t3 = t2 * t;
            let x = 0.5
                * ((2.0 * p1.x)
                    + (-p0.x + p2.x) * t
                    + (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2
                    + (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3);
            let y = 0.5
                * ((2.0 * p1.y)
                    + (-p0.y + p2.y) * t
                    + (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2
                    + (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3);
            out.push(Point::new(x, y));
        }
    }
    if !closed && let Some(&last) = points.last() {
        out.push(last);
    }
    out
}

#[derive(Default, Clone, Copy)]
pub(super) struct Extents {
    pub(super) min_x: Option<f64>,
    pub(super) min_y: Option<f64>,
    pub(super) max_x: Option<f64>,
    pub(super) max_y: Option<f64>,
}

impl Extents {
    fn include(&mut self, p: Point) {
        let (x, y) = to_dxf(p);
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        self.min_x = Some(self.min_x.map_or(x, |m| m.min(x)));
        self.min_y = Some(self.min_y.map_or(y, |m| m.min(y)));
        self.max_x = Some(self.max_x.map_or(x, |m| m.max(x)));
        self.max_y = Some(self.max_y.map_or(y, |m| m.max(y)));
    }

    fn include_slice(&mut self, pts: &[Point]) {
        for &p in pts {
            self.include(p);
        }
    }
}
