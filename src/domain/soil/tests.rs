use super::settlement::{
    MAX_PILE_SETTLEMENT_MM, differential_settlement_mm, estimate_footing_settlement_mm,
};
use super::study::FoundationOption;
use super::*;
use crate::domain::geometry::Point;
use crate::domain::model::Project;

fn reading(depth: f64, n: u32, soil: SoilKind) -> SptReading {
    SptReading {
        depth_m: depth,
        n,
        soil,
    }
}

fn survey(ns: &[u32], soil: SoilKind) -> SoilSurvey {
    let readings = ns
        .iter()
        .enumerate()
        .map(|(i, n)| reading(i as f64 + 1.0, *n, soil))
        .collect();
    SoilSurvey::new("SP-01".to_owned(), readings, None, 1.0).unwrap()
}

#[test]
fn the_footing_pressure_follows_the_mean_blow_count_in_the_bulb() {
    let s = survey(&[6, 8, 10, 12, 30], SoilKind::Clay);
    // 1 m wide at 1 m: readings from 1 to 3 m → N̄ = 8 → 160 kPa.
    assert!((s.footing_kpa(1.0).unwrap() - 160.0).abs() < 1e-9);
    // Capped at 400 kPa.
    let hard = survey(&[40, 40, 40], SoilKind::Sand);
    assert!((hard.footing_kpa(1.0).unwrap() - 400.0).abs() < 1e-9);
    // A survey that stops above the base gives nothing.
    let short =
        SoilSurvey::new("x".into(), vec![reading(0.5, 5, SoilKind::Clay)], None, 1.0).unwrap();
    assert!(short.footing_kpa(1.0).is_none());
}

#[test]
fn decourt_quaresma_matches_a_hand_calculation() {
    // Clay, N = 4, 6, 8, 10, 12, 14 from 1 to 6 m; Ø 25 cm × 4 m.
    let s = survey(&[4, 6, 8, 10, 12, 14], SoilKind::Clay);
    let pile = s.pile_capacity(0.25, 4.0).unwrap();
    // Tip: N at 3, 4 and 5 m → 10; Rp = 0.85 × 120 × 10 × 0.0491 = 50.1 kN.
    let area = std::f64::consts::PI * 0.25 * 0.25 / 4.0;
    assert!((pile.tip_kn - 0.85 * 120.0 * 10.0 * area).abs() < 1e-9);
    // Shaft: N at 1 and 2 m → 5; Rl = 0.8 × 10 × (5/3 + 1) × π·0.25 × 4.
    let shaft = 0.8 * 10.0 * (5.0 / 3.0 + 1.0) * std::f64::consts::PI * 0.25 * 4.0;
    assert!((pile.shaft_kn - shaft).abs() < 1e-9);
    let expected = ((pile.tip_kn + shaft) / 2.0)
        .min(pile.tip_kn / 4.0 + shaft / 1.3)
        .min(1.25 * shaft / 2.0);
    assert!((pile.allowable_kn - expected).abs() < 1e-9);
    assert!(pile.settlement_mm > 0.0 && pile.settlement_mm < MAX_PILE_SETTLEMENT_MM);
    // Deep CFA pile (α = 0.30, β = 1.00) at 6 m in the same survey.
    let cfa = s.deep_pile_capacity(0.40, 6.0).unwrap();
    assert!(cfa.allowable_kn > pile.allowable_kn);
    // Too long for the survey.
    assert!(s.pile_capacity(0.25, 7.0).is_none());
}

#[test]
fn brocas_stay_above_the_water_table_and_within_six_metres() {
    let mut s = survey(&[5, 5, 5, 5, 5, 5, 5, 5, 5, 5], SoilKind::Clay);
    assert!((s.max_broca_m() - 6.0).abs() < 1e-9);
    s.water_table_m = Some(3.4);
    assert!((s.max_broca_m() - 3.0).abs() < 1e-9);
    // Deep CFA piles ignore the water table and go up to survey depth.
    assert!((s.max_deep_pile_m() - 10.0).abs() < 1e-9);
    assert!(SoilSurvey::new("x".into(), vec![], None, 1.0).is_err());
    assert!(
        SoilSurvey::new(
            "x".into(),
            vec![
                reading(1.0, 5, SoilKind::Clay),
                reading(1.0, 6, SoilKind::Clay)
            ],
            None,
            1.0
        )
        .is_err()
    );
}

#[test]
fn multiple_boreholes_use_the_weakest_bulb_and_shallowest_water_table() {
    let sp1 = Borehole::new(
        "SP-01".to_owned(),
        (1..=10)
            .map(|d| reading(f64::from(d), 14, SoilKind::Sand))
            .collect(),
        Some(6.0),
    )
    .unwrap();
    let sp2 = Borehole::new(
        "SP-02".to_owned(),
        (1..=10)
            .map(|d| reading(f64::from(d), 6, SoilKind::Clay))
            .collect(),
        Some(2.5),
    )
    .unwrap();
    let survey = SoilSurvey::from_boreholes(vec![sp1, sp2], 1.0).unwrap();
    assert_eq!(survey.borehole_count(), 2);
    assert_eq!(survey.governing_water_table_m(), Some(2.5));
    let (n, soil, gov) = survey.governing_footing_bulb(1.0).unwrap();
    assert!((n - 6.0).abs() < 1e-9);
    assert_eq!(soil, SoilKind::Clay);
    assert_eq!(gov, "SP-02");
    assert!((survey.footing_kpa(1.0).unwrap() - 120.0).abs() < 1e-9);
    // Water table at 2.5 m blocks hand-dug brocas (< 3 m) while CFA still reaches 10 m.
    assert!((survey.max_broca_m() - 2.0).abs() < 1e-9);
    assert!((survey.max_deep_pile_m() - 10.0).abs() < 1e-9);
}

#[test]
fn elastic_settlement_matches_hand_calculation() {
    // 1.0 × 1.0 m square footing, q = 150 kPa, clay N̄ = 10 → E_s = 25 MPa, ν = 0.40, Is = 0.82.
    // s = 150 * 1.0 * (1 - 0.16) * 0.82 / 25_000 = 0.0041328 m = 4.1328 mm.
    let s = estimate_footing_settlement_mm(150.0, 1.0, 1.0, SoilKind::Clay, 10.0);
    assert!((s - 4.1328).abs() < 1e-3, "got {s}");
    assert!((differential_settlement_mm(12.0, 4.0) - 8.0).abs() < 1e-9);
    // Equal-load footings still expect 50% heterogeneity differential settlement (Skempton–MacDonald).
    assert!((differential_settlement_mm(10.0, 10.0) - 5.0).abs() < 1e-9);
}

#[test]
fn the_study_picks_footings_on_firm_soil_brocas_on_soft_top_and_deep_piles_under_water() {
    let mut project = Project::default();
    project.set_terrain(Point::new(0.0, 0.0), Point::new(15.0, 15.0));
    project
        .add_room(Point::new(1.0, 1.0), Point::new(6.0, 6.0), "Sala")
        .unwrap();
    project.generate_structure();

    // 1. Firm sand → footings win.
    project.set_soil_survey(Some(survey(&[10, 12, 14, 16, 18, 20], SoilKind::Sand)));
    let study = project.foundation_study();
    assert_eq!(study.options.len(), 4);
    assert_eq!(study.recommended, Some(FoundationOption::Footings));

    // 2. Soft upper clay over firm layer, dry → brocas win over footings.
    project.set_soil_survey(Some(survey(&[2, 3, 4, 12, 15, 18], SoilKind::Clay)));
    let study = project.foundation_study();
    assert!(
        !study
            .options
            .iter()
            .find(|o| o.option == FoundationOption::Footings)
            .unwrap()
            .viable
    );
    assert_eq!(study.recommended, Some(FoundationOption::BoredPiles));

    // 3. Very soft clay down to 5 m AND shallow water table (1.5 m), firm sand at 7–10 m
    //    → footings, raft and hand-dug brocas fail, deep CFA piles succeed!
    let deep_readings: Vec<SptReading> = [1, 1, 2, 2, 2, 10, 14, 18, 22, 25]
        .iter()
        .enumerate()
        .map(|(i, &n)| {
            let soil = if i < 5 {
                SoilKind::Clay
            } else {
                SoilKind::Sand
            };
            reading(i as f64 + 1.0, n, soil)
        })
        .collect();
    let deep_survey = SoilSurvey::new("SP-01".to_owned(), deep_readings, Some(1.5), 1.0).unwrap();
    project.set_soil_survey(Some(deep_survey));
    let study = project.foundation_study();
    assert!(
        !study
            .options
            .iter()
            .find(|o| o.option == FoundationOption::Footings)
            .unwrap()
            .viable
    );
    assert!(
        !study
            .options
            .iter()
            .find(|o| o.option == FoundationOption::Raft)
            .unwrap()
            .viable
    );
    assert!(
        !study
            .options
            .iter()
            .find(|o| o.option == FoundationOption::BoredPiles)
            .unwrap()
            .viable
    );
    let cfa = study
        .options
        .iter()
        .find(|o| o.option == FoundationOption::DeepPiles)
        .unwrap();
    assert!(cfa.viable, "{cfa:?}");
    assert_eq!(study.recommended, Some(FoundationOption::DeepPiles));
    assert!(study.deep_pile.is_some() && study.deep_piles > 0);
}

#[test]
fn old_single_borehole_json_deserializes_cleanly() {
    let legacy = r#"{
        "name": "SP-01",
        "readings": [
            {"depth_m": 1.0, "n": 6, "soil": "clay"},
            {"depth_m": 2.0, "n": 8, "soil": "clay"}
        ],
        "water_table_m": 4.0,
        "footing_depth_m": 1.0
    }"#;
    let parsed: SoilSurvey = serde_json::from_str(legacy).unwrap();
    assert_eq!(parsed.borehole_count(), 1);
    assert!(parsed.extra_boreholes.is_empty());
    assert!((parsed.footing_kpa(1.0).unwrap() - 140.0).abs() < 1e-9);
}
