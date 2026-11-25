//! 2D Euler–Bernoulli Direct Stiffness Method (DSM) solver with exact static
//! condensation for pinned end releases, fixed-end forces for uniform and
//! point member loads, global equilibrium check, and exact Hermite + load
//! particular deflected shape.

use super::model::{EndRelease, Frame2D, FrameMember, MemberKind, MemberLoad};

/// Global nodal displacement `[u (m), v (m), θ (rad)]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NodalDisplacement {
    pub u_m: f64,
    pub v_m: f64,
    pub theta_rad: f64,
}

/// Global support reaction at a node `[Rx (kN), Ry (kN), Mz (kN·m)]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SupportReaction {
    pub node: usize,
    pub rx_kn: f64,
    pub ry_kn: f64,
    pub mz_knm: f64,
}

/// Global equilibrium residual (`ΣFx, ΣFy, ΣM_origin`) of the solved frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EquilibriumResidual {
    pub sum_fx_kn: f64,
    pub sum_fy_kn: f64,
    pub sum_mz_knm: f64,
}

/// Sample point along a member with internal forces and global deformed point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MemberSample {
    /// Position `x` along the member from `start`, metres.
    pub s_m: f64,
    /// Undeformed global coordinates, metres.
    pub x0: f64,
    pub y0: f64,
    /// Global displacement `(dx, dy)` at this station, metres.
    pub dx_m: f64,
    pub dy_m: f64,
    /// Axial force `N` (tension positive, compression negative), kN.
    pub axial_kn: f64,
    /// Shear force `V`, kN.
    pub shear_kn: f64,
    /// Bending moment `M` (sagging / local `-y'` fibre tension positive), kN·m.
    pub moment_knm: f64,
}

/// Solved internal forces and sampled deformed shape for one frame member.
#[derive(Clone, Debug, PartialEq)]
pub struct MemberResult {
    pub member: usize,
    pub label: String,
    pub kind: MemberKind,
    pub length_m: f64,
    /// Local end forces acting BY nodes ON member: `[N_i, V_i, M_i, N_j, V_j, M_j]`.
    pub local_end_forces: [f64; 6],
    /// Engineering bending moment at start `M(0) = -M_i` and end `M(L) = M_j`, kN·m.
    pub moment_start_knm: f64,
    pub moment_end_knm: f64,
    /// Largest positive (sagging) and largest absolute bending moment along span, kN·m.
    pub max_sagging_knm: f64,
    pub max_abs_moment_knm: f64,
    /// Largest absolute shear force, kN.
    pub max_abs_shear_kn: f64,
    /// Constant axial force (`-N_i`, tension positive, compression negative), kN.
    pub axial_kn: f64,
    /// Largest transverse local deflection magnitude along the member, mm.
    pub max_local_deflection_mm: f64,
    /// Uniformly spaced samples from `s = 0` to `s = L`.
    pub samples: Vec<MemberSample>,
}

/// Complete solution of a 2D plane frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameSolution {
    pub displacements: Vec<NodalDisplacement>,
    pub reactions: Vec<SupportReaction>,
    pub members: Vec<MemberResult>,
    pub equilibrium: EquilibriumResidual,
}

/// Local 6×6 Euler–Bernoulli frame stiffness matrix (rigid-rigid).
fn local_stiffness_rigid(e: f64, a: f64, i: f64, l: f64) -> [[f64; 6]; 6] {
    let ea = e * a / l;
    let ei = e * i;
    let k1 = 12.0 * ei / l.powi(3);
    let k2 = 6.0 * ei / l.powi(2);
    let k3 = 4.0 * ei / l;
    let k4 = 2.0 * ei / l;
    [
        [ea, 0.0, 0.0, -ea, 0.0, 0.0],
        [0.0, k1, k2, 0.0, -k1, k2],
        [0.0, k2, k3, 0.0, -k2, k4],
        [-ea, 0.0, 0.0, ea, 0.0, 0.0],
        [0.0, -k1, -k2, 0.0, k1, -k2],
        [0.0, k2, k4, 0.0, -k2, k3],
    ]
}

/// Fixed-end local force vector `f_fe` (forces exerted BY nodes ON member to
/// hold all 6 local DOFs zero under the member's transverse loads).
fn local_fixed_end_forces(l: f64, loads: &[MemberLoad], member_id: usize) -> [f64; 6] {
    let mut f = [0.0; 6];
    for load in loads {
        match *load {
            MemberLoad::Uniform { member, q_perp } if member == member_id => {
                let v = -0.5 * q_perp * l;
                let m = -q_perp * l * l / 12.0;
                f[1] += v;
                f[2] += m;
                f[4] += v;
                f[5] -= m;
            }
            MemberLoad::Point {
                member,
                a_m,
                p_perp,
            } if member == member_id => {
                let a = a_m.clamp(0.0, l);
                let b = l - a;
                let l2 = l * l;
                let l3 = l2 * l;
                f[1] += -p_perp * b * b * (3.0 * a + b) / l3;
                f[2] += -p_perp * a * b * b / l2;
                f[4] += -p_perp * a * a * (a + 3.0 * b) / l3;
                f[5] += p_perp * a * a * b / l2;
            }
            _ => {}
        }
    }
    f
}

/// Condenses a single released local rotational DOF `r` (`r = 2` or `r = 5`)
/// out of `(k, f_fe)` in-place.
fn condense_dof(k: &mut [[f64; 6]; 6], f_fe: &mut [f64; 6], r: usize) {
    let diag = k[r][r];
    if diag.abs() < 1e-18 {
        return;
    }
    let col: [f64; 6] = [k[0][r], k[1][r], k[2][r], k[3][r], k[4][r], k[5][r]];
    let row = k[r];
    let f_r = f_fe[r];
    for a in 0..6 {
        let factor = col[a] / diag;
        f_fe[a] -= factor * f_r;
        for b in 0..6 {
            k[a][b] -= factor * row[b];
        }
    }
}

/// Applies static condensation according to `release` on local `(k, f_fe)`.
fn apply_release(k: &mut [[f64; 6]; 6], f_fe: &mut [f64; 6], release: EndRelease) {
    match release {
        EndRelease::Rigid => {}
        EndRelease::PinnedStart => condense_dof(k, f_fe, 2),
        EndRelease::PinnedEnd => condense_dof(k, f_fe, 5),
        EndRelease::PinnedBoth => {
            condense_dof(k, f_fe, 2);
            condense_dof(k, f_fe, 5);
        }
    }
}

/// Recovers the true local displacement vector (replacing released end
/// rotations with their exact internal beam-end slopes for deflected shape).
fn recover_internal_local_displacements(
    k_orig: &[[f64; 6]; 6],
    f_fe_orig: &[f64; 6],
    mut u_l: [f64; 6],
    release: EndRelease,
) -> [f64; 6] {
    match release {
        EndRelease::Rigid => u_l,
        EndRelease::PinnedStart => {
            let diag = k_orig[2][2];
            if diag.abs() > 1e-18 {
                let mut rhs = -f_fe_orig[2];
                for c in [0, 1, 3, 4, 5] {
                    rhs -= k_orig[2][c] * u_l[c];
                }
                u_l[2] = rhs / diag;
            }
            u_l
        }
        EndRelease::PinnedEnd => {
            let diag = k_orig[5][5];
            if diag.abs() > 1e-18 {
                let mut rhs = -f_fe_orig[5];
                for c in [0, 1, 2, 3, 4] {
                    rhs -= k_orig[5][c] * u_l[c];
                }
                u_l[5] = rhs / diag;
            }
            u_l
        }
        EndRelease::PinnedBoth => {
            // Solve 2×2 system for (u_l[2], u_l[5]).
            let a11 = k_orig[2][2];
            let a12 = k_orig[2][5];
            let a21 = k_orig[5][2];
            let a22 = k_orig[5][5];
            let det = a11 * a22 - a12 * a21;
            if det.abs() > 1e-18 {
                let mut r1 = -f_fe_orig[2];
                let mut r2 = -f_fe_orig[5];
                for c in [0, 1, 3, 4] {
                    r1 -= k_orig[2][c] * u_l[c];
                    r2 -= k_orig[5][c] * u_l[c];
                }
                u_l[2] = (a22 * r1 - a12 * r2) / det;
                u_l[5] = (-a21 * r1 + a11 * r2) / det;
            }
            u_l
        }
    }
}

/// Orthogonal 6×6 transformation `T` (`u_l = T · u_g`).
fn transformation(c: f64, s: f64) -> [[f64; 6]; 6] {
    [
        [c, s, 0.0, 0.0, 0.0, 0.0],
        [-s, c, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, c, s, 0.0],
        [0.0, 0.0, 0.0, -s, c, 0.0],
        [0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
    ]
}

fn mat_t_vec(t: &[[f64; 6]; 6], v: &[f64; 6]) -> [f64; 6] {
    let mut out = [0.0; 6];
    for r in 0..6 {
        let mut sum = 0.0;
        for k in 0..6 {
            sum += t[k][r] * v[k];
        }
        out[r] = sum;
    }
    out
}

fn mat_vec(m: &[[f64; 6]; 6], v: &[f64; 6]) -> [f64; 6] {
    let mut out = [0.0; 6];
    for r in 0..6 {
        let mut sum = 0.0;
        for c in 0..6 {
            sum += m[r][c] * v[c];
        }
        out[r] = sum;
    }
    out
}

fn transform_stiffness(t: &[[f64; 6]; 6], k_l: &[[f64; 6]; 6]) -> [[f64; 6]; 6] {
    // tmp = k_l * t
    let mut tmp = [[0.0; 6]; 6];
    for i in 0..6 {
        for j in 0..6 {
            let mut sum = 0.0;
            for k in 0..6 {
                sum += k_l[i][k] * t[k][j];
            }
            tmp[i][j] = sum;
        }
    }
    // out = t^T * tmp
    let mut out = [[0.0; 6]; 6];
    for i in 0..6 {
        for j in 0..6 {
            let mut sum = 0.0;
            for k in 0..6 {
                sum += t[k][i] * tmp[k][j];
            }
            out[i][j] = sum;
        }
    }
    out
}

/// Gaussian elimination with partial pivoting for dense `A · x = b`.
fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Result<Vec<f64>, String> {
    let n = b.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    for col in 0..n {
        let mut pivot = col;
        let mut max_val = a[col][col].abs();
        for (row, r_vec) in a.iter().enumerate().skip(col + 1) {
            let v = r_vec[col].abs();
            if v > max_val {
                max_val = v;
                pivot = row;
            }
        }
        if max_val < 1e-12 || !max_val.is_finite() {
            return Err("Pórtico hipostático ou matriz de rigidez singular.".to_owned());
        }
        if pivot != col {
            a.swap(col, pivot);
            b.swap(col, pivot);
        }
        let diag = a[col][col];
        let pivot_row: Vec<f64> = a[col][col..n].to_vec();
        for row in (col + 1)..n {
            let factor = a[row][col] / diag;
            if factor.abs() < 1e-18 {
                continue;
            }
            for (dst, &src) in a[row][col..n].iter_mut().zip(&pivot_row) {
                *dst -= factor * src;
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let mut rhs = b[row];
        for c in (row + 1)..n {
            rhs -= a[row][c] * x[c];
        }
        x[row] = rhs / a[row][row];
    }
    Ok(x)
}

/// Particular transverse deflection `v_p(s)` of a fixed-fixed beam of length
/// `l` and bending rigidity `ei` under all transverse loads on `member_id`.
fn fixed_fixed_particular_deflection(
    s: f64,
    l: f64,
    ei: f64,
    loads: &[MemberLoad],
    member_id: usize,
) -> f64 {
    if ei <= 1e-15 {
        return 0.0;
    }
    let mut v = 0.0;
    for load in loads {
        match *load {
            MemberLoad::Uniform { member, q_perp } if member == member_id => {
                // EI v'''' = q_perp with v(0)=v'(0)=v(L)=v'(L)=0 -> v(s) = q s² (L - s)² / (24 EI).
                v += q_perp * s * s * (l - s).powi(2) / (24.0 * ei);
            }
            MemberLoad::Point {
                member,
                a_m,
                p_perp,
            } if member == member_id => {
                let a = a_m.clamp(0.0, l);
                let b = l - a;
                let l3 = l.powi(3);
                // Fixed-end reaction V_i_fe = -P b²(3a+b)/L³, M_i_fe = -P a b²/L².
                // Bending moment M(x) = -M_i_fe + V_i_fe x + P <x - a>.
                // Integrating EI v'' = M(x) with v(0)=v'(0)=0:
                // EI v(s) = -M_i_fe s²/2 + V_i_fe s³/6 + P <s - a>³/6.
                let v_i = -p_perp * b * b * (3.0 * a + b) / l3;
                let m_i = -p_perp * a * b * b / (l * l);
                let mut ei_v = -0.5 * m_i * s * s + v_i * s.powi(3) / 6.0;
                if s > a {
                    ei_v += p_perp * (s - a).powi(3) / 6.0;
                }
                v += ei_v / ei;
            }
            _ => {}
        }
    }
    v
}

/// Evaluates internal forces and global deformed shape along one member.
fn sample_member(
    frame: &Frame2D,
    member: &FrameMember,
    geom: (f64, f64, f64),
    u_l_true: [f64; 6],
    f_l: [f64; 6],
    n_steps: usize,
) -> MemberResult {
    let (c, s, l) = geom;
    let n_i = frame.nodes[member.start].x;
    let y_i = frame.nodes[member.start].y;
    let ei = member.modulus_kn_m2 * member.inertia_m4;
    let steps = n_steps.max(4);
    let mut samples = Vec::with_capacity(steps + 1);

    let axial_kn = -f_l[0];
    let mut max_sagging = f64::NEG_INFINITY;
    let mut max_abs_moment = 0.0_f64;
    let mut max_abs_shear = 0.0_f64;
    let mut max_local_def_m = 0.0_f64;

    for step in 0..=steps {
        let xi = step as f64 / steps as f64;
        let x = xi * l;

        // Internal shear and bending moment from left-end equilibrium:
        // V(x) = V_i + ∫_0^x q(t) dt + Σ_{a <= x} P
        // M(x) = -M_i + V_i x + ∫_0^x q(t)(x - t) dt + Σ_{a <= x} P(x - a)
        let mut shear = f_l[1];
        let mut moment = -f_l[2] + f_l[1] * x;
        for load in &frame.member_loads {
            match *load {
                MemberLoad::Uniform {
                    member: mid,
                    q_perp,
                } if mid == member.id => {
                    shear += q_perp * x;
                    moment += 0.5 * q_perp * x * x;
                }
                MemberLoad::Point {
                    member: mid,
                    a_m,
                    p_perp,
                } if mid == member.id => {
                    let a = a_m.clamp(0.0, l);
                    if x >= a {
                        shear += p_perp;
                        moment += p_perp * (x - a);
                    }
                }
                _ => {}
            }
        }

        // Exact local axial (linear) + transverse (Hermite cubic + fixed-fixed particular) displacement.
        let u_loc = (1.0 - xi) * u_l_true[0] + xi * u_l_true[3];
        let h1 = 1.0 - 3.0 * xi * xi + 2.0 * xi.powi(3);
        let h2 = l * (xi - 2.0 * xi * xi + xi.powi(3));
        let h3 = 3.0 * xi * xi - 2.0 * xi.powi(3);
        let h4 = l * (-xi * xi + xi.powi(3));
        let v_hom = h1 * u_l_true[1] + h2 * u_l_true[2] + h3 * u_l_true[4] + h4 * u_l_true[5];
        let v_part = fixed_fixed_particular_deflection(x, l, ei, &frame.member_loads, member.id);
        let v_loc = v_hom + v_part;

        // Relative transverse deflection (subtracting linear chord displacement) for member deflection check.
        let v_chord = (1.0 - xi) * u_l_true[1] + xi * u_l_true[4];
        max_local_def_m = max_local_def_m.max((v_loc - v_chord).abs());

        // Transform local displacement (u_loc, v_loc) back to global (dx, dy).
        let dx_m = c * u_loc - s * v_loc;
        let dy_m = s * u_loc + c * v_loc;

        max_sagging = max_sagging.max(moment);
        max_abs_moment = max_abs_moment.max(moment.abs());
        max_abs_shear = max_abs_shear.max(shear.abs());

        samples.push(MemberSample {
            s_m: x,
            x0: n_i + c * x,
            y0: y_i + s * x,
            dx_m,
            dy_m,
            axial_kn,
            shear_kn: shear,
            moment_knm: moment,
        });
    }

    MemberResult {
        member: member.id,
        label: member.label.clone(),
        kind: member.kind,
        length_m: l,
        local_end_forces: f_l,
        moment_start_knm: -f_l[2],
        moment_end_knm: f_l[5],
        max_sagging_knm: max_sagging.max(0.0),
        max_abs_moment_knm: max_abs_moment,
        max_abs_shear_kn: max_abs_shear,
        axial_kn,
        max_local_deflection_mm: max_local_def_m * 1000.0,
        samples,
    }
}

/// Solves a 2D plane frame by the Direct Stiffness Method.
pub fn solve_frame(frame: &Frame2D) -> Result<FrameSolution, String> {
    let n_nodes = frame.nodes.len();
    if n_nodes == 0 || frame.members.is_empty() {
        return Err("Pórtico sem nós ou barras.".to_owned());
    }
    let n_dof = n_nodes
        .checked_mul(3)
        .ok_or("Pórtico excede o limite de memória da análise.")?;
    if n_dof
        .checked_mul(n_dof)
        .and_then(|cells| cells.checked_mul(8))
        .is_none_or(|bytes| bytes > 128 << 20)
    {
        return Err(
            "Pórtico excede o limite de memória da análise; divida o modelo em pórticos menores."
                .to_owned(),
        );
    }
    if frame
        .nodes
        .iter()
        .enumerate()
        .any(|(index, node)| node.id != index || !node.x.is_finite() || !node.y.is_finite())
    {
        return Err("Pórtico contém identificação ou coordenadas de nó inválidas.".to_owned());
    }
    if frame
        .nodal_loads
        .iter()
        .any(|load| !load.fx_kn.is_finite() || !load.fy_kn.is_finite() || !load.mz_knm.is_finite())
        || frame.member_loads.iter().any(|load| match load {
            MemberLoad::Uniform { q_perp, .. } => !q_perp.is_finite(),
            MemberLoad::Point { a_m, p_perp, .. } => !a_m.is_finite() || !p_perp.is_finite(),
        })
    {
        return Err("Pórtico contém cargas não finitas.".to_owned());
    }
    if frame.members.iter().any(|member| {
        [member.area_m2, member.inertia_m4, member.modulus_kn_m2]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    }) {
        return Err("Pórtico contém seção ou rigidez inválida.".to_owned());
    }

    let mut k_global = vec![vec![0.0; n_dof]; n_dof];
    let mut f_global = vec![0.0; n_dof];

    // External nodal loads.
    for nl in &frame.nodal_loads {
        if nl.node >= n_nodes {
            return Err("Carga nodal referencia nó inexistente.".to_owned());
        }
        let base = nl.node * 3;
        f_global[base] += nl.fx_kn;
        f_global[base + 1] += nl.fy_kn;
        f_global[base + 2] += nl.mz_knm;
    }

    // Precompute element kinematics, stiffnesses and FEFs so recovery reuses them identically.
    struct ElemCache {
        c: f64,
        s: f64,
        l: f64,
        t: [[f64; 6]; 6],
        k_orig: [[f64; 6]; 6],
        f_fe_orig: [f64; 6],
        k_cond: [[f64; 6]; 6],
        f_fe_cond: [f64; 6],
        dofs: [usize; 6],
    }

    let mut elems = Vec::with_capacity(frame.members.len());
    for m in &frame.members {
        if m.start >= n_nodes || m.end >= n_nodes {
            return Err("Barra referencia nó inexistente.".to_owned());
        }
        let ni = &frame.nodes[m.start];
        let nj = &frame.nodes[m.end];
        let dx = nj.x - ni.x;
        let dy = nj.y - ni.y;
        let l = dx.hypot(dy);
        if !l.is_finite() || l < 1e-9 {
            return Err(format!("Barra '{}' tem comprimento inválido.", m.label));
        }
        let c = dx / l;
        let s = dy / l;
        let t = transformation(c, s);
        let k_orig = local_stiffness_rigid(m.modulus_kn_m2, m.area_m2, m.inertia_m4, l);
        let f_fe_orig = local_fixed_end_forces(l, &frame.member_loads, m.id);
        let mut k_cond = k_orig;
        let mut f_fe_cond = f_fe_orig;
        apply_release(&mut k_cond, &mut f_fe_cond, m.release);

        let k_g = transform_stiffness(&t, &k_cond);
        let f_eq_g = mat_t_vec(&t, &f_fe_cond);
        let dofs = [
            m.start * 3,
            m.start * 3 + 1,
            m.start * 3 + 2,
            m.end * 3,
            m.end * 3 + 1,
            m.end * 3 + 2,
        ];

        for a in 0..6 {
            f_global[dofs[a]] -= f_eq_g[a];
            for b in 0..6 {
                k_global[dofs[a]][dofs[b]] += k_g[a][b];
            }
        }

        elems.push(ElemCache {
            c,
            s,
            l,
            t,
            k_orig,
            f_fe_orig,
            k_cond,
            f_fe_cond,
            dofs,
        });
    }

    // Partition free vs restrained DOFs.
    let mut free_dofs = Vec::with_capacity(n_dof);
    for node in &frame.nodes {
        let mask = node.support.restraints();
        for (local, restrained) in mask.into_iter().enumerate() {
            if !restrained {
                free_dofs.push(node.id * 3 + local);
            }
        }
    }

    let nf = free_dofs.len();
    let mut k_ff = vec![vec![0.0; nf]; nf];
    let mut f_f = vec![0.0; nf];
    for (r, &dof_r) in free_dofs.iter().enumerate() {
        f_f[r] = f_global[dof_r];
        for (c_idx, &dof_c) in free_dofs.iter().enumerate() {
            k_ff[r][c_idx] = k_global[dof_r][dof_c];
        }
    }

    let u_f = solve_linear(k_ff, f_f)?;
    let mut u_global = vec![0.0; n_dof];
    for (i, &dof) in free_dofs.iter().enumerate() {
        u_global[dof] = u_f[i];
    }

    // Global nodal reactions R = K · u - F_global (non-zero at restrained DOFs).
    let mut r_global = vec![0.0; n_dof];
    for i in 0..n_dof {
        let mut ku = 0.0;
        for j in 0..n_dof {
            ku += k_global[i][j] * u_global[j];
        }
        r_global[i] = ku - f_global[i];
    }

    let displacements: Vec<NodalDisplacement> = (0..n_nodes)
        .map(|i| NodalDisplacement {
            u_m: u_global[i * 3],
            v_m: u_global[i * 3 + 1],
            theta_rad: u_global[i * 3 + 2],
        })
        .collect();

    let mut reactions = Vec::new();
    for node in &frame.nodes {
        let mask = node.support.restraints();
        if mask.iter().any(|&r| r) {
            reactions.push(SupportReaction {
                node: node.id,
                rx_kn: if mask[0] { r_global[node.id * 3] } else { 0.0 },
                ry_kn: if mask[1] {
                    r_global[node.id * 3 + 1]
                } else {
                    0.0
                },
                mz_knm: if mask[2] {
                    r_global[node.id * 3 + 2]
                } else {
                    0.0
                },
            });
        }
    }

    // Recover member end forces, internal diagrams and deformed shape.
    let mut members = Vec::with_capacity(frame.members.len());
    for (m, el) in frame.members.iter().zip(elems.iter()) {
        let u_g_e = [
            u_global[el.dofs[0]],
            u_global[el.dofs[1]],
            u_global[el.dofs[2]],
            u_global[el.dofs[3]],
            u_global[el.dofs[4]],
            u_global[el.dofs[5]],
        ];
        let u_l_nodal = mat_vec(&el.t, &u_g_e);
        let ku_l = mat_vec(&el.k_cond, &u_l_nodal);
        let mut f_l = [0.0; 6];
        for i in 0..6 {
            f_l[i] = ku_l[i] + el.f_fe_cond[i];
        }
        let u_l_true =
            recover_internal_local_displacements(&el.k_orig, &el.f_fe_orig, u_l_nodal, m.release);
        members.push(sample_member(
            frame,
            m,
            (el.c, el.s, el.l),
            u_l_true,
            f_l,
            24,
        ));
    }

    // Independent global equilibrium check: Σ(Reactions + External Loads) = 0.
    let mut sum_fx = 0.0;
    let mut sum_fy = 0.0;
    let mut sum_mz = 0.0;
    for r in &reactions {
        let n = &frame.nodes[r.node];
        sum_fx += r.rx_kn;
        sum_fy += r.ry_kn;
        sum_mz += r.mz_knm + n.x * r.ry_kn - n.y * r.rx_kn;
    }
    for nl in &frame.nodal_loads {
        let n = &frame.nodes[nl.node];
        sum_fx += nl.fx_kn;
        sum_fy += nl.fy_kn;
        sum_mz += nl.mz_knm + n.x * nl.fy_kn - n.y * nl.fx_kn;
    }
    for load in &frame.member_loads {
        match *load {
            MemberLoad::Uniform { member, q_perp } => {
                let m = &frame.members[member];
                let el = &elems[member];
                let ni = &frame.nodes[m.start];
                let nj = &frame.nodes[m.end];
                let total = q_perp * el.l;
                let fx = -el.s * total;
                let fy = el.c * total;
                let mx = 0.5 * (ni.x + nj.x);
                let my = 0.5 * (ni.y + nj.y);
                sum_fx += fx;
                sum_fy += fy;
                sum_mz += mx * fy - my * fx;
            }
            MemberLoad::Point {
                member,
                a_m,
                p_perp,
            } => {
                let m = &frame.members[member];
                let el = &elems[member];
                let ni = &frame.nodes[m.start];
                let a = a_m.clamp(0.0, el.l);
                let px = ni.x + el.c * a;
                let py = ni.y + el.s * a;
                let fx = -el.s * p_perp;
                let fy = el.c * p_perp;
                sum_fx += fx;
                sum_fy += fy;
                sum_mz += px * fy - py * fx;
            }
        }
    }

    Ok(FrameSolution {
        displacements,
        reactions,
        members,
        equilibrium: EquilibriumResidual {
            sum_fx_kn: sum_fx,
            sum_fy_kn: sum_fy,
            sum_mz_knm: sum_mz,
        },
    })
}
