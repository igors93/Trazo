use super::{
    diagrams::frame_diagrams_svg,
    model::{EndRelease, Frame2D, MemberKind, SupportKind},
    solver::solve_frame,
};
use crate::domain::{fixtures::FixtureKind, geometry::Point, model::Project};

/// Validation #1 (ROADMAP #94): Cantilever column under lateral tip load `P`.
/// Exact Euler–Bernoulli solution:
/// - Base moment: `M_base = P · h`
/// - Tip horizontal deflection: `Δ = P · h³ / (3 · E · I)`
/// - Tip rotation: `θ = -P · h² / (2 · E · I)` (CW for `+X` load)
#[test]
fn cantilever_column_matches_exact_moment_and_tip_deflection() {
    let h: f64 = 3.2;
    let p: f64 = 10.0; // kN to the right (+X)
    let e: f64 = 25_000_000.0; // kN/m² (25 GPa)
    let b: f64 = 0.20;
    let d: f64 = 0.30;
    let a = b * d;
    let i = b * d.powi(3) / 12.0;

    let mut frame = Frame2D::default();
    let base = frame.add_node("A", 0.0, 0.0, SupportKind::Fixed);
    let tip = frame.add_node("B", 0.0, h, SupportKind::Free);
    frame.add_member(
        "P1",
        MemberKind::Column,
        base,
        tip,
        a,
        i,
        e,
        EndRelease::Rigid,
    );
    frame.add_nodal_load(tip, p, 0.0, 0.0);

    let sol = solve_frame(&frame).unwrap();

    let expected_delta = p * h.powi(3) / (3.0 * e * i);
    let expected_theta = -p * h.powi(2) / (2.0 * e * i);
    let expected_moment = p * h;

    assert!(
        (sol.displacements[tip].u_m - expected_delta).abs() < 1e-12,
        "tip deflection {} != exact {}",
        sol.displacements[tip].u_m,
        expected_delta
    );
    assert!(
        (sol.displacements[tip].theta_rad - expected_theta).abs() < 1e-12,
        "tip rotation {} != exact {}",
        sol.displacements[tip].theta_rad,
        expected_theta
    );
    assert!(
        (sol.reactions[0].rx_kn + p).abs() < 1e-9,
        "base shear reaction {} != -{}",
        sol.reactions[0].rx_kn,
        p
    );
    assert!(
        (sol.reactions[0].mz_knm - expected_moment).abs() < 1e-9,
        "base moment reaction {} != {}",
        sol.reactions[0].mz_knm,
        expected_moment
    );
    assert!((sol.members[0].max_abs_moment_knm - expected_moment).abs() < 1e-9);
    assert!(sol.equilibrium.sum_fx_kn.abs() < 1e-9);
    assert!(sol.equilibrium.sum_fy_kn.abs() < 1e-9);
    assert!(sol.equilibrium.sum_mz_knm.abs() < 1e-9);
}

/// Validation #2 (ROADMAP #94): When column flexural stiffness goes to zero
/// (`I_col → 0`), the 2D frame reduces identically to the continuous-beam
/// solution on pinned supports:
/// Two equal spans `L = 4.0 m`, uniform load `w = 15 kN/m`:
/// - Interior support hogging moment `M_B = -w·L² / 8 = -30.0 kN·m`
/// - Exterior reactions `R_A = R_C = 3/8 · w·L = 22.5 kN`
/// - Interior reaction `R_B = 10/8 · w·L = 75.0 kN`
/// - Max positive span moment `M_max = 9/128 · w·L² = 16.875 kN·m`
#[test]
fn frame_reduces_to_continuous_beam_when_column_stiffness_vanishes() {
    let l = 4.0;
    let h = 3.0;
    let w = 15.0;
    let e = 25_000_000.0;
    let i_beam = 0.20 * 0.40_f64.powi(3) / 12.0;
    let a_beam = 0.20 * 0.40;
    // Columns keep high axial stiffness (rigid vertical supports) while I_col → 0.
    let a_col = 10_000.0;
    let i_col = 1e-12;

    let mut frame = Frame2D::default();
    let b0 = frame.add_node("A0", 0.0, 0.0, SupportKind::Pinned);
    let b1 = frame.add_node("B0", l, 0.0, SupportKind::Pinned);
    let b2 = frame.add_node("C0", 2.0 * l, 0.0, SupportKind::Pinned);
    // Lateral brace at t0 keeps horizontal translation restrained when I_col → 0.
    let t0 = frame.add_node("A", 0.0, h, SupportKind::RollerX);
    let t1 = frame.add_node("B", l, h, SupportKind::Free);
    let t2 = frame.add_node("C", 2.0 * l, h, SupportKind::Free);

    let c0 = frame.add_member(
        "P1",
        MemberKind::Column,
        b0,
        t0,
        a_col,
        i_col,
        e,
        EndRelease::Rigid,
    );
    let c1 = frame.add_member(
        "P2",
        MemberKind::Column,
        b1,
        t1,
        a_col,
        i_col,
        e,
        EndRelease::Rigid,
    );
    let c2 = frame.add_member(
        "P3",
        MemberKind::Column,
        b2,
        t2,
        a_col,
        i_col,
        e,
        EndRelease::Rigid,
    );
    let v1 = frame.add_member(
        "V1.1",
        MemberKind::Beam,
        t0,
        t1,
        a_beam,
        i_beam,
        e,
        EndRelease::Rigid,
    );
    let v2 = frame.add_member(
        "V1.2",
        MemberKind::Beam,
        t1,
        t2,
        a_beam,
        i_beam,
        e,
        EndRelease::Rigid,
    );
    frame.add_uniform_load(v1, -w);
    frame.add_uniform_load(v2, -w);

    let sol = solve_frame(&frame).unwrap();

    let expected_m_b = -w * l * l / 8.0; // -30.0 kN·m
    let expected_m_span = 9.0 / 128.0 * w * l * l; // 16.875 kN·m
    let expected_r_ext = 3.0 / 8.0 * w * l; // 22.5 kN
    let expected_r_int = 10.0 / 8.0 * w * l; // 75.0 kN

    assert!((sol.members[v1].moment_start_knm).abs() < 1e-5);
    assert!(
        (sol.members[v1].moment_end_knm - expected_m_b).abs() < 1e-4,
        "M_B = {}, expected {}",
        sol.members[v1].moment_end_knm,
        expected_m_b
    );
    assert!((sol.members[v2].moment_start_knm - expected_m_b).abs() < 1e-4);
    assert!((sol.members[v2].moment_end_knm).abs() < 1e-5);
    assert!(
        (sol.members[v1].max_sagging_knm - expected_m_span).abs() < 0.05,
        "M_span = {}, expected {}",
        sol.members[v1].max_sagging_knm,
        expected_m_span
    );

    // Column reactions match continuous-beam reactions, and column moments vanish.
    assert!((sol.members[c0].axial_kn + expected_r_ext).abs() < 1e-4);
    assert!((sol.members[c1].axial_kn + expected_r_int).abs() < 1e-4);
    assert!((sol.members[c2].axial_kn + expected_r_ext).abs() < 1e-4);
    assert!(sol.members[c0].max_abs_moment_knm < 1e-5);
    assert!(sol.members[c1].max_abs_moment_knm < 1e-5);
    assert!(sol.members[c2].max_abs_moment_knm < 1e-5);

    assert!(sol.equilibrium.sum_fx_kn.abs() < 1e-7);
    assert!(sol.equilibrium.sum_fy_kn.abs() < 1e-7);
    assert!(sol.equilibrium.sum_mz_knm.abs() < 1e-7);
}

/// Validation #3a (ROADMAP #94): Symmetric fixed-base portal frame under
/// uniform gravity load `w` on the beam, compared against exact closed-form
/// slope-deflection equations (inextensible members, `k = (I_b/L) / (I_c/h)`):
/// - `M_top = (w·L² / 12) · 2 / (2 + k)`
/// - `M_base = M_top / 2`
/// - `M_mid = w·L² / 8 - M_top`
/// - `H_base = (M_top + M_base) / h`
#[test]
fn symmetric_fixed_base_portal_gravity_matches_slope_deflection() {
    let l = 6.0;
    let h = 3.0;
    let w = 20.0;
    let e = 25_000_000.0;
    let i_b = 0.20 * 0.45_f64.powi(3) / 12.0;
    let i_c = 0.20 * 0.30_f64.powi(3) / 12.0;
    let k = (i_b / l) / (i_c / h);
    // High axial stiffness matches classical inextensible slope-deflection assumption.
    let a_rigid = 10_000.0;

    let mut frame = Frame2D::default();
    let a = frame.add_node("A", 0.0, 0.0, SupportKind::Fixed);
    let b = frame.add_node("B", 0.0, h, SupportKind::Free);
    let c = frame.add_node("C", l, h, SupportKind::Free);
    let d = frame.add_node("D", l, 0.0, SupportKind::Fixed);

    let col_l = frame.add_member(
        "P1",
        MemberKind::Column,
        a,
        b,
        a_rigid,
        i_c,
        e,
        EndRelease::Rigid,
    );
    let beam = frame.add_member(
        "V1",
        MemberKind::Beam,
        b,
        c,
        a_rigid,
        i_b,
        e,
        EndRelease::Rigid,
    );
    let _col_r = frame.add_member(
        "P2",
        MemberKind::Column,
        d,
        c,
        a_rigid,
        i_c,
        e,
        EndRelease::Rigid,
    );
    frame.add_uniform_load(beam, -w);

    let sol = solve_frame(&frame).unwrap();

    let m_top_exact = (w * l * l / 12.0) * 2.0 / (2.0 + k);
    let m_base_exact = 0.5 * m_top_exact;
    let m_mid_exact = w * l * l / 8.0 - m_top_exact;
    let h_base_exact = (m_top_exact + m_base_exact) / h;

    assert!(
        (sol.members[beam].moment_start_knm + m_top_exact).abs() < 1e-4,
        "beam left M = {}, expected -{}",
        sol.members[beam].moment_start_knm,
        m_top_exact
    );
    assert!(
        (sol.members[beam].max_sagging_knm - m_mid_exact).abs() < 1e-4,
        "beam mid M = {}, expected {}",
        sol.members[beam].max_sagging_knm,
        m_mid_exact
    );
    assert!(
        (sol.members[col_l].moment_start_knm.abs() - m_base_exact).abs() < 1e-4,
        "col base M = {}, expected {}",
        sol.members[col_l].moment_start_knm,
        m_base_exact
    );
    assert!(
        (sol.members[col_l].moment_end_knm.abs() - m_top_exact).abs() < 1e-4,
        "col top M = {}, expected {}",
        sol.members[col_l].moment_end_knm,
        m_top_exact
    );
    let r_a = sol.reactions.iter().find(|r| r.node == a).unwrap();
    assert!(
        (r_a.rx_kn - h_base_exact).abs() < 1e-4,
        "base horizontal thrust {} != {}",
        r_a.rx_kn,
        h_base_exact
    );
    assert!((r_a.ry_kn - 0.5 * w * l).abs() < 1e-6);
    assert!(sol.equilibrium.sum_fx_kn.abs() < 1e-7);
    assert!(sol.equilibrium.sum_fy_kn.abs() < 1e-7);
    assert!(sol.equilibrium.sum_mz_knm.abs() < 1e-7);
}

/// Validation #3b (ROADMAP #94): Symmetric fixed-base portal frame under
/// horizontal sway force `H` at roof level, compared against exact closed-form
/// slope-deflection equations (`k = (I_b/L) / (I_c/h)`):
/// - `|M_base| = (H·h / 2) · (1 + 3k) / (1 + 6k)`
/// - `|M_top|  = (H·h / 2) · (3k) / (1 + 6k)`
/// - `Δ_sway   = (H·h³ / (12·E·I_c)) · (2 + 3k) / (1 + 6k)`
#[test]
fn symmetric_fixed_base_portal_lateral_sway_matches_slope_deflection() {
    let l = 6.0;
    let h = 3.0;
    let lat = 12.0;
    let e = 25_000_000.0;
    let i_b = 0.20 * 0.45_f64.powi(3) / 12.0;
    let i_c = 0.20 * 0.30_f64.powi(3) / 12.0;
    let k = (i_b / l) / (i_c / h);
    let a_rigid = 10_000.0;

    let mut frame = Frame2D::default();
    let a = frame.add_node("A", 0.0, 0.0, SupportKind::Fixed);
    let b = frame.add_node("B", 0.0, h, SupportKind::Free);
    let c = frame.add_node("C", l, h, SupportKind::Free);
    let d = frame.add_node("D", l, 0.0, SupportKind::Fixed);

    let col_l = frame.add_member(
        "P1",
        MemberKind::Column,
        a,
        b,
        a_rigid,
        i_c,
        e,
        EndRelease::Rigid,
    );
    let _beam = frame.add_member(
        "V1",
        MemberKind::Beam,
        b,
        c,
        a_rigid,
        i_b,
        e,
        EndRelease::Rigid,
    );
    let _col_r = frame.add_member(
        "P2",
        MemberKind::Column,
        d,
        c,
        a_rigid,
        i_c,
        e,
        EndRelease::Rigid,
    );
    frame.add_nodal_load(b, lat, 0.0, 0.0);

    let sol = solve_frame(&frame).unwrap();

    let m_base_exact = (lat * h / 2.0) * (1.0 + 3.0 * k) / (1.0 + 6.0 * k);
    let m_top_exact = (lat * h / 2.0) * (3.0 * k) / (1.0 + 6.0 * k);
    let delta_exact = (lat * h.powi(3) / (12.0 * e * i_c)) * (2.0 + 3.0 * k) / (1.0 + 6.0 * k);

    assert!(
        (sol.displacements[b].u_m - delta_exact).abs() < 1e-7,
        "sway Δ = {}, expected {}",
        sol.displacements[b].u_m,
        delta_exact
    );
    assert!(
        (sol.members[col_l].moment_start_knm.abs() - m_base_exact).abs() < 1e-4,
        "sway M_base = {}, expected {}",
        sol.members[col_l].moment_start_knm,
        m_base_exact
    );
    assert!(
        (sol.members[col_l].moment_end_knm.abs() - m_top_exact).abs() < 1e-4,
        "sway M_top = {}, expected {}",
        sol.members[col_l].moment_end_knm,
        m_top_exact
    );
    assert!(sol.equilibrium.sum_fx_kn.abs() < 1e-7);
    assert!(sol.equilibrium.sum_fy_kn.abs() < 1e-7);
    assert!(sol.equilibrium.sum_mz_knm.abs() < 1e-7);
}

#[test]
fn pinned_end_releases_zero_end_moments_and_match_simply_supported_deflection() {
    let l = 5.0;
    let w = 12.0;
    let e = 25_000_000.0;
    let a = 0.14 * 0.40;
    let i = 0.14 * 0.40_f64.powi(3) / 12.0;

    let mut frame = Frame2D::default();
    let n0 = frame.add_node("A", 0.0, 0.0, SupportKind::Fixed);
    let n1 = frame.add_node("B", l, 0.0, SupportKind::Fixed);
    let beam = frame.add_member(
        "V1",
        MemberKind::Beam,
        n0,
        n1,
        a,
        i,
        e,
        EndRelease::PinnedBoth,
    );
    frame.add_uniform_load(beam, -w);

    let sol = solve_frame(&frame).unwrap();
    let res = &sol.members[beam];

    let expected_m = w * l * l / 8.0;
    let expected_delta_mm = 5.0 * w * l.powi(4) / (384.0 * e * i) * 1000.0;

    assert!(res.moment_start_knm.abs() < 1e-9);
    assert!(res.moment_end_knm.abs() < 1e-9);
    assert!((res.max_sagging_knm - expected_m).abs() < 1e-7);
    assert!(
        (res.max_local_deflection_mm - expected_delta_mm).abs() < 1e-6,
        "deflection {} mm != {} mm",
        res.max_local_deflection_mm,
        expected_delta_mm
    );
    assert!(sol.equilibrium.sum_fy_kn.abs() < 1e-9);
}

#[test]
fn simply_supported_midspan_point_load_matches_exact_solution() {
    let l: f64 = 5.0;
    let p: f64 = 18.0;
    let e: f64 = 25_000_000.0;
    let area: f64 = 0.14 * 0.40;
    let inertia: f64 = 0.14 * 0.40_f64.powi(3) / 12.0;
    let mut frame = Frame2D::default();
    let left = frame.add_node("A", 0.0, 0.0, SupportKind::Fixed);
    let right = frame.add_node("B", l, 0.0, SupportKind::Fixed);
    let beam = frame.add_member(
        "V1",
        MemberKind::Beam,
        left,
        right,
        area,
        inertia,
        e,
        EndRelease::PinnedBoth,
    );
    frame.add_point_load(beam, l / 2.0, -p);

    let solution = solve_frame(&frame).unwrap();
    let result = &solution.members[beam];
    let expected_moment = p * l / 4.0;
    let expected_deflection_mm = p * l.powi(3) / (48.0 * e * inertia) * 1000.0;
    assert!((result.max_sagging_knm - expected_moment).abs() < 1e-8);
    assert!((result.max_local_deflection_mm - expected_deflection_mm).abs() < 1e-6);
    assert!((solution.reactions[0].ry_kn - p / 2.0).abs() < 1e-8);
    assert!((solution.reactions[1].ry_kn - p / 2.0).abs() < 1e-8);
    assert!(solution.equilibrium.sum_fy_kn.abs() < 1e-8);
}

#[test]
fn project_frame_keeps_a_water_tank_load_on_its_beam() {
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(8.0, 5.0));
    let c1 = project
        .add_fixture(FixtureKind::Column, Point::new(0.0, 0.0), 0.20, 0.30, 0)
        .unwrap();
    let c2 = project
        .add_fixture(FixtureKind::Column, Point::new(5.0, 0.0), 0.20, 0.30, 0)
        .unwrap();
    let beam_id = project.add_beam(c1, c2, 0.14, 0.40).unwrap();

    let before = project.structure_report();
    let before_frame = before.frames.first().unwrap();
    let before_reaction: f64 = before_frame.reactions.iter().map(|r| r.ry_kn).sum();
    let before_moment = before_frame.max_beam_moment_knm();
    let before_deflection = before_frame.beams[0].elastic_deflection_mm;

    let tank = project
        .add_fixture(FixtureKind::WaterTank, Point::new(2.5, 0.0), 1.20, 1.20, 0)
        .unwrap();
    let tank_load = project
        .fixtures
        .iter()
        .find(|fixture| fixture.id == tank)
        .and_then(|fixture| fixture.structural_load_kn)
        .unwrap();
    let after = project.structure_report();
    let after_frame = after.frames.first().unwrap();
    let frame_beam = after_frame
        .beams
        .iter()
        .find(|beam| beam.beam_id == beam_id)
        .unwrap();
    let after_reaction: f64 = after_frame.reactions.iter().map(|r| r.ry_kn).sum();

    assert_eq!(frame_beam.point_loads.len(), 1);
    assert!((frame_beam.point_loads[0].position_m - 2.5).abs() < 1e-9);
    assert!((frame_beam.point_loads[0].load_kn - tank_load).abs() < 1e-9);
    assert!((after_frame.total_point_load_kn() - tank_load).abs() < 1e-9);
    assert!((after_reaction - before_reaction - tank_load).abs() < 1e-7);
    assert!(after_frame.max_beam_moment_knm() > before_moment);
    assert!(frame_beam.elastic_deflection_mm > before_deflection);
    assert!(after_frame.equilibrium_residual_kn < 1e-7);
}

#[test]
fn solver_and_project_report_expose_an_unstable_frame() {
    let mut unstable = Frame2D::default();
    let a = unstable.add_node("A", 0.0, 0.0, SupportKind::Free);
    let b = unstable.add_node("B", 4.0, 0.0, SupportKind::Free);
    unstable.add_member(
        "V-instável",
        MemberKind::Beam,
        a,
        b,
        0.14 * 0.40,
        0.14 * 0.40_f64.powi(3) / 12.0,
        25_000_000.0,
        EndRelease::Rigid,
    );
    let reason = solve_frame(&unstable).unwrap_err();
    assert!(reason.contains("hipostático") || reason.contains("singular"));

    // Force a non-finite stiffness through the low-level project model. Public
    // setters reject it, but the report must still expose any solver error
    // instead of turning a failed eligible line into an empty result.
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(8.0, 5.0));
    let c1 = project
        .add_fixture(FixtureKind::Column, Point::new(0.0, 0.0), 0.20, 0.30, 0)
        .unwrap();
    let c2 = project
        .add_fixture(FixtureKind::Column, Point::new(5.0, 0.0), 0.20, 0.30, 0)
        .unwrap();
    project.add_beam(c1, c2, 0.14, 0.40).unwrap();
    project.structure.settings.fck_mpa = f64::INFINITY;

    let report = project.structure_report();
    assert!(report.frames.is_empty());
    assert_eq!(
        report.frame_failures.len(),
        1,
        "{:#?}",
        report.frame_failures
    );
    let failure = &report.frame_failures[0];
    assert!(failure.reason.contains("hipostático") || failure.reason.contains("singular"));
    assert!(report.warnings.iter().any(|warning| {
        warning.contains(&failure.label) && warning.contains("análise 2D não concluída")
    }));
    let verdict = crate::domain::verdict::structure_verdicts(&project, &report)
        .into_iter()
        .find(|verdict| verdict.element == failure.label)
        .unwrap();
    assert_eq!(verdict.light(), crate::domain::verdict::Light::Red);

    let svg = super::frame_diagrams_svg(&project);
    assert!(svg.contains("Nenhum pórtico 2D foi resolvido"));
    assert!(svg.contains(&failure.label));
    assert!(svg.contains(&failure.reason));
}

/// Validation #4 (ROADMAP #94): Global equilibrium, rigid vs pinned joints,
/// and A3 vector SVG generation on a real generated house project.
#[test]
fn project_2d_frames_satisfy_global_equilibrium_and_render_valid_svg() {
    let mut project = Project::default();
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.5), "Sala")
        .unwrap();
    project
        .add_room(Point::new(4.0, 0.0), Point::new(7.5, 3.5), "Quarto")
        .unwrap();
    project.generate_structure();

    // Rigid joints (default): columns carry frame moments and equilibrium holds.
    let rigid_report = project.structure_report();
    assert!(!rigid_report.frames.is_empty());
    let mut any_col_moment = false;
    for f in &rigid_report.frames {
        assert!(
            f.equilibrium_residual_kn < 1e-6,
            "equilibrium force residual too large: {}",
            f.equilibrium_residual_kn
        );
        assert!(
            f.equilibrium_residual_knm < 1e-5,
            "equilibrium moment residual too large: {}",
            f.equilibrium_residual_knm
        );
        assert!(!f.columns.is_empty());
        assert!(!f.beams.is_empty());
        if f.max_column_moment_knm() > 0.1 {
            any_col_moment = true;
        }
    }
    assert!(
        any_col_moment,
        "rigid-joint portal columns must carry moment"
    );

    // Pinned joints: column top moments drop to zero.
    project.structure.settings.frame_rigid_joints = false;
    let pinned_report = project.structure_report();
    for f in &pinned_report.frames {
        assert!(!f.rigid_joints);
        for c in &f.columns {
            assert!(
                c.moment_top_knm.abs() < 1e-6,
                "pinned joint column top moment should vanish, got {}",
                c.moment_top_knm
            );
        }
        assert!(f.equilibrium_residual_kn < 1e-6);
    }

    // SVG rendering is valid and contains deformed + moment titles.
    project.structure.settings.frame_rigid_joints = true;
    let svg = frame_diagrams_svg(&project);
    assert!(svg.starts_with("<svg"));
    assert!(svg.trim_end().ends_with("</svg>"));
    assert!(svg.contains("Deformada"));
    assert!(svg.contains("Diagrama de Momento Fletor"));
}
