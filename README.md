# Trazo

Trazo is a native Rust floor plan editor designed for quick, simple drafting with the feel of a professional CAD tool. The source code and file names are in English; the application interface is in Brazilian Portuguese.

## Run

```powershell
cargo run --release
```

The window opens maximized.

## The interface

A light, white and light-blue workspace laid out like CAD/BIM editors:

- **Menu bar**: *Arquivo*, *Editar*, *Exibir*, *Desenhar* and *Ajuda*, each entry with its icon and shortcut; quick buttons for new, open, save, undo and redo; the project name with a dot that shows unsaved changes; and **Exportar** (PNG or PDF).
- **Ribbon** with three tabs: *Arquitetura* (selection, terrain, walls, rooms, doors, windows, modify, drafting aids), *Vista* (navigation, grid, panels, help) and *Saída* (project files and export). Click the active tab, or the arrow at the right, to fold the ribbon and gain drawing space.
- **Options bar**: the active tool and its settings, for example the terrain's exact width and length, wall thickness, or the default width of doors and windows.
- **Project browser** (left): terrain, rooms, walls, doors and windows with their areas and lengths, a filter box, foldable groups, click to select and double-click to zoom to an element.
- **Properties** (right): foldable sections for the selected element (dimensions, location, relations); with nothing selected, the project's area schedule, occupancy rate and view settings.
- **Plan view**: hover highlight, a CAD crosshair while drawing, snap markers (a square on wall ends, a dot on the grid), live dimensions, graphic scale, north arrow and a floating zoom bar. Right-click for a context menu.
- **Status bar**: messages, cursor coordinates (Y pointing up), the GRADE, ORTO and ENCAIXE switches, zoom and element count.

Grid, snapping, ortho mode, open panels and the folded ribbon are remembered between sessions.

## Main workflow

1. Choose **Terreno** (`T`) and enter the exact width and length in meters (e.g. 5 m × 10 m) and click **Criar terreno**, or draw it directly on the canvas using click-and-drag or two opposite corner clicks. Existing walls and rooms are preserved and centered when appropriate.
2. Choose **Parede** (`P`) to draw a wall using either click-and-drag or two sequential clicks. Snapping to the 0.25 m grid and to wall ends assists precise alignment; ortho mode (`F8`) keeps walls horizontal or vertical.
3. Choose **Cômodo** (`C`) to create a rectangular room by dragging or clicking two corners, then name the room in the dialog. Four connected boundary walls are generated, sharing existing collinear walls automatically without duplication.
4. Add openings by dragging **Porta** (`R`) or **Janela** (`J`) from the ribbon onto a wall, or by selecting the tool and clicking a wall; the wall that will receive it is highlighted. Openings respect minimum distances from wall ends and cannot overlap other openings.
5. Choose **Selecionar** (`V`) to inspect and edit elements, from the plan or the project browser:
   - **Walls**: adjust length numerically.
   - **Rooms**: rename rooms or update width and length numerically. Connected openings, wall IDs, and shared walls are preserved transactionally.
   - **Openings**: drag along the wall or set custom widths within the limits (doors: 0.60 m to 2.00 m; windows: 0.40 m to 4.00 m).
   - **Terrain**: resize, check the occupancy rate, or use **Centralizar planta**.
   - Move the selection with the arrow keys (0.25 m, or 1 m with `Shift`), and remove it with **Excluir** or `Delete`.
6. Save projects (`.json`), export a high-resolution PNG or vector PDF, open existing files, or create new plans. Safe atomic file replacement with an automatic `.trazo.bak` backup, plus a background crash-recovery draft, protect your work.

Drawing can be cancelled at any time with `Esc`, a right click, or by switching tools. Portuguese decimal commas are accepted in every field. Older project file schemas remain backward-compatible.

## Keyboard shortcuts

| Keys | Action |
| --- | --- |
| `V` `T` `P` `C` `R` `J` | Select, Terrain, Wall, Room, Door, Window |
| `Ctrl+N` / `Ctrl+O` / `Ctrl+S` | New, open, save |
| `Ctrl+Z` / `Ctrl+Y` (or `Ctrl+Shift+Z`) | Undo, redo |
| `Delete` | Delete the selection |
| Arrows / `Shift`+arrows | Move the selection 0.25 m / 1 m |
| `Esc` | Cancel the current action, or clear the selection |
| `F` | Zoom to fit the whole plan |
| `F3` / `F7` / `F8` | Snapping, grid, ortho mode |
| `F1` | Shortcuts and quick guide |
| Mouse wheel / middle button / right button | Zoom at the cursor / pan / context menu |

## Using Trazo from an AI assistant (MCP)

Trazo includes a [Model Context Protocol](https://modelcontextprotocol.io) server, so ChatGPT, Claude and other MCP clients can read and edit the plan open on screen from plain instructions ("create a 12 × 20 m lot with a 6 × 5 m living room and a door on the left wall"). Everything the assistant does shows live in the window and can be undone with `Ctrl+Z`.

The server starts with the window at `http://127.0.0.1:7799/mcp` (streamable HTTP transport). The **IA** switch in the status bar shows whether it is running; clicking it opens **Conectar uma IA**, which has everything below ready to copy. Keep Trazo open while the assistant works; only one Trazo window serves MCP at a time. Set `TRAZO_MCP_PORT` to use another port.

| Client | How to connect |
| --- | --- |
| Claude Desktop | **Configurar Claude Desktop** in *Conectar uma IA* adds Trazo to `claude_desktop_config.json` (keeping your other settings, with a `.bak`), then restart Claude Desktop. Or add it yourself: `{"mcpServers": {"trazo": {"command": "<path>\\trazo.exe", "args": ["--mcp-stdio"]}}}`. In this mode Trazo opens its window by itself on the first tool call. |
| Claude Code | `claude mcp add --transport http trazo http://127.0.0.1:7799/mcp` |
| ChatGPT, Claude on the web | They run in the cloud and need a public https address. **Abrir túnel** starts a free Cloudflare quick tunnel (`cloudflared`, installable from the same window with winget) and shows the connector address `https://….trycloudflare.com/mcp`; or run `cloudflared tunnel --url http://127.0.0.1:7799` yourself and add `/mcp`. Create a connector with it (in ChatGPT, in developer mode), with no authentication. |

Tools: `obter_planta` (plan with ids, coordinates, sizes and areas), `imagem_da_planta` (a PNG preview), `definir_terreno`, `criar_comodo`, `criar_parede`, `inserir_abertura` (door or window), `editar_comodo`, `editar_parede`, `editar_abertura`, `mover_elemento`, `excluir_elemento`, `desfazer`, `refazer`, `centralizar_planta`, `novo_projeto`, `abrir_projeto`, `salvar_projeto` and `exportar_planta` (PDF or PNG). Coordinates are in meters with Y pointing up, as in the status bar. Files go to `Documents\Trazo` unless a full path is given.

Each change runs on a copy of the project and is applied only if it succeeds, as one undo step; a failing tool explains why, so the assistant can correct itself. Opening a new or other project refuses to drop unsaved changes unless the assistant asks to discard them.

**Security.** For ease of use the server has no authentication. It only listens on this computer (`127.0.0.1`), but while a tunnel is open anyone who knows its address can read and edit the open plan, and save or export files. Close the tunnel when you are done; it also closes with Trazo.

If Claude Desktop is using Trazo, close it before rebuilding (`cargo build --release`), since Windows locks the running `trazo.exe`.

## Files

- Projects: any `.json` file you choose, saved atomically with a `.trazo.bak` backup.
- Crash-recovery draft and interface preferences: `%LOCALAPPDATA%\Trazo\recovery.json` and `settings.json` (the system temporary folder when `LOCALAPPDATA` is not set).
- Files saved or exported by an AI assistant without a full path: `Documents\Trazo`.

## Structure

- `src/domain/geometry.rs`: points, distances, projections, and robust polygon ear-clipping triangulation.
- `src/domain/model.rs`: terrain, walls, rooms, openings, validation rules, and geometric operations.
- `src/domain/history.rs`: undo and redo stack with duplicate-state suppression.
- `src/app/canvas.rs`: rendering grid, terrain, walls, rooms, openings, labels, hover and selection highlights, and live previews.
- `src/app/canvas/overlay.rs`: drawing aids over the plan: graphic scale, north arrow, crosshair, snap markers.
- `src/app/interaction.rs`: canvas mouse interactions, creation state machine, picking, dragging, snapping and the context menu trigger.
- `src/app/ui/`: menu bar, ribbon, options bar, project browser, properties panel, status bar, view overlays, dialogs (including *Conectar uma IA*), icons and shared widgets.
- `src/app/theme.rs`: the white and light-blue theme, colours and system fonts.
- `src/app/mod.rs`: application state, data protection flows, commands, and shortcuts.
- `src/io/project_file.rs`: atomic project persistence with backup and backward-compatible deserialization.
- `src/io/recovery.rs`: auto-save and draft recovery mechanism.
- `src/io/settings.rs`: interface preferences kept between sessions.
- `src/io/export.rs`: high-resolution PNG and vector PDF export via SVG rendering, and PNG previews.
- `src/mcp/`: the MCP server: `protocol.rs` (JSON-RPC and the tool catalogue), `server.rs` (HTTP endpoint inside the window), `bridge.rs` (`--mcp-stdio` for stdio clients), `clients.rs` (client setup) and `tunnel.rs` (public address for cloud assistants).
- `src/app/mcp_tools.rs`: the tools themselves, run on the open project.
