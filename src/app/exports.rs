//! Export and import commands: sheets and plans to PDF, SVG and PNG,
//! schedules to CSV, and DXF/DWG exchange.

use super::FloorPlanApp;
use crate::io::export;

impl FloorPlanApp {
    pub(super) fn export_sheet_pdf(&mut self) {
        let views = self.project_tab_export_views();
        let default_name = if views.len() > 1 {
            format!(
                "pranchas-{}-{}.pdf",
                self.project.sheet.size.short_name().to_lowercase(),
                self.project.sheet.scale.label().replace(':', "-")
            )
        } else {
            format!(
                "prancha-{}-{}.pdf",
                self.project.sheet.size.short_name().to_lowercase(),
                self.project.sheet.scale.label().replace(':', "-")
            )
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Prancha PDF em escala", &["pdf"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        self.message = match export::export_scaled_pdf_views(&self.project, &views, &path) {
            Ok(pages) if pages > 1 => {
                let labels: Vec<&str> = views.iter().map(|v| v.discipline.label()).collect();
                format!(
                    "Pranchas {} em escala {} exportadas em PDF ({pages} páginas: {}).",
                    self.project.sheet.size.short_name(),
                    self.project.sheet.scale.label(),
                    labels.join(", ")
                )
            }
            Ok(_) => format!(
                "Prancha {} em escala {} exportada em PDF ({}).",
                self.project.sheet.size.short_name(),
                self.project.sheet.scale.label(),
                self.discipline.label()
            ),
            Err(error) => format!("Não foi possível exportar a prancha PDF: {error}"),
        };
    }

    pub(super) fn export_active_tab_sheet_pdf(&mut self) {
        let view = self.active_export_view();
        let default_name = format!(
            "prancha-{}-{}-{}.pdf",
            view.discipline.key(),
            self.project.sheet.size.short_name().to_lowercase(),
            self.project.sheet.scale.label().replace(':', "-")
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Prancha PDF em escala", &["pdf"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        self.message = match export::export_scaled_pdf_for_view(&self.project, &view, &path) {
            Ok(()) => format!(
                "Prancha {} em escala {} exportada em PDF ({}).",
                self.project.sheet.size.short_name(),
                self.project.sheet.scale.label(),
                view.discipline.label()
            ),
            Err(error) => format!("Não foi possível exportar a prancha PDF: {error}"),
        };
    }

    pub(super) fn export_sheet_svg(&mut self) {
        let view = self.active_export_view();
        let default_name = format!(
            "prancha-{}-{}.svg",
            self.project.sheet.size.short_name().to_lowercase(),
            self.project.sheet.scale.label().replace(':', "-")
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Prancha SVG em escala", &["svg"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        self.message = match export::export_scaled_svg_for_view(&self.project, &view, &path) {
            Ok(()) => format!(
                "Prancha {} em escala {} exportada em SVG ({}).",
                self.project.sheet.size.short_name(),
                self.project.sheet.scale.label(),
                view.discipline.label()
            ),
            Err(error) => format!("Não foi possível exportar a prancha SVG: {error}"),
        };
    }

    pub(super) fn export_png(&mut self) {
        let view = self.active_export_view();
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Imagem PNG", &["png"])
            .set_file_name("planta-baixa.png")
            .save_file()
        else {
            return;
        };
        self.message = match export::png_for_view(&self.project, &view, &path) {
            Ok(()) => format!("Imagem PNG exportada ({}).", view.discipline.label()),
            Err(error) => format!("Não foi possível exportar PNG: {error}"),
        };
    }

    pub(super) fn export_pdf(&mut self) {
        let views = self.project_tab_export_views();
        let default_name = if views.len() > 1 {
            "projeto-abas.pdf"
        } else {
            "planta-baixa.pdf"
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Documento PDF", &["pdf"])
            .set_file_name(default_name)
            .save_file()
        else {
            return;
        };
        self.message = match export::pdf_views(&self.project, &views, &path) {
            Ok(pages) if pages > 1 => {
                let labels: Vec<&str> = views.iter().map(|v| v.discipline.label()).collect();
                format!(
                    "PDF exportado com {pages} páginas (1 por aba: {}).",
                    labels.join(", ")
                )
            }
            Ok(_) => format!("PDF exportado ({}).", views[0].discipline.label()),
            Err(error) => format!("Não foi possível exportar PDF: {error}"),
        };
    }

    pub(super) fn export_active_tab_pdf(&mut self) {
        let view = self.active_export_view();
        let default_name = format!("planta-{}.pdf", view.discipline.key());
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Documento PDF", &["pdf"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        self.message = match export::pdf_for_view(&self.project, &view, &path) {
            Ok(()) => format!("PDF da aba atual exportado ({}).", view.discipline.label()),
            Err(error) => format!("Não foi possível exportar PDF: {error}"),
        };
    }

    pub(super) fn export_svg(&mut self) {
        let view = self.active_export_view();
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Desenho SVG editável", &["svg"])
            .set_file_name("planta-baixa.svg")
            .save_file()
        else {
            return;
        };
        self.message = match export::svg_for_view(&self.project, &view, &path) {
            Ok(()) => format!("SVG editável exportado ({}).", view.discipline.label()),
            Err(error) => format!("Não foi possível exportar SVG: {error}"),
        };
    }

    pub(super) fn export_csv(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Planilha CSV", &["csv"])
            .set_file_name("quantitativos.csv")
            .save_file()
        else {
            return;
        };
        self.message = match export::csv(&self.project, &path) {
            Ok(()) => "CSV de áreas, esquadrias e quantitativos exportado.".to_owned(),
            Err(error) => format!("Não foi possível exportar CSV: {error}"),
        };
    }

    pub(super) fn export_dxf(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Desenho AutoCAD DXF", &["dxf"])
            .set_file_name("planta-baixa.dxf")
            .save_file()
        else {
            return;
        };
        self.message = match crate::io::dxf::export(&self.project, &path) {
            Ok(()) => "Desenho DXF exportado (camadas, cotas e unidades em metros).".to_owned(),
            Err(error) => format!("Não foi possível exportar DXF: {error}"),
        };
    }

    pub(super) fn import_dxf(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Desenho AutoCAD DXF", &["dxf"])
            .pick_file()
        else {
            return;
        };
        let previous = self.project.clone();
        match crate::io::dxf::import_file(&mut self.project, &path) {
            Ok(report) => {
                if report.total_imported() > 0 || report.layers > 0 {
                    self.record_change(previous);
                    self.fit_view_requested();
                }
                self.message = report.summary_pt();
            }
            Err(error) => {
                self.project = previous;
                self.message = format!("Não foi possível importar DXF: {error}");
            }
        }
    }

    pub(super) fn export_dwg(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Desenho AutoCAD DWG", &["dwg"])
            .set_file_name("planta-baixa.dwg")
            .save_file()
        else {
            return;
        };
        self.message = match crate::io::dwg::export_dwg_file(
            &self.project,
            &path,
            crate::io::dwg::DwgTargetVersion::Acad2018,
        ) {
            Ok(converter) => format!("Desenho DWG (AutoCAD 2018) exportado via {converter}."),
            Err(error) => format!("Não foi possível exportar DWG: {error}"),
        };
    }

    pub(super) fn import_dwg(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Desenho AutoCAD DWG", &["dwg"])
            .pick_file()
        else {
            return;
        };
        let previous = self.project.clone();
        match crate::io::dwg::import_dwg_file(&mut self.project, &path) {
            Ok(result) => {
                if result.report.total_imported() > 0 || result.report.layers > 0 {
                    self.record_change(previous);
                    self.fit_view_requested();
                }
                self.message = format!(
                    "DWG ({}) importado via {}: {}",
                    result.header.release_name,
                    result.converter_name,
                    result.report.summary_pt()
                );
            }
            Err(error) => {
                self.project = previous;
                self.message = format!("Não foi possível importar DWG: {error}");
            }
        }
    }

    /// Saves the first-course plan and the wall elevations (#54) as a PDF.
    pub(super) fn export_paging(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Documento PDF", &["pdf"])
            .set_file_name(format!("{}-primeira-fiada.pdf", self.document_name()))
            .save_file()
        else {
            return;
        };
        self.message =
            match crate::io::construction::paging::export_paging_pdf(&self.project, &path) {
                Ok(pages) => format!(
                    "Primeira fiada e paginação das paredes ({pages} prancha(s)) salvas em {}",
                    path.display()
                ),
                Err(error) => format!("Erro ao exportar a paginação: {error}"),
            };
    }

    /// Saves the setting-out plan (#55) as a PDF.
    pub(super) fn export_setting_out(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Documento PDF", &["pdf"])
            .set_file_name(format!("{}-locacao.pdf", self.document_name()))
            .save_file()
        else {
            return;
        };
        self.message = match crate::io::construction::setting_out::export_setting_out_pdf(
            &self.project,
            &path,
        ) {
            Ok(_) => format!("Planta de locação salva em {}", path.display()),
            Err(error) => format!("Erro ao exportar a locação: {error}"),
        };
    }
}
