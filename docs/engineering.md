# Engineering methods

This guide documents every engineering calculation in Trazo: the formula, the standard it follows, the assumptions and the limits, and the test that checks it. It is the reference behind the [safety contract](../ROADMAP.md#engineering-in-trazo-the-safety-contract): if a result cannot be traced to a section here, it should not be shown as engineering.

> **Pre-sizing, not design.** Every result here is a first estimate to guide decisions and the conversation with a licensed professional. Building requires a professional's responsibility (ART or RRT) and the municipal permits.

> **Normative edition notice (2026).** The structural methods already present in Trazo were implemented and traced against ABNT NBR 6118:2023. The ABNT catalogue now lists NBR 6118:2026. The new edition has not yet received a clause-by-clause audit in Trazo, so no output claims compliance with the 2026 edition; every structural result remains explicitly preliminary until that audit and professional review are complete.

- [Conventions](#conventions)
- [Loads](#loads)
- [Slabs](#slabs)
- [Concentrated loads](#concentrated-loads)
- [Beams](#beams)
- [Preliminary reinforcement](#preliminary-reinforcement)
- [Columns](#columns)
- [Load path across floors](#load-path-across-floors)
- [Footings](#footings)
- [Electrical installation](#electrical-installation)
- [Construction quantities](#construction-quantities)
- [Applicability limits](#applicability-limits)
- [Validation suite](#validation-suite)
- [Known limitations](#known-limitations)

## Conventions

| Item | Convention |
| --- | --- |
| Units | Metres, kilonewtons (kN), kN·m, kN/m², kPa (= kN/m²), MPa for material strengths, volt-amperes (VA) |
| Moments | Sagging positive: hogging (support) moments are negative |
| Loads | Characteristic values unless stated; ultimate design uses γf = 1.4 (NBR 8681 normal combination, simplified) |
| Quasi-permanent load | g + ψ2·q with ψ2 = 0.3 (dwellings, NBR 6118:2023, table 11.2) |
| Code | `src/domain/structure/`, `src/domain/reinforcement/`, `src/domain/roof/`, `src/domain/soil/`, `src/domain/foundations/`, `src/domain/electrical/`, `src/domain/distribution/`, `src/domain/conduits.rs`, `src/domain/verdict/` |

## Loads

| Load | Value | Source |
| --- | --- | --- |
| Reinforced concrete | 25 kN/m³ | NBR 6120:2019 (unit weights of materials) |
| Masonry (hollow ceramic blocks, plastered) | 13 kN/m³ (setting) | NBR 6120:2019 (unit weights of materials), typical value |
| Lattice slab (*treliçada*) self weight | 25 · h · 0.55 kN/m² | Approximation: joists, fillers and topping weigh about 55 % of a solid slab of the same thickness |
| Solid slab self weight | 25 · h kN/m² | — |
| Finishes and screed | 1.0 kN/m² (setting) | Typical value |
| Live load, dwellings | 1.5 kN/m² (setting) | NBR 6120:2019, table 10 |
| Live load per room (optional) | corridors, service areas 2.0; stairs, balconies 2.5; garages 3.0 kN/m² | NBR 6120:2019, table 10 (simplified) |
| Walls on beams | t · H · γ (kN/m), for the floors above | — |

Loads are applied as uniform loads per beam span, plus point loads where a beam or a column rests on a beam.

## Slabs

**Minimum thickness** (`SlabKind::minimum_thickness`), from the smaller span lx:

| Kind | Rule |
| --- | --- |
| Solid | h ≥ lx / 40, at least 8 cm (NBR 6118:2023, 13.2.4.1 for floor slabs) |
| Lattice | 12 cm up to 3.5 m, 16 cm up to 4.5 m, 20 cm up to 5.5 m, 25 cm up to 6.5 m, otherwise 30 cm (typical manufacturer tables) |

**Spans.** Rectangular rooms use their sides. Irregular and L-shaped rooms are split into rectangular panels (`decompose_slab_panels`), and the governing lx is the smallest panel span.

**Load to the beams.** The slab load (self weight, finishes and live load, times the room area) goes to the beams along its edges by the 45° yield-line method (`yield_line_shares`, NBR 6118:2023, 14.7.6.1):

- a rectangle sheds triangles to its short edges and trapezoids to its long ones;
- other shapes shed each point to the nearest supported edge;
- edges without a beam carry nothing, and their share goes to the supported ones.

**Two-way moments and deflection** (`slabs::analyse_panel`). A solid slab is analysed as the rectangular panels returned by `decompose_slab_panels`; this makes an L-shaped room an explicit set of rectangles instead of using its bounding box. Each panel has four simply supported edges (case 1), `λ = ly/lx` and:

- `Mx = q·lx²/mx` and `My = q·lx²/my`, calculated from both Czerny and Marcus coefficients, linearly interpolated between the tabulated ratios; the larger result governs each direction;
- for `λ > 2`, the panel is one-way: `Mx = q·lx²/8` and `My = 0` before minimum transverse steel;
- Czerny deflection `a0 = (g + 0.3q)·lx⁴/(w·Ecs·h³)`, multiplied by the same long-term factor `2.32` used for beams, must be at most `lx/250`;
- a concentrated load's local strip moment is conservatively added in both directions.

The table ordinates are checked in tests at `λ = 1.00` and `1.50` against the published case-1 examples from [IFRN's Czerny tables](https://docente.ifrn.edu.br/edilbertoborja/construcoes-em-concreto-armado/lajes-macicas/laje-macica) and the [UTFPR Marcus appendix](https://riut.utfpr.edu.br/jspui/bitstream/1/15782/1/lajesmacicasconcretoarmado.pdf). Lattice slabs are not homogeneous plates: Trazo keeps their equivalent preliminary mesh and explicitly leaves final joist, filler and deflection verification to the manufacturer.

## Concentrated loads

Water tanks and heavy equipment are stored as structural fixtures with a footprint and a positive characteristic load. The water-tank default is 10.81 kN: 9.81 kN for 1,000 L of water plus 1.0 kN for the tank and fittings. The 15 kN equipment default is only an editable placeholder and must be replaced with manufacturer data. The load is treated as permanent in the quasi-permanent service combination.

**Load path.** If the footprint intersects a beam axis, the full load becomes a point load at that position on the beam. Otherwise Trazo finds the containing slab and sends the load to the nearest supported edge beam(s); beams at practically the same distance (within 5 cm) share it equally. The beam stiffness analysis then passes the reactions to columns and footings. A load outside every slab and beam is red and contributes nothing until it is repositioned or given its own support.

**Local slab model.** For a load on a slab, the conservative additional one-metre-strip moment is `Mk,local = P·lx/(4·beff)`, with `beff = min(ly, max(0.50 m, smaller footprint side + 2h))`. That moment is added to the slab flexural reinforcement demand. For a solid slab, punching is checked around the loaded footprint at its face (`u0 = 2(w + d)`, or `πD` for circular water tanks) against `τRd2` and at the NBR 6118:2023 §19.5.2.1 `2d` rounded critical contour (`u1 = 2(w + d) + 4πd`, or `π(D + 4d)` for circular tanks) against `τRd1`. For a lattice/ribbed slab, Trazo does not know the joist and filler geometry, so it deliberately leaves punching unverified and asks for a beam, solid strip or manufacturer check.

**Limitations:** the nearest-edge split is a transparent preliminary load path, not plate finite-element analysis; load dispersion through pedestals, vibration/impact and dynamic equipment forces are not modelled. Equipment input must already include those effects when applicable.

## Beams

**Analysis** (`continuous_beam_with_point_loads`): each run of collinear beams through consecutive supports is a continuous beam on pinned supports, solved by the slope-deflection (stiffness) method:

- unknowns: the rotation at each support;
- element stiffness: 4EI/L and 2EI/L;
- fixed-end moments: −wL²/12 and +wL²/12 for uniform loads, and −Pab²/L² and +Pa²b/L² for point loads.

From the end moments come the shears, the reactions (passed to the columns, or as point loads to a supporting beam) and the largest span moment.

**Secondary beams.** Either end may be supported at a parameter `t` along another beam. The secondary beam is solved first; its end reaction becomes a point load at that exact `t` on the primary girder, whose analysis then carries it to its own supports. Cyclic supports, a beam supported on itself and coincident ends are refused instead of producing a result.

**Sizing rules:**

| Check | Rule | Source |
| --- | --- | --- |
| Height | h ≥ L/10 in end spans and single spans, L/12 in interior spans of continuous beams; at least 25 cm | Usual pre-sizing rule |
| Width | b ≥ 12 cm | NBR 6118:2023, 13.2.2 |
| Deflection | long-term deflection ≤ L/250 | NBR 6118:2023, 13.3, table 13.3 |

**Deflection** (`effective_beam_stiffness`, `span_deflection_with_point_loads`):

1. The continuous beam is solved directly under the quasi-permanent load `(g + 0.3q)` and quasi-permanent point loads (`continuous_beam_with_point_loads`), giving exact quasi-permanent support moments without scaling errors when point loads are present.
2. The equivalent inertia is Branson's formula, NBR 6118:2023, 17.3.2.1.1: I_eq = (Mr/Ma)³·Ic + [1 − (Mr/Ma)³]·I_II ≤ Ic, with:
   - the cracking moment Mr = α·fct·Ic / yt, where α = 1.5 for rectangles and fct = 0.3·fck^(2/3) MPa;
   - I_II, the cracked transformed inertia, with Es = 210 GPa and the tension steel pre-sized for 1.4·Mk (CA-50, fyd = 500/1.15 MPa, never below ρmin from table 17.3).
3. The modulus is Ecs = 0.85 · 5600 · √fck MPa. This is the secant modulus of NBR 6118:2003; for fck = 25 MPa and granite aggregate it is within 2 % of the αi·Eci of NBR 6118:2023, 8.2.8.
4. The curvature M/EI is integrated twice numerically (100 steps). The long-term value multiplies the immediate deflection by 1 + αf = 2.32 (αf = 1.32 for loading at about one month, no compression steel, 17.3.2.1.2).

**Assumptions:** the continuous-beam pre-sizing table uses pinned supports (no torsion); coupled beam–column frame action is computed in the dedicated 2D frame analysis module (`src/domain/frame2d/`, #94) below.

## 2D plane-frame analysis (#94)

Each continuous beam line and its supporting columns are assembled and solved together as a coupled 2D Euler–Bernoulli plane frame (`src/domain/frame2d/`):

| Feature | Formulation | Reference |
| --- | --- | --- |
| Element stiffness | 6×6 local Euler–Bernoulli frame element (`EA/L`, `12EI/L³`, `6EI/L²`, `4EI/L`, `2EI/L`) transformed by `K_g = Tᵀ K_l T` with 3 DOFs/node (`u, v, θ`) | Direct Stiffness Method |
| Column & beam stiffness | Real axial area `A = b·h` and in-plane inertia `I = b·h³/12` aligned with the frame axis, concrete secant modulus `Ecs = 0.85 · 5600 · √fck` | NBR 6118 |
| Gravity loads | Every span receives its characteristic distributed dead + live load and every retained concentrated load at its actual distance from the left support; this includes tanks/equipment, secondary-beam and roof reactions, and transfer columns | Same load path as the continuous-beam analysis |
| Rigid & pinned joints | Monolithic rigid beam–column joints (`EndRelease::Rigid`) or exact static condensation for internal hinges (`PinnedStart`, `PinnedEnd`, `PinnedBoth`); column bases fixed (`Fixed`) or pinned (`Pinned`), with automatic sway bracing (`RollerX`) when both joints and bases are pinned | Direct Stiffness Method |
| Loads | Characteristic uniform gravity loads (`dead + live`) and concentrated loads via exact fixed-end forces, plus frequent service horizontal wind at roof level `H = ψ1 · q · (H/2) · b_trib · 1.2` (`ψ1 = 0.30`, NBR 6118:2023 table 11.2 / 13.3) | NBR 6120, NBR 6123, NBR 6118 |
| Internal forces & deformed shape | Member axial `N`, shear `V(x)`, bending moment `M(x)` for both beams and columns, support reactions (`Rx, Ry, Mz`), exact Hermite cubic + fixed-fixed particular deflected shape, and global equilibrium check (`ΣFx, ΣFy, ΣM`) | Structural mechanics |
| Lateral drift | Service lateral sway `Δh` at beam level checked against `H / 850` | NBR 6118:2023, table 13.3 |
| Solver failure contract | Every eligible line that cannot be solved keeps its frame label and the original diagnostic. It is excluded only from numerical result tables, while an explicit red failure is emitted to warnings, safety verdicts, the UI, PDF memorial, A3 diagram and MCP `falhas_porticos_2d` | Fail-safe reporting contract |

Validated in CI by `cantilever_column_matches_exact_moment_and_tip_deflection`, `frame_reduces_to_continuous_beam_when_column_stiffness_vanishes`, `symmetric_fixed_base_portal_gravity_matches_slope_deflection`, `symmetric_fixed_base_portal_lateral_sway_matches_slope_deflection`, `pinned_end_releases_zero_end_moments_and_match_simply_supported_deflection`, `simply_supported_midspan_point_load_matches_exact_solution`, `project_frame_keeps_a_water_tank_load_on_its_beam`, `solver_and_project_report_expose_an_unstable_frame`, and `project_2d_frames_satisfy_global_equilibrium_and_render_valid_svg`. **Limitations:** linear-elastic 2D planar analysis per frame line (no 3D torsion, P–Δ second-order geometric nonlinearity, or cracked-section stiffness reduction inside the global frame matrix).

## Masonry lintels, sills and panels

`masonry_structure_report` derives these elements from the live architectural model, so moving a wall or opening changes them without a second stored copy. The method is a conservative pre-sizing interpretation of ABNT NBR 16868-1:2020 for small buildings; it does not replace the masonry designer's specification of unit/prism tests, grouting, bond beams and reinforcement details.

**Lintels and sills.** Every opening that leaves masonry above it receives a lintel (`VA`); full-height open spans do not. Every window with a sill receives a sill band (`CV`). Both extend up to the preferred 30 cm into the wall on each side and fail the check below 20 cm. The simplified lintel method stops at a 3.0 m clear span.

- width = the raw masonry width;
- depth = whole masonry courses, rounded up from `max(10 cm, L/12)`;
- effective span `Lef = L0 + min(a1/2, h/2) + min(a2/2, h/2)` (NBR 16868-1);
- when `masonry above ≥ L0/2`, the triangular masonry arch carries peak load `0.5·L0·t·γm` (`q_avg = 0.25·L0·t·γm`) with triangular bending moment `q_avg·Lef²/6`; when `masonry above < L0/2`, the full rectangular masonry strip `clear_above·t·γm` acts with `/8`. Self-weight `25·b·h` and `qfloor` (`0` for infill, or `2.5 + finish + live` kN/m for load-bearing walls) act uniformly with `Lef²/8`;
- `Md = 1.4·Mk`, and `As = Md/(0.9·d·fyd)` with CA-50. Two equal bars are selected from 6.3, 8, 10, 12.5 and 16 mm; sills receive the crack-control minimum `2Ø6.3`.

**Wall panels.** A masonry name containing *estrutural* opts that wall into the load-bearing checks; all other names are treated as infill. Height slenderness is `hef/t`, limited here to 24 for unreinforced load-bearing walls and to the conservative interface limit 35 for infill. Transverse wall returns (at least 60° to the panel) and concrete columns divide the unbraced length; collinear wall joints do not count as lateral bracing. This remains a simplified geometric check and does not verify the return's own anchorage. Infill is limited to 4.5 m; load-bearing masonry uses `min(max(30t, 2.5 m), 4.5 m)`.

For a load-bearing panel, the preliminary design compression is `σd = 1.4·(γm·t·H + (2.5 + finish + live))·floors/(t·net_fraction)`. Openings reduce `net_fraction`. Until the material catalogue (#135) stores tested prism results, Trazo deliberately assumes `fpk = 3 MPa` for structural ceramic block, 4 MPa for structural concrete block and 2 MPa for another custom structural name. Following NBR 16868-1 §6.2.2 and §11.2.2, the design wall resistance applies the 0.70 prism-to-wall factor and cubic slenderness reduction: `fd,red = (0.70·fpk/2.0)·max(0, 1 − (λ/40)³)`.

**Known limits:** spans over 3 m, concentrated loads directly over an opening, reinforced/grouted masonry, out-of-plane wind and global stability require a masonry engineer. Wind on tall walls is checked separately by #147. All assumed prism strengths must be replaced by the specified unit, mortar, grout and prism tests before construction.

Regression checks: `every_built_opening_gets_a_lintel_and_windows_get_a_sill`, `an_opening_too_close_to_the_wall_end_fails_the_bearing_check`, and `slender_infill_and_loaded_structural_masonry_are_checked_separately`.

## Preliminary reinforcement

`build_reinforcement_report` turns the characteristic forces from `structure_report` into a reviewable reinforcement proposal and a theoretical bar schedule. It uses CA-50 (`fyk = 500 MPa`), steel density 7,850 kg/m³ and the same structural applicability limits. If the whole project or an element is outside those limits, it refuses to emit reinforcement for it.

**Beams.** For the span and support moments, `Md = 1.4·|Mk|`. A singly reinforced rectangular section uses the 0.85 concrete stress block, `fcd = fck/1.4`, `fyd = 500/1.15 MPa`, effective depth `d = h − 4 cm` and lever arm `z = d·(1 − 0.4ξ)`. The required area is `As = Md/(z·fyd)`, never below the minimum ratio already used by the cracked-section calculation. The selector tries 10, 12.5, 16, 20 and 25 mm bars and checks whether one layer fits with cover, stirrup and clear spacing of at least the larger of 20 mm or one diameter.

**Beams (flexure, shear, fit and anchorage).** Longitudinal steel uses `d' = 4 cm` (`d = h − 4 cm`), NBR 6118:2023 Table 17.3 minimum ratios (`ρmin` from 0.15% at C20 up to 0.208% at C50), and warns if total longitudinal steel exceeds `4%·Ac` (§17.3.5.2.4). Single-layer bar fit uses nominal cover `c_nom = 3 cm` (CAA II, Table 7.2), actual selected stirrup diameter, and minimum clear spacing `max(20 mm, Ø, 1.2·19 mm)` (§18.3.2.2). Stirrups compare `Vsd = 1.4·Vk` with `Vc = 0.60·fctd·bw·d`; the remaining shear gives `Asw/s = (Vsd − Vc)/(0.9·d·fyd)`, never below the minimum transverse ratio. Spacing is capped by `d` and shear level (`0.30·d ≤ 20 cm` when `Vsd > 0.67·VRd2`). If diagonal compression (`VRd2`) is exceeded or no stirrup table entry satisfies `Asw/s`, the check turns red and asks for a larger section. Hooked anchorage lengths follow NBR 6118:2023 §9.3–9.4: `fbd = η1·η2·fctd` (`η1 = 2.25` for ribbed CA-50, `η2 = 1.0` good bond for bottom bars and shallow beams, `η2 = 0.7` poor bond for top bars when `h ≥ 30 cm`), `lb = (Ø/4)·(fyd/fbd) ≥ 25·Ø`, and `lb,nec = α1·lb·(As,calc/As,ef) ≥ lb,min` with `α1 = 0.7` for 90° hooks only when perpendicular cover is at least `3·Ø` (§9.4.2.5), otherwise `α1 = 1.0`.

**Columns.** Longitudinal steel satisfies NBR 6118:2023 §17.3.5.3.1 `As,min = max(0.15·Nd/fyd, 0.004·Ac)` and the axial compression demand using Domain-5 steel stress `420 MPa` net of displaced concrete (`420 − 0.85·fcd`). The selector uses an even number of bars, at least four. Floor-level starter lap splices assume 100% of bars spliced in the same section (`α0t = 2.0`, Table 9.3, `l0 = α0t·lb,nec ≥ max(l0,min, 40 cm)`). Tie diameter is chosen dynamically `Øt ≥ max(5 mm, Øl/4)` (§18.4.3) and tie spacing is capped by 20 cm, `12·Øl` and the smaller section dimension. The traffic light checks the steel ratio against the 4.0% pre-detailing limit.

**Slabs.** Solid slabs use the governing Czerny/Marcus moments in X and Y, deducting the lower-layer diameter `Øx` from the upper Y-layer effective depth (`d'_y = 2.5 cm + Øx`); ribbed slabs treat a one-metre strip as simply supported in the smaller direction with `Mk = q·lx²/8`. Mesh spacing is capped by NBR 6118:2023 §20.1 `s_max = min(2h, 20 cm)`; distribution steel is at least 20% of the main requirement and 0.90 cm²/m. For lattice slabs, this is only an equivalent steel estimate and the selected joist/trellis must be confirmed with its manufacturer.

**Footings.** Each direction envelopes cantilever flexure from the column face (incorporating any boundary-footing eccentricity `ex, ey`) with the NBR 6118:2023 §22.5.3 rigid-footing strut-and-tie tension `Td = 1.1·Nd·(B − b)/(8·d)` at 7 cm nominal cover, warning if the compression zone exceeds the ductile limit (`x/d > 0.45`), with 90° upturned bar hooks inside the footing height. It produces the bottom X/Y meshes only after the soil-pressure, overturning and punching checks have been run.

**Schedule, diameter summary and detailing sheet.** The schedule lists mark, element, position, bar shape, diameter, quantity, unit length, total length and theoretical mass, followed by a CA-50 summary by diameter (`Resumo por bitola`) giving total metres, commercial 12 m bar counts and `+10%` cutting/bending waste mass. An A3 schematic detailing sheet (`reinforcement_detail_svg`, exported to SVG/PDF in UI and MCP) illustrates the governing beam cross-section/elevation (with Branson B12 `Mr`, Stage I/II and `I_eq/Ic`), column section/lap, footing plan/section, slab list and CA-50 summary. Openings, construction joints, bend deductions, detailing congestion and any NBR 6118:2026 audit remain the licensed engineer's responsibility (ART/RRT).

## Columns

**Load:**

- the sum of the beam reactions on the column (upward reactions ignored, with a warning);
- plus the column's own weight, 25 · b · h · H, with H the column's height: the top of the tallest wall it stands in (within half its section + 6 cm of the wall's axis), or the storey height when it stands free;
- plus the load of the column standing on it (see [floors](#load-path-across-floors)).

Without real upper floors, the floor's load is multiplied by the *floors* setting instead.

**Required concrete area:** Ac = 1.4 · α · N / (0.85·fcd + ρ·σs), with:

- fcd = fck / 1.4;
- ρ = 1.5 % steel at σs = 420 MPa (CA-50 at 2 ‰ strain);
- α, the usual pre-sizing factor for the bending a column takes at its position: 1.3 for an interior column, 1.5 at an edge, 1.8 at a corner or an isolated column.

The position comes from the beams reaching the column: four directions make it interior, three or two opposite ones an edge, two at an angle or one a corner.

**Minimums:** 360 cm² and 14 cm a side (NBR 6118:2023, 13.2.3).

**Slenderness:** λ = le / i = 3.46 · le / h, with le = H (the column braced at the top by the beams) and h its smallest side (`src/domain/wall_heights.rs`):

- λ ≤ 35: short column, the local second-order effects can be ignored (λ₁, NBR 6118 15.8.2, at its usual value) — green;
- 35 < λ ≤ 90: the second-order effects must be added in the final design by the simplified standard-column methods (NBR 6118 15.8.3) — yellow;
- λ > 90: outside the simplified methods, so outside this pre-sizing — red, with the smallest side (in 5 cm steps) that brings λ to 90.

A 14 cm column is fine up to 3.64 m (λ = 90); a 14 cm column in a 4.30 m boundary wall gives λ = 106 and asks for 20 cm.

**Assumptions:** axial load with the α factor standing in for bending; the slenderness is checked but no second-order moments are computed, and a column free at the top (le = 2 H, a cantilever) is not modelled; no frame moments (#94).

Checked by `a_column_in_a_tall_wall_is_as_tall_as_the_wall_and_may_be_slender`.

## Load path across floors

`structure_report` walks the real upper floors from the top down. For each upper column:

- If a column of the floor below lies within 15 cm (or under its section), the upper column's load is added to it.
- If it lands within 20 cm of a beam below, the load becomes a point load on that beam, which acts as a transfer girder and is reported as such.
- If nothing supports it, a warning says the column has no support below.

When real upper floors exist, the *floors* multiplier is not used.

## Footings

| Check | Rule | Source |
| --- | --- | --- |
| Plan area | A = 1.05 · N / σadm (5 % for the footing's own weight), the same overhang on every side of the column, at least 60 cm | Usual pre-sizing |
| Boundary walls | A column or footing on a wall along the lot line is moved flush inside the lot; the footing is then off-centre from its column and checked for eccentric pressure and overturning (a balance beam, #144, relieves it) | Lot line (Código Civil art. 1.299–1.301) |
| Height | h = 2/3 of the largest overhang, at least 25 cm (rigid footing) | NBR 6118:2023, 22.6.1 (rigidity) |
| Eccentric pressure | while `ex/Bx + ey/By ≤ 1/6`: `σmax = N/A · (1 + 6ex/Bx + 6ey/By)`; beyond the middle third: monotonic triangular peak `max((4/3)·N / ((Bx − 2ex)(By − 2ey)), 2·N / (Bx·By))` | Classic soil mechanics (middle-third rule), NBR 6122:2019 |
| Overturning | FS = 1 / (2·(ex/Bx + ey/By)) ≥ 1.5 | NBR 6122:2019 |
| Punching at the column face | τSd0 = 1.4·N / (u0·d) ≤ τRd2 = 0.27·αv2·fcd, αv2 = 1 − fck/250 | NBR 6118:2023, 19.5.3.1 & 22.6.2 |
| Punching at 2d (flexible footings) | for overhangs > 2d: rounded perimeter `u1 = 2(cw + cd) + π(rx + ry)` checked against `τRd1 = 0.13·(1 + √(20/d))·(100·ρ·fck)^(1/3)` (ρ = 0.2%); rigid footings (overhang ≤ 2d) are governed by `τRd2` per §22.6.2 | NBR 6118:2023, 19.5.2.1 & 22.6.2 |
| Elastic settlement | s = q · B · (1 − ν²) · Is / Es ≤ 25 mm (footings, Skempton–MacDonald differential `δ = max(smax − smin, 0.50·smax)` ≤ 15 mm); ≤ 40 mm (raft); ≤ 15 mm (piles), with Es correlated to SPT N̄ (Teixeira & Godoy) | NBR 6122:2019 (B11) |

Allowable soil pressures by class (`SoilClass`): soft clay 80 kPa, medium clay 150 kPa, medium sand 200 kPa, dense sand 300 kPa, weathered rock 500 kPa. When one or more SPT boreholes (`SP-01`, `SP-02`, …, NBR 6484) are entered in `src/domain/soil/`, Trazo builds the **conservative governing envelope** across boreholes (weakest mean `N̄` in the pressure bulb, shallowest water table, common explored depth), computes `σadm = 20·N̄ ≤ 400 kPa` for spread footings, sizes both **hand-dug bored piles** (*brocas*, Ø 20–30 cm, 3–6 m, dry soil only) and **deep mechanized piles** (*hélice contínua* / CFA, Ø 30–50 cm, 6–20 m, allowed below the water table) via Décourt-Quaresma (1978/1996), estimates elastic/differential settlements, and compares **footings, raft (*radier*), brocas and deep CFA piles** for the structure's real loads. These are preliminary review studies; the final foundation design belongs to a geotechnical/structural engineer.


### Wall foundations: ground beams and strip footings

Every ground-floor wall reaches the soil through a ground beam or a strip footing (`src/domain/wall_foundations.rs`):

| Quantity | Rule |
| --- | --- |
| Wall load | q = γ · t · (net wall area) / L, kN/m, with each wall's own (mean) height and the openings discounted; ground beams iterate self-weight against their actual sized height `h`, and strip footings solve closed-form width for `25 cm` thickness without double-counting the 5% factor |
| Choice | a ground beam (*viga baldrame*) when a column stands within 30 cm of each end of the wall; a strip footing (*sapata corrida*) otherwise; walls whose openings take 90 % or more of their length are left out |
| Ground beam | a continuous beam over the columns on the wall's line; each loaded overhang imposes `M = −q·a²/2` at its end support, which enters the continuous-span stiffness solution and changes all support reactions. Its load `q·a` also enters the end support reaction. The maximum span/overhang moment and shear size the section from 30 to 80 cm for Md = 1.4 M and Vd = 1.4 V, with its steel; signed reactions reach the footings (and P₁ of a balance beam). A negative reaction raises a red support check and a warning to review the beam-column connection and the combined support stability |
| Strip footing | closed-form `B = q_wall / (σadm/1.05 − 25·0.25)` (at least 40 cm, in 5 cm steps); pressure `(1.05·q_wall + 25·0.25·B) / B` checked against σadm |

Checked by `walls_between_columns_get_ground_beams_and_the_others_strip_footings`, `ground_beam_reactions_reach_the_footings`, `ground_beam_overhang_reactions_follow_force_and_moment_equilibrium`, `excessive_overhang_reports_uplift_instead_of_hiding_the_reaction` and `a_taller_wall_weighs_more_on_its_foundation`.

### Balance beams (*vigas alavanca*) and boundary strip alternative

A boundary footing stands off its column by `e`. A balance beam to an interior column at distance `l` turns the pair into a lever (`src/domain/foundations/`):

| Quantity | Formula |
| --- | --- |
| Reaction on the boundary footing, centred | `R₁ = P₁ · l / (l − e)` |
| Relief on the interior column | `ΔP = R₁ − P₁ = P₁ · e / (l − e)`; when multiple balance beams share the same interior anchor column, Trazo accumulates `ΣΔP` before checking anchor uplift (`N₂ − ΣΔP`), overturning (`FS = N₂ / ΣΔP ≥ 1.5`), and anchor footing pressure (`N₂ − ΣΔP/2`, usual NBR 6122 recommendation) |
| Pair overturning safety factor | `FS = N₂ / ΣΔP ≥ 1.5` (NBR 6122:2019); below `1.0` the interior column lifts |
| Pair soil pressures | `σ₁ = 1.05 · R₁ / A₁ ≤ σadm` on the boundary footing and `σ₂ = 1.05 · (N₂ − ΣΔP/2) / A₂ ≤ σadm` on the anchor footing |
| Beam moment, shear & steel | `M = P₁ · e` (hogging, top steel), `V = max(ΔP, P₁)` sized for `Md = 1.4 M` and `Vd = 1.4 V` with top bars and closed stirrups (including §18.3.3.2 `s_max = min(0.30d, 20 cm)` when `Vsd > 0.67·VRd2`) |
| Strip-footing alternative | When `FS < 1.5` or pair soil pressure exceeds `σadm`, Trazo sizes the equivalent continuous boundary strip footing (`B = 1.05 · q / σadm ≥ 40 cm`) |
| Section | the lowest height from 40 to 120 cm with μ = Md / (0.85 fcd b d²) ≤ 0.295 (x/d ≤ 0.45) and τwd ≤ τRd2, Md = 1.4 M, Vd = 1.4 V; As = Md / (z fyd), at least ρmin·b·h (NBR 6118:2023 Table 17.3) |

*Vigas alavanca* lays one from every boundary footing more than 5 cm off its column to the nearest column inward (within 30° of the offset, 1.5 to 10 m away). Checked by `the_lever_follows_the_textbook_statics` (P₁ = 100 kN, l = 4 m, e = 0.30 m: R₁ = 108.1 kN, ΔP = 8.1 kN, M = 30 kN·m), `boundary_footings_are_centred_by_balance_beams_to_interior_columns` and `shared_anchor_column_checks_cumulative_uplift_across_multiple_balance_beams`. The beam's own weight is not yet included in `M` and `V`.

### Soil from a survey and the foundation study

A standard penetration test (SPT, NBR 6484) gives the blow count N every metre and the soil there. `src/domain/soil/` reads one or more boreholes and builds the conservative governing envelope:

| Quantity | Rule | Source |
| --- | --- | --- |
| Allowable pressure for footings | σadm = 20 · N̄ kPa (N̄ / 50 MPa), with N̄ the mean blow count from the footing base D to D + 2B (B at least 0.60 m); at most 400 kPa; footings not advised below N̄ = 5 | Usual Brazilian rule (Teixeira and Godoy), valid for 5 ≤ N̄ ≤ 20 |
| Soil pressure of the structure | *Usar na tensão do solo* (or `definir_sondagem`) takes σadm for 1 m footings, rounded down to 10 kPa (down to 10 kPa minimum so weak soils are never artificially lifted); a warning shows when the survey allows less than the pressure adopted | — |
| Pile, tip | Rp = α · C · Np · Ap, Np the mean N from L − 1 to L + 1; C = 120 (clay), 200 (clayey silt), 250 (sandy silt), 400 kPa (sand) | Décourt-Quaresma (1978) |
| Pile, shaft | Layer-by-layer integration `Rl = Σ β_i · 10 · (N_i / 3 + 1) · U · ΔL_i` (3 ≤ N_i ≤ 50), tip readings left out | Décourt-Quaresma (1978/1996) |
| Pile factors | α = 0.85 / 0.60 / 0.50 and β = 0.80 / 0.65 / 0.50 (bored) or α = 0.30 / 0.30 / 0.30 and β = 1.00 (CFA) for clay / silts / sand | Décourt (1996) |
| Allowable load | min((Rp + Rl) / 2, Rp / 4 + Rl / 1.3), the tip taking at most 20 % (≤ 1.25 · Rl / 2), and at most 5 MPa on the concrete | NBR 6122 (bored piles), Décourt-Quaresma |
| Piles sized | Hand-dug *brocas* (20, 25 or 30 cm, 3 to 6 m, dry soil only) and deep CFA (*hélice contínua*, 30 to 50 cm, 6 to 20 m, allowed below water table); selector prefers options meeting `s ≤ 15 mm` | Usual practice & NBR 6122 |

The foundation study (`foundation_study`) takes the column loads with the ground beams, and the walls on strip footings, and compares:

- **footings**: isolated footings (A = 1.05 · N / σadm, at least 60 cm, height max(25 cm, (B − 20 cm) / 3)) and strip footings, not advised on soft soil (N̄ < 5) or when they would cover more than half the footprint (the bounding rectangle of walls and columns); a water table above the base + 30 cm is flagged;
- **raft** (*radier*): a 15 cm slab with edge beams; mean pressure 1.05 · ΣN / A + 3.75 kPa against `20 · min(N̄[0.5, 3 m], N̄[0.5, 1.5·Beq])`, catching both shallow and deep soft strata, not advised below N̄ = 3;
- **bored piles & CFA piles with ground beams**: the pile that takes the least concrete while meeting the 15 mm settlement limit, with at most three per column (cap length `3d + side` for two piles), and piles at most 2.5 m apart under the strip walls.

It recommends the viable option that takes the least concrete. Checked by `the_footing_pressure_follows_the_mean_blow_count_in_the_bulb`, `decourt_quaresma_matches_a_hand_calculation`, `brocas_stay_above_the_water_table_and_within_six_metres`, `good_soil_recommends_footings_and_soft_soil_rules_them_out`, `the_study_picks_footings_on_firm_soil_brocas_on_soft_top_and_deep_piles_under_water`, `elastic_settlement_matches_hand_calculation` and `the_survey_sets_the_soil_pressure_of_the_structure`.

## Timber roofs

A mono-pitch roof (*meia-água*) over a plan rectangle (`src/domain/roof/`): tiles on battens (*ripas*), on rafters (*caibros*), on purlins (*terças*) along the eaves, on main beams (*vigas*) from the high edge to the low edge. Fibre-cement and metal sheets rest on the purlins directly.

| Quantity | Rule | Source |
| --- | --- | --- |
| Tiles | saturated weight 0.50 kN/m² (ceramic, concrete), 0.18 (fibre-cement 6 mm), 0.10 (metal); usual minimum slope 30 / 30 / 15 / 10 %; gauge 35 / 32 cm; sheets span at most 1.69 m (fibre-cement) and 2.0 m (metal) between purlins | Manufacturers' usual values |
| Dead load | tiles (+ 0.10 kN/m² of battens and rafters only for tile roofs, zero for sheet roofs) on the surface, divided by cos θ for the projection; + 0.15 kN/m² of ceiling unless the roof is exposed; + each piece's own weight (apparent density) | NBR 6120 |
| Live load | 0.25 kN/m² on the horizontal projection | NBR 6120 (roofs without access) |
| Timber | hardwood classes C20–C60 (Table 9: fc0,k 20–60 MPa, fv0,k 4–8 MPa, Ec0,m 9,500–24,500 MPa) and Conifer C20 for *Pinus elliottii* (Table 8: fc0,k 20 MPa, fv0,k 4 MPa, Ec0,m 3,500 MPa, ρ = 500 kg/m³) | NBR 7190:1997, tables 8 & 9 |
| Design strength | fd = kmod · fk / γw, kmod = 0.70 · 1.0 · 0.80 = 0.56 (long-term, roof service class 1–2, second grade; 0.448 for exterior posts in class 3–4 with kmod2 = 0.80), γw = 1.4 (bending, compression) and 1.8 (shear) | NBR 7190 |
| Bending and shear | simply supported under 1.4 (g + q): σ = M / W ≤ fc0,d, τ = 1.5 V / (b h) ≤ fv0,d; battens take the normal load component; purlins (*terças*) are checked in **biaxial bending** (`σx/fc0,d + kM·σy/fc0,d ≤ 1` and `kM·σx/fc0,d + σy/fc0,d ≤ 1`, `kM = 0.5`, NBR 7190 §7.3.4) with resultant deflection `√(δx² + δy²)` | NBR 7190 |
| Deflection | 5 (g + 0.2 q) L⁴ / (384 Ec0,ef I), Ec0,ef = kmod · Ec0,m, within L / 200 (L along the slope for rafters and beams) | NBR 7190 |
| Sections | the lightest that passes: battens 5 × 1.5 or 5 × 2.5; rafters 5 × 6 to 6 × 12; purlins 6 × 12 to 10 × 25; beams 6 × 16 to 15 × 30 cm | Commercial sizes |
| Main beams | at the columns found on the high and low edges (within 35 cm), more added so none is farther apart than the spacing set (3 m by default); each carries its tributary width | — |
| Supports | each beam end, (g + q) · run / 2, is a point load on the column under it (within 35 cm), else on the concrete beam under it (within 25 cm); a roof load is never multiplied by the *floors* setting | — |

*Ajustar as paredes ao telhado* gives the walls under the roof its height at their ends (#145): the wall on the high side rises and the gables slope. The timber and tile quantities come out per piece. Checked by `the_slope_and_heights_follow_the_high_side`, `beams_line_up_with_the_columns_and_fill_wide_gaps`, `a_beam_matches_the_hand_calculation`, `the_gourmet_roof_is_framed_and_bears_on_the_columns` and `an_assistant_roofs_the_gourmet_area_and_the_columns_take_it`. Not covered yet: gable and hip roofs, overhangs, trusses, the 1 kN maintenance point load, and wind uplift and anchorage (#147).

### Timber posts (*mourões*) and timber beams

A roof beam end with no concrete column or beam under it bears on a timber post (`src/domain/timber_posts.rs`) within 35 cm, or on a timber beam along the roof edge that spans from post to post (simply supported; each load shared by statics, P·b/L and P·a/L). The posts take part in lining up the roof beams. Round posts when width = depth (A = πd²/4, I = πd⁴/64), square otherwise; the post height is the roof height over it.

| Check | Rule | Source |
| --- | --- | --- |
| Slenderness | λ = L0 / i, L0 = the post height (braced at the top by the roof), at most 140 | NBR 7190:1997, 7.5 |
| Short post (λ ≤ 40) | σNd = 1.4 (Ngk + Nqk) / A ≤ fc0,d | NBR 7190 |
| Medium and slender posts | σNd / fc0,d + σMd / fc0,d ≤ 1, Md = Nd · ed, ed = (h/30 + L0/300 + ec) · FE / (FE − Nd), FE = π² Ec0,ef I / L0² | NBR 7190 |
| Creep (λ > 80) | ec = (h/30 + L0/300) · (exp(φ Ngq / (FE − Ngq)) − 1), φ = 0.8, Ngq = Ngk + 0.2 Nqk | NBR 7190 |
| Lateral wind | p = 1.3q from the project wind settings acts over the exposed width of the top-braced post: Fw = p·b·L0 and propped moment Mw,k = p·b·L0²/8, amplified by the second-order P–Δ factor `FE/(FE − Nd)` (NBR 7190 §7.5.4/7.5.5) and added to the axial/buckling use; exterior posts use `kmod2 = 0.80` (moisture class 3–4) | NBR 6123 + NBR 7190 interaction |
| Timber beam | 6 × 12 to 12 × 30 cm: σ = 1.4 M / W ≤ fc0,d, τ = 1.5 · 1.4 V / (b h) ≤ fv0,d, the deflections of each load (P b (L² − b²)^1.5 / (9√3 L E I)) and of the own weight added, within L / 200 | NBR 7190 |
| Base | square concrete block, B ≥ 40 cm and D ≥ 60 cm in 5 cm steps; stabilizing gravity uses `0.9·dead_kn + block_weight` (excluding live load) and overturning includes base shear lever arm `Mbase = Mw + Fw·D`, checking `σmax ≤ σadm` and no soil tension (`6·Mbase/(Nstab·B) ≤ 1`); design anchor tension = `1.4·Mw/(0.8·d)` | Preliminary stability + NBR 7190 durability |

The post/eave-beam catalogue accepts a certified C20/C30/C40/C60 lot or *Pinus elliottii*, eucalipto-citriodora (*Corymbia citriodora*), itaúba (*Mezilaurus itauba*), cumaru (*Dipteryx odorata*) and jatobá (*Hymenaea* spp.). Named species use a deliberately conservative preliminary floor—C20 for pine and C30 for the listed hardwoods—but do not certify the delivered lot: its grading and mechanical class still have to be specified and verified. Brazilian species names/properties are cross-checked against the [Brazilian Forest Service/LPF database](https://lpf.florestal.gov.br/pt-br/bd-madeiras-brasileiras) and the official [LPF timber housing reference](https://www.gov.br/florestal/pt-br/assuntos/laboratorio-de-produtos-florestais/publicacoes/livros/livros-editados-pelo-lpf/habitacao-popular-em-madeira.pdf/@@download/file).

All exterior posts and eave beams must be marked as preservative-treated for the intended use class, even when the species is known as naturally durable; the steel anchor keeps every post clear of soil. An untreated member fails the durability traffic light. The project stores the species/class and treatment with backward-compatible migration from the old `post_timber` class field. Checked by `timber_species_have_stable_keys_and_conservative_classes`, `treatment_defaults_true_and_old_class_projects_migrate`, `a_short_post_is_checked_in_plain_compression`, `a_slender_post_pays_for_buckling_and_creep`, `lateral_wind_adds_bending_to_the_axial_post_check`, `the_gourmet_posts_carry_the_low_edge_of_the_roof` and `timber_posts_carry_the_low_edge_of_a_roof`.

## Wind

`src/domain/wind.rs` applies NBR 6123 to what an open-sided building such as a gourmet area adds to a closed house: walls taller than the storey and a roof the wind can get under.

| Quantity | Rule | Source |
| --- | --- | --- |
| Dynamic pressure | q = 0.613 · Vk² N/m², Vk = V0 · S1 · S2 · S3; V0 from the isopleth map (35 m/s by default), S1 = S3 = 1 by default | NBR 6123, 4.2 and 5 |
| S2 | b · Fr · (max(z, 5 m) / 10)^p with b, Fr and p of the terrain category (I to V, III by default) and the building class (A ≤ 20 m, B 20–50 m, C > 50 m), z the top of the wall or roof | NBR 6123, tables 1 & 2 |
| Tall-wall columns | a column standing in a wall taller than the storey (+ 20 cm) takes w = 1.3 · q · (its share of the wall); 1.3 is the net coefficient of a wall of an open-sided building | NBR 6123 (walls, dominant openings) |
| Top condition | propped by the roof when a roof beam bears on the column (M = w H² / 8, which needs the roof braced down to the gable walls), else a cantilever from its footing (M = w H² / 2) | — |
| Column check | bending across the wall, Md = 1.4 M, `d = h − 5 cm`, μ = Md / (0.85 fcd b d²) ≤ 0.295 (x/d ≤ 0.45), steel per face Md / (z fyd); the smallest depth across the wall that passes is suggested | NBR 6118 |
| Roof uplift | (−Cpe + Cpi) · q with Cpe = −1.0 (conservative for one slope) and Cpi = +0.8 when a side is open (a beam end on a post or on nothing), else +0.2 for closed buildings | NBR 6123, 6.2.5 |
| Anchorage | net 1.4 · uplift − 0.9 · g; when positive, each beam end is anchored for it over its share of the roof, and the tiles are tied | NBR 8681 combinations |

Checked by `the_pressure_follows_nbr_6123`, `a_tall_wall_column_is_propped_by_the_roof_or_stands_as_a_cantilever` and `the_wind_checks_the_tall_wall_and_lifts_an_open_roof`. For the gourmet area (4.30 m wall, V0 = 35 m/s, category III) a 14 cm column across the wall does not hold the wall as a cantilever; propped by a braced roof it does. Not covered: the masonry panel between columns (NBR 16868), the global stability of the building, and the bracing of the roof and of the posts themselves.

## Electrical installation

| Calculation | Rule | Source |
| --- | --- | --- |
| Lighting load | 100 VA for the first 6 m², plus 60 VA per whole 4 m² beyond | NBR 5410:2004, 9.5.2.1.2 |
| Outlets per room | 1 per 3.5 m of perimeter in kitchens and service areas, 1 per 5 m elsewhere, at least 1 in bathrooms and balconies | NBR 5410:2004, 9.5.2.2.1 |
| Outlet power | 600 VA for the first three in wet rooms, 100 VA otherwise | NBR 5410:2004, 9.5.2.2.2 |
| Breaker | smallest standard rating ≥ circuit current | — |
| Wire | smallest section whose capacity (copper, PVC 70 °C, method B1, 30 °C) ≥ breaker — 2 loaded conductors for single-phase branch circuits, 3 loaded conductors for 2F+N / 3F+N service entrances and QDC feeders (table 46); at least 1.5 mm² for lighting and 2.5 mm² for outlets | NBR 5410:2004, tables 36, 46 & 6.2.6.1.1 |
| Voltage drop | ΔV% = 2 · ρ · Σ(L·I) / (S · V) · 100 (or √3 · ρ · L · I / (S · V_ll) · 100 for 3F+N feeders), ρ = 1/56 Ω·mm²/m; wire raised until ≤ 4 % | NBR 5410:2004, 6.2.7.1 |
| RCD | 30 mA on circuits of wet areas (including bathroom lighting), garages and outdoor areas | NBR 5410:2004, 5.1.3.2.2 |
| Conduit fill | ≤ 53 % for one conductor, 31 % for two, 40 % for three or more; standard sizes DN 20 through DN 75 | NBR 5410:2004, 6.2.11.1.6 |
| Demand | lighting and general outlets by the table of their installed kVA (0.86 up to 1 kVA … 0.24 above 10 kVA); specific-use circuits by their count (1.00 for one or two … 0.38 from 24); per-phase entrance and QDC demand currents apply general and specific demand factors separately so high-current TUEs are never diluted | Usual utility tables for dwellings |
| Supply and entrance | single/two/three-phase from the demand; main breaker from the demand current on the most loaded phase; feeder from the 2- or 3-loaded-conductor wire table; earth from NBR 5410 table 58 | Utility standard (see below) |
| Phase balancing | largest circuit first, each on the least loaded phase (two-pole circuits on the least loaded pair) | — |

The regional utility profiles (`src/domain/utility_profiles.rs`: Generic NBR 5410, Enel SP LIG-BT 12ª ed., CPFL GED-13, CEMIG ND 5.1, COPEL NTC 901100) use explicit per-utility residential demand tables and service-entrance thresholds (no hidden scale multipliers) and carry a `UtilityStandardCitation` (standard, edition, tables and local-bulletin verification note) surfaced in the UI, single-line diagram and MCP `quadro_de_cargas` (B21). Because utilities publish periodic technical bulletins (*comunicados técnicos*), designers should still confirm the active local revision before submitting a connection request.

## Construction quantities

`construction_takeoff` (`src/domain/quantities.rs`) measures the active floor:

| Quantity | Rule |
| --- | --- |
| Net wall area | axis length × the wall's mean height (the storey height unless the wall has its own top, #145; a sloped top averages its two ends), less each opening's width × height, up to the top over it |
| Blocks per m² | 1 / ((block length + joint) × (block height + joint)), each wall with its own masonry |
| Half blocks | one every other course at each wall end (each end at its own height) and at each jamb (courses = height / (block height + joint), rounded) |
| U-blocks | the top course of every wall (bond beam), along the slope of a sloped top, replacing its blocks, plus the lintels and sills |
| Whole blocks | net area × blocks per m² − half blocks / 2 − top-course U-blocks; waste from *Quadros* (10 % by default) |
| Lintels and sills | a lintel over each door and window and a sill under each window, width + 2 × 30 cm |
| Laying mortar | net area × (1 − blocks per m² × block length × block height) × block width, + 10 % |
| Plaster | both faces of the net area, at the masonry's plaster thickness, + 10 % on the volume |
| Finishes | per room: floor + 10 %, skirting less door widths, net walls and ceiling (`room_finish_quantities`) |

Checked by `the_takeoff_matches_a_manual_takeoff_within_2_percent`. Axis lengths count each corner twice, which the waste allowance covers.

## Applicability limits

Each method holds only within a range of buildings (`src/domain/limits.rs`). Outside it Trazo refuses to size, says why and points to a professional, rather than showing numbers that look trustworthy and are not.

| Limit | Value | Scope | Why |
| --- | --- | --- | --- |
| Storeys carried | at most 3 (real floors with structure, or the *floors* setting) | whole structure | The column rule ignores slenderness and frame action, and wind becomes significant |
| Storey height | at most 4.0 m | whole structure | Columns are sized without a buckling check |
| Live load | at most 5.0 kN/m² | whole structure | Beyond it the use is not residential or office (storage, industry) |
| Beam span | at most 7.0 m | that beam span | The L/10–L/12 height rule and simple-support analysis lose accuracy on long spans |
| Smaller slab span | at most 6.5 m | that slab | The thickness tables end there |
| Soil for footings | at least 100 kPa | each footing | Soft soils need a soil survey and possibly deep foundations or a raft |
| Electrical demand | at most 75 kVA | service entrance | Above it the utility requires a dedicated transformer |

These values are conservative choices for houses and small buildings, not values from a standard. The traffic light ([#101](../ROADMAP.md#101-traffic-light-results-with-margins-and-explanations--done)) shows a refused element in red as *Fora dos limites*, with no sizing numbers.

## Validation suite

The suite lives in `src/domain/validation.rs` and runs with `cargo test` on Windows, Linux and macOS in CI. Each test reproduces a published result.

| Test | Reference | Expected | Tolerance |
| --- | --- | --- | --- |
| `aisc_case_1_simple_beam_uniform_load` | AISC Manual table 3-23, case 1 | M = wL²/8; Δ = 5wL⁴/384EI | 0.1 % |
| `aisc_case_7_simple_beam_central_point_load` | AISC table 3-23, case 7 | Δ = PL³/48EI | 0.1 % |
| `aisc_case_29_two_equal_spans_uniform_load` | AISC table 3-23, case 29 | M_B = −wL²/8; R = 3/8, 10/8 wL; M+ = 9wL²/128; Δ = wL⁴/185EI | exact; Δ 0.5 % |
| `three_equal_spans_uniform_load_coefficients` | Standard coefficients for three equal spans (beam tables in the AISC Manual and in Hibbeler, *Structural Analysis*) | M_B = −0.100wL²; R = 0.400, 1.100 wL; M+ = 0.080 and 0.025 wL² | exact |
| `aisc_case_31_two_equal_spans_central_point_loads` | AISC table 3-23, case 31 | M_B = −3PL/16; R_A = 5P/16; R_B = 11P/8 | exact |
| `yield_lines_give_the_classic_tributary_areas` | 45° yield lines (NBR 6118:2023, 14.7.6.1) | square: ¼ per edge; 6 × 4 m: ⅓ per long edge, ⅙ per short edge | 1e-6 |
| `branson_inertia_follows_nbr_6118_17_3_2_1_1` | NBR 6118:2023, 17.3.2.1.1 | Mr; I_eq = Ic below Mr and Branson's interpolation above | 1e-9 |
| `eccentric_footing_pressure_follows_the_middle_third_rule` | Middle-third rule | σmax = N/A·(1 + 6e/B); FS = B/2e | 1e-9 |
| `punching_at_the_column_face_follows_nbr_6118_19_5_3_1` | NBR 6118:2023, 19.5.3.1 | τSd/τRd2 | 1e-9 |
| `bar_geometry_and_mass_match_the_physical_section` | Circular bar geometry and 7,850 kg/m³ steel density | Ø10 area and Ø10/Ø12.5 linear masses | 0.3 % |
| `flexural_steel_matches_the_existing_branson_input` | Same rectangular ULS model used for cracked-section stiffness | beam `As` agrees between both calculation paths | 1 % |
| `generated_house_has_reinforcement_and_a_balanced_schedule` | Internal equilibrium and minimum/detail selection rules | all four element families emitted; supplied steel ≥ required; schedule mass closes | exact |
| `nbr_6118_bond_anchorage_and_lap_lengths_match_hand_calculation` | NBR 6118:2023 §9.3–9.5 (C25, CA-50 Ø10) | `fbd` good/poor (`η2 = 0.7`), `lb`, hooked `lb,nec` (`α1 = 0.7`), lap `l0` | 0.002 MPa / 2 mm |
| `diameter_summary_reconciles_with_schedule_and_commercial_bars` | CA-50 diameter summary & commercial 12 m bar rounding (+10% waste) | summary mass closes with schedule; `bars_12m = ceil(1.10·L/12)` | exact |
| `reinforcement_detail_svg_is_valid_svg_with_all_sections` | A3 landscape schematic detailing sheet | valid SVG (`usvg`) with beam/column/footing/slab panels, Branson B12 note and CA-50 table | exact |
| `concentrated_load_flows_from_slab_to_beams_columns_and_footings` | Equilibrium and the declared local slab model | added column reactions equal the point load; beam moment, local moment and punching are present; unsupported case is red | 1e-6 kN |
| `a_load_directly_on_a_beam_skips_the_slab_local_check` | Direct footprint/beam intersection | point load enters the beam without a fictitious slab punching check | exact |
| `loads_are_conserved_from_the_slabs_to_the_columns` | Equilibrium | Σ column loads = slabs + beams + columns' own weight | 0.5 % |
| `loads_are_conserved_across_floors` | Equilibrium | ground column = its own floor's load + the column above | 1e-9 |

Further checks live next to their modules: `nbr_5410_minimums_for_lighting_and_outlets`, `circuits_are_sized_with_breaker_then_wire`, `demand_factors_follow_the_usual_tables` (which includes a voltage-drop example) and `conduits_are_sized_by_the_fill_limits`.

## Known limitations

- Beams are analysed on pinned supports; frame action with the columns arrives with #94.
- Columns are sized for axial load with a position factor; no slenderness or biaxial bending check.
- Solid-slab tables assume four simply supported edges. Edge fixity, openings, torsion reinforcement and compatibility between adjacent panels still require the licensed engineer's model; lattice slabs require manufacturer data.
- Wind, earthquake and load combinations beyond the simplified ones are not considered (#105).
- Regional values (utility profiles, soil classes, demand tables) are typical, not site-specific (B21, #140).
- The applicability limits are conservative choices, not values from a standard; a professional may well design beyond them with other methods.
- Reinforcement is a preliminary review estimate traced to NBR 6118:2023, not an executive fabrication drawing: openings, construction joints, bend deductions, congestion in bar intersections and any NBR 6118:2026 clause-by-clause audit still require a licensed engineer (ART/RRT) (#110).
- Concentrated loads use a conservative nearest-edge slab distribution; dynamic equipment, pedestals and detailed plate behaviour still require professional analysis (#113).
