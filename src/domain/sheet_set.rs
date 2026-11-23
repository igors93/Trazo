//! Multi-sheet documentation sets with automatic pagination (`01/03`),
//! revision history (`R00`, `R01`, ...), emission status and master document
//! list (Lista Mestra de Documentos) export.

use serde::{Deserialize, Serialize};

use super::model::Project;
use super::sheet::SheetLayout;
use super::verdict::{Light, safety_summary};

/// Explicit professional acknowledgement of the red checks in one sheet revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseReview {
    pub reviewer: String,
    pub registration: String,
    pub date: String,
    pub revision: String,
    pub red_checks: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionEntry {
    pub code: String,
    pub date: String,
    pub author: String,
    pub description: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetStatus {
    #[default]
    EmElaboracao,
    ParaAprovacao,
    LiberadoObra,
    AsBuilt,
}

impl SheetStatus {
    pub const ALL: [SheetStatus; 4] = [
        SheetStatus::EmElaboracao,
        SheetStatus::ParaAprovacao,
        SheetStatus::LiberadoObra,
        SheetStatus::AsBuilt,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::EmElaboracao => "Em elaboração",
            Self::ParaAprovacao => "Para aprovação",
            Self::LiberadoObra => "Liberado para obra",
            Self::AsBuilt => "As Built",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SheetContent {
    #[default]
    FloorPlan,
    Section {
        id: u64,
    },
    Elevation {
        id: u64,
    },
    Detail {
        id: u64,
    },
}

impl SheetContent {
    pub fn label(self) -> &'static str {
        match self {
            Self::FloorPlan => "Planta Baixa",
            Self::Section { .. } => "Corte",
            Self::Elevation { .. } => "Fachada",
            Self::Detail { .. } => "Detalhamento",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SheetItem {
    pub id: u64,
    pub code: String,
    pub number: String,
    pub title: String,
    #[serde(default)]
    pub content: SheetContent,
    #[serde(default)]
    pub layout: SheetLayout,
    #[serde(default)]
    pub status: SheetStatus,
    #[serde(default)]
    pub revisions: Vec<RevisionEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_review: Option<ReleaseReview>,
    /// The discipline the sheet shows (#96): its layers on paper.
    #[serde(default, skip_serializing_if = "super::discipline::Discipline::is_all")]
    pub discipline: super::discipline::Discipline,
}

impl SheetItem {
    pub fn latest_revision_code(&self) -> &str {
        self.revisions
            .last()
            .map(|r| r.code.as_str())
            .unwrap_or("R00")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetSet {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sheets: Vec<SheetItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_sheet_id: Option<u64>,
}

impl SheetSet {
    pub fn is_empty(&self) -> bool {
        self.sheets.is_empty()
    }

    pub fn add_sheet(
        &mut self,
        title: &str,
        content: SheetContent,
        base_layout: &SheetLayout,
    ) -> u64 {
        let id = self.sheets.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        let idx = self.sheets.len() + 1;
        let mut layout = base_layout.clone();
        let clean_title = if title.trim().is_empty() {
            content.label().to_owned()
        } else {
            title.trim().to_owned()
        };
        layout.title_block.drawing_title = clean_title.clone();
        layout.title_block.revision = "R00".to_owned();

        let initial_author = layout.title_block.author.clone();
        let initial_date = layout.title_block.date.clone();
        self.sheets.push(SheetItem {
            id,
            code: format!("PR-{idx:02}"),
            number: format!("{idx:02}/{idx:02}"),
            title: clean_title,
            content,
            layout,
            status: SheetStatus::EmElaboracao,
            revisions: vec![RevisionEntry {
                code: "R00".to_owned(),
                date: initial_date,
                author: initial_author,
                description: "Emissão inicial".to_owned(),
            }],
            release_review: None,
            discipline: super::discipline::Discipline::All,
        });
        self.active_sheet_id = Some(id);
        self.renumber_sheets();
        id
    }

    pub fn remove_sheet(&mut self, id: u64) -> bool {
        let before = self.sheets.len();
        self.sheets.retain(|s| s.id != id);
        if self.sheets.len() == before {
            return false;
        }
        if self.active_sheet_id == Some(id) {
            self.active_sheet_id = self.sheets.first().map(|s| s.id);
        }
        self.renumber_sheets();
        true
    }

    pub fn move_sheet(&mut self, id: u64, delta: isize) -> bool {
        let Some(pos) = self.sheets.iter().position(|s| s.id == id) else {
            return false;
        };
        let target = pos as isize + delta;
        if target < 0 || target >= self.sheets.len() as isize {
            return false;
        }
        self.sheets.swap(pos, target as usize);
        self.renumber_sheets();
        true
    }

    pub fn renumber_sheets(&mut self) {
        let total = self.sheets.len().max(1);
        for (i, sheet) in self.sheets.iter_mut().enumerate() {
            let num = format!("{:02}/{:02}", i + 1, total);
            sheet.code = format!("PR-{:02}", i + 1);
            sheet.number = num.clone();
            sheet.layout.title_block.sheet_number = num;
            sheet.layout.title_block.revision = sheet.latest_revision_code().to_owned();
        }
    }

    pub fn add_revision(
        &mut self,
        sheet_id: u64,
        date: &str,
        author: &str,
        description: &str,
    ) -> Result<String, String> {
        let desc = description.trim();
        if desc.is_empty() {
            return Err("Informe a descrição da revisão.".to_owned());
        }
        let sheet = self
            .sheets
            .iter_mut()
            .find(|s| s.id == sheet_id)
            .ok_or_else(|| "Prancha não encontrada.".to_owned())?;
        let next_num = sheet.revisions.len();
        let code = format!("R{next_num:02}");
        sheet.revisions.push(RevisionEntry {
            code: code.clone(),
            date: date.trim().to_owned(),
            author: author.trim().to_owned(),
            description: desc.to_owned(),
        });
        sheet.layout.title_block.revision = code.clone();
        sheet.release_review = None;
        if sheet.status == SheetStatus::LiberadoObra {
            sheet.status = SheetStatus::ParaAprovacao;
        }
        if !date.trim().is_empty() {
            sheet.layout.title_block.date = date.trim().to_owned();
        }
        Ok(code)
    }

    /// Exports the Master Document List (Lista Mestra de Documentos) as CSV.
    pub fn transmittal_csv(&self, project: &Project) -> String {
        let red_checks = red_checks(project);
        let project_name = &project.name;
        let mut out = format!("Lista Mestra de Documentos — {project_name}\n");
        out.push_str("Código;Prancha;Título;Conteúdo;Formato;Escala;Revisão;Situação;Última Data;Última Descrição;Revisor;Registro do Revisor;Data da Revisão Técnica\n");
        for sheet in &self.sheets {
            let last = sheet.revisions.last();
            let last_date = last.map(|r| r.date.as_str()).unwrap_or("");
            let last_desc = last.map(|r| r.description.as_str()).unwrap_or("");
            let status = sheet.effective_status(&red_checks);
            let review = if status == SheetStatus::LiberadoObra && !red_checks.is_empty() {
                sheet.release_review.as_ref()
            } else {
                None
            };
            out.push_str(&format!(
                "{};{};{};{};{};{};{};{};{};{};{};{};{}\n",
                sheet.code,
                sheet.number,
                sheet.title,
                sheet.content.label(),
                sheet.layout.size.short_name(),
                sheet.layout.scale.label(),
                sheet.latest_revision_code(),
                status.label(),
                last_date,
                last_desc,
                review.map(|r| r.reviewer.as_str()).unwrap_or(""),
                review.map(|r| r.registration.as_str()).unwrap_or(""),
                review.map(|r| r.date.as_str()).unwrap_or("")
            ));
        }
        out
    }
}

/// A stable snapshot of every failed check, including the exact calculated value.
/// A changed result or a new failure requires another explicit review.
pub fn red_checks(project: &Project) -> Vec<String> {
    let mut checks: Vec<_> = safety_summary(project)
        .verdicts
        .into_iter()
        .flat_map(|verdict| {
            verdict.checks.into_iter().filter_map(move |check| {
                (check.light == Light::Red).then(|| {
                    format!(
                        "{}|{}|{}|{:016x}|{}|{}",
                        verdict.discipline.label(),
                        verdict.element,
                        check.name,
                        check.utilisation.to_bits(),
                        check.simple,
                        check.detail
                    )
                })
            })
        })
        .collect();
    checks.sort();
    checks
}

impl SheetItem {
    pub fn effective_status(&self, current_red_checks: &[String]) -> SheetStatus {
        if self.status != SheetStatus::LiberadoObra || current_red_checks.is_empty() {
            return self.status;
        }
        match &self.release_review {
            Some(review)
                if review.revision == self.latest_revision_code()
                    && review.red_checks == current_red_checks
                    && !review.date.trim().is_empty()
                    && valid_reviewer(&review.reviewer, &review.registration) =>
            {
                SheetStatus::LiberadoObra
            }
            _ => SheetStatus::ParaAprovacao,
        }
    }
}

fn valid_reviewer(name: &str, registration: &str) -> bool {
    let reg = registration.trim().to_uppercase();
    !name.trim().is_empty()
        && (reg.starts_with("CREA") || reg.starts_with("CAU"))
        && reg.chars().any(|c| c.is_ascii_digit())
}

impl Project {
    /// The only supported transition to release. Red results need an explicit
    /// reviewer, professional registration and dated acknowledgement.
    pub fn set_sheet_status(
        &mut self,
        sheet_id: u64,
        status: SheetStatus,
        reviewer: Option<(&str, &str, &str)>,
    ) -> Result<(), String> {
        let red = if status == SheetStatus::LiberadoObra {
            red_checks(self)
        } else {
            Vec::new()
        };
        let sheet = self
            .sheet_set
            .sheets
            .iter_mut()
            .find(|s| s.id == sheet_id)
            .ok_or_else(|| "Prancha não encontrada.".to_owned())?;
        let review = if status == SheetStatus::LiberadoObra && !red.is_empty() {
            let (name, registration, date) = reviewer.ok_or_else(|| {
                format!("Há {} verificações vermelhas. Informe nome, CREA/CAU e data do responsável pela revisão para liberar a prancha.", red.len())
            })?;
            if !valid_reviewer(name, registration) || date.trim().is_empty() {
                return Err(
                    "Informe nome, registro CREA/CAU com número e data da revisão.".to_owned(),
                );
            }
            Some(ReleaseReview {
                reviewer: name.trim().to_owned(),
                registration: registration.trim().to_owned(),
                date: date.trim().to_owned(),
                revision: sheet.latest_revision_code().to_owned(),
                red_checks: red,
            })
        } else {
            None
        };
        sheet.status = status;
        sheet.release_review = review;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn clean_sheet_can_be_released_but_a_new_red_check_revokes_it() {
        let mut project = Project::default();
        let id = project
            .sheet_set
            .add_sheet("Planta", SheetContent::FloorPlan, &project.sheet);
        assert!(red_checks(&project).is_empty());
        project
            .set_sheet_status(id, SheetStatus::LiberadoObra, None)
            .unwrap();
        let sheet = &project.sheet_set.sheets[0];
        assert_eq!(sheet.effective_status(&[]), SheetStatus::LiberadoObra);
        assert_eq!(
            sheet.effective_status(&["nova falha".to_owned()]),
            SheetStatus::ParaAprovacao
        );
    }

    #[test]
    fn sheet_set_handles_pagination_reordering_revisions_and_master_list() {
        let mut set = SheetSet::default();
        let base = SheetLayout::default();

        let s1 = set.add_sheet("Planta Baixa", SheetContent::FloorPlan, &base);
        let s2 = set.add_sheet("Corte AA", SheetContent::Section { id: 1 }, &base);
        assert_eq!(set.sheets.len(), 2);
        assert_eq!(set.sheets[0].code, "PR-01");
        assert_eq!(set.sheets[0].number, "01/02");
        assert_eq!(set.sheets[1].code, "PR-02");
        assert_eq!(set.sheets[1].number, "02/02");

        let rev = set
            .add_revision(s1, "20/11/2026", "Arq. Maria", "Ajuste de esquadrias")
            .unwrap();
        assert_eq!(rev, "R01");
        assert_eq!(set.sheets[0].latest_revision_code(), "R01");
        assert_eq!(set.sheets[0].layout.title_block.revision, "R01");

        assert!(set.move_sheet(s2, -1));
        assert_eq!(set.sheets[0].id, s2);
        assert_eq!(set.sheets[0].number, "01/02");
        assert_eq!(set.sheets[1].id, s1);
        assert_eq!(set.sheets[1].number, "02/02");

        let mut project = Project::default();
        project.name = "Residência Sul".to_owned();
        project.sheet_set = set.clone();
        let csv = set.transmittal_csv(&project);
        assert!(csv.contains("PR-01;01/02;Corte AA"));
        assert!(csv.contains("PR-02;02/02;Planta Baixa"));
        assert!(csv.contains("R01"));

        assert!(set.remove_sheet(s2));
        assert_eq!(set.sheets.len(), 1);
        assert_eq!(set.sheets[0].number, "01/01");
    }

    #[test]
    fn red_results_require_a_dated_reviewer_and_new_results_revoke_release() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project.generate_structure();
        let beam = project.structure.beams[0].id;
        project.update_beam(beam, None, Some(0.20)).unwrap();
        let base = project.sheet.clone();
        let id = project
            .sheet_set
            .add_sheet("Forma", SheetContent::FloorPlan, &base);
        let red = red_checks(&project);
        assert!(!red.is_empty());

        assert!(
            project
                .set_sheet_status(id, SheetStatus::LiberadoObra, None)
                .is_err()
        );
        assert_eq!(
            project.sheet_set.sheets[0].status,
            SheetStatus::EmElaboracao
        );
        // A legacy or hand-edited file with a raw released flag is still gated.
        project.sheet_set.sheets[0].status = SheetStatus::LiberadoObra;
        assert_eq!(
            project.sheet_set.sheets[0].effective_status(&red),
            SheetStatus::ParaAprovacao
        );
        project.sheet_set.sheets[0].status = SheetStatus::EmElaboracao;
        assert!(
            project
                .set_sheet_status(
                    id,
                    SheetStatus::LiberadoObra,
                    Some(("Ana", "sem registro", "23/11/2026"))
                )
                .is_err()
        );
        project
            .set_sheet_status(
                id,
                SheetStatus::LiberadoObra,
                Some(("Ana Souza", "CREA 12345", "23/11/2026")),
            )
            .unwrap();
        let sheet = &project.sheet_set.sheets[0];
        assert_eq!(sheet.effective_status(&red), SheetStatus::LiberadoObra);
        let released_csv = project.sheet_set.transmittal_csv(&project);
        assert!(released_csv.contains("Ana Souza;CREA 12345;23/11/2026"));
        let saved = serde_json::to_string(&project.sheet_set).unwrap();
        assert!(saved.contains("Ana Souza"));
        let restored: SheetSet = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            restored.sheets[0].effective_status(&red),
            SheetStatus::LiberadoObra
        );

        project.update_beam(beam, None, Some(0.18)).unwrap();
        let changed_red = red_checks(&project);
        assert_ne!(red, changed_red);
        assert_eq!(
            project.sheet_set.sheets[0].effective_status(&changed_red),
            SheetStatus::ParaAprovacao
        );
        let csv = project.sheet_set.transmittal_csv(&project);
        assert!(csv.contains("Para aprovação"));
        assert!(!csv.contains("Ana Souza;CREA 12345"));

        project
            .set_sheet_status(
                id,
                SheetStatus::LiberadoObra,
                Some(("Ana Souza", "CREA 12345", "24/11/2026")),
            )
            .unwrap();
        project
            .sheet_set
            .add_revision(id, "24/11/2026", "Ana Souza", "Viga revista")
            .unwrap();
        assert_eq!(
            project.sheet_set.sheets[0].status,
            SheetStatus::ParaAprovacao
        );
        assert!(project.sheet_set.sheets[0].release_review.is_none());
    }
}
