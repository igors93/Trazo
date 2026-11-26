//! The first-course plan and the wall elevations with block paging (#54).

use std::path::Path;

use super::{Drawn, INK, MUTED, Sheet, bounds, decimal, finish_all, fit_scale, write_pdf};
use crate::domain::construction::paging::{PieceKind, WallPaging, wall_paging};
use crate::domain::geometry::Point;
use crate::domain::masonry_structure::MasonryBandKind;
use crate::domain::model::Project;
use crate::domain::sheet::DrawingScale;

const WHOLE_FILL: &str = "#ffffff";
const HALF_FILL: &str = "#cfe3f7";
const CUT_FILL: &str = "#f6c99a";
const BAND_FILL: &str = "#c9ced6";

fn fill(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Whole => WHOLE_FILL,
        PieceKind::Half => HALF_FILL,
        PieceKind::Cut => CUT_FILL,
    }
}

/// The legend of the piece colours, at `(x, y)`.
fn legend(sheet: &mut Sheet, x: f64, y: f64) {
    sheet.text(x, y, 2.4, true, INK, "LEGENDA");
    for (row, (color, label)) in [
        (WHOLE_FILL, "Bloco inteiro"),
        (HALF_FILL, "Meio-bloco"),
        (CUT_FILL, "Bloco cortado na obra"),
        (BAND_FILL, "Verga (VA) / contraverga (CV) de concreto"),
    ]
    .into_iter()
    .enumerate()
    {
        let ry = y + 3.0 + row as f64 * 3.6;
        sheet.rect((x, ry, 6.0, 2.4), color, INK, 0.2);
        sheet.text(x + 8.0, ry + 2.0, 2.1, false, INK, label);
    }
}

/// The first-course plan: every wall's first course of blocks, the doors
/// that interrupt it, the lintels and sills, and the count of pieces.
pub(crate) fn first_course_drawing(project: &Project) -> Drawn {
    let paging = wall_paging(project);
    let mut sheet = Sheet::new();
    let (ax, ay, aw, ah) = sheet.area;
    let table_w = 100.0;
    let plan_w = aw - table_w - 8.0;
    let (min, max) = bounds(paging.iter().flat_map(|p| [p.start, p.end]), 0.6)
        .unwrap_or((Point::new(0.0, 0.0), Point::new(8.0, 6.0)));
    let (w_m, h_m) = (max.x - min.x, max.y - min.y);
    let scale = fit_scale(w_m, h_m, plan_w, ah - 8.0);
    let s = scale.mm_per_meter();
    let ox = ax + 4.0 + (plan_w - w_m * s) / 2.0;
    let oy = ay + 4.0 + (ah - 8.0 - h_m * s) / 2.0;
    let pt = |p: Point| (ox + (p.x - min.x) * s, oy + (p.y - min.y) * s);

    for wall in &paging {
        let length = wall.length_m.max(1e-9);
        let dir = Point::new(
            (wall.end.x - wall.start.x) / length,
            (wall.end.y - wall.start.y) / length,
        );
        let normal = Point::new(-dir.y, dir.x);
        let half = wall.masonry.block_width / 2.0;
        let at = |u: f64, n: f64| {
            pt(Point::new(
                wall.start.x + dir.x * u + normal.x * n,
                wall.start.y + dir.y * u + normal.y * n,
            ))
        };
        let thickness = wall.masonry.thickness() / 2.0;
        sheet.polygon(
            &[
                at(0.0, -thickness),
                at(length, -thickness),
                at(length, thickness),
                at(0.0, thickness),
            ],
            "#f3f6f9",
            "#9aa8b6",
            0.15,
        );
        for piece in wall.first_course() {
            let (a, b) = (piece.start_m, piece.start_m + piece.length_m);
            sheet.polygon(
                &[at(a, -half), at(b, -half), at(b, half), at(a, half)],
                fill(piece.kind),
                INK,
                0.18,
            );
        }
        for band in &wall.bands {
            let offset = thickness + 0.12;
            let side = if band.kind == MasonryBandKind::Lintel {
                offset
            } else {
                -offset
            };
            sheet.line(
                at(band.from_m, side),
                at(band.to_m, side),
                MUTED,
                0.3,
                Some("1.2 0.6"),
            );
            let (lx, ly) = at((band.from_m + band.to_m) / 2.0, side * 1.9);
            sheet.text(lx - 2.5, ly + 0.8, 1.8, false, MUTED, &band.label);
        }
        for opening in wall.openings.iter().filter(|o| o.bottom_m < 0.05) {
            let (lx, ly) = at((opening.from_m + opening.to_m) / 2.0, 0.0);
            sheet.text(lx - 2.5, ly + 0.8, 2.0, true, INK, &opening.code);
        }
        let (lx, ly) = at(length / 2.0, -(thickness + 0.35));
        sheet.text(
            lx - 2.0,
            ly + 0.8,
            2.0,
            true,
            "#2878d2",
            &wall.wall_id.to_string(),
        );
    }

    // Legend and the count of each wall's first course.
    let tx = ax + aw - table_w;
    legend(&mut sheet, tx, ay + 6.0);
    let mut y = ay + 27.0;
    sheet.text(tx, y, 2.4, true, INK, "PRIMEIRA FIADA POR PAREDE");
    y += 4.0;
    let rows = paging.len();
    let line_h = 3.1;
    let room = ((ay + ah - y - 16.0) / line_h).max(1.0) as usize;
    for wall in paging.iter().take(room) {
        let cut: Vec<String> = wall
            .first_course()
            .iter()
            .filter(|p| p.kind == PieceKind::Cut)
            .map(|p| format!("{:.0}", p.length_m * 100.0))
            .collect();
        let cut = if cut.is_empty() {
            String::new()
        } else {
            format!(" ({} cm)", cut.join(", "))
        };
        sheet.text(
            tx,
            y,
            2.0,
            false,
            INK,
            &format!(
                "{} · {} m: {} int., {} meios, {} cort.{cut}",
                wall.wall_id,
                decimal(wall.length_m, 2),
                wall.first.whole,
                wall.first.half,
                wall.first.cut
            ),
        );
        y += line_h;
    }
    if rows > room {
        sheet.text(
            tx,
            y,
            2.0,
            false,
            MUTED,
            &format!("… e mais {} parede(s): veja as elevações.", rows - room),
        );
        y += line_h;
    }
    let whole: usize = paging.iter().map(|p| p.first.whole).sum();
    let half: usize = paging.iter().map(|p| p.first.half).sum();
    let cut: usize = paging.iter().map(|p| p.first.cut).sum();
    y += 1.5;
    sheet.text(
        tx,
        y,
        2.2,
        true,
        INK,
        &format!("Total da 1ª fiada: {whole} inteiros, {half} meios, {cut} cortados"),
    );
    sheet.text(
        tx,
        y + 3.6,
        1.9,
        false,
        MUTED,
        "Comece pelos cantos, conferindo esquadro e nível; os números azuis são as paredes.",
    );
    Drawn {
        sheet,
        title: "Planta de primeira fiada".to_owned(),
        scale: scale.label(),
    }
}

/// One wall elevation with its courses, openings and bands, with its
/// ground line at `(x, base)`.
fn elevation(sheet: &mut Sheet, wall: &WallPaging, x: f64, base: f64, s: f64) {
    let ex = |u: f64| x + u * s;
    let ey = |h: f64| base - h * s;
    for course in &wall.courses {
        for piece in &course.pieces {
            sheet.rect(
                (
                    ex(piece.start_m),
                    ey(course.bottom_m + course.height_m),
                    piece.length_m * s,
                    course.height_m * s,
                ),
                fill(piece.kind),
                INK,
                0.12,
            );
        }
    }
    for band in &wall.bands {
        sheet.rect(
            (
                ex(band.from_m),
                ey(band.top_m),
                (band.to_m - band.from_m) * s,
                (band.top_m - band.bottom_m) * s,
            ),
            BAND_FILL,
            INK,
            0.18,
        );
        sheet.text(
            ex(band.from_m) + 0.6,
            ey(band.bottom_m) - 0.5,
            1.6,
            false,
            INK,
            &band.label,
        );
    }
    for opening in &wall.openings {
        sheet.rect(
            (
                ex(opening.from_m),
                ey(opening.top_m),
                (opening.to_m - opening.from_m) * s,
                (opening.top_m - opening.bottom_m) * s,
            ),
            "#ffffff",
            "#2878d2",
            0.3,
        );
        sheet.text(
            ex((opening.from_m + opening.to_m) / 2.0) - 2.5,
            ey((opening.top_m + opening.bottom_m) / 2.0),
            2.0,
            true,
            "#2878d2",
            &opening.code,
        );
    }
    let (t0, t1) = wall.top_m;
    sheet.polygon(
        &[
            (ex(0.0), ey(0.0)),
            (ex(wall.length_m), ey(0.0)),
            (ex(wall.length_m), ey(t1)),
            (ex(0.0), ey(t0)),
        ],
        "none",
        INK,
        0.4,
    );
    sheet.line(
        (ex(-0.2), ey(0.0)),
        (ex(wall.length_m + 0.2), ey(0.0)),
        INK,
        0.5,
        None,
    );
    sheet.text(
        x,
        base + 4.0,
        2.2,
        true,
        INK,
        &format!(
            "Parede {} · {} m · {} fiada(s)",
            wall.wall_id,
            decimal(wall.length_m, 2),
            wall.courses.len()
        ),
    );
    sheet.text(x, base + 7.0, 1.9, false, MUTED, &wall.total.summary());
}

/// The wall elevations, as many sheets as they need.
pub(crate) fn elevation_drawings(project: &Project) -> Vec<Drawn> {
    let paging: Vec<WallPaging> = wall_paging(project)
        .into_iter()
        .filter(|w| !w.courses.is_empty())
        .collect();
    if paging.is_empty() {
        return Vec::new();
    }
    let probe = Sheet::new();
    let (_, _, aw, ah) = probe.area;
    let longest = paging.iter().map(|w| w.length_m).fold(0.5, f64::max);
    let tallest = paging
        .iter()
        .map(|w| w.top_m.0.max(w.top_m.1))
        .fold(1.0, f64::max);
    let scale = [
        DrawingScale::S1_50,
        DrawingScale::S1_75,
        DrawingScale::S1_100,
        DrawingScale::S1_200,
    ]
    .into_iter()
    .find(|scale| scale.fits(longest, tallest + 0.6, aw - 10.0, ah - 14.0))
    .unwrap_or(DrawingScale::S1_200);
    let s = scale.mm_per_meter();
    let row_h = tallest * s + 14.0;
    let mut drawn = Vec::new();
    let mut sheet = Sheet::new();
    let (mut x, mut base) = (sheet.area.0 + 5.0, sheet.area.1 + 6.0 + tallest * s);
    let mut on_sheet = 0;
    for wall in &paging {
        let width = wall.length_m * s;
        let (ax, ay, aw, ah) = sheet.area;
        if x + width > ax + aw - 5.0 && on_sheet > 0 {
            x = ax + 5.0;
            base += row_h;
        }
        if base + 8.0 > ay + ah && on_sheet > 0 {
            legend(&mut sheet, ax + aw - 70.0, ay + ah - 20.0);
            drawn.push(Drawn {
                sheet,
                title: "Paginação das paredes (elevações)".to_owned(),
                scale: scale.label(),
            });
            sheet = Sheet::new();
            x = sheet.area.0 + 5.0;
            base = sheet.area.1 + 6.0 + tallest * s;
            on_sheet = 0;
        }
        elevation(&mut sheet, wall, x, base, s);
        x += width.max(40.0) + 12.0;
        on_sheet += 1;
    }
    let (ax, ay, aw, ah) = sheet.area;
    legend(&mut sheet, ax + aw - 70.0, ay + ah - 20.0);
    drawn.push(Drawn {
        sheet,
        title: "Paginação das paredes (elevações)".to_owned(),
        scale: scale.label(),
    });
    drawn
}

/// The first-course plan followed by the wall elevations.
pub fn paging_pages(project: &Project) -> Vec<String> {
    let mut drawn = vec![first_course_drawing(project)];
    drawn.extend(elevation_drawings(project));
    finish_all(project, drawn)
}

/// The first-course plan alone, as one SVG sheet.
pub fn first_course_svg(project: &Project) -> String {
    super::finish(project, first_course_drawing(project), "01/01")
}

/// Writes the first-course plan and the wall elevations as one PDF.
pub fn export_paging_pdf(project: &Project, path: &Path) -> Result<usize, String> {
    write_pdf(&paging_pages(project), path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_house_gets_a_first_course_sheet_and_elevations() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let pages = paging_pages(&project);
        assert!(pages.len() >= 2);
        assert!(pages[0].contains("Planta de primeira fiada"));
        assert!(pages[0].contains("PRIMEIRA FIADA POR PAREDE"));
        assert!(pages[1].contains("fiada(s)"));
        assert!(pages.iter().all(|p| p.contains("01/") || p.contains("/0")));
    }
}
