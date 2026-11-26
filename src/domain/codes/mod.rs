//! Data-driven building-code engine (`T-08`, `T-09`, `T-10`, `T-11`):
//!
//! - Five disciplines (`Architectural`, `Structural`, `Electrical`, `Plumbing`, `Mechanical`)
//! - Layered resolution: `Base` -> up to `16` ordered `Amendments` -> `Project` overrides
//! - Edition pinning saved inside the project so Trazo upgrades never silently shift verdicts
//! - Full per-rule provenance (`document`, `item`, `page`, `kind`, `checked`)
//! - `WHOLE` replacement sections (utility electrical tables never cross-contaminate)
//! - Format-2 XML & JSON import/export
//! - Shared site & climate parameters (`SiteClimate`)
//! - Real-time architectural checks (26 stair rules + headroom + interactive solver, ramps, doors, rooms)
//! - 9 Brazilian electrical utilities + collective multifamily residential demand

pub mod checks;
pub mod data;
pub mod electrical_demand;
pub mod site;
pub mod stairs;
pub mod xml;

#[cfg(test)]
mod rule_tests;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub use checks::check_project_codes;
pub use data::{builtin_codes, builtin_presets, builtin_site_presets};
pub use electrical_demand::{BuildingDemandInput, DwellingGroup, calculate_building_demand};
pub use site::{KeraunicLevel, SiteClimate, SitePreset};
pub use stairs::{
    StairFinding, StairLocks, StairProposal, apply_stair_proposal, solve_stair, stair_findings,
};

/// Maximum number of ordered amendments active per discipline.
pub const MAX_AMENDMENTS: usize = 16;
/// Maximum number of project-owned custom code packs.
pub const MAX_OWN_CODES: usize = 100;
/// Maximum XML / JSON payload size accepted on import (16 MiB).
pub const MAX_XML_BYTES: usize = 16 * 1024 * 1024;

/// Sections that are replaced as a whole rather than merged rule-by-rule across
/// utilities (prevents mixing tables from two different electrical utilities).
const WHOLE_SECTIONS: &[(CodeDiscipline, &str)] = &[
    (CodeDiscipline::Electrical, "unit_demand"),
    (CodeDiscipline::Electrical, "building_demand"),
    (CodeDiscipline::Electrical, "entrance"),
    (CodeDiscipline::Electrical, "utility_profile"),
];

/// Discipline covered by the data-driven building code engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeDiscipline {
    Architectural,
    Structural,
    Electrical,
    Plumbing,
    Mechanical,
}

impl CodeDiscipline {
    pub const ALL: [CodeDiscipline; 5] = [
        Self::Architectural,
        Self::Structural,
        Self::Electrical,
        Self::Plumbing,
        Self::Mechanical,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Architectural => "architectural",
            Self::Structural => "structural",
            Self::Electrical => "electrical",
            Self::Plumbing => "plumbing",
            Self::Mechanical => "mechanical",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Architectural => "Arquitetura",
            Self::Structural => "Estrutura",
            Self::Electrical => "Elétrica",
            Self::Plumbing => "Hidrossanitário",
            Self::Mechanical => "Conforto / Mecânica",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "architectural" | "arquitetura" => Some(Self::Architectural),
            "structural" | "estrutura" => Some(Self::Structural),
            "electrical" | "eletrica" | "elétrica" => Some(Self::Electrical),
            "plumbing" | "hidrossanitario" | "hidrossanitário" | "hidraulica" | "hidráulica" => {
                Some(Self::Plumbing)
            }
            "mechanical" | "conforto" | "mecanica" | "mecânica" => Some(Self::Mechanical),
            _ => None,
        }
    }
}

/// Normative force of a rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    #[default]
    Requirement,
    Recommendation,
    Practice,
    Assumption,
    Historical,
    Forgiving,
}

impl RuleKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Requirement => "requirement",
            Self::Recommendation => "recommendation",
            Self::Practice => "practice",
            Self::Assumption => "assumption",
            Self::Historical => "historical",
            Self::Forgiving => "forgiving",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Requirement => "Requisito obrigatório",
            Self::Recommendation => "Recomendação",
            Self::Practice => "Prática usual",
            Self::Assumption => "Premissa / ajuste do projeto",
            Self::Historical => "Referência histórica",
            Self::Forgiving => "Referência permissiva",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "requirement" | "requisito" => Some(Self::Requirement),
            "recommendation" | "recomendacao" | "recomendação" => Some(Self::Recommendation),
            "practice" | "pratica" | "prática" => Some(Self::Practice),
            "assumption" | "premissa" => Some(Self::Assumption),
            "historical" | "historico" | "histórico" => Some(Self::Historical),
            "forgiving" | "permissivo" => Some(Self::Forgiving),
            _ => None,
        }
    }
}

/// Structured reference to a published standard document.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DocumentRef {
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub edition: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub publisher: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub accessed: String,
}

/// Clause-level citation pointing into [`BuildingCode::documents`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Citation {
    pub doc: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub item: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub page: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// Source attribution attached to a section or rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleSource {
    Text(String),
    Cited(Citation),
}

impl RuleSource {
    pub fn format_with(&self, documents: &BTreeMap<String, DocumentRef>) -> String {
        match self {
            Self::Text(t) => t.clone(),
            Self::Cited(c) => {
                let mut parts = Vec::new();
                if let Some(doc) = documents.get(&c.doc) {
                    if doc.edition.is_empty() {
                        parts.push(doc.title.clone());
                    } else {
                        parts.push(format!("{} ({})", doc.title, doc.edition));
                    }
                } else if !c.doc.is_empty() {
                    parts.push(c.doc.clone());
                }
                if !c.item.is_empty() {
                    parts.push(c.item.clone());
                }
                if !c.page.is_empty() {
                    parts.push(format!("p. {}", c.page));
                }
                if !c.note.is_empty() {
                    parts.push(c.note.clone());
                }
                parts.join(", ")
            }
        }
    }
}

/// One versioned building code pack or amendment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildingCode {
    pub code: String,
    pub discipline: CodeDiscipline,
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub region: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub edition: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub superseded_by: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub amends: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub based_on: String,
    #[serde(skip_serializing_if = "is_false")]
    pub default: bool,
    /// Audit note describing verification against the published standard text (empty = unchecked).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub checked: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub documents: BTreeMap<String, DocumentRef>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub sections: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<String, RuleSource>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub kinds: BTreeMap<String, RuleKind>,
}

impl Default for BuildingCode {
    fn default() -> Self {
        Self {
            code: String::new(),
            discipline: CodeDiscipline::Architectural,
            name: String::new(),
            region: String::new(),
            edition: String::new(),
            superseded_by: String::new(),
            amends: String::new(),
            based_on: String::new(),
            default: false,
            checked: String::new(),
            references: Vec::new(),
            note: String::new(),
            documents: BTreeMap::new(),
            sections: BTreeMap::new(),
            sources: BTreeMap::new(),
            kinds: BTreeMap::new(),
        }
    }
}

impl BuildingCode {
    /// Whether this pack has been audited against the published standard text.
    pub fn is_checked(&self) -> bool {
        !self.checked.trim().is_empty()
    }

    pub fn rule_source(&self, section: &str, rule: &str) -> Option<String> {
        let dotted = format!("{section}.{rule}");
        self.sources
            .get(&dotted)
            .or_else(|| self.sources.get(section))
            .map(|s| s.format_with(&self.documents))
    }

    pub fn rule_kind(&self, section: &str, rule: &str) -> RuleKind {
        let dotted = format!("{section}.{rule}");
        self.kinds
            .get(&dotted)
            .or_else(|| self.kinds.get(section))
            .copied()
            .unwrap_or(RuleKind::Requirement)
    }
}

/// Regional preset configuring all 5 disciplines at once.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CodePreset {
    pub id: String,
    pub name: String,
    pub region: String,
    pub note: String,
    pub architectural: String,
    #[serde(default)]
    pub architectural_amendments: Vec<String>,
    pub structural: String,
    #[serde(default)]
    pub structural_amendments: Vec<String>,
    pub electrical: String,
    #[serde(default)]
    pub electrical_amendments: Vec<String>,
    pub plumbing: String,
    #[serde(default)]
    pub plumbing_amendments: Vec<String>,
    pub mechanical: String,
    #[serde(default)]
    pub mechanical_amendments: Vec<String>,
}

/// Step-function lookup table (`rows` sorted ascending by threshold, plus `beyond`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Steps {
    pub rows: Vec<(f64, f64)>,
    pub beyond: f64,
}

impl Steps {
    pub fn at(&self, x: f64) -> f64 {
        for &(limit, value) in &self.rows {
            if x <= limit {
                return value;
            }
        }
        self.beyond
    }
}

/// Active selection + pinned editions + project-level overrides for one discipline.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisciplineChoice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub amendments: Vec<String>,
    /// Pinned edition per code key so upgrades never silently change saved projects.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub editions: BTreeMap<String, String>,
    /// Project-level rule overrides: `section -> { rule_key -> Value }`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub changes: BTreeMap<String, Map<String, Value>>,
}

impl DisciplineChoice {
    pub fn is_default(&self) -> bool {
        self.code.is_none()
            && self.amendments.is_empty()
            && self.editions.is_empty()
            && self.changes.is_empty()
    }
}

/// Full building-code and site configuration persisted inside `Project`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildingCodes {
    #[serde(skip_serializing_if = "DisciplineChoice::is_default")]
    pub architectural: DisciplineChoice,
    #[serde(skip_serializing_if = "DisciplineChoice::is_default")]
    pub structural: DisciplineChoice,
    #[serde(skip_serializing_if = "DisciplineChoice::is_default")]
    pub electrical: DisciplineChoice,
    #[serde(skip_serializing_if = "DisciplineChoice::is_default")]
    pub plumbing: DisciplineChoice,
    #[serde(skip_serializing_if = "DisciplineChoice::is_default")]
    pub mechanical: DisciplineChoice,
    /// Project-owned custom codes or imported municipal/utility amendments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub own: Vec<BuildingCode>,
    /// Shared site & climate parameters (`T-09`).
    #[serde(skip_serializing_if = "SiteClimate::is_empty")]
    pub site: SiteClimate,
}

/// Origin layer of a resolved rule value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layer {
    Library(String),
    Base(String),
    Amendment(String),
    Project,
}

impl Layer {
    pub fn label(&self) -> String {
        match self {
            Self::Library(c) => format!("Biblioteca ({c})"),
            Self::Base(c) => format!("Base ({c})"),
            Self::Amendment(c) => format!("Emenda ({c})"),
            Self::Project => "Ajuste do projeto".to_owned(),
        }
    }
}

/// One resolved rule with its final JSON value, provenance and force.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRule {
    pub key: String,
    pub value: Value,
    pub layer: Layer,
    pub source: Option<String>,
    pub kind: RuleKind,
    pub checked: bool,
}

/// Complete resolved section merging `Base -> Amendments -> Project`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedSection {
    pub value: Value,
    pub rules: BTreeMap<String, ResolvedRule>,
    pub section_layer: Option<Layer>,
    pub section_source: Option<String>,
    pub section_kind: RuleKind,
    pub section_checked: bool,
}

/// Warning emitted when a project's pinned edition differs from the bundled edition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditionDrift {
    pub discipline: CodeDiscipline,
    pub code: String,
    pub pinned: String,
    pub available: String,
}

impl BuildingCodes {
    pub fn is_default(&self) -> bool {
        self.architectural.is_default()
            && self.structural.is_default()
            && self.electrical.is_default()
            && self.plumbing.is_default()
            && self.mechanical.is_default()
            && self.own.is_empty()
            && self.site.is_empty()
    }

    pub fn choice(&self, discipline: CodeDiscipline) -> &DisciplineChoice {
        match discipline {
            CodeDiscipline::Architectural => &self.architectural,
            CodeDiscipline::Structural => &self.structural,
            CodeDiscipline::Electrical => &self.electrical,
            CodeDiscipline::Plumbing => &self.plumbing,
            CodeDiscipline::Mechanical => &self.mechanical,
        }
    }

    pub fn choice_mut(&mut self, discipline: CodeDiscipline) -> &mut DisciplineChoice {
        match discipline {
            CodeDiscipline::Architectural => &mut self.architectural,
            CodeDiscipline::Structural => &mut self.structural,
            CodeDiscipline::Electrical => &mut self.electrical,
            CodeDiscipline::Plumbing => &mut self.plumbing,
            CodeDiscipline::Mechanical => &mut self.mechanical,
        }
    }

    /// All available packs for a discipline (project-owned packs shadow built-ins of same code).
    pub fn available_codes(&self, discipline: CodeDiscipline) -> Vec<&BuildingCode> {
        let mut out: Vec<&BuildingCode> = self
            .own
            .iter()
            .filter(|c| c.discipline == discipline && c.superseded_by.is_empty())
            .collect();
        for builtin in builtin_codes() {
            if builtin.discipline == discipline
                && builtin.superseded_by.is_empty()
                && !out.iter().any(|o| o.code == builtin.code)
            {
                out.push(builtin);
            }
        }
        out
    }

    /// Find a code pack by discipline + key (checking project-owned first, then built-ins).
    pub fn find_code(&self, discipline: CodeDiscipline, code: &str) -> Option<&BuildingCode> {
        self.own
            .iter()
            .find(|c| c.discipline == discipline && c.code == code)
            .or_else(|| {
                builtin_codes().iter().find(|c| {
                    c.discipline == discipline && c.code == code && c.superseded_by.is_empty()
                })
            })
            .or_else(|| {
                builtin_codes()
                    .iter()
                    .find(|c| c.discipline == discipline && c.code == code)
            })
    }

    /// Find a code pack honouring a pinned edition when older editions exist.
    pub fn find_code_edition(
        &self,
        discipline: CodeDiscipline,
        code: &str,
        pinned_edition: Option<&str>,
    ) -> Option<&BuildingCode> {
        if let Some(edition) = pinned_edition {
            if let Some(found) = self
                .own
                .iter()
                .find(|c| c.discipline == discipline && c.code == code && c.edition == edition)
            {
                return Some(found);
            }
            if let Some(found) = builtin_codes()
                .iter()
                .find(|c| c.discipline == discipline && c.code == code && c.edition == edition)
            {
                return Some(found);
            }
        }
        self.find_code(discipline, code)
    }

    /// Default base code for a discipline (`default: true` or first built-in).
    pub fn default_code(&self, discipline: CodeDiscipline) -> Option<&BuildingCode> {
        builtin_codes()
            .iter()
            .find(|c| c.discipline == discipline && c.default && c.superseded_by.is_empty())
            .or_else(|| builtin_codes().iter().find(|c| c.discipline == discipline))
    }

    /// Active base code key for a discipline.
    pub fn active_base_key(&self, discipline: CodeDiscipline) -> String {
        if let Some(key) = &self.choice(discipline).code
            && self.find_code(discipline, key).is_some()
        {
            return key.clone();
        }
        self.default_code(discipline)
            .map(|c| c.code.clone())
            .unwrap_or_else(|| "generic".to_owned())
    }

    /// Set the active base code and pin its edition (clearing incompatible amendments).
    pub fn set_base(&mut self, discipline: CodeDiscipline, code: &str) -> Result<(), String> {
        let Some(pack) = self.find_code(discipline, code).cloned() else {
            return Err(format!(
                "Norma '{code}' não encontrada na disciplina {}.",
                discipline.label()
            ));
        };
        if !pack.amends.is_empty() {
            return Err(format!(
                "'{code}' é uma emenda de '{}' e deve ser adicionada como emenda, não como norma base.",
                pack.amends
            ));
        }
        let choice = self.choice_mut(discipline);
        choice.code = Some(pack.code.clone());
        if !pack.edition.is_empty() {
            choice.editions.insert(pack.code.clone(), pack.edition);
        }
        self.prune_invalid_amendments(discipline);
        Ok(())
    }

    /// Add an ordered amendment (up to `MAX_AMENDMENTS = 16`) and pin its edition.
    pub fn add_amendment(&mut self, discipline: CodeDiscipline, code: &str) -> Result<(), String> {
        let Some(pack) = self.find_code(discipline, code).cloned() else {
            return Err(format!(
                "Emenda '{code}' não encontrada na disciplina {}.",
                discipline.label()
            ));
        };
        let active = self.active_layer_keys(discipline);
        if !pack.amends.is_empty() && !active.iter().any(|k| k == &pack.amends) {
            return Err(format!(
                "A emenda '{}' requer '{}' ativo como norma base ou emenda anterior.",
                pack.code, pack.amends
            ));
        }
        let choice = self.choice_mut(discipline);
        if choice.amendments.iter().any(|a| a == &pack.code) {
            return Ok(());
        }
        if choice.amendments.len() >= MAX_AMENDMENTS {
            return Err(format!(
                "Limite máximo de {MAX_AMENDMENTS} emendas por disciplina atingido."
            ));
        }
        choice.amendments.push(pack.code.clone());
        if !pack.edition.is_empty() {
            choice.editions.insert(pack.code, pack.edition);
        }
        Ok(())
    }

    /// Replace the full ordered list of amendments for a discipline.
    pub fn set_amendments(
        &mut self,
        discipline: CodeDiscipline,
        amendments: Vec<String>,
    ) -> Result<(), String> {
        if amendments.len() > MAX_AMENDMENTS {
            return Err(format!(
                "Limite máximo de {MAX_AMENDMENTS} emendas por disciplina excedido."
            ));
        }
        let previous = self.choice(discipline).clone();
        self.choice_mut(discipline).amendments.clear();
        for code in amendments {
            if let Err(err) = self.add_amendment(discipline, &code) {
                *self.choice_mut(discipline) = previous;
                return Err(err);
            }
        }
        Ok(())
    }

    /// Remove an amendment (and any downstream amendments whose `amends` depended on it).
    pub fn remove_amendment(&mut self, discipline: CodeDiscipline, code: &str) {
        self.choice_mut(discipline).amendments.retain(|a| a != code);
        self.prune_invalid_amendments(discipline);
    }

    /// Apply a bundled regional preset (`generic`, `brasil`, `brasil-emergencia`) across all 5 disciplines.
    pub fn apply_preset(&mut self, preset_id: &str) -> Result<&'static CodePreset, String> {
        let q = preset_id.trim().to_ascii_lowercase();
        let Some(preset) = builtin_presets()
            .iter()
            .find(|p| p.id.to_ascii_lowercase() == q || p.name.to_ascii_lowercase() == q)
        else {
            return Err(format!("Preset normativo '{preset_id}' não encontrado."));
        };
        let specs = [
            (
                CodeDiscipline::Architectural,
                &preset.architectural,
                &preset.architectural_amendments,
            ),
            (
                CodeDiscipline::Structural,
                &preset.structural,
                &preset.structural_amendments,
            ),
            (
                CodeDiscipline::Electrical,
                &preset.electrical,
                &preset.electrical_amendments,
            ),
            (
                CodeDiscipline::Plumbing,
                &preset.plumbing,
                &preset.plumbing_amendments,
            ),
            (
                CodeDiscipline::Mechanical,
                &preset.mechanical,
                &preset.mechanical_amendments,
            ),
        ];
        for (disc, base, amends) in specs {
            if !base.is_empty() {
                self.set_base(disc, base)?;
            }
            self.set_amendments(disc, amends.clone())?;
        }
        Ok(preset)
    }

    /// Override one rule in the Project layer (disallowed for `WHOLE` sections;
    /// edit those inside a custom amendment instead).
    pub fn set_project_rule(
        &mut self,
        discipline: CodeDiscipline,
        section: &str,
        rule: &str,
        value: Value,
    ) -> Result<(), String> {
        if is_whole_section(discipline, section) {
            return Err(format!(
                "A seção '{section}' é substituída por inteiro (WHOLE). Crie ou edite uma emenda própria em vez de sobrescrever regra isolada no projeto."
            ));
        }
        let choice = self.choice_mut(discipline);
        choice
            .changes
            .entry(section.to_owned())
            .or_default()
            .insert(rule.to_owned(), value);
        Ok(())
    }

    /// Remove a single project-level override (or an entire section's overrides if `rule` is `None`).
    pub fn reset_project_rule(
        &mut self,
        discipline: CodeDiscipline,
        section: &str,
        rule: Option<&str>,
    ) {
        let choice = self.choice_mut(discipline);
        if let Some(rule_key) = rule {
            if let Some(map) = choice.changes.get_mut(section) {
                map.remove(rule_key);
                if map.is_empty() {
                    choice.changes.remove(section);
                }
            }
        } else {
            choice.changes.remove(section);
        }
    }

    /// Register or replace a project-owned custom code/amendment pack.
    pub fn install_own_code(&mut self, pack: BuildingCode) -> Result<(), String> {
        if pack.code.trim().is_empty() {
            return Err("Código da norma não pode ser vazio.".to_owned());
        }
        if let Some(existing) = self
            .own
            .iter_mut()
            .find(|c| c.discipline == pack.discipline && c.code == pack.code)
        {
            *existing = pack;
            return Ok(());
        }
        if self.own.len() >= MAX_OWN_CODES {
            return Err(format!(
                "Limite máximo de {MAX_OWN_CODES} normas próprias no projeto atingido."
            ));
        }
        self.own.push(pack);
        Ok(())
    }

    /// Ordered list of active layer code keys (`[base, valid_amend_1, ...]`).
    pub fn active_layer_keys(&self, discipline: CodeDiscipline) -> Vec<String> {
        let base_key = self.active_base_key(discipline);
        let choice = self.choice(discipline);
        let mut active = vec![base_key];
        // Fixed-point filtering so chained amendments (A -> B -> base) resolve cleanly.
        let mut changed = true;
        while changed {
            changed = false;
            for amend_key in &choice.amendments {
                if active.iter().any(|k| k == amend_key) {
                    continue;
                }
                let pinned = choice.editions.get(amend_key).map(String::as_str);
                if let Some(pack) = self.find_code_edition(discipline, amend_key, pinned)
                    && (pack.amends.is_empty() || active.iter().any(|k| k == &pack.amends))
                {
                    active.push(amend_key.clone());
                    changed = true;
                }
            }
        }
        active
    }

    fn prune_invalid_amendments(&mut self, discipline: CodeDiscipline) {
        let valid = self.active_layer_keys(discipline);
        self.choice_mut(discipline)
            .amendments
            .retain(|a| valid.iter().any(|v| v == a));
    }

    /// Detect any pinned edition in the project that differs from the latest bundled edition.
    pub fn edition_drifts(&self) -> Vec<EditionDrift> {
        let mut drifts = Vec::new();
        for disc in CodeDiscipline::ALL {
            let choice = self.choice(disc);
            for (code_key, pinned) in &choice.editions {
                if let Some(current) = self.find_code(disc, code_key)
                    && !current.edition.is_empty()
                    && !pinned.is_empty()
                    && current.edition != *pinned
                {
                    drifts.push(EditionDrift {
                        discipline: disc,
                        code: code_key.clone(),
                        pinned: pinned.clone(),
                        available: current.edition.clone(),
                    });
                }
            }
        }
        drifts
    }

    /// Resolve a section by merging `Library fallback -> Base -> Amendments -> Project`.
    pub fn resolve_section(&self, discipline: CodeDiscipline, section: &str) -> ResolvedSection {
        let whole = is_whole_section(discipline, section);
        let choice = self.choice(discipline);
        let active_keys = self.active_layer_keys(discipline);
        let mut out = ResolvedSection::default();

        // Apply active layers in order: Base (index 0) then Amendments (1..).
        for (idx, code_key) in active_keys.iter().enumerate() {
            let pinned = choice.editions.get(code_key).map(String::as_str);
            let Some(pack) = self.find_code_edition(discipline, code_key, pinned) else {
                continue;
            };
            let Some(sec_val) = pack.sections.get(section) else {
                continue;
            };
            let layer = if idx == 0 {
                Layer::Base(pack.code.clone())
            } else {
                Layer::Amendment(pack.code.clone())
            };
            apply_pack_section(&mut out, pack, section, sec_val, layer, whole);
        }

        // Library fallback if no active layer provided the section at all.
        if out.section_layer.is_none()
            && let Some(fallback) = builtin_codes()
                .iter()
                .find(|c| c.discipline == discipline && c.sections.contains_key(section))
            && let Some(sec_val) = fallback.sections.get(section)
        {
            apply_pack_section(
                &mut out,
                fallback,
                section,
                sec_val,
                Layer::Library(fallback.code.clone()),
                whole,
            );
        }

        // Apply project-level overrides (never for WHOLE sections).
        if !whole
            && let Some(overrides) = choice.changes.get(section)
            && !overrides.is_empty()
        {
            let mut merged_obj = out.value.as_object().cloned().unwrap_or_default();
            for (k, v) in overrides {
                merged_obj.insert(k.clone(), v.clone());
                out.rules.insert(
                    k.clone(),
                    ResolvedRule {
                        key: k.clone(),
                        value: v.clone(),
                        layer: Layer::Project,
                        source: None,
                        kind: RuleKind::Assumption,
                        checked: false,
                    },
                );
            }
            out.value = Value::Object(merged_obj);
        }

        out
    }

    /// Resolve a section and deserialize it into a strongly-typed rules struct `T`.
    pub fn resolve_typed<T>(&self, discipline: CodeDiscipline, section: &str) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let resolved = self.resolve_section(discipline, section);
        serde_json::from_value::<T>(resolved.value).ok()
    }

    /// Build a single flattened [`BuildingCode`] snapshot of the effective rules
    /// in force for one discipline (used for XML/JSON export).
    pub fn export_effective_pack(&self, discipline: CodeDiscipline) -> BuildingCode {
        let active = self.active_layer_keys(discipline);
        let base_key = active
            .first()
            .cloned()
            .unwrap_or_else(|| "generic".to_owned());
        let base_pack = self.find_code(discipline, &base_key);

        let mut sections_set: Vec<String> = Vec::new();
        for key in &active {
            if let Some(pack) = self.find_code(discipline, key) {
                for sec in pack.sections.keys() {
                    if !sections_set.contains(sec) {
                        sections_set.push(sec.clone());
                    }
                }
            }
        }
        for sec in self.choice(discipline).changes.keys() {
            if !sections_set.contains(sec) {
                sections_set.push(sec.clone());
            }
        }
        sections_set.sort();

        let mut exported = BuildingCode {
            code: if active.len() <= 1 && self.choice(discipline).changes.is_empty() {
                base_key.clone()
            } else {
                format!("{base_key}-projeto")
            },
            discipline,
            name: base_pack
                .map(|p| p.name.clone())
                .unwrap_or_else(|| format!("Norma {}", discipline.label())),
            region: base_pack.map(|p| p.region.clone()).unwrap_or_default(),
            edition: base_pack.map(|p| p.edition.clone()).unwrap_or_default(),
            based_on: base_key,
            checked: base_pack.map(|p| p.checked.clone()).unwrap_or_default(),
            references: base_pack.map(|p| p.references.clone()).unwrap_or_default(),
            note: base_pack.map(|p| p.note.clone()).unwrap_or_default(),
            documents: base_pack.map(|p| p.documents.clone()).unwrap_or_default(),
            ..BuildingCode::default()
        };

        for sec in &sections_set {
            let resolved = self.resolve_section(discipline, sec);
            if !resolved.value.is_null() {
                exported.sections.insert(sec.clone(), resolved.value);
                if let Some(src) = resolved.section_source {
                    exported.sources.insert(sec.clone(), RuleSource::Text(src));
                }
                exported.kinds.insert(sec.clone(), resolved.section_kind);
                for (k, r) in resolved.rules {
                    let dotted = format!("{sec}.{k}");
                    if let Some(src) = r.source {
                        exported
                            .sources
                            .insert(dotted.clone(), RuleSource::Text(src));
                    }
                    exported.kinds.insert(dotted, r.kind);
                }
            }
        }

        exported
    }

    /// Export all 5 disciplines' effective packs to a single Format-2 XML document.
    pub fn export_all_xml(&self) -> String {
        let packs: Vec<BuildingCode> = CodeDiscipline::ALL
            .iter()
            .map(|&d| self.export_effective_pack(d))
            .collect();
        xml::export_bundle_xml(&packs)
    }

    /// Import XML or JSON pack(s), install into `self.own`, and optionally activate them.
    pub fn import_packs(
        &mut self,
        raw: &str,
        activate: bool,
    ) -> Result<xml::ImportedPacks, String> {
        let imported = xml::import_auto(raw)?;
        for pack in &imported.codes {
            self.install_own_code(pack.clone())?;
            if activate {
                if pack.amends.is_empty() {
                    let _ = self.set_base(pack.discipline, &pack.code);
                } else {
                    let _ = self.add_amendment(pack.discipline, &pack.code);
                }
            }
        }
        Ok(imported)
    }
}

fn apply_pack_section(
    out: &mut ResolvedSection,
    pack: &BuildingCode,
    section: &str,
    sec_val: &Value,
    layer: Layer,
    whole: bool,
) {
    let sec_src = pack
        .sources
        .get(section)
        .map(|s| s.format_with(&pack.documents));
    let sec_kind = pack
        .kinds
        .get(section)
        .copied()
        .unwrap_or(RuleKind::Requirement);
    let checked = pack.is_checked();

    if whole || !sec_val.is_object() {
        out.value = sec_val.clone();
        out.rules.clear();
        out.section_layer = Some(layer.clone());
        out.section_source = sec_src.clone();
        out.section_kind = sec_kind;
        out.section_checked = checked;
        if let Some(obj) = sec_val.as_object() {
            for (k, v) in obj {
                out.rules.insert(
                    k.clone(),
                    ResolvedRule {
                        key: k.clone(),
                        value: v.clone(),
                        layer: layer.clone(),
                        source: pack.rule_source(section, k).or_else(|| sec_src.clone()),
                        kind: pack.rule_kind(section, k),
                        checked,
                    },
                );
            }
        }
        return;
    }

    // Object section: merge rule-by-rule (last layer wins per key).
    let mut merged = out.value.as_object().cloned().unwrap_or_default();
    if let Some(obj) = sec_val.as_object() {
        for (k, v) in obj {
            merged.insert(k.clone(), v.clone());
            out.rules.insert(
                k.clone(),
                ResolvedRule {
                    key: k.clone(),
                    value: v.clone(),
                    layer: layer.clone(),
                    source: pack.rule_source(section, k),
                    kind: pack.rule_kind(section, k),
                    checked,
                },
            );
        }
    }
    out.value = Value::Object(merged);
    out.section_layer = Some(layer);
    if sec_src.is_some() {
        out.section_source = sec_src;
    }
    out.section_kind = sec_kind;
    out.section_checked = checked;
}

pub fn is_whole_section(discipline: CodeDiscipline, section: &str) -> bool {
    WHOLE_SECTIONS
        .iter()
        .any(|&(d, s)| d == discipline && s == section)
}

pub fn slugify_code(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_owned();
    if trimmed.is_empty() {
        "norma-customizada".to_owned()
    } else {
        trimmed
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}
