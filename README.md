# Trazo

Trazo is a native Rust floor plan editor designed for quick, simple drafting. The source code and file names are in English; the application interface is in Brazilian Portuguese.

## Run

```powershell
cargo run --release
```

## Main workflow

1. Choose **Terreno** and enter exact width and length in meters (e.g. 5 m × 10 m) and click **Criar terreno**, or draw it directly on the canvas using click-and-drag or two opposite corner clicks. Existing walls and rooms are preserved and centered when appropriate.
2. Choose **Parede** to draw a wall using either click-and-drag or two sequential clicks. Snapping to the 0.25 m grid, vertices, and orthogonal lines assists precise alignment.
3. Choose **Cômodo** to create a rectangular room by dragging or clicking two corners, then name the room in the modal dialog. Four connected boundary walls are generated, sharing existing collinear walls automatically without duplication.
4. Add openings by either dragging **Porta** or **Janela** from the ribbon onto a wall, or by selecting the tool and clicking a wall. Openings respect minimum distances from wall ends and cannot overlap other openings.
5. Choose **Selecionar** to inspect and edit elements:
   - **Walls**: adjust length numerically with instant preview.
   - **Rooms**: rename rooms or update width and length numerically. Connected openings, wall IDs, and shared walls are preserved transactionally.
   - **Openings**: drag along the wall or set custom widths within certified limits (doors: 0.60 m to 2.00 m; windows: 0.40 m to 4.00 m).
   - **Terrain**: resize or use **Centralizar planta no terreno**.
   - Use **Excluir** or the `Delete` key to remove selected items.
6. Use the **Arquivo** menu to save projects (`.json`), export high-resolution PNG or vector PDF, open existing files, or create new plans. Safe atomic file replacement with automatic `.trazo.bak` backup and background crash recovery (`.trazo_recovery.json`) ensures zero data loss.

Drawing creation can be cancelled at any time by pressing `Esc` or switching tools. Standard shortcuts (`Ctrl+S`, `Ctrl+O`, `Ctrl+Z`, `Ctrl+Y`) and Portuguese decimal comma input are fully supported. Older project file schemas remain fully backward-compatible.

## Structure

- `src/domain/geometry.rs`: points, distances, projections, and robust polygon ear-clipping triangulation.
- `src/domain/model.rs`: terrain, walls, rooms, openings, validation rules, and geometric operations.
- `src/domain/history.rs`: undo and redo stack with duplicate-state suppression.
- `src/app/canvas.rs`: rendering grid, walls, rooms, openings, labels, and live previews.
- `src/app/ui/`: header menu, ribbon toolbar, contextual inspector, room dialog, status bar, and icons.
- `src/app/interaction.rs`: canvas mouse interactions, creation state machine, dragging, and snapping.
- `src/app/theme.rs`: custom egui visual theme.
- `src/app/mod.rs`: application state, data protection flows, commands, and shortcuts.
- `src/io/project_file.rs`: atomic project persistence with backup and backward-compatible deserialization.
- `src/io/recovery.rs`: auto-save and draft recovery mechanism.
- `src/io/export.rs`: high-resolution PNG and vector PDF export via SVG rendering.
