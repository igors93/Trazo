<div align="center">

# Trazo

**A native 2D architectural CAD for floor plans:**<br>
**precise enough for the construction site, simple enough for a first house, and built to work with AI.**

[![CI](https://github.com/igors93/Trazo/actions/workflows/ci.yml/badge.svg)](https://github.com/igors93/Trazo/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust 2024](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org)
[![MCP: 41 tools](https://img.shields.io/badge/MCP-41%20tools-7C3AED.svg)](docs/mcp.md)

[Features](#features) · [Getting started](#getting-started) · [AI assistants](#ai-assistants) · [Documentation](#documentation) · [Roadmap](ROADMAP.md) · [Contributing](CONTRIBUTING.md)

</div>

![Trazo with a furnished single-storey house: ribbon, project browser, plan with dimensions and door and window tags, and the area schedule](docs/images/workspace.png)

> [!NOTE]
> Trazo's interface is in **Brazilian Portuguese** and follows Brazilian standards (ABNT NBR 6492, NBR 9050). The code and documentation are in English. An English interface is on the [roadmap](ROADMAP.md) (#68).

## Why Trazo

- **It understands architecture, not just lines.** Walls, rooms, doors and windows know about each other. Drag the wall between two bedrooms and both rooms, their doors and every dimension follow.
- **It is made for the construction site.** Real bricks and blocks, wall thicknesses that include plaster, dimensions in centimetres, door and window schedules, and sheets printed at exact scale.
- **AI is a first-class collaborator.** A built-in [MCP](https://modelcontextprotocol.io) server lets Claude, ChatGPT and other assistants draw and edit the open plan live, with every change visible and undoable.
- **It is fast and self-contained.** A single native executable written in Rust. No account and no cloud service required; your projects stay as plain JSON files on your computer.

## Features

<table>
<tr>
<td width="50%" valign="top">

**Drawing**
- Lots, walls and rooms with real measurements; L, U and free-shape rooms
- Object snaps, ortho mode and typed distances, angles and coordinates
- Copy, move, rotate, mirror, offset, trim, extend, divide, join
- Align, distribute, group and repeat; window and crossing selection
- Geometric constraints: parallel, perpendicular, coincident, fixed distance
- Lines, polylines, arcs, circles and splines for 2D drafting

**Architecture**
- Wall junctions at any angle; shared walls that move with both rooms
- Masonry with real bricks and blocks, modulation and block estimates
- Multilayer walls; walls placed by axis or by face
- Hinged, sliding, pocket, pivot, bifold and saloon doors; seven window types
- Entrance gates: pedestrian (from 0.80 m) and vehicle (up to 8 m), sliding or swinging
- Stairs, accessible ramps, landings, railings and multiple floors
- Irregular lots, setbacks, and usable, built and lot areas

</td>
<td width="50%" valign="top">

**Electrical design (NBR 5410)**
- Outlets (low, mid, high, specific use), switches, lights, panel and data points
- Minimum points per room generated from the norm: lighting by area, outlets by perimeter
- Automatic circuits, and a load schedule with current, wire gauge, breaker and RCD
- An electrical-only view, and the same symbols and tags on screen and on paper

**Interiors**
- Furniture and fixtures that stand against the nearest wall face
- A customisable library of furniture, equipment and symbols
- Per-room finishes, baseboards and waterproofing, with quantities
- Circulation, door-swing and clearance checks (NBR 9050)

**Documentation**
- Dimension chains and associative dimensions
- Door and window tags ("P1 80/210") and schedules
- Layers, hatches, texts, leaders and architectural symbols
- Sheets from A4 to A0 at exact scale, title blocks, sheet sets and revisions
- Sections, elevations and details with cross-references
- Export to PDF, PNG, editable SVG and CSV

**Safety**
- Atomic saves with a backup, crash recovery, and undo for every edit
- Several projects in tabs, side by side with synchronised views (`Ctrl+T`, `Ctrl+Tab`)

</td>
</tr>
</table>

<p align="center">
  <img src="docs/images/plan-export.png" width="560" alt="A plan exported by Trazo: dimension chains, door and window tags, furniture and room labels placed clear of the furniture">
  <br><em>A plan exported to PNG. Room labels move clear of furniture and door swings, on screen and on paper.</em>
</p>

## Getting started

### Requirements

- [Rust](https://rustup.rs) 1.85 or newer (stable).
- Windows 10 or 11 is the main platform. Linux and macOS builds should work but are less tested. On Debian or Ubuntu, first install `libxkbcommon-dev libgl1-mesa-dev libgtk-3-dev`.

Pre-built downloads are planned ([#77](ROADMAP.md#e-release-and-quality)).

### Build and run

```sh
git clone https://github.com/igors93/Trazo.git
cd Trazo
cargo run --release
```

The window opens maximised, with an empty plan.

### Your first plan

1. **Terreno** (`T`): type the lot's width and length, for example 12 × 25 m.
2. **Cômodo** (`C`): drag a room and name it. Its walls are created and joined to the existing ones.
3. **Porta** (`R`) and **Janela** (`J`): drag them onto a wall.
4. **Interiores** tab: place a table, a sink or a bed. Near a wall, each one turns its back to it.
5. **Exportar**: save a PDF or PNG, or open **Prancha** on the *Saída* tab for an A3 sheet at exact 1:50 scale.

The [user guide](docs/user-guide.md) covers every tool and shortcut.

## AI assistants

Trazo runs an MCP server while it is open, at `http://127.0.0.1:7799/mcp`. Connect Claude Desktop with one click, Claude Code with one command, or ChatGPT through a protected tunnel, then ask in plain language:

> *"Create a 12 × 25 m lot, a 5 × 5 m living room with a door facing the street, and a 4 × 5 m kitchen next to it with a counter under the window."*

![An assistant adding a porch: the new room flashes purple and a badge shows the tool at work](docs/images/ai-live-edit.png)

- **41 tools** cover reading the plan, rooms, walls, openings and gates, fixtures, the electrical design, CAD editing, constraints, batches, files and export.
- **Several projects at once.** Assistants list, switch, create and duplicate tabs with `abas`; each Trazo window also has its own server (7799, 7800, …).
- **Live and visible.** Each AI edit flashes on the plan, and *Conectar uma IA* logs what the assistant did.
- **Safe to try.** Every tool call is one undo step, and failed calls change nothing.
- **Real-time collaboration.** Revisions and change notifications let the assistant notice your edits instead of overwriting them.

See the [MCP guide](docs/mcp.md) for setup and the full tool list.

## Documentation

| Document | For |
| --- | --- |
| [User guide](docs/user-guide.md) | Every tool, workflow and keyboard shortcut |
| [MCP guide](docs/mcp.md) | Connecting AI assistants, tools, real-time collaboration and security |
| [Architecture](docs/architecture.md) | How the code is organised, for contributors |
| [Roadmap](ROADMAP.md) | What comes next, milestone by milestone |
| [Changelog](CHANGELOG.md) | What has shipped |
| [Contributing](CONTRIBUTING.md) | How to set up, code style, tests and pull requests |
| [Security policy](SECURITY.md) | How to report a vulnerability, and the security model |

## Roadmap

The next milestones are:

1. **Ready to share:** downloadable releases, an installer and crash reports.
2. **Works with other CAD:** DXF import and export, printing to scale, image underlays.
3. **The construction-site set:** opening dimensions, a first-course plan, setting-out and quantities, in one click.
4. **Engineering disciplines:** wiring and single-line diagrams for the electrical design, plumbing, and 2D structural design with pre-sizing.
5. **AI co-pilot:** MCP for the whole model, approval mode, and explained validation.

Read the full [roadmap](ROADMAP.md) for details and the order of work.

## Contributing

Contributions of all sizes are welcome: bug reports, ideas, documentation and code. Start with [CONTRIBUTING.md](CONTRIBUTING.md) and please follow the [Code of Conduct](CODE_OF_CONDUCT.md). To report a security problem, see [SECURITY.md](SECURITY.md).

## License

Trazo is licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE), or
- [MIT license](LICENSE-MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Trazo by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Acknowledgements

Trazo is built on [egui and eframe](https://github.com/emilk/egui) for the interface, [resvg](https://github.com/linebender/resvg) and [svg2pdf](https://github.com/typst/svg2pdf) for export, [tiny_http](https://github.com/tiny-http/tiny-http) for the MCP server, and [serde](https://serde.rs) for project files.
