//! AutoCAD DWG import and export via reliable external converters (#39).
//!
//! DWG is a proprietary binary format; Trazo integrates with industry-standard
//! converters — **ODA File Converter** (Open Design Alliance, free) and
//! **GNU LibreDWG** (`dwg2dxf` / `dxf2dwg`) — using Trazo's full DXF engine
//! (`io::dxf`) for geometry, layers, colours, dimensions, texts and hatches.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    domain::model::Project,
    io::dxf::{self, DxfImportReport},
};

/// Target AutoCAD DWG format revision for export.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DwgTargetVersion {
    Acad2000,
    Acad2010,
    Acad2013,
    #[default]
    Acad2018,
}

impl DwgTargetVersion {
    pub fn oda_token(self) -> &'static str {
        match self {
            Self::Acad2000 => "ACAD2000",
            Self::Acad2010 => "ACAD2010",
            Self::Acad2013 => "ACAD2013",
            Self::Acad2018 => "ACAD2018",
        }
    }

    pub fn libredwg_token(self) -> &'static str {
        match self {
            Self::Acad2000 => "r2000",
            Self::Acad2010 => "r2010",
            Self::Acad2013 => "r2013",
            Self::Acad2018 => "r2018",
        }
    }
}

/// Detected DWG header version from the 6-byte AutoCAD magic signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DwgHeaderInfo {
    pub code: String,
    pub release_name: &'static str,
}

/// Validates a DWG binary header (`AC10xx`) and identifies the AutoCAD release.
pub fn inspect_dwg_header(bytes: &[u8]) -> Result<DwgHeaderInfo, String> {
    if bytes.len() < 6 {
        return Err("Arquivo DWG muito pequeno ou corrompido.".to_owned());
    }
    let code = std::str::from_utf8(&bytes[..6]).map_err(|_| {
        "Cabeçalho inválido: este arquivo não parece ser um DWG AutoCAD.".to_owned()
    })?;
    let release_name = match code {
        "AC1009" => "AutoCAD R11/R12",
        "AC1012" => "AutoCAD R13",
        "AC1014" => "AutoCAD R14",
        "AC1015" => "AutoCAD 2000–2002",
        "AC1018" => "AutoCAD 2004–2006",
        "AC1021" => "AutoCAD 2007–2009",
        "AC1024" => "AutoCAD 2010–2012",
        "AC1027" => "AutoCAD 2013–2017",
        "AC1032" => "AutoCAD 2018+",
        _ => {
            return Err(format!(
                "Assinatura DWG não reconhecida ('{code}'). Verifique se o arquivo é um DWG válido."
            ));
        }
    };
    Ok(DwgHeaderInfo {
        code: code.to_owned(),
        release_name,
    })
}

/// Supported external converter backends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DwgConverter {
    OdaFileConverter(PathBuf),
    LibreDwg {
        dwg2dxf: Option<PathBuf>,
        dxf2dwg: Option<PathBuf>,
    },
}

impl DwgConverter {
    pub fn label(&self) -> &'static str {
        match self {
            Self::OdaFileConverter(_) => "ODA File Converter",
            Self::LibreDwg { .. } => "GNU LibreDWG",
        }
    }
}

/// Searches `TRAZO_DWG_CONVERTER`, `PATH`, and standard OS installation folders
/// for ODA File Converter or GNU LibreDWG.
pub fn detect_converter() -> Option<DwgConverter> {
    // 1) Explicit user environment variable override.
    if let Some(custom) = std::env::var_os("TRAZO_DWG_CONVERTER") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return classify_converter_path(p);
        }
    }

    // 2) ODA File Converter in PATH or standard install folders.
    if let Some(oda) = find_oda_converter() {
        return Some(DwgConverter::OdaFileConverter(oda));
    }

    // 3) GNU LibreDWG CLI tools in PATH.
    let dwg2dxf = find_in_path("dwg2dxf");
    let dxf2dwg = find_in_path("dxf2dwg");
    if dwg2dxf.is_some() || dxf2dwg.is_some() {
        return Some(DwgConverter::LibreDwg { dwg2dxf, dxf2dwg });
    }

    None
}

fn classify_converter_path(path: PathBuf) -> Option<DwgConverter> {
    let lower = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if lower.contains("dwg2dxf") {
        let dxf2dwg = path
            .parent()
            .map(|dir| {
                dir.join(if cfg!(windows) {
                    "dxf2dwg.exe"
                } else {
                    "dxf2dwg"
                })
            })
            .filter(|p| p.is_file());
        Some(DwgConverter::LibreDwg {
            dwg2dxf: Some(path),
            dxf2dwg,
        })
    } else if lower.contains("dxf2dwg") {
        let dwg2dxf = path
            .parent()
            .map(|dir| {
                dir.join(if cfg!(windows) {
                    "dwg2dxf.exe"
                } else {
                    "dwg2dxf"
                })
            })
            .filter(|p| p.is_file());
        Some(DwgConverter::LibreDwg {
            dwg2dxf,
            dxf2dwg: Some(path),
        })
    } else {
        Some(DwgConverter::OdaFileConverter(path))
    }
}

fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let candidates: Vec<String> = if cfg!(windows) {
        vec![format!("{binary}.exe"), binary.to_owned()]
    } else {
        vec![binary.to_owned()]
    };
    for dir in std::env::split_paths(&path_var) {
        for cand in &candidates {
            let full = dir.join(cand);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

fn find_oda_converter() -> Option<PathBuf> {
    if let Some(p) = find_in_path("ODAFileConverter") {
        return Some(p);
    }
    #[cfg(target_os = "windows")]
    {
        for root in [
            std::env::var_os("ProgramFiles"),
            std::env::var_os("ProgramFiles(x86)"),
        ]
        .into_iter()
        .flatten()
        {
            let oda_root = PathBuf::from(root).join("ODA");
            if let Ok(entries) = fs::read_dir(&oda_root) {
                for entry in entries.flatten() {
                    let exe = entry.path().join("ODAFileConverter.exe");
                    if exe.is_file() {
                        return Some(exe);
                    }
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let app_bin =
            PathBuf::from("/Applications/ODAFileConverter.app/Contents/MacOS/ODAFileConverter");
        if app_bin.is_file() {
            return Some(app_bin);
        }
    }
    #[cfg(target_os = "linux")]
    {
        for candidate in [
            "/usr/bin/ODAFileConverter",
            "/usr/local/bin/ODAFileConverter",
        ] {
            let p = PathBuf::from(candidate);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Builds ODA File Converter CLI arguments:
/// `ODAFileConverter <in_dir> <out_dir> <version> <DXF|DWG> <recurse:0> <audit:1> [filter]`
pub fn build_oda_args(
    in_dir: &Path,
    out_dir: &Path,
    version: DwgTargetVersion,
    output_format: &str,
    filter: &str,
) -> Vec<String> {
    vec![
        in_dir.to_string_lossy().into_owned(),
        out_dir.to_string_lossy().into_owned(),
        version.oda_token().to_owned(),
        output_format.to_owned(),
        "0".to_owned(),
        "1".to_owned(),
        filter.to_owned(),
    ]
}

/// Builds LibreDWG `dwg2dxf` CLI arguments (DWG → ASCII DXF).
pub fn build_libredwg_import_args(input_dwg: &Path, output_dxf: &Path) -> Vec<String> {
    vec![
        "-y".to_owned(),
        "-o".to_owned(),
        output_dxf.to_string_lossy().into_owned(),
        input_dwg.to_string_lossy().into_owned(),
    ]
}

/// Builds LibreDWG `dxf2dwg` CLI arguments (ASCII DXF → DWG).
pub fn build_libredwg_export_args(
    input_dxf: &Path,
    output_dwg: &Path,
    version: DwgTargetVersion,
) -> Vec<String> {
    vec![
        "-y".to_owned(),
        "--as".to_owned(),
        version.libredwg_token().to_owned(),
        "-o".to_owned(),
        output_dwg.to_string_lossy().into_owned(),
        input_dxf.to_string_lossy().into_owned(),
    ]
}

pub fn missing_converter_help() -> String {
    "Nenhum conversor DWG foi encontrado neste computador. \
     Instale o ODA File Converter (gratuito em opendesign.com) ou o GNU LibreDWG (dwg2dxf/dxf2dwg), \
     ou defina a variável de ambiente TRAZO_DWG_CONVERTER apontando para o executável. \
     Alternativamente, use Importar/Exportar DXF (nativo)."
        .to_owned()
}

/// Result of importing a DWG file via converter + Trazo's DXF importer.
pub struct DwgImportResult {
    pub header: DwgHeaderInfo,
    pub converter_name: String,
    pub report: DxfImportReport,
}

/// Imports an AutoCAD `.dwg` file by validating its header, converting to DXF
/// with the detected reliable converter, and merging into `project` via `io::dxf`.
pub fn import_dwg_file(project: &mut Project, path: &Path) -> Result<DwgImportResult, String> {
    let converter = detect_converter().ok_or_else(missing_converter_help)?;
    import_dwg_with_converter(project, path, &converter)
}

pub fn import_dwg_with_converter(
    project: &mut Project,
    path: &Path,
    converter: &DwgConverter,
) -> Result<DwgImportResult, String> {
    let raw =
        fs::read(path).map_err(|error| format!("Não foi possível ler o arquivo DWG: {error}"))?;
    let header = inspect_dwg_header(&raw)?;

    let work_dir = make_temp_work_dir("dwg-in")?;
    let in_dir = work_dir.join("in");
    let out_dir = work_dir.join("out");
    let _ = fs::create_dir_all(&in_dir);
    let _ = fs::create_dir_all(&out_dir);

    let result = (|| -> Result<DwgImportResult, String> {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("input.dwg");
        let staged_dwg = in_dir.join(file_name);
        fs::write(&staged_dwg, &raw)
            .map_err(|error| format!("Falha ao preparar arquivo DWG temporário: {error}"))?;
        let target_dxf = out_dir.join(Path::new(file_name).with_extension("dxf"));

        match converter {
            DwgConverter::OdaFileConverter(exe) => {
                let args = build_oda_args(
                    &in_dir,
                    &out_dir,
                    DwgTargetVersion::Acad2018,
                    "DXF",
                    "*.dwg",
                );
                run_converter(exe, &args, "ODA File Converter")?;
            }
            DwgConverter::LibreDwg { dwg2dxf, .. } => {
                let exe = dwg2dxf.as_ref().ok_or_else(|| {
                    "Ferramenta dwg2dxf (GNU LibreDWG) não encontrada para importar DWG.".to_owned()
                })?;
                let args = build_libredwg_import_args(&staged_dwg, &target_dxf);
                run_converter(exe, &args, "dwg2dxf")?;
            }
        }

        let produced_dxf = if target_dxf.is_file() {
            target_dxf
        } else {
            find_first_with_ext(&out_dir, "dxf").ok_or_else(|| {
                "O conversor DWG terminou sem gerar o arquivo DXF intermediário.".to_owned()
            })?
        };
        let report = dxf::import_file(project, &produced_dxf)?;
        Ok(DwgImportResult {
            header,
            converter_name: converter.label().to_owned(),
            report,
        })
    })();

    let _ = fs::remove_dir_all(&work_dir);
    result
}

/// Exports a Trazo `Project` to an AutoCAD `.dwg` file via Trazo's DXF
/// exporter + the detected reliable DWG converter.
pub fn export_dwg_file(
    project: &Project,
    path: &Path,
    version: DwgTargetVersion,
) -> Result<String, String> {
    let converter = detect_converter().ok_or_else(missing_converter_help)?;
    export_dwg_with_converter(project, path, version, &converter)
}

pub fn export_dwg_with_converter(
    project: &Project,
    path: &Path,
    version: DwgTargetVersion,
    converter: &DwgConverter,
) -> Result<String, String> {
    let work_dir = make_temp_work_dir("dwg-out")?;
    let in_dir = work_dir.join("in");
    let out_dir = work_dir.join("out");
    let _ = fs::create_dir_all(&in_dir);
    let _ = fs::create_dir_all(&out_dir);

    let result = (|| -> Result<String, String> {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("planta");
        let staged_dxf = in_dir.join(format!("{stem}.dxf"));
        let staged_dwg = out_dir.join(format!("{stem}.dwg"));
        dxf::export(project, &staged_dxf)?;

        match converter {
            DwgConverter::OdaFileConverter(exe) => {
                let args = build_oda_args(&in_dir, &out_dir, version, "DWG", "*.dxf");
                run_converter(exe, &args, "ODA File Converter")?;
            }
            DwgConverter::LibreDwg { dxf2dwg, .. } => {
                let exe = dxf2dwg.as_ref().ok_or_else(|| {
                    "Ferramenta dxf2dwg (GNU LibreDWG) não encontrada para exportar DWG.".to_owned()
                })?;
                let args = build_libredwg_export_args(&staged_dxf, &staged_dwg, version);
                run_converter(exe, &args, "dxf2dwg")?;
            }
        }

        let produced_dwg = if staged_dwg.is_file() {
            staged_dwg
        } else {
            find_first_with_ext(&out_dir, "dwg").ok_or_else(|| {
                "O conversor DWG terminou sem gerar o arquivo DWG de saída.".to_owned()
            })?
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::copy(&produced_dwg, path)
            .map_err(|error| format!("Não foi possível gravar o arquivo DWG: {error}"))?;
        Ok(converter.label().to_owned())
    })();

    let _ = fs::remove_dir_all(&work_dir);
    result
}

fn run_converter(exe: &Path, args: &[String], label: &str) -> Result<(), String> {
    let output = Command::new(exe)
        .args(args)
        .output()
        .map_err(|error| format!("Não foi possível executar {label}: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let detail = if stderr.is_empty() {
            format!("código {}", output.status)
        } else {
            stderr
        };
        return Err(format!("Falha na conversão DWG ({label}): {detail}"));
    }
    Ok(())
}

fn make_temp_work_dir(tag: &str) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("trazo-{tag}-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Não foi possível criar pasta temporária DWG: {error}"))?;
    Ok(dir)
}

fn find_first_with_ext(dir: &Path, ext: &str) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_valid_dwg_headers_and_rejects_non_dwg_files() {
        let info = inspect_dwg_header(b"AC1032\x00\x00\x00\x00").unwrap();
        assert_eq!(info.code, "AC1032");
        assert_eq!(info.release_name, "AutoCAD 2018+");

        let old = inspect_dwg_header(b"AC1015extra").unwrap();
        assert_eq!(old.release_name, "AutoCAD 2000–2002");

        assert!(inspect_dwg_header(b"short").is_err());
        assert!(inspect_dwg_header(b"%PDF-1.7").is_err());
    }

    #[test]
    fn builds_exact_oda_and_libredwg_cli_arguments() {
        let oda = build_oda_args(
            Path::new("/tmp/in"),
            Path::new("/tmp/out"),
            DwgTargetVersion::Acad2018,
            "DXF",
            "*.dwg",
        );
        assert_eq!(oda[2], "ACAD2018");
        assert_eq!(oda[3], "DXF");
        assert_eq!(oda[4], "0");
        assert_eq!(oda[5], "1");
        assert_eq!(oda[6], "*.dwg");

        let imp = build_libredwg_import_args(Path::new("a.dwg"), Path::new("a.dxf"));
        assert_eq!(imp, vec!["-y", "-o", "a.dxf", "a.dwg"]);

        let exp = build_libredwg_export_args(
            Path::new("a.dxf"),
            Path::new("a.dwg"),
            DwgTargetVersion::Acad2013,
        );
        assert_eq!(exp, vec!["-y", "--as", "r2013", "-o", "a.dwg", "a.dxf"]);
    }

    #[test]
    fn classifies_converter_paths_cleanly() {
        assert!(matches!(
            classify_converter_path(PathBuf::from("ODAFileConverter.exe")),
            Some(DwgConverter::OdaFileConverter(_))
        ));
        assert!(matches!(
            classify_converter_path(PathBuf::from("/usr/bin/dwg2dxf")),
            Some(DwgConverter::LibreDwg {
                dwg2dxf: Some(_),
                ..
            })
        ));
        assert!(missing_converter_help().contains("ODA File Converter"));
        assert!(missing_converter_help().contains("LibreDWG"));
    }
}
