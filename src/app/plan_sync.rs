//! Keeping a connected assistant in step with the person: every change to
//! the plan gets a revision number and an entry saying who made it (the
//! person or the assistant, with its tool) and which elements it touched.
//! Assistants ask `mudancas_desde` for what happened since they last
//! looked, and subscribers to `trazo://planta` are told at once.

use std::collections::VecDeque;

use serde_json::{Value, json};

use super::{FloorPlanApp, Selection};
use crate::domain::model::Project;
use crate::mcp::events;

/// How many changes the log keeps.
const LOG_SIZE: usize = 500;

/// One change of the plan.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PlanChange {
    pub revision: u64,
    /// The assistant's tool, or `None` for the person.
    pub tool: Option<String>,
    /// The whole plan was replaced (new project, file opened).
    pub replaced: bool,
    pub added: Vec<Selection>,
    pub removed: Vec<Selection>,
    pub changed: Vec<Selection>,
}

/// An element as the assistant reads it: its id and kind.
fn element_json(item: &Selection) -> Value {
    json!({ "id": item.id(), "tipo": item.kind().singular().to_lowercase() })
}

impl PlanChange {
    fn to_json(&self) -> Value {
        let list = |items: &[Selection]| items.iter().map(element_json).collect::<Vec<_>>();
        json!({
            "revisao": self.revision,
            "origem": if self.tool.is_some() { "ia" } else { "usuario" },
            "ferramenta": self.tool,
            "planta_substituida": self.replaced,
            "adicionados": list(&self.added),
            "removidos": list(&self.removed),
            "alterados": list(&self.changed),
        })
    }
}

/// The plan's revisions and what changed in each.
#[derive(Debug, Default)]
pub(super) struct PlanLog {
    pub revision: u64,
    /// The revision subscribers were last told about.
    published: u64,
    pub changes: VecDeque<PlanChange>,
}

impl FloorPlanApp {
    /// Notes that the plan went from `before` to what it is now.
    pub(super) fn note_plan_change(&mut self, before: &Project, replaced: bool) {
        if !replaced && *before == self.project {
            return;
        }
        let changes = self.project.changes_since(before);
        let log = &mut self.plan_log;
        log.revision += 1;
        log.changes.push_back(PlanChange {
            revision: log.revision,
            tool: self.mcp_running.clone(),
            replaced,
            added: changes.added,
            removed: changes.removed,
            changed: changes.changed,
        });
        while log.changes.len() > LOG_SIZE {
            log.changes.pop_front();
        }
    }

    /// Tells the assistants listening that the plan changed (at most once
    /// per frame, however many edits it held).
    pub(super) fn publish_plan_changes(&mut self) {
        if self.plan_log.published != self.plan_log.revision {
            self.plan_log.published = self.plan_log.revision;
            self.mcp_events.publish(&events::plan_updated());
        }
    }

    /// `mudancas_desde`: what changed after revision `revisao`.
    pub(super) fn plan_changes_since(&self, args: &Value) -> Result<Value, String> {
        let current = self.plan_log.revision;
        let since = match super::mcp_tools::optional_count(args, "revisao")? {
            Some(since) => since,
            // Without a revision: the last twenty changes.
            None => current.saturating_sub(20),
        };
        if since > current {
            return Err(format!(
                "A revisão {since} ainda não existe: a atual é {current}."
            ));
        }
        let oldest_kept = self
            .plan_log
            .changes
            .front()
            .map(|change| change.revision)
            .unwrap_or(current + 1);
        let complete = since + 1 >= oldest_kept || since == current;
        let changes: Vec<Value> = self
            .plan_log
            .changes
            .iter()
            .filter(|change| change.revision > since)
            .map(PlanChange::to_json)
            .collect();
        let by_person = self
            .plan_log
            .changes
            .iter()
            .filter(|change| change.revision > since && change.tool.is_none())
            .count();
        Ok(json!({
            "revisao_atual": current,
            "desde": since,
            "completo": complete,
            "mudancas_do_usuario": by_person,
            "mudancas": changes,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;
    use eframe::egui;

    #[test]
    fn changes_are_numbered_and_tell_the_person_from_the_assistant() {
        let mut app = FloorPlanApp::new(&egui::Context::default());
        let listening = app.mcp_events.listen();
        assert_eq!(app.plan_log.revision, 0);

        // The person draws a wall.
        let previous = app.project.clone();
        let wall = app
            .project
            .add_wall(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            .unwrap();
        app.record_change(previous);
        // The assistant draws another one.
        app.run_mcp_tool(
            "criar_parede",
            &json!({ "x1": 0, "y1": 3, "x2": 4, "y2": 3 }),
        );
        assert_eq!(app.plan_log.revision, 2);

        let answer = app
            .run_mcp_tool("mudancas_desde", &json!({ "revisao": 0 }))
            .structured
            .unwrap();
        assert_eq!(answer["revisao_atual"], 2);
        assert_eq!(answer["mudancas_do_usuario"], 1);
        let changes = answer["mudancas"].as_array().unwrap();
        assert_eq!(changes[0]["origem"], "usuario");
        assert_eq!(changes[0]["adicionados"][0]["id"], wall);
        assert_eq!(changes[1]["origem"], "ia");
        assert_eq!(changes[1]["ferramenta"], "criar_parede");
        // Only what came after a given revision.
        let later = app
            .run_mcp_tool("mudancas_desde", &json!({ "revisao": 1 }))
            .structured
            .unwrap();
        assert_eq!(later["mudancas"].as_array().unwrap().len(), 1);
        assert!(
            app.run_mcp_tool("mudancas_desde", &json!({ "revisao": 9 }))
                .is_error
        );

        // Undo is a change too, and obter_planta says the revision.
        app.undo();
        assert_eq!(app.plan_log.revision, 3);
        let plan = app
            .run_mcp_tool("obter_planta", &json!({}))
            .structured
            .unwrap();
        assert_eq!(plan["revisao"], 3);

        // Subscribers are told once per frame, however many changes.
        app.publish_plan_changes();
        let message: Value = serde_json::from_str(&listening.recv().unwrap()).unwrap();
        assert_eq!(message["method"], "notifications/resources/updated");
        app.publish_plan_changes();
        assert!(listening.try_recv().is_err());
    }
}
