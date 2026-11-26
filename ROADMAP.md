# Trazo Roadmap

*Last updated 2026-11-26. This update reframes Trazo around its mission (see [the proposal](docs/proposta.md)): the first user is the family that builds without a professional. It adds milestone [M0: Protection for self-builders](#m0-protection-for-self-builders) as the first priority.*

This is the plan for what comes next. Work through it **milestone by milestone, in order**: each milestone builds on the previous one and ends with a concrete test of "done". [Next up](#next-up) lists the first ten items on that path, and [Bug assessment](#bug-assessment) lists what to fix along the way. Everything already shipped is in the [changelog](CHANGELOG.md).

- [Vision](#vision)
- [Engineering in Trazo: the safety contract](#engineering-in-trazo-the-safety-contract)
- [Where Trazo stands](#where-trazo-stands)
- [How to read this roadmap](#how-to-read-this-roadmap)
- [Bug assessment](#bug-assessment)
- [Implementation log](#implementation-log)
- [Next up](#next-up)
- [Milestones](#milestones)
- [Backlog](#backlog)
- [Definition of done](#definition-of-done)
- [Out of scope](#out-of-scope)

## Vision

> **Trazo exists so that a lack of money and technical knowledge does not leave a family completely unprotected against the risks of building its own home.**
>
> *O Trazo existe para que a falta de dinheiro e de conhecimento técnico não deixe uma família completamente desamparada diante dos riscos de construir a própria casa.*

Most homes in Brazil's favelas, peripheries and poorer neighbourhoods are built without an engineer or an architect. Design tools are made for people who already know how to design, so the people who most need guidance get none. That is the gap Trazo exists to close.

**Trazo is built for people who build without a professional:** families building or extending their own home with little money and little technical knowledge, the bricklayers who build for them, and the community groups that support them. It is not built for the engineer who already knows how to design. It draws the house, checks every risky decision, refuses what it cannot size safely, explains in plain language, and shows when to stop and get help, including the free technical assistance low-income families are entitled to (ATHIS, Lei 11.888/2008).

**MCP is Trazo's operational centre.** An AI assistant connected through MCP does the intellectual work of design: it turns a family's description into a plan, compares alternatives, runs and reviews every check, and organises the documents. The person does not need to learn CAD or engineering to start; they receive clear instructions to build. The drawing tools stay for anyone who wants to adjust by hand.

**Trazo contests the AI.** An assistant that designs alone can call a simulation with wrong hypotheses (a firm soil nobody surveyed, a live load below the standard, a stronger concrete than the site will mix) and receive a result that looks safe. So the essential condition is that Trazo's deterministic rules check and contest every AI decision, the hypotheses included: the AI supplies inputs, never results; every hypothesis records its origin; a favourable hypothesis without evidence cannot produce green; normative minimums cannot be lowered; and changing a hypothesis needs the person. Trazo now records the origin of every hypothesis (#168), contests favourable ones without evidence (#169) and holds an assistant's favourable changes for the person (#171); the rest of this rule is in [M0](#m0-protection-for-self-builders).

Trazo is not the engineer, and never pretends to be. **Professionals are allies:** Trazo prepares exact drawings, a calculation report and a review box so that an engineer or architect, often a volunteer or a public technical-assistance service, can review a family's project quickly and cheaply. When a decision has to choose between the family and the specialist, Trazo chooses the family.

The full proposal, written for everyone in Portuguese, is in [docs/proposta.md](docs/proposta.md).

### Principles

1. **Protect before impressing.** A check that prevents a serious mistake is worth more than ten new commands.
2. **MCP at the centre, Trazo as the challenger.** The AI does the intellectual work (designing, comparing, reviewing, organising) through MCP. Trazo's rules check and contest everything it does, hypotheses included, and cannot be bypassed. The person sees every change, stays in control and receives clear instructions to build.
3. **Write for the family, not the engineer.** The plain sentence comes first; the formula and the standard are one click away for the reviewer. An unexplained technical term is a defect.
4. **No dangerous decision passes in silence.** Red is never hidden, rounded away or switched off (see the [safety contract](#engineering-in-trazo-the-safety-contract)).
5. **The safe side when unsure.** Missing data takes the unfavourable value, and the result says so.
6. **Accessible for real.** Free, offline, fast on a modest computer, in Portuguese, with the files in the family's hands.
7. **Professionals are allies.** Everything Trazo produces should make a professional's review easier, including the free review of public technical assistance.
8. **Respect, without judgement.** People who build without an engineer are doing what they can. Trazo's role is to reduce the risk of that reality, not to blame those who live in it.
9. **Architecture before command count.** Moving a wall and seeing rooms, doors, dimensions, beams and conduits follow is worth more than a hundred generic commands.
10. **Built for the construction site.** Drawings leave Trazo ready for the site: dimensions in centimetres, located openings, modular masonry, quantities, printing to scale.
11. **Screen equals paper.** What you see is what you export; rules shared by both live in the domain.
12. **One model, every discipline.** Architecture, electrical, plumbing and structure are views of the same project, and a change in one shows up in the others.
13. **Every capability through MCP.** Anything a person can do, an assistant can do through MCP, visibly, reversibly, in plain language and with the person in control.
14. **Ease is a feature.** Every new capability, including safety measures, should add as few steps as possible.

## Engineering in Trazo: the safety contract

Engineering features follow these rules. They apply to everything in milestones M0 and M3 to M9, and an item that breaks them is not done. They matter most for the families Trazo is built for: for many of them, Trazo is the only check their house will get before it is built, so none of these rules is optional.

1. **Pre-sizing, not design.** Results guide decisions and the conversation with a professional. Building still requires a licensed professional's responsibility (ART or RRT) and the municipal permits; Trazo says so wherever a result could be mistaken for a final design, and never presents green as a guarantee.
2. **Show the work.** Every number can be opened to see its formula, the standard it follows (NBR 6118, 5410, 5626, 8160…), the inputs and the assumptions.
3. **Traffic light with margins.** Green has a comfortable margin, yellow passes with a small one, red fails or lies outside the method's limits. Red is never hidden, rounded away or silenced.
4. **Known limits.** Each method declares where it applies (storeys, spans, loads, soils). Outside them Trazo refuses to size and explains why, instead of guessing.
5. **Verified.** Each method is checked in CI against published worked examples and, for analyses, against a reference program.
6. **Conservative when unsure.** Missing data takes the safer default, and the result says that it did (#160 audits the defaults that do not yet, starting with the soil pressure).
7. **Plain language first.** The first explanation is for someone who never studied engineering ("this beam is too shallow for its span: make it 40 cm tall"); the full detail is for the reviewing professional.
8. **A way forward.** Every red result and every refusal says what to change, or where to find help: a professional, free technical assistance (ATHIS) or, for site risks, the Defesa Civil (#159).
9. **Ask about what Trazo cannot see.** Risks outside the model, such as a slope, a nearby stream, landfill or an unknown existing foundation, are asked about and never assumed away (#156, #157).
10. **Trazo contests the AI.** An assistant must not be able to reach a reassuring result through wrong hypotheses:
    - the AI supplies inputs, never results: every check is computed by Trazo's deterministic, tested rules;
    - every hypothesis (soil pressure, concrete, loads, room use, wind, terrain category) records its origin: Trazo's conservative default, data from the person (a soil survey, a concrete specification, the city) or the AI (#168);
    - a result that depends on a hypothesis more favourable than the conservative default, without evidence, is **contested**: never green, with the reason (#169);
    - normative minimums (NBR 6120 live loads for the room's use, the regional NBR 6123 wind speed) cannot be lowered (#169);
    - changing a hypothesis needs the person's confirmation in the window, with its evidence (#171);
    - simple independent cross-checks (total weight against foundation capacity, span-to-depth rules) contest a detailed simulation that disagrees with them (#172);
    - every AI action reports what it made worse: checks that changed colour, structural elements removed and hypotheses changed, in the tool answer and on screen (#170).

    All of it is in place (#168–#172).

## Where Trazo stands

| Area | Status |
| --- | --- |
| Protection for self-builders (M0) | 🟡 Traffic light with plain sentences, refusal outside the limits, declared assumptions, the review report, a logged AI activity and protective MCP server instructions exist; contesting the AI's hypotheses (#168–#172), safe defaults, help paths, the building guide, the plain-language pass, site-risk questions, building on top and in stages, and the guided first house remain |
| Drawing engine (#01–#10) | ✅ Complete: `f64` geometry, free-shape rooms, snaps, typed input, CAD modify tools, constraints, multiple selection |
| Architecture (#11–#25) | ✅ Complete: junctions, shared walls, multilayer walls, door and window types, stairs, floors, library, finishes, circulation checks |
| Documentation (#26–#37, #71, #151–#155) | 🟡 Core complete (#26–#37: associative dimensions, layers, sheets, scales, title blocks, hatches, schedules, schematic sections, sheet sets, ABNT styles); multi-viewport mixed-scale sheet composer, executive sections, site/roof/location plans, urban occupancy/permeability tables and 3D sheet perspective remain (#71, #151–#155) |
| Exchange (#38–#45, #70) | ✅ Complete: DXF and DWG, SVG and CSV, print to scale, image and PDF underlays, blocks with attributes, external references, version comparison |
| Release (#58–#64, #77–#81) | 🟡 CI, licence, property tests, migrations, release binaries, crash reports, Windows installer and samples done; schema, signing and CI on Windows and macOS remain |
| Construction site (#53–#57) | 🟡 Building blocks exist (first course per wall, block estimate, sheets); the site deliverables do not |
| Electrical design (#88–#91) | ✅ Devices, NBR 5410 forecast, circuits, conduits with wiring, phase balancing, voltage drop, demand, service entrance, single-line diagram |
| Structure (#92–#94, #105–#117, #143–#147, #150) | ✅ Complete from foundation to roof (M4 & M5): columns/beams/slabs/footings, 2D plane-frame analysis with NBR 6120/6123/8681 combinations, coupled slab+beam grid analogy, overlays, What-if, background cache, reinforcement & steel schedule, lintels/sills & NBR 16868 masonry, SPT & 4-way foundation study, ground beams & strip footings, boundary balance beams, per-wall sloped tops, timber roofs (mono-pitch/gable/hip/trusses/overhangs/uplift), timber posts & eave beams, stairs/balconies/cantilevers, tall-wall wind/sliding/bolts/ties/jambs and the end-to-end gourmet reference project (`examples/area-gourmet.json`) |
| Discipline views (#96) | ✅ Architecture, electrical, structure, foundations and prepared plumbing views on synchronized linked tabs sharing a single live model; hydraulic pipe calculations join with #72 |
| Plumbing (#72) | ⬜ Not started (sanitary fixture view discipline and dependency tracking prepared) |
| Simulation, materials and planning | 🟡 Simulation core (M4: #94, #105–#109) complete; materials catalogue, costs and construction schedule remain |
| AI (#46–#52, #74–#76) | 🟡 45 MCP tools (including synchronized tabs/views, floors, cross-discipline dependencies, regional utility/soil profiles, beam-on-beam supports and target `projeto_id` guard), real-time sync, tunnel token; no approval or read-only mode |
| Experience (#65–#69) | 🟡 Synchronized multi-view tabs (#95) with `.trazo` workspace persistence shipped; levels, guided flows and English interface not started |

**Shipped since the last update:** #38, #39, #40, #43, #44, #45, #63, #70, #77, #79, #81, #88, #89, #90, #91, #92, #93 and #95. They have left the tables below.

## How to read this roadmap

| Field | Meaning |
| --- | --- |
| **#** | Item number. Items keep their number for life; new items continue from #173. Use it in commits and pull requests: `feat: add lintels per opening (#111)`. |
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
| B03 | High | Structure | Each floor's structure is sized on its own, and the load of the floors above comes from a *floors* multiplier, not from the real upper floors. Columns that do not line up between floors are not detected. | ✅ Fixed: `structure_report` propagates real column loads top-down across floors, routes misaligned columns onto transfer beams as point loads and flags unsupported drops (#99); regression test `upper_floor_loads_flow_down_to_aligned_columns_and_flag_misaligned_transfers`. |
| B04 | Medium | Performance | Conduit marks, structure marks, the load schedule and device tags are recomputed on every frame. The cost grows with the square of the device and beam count, so large plans will stutter. | ✅ Fixed: added content-fingerprinted `Arc` caches (`domain::cache`) for conduit/structure marks, load schedule, device tags and structure report across frames, and replaced quadratic device/beam scans with indexed `HashMap`/`BTreeMap` passes; regression test `derived_caches_reuse_results_across_frames_and_invalidate_on_edit`. |
| B05 | Medium | Drawings | Conduit, beam, column and footing labels ignore the label layout, so they overlap at crowded wall junctions (seen on the sample house). | ✅ Fixed: `resolve_plan_mark_labels` places conduit, beam, column, footing and slab labels without overlaps (and registers them in `label_obstacles`); regression test `conduit_and_structure_labels_avoid_overlapping_at_crowded_junctions`. |
| B06 | Medium | Structure | Slab spans come from the room's bounding box, which is wrong for L-shaped rooms. Slab loads are shared by edge length, a rough stand-in for the yield-line method. | ✅ Fixed: `decompose_slab_panels` splits irregular/L-shaped rooms into rectangular panels for true short spans and `yield_line_shares` distributes slab loads by 45° yield lines (NBR 6118 14.7.6.1); regression test `l_shaped_slab_uses_panel_spans_and_yield_line_load_distribution`. |
| B07 | Medium | Structure | Columns turn in quarter turns only, so they come out misoriented on angled walls. Beams can only rest on columns, not on other beams. | ✅ Fixed: added `angle_rad` to `Fixture` for arbitrary-angle wall alignment and `BeamSupport::Beam` with point-load transfer into primary girders; regression test `angled_columns_and_beam_on_beam_supports_transfer_loads_accurately`. |
| B08 | Medium | Electrical | Two-way switches are wired in pairs only; intermediate (four-way) switches are counted as one-way, giving wrong wire counts. | ✅ Fixed: added `FixtureKind::SwitchIntermediate` (four-way) and generalized multi-way switch chains in `conduit_wiring`; regression test `intermediate_switches_carry_four_travellers_and_chain_multiple_points`. |
| B09 | Medium | Electrical | Only the first board is used, so sub-boards (QDC) are not modelled. | ✅ Fixed: `ConduitTree`, `generate_conduits`, `cable_length`, `LoadSchedule` (`SubBoardSummary`) and `single_line_svg` now model secondary boards (`QDC1`, …) with dedicated feeders and local terminal lengths; regression test `sub_boards_are_modelled_with_dedicated_feeders_and_shortened_terminal_runs`. |
| B10 | Medium | Electrical | Devices are assigned to rooms by their centre. A device right on a room boundary can land in the neighbouring room, which changes its circuit and RCD. | ✅ Fixed: `Project::room_for_fixture` resolves boundary devices by the room their front face points into (with footprint-overlap fallback); regression test `boundary_device_is_assigned_to_the_room_it_faces`. |
| B11 | Medium | Structure | Footings are checked by area only: no eccentric load, punching or overturning check. | ✅ Fixed: `check_footing` and `FootingResult` now check eccentric peak soil pressure, middle-third / overturning safety factor (NBR 6122) and punching shear (NBR 6118 19.5); regression test `footings_check_eccentric_pressure_overturning_and_punching_shear`. |
| B12 | Medium | Structure | Deflection uses half the gross inertia and no reinforcement, so it can be off in either direction. | ✅ Fixed: `effective_beam_stiffness` computes NBR 6118 Branson equivalent inertia (`M_r`, pre-sized CA-50 tension steel `A_s` and Stage II `I_II`); regression test `branson_deflection_uses_uncracked_inertia_below_mr_and_cracked_as_above_mr`. |
| B13 | Medium | Data | Demand factors, entrance tables, live loads and soil pressures are generic Brazilian values; some utilities and regions differ. | ✅ Fixed: added regional `UtilityProfile` presets (Enel SP, CPFL, CEMIG, COPEL) with `UtilityOverrides`, plus `SoilClass`, `OccupancyClass` and per-room NBR 6120 live loads; regression test `utility_profiles_and_regional_overrides_change_demand_and_entrance`. |
| B14 | Low | DXF | Floor conduits export as solid lines, and structure labels have a fixed text height. | ✅ Fixed: floor conduits export with `DASHED` linetype (group 6) and structure labels scale their text height with the sheet scale; regression test `floor_conduits_use_dashed_linetype_and_structure_labels_scale_with_sheet`. |
| B15 | Low | Views | In the side-by-side view, the inactive pane ignores its tab's discipline. | ✅ Fixed: `side_plan` filters the inactive tab's project through `for_discipline`, renders its focus overlay and shows its discipline in the view plate; regression test `inactive_split_pane_respects_its_tab_discipline`. |
| B16 | Low | Sheets | A sheet exported at a scale that does not fit crops the plan. The sheet panel warns, but the exported file does not. | ✅ Fixed: exported sheets stamp a visible crop warning banner (`#aviso-recorte`) with a suggested scale when the plan exceeds the drawing area; regression test `cropped_sheet_includes_warning_banner_and_fitting_sheet_omits_it`. |
| B17 | Low | Performance | System fonts are loaded on every SVG rasterisation (print preview, single-line diagram, beam diagrams), causing a hitch after each change. | ✅ Fixed: shared `OnceLock` font databases reused across `resvg` rasterisation and `svg2pdf` export; regression test `shared_font_databases_are_reused_across_calls`. |
| B18 | Low | Repository | No `.gitattributes`; with `core.autocrlf` the working tree mixes LF and CRLF, which makes diffs noisy. | ✅ Fixed: added `.gitattributes` enforcing LF for source/docs and CRLF for Windows scripts (#98). |
| B19 | Low | Fixtures | The minimum fixture size dropped from 20 to 10 cm for every kind so that 14 cm columns fit; it should be a per-kind minimum. | ✅ Fixed: `FixtureKind::min_size` enforces 10 cm for columns and 20 cm for all other fixtures; regression test `minimum_size_is_per_kind`. |
| B20 | Medium | Quantities | The materials schedule counts blocks with a fixed 19 cm block height instead of the masonry's own height, so walls of shorter bricks get too few blocks. | ✅ Fixed: `build_materials_schedule` (`src/domain/schedules/materials.rs`) uses `Masonry::blocks_per_square_metre` with each wall's own masonry and groups mixed masonries by wall; regression test `materials_schedule_uses_each_walls_block_height_and_masonry_name`. |
| B21 | Medium | Data | The regional utility profiles carry utility names and standards but thresholds and demand multipliers that are not traced to those standards. | ✅ Fixed: extracted `src/domain/utility_profiles.rs` with explicit per-utility demand tables (replacing hidden scale multipliers), corrected CEMIG ND 5.1 (`8/16 kVA`) and COPEL NTC 901100 (`9/15 kVA`) limits, and `UtilityStandardCitation` (standard, edition, tables, verification note) surfaced in UI, single-line SVG and MCP `quadro_de_cargas`; regression tests `every_builtin_profile_cites_a_standard_and_uses_no_hidden_multipliers` and `utility_tables_reflect_published_regional_rules`. |
| B22 | Low | Sheets | The sheet panel checks whether the plan fits (and computes *Melhor escala*) using `ph - 16 mm`, ignoring the 50 mm title-block reservation used during export. | ✅ Fixed: added shared `SheetLayout::drawing_area_mm()` (reserving 50 mm with title block, 14 mm without) used identically by the Sheet panel UI, MCP `prancha` and SVG/PDF sheet exporter; regression test `drawing_area_reserves_title_block_height_and_matches_sheet_export`. |
| B23 | High | Structure | Columns and footings cannot stand on a boundary wall: they snap to the wall's centre line, which lies on the lot line, and are refused for leaving the lot; *Gerar estrutura* silently leaves those walls without columns. | ✅ Fixed: columns and footings snapped to a boundary wall are moved flush inside the lot (`fit_inside_lot`, `structural_placement_fitted`), *Gerar estrutura* places them and reports any it could not (`skipped`); regression test `columns_and_footings_on_a_boundary_wall_stand_flush_inside_the_lot`. |

### New bugs found during implementation

Recorded while working through the [implementation log](#implementation-log), and deliberately not fixed in those commits.

#### B20: Materials schedule ignores the block height

- **Status:** ✅ Fixed (`src/domain/schedules/materials.rs`, regression test `materials_schedule_uses_each_walls_block_height_and_masonry_name`).
- **Severity:** Medium. Quantities feed purchases; an undercount leaves the site short of blocks.
- **Description:** `build_materials_schedule` in `src/domain/schedules.rs` divides the net wall area by `(block_length + joint) × (0.19 + joint)`, a hard-coded 19 cm block height, instead of `masonry.block_height`. It also uses the project's default masonry name in the first row even when walls have their own masonry.
- **Expected:** blocks per m² = 1 / ((block length + joint) × (block height + joint)), with each wall's own masonry, as `Masonry::blocks_per_square_metre` already computes.
- **Observed:** for the default *Tijolo cerâmico 6 furos (em pé)* (19 × 14 cm), the schedule counts 1 / (0.20 × 0.20) = 25 blocks/m² instead of 1 / (0.20 × 0.15) ≈ 33.3 blocks/m², about 25 % too few.
- **Steps to reproduce:** new project, draw any wall with the default masonry, open *Quadros*, compare *Blocos estimados* with net wall area × 33.3.
- **Affected:** `src/domain/schedules.rs` (`build_materials_schedule`), the *Quadro de Materiais e Alvenaria* on sheets and in CSV.
- **Impact:** wrong purchase quantities on every project not built with 19 cm-high blocks.

#### B21: Regional utility profiles are not traced to their standards

- **Status:** ✅ Fixed (`src/domain/utility_profiles.rs`, surfaced in UI, single-line diagram and MCP `quadro_de_cargas`).
- **Severity:** Medium. The values decide the supply type, main breaker and feeder shown under a utility's name.
- **Description:** `UtilityProfile` in `src/domain/distribution.rs` names Enel SP (LIG-BT), CPFL (GED-13), CEMIG (ND 5.1) and COPEL (NTC 901100), with single- and two-phase limits, minimum breakers and feeders, and `general_demand_scale` / `specific_demand_scale` multipliers (0.90 to 1.0). No clause of those standards is cited, and the standards define their own demand tables rather than multipliers on a generic one.
- **Expected:** every regional value cites the standard, edition and table it comes from, and demand follows each utility's own table; unverified profiles are labelled as such in the interface.
- **Observed:** the values are presented as the utility's rules with no source; the scale factors have no counterpart in the standards.
- **Steps to reproduce:** open *Projeto elétrico › Quadro e diagrama unifilar*, pick a utility profile, and compare the supply limits with the utility's published standard.
- **Affected:** `src/domain/distribution.rs` (`UtilityProfile`, `UtilityRules`), the board summary, the single-line diagram and `quadro_de_cargas`.
- **Impact:** a wrong supply category or main breaker under a utility's name, which a newcomer would trust. Documented as unverified in `docs/engineering.md` until fixed (#140).

#### B22: Sheet panel fit check ignores title-block height

- **Status:** ✅ Fixed (`SheetLayout::drawing_area_mm` shared by UI, MCP and exporter; regression test `drawing_area_reserves_title_block_height_and_matches_sheet_export`).
- **Severity:** Low. Misleads the user in the Sheet panel and can make *Melhor escala* pick a scale that overflows into the title block.
- **Description:** `src/app/ui/sheet.rs` computes both `DrawingScale::best_fit` and `scale.fits` with `(ph - 16.0).max(20.0)` regardless of whether `title_block.show_title_block` is enabled, whereas `scaled_sheet_svg` in `src/io/export/sheet.rs` reserves `50.0 mm` at the bottom when the title block is shown (`14.0 mm` when hidden).
- **Expected:** Both the Sheet panel fit check / *Melhor escala* button and the SVG/PDF sheet exporter share the same drawing-area height calculation (reserving 50 mm when `show_title_block` is true, 14 mm otherwise).
- **Observed:** A plan whose scaled height lies between `frame_h - 50 mm` and `frame_h - 16 mm` displays *"cabe na folha"* in the UI while the exported sheet stamps `#aviso-recorte` and overlaps the title block.
- **Steps to reproduce:** Create a plan whose height at 1:50 on A4 landscape (`frame_h = 190 mm`) is between `140 mm` (`7.0 m`) and `174 mm` (`8.7 m`) with the title block enabled; open the Sheet panel (shows green "cabe na folha") and export the sheet (shows the crop warning banner).
- **Affected:** `src/app/ui/sheet.rs` (lines 183 and 198) and `src/io/export/sheet.rs` (`scaled_sheet_svg`).
- **Impact:** Inconsistent fit status between screen and exported sheet, breaking the *screen equals paper* principle.

#### B23: Columns on boundary walls are refused or silently skipped

- **Status:** ✅ Fixed (found while laying out the structure of a real project, the gourmet area of *Casa com área de lazer*, through MCP).
- **Severity:** High. In Brazilian lots houses are commonly built against the boundary, and those walls are the ones that most need columns and special footings.
- **Description:** `Project::structural_placement` snaps a column or footing to the nearest wall junction or centre line within 0.5 m. A wall drawn on the lot line has its centre line on the boundary, so the column extends half its section past the lot, and `add_fixture` refuses it ("o ponto precisa ficar dentro do terreno"). `generate_structure` calls `add_fixture` the same way and ignores the refusal, so it leaves boundary walls without columns without saying so.
- **Expected:** columns on a boundary wall stand flush inside the lot (offset by half their section), with boundary footings (#144). *Gerar estrutura* reports any column it could not place.
- **Observed:**
  - `projeto_estrutural` › `inserir_pilar` at (0, 25.75), on a wall along x = 0 on the lot line, fails; so does (0.07, 29.83), which snaps back onto the line.
  - The workaround was `inserir_objeto` with `tipo: pilar`, which places it exactly.
- **Steps to reproduce:** a 10 × 50 m lot, a room against the left boundary, *Estrutura* › *Pilar* on that wall (or *Gerar estrutura*).
- **Affected:** `src/domain/structure.rs` (`structural_placement`, `generate_structure`), `src/app/interaction.rs` (`fixture_placement`), `src/app/mcp_tools/structure.rs` (`inserir_pilar`, `inserir_sapata`).
- **Impact:** missing columns on the walls that need them most, with no warning to a newcomer.

## Implementation log

### #115 Stairs, balconies and cantilevers — ✅ Done

- **Changes:** added `src/domain/cantilevers.rs` integrated with `StructureReport`, `verdict::structure`, `domain::cache`, `src/app/ui/structure.rs` (*Escadas, sacadas e balanços*) and MCP (`projeto_estrutural` → `definir_balanco`/`remover_balanco`, `pre_dimensionamento.escadas_e_balancos`):
  - **Concrete stairs** automatically derived from drawn `CirculationKind::Stair`: equivalent vertical thickness `h_waist/cos(α) + riser/2`, NBR 6120 Table 10 live load (2.5 / 3.0 kN/m²), one-way `Md = 1.4·q·L²/8`, bottom/top CA-50 steel, support reactions and long-term `L/250` deflection;
  - **Balconies & cantilever slabs** (automatically detected from balcony room names or 1-edge-supported slabs, plus explicit `CantileverSpec::Slab`): mandatory solid-slab check (flags ribbed cantilevers red), **ABNT NBR 6118:2023 §13.2.4.1 Table 13.2 `γ_n = 1.95 − 0.05·h_cm`** additional safety factor, NBR 6120 ≥ 2.5 kN/m² live load + parapet line load + `0.80 kN/m × 1.10 m` guardrail thrust, top negative CA-50 steel, strict **`L/125`** cantilever deflection (NBR 6118 Table 13.3) and **`1.5 · L_bal`** back-span overturning/anchorage check;
  - **Cantilever beams** (`CantileverSpec::Beam`): `L/5` minimum depth, root negative moment & shear, top CA-50 steel, `L/125` long-term deflection and `1.5 · L_bal` back-span check.
- **Tests:** `cantilever_balcony_matches_nbr6118_table13_2_gamma_n_and_l125_deflection` and `stair_and_cantilever_beam_match_worked_hand_calculations`.

### #114 Roof structure (gable, hip, trusses, wall plates, overhangs & uplift) — ✅ Done

- **Changes:** extended `src/domain/roof/` (`mod.rs`, `sizing.rs`, `analysis.rs`, `marks.rs`, `tests.rs`), `src/domain/verdict/structure.rs`, `src/app/ui/roof.rs`, `src/app/mcp_tools/structure.rs` and `src/mcp/protocol.rs` to support **gable (*duas águas*)** and **hip (*quatro águas*)** timber roofs alongside mono-pitch (*meia-água*), configurable perimeter **overhangs (*beirais*, 0–1.50 m)** with cantilever bending checks, **wall plates (*frechais*)**, 4-panel **Howe pitched timber trusses (*tesouras*)** sized by exact Method of Joints equilibrium + NBR 7190 compression/buckling and tension checks, 4 diagonal **hip rafters (*espigões*)**, integrated **NBR 6123 wind uplift** (including overhang edge suction `Cpe = −1.50` and support anchor forces), ridge/hip/overhang plan marks, traffic lights, UI controls and MCP (`definir_telhado` with `estilo`, `beiral_m`, `tesouras`).
- **Tests:** `gable_and_hip_roofs_size_howe_trusses_wall_plates_hip_rafters_overhangs_and_uplift` verifies exact 4-panel Howe truss axial forces (`3/8·W/sinθ`, `3/8·W/tanθ`, `1/8·W/sinθ`, `W/4`), gable/hip height planes, hip surface area `(W+2oh)(L+2oh)/cosα`, wall plates, 4 hip rafters, overhang bending, wind uplift and plan marks.

### #106, #107, #108, #109 Simulation core completion — ✅ Done

- **#106 Results on the plan and on paper (`src/domain/plan_results.rs`, `src/app/canvas/sim_overlay.rs`, `src/io/export/conduits.rs`):** shared `SimulationOverlay` state persisted on `ProjectStructure::overlay` drives identical on-screen and SVG/PDF paper overlays (*screen equals paper*): traffic-light utilisation colours on slabs, beams, columns and footings; bending-moment (`M`) and shear (`V`) diagrams drawn directly over beams on the floor plan; exaggerated deformed shapes (`δ` and `L/xxx`); directional slab → beam → column → footing/soil load path (animated marching flow pulses on screen, static directional arrows on paper); and overlap-free label resolution (`resolve_overlay_labels`, B05). Accessible from the *Estrutura* ribbon (*Simulação na planta*) and MCP (`projeto_estrutural` → `configurar_sobreposicao`).
- **#107 What-if mode (`src/domain/whatif.rs`, `src/app/ui/whatif.rs`):** non-destructive interactive sliders for span scale (`0.60–1.60×`), beam width/height, column side, slab thickness, live load, concrete `fck`, soil allowable pressure, wind `V0` and NBR 6120 occupancy, running Trazo's real structural & verdict solvers in `< 100 ms` on the sample house, painting green/yellow/red safe-range bars under each slider, comparing baseline vs. simulated deltas (`Δ utilisation`, `Δ steel kg`, `Δ concrete m³`), with one-click *Aplicar* (with undo) and MCP (`projeto_estrutural` → `simular_whatif`).
- **#108 Background analysis with cached results & shared font database (`src/domain/background.rs`, `src/domain/cache.rs`):** persistent lazily-spawned worker thread (`trazo-analysis-worker`) with latest-wins job coalescing computes `StructureReport`, `ReinforcementReport` and `LoadSchedule` off the UI thread into the shared content-fingerprinted `RingCache` (B04) alongside the shared `OnceLock` font databases (B17), displaying a status-bar progress indicator while computing and waking `egui::Context::request_repaint()` on completion.
- **#109 Coupled slab + beam grid analysis (`src/domain/grid_analysis/`, `src/app/ui/grid.rs`):** 3-DOF/node (`w, θx, θy`) grillage FEM (*analogia de grelha*, Hambly 1991 / NBR 6118 §14.7.5) with banded Cholesky solver coupling rectangular, L-shaped and irregular slab polygons (B06) with supporting concrete beams (`EI` + cracked torsional stiffness `0.15·GJ`) and columns, reporting `mx/my/mxy`, elastic & long-term deflections vs. `L/250`, coupled beam moments/shear/torsion, column reactions with global vertical equilibrium verification, printable A3 SVG/PDF deflection heatmap (`grid_analysis_svg`), UI *Grelha (Lajes+Vigas)* tab, and MCP (`pre_dimensionamento.grelha_lajes_vigas`, `exportar_planta` `grelha_svg`/`grelha_pdf`).
- **Tests:** `background_analyzer_computes_off_main_thread_populates_cache_and_wakes_ui`, `plan_simulation_overlay_builds_diagrams_deformed_shape_utilisation_and_load_path_without_label_overlap` (including ABNT sheet SVG round-trip), `whatif_sliders_update_sample_house_under_100ms_and_compute_safe_ranges`, `simply_supported_square_plate_matches_timoshenko_theory` (Timoshenko Table 8), `simply_supported_rectangular_2_to_1_plate_matches_timoshenko_theory` (Timoshenko Table 8), `l_shaped_slab_coupled_with_beams_satisfies_equilibrium_and_renders_valid_svg`, and `mcp_covers_plan_overlay_whatif_and_grid_analysis`.

### #105 Load cases, combinations and building wind — ✅ Done

- **Changes:** added `src/domain/loads/` (`nbr6120.rs`, `nbr6123.rs`, `nbr8681.rs`, `analysis.rs`, `tests.rs`) separating characteristic actions into `G1` (structural concrete self-weight), `G2` (finishes, masonry walls, roof dead, fixed concentrated loads), `Q` (NBR 6120:2019 occupancy & per-room Table 10 live loads with occupancy-dependent `ψ0/ψ1/ψ2` persisted in `StructureSettings.occupancy`), and `W±` (ABNT NBR 6123 whole-building wind in `+X`, `−X`, `+Y`, `−Y` with Tables 1–3 `S1/S2(z)/S3` presets, Table 4 `Cpe`/`Ca` with `a/b` interpolation, storey-by-storey forces, base shear, overturning moment and `ELU-EQU` rigid-body stability screening); generated all 17 ABNT NBR 8681:2003 / NBR 6118:2023 §11.8 combinations (`ELU-1..7`, `ELU-EQU±`, `ELS-QP`, `ELS-F1..3`, `ELS-R1..4`); upgraded `frame2d` (`builder.rs`, `diagrams.rs`) to solve the four fundamental load cases independently and superpose all 17 combinations linearly into governing ELU/ELS beam & column envelopes, `ELS-F` drift (`H/850`) and `ELU-6/7` column uplift checks; surfaced identically in the new *Cargas e combinações* UI tab (`src/app/ui/loads.rs`), enhanced *Pórticos 2D* tab, traffic-light verdicts (`src/domain/verdict/structure.rs`), PDF memorial (`src/io/report.rs`), MCP `pre_dimensionamento.cargas_e_combinacoes` + `porticos_2d`, and `docs/engineering.md`.
- **Tests:** `nbr_8681_combinations_match_worked_hand_example_across_occupancies` (worked hand example for Residential/Office/Garage), `nbr_6123_building_wind_matches_published_table4_worked_example` (2-storey 12×6 m building, `V0 = 40 m/s`, Cat. III, Table 4 `α = 0°/90°` & interpolation), `frame_nbr8681_superposition_matches_direct_factored_solve_and_feeds_outputs` (linear superposition vs direct factored `Frame2D` solve to `< 1e-9` + traffic lights + PDF memorial), `multi_storey_real_floors_raise_s2_with_height_and_narrow_frame_flags_wind_uplift`, and `mcp_reports_nbr6120_nbr8681_combinations_and_nbr6123_building_wind`.
- **Limitations:** static equivalent wind only (no NBR 6123 §9 dynamic gust analysis, 3D torsional diaphragm eccentricity, or seismic NBR 15421); remains inside Trazo's #102 applicability limits (≤ 3 storeys, ≤ 4 m storey height, ≤ 5 kN/m² live load).

### #104 Sheet release gate — ✅ Done

- **Changes:** `Project::set_sheet_status` refuses *Liberado para obra* while red engineering checks are open unless a named reviewer records a numbered CREA/CAU registration and review date. The acknowledgement stores the exact failed checks and sheet revision. New red results or a new revision make the effective status *Para aprovação* in the sheet UI, master document CSV and SVG/PDF exports. The exported revision table identifies the reviewer. MCP `prancha` can list, add, set status and export set sheets through the same rule.
- **Tests:** regression coverage for a denied release, valid review, JSON persistence, changed red results, a new revision, MCP release and SVG export. The entry is a recorded acknowledgement, not a cryptographic signature or proof of ART/RRT.

The items worked through in the order set on 2026-11-21: #97 → #99 → #100 → #101 → #102 → #103 → #56 → #53 → #113 → #94. Each entry records what was done, how it was checked, the commit and what remains.

### #97 CI on Windows and macOS — ✅ Done

- **Changes:** `.github/workflows/ci.yml` runs format, Clippy (`-D warnings`) and the tests on `ubuntu-latest`, `windows-latest` and `macos-latest`, with `fail-fast: false`.
- **Tests:** CI run 44919078527 (commit `fb23c1e`) passed on all three systems; locally on Windows, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-targets --all-features` pass (344 tests).
- **Commit:** `3f299a7`.
- **Limitations:** none known. Linux and macOS are checked only in CI, not on the development machine.

### #99 Load path across floors — ✅ Done

- **Changes:** `structure_report` walks the real upper floors from the top down and adds each upper column's load to the aligned column below (within 15 cm). A misaligned column becomes a point load on the beam under it (a transfer girder), or a warning when nothing supports it. The *floors* multiplier is only used when there are no real upper floors.
- **Tests:** `upper_floor_loads_flow_down_to_aligned_columns_and_flag_misaligned_transfers` (aligned, transfer and unsupported cases). The load-conservation check across floors joins the validation suite in #100.
- **Commit:** `79b7072`.
- **Limitations:** columns are matched by plan position only; a transfer girder gets the point load in its own analysis, but there is no detailing check of the transfer itself.

### #100 Engineering methods guide and validation suite — ✅ Done

- **Changes:**
  - `docs/engineering.md` documents every calculation (loads, slabs, beams, columns, the load path across floors, footings and the electrical installation) with its formula, standard clause, assumptions, limits and the test that checks it.
  - `src/domain/validation.rs` adds the validation suite: the AISC table 3-23 beam cases (simple span, two spans, three spans, central point loads), the 45° yield-line areas, the NBR 6118 Branson inertia and punching formulas, the middle-third footing pressure, and load conservation within a floor and across floors.
- **Tests:** 11 new tests, all passing; full suite, Clippy and format pass on Windows (CI covers Linux and macOS).
- **Commit:** `3b1b4db`.
- **Limitations:**
  - The references are closed-form textbook results and standard formulas, not a comparison with a commercial program; the frame comparison comes with #94.
  - Three uncertain references were stated conservatively in the guide.
  - The utility profiles are documented as unverified (B21).

### #101 Traffic-light results with margins and explanations — ✅ Done

- **Changes:**
  - `src/domain/verdict.rs` turns every structural and electrical check into a utilisation (demand over capacity) with a light, a margin, a plain-language sentence on what to change and the technical detail with its standard. Light thresholds: green up to 85 %, yellow up to 100 %, red above.
  - Checks covered: slab thickness and support; beam height, deflection and width; column section and smallest side; footing area, soil pressure, punching and overturning; transfer and unsupported columns; circuit current and voltage drop.
  - The new *Segurança* window (ribbon *Estrutura* and *Elétrica*) lists every element worst first, with the overall status, a filter, optional technical detail and the assumptions.
  - `pre_dimensionamento` and `quadro_de_cargas` return the traffic light per element (`semaforo`) and the assumptions (`premissas`).
- **Tests:** 4 new unit tests (thresholds, a generated house with no red and every element judged, a shallow beam turning red with what to change, the electrical checks) and an MCP assertion. The full suite, Clippy and format pass on an export of exactly this commit.
- **Commit:** `53e445b`.
- **Limitations:**
  - The window was checked by compiling and by unit tests of its data, not by a usability test.
  - The structure and electrical dialogs still show their own warnings next to it.
  - Conduit fill and the service entrance have no light yet.

### #102 Applicability limits per module — ✅ Done

- **Changes:**
  - `src/domain/limits.rs` sets the limits of the methods: at most 3 storeys (real floors or the *floors* setting), storey height 4 m, live load 5 kN/m², beam span 7 m, smaller slab span 6.5 m, soil at least 100 kPa for footings, demand 75 kVA for a low-voltage connection.
  - A project-wide violation refuses the whole structural sizing. *Gerar estrutura* and *Ajustar sapatas* are blocked in the interface and over MCP, the traffic light shows a single red *Estrutura* verdict, and the structure window opens with the reason.
  - An element beyond its limit keeps its place on the plan but gets a red *Fora dos limites* with no sizing numbers, the reason and what to do.
  - `pre_dimensionamento` returns `limites` (refusal and refused elements).
  - The limits are documented in `docs/engineering.md` and the user guide.
- **Tests:** 4 new tests (a house within the limits, a 9 m span refused with an explanation and no numbers, too many storeys, too much load and too tall a storey refusing the whole structure, footings on soft soil), plus the existing traffic-light tests. The full suite, Clippy and format pass on an export of exactly this commit.
- **Commit:** `cf70684`.
- **Limitations:**
  - The limit values are conservative choices for houses and small buildings, not values taken from a standard.
  - The structure window still lists the numbers of refused elements in its own tables (the traffic light, the report and MCP do not).

### #103 Pre-sizing report for professional review — ✅ Done

- **Changes:**
  - `src/io/report.rs` builds the report as numbered A4 pages and joins them into one PDF with `pdf-writer` (the version `svg2pdf` already uses), with the drawings appended at their own sheet size.
  - Report contents:
    - a cover with the project data, the overall traffic light and a review box for the professional (name, CREA/CAU, ART/RRT number, date, signature, opinion);
    - the inputs (building, rooms, materials and loads, supply);
    - the assumptions and applicability limits, with any refusal;
    - the slab, beam, column, footing and circuit tables with their lights, refused elements shown without numbers;
    - every check with its margin, what to change and the technical detail;
    - the warnings;
    - the formwork plan, beam forces, electrical plan and single-line diagram.
  - *Exportar memorial (PDF)* in the *Segurança* window saves it.
- **Tests:** 4 new tests (every section on valid, numbered SVG pages; the PDF page count with the drawings; a refused project stating the refusal with no structural numbers or drawings; the date). The report of the *Casa Térrea* sample (14 report pages and 4 drawings) was exported and its pages inspected as images. The full suite, Clippy and format pass on an export of exactly this commit.

### Synchronized multi-view tabs, cross-discipline dependencies and MCP upgrade — ✅ Done

- **Changes:**
  - Separated single-source-of-truth `SharedProject` (`Project`, `History`, `file_path`, `dirty`, `PlanLog`, `DependencySyncReport`) from lightweight `ViewTab` visual state (`discipline`, `floor_id`, `hidden_layers`, `selection`, `scale`, `pan`, `pane`) in `src/app/tabs.rs`, eliminating duplicate project copies across linked tabs.
  - Added `Discipline::Foundations` and prepared `Discipline::Plumbing` view overlays (`src/domain/discipline.rs`, `src/app/canvas.rs`) plus backward-compatible `Project.workspace_tabs` persistence so reopening a `.trazo` file restores all linked discipline tabs and split view.
  - Added `src/domain/dependencies.rs` (`propagate_dependencies`, `audit_dependencies`) and cached canvas review badges: wall-mounted electrical/sanitary fixtures safely follow moved walls (preserving IDs and conduits), footings follow moved columns, unsafe moves and structural off-axis shifts are preserved and flagged for manual review without auto-approving engineering checks.
  - Fixed file reopening deduplication (`open_in_tab` + MCP `abrir_projeto`) and last-link close safety (closing a secondary linked view never discards unsaved work; closing the last link prompts when dirty).
  - Upgraded MCP to 45 tools (`src/mcp/protocol.rs`, `src/app/mcp_tools/`): added `pavimentos` and `dependencias`, expanded `abas` with `nova_visao`/`vincular`/`configurar_visao`/`lado_a_lado`/`fechar` and project/tab hierarchy, added `projeto_id` safety guard, per-tab/discipline `imagem_da_planta` & `exportar_planta`, regional utility/soil/occupancy presets, secondary boards, beam-on-beam supports and upper-floor load reporting.
- **Tests:** 4 dependency unit tests (`domain::dependencies::tests`), 2 MCP synchronized-tab tests (`app::mcp_tools::tabs::tests`) and all 10 mandatory end-to-end scenarios (`app::tabs::synced_tabs_tests::scenario_01`–`scenario_10`); full suite (387 passed), Clippy (`-D warnings`) and `cargo fmt --check` pass on Windows.
- **Commits:** `ab7d9fc`, `480a71a`, `137ac4d`, `9cf57e8`, `bc97588`.
- **Limitations:**
  - `Discipline::Plumbing` highlights sanitary fixtures and flags wall/opening conflicts; hydraulic pipe routing and sizing remain scheduled for #72.
  - Side-by-side split view locks both panes to the active camera zoom/pan for visual alignment.
- **Commit:** `56d87c5`.
- **Limitations:**
  - There is no PDF viewer on the development machine. The PDF itself was checked by structure (page count) and the pages through their SVG, not opened in a reader.
  - The report has no MCP tool, since it does not change the model.
  - The cover date uses UTC.

### #56 Construction quantities — ✅ Done

- **Changes:**
  - `src/domain/quantities.rs` and `src/domain/finishes.rs` consolidate the construction takeoff for the active floor or across all floors (`construction_takeoff`, `construction_takeoff_all_floors`, `construction_takeoff_with_options`):
    - per wall: floor label, net area (using each wall's real height including sloped tops, #145), whole blocks, half blocks for the running bond, U-blocks of the bond-beam course, laying mortar, plaster on both faces and U-block grout fill (`0.55 · b · h` + 10 %);
    - lintels over doors and windows and sills under windows, with opening schedule codes (`P1`, `J1`, …), clear span, 30 cm bearing each side, U-blocks and grout fill;
    - finishes per room: floor with +10 % waste, skirting less door/open-span widths, net wall area using each enclosing wall's real height + 10 % waste, ceiling, NBR 9575 waterproofing allowance and per-room finish specifications;
    - per-masonry totals with bond-beam/lintel U-block breakdown, block and U-block waste from *Quadros* (or override), plus per-floor breakdown (`FloorTakeoffSummary`) in multi-floor mode.
  - ABNT sheet & printable outputs: `ScheduleConfig::show_quantities_schedule` (`build_quantities_schedule`, *Quadro de Quantitativos de Obra*) places the takeoff on ABNT scaled sheets (`scaled_sheet_svg`), and `takeoff_sheet_svg` renders a dedicated A3 landscape vector takeoff sheet (SVG/PDF) alongside UTF-8 BOM CSV (`takeoff_csv_for`).
  - UI & MCP: *Quantitativos da obra* (`src/app/ui/quantities.rs`) adds live block waste %, active-floor / all-floors scope toggle, sheet-inclusion toggle, and CSV/PDF/SVG exports; *Quadros* (`src/app/ui/schedules.rs`) adds the *Quadro de quantitativos* toggle; MCP exposes `quantitativos` (`todos_pavimentos`, `perda_blocos_pct`) and `exportar_planta` (`quantitativos_csv`, `quantitativos_svg`, `quantitativos_pdf`).
  - The method is documented in `docs/engineering.md`, `docs/user-guide.md`, `docs/mcp.md` and `CHANGELOG.md`.
- **Tests:** `the_takeoff_matches_a_manual_takeoff_within_2_percent` (a 4 × 3 m room with a door and a window, worked out by hand: net area, blocks, half blocks, bond-beam & lintel U-blocks with waste, grout, opening codes, lintels, mortar, plaster and room wall waste, all within 2 %), `each_kind_of_masonry_is_totalled_on_its_own_and_exports`, `multi_floor_takeoff_consolidates_all_floors_finishes_and_renders_sheet_svg` and `multi_floor_quantities_and_sheet_exports_work_via_mcp`.
- **Limitations:**
  - Axis lengths count wall corners twice (covered by the waste allowance).
  - The legacy *Quadro de Materiais* schedule still retains bug B20 (tracked separately).

### #53 Opening dimension chains — ✅ Done

- **Changes:** split `src/domain/dimensions/` into `mod.rs`, `chains.rs` and `tests.rs`; added `Project::generate_opening_dimension_chains` (associative `Aligned` chains `WallStart → Opening1.Left → Opening1.Right → … → WallEnd` with smart outward offset away from room interiors, degenerate-segment skip and idempotency) and one-click `Project::generate_site_dimensions` (internal room dimensions + opening chains); added rotated SVG dimension labels for vertical/angled chains (`src/io/export.rs`), UI buttons (*Cadeias de aberturas*, *Cotas completas*, per-wall chain) in `src/app/ui/dimensions.rs`, and MCP `cotas` actions `gerar_aberturas` and `gerar_completas`.
- **Tests:** multi-opening wall chain summing to exact wall length, outward placement on all 4 room walls, live associative updates on opening move/resize, angled walls, idempotency, and MCP round-trip.

### #113 Point loads — ✅ Done by another process

- Implemented and committed as `9ce409b` (see its entry above). Not reworked here.

### #94 2D frame analysis — ✅ Done

- **Changes:** added modular 2D Euler–Bernoulli Direct Stiffness Method solver in `src/domain/frame2d/` (`mod.rs`, `model.rs`, `solver.rs`, `builder.rs`, `diagrams.rs`, `tests.rs`): 3 DOF/node (`u, v, θ`) analyzing beams and columns together with real axial and in-plane flexural stiffness (`EA/L`, `EI/L`), static condensation for pinned end releases (`Rigid`, `PinnedStart`, `PinnedEnd`, `PinnedBoth`), fixed/pinned/roller supports, uniform & point member loads + NBR 6123 service lateral wind, exact Hermite + load-particular deformed shape, global equilibrium verification (`ΣFx, ΣFy, ΣM`), inter-storey lateral drift check (`H/850`), A3 vector diagram sheet (`frame_diagrams_svg`), dedicated *Pórticos 2D* UI tab (`src/app/ui/frames.rs`) with joint/base toggles and PDF/SVG export, PDF memorial table (`src/io/report.rs`), and MCP integration (`pre_dimensionamento.porticos_2d`, `pre_dimensionamento.falhas_porticos_2d`, `projeto_estrutural → configurar` `portico_nos_rigidos`/`portico_bases_engastadas`, `exportar_planta` `porticos_svg`/`porticos_pdf`). Failed eligible frames retain their label and solver diagnostic, appear as red safety failures in every output, and are never silently omitted.
- **Validation tests:**
  - `cantilever_column_matches_exact_moment_and_tip_deflection` (`M = P·h`, `Δ = Ph³/3EI`, `θ = Ph²/2EI`);
  - `frame_reduces_to_continuous_beam_when_column_stiffness_vanishes` (`I_col → 0` matches `M_B = -wL²/8`, `R = 3/8, 10/8, 3/8 wL`);
  - `symmetric_fixed_base_portal_gravity_matches_slope_deflection` and `symmetric_fixed_base_portal_lateral_sway_matches_slope_deflection` against closed-form slope-deflection equations;
  - `pinned_end_releases_zero_end_moments_and_match_simply_supported_deflection` (`5wL⁴/384EI`), `simply_supported_midspan_point_load_matches_exact_solution` (`PL/4`, `PL³/48EI`) and `project_frame_keeps_a_water_tank_load_on_its_beam` (load/reaction conservation);
  - `solver_and_project_report_expose_an_unstable_frame` (solver diagnostic retained in report warnings, red safety verdict and A3 diagram);
  - `project_2d_frames_satisfy_global_equilibrium_and_render_valid_svg` (`|ΣF| < 1e-6`, `|ΣM| < 1e-5`, rigid vs pinned joint toggle, valid SVG).

### #110 Reinforcement pre-sizing and steel schedule — ✅ Done

- **Changes:** preliminary longitudinal/transverse reinforcement for beams and columns, meshes for slabs and footings, NBR 6118:2023 §9.3–9.5 bond/anchorage (`fbd`, `lb`, hooked `lb,nec` with good/poor bond zones) and column lap splices (`l0`), B12 Branson cracked-section deflection metadata (`Mr`, Stage I/II, `I_eq/Ic`, `As`), reinforcement traffic lights (flexural domain, single-layer bar fit, shear strut `VRd2`, column steel ratio), bar shapes, CA-50 summary by diameter (`12 m` commercial bars and `+10%` cutting/bending waste), A3 schematic detailing sheet (`reinforcement_detail_svg` exported to SVG/PDF in UI and MCP `armaduras_svg`/`armaduras_pdf`), CSV schedule, PDF memorial tables and MCP output.
- **Tests:** bar geometry/mass, NBR 6118 bond/anchorage/lap hand calculation, agreement with Branson cracked-section steel, diameter-summary & 12 m bar reconciliation, valid A3 schematic detailing SVG, reinforcement traffic lights, MCP JSON/export and refusal outside applicability limits.
- **Note:** preliminary calculations are traced to ABNT NBR 6118:2023; executive fabrication drawings, openings/joints and any NBR 6118:2026 clause-by-clause audit remain the licensed engineer's responsibility (ART/RRT).

### #113 Concentrated loads — ✅ Done

- **Changes:** water-tank and heavy-equipment objects with editable footprint/load; direct beam or slab-to-nearest-edge load path; recalculation of beams, columns, footings and reinforcement; solid-slab local bending/punching, explicit lattice-slab refusal, plan labels, traffic light, PDF and MCP.
- **Tests:** exact load conservation through column reactions, increased beam demand, local slab/punching result, unsupported warning and direct-on-beam path.
- **Limitations:** nearest-edge distribution is preliminary and dynamic equipment effects remain input/professional responsibilities.

### What a real project still needs: the gourmet area — ⬜ Recorded

Laying out the structure of a real project through MCP (the gourmet area of *Casa com área de lazer*, 2026-11-21) showed what Trazo cannot yet compute. The design:

- a 5.00 × 14.75 m pavilion with a bathroom and pantry block;
- the left wall on the lot line, raised to about 4.30 m;
- the side facing the pool open, on timber posts (*mourões*) joined by a timber beam;
- no slab, but an exposed mono-pitch roof (*meia-água*) with visible tiles and battens, draining towards the pool.

What the model holds today:

- concrete columns about every 3 m on the left wall, with a 14 cm beam on top, the bathroom block columns and the footings;
- the posts, the timber beam and the roof slope as plain drawings, without any calculation.

| Gap | Why it matters here | Item |
| --- | --- | --- |
| Ground beams (*vigas baldrame*) and wall foundations | ✅ *Baldrames* puts ground beams under the left wall and strip footings under the bathroom and pantry walls | #143 |
| Boundary columns and footings | ✅ Columns stand flush inside the lot (B23) and *Vigas alavanca* centres their footings; the posts on the pool side still need concrete bases to anchor them | #144 |
| Wall height per wall, sloped wall tops | ✅ The left wall takes its own 4.30 m top and the gables a sloped one (Properties or `editar_parede`); the masonry, the wall loads and the column heights follow, and the slenderness check flags 14 cm columns in the tall wall (λ ≈ 106, use 20 cm). Sections and elevations still draw the storey height | #145 |
| Soil and foundation type | ✅ With the site's SPT, the soil pressure comes from the survey and the study says whether footings, a raft or brocas with ground beams suit the gourmet area | #112 |
| Roof structure and its loads | ✅ *Telhado* (or `definir_telhado`) frames the 34 % mono-pitch in C30 timber (battens, rafters, purlins, beams), counts tiles and timber, and puts each beam end on the column of the left wall (about 7.7 kN each); the low edge waits for the posts (#146) and the wind uplift for #147 | #114, #71 |
| Timber posts and beams | ✅ *Mourão* posts on the pool side take the roof's low edge, directly or through timber beams from post to post; species/class and treatment are specified, and each post is checked for compression, buckling and lateral wind (NBR 7190) over an anchored concrete base without soil contact | #146 |
| Wind on a tall, open-sided wall | ✅ *Vento* checks tall-wall columns (including footing eccentricity/bearing under wind), roof uplift with NBR 8800 anchor bolts, CA-50 X-bracing tie rods, NBR 16868 masonry panels with ties, crown beam and grouted vertical jambs beside openings, and four-direction global stability including NBR 6122 soil sliding (`FS ≥ 1.50`), α, drift and overturning | #147 |
| Boundary and neighbour rules | Openings in a boundary wall are limited (Código Civil art. 1.301); the roof must not drain onto the neighbour (art. 1.300) | #148 |
| Gutter towards the pool | Roof water would fall into the pool | #122 |
| The whole case as a reference | ✅ `examples/area-gourmet.json` (`SampleProject::AreaGourmet`), computed end to end and checked in CI (`gourmet_area_reference_project_computes_end_to_end_in_ci`) | #150 |

## Next up

The first ten items on the path, in order. The other items of each milestone fit in around them.

| Order | # | Item | Milestone | Why now |
| --- | --- | --- | --- | --- |
| 1 | #168 | Hypothesis provenance | M0 | Trazo cannot contest a hypothesis if it does not know who set it and on what evidence. |
| 2 | #169 | Contested hypotheses | M0 | Today an assistant can raise the soil pressure or lower a load through MCP and receive green: the most dangerous gap once the AI is at the centre. |
| 3 | #171 | Hypotheses need the person | M0 | The AI may propose a hypothesis; only the person, with evidence, may adopt it. |
| 4 | #160 | Safe defaults audit | M0 | A default that is too optimistic is a dangerous decision nobody sees. ✅ Soil 100 kPa, C20 and V0 45 m/s without data. |
| 5 | #170 | AI action report | M0 | An edit that makes the house less safe must be impossible to miss, for the assistant and for the person. |
| 6 | #159 | Where to get help | M0 | A red light with no way forward leaves the family alone. Cheap to build, and it changes every result. |
| 7 | #166 | From a description to a checked house | M0 | The family's way in: a conversation, not a drawing tool. |
| 8 | #142 | The assistant explains any result | M0 | Each result must reach the family as a sentence it understands, with the change that fixes it. |
| 9 | #165 | Building guide | M0 | What the family takes to the site: clear instructions to build, stage by stage. |
| 10 | #172 | Independent cross-checks | M0 | A second, simpler opinion catches a detailed simulation fed with the wrong geometry or loads. |

The rest of M0 (#161 plain language, #156 site risks, #157 building on top, #158 building in stages, #50 and #76 for the AI workflow, #65 and #66 for those who draw by hand, #162, #163, #164 and #167) fits in around them.

## Milestones

### M0: Protection for self-builders

The first priority, ahead of every other milestone: a family without engineering training describes its house to an AI assistant, the assistant designs it through MCP, Trazo checks and contests every decision, and the family receives clear instructions to build, is warned in words it understands before any dangerous decision, and knows where to find help.

- **Items:** #168, #169, #171, #160, #170, #159, #166, #142, #165, #172, #161, #156, #157, #158, #50, #76, #65, #66, #162, #163, #164, #167
- **Done when:**
  - a family describes its house in a conversation (lot, rooms, budget, future floor, site) and, through MCP only, receives a drawn and checked plan, a comparison of at least two layouts, a plain-language review and a step-by-step building guide, without touching a drawing tool;
  - an assistant that raises the soil pressure, raises the concrete strength, lowers a live load or picks a lighter room use without evidence never gets a green result: the tool answer, *Segurança* and the review report all show the contested hypothesis and the conservative value used instead;
  - every AI edit that turns a check worse, removes a structural element or changes a hypothesis is reported in its tool answer and on screen;
  - in a usability test with three people who never studied engineering or used a CAD, at least one of them planning to build their own house, each gets a checked two-bedroom house (by conversation or by drawing), reads every red result as one sentence, fixes it or understands why the work must stop, and names where to find free help, without assistance, in under 30 minutes;
  - a lot on a slope or next to a stream, or a second floor over an unknown foundation, is never sized silently: it ends in a red "stop and get help";
  - every refusal, every red result and the review report point to a free help path;
  - every value assumed for missing data is on the unfavourable side and listed as an assumption;
  - Trazo opens and draws the sample house smoothly on a 4 GB RAM laptop with integrated graphics.

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

- **Items:** #110, #111, #112, #113, #114, #115, #116, #117, #143, #144, #145, #146, #147, #150
- **Done when:**
  - from the sample house and a given soil survey, Trazo produces a formwork plan, a bar schedule, footings, lintels and a roof structure, each with its traffic light;
  - a water tank placed on a slab is checked;
  - reinforcement matches hand calculations of published examples within 5 %;
  - the gourmet reference project (#150) is computed end to end: ground beams, boundary footings with balance beams, the raised boundary wall under wind, the timber posts and beam, and the mono-pitch roof with its gutter.

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

### M10: MCP for the whole model

M0 puts the assistant at the centre for a family's first house; M10 extends it to every part of the model, with the person in control.

- **Items:** #74, #75, #46, #51, #49, #73, #148, #48, #52 (#142, #50 and #76 moved to M0)
- **Done when:**
  - an assistant draws a two-storey house with stairs, finishes, installations, structure and a sheet set **using MCP only**;
  - the person approves each proposed change, can compare and revert any AI action, and gets every warning and simulation explained in plain language with a suggested fix.

### M11: Depth on demand

Simple by default for families (M0); depth on demand for the professionals who review their projects.

- **Items:** #67, #68, #69, #82 (#65 and #66 moved to M0)
- **Done when:**
  - a guided documentation flow builds and reviews a sheet set before export;
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

### M14: Executive & municipal architectural sheets

Everything needed to assemble a complete Brazilian municipal permit / executive architectural sheet (*projeto legal / prancha de prefeitura*) inside Trazo without finishing in external CAD/BIM software.

- **Items:** #71, #85, #151, #152, #153, #154, #155
- **Done when:**
  - from a residential project, Trazo composes a single A1/A0 ABNT sheet combining a `1:50` dimensioned floor plan, executive `1:50` cross-sections (`Corte AA` and `Corte BB` with foundations, earth fill, timber roof trusses, visible tile profile, water-tank enclosure, wet-area wall tile hatches, vertical dimensions and callout leaders), a `1:200` site & roof plan (*Implantação e Cobertura*), a `1:1000` city-block location plan (*Planta de Situação*), urban occupancy/permeability/openings/specification tables, a numbered schematic area/ridge-post breakdown and a clean architectural 3D exterior perspective above the ABNT title block;
  - every viewport updates live from the model and exports identically to vector PDF, SVG and DXF.

### Exploratory

Worth prototyping when the core is solid: #85, #86, #109, #139.

## Backlog

### 0. Protection for self-builders

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 156 | Site risk questions before any structure is sized: slope, a nearby stream or canal, landfill, water in the ground, cracks in neighbouring houses, digging next to a neighbour's wall. A risky answer turns the foundation red with "stop and get help" (Defesa Civil 199, a professional) instead of sizing it | P0 | M | M0 | ✅ |
| 157 | Building on top of an existing house: an upper floor over a ground floor whose foundation and columns are unknown is refused until they are surveyed, with plain-language guidance on what to find out and how | P0 | M | M0 | ✅ |
| 158 | Building in stages: plan the future floor today so the first stage's footings, columns and slab already carry it, and warn when an extension exceeds what the house was sized for | P0 | M | M0 | ✅ One drawn storey plus up to two future floors |
| 159 | Where to get help: every red result, every refusal and the review report name the free paths (ATHIS under Lei 11.888/2008, through the municipality, CAU or CREA; university model offices; Defesa Civil 199), with a regional list editable through #140 | P0 | S | M0 | ✅ Local contacts per project; regional packs wait for #140 |
| 160 | Safe defaults audit: every value Trazo assumes when data is missing takes the unfavourable side and is listed as an assumption; soil 100 kPa without a survey, C20 concrete and V0 45 m/s while the city is unknown | P0 | S | M0 | ✅ |
| 161 | Plain-language pass: every message, check and refusal has a one-sentence version that a person without engineering training understands, technical terms are explained on hover, and the texts are reviewed with self-builders | P0 | M | M0 | 🟡 Glossary on hover and in MCP, a one-sentence family summary; review with self-builders remains |
| 162 | Before-the-pour checklists: printable, plain-language checklists for each concrete pour (footing, column, beam, slab) taken from the bar schedule: bar count and diameter, cover, formwork, shoring and curing time | P1 | M | M0 | ✅ |
| 163 | Family booklet: the plan, the shopping list, the traffic-light summary and "when to stop and get help" on a few A4 pages to take to the hardware store, the builder and the reviewer | P1 | M | M0 | ✅ |
| 164 | Modest computers: smooth on a 4 GB RAM laptop with integrated graphics, measured with `trazo --bench` | P1 | M | M0 | ⬜ |
| 165 | Building guide: step-by-step instructions to build the checked house, in plain language and in order (setting out, foundations, columns and beams, slab, walls, roof, electrical), each stage with its materials, its checklist (#162) and the points where a professional should look; produced through MCP from the model | P0 | L | M0 | ✅ |
| 166 | From a description to a checked house: an MCP prompt and workflow where the assistant turns a family's description (lot, rooms, budget, future floor, site risks) into a drawn plan, compares alternatives (#50), runs every check, fixes what it can and reports in plain language what it cannot, without the person touching the drawing tools (builds on #76) | P0 | L | M0 | ⬜ |
| 167 | Affordable AI: the guidance works with free assistant tiers and local models, and every check, refusal and contestation keeps working with no AI connected | P1 | M | M0 | ⬜ |
| 168 | Hypothesis provenance: every structural and electrical hypothesis records who set it (Trazo's default, the person, the AI) and its evidence (a soil survey, a specification, the city's wind map), shown in *Segurança*, the review report and every MCP result | P0 | M | M0 | ✅ |
| 169 | Contested hypotheses: a result that depends on a hypothesis more favourable than the conservative default without evidence is shown as contested (never green); NBR minimums for loads and the regional wind cannot be lowered; implausible combinations are flagged (for example, a soil pressure that contradicts the survey's soil type) | P0 | M | M0 | ✅ |
| 170 | AI action report: each MCP edit returns and shows what it made worse (checks that changed colour, structural elements removed, hypotheses changed), so neither the assistant nor the person can miss it | P0 | S | M0 | ✅ |
| 171 | Hypotheses need the person: an assistant's change to a hypothesis is proposed, not applied, until the person confirms it in the window with its evidence (the first use of approval mode, #46) | P0 | M | M0 | ✅ |
| 172 | Independent cross-checks: simplified global checks (total weight against the sum of foundation capacities, span-to-depth rules for beams and slabs, column load per floor) contest a detailed simulation that disagrees beyond a tolerance | P1 | M | M0 | ✅ Weight and foundation capacity, one real storey (with the *floors* multiplier) |

### A. Construction site

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 53 | Automatic dimension chains locating each door and window along its wall, plus internal room dimensions | P0 | M | M2 | ✅ |
| 54 | First-course plan and wall elevations with block paging: whole, half and cut blocks, lintels and sills | P1 | L | M2 | 🟡 Properties show each wall's first course; there is no drawing |
| 55 | Setting-out plan (*locação*): cumulative dimensions from an origin on the lot, for marking out the building | P1 | M | M2 | ⬜ |
| 56 | Construction quantities: blocks with waste, half blocks and U-blocks, lintels per opening, laying mortar, plaster, flooring and wall finishes per room | P0 | M | M2 | ✅ |
| 57 | One-click construction set: A3 at 1:50 with dimensions, openings, schedules, first course, setting-out and legend | P0 | M | M2 | 🟡 Sheets, scales, templates and title blocks exist |
| 71 | Roof plan (*planta de cobertura*): roof planes (mono-pitch, gable, hip), slopes with the tile's minimum, ridges, gutters and downpipes | P1 | L | M2 | ⬜ |

### B. Engineering safety and trust

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 99 | Load path across floors: columns stacked through the storeys, loads summed from the real floors above, misaligned columns flagged as transfers (B03) | P0 | M | M3 | ✅ |
| 100 | Engineering methods guide (`docs/engineering.md`): every calculation with its formula, standard, assumptions and limits, validated in CI against published worked examples (B12) | P0 | M | M3 | ✅ |
| 101 | Results as a traffic light: margin to the limit, the assumptions used, and a plain-language explanation of what to change | P0 | M | M3 | ✅ |
| 102 | Applicability limits per module (storeys, spans, loads, soils); outside them Trazo refuses to size and asks for a professional | P0 | S | M3 | ✅ |
| 103 | Pre-sizing report (*memorial*) as PDF: inputs, assumptions, results, warnings and drawings, for a licensed professional to review and sign (ART or RRT) | P0 | M | M3 | ✅ |
| 104 | Release gate: a sheet set cannot be marked *Liberado para obra* with open red results unless a reviewer (name and CREA/CAU number) signs them off | P1 | S | M3 | ✅ |

### C. Simulation core

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 94 | 2D frame analysis: beams and columns together (column stiffness, rigid and pinned joints), internal forces, reactions and the deformed shape | P0 | XL | M4 | ✅ |
| 105 | Load cases and combinations: NBR 6120 loads, NBR 8681 ultimate and service combinations, and wind on the building (NBR 6123) | P1 | L | M4 | ✅ |
| 106 | Results on the plan and on paper: diagrams over the beams, deformed shape, utilisation colours and an animated load path from slab to soil, with labels that avoid each other (B05) | P1 | M | M4 | ✅ |
| 107 | What-if mode: sliders for span, section, load and material, results updating live and the safe range highlighted | P1 | M | M4 | ✅ |
| 108 | Analysis in the background with cached results and a shared font database, so large projects stay responsive (B04, B17) | P1 | M | M4 | ✅ |
| 109 | Grid analysis of slabs and beams together (grid analogy), including L-shaped and irregular slabs (B06) | P2 | XL | Exploratory | ✅ |

### D. Structure from foundation to roof

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 110 | Reinforcement pre-sizing for beams, columns, slabs and footings, with a bar schedule (*tabela de aço*), steel weights and cracked-section deflection (B12) | P1 | L | M5 | ✅ |
| 111 | Lintels and sills (*vergas e contravergas*) for every opening, and masonry checks for infill and load-bearing walls (NBR 16868) | P1 | M | M5 | ✅ |
| 112 | Foundations: soil from a survey (SPT) or a soil-type table; footing, raft (*radier*) and bored-pile (*broca*) options; eccentricity, punching and overturning checks (B11) | P1 | L | M5 | ✅ |
| 113 | Point loads on the structure: water tank (*caixa d'água*) and heavy equipment, checked on the slab or beam below | P0 | S | M5 | ✅ |
| 114 | Roof structure: mono-pitch (*meia-água*), gable and hip roofs in timber, with rafters (*caibros*), purlins (*terças*), battens (*ripas*), wall plates (*frechais*) and trusses, an exposed-ceiling option, the minimum slope of each tile, tile weights, live load and wind uplift (NBR 7190, NBR 6120, NBR 6123), and the roof loads delivered to the walls, beams, posts and footings, from the roof plan (#71) | P1 | XL | M5 | ✅ |
| 115 | Stairs, balconies and cantilevers: slab and beam checks with the extra care cantilevers need | P1 | M | M5 | ✅ |
| 116 | Two-way slab moments and deflections by tables (Czerny, Marcus), and L-shaped rooms split into rectangular panels (B06) | P1 | M | M5 | ✅ |
| 117 | Columns at any angle and beams supported on beams (B07) | P2 | M | M5 | ✅ |
| 143 | Ground beams (*vigas baldrame*) and wall foundations: a ground beam under every wall, resting on the footings, or a strip footing (*sapata corrida*) where there are no columns; the wall weight carried down to them; interior walls (bathrooms, pantry) included; beams and footings sized with that load | P0 | L | M5 | ✅ Ground beams between columns and strip footings elsewhere, wall load to the footings (and to balance beams), sizing with steel, plan drawing, MCP and traffic light; overhang moments enter the continuous spans and signed reactions flag uplift; secondary beams transfer point reactions through supported beam chains to columns and footings; unsupported cycles fall back to strip footings |
| 144 | Boundary foundations: columns and footings on a boundary wall placed flush inside the lot (B23); eccentric boundary footings balanced by a balance beam (*viga alavanca*) to an interior footing, or a strip footing, with the soil and overturning checks of the pair (NBR 6122) | P0 | M | M5 | ✅ |
| 145 | Wall height per wall and sloped wall tops (gable ends, *oitões*, parapets), with the storey height as the default; sections, elevations, quantities, structure and the limits use each wall's real height | P1 | M | M5 | ✅ |
| 146 | Timber structural elements: posts (*mourões*) and beams of timber, with species and strength class (NBR 7190), bending, shear, compression, buckling and deflection checks, durability (treated wood, no soil contact) and post bases (concrete block with a steel anchor) | P1 | L | M5 | ✅ |
| 147 | Tall and free-standing walls under wind: wind pressure on walls and open-sided buildings (NBR 6123: basic speed by region, terrain, openings), masonry panel slenderness and the bracing it asks for (column spacing, intermediate bond beam, thickness), for boundary walls and walls raised under a roof | P1 | M | M5 | ✅ |
| 150 | Reference project: the semi-open gourmet area (5 × 14.75 m, boundary wall raised under a mono-pitch roof, timber posts, exposed roof draining to the pool) in `examples/`, computed end to end and checked in CI once #114 and #143–#148 ship | P1 | S | M5 | ✅ |

### E. Water, sewage and rain

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 72 | Plumbing points (water, drains, gas) with legend, counts and a fixture schedule | P1 | M | M6 | ⬜ |
| 96 | Discipline views: architecture, electrical, plumbing and structure as views of one model, each with its own visibility, colours and sheets | P1 | M | M6 | 🟡 Architecture, electrical and structure on screen and per sheet; plumbing joins with #72 |
| 118 | Cold water (NBR 5626): daily consumption, water tank volume, pipe diameters by fixture weights and the pressure left at each fixture, so a weak shower shows up before it is built | P1 | L | M6 | ⬜ |
| 119 | Hot water: electric, gas and solar heater sizing | P2 | M | M6 | ⬜ |
| 120 | Sewage (NBR 8160): fixture units, pipe diameters and slopes, inspection boxes and vents | P1 | L | M6 | ⬜ |
| 121 | Septic tank, filter and soakaway (NBR 7229, NBR 13969), with the distances to wells and boundaries | P1 | M | M6 | ⬜ |
| 122 | Rainwater (NBR 10844): gutters and downpipes from the roof area and local rainfall, and a cistern for reuse; a warning when a roof drains into a pool or onto a neighbour | P1 | M | M6 | ⬜ |
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
| 46 | Approval mode: the assistant proposes, the plan shows a preview, the person accepts or rejects (hypothesis changes come first, #171) | P1 | M | M10 | ⬜ |
| 51 | AI history with a diff per action, comparison and selective revert | P1 | M | M10 | 🟡 Live highlight, action log, revisions and version comparison (#45) exist |
| 49 | Plan validation with explanations and one-click fixes: unclosed rooms, overlaps, rooms without doors, unreachable spaces | P1 | M | M10 | 🟡 The circulation check (#25) covers clearances and door swings |
| 73 | Building-code checks: minimum room areas and widths, lighting and ventilation ratios, door widths; rule sets configurable per municipality | P1 | L | M10 | ⬜ |
| 148 | Boundary and neighbour rules: openings in walls less than 1.5 m from the boundary (Código Civil art. 1.301; only ventilation openings up to 10 × 20 cm above 2 m), roofs that drain onto a neighbour (art. 1.300) and municipal limits on boundary wall height, as traffic-light checks | P1 | M | M10 | ⬜ |
| 48 | Built-in assistant panel: natural-language commands inside Trazo, using the user's own AI provider | P1 | L | M10 | 🟡 Works today through external assistants over MCP |
| 76 | MCP prompts (for example "draw a house from this brief") and richer resources: sheet images, schedules | P0 | S | M0 | ⬜ |
| 50 | Room layout proposals from a brief (rooms, areas, adjacencies), shown as alternatives the assistant compares with the family | P1 | L | M0 | ⬜ |
| 52 | An assistant that teaches the editor: context tips and guided exercises | P2 | M | M10 | ⬜ |
| 142 | Explain any result: the assistant reads a simulation or warning, explains it in plain language and proposes the change that turns it green | P0 | M | M0 | 🟡 The MCP server instructions already ask assistants to speak simply, never play down red and point to help |

### J. Experience

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 65 | Simple, Intermediate and Professional interface levels on the same project and file, engineering results included; **Simple is the default** | P0 | L | M0 | ⬜ |
| 66 | Guided flow *Minha primeira casa*: lot, desired rooms, adjustable layout, openings, structure, the site risk questions and a final check that ends with where to get help | P0 | L | M0 | ⬜ |
| 67 | Guided documentation flow: build and review a sheet set before exporting | P2 | M | M11 | ⬜ |
| 68 | English interface (internationalisation), keeping Brazilian Portuguese as the default | P1 | L | M11 | ⬜ |
| 69 | Command line and command palette with typed commands, aliases and recent commands | P1 | M | M11 | ⬜ |
| 82 | Dark theme, interface scale and full keyboard navigation | P2 | M | M11 | ⬜ |

Trazo opens at the Simple level, because its first users are families, not engineers. A chosen level must never be a wall: a family can open an advanced tool when needed, and a reviewing professional keeps the simple actions. For example, *check this beam*:

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

### N. Executive & municipal architectural sheets

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 151 | Multi-viewport sheet composer (*montador de prancha multi-escala*): free Paper Space / Sheet layout placing and resizing multiple live viewports at different scales (`1:50`, `1:200`, `1:1000`) alongside schedules, legends, callout diagrams and the ABNT title block on a single A1/A0/A2/A3 sheet, exported identically to SVG, PDF and DXF | P0 | XL | M14 | ⬜ |
| 152 | Executive architectural sections & elevations (*cortes e fachadas executivos*): upgrade automatic `Corte AA` / `Corte BB` generation with foundation footings/ground beams, hatched earth fill and subfloor, detailed timber roof trusses/rafters/battens, visible roof-tile elevation profile, elevated water-tank (*caixa d'água*) enclosure and slab, wet-area wall tile hatches, interior doors/windows in section/elevation, vertical dimensions (floor-to-ceiling, sills, ridge) and automatic construction callout leaders | P0 | XL | M14 | 🟡 Schematic sections and elevations exist (#35) |
| 153 | Site, roof & location plans (*Implantação/Cobertura 1:200 e Planta de Situação 1:1000*): dedicated *Implantação e Cobertura* viewport (lot, setback chains, roof tile hatch, slope arrows, gutters, sidewalk/curb, street name and north arrow, building on #71) plus a parametric *Planta de Situação* generator (city block grid, neighbouring lots, highlighted lot *IMÓVEL*, street names and corner distance) | P1 | L | M14 | 🟡 Lot, setbacks and roof geometry exist |
| 154 | Urban occupancy parameters, technical specification schedule & schematic breakdown (*parâmetros urbanísticos e tabelas de prefeitura*): *Tabela de Parâmetros de Ocupação do Solo* with Floor Area Ratio (*Índice / Coeficiente de Aproveitamento*), Lot Coverage (*Taxa de Ocupação*) and explicit Permeable / Infiltration Area & Rate (*Taxa de Infiltração / Permeabilidade*), plus a *Tabela de Especificações Técnicas* by building system and numbered area / ridge-post height (*altura de pontaletes*) schematic breakdown | P1 | M | M14 | 🟡 Area, openings and finishes schedules exist |
| 155 | Architectural 3D perspective for sheets (*perspectiva 3D arquitetônica*): clean hidden-line / technical shaded 3D exterior view generated from the model (extruded walls, openings, pitched roofs with overhangs, sun shadows and optional schematic entourage, extending #85) placeable directly as a viewport above the ABNT title block | P2 | XL | M14 | ⬜ |

## Definition of done

An item is done when:

- someone without engineering training can understand what it does and every message it shows (the plain sentence comes first);
- it works on screen **and** in every export it touches (screen equals paper);
- it has tests, including a regression test for each bug fixed along the way;
- assistants can use it through MCP, when it changes the model;
- the [user guide](docs/user-guide.md), the [MCP guide](docs/mcp.md) and the [changelog](CHANGELOG.md) are updated;
- `cargo fmt`, `cargo clippy -- -D warnings` and `cargo test` pass on Windows, Linux and macOS.

An **engineering or simulation** item also needs:

- its method in `docs/engineering.md`: formula, standard, assumptions and limits;
- at least two published worked examples reproduced in tests, or agreement with a reference program;
- results that follow the [safety contract](#engineering-in-trazo-the-safety-contract): traffic light, margin, plain-language explanation, refusal outside the limits and a way forward (what to change or where to get help).

## Out of scope

Trazo protects; it does not certify. To stay an approachable tool that helps families build more safely, these are not planned:

- signing, approving or replacing a licensed professional: Trazo never issues a final design, an ART or RRT, or a permit;
- deciding whether a site is safe to build on (slopes, flood zones, risk areas): Trazo cannot see the ground, so it asks and sends these cases to the Defesa Civil and to professionals (#156);
- buildings beyond the limits of #102, such as more than three storeys, spans over 7 m, prestressed or special structures, or retaining walls taller than 1.5 m. These belong to professionals and dedicated software, and Trazo says so;
- photorealistic 3D rendering (a simple massing view is #85 and clean technical 3D perspective for sheets is #155);
- full BIM authoring (IFC export is exploratory, #86);
- fire-safety, HVAC and gas-piping design (gas points are part of #72);
- real-time cloud collaboration between people (AI collaboration is in scope).

Want to suggest a change to the roadmap? Open a [feature request](https://github.com/igors93/Trazo/issues/new?template=feature_request.yml) and explain the problem it solves.
