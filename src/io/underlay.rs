//! Loading and decoding external raster images (PNG, JPEG, WebP, BMP), SVG,
//! and PDF pages for use as floor-plan underlays (#40).

use std::{fs, io::Cursor, path::Path, process::Command};

use base64::Engine as _;
use image::ImageFormat;
use resvg::{tiny_skia, usvg};

/// Decoded underlay pixels ready for GPU texture upload and optional embedded
/// PNG storage.
pub struct DecodedUnderlay {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub png_base64: String,
}

const MAX_UNDERLAY_SIDE: u32 = 2400;

/// Loads an image (PNG, JPEG, WebP, BMP), SVG, or PDF (specific 1-based page)
/// from disk and returns straight RGBA pixels plus a compact embedded PNG.
pub fn load_underlay_file(path: &Path, pdf_page: u32) -> Result<DecodedUnderlay, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "pdf" {
        let png_bytes = rasterize_pdf_page(path, pdf_page.max(1))?;
        return decode_raster_bytes(&png_bytes);
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("Não foi possível ler o arquivo de fundo: {error}"))?;
    if ext == "svg" {
        return decode_svg_bytes(&bytes);
    }
    decode_raster_bytes(&bytes)
}

/// Decodes a base64-encoded PNG (embedded inside a saved `.trazo` file) into
/// straight RGBA pixels.
pub fn decode_embedded_png(base64_str: &str) -> Result<(u32, u32, Vec<u8>), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_str.trim())
        .map_err(|error| format!("Imagem embutida inválida: {error}"))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|error| format!("Não foi possível decodificar a imagem embutida: {error}"))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}

/// Decodes PNG, JPEG, WebP, or BMP bytes, downscaling gently if wider than
/// `MAX_UNDERLAY_SIDE`, and produces a normalized embedded PNG representation.
pub fn decode_raster_bytes(bytes: &[u8]) -> Result<DecodedUnderlay, String> {
    let mut img = image::load_from_memory(bytes)
        .map_err(|error| format!("Formato de imagem não suportado ou corrompido: {error}"))?;
    if img.width() > MAX_UNDERLAY_SIDE || img.height() > MAX_UNDERLAY_SIDE {
        img = img.resize(
            MAX_UNDERLAY_SIDE,
            MAX_UNDERLAY_SIDE,
            image::imageops::FilterType::Triangle,
        );
    }
    let rgba_img = img.to_rgba8();
    let (width, height) = rgba_img.dimensions();
    let mut png_buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba_img.clone())
        .write_to(&mut png_buf, ImageFormat::Png)
        .map_err(|error| format!("Não foi possível comprimir a imagem de fundo: {error}"))?;
    let png_base64 = base64::engine::general_purpose::STANDARD.encode(png_buf.into_inner());
    Ok(DecodedUnderlay {
        width,
        height,
        rgba: rgba_img.into_raw(),
        png_base64,
    })
}

fn decode_svg_bytes(bytes: &[u8]) -> Result<DecodedUnderlay, String> {
    let options = crate::io::export::resvg_options();
    let tree = usvg::Tree::from_data(bytes, &options)
        .map_err(|error| format!("SVG de fundo inválido: {error}"))?;
    let size = tree.size();
    let max_dim = size.width().max(size.height()).max(1.0);
    let scale = (1600.0 / max_dim).min(2.0);
    let w = (size.width() * scale)
        .round()
        .clamp(16.0, MAX_UNDERLAY_SIDE as f32) as u32;
    let h = (size.height() * scale)
        .round()
        .clamp(16.0, MAX_UNDERLAY_SIDE as f32) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| "Não foi possível rasterizar o SVG de fundo.".to_owned())?;
    pixmap.fill(tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(
            w as f32 / size.width().max(1.0),
            h as f32 / size.height().max(1.0),
        ),
        &mut pixmap.as_mut(),
    );
    let png = pixmap
        .encode_png()
        .map_err(|error| format!("Falha ao codificar SVG de fundo: {error}"))?;
    decode_raster_bytes(&png)
}

/// Rasterizes a single 1-based page of a PDF file into PNG bytes using
/// available system renderers (Poppler `pdftoppm`, MuPDF `mutool`, Ghostscript
/// `gs`/`gswin64c`, or native Windows WinRT `Windows.Data.Pdf`).
fn rasterize_pdf_page(pdf_path: &Path, page: u32) -> Result<Vec<u8>, String> {
    let dir = std::env::temp_dir().join(format!(
        "trazo-pdf-underlay-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Não foi possível preparar pasta temporária de PDF: {error}"))?;
    let out_png = dir.join("page.png");

    // 1) Try pdftoppm (Poppler) — fast and crisp on Linux, macOS and Windows.
    let prefix = dir.join("out");
    if let Ok(status) = Command::new("pdftoppm")
        .args([
            "-png",
            "-r",
            "150",
            "-singlefile",
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
        ])
        .arg(pdf_path)
        .arg(&prefix)
        .status()
        && status.success()
    {
        let candidate = dir.join("out.png");
        if let Ok(bytes) = fs::read(&candidate) {
            let _ = fs::remove_dir_all(&dir);
            return Ok(bytes);
        }
    }

    // 2) Try mutool (MuPDF).
    if let Ok(status) = Command::new("mutool")
        .args(["draw", "-r", "150", "-o"])
        .arg(&out_png)
        .arg(pdf_path)
        .arg(page.to_string())
        .status()
        && status.success()
        && let Ok(bytes) = fs::read(&out_png)
    {
        let _ = fs::remove_dir_all(&dir);
        return Ok(bytes);
    }

    // 3) Try Ghostscript (gs / gswin64c).
    for gs_bin in ["gswin64c", "gswin32c", "gs"] {
        if let Ok(status) = Command::new(gs_bin)
            .args([
                "-dNOPAUSE",
                "-dBATCH",
                "-sDEVICE=png16m",
                "-r150",
                &format!("-dFirstPage={page}"),
                &format!("-dLastPage={page}"),
                &format!("-sOutputFile={}", out_png.display()),
            ])
            .arg(pdf_path)
            .status()
            && status.success()
            && let Ok(bytes) = fs::read(&out_png)
        {
            let _ = fs::remove_dir_all(&dir);
            return Ok(bytes);
        }
    }

    // 4) On Windows, use built-in WinRT Windows.Data.Pdf.PdfDocument (no external tools required).
    #[cfg(target_os = "windows")]
    {
        if let Ok(bytes) = rasterize_pdf_winrt(pdf_path, page, &out_png) {
            let _ = fs::remove_dir_all(&dir);
            return Ok(bytes);
        }
    }

    let _ = fs::remove_dir_all(&dir);
    Err("Não foi possível converter a página do PDF em imagem. Instale Poppler (pdftoppm), Ghostscript ou exporte a página como PNG/JPG.".to_owned())
}

#[cfg(target_os = "windows")]
fn rasterize_pdf_winrt(pdf_path: &Path, page: u32, out_png: &Path) -> Result<Vec<u8>, String> {
    let abs_pdf = fs::canonicalize(pdf_path).unwrap_or_else(|_| pdf_path.to_path_buf());
    let esc = |s: &str| s.replace('\'', "''");
    let pdf_str = esc(abs_pdf.to_string_lossy().trim_start_matches(r"\\?\"));
    let out_str = esc(&out_png.to_string_lossy());
    let zero_based = page.saturating_sub(1);
    let script = format!(
        "Add-Type -AssemblyName System.Runtime.WindowsRuntime\n\
         $asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{ $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' }})[0]\n\
         $asTaskAction = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{ $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncAction' }})[0]\n\
         function Await($op, $type) {{ $t = $asTaskGeneric.MakeGenericMethod($type).Invoke($null, @($op)); $t.Wait(); return $t.Result }}\n\
         function AwaitAction($op) {{ $t = $asTaskAction.Invoke($null, @($op)); $t.Wait() }}\n\
         $null = [Windows.Storage.StorageFile, Windows.Storage, ContentType=WindowsRuntime]\n\
         $null = [Windows.Data.Pdf.PdfDocument, Windows.Data.Pdf, ContentType=WindowsRuntime]\n\
         $null = [Windows.Storage.Streams.InMemoryRandomAccessStream, Windows.Storage.Streams, ContentType=WindowsRuntime]\n\
         $null = [Windows.Storage.Streams.DataReader, Windows.Storage.Streams, ContentType=WindowsRuntime]\n\
         $file = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync('{pdf_str}')) ([Windows.Storage.StorageFile])\n\
         $pdf = Await ([Windows.Data.Pdf.PdfDocument]::LoadFromFileAsync($file)) ([Windows.Data.Pdf.PdfDocument])\n\
         $idx = [Math]::Min({zero_based}, [int]$pdf.PageCount - 1)\n\
         $pg = $pdf.GetPage([uint32]$idx)\n\
         $opts = New-Object Windows.Data.Pdf.PdfPageRenderOptions\n\
         $opts.DestinationWidth = [uint32][Math]::Min(2000, [Math]::Max(800, $pg.Size.Width * 2))\n\
         $stream = New-Object Windows.Storage.Streams.InMemoryRandomAccessStream\n\
         AwaitAction ($pg.RenderToStreamAsync($stream, $opts))\n\
         $reader = New-Object Windows.Storage.Streams.DataReader($stream.GetInputStreamAt(0))\n\
         $size = [uint32]$stream.Size\n\
         $null = Await ($reader.LoadAsync($size)) ([uint32])\n\
         $bytes = New-Object byte[] $size\n\
         $reader.ReadBytes($bytes)\n\
         [System.IO.File]::WriteAllBytes('{out_str}', $bytes)\n"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    fs::read(out_png).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_png_and_round_trips_embedded_base64() {
        // Generate a tiny 4×2 valid PNG in memory.
        let mut pixmap = tiny_skia::Pixmap::new(4, 2).unwrap();
        pixmap.fill(tiny_skia::Color::from_rgba8(30, 111, 217, 255));
        let png_bytes = pixmap.encode_png().unwrap();

        let decoded = decode_raster_bytes(&png_bytes).unwrap();
        assert_eq!(decoded.width, 4);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.rgba.len(), 4 * 2 * 4);
        assert!(!decoded.png_base64.is_empty());

        let (w, h, rgba) = decode_embedded_png(&decoded.png_base64).unwrap();
        assert_eq!((w, h), (4, 2));
        assert_eq!(rgba.len(), 4 * 2 * 4);
    }

    #[test]
    fn decodes_svg_underlay_to_rgba() {
        let svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='200' height='100'><rect width='200' height='100' fill='#1d2836'/></svg>";
        let decoded = decode_svg_bytes(svg).unwrap();
        assert!(decoded.width >= 100);
        assert!(decoded.height >= 50);
        assert_eq!(
            decoded.rgba.len(),
            (decoded.width * decoded.height * 4) as usize
        );
    }
}
