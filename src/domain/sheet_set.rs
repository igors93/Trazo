//! Multi-sheet documentation sets with automatic pagination (`01/03`),
//! revision history (`R00`, `R01`, ...), emission status and master document
//! list (Lista Mestra de Documentos) export.

use serde::{Deserialize, Serialize};

use super::sheet::SheetLayout;

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
        if !date.trim().is_empty() {
            sheet.layout.title_block.date = date.trim().to_owned();
        }
        Ok(code)
    }

    /// Exports the Master Document List (Lista Mestra de Documentos) as CSV.
    pub fn transmittal_csv(&self, project_name: &str) -> String {
        let mut out = format!("Lista Mestra de Documentos — {project_name}\n");
        out.push_str("Código;Prancha;Título;Conteúdo;Formato;Escala;Revisão;Situação;Última Data;Última Descrição\n");
        for sheet in &self.sheets {
            let last = sheet.revisions.last();
            let last_date = last.map(|r| r.date.as_str()).unwrap_or("");
            let last_desc = last.map(|r| r.description.as_str()).unwrap_or("");
            out.push_str(&format!(
                "{};{};{};{};{};{};{};{};{};{}\n",
                sheet.code,
                sheet.number,
                sheet.title,
                sheet.content.label(),
                sheet.layout.size.short_name(),
                sheet.layout.scale.label(),
                sheet.latest_revision_code(),
                sheet.status.label(),
                last_date,
                last_desc
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let csv = set.transmittal_csv("Residência Sul");
        assert!(csv.contains("PR-01;01/02;Corte AA"));
        assert!(csv.contains("PR-02;02/02;Planta Baixa"));
        assert!(csv.contains("R01"));

        assert!(set.remove_sheet(s2));
        assert_eq!(set.sheets.len(), 1);
        assert_eq!(set.sheets[0].number, "01/01");
    }
}
