//! What the connected assistant is doing, made visible: a badge while it
//! works, a fading highlight on every element it adds or changes, an
//! optional view that follows its edits, and a log of its actions.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};

use super::canvas::ViewTransform;
use super::{FloorPlanApp, Selection, theme};
use crate::domain::model::{ChangeSet, Project};
use crate::mcp::ToolOutput;

/// How long an edit stays highlighted.
const HIGHLIGHT: Duration = Duration::from_millis(2600);
/// How long the "working" badge stays after the last call.
const BUSY: Duration = Duration::from_millis(1800);
/// How many actions the log keeps.
const LOG_SIZE: usize = 200;

/// One tool the assistant ran.
#[derive(Clone, Debug)]
pub(super) struct AiAction {
    pub tool: String,
    pub ok: bool,
    /// The first line of the answer.
    pub summary: String,
    pub at: Instant,
    /// Elements added, removed or altered.
    pub changed: usize,
}

/// The assistant's activity in this session.
#[derive(Default)]
pub(super) struct AiActivity {
    pub log: VecDeque<AiAction>,
    /// What the last edits touched, and when.
    highlight: Vec<(Selection, Instant)>,
    /// The tool running or just run, and when.
    busy: Option<(String, Instant)>,
}

/// "agora", "há 12 s", "há 3 min", "há 2 h".
pub(super) fn time_ago(at: Instant) -> String {
    let seconds = at.elapsed().as_secs();
    match seconds {
        0..=4 => "agora".to_owned(),
        5..=59 => format!("há {seconds} s"),
        60..=3599 => format!("há {} min", seconds / 60),
        _ => format!("há {} h", seconds / 3600),
    }
}

impl AiActivity {
    /// Notes a tool call and what it changed.
    pub fn record(&mut self, tool: &str, output: &ToolOutput, changes: &ChangeSet) {
        let now = Instant::now();
        self.busy = Some((tool.to_owned(), now));
        let touched = changes.touched();
        if !changes.is_empty() {
            self.highlight.retain(|(item, _)| !touched.contains(item));
            self.highlight
                .extend(touched.into_iter().map(|item| (item, now)));
        }
        self.log.push_back(AiAction {
            tool: tool.to_owned(),
            ok: !output.is_error,
            summary: output.text.lines().next().unwrap_or_default().to_owned(),
            at: now,
            changed: changes.added.len() + changes.removed.len() + changes.changed.len(),
        });
        while self.log.len() > LOG_SIZE {
            self.log.pop_front();
        }
    }

    /// Whether something is still being shown (the view must keep
    /// repainting meanwhile).
    pub fn animating(&self) -> bool {
        self.busy
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() < BUSY)
            || self
                .highlight
                .iter()
                .any(|(_, at)| at.elapsed() < HIGHLIGHT)
    }

    /// The tool shown in the badge, while it shows.
    pub fn busy_tool(&self) -> Option<&str> {
        self.busy
            .as_ref()
            .filter(|(_, at)| at.elapsed() < BUSY)
            .map(|(tool, _)| tool.as_str())
    }

    /// Forgets what to highlight (another project took the place of this one).
    pub fn forget_highlight(&mut self) {
        self.highlight.clear();
    }
}

impl FloorPlanApp {
    /// After a tool ran: logs it, highlights what it changed and, when the
    /// view follows the assistant, brings that into view.
    pub(super) fn note_ai_action(&mut self, tool: &str, output: &ToolOutput, before: &Project) {
        let changes = self.project.changes_since(before);
        self.ai_activity.record(tool, output, &changes);
        if self.follow_ai && !changes.touched().is_empty() {
            self.bring_into_view(&changes.touched());
        }
    }

    /// Zooms to `items` unless they are already wholly on screen.
    fn bring_into_view(&mut self, items: &[Selection]) {
        let Some((low, high)) = self.project.elements_bounds(items) else {
            return;
        };
        if let Some(rect) = self.last_canvas_rect {
            let view = ViewTransform {
                rect,
                scale: self.scale,
                pan: self.pan,
            };
            let shown = rect.shrink(40.0);
            if shown.contains(view.to_screen(low)) && shown.contains(view.to_screen(high)) {
                return;
            }
        }
        let margin = 1.5;
        let low = crate::domain::geometry::Point::new(low.x - margin, low.y - margin);
        let high = crate::domain::geometry::Point::new(high.x + margin, high.y + margin);
        if let Some(rect) = self.last_canvas_rect {
            // Off screen: slide the view to the edit and keep the person's
            // zoom; zoom out only when the edit does not fit, never in.
            let scale = f64::from(self.scale);
            let fits = (high.x - low.x) * scale <= f64::from(rect.width() - 100.0)
                && (high.y - low.y) * scale <= f64::from(rect.height() - 100.0);
            if fits {
                self.pan = eframe::egui::vec2(
                    (-(low.x + high.x) * scale / 2.0) as f32,
                    (-(low.y + high.y) * scale / 2.0) as f32,
                );
                return;
            }
        }
        self.zoom_target = Some((low, high));
    }

    /// The highlight of the assistant's edits and its "working" badge.
    pub(super) fn paint_ai_activity(&self, painter: &Painter, view: ViewTransform) {
        for (item, at) in &self.ai_activity.highlight {
            let age = at.elapsed().as_secs_f32() / HIGHLIGHT.as_secs_f32();
            if age >= 1.0 {
                continue;
            }
            let Some(outline) = self.project.element_outline(*item) else {
                continue;
            };
            let points: Vec<Pos2> = outline.iter().map(|point| view.to_screen(*point)).collect();
            // Strong at first, then fading out.
            let strength = 1.0 - age * age;
            let color = theme::AI.gamma_multiply(strength);
            painter.add(Shape::line(
                points.clone(),
                Stroke::new(6.0, theme::AI.gamma_multiply(0.18 * strength)),
            ));
            painter.add(Shape::line(points, Stroke::new(2.0, color)));
        }
        if let Some(tool) = self.ai_activity.busy_tool() {
            paint_ai_badge(painter, view.rect, &format!("IA trabalhando · {tool}"));
        }
    }
}

/// A pill at the top middle of the plan saying the assistant is at work.
fn paint_ai_badge(painter: &Painter, rect: Rect, text: &str) {
    let galley = painter.layout_no_wrap(text.to_owned(), theme::strong(12.0), Color32::WHITE);
    let size = galley.size() + vec2(36.0, 10.0);
    let badge = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + 12.0 + size.y / 2.0),
        size,
    );
    if !rect.contains_rect(badge) {
        return;
    }
    painter.rect(
        badge,
        size.y / 2.0,
        theme::AI,
        Stroke::new(1.0, theme::AI.gamma_multiply(0.6)),
        StrokeKind::Inside,
    );
    // A dot that breathes while the badge shows.
    let pulse = (egui::emath::remap(
        (painter.ctx().input(|input| input.time) * 4.0).sin(),
        -1.0..=1.0,
        0.45..=1.0,
    )) as f32;
    painter.circle_filled(
        Pos2::new(badge.left() + 14.0, badge.center().y),
        3.5,
        Color32::WHITE.gamma_multiply(pulse),
    );
    painter.galley(
        Pos2::new(
            badge.left() + 24.0,
            badge.center().y - galley.size().y / 2.0,
        ),
        galley,
        Color32::WHITE,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    #[test]
    fn the_assistants_edits_are_logged_and_highlighted() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let output = app.run_mcp_tool(
            "criar_parede",
            &serde_json::json!({ "x1": 0, "y1": 0, "x2": 4, "y2": 0 }),
        );
        assert!(!output.is_error, "{}", output.text);
        let wall = app.project.walls[0].id;
        let last = app.ai_activity.log.back().unwrap();
        assert_eq!(last.tool, "criar_parede");
        assert!(last.ok && last.changed == 1);
        assert!(app.ai_activity.animating());
        assert_eq!(app.ai_activity.busy_tool(), Some("criar_parede"));
        assert!(
            app.ai_activity
                .highlight
                .iter()
                .any(|(item, _)| *item == Selection::Wall(wall))
        );

        // A failing call is logged too, and changes nothing.
        let failed = app.run_mcp_tool("excluir_elemento", &serde_json::json!({ "id": 999 }));
        assert!(failed.is_error);
        let last = app.ai_activity.log.back().unwrap();
        assert!(!last.ok && last.changed == 0);
        assert_eq!(time_ago(last.at), "agora");

        // Following the assistant brings its edits into view.
        app.follow_ai = true;
        app.last_canvas_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        ));
        app.pan = egui::vec2(5000.0, 5000.0);
        app.zoom_target = None;
        let scale = app.scale;
        app.run_mcp_tool(
            "criar_parede",
            &serde_json::json!({ "x1": 0, "y1": 5, "x2": 4, "y2": 5 }),
        );
        // It slides to the edit and keeps the person's zoom.
        assert!(app.zoom_target.is_none());
        assert_eq!(app.scale, scale);
        let view = crate::app::canvas::ViewTransform {
            rect: app.last_canvas_rect.unwrap(),
            scale: app.scale,
            pan: app.pan,
        };
        let middle = view.to_screen(crate::domain::geometry::Point::new(2.0, -5.0));
        assert!(app.last_canvas_rect.unwrap().shrink(40.0).contains(middle));

        // An edit too big for this zoom zooms out to show it whole.
        app.pan = egui::vec2(5000.0, 5000.0);
        app.run_mcp_tool(
            "criar_parede",
            &serde_json::json!({ "x1": 0, "y1": 40, "x2": 60, "y2": 40 }),
        );
        let (low, high) = app.zoom_target.expect("zooms out to fit");
        assert!(low.x < 0.0 && high.x > 60.0);
    }
}
