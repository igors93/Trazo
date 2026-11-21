//! Results as a traffic light (#101): every engineering check expressed as
//! a utilisation (demand over capacity), a green, yellow or red light, the
//! margin to the limit, a plain-language sentence on what to change and the
//! technical detail with its standard, plus the assumptions behind it.
//!
//! Green leaves a comfortable margin (utilisation up to 85 %), yellow passes
//! with a small one (85 to 100 %), red fails. Red is never hidden: the
//! summary counts it and every view lists it first.

use super::distribution::MAX_DROP_PERCENT;
use super::electrical::LoadSchedule;
use super::model::Project;
use super::structure::StructureReport;

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
}

impl Discipline {
    pub fn label(self) -> &'static str {
        match self {
            Self::Structure => "Estrutura",
            Self::Electrical => "Elétrica",
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

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

fn cm(metres: f64) -> String {
    format!("{:.0} cm", metres * 100.0)
}

/// The structural checks, from the pre-sizing report.
pub fn structure_verdicts(report: &StructureReport) -> Vec<Verdict> {
    let mut verdicts = Vec::new();
    for slab in &report.slabs {
        let mut checks = Vec::new();
        if slab.supported {
            checks.push(Check::new(
                "Espessura",
                slab.thickness_min_m / slab.thickness_m.max(1e-6),
                if slab.thickness_m + 1e-6 >= slab.thickness_min_m {
                    format!("A espessura de {} serve para o vão de {} m.", cm(slab.thickness_m), decimal(slab.lx_m, 2))
                } else {
                    format!("Aumente a laje para pelo menos {}.", cm(slab.thickness_min_m))
                },
                format!(
                    "Laje {} de {} para o menor vão lx = {} m: mínima {} (maciça: lx/40 ≥ 8 cm, NBR 6118 13.2.4.1; treliçada: tabela de fabricante).",
                    slab.kind.label().to_lowercase(),
                    cm(slab.thickness_m),
                    decimal(slab.lx_m, 2),
                    cm(slab.thickness_min_m)
                ),
            ));
        } else {
            checks.push(Check::fail(
                "Apoio",
                "Nenhuma viga ao longo das bordas: ligue os pilares com vigas sob a laje."
                    .to_owned(),
                "A carga da laje não tem para onde ir: sem viga ao longo de nenhuma borda."
                    .to_owned(),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Laje {} ({})", slab.label, slab.room),
            checks,
        });
    }
    for line in &report.beams {
        for (i, span) in line.spans.iter().enumerate() {
            let name = if line.spans.len() > 1 {
                format!("Viga {} · vão {}", line.label, i + 1)
            } else {
                format!("Viga {}", line.label)
            };
            let checks = vec![
                Check::new(
                    "Altura",
                    span.height_min_m / span.height_m.max(1e-6),
                    if span.height_m + 1e-6 >= span.height_min_m {
                        format!(
                            "A altura de {} serve para o vão de {} m.",
                            cm(span.height_m),
                            decimal(span.length_m, 2)
                        )
                    } else {
                        format!(
                            "Aumente a viga para pelo menos {} de altura.",
                            cm(span.height_min_m)
                        )
                    },
                    format!(
                        "h = {} para L = {} m; mínima {} (L/10 em vão de extremidade ou isolado, L/12 em vão interno).",
                        cm(span.height_m),
                        decimal(span.length_m, 2),
                        cm(span.height_min_m)
                    ),
                ),
                Check::new(
                    "Flecha",
                    span.deflection_mm / span.limit_mm.max(1e-9),
                    if span.deflection_mm <= span.limit_mm {
                        format!(
                            "A viga deforma {} mm, dentro do limite de {} mm.",
                            decimal(span.deflection_mm, 1),
                            decimal(span.limit_mm, 1)
                        )
                    } else {
                        "A viga vai deformar demais: aumente a altura ou diminua o vão.".to_owned()
                    },
                    format!(
                        "Flecha diferida {} mm ≤ L/250 = {} mm (carga quase permanente, inércia de Branson, 1 + αf = 2,32; NBR 6118 13.3 e 17.3.2.1).",
                        decimal(span.deflection_mm, 1),
                        decimal(span.limit_mm, 1)
                    ),
                ),
                Check::new(
                    "Largura",
                    0.12 / span.width_m.max(1e-6),
                    if span.width_m >= 0.12 - 1e-9 {
                        format!("A largura de {} atende ao mínimo.", cm(span.width_m))
                    } else {
                        "Alargue a viga para pelo menos 12 cm.".to_owned()
                    },
                    format!("b = {} ≥ 12 cm (NBR 6118 13.2.2).", cm(span.width_m)),
                ),
            ];
            verdicts.push(Verdict {
                discipline: Discipline::Structure,
                element: name,
                checks,
            });
        }
    }
    for column in &report.columns {
        let needed = column.area_needed_cm2.max(360.0);
        let side = column.width_m.min(column.depth_m);
        let mut checks = vec![
            Check::new(
                "Seção",
                needed / column.area_cm2.max(1e-6),
                if column.area_cm2 + 1e-6 >= needed {
                    format!(
                        "A seção de {:.0} cm² aguenta a carga de {:.0} kN.",
                        column.area_cm2, column.load_kn
                    )
                } else {
                    format!("Aumente o pilar para pelo menos {needed:.0} cm² de seção.")
                },
                format!(
                    "Ac = 1,4·α·N / (0,85 fcd + 1,5 %·420 MPa) = {:.0} cm² (α = {} para pilar {}), mínimo 360 cm² (NBR 6118 13.2.3); N = {:.0} kN.",
                    column.area_needed_cm2,
                    decimal(column.position.factor(), 1),
                    column.position.label().to_lowercase(),
                    column.load_kn
                ),
            ),
            Check::new(
                "Menor lado",
                0.14 / side.max(1e-6),
                if side >= 0.14 - 1e-9 {
                    format!("O menor lado de {} atende ao mínimo.", cm(side))
                } else {
                    "O pilar precisa ter pelo menos 14 cm em cada lado.".to_owned()
                },
                format!(
                    "Menor dimensão {} ≥ 14 cm (NBR 6118 13.2.3, com coeficiente adicional γn).",
                    cm(side)
                ),
            ),
        ];
        if column.position == super::structure::ColumnPosition::Isolated {
            checks.push(Check::fail(
                "Vigas",
                "Nenhuma viga chega a este pilar: ligue-o às vigas ou remova-o.".to_owned(),
                "Pilar sem vigas: a carga calculada é só o peso próprio.".to_owned(),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Pilar {}", column.label),
            checks,
        });
    }
    for footing in &report.footings {
        let mut checks = Vec::new();
        if footing.column.is_none() {
            checks.push(Check::fail(
                "Pilar",
                "Esta sapata não tem pilar em cima: mova-a para baixo de um pilar.".to_owned(),
                "Sapata sem pilar: não há carga para dimensionar.".to_owned(),
            ));
        } else {
            let area = footing.width_m * footing.depth_m;
            let needed = footing.needed_width_m * footing.needed_depth_m;
            checks.push(Check::new(
                "Área",
                needed / area.max(1e-9),
                if area + 1e-6 >= needed {
                    format!(
                        "A sapata de {} × {} distribui bem a carga no solo.",
                        cm(footing.width_m),
                        cm(footing.depth_m)
                    )
                } else {
                    format!(
                        "Aumente a sapata para {} × {} (use Ajustar sapatas).",
                        cm(footing.needed_width_m),
                        cm(footing.needed_depth_m)
                    )
                },
                format!(
                    "A = 1,05·N / σadm = 1,05 × {:.0} kN / {:.0} kPa = {} m² (tem {} m²).",
                    footing.load_kn,
                    footing.soil_kpa,
                    decimal(needed, 2),
                    decimal(area, 2)
                ),
            ));
            checks.push(Check::new(
                "Pressão no solo",
                footing.max_pressure_kpa / footing.soil_kpa.max(1e-9),
                if footing.max_pressure_kpa <= footing.soil_kpa + 1e-3 {
                    "A pressão no solo fica dentro do admissível.".to_owned()
                } else {
                    "O solo fica sobrecarregado: centralize a sapata sob o pilar ou aumente-a.".to_owned()
                },
                format!(
                    "σmax = {:.0} kPa ≤ σadm = {:.0} kPa, excentricidade {} (regra do terço médio, NBR 6122).",
                    footing.max_pressure_kpa,
                    footing.soil_kpa,
                    cm(footing.eccentricity_m)
                ),
            ));
            checks.push(Check::new(
                "Punção",
                footing.punching_ratio,
                if footing.punching_ratio <= 1.0 {
                    "A sapata resiste à punção do pilar.".to_owned()
                } else {
                    "O pilar pode furar a sapata: aumente a altura dela.".to_owned()
                },
                format!(
                    "τSd/τRd = {} na face do pilar e a d/2 (NBR 6118 19.5); altura {}.",
                    decimal(footing.punching_ratio, 2),
                    cm(footing.height_m)
                ),
            ));
            if footing.overturning_fs.is_finite() {
                checks.push(Check::new(
                    "Tombamento",
                    1.5 / footing.overturning_fs.max(1e-9),
                    if footing.overturning_fs >= 1.5 {
                        "Não há risco de tombamento.".to_owned()
                    } else {
                        "Risco de tombar: centralize a sapata ou use uma viga de equilíbrio."
                            .to_owned()
                    },
                    format!(
                        "FS = {} ≥ 1,5 (NBR 6122).",
                        decimal(footing.overturning_fs, 2)
                    ),
                ));
            }
        }
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element: format!("Sapata {}", footing.label),
            checks,
        });
    }
    for misaligned in &report.misaligned_columns {
        let element = format!(
            "Pilar {} ({})",
            misaligned.column_label, misaligned.floor_name
        );
        let check = match &misaligned.transfer_beam {
            Some(beam) => Check::new(
                "Transição",
                1.0,
                format!("O pilar nasce sobre a viga {beam}: um engenheiro precisa detalhar essa viga de transição."),
                format!("Carga de {:.0} kN aplicada como carga concentrada em {beam}.", misaligned.load_kn),
            ),
            None => Check::fail(
                "Prumada",
                "Não há pilar nem viga embaixo deste pilar: alinhe-o com um pilar do andar de baixo.".to_owned(),
                format!("Carga de {:.0} kN sem apoio no pavimento inferior.", misaligned.load_kn),
            ),
        };
        verdicts.push(Verdict {
            discipline: Discipline::Structure,
            element,
            checks: vec![check],
        });
    }
    verdicts
}

/// The electrical checks, from the load schedule.
pub fn electrical_verdicts(schedule: &LoadSchedule) -> Vec<Verdict> {
    let mut verdicts = Vec::new();
    for row in &schedule.circuits {
        let mut checks = Vec::new();
        match (row.breaker_a, row.wire_mm2) {
            (Some(breaker), Some(wire)) => {
                checks.push(Check::new(
                    "Corrente",
                    row.current_a / breaker,
                    if row.current_a <= breaker {
                        format!("O disjuntor de {breaker:.0} A protege os {} A do circuito.", decimal(row.current_a, 1))
                    } else {
                        "O circuito passa do disjuntor: divida as cargas.".to_owned()
                    },
                    format!(
                        "I = {} A ≤ In = {breaker:.0} A ≤ Iz do fio de {} mm² (NBR 5410 5.3.4 e tabela 36).",
                        decimal(row.current_a, 1),
                        decimal(wire, 1)
                    ),
                ));
            }
            _ => checks.push(Check::fail(
                "Corrente",
                "A corrente é alta demais para um circuito: divida as cargas.".to_owned(),
                format!(
                    "{} A não cabe nos disjuntores e fios da tabela.",
                    decimal(row.current_a, 1)
                ),
            )),
        }
        if let Some(drop) = row.drop_percent {
            checks.push(Check::new(
                "Queda de tensão",
                drop / MAX_DROP_PERCENT,
                if drop <= MAX_DROP_PERCENT {
                    format!("A queda de tensão de {} % está dentro dos 4 %.", decimal(drop, 1))
                } else {
                    "A tensão cai demais no fim do circuito: aproxime o quadro ou divida o circuito.".to_owned()
                },
                format!(
                    "ΔV = {} % ≤ 4 % (NBR 5410 6.2.7.1), ρ = 1/56 Ω·mm²/m, {} m até o ponto mais longe.",
                    decimal(drop, 2),
                    decimal(row.length_m.unwrap_or(0.0), 1)
                ),
            ));
        }
        verdicts.push(Verdict {
            discipline: Discipline::Electrical,
            element: format!("Circuito C{} ({})", row.number, row.kind.short()),
            checks,
        });
    }
    verdicts
}

/// The assumptions every structural and electrical result rests on.
pub fn assumptions(project: &Project) -> Vec<String> {
    let s = project.structure.settings;
    vec![
        "Pré-dimensionamento, não projeto final: construir exige um profissional habilitado (ART ou RRT) e o alvará da prefeitura.".to_owned(),
        format!(
            "Concreto fck {:.0} MPa, aço CA-50, solo com tensão admissível de {:.0} kPa.",
            s.fck_mpa, s.soil_kpa
        ),
        format!(
            "Cargas: sobrecarga {} kN/m², revestimento {} kN/m², alvenaria {} kN/m³ (NBR 6120).",
            decimal(s.live_kn_m2, 1),
            decimal(s.finish_kn_m2, 1),
            decimal(s.masonry_kn_m3, 0)
        ),
        "Vigas contínuas sobre apoios simples (pilares), sem vento nem sismo.".to_owned(),
        "Pilares por carga axial com fator de posição; sem flambagem nem flexão composta.".to_owned(),
        "Fios de cobre com isolação de PVC, método B1, 30 °C; queda de tensão até 4 %.".to_owned(),
        "Métodos, normas e limites: docs/engineering.md.".to_owned(),
    ]
}

/// Every verdict of the project, worst first.
pub fn safety_summary(project: &Project) -> SafetySummary {
    let mut verdicts = structure_verdicts(&project.structure_report());
    verdicts.extend(electrical_verdicts(&project.load_schedule()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    fn house() -> Project {
        let mut project = Project::default();
        project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
            .unwrap();
        project
            .add_room(Point::new(4.0, 0.0), Point::new(7.0, 4.0), "Cozinha")
            .unwrap();
        project.generate_structure();
        project
    }

    #[test]
    fn lights_follow_the_utilisation_thresholds() {
        assert_eq!(Light::from_utilisation(0.5), Light::Green);
        assert_eq!(Light::from_utilisation(0.85), Light::Green);
        assert_eq!(Light::from_utilisation(0.9), Light::Yellow);
        assert_eq!(Light::from_utilisation(1.0), Light::Yellow);
        assert_eq!(Light::from_utilisation(1.01), Light::Red);
        assert_eq!(Light::from_utilisation(f64::NAN), Light::Red);
        let check = Check::new("x", 0.6, String::new(), String::new());
        assert!((check.margin_percent() - 40.0).abs() < 1e-9);
    }

    #[test]
    fn a_generated_house_has_no_red_and_every_element_is_judged() {
        let project = house();
        let summary = safety_summary(&project);
        let report = project.structure_report();
        let elements = report.slabs.len()
            + report.beams.iter().map(|l| l.spans.len()).sum::<usize>()
            + report.columns.len()
            + report.footings.len();
        assert_eq!(summary.verdicts.len(), elements);
        assert_eq!(summary.red, 0, "{:?}", summary.verdicts);
        assert_eq!(summary.green + summary.yellow, elements);
        assert!(summary.verdicts.iter().all(|v| !v.checks.is_empty()));
        assert!(!summary.assumptions.is_empty());
    }

    #[test]
    fn a_shallow_beam_turns_red_with_what_to_change_and_comes_first() {
        let mut project = house();
        let beam = project.structure.beams[0].id;
        project.update_beam(beam, None, Some(0.20)).unwrap();
        let summary = safety_summary(&project);
        assert!(summary.red >= 1);
        assert_eq!(summary.worst(), Some(Light::Red));
        let first = &summary.verdicts[0];
        assert_eq!(first.light(), Light::Red);
        let height = first.checks.iter().find(|c| c.name == "Altura").unwrap();
        assert_eq!(height.light, Light::Red);
        assert!(height.margin_percent() < 0.0);
        assert!(
            height.simple.contains("Aumente a viga"),
            "{}",
            height.simple
        );
        assert!(height.detail.contains("L/10") || height.detail.contains("L/12"));
    }

    #[test]
    fn electrical_circuits_get_current_and_voltage_drop_checks() {
        let mut project = Project::default();
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(60.0, 10.0));
        let (w, d) = crate::domain::fixtures::FixtureKind::Panel.default_size();
        project
            .add_fixture(
                crate::domain::fixtures::FixtureKind::Panel,
                Point::new(0.0, 0.0),
                w,
                d,
                0,
            )
            .unwrap();
        let (w, d) = crate::domain::fixtures::FixtureKind::OutletSpecific.default_size();
        let far = project
            .add_fixture(
                crate::domain::fixtures::FixtureKind::OutletSpecific,
                Point::new(50.0, 0.0),
                w,
                d,
                0,
            )
            .unwrap();
        project
            .update_electrical(far, Some(Some(1)), Some(4400.0), None)
            .unwrap();
        let verdicts = electrical_verdicts(&project.load_schedule());
        assert_eq!(verdicts.len(), 1);
        let names: Vec<&str> = verdicts[0].checks.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Corrente", "Queda de tensão"]);
        // The wire was raised to keep the drop within 4 %: not red.
        assert!(verdicts[0].light() != Light::Red);
    }
}
