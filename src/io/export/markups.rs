//! Shared review-mark SVG for raster previews, PDF and scaled sheets.
use crate::domain::{geometry::Point, markups::MarkupKind, model::Project};
use std::fmt::Write;

pub(super) fn bounds(project: &Project) -> Option<(Point, Point)> {
    if project.markup_settings.export || project.markups.is_empty() {
        return project.bounds();
    }
    let mut clean = project.clone();
    clean.markups.clear();
    clean.bounds()
}

pub(super) fn write(
    svg: &mut String,
    project: &Project,
    point: impl Fn(Point) -> (f64, f64),
    font_size: f64,
) {
    if !project.markup_settings.export {
        return;
    }
    for m in &project.markups {
        if !project.markup_shown(m) || m.validate().is_err() {
            continue;
        }
        let color = format!("#{:02x}{:02x}{:02x}", m.color[0], m.color[1], m.color[2]);
        let path = m
            .shape()
            .outline()
            .into_iter()
            .map(|p| {
                let (x, y) = point(p);
                format!("{x:.3},{y:.3}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        let fill = if matches!(m.kind, MarkupKind::Rectangle | MarkupKind::Circle) {
            color.as_str()
        } else {
            "none"
        };
        let _ = writeln!(
            svg,
            "<g data-marcacao=\"{}\" opacity=\"{}\"><title>{}</title><polyline points=\"{path}\" fill=\"{fill}\" fill-opacity=\"0.08\" stroke=\"{color}\" stroke-width=\"{:.3}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
            m.label(),
            if m.resolved { 0.55 } else { 1.0 },
            super::escaped(&m.comment),
            font_size * 0.17
        );
        if m.kind == MarkupKind::Arrow {
            let (x, y) = point(m.points[1]);
            let (a, b) = point(m.points[0]);
            let len = (x - a).hypot(y - b).max(1e-9);
            let dx = (x - a) / len;
            let dy = (y - b) / len;
            let h = font_size;
            let _ = writeln!(
                svg,
                "<polygon points=\"{x:.3},{y:.3} {:.3},{:.3} {:.3},{:.3}\" fill=\"{color}\"/>",
                x - h * dx - h * 0.4 * dy,
                y - h * dy + h * 0.4 * dx,
                x - h * dx + h * 0.4 * dy,
                y - h * dy - h * 0.4 * dx
            );
        }
        let (x, y) = point(m.points[0]);
        let _ = writeln!(
            svg,
            "<text x=\"{:.3}\" y=\"{:.3}\" font-family=\"Arial, sans-serif\" font-size=\"{font_size}\" font-weight=\"700\" fill=\"{color}\" stroke=\"white\" stroke-width=\"{:.3}\" paint-order=\"stroke\">{}{}</text></g>",
            x + font_size * 0.2,
            y - font_size * 0.5,
            font_size * 0.18,
            m.label(),
            if m.resolved { " ✓" } else { "" }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{fixtures::FixtureKind, markups::Markup};
    #[test]
    fn markups_export_only_when_requested_and_survive_native_project_files() {
        let mut p = Project::default();
        p.add_room(Point::ORIGIN, Point::new(6.0, 4.0), "Sala")
            .unwrap();
        p.add_room(Point::new(6.0, 0.0), Point::new(10.0, 4.0), "Cozinha")
            .unwrap();
        p.add_fixture(FixtureKind::Sofa, Point::new(2.0, 0.9), 2.1, 0.85, 0)
            .unwrap();
        p.add_fixture(FixtureKind::Counter, Point::new(8.0, 0.35), 2.8, 0.6, 0)
            .unwrap();
        for (kind, points, color, comment) in [
            (
                MarkupKind::Rectangle,
                vec![Point::new(0.6, 0.3), Point::new(3.4, 1.6)],
                [220, 65, 65],
                "Revisar & reposicionar o sofá",
            ),
            (
                MarkupKind::Circle,
                vec![Point::new(8.0, 0.7), Point::new(9.5, 0.7)],
                [38, 112, 216],
                "Ampliar a bancada",
            ),
            (
                MarkupKind::Arrow,
                vec![Point::new(4.7, 2.7), Point::new(6.0, 2.0)],
                [35, 142, 86],
                "Conferir passagem",
            ),
            (
                MarkupKind::Freehand,
                vec![
                    Point::new(0.7, 3.0),
                    Point::new(1.4, 2.8),
                    Point::new(2.2, 3.1),
                    Point::new(3.2, 2.9),
                ],
                [143, 80, 191],
                "Estudar disposição",
            ),
        ] {
            p.add_markup(Markup {
                id: 0,
                kind,
                points,
                color,
                comment: comment.into(),
                resolved: false,
                linked_elements: vec![],
            })
            .unwrap();
        }
        assert!(!super::super::document(&p).markup.contains("data-marcacao"));
        assert!(!super::super::sheet::scaled_sheet_svg(&p).contains("data-marcacao"));
        assert!(!crate::io::dxf::to_dxf_string(&p).contains("Marcacoes_"));
        p.markup_settings.export = true;
        let svg = super::super::document(&p).markup;
        assert_eq!(svg.matches("data-marcacao").count(), 4);
        assert!(svg.contains("Revisar &amp; reposicionar"));
        assert_eq!(
            super::super::sheet::scaled_sheet_svg(&p)
                .matches("data-marcacao")
                .count(),
            4
        );
        let dxf = crate::io::dxf::to_dxf_string(&p);
        assert!(dxf.contains("Marcacoes_DC4141"));
        let png = super::super::png_preview(&p, 1440).unwrap();
        assert!(png.len() > 1000);
        let path = std::env::temp_dir().join(format!(
            "trazo-review-roundtrip-{}.trazo",
            std::process::id()
        ));
        crate::io::project_file::save(&p, &path).unwrap();
        let (loaded, _) = crate::io::project_file::load(&path).unwrap();
        assert_eq!(loaded.markups, p.markups);
        std::fs::remove_file(path).unwrap();
        if let Some(folder) = std::env::var_os("TRAZO_MARKUP_PREVIEW_DIR") {
            let folder = std::path::PathBuf::from(folder);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("marcacoes.svg"), svg).unwrap();
            std::fs::write(folder.join("marcacoes.png"), png).unwrap();
        }
        p.markups[0].resolved = true;
        assert_eq!(
            super::super::document(&p)
                .markup
                .matches("data-marcacao")
                .count(),
            3
        );
        p.markup_settings.visible = false;
        assert!(!super::super::document(&p).markup.contains("data-marcacao"));
    }
}
