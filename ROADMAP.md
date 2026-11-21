# Trazo Roadmap

*Last updated 2026-11-21, after the electrical design, the structure and the discipline views. This update adds the engineering and simulation track and a bug assessment.*

This is the plan for what comes next. Work through it **milestone by milestone, in order**: each milestone builds on the previous one and ends with a concrete test of "done". [Next up](#next-up) lists the first ten items on that path, and [Bug assessment](#bug-assessment) lists what to fix along the way. Everything already shipped is in the [changelog](CHANGELOG.md).

- [Vision](#vision)
- [Engineering in Trazo: the safety contract](#engineering-in-trazo-the-safety-contract)
- [Where Trazo stands](#where-trazo-stands)
- [How to read this roadmap](#how-to-read-this-roadmap)
- [Bug assessment](#bug-assessment)
- [Next up](#next-up)
- [Milestones](#milestones)
- [Backlog](#backlog)
- [Definition of done](#definition-of-done)
- [Out of scope](#out-of-scope)

## Vision

> From the first sketch to a safe building site: a 2D CAD that draws the house, designs its installations and structure, simulates how they behave, lists the materials and plans the work, so that someone who knows nothing about engineering can build with a minimum of safety, and a professional can do it faster.

Trazo is not the engineer, and never pretends to be. It is the tool that lets both ends of the spectrum work on the same project and file:

- **Newcomers** draw a whole house without learning CAD, see in plain language what each wall, beam, wire and pipe needs, run simulations with one click, and arrive at the professional with a well-prepared project instead of a blank page.
- **Professionals** get exact drawings, fast pre-sizing of every discipline, simulations they can inspect and adjust, quantities and a calculation report to review and sign.

### Principles

1. **Architecture before command count.** Moving a wall and seeing rooms, doors, dimensions, beams and conduits follow is worth more than a hundred generic commands.
2. **Built for the construction site.** Drawings leave Trazo ready for the site: dimensions in centimetres, located openings, modular masonry, quantities, printing to scale.
3. **Screen equals paper.** What you see is what you export; rules shared by both live in the domain.
4. **One model, every discipline.** Architecture, electrical, plumbing and structure are views of the same project, and a change in one shows up in the others.
5. **Safety you can see.** Every engineering number shows where it came from, how close it is to its limit and what to do when it fails (see the [safety contract](#engineering-in-trazo-the-safety-contract)).
6. **AI as a collaborator.** Anything a person can do, an assistant can do through MCP, visibly, reversibly and with the person in control.
7. **Ease is a feature.** Every new capability, including safety measures, should add as few steps as possible.

## Engineering in Trazo: the safety contract

Engineering features follow these rules. They apply to everything in milestones M3 to M9, and an item that breaks them is not done.

1. **Pre-sizing, not design.** Results guide decisions and the conversation with a professional. Building still requires a licensed professional's responsibility (ART or RRT) and the municipal permits; Trazo says so wherever a result could be mistaken for a final design.
2. **Show the work.** Every number can be opened to see its formula, the standard it follows (NBR 6118, 5410, 5626, 8160…), the inputs and the assumptions.
3. **Traffic light with margins.** Green has a comfortable margin, yellow passes with a small one, red fails or lies outside the method's limits. Red is never hidden, rounded away or silenced.
4. **Known limits.** Each method declares where it applies (storeys, spans, loads, soils). Outside them Trazo refuses to size and explains why, instead of guessing.
5. **Verified.** Each method is checked in CI against published worked examples and, for analyses, against a reference program.
6. **Conservative when unsure.** Missing data takes the safer default, and the result says that it did.
7. **Two depths of explanation.** A plain-language summary for newcomers ("this beam is too shallow for its span: make it 40 cm tall") and the full detail for professionals.

## Where Trazo stands

| Area | Status |
| --- | --- |
| Drawing engine (#01–#10) | ✅ Complete: `f64` geometry, free-shape rooms, snaps, typed input, CAD modify tools, constraints, multiple selection |
| Architecture (#11–#25) | ✅ Complete: junctions, shared walls, multilayer walls, door and window types, stairs, floors, library, finishes, circulation checks |
| Documentation (#26–#37) | ✅ Complete: associative dimensions, layers, sheets, scales, title blocks, hatches, schedules, sections, sheet sets, ABNT styles |
| Exchange (#38–#45, #70) | ✅ Complete: DXF and DWG, SVG and CSV, print to scale, image and PDF underlays, blocks with attributes, external references, version comparison |
| Release (#58–#64, #77–#81) | 🟡 CI, licence, property tests, migrations, release binaries, crash reports, Windows installer and samples done; schema, signing and CI on Windows and macOS remain |
| Construction site (#53–#57) | 🟡 Building blocks exist (first course per wall, block estimate, sheets); the site deliverables do not |
| Electrical design (#88–#91) | ✅ Devices, NBR 5410 forecast, circuits, conduits with wiring, phase balancing, voltage drop, demand, service entrance, single-line diagram |
| Structure (#92–#94) | ✅ Columns, beams, slabs and footings on a formwork plan, pre-sizing with warnings · 🟡 analysis covers continuous beams only |
| Discipline views (#96) | 🟡 Architecture, electrical and structure on screen and per sheet; plumbing joins with #72 |
| Plumbing (#72) | ⬜ Not started |
| Simulation, materials and planning | ⬜ Beam analysis is the only simulation so far; no material catalogue, costs or schedule |
| AI (#46–#52, #74–#76) | 🟡 43 MCP tools, real-time sync, tunnel token; no approval or read-only mode |
| Experience (#65–#69) | 🟡 Tabbed documents (#95) shipped; levels, guided flows and English interface not started |

**Shipped since the last update:** #38, #39, #40, #43, #44, #45, #63, #70, #77, #79, #81, #88, #89, #90, #91, #92, #93 and #95. They have left the tables below.

## How to read this roadmap

| Field | Meaning |
| --- | --- |
| **#** | Item number. Items keep their number for life; new items continue from #97. Use it in commits and pull requests: `feat: add lintels per opening (#111)`. |
| **Priority** | **P0** required for its milestone · **P1** important · **P2** valuable, can slip |
| **Size** | Rough effort for one developer: **S** ≤ 2 days · **M** ≤ 1 week · **L** ≤ 3 weeks · **XL** more than 3 weeks |
| **Status** | ⬜ not started · 🟡 partial (the note says what exists) · 🔵 in progress · ✅ done |
| **B##** | A known bug or risk from the [bug assessment](#bug-assessment). |

When an item ships, mark it ✅ here, add it to the changelog, and remove it from this file at the next update.

## Bug assessment

Reviewed on 2026-11-21. Sources:

- a static scan of everything outside tests: 20 `unwrap`, 4 `expect` and 2 `unreachable!`, all reviewed and guarded by earlier checks, so no action is needed;
- the CI logs;
- exports of the sample projects inspected page by page;
- a review of the engineering modules added in #88–#96.

**Severity:**

- **High:** loses data, gives an unsafe engineering result or breaks the build.
- **Medium:** gives a wrong or misleading result in some cases, or slows the program down.
- **Low:** cosmetic, or an inconvenience.

| ID | Severity | Area | Problem | Status / fix |
| --- | --- | --- | --- | --- |
| B01 | High | Floors | Switching floors left conduits, beams and slabs behind, and the pruning on the next edit deleted those of the other floor. | ✅ Fixed in this update: they travel with their floor; regression test `each_floor_keeps_its_beams_slabs_and_conduits`. |
| B02 | High | CI | CI runs only on Linux, so code behind `cfg(windows)` or `cfg(macos)` is only checked by the release workflow. Windows-only print helpers already broke the Linux clippy run once. | ✅ Fixed: CI workflow runs format, Clippy and tests across `ubuntu-latest`, `windows-latest` and `macos-latest` (#97). |
| B03 | High | Structure | Each floor's structure is sized on its own, and the load of the floors above comes from a *floors* multiplier, not from the real upper floors. Columns that do not line up between floors are not detected. | Open → #99 |
| B04 | Medium | Performance | Conduit marks, structure marks, the load schedule and device tags are recomputed on every frame. The cost grows with the square of the device and beam count, so large plans will stutter. | Open → #108, #62 |
| B05 | Medium | Drawings | Conduit, beam, column and footing labels ignore the label layout, so they overlap at crowded wall junctions (seen on the sample house). | Open → #106 |
| B06 | Medium | Structure | Slab spans come from the room's bounding box, which is wrong for L-shaped rooms. Slab loads are shared by edge length, a rough stand-in for the yield-line method. | Open → #116, #109 |
| B07 | Medium | Structure | Columns turn in quarter turns only, so they come out misoriented on angled walls. Beams can only rest on columns, not on other beams. | Open → #117 |
| B08 | Medium | Electrical | Two-way switches are wired in pairs only; intermediate (four-way) switches are counted as one-way, giving wrong wire counts. | ✅ Fixed: added `FixtureKind::SwitchIntermediate` (four-way) and generalized multi-way switch chains in `conduit_wiring`; regression test `intermediate_switches_carry_four_travellers_and_chain_multiple_points`. |
| B09 | Medium | Electrical | Only the first board is used, so sub-boards (QDC) are not modelled. | ✅ Fixed: `ConduitTree`, `generate_conduits`, `cable_length`, `LoadSchedule` (`SubBoardSummary`) and `single_line_svg` now model secondary boards (`QDC1`, …) with dedicated feeders and local terminal lengths; regression test `sub_boards_are_modelled_with_dedicated_feeders_and_shortened_terminal_runs`. |
| B10 | Medium | Electrical | Devices are assigned to rooms by their centre. A device right on a room boundary can land in the neighbouring room, which changes its circuit and RCD. | ✅ Fixed: `Project::room_for_fixture` resolves boundary devices by the room their front face points into (with footprint-overlap fallback); regression test `boundary_device_is_assigned_to_the_room_it_faces`. |
| B11 | Medium | Structure | Footings are checked by area only: no eccentric load, punching or overturning check. | Open → #112 |
| B12 | Medium | Structure | Deflection uses half the gross inertia and no reinforcement, so it can be off in either direction. | Open → #110, #100 |
| B13 | Medium | Data | Demand factors, entrance tables, live loads and soil pressures are generic Brazilian values; some utilities and regions differ. | Open → #140 |
| B14 | Low | DXF | Floor conduits export as solid lines, and structure labels have a fixed text height. | ✅ Fixed: floor conduits export with `DASHED` linetype (group 6) and structure labels scale their text height with the sheet scale; regression test `floor_conduits_use_dashed_linetype_and_structure_labels_scale_with_sheet`. |
| B15 | Low | Views | In the side-by-side view, the inactive pane ignores its tab's discipline. | ✅ Fixed: `side_plan` filters the inactive tab's project through `for_discipline`, renders its focus overlay and shows its discipline in the view plate; regression test `inactive_split_pane_respects_its_tab_discipline`. |
| B16 | Low | Sheets | A sheet exported at a scale that does not fit crops the plan. The sheet panel warns, but the exported file does not. | ✅ Fixed: exported sheets stamp a visible crop warning banner (`#aviso-recorte`) with a suggested scale when the plan exceeds the drawing area; regression test `cropped_sheet_includes_warning_banner_and_fitting_sheet_omits_it`. |
| B17 | Low | Performance | System fonts are loaded on every SVG rasterisation (print preview, single-line diagram, beam diagrams), causing a hitch after each change. | ✅ Fixed: shared `OnceLock` font databases reused across `resvg` rasterisation and `svg2pdf` export; regression test `shared_font_databases_are_reused_across_calls`. |
| B18 | Low | Repository | No `.gitattributes`; with `core.autocrlf` the working tree mixes LF and CRLF, which makes diffs noisy. | ✅ Fixed: added `.gitattributes` enforcing LF for source/docs and CRLF for Windows scripts (#98). |
| B19 | Low | Fixtures | The minimum fixture size dropped from 20 to 10 cm for every kind so that 14 cm columns fit; it should be a per-kind minimum. | ✅ Fixed: `FixtureKind::min_size` enforces 10 cm for columns and 20 cm for all other fixtures; regression test `minimum_size_is_per_kind`. |

## Next up

The first ten items on the path, in order. The other items of each milestone fit in around them.

| Order | # | Item | Milestone | Why now |
| --- | --- | --- | --- | --- |
| 1 | #97 | CI on Windows and macOS | M1 | The build must not break on the platforms people actually use (B02). |
| 2 | #99 | Load path across floors | M3 | Two-storey houses are the common case, and today their columns are under-informed (B03). |
| 3 | #100 | Engineering methods guide and validation suite | M3 | Nothing else in the engineering track is trustworthy until the methods are written down and tested against published examples. |
| 4 | #101 | Traffic-light results with margins and explanations | M3 | The core of the safety contract, and what newcomers will read first. |
| 5 | #102 | Applicability limits per module | M3 | Refusing outside the limits is safer than any warning. |
| 6 | #103 | Pre-sizing report for professional review | M3 | Turns Trazo's results into something an engineer can check and sign. |
| 7 | #56 | Construction quantities | M2 | Turns the drawing into a shopping list, and feeds the material catalogue. |
| 8 | #53 | Opening dimension chains | M2 | The construction-site deliverable builders use most. |
| 9 | #113 | Point loads: water tank and heavy equipment | M5 | A water tank on a slab is the most common unsafe decision in self-built houses. |
| 10 | #94 | 2D frame analysis | M4 | The base of every structural simulation that follows. |

## Milestones

### M1: Ready to share (finishing)

- **Items:** #80, #78, #97, #98
- **Done when:** CI passes on Windows, Linux and macOS; the project file has a published JSON Schema; Windows binaries are signed.

### M2: The construction-site set

The differentiator: everything a builder needs comes straight from the plan.

- **Items:** #53, #54, #55, #56, #57, #71
- **Done when:** from the sample house, one click produces an A3 1:50 set with a dimensioned plan that locates every opening, a first-course plan with lintels, a setting-out plan and a quantity list. The quantities (blocks with waste, half blocks, U-blocks, lintels, mortar, plaster, flooring) match a manual takeoff within 2 %.

### M3: Engineering you can trust

Before more simulations: make every engineering result explainable, limited, verified and reviewable.

- **Items:** #99, #100, #101, #102, #103, #104
- **Done when:**
  - every structural and electrical module passes its published worked examples in CI;
  - on the two-storey sample, the upper floor's loads reach the ground-floor columns, and a misaligned column is flagged;
  - a 9 m span is refused with an explanation;
  - every result shows its traffic light, margin and assumptions;
  - the pre-sizing report exports to PDF;
  - a sheet set with red warnings cannot be released without a named reviewer.

### M4: Simulation core

One analysis engine for every structural simulation, with results you can see and play with.

- **Items:** #94, #105, #106, #107, #108
- **Done when:**
  - the frame analysis of the sample house matches a reference program (such as Ftool) within 2 % for moments and reactions;
  - the diagrams, deformed shape and utilisation colours show on the plan and on paper;
  - moving a what-if slider updates the results in under 100 ms on the sample;
  - large plans stay responsive while the analysis runs.

### M5: Structure from foundation to roof

Everything a small house needs, from the soil to the tiles.

- **Items:** #110, #111, #112, #113, #114, #115, #116, #117
- **Done when:**
  - from the sample house and a given soil survey, Trazo produces a formwork plan, a bar schedule, footings, lintels and a roof structure, each with its traffic light;
  - a water tank placed on a slab is checked;
  - reinforcement matches hand calculations of published examples within 5 %.

### M6: Water, sewage and rain

The plumbing discipline, designed and simulated.

- **Items:** #72, #96, #118, #119, #120, #121, #122, #123
- **Done when:**
  - from the sample house, Trazo produces a plumbing sheet with cold and hot water, sewage and rainwater;
  - the water tank and septic system are sized, and the pressure at every fixture is shown;
  - an isometric diagram exports;
  - plumbing is a fourth discipline view.

### M7: Electrical, beyond the basics

- **Items:** #124, #125, #126, #127, #128, #129
- **Done when:**
  - the electrical shopping list matches a manual takeoff of the sample house within 5 %;
  - every room shows its lighting level;
  - a photovoltaic system is pre-sized for the project's city;
  - a main board with a sub-board is modelled.

### M8: Comfort and performance

Simulations of how the house will feel to live in.

- **Items:** #130, #131, #132, #133, #134
- **Done when:**
  - the plan shows shadows on the lot on 21 June and 21 December at 9 h and 15 h;
  - walls and roof pass or fail the NBR 15575 simplified thermal method for the project's climate zone;
  - every room shows its lighting and ventilation ratios.

### M9: From model to construction

Materials detected from the model, costs, the work plan and the safety of the site.

- **Items:** #135, #136, #137, #138, #87
- **Done when:**
  - one click produces, for the sample house, the shopping list by construction stage, its cost from an imported SINAPI table and a schedule;
  - every material used by a calculation comes from the editable catalogue;
  - the site safety and permits guide covers every stage.

### M10: AI co-pilot

Assistants that can do everything, explain problems and leave the person in control.

- **Items:** #74, #75, #46, #51, #49, #73, #48, #76, #50, #52, #142
- **Done when:**
  - an assistant draws a two-storey house with stairs, finishes, installations, structure and a sheet set **using MCP only**;
  - the person approves each proposed change, can compare and revert any AI action, and gets every warning and simulation explained in plain language with a suggested fix.

### M11: Progressive experience

Simple for newcomers, fast for professionals.

- **Items:** #65, #66, #67, #68, #69, #82
- **Done when:**
  - in a usability test with three people new to CAD, each completes *Minha primeira casa* (my first house), including its structure check, without help in under 20 minutes;
  - professional mode offers a command line;
  - the interface is available in English.

### M12: Platform

Regional data and extensions without touching Trazo's code.

- **Items:** #140, #141
- **Done when:**
  - installing a regional pack (for example, one utility's entrance table) changes the sizing that uses it;
  - a sample mod adds a check and a schedule through the public API.

### M13: Scale and robustness

Runs in parallel with the other milestones.

- **Items:** #62, #61, #83, #84
- **Done when:**
  - a project with 500 walls on 5 floors pans at 60 fps with every edit under 50 ms;
  - undo memory stays bounded;
  - exported PDFs are compared against reference images in CI.

### Exploratory

Worth prototyping when the core is solid: #85, #86, #109, #139.

## Backlog

### A. Construction site

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 53 | Automatic dimension chains locating each door and window along its wall, plus internal room dimensions | P0 | M | M2 | 🟡 Associative dimensions can anchor to openings and internal room dimensions can be generated; no automatic opening chains |
| 54 | First-course plan and wall elevations with block paging: whole, half and cut blocks, lintels and sills | P1 | L | M2 | 🟡 Properties show each wall's first course; there is no drawing |
| 55 | Setting-out plan (*locação*): cumulative dimensions from an origin on the lot, for marking out the building | P1 | M | M2 | ⬜ |
| 56 | Construction quantities: blocks with waste, half blocks and U-blocks, lintels per opening, laying mortar, plaster, flooring and wall finishes per room | P0 | M | M2 | 🟡 Block estimate, wall area and finish quantities exist separately; no consolidated takeoff |
| 57 | One-click construction set: A3 at 1:50 with dimensions, openings, schedules, first course, setting-out and legend | P0 | M | M2 | 🟡 Sheets, scales, templates and title blocks exist |
| 71 | Roof plan (*planta de cobertura*): roof planes, slopes, ridges, gutters and downpipes | P1 | L | M2 | ⬜ |

### B. Engineering safety and trust

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 99 | Load path across floors: columns stacked through the storeys, loads summed from the real floors above, misaligned columns flagged as transfers (B03) | P0 | M | M3 | ⬜ |
| 100 | Engineering methods guide (`docs/engineering.md`): every calculation with its formula, standard, assumptions and limits, validated in CI against published worked examples (B12) | P0 | M | M3 | 🟡 Methods are described in the user guide; textbook checks exist for beams, demand and voltage drop |
| 101 | Results as a traffic light: margin to the limit, the assumptions used, and a plain-language explanation of what to change | P0 | M | M3 | 🟡 Warnings exist; no margins or traffic light |
| 102 | Applicability limits per module (storeys, spans, loads, soils); outside them Trazo refuses to size and asks for a professional | P0 | S | M3 | ⬜ |
| 103 | Pre-sizing report (*memorial*) as PDF: inputs, assumptions, results, warnings and drawings, for a licensed professional to review and sign (ART or RRT) | P0 | M | M3 | ⬜ |
| 104 | Release gate: a sheet set cannot be marked *Liberado para obra* with open red results unless a reviewer (name and CREA/CAU number) signs them off | P1 | S | M3 | ⬜ |

### C. Simulation core

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 94 | 2D frame analysis: beams and columns together (column stiffness, rigid and pinned joints), internal forces, reactions and the deformed shape | P0 | XL | M4 | 🟡 Continuous beams on pinned supports: moments, shear, reactions and deflection |
| 105 | Load cases and combinations: NBR 6120 loads, NBR 8681 ultimate and service combinations, and wind on the building (NBR 6123) | P1 | L | M4 | ⬜ |
| 106 | Results on the plan and on paper: diagrams over the beams, deformed shape, utilisation colours and an animated load path from slab to soil, with labels that avoid each other (B05) | P1 | M | M4 | 🟡 Beam diagrams on a separate sheet |
| 107 | What-if mode: sliders for span, section, load and material, results updating live and the safe range highlighted | P1 | M | M4 | ⬜ |
| 108 | Analysis in the background with cached results and a shared font database, so large projects stay responsive (B04, B17) | P1 | M | M4 | ⬜ |
| 109 | Grid analysis of slabs and beams together (grid analogy), including L-shaped and irregular slabs (B06) | P2 | XL | Exploratory | ⬜ |

### D. Structure from foundation to roof

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 110 | Reinforcement pre-sizing for beams, columns, slabs and footings, with a bar schedule (*tabela de aço*), steel weights and cracked-section deflection (B12) | P1 | L | M5 | ⬜ |
| 111 | Lintels and sills (*vergas e contravergas*) for every opening, and masonry checks for infill and load-bearing walls (NBR 16868) | P1 | M | M5 | ⬜ |
| 112 | Foundations: soil from a survey (SPT) or a soil-type table; footing, raft (*radier*) and bored-pile (*broca*) options; eccentricity, punching and overturning checks (B11) | P1 | L | M5 | 🟡 Square and rectangular footings sized by soil pressure |
| 113 | Point loads on the structure: water tank (*caixa d'água*) and heavy equipment, checked on the slab or beam below | P0 | S | M5 | ⬜ |
| 114 | Roof structure pre-sizing: timber trusses and rafters, tile weights and wind uplift, from the roof plan (#71) | P1 | L | M5 | ⬜ |
| 115 | Stairs, balconies and cantilevers: slab and beam checks with the extra care cantilevers need | P1 | M | M5 | ⬜ |
| 116 | Two-way slab moments and deflections by tables (Czerny, Marcus), and L-shaped rooms split into rectangular panels (B06) | P1 | M | M5 | 🟡 Minimum thickness by span; loads shared by edge length |
| 117 | Columns at any angle and beams supported on beams (B07) | P2 | M | M5 | ⬜ |

### E. Water, sewage and rain

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 72 | Plumbing points (water, drains, gas) with legend, counts and a fixture schedule | P1 | M | M6 | ⬜ |
| 96 | Discipline views: architecture, electrical, plumbing and structure as views of one model, each with its own visibility, colours and sheets | P1 | M | M6 | 🟡 Architecture, electrical and structure on screen and per sheet; plumbing joins with #72 |
| 118 | Cold water (NBR 5626): daily consumption, water tank volume, pipe diameters by fixture weights and the pressure left at each fixture, so a weak shower shows up before it is built | P1 | L | M6 | ⬜ |
| 119 | Hot water: electric, gas and solar heater sizing | P2 | M | M6 | ⬜ |
| 120 | Sewage (NBR 8160): fixture units, pipe diameters and slopes, inspection boxes and vents | P1 | L | M6 | ⬜ |
| 121 | Septic tank, filter and soakaway (NBR 7229, NBR 13969), with the distances to wells and boundaries | P1 | M | M6 | ⬜ |
| 122 | Rainwater (NBR 10844): gutters and downpipes from the roof area and local rainfall, and a cistern for reuse | P1 | M | M6 | ⬜ |
| 123 | Plumbing isometric and riser diagrams generated from the model | P2 | M | M6 | ⬜ |

### F. Electrical, beyond the basics

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 124 | Electrical shopping list: wire length per section and colour, conduits, boxes, breakers and RCDs | P1 | S | M7 | ⬜ |
| 125 | Lighting calculation (lumen method): lux per room against the NBR ISO/CIE 8995-1 targets | P2 | M | M7 | ⬜ |
| 126 | Photovoltaic pre-sizing: consumption, kWp, panels on the roof planes and monthly generation for the city | P2 | M | M7 | ⬜ |
| 127 | Several boards (main and sub-boards), short-circuit estimate and breaker selectivity (B09) | P2 | L | M7 | ⬜ |
| 128 | Intermediate (four-way) switches, and devices on room boundaries assigned to the right room (B08, B10) | P1 | S | M7 | ✅ |
| 129 | Earthing layout and a simplified lightning-protection risk check (NBR 5419) | P2 | M | M7 | ⬜ |

### G. Comfort and performance

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 130 | Sun path and shadows on the lot for any date and hour, from the site's latitude and the north arrow | P1 | M | M8 | ⬜ |
| 131 | Thermal performance (NBR 15575, simplified method): transmittance and thermal capacity of walls and roofs from their layers, by bioclimatic zone (NBR 15220) | P1 | L | M8 | ⬜ |
| 132 | Natural lighting and ventilation ratios per room (openings against floor area), feeding the code checks (#73) | P1 | S | M8 | ⬜ |
| 133 | Acoustic estimate of walls and floors (NBR 15575) | P2 | M | M8 | ⬜ |
| 134 | Energy use estimate: appliances, lighting, water heating and the photovoltaic offset | P2 | M | M8 | ⬜ |

### H. From model to construction

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 135 | Materials catalogue: properties of concretes, steels, blocks, mortars, timbers, tiles, pipes and wires, detected from the model and used by every calculation; editable | P0 | M | M9 | 🟡 Masonry, wall layers and finishes have their own materials |
| 136 | Shopping list by construction stage from every discipline, with waste and unit conversion (cement bags, sand and gravel by mix) | P1 | M | M9 | ⬜ |
| 137 | Construction schedule: stages, dependencies, curing and shoring times, crews (SINAPI productivity) and a physical-financial timeline | P1 | L | M9 | ⬜ |
| 138 | Site safety and permits guide: a checklist per stage (NR-18: excavations, shoring, scaffolding, protective equipment) and what needs a professional or a permit (ART or RRT, *alvará*, *habite-se*) | P0 | M | M9 | ⬜ |
| 87 | Cost estimate from the quantities, with editable price tables (SINAPI import) | P1 | M | M9 | ⬜ |
| 139 | Site progress log: photos and notes per stage, compared with the plan | P2 | M | Exploratory | ⬜ |

### I. AI assistants

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 74 | MCP covers the whole model: floors, stairs and ramps, finishes, layers, dimensions, notes, hatches, library items, sheets and views | P0 | L | M10 | 🟡 43 tools cover lot, walls, rooms, openings and gates, fixtures, the electrical design, the structure, drawings and CAD editing |
| 75 | Read-only switch in *Conectar uma IA*, for assistants that should only analyse | P1 | S | M10 | ⬜ |
| 46 | Approval mode: the assistant proposes, the plan shows a preview, the person accepts or rejects | P1 | M | M10 | ⬜ |
| 51 | AI history with a diff per action, comparison and selective revert | P1 | M | M10 | 🟡 Live highlight, action log, revisions and version comparison (#45) exist |
| 49 | Plan validation with explanations and one-click fixes: unclosed rooms, overlaps, rooms without doors, unreachable spaces | P1 | M | M10 | 🟡 The circulation check (#25) covers clearances and door swings |
| 73 | Building-code checks: minimum room areas and widths, lighting and ventilation ratios, door widths; rule sets configurable per municipality | P1 | L | M10 | ⬜ |
| 48 | Built-in assistant panel: natural-language commands inside Trazo, using the user's own AI provider | P1 | L | M10 | 🟡 Works today through external assistants over MCP |
| 76 | MCP prompts (for example "draw a house from this brief") and richer resources: sheet images, schedules | P2 | S | M10 | ⬜ |
| 50 | Room layout proposals from a brief (rooms, areas, adjacencies), shown as alternatives | P2 | L | M10 | ⬜ |
| 52 | An assistant that teaches the editor: context tips and guided exercises | P2 | M | M10 | ⬜ |
| 142 | Explain any result: the assistant reads a simulation or warning, explains it in plain language and proposes the change that turns it green | P1 | M | M10 | ⬜ |

### J. Experience

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 65 | Simple, Intermediate and Professional interface levels on the same project and file, engineering results included | P1 | L | M11 | ⬜ |
| 66 | Guided flow *Minha primeira casa*: lot, desired rooms, adjustable layout, openings, structure and a final check | P1 | L | M11 | ⬜ |
| 67 | Guided documentation flow: build and review a sheet set before exporting | P2 | M | M11 | ⬜ |
| 68 | English interface (internationalisation), keeping Brazilian Portuguese as the default | P1 | L | M11 | ⬜ |
| 69 | Command line and command palette with typed commands, aliases and recent commands | P1 | M | M11 | ⬜ |
| 82 | Dark theme, interface scale and full keyboard navigation | P2 | M | M11 | ⬜ |

A chosen level must never be a wall. A newcomer can open an advanced tool when needed, and a professional keeps the simple actions. For example, *check this beam*:

- **Simple:** a green, yellow or red badge and one sentence on what to change.
- **Professional:** the same beam with its loads, diagrams, margins, formulas and the standard's clauses.

### K. Platform

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 140 | Regional data packs: utility entrance tables, rainfall intensity, wind speed, climate zones, soil tables, prices and municipal rules, installable and versioned (B13) | P1 | M | M12 | ⬜ |
| 141 | Mod API: commands, plan overlays, schedules and checks from sandboxed scripts, built on the MCP command layer | P2 | XL | M12 | ⬜ |

### L. Release and quality

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 97 | CI on Windows and macOS as well as Linux: format, lint and tests on every platform (B02) | P0 | S | M1 | ✅ |
| 98 | Line-ending policy (`.gitattributes`) for a clean working tree on every platform (B18) | P2 | S | M1 | ✅ |
| 80 | Documented project file format (JSON Schema) for third-party tools | P1 | M | M1 | ⬜ |
| 78 | Signed Windows binaries | P2 | S | M1 | ⬜ |
| 62 | Performance with large plans: spatial index for picking, cached rendering, benchmarks in CI (B04) | P1 | L | M13 | ⬜ |
| 61 | Visual regression tests for PDF, PNG and SVG export | P1 | M | M13 | ⬜ |
| 84 | Transaction-based undo history (diffs instead of full project copies) | P2 | L | M13 | ⬜ |
| 83 | Undo history panel: list of steps, jump to any step | P2 | S | M13 | ⬜ |

### M. Exploratory

| # | Item | Priority | Size | Status |
| --- | --- | --- | --- | --- |
| 85 | 3D massing preview: extruded walls, openings, slabs and the structure, orbit view | P2 | XL | ⬜ |
| 86 | IFC export of walls, spaces, openings, floors and structural elements, for BIM workflows | P2 | L | ⬜ |

## Definition of done

An item is done when:

- it works on screen **and** in every export it touches (screen equals paper);
- it has tests, including a regression test for each bug fixed along the way;
- assistants can use it through MCP, when it changes the model;
- the [user guide](docs/user-guide.md), the [MCP guide](docs/mcp.md) and the [changelog](CHANGELOG.md) are updated;
- `cargo fmt`, `cargo clippy -- -D warnings` and `cargo test` pass on Windows, Linux and macOS.

An **engineering or simulation** item also needs:

- its method in `docs/engineering.md`: formula, standard, assumptions and limits;
- at least two published worked examples reproduced in tests, or agreement with a reference program;
- results that follow the [safety contract](#engineering-in-trazo-the-safety-contract): traffic light, margin, plain-language explanation and refusal outside the limits.

## Out of scope

To stay an approachable tool that helps people build safely, these are not planned:

- signing, approving or replacing a licensed professional: Trazo never issues a final design, an ART or RRT, or a permit;
- buildings beyond the limits of #102, such as more than three storeys, spans over 7 m, prestressed or special structures, or retaining walls taller than 1.5 m. These belong to professionals and dedicated software, and Trazo says so;
- photorealistic 3D rendering (a simple massing view is exploratory, #85);
- full BIM authoring (IFC export is exploratory, #86);
- fire-safety, HVAC and gas-piping design (gas points are part of #72);
- real-time cloud collaboration between people (AI collaboration is in scope).

Want to suggest a change to the roadmap? Open a [feature request](https://github.com/igors93/Trazo/issues/new?template=feature_request.yml) and explain the problem it solves.
