//! AutoCAD DXF import and export (#38).
//!
//! Exports a Trazo [`Project`] as standard ASCII DXF (`AC1009` / R12 with
//! `$INSUNITS = 6` metres and `$MEASUREMENT = 1` metric) so plans open cleanly
//! in AutoCAD, QCAD, LibreCAD and other CAD tools with layers, colours,
//! linetypes, walls, openings, fixtures, drawings, associative dimensions,
//! texts and hatches intact.
//!
//! Imports ASCII DXF files (R12 through modern AutoCAD with `LWPOLYLINE`,
//! `MTEXT`, `SPLINE`, `DIMENSION` and `$INSUNITS` scaling from mm, cm, m,
//! inches or feet), mapping wall layers to Trazo walls and preserving layers,
//! colours, drawings, texts and dimensions.

use std::{
    collections::HashMap,
    f64::consts::{FRAC_PI_2, PI, TAU},
    fmt::Write as _,
    fs,
    path::Path,
};

use crate::domain::{
    dimensions::{DimensionAnchor, DimensionKind, ResolvedDimension},
    drawing::Shape,
    geometry::{Point, polygon_centroid},
    hatch::hatch_segments_in_polygon,
    layers::{Layer, LayerCategory, LineType},
    model::{OpeningKind, Project, Wall},
    notes::NoteKind,
};

/// Summary returned after importing a DXF file into a [`Project`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DxfImportReport {
    pub layers: usize,
    pub walls: usize,
    pub drawings: usize,
    pub notes: usize,
    pub dimensions: usize,
    pub terrain_imported: bool,
    pub skipped_entities: usize,
    pub units_label: &'static str,
}

impl DxfImportReport {
    pub fn total_imported(&self) -> usize {
        self.walls
            + self.drawings
            + self.notes
            + self.dimensions
            + usize::from(self.terrain_imported)
    }

    pub fn summary_pt(&self) -> String {
        let mut parts = Vec::new();
        if self.walls > 0 {
            parts.push(format!("{} paredes", self.walls));
        }
        if self.drawings > 0 {
            parts.push(format!("{} entidades 2D", self.drawings));
        }
        if self.notes > 0 {
            parts.push(format!("{} textos", self.notes));
        }
        if self.dimensions > 0 {
            parts.push(format!("{} cotas", self.dimensions));
        }
        if self.terrain_imported {
            parts.push("terreno".to_owned());
        }
        if parts.is_empty() {
            format!(
                "DXF lido sem entidades geométricas importáveis (unidades: {}).",
                self.units_label
            )
        } else {
            format!(
                "DXF importado: {} ({} camadas, unidades: {}).",
                parts.join(", "),
                self.layers,
                self.units_label
            )
        }
    }
}

/// Exports `project` to an ASCII DXF file at `path`.
pub fn export(project: &Project, path: &Path) -> Result<(), String> {
    let content = to_dxf_string(project);
    fs::write(path, content).map_err(|error| format!("Não foi possível gravar o DXF: {error}"))
}

/// Imports an ASCII DXF file from `path` into `project`.
pub fn import_file(project: &mut Project, path: &Path) -> Result<DxfImportReport, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("Não foi possível ler o arquivo DXF: {error}"))?;
    let content = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) => err
            .into_bytes()
            .into_iter()
            .map(|b| b as char)
            .collect::<String>(),
    };
    import_str(project, &content)
}

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
        let hw = fixture.width * 0.5;
        let hd = fixture.depth * 0.5;
        let corners = [
            fixture.to_world(Point::new(-hw, -hd)),
            fixture.to_world(Point::new(hw, -hd)),
            fixture.to_world(Point::new(hw, hd)),
            fixture.to_world(Point::new(-hw, hd)),
        ];
        write_polyline(&mut entities_buf, &layer, &corners, true, None);
        bounds.include_slice(&corners);

        // Add subtle internal detail for quick identification in CAD viewers.
        if fixture.kind.is_electrical() {
            let r = (fixture.width.min(fixture.depth) * 0.35).max(0.06);
            write_circle(&mut entities_buf, &layer, fixture.center, r, None);
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
        } else {
            let mid_back = fixture.to_world(Point::new(0.0, -hd * 0.65));
            let mid_front = fixture.to_world(Point::new(0.0, hd * 0.65));
            write_line(&mut entities_buf, &layer, mid_back, mid_front, None);
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

    // Assemble complete DXF document.
    let mut out = String::with_capacity(blocks_buf.len() + entities_buf.len() + 4096);
    write_header(&mut out, bounds);
    write_tables(&mut out, &project.layers, has_hatches);
    write_blocks(&mut out, &blocks_buf);
    write_pair(&mut out, 0, "SECTION");
    write_pair(&mut out, 2, "ENTITIES");
    out.push_str(&entities_buf);
    write_pair(&mut out, 0, "ENDSEC");
    write_pair(&mut out, 0, "EOF");
    out
}

fn default_category_name(cat: LayerCategory) -> &'static str {
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
    }
}

fn sanitize_layer_name(name: &str) -> String {
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

fn wall_rectangle(wall: &Wall) -> [Point; 4] {
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

fn export_opening(
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

fn sample_spline(points: &[Point], closed: bool, steps_per_seg: usize) -> Vec<Point> {
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
struct Extents {
    min_x: Option<f64>,
    min_y: Option<f64>,
    max_x: Option<f64>,
    max_y: Option<f64>,
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

fn to_dxf(p: Point) -> (f64, f64) {
    (p.x, -p.y)
}

fn from_dxf(x: f64, y: f64, scale: f64) -> Point {
    Point::new(x * scale, -y * scale)
}

fn write_pair(out: &mut String, code: i32, value: &str) {
    let _ = writeln!(out, "{code:3}\n{value}");
}

fn write_f64(out: &mut String, code: i32, value: f64) {
    let cleaned = if value.abs() < 1e-10 { 0.0 } else { value };
    let _ = writeln!(out, "{code:3}\n{cleaned:.6}");
}

fn write_i32(out: &mut String, code: i32, value: i32) {
    let _ = writeln!(out, "{code:3}\n{value}");
}

fn write_header(out: &mut String, bounds: Extents) {
    write_pair(out, 0, "SECTION");
    write_pair(out, 2, "HEADER");

    write_pair(out, 9, "$ACADVER");
    write_pair(out, 1, "AC1009");

    // 6 = Metres, 1 = Metric measurement system, 2 = Decimal units.
    write_pair(out, 9, "$INSUNITS");
    write_i32(out, 70, 6);
    write_pair(out, 9, "$MEASUREMENT");
    write_i32(out, 70, 1);
    write_pair(out, 9, "$LUNITS");
    write_i32(out, 70, 2);
    write_pair(out, 9, "$LUPREC");
    write_i32(out, 70, 4);

    let min_x = bounds.min_x.unwrap_or(0.0);
    let min_y = bounds.min_y.unwrap_or(0.0);
    let max_x = bounds.max_x.unwrap_or(10.0);
    let max_y = bounds.max_y.unwrap_or(10.0);

    write_pair(out, 9, "$EXTMIN");
    write_f64(out, 10, min_x);
    write_f64(out, 20, min_y);
    write_f64(out, 30, 0.0);

    write_pair(out, 9, "$EXTMAX");
    write_f64(out, 10, max_x);
    write_f64(out, 20, max_y);
    write_f64(out, 30, 0.0);

    write_pair(out, 0, "ENDSEC");
}

fn write_tables(out: &mut String, layers: &[Layer], include_hatches_layer: bool) {
    write_pair(out, 0, "SECTION");
    write_pair(out, 2, "TABLES");

    // LTYPE table.
    write_pair(out, 0, "TABLE");
    write_pair(out, 2, "LTYPE");
    write_i32(out, 70, 4);
    write_ltype(out, "CONTINUOUS", "Solid line", &[]);
    write_ltype(out, "DASHED", "Dashed __ __ __", &[0.25, -0.15]);
    write_ltype(
        out,
        "DASHDOT",
        "Dash dot __ . __ .",
        &[0.35, -0.10, 0.0, -0.10],
    );
    write_ltype(out, "DOTTED", "Dotted . . . .", &[0.0, -0.12]);
    write_pair(out, 0, "ENDTAB");

    // LAYER table.
    let mut seen = HashMap::<String, ()>::new();
    let mut layer_entries: Vec<(String, i32, &str, i32)> = Vec::new();
    for layer in layers {
        let name = sanitize_layer_name(&layer.name);
        if seen.insert(name.to_uppercase(), ()).is_some() {
            continue;
        }
        let aci = i32::from(rgb_to_aci(layer.color));
        let signed_color = if layer.visible { aci } else { -aci };
        let flags = if layer.locked { 4 } else { 0 };
        layer_entries.push((name, signed_color, linetype_to_dxf(layer.line_type), flags));
    }
    if include_hatches_layer && seen.insert("HACHURAS".to_owned(), ()).is_none() {
        layer_entries.push(("Hachuras".to_owned(), 9, "CONTINUOUS", 0));
    }

    write_pair(out, 0, "TABLE");
    write_pair(out, 2, "LAYER");
    write_i32(out, 70, layer_entries.len() as i32);
    for (name, color, ltype, flags) in layer_entries {
        write_pair(out, 0, "LAYER");
        write_pair(out, 2, &name);
        write_i32(out, 70, flags);
        write_i32(out, 62, color);
        write_pair(out, 6, ltype);
    }
    write_pair(out, 0, "ENDTAB");

    // STYLE table.
    write_pair(out, 0, "TABLE");
    write_pair(out, 2, "STYLE");
    write_i32(out, 70, 1);
    write_pair(out, 0, "STYLE");
    write_pair(out, 2, "STANDARD");
    write_i32(out, 70, 0);
    write_f64(out, 40, 0.0);
    write_f64(out, 41, 1.0);
    write_f64(out, 50, 0.0);
    write_i32(out, 71, 0);
    write_f64(out, 42, 0.20);
    write_pair(out, 3, "txt");
    write_pair(out, 4, "");
    write_pair(out, 0, "ENDTAB");

    write_pair(out, 0, "ENDSEC");
}

fn write_ltype(out: &mut String, name: &str, desc: &str, pattern: &[f64]) {
    write_pair(out, 0, "LTYPE");
    write_pair(out, 2, name);
    write_i32(out, 70, 0);
    write_pair(out, 3, desc);
    write_i32(out, 72, 65);
    write_i32(out, 73, pattern.len() as i32);
    let total_len: f64 = pattern.iter().map(|v| v.abs()).sum();
    write_f64(out, 40, total_len);
    for &seg in pattern {
        write_f64(out, 49, seg);
    }
}

fn write_blocks(out: &mut String, blocks_buf: &str) {
    write_pair(out, 0, "SECTION");
    write_pair(out, 2, "BLOCKS");
    out.push_str(blocks_buf);
    write_pair(out, 0, "ENDSEC");
}

#[allow(clippy::too_many_arguments)]
fn write_linear_dimension_block(
    out: &mut String,
    block_name: &str,
    layer: &str,
    p1: Point,
    p2: Point,
    dim_p1: Point,
    dim_p2: Point,
    label: &str,
) {
    write_pair(out, 0, "BLOCK");
    write_pair(out, 8, layer);
    write_pair(out, 2, block_name);
    write_i32(out, 70, 1);
    write_f64(out, 10, 0.0);
    write_f64(out, 20, 0.0);
    write_f64(out, 30, 0.0);

    // Extension lines + dimension line.
    write_line(out, layer, p1, dim_p1, None);
    write_line(out, layer, p2, dim_p2, None);
    write_line(out, layer, dim_p1, dim_p2, None);

    // Architectural 45° tick marks at both ends.
    let dx = dim_p2.x - dim_p1.x;
    let dy = dim_p2.y - dim_p1.y;
    let angle = dy.atan2(dx);
    let tick_angle = angle + FRAC_PI_2 * 0.5;
    let tx = 0.06 * tick_angle.cos();
    let ty = 0.06 * tick_angle.sin();
    for end in [dim_p1, dim_p2] {
        write_line(
            out,
            layer,
            Point::new(end.x - tx, end.y - ty),
            Point::new(end.x + tx, end.y + ty),
            None,
        );
    }

    let mid = dim_p1.lerp(dim_p2, 0.5);
    let rot_deg = (-angle).to_degrees();
    write_text(out, layer, mid, 0.14, rot_deg, label, None);

    write_pair(out, 0, "ENDBLK");
    write_pair(out, 8, layer);
}

#[allow(clippy::too_many_arguments)]
fn write_linear_dimension_entity(
    out: &mut String,
    layer: &str,
    block_name: &str,
    p1: Point,
    p2: Point,
    dim_p1: Point,
    dim_p2: Point,
    label: &str,
) {
    let (d1x, d1y) = to_dxf(p1);
    let (d2x, d2y) = to_dxf(p2);
    let (lx, ly) = to_dxf(dim_p2);
    let (mx, my) = to_dxf(dim_p1.lerp(dim_p2, 0.5));

    write_pair(out, 0, "DIMENSION");
    write_pair(out, 8, layer);
    write_pair(out, 2, block_name);
    write_f64(out, 10, lx);
    write_f64(out, 20, ly);
    write_f64(out, 30, 0.0);
    write_f64(out, 11, mx);
    write_f64(out, 21, my);
    write_f64(out, 31, 0.0);
    // 1 = aligned linear dimension.
    write_i32(out, 70, 1);
    write_pair(out, 1, label);
    write_f64(out, 13, d1x);
    write_f64(out, 23, d1y);
    write_f64(out, 33, 0.0);
    write_f64(out, 14, d2x);
    write_f64(out, 24, d2y);
    write_f64(out, 34, 0.0);
}

fn write_line(out: &mut String, layer: &str, a: Point, b: Point, aci: Option<u8>) {
    let (ax, ay) = to_dxf(a);
    let (bx, by) = to_dxf(b);
    write_pair(out, 0, "LINE");
    write_pair(out, 8, layer);
    if let Some(color) = aci {
        write_i32(out, 62, i32::from(color));
    }
    write_f64(out, 10, ax);
    write_f64(out, 20, ay);
    write_f64(out, 30, 0.0);
    write_f64(out, 11, bx);
    write_f64(out, 21, by);
    write_f64(out, 31, 0.0);
}

fn write_polyline(out: &mut String, layer: &str, pts: &[Point], closed: bool, aci: Option<u8>) {
    write_polyline_styled(out, layer, pts, closed, aci, None);
}

fn write_polyline_styled(
    out: &mut String,
    layer: &str,
    pts: &[Point],
    closed: bool,
    aci: Option<u8>,
    ltype: Option<&str>,
) {
    if pts.len() < 2 {
        return;
    }
    write_pair(out, 0, "POLYLINE");
    write_pair(out, 8, layer);
    if let Some(lt) = ltype {
        write_pair(out, 6, lt);
    }
    if let Some(color) = aci {
        write_i32(out, 62, i32::from(color));
    }
    write_i32(out, 66, 1);
    write_f64(out, 10, 0.0);
    write_f64(out, 20, 0.0);
    write_f64(out, 30, 0.0);
    write_i32(out, 70, if closed { 1 } else { 0 });

    for &pt in pts {
        let (x, y) = to_dxf(pt);
        write_pair(out, 0, "VERTEX");
        write_pair(out, 8, layer);
        write_f64(out, 10, x);
        write_f64(out, 20, y);
        write_f64(out, 30, 0.0);
    }
    write_pair(out, 0, "SEQEND");
    write_pair(out, 8, layer);
}

fn write_circle(out: &mut String, layer: &str, center: Point, radius: f64, aci: Option<u8>) {
    let (cx, cy) = to_dxf(center);
    write_pair(out, 0, "CIRCLE");
    write_pair(out, 8, layer);
    if let Some(color) = aci {
        write_i32(out, 62, i32::from(color));
    }
    write_f64(out, 10, cx);
    write_f64(out, 20, cy);
    write_f64(out, 30, 0.0);
    write_f64(out, 40, radius);
}

fn write_arc(
    out: &mut String,
    layer: &str,
    center: Point,
    radius: f64,
    start_rad: f64,
    sweep_rad: f64,
    aci: Option<u8>,
) {
    let (cx, cy) = to_dxf(center);
    // Flipping Y reverses orientation: Trazo positive sweep (CW in Y-up) becomes
    // negative CCW in DXF, so swap start and end when sweep > 0.
    let end_rad = start_rad + sweep_rad;
    let (dxf_start, dxf_end) = if sweep_rad >= 0.0 {
        ((-end_rad).rem_euclid(TAU), (-start_rad).rem_euclid(TAU))
    } else {
        ((-start_rad).rem_euclid(TAU), (-end_rad).rem_euclid(TAU))
    };
    write_pair(out, 0, "ARC");
    write_pair(out, 8, layer);
    if let Some(color) = aci {
        write_i32(out, 62, i32::from(color));
    }
    write_f64(out, 10, cx);
    write_f64(out, 20, cy);
    write_f64(out, 30, 0.0);
    write_f64(out, 40, radius);
    write_f64(out, 50, dxf_start.to_degrees());
    write_f64(out, 51, dxf_end.to_degrees());
}

fn write_text(
    out: &mut String,
    layer: &str,
    position: Point,
    height: f64,
    rotation_deg: f64,
    text: &str,
    aci: Option<u8>,
) {
    let cleaned = text.replace(['\r', '\n'], " ");
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return;
    }
    let (x, y) = to_dxf(position);
    write_pair(out, 0, "TEXT");
    write_pair(out, 8, layer);
    if let Some(color) = aci {
        write_i32(out, 62, i32::from(color));
    }
    write_f64(out, 10, x);
    write_f64(out, 20, y);
    write_f64(out, 30, 0.0);
    write_f64(out, 40, height);
    write_pair(out, 1, trimmed);
    if rotation_deg.abs() > 1e-4 {
        write_f64(out, 50, rotation_deg.rem_euclid(360.0));
    }
    write_pair(out, 7, "STANDARD");
}

fn linetype_to_dxf(line_type: LineType) -> &'static str {
    match line_type {
        LineType::Solid => "CONTINUOUS",
        LineType::Dashed => "DASHED",
        LineType::DashDot => "DASHDOT",
        LineType::Dotted => "DOTTED",
    }
}

fn linetype_from_dxf(name: &str) -> LineType {
    let upper = name.trim().to_ascii_uppercase();
    if upper.contains("DASHDOT") || upper.contains("CENTER") || upper.contains("PHANTOM") {
        LineType::DashDot
    } else if upper.contains("DOT") {
        LineType::Dotted
    } else if upper.contains("DASH") || upper.contains("HIDDEN") {
        LineType::Dashed
    } else {
        LineType::Solid
    }
}

/// Curated AutoCAD Color Index (ACI) table covering standard architectural colours.
const ACI_PALETTE: &[(u8, [u8; 3])] = &[
    (1, [255, 0, 0]),       // Red
    (2, [255, 255, 0]),     // Yellow
    (3, [0, 255, 0]),       // Green
    (4, [0, 255, 255]),     // Cyan
    (5, [0, 0, 255]),       // Blue
    (6, [255, 0, 255]),     // Magenta
    (7, [35, 38, 46]),      // Default white/black
    (8, [128, 128, 128]),   // Dark grey
    (9, [192, 192, 192]),   // Light grey
    (10, [220, 60, 50]),    // Brick / red-orange
    (30, [215, 130, 50]),   // Orange / wood
    (40, [195, 160, 75]),   // Warm ochre
    (52, [160, 145, 90]),   // Sand / beige
    (82, [85, 150, 95]),    // Architectural green
    (132, [75, 155, 165]),  // Teal
    (150, [94, 139, 191]),  // Terrain blue
    (170, [65, 105, 180]),  // Cobalt blue
    (210, [145, 95, 175]),  // Purple
    (251, [90, 96, 108]),   // Charcoal
    (253, [165, 174, 188]), // Silver
    (254, [210, 215, 222]), // Pale grey
];

/// Converts an RGB colour to the nearest AutoCAD Color Index (1..=255).
pub fn rgb_to_aci(rgb: [u8; 3]) -> u8 {
    let mut best_aci = 7u8;
    let mut best_dist = u32::MAX;
    for &(aci, [pr, pg, pb]) in ACI_PALETTE {
        let dr = i32::from(rgb[0]) - i32::from(pr);
        let dg = i32::from(rgb[1]) - i32::from(pg);
        let db = i32::from(rgb[2]) - i32::from(pb);
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best_aci = aci;
        }
    }
    best_aci
}

/// Converts an AutoCAD Color Index (positive or negative) to RGB.
pub fn aci_to_rgb(aci: i32) -> [u8; 3] {
    let abs = aci.unsigned_abs().min(255) as u8;
    if let Some(&(_, rgb)) = ACI_PALETTE.iter().find(|&&(code, _)| code == abs) {
        return rgb;
    }
    match abs {
        0 | 7 => [35, 38, 46],
        10..=29 => [210, 75, 65],
        30..=49 => [205, 140, 60],
        50..=79 => [150, 165, 65],
        80..=119 => [75, 155, 95],
        120..=149 => [70, 155, 170],
        150..=189 => [80, 120, 185],
        190..=229 => [145, 95, 175],
        230..=249 => [185, 85, 130],
        250..=255 => {
            let v = 60u8.saturating_add((abs - 250).saturating_mul(36));
            [v, v, v]
        }
        _ => [90, 96, 108],
    }
}

// ---------------------------------------------------------------------------
// DXF Importer
// ---------------------------------------------------------------------------

struct DxfPair<'a> {
    code: i32,
    value: &'a str,
}

fn tokenize_dxf(content: &str) -> Result<Vec<DxfPair<'_>>, String> {
    let mut lines = content.lines();
    let mut pairs = Vec::new();
    while let Some(code_line) = lines.next() {
        let trimmed_code = code_line.trim();
        if trimmed_code.is_empty() {
            continue;
        }
        let code = trimmed_code
            .parse::<i32>()
            .map_err(|_| format!("Código DXF inválido: '{trimmed_code}'"))?;
        let value = lines.next().unwrap_or("").trim_end_matches('\r');
        pairs.push(DxfPair { code, value });
    }
    if pairs.is_empty() {
        return Err("Arquivo DXF vazio.".to_owned());
    }
    Ok(pairs)
}

fn insunits_scale(code: i32) -> (f64, &'static str) {
    match code {
        1 => (0.0254, "polegadas"),
        2 => (0.3048, "pés"),
        4 => (0.001, "milímetros"),
        5 => (0.01, "centímetros"),
        6 => (1.0, "metros"),
        _ => (1.0, "metros"),
    }
}

fn is_wall_layer(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    lower.contains("parede")
        || lower.contains("wall")
        || lower.contains("alvenaria")
        || lower.contains("muro")
}

fn is_terrain_layer(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    lower.contains("terreno") || lower == "lote" || lower.contains("site") || lower.contains("lot")
}

/// Parses ASCII DXF `content` and merges its layers and entities into `project`.
pub fn import_str(project: &mut Project, content: &str) -> Result<DxfImportReport, String> {
    let pairs = tokenize_dxf(content)?;
    let mut report = DxfImportReport {
        units_label: "metros",
        ..Default::default()
    };

    let mut scale = 1.0_f64;
    let mut layer_ids = HashMap::<String, u64>::new();
    for layer in &project.layers {
        layer_ids.insert(sanitize_layer_name(&layer.name).to_uppercase(), layer.id);
        layer_ids.insert(layer.name.trim().to_uppercase(), layer.id);
    }

    let mut i = 0usize;
    let mut section = String::new();

    while i < pairs.len() {
        let pair = &pairs[i];
        if pair.code == 0 && pair.value.trim().eq_ignore_ascii_case("SECTION") {
            if i + 1 < pairs.len() && pairs[i + 1].code == 2 {
                section = pairs[i + 1].value.trim().to_ascii_uppercase();
                i += 2;
                continue;
            }
        } else if pair.code == 0 && pair.value.trim().eq_ignore_ascii_case("ENDSEC") {
            section.clear();
            i += 1;
            continue;
        }

        match section.as_str() {
            "HEADER" => {
                if pair.code == 9 && pair.value.trim().eq_ignore_ascii_case("$INSUNITS") {
                    if i + 1 < pairs.len()
                        && pairs[i + 1].code == 70
                        && let Ok(code) = pairs[i + 1].value.trim().parse::<i32>()
                    {
                        let (s, label) = insunits_scale(code);
                        scale = s;
                        report.units_label = label;
                    }
                    i += 2;
                    continue;
                }
                i += 1;
            }
            "TABLES" => {
                if pair.code == 0 && pair.value.trim().eq_ignore_ascii_case("LAYER") {
                    i += 1;
                    let mut name = String::new();
                    let mut color_aci = 7i32;
                    let mut flags = 0i32;
                    let mut ltype = LineType::Solid;
                    while i < pairs.len() && pairs[i].code != 0 {
                        match pairs[i].code {
                            2 => name = pairs[i].value.trim().to_owned(),
                            62 => {
                                if let Ok(c) = pairs[i].value.trim().parse::<i32>() {
                                    color_aci = c;
                                }
                            }
                            70 => {
                                if let Ok(f) = pairs[i].value.trim().parse::<i32>() {
                                    flags = f;
                                }
                            }
                            6 => ltype = linetype_from_dxf(pairs[i].value),
                            _ => {}
                        }
                        i += 1;
                    }
                    if !name.is_empty() && name != "0" {
                        let visible = color_aci >= 0;
                        let locked = (flags & 4) != 0;
                        let rgb = aci_to_rgb(color_aci);
                        let key = name.to_uppercase();
                        if let Some(&existing_id) = layer_ids.get(&key) {
                            if let Some(layer) =
                                project.layers.iter_mut().find(|l| l.id == existing_id)
                            {
                                layer.color = rgb;
                                layer.line_type = ltype;
                                layer.visible = visible;
                                layer.locked = locked;
                            }
                        } else if let Ok(new_id) = project.add_layer(&name, rgb, 0.25, ltype) {
                            if let Some(layer) = project.layers.iter_mut().find(|l| l.id == new_id)
                            {
                                layer.visible = visible;
                                layer.locked = locked;
                            }
                            layer_ids.insert(key, new_id);
                            layer_ids.insert(sanitize_layer_name(&name).to_uppercase(), new_id);
                            report.layers += 1;
                        }
                    }
                    continue;
                }
                i += 1;
            }
            "ENTITIES" => {
                if pair.code == 0 {
                    let entity_type = pair.value.trim().to_ascii_uppercase();
                    i += 1;
                    let start_idx = i;
                    while i < pairs.len() && pairs[i].code != 0 {
                        i += 1;
                    }
                    let slice = &pairs[start_idx..i];

                    match entity_type.as_str() {
                        "LINE" => {
                            import_line(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "LWPOLYLINE" => {
                            import_lwpolyline(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "POLYLINE" => {
                            let closed = slice
                                .iter()
                                .find(|p| p.code == 70)
                                .and_then(|p| p.value.trim().parse::<i32>().ok())
                                .is_some_and(|f| (f & 1) != 0);
                            let layer_name = find_str(slice, 8).unwrap_or("0");
                            let mut pts = Vec::new();
                            while i < pairs.len() {
                                if pairs[i].code == 0
                                    && pairs[i].value.trim().eq_ignore_ascii_case("VERTEX")
                                {
                                    i += 1;
                                    let v_start = i;
                                    while i < pairs.len() && pairs[i].code != 0 {
                                        i += 1;
                                    }
                                    let v_slice = &pairs[v_start..i];
                                    if let (Some(x), Some(y)) =
                                        (find_f64(v_slice, 10), find_f64(v_slice, 20))
                                    {
                                        pts.push(from_dxf(x, y, scale));
                                    }
                                } else if pairs[i].code == 0
                                    && pairs[i].value.trim().eq_ignore_ascii_case("SEQEND")
                                {
                                    i += 1;
                                    while i < pairs.len() && pairs[i].code != 0 {
                                        i += 1;
                                    }
                                    break;
                                } else {
                                    break;
                                }
                            }
                            import_polyline_points(
                                project,
                                layer_name,
                                pts,
                                closed,
                                &mut layer_ids,
                                &mut report,
                            );
                        }
                        "CIRCLE" => {
                            import_circle(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "ARC" => {
                            import_arc(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "SPLINE" => {
                            import_spline(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "TEXT" | "MTEXT" => {
                            import_text(
                                project,
                                slice,
                                scale,
                                entity_type == "MTEXT",
                                &mut layer_ids,
                                &mut report,
                            );
                        }
                        "DIMENSION" => {
                            import_dimension(project, slice, scale, &mut report);
                        }
                        "HATCH" => {
                            import_hatch(project, slice, scale, &mut layer_ids, &mut report);
                        }
                        "ENDSEC" | "EOF" | "SEQEND" | "VERTEX" => {}
                        _ => {
                            report.skipped_entities += 1;
                        }
                    }
                    continue;
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    Ok(report)
}

fn ensure_layer_id(
    project: &mut Project,
    layer_name: &str,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) -> Option<u64> {
    let trimmed = layer_name.trim();
    if trimmed.is_empty() || trimmed == "0" {
        return None;
    }
    let key = trimmed.to_uppercase();
    if let Some(&id) = layer_ids.get(&key) {
        return Some(id);
    }
    let clean_key = sanitize_layer_name(trimmed).to_uppercase();
    if let Some(&id) = layer_ids.get(&clean_key) {
        return Some(id);
    }
    if let Ok(id) = project.add_layer(trimmed, [90, 96, 108], 0.25, LineType::Solid) {
        layer_ids.insert(key, id);
        layer_ids.insert(clean_key, id);
        report.layers += 1;
        return Some(id);
    }
    None
}

fn find_str<'a>(pairs: &'a [DxfPair<'a>], code: i32) -> Option<&'a str> {
    pairs
        .iter()
        .find(|p| p.code == code)
        .map(|p| p.value.trim())
}

fn find_f64(pairs: &[DxfPair<'_>], code: i32) -> Option<f64> {
    pairs
        .iter()
        .find(|p| p.code == code)
        .and_then(|p| p.value.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

fn import_line(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
        find_f64(slice, 10),
        find_f64(slice, 20),
        find_f64(slice, 11),
        find_f64(slice, 21),
    ) else {
        return;
    };
    let a = from_dxf(x1, y1, scale);
    let b = from_dxf(x2, y2, scale);
    if a.distance(b) < 0.01 {
        return;
    }
    let layer_name = find_str(slice, 8).unwrap_or("0");
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);

    if is_wall_layer(layer_name)
        && let Some(wall_id) = project.add_wall(a, b)
    {
        if let Some(wall) = project.walls.iter_mut().find(|w| w.id == wall_id) {
            wall.layer_id = layer_id;
        }
        report.walls += 1;
        return;
    }

    if let Ok(id) = project.add_drawing(Shape::Line { start: a, end: b }) {
        if let Some(d) = project.drawings.iter_mut().find(|d| d.id == id) {
            d.layer_id = layer_id;
        }
        report.drawings += 1;
    }
}

fn import_lwpolyline(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let layer_name = find_str(slice, 8).unwrap_or("0");
    let closed = slice
        .iter()
        .find(|p| p.code == 70)
        .and_then(|p| p.value.trim().parse::<i32>().ok())
        .is_some_and(|f| (f & 1) != 0);

    let mut pts = Vec::new();
    let mut pending_x: Option<f64> = None;
    for pair in slice {
        match pair.code {
            10 => {
                pending_x = pair
                    .value
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite());
            }
            20 => {
                if let (Some(x), Some(y)) = (
                    pending_x.take(),
                    pair.value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite()),
                ) {
                    pts.push(from_dxf(x, y, scale));
                }
            }
            _ => {}
        }
    }
    import_polyline_points(project, layer_name, pts, closed, layer_ids, report);
}

/// Checks whether a closed 4-vertex polygon is a thin wall rectangle and,
/// if so, returns its centreline endpoints `(start, end)`.
fn extract_wall_centerline(pts: &[Point]) -> Option<(Point, Point)> {
    if pts.len() != 4 {
        return None;
    }
    let d01 = pts[0].distance(pts[1]);
    let d12 = pts[1].distance(pts[2]);
    let d23 = pts[2].distance(pts[3]);
    let d30 = pts[3].distance(pts[0]);

    // Opposite sides must match within tolerance.
    if (d01 - d23).abs() > 0.03 || (d12 - d30).abs() > 0.03 {
        return None;
    }

    let (short, long, start, end) = if d01 >= d12 {
        (d12, d01, pts[0].lerp(pts[3], 0.5), pts[1].lerp(pts[2], 0.5))
    } else {
        (d01, d12, pts[0].lerp(pts[1], 0.5), pts[3].lerp(pts[2], 0.5))
    };

    if (0.05..=0.60).contains(&short) && long >= 0.24 && long / short >= 1.4 {
        Some((start, end))
    } else {
        None
    }
}

fn import_polyline_points(
    project: &mut Project,
    layer_name: &str,
    mut pts: Vec<Point>,
    closed: bool,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    // Drop trailing duplicate vertex when explicitly closed.
    if closed && pts.len() > 2 && pts.first().unwrap().near(*pts.last().unwrap(), 1e-4) {
        pts.pop();
    }
    if pts.len() < 2 {
        return;
    }
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);

    if is_wall_layer(layer_name) {
        if closed
            && let Some((start, end)) = extract_wall_centerline(&pts)
            && let Some(wall_id) = project.add_wall(start, end)
        {
            if let Some(wall) = project.walls.iter_mut().find(|w| w.id == wall_id) {
                wall.layer_id = layer_id;
            }
            report.walls += 1;
            return;
        }
        // Open or multi-segment polyline on wall layer -> import each segment as a wall.
        let seg_count = if closed { pts.len() } else { pts.len() - 1 };
        let mut added = 0;
        for idx in 0..seg_count {
            let a = pts[idx];
            let b = pts[(idx + 1) % pts.len()];
            if let Some(wall_id) = project.add_wall(a, b) {
                if let Some(wall) = project.walls.iter_mut().find(|w| w.id == wall_id) {
                    wall.layer_id = layer_id;
                }
                added += 1;
            }
        }
        if added > 0 {
            report.walls += added;
            return;
        }
    }

    if closed && pts.len() >= 3 && is_terrain_layer(layer_name) && project.terrain.is_none() {
        let min_x = pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let min_y = pts.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
        let max_x = pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        let max_y = pts.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
        if project
            .set_terrain(Point::new(min_x, min_y), Point::new(max_x, max_y))
            .is_some()
        {
            if let Some(terrain) = &mut project.terrain {
                terrain.vertices = pts;
            }
            report.terrain_imported = true;
            return;
        }
    }

    if let Ok(id) = project.add_drawing(Shape::Polyline {
        points: pts,
        closed,
    }) {
        if let Some(d) = project.drawings.iter_mut().find(|d| d.id == id) {
            d.layer_id = layer_id;
        }
        report.drawings += 1;
    }
}

fn import_circle(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let (Some(cx), Some(cy), Some(r)) = (
        find_f64(slice, 10),
        find_f64(slice, 20),
        find_f64(slice, 40),
    ) else {
        return;
    };
    let radius = r * scale;
    if radius < 0.01 {
        return;
    }
    let layer_name = find_str(slice, 8).unwrap_or("0");
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);
    if let Ok(id) = project.add_drawing(Shape::Circle {
        center: from_dxf(cx, cy, scale),
        radius,
    }) {
        if let Some(d) = project.drawings.iter_mut().find(|d| d.id == id) {
            d.layer_id = layer_id;
        }
        report.drawings += 1;
    }
}

fn import_arc(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let (Some(cx), Some(cy), Some(r), Some(a0_deg), Some(a1_deg)) = (
        find_f64(slice, 10),
        find_f64(slice, 20),
        find_f64(slice, 40),
        find_f64(slice, 50),
        find_f64(slice, 51),
    ) else {
        return;
    };
    let radius = r * scale;
    if radius < 0.01 {
        return;
    }
    let dxf_sweep_deg = (a1_deg - a0_deg).rem_euclid(360.0);
    if dxf_sweep_deg < 1e-3 {
        return;
    }
    // Inverse of Y-flip: DXF CCW from a0 to a1 corresponds to Trazo start=-a1, positive sweep.
    let start = (-a1_deg.to_radians()).rem_euclid(TAU);
    let sweep = dxf_sweep_deg.to_radians();

    let layer_name = find_str(slice, 8).unwrap_or("0");
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);
    if let Ok(id) = project.add_drawing(Shape::Arc {
        center: from_dxf(cx, cy, scale),
        radius,
        start,
        sweep,
    }) {
        if let Some(d) = project.drawings.iter_mut().find(|d| d.id == id) {
            d.layer_id = layer_id;
        }
        report.drawings += 1;
    }
}

fn import_spline(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let closed = slice
        .iter()
        .find(|p| p.code == 70)
        .and_then(|p| p.value.trim().parse::<i32>().ok())
        .is_some_and(|f| (f & 1) != 0);

    let mut pts = Vec::new();
    let mut pending_x: Option<f64> = None;
    for pair in slice {
        match pair.code {
            10 | 11 => {
                pending_x = pair
                    .value
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite());
            }
            20 | 21 => {
                if let (Some(x), Some(y)) = (
                    pending_x.take(),
                    pair.value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite()),
                ) {
                    pts.push(from_dxf(x, y, scale));
                }
            }
            _ => {}
        }
    }
    if pts.len() < 3 {
        return;
    }
    let layer_name = find_str(slice, 8).unwrap_or("0");
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);
    if let Ok(id) = project.add_drawing(Shape::Spline {
        points: pts,
        closed,
    }) {
        if let Some(d) = project.drawings.iter_mut().find(|d| d.id == id) {
            d.layer_id = layer_id;
        }
        report.drawings += 1;
    }
}

fn clean_mtext(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                'P' | 'p' => {
                    out.push(' ');
                    i += 2;
                    continue;
                }
                '~' => {
                    out.push(' ');
                    i += 2;
                    continue;
                }
                'f' | 'F' | 'C' | 'H' | 'W' | 'T' | 'Q' | 'A' => {
                    i += 2;
                    while i < chars.len() && chars[i] != ';' {
                        i += 1;
                    }
                    if i < chars.len() {
                        i += 1;
                    }
                    continue;
                }
                'L' | 'l' | 'O' | 'o' => {
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        if chars[i] == '{' || chars[i] == '}' {
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out.trim().to_owned()
}

fn import_text(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    is_mtext: bool,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let (Some(x), Some(y)) = (find_f64(slice, 10), find_f64(slice, 20)) else {
        return;
    };
    let mut text = String::new();
    for pair in slice {
        if pair.code == 3 || pair.code == 1 {
            text.push_str(pair.value);
        }
    }
    let cleaned = if is_mtext {
        clean_mtext(&text)
    } else {
        text.trim().to_owned()
    };
    if cleaned.is_empty() {
        return;
    }
    let layer_name = find_str(slice, 8).unwrap_or("0");
    let layer_id = ensure_layer_id(project, layer_name, layer_ids, report);
    let pos = from_dxf(x, y, scale);
    let rotation_deg = find_f64(slice, 50).map(|deg| -deg).unwrap_or(0.0);
    let height = find_f64(slice, 40).map(|h| h * scale).unwrap_or(0.16);

    if let Ok(id) = project.add_note(pos, &cleaned, NoteKind::Text) {
        if let Some(note) = project.notes.iter_mut().find(|n| n.id == id) {
            note.layer_id = layer_id;
            note.rotation_deg = rotation_deg;
            note.font_size = (height / 0.014).clamp(8.0, 32.0);
        }
        report.notes += 1;
    }
}

fn import_dimension(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    report: &mut DxfImportReport,
) {
    let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
        find_f64(slice, 13),
        find_f64(slice, 23),
        find_f64(slice, 14),
        find_f64(slice, 24),
    ) else {
        return;
    };
    let a = from_dxf(x1, y1, scale);
    let b = from_dxf(x2, y2, scale);
    if a.distance(b) < 0.02 {
        return;
    }

    // Estimate signed perpendicular offset from definition line (10, 20).
    let offset = if let (Some(lx), Some(ly)) = (find_f64(slice, 10), find_f64(slice, 20)) {
        let line_pt = from_dxf(lx, ly, scale);
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = dx.hypot(dy).max(1e-6);
        let nx = -dy / len;
        let ny = dx / len;
        let proj = (line_pt.x - a.x) * nx + (line_pt.y - a.y) * ny;
        if proj.abs() < 0.05 { 0.40 } else { proj }
    } else {
        0.40
    };

    let kind = DimensionKind::Aligned {
        a: DimensionAnchor::from_point(a),
        b: DimensionAnchor::from_point(b),
        offset,
    };
    if let Ok(id) = project.add_dimension(kind) {
        if let Some(text) = find_str(slice, 1)
            && !text.is_empty()
            && text != "<>"
            && let Some(dim) = project.dimensions.iter_mut().find(|d| d.id == id)
        {
            dim.override_text = Some(text.to_owned());
        }
        report.dimensions += 1;
    }
}

fn import_hatch(
    project: &mut Project,
    slice: &[DxfPair<'_>],
    scale: f64,
    layer_ids: &mut HashMap<String, u64>,
    report: &mut DxfImportReport,
) {
    let layer_name = find_str(slice, 8).unwrap_or("Hachuras");
    let mut pts = Vec::new();
    let mut pending_x: Option<f64> = None;
    let mut in_boundary = false;
    for pair in slice {
        if pair.code == 92 {
            in_boundary = true;
            continue;
        }
        if !in_boundary {
            continue;
        }
        match pair.code {
            10 => {
                pending_x = pair
                    .value
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite());
            }
            20 => {
                if let (Some(x), Some(y)) = (
                    pending_x.take(),
                    pair.value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite()),
                ) {
                    pts.push(from_dxf(x, y, scale));
                }
            }
            _ => {}
        }
    }
    if pts.len() >= 3 {
        import_polyline_points(project, layer_name, pts, true, layer_ids, report);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        fixtures::FixtureKind,
        hatch::{HatchPattern, HatchStyle},
        samples::SampleProject,
    };

    #[test]
    fn round_trip_exports_and_reimports_walls_drawings_texts_dimensions_and_layers() {
        let mut project = Project::default();
        project.add_wall(Point::new(0.0, 0.0), Point::new(5.0, 0.0));
        project.add_wall(Point::new(5.0, 0.0), Point::new(5.0, 4.0));
        let _ = project.add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala");
        if let Some(room) = project.rooms.first_mut() {
            room.hatch = Some(HatchStyle::for_pattern(HatchPattern::CeramicGrid));
        }
        let wall_id = project.walls[0].id;
        let _ = project.add_opening(wall_id, OpeningKind::Door, 0.4);
        let _ = project.add_fixture(FixtureKind::Sofa, Point::new(2.0, 2.0), 2.0, 0.9, 0);
        let _ = project.add_drawing(Shape::Circle {
            center: Point::new(1.5, 1.5),
            radius: 0.45,
        });
        let _ = project.add_drawing(Shape::Arc {
            center: Point::new(3.0, 2.0),
            radius: 0.60,
            start: 0.0,
            sweep: FRAC_PI_2,
        });
        let _ = project.add_note(Point::new(2.5, 1.0), "NOTA TECNICA", NoteKind::Text);
        let _ = project.add_dimension(DimensionKind::Aligned {
            a: DimensionAnchor::from_point(Point::new(0.0, 0.0)),
            b: DimensionAnchor::from_point(Point::new(5.0, 0.0)),
            offset: -0.5,
        });

        let dxf = to_dxf_string(&project);
        assert!(dxf.contains("$ACADVER\n  1\nAC1009"));
        assert!(dxf.contains("$INSUNITS\n 70\n6"));
        assert!(dxf.contains("SECTION\n  2\nTABLES"));
        assert!(dxf.contains("SECTION\n  2\nBLOCKS"));
        assert!(dxf.contains("SECTION\n  2\nENTITIES"));
        assert!(dxf.contains("DIMENSION"));
        assert!(dxf.contains("Hachuras"));
        assert!(dxf.trim_end().ends_with("EOF"));

        let mut reimported = Project::default();
        let report = import_str(&mut reimported, &dxf).unwrap();
        assert!(
            report.walls >= 4,
            "expected walls reconstructed from exported polylines, got {}",
            report.walls
        );
        assert!(report.drawings >= 2);
        assert!(report.notes >= 1);
        assert_eq!(report.dimensions, 1);
        assert_eq!(report.units_label, "metros");
        assert!(
            reimported
                .notes
                .iter()
                .any(|n| n.text.contains("NOTA TECNICA"))
        );
    }

    #[test]
    fn imports_autocad_millimetre_dxf_with_lwpolyline_mtext_and_custom_layers() {
        // Minimal AutoCAD-style DXF in millimetres ($INSUNITS = 4).
        let dxf = "\
  0
SECTION
  2
HEADER
  9
$INSUNITS
 70
4
  0
ENDSEC
  0
SECTION
  2
TABLES
  0
TABLE
  2
LAYER
 70
2
  0
LAYER
  2
A-WALL
 70
0
 62
1
  6
CONTINUOUS
  0
LAYER
  2
A-ANNO
 70
0
 62
5
  6
DASHED
  0
ENDTAB
  0
ENDSEC
  0
SECTION
  2
ENTITIES
  0
LINE
  8
A-WALL
 10
0.0
 20
0.0
 11
4000.0
 21
0.0
  0
LWPOLYLINE
  8
A-WALL
 70
1
 10
0.0
 20
0.0
 10
4000.0
 20
0.0
 10
4000.0
 20
150.0
 10
0.0
 20
150.0
  0
CIRCLE
  8
A-ANNO
 10
2000.0
 20
1500.0
 40
500.0
  0
MTEXT
  8
A-ANNO
 10
1000.0
 20
2000.0
 40
180.0
  1
{\\fArial|b1;SALA DE ESTAR\\P16,00 m2}
  0
DIMENSION
  8
A-ANNO
 10
4000.0
 20
500.0
 13
0.0
 23
0.0
 14
4000.0
 24
0.0
  1
400
  0
ENDSEC
  0
EOF
";
        let mut project = Project::default();
        let report = import_str(&mut project, dxf).unwrap();
        assert_eq!(report.units_label, "milímetros");
        assert_eq!(report.walls, 2);
        assert_eq!(report.drawings, 1);
        assert_eq!(report.notes, 1);
        assert_eq!(report.dimensions, 1);

        // Verify 4000 mm scaled to 4.0 m.
        assert!((project.walls[0].length() - 4.0).abs() < 1e-4);
        // Verify custom layer color and linetype imported.
        let anno = project
            .layers
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case("A-ANNO"))
            .expect("A-ANNO layer imported");
        assert_eq!(anno.color, [0, 0, 255]);
        assert_eq!(anno.line_type, LineType::Dashed);
        // Verify MTEXT formatting codes stripped.
        assert_eq!(project.notes[0].text, "SALA DE ESTAR 16,00 m2");
    }

    #[test]
    fn all_sample_projects_export_to_valid_dxf() {
        for sample in SampleProject::ALL {
            let project = sample.load_embedded();
            let dxf = to_dxf_string(&project);
            assert!(dxf.contains("SECTION\n  2\nHEADER"));
            assert!(dxf.contains("SECTION\n  2\nTABLES"));
            assert!(dxf.contains("SECTION\n  2\nENTITIES"));
            assert!(dxf.trim_end().ends_with("EOF"));
            let sections = dxf.matches("\nSECTION\n").count();
            let endsecs = dxf.matches("\nENDSEC\n").count();
            assert_eq!(
                sections,
                endsecs,
                "unbalanced sections for {}",
                sample.file_name()
            );
        }
    }

    #[test]
    fn aci_color_mapping_round_trips_primary_colors() {
        assert_eq!(rgb_to_aci([255, 0, 0]), 1);
        assert_eq!(aci_to_rgb(1), [255, 0, 0]);
        assert_eq!(rgb_to_aci([0, 0, 255]), 5);
        assert_eq!(aci_to_rgb(-5), [0, 0, 255]);
    }

    #[test]
    fn floor_conduits_use_dashed_linetype_and_structure_labels_scale_with_sheet() {
        use crate::domain::{conduits::ConduitRun, fixtures::FixtureKind, sheet::DrawingScale};

        let mut project = Project::default();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let (pw, pd) = FixtureKind::Panel.default_size();
        let panel = project
            .add_fixture(FixtureKind::Panel, Point::new(0.4, 0.4), pw, pd, 0)
            .unwrap();
        let (ow, od) = FixtureKind::OutletLow.default_size();
        let outlet = project
            .add_fixture(FixtureKind::OutletLow, Point::new(3.6, 2.6), ow, od, 0)
            .unwrap();
        let _ = project
            .add_conduit(panel, outlet, ConduitRun::Floor)
            .unwrap();
        project.generate_structure();

        project.sheet.scale = DrawingScale::S1_50;
        let dxf_50 = to_dxf_string(&project);
        // Floor conduit POLYLINE carries entity-level DASHED linetype (group 6).
        let entities_50 = dxf_50.split("ENTITIES").nth(1).unwrap();
        assert!(
            entities_50.contains("  6\nDASHED\n"),
            "expected floor conduit to export with DASHED linetype"
        );
        // Structure text height at 1:50 is 2.2 / 20.0 = 0.11 m.
        assert!(entities_50.contains(" 40\n0.110000\n"));

        // Switching to a ceiling conduit removes the DASHED linetype override.
        project.conduits[0].run = ConduitRun::Ceiling;
        let dxf_ceiling = to_dxf_string(&project);
        let entities_ceiling = dxf_ceiling.split("ENTITIES").nth(1).unwrap();
        assert!(!entities_ceiling.contains("  6\nDASHED\n"));

        // At 1:100 structure text height doubles to 0.22 m.
        project.sheet.scale = DrawingScale::S1_100;
        let dxf_100 = to_dxf_string(&project);
        let entities_100 = dxf_100.split("ENTITIES").nth(1).unwrap();
        assert!(entities_100.contains(" 40\n0.220000\n"));
    }
}
