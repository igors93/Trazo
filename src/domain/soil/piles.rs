//! Décourt-Quaresma bearing capacity and sizing for hand-dug bored piles
//! (*brocas*, 3–6 m above the water table) and mechanized deep piles
//! (*hélice contínua* / CFA, 6–20 m, allowed below the water table).

use std::f64::consts::PI;

use super::settlement::{MAX_PILE_SETTLEMENT_MM, estimate_pile_settlement_mm};
use super::study::{FoundationOption, FoundationStudy, OptionResult};
use super::survey::{Borehole, MAX_N, SoilSurvey};
use super::{SoilKind, decimal};

/// Shortest hand-dug bored pile, metres: below the seasonal active layer.
pub const MIN_BROCA_M: f64 = 3.0;
/// Longest hand-dug bored pile, metres.
pub const MAX_BROCA_M: f64 = 6.0;
/// Shortest mechanized deep pile (*hélice contínua*), metres.
pub const MIN_DEEP_PILE_M: f64 = 6.0;
/// Longest mechanized deep pile considered by this pre-sizing, metres.
pub const MAX_DEEP_PILE_M: f64 = 20.0;

/// Concrete stress allowed in a hand-dug bored pile, kPa (5 MPa).
const BROCA_CONCRETE_KPA: f64 = 5_000.0;
/// Concrete stress allowed in a mechanized CFA pile, kPa (6 MPa).
const CFA_CONCRETE_KPA: f64 = 6_000.0;
/// Most hand-dug brocas under one column before switching to deep piles.
const MAX_BROCAS_PER_COLUMN: u32 = 3;
/// Most deep piles under one column in this pre-sizing study.
const MAX_DEEP_PILES_PER_COLUMN: u32 = 4;

/// Pile execution family considered in the foundation study.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PileKind {
    /// Hand-dug bored pile (*estaca broca*, 20–30 cm, 3–6 m, dry soil only).
    BoredBroca,
    /// Mechanized continuous flight auger (*hélice contínua*, 30–50 cm, 6–20 m).
    DeepCfa,
}

impl PileKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::BoredBroca => "Broca manual",
            Self::DeepCfa => "Hélice contínua",
        }
    }
}

/// Bearing capacity of one pile (weakest borehole envelope).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PileCapacity {
    pub kind: PileKind,
    pub diameter_m: f64,
    pub length_m: f64,
    pub tip_n: f64,
    pub shaft_n: f64,
    /// Ultimate tip and shaft resistance, kN.
    pub tip_kn: f64,
    pub shaft_kn: f64,
    /// Limit of the concrete shaft, kN.
    pub structural_kn: f64,
    /// Allowable working load, kN.
    pub allowable_kn: f64,
    /// Estimated working settlement at allowable load, mm.
    pub settlement_mm: f64,
}

/// Computes Décourt-Quaresma capacity in a single borehole.
fn capacity_in_borehole(
    borehole: &Borehole,
    kind: PileKind,
    diameter: f64,
    length: f64,
) -> Option<(PileCapacity, SoilKind)> {
    if borehole.deepest_m() + 1e-6 < length {
        return None;
    }
    let tip_n = borehole.mean_n(length - 1.0, length + 1.0)?;
    let mut shaft: Vec<&super::survey::SptReading> = borehole
        .readings
        .iter()
        .filter(|r| r.depth_m < length - 1.0 - 1e-6)
        .collect();
    if shaft.is_empty() {
        shaft = borehole
            .readings
            .iter()
            .filter(|r| r.depth_m <= length + 1e-6)
            .collect();
    }
    let shaft_n = shaft
        .iter()
        .map(|r| f64::from(r.n).clamp(3.0, MAX_N))
        .sum::<f64>()
        / shaft.len().max(1) as f64;
    let tip_soil = borehole.reading_at(length)?.soil;
    let (alpha, beta) = match kind {
        PileKind::BoredBroca => {
            let beta = shaft.iter().map(|r| r.soil.bored_factors().1).sum::<f64>()
                / shaft.len().max(1) as f64;
            (tip_soil.bored_factors().0, beta)
        }
        PileKind::DeepCfa => {
            let beta = shaft.iter().map(|r| r.soil.cfa_factors().1).sum::<f64>()
                / shaft.len().max(1) as f64;
            (tip_soil.cfa_factors().0, beta)
        }
    };
    let area = PI * diameter * diameter / 4.0;
    let perimeter = PI * diameter;
    let tip_kn = alpha * tip_soil.tip_c_kpa() * tip_n * area;
    let shaft_kn = beta * 10.0 * (shaft_n / 3.0 + 1.0) * perimeter * length;
    let mut geotechnical = ((tip_kn + shaft_kn) / 2.0).min(tip_kn / 4.0 + shaft_kn / 1.3);
    if kind == PileKind::BoredBroca {
        // NBR 6122 for hand-dug bored piles: tip takes at most 20% of allowable load.
        geotechnical = geotechnical.min(1.25 * shaft_kn / 2.0);
    }
    let structural_kn = match kind {
        PileKind::BoredBroca => BROCA_CONCRETE_KPA * area,
        PileKind::DeepCfa => CFA_CONCRETE_KPA * area,
    };
    let allowable_kn = geotechnical.min(structural_kn);
    let mut cap = PileCapacity {
        kind,
        diameter_m: diameter,
        length_m: length,
        tip_n,
        shaft_n,
        tip_kn,
        shaft_kn,
        structural_kn,
        allowable_kn,
        settlement_mm: 0.0,
    };
    cap.settlement_mm = estimate_pile_settlement_mm(allowable_kn, &cap, tip_soil);
    Some((cap, tip_soil))
}

impl SoilSurvey {
    /// Governing capacity of one hand-dug bored pile (`diameter` × `length`)
    /// across all boreholes on the site.
    #[cfg(test)]
    pub fn pile_capacity(&self, diameter: f64, length: f64) -> Option<PileCapacity> {
        self.pile_capacity_by_kind(PileKind::BoredBroca, diameter, length)
            .map(|(cap, _)| cap)
    }

    /// Governing capacity of one deep CFA pile (`diameter` × `length`) across
    /// all boreholes on the site.
    #[cfg(test)]
    pub fn deep_pile_capacity(&self, diameter: f64, length: f64) -> Option<PileCapacity> {
        self.pile_capacity_by_kind(PileKind::DeepCfa, diameter, length)
            .map(|(cap, _)| cap)
    }

    pub(crate) fn pile_capacity_by_kind(
        &self,
        kind: PileKind,
        diameter: f64,
        length: f64,
    ) -> Option<(PileCapacity, String)> {
        let mut weakest: Option<(PileCapacity, String)> = None;
        for borehole in self.boreholes() {
            let (cap, _) = capacity_in_borehole(&borehole, kind, diameter, length)?;
            if weakest
                .as_ref()
                .is_none_or(|(old, _)| cap.allowable_kn < old.allowable_kn)
            {
                weakest = Some((cap, borehole.name));
            }
        }
        weakest
    }

    /// Longest hand-dug bored pile allowed: up to 6 m, strictly above the
    /// shallowest water table and within every borehole's explored depth.
    pub fn max_broca_m(&self) -> f64 {
        let mut limit = MAX_BROCA_M.min(self.deepest_m());
        if let Some(water) = self.governing_water_table_m() {
            limit = limit.min(water);
        }
        limit.floor().max(0.0)
    }

    /// Longest deep mechanized pile allowed: up to 20 m within every borehole's
    /// explored depth (may penetrate below the water table).
    pub fn max_deep_pile_m(&self) -> f64 {
        MAX_DEEP_PILE_M.min(self.deepest_m()).floor().max(0.0)
    }
}

/// Concrete volume in a pile cap for `n` piles of diameter `d`, m³.
pub(crate) fn cap_m3(n: u32, d: f64) -> f64 {
    let side = (d + 0.30).max(0.45);
    match n {
        0 => 0.0,
        1 => side * side * 0.40,
        2 => side * (3.0 * d + 0.30 + side) * 0.45,
        3 => (3.0 * d + 0.60).powi(2) * 0.50,
        _ => (3.0 * d + 0.60).powi(2) * 0.55,
    }
}

/// Evaluates hand-dug bored piles (*brocas*, 20–30 cm, 3–6 m, above water table).
pub(crate) fn broca_option(
    study: &FoundationStudy,
    survey: Option<&SoilSurvey>,
) -> (OptionResult, Option<PileCapacity>, u32) {
    let fail = |summary: String, notes: Vec<String>| {
        (
            OptionResult {
                option: FoundationOption::BoredPiles,
                viable: false,
                summary,
                notes,
                concrete_m3: 0.0,
                soil_kpa: None,
                settlement_mm: None,
                differential_settlement_mm: None,
            },
            None,
            0,
        )
    };
    let Some(survey) = survey else {
        return fail(
            "Sem sondagem: informe o SPT para dimensionar as brocas.".to_owned(),
            Vec::new(),
        );
    };
    let max_length = survey.max_broca_m();
    if max_length < MIN_BROCA_M {
        return fail(
            format!(
                "Brocas precisam de pelo menos 3 m acima do lençol e dentro da sondagem (limite atual: {} m).",
                decimal(max_length, 1)
            ),
            vec!["Lençol freático raso ou sondagem curta inviabilizam brocas manuais; avalie estacas profundas.".to_owned()],
        );
    }
    let Some((concrete, pile, count, governing)) = select_best_pile(
        study,
        survey,
        PileKind::BoredBroca,
        &[0.20, 0.25, 0.30],
        MIN_BROCA_M,
        max_length,
        MAX_BROCAS_PER_COLUMN,
    ) else {
        return fail(
            format!(
                "Nenhuma broca de 20 a 30 cm com até {} m resolve com até 3 por pilar: avalie estacas profundas.",
                decimal(max_length, 0)
            ),
            Vec::new(),
        );
    };
    let mut notes =
        vec!["Cada pilar fica num bloco sobre as brocas, ligado pelas vigas baldrame.".to_owned()];
    if survey.borehole_count() > 1 {
        notes.push(format!("Capacidade governada pelo furo {governing}."));
    }
    if survey.governing_water_table_m().is_some() {
        notes
            .push("Brocas só acima do lençol freático; o comprimento já respeita isso.".to_owned());
    }
    let viable = pile.settlement_mm <= MAX_PILE_SETTLEMENT_MM + 1e-6;
    if !viable {
        notes.push(format!(
            "Recalque estimado de {} mm acima do limite preliminar de {:.0} mm.",
            decimal(pile.settlement_mm, 1),
            MAX_PILE_SETTLEMENT_MM
        ));
    }
    (
        OptionResult {
            option: FoundationOption::BoredPiles,
            viable,
            summary: format!(
                "{count} broca(s) de Ø {:.0} cm × {} m, {:.0} kN cada (ponta {:.0} kN, atrito {:.0} kN na ruptura; recalque est. {} mm).",
                pile.diameter_m * 100.0,
                decimal(pile.length_m, 0),
                pile.allowable_kn,
                pile.tip_kn,
                pile.shaft_kn,
                decimal(pile.settlement_mm, 1),
            ),
            notes,
            concrete_m3: concrete,
            soil_kpa: None,
            settlement_mm: Some(pile.settlement_mm),
            differential_settlement_mm: None,
        },
        Some(pile),
        count,
    )
}

/// Evaluates deep mechanized piles (*hélice contínua* / CFA, 30–50 cm, 6–20 m).
/// Works below the water table and bridges weak surface layers.
pub(crate) fn deep_pile_option(
    study: &FoundationStudy,
    survey: Option<&SoilSurvey>,
) -> (OptionResult, Option<PileCapacity>, u32) {
    let fail = |summary: String, notes: Vec<String>| {
        (
            OptionResult {
                option: FoundationOption::DeepPiles,
                viable: false,
                summary,
                notes,
                concrete_m3: 0.0,
                soil_kpa: None,
                settlement_mm: None,
                differential_settlement_mm: None,
            },
            None,
            0,
        )
    };
    let Some(survey) = survey else {
        return fail(
            "Sem sondagem: informe o SPT (mín. 6 m) para dimensionar estacas profundas.".to_owned(),
            Vec::new(),
        );
    };
    let max_length = survey.max_deep_pile_m();
    if max_length < MIN_DEEP_PILE_M {
        return fail(
            format!(
                "A sondagem chega a {} m: estacas profundas exigem leituras de pelo menos 6 m.",
                decimal(survey.deepest_m(), 1)
            ),
            Vec::new(),
        );
    }
    let Some((concrete, pile, count, governing)) = select_best_pile(
        study,
        survey,
        PileKind::DeepCfa,
        &[0.30, 0.40, 0.50],
        MIN_DEEP_PILE_M,
        max_length,
        MAX_DEEP_PILES_PER_COLUMN,
    ) else {
        return fail(
            format!(
                "Nenhuma estaca hélice contínua de 30 a 50 cm até {} m atendeu com até 4 por pilar; aprofunde a sondagem ou consulte engenheiro geotécnico.",
                decimal(max_length, 0)
            ),
            Vec::new(),
        );
    };
    let mut notes = vec![
        "Execução mecanizada (hélice contínua / escavada com fluido): opera abaixo do lençol freático e atravessa camadas moles superficiais.".to_owned(),
    ];
    if survey.borehole_count() > 1 {
        notes.push(format!("Capacidade governada pelo furo {governing}."));
    }
    let viable = pile.settlement_mm <= MAX_PILE_SETTLEMENT_MM + 1e-6;
    if !viable {
        notes.push(format!(
            "Recalque estimado de {} mm acima do limite preliminar de {:.0} mm.",
            decimal(pile.settlement_mm, 1),
            MAX_PILE_SETTLEMENT_MM
        ));
    }
    (
        OptionResult {
            option: FoundationOption::DeepPiles,
            viable,
            summary: format!(
                "{count} estaca(s) hélice contínua Ø {:.0} cm × {} m, {:.0} kN cada (ponta {:.0} kN, atrito {:.0} kN na ruptura; recalque est. {} mm).",
                pile.diameter_m * 100.0,
                decimal(pile.length_m, 0),
                pile.allowable_kn,
                pile.tip_kn,
                pile.shaft_kn,
                decimal(pile.settlement_mm, 1),
            ),
            notes,
            concrete_m3: concrete,
            soil_kpa: None,
            settlement_mm: Some(pile.settlement_mm),
            differential_settlement_mm: None,
        },
        Some(pile),
        count,
    )
}

fn select_best_pile(
    study: &FoundationStudy,
    survey: &SoilSurvey,
    kind: PileKind,
    diameters: &[f64],
    min_length: f64,
    max_length: f64,
    max_per_column: u32,
) -> Option<(f64, PileCapacity, u32, String)> {
    let mut best: Option<(f64, PileCapacity, u32, String)> = None;
    for &diameter in diameters {
        let mut length = min_length;
        while length <= max_length + 1e-9 {
            if let Some((pile, governing)) = survey.pile_capacity_by_kind(kind, diameter, length)
                && pile.allowable_kn > 1.0
            {
                let area = PI * diameter * diameter / 4.0;
                let mut fits = true;
                let mut count = 0_u32;
                let mut concrete = 0.0;
                for load in &study.column_loads_kn {
                    let n = ((load / pile.allowable_kn) - 1e-9).ceil().max(1.0) as u32;
                    if n > max_per_column {
                        fits = false;
                        break;
                    }
                    count += n;
                    concrete += f64::from(n) * area * length + cap_m3(n, diameter);
                }
                for (q, l) in &study.strip_loads {
                    let spacing = (pile.allowable_kn / q.max(1e-6)).min(2.5);
                    if spacing < 3.0 * diameter {
                        fits = false;
                        break;
                    }
                    let n = (l / spacing - 1e-9).ceil() as u32 + 1;
                    count += n;
                    concrete += f64::from(n) * area * length + 0.15 * 0.30 * l;
                }
                if fits
                    && best
                        .as_ref()
                        .is_none_or(|(c, _, _, _)| concrete < *c - 1e-9)
                {
                    best = Some((concrete, pile, count, governing));
                }
            }
            length += 1.0;
        }
    }
    best
}
