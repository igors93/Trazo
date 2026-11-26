//! Room lighting calculation by the Lumen Method (*Método dos Lúmens*,
//! ABNT NBR ISO/CIE 8995-1:2013) (#125): computes the room index `K`,
//! utilization factor `Fu`, required luminous flux, achieved maintained
//! illuminance (lux), recommended luminaire count, power density (W/m²),
//! traffic-light status, plan lux overlay marks, CSV and A3 SVG sheet.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::domain::{
    conduits::PlanMark,
    electrical::DeviceRole,
    fixtures::FixtureKind,
    geometry::Point,
    model::{Project, Room},
    verdict::Light,
};

/// Default ceiling luminaire luminous flux (18 W residential LED plafon), lm.
pub const DEFAULT_CEILING_LUMENS: f64 = 1600.0;
/// Default wall sconce (*arandela*) luminous flux (9 W LED), lm.
pub const DEFAULT_WALL_LUMENS: f64 = 800.0;
/// Conservative residential LED luminous efficacy, lm/W.
pub const DEFAULT_LAMP_EFFICACY_LM_W: f64 = 90.0;
/// Standard work-plane height (ABNT NBR ISO/CIE 8995-1), m.
pub const DEFAULT_WORK_PLANE_M: f64 = 0.75;
/// Clean residential interior maintenance factor `Fd`, dimensionless.
pub const DEFAULT_MAINTENANCE_FACTOR: f64 = 0.80;

/// Ceiling / wall / floor reflectance combination for the utilization factor table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceReflectance {
    /// 70 % ceiling / 50 % walls / 20 % floor (light painted interiors).
    #[default]
    Light,
    /// 50 % ceiling / 30 % walls / 10 % floor (medium tones / wood / brick).
    Medium,
    /// 30 % ceiling / 10 % walls / 10 % floor (dark surfaces).
    Dark,
}

impl SurfaceReflectance {
    pub const ALL: [Self; 3] = [Self::Light, Self::Medium, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Claras (teto 70% · parede 50% · piso 20%)",
            Self::Medium => "Médias (teto 50% · parede 30% · piso 10%)",
            Self::Dark => "Escuras (teto 30% · parede 10% · piso 10%)",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Light => "70/50/20",
            Self::Medium => "50/30/10",
            Self::Dark => "30/10/10",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Light => "claras",
            Self::Medium => "medias",
            Self::Dark => "escuras",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "claras" | "claro" | "light" | "70/50/20" => Some(Self::Light),
            "medias" | "médias" | "medio" | "médio" | "medium" | "50/30/10" => Some(Self::Medium),
            "escuras" | "escuro" | "dark" | "30/10/10" => Some(Self::Dark),
            _ => None,
        }
    }
}

fn default_ceiling_lumens() -> f64 {
    DEFAULT_CEILING_LUMENS
}
fn default_wall_lumens() -> f64 {
    DEFAULT_WALL_LUMENS
}
fn default_efficacy() -> f64 {
    DEFAULT_LAMP_EFFICACY_LM_W
}
fn default_work_plane() -> f64 {
    DEFAULT_WORK_PLANE_M
}
fn default_maintenance() -> f64 {
    DEFAULT_MAINTENANCE_FACTOR
}
fn default_true() -> bool {
    true
}

/// Project-wide Lumen Method settings persisted inside `ElectricalSettings`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingSettings {
    #[serde(default = "default_ceiling_lumens")]
    pub ceiling_lumens: f64,
    #[serde(default = "default_wall_lumens")]
    pub wall_lumens: f64,
    #[serde(default = "default_efficacy")]
    pub lamp_efficacy_lm_w: f64,
    #[serde(default = "default_work_plane")]
    pub work_plane_m: f64,
    #[serde(default = "default_maintenance")]
    pub maintenance_factor: f64,
    #[serde(default)]
    pub reflectance: SurfaceReflectance,
    #[serde(default = "default_true")]
    pub show_plan_overlay: bool,
}

impl Default for LightingSettings {
    fn default() -> Self {
        Self {
            ceiling_lumens: DEFAULT_CEILING_LUMENS,
            wall_lumens: DEFAULT_WALL_LUMENS,
            lamp_efficacy_lm_w: DEFAULT_LAMP_EFFICACY_LM_W,
            work_plane_m: DEFAULT_WORK_PLANE_M,
            maintenance_factor: DEFAULT_MAINTENANCE_FACTOR,
            reflectance: SurfaceReflectance::Light,
            show_plan_overlay: true,
        }
    }
}

impl LightingSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Maintained illuminance target per ABNT NBR ISO/CIE 8995-1:2013.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomLightingTarget {
    pub category: &'static str,
    pub target_lux: f64,
    pub clause: &'static str,
}

/// Classifies a room name into its ABNT NBR ISO/CIE 8995-1 maintained illuminance target.
pub fn target_illuminance_for_room(name: &str) -> RoomLightingTarget {
    let lower = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if has(&[
        "escritório",
        "escritorio",
        "home office",
        "estudo",
        "biblioteca",
        "ateliê",
        "atelie",
    ]) {
        RoomLightingTarget {
            category: "Escritório / leitura",
            target_lux: 500.0,
            clause: "NBR ISO/CIE 8995-1 §5.26 (leitura, escrita e trabalho em mesa)",
        }
    } else if has(&[
        "cozinha",
        "copa",
        "serviço",
        "servico",
        "lavanderia",
        "gourmet",
        "churrasqueira",
    ]) {
        RoomLightingTarget {
            category: "Cozinha / área de serviço",
            target_lux: 300.0,
            clause: "NBR ISO/CIE 8995-1 §5.22 (preparo de alimentos e bancadas)",
        }
    } else if has(&[
        "banheiro",
        "lavabo",
        "banho",
        "sanitário",
        "sanitario",
        "wc",
        "vestiário",
        "vestiario",
    ]) {
        RoomLightingTarget {
            category: "Banheiro / lavabo",
            target_lux: 200.0,
            clause: "NBR ISO/CIE 8995-1 §5.1.3 (sanitários e espelho)",
        }
    } else if has(&["jantar"]) {
        RoomLightingTarget {
            category: "Sala de jantar",
            target_lux: 200.0,
            clause: "NBR ISO/CIE 8995-1 §5.22 (refeições)",
        }
    } else if has(&[
        "sala",
        "estar",
        "quarto",
        "dormitório",
        "dormitorio",
        "suíte",
        "suite",
        "tv",
    ]) {
        RoomLightingTarget {
            category: "Sala / dormitório",
            target_lux: 150.0,
            clause: "NBR ISO/CIE 8995-1 (iluminação geral residencial)",
        }
    } else {
        RoomLightingTarget {
            category: "Circulação / garagem / apoio",
            target_lux: 100.0,
            clause: "NBR ISO/CIE 8995-1 §5.1.1 (corredores, escadas, garagem e varandas)",
        }
    }
}

/// Utilization factor `Fu` for a direct-distribution LED luminaire (LOR ≈ 0.85)
/// interpolated linearly from Room Index `K` (`0.60 .. 5.00`) and surface reflectances.
pub fn utilization_factor(room_index_k: f64, reflectance: SurfaceReflectance) -> f64 {
    const K_GRID: [f64; 10] = [0.60, 0.80, 1.00, 1.25, 1.50, 2.00, 2.50, 3.00, 4.00, 5.00];
    const FU_LIGHT: [f64; 10] = [0.42, 0.50, 0.56, 0.62, 0.66, 0.72, 0.76, 0.79, 0.82, 0.84];
    const FU_MEDIUM: [f64; 10] = [0.35, 0.43, 0.49, 0.54, 0.59, 0.65, 0.69, 0.72, 0.76, 0.78];
    const FU_DARK: [f64; 10] = [0.30, 0.37, 0.43, 0.48, 0.53, 0.59, 0.63, 0.66, 0.70, 0.72];

    let table = match reflectance {
        SurfaceReflectance::Light => &FU_LIGHT,
        SurfaceReflectance::Medium => &FU_MEDIUM,
        SurfaceReflectance::Dark => &FU_DARK,
    };
    let k = room_index_k.clamp(K_GRID[0], K_GRID[K_GRID.len() - 1]);
    for i in 0..K_GRID.len() - 1 {
        let (k0, k1) = (K_GRID[i], K_GRID[i + 1]);
        if k <= k1 + 1e-12 {
            let t = ((k - k0) / (k1 - k0)).clamp(0.0, 1.0);
            return table[i] + t * (table[i + 1] - table[i]);
        }
    }
    table[table.len() - 1]
}

/// Lumen Method result for one room.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomLightingResult {
    pub room_id: u64,
    pub room_name: String,
    pub category: String,
    pub area_m2: f64,
    pub perimeter_m: f64,
    pub mounting_height_m: f64,
    pub room_index_k: f64,
    pub utilization_factor: f64,
    pub maintenance_factor: f64,
    pub target_lux: f64,
    pub clause: String,
    pub luminaires: usize,
    pub installed_lumens: f64,
    pub required_lumens: f64,
    pub calculated_lux: f64,
    pub recommended_luminaires: usize,
    pub power_density_w_m2: f64,
    /// `target_lux / calculated_lux` (≤ 0.85 green, ≤ 1.0 yellow, > 1.0 red).
    pub utilisation: f64,
    pub light: Light,
    pub plain_note: String,
    pub anchor: Option<Point>,
}

/// Full lighting study for all rooms in the project.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LightingReport {
    pub settings: LightingSettings,
    pub rooms: Vec<RoomLightingResult>,
    pub total_installed_lumens: f64,
    pub total_estimated_watts: f64,
    pub rooms_passing: usize,
    pub rooms_failing: usize,
    pub assumptions: Vec<String>,
}

fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn room_perimeter_m(project: &Project, room: &Room) -> f64 {
    project
        .room_usable_polygon(room)
        .or_else(|| project.room_outline(room))
        .map(|outline| {
            (0..outline.len())
                .map(|i| outline[i].distance(outline[(i + 1) % outline.len()]))
                .sum()
        })
        .unwrap_or(0.0)
}

/// Computes the ABNT NBR ISO/CIE 8995-1 Lumen Method lighting calculation for every room.
pub fn lighting_report(project: &Project) -> LightingReport {
    let settings = project.electrical.lighting;
    let hm = (project.wall_height - settings.work_plane_m).max(0.5);
    let fd = settings.maintenance_factor.clamp(0.50, 0.95);
    let efficacy = settings.lamp_efficacy_lm_w.max(20.0);
    let ceil_lm = settings.ceiling_lumens.max(100.0);
    let wall_lm = settings.wall_lumens.max(100.0);

    let mut report = LightingReport {
        settings,
        ..LightingReport::default()
    };

    for room in &project.rooms {
        let area = project
            .room_usable_area(room)
            .or_else(|| project.room_area(room))
            .unwrap_or(0.0);
        if area <= 1e-4 {
            continue;
        }
        let perimeter = room_perimeter_m(project, room).max(4.0 * area.sqrt());
        // Exact generalization of K = (a·b) / (hm·(a+b)) = 2·S / (hm·P).
        let k = (2.0 * area) / (hm * perimeter);
        let fu = utilization_factor(k, settings.reflectance);
        let target = target_illuminance_for_room(room.display_name());
        let required_lumens = target.target_lux * area / (fu * fd);

        let mut luminaires = 0usize;
        let mut installed_lumens = 0.0;
        for dev in project.electrical_devices() {
            if dev.kind.role() != Some(DeviceRole::Lighting) {
                continue;
            }
            if project.room_for_fixture(dev).map(|r| r.id) != Some(room.id) {
                continue;
            }
            luminaires += 1;
            let default_lm = if dev.kind == FixtureKind::WallLight {
                wall_lm
            } else {
                ceil_lm
            };
            installed_lumens += dev
                .electrical
                .and_then(|d| d.lumens)
                .unwrap_or(default_lm)
                .max(0.0);
        }

        let calculated_lux = installed_lumens * fu * fd / area;
        let recommended = (required_lumens / ceil_lm).ceil().max(1.0) as usize;
        let watts = installed_lumens / efficacy;
        let power_density = watts / area;
        let utilisation = if calculated_lux > 1e-6 {
            target.target_lux / calculated_lux
        } else {
            f64::INFINITY
        };
        let light = Light::from_utilisation(utilisation);
        let plain_note = if luminaires == 0 {
            format!(
                "{} está sem ponto de luz: coloque ao menos {} luminária(s) de {:.0} lm para atingir {:.0} lux.",
                room.display_name(),
                recommended,
                ceil_lm,
                target.target_lux
            )
        } else if light == Light::Red {
            format!(
                "{} ficou abaixo da norma ({:.0} lux para meta de {:.0} lux): use {} luminária(s) de {:.0} lm ou lâmpadas mais fortes ({:.0} lm no total).",
                room.display_name(),
                calculated_lux,
                target.target_lux,
                recommended,
                ceil_lm,
                required_lumens.ceil()
            )
        } else if calculated_lux > target.target_lux * 3.0 {
            format!(
                "{} atende ({:.0} lux para meta de {:.0} lux), mas com iluminação bem acima da meta: avalie lâmpadas menores para evitar ofuscamento.",
                room.display_name(),
                calculated_lux,
                target.target_lux
            )
        } else if light == Light::Yellow {
            format!(
                "{} atende no limite ({:.0} lux para meta de {:.0} lux): qualquer sujeira ou lâmpada mais fraca pode deixar o ambiente escuro.",
                room.display_name(),
                calculated_lux,
                target.target_lux
            )
        } else {
            format!(
                "{} bem iluminado ({:.0} lux para meta de {:.0} lux).",
                room.display_name(),
                calculated_lux,
                target.target_lux
            )
        };

        let anchor = project
            .room_label_anchor(room)
            .map(|(p, _, _)| p)
            .or_else(|| {
                project
                    .room_bounds(room)
                    .map(|(min, max)| min.midpoint(max))
            });

        report.total_installed_lumens += installed_lumens;
        report.total_estimated_watts += watts;
        if light == Light::Red {
            report.rooms_failing += 1;
        } else {
            report.rooms_passing += 1;
        }
        report.rooms.push(RoomLightingResult {
            room_id: room.id,
            room_name: room.display_name().to_owned(),
            category: target.category.to_owned(),
            area_m2: area,
            perimeter_m: perimeter,
            mounting_height_m: hm,
            room_index_k: k,
            utilization_factor: fu,
            maintenance_factor: fd,
            target_lux: target.target_lux,
            clause: target.clause.to_owned(),
            luminaires,
            installed_lumens,
            required_lumens,
            calculated_lux,
            recommended_luminaires: recommended,
            power_density_w_m2: power_density,
            utilisation,
            light,
            plain_note,
            anchor,
        });
    }

    report.assumptions = vec![
        format!(
            "Método dos Lúmens (CIE / ABNT NBR ISO/CIE 8995-1:2013): plano de trabalho a {} m, altura útil hm = {} m, refletâncias {} e fator de manutenção Fd = {}.",
            decimal(settings.work_plane_m, 2),
            decimal(hm, 2),
            settings.reflectance.short(),
            decimal(fd, 2)
        ),
        format!(
            "Fluxo padrão por ponto (quando não informado no ponto): {:.0} lm no teto e {:.0} lm em arandela (eficiência estimada {:.0} lm/W em LED).",
            ceil_lm, wall_lm, efficacy
        ),
        "Pré-dimensionamento luminotécnico médio no plano de trabalho: não substitui simulação fotométrica ponto a ponto (IES/LDT) para ambientes especiais.".to_owned(),
    ];

    report
}

/// Plan overlay labels showing each room's maintained illuminance (lux) on
/// screen and in exported electrical sheets (*screen equals paper*, M7).
pub fn lighting_plan_marks(project: &Project) -> Vec<PlanMark> {
    if !project.electrical.lighting.show_plan_overlay {
        return Vec::new();
    }
    let has_lights = project
        .electrical_devices()
        .any(|d| d.kind.role() == Some(DeviceRole::Lighting));
    if !has_lights {
        return Vec::new();
    }
    let report = lighting_report(project);
    let mut marks = Vec::new();
    for room in &report.rooms {
        let Some(anchor) = room.anchor else {
            continue;
        };
        let text = if room.light == Light::Red {
            format!("{:.0}/{:.0} lx !", room.calculated_lux, room.target_lux)
        } else {
            format!("{:.0} lx", room.calculated_lux)
        };
        // Offset cleanly below the room name/dimensions label.
        marks.push(PlanMark::Text {
            anchor: Point::new(anchor.x, anchor.y + 0.42),
            text,
        });
    }
    marks
}

/// UTF-8 BOM CSV export of the room lighting study.
pub fn lighting_report_csv(project: &Project) -> String {
    let report = lighting_report(project);
    let mut csv = String::from(
        "\u{feff}comodo,categoria,area_m2,indice_k,fu,fd,luminarias,fluxo_instalado_lm,fluxo_necessario_lm,iluminancia_lux,meta_lux,recomendado_un,densidade_w_m2,semaforo\r\n",
    );
    for r in &report.rooms {
        let _ = writeln!(
            csv,
            "\"{}\",\"{}\",{:.2},{:.2},{:.2},{:.2},{},{:.0},{:.0},{:.0},{:.0},{},{:.1},{}\r",
            r.room_name.replace('"', "\"\""),
            r.category.replace('"', "\"\""),
            r.area_m2,
            r.room_index_k,
            r.utilization_factor,
            r.maintenance_factor,
            r.luminaires,
            r.installed_lumens,
            r.required_lumens,
            r.calculated_lux,
            r.target_lux,
            r.recommended_luminaires,
            r.power_density_w_m2,
            r.light.label()
        );
    }
    csv
}

/// Printable A3 landscape SVG sheet of the room lighting calculation (#125).
pub fn lighting_report_svg(project: &Project) -> String {
    let report = lighting_report(project);
    let width = 420.0;
    let height = (95.0 + report.rooms.len() as f64 * 8.0 + 48.0).max(297.0);
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

    let text = |svg: &mut String, x: f64, y: f64, size: f64, bold: bool, color: &str, value: &str| {
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
        "CÁLCULO LUMINOTÉCNICO · MÉTODO DOS LÚMENS (NBR ISO/CIE 8995-1)",
    );
    text(
        &mut svg,
        18.0,
        28.5,
        3.0,
        false,
        muted,
        &format!(
            "{} · {} ambiente(s) ({} atendem, {} abaixo da meta) · Fluxo total: {:.0} lm (~{:.0} W LED)",
            project.name,
            report.rooms.len(),
            report.rooms_passing,
            report.rooms_failing,
            report.total_installed_lumens,
            report.total_estimated_watts
        ),
    );

    let cols: [(&str, f64); 11] = [
        ("Cômodo", 18.0),
        ("Categoria", 64.0),
        ("Área", 118.0),
        ("K", 138.0),
        ("Fu", 154.0),
        ("Pontos", 170.0),
        ("Fluxo (lm)", 190.0),
        ("Calc. (lx)", 222.0),
        ("Meta (lx)", 250.0),
        ("Rec.", 278.0),
        ("Situação", 300.0),
    ];
    let mut y = 40.0;
    let _ = writeln!(
        svg,
        "<rect x=\"16\" y=\"{:.1}\" width=\"388\" height=\"7.0\" fill=\"#edf2f7\" stroke=\"{ink}\" stroke-width=\"0.3\"/>",
        y - 4.8
    );
    for (title, x) in cols {
        text(&mut svg, x, y, 2.7, true, ink, title);
    }
    y += 7.5;

    for r in &report.rooms {
        let color = match r.light {
            Light::Green => "#2e7d32",
            Light::Yellow => "#b85d00",
            Light::Red => "#c62828",
        };
        text(&mut svg, cols[0].1, y, 2.6, true, ink, &r.room_name);
        text(&mut svg, cols[1].1, y, 2.4, false, muted, &r.category);
        text(&mut svg, cols[2].1, y, 2.5, false, ink, &format!("{} m²", decimal(r.area_m2, 1)));
        text(&mut svg, cols[3].1, y, 2.5, false, ink, &decimal(r.room_index_k, 2));
        text(&mut svg, cols[4].1, y, 2.5, false, ink, &decimal(r.utilization_factor, 2));
        text(&mut svg, cols[5].1, y, 2.5, false, ink, &r.luminaires.to_string());
        text(&mut svg, cols[6].1, y, 2.5, false, ink, &format!("{:.0}", r.installed_lumens));
        text(&mut svg, cols[7].1, y, 2.6, true, color, &format!("{:.0} lx", r.calculated_lux));
        text(&mut svg, cols[8].1, y, 2.5, false, ink, &format!("{:.0} lx", r.target_lux));
        text(&mut svg, cols[9].1, y, 2.5, false, ink, &format!("{} un", r.recommended_luminaires));
        text(&mut svg, cols[10].1, y, 2.5, true, color, r.light.label());
        let _ = writeln!(
            svg,
            "<line x1=\"16\" y1=\"{:.1}\" x2=\"404\" y2=\"{:.1}\" stroke=\"#d0dae5\" stroke-width=\"0.2\"/>",
            y + 2.4,
            y + 2.4
        );
        y += 7.5;
    }

    y += 6.0;
    text(
        &mut svg,
        18.0,
        y,
        2.7,
        true,
        ink,
        "Fórmulas: K = 2·S / (hm·P) · Φ_req = (Em · S) / (Fu · Fd) · E_calc = (Φ_inst · Fu · Fd) / S",
    );
    y += 5.5;
    for note in &report.assumptions {
        text(&mut svg, 18.0, y, 2.3, false, muted, note);
        y += 4.2;
    }

    svg.push_str("</svg>\n");
    svg
}

impl Project {
    /// ABNT NBR ISO/CIE 8995-1 Lumen Method lighting report (#125).
    pub fn lighting_report(&self) -> LightingReport {
        lighting_report(self)
    }

    /// Room illuminance plan marks (lux badges) for screen and paper (#125).
    pub fn lighting_plan_marks(&self) -> Vec<PlanMark> {
        lighting_plan_marks(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::samples::SampleProject;

    /// Worked example 1: 4 × 5 m office (`hm = 2.05 m`, Light 70/50/20, `Fd = 0.80`, target 500 lx).
    /// Hand calculation:
    ///   S = 20 m², P = 18 m -> K = 40 / (2.05 * 18) = 1.084
    ///   Fu(1.084, Light) = 0.56 + (1.084 - 1.00)/0.25 * (0.62 - 0.56) = 0.5802
    ///   Φ_req = 500 * 20 / (0.5802 * 0.80) = 21545 lm
    ///   With 6 × 4000 lm LED panels (24000 lm): E_calc = 24000 * 0.5802 * 0.80 / 20 = 557 lx (≥ 500 lx).
    #[test]
    fn worked_example_office_matches_lumen_method_hand_calculation() {
        let mut project = Project::default();
        project.wall_height = 2.80;
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(15.0, 15.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Escritório")
            .unwrap();
        let (w, d) = FixtureKind::CeilingLight.default_size();
        for &(x, y) in &[
            (1.2, 1.0),
            (2.5, 1.0),
            (3.8, 1.0),
            (1.2, 3.0),
            (2.5, 3.0),
            (3.8, 3.0),
        ] {
            let id = project
                .add_fixture(FixtureKind::CeilingLight, Point::new(x, y), w, d, 0)
                .unwrap();
            if let Some(f) = project.fixtures.iter_mut().find(|f| f.id == id)
                && let Some(e) = f.electrical.as_mut()
            {
                e.lumens = Some(4000.0);
            }
        }
        let report = project.lighting_report();
        assert_eq!(report.rooms.len(), 1);
        let r = &report.rooms[0];
        assert!((r.target_lux - 500.0).abs() < 1e-6);
        let expected_k = (2.0 * r.area_m2) / (2.05 * r.perimeter_m);
        let expected_fu = utilization_factor(expected_k, SurfaceReflectance::Light);
        let expected_lux = 24_000.0 * expected_fu * 0.80 / r.area_m2;
        assert!((r.room_index_k - expected_k).abs() < 1e-6);
        assert!((r.utilization_factor - expected_fu).abs() < 1e-6);
        assert!((r.calculated_lux - expected_lux).abs() < 1e-4);
        // Check pure textbook 5x4 m clear values directly via utilization_factor.
        let textbook_k = 20.0 / (2.05 * 9.0);
        let textbook_fu = utilization_factor(textbook_k, SurfaceReflectance::Light);
        assert!((textbook_fu - 0.5802).abs() < 0.002);
        assert!(r.calculated_lux > 500.0);
        assert!(r.light != Light::Red);
    }

    /// Worked example 2: 3 × 4 m kitchen (`hm = 2.05 m`, Medium 50/30/10, `Fd = 0.75`, target 300 lx).
    #[test]
    fn worked_example_underlit_kitchen_turns_red_and_recommends_luminaires() {
        let mut project = Project::default();
        project.wall_height = 2.80;
        project.electrical.lighting.reflectance = SurfaceReflectance::Medium;
        project.electrical.lighting.maintenance_factor = 0.75;
        project.set_terrain(Point::new(-1.0, -1.0), Point::new(15.0, 15.0));
        project
            .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Cozinha")
            .unwrap();
        let (w, d) = FixtureKind::CeilingLight.default_size();
        project
            .add_fixture(FixtureKind::CeilingLight, Point::new(2.0, 1.5), w, d, 0)
            .unwrap();

        let report = project.lighting_report();
        let r = &report.rooms[0];
        assert!((r.target_lux - 300.0).abs() < 1e-6);
        let expected_k = (2.0 * r.area_m2) / (2.05 * r.perimeter_m);
        let expected_fu = utilization_factor(expected_k, SurfaceReflectance::Medium);
        let expected_lux = 1600.0 * expected_fu * 0.75 / r.area_m2;
        assert!((r.room_index_k - expected_k).abs() < 1e-6);
        assert!((r.calculated_lux - expected_lux).abs() < 1e-4);
        assert!(r.calculated_lux < 80.0);
        assert_eq!(r.light, Light::Red);
        assert!(r.recommended_luminaires >= 5);
        assert!(r.plain_note.contains("abaixo da norma"));
    }

    /// M7 criterion: every room in the sample house shows its lighting level and exports SVG/CSV.
    #[test]
    fn every_room_in_sample_house_shows_lighting_level_and_plan_marks() {
        let mut project = SampleProject::CasaTerrea.build();
        project.generate_electrical_points();

        let report = project.lighting_report();
        assert_eq!(report.rooms.len(), project.rooms.len());
        assert!(report.rooms.iter().all(|r| r.calculated_lux > 0.0));
        let marks = project.lighting_plan_marks();
        assert_eq!(marks.len(), project.rooms.len());
        assert!(marks.iter().all(|m| matches!(m, PlanMark::Text { text, .. } if text.contains("lx"))));

        let svg = lighting_report_svg(&project);
        assert!(svg.contains("CÁLCULO LUMINOTÉCNICO"));
        assert!(svg.contains("</svg>"));
        let csv = lighting_report_csv(&project);
        assert!(csv.starts_with("\u{feff}comodo,categoria"));
    }
}
