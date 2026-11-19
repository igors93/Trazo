use std::{fmt::Write as _, fs, path::Path};

use crate::domain::{annotations::frame_types, model::Project};

fn field(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\r' | '\n'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn number(value: f64) -> String {
    let mut formatted = format!("{value:.3}");
    while formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.push('0');
    }
    formatted
}

pub(super) fn content(project: &Project) -> String {
    // The BOM lets spreadsheet programs identify UTF-8 names without an
    // import wizard. Numbers keep the interoperable decimal point.
    let mut csv = String::from(
        "\u{feff}categoria,codigo,descricao,quantidade,unidade,largura_m,altura_m,peitoril_m\r\n",
    );

    if let Some(terrain) = &project.terrain {
        let _ = writeln!(
            csv,
            "area,TERRENO,Área do terreno,{},m2,,,\r",
            number(terrain.area())
        );
    }
    for room in &project.rooms {
        let Some(area) = project.room_area(room) else {
            continue;
        };
        let _ = writeln!(
            csv,
            "area,A{},{},{},m2,,,\r",
            room.id,
            field(room.display_name()),
            number(area)
        );
    }

    for frame in frame_types(project) {
        let height = frame.height.map(number).unwrap_or_default();
        let sill = (frame.kind == crate::domain::model::OpeningKind::Window)
            .then(|| number(frame.sill))
            .unwrap_or_default();
        let _ = writeln!(
            csv,
            "esquadria,{},{},{},un,{},{},{}\r",
            field(&frame.code),
            field(frame.kind.label()),
            frame.ids.len(),
            number(frame.width),
            height,
            sill
        );
    }

    let (wall_area, blocks) = project.masonry_estimate();
    if !project.walls.is_empty() {
        let _ = writeln!(
            csv,
            "quantitativo,ALV,Área líquida de alvenaria,{},m2,,,\r",
            number(wall_area)
        );
        let _ = writeln!(
            csv,
            "quantitativo,BLOCO,{},{},un,,,\r",
            field(&format!("{} — blocos estimados", project.masonry.name)),
            number(blocks.ceil())
        );
        let _ = writeln!(
            csv,
            "quantitativo,BLOCO+5%,{},{},un,,,\r",
            field(&format!(
                "{} — blocos com 5% de perda",
                project.masonry.name
            )),
            number((blocks * 1.05).ceil())
        );
    }

    csv
}

pub(super) fn write(project: &Project, path: &Path) -> Result<(), String> {
    fs::write(path, content(project)).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{geometry::Point, model::OpeningKind};

    #[test]
    fn csv_contains_areas_frames_and_masonry_quantities() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        project
            .add_room(Point::new(1.0, 1.0), Point::new(5.0, 4.0), "Sala, jantar")
            .unwrap();
        let wall = project.walls[0].id;
        project.add_opening(wall, OpeningKind::Window, 0.5).unwrap();

        let csv = content(&project);
        assert!(csv.starts_with("\u{feff}categoria,codigo"));
        assert!(csv.contains("area,TERRENO,Área do terreno,200.0,m2"));
        assert!(csv.contains("\"Sala, jantar\",12.0,m2"));
        assert!(csv.contains("esquadria,J1,Janela,1,un,1.2,1.0,1.1"));
        assert!(csv.contains("quantitativo,ALV,Área líquida de alvenaria"));
        assert!(csv.contains("quantitativo,BLOCO+5%"));
        assert!(csv.lines().all(|line| line.split(',').count() >= 8));
    }

    #[test]
    fn csv_escapes_quotes_and_line_breaks() {
        assert_eq!(field("Suíte \"A\"\nfrente"), "\"Suíte \"\"A\"\"\nfrente\"");
    }
}
