//! Construction cost estimate (#87): prices the stage-by-stage shopping list
//! (#136) using an editable Brazilian SINAPI-referenced price table (materials
//! and optional labour + BDI), supports direct SINAPI CSV/TSV import, builds
//! per-stage subtotals, cost per m² (CUB comparison) and ABC curve, and
//! exports CSV / A3 SVG.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::domain::{
    construction::shopping::{ConstructionStage, ShoppingItem, build_stage_shopping_list},
    model::Project,
};

/// Persisted custom or SINAPI-imported unit price for one `item_code`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PriceOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material_brl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labor_brl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sinapi_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl PriceOverride {
    pub fn is_empty(&self) -> bool {
        self.material_brl.is_none()
            && self.labor_brl.is_none()
            && self.sinapi_code.is_none()
            && self.source.is_none()
    }
}

fn default_include_labor() -> bool {
    true
}

fn default_sinapi_reference() -> String {
    "SINAPI referência nacional (edite com preços locais)".to_owned()
}

/// Persisted cost estimate settings attached to `ConstructionPlan` (#87).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CostSettings {
    /// Indirect benefits & expenses (BDI, 0–40 %; default 0 % for self-builders).
    pub bdi_pct: f64,
    /// Include estimated site labour alongside materials (default `true`).
    #[serde(default = "default_include_labor")]
    pub include_labor: bool,
    /// Label of the active price table / imported SINAPI sheet.
    #[serde(default = "default_sinapi_reference")]
    pub sinapi_reference: String,
    /// Editable/imported unit prices keyed by `ShoppingItem::item_code`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub unit_prices: BTreeMap<String, PriceOverride>,
}

impl Default for CostSettings {
    fn default() -> Self {
        Self {
            bdi_pct: 0.0,
            include_labor: default_include_labor(),
            sinapi_reference: default_sinapi_reference(),
            unit_prices: BTreeMap::new(),
        }
    }
}

impl CostSettings {
    pub fn is_default(&self) -> bool {
        self.bdi_pct.abs() < 1e-9
            && self.include_labor
            && self.sinapi_reference == default_sinapi_reference()
            && self.unit_prices.is_empty()
    }
}

/// Default reference price row `(sinapi_code, material_brl, labor_brl)`.
#[derive(Clone, Copy, Debug)]
struct DefaultPrice {
    sinapi_code: &'static str,
    material_brl: f64,
    labor_brl: f64,
}

fn default_price_for(item_code: &str) -> DefaultPrice {
    match item_code {
        "cimento_50kg" => DefaultPrice {
            sinapi_code: "00001379",
            material_brl: 38.50,
            labor_brl: 18.00,
        },
        "cal_20kg" => DefaultPrice {
            sinapi_code: "00001106",
            material_brl: 18.00,
            labor_brl: 7.00,
        },
        "areia_m3" => DefaultPrice {
            sinapi_code: "00000370",
            material_brl: 145.00,
            labor_brl: 45.00,
        },
        "brita_m3" => DefaultPrice {
            sinapi_code: "00004721",
            material_brl: 165.00,
            labor_brl: 45.00,
        },
        "aco_ca50_kg" => DefaultPrice {
            sinapi_code: "00000034",
            material_brl: 9.80,
            labor_brl: 5.20,
        },
        "arame_recozido_kg" => DefaultPrice {
            sinapi_code: "00000337",
            material_brl: 16.50,
            labor_brl: 3.50,
        },
        "forma_madeira_m2" => DefaultPrice {
            sinapi_code: "00001349",
            material_brl: 62.00,
            labor_brl: 48.00,
        },
        "escora_eucalipto_un" => DefaultPrice {
            sinapi_code: "00004514",
            material_brl: 24.00,
            labor_brl: 12.00,
        },
        "tabua_gabarito_m" => DefaultPrice {
            sinapi_code: "00006191",
            material_brl: 11.50,
            labor_brl: 9.50,
        },
        "pontalete_gabarito_un" => DefaultPrice {
            sinapi_code: "00004491",
            material_brl: 18.00,
            labor_brl: 10.00,
        },
        "impermeabilizante_baldrame_l" => DefaultPrice {
            sinapi_code: "00007334",
            material_brl: 24.00,
            labor_brl: 14.00,
        },
        "bloco_alvenaria_un" => DefaultPrice {
            sinapi_code: "00007271",
            material_brl: 2.30,
            labor_brl: 1.95,
        },
        "meio_bloco_un" => DefaultPrice {
            sinapi_code: "00007272",
            material_brl: 1.65,
            labor_brl: 1.35,
        },
        "canaleta_un" => DefaultPrice {
            sinapi_code: "00037595",
            material_brl: 3.40,
            labor_brl: 2.20,
        },
        "madeira_telhado_m3" => DefaultPrice {
            sinapi_code: "00004417",
            material_brl: 3200.00,
            labor_brl: 1650.00,
        },
        "telha_un" => DefaultPrice {
            sinapi_code: "00007177",
            material_brl: 3.80,
            labor_brl: 2.40,
        },
        "telha_m2" => DefaultPrice {
            sinapi_code: "00007189",
            material_brl: 58.00,
            labor_brl: 32.00,
        },
        "calha_m" => DefaultPrice {
            sinapi_code: "00001175",
            material_brl: 72.00,
            labor_brl: 38.00,
        },
        "eletroduto_m" => DefaultPrice {
            sinapi_code: "00002692",
            material_brl: 3.60,
            labor_brl: 4.50,
        },
        "caixa_eletrica_un" => DefaultPrice {
            sinapi_code: "00001872",
            material_brl: 8.50,
            labor_brl: 14.00,
        },
        "disjuntor_un" => DefaultPrice {
            sinapi_code: "00002375",
            material_brl: 26.00,
            labor_brl: 22.00,
        },
        "dr_dps_un" => DefaultPrice {
            sinapi_code: "00039453",
            material_brl: 125.00,
            labor_brl: 35.00,
        },
        "ponto_acabamento_un" => DefaultPrice {
            sinapi_code: "00007567",
            material_brl: 22.00,
            labor_brl: 18.00,
        },
        "caixa_dagua_un" => DefaultPrice {
            sinapi_code: "00000786",
            material_brl: 540.00,
            labor_brl: 260.00,
        },
        "conexao_pvc_un" => DefaultPrice {
            sinapi_code: "00003527",
            material_brl: 8.50,
            labor_brl: 9.00,
        },
        "tubo_pvc_esgoto_m" => DefaultPrice {
            sinapi_code: "00009835",
            material_brl: 18.50,
            labor_brl: 21.00,
        },
        "impermeabilizante_kg" => DefaultPrice {
            sinapi_code: "00036149",
            material_brl: 12.50,
            labor_brl: 11.00,
        },
        "piso_m2" => DefaultPrice {
            sinapi_code: "00000531",
            material_brl: 54.00,
            labor_brl: 46.00,
        },
        "argamassa_colante_20kg" => DefaultPrice {
            sinapi_code: "00001381",
            material_brl: 29.00,
            labor_brl: 8.00,
        },
        "rejunte_kg" => DefaultPrice {
            sinapi_code: "00034357",
            material_brl: 9.50,
            labor_brl: 4.50,
        },
        "rodape_m" => DefaultPrice {
            sinapi_code: "00000537",
            material_brl: 14.50,
            labor_brl: 12.00,
        },
        "tinta_18l" => DefaultPrice {
            sinapi_code: "00007356",
            material_brl: 340.00,
            labor_brl: 280.00,
        },
        code if code.starts_with("cabo_cobre_") => {
            let sec10: u32 = code
                .trim_start_matches("cabo_cobre_")
                .trim_end_matches("_m")
                .parse()
                .unwrap_or(25);
            let mat = match sec10 {
                0..=15 => 2.10,
                16..=25 => 3.30,
                26..=40 => 5.40,
                41..=60 => 8.10,
                61..=100 => 13.50,
                _ => 21.00,
            };
            DefaultPrice {
                sinapi_code: match sec10 {
                    0..=15 => "00000980",
                    16..=25 => "00000981",
                    26..=40 => "00000982",
                    41..=60 => "00000983",
                    _ => "00000991",
                },
                material_brl: mat,
                labor_brl: 2.80,
            }
        }
        code if code.starts_with("tubo_pvc_af_") => {
            let dn: u32 = code
                .trim_start_matches("tubo_pvc_af_")
                .trim_end_matches("_m")
                .parse()
                .unwrap_or(25);
            let mat = match dn {
                0..=20 => 6.80,
                21..=25 => 8.90,
                26..=32 => 14.50,
                33..=40 => 19.80,
                _ => 26.00,
            };
            DefaultPrice {
                sinapi_code: match dn {
                    0..=20 => "00009867",
                    21..=25 => "00009868",
                    26..=32 => "00009869",
                    _ => "00009870",
                },
                material_brl: mat,
                labor_brl: 14.50,
            }
        }
        _ => DefaultPrice {
            sinapi_code: "00000000",
            material_brl: 20.00,
            labor_brl: 12.00,
        },
    }
}

/// Maps a normalized SINAPI numeric code (leading zeros stripped) to a Trazo `item_code`.
fn map_sinapi_code(code: &str) -> Option<&'static str> {
    let trimmed = code.trim().trim_start_matches('0');
    match trimmed {
        "1379" | "88316" => Some("cimento_50kg"),
        "1106" => Some("cal_20kg"),
        "370" | "367" => Some("areia_m3"),
        "4721" | "4720" => Some("brita_m3"),
        "34" | "33" | "32" | "92762" => Some("aco_ca50_kg"),
        "337" => Some("arame_recozido_kg"),
        "1349" | "1352" => Some("forma_madeira_m2"),
        "4514" => Some("escora_eucalipto_un"),
        "6191" => Some("tabua_gabarito_m"),
        "4491" => Some("pontalete_gabarito_un"),
        "7334" => Some("impermeabilizante_baldrame_l"),
        "7271" | "7266" | "7269" => Some("bloco_alvenaria_un"),
        "7272" => Some("meio_bloco_un"),
        "37595" => Some("canaleta_un"),
        "4417" => Some("madeira_telhado_m3"),
        "7177" | "7180" => Some("telha_un"),
        "7189" => Some("telha_m2"),
        "1175" => Some("calha_m"),
        "980" => Some("cabo_cobre_15_m"),
        "981" => Some("cabo_cobre_25_m"),
        "982" => Some("cabo_cobre_40_m"),
        "983" => Some("cabo_cobre_60_m"),
        "991" => Some("cabo_cobre_100_m"),
        "2692" => Some("eletroduto_m"),
        "1872" => Some("caixa_eletrica_un"),
        "2375" => Some("disjuntor_un"),
        "39453" => Some("dr_dps_un"),
        "7567" => Some("ponto_acabamento_un"),
        "786" => Some("caixa_dagua_un"),
        "9867" => Some("tubo_pvc_af_20_m"),
        "9868" => Some("tubo_pvc_af_25_m"),
        "9869" => Some("tubo_pvc_af_32_m"),
        "9870" => Some("tubo_pvc_af_50_m"),
        "3527" => Some("conexao_pvc_un"),
        "9835" => Some("tubo_pvc_esgoto_m"),
        "36149" => Some("impermeabilizante_kg"),
        "531" => Some("piso_m2"),
        "1381" => Some("argamassa_colante_20kg"),
        "34357" => Some("rejunte_kg"),
        "537" => Some("rodape_m"),
        "7356" => Some("tinta_18l"),
        _ => None,
    }
}

fn map_sinapi_description(desc: &str) -> Option<&'static str> {
    let lower = desc.to_lowercase();
    if lower.contains("cimento") && lower.contains("50") {
        Some("cimento_50kg")
    } else if lower.contains("cal hidratada") {
        Some("cal_20kg")
    } else if lower.contains("areia") && (lower.contains("media") || lower.contains("média")) {
        Some("areia_m3")
    } else if lower.contains("brita") || lower.contains("pedrisco") {
        Some("brita_m3")
    } else if lower.contains("ca-50") || lower.contains("ca50") {
        Some("aco_ca50_kg")
    } else if lower.contains("arame recozido") {
        Some("arame_recozido_kg")
    } else if lower.contains("canaleta") {
        Some("canaleta_un")
    } else if lower.contains("bloco cer") || lower.contains("bloco de concreto") {
        Some("bloco_alvenaria_un")
    } else if lower.contains("argamassa colante") {
        Some("argamassa_colante_20kg")
    } else if lower.contains("tinta acril") || lower.contains("tinta acrílica") {
        Some("tinta_18l")
    } else if lower.contains("porcelanato") || lower.contains("piso cer") {
        Some("piso_m2")
    } else if lower.contains("eletroduto") {
        Some("eletroduto_m")
    } else if lower.contains("caixa d'agua") || lower.contains("caixa d'água") {
        Some("caixa_dagua_un")
    } else {
        None
    }
}

/// Parses a Brazilian or US decimal string (e.g. `"R$ 1.234,56"`, `"38,50"`, `"38.50"`).
pub fn parse_brl_price(raw: &str) -> Option<f64> {
    let cleaned = raw
        .trim()
        .trim_matches('"')
        .replace("R$", "")
        .replace("r$", "")
        .replace(' ', "");
    if cleaned.is_empty() {
        return None;
    }
    let normalized = if cleaned.contains(',') && cleaned.contains('.') {
        // Brazilian format `1.234,56`
        cleaned.replace('.', "").replace(',', ".")
    } else if cleaned.contains(',') {
        cleaned.replace(',', ".")
    } else {
        cleaned
    };
    normalized
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
}

/// Summary returned after importing a SINAPI CSV/TSV price table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SinapiImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub reference_label: String,
    pub matched_codes: Vec<String>,
}

/// Imports a SINAPI-style delimited table (CSV/TSV with `;`, `,` or `\t`)
/// into `project.construction.cost.unit_prices` (#87).
///
/// Expected columns per line (header optional):
/// `codigo;descricao;unidade;preco_material[;preco_mao_obra]`
/// or `item_code;preco_material[;preco_mao_obra]`.
pub fn import_sinapi_table(
    project: &mut Project,
    csv_text: &str,
    reference_label: Option<&str>,
) -> Result<SinapiImportResult, String> {
    let body = csv_text.trim_start_matches('\u{feff}');
    if body.trim().is_empty() {
        return Err("O arquivo SINAPI está vazio.".to_owned());
    }
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut matched_codes = Vec::new();
    let label = reference_label
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Tabela SINAPI importada")
        .to_owned();

    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let delim = if line.contains(';') {
            ';'
        } else if line.contains('\t') {
            '\t'
        } else {
            ','
        };
        let cols: Vec<&str> = line
            .split(delim)
            .map(|c| c.trim().trim_matches('"'))
            .collect();
        if cols.len() < 2 {
            skipped += 1;
            continue;
        }
        // Skip header rows.
        let first_low = cols[0].to_lowercase();
        if first_low.contains("codigo")
            || first_low.contains("código")
            || first_low == "cod"
            || first_low == "item"
        {
            continue;
        }

        // Case A: short 2/3-column `item_code;mat_brl[;labor_brl]` or `sinapi_code;mat_brl`
        let (target_code, sinapi_code, mat_price, labor_price) = if cols.len() <= 3 {
            let Some(price) = parse_brl_price(cols[1]) else {
                skipped += 1;
                continue;
            };
            let labor = cols.get(2).and_then(|c| parse_brl_price(c));
            if let Some(mapped) = map_sinapi_code(cols[0]) {
                (mapped.to_owned(), Some(cols[0].to_owned()), price, labor)
            } else {
                (cols[0].to_lowercase(), None, price, labor)
            }
        } else {
            // Case B: standard SINAPI `codigo;descricao;unidade;preco[;mao_obra]`
            let price =
                parse_brl_price(cols[3]).or_else(|| cols.get(4).and_then(|c| parse_brl_price(c)));
            let Some(mat) = price else {
                skipped += 1;
                continue;
            };
            let labor = cols.get(4).and_then(|c| parse_brl_price(c));
            let mapped = map_sinapi_code(cols[0])
                .or_else(|| map_sinapi_description(cols[1]))
                .map(str::to_owned);
            let Some(item_code) = mapped else {
                skipped += 1;
                continue;
            };
            (item_code, Some(cols[0].to_owned()), mat, labor)
        };

        let entry = project
            .construction
            .cost
            .unit_prices
            .entry(target_code.clone())
            .or_default();
        entry.material_brl = Some(mat_price);
        if let Some(lb) = labor_price {
            entry.labor_brl = Some(lb);
        }
        if let Some(sc) = sinapi_code {
            entry.sinapi_code = Some(sc);
        }
        entry.source = Some(label.clone());
        matched_codes.push(target_code);
        imported += 1;
    }

    if imported == 0 {
        return Err(
            "Nenhuma linha válida foi reconhecida na tabela SINAPI. Use formato 'codigo;descricao;unidade;preco' ou 'codigo_item;preco'.".to_owned(),
        );
    }

    project.construction.cost.sinapi_reference = label.clone();
    Ok(SinapiImportResult {
        imported,
        skipped,
        reference_label: label,
        matched_codes,
    })
}

/// Edits one item's unit material and/or labour price in `Project::construction.cost`.
pub fn edit_unit_price(
    project: &mut Project,
    item_code: &str,
    material_brl: Option<f64>,
    labor_brl: Option<f64>,
) -> Result<String, String> {
    if item_code.trim().is_empty() {
        return Err("Informe o código do item para editar o preço.".to_owned());
    }
    if let Some(m) = material_brl
        && (!m.is_finite() || m < 0.0)
    {
        return Err("O preço unitário do material deve ser um valor ≥ 0.".to_owned());
    }
    if let Some(l) = labor_brl
        && (!l.is_finite() || l < 0.0)
    {
        return Err("O custo de mão de obra unitário deve ser um valor ≥ 0.".to_owned());
    }
    let code = item_code.trim().to_lowercase();
    let entry = project
        .construction
        .cost
        .unit_prices
        .entry(code.clone())
        .or_default();
    if let Some(m) = material_brl {
        entry.material_brl = Some(m);
    }
    if let Some(l) = labor_brl {
        entry.labor_brl = Some(l);
    }
    entry.source = Some("Editado manualmente".to_owned());
    if entry.is_empty() {
        project.construction.cost.unit_prices.remove(&code);
    }
    Ok(format!("Preço unitário de '{code}' atualizado."))
}

/// One priced line item in the cost estimate.
#[derive(Clone, Debug, PartialEq)]
pub struct PricedItem {
    pub item_code: String,
    pub sinapi_code: String,
    pub stage: ConstructionStage,
    pub discipline: &'static str,
    pub description: String,
    pub buy_quantity: f64,
    pub unit: String,
    pub material_unit_brl: f64,
    pub labor_unit_brl: f64,
    pub material_total_brl: f64,
    pub labor_total_brl: f64,
    pub direct_total_brl: f64,
    pub total_with_bdi_brl: f64,
    pub source: String,
}

/// Aggregated cost for one construction stage.
#[derive(Clone, Debug, PartialEq)]
pub struct StageCostSummary {
    pub stage: ConstructionStage,
    pub item_count: usize,
    pub material_brl: f64,
    pub labor_brl: f64,
    pub direct_brl: f64,
    pub bdi_brl: f64,
    pub total_brl: f64,
    pub share_pct: f64,
}

/// One entry in the ABC curve (grouped by `item_code`, descending total cost).
#[derive(Clone, Debug, PartialEq)]
pub struct AbcCurveRow {
    pub item_code: String,
    pub description: String,
    pub unit: String,
    pub total_quantity: f64,
    pub total_brl: f64,
    pub share_pct: f64,
    pub cumulative_pct: f64,
    pub class: char,
}

/// Complete construction cost estimate report (#87).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CostEstimateReport {
    pub reference_label: String,
    pub include_labor: bool,
    pub bdi_pct: f64,
    pub built_area_m2: f64,
    pub material_total_brl: f64,
    pub labor_total_brl: f64,
    pub direct_total_brl: f64,
    pub bdi_total_brl: f64,
    pub grand_total_brl: f64,
    pub cost_per_m2_brl: f64,
    pub stages: Vec<StageCostSummary>,
    pub items: Vec<PricedItem>,
    pub abc_curve: Vec<AbcCurveRow>,
    pub plain_summary: String,
    pub assumptions: Vec<String>,
}

fn price_shopping_item(item: &ShoppingItem, settings: &CostSettings) -> PricedItem {
    let def = default_price_for(&item.item_code);
    let ov = settings.unit_prices.get(&item.item_code);
    let mat_u = ov.and_then(|o| o.material_brl).unwrap_or(def.material_brl);
    let lab_u = if settings.include_labor {
        ov.and_then(|o| o.labor_brl).unwrap_or(def.labor_brl)
    } else {
        0.0
    };
    let sinapi = ov
        .and_then(|o| o.sinapi_code.clone())
        .unwrap_or_else(|| def.sinapi_code.to_owned());
    let source = ov
        .and_then(|o| o.source.clone())
        .unwrap_or_else(|| "SINAPI ref.".to_owned());
    let mat_tot = item.buy_quantity * mat_u;
    let lab_tot = item.buy_quantity * lab_u;
    let direct = mat_tot + lab_tot;
    let bdi_mult = 1.0 + settings.bdi_pct.clamp(0.0, 50.0) / 100.0;
    PricedItem {
        item_code: item.item_code.clone(),
        sinapi_code: sinapi,
        stage: item.stage,
        discipline: item.discipline,
        description: item.description.clone(),
        buy_quantity: item.buy_quantity,
        unit: item.unit.clone(),
        material_unit_brl: mat_u,
        labor_unit_brl: lab_u,
        material_total_brl: mat_tot,
        labor_total_brl: lab_tot,
        direct_total_brl: direct,
        total_with_bdi_brl: direct * bdi_mult,
        source,
    }
}

/// Computes the complete cost estimate from the stage shopping list (#136) and
/// the project's editable / SINAPI-imported price table (#87).
pub fn build_cost_estimate(project: &Project) -> CostEstimateReport {
    let shopping = build_stage_shopping_list(project);
    let settings = &project.construction.cost;
    let built_area_m2: f64 = project.rooms.iter().map(|r| r.area()).sum::<f64>().max(1.0);

    let mut items = Vec::new();
    let mut stage_map: BTreeMap<ConstructionStage, (usize, f64, f64)> = BTreeMap::new();
    let mut abc_agg: BTreeMap<String, (String, String, f64, f64)> = BTreeMap::new();

    for group in &shopping.stages {
        for raw in &group.items {
            let priced = price_shopping_item(raw, settings);
            let entry = stage_map.entry(group.stage).or_insert((0, 0.0, 0.0));
            entry.0 += 1;
            entry.1 += priced.material_total_brl;
            entry.2 += priced.labor_total_brl;

            let abc = abc_agg
                .entry(priced.item_code.clone())
                .or_insert_with(|| (priced.description.clone(), priced.unit.clone(), 0.0, 0.0));
            abc.2 += priced.buy_quantity;
            abc.3 += priced.total_with_bdi_brl;

            items.push(priced);
        }
    }

    let bdi_frac = settings.bdi_pct.clamp(0.0, 50.0) / 100.0;
    let material_total_brl: f64 = items.iter().map(|i| i.material_total_brl).sum();
    let labor_total_brl: f64 = items.iter().map(|i| i.labor_total_brl).sum();
    let direct_total_brl = material_total_brl + labor_total_brl;
    let bdi_total_brl = direct_total_brl * bdi_frac;
    let grand_total_brl = direct_total_brl + bdi_total_brl;
    let cost_per_m2_brl = grand_total_brl / built_area_m2;

    let stages: Vec<StageCostSummary> = ConstructionStage::ALL
        .into_iter()
        .filter_map(|stage| {
            let (count, mat, lab) = stage_map.get(&stage).copied()?;
            let direct = mat + lab;
            let bdi = direct * bdi_frac;
            let total = direct + bdi;
            let share = if grand_total_brl > 1e-6 {
                100.0 * total / grand_total_brl
            } else {
                0.0
            };
            Some(StageCostSummary {
                stage,
                item_count: count,
                material_brl: mat,
                labor_brl: lab,
                direct_brl: direct,
                bdi_brl: bdi,
                total_brl: total,
                share_pct: share,
            })
        })
        .collect();

    let mut abc_vec: Vec<(String, String, String, f64, f64)> = abc_agg
        .into_iter()
        .map(|(code, (desc, unit, qty, tot))| (code, desc, unit, qty, tot))
        .collect();
    abc_vec.sort_by(|a, b| b.4.total_cmp(&a.4));
    let mut cumulative = 0.0;
    let abc_curve: Vec<AbcCurveRow> = abc_vec
        .into_iter()
        .map(
            |(item_code, description, unit, total_quantity, total_brl)| {
                let share = if grand_total_brl > 1e-6 {
                    100.0 * total_brl / grand_total_brl
                } else {
                    0.0
                };
                cumulative += share;
                let class = if cumulative <= 80.0 + 1e-6 {
                    'A'
                } else if cumulative <= 95.0 + 1e-6 {
                    'B'
                } else {
                    'C'
                };
                AbcCurveRow {
                    item_code,
                    description,
                    unit,
                    total_quantity,
                    total_brl,
                    share_pct: share,
                    cumulative_pct: cumulative.min(100.0),
                    class,
                }
            },
        )
        .collect();

    let scope_txt = if settings.include_labor {
        "materiais + mão de obra estimada"
    } else {
        "apenas materiais (sem mão de obra)"
    };
    let plain_summary = format!(
        "Estimativa de {} ({scope_txt}, BDI {:.0} %): cerca de {} por m² para {:.1} m² de área útil.",
        format_brl(grand_total_brl),
        settings.bdi_pct,
        format_brl(cost_per_m2_brl),
        built_area_m2
    );

    CostEstimateReport {
        reference_label: settings.sinapi_reference.clone(),
        include_labor: settings.include_labor,
        bdi_pct: settings.bdi_pct,
        built_area_m2,
        material_total_brl,
        labor_total_brl,
        direct_total_brl,
        bdi_total_brl,
        grand_total_brl,
        cost_per_m2_brl,
        stages,
        items,
        abc_curve,
        plain_summary,
        assumptions: vec![
            format!("Base de preços: {}.", settings.sinapi_reference),
            "Orçamento paramétrico preliminar derivado das quantidades do modelo (#136): não inclui terreno, projetos/taxas, esquadrias sob medida, louças/metais de alto padrão nem terraplenagem pesada.".to_owned(),
            "Importe a planilha SINAPI da sua UF ou edite os preços unitários com orçamentos de lojas locais antes de fechar compras.".to_owned(),
        ],
    }
}

/// Formats a Brazilian Real amount (`R$ 12.345,67`).
pub fn format_brl(value: f64) -> String {
    let neg = value < 0.0;
    let abs = value.abs();
    let cents = (abs * 100.0).round() as u64;
    let int_part = cents / 100;
    let frac_part = cents % 100;
    let digits = int_part.to_string();
    let mut grouped = String::new();
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    let int_fmt: String = grouped.chars().rev().collect();
    if neg {
        format!("-R$ {int_fmt},{frac_part:02}")
    } else {
        format!("R$ {int_fmt},{frac_part:02}")
    }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// UTF-8 BOM CSV export of the cost estimate (#87).
pub fn cost_estimate_csv(project: &Project) -> String {
    let report = build_cost_estimate(project);
    let mut csv = String::from(
        "\u{feff}etapa,codigo,sinapi,descricao,quantidade,unidade,unit_material_brl,unit_mao_obra_brl,total_material_brl,total_mao_obra_brl,total_com_bdi_brl,fonte\r\n",
    );
    for it in &report.items {
        let _ = writeln!(
            csv,
            "\"{}\",{},{},\"{}\",{:.2},{},{:.2},{:.2},{:.2},{:.2},{:.2},\"{}\"\r",
            it.stage.label(),
            it.item_code,
            it.sinapi_code,
            it.description.replace('"', "\"\""),
            it.buy_quantity,
            it.unit,
            it.material_unit_brl,
            it.labor_unit_brl,
            it.material_total_brl,
            it.labor_total_brl,
            it.total_with_bdi_brl,
            it.source.replace('"', "\"\"")
        );
    }
    csv
}

/// Printable A3 landscape SVG sheet of the cost estimate by stage + ABC curve (#87).
pub fn cost_estimate_svg(project: &Project) -> String {
    let report = build_cost_estimate(project);
    let top_abc = report.abc_curve.len().min(15);
    let width = 420.0;
    let height = (115.0 + (report.stages.len() + top_abc) as f64 * 7.2 + 30.0).max(297.0);
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
        "ORÇAMENTO ESTIMATIVO DA OBRA (POR ETAPA E CURVA ABC)",
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
        2.5,
        false,
        muted,
        &format!(
            "Materiais: {} · Mão de obra: {} · BDI ({:.0} %): {} · Total: {} · Base: {}",
            format_brl(report.material_total_brl),
            format_brl(report.labor_total_brl),
            report.bdi_pct,
            format_brl(report.bdi_total_brl),
            format_brl(report.grand_total_brl),
            report.reference_label
        ),
    );

    // Table 1: By stage
    let mut y = 44.0;
    text(
        &mut svg,
        18.0,
        y,
        3.2,
        true,
        ink,
        "1. Resumo por etapa construtiva",
    );
    y += 6.5;
    let _ = writeln!(
        svg,
        "<rect x=\"16\" y=\"{:.1}\" width=\"388\" height=\"6.2\" fill=\"#edf2f7\" stroke=\"{ink}\" stroke-width=\"0.25\"/>",
        y - 4.2
    );
    for (title, x) in [
        ("Etapa", 18.0),
        ("Itens", 145.0),
        ("Materiais", 172.0),
        ("Mão de obra", 228.0),
        ("BDI", 284.0),
        ("Total etapa", 322.0),
        ("%", 384.0),
    ] {
        text(&mut svg, x, y, 2.4, true, ink, title);
    }
    y += 6.8;

    for st in &report.stages {
        text(&mut svg, 18.0, y, 2.5, true, ink, st.stage.label());
        text(
            &mut svg,
            145.0,
            y,
            2.4,
            false,
            muted,
            &st.item_count.to_string(),
        );
        text(
            &mut svg,
            172.0,
            y,
            2.4,
            false,
            ink,
            &format_brl(st.material_brl),
        );
        text(
            &mut svg,
            228.0,
            y,
            2.4,
            false,
            ink,
            &format_brl(st.labor_brl),
        );
        text(
            &mut svg,
            284.0,
            y,
            2.4,
            false,
            muted,
            &format_brl(st.bdi_brl),
        );
        text(
            &mut svg,
            322.0,
            y,
            2.5,
            true,
            ink,
            &format_brl(st.total_brl),
        );
        text(
            &mut svg,
            384.0,
            y,
            2.4,
            false,
            ink,
            &format!("{:.1}%", st.share_pct),
        );
        let _ = writeln!(
            svg,
            "<line x1=\"16\" y1=\"{:.1}\" x2=\"404\" y2=\"{:.1}\" stroke=\"#dce4ec\" stroke-width=\"0.15\"/>",
            y + 2.0,
            y + 2.0
        );
        y += 6.8;
    }

    // Table 2: Top ABC Curve
    y += 8.0;
    text(
        &mut svg,
        18.0,
        y,
        3.2,
        true,
        ink,
        "2. Curva ABC de insumos (maiores impactos no orçamento)",
    );
    y += 6.5;
    let _ = writeln!(
        svg,
        "<rect x=\"16\" y=\"{:.1}\" width=\"388\" height=\"6.2\" fill=\"#edf2f7\" stroke=\"{ink}\" stroke-width=\"0.25\"/>",
        y - 4.2
    );
    for (title, x) in [
        ("Classe", 18.0),
        ("Insumo", 40.0),
        ("Qtd.", 230.0),
        ("Unid.", 260.0),
        ("Total (R$)", 295.0),
        ("% Item", 350.0),
        ("% Acum.", 380.0),
    ] {
        text(&mut svg, x, y, 2.4, true, ink, title);
    }
    y += 6.8;

    for row in report.abc_curve.iter().take(top_abc) {
        let cls_color = match row.class {
            'A' => "#b71c1c",
            'B' => "#b26a00",
            _ => "#1b5e20",
        };
        text(
            &mut svg,
            18.0,
            y,
            2.5,
            true,
            cls_color,
            &row.class.to_string(),
        );
        text(&mut svg, 40.0, y, 2.4, false, ink, &row.description);
        text(
            &mut svg,
            230.0,
            y,
            2.4,
            false,
            ink,
            &format!("{:.1}", row.total_quantity).replace('.', ","),
        );
        text(&mut svg, 260.0, y, 2.4, false, muted, &row.unit);
        text(
            &mut svg,
            295.0,
            y,
            2.5,
            true,
            ink,
            &format_brl(row.total_brl),
        );
        text(
            &mut svg,
            350.0,
            y,
            2.4,
            false,
            muted,
            &format!("{:.1}%", row.share_pct),
        );
        text(
            &mut svg,
            380.0,
            y,
            2.4,
            false,
            ink,
            &format!("{:.1}%", row.cumulative_pct),
        );
        let _ = writeln!(
            svg,
            "<line x1=\"16\" y1=\"{:.1}\" x2=\"404\" y2=\"{:.1}\" stroke=\"#dce4ec\" stroke-width=\"0.15\"/>",
            y + 2.0,
            y + 2.0
        );
        y += 6.8;
    }

    svg.push_str("</svg>\n");
    svg
}

impl Project {
    /// Complete cost estimate from quantities and editable/SINAPI prices (#87).
    pub fn cost_estimate(&self) -> CostEstimateReport {
        build_cost_estimate(self)
    }

    /// Imports a SINAPI CSV/TSV price table into `project.construction.cost` (#87).
    pub fn import_sinapi_table(
        &mut self,
        csv_text: &str,
        reference_label: Option<&str>,
    ) -> Result<SinapiImportResult, String> {
        import_sinapi_table(self, csv_text, reference_label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::samples::SampleProject;

    #[test]
    fn sinapi_import_updates_prices_and_changes_total_cost() {
        let mut project = SampleProject::CasaTerrea.build();
        project.generate_structure();
        project.generate_electrical_points();
        project.assign_circuits();

        let before = project.cost_estimate();
        assert!(before.grand_total_brl > 10_000.0);
        assert!(before.cost_per_m2_brl > 200.0);
        assert!(!before.abc_curve.is_empty());
        assert_eq!(before.abc_curve[0].class, 'A');

        // Import a SINAPI CSV raising cement and CA-50 steel prices.
        let sinapi_csv = "\
codigo;descricao;unidade;preco\n\
00001379;CIMENTO PORTLAND COMPOSTO CP II-32 SACO 50 KG;UN;55,00\n\
00000034;ACO CA-50 10,0 MM VERGALHAO;KG;14,50\n\
00000370;AREIA MEDIA LAVADA;M3;210,00\n";
        let imported = project
            .import_sinapi_table(sinapi_csv, Some("SINAPI SP 2026-01"))
            .unwrap();
        assert_eq!(imported.imported, 3);
        assert_eq!(imported.reference_label, "SINAPI SP 2026-01");

        let after = project.cost_estimate();
        assert!(after.grand_total_brl > before.grand_total_brl + 500.0);
        assert_eq!(after.reference_label, "SINAPI SP 2026-01");

        // BDI increases total proportionally.
        project.construction.cost.bdi_pct = 20.0;
        let with_bdi = project.cost_estimate();
        assert!((with_bdi.grand_total_brl - after.direct_total_brl * 1.20).abs() < 1e-3);

        let csv = cost_estimate_csv(&project);
        assert!(csv.starts_with("\u{feff}etapa,codigo,sinapi"));
        let svg = cost_estimate_svg(&project);
        assert!(svg.contains("ORÇAMENTO ESTIMATIVO"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn parse_brl_price_handles_brazilian_and_dot_formats() {
        assert!((parse_brl_price("R$ 1.234,56").unwrap() - 1234.56).abs() < 1e-6);
        assert!((parse_brl_price("38,50").unwrap() - 38.50).abs() < 1e-6);
        assert!((parse_brl_price("14.5").unwrap() - 14.5).abs() < 1e-6);
        assert!(parse_brl_price("invalido").is_none());
    }
}
