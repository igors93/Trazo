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

**AI assistants (MCP)**

- An MCP server with 35 tools over streamable HTTP, a `--mcp-stdio` bridge and one-click Claude Desktop setup.
- A public tunnel for cloud assistants, protected by a token in the address that changes on every opening (#47).
- `executar_lote`, which runs many tools as one all-or-nothing undo step, plus CAD editing tools: transform, align, distribute, group, repeat, trim, extend, offset, divide, join and constraints.
- Real-time collaboration: plan revisions, `mudancas_desde`, the `trazo://planta` resource and change notifications on the event stream.
- A live highlight of each AI edit, a "working" badge, *Acompanhar a IA* and an action log (part of #51).

**Project**

- Continuous integration with formatting, Clippy and tests (#58), geometry property tests (#60), and a versioned file format with tested migrations (#64).
- Dual licence, MIT or Apache-2.0 (#59), contributing guide, code of conduct, security policy, and issue and pull request templates.

### Changed

- Room names and sizes go to the free spot nearest the middle of the room, clear of furniture and door swings. Exported drawings use the same rule as the screen.
- With *Acompanhar a IA* on, the view slides to edits made off screen and keeps your zoom, instead of zooming in on them.
- Empty categories in the project browser are compact, dimmed rows.

### Fixed

- The area schedule of an empty project showed "-0,00 m²".
- The ▾, ↻ and ↺ symbols in ribbon labels appeared as empty boxes on Windows.
- Switching, adding or removing a floor cleared only the primary selection. A following `Delete` could remove selected elements on another floor.

[Unreleased]: https://github.com/igors93/Trazo/commits/main
