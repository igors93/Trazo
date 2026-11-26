//! Comprehensive Phase 2 unit & integration tests covering T-08, T-09, T-10 and T-11.

use std::collections::BTreeMap;

use serde_json::json;

use super::{
    BuildingCode, BuildingCodes, CodeDiscipline, RuleKind,
    checks::check_project_codes,
    electrical_demand::{
        BuildingDemandInput, DwellingGroup, EntrancePickInput, UnitDemandInput,
        calculate_building_demand, calculate_unit_demand, pick_entrance_category,
    },
    site::KeraunicLevel,
    stairs::{StairLocks, apply_stair_proposal, solve_stair, stair_findings, stair_headroom},
    xml,
};
use crate::domain::{
    circulation::StairShape,
    fixtures::FixtureKind,
    geometry::Point,
    model::{OpeningKind, Project},
    openings::WindowType,
    utility_profiles::UtilityProfile,
    verdict::{Discipline, Light, safety_summary},
};

// ===========================================================================
// T-08 — Layered codes, edition pinning, WHOLE sections, XML & JSON round-trip
// ===========================================================================

#[test]
fn layered_resolution_base_amendment_and_project_override() {
    let mut codes = BuildingCodes::default();
    codes.apply_preset("brasil").unwrap();

    // Base (nbr-9050) defines stairs.width_min = 1200; amendment (codigo-obras-br) defines doors.frame_allowance = 40.
    let stairs = codes.resolve_section(CodeDiscipline::Architectural, "stairs");
    assert_eq!(stairs.value["width_min"], json!(1200));
    let doors = codes.resolve_section(CodeDiscipline::Architectural, "doors");
    assert_eq!(doors.value["frame_allowance"], json!(40.0));
    assert!(
        doors.rules["clear_width_min"]
            .source
            .as_deref()
            .unwrap_or("")
            .contains("6.11.2.4")
    );

    // Project-level override wins over base/amendment without recompiling.
    codes
        .set_project_rule(
            CodeDiscipline::Architectural,
            "stairs",
            "width_min",
            json!(1350.0),
        )
        .unwrap();
    let stairs_after = codes.resolve_section(CodeDiscipline::Architectural, "stairs");
    assert_eq!(stairs_after.value["width_min"], json!(1350.0));
    assert_eq!(stairs_after.rules["width_min"].kind, RuleKind::Assumption);

    // Resetting restores base value.
    codes.reset_project_rule(CodeDiscipline::Architectural, "stairs", Some("width_min"));
    let stairs_reset = codes.resolve_section(CodeDiscipline::Architectural, "stairs");
    assert_eq!(stairs_reset.value["width_min"], json!(1200));
}

#[test]
fn whole_sections_never_cross_contaminate_and_block_project_rule_edit() {
    let mut codes = BuildingCodes::default();
    codes
        .set_base(CodeDiscipline::Electrical, "nbr-5410")
        .unwrap();
    codes
        .add_amendment(CodeDiscipline::Electrical, "cpfl")
        .unwrap();
    codes
        .add_amendment(CodeDiscipline::Electrical, "cemig")
        .unwrap();

    // Stacking CEMIG on top of CPFL replaces `utility_profile` as a WHOLE section.
    let resolved = codes.resolve_section(CodeDiscipline::Electrical, "utility_profile");
    assert_eq!(resolved.value["single_phase_max_kva"], json!(8.0));

    // Direct project-level rule override on a WHOLE section is refused.
    let err = codes.set_project_rule(
        CodeDiscipline::Electrical,
        "utility_profile",
        "single_phase_max_kva",
        json!(99.0),
    );
    assert!(err.is_err());
}

#[test]
fn edition_pinning_and_drift_detection_work() {
    let mut codes = BuildingCodes::default();
    codes
        .set_base(CodeDiscipline::Architectural, "nbr-9050")
        .unwrap();
    assert_eq!(
        codes
            .architectural
            .editions
            .get("nbr-9050")
            .map(String::as_str),
        Some("2020")
    );
    assert!(codes.edition_drifts().is_empty());

    // Simulate an older saved project pinned to a previous edition.
    codes
        .architectural
        .editions
        .insert("nbr-9050".to_owned(), "2015".to_owned());
    let drifts = codes.edition_drifts();
    assert_eq!(drifts.len(), 1);
    assert_eq!(drifts[0].pinned, "2015");
    assert_eq!(drifts[0].available, "2020");
}

#[test]
fn xml_format2_and_json_export_import_round_trip_with_left_out() {
    let mut codes = BuildingCodes::default();
    codes.apply_preset("brasil").unwrap();

    // Format-2 XML single-discipline round-trip
    let pack = codes.export_effective_pack(CodeDiscipline::Architectural);
    let xml_str = xml::export_code_xml(&pack);
    assert!(xml_str.contains("<building-code format=\"2\""));
    let imported = xml::import_xml(&xml_str).unwrap();
    assert_eq!(imported.codes.len(), 1);
    assert_eq!(imported.codes[0].discipline, CodeDiscipline::Architectural);
    assert_eq!(
        imported.codes[0].sections["stairs"]["riser_max"],
        pack.sections["stairs"]["riser_max"]
    );

    // Multi-discipline bundle XML round-trip
    let all_xml = codes.export_all_xml();
    assert!(all_xml.contains("<building-codes format=\"2\">"));
    let bundle = xml::import_auto(&all_xml).unwrap();
    assert_eq!(bundle.codes.len(), 5);

    // Lenient import collects unknown future tags in `left_out` without failing
    let future_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<building-code format="2" code="municipal-teste" discipline="architectural" name="COE Teste" edition="2026">
  <future-tag>ignorado</future-tag>
  <section name="doors">
    <rule name="clear_width_min" value="850.0" source="Lei Municipal 123" kind="requirement"/>
  </section>
</building-code>"#;
    let lenient = xml::import_xml(future_xml).unwrap();
    assert_eq!(lenient.codes.len(), 1);
    assert!(!lenient.left_out.is_empty());
    assert_eq!(
        lenient.codes[0].sections["doors"]["clear_width_min"],
        json!(850.0)
    );

    // JSON round-trip
    let json_str = xml::export_bundle_json(std::slice::from_ref(&pack)).unwrap();
    let json_imported = xml::import_auto(&json_str).unwrap();
    assert_eq!(json_imported.codes.len(), 1);
    assert_eq!(json_imported.codes[0].code, pack.code);
}

// ===========================================================================
// T-09 — Shared Site & Climate parameters + wind synchronization
// ===========================================================================

#[test]
fn applying_city_preset_updates_climate_and_syncs_structural_wind_v0() {
    let mut project = Project::default();
    assert!((project.structure.wind.v0_ms - 35.0).abs() < 1e-6);

    let preset = project.apply_site_preset("florianopolis_sc").unwrap();
    assert_eq!(preset.city, "Florianópolis");
    assert_eq!(project.codes.site.bioclimatic_zone, Some(3));
    assert_eq!(project.codes.site.rain_intensity_mm_h, Some(146.0));
    assert_eq!(project.codes.site.thunderstorms, Some(KeraunicLevel::Aq3));
    // V0 propagated automatically to NBR 6123 structural wind!
    assert!((project.structure.wind.v0_ms - 45.0).abs() < 1e-6);
    assert!(project.codes.site.assumption_line().is_some());

    // Invalid bioclimatic zone is rejected cleanly
    let mut bad = project.codes.site.clone();
    bad.bioclimatic_zone = Some(9);
    assert!(project.set_site_climate(bad).is_err());
}

// ===========================================================================
// T-10 — 26 stair rules, 3D headroom, interactive solver, ramps, doors, rooms
// ===========================================================================

#[test]
fn stair_26_rules_headroom_and_interactive_solver() {
    let mut project = Project::default();
    project.codes.apply_preset("brasil").unwrap();

    // Create a steep, narrow stair violating NBR 9050 (width 0.90 < 1.20, tread 0.24 < 0.28, 3.60m > 3.20m flight rise)
    let stair_id = project
        .add_stair(Point::new(0.0, 0.0), StairShape::Straight, 0.90, 3.60, 0.24)
        .unwrap();
    if let Some(el) = project.circulations.iter_mut().find(|c| c.id == stair_id) {
        el.stair_details.open_risers = true;
        el.stair_details.nosing_m = 0.03; // 30 mm > 15 mm max
        el.stair_details.guard_height_m = Some(0.90); // 900 mm < 1050 mm min
    }

    let stair = project.circulations[0].clone();
    let findings = stair_findings(&project, &stair);
    let rule_names: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
    assert!(rule_names.contains(&"width_min"), "{rule_names:?}");
    assert!(rule_names.contains(&"going_min"), "{rule_names:?}");
    assert!(rule_names.contains(&"flight_rise_max"), "{rule_names:?}");
    assert!(rule_names.contains(&"open_risers"), "{rule_names:?}");
    assert!(rule_names.contains(&"nosing_max"), "{rule_names:?}");
    assert!(rule_names.contains(&"guard_min"), "{rule_names:?}");

    // Fix non-geometric flags and test interactive solver (`solve_stair`)
    project.circulations[0].stair_details.open_risers = false;
    project.circulations[0].stair_details.nosing_m = 0.0;
    project.circulations[0].stair_details.guard_height_m = Some(1.10);

    let proposals = solve_stair(&project, &project.circulations[0], StairLocks::default());
    assert!(!proposals.is_empty());
    assert!(
        proposals[0].compliant,
        "proposal should be compliant: {:?}",
        proposals[0]
    );
    assert!(proposals[0].width_m >= 1.20 - 1e-6);
    assert!(!proposals[0].intermediate_landings.is_empty());

    apply_stair_proposal(&mut project.circulations[0], &proposals[0]).unwrap();
    let after = stair_findings(&project, &project.circulations[0]);
    assert!(
        after.is_empty(),
        "expected 0 findings after solver: {after:?}"
    );

    // Locking width keeps original width in solver proposals
    project.circulations[0].kind = crate::domain::circulation::CirculationKind::Stair {
        shape: StairShape::Straight,
        width_m: 0.90,
        total_rise_m: 2.80,
        tread_m: 0.25,
        landing_depth_m: 1.20,
    };
    let locked = solve_stair(
        &project,
        &project.circulations[0],
        StairLocks {
            width: true,
            ..StairLocks::default()
        },
    );
    assert!(!locked.is_empty());
    assert!((locked[0].width_m - 0.90).abs() < 1e-6);
}

#[test]
fn stair_3d_headroom_detects_low_overhead_beam() {
    let mut project = Project::default();
    project.codes.apply_preset("brasil").unwrap();
    project.wall_height = 2.80;

    let stair_id = project
        .add_stair(Point::new(0.0, 0.0), StairShape::Straight, 1.20, 2.80, 0.30)
        .unwrap();
    // Add an overhead beam crossing the upper half of the stair at x = 3.0m with height 0.50m (underside z = 2.30m)
    // At step ~10 (z ≈ 1.75m), clearance = 2.30 - 1.75 = 0.55m << 2.10m!
    let c1 = project
        .add_fixture(FixtureKind::Column, Point::new(3.0, -1.0), 0.20, 0.20, 0)
        .unwrap();
    let c2 = project
        .add_fixture(FixtureKind::Column, Point::new(3.0, 2.5), 0.20, 0.20, 0)
        .unwrap();
    project.add_beam(c1, c2, 0.20, 0.50).unwrap();

    let (rules, _) = super::stairs::resolve_stair_rules(&project);
    let el = project
        .circulations
        .iter()
        .find(|c| c.id == stair_id)
        .unwrap();
    let hr = stair_headroom(&project, el, &rules);
    assert!(hr.obstruction.is_some());
    assert!(hr.min_clearance_mm.unwrap() < rules.headroom);

    let findings = stair_findings(&project, el);
    assert!(findings.iter().any(|f| f.rule == "headroom"));
}

#[test]
fn ramps_doors_and_rooms_light_ventilation_surface_in_safety_summary() {
    let mut project = Project::default();
    project.codes.apply_preset("brasil").unwrap();

    // 1. Ramp: 0.90 m rise requires <= 6.25% (NBR 9050 Tabela 4); length 11.25 m gives 8.0% -> fails!
    project
        .add_ramp(Point::new(0.0, 10.0), 1.00, 11.25, 0.90)
        .unwrap();

    // 2. Room: "Quarto" 3.0x3.0 (9 m² >= 8 m²), NO windows -> fails light_min & vent_min
    let room_id = project
        .add_room(Point::new(0.0, 0.0), Point::new(3.0, 3.0), "Quarto")
        .unwrap();
    let wall_id = project
        .rooms
        .iter()
        .find(|r| r.id == room_id)
        .unwrap()
        .wall_ids[0];

    // 3. Door: 0.80 m nominal with 40 mm frame allowance => 760 mm clear < 800 mm -> fails clear_width_min!
    let door_id = project
        .add_opening(wall_id, OpeningKind::Door, 0.3)
        .unwrap();
    project.update_opening_width(door_id, 0.80).unwrap();

    let report = check_project_codes(&project);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.section == "ramps" && f.rule == "slope_by_rise")
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.section == "ramps" && f.rule == "width_min")
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.section == "doors" && f.rule == "clear_width_min")
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.section == "rooms" && f.rule == "light_min")
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.section == "rooms" && f.rule == "vent_min")
    );

    // All architectural findings appear in Safety traffic-light summary
    let summary = safety_summary(&project);
    assert!(
        summary
            .verdicts
            .iter()
            .any(|v| v.discipline == Discipline::Architecture && v.light() == Light::Red)
    );

    // Adding a fixed window of 1.50x1.00 (1.5 m² / 9 m² = 16.7% >= 12.5%) satisfies light_min,
    // but NOT vent_min (fixed window has vent factor 0.0)!
    let win_wall = project
        .rooms
        .iter()
        .find(|r| r.id == room_id)
        .unwrap()
        .wall_ids[1];
    let win_id = project
        .add_opening(win_wall, OpeningKind::Window, 0.5)
        .unwrap();
    project.update_opening_width(win_id, 1.50).unwrap();
    if let Some(w) = project.openings.iter_mut().find(|o| o.id == win_id) {
        w.window_type = Some(WindowType::Fixed);
    }
    let after_fixed = check_project_codes(&project);
    assert!(!after_fixed.findings.iter().any(|f| f.rule == "light_min"));
    assert!(after_fixed.findings.iter().any(|f| f.rule == "vent_min"));

    // Switching the window to Casement (0.90 openable fraction -> 1.35 / 9 = 15% >= 6.25%) passes vent_min too!
    if let Some(w) = project.openings.iter_mut().find(|o| o.id == win_id) {
        w.window_type = Some(WindowType::Casement);
    }
    let after_casement = check_project_codes(&project);
    assert!(!after_casement.findings.iter().any(|f| f.rule == "vent_min"));
}

// ===========================================================================
// T-11 — 9 Brazilian electrical utilities + collective multifamily demand
// ===========================================================================

#[test]
fn cpfl_ged13_full_d_a_to_i_and_all_nine_utilities_tables() {
    // 1. CPFL GED-13 full D = a..i
    let cpfl_std = UtilityProfile::Cpfl.unit_demand_standard(None).unwrap();
    let cpfl_res = calculate_unit_demand(
        &cpfl_std,
        &UnitDemandInput {
            system: "127/220".to_owned(),
            lighting_va: 1500.0,
            outlets_va: 3000.0, // a_inst = 4.5 kVA -> factor 0.52 -> 2.34 kVA
            heaters_kw: vec![5.5, 5.5], // 2 showers -> factor 1.00 -> 11.0 kVA
            appliances_kw: vec![1.5, 1.2],
            air_kva: vec![1.4],
            motors_kva: vec![1.0],
            ..UnitDemandInput::default()
        },
    );
    assert!((cpfl_res.terms["a_iluminacao_tug"] - 2.34).abs() < 0.05);
    assert!((cpfl_res.terms["b_aquecimento"] - 11.0).abs() < 0.05);
    assert!(cpfl_res.demand_kva > 15.0);
    assert_eq!(cpfl_res.category.as_deref(), Some("B2"));
    assert_eq!(cpfl_res.breaker_a, Some(80.0));

    // 2. Enel annexes
    let enel_std = UtilityProfile::EnelSp.unit_demand_standard(None).unwrap();
    let enel_res = calculate_unit_demand(
        &enel_std,
        &UnitDemandInput {
            network: "aerial".to_owned(),
            installed_kw_override: Some(12.0),
            ..UnitDemandInput::default()
        },
    );
    assert_eq!(enel_res.category.as_deref(), Some("B2"));

    // 3. COPEL NTC 901100 (urban + rural 127/254)
    let copel = UtilityProfile::Copel.entrance_utility(None).unwrap();
    let urban = pick_entrance_category(
        &copel,
        &EntrancePickInput {
            system: "127/220".to_owned(),
            use_kind: "individual".to_owned(),
            demand_kva: Some(7.5),
            ..EntrancePickInput::default()
        },
    )
    .unwrap();
    assert_eq!(urban.category.as_deref(), Some("14"));
    let rural = pick_entrance_category(
        &copel,
        &EntrancePickInput {
            system: "127/254".to_owned(),
            use_kind: "individual".to_owned(),
            demand_kva: Some(12.0),
            ..EntrancePickInput::default()
        },
    )
    .unwrap();
    assert!(rural.category.is_some());

    // 4. Energisa (9 members) & Equatorial
    let energisa = UtilityProfile::Energisa.entrance_utility(None).unwrap();
    assert_eq!(energisa.members.len(), 9);
    let eng_pick = pick_entrance_category(
        &energisa,
        &EntrancePickInput {
            system: "127/220".to_owned(),
            use_kind: "individual".to_owned(),
            member: Some("EMT".to_owned()),
            installed_kw: Some(12.0),
            demand_kva: Some(10.0),
            ..EntrancePickInput::default()
        },
    )
    .unwrap();
    assert!(eng_pick.category.is_some());

    let equatorial = UtilityProfile::Equatorial.entrance_utility(None).unwrap();
    let eq_pick = pick_entrance_category(
        &equatorial,
        &EntrancePickInput {
            system: "127/220".to_owned(),
            use_kind: "individual".to_owned(),
            installed_kw: Some(14.0),
            ..EntrancePickInput::default()
        },
    )
    .unwrap();
    assert!(eq_pick.category.is_some());

    // 5. Fecoergs (15 cooperatives + COPREL member override) & Hidropan
    let fecoergs = UtilityProfile::Fecoergs.entrance_utility(None).unwrap();
    assert_eq!(fecoergs.members.len(), 15);
    let fec_pick = pick_entrance_category(
        &fecoergs,
        &EntrancePickInput {
            system: "127/220".to_owned(),
            use_kind: "individual".to_owned(),
            member: Some("CERTEL".to_owned()),
            installed_kw: Some(12.0),
            ..EntrancePickInput::default()
        },
    )
    .unwrap();
    assert_eq!(fec_pick.category.as_deref(), Some("B1"));

    let hidropan = UtilityProfile::Hidropan.entrance_utility(None).unwrap();
    let hid_pick = pick_entrance_category(
        &hidropan,
        &EntrancePickInput {
            system: "220/380".to_owned(),
            use_kind: "individual".to_owned(),
            installed_kw: Some(10.0),
            ..EntrancePickInput::default()
        },
    )
    .unwrap();
    assert_eq!(hid_pick.category.as_deref(), Some("A"));
}

#[test]
fn collective_multifamily_demand_matches_cemig_celesc_fecoergs_hidropan() {
    // Small 3-storey residential building: 6 apartments of 70 m²
    let input = BuildingDemandInput {
        dwellings: vec![DwellingGroup {
            count: 6,
            area_m2: 70.0,
        }],
        common_installed_kw: BTreeMap::from([("lighting".to_owned(), 2.0)]),
        ..BuildingDemandInput::default()
    };

    // Celesc N-321.0003: factor 1.2, factor_on_common = true, by_area(70) = 1.57
    let celesc_m = UtilityProfile::Celesc.building_demand_method(None).unwrap();
    let celesc = calculate_building_demand(&celesc_m, &input).unwrap();
    assert_eq!(celesc.total_dwellings, 6);
    assert!((celesc.dwelling_demand_d - 1.57).abs() < 1e-6);
    assert!(celesc.total_demand > 10.0);
    assert!(celesc.entrance_breaker_a.is_some());

    // Celesc erratum at n = 62 (corrected 42.06 vs printed 42.05)
    let err_input_corr = BuildingDemandInput {
        dwellings: vec![DwellingGroup {
            count: 62,
            area_m2: 70.0,
        }],
        as_printed: false,
        ..BuildingDemandInput::default()
    };
    let err_input_print = BuildingDemandInput {
        as_printed: true,
        ..err_input_corr.clone()
    };
    let res_corr = calculate_building_demand(&celesc_m, &err_input_corr).unwrap();
    let res_print = calculate_building_demand(&celesc_m, &err_input_print).unwrap();
    assert!((res_corr.diversity_factor_f - 42.06).abs() < 1e-6);
    assert!((res_print.diversity_factor_f - 42.05).abs() < 1e-6);
    assert!(res_corr.erratum_applied.is_some());

    // CEMIG ND-5.2: factor 1.4, factor_on_common = false
    let cemig_m = UtilityProfile::Cemig.building_demand_method(None).unwrap();
    let cemig = calculate_building_demand(&cemig_m, &input).unwrap();
    assert!((cemig.safety_factor - 1.4).abs() < 1e-6);
    assert!(cemig.entrance_breaker_a.is_some());

    // Fecoergs & Hidropan collective demand
    let fec_m = UtilityProfile::Fecoergs
        .building_demand_method(None)
        .unwrap();
    let fec = calculate_building_demand(&fec_m, &input).unwrap();
    assert!(fec.total_demand > 0.0);

    let hid_m = UtilityProfile::Hidropan
        .building_demand_method(None)
        .unwrap();
    let hid = calculate_building_demand(&hid_m, &input).unwrap();
    assert!(hid.total_demand > 0.0);
}

#[test]
fn installing_custom_utility_pack_in_project_changes_electrical_sizing() {
    let mut project = Project::default();
    project.set_terrain(Point::new(-1.0, -1.0), Point::new(20.0, 20.0));
    let (pw, pd) = FixtureKind::Panel.default_size();
    project
        .add_fixture(FixtureKind::Panel, Point::new(1.0, 1.0), pw, pd, 0)
        .unwrap();
    let (ow, od) = FixtureKind::OutletSpecific.default_size();
    let f1 = project
        .add_fixture(FixtureKind::OutletSpecific, Point::new(3.0, 1.0), ow, od, 0)
        .unwrap();
    project
        .update_electrical(f1, Some(Some(1)), Some(6000.0), None)
        .unwrap();

    // Baseline Generic: single_phase_max_kva = 8.0, min_breaker_a = 32.0
    let before = project.load_schedule();
    let before_breaker = before.entrance.as_ref().unwrap().breaker_a;

    // Install custom regional pack raising `min_breaker_a` to 80 A
    let mut custom_pack = BuildingCode {
        code: "generic".to_owned(),
        discipline: CodeDiscipline::Electrical,
        name: "Distribuidora Regional Customizada".to_owned(),
        edition: "2026".to_owned(),
        ..BuildingCode::default()
    };
    custom_pack.sections.insert(
        "utility_profile".to_owned(),
        json!({
            "single_phase_max_kva": 15.0,
            "two_phase_max_kva": 30.0,
            "max_bt_kva": 75.0,
            "min_breaker_a": 80.0,
            "min_wire_mm2": 25.0,
            "general_table": [[10.0, 0.5]],
            "general_over_max": 0.4,
            "specific_table": [1.0],
            "citation": {
                "standard": "Norma Regional Teste",
                "edition": "2026",
                "tables": "Tabela Regional",
                "nominal_system": "127/220 V",
                "verification_note": "Teste"
            }
        }),
    );
    project.codes.install_own_code(custom_pack).unwrap();

    let after = project.load_schedule();
    let after_entrance = after.entrance.as_ref().unwrap();
    assert!(after_entrance.breaker_a >= Some(80.0));
    assert!(after_entrance.breaker_a > before_breaker);
    assert!(after_entrance.wire_mm2 >= Some(25.0));
}
