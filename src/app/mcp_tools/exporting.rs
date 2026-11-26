//! The export tool: plans, sheets, reports and schedules written to PDF,
//! PNG, SVG, DXF and CSV.

use serde_json::{Value, json};

use super::{documents_dir, ensure_parent, file_stem, resolve_path, text, unique_path};
use crate::app::FloorPlanApp;
use crate::{io::export, mcp::ToolOutput};

impl FloorPlanApp {
    pub(super) fn tool_export(&self, args: &Value) -> Result<ToolOutput, String> {
        let format = text(args, "formato")
            .map(|format| format.to_lowercase())
            .ok_or("Informe 'formato': pdf, pdf_abas, png, dxf, svg, prancha_pdf, prancha_pdf_abas, prancha_svg, memorial_pdf, caderno_familia_pdf, conjunto_obra_pdf, primeira_fiada_svg, paginacao_pdf, locacao_svg, locacao_pdf, cobertura_svg, cobertura_pdf, unifilar_svg, unifilar_pdf, esforcos_svg, esforcos_pdf, porticos_svg, porticos_pdf, grelha_svg, grelha_pdf, armaduras_svg, armaduras_pdf, quantitativos_csv, quantitativos_svg, quantitativos_pdf, tabela_aco_csv ou csv.")?;
        let (ext, suffix) = match format.as_str() {
            "pdf" => ("pdf", ""),
            "pdf_abas" => ("pdf", "-abas"),
            "png" => ("png", ""),
            "dxf" => ("dxf", ""),
            "svg" => ("svg", ""),
            "prancha_pdf" => ("pdf", "-prancha"),
            "prancha_pdf_abas" => ("pdf", "-pranchas-abas"),
            "prancha_svg" => ("svg", "-prancha"),
            "memorial_pdf" => ("pdf", "-memorial"),
            "caderno_familia_pdf" => ("pdf", "-caderno-da-familia"),
            "conjunto_obra_pdf" => ("pdf", "-conjunto-para-obra"),
            "primeira_fiada_svg" => ("svg", "-primeira-fiada"),
            "paginacao_pdf" => ("pdf", "-paginacao"),
            "locacao_svg" => ("svg", "-locacao"),
            "locacao_pdf" => ("pdf", "-locacao"),
            "cobertura_svg" => ("svg", "-cobertura"),
            "cobertura_pdf" => ("pdf", "-cobertura"),
            "unifilar_svg" => ("svg", "-unifilar"),
            "unifilar_pdf" => ("pdf", "-unifilar"),
            "esforcos_svg" => ("svg", "-esforcos"),
            "esforcos_pdf" => ("pdf", "-esforcos"),
            "porticos_svg" => ("svg", "-porticos"),
            "porticos_pdf" => ("pdf", "-porticos"),
            "grelha_svg" => ("svg", "-grelha"),
            "grelha_pdf" => ("pdf", "-grelha"),
            "armaduras_svg" => ("svg", "-armaduras"),
            "armaduras_pdf" => ("pdf", "-armaduras"),
            "quantitativos_csv" => ("csv", "-quantitativos"),
            "quantitativos_svg" => ("svg", "-quantitativos"),
            "quantitativos_pdf" => ("pdf", "-quantitativos"),
            "tabela_aco_csv" => ("csv", "-tabela-aco"),
            "csv" => ("csv", "-quadros"),
            _ => {
                return Err("Formato inválido. Use: pdf, pdf_abas, png, dxf, svg, prancha_pdf, prancha_pdf_abas, prancha_svg, memorial_pdf, caderno_familia_pdf, conjunto_obra_pdf, primeira_fiada_svg, paginacao_pdf, locacao_svg, locacao_pdf, cobertura_svg, cobertura_pdf, unifilar_svg, unifilar_pdf, esforcos_svg, esforcos_pdf, porticos_svg, porticos_pdf, grelha_svg, grelha_pdf, armaduras_svg, armaduras_pdf, quantitativos_csv, quantitativos_svg, quantitativos_pdf, tabela_aco_csv ou csv.".to_owned());
            }
        };
        let explicit_discipline = text(args, "disciplina");
        let discipline = match explicit_discipline.as_deref() {
            Some(raw) => crate::domain::discipline::Discipline::from_key(raw).ok_or(
                "'disciplina' deve ser tudo, arquitetura, eletrica, hidraulica, estrutura ou fundacoes.",
            )?,
            None => self.discipline,
        };
        let path = match text(args, "caminho") {
            Some(input) => resolve_path(&input, ext),
            None => unique_path(
                documents_dir().join(format!("{}{suffix}.{ext}", file_stem(&self.project.name))),
            ),
        };
        ensure_parent(&path)?;
        let view = if explicit_discipline.is_none() {
            self.active_export_view()
        } else {
            export::ExportView::for_discipline(&self.project, discipline)
        };
        let shown = self.project.for_discipline(discipline);
        let mut extra_pages: Option<usize> = None;
        match format.as_str() {
            "pdf" => export::pdf_for_view(&self.project, &view, &path)?,
            "pdf_abas" => {
                let views = self.project_tab_export_views();
                extra_pages = Some(export::pdf_views(&self.project, &views, &path)?);
            }
            "png" => export::png_for_view(&self.project, &view, &path)?,
            "dxf" => crate::io::dxf::export(&shown, &path)?,
            "svg" => export::svg_for_view(&self.project, &view, &path)?,
            "prancha_pdf" => export::export_scaled_pdf_for_view(&self.project, &view, &path)?,
            "prancha_pdf_abas" => {
                let views = self.project_tab_export_views();
                extra_pages = Some(export::export_scaled_pdf_views(
                    &self.project,
                    &views,
                    &path,
                )?);
            }
            "prancha_svg" => export::export_scaled_svg_for_view(&self.project, &view, &path)?,
            "conjunto_obra_pdf" => {
                let pages = crate::io::construction::set::export_construction_set_pdf(
                    &self.project,
                    &path,
                )?;
                extra_pages = Some(pages);
            }
            "primeira_fiada_svg" => {
                let markup = crate::io::construction::paging::first_course_svg(&self.project);
                std::fs::write(&path, markup)
                    .map_err(|e| format!("Não foi possível gravar: {e}"))?;
            }
            "cobertura_svg" => {
                let markup = crate::io::construction::roof_plan::roof_plan_svg(&self.project)?;
                std::fs::write(&path, markup)
                    .map_err(|e| format!("Não foi possível gravar: {e}"))?;
            }
            "cobertura_pdf" => {
                crate::io::construction::roof_plan::export_roof_plan_pdf(&self.project, &path)?;
            }
            "locacao_svg" => {
                let markup = crate::io::construction::setting_out::setting_out_svg(&self.project)?;
                std::fs::write(&path, markup)
                    .map_err(|e| format!("Não foi possível gravar: {e}"))?;
            }
            "locacao_pdf" => {
                crate::io::construction::setting_out::export_setting_out_pdf(&self.project, &path)?;
            }
            "paginacao_pdf" => {
                let pages =
                    crate::io::construction::paging::export_paging_pdf(&self.project, &path)?;
                extra_pages = Some(pages);
            }
            "caderno_familia_pdf" => {
                let date = crate::io::report::today();
                let pages = crate::io::booklet::export_family_booklet(&self.project, &date, &path)?;
                extra_pages = Some(pages);
            }
            "memorial_pdf" => {
                let date = crate::io::report::today();
                let pages =
                    crate::io::report::export_presizing_report(&self.project, &date, &path)?;
                extra_pages = Some(pages);
            }
            "unifilar_svg" => {
                let markup = crate::domain::distribution::single_line_svg(&self.project);
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "unifilar_pdf" => {
                let markup = crate::domain::distribution::single_line_svg(&self.project);
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "esforcos_svg" => {
                let markup = crate::domain::structure::beam_diagrams_svg(&self.project);
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "esforcos_pdf" => {
                let markup = crate::domain::structure::beam_diagrams_svg(&self.project);
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "porticos_svg" => {
                let markup = crate::domain::frame2d::frame_diagrams_svg(&self.project);
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "porticos_pdf" => {
                let markup = crate::domain::frame2d::frame_diagrams_svg(&self.project);
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "grelha_svg" => {
                let grid = self.project.cached_grid_analysis();
                let markup = crate::domain::grid_analysis::grid_analysis_svg(&self.project, &grid);
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "grelha_pdf" => {
                let grid = self.project.cached_grid_analysis();
                let markup = crate::domain::grid_analysis::grid_analysis_svg(&self.project, &grid);
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "armaduras_svg" => {
                let structure = self.project.cached_structure_report();
                let reinforcement = self.project.cached_reinforcement_report();
                let markup = crate::domain::reinforcement::reinforcement_detail_svg(
                    &self.project,
                    &structure,
                    &reinforcement,
                );
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "armaduras_pdf" => {
                let structure = self.project.cached_structure_report();
                let reinforcement = self.project.cached_reinforcement_report();
                let markup = crate::domain::reinforcement::reinforcement_detail_svg(
                    &self.project,
                    &structure,
                    &reinforcement,
                );
                crate::domain::views::export_view_pdf(&markup, &path)?;
            }
            "quantitativos_csv" => {
                let all_floors = args
                    .get("todos_pavimentos")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let takeoff = self
                    .project
                    .construction_takeoff_with_options(all_floors, None);
                let csv = crate::domain::quantities::takeoff_csv_for(&takeoff);
                std::fs::write(&path, format!("\u{FEFF}{csv}"))
                    .map_err(|e| format!("Não foi possível gravar: {e}"))?;
            }
            "quantitativos_svg" => {
                let all_floors = args
                    .get("todos_pavimentos")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let markup =
                    crate::domain::quantities::takeoff_sheet_svg(&self.project, all_floors);
                crate::domain::views::export_view_svg(&markup, &path)?;
            }
            "quantitativos_pdf" => {
                let all_floors = args
                    .get("todos_pavimentos")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                crate::io::quantities::export_pdf(&self.project, all_floors, &path)?;
            }
            "tabela_aco_csv" => {
                let reinforcement = self.project.cached_reinforcement_report();
                let csv = crate::domain::reinforcement::steel_schedule_csv(&reinforcement);
                std::fs::write(&path, csv).map_err(|e| format!("Não foi possível gravar: {e}"))?;
            }
            "csv" => export::csv(&shown, &path)?,
            other => return Err(format!("Formato de exportação desconhecido: '{other}'.")),
        }
        let msg = match (format.as_str(), extra_pages) {
            ("memorial_pdf", Some(pages)) => format!(
                "Memorial técnico ({pages} páginas) exportado em {}",
                path.display()
            ),
            ("conjunto_obra_pdf", Some(pages)) => format!(
                "Conjunto para obra ({pages} prancha(s) A3) exportado em {}",
                path.display()
            ),
            ("paginacao_pdf", Some(pages)) => format!(
                "Primeira fiada e paginação das paredes ({pages} prancha(s)) exportadas em {}",
                path.display()
            ),
            ("caderno_familia_pdf", Some(pages)) => format!(
                "Caderno da família ({pages} páginas) exportado em {}",
                path.display()
            ),
            ("pdf_abas" | "prancha_pdf_abas", Some(pages)) => format!(
                "PDF com todas as abas ({pages} página(s)) exportado em {}",
                path.display()
            ),
            _ => format!("Exportado ({format}) em {}", path.display()),
        };
        Ok(ToolOutput::ok(msg).with_structured(json!({
            "caminho": path.display().to_string(),
            "formato": format,
            "disciplina": discipline.key(),
            "paginas": extra_pages,
        })))
    }
}
