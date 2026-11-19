//! Organising the selection: join, align, distribute, group and ungroup,
//! and repeat it in a grid or around a centre (the "Repetir" dialog).

use crate::app::{FloorPlanApp, Selection, Tool};
use crate::domain::geometry::Point;
use crate::domain::model::{Alignment, Axis};

/// What the "Repetir" dialog holds while it is open.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct RepeatForm {
    /// Around a centre instead of a grid.
    pub polar: bool,
    pub columns: String,
    pub rows: String,
    pub spacing_x: String,
    pub spacing_y: String,
    pub count: String,
    pub degrees: String,
    pub center_x: String,
    pub center_y: String,
}

fn metres(value: f64) -> String {
    format!("{value:.2}").replace('.', ",")
}

fn number(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn whole(text: &str) -> Option<usize> {
    text.trim().parse::<usize>().ok()
}

impl Alignment {
    pub(in crate::app) const ALL: [Alignment; 6] = [
        Alignment::Left,
        Alignment::CenterX,
        Alignment::Right,
        Alignment::Top,
        Alignment::CenterY,
        Alignment::Bottom,
    ];

    pub(in crate::app) fn label(self) -> &'static str {
        match self {
            Alignment::Left => "Alinhar à esquerda",
            Alignment::CenterX => "Centralizar na horizontal",
            Alignment::Right => "Alinhar à direita",
            Alignment::Top => "Alinhar em cima",
            Alignment::CenterY => "Centralizar na vertical",
            Alignment::Bottom => "Alinhar embaixo",
        }
    }
}

impl Axis {
    pub(in crate::app) fn label(self) -> &'static str {
        match self {
            Axis::Horizontal => "Distribuir na horizontal",
            Axis::Vertical => "Distribuir na vertical",
        }
    }
}

impl FloorPlanApp {
    /// Runs `change` on the project as one undo step, or reports why not.
    fn change_selection(
        &mut self,
        change: impl FnOnce(&mut crate::domain::model::Project, &[Selection]) -> Result<String, String>,
    ) {
        let items = self.selected();
        if items.is_empty() {
            self.message = "Selecione algo primeiro.".to_owned();
            return;
        }
        let previous = self.project.clone();
        match change(&mut self.project, &items) {
            Ok(done) => {
                self.record_change(previous);
                self.revalidate_selection();
                self.sync_inspector_fields();
                self.message = done;
            }
            Err(error) => {
                self.project = previous;
                self.message = error;
            }
        }
    }

    /// U: joins the selected walls in line, or the drawings that meet.
    pub(in crate::app) fn join_selection(&mut self) {
        let items = self.selected();
        let previous = self.project.clone();
        match self.project.join_elements(&items) {
            Ok(joined) => {
                self.record_change(previous);
                self.set_selection(Some(joined));
                self.sync_inspector_fields();
                self.message = match joined {
                    Selection::Wall(_) => "Paredes unidas em uma só.",
                    _ => "Desenhos unidos em um só.",
                }
                .to_owned();
            }
            Err(error) => self.message = error,
        }
    }

    pub(in crate::app) fn align_selection(&mut self, alignment: Alignment) {
        self.change_selection(|project, items| {
            let moved = project.align_elements(items, alignment)?;
            Ok(if moved == 0 {
                "Já estavam alinhados.".to_owned()
            } else {
                format!("{} ({moved} movido(s)).", alignment.label())
            })
        });
    }

    pub(in crate::app) fn distribute_selection(&mut self, axis: Axis) {
        self.change_selection(|project, items| {
            let moved = project.distribute_elements(items, axis)?;
            Ok(if moved == 0 {
                "Já estavam distribuídos por igual.".to_owned()
            } else {
                format!("{} ({moved} movido(s)).", axis.label())
            })
        });
    }

    /// Ctrl+G.
    pub(in crate::app) fn group_selection(&mut self) {
        let items = self.selected();
        let previous = self.project.clone();
        match self.project.group_elements(&items, "") {
            Ok(id) => {
                self.record_change(previous);
                let name = self
                    .project
                    .groups
                    .iter()
                    .find(|group| group.id == id)
                    .map(|group| group.name.clone())
                    .unwrap_or_default();
                self.sync_inspector_fields();
                self.message = format!(
                    "\"{name}\" criado: um clique seleciona o grupo todo; Ctrl+clique, só o elemento."
                );
            }
            Err(error) => self.message = error,
        }
    }

    /// Ctrl+Shift+G.
    pub(in crate::app) fn ungroup_selection(&mut self) {
        let items = self.selected();
        let previous = self.project.clone();
        let count = self.project.ungroup_elements(&items);
        if count == 0 {
            self.message = "A seleção não está em nenhum grupo.".to_owned();
            return;
        }
        self.record_change(previous);
        self.sync_inspector_fields();
        self.message =
            format!("{count} grupo(s) desfeito(s); os elementos continuam selecionados.");
    }

    /// The group the selection is, when it is exactly one group.
    pub(in crate::app) fn selected_group(&self) -> Option<u64> {
        let items = self.selected();
        let group = self.project.group_of(*items.first()?)?;
        (group.members.len() == items.len()
            && group.members.iter().all(|member| items.contains(member)))
        .then_some(group.id)
    }

    /// Opens "Repetir" filled in for the selection: columns a little apart,
    /// or a ring around the selection's centre.
    pub(in crate::app) fn open_repeat_dialog(&mut self) {
        let Some((low, high)) = self.project.elements_bounds(&self.selected()) else {
            self.message = "Selecione o que quer repetir.".to_owned();
            return;
        };
        let center = low.midpoint(high);
        self.repeat_form = Some(RepeatForm {
            polar: false,
            columns: "3".to_owned(),
            rows: "1".to_owned(),
            spacing_x: metres(high.x - low.x + 0.5),
            spacing_y: metres(high.y - low.y + 0.5),
            count: "4".to_owned(),
            degrees: "360".to_owned(),
            center_x: metres(center.x + 0.0),
            center_y: metres(-center.y + 0.0),
        });
    }

    /// Makes the copies the "Repetir" dialog asks for, as one undo step;
    /// returns whether it worked (the dialog then closes).
    pub(in crate::app) fn apply_repeat(&mut self) -> bool {
        let Some(form) = self.repeat_form.clone() else {
            return false;
        };
        let items = self.selected();
        let previous = self.project.clone();
        let result = if form.polar {
            let (Some(count), Some(degrees), Some(x), Some(y)) = (
                whole(&form.count),
                number(&form.degrees),
                number(&form.center_x),
                number(&form.center_y),
            ) else {
                self.message = "Preencha quantidade, ângulo e centro com números.".to_owned();
                return false;
            };
            // The centre is typed with Y up, as the status bar shows it.
            self.project
                .repeat_around(&items, Point::new(x, -y), count, degrees)
        } else {
            let (Some(columns), Some(rows), Some(dx), Some(dy)) = (
                whole(&form.columns),
                whole(&form.rows),
                number(&form.spacing_x),
                number(&form.spacing_y),
            ) else {
                self.message =
                    "Preencha colunas, linhas e espaçamentos com números (ex.: 3 e 1,50)."
                        .to_owned();
                return false;
            };
            // Rows go up the screen for a positive spacing (Y up).
            self.project
                .repeat_grid(&items, columns, rows, Point::new(dx, -dy))
        };
        match result {
            Ok(copies) => {
                if self.tool != Tool::Move {
                    self.set_tool(Tool::Move);
                    self.set_selection_set(items);
                }
                self.record_change(previous);
                self.sync_inspector_fields();
                self.message = format!("{} cópia(s) criada(s).", copies.len());
                self.repeat_form = None;
                true
            }
            Err(error) => {
                self.project = previous;
                self.message = error;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::FixtureKind;
    use eframe::egui;

    fn app_with_tables() -> (FloorPlanApp, Vec<u64>) {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let ids = [(0.0, 0.0), (3.0, 1.0), (8.0, 0.5)]
            .into_iter()
            .map(|(x, y)| {
                app.project
                    .add_fixture(FixtureKind::Table, Point::new(x, y), 1.0, 1.0, 0)
                    .unwrap()
            })
            .collect();
        (app, ids)
    }

    #[test]
    fn grouping_makes_one_click_pick_the_whole_group() {
        let (mut app, ids) = app_with_tables();
        app.set_selection_set(ids[..2].iter().map(|id| Selection::Fixture(*id)).collect());
        app.group_selection();
        assert_eq!(app.project.groups.len(), 1);
        assert!(app.selected_group().is_some());

        // A click on one member selects both; Ctrl+click only that one.
        app.clear_selection();
        app.begin_item_drag_with(Point::new(3.0, 1.0), false, false);
        assert_eq!(app.selection_count(), 2);
        assert_eq!(app.selection, Some(Selection::Fixture(ids[1])));
        app.commit_item_drag();
        app.begin_item_drag_with(Point::new(3.0, 1.0), false, true);
        assert_eq!(app.selection_count(), 1);
        app.commit_item_drag();

        // Dragging a member moves the group.
        app.begin_item_drag_with(Point::new(0.0, 0.0), false, false);
        app.drag_item_to(Point::new(1.0, 0.0));
        app.commit_item_drag();
        let centers: Vec<Point> = app.project.fixtures.iter().map(|f| f.center).collect();
        assert_eq!(centers[0], Point::new(1.0, 0.0));
        assert_eq!(centers[1], Point::new(4.0, 1.0));
        assert_eq!(centers[2], Point::new(8.0, 0.5));

        app.ungroup_selection();
        assert!(app.project.groups.is_empty());
        app.undo();
        assert_eq!(app.project.groups.len(), 1);
    }

    #[test]
    fn align_distribute_and_repeat_from_the_app() {
        let (mut app, ids) = app_with_tables();
        app.set_selection_set(ids.iter().map(|id| Selection::Fixture(*id)).collect());
        app.align_selection(Alignment::Bottom);
        assert!(
            app.project
                .fixtures
                .iter()
                .all(|f| (f.center.y - 1.0).abs() < 1e-9)
        );
        app.distribute_selection(Axis::Horizontal);
        assert!((app.project.fixtures[1].center.x - 4.0).abs() < 1e-9);
        app.undo();
        assert!((app.project.fixtures[1].center.x - 3.0).abs() < 1e-9);

        app.set_selection(Some(Selection::Fixture(ids[0])));
        app.open_repeat_dialog();
        let form = app.repeat_form.as_mut().unwrap();
        form.columns = "2".to_owned();
        form.rows = "2".to_owned();
        form.spacing_x = "2".to_owned();
        form.spacing_y = "2".to_owned();
        assert!(app.apply_repeat());
        assert_eq!(app.project.fixtures.len(), 6);
        assert!(app.repeat_form.is_none());
        // Rows go up the screen: 2 m up is -2 in plan coordinates.
        assert!(
            app.project
                .fixtures
                .iter()
                .any(|f| f.center == Point::new(2.0, -1.0))
        );

        app.open_repeat_dialog();
        let form = app.repeat_form.as_mut().unwrap();
        form.polar = true;
        form.count = "abc".to_owned();
        assert!(!app.apply_repeat());
        assert!(app.repeat_form.is_some());
    }
}
