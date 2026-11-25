//! Data model for a general 2D plane frame (`X` right, `Y` up, `θ` CCW).

/// Boundary restraint at a frame node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportKind {
    /// All 3 DOFs (`u, v, θ`) are free.
    Free,
    /// Fixed support (`u = v = θ = 0`).
    Fixed,
    /// Pinned support (`u = v = 0`, `θ` free).
    Pinned,
    /// Vertical roller (`v = 0`, `u` and `θ` free).
    RollerY,
    /// Horizontal restraint / lateral brace (`u = 0`, `v` and `θ` free).
    #[cfg_attr(not(test), allow(dead_code))]
    RollerX,
}

impl SupportKind {
    /// Restrained mask `[u, v, θ]` in global coordinates.
    pub fn restraints(self) -> [bool; 3] {
        match self {
            Self::Free => [false, false, false],
            Self::Fixed => [true, true, true],
            Self::Pinned => [true, true, false],
            Self::RollerY => [false, true, false],
            Self::RollerX => [true, false, false],
        }
    }
}

/// Internal flexural hinge release at member ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndRelease {
    /// Moment-resisting monolithic joint at both ends.
    Rigid,
    /// Internal pin at local start node `i` (`M_i = 0`).
    #[allow(dead_code)]
    PinnedStart,
    /// Internal pin at local end node `j` (`M_j = 0`).
    PinnedEnd,
    /// Internal pins at both ends (`M_i = M_j = 0`).
    #[allow(dead_code)]
    PinnedBoth,
}

/// Semantic role of a frame member (used for diagrams and reporting).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberKind {
    Beam,
    Column,
}

/// A 2D frame node with global coordinates `(x, y)` in metres.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameNode {
    pub id: usize,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub support: SupportKind,
}

/// A 2D Euler–Bernoulli frame element joining `start -> end`.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameMember {
    pub id: usize,
    pub label: String,
    pub kind: MemberKind,
    pub start: usize,
    pub end: usize,
    /// Cross-sectional area `A`, m².
    pub area_m2: f64,
    /// In-plane second moment of area `I`, m⁴.
    pub inertia_m4: f64,
    /// Young's modulus `E`, kN/m².
    pub modulus_kn_m2: f64,
    /// Flexural end release condition.
    pub release: EndRelease,
}

/// Distributed or concentrated transverse load applied along a member in local
/// `+y'` direction (left of `start -> end`; for a left-to-right horizontal
/// beam, gravity downward is negative).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MemberLoad {
    /// Uniform transverse load `q_perp` (kN/m) over the full member length.
    Uniform { member: usize, q_perp: f64 },
    /// Point transverse load `p_perp` (kN) at distance `a_m` from `start`.
    Point {
        member: usize,
        a_m: f64,
        p_perp: f64,
    },
}

/// External nodal load in global coordinates (`+X` right, `+Y` up, `+Mz` CCW).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodalLoad {
    pub node: usize,
    pub fx_kn: f64,
    pub fy_kn: f64,
    pub mz_knm: f64,
}

/// Complete 2D plane frame definition ready for Direct Stiffness analysis.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame2D {
    pub nodes: Vec<FrameNode>,
    pub members: Vec<FrameMember>,
    pub member_loads: Vec<MemberLoad>,
    pub nodal_loads: Vec<NodalLoad>,
}

impl Frame2D {
    pub fn add_node(
        &mut self,
        label: impl Into<String>,
        x: f64,
        y: f64,
        support: SupportKind,
    ) -> usize {
        let id = self.nodes.len();
        self.nodes.push(FrameNode {
            id,
            label: label.into(),
            x,
            y,
            support,
        });
        id
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_member(
        &mut self,
        label: impl Into<String>,
        kind: MemberKind,
        start: usize,
        end: usize,
        area_m2: f64,
        inertia_m4: f64,
        modulus_kn_m2: f64,
        release: EndRelease,
    ) -> usize {
        let id = self.members.len();
        self.members.push(FrameMember {
            id,
            label: label.into(),
            kind,
            start,
            end,
            area_m2,
            inertia_m4,
            modulus_kn_m2,
            release,
        });
        id
    }

    pub fn add_uniform_load(&mut self, member: usize, q_perp: f64) {
        if q_perp.abs() > 1e-12 {
            self.member_loads
                .push(MemberLoad::Uniform { member, q_perp });
        }
    }

    pub fn add_point_load(&mut self, member: usize, a_m: f64, p_perp: f64) {
        if p_perp.abs() > 1e-12 {
            self.member_loads.push(MemberLoad::Point {
                member,
                a_m,
                p_perp,
            });
        }
    }

    pub fn add_nodal_load(&mut self, node: usize, fx_kn: f64, fy_kn: f64, mz_knm: f64) {
        if fx_kn.abs() > 1e-12 || fy_kn.abs() > 1e-12 || mz_knm.abs() > 1e-12 {
            self.nodal_loads.push(NodalLoad {
                node,
                fx_kn,
                fy_kn,
                mz_knm,
            });
        }
    }
}
