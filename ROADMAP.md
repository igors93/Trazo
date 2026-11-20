# Trazo Roadmap

*Last updated 2026-11-20.*

This is the plan for what comes next. Work through it **milestone by milestone, in order**: each milestone builds on the previous one and ends with a concrete test of "done". [Next up](#next-up) lists the first ten items on that path. Everything already shipped, items #01–#37 and more, is in the [changelog](CHANGELOG.md).

- [Vision](#vision)
- [Where Trazo stands](#where-trazo-stands)
- [How to read this roadmap](#how-to-read-this-roadmap)
- [Next up](#next-up)
- [Milestones](#milestones)
- [Backlog](#backlog)
- [Definition of done](#definition-of-done)
- [Out of scope](#out-of-scope)

## Vision

> A smart 2D architectural CAD with the simplicity of a visual editor and the precision of an engineering tool, ready for the construction site and built to work with AI.

Trazo should serve both ends of the spectrum on the same project and file:

- **Newcomers** draw a whole house without learning coordinates, layers or CAD commands.
- **Professionals** produce detailed technical drawings: exact geometry, dimensions, layers, scales, quantities and exchange with other CAD programs.

### Principles

1. **Architecture before command count.** Moving a wall and seeing rooms, doors and dimensions follow is worth more than a hundred generic commands.
2. **Built for the construction site.** Drawings must leave Trazo ready for the site: dimensions in centimetres, located openings, modular masonry, quantities, printing to scale.
3. **Screen equals paper.** What you see is what you export; rules shared by both live in the domain.
4. **AI as a collaborator.** Anything a person can do, an assistant can do through MCP, visibly, reversibly and with the person in control.
5. **Ease is a feature.** Every new capability, including safety measures, should add as few steps as possible.

## Where Trazo stands

| Area | Status |
| --- | --- |
| Drawing engine (#01–#10) | ✅ Complete: `f64` geometry, free-shape rooms, snaps, typed input, CAD modify tools, constraints, multiple selection |
| Architecture (#11–#25) | ✅ Complete: junctions, shared walls, multilayer walls, door and window types, stairs, floors, library, finishes, circulation checks |
| Documentation (#26–#37) | ✅ Complete: associative dimensions, layers, sheets, scales, title blocks, hatches, schedules, sections, sheet sets, ABNT styles |
| Exchange (#38–#45) | 🟡 SVG and CSV export only; no DXF or DWG yet |
| AI (#46–#52) | 🟡 35 MCP tools, real-time sync, tunnel token; it covers only part of the model, and there is no approval or read-only mode |
| Construction site (#53–#57) | 🟡 Building blocks exist (first course per wall, block estimate, sheets); the site deliverables do not |
| Release (#58–#64) | 🟡 CI, licence, property tests and file migrations done; no binaries, installer or crash reports |
| Experience (#65–#67) | ⬜ Not started |

## How to read this roadmap

| Field | Meaning |
| --- | --- |
| **#** | Item number. Carried-over items keep their original number; new items start at #68. Use it in commits and pull requests: `feat: import DXF walls (#38)`. |
| **Priority** | **P0** required for its milestone · **P1** important · **P2** valuable, can slip |
| **Size** | Rough effort for one developer: **S** ≤ 2 days · **M** ≤ 1 week · **L** ≤ 3 weeks · **XL** more than 3 weeks |
| **Status** | ⬜ not started · 🟡 partial (the note says what exists) · 🔵 in progress · ✅ done |

When an item ships, mark it ✅ here, add it to the changelog, and remove it from this file at the next update.

## Next up

The first ten items on the path through M1, M2 and M3, in order. The other items of each milestone fit in around them.

| Order | # | Item | Milestone | Why now |
| --- | --- | --- | --- | --- |
| 1 | #77 | Release pipeline with downloadable binaries | M1 | The repository is public and licensed; people need to run Trazo without installing Rust. |
| 2 | #79 | Crash log and "report a problem" | M1 | Bug reports from real users need logs from day one. |
| 3 | #63 | Windows installer and file association | M1 | Double-clicking a project should open it in Trazo. |
| 4 | #81 | Sample projects | M1 | Docs, screenshots, tests and new users all need ready-made plans. |
| 5 | #38 | DXF import and export | M2 | The biggest blocker for professional use: exchanging drawings with AutoCAD users. |
| 6 | #70 | Print to scale | M2 | A3 at 1:50 straight to the printer, without going through PDF. |
| 7 | #40 | Underlay image or PDF with scale calibration | M2 | Start from a photographed sketch or an existing plan. |
| 8 | #53 | Opening dimension chains | M3 | The first construction-site deliverable, and the one builders use most. |
| 9 | #56 | Construction quantities | M3 | Turns the drawing into a shopping list. |
| 10 | #57 | One-click construction set | M3 | Packages #53–#56 into what a site actually uses. |

## Milestones

### M1: Ready to share

Make Trazo easy to download, try and report problems with.

- **Items:** #77, #63, #79, #81, #80, #78
- **Done when:** someone without Rust downloads a release from GitHub, installs it, opens a sample project by double-clicking it, and can attach a crash log to a bug report in one click.

### M2: Works with other CAD

Exchange drawings with the rest of the profession and start from existing material.

- **Items:** #38, #70, #40, #43, #39
- **Done when:** a Trazo plan opens in AutoCAD, QCAD and LibreCAD with layers, units and dimensions intact; a DXF from AutoCAD imports as walls and lines on the right layers; a photographed sketch can be calibrated and traced; and an A3 1:50 print measures correctly with a scale ruler.

### M3: The construction-site set

The differentiator: everything a builder needs comes straight from the plan.

- **Items:** #53, #54, #55, #56, #57, #71
- **Done when:** from the sample house, one click produces an A3 1:50 set with a dimensioned plan that locates every opening, a first-course plan with lintels, a setting-out plan and a quantity list. The quantities (blocks with waste, half blocks, U-blocks, lintels, mortar, plaster, flooring) must match a manual takeoff within 2 %.

### M4: AI co-pilot

Assistants that can do everything, explain problems and leave the person in control.

- **Items:** #74, #75, #46, #51, #49, #73, #48, #76, #50, #52
- **Done when:** an assistant draws a two-storey house with stairs, finishes, dimensions and a sheet set **using MCP only**. The person can review and approve each proposed change before it applies, compare and revert any AI action, and see every validation problem explained with a suggested fix.

### M5: Progressive experience

Simple for newcomers, fast for professionals.

- **Items:** #65, #66, #67, #68, #69, #82
- **Done when:** in a usability test with three people new to CAD, each completes *Minha primeira casa* (my first house) without help in under 15 minutes, while professional mode offers a command line, and the interface is available in English.

### M6: Scale and robustness

Runs in parallel with the other milestones.

- **Items:** #62, #61, #83, #84
- **Done when:** a project with 500 walls on 5 floors pans at 60 fps with every edit under 50 ms, undo memory stays bounded, and exported PDFs are compared against reference images in CI.

### Exploratory

Worth prototyping after M5, when the core is solid: #85, #86, #87, #72, #44, #45.

## Backlog

### A. Exchange and output

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 38 | DXF import and export: walls, openings, fixtures, drawings, dimensions, texts and hatches, with layers, colours and units | P0 | L | M2 | ⬜ |
| 70 | Print to a printer at exact scale, with the system print dialog and sheet preview | P0 | M | M2 | ⬜ |
| 40 | Image and PDF underlay with two-point scale calibration, opacity and lock | P1 | M | M2 | ⬜ The faded underlay of the floor below exists (#22); external images do not |
| 43 | Reusable blocks with attributes and a personal library shared between projects | P1 | M | M2 | 🟡 The library (#23) customises symbols; there are no user blocks with attributes yet |
| 39 | DWG, through a reliable library or converter | P2 | L | M2 | ⬜ |
| 44 | External references (other drawings shown and updated live) | P2 | L | Exploratory | ⬜ |
| 45 | Visual comparison between two versions of a project | P2 | M | Exploratory | ⬜ |

### B. Construction site

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 53 | Automatic dimension chains locating each door and window along its wall, plus internal room dimensions | P0 | M | M3 | 🟡 Associative dimensions can anchor to openings and internal room dimensions can be generated; no automatic opening chains |
| 54 | First-course plan and wall elevations with block paging: whole, half and cut blocks, lintels and sills | P1 | L | M3 | 🟡 Properties show each wall's first course; there is no drawing |
| 55 | Setting-out plan (*locação*): cumulative dimensions from an origin on the lot, for marking out the building | P1 | M | M3 | ⬜ |
| 56 | Construction quantities: blocks with waste, half blocks and U-blocks, lintels per opening, laying mortar, plaster, flooring and wall finishes per room | P0 | M | M3 | 🟡 Block estimate, wall area and finish quantities exist separately; no consolidated takeoff |
| 57 | One-click construction set: A3 at 1:50 with dimensions, openings, schedules, first course, setting-out and legend | P0 | M | M3 | 🟡 Sheets, scales, templates and title blocks exist |
| 71 | Roof plan (*planta de cobertura*): roof planes, slopes, ridges, gutters and downpipes | P1 | L | M3 | ⬜ |
| 72 | Electrical and plumbing points (outlets, switches, lights, water points) with legend and counts; layout only, not engineering design | P2 | M | Exploratory | ⬜ The library has some technical symbols |

### C. AI assistants

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 74 | MCP covers the whole model: floors, stairs and ramps, finishes, layers, dimensions, notes, hatches, library items, sheets and views | P0 | L | M4 | 🟡 35 tools cover lot, walls, rooms, openings, 11 fixture kinds, drawings and CAD editing |
| 75 | Read-only switch in *Conectar uma IA*, for assistants that should only analyse | P1 | S | M4 | ⬜ |
| 46 | Approval mode: the assistant proposes, the plan shows a preview, the person accepts or rejects | P1 | M | M4 | ⬜ |
| 51 | AI history with a diff per action, comparison and selective revert | P1 | M | M4 | 🟡 Live highlight, action log and revisions exist |
| 49 | Plan validation with explanations and one-click fixes: unclosed rooms, overlaps, rooms without doors, unreachable spaces | P1 | M | M4 | 🟡 The circulation check (#25) covers clearances and door swings |
| 73 | Building-code checks: minimum room areas and widths, lighting and ventilation ratios, door widths; rule sets configurable per municipality | P1 | L | M4 | ⬜ |
| 48 | Built-in assistant panel: natural-language commands inside Trazo, using the user's own AI provider | P1 | L | M4 | 🟡 Works today through external assistants over MCP |
| 76 | MCP prompts (for example "draw a house from this brief") and richer resources: sheet images, schedules | P2 | S | M4 | ⬜ |
| 50 | Room layout proposals from a brief (rooms, areas, adjacencies), shown as alternatives | P2 | L | M4 | ⬜ |
| 52 | An assistant that teaches the editor: context tips and guided exercises | P2 | M | M4 | ⬜ |

### D. Experience

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 65 | Simple, Intermediate and Professional interface levels on the same project and file | P1 | L | M5 | ⬜ |
| 66 | Guided flow *Minha primeira casa*: lot, desired rooms, adjustable layout, openings and a final check | P1 | L | M5 | ⬜ |
| 67 | Guided documentation flow: build and review a sheet set before exporting | P2 | M | M5 | ⬜ |
| 68 | English interface (internationalisation), keeping Brazilian Portuguese as the default | P1 | L | M5 | ⬜ |
| 69 | Command line and command palette with typed commands, aliases and recent commands | P1 | M | M5 | ⬜ |
| 82 | Dark theme, interface scale and full keyboard navigation | P2 | M | M5 | ⬜ |

A chosen level must never be a wall. A newcomer can open an advanced tool when needed, and a professional keeps the simple actions. For example, *create a 3.00 × 4.00 m bedroom*:

- **Simple:** type the size or drag; Trazo creates the walls, the name and the area.
- **Professional:** the same room, with walls referenced by face, associative dimensions and an assigned layer.

### E. Release and quality

| # | Item | Priority | Size | Milestone | Status |
| --- | --- | --- | --- | --- | --- |
| 77 | Release pipeline: tagged GitHub Releases with Windows, Linux and macOS binaries, checksums and changelog notes | P0 | M | M1 | ⬜ |
| 63 | Windows installer, `.json` project association and update check | P0 | M | M1 | ⬜ |
| 79 | Crash log and *Relatar problema* (report a problem): panic hook, rotating log file, pre-filled GitHub issue | P0 | S | M1 | ⬜ |
| 81 | Sample projects in `examples/` (single-storey house, two-storey house, apartment), used by docs and tests | P1 | S | M1 | ⬜ |
| 80 | Documented project file format (JSON Schema) for third-party tools | P1 | M | M1 | ⬜ |
| 78 | Signed Windows binaries | P2 | S | M1 | ⬜ |
| 62 | Performance with large plans: spatial index for picking, cached rendering, benchmarks in CI | P1 | L | M6 | ⬜ |
| 61 | Visual regression tests for PDF, PNG and SVG export | P1 | M | M6 | ⬜ |
| 84 | Transaction-based undo history (diffs instead of full project copies) | P2 | L | M6 | ⬜ |
| 83 | Undo history panel: list of steps, jump to any step | P2 | S | M6 | ⬜ |

### F. Exploratory

| # | Item | Priority | Size | Status |
| --- | --- | --- | --- | --- |
| 85 | 3D massing preview: extruded walls, openings and slabs, orbit view | P2 | XL | ⬜ |
| 86 | IFC export of walls, spaces, openings and floors, for BIM workflows | P2 | L | ⬜ |
| 87 | Cost estimate from the quantities, with editable price tables (SINAPI import) | P2 | M | ⬜ |

## Definition of done

An item is done when:

- it works on screen **and** in every export it touches (screen equals paper);
- it has tests, including a regression test for each bug fixed along the way;
- assistants can use it through MCP, when it changes the model;
- the [user guide](docs/user-guide.md), the [MCP guide](docs/mcp.md) and the [changelog](CHANGELOG.md) are updated;
- `cargo fmt`, `cargo clippy -- -D warnings` and `cargo test` pass.

## Out of scope

To stay an excellent, approachable 2D architectural editor, these are not planned:

- photorealistic 3D rendering (a simple massing view is exploratory, #85);
- full BIM authoring (IFC export is exploratory, #86);
- structural calculation;
- electrical and plumbing engineering design (points layout is #72);
- real-time cloud collaboration between people (AI collaboration is in scope).

Want to suggest a change to the roadmap? Open a [feature request](https://github.com/igors93/Trazo/issues/new?template=feature_request.yml) and explain the problem it solves.
