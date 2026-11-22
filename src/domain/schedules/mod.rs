//! Configurable architectural schedules: area summary, openings schedule,
//! rooms schedule and masonry/materials schedule, rendered both in the UI
//! and on exported technical sheets.

mod materials;

use serde::{Deserialize, Serialize};

use super::{
    annotations::{centimetres, frame_types},
    geometry::polygon_perimeter,
    model::Project,
};

pub use materials::build_materials_schedule;

fn default_true() -> bool {
    true
}

fn default_waste_percent() -> f64 {
    10.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduleConfig {
    #[serde(default = "default_true")]
    pub show_area_schedule: bool,
    #[serde(default = "default_true")]
    pub show_openings_schedule: bool,
    #[serde(default = "default_true")]
    pub show_rooms_schedule: bool,
    #[serde(default = "default_true")]
    pub show_materials_schedule: bool,
    #[serde(default = "default_true")]
    pub show_perimeter: bool,
    #[serde(default = "default_true")]
    pub show_sill: bool,
    #[serde(default = "default_true")]
    pub show_occupancy: bool,
    #[serde(default = "default_waste_percent")]
    pub waste_percent: f64,
    #[serde(default = "default_true")]
    pub include_on_sheet: bool,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            show_area_schedule: true,
            show_openings_schedule: true,
            show_rooms_schedule: true,
            show_materials_schedule: true,
            show_perimeter: true,
            show_sill: true,
            show_occupancy: true,
            waste_percent: default_waste_percent(),
            include_on_sheet: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleTable {
    pub title: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub footer: Option<Vec<String>>,
}

pub(crate) fn fmt_area(m2: f64) -> String {
    format!("{m2:.2} m²").replace('.', ",")
}

pub(crate) fn fmt_len(m: f64) -> String {
    format!("{m:.2} m").replace('.', ",")
}

pub fn build_rooms_schedule(project: &Project, config: &ScheduleConfig) -> ScheduleTable {
    let mut headers = vec!["Ambiente".to_owned(), "Área útil".to_owned()];
    if config.show_perimeter {
        headers.push("Perímetro".to_owned());
    }
    headers.push("Acabamento / Hachura".to_owned());

    let mut total_area = 0.0;
    let mut total_perim = 0.0;
    let mut rows = Vec::new();
    for room in &project.rooms {
        let area = project.room_area(room).unwrap_or(0.0);
        let perim = project
            .room_outline(room)
            .map(|outline| polygon_perimeter(&outline))
            .unwrap_or(0.0);
        total_area += area;
        total_perim += perim;
        let finish = room
            .hatch
            .map(|h| h.pattern.label())
            .unwrap_or("Padrão")
            .to_owned();
        let mut row = vec![room.display_name().to_owned(), fmt_area(area)];
        if config.show_perimeter {
            row.push(fmt_len(perim));
        }
        row.push(finish);
        rows.push(row);
    }

    let mut footer = vec!["Total".to_owned(), fmt_area(total_area)];
    if config.show_perimeter {
        footer.push(fmt_len(total_perim));
    }
    footer.push(format!("{} ambiente(s)", project.rooms.len()));

    ScheduleTable {
        title: "Quadro de Ambientes".to_owned(),
        headers,
        rows,
        footer: Some(footer),
    }
}

pub fn build_area_schedule(project: &Project, config: &ScheduleConfig) -> ScheduleTable {
    let summary = project.area_summary();
    let mut rows = vec![
        vec!["Área útil (cômodos)".to_owned(), fmt_area(summary.usable)],
        vec!["Área de paredes".to_owned(), fmt_area(summary.walls)],
        vec!["Área construída".to_owned(), fmt_area(summary.constructed)],
        vec!["Área de projeção".to_owned(), fmt_area(summary.projection)],
    ];
    if summary.terrain > 1e-4 {
        rows.push(vec![
            "Área do terreno".to_owned(),
            fmt_area(summary.terrain),
        ]);
        if config.show_occupancy {
            let rate = (summary.projection / summary.terrain * 100.0).min(100.0);
            rows.push(vec![
                "Taxa de ocupação".to_owned(),
                format!("{rate:.1}%").replace('.', ","),
            ]);
        }
    }

    ScheduleTable {
        title: "Quadro de Áreas".to_owned(),
        headers: vec!["Item".to_owned(), "Medida".to_owned()],
        rows,
        footer: None,
    }
}

pub fn build_openings_schedule(project: &Project, config: &ScheduleConfig) -> ScheduleTable {
    let mut headers = vec![
        "Código".to_owned(),
        "Tipo".to_owned(),
        "Largura × Altura (cm)".to_owned(),
    ];
    if config.show_sill {
        headers.push("Peitoril".to_owned());
    }
    headers.push("Qtd.".to_owned());

    let types = frame_types(project);
    let mut total_qty = 0usize;
    let mut rows = Vec::new();
    for frame in &types {
        let qty = frame.ids.len();
        total_qty += qty;
        let dim = match frame.height {
            Some(h) => format!("{} × {}", centimetres(frame.width), centimetres(h)),
            None => format!("{} × pé-direito", centimetres(frame.width)),
        };
        let mut row = vec![frame.code.clone(), frame.description(), dim];
        if config.show_sill {
            row.push(if frame.sill > 0.005 {
                format!("{} cm", centimetres(frame.sill))
            } else {
                "—".to_owned()
            });
        }
        row.push(qty.to_string());
        rows.push(row);
    }

    let mut footer = vec!["Total".to_owned(), String::new(), String::new()];
    if config.show_sill {
        footer.push(String::new());
    }
    footer.push(total_qty.to_string());

    ScheduleTable {
        title: "Quadro de Esquadrias".to_owned(),
        headers,
        rows,
        footer: Some(footer),
    }
}

pub fn active_schedules(project: &Project, config: &ScheduleConfig) -> Vec<ScheduleTable> {
    let mut tables = Vec::new();
    if config.show_area_schedule {
        tables.push(build_area_schedule(project, config));
    }
    if config.show_rooms_schedule && !project.rooms.is_empty() {
        tables.push(build_rooms_schedule(project, config));
    }
    if config.show_openings_schedule && !project.openings.is_empty() {
        tables.push(build_openings_schedule(project, config));
    }
    if config.show_materials_schedule && !project.walls.is_empty() {
        tables.push(build_materials_schedule(project, config));
    }
    if project.rooms.iter().any(|r| r.finishes.is_some()) {
        tables.push(super::finishes::build_finishes_schedule(project));
    }
    // The electrical board, on sheets that show the electrical layer.
    if project.category_visible(super::layers::LayerCategory::Electrical)
        && project
            .electrical_devices()
            .any(|device| device.electrical.and_then(|data| data.circuit).is_some())
    {
        tables.push(super::distribution::build_panel_schedule(project));
    }
    // The columns and footings, on sheets that show the structure.
    if project.category_visible(super::layers::LayerCategory::Structure)
        && project.structural_columns().next().is_some()
    {
        tables.push(super::structure::build_columns_schedule(project));
    }
    tables
}

pub fn schedules_csv(project: &Project, config: &ScheduleConfig) -> String {
    let mut out = String::new();
    for table in active_schedules(project, config) {
        out.push_str(&table.title);
        out.push('\n');
        out.push_str(&table.headers.join(";"));
        out.push('\n');
        for row in &table.rows {
            out.push_str(&row.join(";"));
            out.push('\n');
        }
        if let Some(footer) = &table.footer {
            out.push_str(&footer.join(";"));
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{geometry::Point, model::OpeningKind};

    #[test]
    fn configurable_schedules_compute_areas_openings_rooms_and_materials() {
        let mut project = Project::default();
        project.set_terrain(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();
        let wall_id = project.walls[0].id;
        project
            .add_opening(wall_id, OpeningKind::Door, 0.5)
            .unwrap();

        let cfg = ScheduleConfig::default();
        let all = active_schedules(&project, &cfg);
        assert_eq!(all.len(), 4);

        let rooms = build_rooms_schedule(&project, &cfg);
        assert_eq!(rooms.headers.len(), 4);
        assert_eq!(rooms.rows[0][0], "Sala");
        assert_eq!(rooms.rows[0][1], "12,00 m²");

        let openings = build_openings_schedule(&project, &cfg);
        assert_eq!(openings.rows.len(), 1);
        assert_eq!(openings.rows[0][0], "P1");

        // Column & toggle customization works.
        let minimal = ScheduleConfig {
            show_perimeter: false,
            show_sill: false,
            show_materials_schedule: false,
            ..Default::default()
        };
        assert_eq!(build_rooms_schedule(&project, &minimal).headers.len(), 3);
        assert_eq!(build_openings_schedule(&project, &minimal).headers.len(), 4);
        assert_eq!(active_schedules(&project, &minimal).len(), 3);
    }
}
