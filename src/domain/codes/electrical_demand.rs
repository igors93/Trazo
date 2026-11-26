//! Data-driven Brazilian electrical utility demand, service-entrance category
//! and multifamily collective residential demand engines (`T-11`).
//!
//! All tables, thresholds and citations are deserialized from versioned JSON
//! packs in `data/codes/electrical/*.json`. Zero normative numbers live in Rust.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Steps;

const EPS: f64 = 1e-9;
const W_PER_KW: f64 = 1000.0;

// ---------------------------------------------------------------------------
// 1. Trazo LoadSchedule utility profile section (`sections.utility_profile`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UtilityCitationData {
    pub standard: String,
    pub edition: String,
    pub tables: String,
    pub nominal_system: String,
    pub verification_note: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UtilityProfileData {
    pub single_phase_max_kva: f64,
    pub two_phase_max_kva: f64,
    pub max_bt_kva: f64,
    pub min_breaker_a: f64,
    pub min_wire_mm2: f64,
    pub general_table: Vec<(f64, f64)>,
    pub general_over_max: f64,
    pub specific_table: Vec<f64>,
    pub citation: UtilityCitationData,
}

impl UtilityProfileData {
    #[allow(dead_code)]
    pub fn general_demand_factor(&self, kva: f64) -> f64 {
        if !kva.is_finite() || kva <= 0.0 {
            return 1.0;
        }
        for &(upper_kva, factor) in &self.general_table {
            if kva <= upper_kva + EPS {
                return factor;
            }
        }
        if self.general_over_max > 0.0 {
            self.general_over_max
        } else {
            1.0
        }
    }

    #[allow(dead_code)]
    pub fn specific_demand_factor(&self, count: usize) -> f64 {
        if count == 0 || self.specific_table.is_empty() {
            return 1.0;
        }
        let idx = count.saturating_sub(1).min(self.specific_table.len() - 1);
        self.specific_table.get(idx).copied().unwrap_or(1.0)
    }
}

// ---------------------------------------------------------------------------
// 2. Unit Demand (`sections.unit_demand`: CPFL GED-13 D=a..i, Enel, Celesc)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitDemandStandard {
    pub utility: String,
    pub method: UnitDemandMethod,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitDemandMethod {
    Terms(Box<CpflTermsMethod>),
    Annexes(EnelAnnexesMethod),
    Table(CelescTableMethod),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CpflTermsMethod {
    pub least_outlets: Vec<[f64; 3]>,
    pub outlet: f64,
    pub kitchen_outlet: f64,
    pub kitchen_outlets: usize,
    pub lighting_point: f64,
    pub lighting: Steps,
    pub heaters: Steps,
    pub heaters_by_table: Vec<String>,
    pub boilers: Steps,
    pub appliances: Steps,
    pub stoves: Steps,
    pub air: Steps,
    pub motors: Steps,
    pub uses: Vec<CpflUseTiers>,
    pub special_power_factor: f64,
    pub special: Vec<CpflSpecialKind>,
    pub categories: Vec<CpflCategorySystem>,
    pub primary_above: f64,
    pub not_covered: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CpflUseTiers {
    #[serde(rename = "use")]
    pub use_kind: String,
    pub tiers: Vec<(Option<f64>, f64)>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CpflSpecialKind {
    pub kind: String,
    pub factors: Steps,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CpflCategorySystem {
    pub system: String,
    pub rows: Vec<CpflCategoryRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CpflCategoryRow {
    pub name: String,
    pub phases: u8,
    pub installed_up_to: f64,
    pub demand_up_to: Option<f64>,
    pub breaker: f64,
    pub cable: f64,
    pub aluminium: Option<f64>,
    pub conduit: f64,
    pub earthing: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnelAnnexesMethod {
    pub networks: Vec<EnelNetworkAnnex>,
    pub by_current: Option<EnelByCurrent>,
    pub demand_note: String,
    pub primary_note: String,
    pub not_covered: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnelNetworkAnnex {
    pub network: String,
    pub annex: String,
    pub primary_above: f64,
    pub rows: Vec<EnelAnnexRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnelAnnexRow {
    pub name: String,
    pub phases: u8,
    pub over: f64,
    pub up_to: f64,
    pub breaker: f64,
    pub earthing: f64,
    pub measurement: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnelByCurrent {
    pub network: String,
    pub annex: String,
    pub rows: Vec<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CelescTableMethod {
    pub table: String,
    pub networks: Vec<CelescNetworkTable>,
    pub asks_demand_from_breaker: Option<f64>,
    pub asks_demand_note: String,
    pub demand_note: String,
    pub not_covered: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CelescNetworkTable {
    pub system: String,
    pub rows: Vec<CelescCategoryRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CelescCategoryRow {
    pub name: String,
    pub phases: u8,
    pub over: f64,
    pub up_to: f64,
    pub breaker: f64,
    pub cable: Option<f64>,
}

/// Input loads for a single consumer unit demand evaluation (`CPFL D = a..i` / Enel / Celesc).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnitDemandInput {
    pub system: String,
    pub network: String,
    pub area_m2: f64,
    pub lighting_va: f64,
    pub outlets_va: f64,
    pub heaters_kw: Vec<f64>,
    pub boilers_kw: Vec<f64>,
    pub appliances_kw: Vec<f64>,
    pub stoves_kw: Vec<f64>,
    pub air_kva: Vec<f64>,
    pub motors_kva: Vec<f64>,
    pub special_kw: Vec<(String, f64)>,
    pub hydromassage_kva: Vec<f64>,
    pub commercial_air: bool,
    pub installed_kw_override: Option<f64>,
    pub min_phases: Option<u8>,
}

/// Breakdown of unit demand (`D = a..i`) and selected service-entrance category.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UnitDemandResult {
    pub utility: String,
    pub installed_kw: f64,
    pub demand_kva: f64,
    pub terms: BTreeMap<String, f64>,
    pub category: Option<String>,
    pub phases: Option<u8>,
    pub breaker_a: Option<f64>,
    pub cable_mm2: Option<f64>,
    pub conduit_mm: Option<f64>,
    pub earthing_mm2: Option<f64>,
    pub requires_primary: bool,
    pub notes: Vec<String>,
}

/// Evaluate unit demand and service category using the active utility's `unit_demand` JSON.
pub fn calculate_unit_demand(
    std: &UnitDemandStandard,
    input: &UnitDemandInput,
) -> UnitDemandResult {
    match &std.method {
        UnitDemandMethod::Terms(cpfl) => eval_cpfl_terms(&std.utility, cpfl, input),
        UnitDemandMethod::Annexes(enel) => eval_enel_annexes(&std.utility, enel, input),
        UnitDemandMethod::Table(celesc) => eval_celesc_table(&std.utility, celesc, input),
    }
}

fn eval_cpfl_terms(
    utility: &str,
    m: &CpflTermsMethod,
    input: &UnitDemandInput,
) -> UnitDemandResult {
    // a: lighting + outlets (if declared, use declared; otherwise derive from Tabela 2 by area)
    let base_a_va = if input.lighting_va + input.outlets_va > EPS {
        input.lighting_va + input.outlets_va
    } else {
        let (tugs, light_pts) = lookup_least_outlets(&m.least_outlets, input.area_m2);
        tugs * m.outlet
            + (m.kitchen_outlets as f64) * m.kitchen_outlet
            + light_pts * m.lighting_point
    };
    let a_kva_inst = base_a_va / W_PER_KW;
    let a = a_kva_inst * m.lighting.at(a_kva_inst);

    // b: heaters / electric showers
    let b_inst: f64 = input.heaters_kw.iter().sum();
    let b = if input.heaters_kw.is_empty() {
        0.0
    } else {
        b_inst * m.heaters.at(input.heaters_kw.len() as f64)
    };

    // c: boilers
    let c_inst: f64 = input.boilers_kw.iter().sum();
    let c = if input.boilers_kw.is_empty() {
        0.0
    } else {
        c_inst * m.boilers.at(input.boilers_kw.len() as f64)
    };

    // d: appliances
    let d_inst: f64 = input.appliances_kw.iter().sum();
    let d = if input.appliances_kw.is_empty() {
        0.0
    } else {
        d_inst * m.appliances.at(input.appliances_kw.len() as f64)
    };

    // e: electric stoves
    let e_inst: f64 = input.stoves_kw.iter().sum();
    let e = if input.stoves_kw.is_empty() {
        0.0
    } else {
        e_inst * m.stoves.at(input.stoves_kw.len() as f64)
    };

    // f: air conditioning
    let f_inst: f64 = input.air_kva.iter().sum();
    let f = if input.air_kva.is_empty() {
        0.0
    } else if input.commercial_air {
        f_inst * m.air.at(input.air_kva.len() as f64)
    } else {
        f_inst
    };

    // g: motors sorted largest first, weighted by rank
    let g = sum_ranked(&input.motors_kva, &m.motors);

    // h: special equipment by kind, largest first, divided by special_power_factor
    let mut by_kind: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for (kind, kw) in &input.special_kw {
        by_kind.entry(kind.as_str()).or_default().push(*kw);
    }
    let pf = if m.special_power_factor > EPS {
        m.special_power_factor
    } else {
        1.0
    };
    let mut h = 0.0;
    for (kind, vals) in by_kind {
        let steps = m
            .special
            .iter()
            .find(|s| s.kind == kind)
            .map(|s| &s.factors);
        if let Some(st) = steps {
            h += sum_ranked(&vals, st) / pf;
        } else {
            h += vals.iter().sum::<f64>() / pf;
        }
    }

    // i: hydromassage sorted largest first using motors rank table
    let i = sum_ranked(&input.hydromassage_kva, &m.motors);

    let demand_kva = a + b + c + d + e + f + g + h + i;
    let installed_kw = input.installed_kw_override.unwrap_or_else(|| {
        a_kva_inst
            + b_inst
            + c_inst
            + d_inst
            + e_inst
            + f_inst
            + input.motors_kva.iter().sum::<f64>()
            + input.special_kw.iter().map(|(_, k)| *k).sum::<f64>()
            + input.hydromassage_kva.iter().sum::<f64>()
    });

    let mut terms = BTreeMap::new();
    terms.insert("a_iluminacao_tug".to_owned(), a);
    terms.insert("b_aquecimento".to_owned(), b);
    terms.insert("c_boilers".to_owned(), c);
    terms.insert("d_eletrodomesticos".to_owned(), d);
    terms.insert("e_fogoes".to_owned(), e);
    terms.insert("f_ar_condicionado".to_owned(), f);
    terms.insert("g_motores".to_owned(), g);
    terms.insert("h_especiais".to_owned(), h);
    terms.insert("i_hidromassagem".to_owned(), i);

    let sys = if input.system.is_empty() {
        m.categories.first()
    } else {
        m.categories
            .iter()
            .find(|c| c.system == input.system)
            .or_else(|| m.categories.first())
    };
    let min_phases = input.min_phases.unwrap_or(1);
    let requires_primary = m.primary_above > EPS && installed_kw > m.primary_above + EPS;
    let picked = if requires_primary {
        None
    } else {
        sys.and_then(|s| {
            s.rows.iter().find(|r| {
                r.phases >= min_phases
                    && installed_kw <= r.installed_up_to + EPS
                    && r.demand_up_to.is_none_or(|up| demand_kva <= up + EPS)
            })
        })
    };

    let mut notes = Vec::new();
    if requires_primary {
        notes.push(format!(
            "Carga instalada ({installed_kw:.1} kW) excede o limite secundário ({:.0} kW): exige atendimento em tensão primária (MT).",
            m.primary_above
        ));
    }

    UnitDemandResult {
        utility: utility.to_owned(),
        installed_kw,
        demand_kva,
        terms,
        category: picked.map(|r| r.name.clone()),
        phases: picked.map(|r| r.phases),
        breaker_a: picked.map(|r| r.breaker),
        cable_mm2: picked.map(|r| r.cable),
        conduit_mm: picked.map(|r| r.conduit),
        earthing_mm2: picked.map(|r| r.earthing),
        requires_primary,
        notes,
    }
}

fn eval_enel_annexes(
    utility: &str,
    m: &EnelAnnexesMethod,
    input: &UnitDemandInput,
) -> UnitDemandResult {
    let installed_kw = input.installed_kw_override.unwrap_or_else(|| {
        (input.lighting_va + input.outlets_va) / W_PER_KW
            + input.heaters_kw.iter().sum::<f64>()
            + input.boilers_kw.iter().sum::<f64>()
            + input.appliances_kw.iter().sum::<f64>()
            + input.stoves_kw.iter().sum::<f64>()
            + input.air_kva.iter().sum::<f64>()
            + input.motors_kva.iter().sum::<f64>()
    });
    let net = if input.network.is_empty() {
        m.networks.first()
    } else {
        m.networks
            .iter()
            .find(|n| n.network == input.network)
            .or_else(|| m.networks.first())
    };
    let min_phases = input.min_phases.unwrap_or(1);
    let primary_above = net.map(|n| n.primary_above).unwrap_or(0.0);
    let requires_primary = primary_above > EPS && installed_kw > primary_above + EPS;
    let picked = if requires_primary {
        None
    } else {
        net.and_then(|n| {
            n.rows.iter().find(|r| {
                r.phases >= min_phases
                    && installed_kw > r.over - EPS
                    && installed_kw <= r.up_to + EPS
            })
        })
    };
    let mut notes = Vec::new();
    if let Some(n) = net {
        notes.push(format!("Anexo {} ({})", n.annex, n.network));
    }
    if requires_primary && !m.primary_note.is_empty() {
        notes.push(m.primary_note.clone());
    }
    UnitDemandResult {
        utility: utility.to_owned(),
        installed_kw,
        demand_kva: installed_kw,
        terms: BTreeMap::new(),
        category: picked.map(|r| r.name.clone()),
        phases: picked.map(|r| r.phases),
        breaker_a: picked.map(|r| r.breaker),
        cable_mm2: None,
        conduit_mm: None,
        earthing_mm2: picked.map(|r| r.earthing),
        requires_primary,
        notes,
    }
}

fn eval_celesc_table(
    utility: &str,
    m: &CelescTableMethod,
    input: &UnitDemandInput,
) -> UnitDemandResult {
    let installed_kw = input.installed_kw_override.unwrap_or_else(|| {
        (input.lighting_va + input.outlets_va) / W_PER_KW
            + input.heaters_kw.iter().sum::<f64>()
            + input.appliances_kw.iter().sum::<f64>()
    });
    let net = if input.system.is_empty() {
        m.networks.first()
    } else {
        m.networks
            .iter()
            .find(|n| n.system == input.system)
            .or_else(|| m.networks.first())
    };
    let min_phases = input.min_phases.unwrap_or(1);
    let picked = net.and_then(|n| {
        n.rows.iter().find(|r| {
            r.phases >= min_phases && installed_kw > r.over - EPS && installed_kw <= r.up_to + EPS
        })
    });
    let mut notes = Vec::new();
    if let (Some(r), Some(thr)) = (picked, m.asks_demand_from_breaker)
        && r.breaker >= thr - EPS
        && !m.asks_demand_note.is_empty()
    {
        notes.push(m.asks_demand_note.clone());
    }
    UnitDemandResult {
        utility: utility.to_owned(),
        installed_kw,
        demand_kva: installed_kw,
        terms: BTreeMap::new(),
        category: picked.map(|r| r.name.clone()),
        phases: picked.map(|r| r.phases),
        breaker_a: picked.map(|r| r.breaker),
        cable_mm2: picked.and_then(|r| r.cable),
        conduit_mm: None,
        earthing_mm2: None,
        requires_primary: picked.is_none() && installed_kw > EPS,
        notes,
    }
}

fn lookup_least_outlets(rows: &[[f64; 3]], area_m2: f64) -> (f64, f64) {
    for row in rows {
        if area_m2 <= row[0] + EPS {
            return (row[1], row[2]);
        }
    }
    rows.last().map(|r| (r[1], r[2])).unwrap_or((0.0, 0.0))
}

fn sum_ranked(values: &[f64], steps: &Steps) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let mut sum = 0.0;
    for (idx, val) in sorted.iter().enumerate() {
        sum += val * steps.at((idx + 1) as f64);
    }
    sum
}

// ---------------------------------------------------------------------------
// 3. Data-driven Service Entrance Categories (`sections.entrance`)
//    Covers COPEL (+ rural), Energisa (9 members), Equatorial, Fecoergs (15), Hidropan
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntranceUtility {
    pub id: String,
    pub name: String,
    pub standard: String,
    pub source: String,
    pub rule: String,
    pub members: Vec<EntranceMember>,
    pub columns: BTreeMap<String, String>,
    pub tables: Vec<EntranceTable>,
    pub notes: Vec<String>,
    pub not_covered: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntranceMember {
    pub id: String,
    pub seat: String,
    pub changes: Vec<EntranceMemberChange>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntranceMemberChange {
    pub table: Option<String>,
    pub row: Option<String>,
    pub measurement: Option<String>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntranceTable {
    pub system: String,
    #[serde(rename = "use")]
    pub use_kind: String,
    pub source: String,
    pub members: Vec<String>,
    pub rows: Vec<EntranceRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntranceRange {
    pub over: Option<f64>,
    pub up_to: Option<f64>,
}

impl EntranceRange {
    pub fn contains(&self, val: f64) -> bool {
        let ok_lo = self.over.is_none_or(|lo| val > lo - EPS);
        let ok_hi = self.up_to.is_none_or(|hi| val <= hi + EPS);
        ok_lo && ok_hi
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntranceRow {
    pub name: String,
    pub phases: u8,
    pub installed_kw: Option<EntranceRange>,
    pub demand_kva: Option<EntranceRange>,
    pub breaker_a: Vec<f64>,
    pub measurement: Option<String>,
    pub columns: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntrancePickInput {
    pub system: String,
    pub use_kind: String,
    pub member: Option<String>,
    pub installed_kw: Option<f64>,
    pub demand_kva: Option<f64>,
    pub min_phases: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EntranceCategoryPick {
    pub utility: String,
    pub standard: String,
    pub system: String,
    pub table_source: String,
    pub category: Option<String>,
    pub phases: Option<u8>,
    pub breaker_a: Vec<f64>,
    pub measurement: Option<String>,
    pub columns: BTreeMap<String, Value>,
    pub changed_by: Option<String>,
    pub needs: Vec<String>,
}

/// Select the service-entrance category from a data-driven `entrance` section.
pub fn pick_entrance_category(
    util: &EntranceUtility,
    input: &EntrancePickInput,
) -> Option<EntranceCategoryPick> {
    let table = util
        .tables
        .iter()
        .find(|t| {
            let sys_ok = input.system.is_empty() || t.system == input.system;
            let use_ok = input.use_kind.is_empty() || t.use_kind == input.use_kind;
            let mem_ok = t.members.is_empty()
                || input
                    .member
                    .as_deref()
                    .is_some_and(|m| t.members.iter().any(|tm| tm.eq_ignore_ascii_case(m)));
            sys_ok && use_ok && mem_ok
        })
        .or_else(|| util.tables.first())?;

    let min_phases = input.min_phases.unwrap_or(1);
    let mut needs = Vec::new();
    let mut picked: Option<&EntranceRow> = None;

    for row in &table.rows {
        if row.phases < min_phases {
            continue;
        }
        let inst_ok = match (&row.installed_kw, input.installed_kw) {
            (Some(range), Some(kw)) => range.contains(kw),
            (Some(_), None) => {
                if !needs.iter().any(|n| n == "installed_kw") {
                    needs.push("installed_kw".to_owned());
                }
                false
            }
            (None, _) => true,
        };
        if !inst_ok {
            continue;
        }
        let dem_ok = match (&row.demand_kva, input.demand_kva) {
            (Some(range), Some(kva)) => range.contains(kva),
            (Some(_), None) => {
                if !needs.iter().any(|n| n == "demand_kva") {
                    needs.push("demand_kva".to_owned());
                }
                false
            }
            (None, _) => true,
        };
        if dem_ok {
            picked = Some(row);
            break;
        }
    }

    let mut measurement = picked.and_then(|r| r.measurement.clone());
    let mut changed_by = None;

    if let (Some(row), Some(mem_id)) = (picked, input.member.as_deref())
        && let Some(member) = util
            .members
            .iter()
            .find(|m| m.id.eq_ignore_ascii_case(mem_id))
    {
        for ch in &member.changes {
            let row_matches = ch
                .row
                .as_deref()
                .is_none_or(|rn| rn.eq_ignore_ascii_case(&row.name));
            if row_matches {
                if let Some(m) = &ch.measurement {
                    measurement = Some(m.clone());
                    changed_by = Some(member.id.clone());
                } else if ch.note.is_some() {
                    changed_by = Some(member.id.clone());
                }
            }
        }
    }

    Some(EntranceCategoryPick {
        utility: util.name.clone(),
        standard: util.standard.clone(),
        system: table.system.clone(),
        table_source: table.source.clone(),
        category: picked.map(|r| r.name.clone()),
        phases: picked.map(|r| r.phases),
        breaker_a: picked.map(|r| r.breaker_a.clone()).unwrap_or_default(),
        measurement,
        columns: picked.map(|r| r.columns.clone()).unwrap_or_default(),
        changed_by,
        needs: if picked.is_some() { Vec::new() } else { needs },
    })
}

// ---------------------------------------------------------------------------
// 4. Collective Multifamily Residential Demand (`sections.building_demand`)
//    Covers CODI RTD-027 / CEMIG ND-5.2 / Celesc N-321.0003 / Fecoergs / Hidropan
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildingDemandMethod {
    pub standard: String,
    pub utilities: Vec<String>,
    pub country: String,
    pub source: String,
    pub unit: String,
    pub min_dwellings: usize,
    pub mean_of: String,
    pub dwelling: CollectiveDwelling,
    pub diversity: CollectiveDiversity,
    pub factor: f64,
    pub factor_source: String,
    pub factor_on_common: bool,
    pub common: Vec<CommonAreaClass>,
    pub entrance: Option<CollectiveEntrance>,
    pub notes: Vec<String>,
    pub not_covered: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CollectiveDwelling {
    pub source: String,
    pub by_area: Vec<(f64, f64)>,
    pub above: Option<AreaCurve>,
    pub least: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AreaCurve {
    pub coefficient: f64,
    pub exponent: f64,
    pub source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CollectiveDiversity {
    pub source: String,
    pub kind: Option<String>,
    pub steps: Vec<(f64, f64)>,
    pub beyond: Option<f64>,
    pub errata: Vec<TableErratum>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableErratum {
    pub at: usize,
    pub printed: f64,
    pub value: f64,
    pub why: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CommonAreaClass {
    pub class: String,
    pub power_factor: f64,
    pub bands: Vec<CommonBand>,
    pub source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CommonBand {
    pub up_to: Option<f64>,
    pub factor: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CollectiveEntrance {
    pub source: String,
    pub rows: Vec<CollectiveEntranceRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CollectiveEntranceRow {
    pub over: f64,
    pub up_to: Option<f64>,
    pub breaker_a: Option<f64>,
    pub columns: BTreeMap<String, Value>,
}

/// Group of identical residential apartments/units for collective demand.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DwellingGroup {
    pub count: usize,
    pub area_m2: f64,
}

/// Input parameters for multifamily collective residential demand (`T-11`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildingDemandInput {
    pub dwellings: Vec<DwellingGroup>,
    /// Optional installed loads per common-area class (`"lighting"`, `"outlets"`), in kW.
    pub common_installed_kw: BTreeMap<String, f64>,
    /// Additional already-demanded common-area load (`kVA` or `kW` matching utility unit).
    pub common_demand_extra: f64,
    /// Special loads (elevators, pumps, central AC), in `kVA`/`kW`.
    pub special_demand: f64,
    /// Commercial premises demand, if mixed-use.
    pub commercial_demand: f64,
    /// If `true`, evaluate literal printed values when an official table has a known typo.
    pub as_printed: bool,
}

/// Detailed result of collective residential building demand (`CODI / CEMIG / Celesc / Fecoergs / Hidropan`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildingDemandResult {
    pub standard: String,
    pub source: String,
    pub unit: String,
    pub total_dwellings: usize,
    pub weighted_mean_area_m2: f64,
    pub dwelling_demand_d: f64,
    pub diversity_factor_f: f64,
    pub safety_factor: f64,
    pub dwellings_total_demand: f64,
    pub common_demand: f64,
    pub special_demand: f64,
    pub commercial_demand: f64,
    pub total_demand: f64,
    pub entrance_breaker_a: Option<f64>,
    pub entrance_columns: BTreeMap<String, Value>,
    pub entrance_source: Option<String>,
    pub erratum_applied: Option<String>,
    pub warnings: Vec<String>,
}

/// Calculate collective residential demand for a multifamily building (`T-11`).
pub fn calculate_building_demand(
    method: &BuildingDemandMethod,
    input: &BuildingDemandInput,
) -> Result<BuildingDemandResult, String> {
    let total_dwellings: usize = input.dwellings.iter().map(|g| g.count).sum();
    if total_dwellings == 0 {
        return Err("Informe ao menos uma unidade residencial.".to_owned());
    }
    let mut warnings = Vec::new();
    if method.min_dwellings > 0 && total_dwellings < method.min_dwellings {
        warnings.push(format!(
            "A norma {} aplica-se a partir de {} unidades residenciais (informado: {total_dwellings}).",
            method.standard, method.min_dwellings
        ));
    }

    let area_sum: f64 = input
        .dwellings
        .iter()
        .map(|g| g.count as f64 * g.area_m2.max(0.0))
        .sum();
    let mean_area = area_sum / total_dwellings as f64;

    // 1. Unit demand `d` by weighted mean area (`by_area` table or `above` curve)
    let mut d = lookup_by_area(&method.dwelling, mean_area)?;
    if let Some(least) = method.dwelling.least {
        d = d.max(least);
    }

    // 2. Diversity factor `F(n)` (with official table erratum support)
    let (f_n, erratum_applied) =
        lookup_diversity(&method.diversity, total_dwellings, input.as_printed)?;

    let factor = if method.factor > EPS {
        method.factor
    } else {
        1.0
    };
    let dwellings_total = factor * f_n * d;

    // 3. Common areas demand
    let mut common_raw = input.common_demand_extra.max(0.0);
    for cls in &method.common {
        if let Some(&kw) = input.common_installed_kw.get(&cls.class)
            && kw > 0.0
        {
            let tiered = eval_tiered_bands(kw, &cls.bands);
            let pf = if method.unit.eq_ignore_ascii_case("kVA") && cls.power_factor > EPS {
                cls.power_factor
            } else {
                1.0
            };
            common_raw += tiered / pf;
        }
    }
    let common_demand = if method.factor_on_common {
        factor * common_raw
    } else {
        common_raw
    };

    let special = input.special_demand.max(0.0);
    let commercial = input.commercial_demand.max(0.0);
    let total_demand = dwellings_total + common_demand + special + commercial;

    // 4. Collective service-entrance row lookup
    let (entrance_breaker_a, entrance_columns, entrance_source) = if let Some(ent) =
        &method.entrance
    {
        let row = ent
            .rows
            .iter()
            .find(|r| {
                total_demand > r.over - EPS && r.up_to.is_none_or(|hi| total_demand <= hi + EPS)
            })
            .or_else(|| {
                ent.rows
                    .first()
                    .filter(|first| total_demand <= first.over + EPS)
            });
        if let Some(r) = row {
            (r.breaker_a, r.columns.clone(), Some(ent.source.clone()))
        } else {
            warnings.push(format!(
                    "Demanda total ({total_demand:.1} {}) acima das faixas da tabela de entrada coletiva BT: avaliar subestação/MT.",
                    method.unit
                ));
            (None, BTreeMap::new(), Some(ent.source.clone()))
        }
    } else {
        (None, BTreeMap::new(), None)
    };

    Ok(BuildingDemandResult {
        standard: method.standard.clone(),
        source: method.source.clone(),
        unit: method.unit.clone(),
        total_dwellings,
        weighted_mean_area_m2: mean_area,
        dwelling_demand_d: d,
        diversity_factor_f: f_n,
        safety_factor: factor,
        dwellings_total_demand: dwellings_total,
        common_demand,
        special_demand: special,
        commercial_demand: commercial,
        total_demand,
        entrance_breaker_a,
        entrance_columns,
        entrance_source,
        erratum_applied,
        warnings,
    })
}

fn lookup_by_area(dw: &CollectiveDwelling, area_m2: f64) -> Result<f64, String> {
    for &(upper_area, demand) in &dw.by_area {
        if area_m2 <= upper_area + EPS {
            return Ok(demand);
        }
    }
    if let Some(curve) = &dw.above {
        return Ok(curve.coefficient * area_m2.max(1.0).powf(curve.exponent));
    }
    dw.by_area
        .last()
        .map(|&(_, demand)| demand)
        .ok_or_else(|| "Tabela de demanda por área (by_area) vazia.".to_owned())
}

fn lookup_diversity(
    div: &CollectiveDiversity,
    n: usize,
    as_printed: bool,
) -> Result<(f64, Option<String>), String> {
    let erratum = div.errata.iter().find(|e| e.at == n);
    if let Some(err) = erratum {
        let val = if as_printed { err.printed } else { err.value };
        let note = if as_printed {
            format!(
                "Valor impresso adotado para n={n}: {} (corrigido: {} — {}).",
                err.printed, err.value, err.why
            )
        } else {
            format!(
                "Errata aplicada para n={n}: {} em vez do impresso {} ({}).",
                err.value, err.printed, err.why
            )
        };
        return Ok((val, Some(note)));
    }
    let nf = n as f64;
    for &(step_n, factor) in &div.steps {
        if nf <= step_n + EPS {
            let effective = if div.kind.as_deref() == Some("per_dwelling") {
                nf * factor
            } else {
                factor
            };
            return Ok((effective, None));
        }
    }
    if let Some(&(last_n, last_f)) = div.steps.last() {
        if let Some(slope) = div.beyond {
            return Ok((last_f + (nf - last_n).max(0.0) * slope, None));
        }
        return Ok((last_f, None));
    }
    Err("Tabela de fator de diversidade F(n) vazia.".to_owned())
}

fn eval_tiered_bands(total_kw: f64, bands: &[CommonBand]) -> f64 {
    let mut remaining = total_kw;
    let mut prev_cap = 0.0;
    let mut acc = 0.0;
    for band in bands {
        if remaining <= EPS {
            break;
        }
        let slice = match band.up_to {
            Some(cap) => (cap - prev_cap).max(0.0).min(remaining),
            None => remaining,
        };
        acc += slice * band.factor;
        remaining -= slice;
        if let Some(cap) = band.up_to {
            prev_cap = cap;
        }
    }
    acc
}
