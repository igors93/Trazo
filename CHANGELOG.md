# Changelog

All notable changes to Trazo are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions will follow [Semantic Versioning](https://semver.org) from the first tagged release. Numbers such as (#38) refer to [roadmap](ROADMAP.md) items.

## [Unreleased]

Everything below is on `main` and will become the first release, 0.1.0.

### Added

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

- Associative dimensions (#26), layers (#27), sheets from A4 to A0 (#28), exact print scales (#29), title block and pagination (#30).
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
- Continuous beam analysis: bending moments, shear, reactions and long-term deflection against L/250, drawn as diagrams and exported to PDF and SVG (#94).
- Preliminary reinforcement for beams, columns, slabs and footings, with section-fit warnings, shear checks, theoretical steel mass, CSV bar schedule and the same tables in the professional-review PDF and MCP output (#110, partial).
- Editable concentrated loads for water tanks and heavy equipment, carried through slabs or directly into beams and onward to columns and footings; solid slabs receive local bending/punching checks, while lattice slabs explicitly require manufacturer/solid-strip review. Results appear on the plan, in the safety traffic light, PDF and MCP (#113).
- Derived VA lintels over every opening with masonry above it and CV sills under windows, plus NBR 16868 preliminary checks for infill and load-bearing masonry panels. Marks, sections/bars, traffic lights and warnings are shared by the screen, exports, professional-review PDF and MCP (#111).
- Columns align to walls at any angle, and the pre-sizing window now creates secondary beams between two chosen primary girders; their end reactions become point loads on the girders, matching the existing MCP workflow (#117).
- Per-wall straight or sloped tops now appear with their real heights in sections and facade elevations, completing the same height through quantities, structural loads and column checks (#145).
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

- The area schedule of an empty project showed "-0,00 m²".
- The ▾, ↻ and ↺ symbols in ribbon labels appeared as empty boxes on Windows.
- Switching, adding or removing a floor cleared only the primary selection. A following `Delete` could remove selected elements on another floor.
- Switching floors left conduits, beams and slabs behind, and the next edit on another floor deleted them. They now travel with their floor.

[Unreleased]: https://github.com/igors93/Trazo/commits/main
