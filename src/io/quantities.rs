//! Quantity sheets use the same layout for the GUI and MCP exports.

use std::path::Path;

use crate::domain::{model::Project, quantities::takeoff_sheet_pages};

/// Writes all quantity rows on separate, correctly sized A3 PDF pages.
pub fn export_pdf(project: &Project, all_floors: bool, path: &Path) -> Result<(), String> {
    let pages = takeoff_sheet_pages(project, all_floors);
    let pdf = super::report::pages_to_pdf(&pages)?;
    std::fs::write(path, pdf).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        finishes::RoomFinishPreset, geometry::Point, quantities::takeoff_sheet_svg,
    };

    #[test]
    fn quantity_pdf_keeps_a3_pages_and_all_walls_on_multiple_floors() {
        let mut project = Project::default();
        project.name = "Conferência de quantitativos e acabamentos".into();
        project
            .add_room(
                Point::new(0.0, 0.0),
                Point::new(12.0, 8.0),
                "Sala de estar e jantar",
            )
            .unwrap();
        project
            .apply_room_finish_preset(project.rooms[0].id, RoomFinishPreset::SocialPorcelanato)
            .unwrap();
        for i in 0..24 {
            let y = 15.0 + i as f64 * 4.0;
            project
                .add_wall(Point::new(0.0, y), Point::new(50.0, y))
                .unwrap();
        }
        project.ensure_default_floor();
        project.add_floor("Superior", true, true);
        let pages = takeoff_sheet_pages(&project, true);
        assert!(pages.len() > 2);
        assert!(
            pages
                .iter()
                .all(|page| page.contains("width=\"420mm\" height=\"297mm\""))
        );
        let pdf = super::super::report::pages_to_pdf(&pages).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains(&format!("/Count {}", pages.len())));
        let svg = takeoff_sheet_svg(&project, true);
        assert!(svg.contains(&format!("height=\"{}mm\"", pages.len() * 297)));
        assert!(
            svg2pdf::usvg::Tree::from_str(&svg, &super::super::export::svg2pdf_options()).is_ok()
        );
        if let Some(folder) = std::env::var_os("TRAZO_QUANTITIES_PREVIEW_DIR") {
            let folder = std::path::PathBuf::from(folder);
            std::fs::create_dir_all(&folder).unwrap();
            export_pdf(&project, true, &folder.join("quantitativos.pdf")).unwrap();
            std::fs::write(folder.join("quantitativos.svg"), svg).unwrap();
        }
    }
}
