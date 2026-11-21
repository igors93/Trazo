//! Architectural sections (cortes), elevations (fachadas), detail callouts
//! (detalhes) and cross-drawing sheet references.

use std::{fmt::Write as _, fs, path::Path};

use serde::{Deserialize, Serialize};

use super::{
    geometry::{Point, line_intersection},
    hatch::{HatchPattern, hatch_segments_in_polygon},
    model::{OpeningKind, Project},
    notes::format_elevation,
    sheet::DrawingScale,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionLine {
    pub id: u64,
    /// Technical identifier, e.g. `"AA"`, `"BB"`.
    pub label: String,
    pub start: Point,
    pub end: Point,
    /// Viewing direction: `true` looks to the left of `start -> end`, `false` to the right.
    #[serde(default = "default_true")]
    pub look_left: bool,
    /// Optional target sheet code (e.g. `"PR-02"` or `"02/03"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_sheet: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElevationSide {
    North,
    South,
    East,
    West,
}

impl ElevationSide {
    pub const ALL: [ElevationSide; 4] = [
        ElevationSide::South,
        ElevationSide::North,
        ElevationSide::East,
        ElevationSide::West,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::North => "Fachada Norte",
            Self::South => "Fachada Sul",
            Self::East => "Fachada Leste",
            Self::West => "Fachada Oeste",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::North => "Norte",
            Self::South => "Sul",
            Self::East => "Leste",
            Self::West => "Oeste",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElevationView {
    pub id: u64,
    pub label: String,
    pub side: ElevationSide,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_sheet: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetailCallout {
    pub id: u64,
    /// Technical identifier, e.g. `"DET-01"`.
    pub label: String,
    pub center: Point,
    /// Radius of the callout circle on plan, in metres.
    pub radius: f64,
    #[serde(default = "default_detail_scale")]
    pub scale: DrawingScale,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_sheet: Option<String>,
}

fn default_detail_scale() -> DrawingScale {
    DrawingScale::S1_25
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectViews {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SectionLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elevations: Vec<ElevationView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<DetailCallout>,
}

impl ProjectViews {
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty() && self.elevations.is_empty() && self.details.is_empty()
    }
}

impl Project {
    fn next_view_id(&self) -> u64 {
        let max_s = self.views.sections.iter().map(|v| v.id).max().unwrap_or(0);
        let max_e = self
            .views
            .elevations
            .iter()
            .map(|v| v.id)
            .max()
            .unwrap_or(0);
        let max_d = self.views.details.iter().map(|v| v.id).max().unwrap_or(0);
        max_s.max(max_e).max(max_d) + 1
    }

    pub fn add_section_line(
        &mut self,
        label: &str,
        start: Point,
        end: Point,
        look_left: bool,
        target_sheet: Option<String>,
    ) -> Result<u64, String> {
        if !start.is_finite() || !end.is_finite() || start.distance(end) < 0.20 {
            return Err("A linha de corte precisa ter pelo menos 20 cm.".to_owned());
        }
        let id = self.next_view_id();
        let clean = if label.trim().is_empty() {
            next_section_label(self.views.sections.len())
        } else {
            label.trim().to_uppercase()
        };
        self.views.sections.push(SectionLine {
            id,
            label: clean,
            start: start.rounded(),
            end: end.rounded(),
            look_left,
            target_sheet: normalize_ref(target_sheet),
        });
        Ok(id)
    }

    /// Creates a longitudinal (`horizontal = true`) or transversal section
    /// slicing right through the centre of the current project bounds.
    pub fn add_default_section(&mut self, horizontal: bool) -> Result<u64, String> {
        let (min, max) = self
            .content_bounds()
            .or_else(|| self.bounds())
            .unwrap_or((Point::new(0.0, 0.0), Point::new(8.0, 6.0)));
        let pad = 0.8;
        let (start, end) = if horizontal {
            let cy = (min.y + max.y) * 0.5;
            (Point::new(min.x - pad, cy), Point::new(max.x + pad, cy))
        } else {
            let cx = (min.x + max.x) * 0.5;
            (Point::new(cx, min.y - pad), Point::new(cx, max.y + pad))
        };
        let label = next_section_label(self.views.sections.len());
        self.add_section_line(&label, start, end, true, None)
    }

    pub fn remove_section(&mut self, id: u64) -> bool {
        let before = self.views.sections.len();
        self.views.sections.retain(|s| s.id != id);
        self.views.sections.len() != before
    }

    pub fn add_elevation_view(&mut self, side: ElevationSide, target_sheet: Option<String>) -> u64 {
        let id = self.next_view_id();
        self.views.elevations.push(ElevationView {
            id,
            label: side.label().to_owned(),
            side,
            target_sheet: normalize_ref(target_sheet),
        });
        id
    }

    pub fn remove_elevation(&mut self, id: u64) -> bool {
        let before = self.views.elevations.len();
        self.views.elevations.retain(|e| e.id != id);
        self.views.elevations.len() != before
    }

    pub fn add_detail_callout(
        &mut self,
        label: &str,
        center: Point,
        radius: f64,
        scale: DrawingScale,
        target_sheet: Option<String>,
    ) -> Result<u64, String> {
        if !center.is_finite() || !radius.is_finite() || !(0.25..=10.0).contains(&radius) {
            return Err("Raio da chamada de detalhe deve ficar entre 0,25 m e 10,0 m.".to_owned());
        }
        let id = self.next_view_id();
        let clean = if label.trim().is_empty() {
            format!("DET-{:02}", self.views.details.len() + 1)
        } else {
            label.trim().to_uppercase()
        };
        self.views.details.push(DetailCallout {
            id,
            label: clean,
            center: center.rounded(),
            radius,
            scale,
            target_sheet: normalize_ref(target_sheet),
        });
        Ok(id)
    }

    pub fn remove_detail_callout(&mut self, id: u64) -> bool {
        let before = self.views.details.len();
        self.views.details.retain(|d| d.id != id);
        self.views.details.len() != before
    }
}

fn next_section_label(existing: usize) -> String {
    let ch = (b'A' + (existing % 26) as u8) as char;
    format!("{ch}{ch}")
}

fn normalize_ref(r: Option<String>) -> Option<String> {
    r.and_then(|s| {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_owned())
    })
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Generates a 2D architectural section SVG along `section`, slicing through
/// walls, openings, floor slab and ceiling slab at `project.wall_height`.
pub fn section_svg(project: &Project, section: &SectionLine) -> String {
    let dir = section.end.minus(section.start);
    let total_len = dir.length().max(1.0);
    let u = Point::new(dir.x / total_len, dir.y / total_len);
    let look_n = if section.look_left {
        Point::new(-u.y, u.x)
    } else {
        Point::new(u.y, -u.x)
    };

    let wall_h = project.wall_height.max(2.0);
    let slab_h = 0.15;
    let px_per_m = 90.0;
    let margin = 65.0;
    let svg_w = (total_len * px_per_m + margin * 2.0).max(520.0);
    let svg_h = ((wall_h + 1.4) * px_per_m + margin * 2.0).max(380.0);
    let ground_y = svg_h - margin - 24.0;

    let sx = |dist_m: f64| margin + dist_m * px_per_m;
    let sy = |elev_m: f64| ground_y - elev_m * px_per_m;

    let style = &project.doc_style;
    let wall_col = style.wall_color();
    let dim_col = style.dimension_color();
    let op_col = style.opening_color();
    let font = esc(&style.font_family);

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{svg_w:.0}\" height=\"{svg_h:.0}\" viewBox=\"0 0 {svg_w:.0} {svg_h:.0}\">\n\
         <rect width=\"{svg_w:.0}\" height=\"{svg_h:.0}\" fill=\"#ffffff\"/>\n"
    );

    // Collect cut walls and background walls in the viewing half-plane.
    let mut cut_spans: Vec<(f64, f64)> = Vec::new();
    let mut min_cut = f64::INFINITY;
    let mut max_cut = f64::NEG_INFINITY;

    for wall in &project.walls {
        let a_rel = wall.start.minus(section.start);
        let b_rel = wall.end.minus(section.start);
        let a_u = a_rel.dot(u);
        let b_u = b_rel.dot(u);
        let a_n = a_rel.dot(look_n);
        let b_n = b_rel.dot(look_n);

        // Check if cut plane intersects wall segment.
        if let Some((_, t_sec, t_wall)) =
            line_intersection(section.start, section.end, wall.start, wall.end)
            && (0.0..=1.0).contains(&t_sec)
            && (-0.01..=1.01).contains(&t_wall)
        {
            let pos = t_sec * total_len;
            let half = wall.thickness() * 0.5;
            cut_spans.push((pos - half, pos + half));
            min_cut = min_cut.min(pos - half);
            max_cut = max_cut.max(pos + half);
            continue;
        }

        // Background wall visible in viewing direction.
        if a_n >= -0.05 || b_n >= -0.05 {
            let x0 = sx(a_u.min(b_u).clamp(0.0, total_len));
            let x1 = sx(a_u.max(b_u).clamp(0.0, total_len));
            if x1 - x0 > 2.0 {
                let top_y = sy(wall_h);
                let _ = writeln!(
                    svg,
                    "<rect x=\"{x0:.1}\" y=\"{top_y:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#f7fafd\" stroke=\"#94a3b8\" stroke-width=\"1.0\"/>",
                    x1 - x0,
                    ground_y - top_y
                );
                // Project openings on this background wall.
                for op in project.openings.iter().filter(|o| o.wall_id == wall.id) {
                    let op_pt = wall.start.lerp(wall.end, op.t);
                    let op_u = op_pt.minus(section.start).dot(u);
                    let op_w_proj = op.width()
                        * wall
                            .end
                            .minus(wall.start)
                            .normalized()
                            .map(|d| d.dot(u).abs())
                            .unwrap_or(1.0);
                    if op_w_proj < 0.08 {
                        continue;
                    }
                    let ox0 = sx(op_u - op_w_proj * 0.5);
                    let ow = op_w_proj * px_per_m;
                    let sill = op.sill();
                    let oh_m = op.height().unwrap_or(wall_h);
                    let oy_top = sy((sill + oh_m).min(wall_h));
                    let oy_bot = sy(sill);
                    let stroke = match op.kind {
                        OpeningKind::Window => op_col,
                        _ => wall_col,
                    };
                    let _ = writeln!(
                        svg,
                        "<rect x=\"{ox0:.1}\" y=\"{oy_top:.1}\" width=\"{ow:.1}\" height=\"{:.1}\" fill=\"#ffffff\" stroke=\"{stroke}\" stroke-width=\"1.4\"/>",
                        (oy_bot - oy_top).max(2.0)
                    );
                }
            }
        }
    }

    // Floor & ceiling slabs spanning the cut building envelope.
    let (slab_x0, slab_x1) = if min_cut.is_finite() && max_cut > min_cut {
        (sx(min_cut - 0.15), sx(max_cut + 0.15))
    } else {
        (sx(0.3), sx(total_len - 0.3))
    };
    let slab_px = slab_h * px_per_m;
    let _ = writeln!(
        svg,
        "<g id=\"lajes\">\n\
         <rect x=\"{slab_x0:.1}\" y=\"{ground_y:.1}\" width=\"{:.1}\" height=\"{slab_px:.1}\" fill=\"#cbd5e1\" stroke=\"{wall_col}\" stroke-width=\"1.6\"/>\n\
         <rect x=\"{slab_x0:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{slab_px:.1}\" fill=\"#cbd5e1\" stroke=\"{wall_col}\" stroke-width=\"1.6\"/>\n\
         </g>",
        slab_x1 - slab_x0,
        sy(wall_h + slab_h),
        slab_x1 - slab_x0
    );

    // Cut wall sections (dark solid profile per NBR 6492).
    let _ = writeln!(svg, "<g id=\"paredes-cortadas\">");
    for (x_start, x_end) in &cut_spans {
        let rx = sx(*x_start);
        let rw = ((x_end - x_start) * px_per_m).max(6.0);
        let top_y = sy(wall_h);
        let _ = writeln!(
            svg,
            "<rect x=\"{rx:.1}\" y=\"{top_y:.1}\" width=\"{rw:.1}\" height=\"{:.1}\" fill=\"{wall_col}\"/>",
            ground_y - top_y
        );
    }
    svg.push_str("</g>\n");

    // Ground line.
    let _ = writeln!(
        svg,
        "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{wall_col}\" stroke-width=\"2.2\"/>",
        margin - 20.0,
        ground_y + slab_px,
        svg_w - margin + 20.0,
        ground_y + slab_px
    );

    // Vertical ceiling height dimension & level marks.
    let dim_x = margin - 26.0;
    let _ = writeln!(
        svg,
        "<g id=\"cotas-corte\">\n\
         <line x1=\"{dim_x:.1}\" y1=\"{:.1}\" x2=\"{dim_x:.1}\" y2=\"{ground_y:.1}\" stroke=\"{dim_col}\" stroke-width=\"1.0\"/>\n\
         <text x=\"{:.1}\" y=\"{:.1}\" transform=\"rotate(-90 {:.1} {:.1})\" font-family=\"{font}\" font-size=\"12\" fill=\"{wall_col}\" text-anchor=\"middle\">{}</text>\n\
         <text x=\"{:.1}\" y=\"{:.1}\" font-family=\"{font}\" font-size=\"11\" font-weight=\"700\" fill=\"{wall_col}\">{}</text>\n\
         <text x=\"{:.1}\" y=\"{:.1}\" font-family=\"{font}\" font-size=\"11\" font-weight=\"700\" fill=\"{wall_col}\">{}</text>\n\
         </g>",
        sy(wall_h),
        dim_x - 8.0,
        (ground_y + sy(wall_h)) * 0.5,
        dim_x - 8.0,
        (ground_y + sy(wall_h)) * 0.5,
        esc(&style.format_length(wall_h)),
        slab_x1 + 10.0,
        ground_y + 4.0,
        esc(&format_elevation(0.0)),
        slab_x1 + 10.0,
        sy(wall_h) + 4.0,
        esc(&format_elevation(wall_h))
    );

    // Title + cross-sheet reference.
    let ref_str = section
        .target_sheet
        .as_deref()
        .map(|s| format!(" · Ref. Prancha {s}"))
        .unwrap_or_default();
    let _ = writeln!(
        svg,
        "<text x=\"{margin:.1}\" y=\"36\" font-family=\"{font}\" font-size=\"18\" font-weight=\"700\" fill=\"{wall_col}\">CORTE {} — {} (Escala {}{})</text>\n\
         </svg>",
        esc(&section.label),
        esc(&project.name),
        esc(&project.sheet.scale.label()),
        esc(&ref_str)
    );
    svg
}

/// Generates a 2D architectural facade elevation SVG looking at the chosen
/// compass side (`North`, `South`, `East`, `West`).
pub fn elevation_svg(project: &Project, elevation: &ElevationView) -> String {
    let (min, max) = project
        .content_bounds()
        .or_else(|| project.bounds())
        .unwrap_or((Point::new(0.0, 0.0), Point::new(8.0, 6.0)));
    let span_m = match elevation.side {
        ElevationSide::North | ElevationSide::South => (max.x - min.x).max(1.0),
        ElevationSide::East | ElevationSide::West => (max.y - min.y).max(1.0),
    };
    let wall_h = project.wall_height.max(2.0);
    let roof_h = 0.65;
    let px_per_m = 90.0;
    let margin = 65.0;
    let svg_w = (span_m * px_per_m + margin * 2.0).max(520.0);
    let svg_h = ((wall_h + roof_h + 1.2) * px_per_m + margin * 2.0).max(380.0);
    let ground_y = svg_h - margin - 24.0;

    let project_u = |p: Point| -> f64 {
        match elevation.side {
            ElevationSide::South => p.x - min.x,
            ElevationSide::North => max.x - p.x,
            ElevationSide::East => p.y - min.y,
            ElevationSide::West => max.y - p.y,
        }
    };
    let sx = |u_m: f64| margin + u_m * px_per_m;
    let sy = |elev_m: f64| ground_y - elev_m * px_per_m;

    let style = &project.doc_style;
    let wall_col = style.wall_color();
    let op_col = style.opening_color();
    let dim_col = style.dimension_color();
    let font = esc(&style.font_family);

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{svg_w:.0}\" height=\"{svg_h:.0}\" viewBox=\"0 0 {svg_w:.0} {svg_h:.0}\">\n\
         <rect width=\"{svg_w:.0}\" height=\"{svg_h:.0}\" fill=\"#ffffff\"/>\n"
    );

    // Building facade silhouette + roof fascia.
    let bx0 = sx(0.0);
    let bw = span_m * px_per_m;
    let top_y = sy(wall_h);
    let roof_top = sy(wall_h + roof_h);
    let _ = writeln!(
        svg,
        "<g id=\"fachada-corpo\">\n\
         <rect x=\"{bx0:.1}\" y=\"{top_y:.1}\" width=\"{bw:.1}\" height=\"{:.1}\" fill=\"#f9fbfd\" stroke=\"{wall_col}\" stroke-width=\"1.8\"/>\n\
         <polygon points=\"{:.1},{top_y:.1} {:.1},{roof_top:.1} {:.1},{roof_top:.1} {:.1},{top_y:.1}\" fill=\"#e2e8f0\" stroke=\"{wall_col}\" stroke-width=\"1.5\"/>\n\
         </g>",
        ground_y - top_y,
        bx0 - 14.0,
        bx0 + 18.0,
        bx0 + bw - 18.0,
        bx0 + bw + 14.0
    );

    // Project exterior walls facing this elevation side + their doors/windows.
    let _ = writeln!(svg, "<g id=\"fachada-esquadrias\">");
    for wall in &project.walls {
        let on_face = match elevation.side {
            ElevationSide::South => {
                (wall.start.y - max.y).abs() < 0.35 && (wall.end.y - max.y).abs() < 0.35
            }
            ElevationSide::North => {
                (wall.start.y - min.y).abs() < 0.35 && (wall.end.y - min.y).abs() < 0.35
            }
            ElevationSide::East => {
                (wall.start.x - max.x).abs() < 0.35 && (wall.end.x - max.x).abs() < 0.35
            }
            ElevationSide::West => {
                (wall.start.x - min.x).abs() < 0.35 && (wall.end.x - min.x).abs() < 0.35
            }
        };
        if !on_face {
            continue;
        }
        for op in project.openings.iter().filter(|o| o.wall_id == wall.id) {
            let center = wall.start.lerp(wall.end, op.t);
            let cu = project_u(center);
            let ow = op.width() * px_per_m;
            let ox = sx(cu) - ow * 0.5;
            let sill = op.sill();
            let oh_m = op.height().unwrap_or(wall_h);
            let oy_top = sy((sill + oh_m).min(wall_h));
            let oy_bot = sy(sill);
            let oh = (oy_bot - oy_top).max(4.0);
            let (fill, stroke) = match op.kind {
                OpeningKind::Window => ("#e0f2fe", op_col),
                OpeningKind::Gap => ("#eef2f6", dim_col),
                _ => ("#f1f5f9", wall_col),
            };
            let _ = writeln!(
                svg,
                "<rect x=\"{ox:.1}\" y=\"{oy_top:.1}\" width=\"{ow:.1}\" height=\"{oh:.1}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.6\"/>"
            );
            if op.kind == OpeningKind::Window {
                use super::openings::WindowType;
                let mid_x = ox + ow * 0.5;
                let mid_y = (oy_top + oy_bot) * 0.5;
                match op.effective_window_type() {
                    WindowType::Sliding => {
                        let _ = writeln!(
                            svg,
                            "<line x1=\"{mid_x:.1}\" y1=\"{oy_top:.1}\" x2=\"{mid_x:.1}\" y2=\"{oy_bot:.1}\" stroke=\"{stroke}\" stroke-width=\"1.0\"/>"
                        );
                    }
                    WindowType::Basculante | WindowType::MaximAr => {
                        let _ = writeln!(
                            svg,
                            "<polyline points=\"{ox:.1},{oy_bot:.1} {mid_x:.1},{oy_top:.1} {:.1},{oy_bot:.1}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.0\" stroke-dasharray=\"4 3\"/>",
                            ox + ow
                        );
                    }
                    WindowType::Casement | WindowType::TiltTurn => {
                        let _ = writeln!(
                            svg,
                            "<line x1=\"{mid_x:.1}\" y1=\"{oy_top:.1}\" x2=\"{mid_x:.1}\" y2=\"{oy_bot:.1}\" stroke=\"{stroke}\" stroke-width=\"1.0\"/><polyline points=\"{ox:.1},{oy_top:.1} {mid_x:.1},{mid_y:.1} {ox:.1},{oy_bot:.1}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"0.9\" stroke-dasharray=\"4 3\"/>"
                        );
                    }
                    WindowType::Fixed | WindowType::BayWindow => {
                        let _ = writeln!(
                            svg,
                            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{stroke}\" stroke-width=\"0.9\"/>",
                            mid_x - 6.0,
                            mid_y - 6.0,
                            mid_x + 6.0,
                            mid_y + 6.0
                        );
                    }
                }
            }
        }
    }
    svg.push_str("</g>\n");

    // Ground line & total width dimension.
    let _ = writeln!(
        svg,
        "<line x1=\"{:.1}\" y1=\"{ground_y:.1}\" x2=\"{:.1}\" y2=\"{ground_y:.1}\" stroke=\"{wall_col}\" stroke-width=\"2.2\"/>\n\
         <line x1=\"{bx0:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{dim_col}\" stroke-width=\"1.0\"/>\n\
         <text x=\"{:.1}\" y=\"{:.1}\" font-family=\"{font}\" font-size=\"12\" fill=\"{wall_col}\" text-anchor=\"middle\">{}</text>",
        margin - 24.0,
        svg_w - margin + 24.0,
        ground_y + 22.0,
        bx0 + bw,
        ground_y + 22.0,
        bx0 + bw * 0.5,
        ground_y + 18.0,
        esc(&style.format_length(span_m))
    );

    let ref_str = elevation
        .target_sheet
        .as_deref()
        .map(|s| format!(" · Ref. Prancha {s}"))
        .unwrap_or_default();
    let _ = writeln!(
        svg,
        "<text x=\"{margin:.1}\" y=\"36\" font-family=\"{font}\" font-size=\"18\" font-weight=\"700\" fill=\"{wall_col}\">{} — {} (Escala {}{})</text>\n\
         </svg>",
        esc(&elevation.label.to_uppercase()),
        esc(&project.name),
        esc(&project.sheet.scale.label()),
        esc(&ref_str)
    );
    svg
}

/// Generates a magnified vector detail callout SVG centred on `detail.center`.
pub fn detail_svg(project: &Project, detail: &DetailCallout) -> String {
    let px_per_m = 180.0;
    let radius_px = (detail.radius * px_per_m).clamp(140.0, 260.0);
    let margin = 56.0;
    let svg_w = radius_px * 2.0 + margin * 2.0;
    let svg_h = radius_px * 2.0 + margin * 2.0 + 28.0;
    let cx = svg_w * 0.5;
    let cy = margin + radius_px + 20.0;

    let pt = |p: Point| -> (f64, f64) {
        (
            cx + (p.x - detail.center.x) * px_per_m,
            cy + (p.y - detail.center.y) * px_per_m,
        )
    };

    let style = &project.doc_style;
    let wall_col = style.wall_color();
    let op_col = style.opening_color();
    let font = esc(&style.font_family);

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{svg_w:.0}\" height=\"{svg_h:.0}\" viewBox=\"0 0 {svg_w:.0} {svg_h:.0}\">\n\
         <rect width=\"{svg_w:.0}\" height=\"{svg_h:.0}\" fill=\"#ffffff\"/>\n\
         <defs><clipPath id=\"detail-clip\"><circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{radius_px:.1}\"/></clipPath></defs>\n\
         <g clip-path=\"url(#detail-clip)\">\n"
    );

    // Room fills & hatches inside callout.
    for room in &project.rooms {
        let Some(poly) = project.room_polygon(room) else {
            continue;
        };
        let pts: String = poly
            .iter()
            .map(|p| {
                let (x, y) = pt(*p);
                format!("{x:.1},{y:.1}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        let fill = match room.hatch {
            Some(h) if h.pattern == HatchPattern::Solid && !style.monochrome => h.hex_color(),
            _ => style.room_fill().to_owned(),
        };
        let _ = writeln!(svg, "<polygon points=\"{pts}\" fill=\"{fill}\"/>");
        if let Some(h) = room.hatch {
            let stroke = if style.monochrome {
                "#444444".to_owned()
            } else {
                h.hex_color()
            };
            for (a, b) in hatch_segments_in_polygon(&poly, &h) {
                let (x1, y1) = pt(a);
                let (x2, y2) = pt(b);
                let _ = writeln!(
                    svg,
                    "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"{stroke}\" stroke-width=\"0.9\"/>"
                );
            }
        }
    }

    // Walls & openings.
    for wall in &project.walls {
        let (x1, y1) = pt(wall.start);
        let (x2, y2) = pt(wall.end);
        let w_px = (wall.thickness() * px_per_m).max(4.0);
        let _ = writeln!(
            svg,
            "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"{wall_col}\" stroke-width=\"{w_px:.1}\" stroke-linecap=\"square\"/>"
        );
        for op in project.openings.iter().filter(|o| o.wall_id == wall.id) {
            let center = wall.start.lerp(wall.end, op.t);
            let dir = wall
                .end
                .minus(wall.start)
                .normalized()
                .unwrap_or(Point::new(1.0, 0.0));
            let a = center.translated(dir.scaled(-op.width() * 0.5));
            let b = center.translated(dir.scaled(op.width() * 0.5));
            let (ox1, oy1) = pt(a);
            let (ox2, oy2) = pt(b);
            let _ = writeln!(
                svg,
                "<line x1=\"{ox1:.1}\" y1=\"{oy1:.1}\" x2=\"{ox2:.1}\" y2=\"{oy2:.1}\" stroke=\"#ffffff\" stroke-width=\"{:.1}\"/>\n\
                 <line x1=\"{ox1:.1}\" y1=\"{oy1:.1}\" x2=\"{ox2:.1}\" y2=\"{oy2:.1}\" stroke=\"{op_col}\" stroke-width=\"2.2\"/>",
                w_px + 1.0
            );
        }
    }

    svg.push_str("</g>\n");
    let _ = writeln!(
        svg,
        "<circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{radius_px:.1}\" fill=\"none\" stroke=\"{wall_col}\" stroke-width=\"2.0\" stroke-dasharray=\"10 5\"/>"
    );

    let ref_str = detail
        .target_sheet
        .as_deref()
        .map(|s| format!(" · Ref. Prancha {s}"))
        .unwrap_or_default();
    let _ = writeln!(
        svg,
        "<text x=\"{margin:.1}\" y=\"34\" font-family=\"{font}\" font-size=\"17\" font-weight=\"700\" fill=\"{wall_col}\">DETALHE {} — Escala {}{}</text>\n\
         </svg>",
        esc(&detail.label),
        esc(&detail.scale.label()),
        esc(&ref_str)
    );
    svg
}

pub fn export_view_svg(markup: &str, path: &Path) -> Result<(), String> {
    fs::write(path, markup).map_err(|e| e.to_string())
}

pub fn export_view_pdf(markup: &str, path: &Path) -> Result<(), String> {
    let options = crate::io::export::svg2pdf_options();
    let tree = svg2pdf::usvg::Tree::from_str(markup, &options).map_err(|e| e.to_string())?;
    let pdf = svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|e| e.to_string())?;
    fs::write(path, pdf).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_elevation_and_detail_generate_valid_technical_svg_with_sheet_refs() {
        let mut project = Project::default();
        project.name = "Casa Teste".to_owned();
        project
            .add_room(Point::new(0.0, 0.0), Point::new(6.0, 4.0), "Sala")
            .unwrap();
        let south_wall = project
            .walls
            .iter()
            .find(|w| (w.start.y - 4.0).abs() < 0.05 && (w.end.y - 4.0).abs() < 0.05)
            .unwrap()
            .id;
        project
            .add_opening(south_wall, OpeningKind::Window, 0.5)
            .unwrap();

        let sec_id = project
            .add_section_line(
                "AA",
                Point::new(-0.5, 2.0),
                Point::new(6.5, 2.0),
                true,
                Some("PR-02".to_owned()),
            )
            .unwrap();
        let sec = project
            .views
            .sections
            .iter()
            .find(|s| s.id == sec_id)
            .unwrap();
        let sec_markup = section_svg(&project, sec);
        assert!(sec_markup.contains("CORTE AA"));
        assert!(sec_markup.contains("Ref. Prancha PR-02"));
        assert!(sec_markup.contains("id=\"paredes-cortadas\""));
        assert!(sec_markup.contains("±0,00"));

        let elev_id = project.add_elevation_view(ElevationSide::South, Some("PR-03".to_owned()));
        let elev = project
            .views
            .elevations
            .iter()
            .find(|e| e.id == elev_id)
            .unwrap();
        let elev_markup = elevation_svg(&project, elev);
        assert!(elev_markup.contains("FACHADA SUL"));
        assert!(elev_markup.contains("Ref. Prancha PR-03"));
        assert!(elev_markup.contains("id=\"fachada-esquadrias\""));

        let det_id = project
            .add_detail_callout(
                "DET-01",
                Point::new(3.0, 2.0),
                1.2,
                DrawingScale::S1_25,
                Some("PR-04".to_owned()),
            )
            .unwrap();
        let det = project
            .views
            .details
            .iter()
            .find(|d| d.id == det_id)
            .unwrap();
        let det_markup = detail_svg(&project, det);
        assert!(det_markup.contains("DETALHE DET-01"));
        assert!(det_markup.contains("Escala 1:25"));
        assert!(det_markup.contains("Ref. Prancha PR-04"));

        assert!(project.remove_section(sec_id));
        assert!(project.remove_elevation(elev_id));
        assert!(project.remove_detail_callout(det_id));
        assert!(project.views.is_empty());
    }
}
