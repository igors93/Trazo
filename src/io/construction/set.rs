//! The one-click construction set (#57): everything a builder needs from
//! the plan, in one numbered A3 PDF. The dimensioned plan with the opening
//! chains, the door and window schedule and the legend; the setting-out
//! plan; the first-course plan and the wall elevations; the roof plan; and
//! the quantities takeoff.

use std::path::Path;

use super::paging::{elevation_drawings, first_course_drawing};
use super::roof_plan::roof_plan_drawing;
use super::setting_out::setting_out_drawing;
use super::{Drawn, finish};
use crate::domain::discipline::Discipline;
use crate::domain::model::Project;
use crate::domain::sheet::{DrawingScale, Orientation, PaperSize, SheetLayout};
use crate::io::export::ExportView;

/// The plan as it goes into the set: A3 landscape at 1:50 when the
/// building fits (the largest standard scale otherwise), with the opening
/// dimension chains added to a copy, never to the person's project.
fn set_plan_project(project: &Project) -> Project {
    let mut working = project.clone();
    let title_block = working.sheet.title_block.clone();
    let mut layout = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
    layout.title_block = title_block;
    layout.title_block.drawing_title = "Planta baixa cotada".to_owned();
    if let Some((min, max)) = working.content_bounds() {
        let (area_w, area_h) = layout.drawing_area_mm();
        let (w, h) = (max.x - min.x + 1.5, max.y - min.y + 1.5);
        layout.scale = if DrawingScale::S1_50.fits(w, h, area_w, area_h) {
            DrawingScale::S1_50
        } else {
            DrawingScale::best_fit(w, h, area_w, area_h)
        };
    }
    working.sheet = layout;
    let _ = working.generate_opening_dimension_chains(None, 0.45);
    working
}

/// The sheets of the set, in order, as SVG.
pub fn construction_set_pages(project: &Project) -> Vec<String> {
    let mut drawn: Vec<Drawn> = Vec::new();
    if let Some(setting_out) = setting_out_drawing(project) {
        drawn.push(setting_out);
    }
    if !project.walls.is_empty() {
        drawn.push(first_course_drawing(project));
        drawn.extend(elevation_drawings(project));
    }
    if let Some(roof) = roof_plan_drawing(project) {
        drawn.push(roof);
    }
    let quantities = crate::domain::quantities::takeoff_sheet_pages(project, true);
    let total = 1 + drawn.len() + quantities.len();
    let plan_project = set_plan_project(project);
    let plan = crate::io::export::sheet::scaled_sheet_svg_for_view(
        &plan_project,
        &ExportView::for_discipline(&plan_project, Discipline::Architecture),
        Some((1, total)),
    );
    let mut pages = vec![plan];
    for (index, sheet) in drawn.into_iter().enumerate() {
        pages.push(finish(
            project,
            sheet,
            &format!("{:02}/{total:02}", index + 2),
        ));
    }
    pages.extend(quantities);
    pages
}

/// Writes the construction set as one PDF; returns the page count.
pub fn export_construction_set_pdf(project: &Project, path: &Path) -> Result<usize, String> {
    if project.walls.is_empty() {
        return Err("Desenhe a casa antes de gerar o conjunto para obra.".to_owned());
    }
    super::write_pdf(&construction_set_pages(project), path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;
    use crate::domain::model::OpeningKind;

    #[test]
    fn the_set_has_the_plan_setting_out_first_course_and_quantities_numbered() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala")
            .unwrap();
        let wall = project.walls[0].id;
        project.add_opening(wall, OpeningKind::Door, 0.5).unwrap();
        let pages = construction_set_pages(&project);
        assert!(pages.len() >= 5);
        let total = pages.len();
        assert!(pages[0].contains(&format!("01/{total:02}")));
        assert!(pages.iter().any(|p| p.contains("Planta de locação")));
        assert!(pages.iter().any(|p| p.contains("Planta de primeira fiada")));
        // The person's project keeps no generated dimension chain.
        assert!(project.dimensions.is_empty());
    }
}
