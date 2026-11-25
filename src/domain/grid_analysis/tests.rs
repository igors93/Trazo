use super::*;
use crate::domain::geometry::Point;
use crate::domain::model::Project;

#[test]
fn grid_solver_rejects_invalid_indices_and_oversized_stiffness_before_allocating() {
    use super::solver::{GridMember, GridNode, solve_grid};
    let node = GridNode {
        id: 0,
        point: Point::new(0.0, 0.0),
        fix_w: false,
        fix_rx: false,
        fix_ry: false,
        point_load_kn: 0.0,
    };
    let mut nodes = vec![node; 10_000];
    nodes[9999].point = Point::new(4.0, 0.0);
    let mut member = GridMember {
        id: 0,
        start: 0,
        end: usize::MAX,
        kind: GridMemberKind::Beam { beam_id: 1 },
        ei_knm2: 1.0,
        gj_knm2: 1.0,
        strip_width_m: 1.0,
        q_kn_m: 1.0,
    };
    assert!(solve_grid(&nodes[..2], &[member]).is_err());
    member.end = 9999;
    assert!(
        solve_grid(&nodes, &[member])
            .unwrap_err()
            .contains("memória")
    );
}

#[test]
fn oversized_project_grid_stops_before_creating_all_mesh_cells() {
    use crate::domain::fixtures::FixtureKind;
    let mut project = Project::default();
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 3.0), "Sala")
        .unwrap();
    project.generate_structure();
    for i in 1..=150 {
        let coord = i as f64;
        project
            .add_fixture(FixtureKind::Column, Point::new(coord, coord), 0.2, 0.2, 0)
            .unwrap();
    }
    let report = project.grid_analysis();
    assert!(
        report
            .error
            .as_ref()
            .is_some_and(|error| error.contains("limite de nós"))
    );
    assert!(report.slabs.is_empty());
}

#[test]
fn simply_supported_square_plate_matches_timoshenko_theory() {
    // Timoshenko & Woinowsky-Krieger, "Theory of Plates and Shells", Table 8:
    // Simply supported square plate (a = b = 4.0 m), q = 10 kN/m², h = 0.12 m,
    // E = 25 GPa, ν = 0.20:
    //   D = E*h^3 / (12*(1 - ν^2))
    //   w_max = 0.00406 * q * a^4 / D
    //   m_x = m_y ≈ 0.0442 * q * a^2 (at ν = 0.20; 0.0479 is for ν = 0.30)
    let a: f64 = 4.0;
    let h: f64 = 0.12;
    let e_kpa: f64 = 25_000_000.0;
    let nu: f64 = 0.20;
    let q: f64 = 10.0;
    let d_plate = e_kpa * h.powi(3) / (12.0 * (1.0 - nu * nu));
    let expected_w_mm = 0.00406 * q * a.powi(4) / d_plate * 1000.0;
    // Timoshenko Table 8 (ν = 0.20): m_max = (β1 + ν*β2)*q*a^2 -> ≈ 0.0442 * q * a^2
    let expected_m = 0.0442 * q * a * a;

    let out = solve_rectangular_plate_grid((a, a), h, e_kpa, nu, q, (12, 12)).unwrap();
    let max_w_mm = out.node_w_m.iter().copied().fold(0.0_f64, f64::max) * 1000.0;

    // Exact global vertical equilibrium: ΣR_z = q * a^2.
    assert!(
        out.equilibrium_error_kn < 1e-6,
        "equilibrium error = {}",
        out.equilibrium_error_kn
    );
    assert!((out.total_applied_kn - q * a * a).abs() < 1e-6);

    // Center deflection within 5% of Timoshenko closed-form solution.
    let w_err = ((max_w_mm - expected_w_mm) / expected_w_mm).abs();
    assert!(
        w_err < 0.05,
        "square plate deflection mismatch: got {max_w_mm:.4} mm, Timoshenko {expected_w_mm:.4} mm (err {:.1}%)",
        w_err * 100.0
    );

    // Center bending moment within 8% (orthogonal grillage without Poisson cross-term).
    let max_mx = out
        .member_results
        .iter()
        .filter(|m| matches!(m.kind, GridMemberKind::SlabX { .. }))
        .map(|m| m.m_mid_knm / m.strip_width_m)
        .fold(0.0_f64, f64::max);
    let max_my = out
        .member_results
        .iter()
        .filter(|m| matches!(m.kind, GridMemberKind::SlabY { .. }))
        .map(|m| m.m_mid_knm / m.strip_width_m)
        .fold(0.0_f64, f64::max);
    assert!(
        (max_mx - max_my).abs() < 1e-6,
        "square plate must be symmetric: mx={max_mx}, my={max_my}"
    );
    // Kirchhoff plate moment recovery with Poisson coupling: m_x = m_bar_x + ν * m_bar_y.
    let plate_mx = max_mx + nu * max_my;
    let m_err = ((plate_mx - expected_m) / expected_m).abs();
    assert!(
        m_err < 0.03,
        "square plate moment mismatch: got {plate_mx:.3}, Timoshenko {expected_m:.3} (err {:.1}%)",
        m_err * 100.0
    );
}

#[test]
fn simply_supported_rectangular_2_to_1_plate_matches_timoshenko_theory() {
    // Timoshenko Table 8 for b/a = 2.0 (a = 3.0 m, b = 6.0 m):
    //   w_max = 0.01013 * q * a^4 / D
    //   Short-span moment m_x dominates long-span moment m_y (m_x > 2 * m_y).
    let a: f64 = 3.0;
    let b: f64 = 6.0;
    let h: f64 = 0.12;
    let e_kpa: f64 = 25_000_000.0;
    let nu: f64 = 0.20;
    let q: f64 = 10.0;
    let d_plate = e_kpa * h.powi(3) / (12.0 * (1.0 - nu * nu));
    let expected_w_mm = 0.01013 * q * a.powi(4) / d_plate * 1000.0;

    let out = solve_rectangular_plate_grid((a, b), h, e_kpa, nu, q, (10, 16)).unwrap();
    let max_w_mm = out.node_w_m.iter().copied().fold(0.0_f64, f64::max) * 1000.0;

    assert!(out.equilibrium_error_kn < 1e-6);
    let w_err = ((max_w_mm - expected_w_mm) / expected_w_mm).abs();
    assert!(
        w_err < 0.05,
        "2:1 plate deflection mismatch: got {max_w_mm:.4} mm, Timoshenko {expected_w_mm:.4} mm (err {:.1}%)",
        w_err * 100.0
    );

    let max_mx = out
        .member_results
        .iter()
        .filter(|m| matches!(m.kind, GridMemberKind::SlabX { .. }))
        .map(|m| m.m_mid_knm / m.strip_width_m)
        .fold(0.0_f64, f64::max);
    let max_my = out
        .member_results
        .iter()
        .filter(|m| matches!(m.kind, GridMemberKind::SlabY { .. }))
        .map(|m| m.m_mid_knm / m.strip_width_m)
        .fold(0.0_f64, f64::max);
    assert!(
        max_mx > max_my * 2.0,
        "short-span moment mx ({max_mx:.2}) must dominate long-span my ({max_my:.2})"
    );
}

#[test]
fn l_shaped_slab_coupled_with_beams_satisfies_equilibrium_and_renders_valid_svg() {
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(16.0, 16.0));
    // Create an L-shaped room polygon (6x6 m with top-right 3x3 m cut out -> area = 27 m²).
    let l_poly = vec![
        Point::new(0.0, 0.0),
        Point::new(6.0, 0.0),
        Point::new(6.0, 3.0),
        Point::new(3.0, 3.0),
        Point::new(3.0, 6.0),
        Point::new(0.0, 6.0),
    ];
    project.add_polygon_room(&l_poly, "Sala em L").unwrap();
    project.generate_structure();

    let report = project.grid_analysis();
    assert!(report.error.is_none(), "grid error: {:?}", report.error);
    assert_eq!(report.slabs.len(), 1);
    assert!(
        report.slabs[0].is_irregular,
        "L-shaped slab must be flagged irregular"
    );
    assert!(!report.beams.is_empty());
    assert!(
        report.equilibrium_error_kn < 1e-3,
        "global vertical equilibrium violated: error = {} kN",
        report.equilibrium_error_kn
    );
    assert!(report.slabs[0].w_elastic_mm > 0.0);
    assert!(report.slabs[0].mx_pos_knm_m > 0.0);
    assert!(report.slabs[0].my_pos_knm_m > 0.0);

    // Ensure no heatmap cell falls inside the cut-out quadrant (x > 3.0 && y > 3.0).
    for cell in &report.heatmap_cells {
        let cx = 0.5 * (cell.min.x + cell.max.x);
        let cy = 0.5 * (cell.min.y + cell.max.y);
        assert!(
            !(cx > 3.05 && cy > 3.05),
            "L-shaped cut-out quadrant must not contain slab cells (found at {cx}, {cy})"
        );
    }

    // Printable A3 SVG sheet renders cleanly with heatmap and summary tables.
    let svg = grid_analysis_svg(&project, &report);
    assert!(svg.starts_with("<svg"));
    assert!(svg.trim_end().ends_with("</svg>"));
    assert!(svg.contains("Analogia de Grelha"));
    assert!(svg.contains("Sala em L"));
}
