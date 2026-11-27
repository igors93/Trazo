//! Construction schedule & physical-financial timeline (#137):
//! builds a dependency-driven task graph (CPM forward/backward pass) from the
//! model's quantities and cost estimate (#87), sizing durations from SINAPI
//! crew productivities, enforcing NBR 14931 wet-curing (7 days) and slab/beam
//! shoring (21 days) waits, computing the weekly physical-financial S-curve
//! (*cronograma físico-financeiro*) and exporting CSV + A3 Gantt SVG.

use std::collections::HashMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::domain::{
    construction::{
        cost::{build_cost_estimate, format_brl},
        shopping::{ConstructionStage, build_stage_shopping_list},
    },
    model::Project,
    plumbing::points::plumbing_points,
};

fn default_workdays_per_week() -> u8 {
    5
}
fn default_masons() -> u32 {
    2
}
fn default_helpers() -> u32 {
    2
}
fn default_specialists() -> u32 {
    1
}

/// Persisted crew and calendar settings for the construction schedule (#137).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduleSettings {
    /// Working days per week (5 = seg–sex, 6 = seg–sáb).
    #[serde(default = "default_workdays_per_week")]
    pub workdays_per_week: u8,
    /// Number of masons (*pedreiros*).
    #[serde(default = "default_masons")]
    pub masons: u32,
    /// Number of helpers (*serventes*).
    #[serde(default = "default_helpers")]
    pub helpers: u32,
    /// Number of electricians.
    #[serde(default = "default_specialists")]
    pub electricians: u32,
    /// Number of plumbers.
    #[serde(default = "default_specialists")]
    pub plumbers: u32,
    /// Optional planned start date (`YYYY-MM-DD`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
}

impl Default for ScheduleSettings {
    fn default() -> Self {
        Self {
            workdays_per_week: default_workdays_per_week(),
            masons: default_masons(),
            helpers: default_helpers(),
            electricians: default_specialists(),
            plumbers: default_specialists(),
            start_date: None,
        }
    }
}

impl ScheduleSettings {
    pub fn is_default(&self) -> bool {
        self.workdays_per_week == default_workdays_per_week()
            && self.masons == default_masons()
            && self.helpers == default_helpers()
            && self.electricians == default_specialists()
            && self.plumbers == default_specialists()
            && self.start_date.is_none()
    }
}

/// Kind of scheduled activity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleTaskKind {
    /// Productive site work executed by a crew.
    Work,
    /// Mandatory wet curing period (NBR 14931 — 7 days min).
    Curing,
    /// Mandatory structural shoring period (NBR 14931 — 21 days min before full stripping).
    Shoring,
}

impl ScheduleTaskKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Work => "Execução",
            Self::Curing => "Cura obrigatória",
            Self::Shoring => "Escoramento (21 d)",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Work => "execucao",
            Self::Curing => "cura",
            Self::Shoring => "escoramento",
        }
    }
}

/// One scheduled task with CPM early/late bounds and cost allocation.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleTask {
    pub id: String,
    pub stage: ConstructionStage,
    pub title: String,
    pub kind: ScheduleTaskKind,
    pub crew_label: String,
    pub productivity_note: String,
    pub dependencies: Vec<String>,
    pub duration_days: u32,
    pub early_start: u32,
    pub early_finish: u32,
    pub late_start: u32,
    pub late_finish: u32,
    pub slack_days: u32,
    pub is_critical: bool,
    pub cost_brl: f64,
}

/// One weekly bucket of the physical-financial timeline (*cronograma físico-financeiro*).
#[derive(Clone, Debug, PartialEq)]
pub struct FinancialPeriod {
    pub week: u32,
    pub label: String,
    pub start_day: u32,
    pub end_day: u32,
    pub cost_brl: f64,
    pub cumulative_cost_brl: f64,
    pub physical_pct: f64,
    pub cumulative_pct: f64,
    pub active_stages: Vec<String>,
}

/// Complete construction schedule & physical-financial report (#137).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConstructionScheduleReport {
    pub settings: ScheduleSettings,
    pub tasks: Vec<ScheduleTask>,
    pub periods: Vec<FinancialPeriod>,
    pub total_workdays: u32,
    pub total_weeks: u32,
    pub total_months_est: f64,
    pub total_cost_brl: f64,
    pub critical_path_ids: Vec<String>,
    pub plain_summary: String,
    pub assumptions: Vec<String>,
}

impl ConstructionScheduleReport {
    /// Planned completion percentage (0..=100) for a given stage at `elapsed_workdays`.
    pub fn planned_stage_pct(&self, stage: ConstructionStage, elapsed_workdays: u32) -> f64 {
        let mut total_dur = 0u32;
        let mut done_dur = 0u32;
        for t in self.tasks.iter().filter(|t| t.stage == stage) {
            total_dur += t.duration_days;
            if elapsed_workdays >= t.early_finish {
                done_dur += t.duration_days;
            } else if elapsed_workdays > t.early_start {
                done_dur += elapsed_workdays - t.early_start;
            }
        }
        if total_dur == 0 {
            0.0
        } else {
            (100.0 * f64::from(done_dur) / f64::from(total_dur)).clamp(0.0, 100.0)
        }
    }
}

struct RawTask {
    id: &'static str,
    stage: ConstructionStage,
    title: String,
    kind: ScheduleTaskKind,
    crew_label: String,
    productivity_note: String,
    dependencies: Vec<&'static str>,
    duration_days: u32,
    cost_brl: f64,
}

/// Builds the full CPM construction schedule and weekly physical-financial timeline (#137).
pub fn build_construction_schedule(project: &Project) -> ConstructionScheduleReport {
    let mut settings = project.construction.schedule.clone();
    settings.workdays_per_week = settings.workdays_per_week.clamp(5, 6);
    settings.masons = settings.masons.clamp(1, 10);
    settings.helpers = settings.helpers.clamp(1, 12);
    settings.electricians = settings.electricians.clamp(1, 4);
    settings.plumbers = settings.plumbers.clamp(1, 4);

    let cost_report = build_cost_estimate(project);
    let shopping = build_stage_shopping_list(project);
    let structure = project.cached_structure_report();
    let all_floors = project.floors.levels.len() > 1;
    let takeoff = project.construction_takeoff_with_options(all_floors, None);

    let stage_cost = |st: ConstructionStage| -> f64 {
        cost_report
            .stages
            .iter()
            .find(|s| s.stage == st)
            .map_or(0.0, |s| s.total_brl)
    };

    // Effective masonry crew factor: each mason needs ~0.75 helper for full SINAPI output.
    let eff_masons = f64::from(settings.masons)
        .min((f64::from(settings.helpers) / 0.75).max(1.0))
        .max(1.0);
    let wpw = u32::from(settings.workdays_per_week);
    // Curing (7 calendar days) and shoring (21 calendar days) converted to workdays.
    let curing_workdays = ((7.0 * f64::from(wpw) / 7.0).ceil() as u32).max(5);
    let shoring_workdays = ((21.0 * f64::from(wpw) / 7.0).ceil() as u32).max(15);

    let mut raw: Vec<RawTask> = Vec::new();

    // T1: Site & setting out
    let perimeter_m: f64 = project.walls.iter().map(|w| w.length()).sum();
    if perimeter_m > 0.5 || !project.rooms.is_empty() {
        let dur = ((perimeter_m / (22.0 * eff_masons)).ceil() as u32).clamp(2, 6);
        raw.push(RawTask {
            id: "T1",
            stage: ConstructionStage::SiteAndSettingOut,
            title: "Limpeza do terreno e gabarito de locação".to_owned(),
            kind: ScheduleTaskKind::Work,
            crew_label: format!(
                "{} pedreiro(s) + {} servente(s)",
                settings.masons, settings.helpers
            ),
            productivity_note: "SINAPI locação ~22 m/dia por equipe".to_owned(),
            dependencies: vec![],
            duration_days: dur,
            cost_brl: stage_cost(ConstructionStage::SiteAndSettingOut),
        });
    }

    // T2: Foundations + T2C curing
    let found_vol =
        (shopping.summary.structural_concrete_m3 * 0.45 + shopping.summary.lean_concrete_m3).max(
            if !structure.footings.is_empty() || !structure.wall_foundations.is_empty() {
                1.5
            } else {
                0.0
            },
        );
    let has_foundations = found_vol > 0.1 || stage_cost(ConstructionStage::Foundations) > 1.0;
    if has_foundations {
        // Excavation + rebar + formwork + pouring: ~1.1 m³ concrete equivalent / mason-team / day.
        let dur = ((found_vol / (1.1 * eff_masons)).ceil() as u32).clamp(4, 25);
        raw.push(RawTask {
            id: "T2",
            stage: ConstructionStage::Foundations,
            title: "Escavação, armaduras, fôrmas e concretagem de fundações/baldrames".to_owned(),
            kind: ScheduleTaskKind::Work,
            crew_label: format!(
                "{} pedreiro(s) + {} servente(s)",
                settings.masons, settings.helpers
            ),
            productivity_note: "SINAPI fundações rasas ~1,1 m³/equipe·dia (escavação+aço+concreto)"
                .to_owned(),
            dependencies: if raw.iter().any(|t| t.id == "T1") {
                vec!["T1"]
            } else {
                vec![]
            },
            duration_days: dur,
            cost_brl: stage_cost(ConstructionStage::Foundations),
        });
        raw.push(RawTask {
            id: "T2C",
            stage: ConstructionStage::Foundations,
            title: "Cura úmida das fundações e impermeabilização dos baldrames (7 d)".to_owned(),
            kind: ScheduleTaskKind::Curing,
            crew_label: "Cura (NBR 14931)".to_owned(),
            productivity_note: "Mínimo 7 dias corridos de cura úmida antes de carregar a alvenaria"
                .to_owned(),
            dependencies: vec!["T2"],
            duration_days: curing_workdays,
            cost_brl: 0.0,
        });
    }

    // T4: Masonry (walls can rise right after foundation curing, before top slab/roof)
    let has_masonry = takeoff.net_wall_m2 > 1.0 || stage_cost(ConstructionStage::Masonry) > 1.0;
    if has_masonry {
        // SINAPI ceramic/concrete block laying ≈ 11 m²/day per mason (including lintels & bond beams).
        let dur = ((takeoff.net_wall_m2 / (11.0 * eff_masons)).ceil() as u32).clamp(3, 40);
        let deps = if has_foundations {
            vec!["T2C"]
        } else if raw.iter().any(|t| t.id == "T1") {
            vec!["T1"]
        } else {
            vec![]
        };
        raw.push(RawTask {
            id: "T4",
            stage: ConstructionStage::Masonry,
            title: "Alvenaria de elevação, vergas, contravergas e cinta de amarração".to_owned(),
            kind: ScheduleTaskKind::Work,
            crew_label: format!(
                "{} pedreiro(s) + {} servente(s)",
                settings.masons, settings.helpers
            ),
            productivity_note: "SINAPI alvenaria ~11 m²/pedreiro·dia (com vergas e canaletas)"
                .to_owned(),
            dependencies: deps,
            duration_days: dur,
            cost_brl: stage_cost(ConstructionStage::Masonry),
        });
    }

    // T3: Superstructure (columns, beams, slabs) + T3C curing (7d) + T3S shoring (21d)
    let has_structure = !structure.columns.is_empty()
        || !structure.beams.is_empty()
        || !structure.slabs.is_empty()
        || stage_cost(ConstructionStage::Structure) > 1.0;
    let has_slabs = !structure.slabs.is_empty();
    if has_structure {
        let super_vol = (shopping.summary.structural_concrete_m3 * 0.55).max(1.5);
        let dur = ((super_vol / (1.0 * eff_masons)).ceil() as u32).clamp(4, 30);
        let mut deps = Vec::new();
        if has_masonry {
            deps.push("T4");
        } else if has_foundations {
            deps.push("T2C");
        }
        raw.push(RawTask {
            id: "T3",
            stage: ConstructionStage::Structure,
            title: "Pilares, vigas superiores e concretagem de lajes".to_owned(),
            kind: ScheduleTaskKind::Work,
            crew_label: format!(
                "{} pedreiro(s) + {} servente(s)",
                settings.masons, settings.helpers
            ),
            productivity_note:
                "SINAPI superestrutura ~1,0 m³/equipe·dia (fôrma+aço+escoramento+concreto)"
                    .to_owned(),
            dependencies: deps,
            duration_days: dur,
            cost_brl: stage_cost(ConstructionStage::Structure),
        });
        raw.push(RawTask {
            id: "T3C",
            stage: ConstructionStage::Structure,
            title: "Cura úmida de pilares, vigas e lajes (7 dias obrigatórios)".to_owned(),
            kind: ScheduleTaskKind::Curing,
            crew_label: "Cura (NBR 14931)".to_owned(),
            productivity_note: "Molhar continuamente por ≥ 7 dias para evitar fissuras de retração"
                .to_owned(),
            dependencies: vec!["T3"],
            duration_days: curing_workdays,
            cost_brl: 0.0,
        });
        if has_slabs {
            raw.push(RawTask {
                id: "T3S",
                stage: ConstructionStage::Structure,
                title: "Permanência do escoramento de lajes e vigas (21 dias mín.)".to_owned(),
                kind: ScheduleTaskKind::Shoring,
                crew_label: "Escoramento (NBR 14931)".to_owned(),
                productivity_note:
                    "Manter escoras ≥ 21 dias (ou conforme engenheiro/fabricante da laje)"
                        .to_owned(),
                dependencies: vec!["T3"],
                duration_days: shoring_workdays,
                cost_brl: 0.0,
            });
        }
    }

    // T5: Roof
    let has_roof = !structure.roofs.is_empty() || stage_cost(ConstructionStage::Roof) > 1.0;
    if has_roof {
        let roof_m2: f64 = structure
            .roofs
            .iter()
            .map(|r| r.surface_m2)
            .sum::<f64>()
            .max(20.0);
        let dur = ((roof_m2 / (16.0 * eff_masons.min(2.0))).ceil() as u32).clamp(3, 20);
        let mut deps = Vec::new();
        if has_structure {
            deps.push("T3C");
        } else if has_masonry {
            deps.push("T4");
        }
        raw.push(RawTask {
            id: "T5",
            stage: ConstructionStage::Roof,
            title: "Estrutura de madeira, telhamento, calhas e condutores".to_owned(),
            kind: ScheduleTaskKind::Work,
            crew_label: "Carpinteiro + ajudante".to_owned(),
            productivity_note: "SINAPI cobertura ~16 m²/equipe·dia".to_owned(),
            dependencies: deps,
            duration_days: dur,
            cost_brl: stage_cost(ConstructionStage::Roof),
        });
    }

    // T6: Electrical & plumbing rough-in (Installations)
    let elec_pts = project.electrical_devices().count();
    let plumb_pts = plumbing_points(project).points.len();
    let has_installations =
        elec_pts > 0 || plumb_pts > 0 || stage_cost(ConstructionStage::Installations) > 1.0;
    if has_installations {
        let elec_days = (elec_pts as f64 / (6.0 * f64::from(settings.electricians))).ceil() as u32;
        let plumb_days = (plumb_pts as f64 / (4.0 * f64::from(settings.plumbers))).ceil() as u32;
        let dur = elec_days.max(plumb_days).clamp(3, 25);
        let mut deps = Vec::new();
        if has_structure {
            deps.push("T3C");
        } else if has_masonry {
            deps.push("T4");
        }
        raw.push(RawTask {
            id: "T6",
            stage: ConstructionStage::Installations,
            title: "Instalações elétricas (eletrodutos/cabos/QDG) e hidrossanitárias".to_owned(),
            kind: ScheduleTaskKind::Work,
            crew_label: format!(
                "{} eletricista(s) + {} encanador(es)",
                settings.electricians, settings.plumbers
            ),
            productivity_note:
                "SINAPI ~6 pontos elétricos/dia e ~4 pontos hidráulicos/dia (em paralelo)"
                    .to_owned(),
            dependencies: deps,
            duration_days: dur,
            cost_brl: stage_cost(ConstructionStage::Installations),
        });
    }

    // T7: Plaster & waterproofing
    let has_plaster =
        takeoff.plaster_m2 > 1.0 || stage_cost(ConstructionStage::PlasterAndWaterproofing) > 1.0;
    if has_plaster {
        // SINAPI roughcast + plaster ≈ 14 m²/mason/day.
        let dur = ((takeoff.plaster_m2 / (14.0 * eff_masons)).ceil() as u32).clamp(3, 35);
        let mut deps = Vec::new();
        if has_installations {
            deps.push("T6");
        }
        if has_roof {
            deps.push("T5");
        }
        if has_structure && has_slabs {
            deps.push("T3S");
        } else if deps.is_empty() && has_masonry {
            deps.push("T4");
        }
        raw.push(RawTask {
            id: "T7",
            stage: ConstructionStage::PlasterAndWaterproofing,
            title: "Chapisco, reboco interno/externo e impermeabilização de áreas molhadas"
                .to_owned(),
            kind: ScheduleTaskKind::Work,
            crew_label: format!(
                "{} pedreiro(s) + {} servente(s)",
                settings.masons, settings.helpers
            ),
            productivity_note: "SINAPI reboco ~14 m²/pedreiro·dia (após desforma das escoras)"
                .to_owned(),
            dependencies: deps,
            duration_days: dur,
            cost_brl: stage_cost(ConstructionStage::PlasterAndWaterproofing),
        });
    }

    // T8: Finishes & paint
    let has_finishes = takeoff.floor_m2 > 1.0 || stage_cost(ConstructionStage::Finishes) > 1.0;
    if has_finishes {
        let area_eq = takeoff.floor_m2 + (takeoff.room_walls_m2 + takeoff.ceiling_m2) * 0.35;
        let dur = ((area_eq / (13.0 * eff_masons)).ceil() as u32).clamp(4, 35);
        let mut deps = Vec::new();
        if has_plaster {
            deps.push("T7");
        } else if has_installations {
            deps.push("T6");
        } else if has_masonry {
            deps.push("T4");
        }
        raw.push(RawTask {
            id: "T8",
            stage: ConstructionStage::Finishes,
            title: "Assentamento de pisos, rodapés, pintura e acabamentos finais".to_owned(),
            kind: ScheduleTaskKind::Work,
            crew_label: format!(
                "{} pedreiro(s)/pintor + {} servente(s)",
                settings.masons, settings.helpers
            ),
            productivity_note: "SINAPI revestimento cerâmico ~12 m²/dia + pintura ~30 m²/dia"
                .to_owned(),
            dependencies: deps,
            duration_days: dur,
            cost_brl: stage_cost(ConstructionStage::Finishes),
        });
    }

    // CPM Forward Pass
    let mut early_start: HashMap<&'static str, u32> = HashMap::new();
    let mut early_finish: HashMap<&'static str, u32> = HashMap::new();
    for task in &raw {
        let es = task
            .dependencies
            .iter()
            .filter_map(|d| early_finish.get(d).copied())
            .max()
            .unwrap_or(0);
        let ef = es + task.duration_days;
        early_start.insert(task.id, es);
        early_finish.insert(task.id, ef);
    }
    let total_workdays = early_finish.values().copied().max().unwrap_or(0);

    // CPM Backward Pass
    let mut late_finish: HashMap<&'static str, u32> = HashMap::new();
    let mut late_start: HashMap<&'static str, u32> = HashMap::new();
    for task in raw.iter().rev() {
        let lf = raw
            .iter()
            .filter(|succ| succ.dependencies.contains(&task.id))
            .filter_map(|succ| late_start.get(succ.id).copied())
            .min()
            .unwrap_or(total_workdays);
        let ls = lf.saturating_sub(task.duration_days);
        late_finish.insert(task.id, lf);
        late_start.insert(task.id, ls);
    }

    let mut critical_path_ids = Vec::new();
    let tasks: Vec<ScheduleTask> = raw
        .into_iter()
        .map(|t| {
            let es = early_start.get(t.id).copied().unwrap_or(0);
            let ef = early_finish.get(t.id).copied().unwrap_or(t.duration_days);
            let ls = late_start.get(t.id).copied().unwrap_or(es);
            let lf = late_finish.get(t.id).copied().unwrap_or(ef);
            let slack = ls.saturating_sub(es);
            let is_critical = slack == 0;
            if is_critical {
                critical_path_ids.push(t.id.to_owned());
            }
            ScheduleTask {
                id: t.id.to_owned(),
                stage: t.stage,
                title: t.title,
                kind: t.kind,
                crew_label: t.crew_label,
                productivity_note: t.productivity_note,
                dependencies: t.dependencies.into_iter().map(str::to_owned).collect(),
                duration_days: t.duration_days,
                early_start: es,
                early_finish: ef,
                late_start: ls,
                late_finish: lf,
                slack_days: slack,
                is_critical,
                cost_brl: t.cost_brl,
            }
        })
        .collect();

    // Weekly physical-financial timeline (*cronograma físico-financeiro*)
    let total_weeks = if total_workdays == 0 {
        0
    } else {
        total_workdays.div_ceil(wpw)
    };
    let total_cost_brl: f64 = tasks.iter().map(|t| t.cost_brl).sum();
    let mut periods = Vec::with_capacity(total_weeks as usize);
    let mut cum_cost = 0.0;

    for w in 0..total_weeks {
        let start_day = w * wpw;
        let end_day = ((w + 1) * wpw).min(total_workdays);
        let mut week_cost = 0.0;
        let mut active = Vec::new();

        for task in &tasks {
            let ov_start = task.early_start.max(start_day);
            let ov_end = task.early_finish.min(end_day);
            if ov_end > ov_start && task.duration_days > 0 {
                let frac = f64::from(ov_end - ov_start) / f64::from(task.duration_days);
                week_cost += task.cost_brl * frac;
                let lbl = task.stage.short_label().to_owned();
                if !active.contains(&lbl) {
                    active.push(lbl);
                }
            }
        }
        cum_cost += week_cost;
        let phys_pct = if total_cost_brl > 1e-6 {
            100.0 * week_cost / total_cost_brl
        } else {
            0.0
        };
        let cum_pct = if total_cost_brl > 1e-6 {
            (100.0 * cum_cost / total_cost_brl).min(100.0)
        } else {
            0.0
        };
        periods.push(FinancialPeriod {
            week: w + 1,
            label: format!("Sem {}", w + 1),
            start_day,
            end_day,
            cost_brl: week_cost,
            cumulative_cost_brl: cum_cost,
            physical_pct: phys_pct,
            cumulative_pct: cum_pct,
            active_stages: active,
        });
    }

    let total_months_est = f64::from(total_weeks) / 4.33;
    let plain_summary = format!(
        "Prazo estimado: {} dias úteis ({} semanas, ~{:.1} meses com {} dias/sem, {} pedreiro(s) e {} servente(s)) · desembolso total {}.",
        total_workdays,
        total_weeks,
        total_months_est,
        settings.workdays_per_week,
        settings.masons,
        settings.helpers,
        format_brl(total_cost_brl)
    );

    ConstructionScheduleReport {
        settings,
        tasks,
        periods,
        total_workdays,
        total_weeks,
        total_months_est,
        total_cost_brl,
        critical_path_ids,
        plain_summary,
        assumptions: vec![
            "Produtividades baseadas em referências SINAPI para obra residencial de pequeno porte.".to_owned(),
            "Inclui tempos técnicos obrigatórios da ABNT NBR 14931: cura úmida ≥ 7 dias e escoramento de lajes/vigas ≥ 21 dias.".to_owned(),
            "Chuvas intensas, atrasos de entrega ou falta de equipe aumentam o prazo real; reserve margem de 15 a 20 %.".to_owned(),
        ],
    }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// UTF-8 BOM CSV export of the CPM task schedule and weekly physical-financial timeline (#137).
pub fn construction_schedule_csv(project: &Project) -> String {
    let report = build_construction_schedule(project);
    let mut csv = String::from(
        "\u{feff}id,etapa,atividade,tipo,equipe,dependencias,duracao_dias,inicio_dia,fim_dia,folga_dias,critica,custo_brl\r\n",
    );
    for t in &report.tasks {
        let _ = writeln!(
            csv,
            "{},\"{}\",\"{}\",{},\"{}\",\"{}\",{},{},{},{},{},{:.2}\r",
            t.id,
            t.stage.label(),
            t.title.replace('"', "\"\""),
            t.kind.key(),
            t.crew_label.replace('"', "\"\""),
            t.dependencies.join(";"),
            t.duration_days,
            t.early_start + 1,
            t.early_finish,
            t.slack_days,
            if t.is_critical { "sim" } else { "nao" },
            t.cost_brl
        );
    }
    csv.push_str("\r\nsemana,periodo,desembolso_brl,acumulado_brl,fisico_pct,acumulado_pct,etapas_ativas\r\n");
    for p in &report.periods {
        let _ = writeln!(
            csv,
            "{},{},{:.2},{:.2},{:.1},{:.1},\"{}\"\r",
            p.week,
            p.label,
            p.cost_brl,
            p.cumulative_cost_brl,
            p.physical_pct,
            p.cumulative_pct,
            p.active_stages.join(" / ")
        );
    }
    csv
}

/// Printable A3 landscape vector Gantt chart + physical-financial S-curve (#137).
pub fn construction_schedule_gantt_svg(project: &Project) -> String {
    let report = build_construction_schedule(project);
    let width = 420.0;
    let gantt_top = 46.0;
    let row_h = 10.5;
    let gantt_h = (report.tasks.len().max(1) as f64) * row_h + 14.0;
    let curve_top = gantt_top + gantt_h + 14.0;
    let curve_h = 58.0;
    let height = (curve_top + curve_h + 32.0).max(297.0);

    let ink = "#1d2836";
    let muted = "#4a5d73";
    let font = "font-family=\"Arial, sans-serif\"";

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height:.0}mm\" viewBox=\"0 0 {width} {height:.0}\">\n\
         <rect width=\"{width}\" height=\"{height:.0}\" fill=\"#ffffff\"/>\n\
         <rect x=\"10\" y=\"10\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"{ink}\" stroke-width=\"0.5\"/>\n",
        width - 20.0,
        height - 20.0
    );
    let text = |svg: &mut String,
                x: f64,
                y: f64,
                size: f64,
                bold: bool,
                color: &str,
                value: &str| {
        let _ = writeln!(
            svg,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" {font} font-size=\"{size}\"{} fill=\"{color}\">{}</text>",
            if bold { " font-weight=\"700\"" } else { "" },
            esc(value)
        );
    };

    text(
        &mut svg,
        18.0,
        22.0,
        5.0,
        true,
        ink,
        "CRONOGRAMA FÍSICO-FINANCEIRO E CAMINHO CRÍTICO (GANTT)",
    );
    text(
        &mut svg,
        18.0,
        28.5,
        2.9,
        false,
        muted,
        &format!("{} · {}", project.name, report.plain_summary),
    );
    text(
        &mut svg,
        18.0,
        34.0,
        2.4,
        false,
        muted,
        "Legenda: Vermelho = caminho crítico (folga 0) · Azul = execução com folga · Âmbar = cura (7 d) / escoramento obrigatório (21 d, NBR 14931)",
    );

    // Gantt chart grid
    let chart_x = 165.0;
    let chart_w = 235.0;
    let total_days = f64::from(report.total_workdays.max(1));
    let wpw = f64::from(report.settings.workdays_per_week.max(5));

    // Weekly vertical grid lines
    let weeks = report.total_weeks.max(1);
    for w in 0..=weeks {
        let day = (f64::from(w) * wpw).min(total_days);
        let x = chart_x + (day / total_days) * chart_w;
        let _ = writeln!(
            svg,
            "<line x1=\"{x:.2}\" y1=\"{:.1}\" x2=\"{x:.2}\" y2=\"{:.1}\" stroke=\"#dce4ec\" stroke-width=\"0.25\"/>",
            gantt_top - 4.0,
            gantt_top + (report.tasks.len() as f64) * row_h
        );
        if w < weeks && (weeks <= 18 || w % 2 == 0) {
            text(
                &mut svg,
                x + 1.0,
                gantt_top - 6.0,
                2.1,
                false,
                muted,
                &format!("S{}", w + 1),
            );
        }
    }

    text(
        &mut svg,
        18.0,
        gantt_top - 6.0,
        2.5,
        true,
        ink,
        "Atividade / Equipe",
    );
    text(&mut svg, 136.0, gantt_top - 6.0, 2.5, true, ink, "Dias");

    for (idx, task) in report.tasks.iter().enumerate() {
        let y = gantt_top + (idx as f64) * row_h;
        text(
            &mut svg,
            18.0,
            y + 4.0,
            2.4,
            task.is_critical,
            if task.is_critical { "#b71c1c" } else { ink },
            &format!("{} · {}", task.id, task.title),
        );
        text(
            &mut svg,
            18.0,
            y + 7.6,
            2.0,
            false,
            muted,
            &format!("{} · {}", task.crew_label, format_brl(task.cost_brl)),
        );
        text(
            &mut svg,
            136.0,
            y + 4.5,
            2.4,
            true,
            ink,
            &format!("{} d", task.duration_days),
        );

        let bx = chart_x + (f64::from(task.early_start) / total_days) * chart_w;
        let bw = ((f64::from(task.duration_days) / total_days) * chart_w).max(2.0);
        let fill = match task.kind {
            ScheduleTaskKind::Curing | ScheduleTaskKind::Shoring => "#e69f00",
            ScheduleTaskKind::Work if task.is_critical => "#c62828",
            ScheduleTaskKind::Work => "#1976d2",
        };
        let _ = writeln!(
            svg,
            "<rect x=\"{bx:.2}\" y=\"{:.2}\" width=\"{bw:.2}\" height=\"5.4\" rx=\"1\" fill=\"{fill}\"/>",
            y + 1.2
        );
        let _ = writeln!(
            svg,
            "<line x1=\"16\" y1=\"{:.1}\" x2=\"404\" y2=\"{:.1}\" stroke=\"#edf2f7\" stroke-width=\"0.2\"/>",
            y + row_h - 0.5,
            y + row_h - 0.5
        );
    }

    // Physical-Financial S-curve panel
    text(
        &mut svg,
        18.0,
        curve_top - 4.0,
        3.1,
        true,
        ink,
        "Curva S físico-financeira (desembolso semanal e % acumulado)",
    );
    let plot_x = 30.0;
    let plot_w = 370.0;
    let plot_y = curve_top + 4.0;
    let plot_h = curve_h - 14.0;
    let _ = writeln!(
        svg,
        "<rect x=\"{plot_x:.1}\" y=\"{plot_y:.1}\" width=\"{plot_w:.1}\" height=\"{plot_h:.1}\" fill=\"#fafcfe\" stroke=\"#b8c7d9\" stroke-width=\"0.3\"/>"
    );

    if !report.periods.is_empty() {
        let max_week_cost = report
            .periods
            .iter()
            .map(|p| p.cost_brl)
            .fold(1.0_f64, f64::max);
        let step_w = plot_w / (report.periods.len() as f64);
        let mut s_points = String::new();

        for (i, p) in report.periods.iter().enumerate() {
            let x = plot_x + (i as f64) * step_w;
            let bar_h = (p.cost_brl / max_week_cost) * (plot_h * 0.72);
            let _ = writeln!(
                svg,
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{bar_h:.2}\" fill=\"#90caf9\" stroke=\"#1565c0\" stroke-width=\"0.15\"/>",
                x + step_w * 0.12,
                plot_y + plot_h - bar_h,
                (step_w * 0.76).max(1.2)
            );
            let px = x + step_w * 0.5;
            let py = plot_y + plot_h - (p.cumulative_pct / 100.0) * plot_h;
            let _ = write!(s_points, "{px:.2},{py:.2} ");
            if report.periods.len() <= 20 || i % 2 == 0 {
                text(
                    &mut svg,
                    x + 1.0,
                    plot_y + plot_h + 4.5,
                    2.0,
                    false,
                    muted,
                    &format!("S{}", p.week),
                );
            }
        }
        let _ = writeln!(
            svg,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"#c62828\" stroke-width=\"0.9\"/>",
            s_points.trim()
        );
    }

    text(&mut svg, 18.0, plot_y + 3.0, 2.0, false, muted, "100%");
    text(&mut svg, 18.0, plot_y + plot_h, 2.0, false, muted, "0%");

    svg.push_str("</svg>\n");
    svg
}

impl Project {
    /// Complete CPM construction schedule and physical-financial timeline (#137).
    pub fn construction_schedule(&self) -> ConstructionScheduleReport {
        build_construction_schedule(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::samples::SampleProject;

    #[test]
    fn sample_house_schedule_enforces_curing_shoring_and_balances_financial_curve() {
        let mut project = SampleProject::CasaTerrea.build();
        project.generate_structure();
        project.generate_electrical_points();
        project.assign_circuits();

        let sched = project.construction_schedule();
        assert!(sched.total_workdays >= 30);
        assert!(sched.total_weeks >= 6);
        assert!(!sched.critical_path_ids.is_empty());

        // Mandatory NBR 14931 curing (7d -> ≥5 workdays) and shoring (21d -> ≥15 workdays).
        let curing = sched
            .tasks
            .iter()
            .find(|t| t.kind == ScheduleTaskKind::Curing)
            .expect("curing task present");
        assert!(curing.duration_days >= 5);
        let shoring = sched
            .tasks
            .iter()
            .find(|t| t.kind == ScheduleTaskKind::Shoring)
            .expect("shoring task present");
        assert!(shoring.duration_days >= 15);

        // Weekly financial disbursements reconcile to total cost & 100%.
        let last = sched.periods.last().expect("at least one week");
        assert!((last.cumulative_pct - 100.0).abs() < 0.2);
        assert!((last.cumulative_cost_brl - sched.total_cost_brl).abs() < 1.0);

        // Doubling crew size shortens work duration (while keeping curing/shoring fixed).
        project.construction.schedule.masons = 4;
        project.construction.schedule.helpers = 4;
        let faster = project.construction_schedule();
        assert!(faster.total_workdays < sched.total_workdays);

        let csv = construction_schedule_csv(&project);
        assert!(csv.starts_with("\u{feff}id,etapa"));
        let svg = construction_schedule_gantt_svg(&project);
        assert!(svg.contains("CRONOGRAMA FÍSICO-FINANCEIRO"));
        assert!(svg.contains("</svg>"));
    }
}
