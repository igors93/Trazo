# Changelog

All notable changes to Trazo are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions will follow [Semantic Versioning](https://semver.org) from the first tagged release. Numbers such as (#38) refer to [roadmap](ROADMAP.md) items.

## [Unreleased]

Everything below is on `main` and will become the first release, 0.1.0.

### Added

- Hardened compilation safety & faster debug builds (`Cargo.toml`, `clippy.toml`, T-01): enforced `unsafe_code = "forbid"` (replacing manual Win32 `BCryptGenRandom` FFI in `src/mcp/tunnel.rs` with `getrandom`) and `clippy::{unwrap_used, expect_used, panic, todo, unimplemented, unreachable} = "deny"` across all production code (47 sites audited and hardened), plus `opt-level = 2` in `[profile.dev.package.*]` for heavy rasterisation/PDF/image dependencies.
- Reference semi-open gourmet area project (`examples/area-gourmet.json`, `SampleProject::AreaGourmet`, #150): canonical 5.00 × 14.75 m pavilion on a 10 × 50 m lot against the left boundary with bathroom/pantry block, 4.30 m tall boundary wall, sloped gables, SPT soil survey (`SP-01`), ground beams + strip footings, boundary columns flush inside the lot with balance beams, treated Cumaru timber posts and eave beams facing the pool, exposed 34 % mono-pitch ceramic roof in C30 timber with 50 cm overhang, and full NBR 6123 wind checks, embedded in *Novo a partir de modelo...* and validated end to end in CI (`gourmet_area_reference_project_computes_end_to_end_in_ci`).
- Tall and free-standing walls under wind completion (`src/domain/wind.rs`, `src/domain/masonry_structure.rs`, #147): added NBR 6122 base-to-soil sliding checks (`FS = μ·(0.9·G − 1.4·U)/W_k ≥ 1.50`) across ±X/±Y global stability, preliminary CA-50 45° X-bracing tie-rod sizing (`size_bracing_tie`), NBR 8800 §6.3.3 combined uplift+shear SAE 1020 anchor-bolt sizing (`size_anchor_bolts`), tall-wall column footing wind eccentricity (`e ≤ B/3`) and bearing checks (`σmax ≤ 1.3·σadm`), and sized grouted vertical concrete jambs (*montantes grauteados*) beside openings in tall wind-loaded masonry panels, wired into traffic lights, *Telhado → Vento* UI, PDF memorial and MCP.
- Stairs, balconies and cantilevers (`src/domain/cantilevers.rs`, #115): structural pre-sizing of drawn concrete stairs (`h_eq = h_waist/cosα + e/2`, NBR 6120 2.5/3.0 kN/m², CA-50 steel & `L/250` deflection), balcony/cantilever slabs (NBR 6118 §13.2.4.1 Table 13.2 `γ_n = 1.95 − 0.05·h_cm`, guardrail line load + `0.80 kN/m` thrust, mandatory solid-slab check, top negative steel, `L/125` deflection and `1.5·L_bal` back-span overturning check) and cantilever beams (`L/5` min depth, `L/125` deflection, top steel & back-span), surfaced in *Estrutura*, traffic lights and MCP (`definir_balanco`, `remover_balanco`, `pre_dimensionamento.escadas_e_balancos`).
- Complete timber roof structures (`src/domain/roof/`, #114): added **gable (*duas águas*)** and **hip (*quatro águas*)** roofs alongside mono-pitch (*meia-água*), perimeter **overhangs (*beirais*)** with cantilever bending checks, **wall plates (*frechais*)**, 4-panel **Howe pitched timber trusses (*tesouras*)** sized to NBR 7190 (tension, compression & Perry-Robertson buckling), diagonal **hip rafters (*espigões*)**, integrated **NBR 6123 wind uplift** & anchor demand, ridge/hip/overhang plan marks, UI controls (*Telhado* tab) and MCP (`definir_telhado` `estilo`/`beiral_m`/`tesouras`).
- Simulation core completion (`#106`, `#107`, `#108`, `#109`):
  - **On-plan & on-paper simulation overlays** (`src/domain/plan_results.rs`, `src/app/canvas/sim_overlay.rs`, `src/io/export/conduits.rs`): traffic-light utilisation colours on slabs/beams/columns/footings, bending-moment (`M`) and shear (`V`) diagrams directly over beams, exaggerated deformed shapes (`L/xxx`), animated slab-to-soil load path on screen (directional arrows on paper), overlap-free labels (B05), ribbon controls and MCP `projeto_estrutural` → `configurar_sobreposicao` (#106).
  - **Interactive What-if simulator** (`src/domain/whatif.rs`, `src/app/ui/whatif.rs`): non-destructive live sliders for span scale, beam/column/slab sections, live load, `fck`, soil pressure, wind `V0` and occupancy, updating in `< 100 ms` with painted green/yellow/red safe-range bars, baseline vs. simulated deltas, one-click Apply and MCP `simular_whatif` (#107).
  - **Background analysis worker** (`src/domain/background.rs`, `src/domain/cache.rs`): lazily-spawned off-UI worker thread with latest-wins job coalescing populating the shared content-fingerprinted caches (B04) alongside shared `OnceLock` font databases (B17) and status-bar progress feedback (#108).
  - **Coupled slab + beam grid analysis** (`src/domain/grid_analysis/`, `src/app/ui/grid.rs`): 3-DOF/node grillage FEM (*analogia de grelha*, NBR 6118 §14.7.5) with banded Cholesky solver handling rectangular, L-shaped and irregular slabs (B06) coupled with beams and columns, validated against Timoshenko Table 8 square and 2:1 plates, with A3 SVG/PDF deflection heatmap (`grelha_svg`/`grelha_pdf`), UI *Grelha* tab and MCP `pre_dimensionamento.grelha_lajes_vigas` (#109).
- Construction quantities takeoff (`src/domain/quantities.rs`, `src/domain/finishes.rs`): multi-floor or active-floor consolidation (`construction_takeoff_all_floors` / `construction_takeoff_with_options`), U-block breakdown into bond beams and lintels/sills with waste, U-block grout fill (`0.55 · b · h` + 10%), opening schedule codes (`P1`/`J1`) and span/bearing detail on lintels and sills, per-wall real heights in room finishes with +10% wall finish waste, NBR 9575 waterproofing allowance and room finish specs, compact ABNT sheet schedule (`ScheduleConfig::show_quantities_schedule`), printable A3 vector sheet (`takeoff_sheet_svg`, SVG/PDF), UTF-8 BOM CSV, live waste % and scope controls in *Quantitativos da obra*, and MCP `quantitativos` (`todos_pavimentos`, `perda_blocos_pct`) + `exportar_planta` (`quantitativos_svg`, `quantitativos_pdf`, `quantitativos_csv`) (#56).
- Load cases, combinations and whole-building wind (`src/domain/loads/`): ABNT NBR 6120:2019 load cases (`G1` structural self-weight, `G2` non-structural permanent, `Q` live loads with per-room Table 10 classification and occupancy-dependent `ψ0/ψ1/ψ2`), ABNT NBR 6123 whole-building wind across `+X`, `−X`, `+Y`, `−Y` (Tables 1–4 `S1/S2/S3/Cpe/Ca`, storey-by-storey forces, base shear, overturning and `ELU-EQU` screening), and all 17 ABNT NBR 8681:2003 / NBR 6118:2023 §11.8 ELU and ELS combinations integrated by exact linear superposition into 2D plane frames (`envoltoria_elu`/`envoltoria_els`, `ELS-F` drift `H/850`, `ELU-6/7` column uplift), with the new *Cargas e combinações* UI tab, A3 frame diagrams, PDF memorial tables, traffic lights and MCP `pre_dimensionamento.cargas_e_combinacoes` (#105).
- Global wind-stability screening in both plan axes and both wind signs now reports lateral force, overturning balance, the NBR 6118 α indicator and lateral drift. Roof bearing only braces a tall-wall column when a declared lateral bracing path has enough design resistance and two grounded supports (#147).
- Tall masonry walls now receive an out-of-plane wind check and a preliminary design of intermediate reinforced-concrete ties plus a crown beam, including spacing, section, steel, plan mark, UI, PDF, MCP and traffic lights (#147).
- Ground beams now connect at transverse T-junctions; secondary beams pass point reactions through chains of ground beams to columns and footings, with a red fallback for unsupported requested beams (#143).

**Drawing engine**

- `f64` geometry with controlled tolerances (#01), free-shape rooms (#02) and direct vertex editing (#10).
- Object snaps: end, middle, intersection, perpendicular, tangent, centre and extension (#03). Typed distances, angles and coordinates while drawing (#04).
- Copy, paste, move, rotate, mirror, offset, trim and extend (#05); divide, join, align, distribute, group and repeat (#06).
- 2D drafting entities: line, polyline, rectangle, arc, circle and spline (#07).
- Geometric constraints: parallel, perpendicular, coincident and fixed distance, kept after every edit (#08).
- Multiple selection by click, window, crossing and filters (#09).

**Architecture**

- Wall junctions at any angle (#11), shared walls that move with both rooms (#12), and walls referenced by axis or by face (#13).
- Room detection from enclosing walls (#14), irregular lots and subdivision (#15), setbacks and buildable limit (#16), and usable, built, projection and lot areas (#17).
- Masonry with real bricks and blocks, modulation and block estimates, plus multilayer walls (#18).
- Door types: hinged, double, sliding, pocket, pivot, bifold and revolving (#19). Window types: sliding, top-hung, awning, casement, tilt-and-turn, fixed and bay (#20).
- Stairs, NBR 9050 ramps, landings, railings and level changes (#21); multiple floors with levels, ceiling heights and an underlay of the floor below (#22).
- A customisable library of furniture, equipment and symbols (#23); per-room finishes, baseboards, waterproofing and specifications (#24).
- Circulation, interference, door-swing and furniture-clearance checks (#25).

**Documentation and output**

- Sheet release gate (#104): red engineering checks require a dated, named CREA/CAU review for *Liberado para obra*. The approval is bound to the exact failed checks and sheet revision; changed results or a new revision revert the effective status to *Para aprovação* on screen, in the master list and in SVG/PDF exports. Valid reviewer details appear in the sheet and master list.
- Associative dimensions (#26), automatic opening dimension chains along walls + one-click site dimensions (#53), layers (#27), sheets from A4 to A0 (#28), exact print scales (#29), title block and pagination (#30).
- Texts, notes, leaders and symbols (#31), hatches by material (#32), templates (#33) and configurable schedules (#34).
- Sections, elevations and details with cross-references (#35), sheet sets with revisions (#36), and ABNT NBR 6492 and NBR 16752 documentation styles (#37).
- PDF and PNG export, editable SVG (#41) and CSV schedules (#42).
- AutoCAD DXF import and export (`AC1009` / R12 in metres, with `$INSUNITS` scaling on import from mm/cm/m/in/ft): walls, openings, fixtures, 2D drawings, associative dimensions (`DIMENSION` with `*D<n>` blocks), texts/MTEXT and hatches, preserving layers, ACI colours and linetypes, plus `exportar_planta` (`dxf`) and `importar_dxf` MCP tools (#38).

**AI assistants (MCP)**

- An MCP server with 35 tools over streamable HTTP, a `--mcp-stdio` bridge and one-click Claude Desktop setup.
- A public tunnel for cloud assistants, protected by a token in the address that changes on every opening (#47).
- `executar_lote`, which runs many tools as one all-or-nothing undo step, plus CAD editing tools: transform, align, distribute, group, repeat, trim, extend, offset, divide, join and constraints.
- Real-time collaboration: plan revisions, `mudancas_desde`, the `trazo://planta` resource and change notifications on the event stream.
- A live highlight of each AI edit, a "working" badge, *Acompanhar a IA* and an action log (part of #51).

**Project**

- Continuous integration with formatting, Clippy and tests (#58), geometry property tests (#60), and a versioned file format with tested migrations (#64).
- Dual licence, MIT or Apache-2.0 (#59), contributing guide, code of conduct, security policy, and issue and pull request templates.
- Tagged GitHub Release pipeline with Windows, Linux and macOS archives, `SHA256SUMS.txt` checksums and extracted changelog notes (#77).
- Rotating crash log (`crash.log` via panic hook) and *Ajuda → Relatar problema...* dialog with one-click pre-filled GitHub bug report and diagnostic bundle (#79).
- Windows Inno Setup installer (`installer/trazo.iss`), dedicated `.trazo` association and non-destructive `.json` *Open with* support (with CLI startup open for double-click), and non-blocking GitHub Releases update check under *Ajuda → Verificar atualizações...* (#63).
- Three sample projects in `examples/` (`casa-terrea.json`, `sobrado.json`, `apartamento.json`), embedded in *Novo a partir de modelo...* and validated by automated load and export tests (#81).

**Electrical design, gates and multiple projects**

- An *Elétrica* tab for the electrical design on the floor plan (ABNT NBR 5410): outlets at three heights and for specific use, switches, ceiling and wall lights, distribution board and data points, on a layer of their own.
- Minimum points generated per room from the norm, automatic circuits, and a load schedule with current, wire section, breaker and RCD, plus warnings. Symbols and tags are the same on screen and in exports; *Só elétrica* fades the architecture.
- Four MCP tools for the electrical design: `inserir_ponto_eletrico`, `editar_ponto_eletrico`, `projeto_eletrico` and `quadro_de_cargas`.
- Conduits (*eletrodutos*) from the devices to the board, drawn as arcs (solid in the ceiling, dashed in the floor) with one tick per conductor (phase, neutral, return, earth) and the circuit numbers. They can be laid automatically from the board or joined by hand, are sized by the NBR 5410 fill limits, and go to every export, DXF included (#88).
- A distribution board computed from the circuits: phase balancing for two- and three-phase supplies (#91), voltage drop along the conduits with wires raised to keep it within 4 %, demand factors and service entrance sizing (main breaker, feeder and conduit) (#90), and a single-line diagram with the panel schedule, exported to PDF and SVG, plus a compact load table on electrical sheets (#89).
- `projeto_eletrico` lays, joins, moves and removes conduits and sets the supply; `quadro_de_cargas` returns phases, voltage drop, demand, the entrance and the wires of each conduit.

**Structure and discipline views**

- An *Estrutura* tab with columns and footings that snap onto wall junctions, beams between columns and slabs over rooms, drawn as a formwork plan on a layer of their own with P, V, L and S labels and sections. **Gerar estrutura** lays out a first structure over the walls (#92).
- Pre-sizing by the usual rules: slab thickness by span, beam height by span, column area by load and position, footing size by soil pressure, with loads going from the slabs through the beams to the soil and a warning for each element to review (#93).
- Continuous beam analysis (moments, shear, reactions and long-term deflection against L/250) and full 2D plane-frame analysis (`src/domain/frame2d/`): beams and columns coupled by Direct Stiffness (`u, v, θ`), distributed and concentrated loads retained at their real positions, rigid or pinned joints and fixed or pinned bases, column & beam internal forces (`N, V, M`), support reactions (`Rx, Ry, Mz`), service lateral wind drift (`H/850`), magnified deformed shape, *Pórticos 2D* UI tab, PDF memorial table, MCP `porticos_2d` and A3 PDF/SVG export. Solver failures are retained with the frame label and original reason, turn the safety light red and remain visible in the UI, memorial, diagrams, warnings and MCP `falhas_porticos_2d` instead of silently dropping the frame (#94).
- Preliminary reinforcement for beams, columns, slabs and footings with NBR 6118:2023 bond/anchorage (`lb,nec`), column lap splices (`l0`), B12 Branson cracked-section deflection metadata (`Mr`, Stage I/II, `I_eq/Ic`), flexural-domain/bar-fit/shear/steel-ratio traffic lights, bar schedule with shapes, CA-50 summary by diameter (commercial 12 m bars and +10% waste), A3 schematic detailing sheet (SVG/PDF in UI and MCP `armaduras_svg`/`armaduras_pdf`), CSV export, PDF memorial tables and MCP output (#110).
- Editable concentrated loads for water tanks and heavy equipment, carried through slabs or directly into beams and onward to columns and footings; solid slabs receive local bending/punching checks, while lattice slabs explicitly require manufacturer/solid-strip review. Results appear on the plan, in the safety traffic light, PDF and MCP (#113).
- Derived VA lintels over every opening with masonry above it and CV sills under windows, plus NBR 16868 preliminary checks for infill and load-bearing masonry panels. Marks, sections/bars, traffic lights and warnings are shared by the screen, exports, professional-review PDF and MCP (#111).
- Columns align to walls at any angle, and the pre-sizing window now creates secondary beams between two chosen primary girders; their end reactions become point loads on the girders, matching the existing MCP workflow (#117).
- Per-wall straight or sloped tops now appear with their real heights in sections and facade elevations, completing the same height through quantities, structural loads and column checks (#145).
- Solid slabs now split L-shaped outlines into rectangular panels, envelope the case-1 Czerny and Marcus two-way moments in both directions, check long-term deflection against L/250 and use those moments in the reinforcement proposal; UI, PDF, MCP and traffic lights expose the same panel results (#116).
- Boundary foundations (#144): columns and footings flush inside lot lines (B23), balance beams (*vigas alavanca*) with NBR 6122 pair overturning (`FS = N₂/ΔP ≥ 1.5`) and pair soil-bearing checks (`σ₁, σ₂ ≤ σadm`), top bars and stirrups, automatic boundary strip-footing alternative when counterweight or soil capacity is insufficient, dedicated UI panel, PDF memorial table, traffic lights and MCP output.
- Foundations & geotechnics (#112): multi-borehole SPT soil surveys (`SP-01`, `SP-02`, … with conservative weakest-borehole / shallowest-water-table envelope), immediate elastic and differential settlement checks (footings ≤ 25 mm / diff. ≤ 15 mm, raft ≤ 40 mm, piles ≤ 15 mm) with footing `Recalque` traffic lights, and a 4-way comparative study across spread footings, stiff raft (*radier*), hand-dug bored piles (*brocas*, 3–6 m) and mechanized deep CFA piles (*hélice contínua*, 6–20 m, operating below the water table), integrated across UI, PDF memorial and MCP (`definir_sondagem` with `furos` and `simular_fundacoes`).
- Timber posts and their eave beams now carry a catalogued species or certified strength class, require exterior preservative treatment and no soil contact, and combine compression/buckling with NBR 6123 lateral-wind bending; UI, PDF memorial tables and traffic lights, MCP output and migration of old class-only projects are complete (#146).
- Discipline views: architecture, electrical and structure shown on their own on screen (the rest faded) and on sheets of their own, with *Planta de Instalações Elétricas* and *Planta de Forma* sheets in the sheet set (#96).
- Two MCP tools, `projeto_estrutural` and `pre_dimensionamento`.
- Entrance gates (*Portão*): pedestrian or vehicle, from 0.80 to 8 m, sliding by default, tagged "PT" and available to assistants.
- Several projects at once: *Nova janela* (`Ctrl+Shift+N`) opens another window, with its own recovery draft and MCP port.
- Tabs: new and opened projects go to tabs of the same window, each with its own undo history, selection and view, a shared clipboard, crash-recovery drafts for every tab, and an `abas` MCP tool. *Duplicar aba* starts another discipline of the same building, and *Lado a lado* compares two tabs with synchronised views (#95).

### Changed

- Room names and sizes go to the free spot nearest the middle of the room, clear of furniture and door swings. Exported drawings use the same rule as the screen.
- With *Acompanhar a IA* on, the view slides to edits made off screen and keeps your zoom, instead of zooming in on them.
- Empty categories in the project browser are compact, dimmed rows.

### Fixed

- Ground beams now transfer loaded overhang moments into the continuous-span analysis and propagate signed reactions to the columns and footings. A support in uplift receives a red check and an explicit warning (#143).
- Collinear wall joints no longer count as lateral bracing when checking masonry panel length; transverse returns and columns still do (#111, #147).
- The area schedule of an empty project showed "-0,00 m²".
- The materials schedule now uses each wall's real block height via `Masonry::blocks_per_square_metre` and groups mixed per-wall masonries instead of assuming a 19 cm height (B20).
- Regional electrical utility profiles (`src/domain/utility_profiles.rs`) now use explicit per-utility demand tables (replacing arbitrary scale multipliers), corrected CEMIG ND 5.1 (`8/16 kVA`) and COPEL NTC 901100 (`9/15 kVA`) supply limits, and standard/edition/table citations shown in the UI, single-line SVG and MCP `quadro_de_cargas` (B21).
- Sheet panel fit check, *Melhor escala* and MCP `prancha` now share `SheetLayout::drawing_area_mm()` with the SVG/PDF exporter, reserving 50 mm for the title block when enabled and 14 mm when hidden (B22).
- The ▾, ↻ and ↺ symbols in ribbon labels appeared as empty boxes on Windows.
- Switching, adding or removing a floor cleared only the primary selection. A following `Delete` could remove selected elements on another floor.
- Switching floors left conduits, beams and slabs behind, and the next edit on another floor deleted them. They now travel with their floor.

[Unreleased]: https://github.com/igors93/Trazo/commits/main
