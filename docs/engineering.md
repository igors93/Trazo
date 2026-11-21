# Engineering methods

This guide documents every engineering calculation in Trazo: the formula, the standard it follows, the assumptions and the limits, and the test that checks it. It is the reference behind the [safety contract](../ROADMAP.md#engineering-in-trazo-the-safety-contract): if a result cannot be traced to a section here, it should not be shown as engineering.

> **Pre-sizing, not design.** Every result here is a first estimate to guide decisions and the conversation with a licensed professional. Building requires a professional's responsibility (ART or RRT) and the municipal permits.

- [Conventions](#conventions)
- [Loads](#loads)
- [Slabs](#slabs)
- [Beams](#beams)
- [Columns](#columns)
- [Load path across floors](#load-path-across-floors)
- [Footings](#footings)
- [Electrical installation](#electrical-installation)
- [Validation suite](#validation-suite)
- [Known limitations](#known-limitations)

## Conventions

| Item | Convention |
| --- | --- |
| Units | Metres, kilonewtons (kN), kN·m, kN/m², kPa (= kN/m²), MPa for material strengths, volt-amperes (VA) |
| Moments | Sagging positive: hogging (support) moments are negative |
| Loads | Characteristic values unless stated; ultimate design uses γf = 1.4 (NBR 8681 normal combination, simplified) |
| Quasi-permanent load | g + ψ2·q with ψ2 = 0.3 (dwellings, NBR 6118:2023, table 11.2) |
| Code | `src/domain/structure.rs`, `src/domain/electrical.rs`, `src/domain/distribution.rs`, `src/domain/conduits.rs` |

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

**Assumptions:** slabs simply supported on beams; no two-way moment or deflection check yet (#116).

## Beams

**Analysis** (`continuous_beam_with_point_loads`): each run of collinear beams through consecutive supports is a continuous beam on pinned supports, solved by the slope-deflection (stiffness) method:

- unknowns: the rotation at each support;
- element stiffness: 4EI/L and 2EI/L;
- fixed-end moments: −wL²/12 and +wL²/12 for uniform loads, and −Pab²/L² and +Pa²b/L² for point loads.

From the end moments come the shears, the reactions (passed to the columns, or as point loads to a supporting beam) and the largest span moment.

**Sizing rules:**

| Check | Rule | Source |
| --- | --- | --- |
| Height | h ≥ L/10 in end spans and single spans, L/12 in interior spans of continuous beams; at least 25 cm | Usual pre-sizing rule |
| Width | b ≥ 12 cm | NBR 6118:2023, 13.2.2 |
| Deflection | long-term deflection ≤ L/250 | NBR 6118:2023, 13.3, table 13.3 |

**Deflection** (`effective_beam_stiffness`, `span_deflection_with_point_loads`):

1. The quasi-permanent load g + 0.3q is applied, with the end moments scaled in the same ratio.
2. The equivalent inertia is Branson's formula, NBR 6118:2023, 17.3.2.1.1: I_eq = (Mr/Ma)³·Ic + [1 − (Mr/Ma)³]·I_II ≤ Ic, with:
   - the cracking moment Mr = α·fct·Ic / yt, where α = 1.5 for rectangles and fct = 0.3·fck^(2/3) MPa;
   - I_II, the cracked transformed inertia, with Es = 210 GPa and the tension steel pre-sized for 1.4·Mk (CA-50, fyd = 500/1.15 MPa, never below ρmin from table 17.3).
3. The modulus is Ecs = 0.85 · 5600 · √fck MPa. This is the secant modulus of NBR 6118:2003; for fck = 25 MPa and granite aggregate it is within 2 % of the αi·Eci of NBR 6118:2023, 8.2.8.
4. The curvature M/EI is integrated twice numerically (100 steps). The long-term value multiplies the immediate deflection by 1 + αf = 2.32 (αf = 1.32 for loading at about one month, no compression steel, 17.3.2.1.2).

**Assumptions:** pinned supports (column stiffness ignored; see #94); no torsion; no shear design; reinforcement is only estimated for the stiffness.

## Columns

**Load:**

- the sum of the beam reactions on the column (upward reactions ignored, with a warning);
- plus the column's own weight, 25 · b · h · H;
- plus the load of the column standing on it (see [floors](#load-path-across-floors)).

Without real upper floors, the floor's load is multiplied by the *floors* setting instead.

**Required concrete area:** Ac = 1.4 · α · N / (0.85·fcd + ρ·σs), with:

- fcd = fck / 1.4;
- ρ = 1.5 % steel at σs = 420 MPa (CA-50 at 2 ‰ strain);
- α, the usual pre-sizing factor for the bending a column takes at its position: 1.3 for an interior column, 1.5 at an edge, 1.8 at a corner or an isolated column.

The position comes from the beams reaching the column: four directions make it interior, three or two opposite ones an edge, two at an angle or one a corner.

**Minimums:** 360 cm² and 14 cm a side (NBR 6118:2023, 13.2.3).

**Assumptions:** axial load with the α factor standing in for bending; no slenderness, buckling or frame moments (#94).

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
| Height | h = 2/3 of the largest overhang, at least 25 cm (rigid footing) | NBR 6118:2023, 22.6.1 (rigidity) |
| Eccentric pressure | while e ≤ B/6: σmax = N/A · (1 + 6ex/Bx + 6ey/By); beyond, an effective area (Bx − 2ex)(By − 2ey) | Classic soil mechanics (middle-third rule), NBR 6122:2019 |
| Overturning | FS = 1 / (2·(ex/Bx + ey/By)) ≥ 1.5 | NBR 6122:2019 |
| Punching at the column face | τSd = 1.4·N / (u0·d) ≤ τRd2 = 0.27·αv2·fcd, αv2 = 1 − fck/250 | NBR 6118:2023, 19.5.3.1 |
| Punching at d/2 | τSd ≤ τRd1 = 0.13·(1 + √(20/d))·(100·ρ·fck)^(1/3), with ρ = 0.2 % | NBR 6118:2023, 19.5.3.2 (simplified) |

Allowable soil pressures by class (`SoilClass`): soft clay 80 kPa, medium clay 150 kPa, medium sand 200 kPa, dense sand 300 kPa, weathered rock 500 kPa. These are typical values for a first estimate; a soil survey (SPT) and a geotechnical engineer set the real value.

## Electrical installation

| Calculation | Rule | Source |
| --- | --- | --- |
| Lighting load | 100 VA for the first 6 m², plus 60 VA per whole 4 m² beyond | NBR 5410:2004, 9.5.2.1.2 |
| Outlets per room | 1 per 3.5 m of perimeter in kitchens and service areas, 1 per 5 m elsewhere, at least 1 in bathrooms and balconies | NBR 5410:2004, 9.5.2.2.1 |
| Outlet power | 600 VA for the first three in wet rooms, 100 VA otherwise | NBR 5410:2004, 9.5.2.2.2 |
| Breaker | smallest standard rating ≥ circuit current | — |
| Wire | smallest section whose capacity (copper, PVC 70 °C, method B1, two loaded conductors, 30 °C) ≥ breaker; at least 1.5 mm² for lighting and 2.5 mm² for outlets | NBR 5410:2004, table 36 and 6.2.6.1.1 |
| Voltage drop | ΔV% = 2 · ρ · Σ(L·I) / (S · V) · 100, ρ = 1/56 Ω·mm²/m; wire raised until ≤ 4 % | NBR 5410:2004, 6.2.7.1 |
| RCD | 30 mA on circuits of wet and outdoor areas | NBR 5410:2004, 5.1.3.2.2 |
| Conduit fill | ≤ 53 % for one conductor, 31 % for two, 40 % for three or more | NBR 5410:2004, 6.2.11.1.6 |
| Demand | lighting and general outlets by the table of their installed kVA (0.86 up to 1 kVA … 0.24 above 10 kVA); specific-use circuits by their count (1.00 for one or two … 0.38 from 24) | Usual utility tables for dwellings |
| Supply and entrance | single/two/three-phase from the demand; main breaker from the demand current on the most loaded phase; feeder from the wire table; earth from NBR 5410 table 58 | Utility standard (see below) |
| Phase balancing | largest circuit first, each on the least loaded phase (two-pole circuits on the least loaded pair) | — |

The regional utility profiles (`UtilityProfile`: generic, Enel SP, CPFL, CEMIG, COPEL) change the supply thresholds, the minimum main breaker and feeder, and scale the demand. Their values have **not yet been checked against the published utility standards** (bug [B21](../ROADMAP.md#b21-regional-utility-profiles-are-not-traced-to-their-standards)); until they are, treat them as placeholders and check the local standard.

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
| `loads_are_conserved_from_the_slabs_to_the_columns` | Equilibrium | Σ column loads = slabs + beams + columns' own weight | 0.5 % |
| `loads_are_conserved_across_floors` | Equilibrium | ground column = its own floor's load + the column above | 1e-9 |

Further checks live next to their modules: `nbr_5410_minimums_for_lighting_and_outlets`, `circuits_are_sized_with_breaker_then_wire`, `demand_factors_follow_the_usual_tables` (which includes a voltage-drop example) and `conduits_are_sized_by_the_fill_limits`.

## Known limitations

- Beams are analysed on pinned supports; frame action with the columns arrives with #94.
- Columns are sized for axial load with a position factor; no slenderness or biaxial bending check.
- Slabs have no moment or deflection check of their own (#116).
- Wind, earthquake and load combinations beyond the simplified ones are not considered (#105).
- Regional values (utility profiles, soil classes, demand tables) are typical, not site-specific (B21, #140).
- Applicability limits (#102) and traffic-light margins (#101) are being added; until then, warnings are the only signal.
