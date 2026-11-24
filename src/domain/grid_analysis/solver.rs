//! 3-DOF/node grillage / grid-analogy FEM solver (#109, Hambly 1991 / NBR 6118 §14.7.5).
//!
//! Global DOFs per node: `[w (downward, m), θx (∂w/∂y, rad), θy (-∂w/∂x, rad)]`.
//! Solves the symmetric positive-definite global system `[K]{u} = {F}` via
//! banded Cholesky decomposition (`L·Lᵀ`) so coupled slab+beam grids with
//! hundreds of nodes solve in a few milliseconds.

use crate::domain::geometry::Point;

/// A node in the 2D structural grillage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridNode {
    pub id: usize,
    pub point: Point,
    /// Vertical support restraint (`w = 0`, e.g. column or rigid wall support).
    pub fix_w: bool,
    /// Rotational restraint about X (`θx = 0`).
    pub fix_rx: bool,
    /// Rotational restraint about Y (`θy = 0`).
    pub fix_ry: bool,
    /// Additional nodal downward point load, kN.
    pub point_load_kn: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridMemberKind {
    SlabX { slab_id: u64 },
    SlabY { slab_id: u64 },
    Beam { beam_id: u64 },
}

/// One beam or slab-strip element connecting two grid nodes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridMember {
    pub id: usize,
    pub start: usize,
    pub end: usize,
    pub kind: GridMemberKind,
    /// Flexural rigidity `E·I` (kN·m²).
    pub ei_knm2: f64,
    /// Torsional rigidity `G·J` (kN·m²).
    pub gj_knm2: f64,
    /// Tributary strip width (m) for slab moments per metre (`kN·m/m`); `1.0` for beams.
    pub strip_width_m: f64,
    /// Uniform downward distributed load along the member, kN/m.
    pub q_kn_m: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridMemberResult {
    pub member_id: usize,
    pub kind: GridMemberKind,
    pub length_m: f64,
    pub strip_width_m: f64,
    /// Sagging-positive bending moments at start, midspan and end (kN·m).
    pub m_start_knm: f64,
    pub m_mid_knm: f64,
    pub m_end_knm: f64,
    /// Maximum shear force (kN).
    pub v_max_kn: f64,
    /// Torsional moment (kN·m).
    pub torsion_knm: f64,
    /// Maximum vertical deflection along the member (mm).
    pub w_max_mm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GridSolveOutput {
    /// Nodal deflections `w` (downward positive, m) per node.
    pub node_w_m: Vec<f64>,
    /// Nodal rotations `(θx, θy)` (rad) per node.
    pub node_rot: Vec<(f64, f64)>,
    /// Upward vertical reaction `R_z` (kN) at each node (nonzero at `fix_w` nodes).
    pub node_reaction_kn: Vec<f64>,
    pub member_results: Vec<GridMemberResult>,
    pub total_applied_kn: f64,
    pub total_reaction_kn: f64,
    pub equilibrium_error_kn: f64,
}

fn local_stiffness(l: f64, ei: f64, gj: f64) -> [[f64; 6]; 6] {
    let l2 = l * l;
    let l3 = l2 * l;
    let a = 12.0 * ei / l3;
    let b = 6.0 * ei / l2;
    let c1 = 4.0 * ei / l;
    let c2 = 2.0 * ei / l;
    let t = gj / l;

    // Local DOF order: [w_i, φx_i, φy_i, w_j, φx_j, φy_j]
    // with dw/dx' = -φy (right-hand rule with +z downward).
    [
        [a, 0.0, -b, -a, 0.0, -b],
        [0.0, t, 0.0, 0.0, -t, 0.0],
        [-b, 0.0, c1, b, 0.0, c2],
        [-a, 0.0, b, a, 0.0, b],
        [0.0, -t, 0.0, 0.0, t, 0.0],
        [-b, 0.0, c2, b, 0.0, c1],
    ]
}

fn transform_matrix(c: f64, s: f64) -> [[f64; 6]; 6] {
    // u_loc = T * u_glob:
    // w_loc = w; φx = c*θx + s*θy; φy = -s*θx + c*θy
    [
        [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, c, s, 0.0, 0.0, 0.0],
        [0.0, -s, c, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, c, s],
        [0.0, 0.0, 0.0, 0.0, -s, c],
    ]
}

fn mat_mul_6(a: &[[f64; 6]; 6], b: &[[f64; 6]; 6]) -> [[f64; 6]; 6] {
    let mut out = [[0.0; 6]; 6];
    for i in 0..6 {
        for k in 0..6 {
            let aik = a[i][k];
            if aik == 0.0 {
                continue;
            }
            for j in 0..6 {
                out[i][j] += aik * b[k][j];
            }
        }
    }
    out
}

fn mat_t_mul_6(a: &[[f64; 6]; 6], b: &[[f64; 6]; 6]) -> [[f64; 6]; 6] {
    let mut out = [[0.0; 6]; 6];
    for i in 0..6 {
        for k in 0..6 {
            let aki = a[k][i];
            if aki == 0.0 {
                continue;
            }
            for j in 0..6 {
                out[i][j] += aki * b[k][j];
            }
        }
    }
    out
}

fn mat_vec_6(a: &[[f64; 6]; 6], x: &[f64; 6]) -> [f64; 6] {
    let mut out = [0.0; 6];
    for i in 0..6 {
        let mut s = 0.0;
        for j in 0..6 {
            s += a[i][j] * x[j];
        }
        out[i] = s;
    }
    out
}

fn mat_t_vec_6(a: &[[f64; 6]; 6], x: &[f64; 6]) -> [f64; 6] {
    let mut out = [0.0; 6];
    for i in 0..6 {
        let mut s = 0.0;
        for j in 0..6 {
            s += a[j][i] * x[j];
        }
        out[i] = s;
    }
    out
}

/// Banded symmetric positive-definite Cholesky solver (`A * x = b`).
/// `band` stores lower-triangular entries `band[i * (bw + 1) + (i - j)]` for `j <= i <= j + bw`.
fn solve_banded_cholesky(
    n: usize,
    bw: usize,
    band: &mut [f64],
    rhs: &[f64],
) -> Result<Vec<f64>, String> {
    let stride = bw + 1;
    // In-place Cholesky factorization: A = L * L^T.
    for i in 0..n {
        let j_start = i.saturating_sub(bw);
        for j in j_start..=i {
            let mut sum = band[i * stride + (i - j)];
            let k_start = i.saturating_sub(bw).max(j.saturating_sub(bw));
            for k in k_start..j {
                sum -= band[i * stride + (i - k)] * band[j * stride + (j - k)];
            }
            if i == j {
                if !sum.is_finite() || sum <= 1e-12 {
                    return Err(
                        "Sistema de grelha singular ou sem apoios verticais suficientes."
                            .to_owned(),
                    );
                }
                band[i * stride] = sum.sqrt();
            } else {
                let diag = band[j * stride];
                band[i * stride + (i - j)] = sum / diag;
            }
        }
    }

    // Forward substitution: L * y = rhs.
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut sum = rhs[i];
        let j_start = i.saturating_sub(bw);
        for j in j_start..i {
            sum -= band[i * stride + (i - j)] * y[j];
        }
        y[i] = sum / band[i * stride];
    }

    // Backward substitution: L^T * x = y.
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = y[i];
        let k_end = (i + bw).min(n - 1);
        for k in (i + 1)..=k_end {
            sum -= band[k * stride + (k - i)] * x[k];
        }
        x[i] = sum / band[i * stride];
    }
    Ok(x)
}

/// Solves a 2D grillage model `[K]{u} = {F}` and recovers nodal reactions,
/// deflections and member internal forces.
pub fn solve_grid(nodes: &[GridNode], members: &[GridMember]) -> Result<GridSolveOutput, String> {
    if nodes.is_empty() || members.is_empty() {
        return Err("Grelha vazia.".to_owned());
    }
    let n_nodes = nodes.len();
    let total_dofs = n_nodes * 3;

    // Map free DOFs to compact indices `0..n_free`.
    let mut dof_map = vec![usize::MAX; total_dofs];
    let mut n_free = 0usize;
    for (idx, node) in nodes.iter().enumerate() {
        if !node.fix_w {
            dof_map[idx * 3] = n_free;
            n_free += 1;
        }
        if !node.fix_rx {
            dof_map[idx * 3 + 1] = n_free;
            n_free += 1;
        }
        if !node.fix_ry {
            dof_map[idx * 3 + 2] = n_free;
            n_free += 1;
        }
    }
    if n_free == 0 {
        return Err("Todos os graus de liberdade da grelha estão restringidos.".to_owned());
    }

    // Compute exact half-bandwidth over connected free DOFs.
    let mut bw = 0usize;
    for m in members {
        let g = [
            m.start * 3,
            m.start * 3 + 1,
            m.start * 3 + 2,
            m.end * 3,
            m.end * 3 + 1,
            m.end * 3 + 2,
        ];
        for &a in &g {
            let fa = dof_map[a];
            if fa == usize::MAX {
                continue;
            }
            for &b in &g {
                let fb = dof_map[b];
                if fb != usize::MAX && fa >= fb {
                    bw = bw.max(fa - fb);
                }
            }
        }
    }

    let stride = bw + 1;
    let mut k_band = vec![0.0_f64; n_free * stride];
    let mut f_total = vec![0.0_f64; total_dofs];
    let mut total_applied_kn = 0.0;

    for (idx, node) in nodes.iter().enumerate() {
        f_total[idx * 3] += node.point_load_kn;
        total_applied_kn += node.point_load_kn;
    }

    // Store precomputed element transforms for fast force recovery.
    let mut elem_cache = Vec::with_capacity(members.len());

    for m in members {
        let p0 = nodes[m.start].point;
        let p1 = nodes[m.end].point;
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let l = dx.hypot(dy);
        if l < 1e-6 {
            continue;
        }
        let c = dx / l;
        let s = dy / l;
        let k_loc = local_stiffness(l, m.ei_knm2, m.gj_knm2);
        let t = transform_matrix(c, s);
        let k_glob = mat_t_mul_6(&t, &mat_mul_6(&k_loc, &t));

        // Equivalent nodal load vector in local coordinates.
        let f_loc = [
            m.q_kn_m * l * 0.5,
            0.0,
            -m.q_kn_m * l * l / 12.0,
            m.q_kn_m * l * 0.5,
            0.0,
            m.q_kn_m * l * l / 12.0,
        ];
        let f_glob = mat_t_vec_6(&t, &f_loc);
        total_applied_kn += m.q_kn_m * l;

        let g = [
            m.start * 3,
            m.start * 3 + 1,
            m.start * 3 + 2,
            m.end * 3,
            m.end * 3 + 1,
            m.end * 3 + 2,
        ];
        for i in 0..6 {
            f_total[g[i]] += f_glob[i];
            let fi = dof_map[g[i]];
            if fi == usize::MAX {
                continue;
            }
            for j in 0..6 {
                let fj = dof_map[g[j]];
                if fj != usize::MAX && fi >= fj {
                    k_band[fi * stride + (fi - fj)] += k_glob[i][j];
                }
            }
        }
        elem_cache.push((m, l, k_loc, t, f_loc, g));
    }

    let mut rhs = vec![0.0_f64; n_free];
    for d in 0..total_dofs {
        let fd = dof_map[d];
        if fd != usize::MAX {
            rhs[fd] = f_total[d];
        }
    }

    let u_free = solve_banded_cholesky(n_free, bw, &mut k_band, &rhs)?;
    let mut u_glob = vec![0.0_f64; total_dofs];
    for d in 0..total_dofs {
        let fd = dof_map[d];
        if fd != usize::MAX {
            u_glob[d] = u_free[fd];
        }
    }

    // Recover nodal reactions (R = K*u - F_eq) and element internal forces.
    let mut r_glob = vec![0.0_f64; total_dofs];
    for d in 0..total_dofs {
        r_glob[d] = -f_total[d];
    }

    let mut member_results = Vec::with_capacity(elem_cache.len());
    for (m, l, k_loc, t, f_loc, g) in elem_cache {
        let u_e = [
            u_glob[g[0]],
            u_glob[g[1]],
            u_glob[g[2]],
            u_glob[g[3]],
            u_glob[g[4]],
            u_glob[g[5]],
        ];
        let u_loc = mat_vec_6(&t, &u_e);
        let f_el_loc = mat_vec_6(&k_loc, &u_loc);
        // Add K_e * u_e to global reactions vector.
        let f_el_glob = mat_t_vec_6(&t, &f_el_loc);
        for i in 0..6 {
            r_glob[g[i]] += f_el_glob[i];
        }

        // Net member end forces (k_loc * u_loc - f_loc).
        let f_int = [
            f_el_loc[0] - f_loc[0],
            f_el_loc[1] - f_loc[1],
            f_el_loc[2] - f_loc[2],
            f_el_loc[3] - f_loc[3],
            f_el_loc[4] - f_loc[4],
            f_el_loc[5] - f_loc[5],
        ];

        // Sagging-positive bending moments.
        let m_start = -f_int[2];
        let m_end = f_int[5];
        let m_mid = 0.5 * (m_start + m_end) + m.q_kn_m * l * l / 8.0;
        let v_max = f_int[0].abs().max(f_int[3].abs());
        let torsion = f_int[4].abs().max(f_int[1].abs());

        // Midspan deflection from Hermite interpolation + particular uniform-load sag.
        let w_i = u_loc[0];
        let psi_i = -u_loc[2];
        let w_j = u_loc[3];
        let psi_j = -u_loc[5];
        let w_mid = 0.5 * (w_i + w_j)
            + (l / 8.0) * (psi_i - psi_j)
            + (m.q_kn_m * l.powi(4)) / (384.0 * m.ei_knm2.max(1e-9));
        let w_max_mm = w_i.abs().max(w_j.abs()).max(w_mid.abs()) * 1000.0;

        member_results.push(GridMemberResult {
            member_id: m.id,
            kind: m.kind,
            length_m: l,
            strip_width_m: m.strip_width_m,
            m_start_knm: m_start,
            m_mid_knm: m_mid,
            m_end_knm: m_end,
            v_max_kn: v_max,
            torsion_knm: torsion,
            w_max_mm,
        });
    }

    let mut node_w_m = Vec::with_capacity(n_nodes);
    let mut node_rot = Vec::with_capacity(n_nodes);
    let mut node_reaction_kn = Vec::with_capacity(n_nodes);
    let mut total_reaction_kn = 0.0;
    for i in 0..n_nodes {
        node_w_m.push(u_glob[i * 3]);
        node_rot.push((u_glob[i * 3 + 1], u_glob[i * 3 + 2]));
        // Upward reaction opposes downward load (`-r_glob` at restrained w DOF).
        let rz = if nodes[i].fix_w { -r_glob[i * 3] } else { 0.0 };
        node_reaction_kn.push(rz);
        total_reaction_kn += rz;
    }

    Ok(GridSolveOutput {
        node_w_m,
        node_rot,
        node_reaction_kn,
        member_results,
        total_applied_kn,
        total_reaction_kn,
        equilibrium_error_kn: (total_reaction_kn - total_applied_kn).abs(),
    })
}
