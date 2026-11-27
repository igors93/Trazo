//! Site progress log (#139): records site photos, diary notes, actual
//! completion percentages and expenses per construction stage inside the
//! `.trazo` file, compares physical and financial execution against the
//! planned schedule (#137) and cost estimate (#87), and exports CSV / A3 SVG.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::domain::{
    construction::{
        cost::{build_cost_estimate, format_brl},
        schedule::build_construction_schedule,
        shopping::ConstructionStage,
    },
    model::Project,
};

/// One persisted site diary entry (#139).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgressEntry {
    pub id: u64,
    /// ISO date string (`YYYY-MM-DD`).
    pub date: String,
    pub stage: ConstructionStage,
    /// Cumulative physical completion of `stage` reported on `date` (0–100 %).
    pub reported_pct: f64,
    /// Plain-language site note (concrete batch, weather, deliveries, occurrences).
    pub note: String,
    /// Paths or references to site photos attached to this entry.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub photo_paths: Vec<String>,
    /// Optional actual expense recorded in this diary entry (R$).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_cost_brl: Option<f64>,
}

impl Default for ProgressEntry {
    fn default() -> Self {
        Self {
            id: 0,
            date: String::new(),
            stage: ConstructionStage::SiteAndSettingOut,
            reported_pct: 0.0,
            note: String::new(),
            photo_paths: Vec::new(),
            actual_cost_brl: None,
        }
    }
}

/// Execution status of one stage compared with the planned schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageProgressStatus {
    NotStarted,
    OnTrack,
    Ahead,
    Behind,
    Completed,
}

impl StageProgressStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotStarted => "Não iniciada",
            Self::OnTrack => "No prazo",
            Self::Ahead => "Adiantada",
            Self::Behind => "Atrasada",
            Self::Completed => "Concluída",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::NotStarted => "nao_iniciada",
            Self::OnTrack => "no_prazo",
            Self::Ahead => "adiantada",
            Self::Behind => "atrasada",
            Self::Completed => "concluida",
        }
    }
}

/// Planned-vs-actual comparison for one construction stage.
#[derive(Clone, Debug, PartialEq)]
pub struct StageProgressSummary {
    pub stage: ConstructionStage,
    pub planned_pct: f64,
    pub actual_pct: f64,
    pub variance_pct: f64,
    pub status: StageProgressStatus,
    pub budget_brl: f64,
    pub earned_budget_brl: f64,
    pub actual_cost_brl: f64,
    pub cost_variance_brl: f64,
    pub entry_count: usize,
    pub photo_count: usize,
    pub last_date: Option<String>,
    pub last_note: Option<String>,
}

/// Full site progress comparison report (#139).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgressLogReport {
    pub reference_workday: u32,
    pub total_planned_workdays: u32,
    pub overall_planned_pct: f64,
    pub overall_actual_pct: f64,
    pub overall_variance_pct: f64,
    pub overall_budget_brl: f64,
    pub overall_actual_cost_brl: f64,
    pub total_entries: usize,
    pub total_photos: usize,
    pub stages: Vec<StageProgressSummary>,
    pub entries: Vec<ProgressEntry>,
    pub plain_summary: String,
}

/// Adds a validated diary entry to `project.construction.progress_log` and returns its id.
pub fn add_progress_entry(
    project: &mut Project,
    date: String,
    stage: ConstructionStage,
    reported_pct: f64,
    note: String,
    photo_paths: Vec<String>,
    actual_cost_brl: Option<f64>,
) -> Result<u64, String> {
    let date = date.trim().to_owned();
    if date.is_empty() {
        return Err("Informe a data do registro (ex.: 2026-11-27).".to_owned());
    }
    if !reported_pct.is_finite() || !(0.0..=100.0).contains(&reported_pct) {
        return Err("O avanço físico deve ficar entre 0 e 100 %.".to_owned());
    }
    if let Some(cost) = actual_cost_brl
        && (!cost.is_finite() || cost < 0.0)
    {
        return Err("O gasto realizado deve ser um valor ≥ 0.".to_owned());
    }
    let clean_photos: Vec<String> = photo_paths
        .into_iter()
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    let next_id = project
        .construction
        .progress_log
        .iter()
        .map(|e| e.id)
        .max()
        .unwrap_or(0)
        + 1;
    project.construction.progress_log.push(ProgressEntry {
        id: next_id,
        date,
        stage,
        reported_pct,
        note: note.trim().to_owned(),
        photo_paths: clean_photos,
        actual_cost_brl,
    });
    Ok(next_id)
}

/// Removes one diary entry by id; returns `true` when found and removed.
pub fn remove_progress_entry(project: &mut Project, entry_id: u64) -> bool {
    let before = project.construction.progress_log.len();
    project
        .construction
        .progress_log
        .retain(|e| e.id != entry_id);
    project.construction.progress_log.len() < before
}

/// Compares persisted diary entries against the planned schedule (#137) and budget (#87).
pub fn build_progress_report(project: &Project) -> ProgressLogReport {
    let schedule = build_construction_schedule(project);
    let cost = build_cost_estimate(project);
    let entries = project.construction.progress_log.clone();

    let ref_day = project
        .construction
        .progress_reference_day
        .min(schedule.total_workdays);

    let mut stages = Vec::with_capacity(ConstructionStage::ALL.len());
    let mut weight_sum = 0.0;
    let mut weighted_planned = 0.0;
    let mut weighted_actual = 0.0;

    for st in ConstructionStage::ALL {
        let planned_pct = schedule.planned_stage_pct(st, ref_day);
        let stage_entries: Vec<&ProgressEntry> = entries.iter().filter(|e| e.stage == st).collect();
        let entry_count = stage_entries.len();
        let photo_count: usize = stage_entries.iter().map(|e| e.photo_paths.len()).sum();
        let latest = stage_entries.last().copied();
        let actual_pct = latest.map_or(0.0, |e| e.reported_pct.clamp(0.0, 100.0));
        let variance_pct = actual_pct - planned_pct;

        let status = if actual_pct >= 99.9 {
            StageProgressStatus::Completed
        } else if entry_count == 0 && planned_pct < 1.0 {
            StageProgressStatus::NotStarted
        } else if variance_pct >= 5.0 {
            StageProgressStatus::Ahead
        } else if variance_pct <= -5.0 {
            StageProgressStatus::Behind
        } else {
            StageProgressStatus::OnTrack
        };

        let budget_brl = cost
            .stages
            .iter()
            .find(|s| s.stage == st)
            .map_or(0.0, |s| s.total_brl);
        let earned_budget_brl = budget_brl * (actual_pct / 100.0);
        let actual_cost_brl: f64 = stage_entries.iter().filter_map(|e| e.actual_cost_brl).sum();
        let cost_variance_brl = actual_cost_brl - earned_budget_brl;

        // Weight overall progress by stage budget when available (fallback to equal weight).
        let w = if budget_brl > 1.0 { budget_brl } else { 1.0 };
        weight_sum += w;
        weighted_planned += planned_pct * w;
        weighted_actual += actual_pct * w;

        stages.push(StageProgressSummary {
            stage: st,
            planned_pct,
            actual_pct,
            variance_pct,
            status,
            budget_brl,
            earned_budget_brl,
            actual_cost_brl,
            cost_variance_brl,
            entry_count,
            photo_count,
            last_date: latest.map(|e| e.date.clone()),
            last_note: latest.map(|e| e.note.clone()).filter(|n| !n.is_empty()),
        });
    }

    let overall_planned_pct = if weight_sum > 1e-6 {
        weighted_planned / weight_sum
    } else {
        0.0
    };
    let overall_actual_pct = if weight_sum > 1e-6 {
        weighted_actual / weight_sum
    } else {
        0.0
    };
    let overall_variance_pct = overall_actual_pct - overall_planned_pct;
    let overall_budget_brl = cost.grand_total_brl;
    let overall_actual_cost_brl: f64 = entries.iter().filter_map(|e| e.actual_cost_brl).sum();
    let total_entries = entries.len();
    let total_photos: usize = entries.iter().map(|e| e.photo_paths.len()).sum();

    let plain_summary = format!(
        "Diário de obra (dia útil {}/{}): avanço real {:.1} % contra {:.1} % previsto ({:+.1} pp) · {} registro(s) e {} foto(s) · gasto anotado {} de {} orçado.",
        ref_day,
        schedule.total_workdays,
        overall_actual_pct,
        overall_planned_pct,
        overall_variance_pct,
        total_entries,
        total_photos,
        format_brl(overall_actual_cost_brl),
        format_brl(overall_budget_brl)
    );

    ProgressLogReport {
        reference_workday: ref_day,
        total_planned_workdays: schedule.total_workdays,
        overall_planned_pct,
        overall_actual_pct,
        overall_variance_pct,
        overall_budget_brl,
        overall_actual_cost_brl,
        total_entries,
        total_photos,
        stages,
        entries,
        plain_summary,
    }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// UTF-8 BOM CSV export of the planned-vs-actual stage comparison and site diary (#139).
pub fn progress_log_csv(project: &Project) -> String {
    let report = build_progress_report(project);
    let mut csv = String::from(
        "\u{feff}etapa,previsto_pct,realizado_pct,desvio_pp,situacao,orcado_brl,realizado_brl,fotos,registros\r\n",
    );
    for st in &report.stages {
        let _ = writeln!(
            csv,
            "\"{}\",{:.1},{:.1},{:+.1},{},{:.2},{:.2},{},{}\r",
            st.stage.label(),
            st.planned_pct,
            st.actual_pct,
            st.variance_pct,
            st.status.key(),
            st.budget_brl,
            st.actual_cost_brl,
            st.photo_count,
            st.entry_count
        );
    }
    csv.push_str("\r\nid,data,etapa,avanco_pct,gasto_brl,fotos,anotacao\r\n");
    for e in &report.entries {
        let _ = writeln!(
            csv,
            "{},{},\"{}\",{:.1},{},\"{}\",\"{}\"\r",
            e.id,
            e.date,
            e.stage.label(),
            e.reported_pct,
            e.actual_cost_brl
                .map_or(String::new(), |v| format!("{v:.2}")),
            e.photo_paths.join("; ").replace('"', "\"\""),
            e.note.replace('"', "\"\"")
        );
    }
    csv
}

/// Printable A3 landscape SVG sheet comparing planned vs. actual progress and listing diary entries (#139).
pub fn progress_log_svg(project: &Project) -> String {
    let report = build_progress_report(project);
    let width = 420.0;
    let height =
        (118.0 + (report.stages.len() + report.entries.len()) as f64 * 7.8 + 25.0).max(297.0);
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
        "DIÁRIO DE OBRA — AVANÇO REAL VS. CRONOGRAMA PREVISTO",
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

    let mut y = 40.0;
    text(
        &mut svg,
        18.0,
        y,
        3.1,
        true,
        ink,
        "1. Previsto × Realizado por etapa",
    );
    y += 6.5;
    let _ = writeln!(
        svg,
        "<rect x=\"16\" y=\"{:.1}\" width=\"388\" height=\"6.2\" fill=\"#edf2f7\" stroke=\"{ink}\" stroke-width=\"0.25\"/>",
        y - 4.2
    );
    for (title, x) in [
        ("Etapa", 18.0),
        ("Prev.", 135.0),
        ("Real", 158.0),
        ("Situação", 182.0),
        ("Barra (cinza=prev., azul=real)", 225.0),
        ("Gasto / Orç.", 318.0),
        ("Fotos", 386.0),
    ] {
        text(&mut svg, x, y, 2.3, true, ink, title);
    }
    y += 7.0;

    for st in &report.stages {
        let st_col = match st.status {
            StageProgressStatus::Completed | StageProgressStatus::Ahead => "#1b5e20",
            StageProgressStatus::Behind => "#b71c1c",
            StageProgressStatus::OnTrack => "#1565c0",
            StageProgressStatus::NotStarted => muted,
        };
        text(&mut svg, 18.0, y, 2.4, true, ink, st.stage.short_label());
        text(
            &mut svg,
            135.0,
            y,
            2.3,
            false,
            muted,
            &format!("{:.0}%", st.planned_pct),
        );
        text(
            &mut svg,
            158.0,
            y,
            2.4,
            true,
            ink,
            &format!("{:.0}%", st.actual_pct),
        );
        text(&mut svg, 182.0, y, 2.3, true, st_col, st.status.label());

        let bar_x = 225.0;
        let bar_w = 82.0;
        let _ = writeln!(
            svg,
            "<rect x=\"{bar_x:.1}\" y=\"{:.1}\" width=\"{bar_w:.1}\" height=\"4.4\" fill=\"#eef2f7\" stroke=\"#c8d3df\" stroke-width=\"0.15\"/>",
            y - 3.2
        );
        let pw = bar_w * (st.planned_pct / 100.0);
        let aw = bar_w * (st.actual_pct / 100.0);
        let _ = writeln!(
            svg,
            "<rect x=\"{bar_x:.1}\" y=\"{:.1}\" width=\"{pw:.2}\" height=\"2.0\" fill=\"#90a4ae\"/>",
            y - 3.0
        );
        let _ = writeln!(
            svg,
            "<rect x=\"{bar_x:.1}\" y=\"{:.1}\" width=\"{aw:.2}\" height=\"2.2\" fill=\"{st_col}\"/>",
            y - 1.0
        );
        text(
            &mut svg,
            318.0,
            y,
            2.2,
            false,
            ink,
            &format!(
                "{} / {}",
                format_brl(st.actual_cost_brl),
                format_brl(st.budget_brl)
            ),
        );
        text(
            &mut svg,
            386.0,
            y,
            2.3,
            false,
            muted,
            &st.photo_count.to_string(),
        );
        y += 7.2;
    }

    y += 7.0;
    text(
        &mut svg,
        18.0,
        y,
        3.1,
        true,
        ink,
        &format!(
            "2. Registros cronológicos do diário ({})",
            report.entries.len()
        ),
    );
    y += 6.5;

    if report.entries.is_empty() {
        text(
            &mut svg,
            18.0,
            y,
            2.5,
            false,
            muted,
            "Nenhum registro adicionado ainda. Registre fotos, medições e ocorrências na aba Diário de obra.",
        );
    } else {
        for e in &report.entries {
            let photos_txt = if e.photo_paths.is_empty() {
                "sem fotos".to_owned()
            } else {
                format!(
                    "{} foto(s): {}",
                    e.photo_paths.len(),
                    e.photo_paths.join(", ")
                )
            };
            text(
                &mut svg,
                18.0,
                y,
                2.4,
                true,
                ink,
                &format!(
                    "#{} · {} · {} ({:.0} %) · {}",
                    e.id,
                    e.date,
                    e.stage.short_label(),
                    e.reported_pct,
                    e.actual_cost_brl.map_or("—".to_owned(), format_brl)
                ),
            );
            text(
                &mut svg,
                18.0,
                y + 3.4,
                2.1,
                false,
                muted,
                &format!("{} · {photos_txt}", e.note),
            );
            y += 7.8;
        }
    }

    svg.push_str("</svg>\n");
    svg
}

impl Project {
    /// Planned-vs-actual progress report from the persisted site progress log (#139).
    pub fn progress_report(&self) -> ProgressLogReport {
        build_progress_report(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::samples::SampleProject;

    #[test]
    fn progress_log_persists_photos_notes_and_compares_with_schedule_and_budget() {
        let mut project = SampleProject::CasaTerrea.build();
        project.generate_structure();
        project.generate_electrical_points();
        project.assign_circuits();

        // Advance reference day to workday 10.
        project.construction.progress_reference_day = 10;

        let id1 = add_progress_entry(
            &mut project,
            "2026-11-20".to_owned(),
            ConstructionStage::SiteAndSettingOut,
            100.0,
            "Gabarito conferido com esquadro 3-4-5.".to_owned(),
            vec!["fotos/gabarito-01.jpg".to_owned()],
            Some(950.0),
        )
        .unwrap();
        let id2 = add_progress_entry(
            &mut project,
            "2026-11-27".to_owned(),
            ConstructionStage::Foundations,
            60.0,
            "Concretagem das sapatas S1 a S6, slump 10 cm.".to_owned(),
            vec![
                "fotos/sapatas-armacao.jpg".to_owned(),
                "fotos/sapatas-concreto.jpg".to_owned(),
            ],
            Some(8_500.0),
        )
        .unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        let report = project.progress_report();
        assert_eq!(report.total_entries, 2);
        assert_eq!(report.total_photos, 3);
        assert!(report.overall_actual_pct > 0.0);
        assert!((report.overall_actual_cost_brl - 9_450.0).abs() < 1e-6);
        let s1 = report
            .stages
            .iter()
            .find(|s| s.stage == ConstructionStage::SiteAndSettingOut)
            .unwrap();
        assert_eq!(s1.status, StageProgressStatus::Completed);
        assert_eq!(s1.photo_count, 1);

        // JSON round-trip preserves diary entries & reference day.
        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.construction.progress_log.len(), 2);
        assert_eq!(loaded.construction.progress_reference_day, 10);

        let csv = progress_log_csv(&project);
        assert!(csv.starts_with("\u{feff}etapa,previsto_pct"));
        assert!(csv.contains("gabarito-01.jpg"));
        let svg = progress_log_svg(&project);
        assert!(svg.contains("DIÁRIO DE OBRA"));
        assert!(svg.contains("</svg>"));

        assert!(remove_progress_entry(&mut project, id1));
        assert_eq!(project.construction.progress_log.len(), 1);
    }
}
