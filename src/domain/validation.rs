//! Validation suite for the engineering methods (#100).
//!
//! Each test reproduces a published result: the beam coefficients of the
//! AISC *Steel Construction Manual*, Table 3-23 "Shears, Moments and
//! Deflections" (identical in Hibbeler's *Structural Analysis* and in any
//! beam-formula table), the 45° yield-line tributary areas, the NBR 6118
//! formulas for cracked inertia and punching, and the middle-third soil
//! pressure of an eccentric footing. Each result is compared with the closed
//! form or with coefficients to the precision they are published with. The
//! method behind each one is in `docs/engineering.md`.

use crate::domain::geometry::Point;
use crate::domain::model::Project;
use crate::domain::structure::{
    check_footing, continuous_beam, continuous_beam_with_point_loads, effective_beam_stiffness,
    span_deflection, span_deflection_with_point_loads, yield_line_shares,
};

/// `value` within `tolerance` (relative) of `expected`.
fn close(value: f64, expected: f64, tolerance: f64) -> bool {
    (value - expected).abs() <= tolerance * expected.abs().max(1e-12)
}

/// Shear at the left end of a span with uniform load `w`, from its end
/// moments (sagging positive).
fn left_shear(length: f64, w: f64, left: f64, right: f64) -> f64 {
    w * length / 2.0 + (right - left) / length
}

/// Largest sagging moment of a span with uniform load and end moments.
fn span_moment(length: f64, w: f64, left: f64, right: f64) -> f64 {
    let v = left_shear(length, w, left, right);
    let x = (v / w).clamp(0.0, length);
    left + v * x - w * x * x / 2.0
}

#[test]
fn aisc_case_1_simple_beam_uniform_load() {
    // M = wL²/8 at mid-span, Δ = 5wL⁴/384EI.
    let (w, l, ei) = (12.0, 5.0, 25_000.0);
    let ends = continuous_beam(&[(l, w, ei)]);
    assert!(ends[0].0.abs() < 1e-9 && ends[0].1.abs() < 1e-9);
    assert!(close(span_moment(l, w, 0.0, 0.0), w * l * l / 8.0, 1e-9));
    let deflection = span_deflection(l, w, 0.0, 0.0, ei);
    assert!(
        close(deflection, 5.0 * w * l.powi(4) / (384.0 * ei), 1e-3),
        "{deflection}"
    );
}

#[test]
fn aisc_case_7_simple_beam_central_point_load() {
    // M = PL/4 at the load, Δ = PL³/48EI.
    let (p, l, ei) = (30.0, 4.0, 18_000.0);
    let deflection = span_deflection_with_point_loads(l, 0.0, &[(l / 2.0, p)], 0.0, 0.0, ei);
    assert!(
        close(deflection, p * l.powi(3) / (48.0 * ei), 1e-3),
        "{deflection}"
    );
}

#[test]
fn aisc_case_29_two_equal_spans_uniform_load() {
    // M_B = −wL²/8; R_A = 3wL/8, R_B = 10wL/8; M+ = 9wL²/128;
    // Δmax = wL⁴/185EI at 0.4215L.
    let (w, l, ei) = (10.0, 4.0, 20_000.0);
    let ends = continuous_beam(&[(l, w, ei), (l, w, ei)]);
    let m_b = ends[0].1;
    assert!(close(m_b, -w * l * l / 8.0, 1e-9));
    assert!(close(ends[1].0, m_b, 1e-9));
    let r_a = left_shear(l, w, 0.0, m_b);
    let r_b = (w * l - r_a) * 2.0;
    assert!(close(r_a, 3.0 * w * l / 8.0, 1e-9));
    assert!(close(r_b, 10.0 * w * l / 8.0, 1e-9));
    assert!(close(
        span_moment(l, w, 0.0, m_b),
        9.0 * w * l * l / 128.0,
        1e-9
    ));
    let deflection = span_deflection(l, w, 0.0, m_b, ei);
    assert!(
        close(deflection, w * l.powi(4) / (185.0 * ei), 5e-3),
        "{deflection}"
    );
}

#[test]
fn three_equal_spans_uniform_load_coefficients() {
    // M_B = M_C = −0.100wL²; R = 0.400, 1.100, 1.100, 0.400 wL;
    // M+ = 0.080wL² in the end spans and 0.025wL² in the middle one.
    let (w, l, ei) = (8.0, 3.5, 15_000.0);
    let ends = continuous_beam(&[(l, w, ei), (l, w, ei), (l, w, ei)]);
    let m_b = ends[0].1;
    assert!(close(m_b, -0.100 * w * l * l, 1e-9));
    assert!(close(ends[2].0, -0.100 * w * l * l, 1e-9));
    let r_a = left_shear(l, w, 0.0, m_b);
    let r_b = (w * l - r_a) + left_shear(l, w, ends[1].0, ends[1].1);
    assert!(close(r_a, 0.400 * w * l, 1e-9));
    assert!(close(r_b, 1.100 * w * l, 1e-9));
    assert!(close(span_moment(l, w, 0.0, m_b), 0.080 * w * l * l, 1e-9));
    assert!(close(
        span_moment(l, w, ends[1].0, ends[1].1),
        0.025 * w * l * l,
        1e-9
    ));
}

#[test]
fn aisc_case_31_two_equal_spans_central_point_loads() {
    // A point load P at the centre of each span: M_B = −3PL/16,
    // R_A = 5P/16, R_B = 11P/8.
    let (p, l, ei) = (20.0, 5.0, 30_000.0);
    let loads = vec![vec![(l / 2.0, p)], vec![(l / 2.0, p)]];
    let ends = continuous_beam_with_point_loads(&[(l, 0.0, ei), (l, 0.0, ei)], &loads);
    let m_b = ends[0].1;
    assert!(close(m_b, -3.0 * p * l / 16.0, 1e-9));
    let r_a = p / 2.0 + m_b / l;
    assert!(close(r_a, 5.0 * p / 16.0, 1e-9));
    assert!(close(2.0 * (p - r_a), 11.0 * p / 8.0, 1e-9));
}

#[test]
fn yield_lines_give_the_classic_tributary_areas() {
    let rect = |w: f64, h: f64| {
        vec![
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, h),
            Point::new(0.0, h),
        ]
    };
    let edges = |o: &[Point]| -> Vec<(u64, Point, Point)> {
        (0..4)
            .map(|i| (i as u64 + 1, o[i], o[(i + 1) % 4]))
            .collect()
    };
    // A square slab supported on all four edges: a quarter to each.
    let square = rect(4.0, 4.0);
    let shares = yield_line_shares(&square, &edges(&square));
    for id in 1..=4 {
        assert!(close(shares[&id], 0.25, 1e-6), "{shares:?}");
    }
    // A 6 × 4 m slab: each short edge takes a triangle lx²/4 = 4 m² of the
    // 24 m², each long edge a trapezoid of 8 m².
    let slab = rect(6.0, 4.0);
    let shares = yield_line_shares(&slab, &edges(&slab));
    for (id, expected) in [
        (1, 8.0 / 24.0),
        (2, 4.0 / 24.0),
        (3, 8.0 / 24.0),
        (4, 4.0 / 24.0),
    ] {
        assert!(close(shares[&id], expected, 1e-6), "{id}: {shares:?}");
    }
}

#[test]
fn branson_inertia_follows_nbr_6118_17_3_2_1_1() {
    // I_eq = (Mr/Ma)³·Ic + [1 − (Mr/Ma)³]·I_II ≤ Ic, with
    // Mr = α·fct·Ic/yt, α = 1.5, fct = 0.3·fck^(2/3) MPa.
    let (b, h, fck): (f64, f64, f64) = (0.14, 0.40, 25.0);
    let e = 0.85 * 5600.0 * fck.sqrt() * 1000.0;
    let ic = b * h.powi(3) / 12.0;
    let m_r = 1.5 * 0.3 * fck.powf(2.0 / 3.0) * 1000.0 * ic / (h / 2.0);
    let below = effective_beam_stiffness(b, h, fck, e, m_r * 0.8, m_r * 0.8);
    assert!(close(below.m_r_knm, m_r, 1e-9));
    assert!(close(below.i_eq_m4, ic, 1e-12), "uncracked below Mr");
    let ma = 2.0 * m_r;
    let above = effective_beam_stiffness(b, h, fck, e, ma, ma);
    let r3 = (m_r / ma).powi(3);
    let expected = r3 * ic + (1.0 - r3) * above.i_cracked_m4;
    assert!(close(above.i_eq_m4, expected, 1e-9));
    assert!(above.i_cracked_m4 < above.i_eq_m4 && above.i_eq_m4 < ic);
}

#[test]
fn eccentric_footing_pressure_follows_the_middle_third_rule() {
    // σmax = N/A·(1 + 6e/B) while e ≤ B/6; overturning FS = B/(2e).
    let (n, b, e) = (200.0, 1.5, 0.15);
    let check = check_footing(n, b, b, 0.4, 0.2, 0.2, e, 0.0, 25.0);
    let soil = 1.05 * n;
    assert!(!check.outside_middle_third);
    assert!(close(
        check.max_pressure_kpa,
        soil / (b * b) * (1.0 + 6.0 * e / b),
        1e-9
    ));
    assert!(close(check.overturning_fs, b / (2.0 * e), 1e-9));
    // Beyond B/6 the base partly lifts and the pressure rises sharply.
    let far = check_footing(n, b, b, 0.4, 0.2, 0.2, 0.4, 0.0, 25.0);
    assert!(far.outside_middle_third && far.max_pressure_kpa > check.max_pressure_kpa);
}

#[test]
fn punching_at_the_column_face_follows_nbr_6118_19_5_3_1() {
    // τ_Sd = F_Sd/(u0·d) ≤ τ_Rd2 = 0.27·αv2·fcd, αv2 = 1 − fck/250.
    let (n, fck, h) = (400.0, 25.0, 0.30);
    let (cw, cd) = (0.20, 0.20);
    let d = h - 0.05;
    let check = check_footing(n, 0.45, 0.45, h, cw, cd, 0.0, 0.0, fck);
    let tau_sd0 = 1.4 * n / (2.0 * (cw + cd) * d);
    let tau_rd2 = 0.27 * (1.0 - fck / 250.0) * fck / 1.4 * 1000.0;
    // The footing barely wider than its column: the d/2 contour falls
    // outside it, so the face check governs.
    assert!(
        close(check.punching_ratio, tau_sd0 / tau_rd2, 1e-9),
        "{check:?}"
    );
}

/// A two-room house with a generated structure.
fn house() -> Project {
    let mut project = Project::default();
    project.set_terrain(Point::new(-2.0, -2.0), Point::new(30.0, 30.0));
    project
        .add_room(Point::new(0.0, 0.0), Point::new(4.0, 4.0), "Sala")
        .unwrap();
    project
        .add_room(Point::new(4.0, 0.0), Point::new(7.0, 4.0), "Cozinha")
        .unwrap();
    project.generate_structure();
    project
}

#[test]
fn loads_are_conserved_from_the_slabs_to_the_columns() {
    // One floor: the column loads add up to the slabs, the beams and the
    // columns' own weight, nothing lost and nothing counted twice.
    let project = house();
    let report = project.structure_report();
    let slabs: f64 = project
        .structure
        .slabs
        .iter()
        .map(|slab| {
            let room = project.rooms.iter().find(|r| r.id == slab.room_id).unwrap();
            let result = report.slabs.iter().find(|r| r.id == slab.id).unwrap();
            result.load_kn_m2 * project.room_area(room).unwrap()
        })
        .sum();
    let beams: f64 = report
        .beams
        .iter()
        .flat_map(|line| &line.spans)
        .map(|s| 25.0 * s.width_m * s.height_m * s.length_m)
        .sum();
    let own: f64 = project
        .structural_columns()
        .map(|c| 25.0 * c.width * c.depth * project.column_height(c))
        .sum();
    let columns: f64 = report.columns.iter().map(|c| c.load_kn).sum();
    assert!(
        close(columns, slabs + beams + own, 0.005),
        "{columns} vs {}",
        slabs + beams + own
    );
}

#[test]
fn loads_are_conserved_across_floors() {
    // With masonry weight left out (so each floor's beams carry the same
    // loads alone or in the stack), each ground column carries exactly its
    // own floor's load plus the column standing on it.
    let mut project = house();
    project.structure.settings.masonry_kn_m3 = 0.0;
    let single = project.structure_report();
    let ground = project.ensure_default_floor();
    project.add_floor("Superior", true, true);
    project.generate_structure();
    let upper_alone: f64 = project
        .structure_report()
        .columns
        .iter()
        .map(|c| c.load_kn)
        .sum();
    project.switch_floor(ground).unwrap();
    let stacked = project.structure_report();
    assert!(stacked.misaligned_columns.is_empty());
    for column in &stacked.columns {
        let alone = single.columns.iter().find(|c| c.id == column.id).unwrap();
        assert!(
            close(column.load_kn - column.upper_load_kn, alone.load_kn, 1e-9),
            "{}",
            column.label
        );
    }
    let carried: f64 = stacked.columns.iter().map(|c| c.upper_load_kn).sum();
    assert!(
        close(carried, upper_alone, 1e-6),
        "{carried} vs {upper_alone}"
    );
}
