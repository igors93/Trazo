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
| Code | `src/domain/structure.rs`, `src/domain/reinforcement.rs`, `src/domain/electrical.rs`, `src/domain/distribution.rs`, `src/domain/conduits.rs` |

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

## Concentrated loads

Water tanks and heavy equipment are stored as structural fixtures with a footprint and a positive characteristic load. The water-tank default is 10.81 kN: 9.81 kN for 1,000 L of water plus 1.0 kN for the tank and fittings. The 15 kN equipment default is only an editable placeholder and must be replaced with manufacturer data. The load is treated as permanent in the quasi-permanent service combination.

**Load path.** If the footprint intersects a beam axis, the full load becomes a point load at that position on the beam. Otherwise Trazo finds the containing slab and sends the load to the nearest supported edge beam(s); beams at practically the same distance (within 5 cm) share it equally. The beam stiffness analysis then passes the reactions to columns and footings. A load outside every slab and beam is red and contributes nothing until it is repositioned or given its own support.

**Local slab model.** For a load on a slab, the conservative additional one-metre-strip moment is `Mk,local = P·lx/(4·beff)`, with `beff = min(ly, max(0.50 m, smaller footprint side + 2h))`. That moment is added to the slab flexural reinforcement demand. For a solid slab, punching is checked around the loaded footprint at its face and at the `d/2` contour using the same `τRd2` and simplified `τRd1` expressions documented for footings. For a lattice/ribbed slab, Trazo does not know the joist and filler geometry, so it deliberately leaves punching unverified and asks for a beam, solid strip or manufacturer check.

**Limitations:** the nearest-edge split is a transparent preliminary load path, not plate finite-element analysis; load dispersion through pedestals, vibration/impact and dynamic equipment forces are not modelled. Equipment input must already include those effects when applicable.

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

**Assumptions:** pinned supports (column stiffness ignored; see #94); no torsion. Reinforcement and shear are estimated as described below, but the analysis still has no frame action or torsion.

## Preliminary reinforcement

`build_reinforcement_report` turns the characteristic forces from `structure_report` into a reviewable reinforcement proposal and a theoretical bar schedule. It uses CA-50 (`fyk = 500 MPa`), steel density 7,850 kg/m³ and the same structural applicability limits. If the whole project or an element is outside those limits, it refuses to emit reinforcement for it.

**Beams.** For the span and support moments, `Md = 1.4·|Mk|`. A singly reinforced rectangular section uses the 0.85 concrete stress block, `fcd = fck/1.4`, `fyd = 500/1.15 MPa`, effective depth `d = h − 4 cm` and lever arm `z = d·(1 − 0.4ξ)`. The required area is `As = Md/(z·fyd)`, never below the minimum ratio already used by the cracked-section calculation. The selector tries 10, 12.5, 16, 20 and 25 mm bars and checks whether one layer fits with cover, stirrup and clear spacing of at least the larger of 20 mm or one diameter.

Stirrups compare `Vsd = 1.4·Vk` with `Vc = 0.60·fctd·bw·d`; the remaining shear gives `Asw/s = (Vsd − Vc)/(0.9·d·fyd)`, never below the minimum transverse ratio. Spacing is also capped as a function of effective depth and shear level. If diagonal compression is exceeded, the result warns that adding stirrups is not a valid fix and asks for a larger section.

**Columns.** Longitudinal steel is the larger of 0.4% of the concrete area and the remainder needed after the concrete compression contribution in the existing axial pre-sizing equation. The selector uses an even number of bars, at least four. Tie spacing is capped by 20 cm, 12 longitudinal diameters and the smaller section dimension. This is still an axial estimate: slenderness, second-order effects and biaxial bending remain for #94.

**Slabs.** A one-metre strip is treated as simply supported in the smaller direction with `Mk = q·lx²/8`. Principal reinforcement follows the same rectangular flexural calculation; distribution steel is at least 20% of the main requirement and 0.90 cm²/m. For lattice slabs, this is only an equivalent steel estimate and the selected joist/trellis must be confirmed with its manufacturer.

**Footings.** Each direction is treated as a cantilever from the column face, with `Mk = σmax·a²/2` per metre and 7 cm nominal cover. It produces the bottom X/Y meshes only after the soil-pressure, overturning and punching checks have been run.

**Schedule and limits.** The schedule lists mark, element, position, diameter, quantity, unit length, total length and theoretical mass. Beam/column lengths include simple preliminary allowances (generally 40 diameters) but not complete anchorage checks, hooks, bend deductions, lap zoning, curtailment, construction joints, openings, detailing congestion or cutting waste. It is therefore a takeoff aid, not a bending schedule for fabrication.

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

## Construction quantities

`construction_takeoff` (`src/domain/quantities.rs`) measures the active floor:

| Quantity | Rule |
| --- | --- |
| Net wall area | axis length × storey height, less each opening's width × height |
| Blocks per m² | 1 / ((block length + joint) × (block height + joint)), each wall with its own masonry |
| Half blocks | one every other course at each wall end and at each jamb (courses = height / (block height + joint), rounded) |
| U-blocks | the top course of every wall (bond beam), replacing its blocks, plus the lintels and sills |
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
| `concentrated_load_flows_from_slab_to_beams_columns_and_footings` | Equilibrium and the declared local slab model | added column reactions equal the point load; beam moment, local moment and punching are present; unsupported case is red | 1e-6 kN |
| `a_load_directly_on_a_beam_skips_the_slab_local_check` | Direct footprint/beam intersection | point load enters the beam without a fictitious slab punching check | exact |
| `loads_are_conserved_from_the_slabs_to_the_columns` | Equilibrium | Σ column loads = slabs + beams + columns' own weight | 0.5 % |
| `loads_are_conserved_across_floors` | Equilibrium | ground column = its own floor's load + the column above | 1e-9 |

Further checks live next to their modules: `nbr_5410_minimums_for_lighting_and_outlets`, `circuits_are_sized_with_breaker_then_wire`, `demand_factors_follow_the_usual_tables` (which includes a voltage-drop example) and `conduits_are_sized_by_the_fill_limits`.

## Known limitations

- Beams are analysed on pinned supports; frame action with the columns arrives with #94.
- Columns are sized for axial load with a position factor; no slenderness or biaxial bending check.
- Slabs have no moment or deflection check of their own (#116).
- Wind, earthquake and load combinations beyond the simplified ones are not considered (#105).
- Regional values (utility profiles, soil classes, demand tables) are typical, not site-specific (B21, #140).
- The applicability limits are conservative choices, not values from a standard; a professional may well design beyond them with other methods.
- Reinforcement is a preliminary estimate, not an executive detail: anchorage, laps, bends, openings, joints, cutting losses and bar drawings are incomplete, and NBR 6118:2026 still requires a clause-by-clause audit (#110).
- Concentrated loads use a conservative nearest-edge slab distribution; dynamic equipment, pedestals and detailed plate behaviour still require professional analysis (#113).
