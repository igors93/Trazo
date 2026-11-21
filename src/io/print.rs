//! Exact-scale sheet printing with rasterized sheet preview and native system
//! print dialog dispatch (#70).

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use resvg::{tiny_skia, usvg};

use crate::{domain::model::Project, io::export::sheet};

/// User-selected options for exact-scale printing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrintOptions {
    pub copies: u16,
    pub dpi: u32,
    pub monochrome: bool,
    /// Optional target printer name; when `None`, the system default is preselected.
    pub printer: Option<String>,
    /// When true, show the operating system's native print dialog before printing.
    pub use_system_dialog: bool,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            copies: 1,
            dpi: 300,
            monochrome: false,
            printer: None,
            use_system_dialog: true,
        }
    }
}

/// Rasterized preview of the exact-scale technical sheet for display inside the
/// Print dialog.
pub struct SheetPreview {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub sheet_mm: (f64, f64),
    pub scale_label: String,
    pub mm_per_meter: f64,
    pub fits: bool,
}

/// Converts physical sheet dimensions in millimetres to hundredths of an inch,
/// the unit used by Windows `PrintDocument` / `PaperSize` for 1:1 physical
/// printing without shrink-to-fit distortion.
pub fn sheet_hundredths_of_inch(width_mm: f64, height_mm: f64) -> (i32, i32) {
    let w = ((width_mm / 25.4) * 100.0).round().max(100.0) as i32;
    let h = ((height_mm / 25.4) * 100.0).round().max(100.0) as i32;
    (w, h)
}

/// Computes exact raster dimensions for a sheet in millimetres at `dpi`, capped
/// safely at `max_side` pixels per axis so very large formats (A0) stay fast
/// while keeping exact aspect ratio.
pub fn sheet_pixels_at_dpi(width_mm: f64, height_mm: f64, dpi: u32, max_side: u32) -> (u32, u32) {
    let dpi = dpi.clamp(72, 600) as f64;
    let raw_w = (width_mm / 25.4 * dpi).round().max(1.0);
    let raw_h = (height_mm / 25.4 * dpi).round().max(1.0);
    let max_raw = raw_w.max(raw_h);
    let factor = if max_raw > f64::from(max_side) {
        f64::from(max_side) / max_raw
    } else {
        1.0
    };
    let w = (raw_w * factor).round().max(1.0) as u32;
    let h = (raw_h * factor).round().max(1.0) as u32;
    (w, h)
}

fn themed_project_for_print(project: &Project, monochrome: bool) -> Project {
    let mut copy = project.clone();
    if monochrome {
        copy.doc_style.monochrome = true;
    }
    copy
}

/// Rasterizes SVG markup to straight (not premultiplied) RGBA, ready to
/// upload as an `egui` texture.
pub(crate) fn svg_to_rgba(markup: &str, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let pixmap = rasterize_svg(markup, width, height)?;
    let mut rgba = pixmap.data().to_vec();
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = px[3];
        if a > 0 && a < 255 {
            let af = f32::from(a) / 255.0;
            px[0] = ((f32::from(px[0]) / af).min(255.0)) as u8;
            px[1] = ((f32::from(px[1]) / af).min(255.0)) as u8;
            px[2] = ((f32::from(px[2]) / af).min(255.0)) as u8;
        }
    }
    Ok(rgba)
}

fn rasterize_svg(markup: &str, width: u32, height: u32) -> Result<tiny_skia::Pixmap, String> {
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_str(markup, &options)
        .map_err(|error| format!("Não foi possível preparar a prancha: {error}"))?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| "Não foi possível alocar a imagem da prancha.".to_owned())?;
    pixmap.fill(tiny_skia::Color::WHITE);
    let size = tree.size();
    let sx = width as f32 / size.width().max(1.0);
    let sy = height as f32 / size.height().max(1.0);
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(sx, sy),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap)
}

/// Renders an RGBA preview of the project's scaled sheet, ready to upload as an
/// `egui` texture.
pub fn render_sheet_preview(
    project: &Project,
    monochrome: bool,
    max_side: u32,
) -> Result<SheetPreview, String> {
    let themed = themed_project_for_print(project, monochrome);
    let (sheet_w, sheet_h) = themed.sheet.sheet_mm();
    let markup = sheet::scaled_sheet_svg(&themed);
    let (w, h) = sheet_pixels_at_dpi(sheet_w, sheet_h, 150, max_side.max(64));
    let pixmap = rasterize_svg(&markup, w, h)?;

    // Convert premultiplied RGBA from tiny-skia to straight RGBA for egui.
    let mut rgba = pixmap.data().to_vec();
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = px[3];
        if a > 0 && a < 255 {
            let af = f32::from(a) / 255.0;
            px[0] = ((f32::from(px[0]) / af).min(255.0)) as u8;
            px[1] = ((f32::from(px[1]) / af).min(255.0)) as u8;
            px[2] = ((f32::from(px[2]) / af).min(255.0)) as u8;
        }
    }

    let fits = if let Some((min, max)) = themed.bounds() {
        let (_, _, pw, ph) = themed.sheet.printable_rect_mm();
        let plan_w = (max.x - min.x).max(0.5);
        let plan_h = (max.y - min.y).max(0.5);
        themed
            .sheet
            .scale
            .fits(plan_w, plan_h, pw, (ph - 16.0).max(20.0))
    } else {
        true
    };

    Ok(SheetPreview {
        width: w as usize,
        height: h as usize,
        rgba,
        sheet_mm: (sheet_w, sheet_h),
        scale_label: themed.sheet.scale.label(),
        mm_per_meter: themed.sheet.scale.mm_per_meter(),
        fits,
    })
}

/// Writes an exact-scale PNG (at `dpi`) and an exact-scale PDF for printing
/// into a temporary job folder.
pub fn prepare_print_files(
    project: &Project,
    options: &PrintOptions,
) -> Result<(PathBuf, PathBuf, (f64, f64)), String> {
    let themed = themed_project_for_print(project, options.monochrome);
    let sheet_mm = themed.sheet.sheet_mm();
    let dir = std::env::temp_dir().join("trazo-print");
    fs::create_dir_all(&dir).map_err(|error| {
        format!("Não foi possível criar pasta temporária de impressão: {error}")
    })?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pdf_path = dir.join(format!("prancha-{stamp}.pdf"));
    let png_path = dir.join(format!("prancha-{stamp}.png"));

    sheet::export_scaled_pdf(&themed, &pdf_path)?;
    let markup = sheet::scaled_sheet_svg(&themed);
    let (w, h) = sheet_pixels_at_dpi(sheet_mm.0, sheet_mm.1, options.dpi, 7200);
    let pixmap = rasterize_svg(&markup, w, h)?;
    pixmap
        .save_png(&png_path)
        .map_err(|error| format!("Não foi possível gerar a imagem de impressão: {error}"))?;
    Ok((png_path, pdf_path, sheet_mm))
}

/// Lists installed printers on the host system, returning an empty list if none
/// can be queried.
pub fn list_printers() -> Vec<String> {
    if cfg!(test) {
        return Vec::new();
    }
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-Printer | Select-Object -ExpandProperty Name",
            ])
            .output();
        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("lpstat").arg("-e").output();
        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// Builds the PowerShell script that opens the native Windows `PrintDialog` (or
/// prints directly) using `System.Drawing.Printing.PrintDocument` at exact 1:1
/// physical millimetres without shrink-to-fit.
pub fn windows_print_script(
    image_path: &Path,
    sheet_mm: (f64, f64),
    options: &PrintOptions,
    document_name: &str,
) -> String {
    let (w_hi, h_hi) = sheet_hundredths_of_inch(sheet_mm.0, sheet_mm.1);
    let landscape = sheet_mm.0 > sheet_mm.1;
    let (paper_w, paper_h) = if landscape {
        (h_hi, w_hi)
    } else {
        (w_hi, h_hi)
    };
    let esc = |s: &str| s.replace('\'', "''");
    let img_esc = esc(&image_path.to_string_lossy());
    let doc_esc = esc(document_name);
    let copies = options.copies.clamp(1, 99);
    let printer_line = match options.printer.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => {
            format!("$doc.PrinterSettings.PrinterName = '{}'\n", esc(name))
        }
        _ => String::new(),
    };
    let dispatch = if options.use_system_dialog {
        "$dlg = New-Object System.Windows.Forms.PrintDialog\n\
         $dlg.Document = $doc\n\
         $dlg.UseEXDialog = $true\n\
         $dlg.AllowSomePages = $false\n\
         if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {\n\
             $doc.Print()\n\
             Write-Output 'PRINTED'\n\
         } else {\n\
             Write-Output 'CANCELLED'\n\
         }\n"
    } else {
        "$doc.Print()\nWrite-Output 'PRINTED'\n"
    };

    format!(
        "Add-Type -AssemblyName System.Drawing\n\
         Add-Type -AssemblyName System.Windows.Forms\n\
         $img = [System.Drawing.Image]::FromFile('{img_esc}')\n\
         $doc = New-Object System.Drawing.Printing.PrintDocument\n\
         $doc.DocumentName = '{doc_esc}'\n\
         $doc.OriginAtMargins = $false\n\
         $doc.PrinterSettings.Copies = {copies}\n\
         {printer_line}\
         $doc.DefaultPageSettings.Landscape = ${landscape}\n\
         $doc.DefaultPageSettings.Margins = New-Object System.Drawing.Printing.Margins(0, 0, 0, 0)\n\
         $doc.DefaultPageSettings.PaperSize = New-Object System.Drawing.Printing.PaperSize('Trazo', {paper_w}, {paper_h})\n\
         $doc.add_PrintPage({{\n\
             param($sender, $e)\n\
             $e.Graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic\n\
             $rect = New-Object System.Drawing.RectangleF(0, 0, {w_hi}, {h_hi})\n\
             $e.Graphics.DrawImage($img, $rect)\n\
             $e.HasMorePages = $false\n\
         }})\n\
         try {{\n\
             {dispatch}\
         }} finally {{\n\
             $img.Dispose()\n\
             $doc.Dispose()\n\
         }}\n"
    )
}

/// Builds CUPS `lp` arguments for exact 100% scale printing on macOS and Linux.
#[allow(dead_code)]
pub fn unix_lp_args(pdf_path: &Path, options: &PrintOptions) -> Vec<String> {
    let mut args = vec![
        "-n".to_owned(),
        options.copies.clamp(1, 99).to_string(),
        "-o".to_owned(),
        "fit-to-page=false".to_owned(),
        "-o".to_owned(),
        "scaling=100".to_owned(),
    ];
    if let Some(printer) = options.printer.as_deref().map(str::trim)
        && !printer.is_empty()
    {
        args.push("-d".to_owned());
        args.push(printer.to_owned());
    }
    args.push(pdf_path.to_string_lossy().into_owned());
    args
}

/// Sends the exact-scale sheet to the OS print dialog (or directly to the
/// selected printer). Intended to be called from a background thread so the
/// editor stays responsive while the system print dialog is open.
pub fn dispatch_print(project: &Project, options: &PrintOptions) -> Result<String, String> {
    let (png_path, pdf_path, sheet_mm) = prepare_print_files(project, options)?;
    let doc_name = if project.name.trim().is_empty() {
        "Trazo - Planta baixa".to_owned()
    } else {
        format!("Trazo - {}", project.name.trim())
    };

    #[cfg(target_os = "windows")]
    {
        let script = windows_print_script(&png_path, sheet_mm, options, &doc_name);
        let script_path = png_path.with_extension("ps1");
        fs::write(&script_path, script)
            .map_err(|error| format!("Não foi possível criar script de impressão: {error}"))?;
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-STA",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                &script_path.to_string_lossy(),
            ])
            .output()
            .map_err(|error| format!("Não foi possível abrir o diálogo de impressão: {error}"))?;
        let _ = fs::remove_file(&script_path);
        let _ = fs::remove_file(&png_path);
        let _ = fs::remove_file(&pdf_path);
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(if stderr.is_empty() {
                "A impressão falhou no sistema.".to_owned()
            } else {
                format!("Erro ao imprimir: {stderr}")
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("CANCELLED") {
            Ok("Impressão cancelada.".to_owned())
        } else {
            Ok(format!(
                "Prancha enviada para impressão em escala exata ({}).",
                project.sheet.scale.label()
            ))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (png_path, sheet_mm, doc_name);
        if options.use_system_dialog {
            let opener = if cfg!(target_os = "macos") {
                "open"
            } else {
                "xdg-open"
            };
            Command::new(opener)
                .arg(&pdf_path)
                .spawn()
                .map_err(|error| {
                    format!("Não foi possível abrir o visualizador de impressão: {error}")
                })?;
            Ok(format!(
                "Prancha aberta em escala exata ({}) para impressão no sistema.",
                project.sheet.scale.label()
            ))
        } else {
            let args = unix_lp_args(&pdf_path, options);
            let output = Command::new("lp")
                .args(&args)
                .output()
                .map_err(|error| format!("Não foi possível enviar para a impressora: {error}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                return Err(format!("Erro ao imprimir: {stderr}"));
            }
            Ok(format!(
                "Prancha enviada para impressão em escala exata ({}).",
                project.sheet.scale.label()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        geometry::Point,
        sheet::{DrawingScale, Orientation, PaperSize, SheetLayout},
    };

    #[test]
    fn sheet_preview_renders_exact_aspect_ratio_and_rgba_buffer() {
        let mut project = Project::default();
        project.sheet = SheetLayout::standard(PaperSize::A3, Orientation::Landscape);
        project.sheet.scale = DrawingScale::S1_50;
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
            .unwrap();

        let preview = render_sheet_preview(&project, false, 420).unwrap();
        assert_eq!(preview.width, 420);
        assert!(preview.height > 280 && preview.height < 310);
        assert_eq!(preview.rgba.len(), preview.width * preview.height * 4);
        assert_eq!(preview.scale_label, "1:50");
        assert!(preview.fits);
        // Must contain both white background pixels and non-white drawn ink.
        assert!(
            preview
                .rgba
                .as_chunks::<4>()
                .0
                .contains(&[255, 255, 255, 255])
        );
        assert!(preview.rgba.as_chunks::<4>().0.iter().any(|p| p[0] < 100));
    }

    #[test]
    fn physical_dimensions_convert_accurately_to_hundredths_of_inch_and_dpi() {
        // A4 portrait: 210 × 297 mm ≈ 827 × 1169 hundredths of an inch.
        assert_eq!(sheet_hundredths_of_inch(210.0, 297.0), (827, 1169));
        // A3 landscape at 300 DPI: 420 mm = 4961 px, 297 mm = 3508 px.
        assert_eq!(sheet_pixels_at_dpi(420.0, 297.0, 300, 7200), (4961, 3508));
        // A0 capped cleanly without losing aspect ratio.
        let (w, h) = sheet_pixels_at_dpi(1189.0, 841.0, 300, 7200);
        assert_eq!(w, 7200);
        assert!((f64::from(w) / f64::from(h) - 1189.0 / 841.0).abs() < 0.01);
    }

    #[test]
    fn windows_print_script_and_unix_lp_args_preserve_exact_scale_settings() {
        let opts = PrintOptions {
            copies: 2,
            dpi: 300,
            monochrome: true,
            printer: Some("HP DesignJet".to_owned()),
            use_system_dialog: true,
        };
        let script = windows_print_script(
            Path::new(r"C:\Temp\prancha.png"),
            (420.0, 297.0),
            &opts,
            "Casa Térrea",
        );
        assert!(script.contains("PrintDialog"));
        assert!(script.contains("Copies = 2"));
        assert!(script.contains("HP DesignJet"));
        assert!(script.contains("Landscape = $true"));
        assert!(script.contains("RectangleF(0, 0, 1654, 1169)"));

        let lp = unix_lp_args(Path::new("/tmp/prancha.pdf"), &opts);
        assert!(lp.contains(&"fit-to-page=false".to_owned()));
        assert!(lp.contains(&"scaling=100".to_owned()));
        assert!(lp.contains(&"HP DesignJet".to_owned()));
    }
}
