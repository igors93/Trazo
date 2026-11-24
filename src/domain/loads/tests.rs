//! Validation & regression tests for #105: NBR 6120:2019 loads, NBR 8681:2003 /
//! NBR 6118:2023 §11.8 ELU & ELS combinations, and NBR 6123 building wind.

use crate::domain::{
    fixtures::FixtureKind,
    frame2d::{
        model::{EndRelease, Frame2D, MemberKind, SupportKind},
        solver::solve_frame,
    },
    geometry::Point,
    model::Project,
    structure::OccupancyClass,
    verdict::{Light, safety_summary},
    wind::WindSettings,
};

use super::{
    CombinationCategory, GAMMA_G_EQU_FAV, GAMMA_G_FAV, GAMMA_G_UNFAV, GAMMA_Q, GAMMA_W,
    LoadCaseKind, PSI0_WIND, PSI1_WIND, WindDirection, build_nbr8681_combinations,
    classify_room_nbr6120, nbr6123_wall_coeffs,
};

fn close(actual: f64, expected: f64, rel_tol: f64) -> bool {
    (actual - expected).abs() <= rel_tol * expected.abs().max(1e-12)
}

/// Published worked example #1 — ABNT NBR 6120:2019 Table 10 & NBR 8681:2003 Table 6
/// / NBR 6118:2023 Table 11.2:
/// Verifies room classification, `ψ0 / ψ1 / ψ2` per occupancy class, and exact
/// hand-calculated ELU normal, ELU-EQU, ELS-QP, ELS-F and ELS-R combinations.
///
/// Reference hand example (Araújo / Carvalho & Figueiredo Filho, *Cálculo e
/// Detalhamento de Estruturas Usuais de Concreto Armado*, Cap. 2):
/// Given characteristic loads on a residential building element:
/// `G1 = 100 kN`, `G2 = 60 kN` (`G = 160 kN`), `Q = 50 kN` (residential: `ψ0=0.5, ψ1=0.4, ψ2=0.3`),
/// `W = 40 kN` (wind: `ψ0=0.6, ψ1=0.3, ψ2=0.0`):
/// - `ELU-1 (Q principal)`: `1.4·160 + 1.4·50 = 294.0 kN`
/// - `ELU-2 (Q principal + W+ sec.)`: `294.0 kN` vert, `1.4·0.6·40 = 33.6 kN` horiz
/// - `ELU-4 (W+ principal + Q sec.)`: `1.4·160 + 1.4·0.5·50 = 259.0 kN` vert, `1.4·40 = 56.0 kN` horiz
/// - `ELU-6 (G fav + W+ principal)`: `1.0·160 = 160.0 kN` vert, `56.0 kN` horiz
/// - `ELU-EQU+ (overturning)`: `0.9·160 = 144.0 kN` vert, `56.0 kN` horiz
/// - `ELS-QP`: `160 + 0.3·50 = 175.0 kN` vert, `0.0 kN` horiz
/// - `ELS-F1 (Q principal)`: `160 + 0.4·50 = 180.0 kN` vert, `0.0 kN` horiz
/// - `ELS-F2 (W+ principal)`: `160 + 0.3·50 = 175.0 kN` vert, `0.3·40 = 12.0 kN` horiz
/// - `ELS-R1 (Q principal + W+ sec.)`: `160 + 50 = 210.0 kN` vert, `0.3·40 = 12.0 kN` horiz
/// - `ELS-R3 (W+ principal + Q sec.)`: `160 + 0.4·50 = 180.0 kN` vert, `40.0 kN` horiz
#[test]
fn nbr_8681_combinations_match_worked_hand_example_across_occupancies() {
    let (g1, g2, q, w) = (100.0, 60.0, 50.0, 40.0);
    let res_combos = build_nbr8681_combinations(OccupancyClass::Residential);
    let find = |code: &str| res_combos.iter().find(|c| c.code == code).unwrap();

    // Verify factors & vertical/horizontal combined values against exact hand values.
    let elu1 = find("ELU-1");
    assert!(close(elu1.combine(g1, g2, q, 0.0), 294.0, 1e-12));
    assert!((elu1.gamma_w).abs() < 1e-12);

    let elu2 = find("ELU-2");
    assert!(close(elu2.combine(g1, g2, q, 0.0), 294.0, 1e-12));
    assert!(close(elu2.gamma_w * w, 33.6, 1e-12));

    let elu4 = find("ELU-4");
    assert!(close(elu4.combine(g1, g2, q, 0.0), 259.0, 1e-12));
    assert!(close(elu4.gamma_w * w, 56.0, 1e-12));

    let elu6 = find("ELU-6");
    assert!(close(elu6.gamma_g1, GAMMA_G_FAV, 1e-12));
    assert!(close(elu6.combine(g1, g2, q, 0.0), 160.0, 1e-12));
    assert!(close(elu6.gamma_w * w, 56.0, 1e-12));

    let equ = find("ELU-EQU+");
    assert_eq!(equ.category, CombinationCategory::EluEquilibrium);
    assert!(close(equ.gamma_g1, GAMMA_G_EQU_FAV, 1e-12));
    assert!(close(equ.combine(g1, g2, q, 0.0), 144.0, 1e-12));
    assert!(close(equ.gamma_w * w, 56.0, 1e-12));

    let els_qp = find("ELS-QP");
    assert!(close(els_qp.combine(g1, g2, q, 0.0), 175.0, 1e-12));
    assert!((els_qp.gamma_w).abs() < 1e-12);

    let els_f1 = find("ELS-F1");
    assert!(close(els_f1.combine(g1, g2, q, 0.0), 180.0, 1e-12));

    let els_f2 = find("ELS-F2");
    assert!(close(els_f2.combine(g1, g2, q, 0.0), 175.0, 1e-12));
    assert!(close(els_f2.gamma_w * w, 12.0, 1e-12));

    let els_r1 = find("ELS-R1");
    assert!(close(els_r1.combine(g1, g2, q, 0.0), 210.0, 1e-12));
    assert!(close(els_r1.gamma_w * w, 12.0, 1e-12));

    let els_r3 = find("ELS-R3");
    assert!(close(els_r3.combine(g1, g2, q, 0.0), 180.0, 1e-12));
    assert!(close(els_r3.gamma_w * w, 40.0, 1e-12));

    // Office (ψ0=0.70, ψ1=0.60, ψ2=0.40) and Garage (ψ0=0.80, ψ1=0.70, ψ2=0.60)
    // must automatically raise combination factors per NBR 8681 Table 6.
    let office = build_nbr8681_combinations(OccupancyClass::Office);
    let off_elu4 = office.iter().find(|c| c.code == "ELU-4").unwrap();
    assert!(close(off_elu4.gamma_q, GAMMA_Q * 0.70, 1e-12)); // 0.98
    let off_qp = office.iter().find(|c| c.code == "ELS-QP").unwrap();
    assert!(close(off_qp.gamma_q, 0.40, 1e-12));

    let garage = build_nbr8681_combinations(OccupancyClass::Garage);
    let gar_elu4 = garage.iter().find(|c| c.code == "ELU-4").unwrap();
    assert!(close(gar_elu4.gamma_q, GAMMA_Q * 0.80, 1e-12)); // 1.12
    let gar_qp = garage.iter().find(|c| c.code == "ELS-QP").unwrap();
    assert!(close(gar_qp.gamma_q, 0.60, 1e-12));

    // NBR 6120:2019 Table 10 room classification checks.
    assert!(close(
        classify_room_nbr6120("Sala", OccupancyClass::Residential).min_live_kn_m2,
        1.5,
        1e-12
    ));
    assert!(close(
        classify_room_nbr6120("Área de Serviço", OccupancyClass::Residential).min_live_kn_m2,
        2.0,
        1e-12
    ));
    assert!(close(
        classify_room_nbr6120("Varanda", OccupancyClass::Residential).min_live_kn_m2,
        2.5,
        1e-12
    ));
    assert!(close(
        classify_room_nbr6120("Garagem", OccupancyClass::Residential).min_live_kn_m2,
        3.0,
        1e-12
    ));
}

/// Published worked example #2 — ABNT NBR 6123 Tables 1, 2, 3 and 4:
/// Rectangular closed building `a = 12 m`, `b = 6 m`, `H = 6 m` (2 storeys of 3 m),
/// `V0 = 40 m/s`, Terrain Category III, `S1 = 1.0`, `S3 = 1.0` (Group 2):
/// - Max dimension = `12 m ≤ 20 m` -> Class A (`b = 0.94, Fr = 1.00, p = 0.10`)
/// - Aspect ratios: `h/b = 6/6 = 1.0` (`1/2 < h/b ≤ 3/2`), `a/b = 12/6 = 2.0` (`2 ≤ a/b ≤ 4`)
/// - NBR 6123 Table 4:
///   - Wind on long face `a = 12 m` (`α = 0°`): `Cpe,barl = +0.70`, `Cpe,sot = -0.60`, `Ca = 1.30`
///   - Wind on short face `b = 6 m` (`α = 90°`): `Cpe,barl = +0.70`, `Cpe,sot = -0.30`, `Ca = 1.00`
/// - Storey 1 (`z_top = 3 m ≤ 5 m`, clamped to `z = 5 m`):
///   `S2(5) = 0.94 · 1.00 · (5/10)^0.10 = 0.877052...`, `Vk = 35.082 m/s`, `q = 0.75445 kPa`
/// - Storey 2 (`z_top = 6 m`):
///   `S2(6) = 0.94 · 1.00 · (6/10)^0.10 = 0.893186...`, `Vk = 35.727 m/s`, `q = 0.78246 kPa`
#[test]
fn nbr_6123_building_wind_matches_published_table4_worked_example() {
    // 1. Direct verification of NBR 6123 Table 4 wall coefficients including a/b interpolation.
    let long_face = nbr6123_wall_coeffs(6.0, 12.0, 6.0, true, true);
    assert!(long_face.within_table4);
    assert!(close(long_face.cpe_windward, 0.70, 1e-12));
    assert!(close(long_face.cpe_leeward, -0.60, 1e-12));
    assert!(close(long_face.cpe_side_edge, -1.00, 1e-12));
    assert!(close(long_face.ca_drag, 1.30, 1e-12));

    let short_face = nbr6123_wall_coeffs(6.0, 12.0, 6.0, false, true);
    assert!(close(short_face.cpe_windward, 0.70, 1e-12));
    assert!(close(short_face.cpe_leeward, -0.30, 1e-12));
    assert!(close(short_face.ca_drag, 1.00, 1e-12));

    // Linear interpolation for 3/2 < a/b < 2 (at midpoint a/b = 1.75, h/b = 1.0, α = 0°):
    // between (-0.50 at a/b=1.5) and (-0.60 at a/b=2.0) -> -0.55, Ca = 1.25.
    let interp = nbr6123_wall_coeffs(6.0, 10.5, 6.0, true, true);
    assert!(close(interp.cpe_leeward, -0.55, 1e-12));
    assert!(close(interp.ca_drag, 1.25, 1e-12));

    // 2. End-to-end 2-storey 12×6 m building in Trazo.
    let mut project = Project::default();
    project.wall_height = 3.0;
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(6.0, 6.0), "Sala")
        .unwrap();
    project
        .add_room(Point::new(6.0, 0.0), Point::new(12.0, 6.0), "Cozinha")
        .unwrap();
    project.structure.settings.floors = 2;
    project
        .set_wind(WindSettings {
            v0_ms: 40.0,
            category: 3,
            s1: 1.0,
            s3: 1.0,
        })
        .unwrap();
    project.generate_structure();

    let report = project.structure_report();
    let bw = &report.loads.building_wind;
    assert_eq!(bw.building_class, "A");
    assert!(close(bw.plan_width_x_m, 12.0, 1e-6));
    assert!(close(bw.plan_depth_y_m, 6.0, 1e-6));
    assert!(close(bw.total_height_m, 6.0, 1e-6));
    assert_eq!(bw.directions.len(), 4);

    // Wind along +Y strikes the 12 m long facade (α = 0°, Ca = 1.30).
    let dir_y = bw
        .directions
        .iter()
        .find(|d| d.direction == WindDirection::PosY)
        .unwrap();
    assert_eq!(dir_y.storeys.len(), 2);
    assert!(close(dir_y.coeffs.ca_drag, 1.30, 1e-9));
    let s2_1 = 0.94 * (5.0_f64 / 10.0).powf(0.10);
    let q_1 = 0.613 * (40.0 * s2_1).powi(2) / 1000.0;
    let f_1 = 1.30 * q_1 * (12.0 * 3.0);
    let s2_2 = 0.94 * (6.0_f64 / 10.0).powf(0.10);
    let q_2 = 0.613 * (40.0 * s2_2).powi(2) / 1000.0;
    let f_2 = 1.30 * q_2 * (12.0 * 3.0);
    let expected_vb = f_1 + f_2;
    let expected_mb = f_1 * 1.5 + f_2 * 4.5;

    assert!(
        close(dir_y.base_shear_kn, expected_vb, 1e-6),
        "Vw = {}, expected {}",
        dir_y.base_shear_kn,
        expected_vb
    );
    assert!(
        close(dir_y.overturning_moment_knm, expected_mb, 1e-6),
        "Mw = {}, expected {}",
        dir_y.overturning_moment_knm,
        expected_mb
    );
    // Diaphragm shear at ground-floor beam level = F_2 + 0.5 * F_1.
    assert!(close(dir_y.diaphragm_shear_kn, f_2 + 0.5 * f_1, 1e-6));

    // Wind along +X strikes the 6 m short facade (α = 90°, Ca = 1.00).
    let dir_x = bw
        .directions
        .iter()
        .find(|d| d.direction == WindDirection::PosX)
        .unwrap();
    assert!(close(dir_x.coeffs.ca_drag, 1.00, 1e-9));
    assert!(dir_y.base_shear_kn > dir_x.base_shear_kn);
}

/// Validation #3 — Exact linear superposition on 2D plane frames:
/// Proves that combining solved fundamental load cases (`G1`, `G2`, `Q`, `W+`)
/// with NBR 8681 factors (`ELU-2`: `1.4·G1 + 1.4·G2 + 1.4·Q + 0.84·W+`) matches
/// solving a directly factored `Frame2D` to machine precision (`< 1e-9`), and
/// verifies that frame load cases, combinations, envelopes, traffic lights and
/// PDF memorial report are all populated end-to-end.
#[test]
fn frame_nbr8681_superposition_matches_direct_factored_solve_and_feeds_outputs() {
    // Direct solve of a portal frame under ELU-2 (1.4·G1 + 1.4·G2 + 1.4·Q + 0.84·W+).
    let (l, h, e) = (5.0, 3.0, 25_000_000.0);
    let (a_b, i_b) = (0.14 * 0.40, 0.14 * 0.40_f64.powi(3) / 12.0);
    let (a_c, i_c) = (0.20 * 0.30, 0.20 * 0.30_f64.powi(3) / 12.0);
    let (g1, g2, q, w) = (4.0, 5.5, 3.0, 8.0);

    let build = |q_beam: f64, f_wind: f64| {
        let mut f = Frame2D::default();
        let b0 = f.add_node("P1", 0.0, 0.0, SupportKind::Fixed);
        let b1 = f.add_node("P2", l, 0.0, SupportKind::Fixed);
        let t0 = f.add_node("P1t", 0.0, h, SupportKind::Free);
        let t1 = f.add_node("P2t", l, h, SupportKind::Free);
        f.add_member(
            "P1",
            MemberKind::Column,
            b0,
            t0,
            a_c,
            i_c,
            e,
            EndRelease::Rigid,
        );
        f.add_member(
            "P2",
            MemberKind::Column,
            b1,
            t1,
            a_c,
            i_c,
            e,
            EndRelease::Rigid,
        );
        let beam = f.add_member(
            "V1",
            MemberKind::Beam,
            t0,
            t1,
            a_b,
            i_b,
            e,
            EndRelease::Rigid,
        );
        f.add_uniform_load(beam, -q_beam);
        f.add_nodal_load(t0, f_wind, 0.0, 0.0);
        f
    };

    let s_g1 = solve_frame(&build(g1, 0.0)).unwrap();
    let s_g2 = solve_frame(&build(g2, 0.0)).unwrap();
    let s_q = solve_frame(&build(q, 0.0)).unwrap();
    let s_w = solve_frame(&build(0.0, w)).unwrap();
    let s_direct = solve_frame(&build(
        GAMMA_G_UNFAV * (g1 + g2) + GAMMA_Q * q,
        GAMMA_W * PSI0_WIND * w,
    ))
    .unwrap();

    // Superposition must equal direct factored solve at every node and reaction.
    for i in 0..s_direct.displacements.len() {
        let u_sup = GAMMA_G_UNFAV * s_g1.displacements[i].u_m
            + GAMMA_G_UNFAV * s_g2.displacements[i].u_m
            + GAMMA_Q * s_q.displacements[i].u_m
            + GAMMA_W * PSI0_WIND * s_w.displacements[i].u_m;
        assert!((u_sup - s_direct.displacements[i].u_m).abs() < 1e-12);
    }
    for i in 0..s_direct.reactions.len() {
        let ry_sup = GAMMA_G_UNFAV * s_g1.reactions[i].ry_kn
            + GAMMA_G_UNFAV * s_g2.reactions[i].ry_kn
            + GAMMA_Q * s_q.reactions[i].ry_kn
            + GAMMA_W * PSI0_WIND * s_w.reactions[i].ry_kn;
        assert!((ry_sup - s_direct.reactions[i].ry_kn).abs() < 1e-9);
    }

    // Full project integration check (load cases, combinations, frame envelopes,
    // traffic lights and PDF memorial pages).
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
        .unwrap();
    project
        .add_room(Point::new(4.0, 0.0), Point::new(7.5, 4.0), "Varanda")
        .unwrap();
    project.structure.settings.per_room_live_loads = true;
    project.generate_structure();

    let report = project.structure_report();
    assert!(!report.loads.is_empty());
    assert!(report.loads.nbr6120.total_g1_kn > 0.0);
    assert!(report.loads.nbr6120.total_g2_kn > 0.0);
    assert!(report.loads.nbr6120.total_q_kn > 0.0);
    assert_eq!(report.loads.combinations.len(), 17);
    assert_eq!(report.loads.building_wind.directions.len(), 4);

    // Per-room NBR 6120 live load: Varanda = 2.5 kN/m², Sala = 1.5 kN/m².
    let varanda = report
        .loads
        .nbr6120
        .rooms
        .iter()
        .find(|r| r.room_name == "Varanda")
        .unwrap();
    assert!(close(varanda.active_q_kn_m2, 2.5, 1e-12));

    // Every 2D frame has 5 load cases (G1, G2, Q, W+, W−), 17 NBR 8681 combinations,
    // ELU envelope > ELS characteristic moments, and consistent frequent wind shear.
    assert!(!report.frames.is_empty());
    for frame in &report.frames {
        assert_eq!(frame.load_cases.len(), 5);
        assert_eq!(frame.load_cases[0].kind, LoadCaseKind::G1);
        assert_eq!(frame.combinations.len(), 17);
        assert!(frame.characteristic_wind_kn > 0.0);
        assert!(close(
            frame.lateral_wind_kn,
            PSI1_WIND * frame.characteristic_wind_kn,
            1e-9
        ));
        assert!(frame.envelope.elu_max_beam_pos_knm > 0.0);
        assert!(!frame.envelope.elu_max_column_moment_combo.is_empty());
        assert!(!frame.envelope.els_f_drift_combo.is_empty());
    }

    // Safety traffic light includes building wind and frame verdicts with zero red on standard house.
    let summary = safety_summary(&project);
    assert_eq!(summary.red, 0, "{:#?}", summary.verdicts);
    assert!(
        summary
            .verdicts
            .iter()
            .any(|v| v.element == "Vento na edificação (±X)")
    );
    assert!(
        summary
            .verdicts
            .iter()
            .any(|v| v.element == "Vento na edificação (±Y)")
    );
    assert!(
        summary
            .verdicts
            .iter()
            .any(|v| v.element.starts_with("Pórtico "))
    );

    // PDF memorial pages include the NBR 6120, NBR 6123 and NBR 8681 sections.
    let pages = crate::io::report::report_pages(&project, "2026-11-24");
    let joined = pages.join("\n");
    assert!(joined.contains("Casos de carga (NBR 6120:2019)"));
    assert!(joined.contains("Combinações de ações (NBR 8681:2003"));
    assert!(joined.contains("ELU-1"));
    assert!(joined.contains("ELS-QP"));
    assert!(joined.contains("ELS-F2"));
    // No structural numbers or building-wind verdicts if project exceeds applicability limits.
    project.structure.settings.floors = 5;
    let refused_summary = safety_summary(&project);
    assert_eq!(refused_summary.verdicts[0].element, "Estrutura");
    assert_eq!(refused_summary.verdicts[0].light(), Light::Red);
}

/// Regression test for multi-storey real floors (real `FloorLevel` heights in
/// NBR 6123 storey wind bands) and column tension/uplift detection under ELU-6/7
/// on a narrow lightweight frame exposed to severe wind.
#[test]
fn multi_storey_real_floors_raise_s2_with_height_and_narrow_frame_flags_wind_uplift() {
    // 1. Two real storeys: S2(z) and q(z) must increase on the upper storey.
    let mut house = Project::default();
    house.wall_height = 3.0;
    house.set_terrain(Point::new(-2.0, -2.0), Point::new(20.0, 20.0));
    house
        .add_room(Point::new(0.0, 0.0), Point::new(5.0, 4.0), "Sala")
        .unwrap();
    house.generate_structure();
    let ground = house.ensure_default_floor();
    house.add_floor("Superior", true, true);
    house.generate_structure();
    house.switch_floor(ground).unwrap();

    let rep = house.structure_report();
    let dir_x = rep
        .loads
        .building_wind
        .directions
        .iter()
        .find(|d| d.direction == WindDirection::PosX)
        .unwrap();
    assert_eq!(dir_x.storeys.len(), 2);
    assert!(dir_x.storeys[1].z_top_m > dir_x.storeys[0].z_top_m);
    assert!(dir_x.storeys[1].s2 > dir_x.storeys[0].s2);
    assert!(dir_x.storeys[1].force_kn > dir_x.storeys[0].force_kn);
    assert!(rep.loads.max_elu_vertical_kn() > rep.loads.els_qp_vertical_kn());
    assert!(rep.loads.max_elu_horizontal_kn() > rep.loads.els_frequent_horizontal_kn());

    // 2. Narrow, tall, lightweight portal frame under V0 = 55 m/s open-sea wind (Cat. I, S1 = 1.5, S3 = 1.1):
    // wind overturning in ELU-6/7 pulls the windward column into net tension (uplift).
    let mut slender = Project::default();
    slender.wall_height = 4.0;
    slender.set_terrain(Point::new(-2.0, -2.0), Point::new(15.0, 15.0));
    // Wide perpendicular facade (10 m along Y) with narrow frame span (1.2 m along X).
    slender
        .add_wall(Point::new(0.0, 0.0), Point::new(0.0, 10.0))
        .unwrap();
    slender
        .add_wall(Point::new(1.2, 0.0), Point::new(1.2, 10.0))
        .unwrap();
    slender.structure.settings.masonry_kn_m3 = 0.0;
    slender
        .set_wind(WindSettings {
            v0_ms: 55.0,
            category: 1,
            s1: 1.5,
            s3: 1.1,
        })
        .unwrap();
    let c1 = slender
        .add_fixture(FixtureKind::Column, Point::new(0.0, 5.0), 0.25, 0.25, 0)
        .unwrap();
    let c2 = slender
        .add_fixture(FixtureKind::Column, Point::new(1.2, 5.0), 0.25, 0.25, 0)
        .unwrap();
    slender.add_beam(c1, c2, 0.14, 0.30).unwrap();

    let srep = slender.structure_report();
    let frame = &srep.frames[0];
    assert!(
        frame.envelope.has_column_uplift,
        "expected column tension/uplift under severe wind on narrow frame: {:?}",
        frame.envelope
    );
    assert!(
        srep.warnings
            .iter()
            .any(|w| w.contains("tração/arrancamento")),
        "{:?}",
        srep.warnings
    );
    let summary = safety_summary(&slender);
    let fverdict = summary
        .verdicts
        .iter()
        .find(|v| v.element == frame.label)
        .unwrap();
    assert_eq!(fverdict.light(), Light::Red);
    assert!(
        fverdict
            .checks
            .iter()
            .any(|c| c.name == "Arrancamento (ELU)" && c.light == Light::Red)
    );
}
