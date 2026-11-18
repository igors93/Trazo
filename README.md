# Trazo

Trazo is a native Rust floor plan editor designed for quick, simple drafting. The source code and file names are in English; the application interface is in Brazilian Portuguese.

## Run

```powershell
cargo run --release
```

## Main workflow

1. Choose **Parede** on the ribbon and click two points on the canvas to create a wall.
2. Choose **Cômodo**, click two opposite corners, and type any name in the dialog. A rectangular room with four connected walls is created.
3. Drag **Porta** or **Janela** from the ribbon onto a wall. You can also select either tool and click a wall.
4. Choose **Selecionar**. Click or drag a wall, or drag inside a room. Use the **Excluir** tile or Delete key to remove the selected wall or whole room. Undo and redo cover these edits.
5. Use the **Arquivo** menu to save the editable project as JSON or export PNG/PDF for sharing.

The grid snaps to 0.25 m, with additional snapping to existing endpoints. Wall length and room width and height update in the live drawing preview. Use the slider at the bottom right or scroll to zoom, use the middle mouse button to pan, and press Esc to cancel drawing. Ctrl+S, Ctrl+O, Ctrl+Z, and Ctrl+Y are available. Older project files with preset room types still open with their original labels.

## Structure

- `src/domain/geometry.rs`: points, distances, projections, and grid spacing.
- `src/domain/model.rs`: walls, rooms, openings, and editing operations.
- `src/domain/history.rs`: undo and redo snapshots.
- `src/app/canvas.rs`: drawing the grid, plan, openings, and live previews.
- `src/app/layout.rs`: menu bar, ribbon, room naming dialog, and status bar.
- `src/app/toolbar.rs`: lightweight vector tool icons.
- `src/app/mod.rs`: interaction handling and application state.
- `src/io/project_file.rs`: editable project persistence.
- `src/io/export.rs`: SVG based PNG and vector PDF export.

This is an initial drafting tool. Rooms are rectangular and wall thickness, opening dimensions, and door swing are fixed defaults. Wall length and room dimensions are shown in meters.
