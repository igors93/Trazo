//! Engineering safety traffic lights (#101): translates structural and
//! electrical pre-sizing results into plain-language green/yellow/red verdicts
//! with margins and declared assumptions.

mod codes;
mod cross_checks;
mod electrical;
mod hypotheses;
mod site_risks;
mod stages;
mod structure;

#[cfg(test)]
mod tests;

use crate::domain::model::Project;

pub use codes::code_verdicts;
pub use electrical::electrical_verdicts;
pub use structure::structure_verdicts;

/// Up to this utilisation the light is green.
pub const GREEN_LIMIT: f64 = 0.85;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Light {
    Green,
    Yellow,
    Red,
}

impl Light {
    pub fn from_utilisation(utilisation: f64) -> Self {
        if !utilisation.is_finite() || utilisation > 1.0 + 1e-9 {
            Self::Red
        } else if utilisation > GREEN_LIMIT {
            Self::Yellow
        } else {
            Self::Green
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Green => "Verde",
            Self::Yellow => "Amarelo",
            Self::Red => "Vermelho",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Green => "verde",
            Self::Yellow => "amarelo",
            Self::Red => "vermelho",
        }
    }

    /// What the light means, in one sentence.
    pub fn meaning(self) -> &'static str {
        match self {
            Self::Green => "Atende com folga.",
            Self::Yellow => "Atende, mas com pouca folga: revise se puder.",
            Self::Red => "Não atende: precisa mudar antes de construir.",
        }
    }
}

/// One check of one element.
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    /// What is checked, such as "Altura" or "Flecha".
    pub name: String,
    /// Demand over capacity: 1.0 is exactly at the limit.
    pub utilisation: f64,
    pub light: Light,
    /// What to do, in plain words.
    pub simple: String,
    /// The numbers, the formula and the standard.
    pub detail: String,
}

impl Check {
    fn new(name: &str, utilisation: f64, simple: String, detail: String) -> Self {
        Self {
            name: name.to_owned(),
            utilisation,
            light: Light::from_utilisation(utilisation),
            simple,
            detail,
        }
    }

    /// A check that fails outright (not a ratio).
    fn fail(name: &str, simple: String, detail: String) -> Self {
        Self {
            name: name.to_owned(),
            utilisation: f64::INFINITY,
            light: Light::Red,
            simple,
            detail,
        }
    }

    /// Margin to the limit, in percent (negative when it fails).
    pub fn margin_percent(&self) -> f64 {
        if self.utilisation.is_finite() {
            (1.0 - self.utilisation) * 100.0
        } else {
            -100.0
        }
    }
}

/// What a verdict is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Discipline {
    Structure,
    Electrical,
    Architecture,
}

impl Discipline {
    pub fn label(self) -> &'static str {
        match self {
            Self::Structure => "Estrutura",
            Self::Electrical => "Elétrica",
            Self::Architecture => "Arquitetura · Normas",
        }
    }
}

/// All the checks of one element.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub discipline: Discipline,
    /// "Viga V1 · vão 2", "Pilar P3", "Circuito C4"…
    pub element: String,
    pub checks: Vec<Check>,
}

impl Verdict {
    /// The worst light of its checks.
    pub fn light(&self) -> Light {
        self.checks
            .iter()
            .map(|check| check.light)
            .max()
            .unwrap_or(Light::Green)
    }

    /// The governing check: the highest utilisation.
    pub fn governing(&self) -> Option<&Check> {
        self.checks
            .iter()
            .max_by(|a, b| a.utilisation.total_cmp(&b.utilisation))
    }
}

/// Every verdict of the project, worst first, with the counts and the
/// assumptions they rest on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SafetySummary {
    pub verdicts: Vec<Verdict>,
    pub green: usize,
    pub yellow: usize,
    pub red: usize,
    pub assumptions: Vec<String>,
}

impl SafetySummary {
    pub fn worst(&self) -> Option<Light> {
        if self.verdicts.is_empty() {
            None
        } else if self.red > 0 {
            Some(Light::Red)
        } else if self.yellow > 0 {
            Some(Light::Yellow)
        } else {
            Some(Light::Green)
        }
    }
}

pub(super) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

pub(super) fn cm(metres: f64) -> String {
    format!("{:.0} cm", metres * 100.0)
}

pub(super) fn limit_detail(violation: &crate::domain::limits::Violation) -> String {
    format!(
        "Valor {} acima do limite de aplicação {} (docs/engineering.md, limites de aplicação).",
        decimal(violation.value, 2),
        decimal(violation.limit, 2)
    )
}

/// The assumptions every structural and electrical result rests on.
pub fn assumptions(project: &Project) -> Vec<String> {
    let s = project.structure.settings;
    let mut lines = vec![
        "Pré-dimensionamento, não projeto final: construir exige um profissional habilitado (ART ou RRT) e o alvará da prefeitura.".to_owned(),
        format!(
            "Concreto fck {:.0} MPa, aço CA-50, solo com tensão admissível de {:.0} kPa.",
            s.fck_mpa, s.soil_kpa
        ),
        format!(
            "Cargas (NBR 6120:2019): ocupação {} (ψ0 = {}, ψ1 = {}, ψ2 = {}), sobrecarga {} kN/m², revestimento {} kN/m², alvenaria {} kN/m³.",
            s.occupancy.label(),
            decimal(s.occupancy.psi0(), 2),
            decimal(s.occupancy.psi1(), 2),
            decimal(s.occupancy.psi2(), 2),
            decimal(s.live_kn_m2, 1),
            decimal(s.finish_kn_m2, 1),
            decimal(s.masonry_kn_m3, 0)
        ),
        format!(
            "Vento na edificação (NBR 6123): V0 = {:.0} m/s, categoria {}, S1 = {}, S3 = {}, coeficientes da Tabela 4; combinações ELU e ELS pela NBR 8681 / NBR 6118 §11.8 (sem ação sísmica NBR 15421).",
            project.structure.wind.v0_ms,
            project.structure.wind.category,
            decimal(project.structure.wind.s1, 2),
            decimal(project.structure.wind.s3, 2)
        ),
        "Caixas-d'água e equipamentos entram como cargas concentradas permanentes; a carga informada de equipamento deve incluir impacto/vibração quando aplicável.".to_owned(),
        "Pilares por carga axial com fator de posição; sem flambagem nem flexão composta.".to_owned(),
        "Métodos estruturais documentados para a NBR 6118:2023; a edição 2026 ainda requer auditoria e revisão profissional.".to_owned(),
        "Fios de cobre com isolação de PVC, método B1, 30 °C; queda de tensão até 4 %.".to_owned(),
        "Métodos, normas e limites: docs/engineering.md.".to_owned(),
    ];
    // Who set each hypothesis, and on what evidence (#168).
    lines.extend(crate::domain::protection::hypotheses::origin_lines(project));
    if let Some(site_line) = project.codes.site.assumption_line() {
        lines.push(site_line);
    }
    let arch_base = project
        .codes
        .active_base_key(crate::domain::codes::CodeDiscipline::Architectural);
    if arch_base != "generic" || !project.codes.architectural.amendments.is_empty() {
        let amends = if project.codes.architectural.amendments.is_empty() {
            String::new()
        } else {
            format!(" + {}", project.codes.architectural.amendments.join(" + "))
        };
        lines.push(format!(
            "Códigos de obra ativos (Arquitetura): {arch_base}{amends}."
        ));
    }
    lines
}

/// Every verdict of the project, worst first.
pub fn safety_summary(project: &Project) -> SafetySummary {
    let mut verdicts = structure_verdicts(project, &project.structure_report());
    verdicts.extend(electrical_verdicts(&project.load_schedule()));
    verdicts.extend(code_verdicts(project));
    verdicts.sort_by(|a, b| {
        b.light()
            .cmp(&a.light())
            .then(a.discipline.cmp(&b.discipline))
    });
    let count = |light: Light| verdicts.iter().filter(|v| v.light() == light).count();
    SafetySummary {
        green: count(Light::Green),
        yellow: count(Light::Yellow),
        red: count(Light::Red),
        verdicts,
        assumptions: assumptions(project),
    }
}
