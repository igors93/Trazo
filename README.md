# Trazo

Trazo is a native Rust floor plan editor designed for quick, simple drafting. The source code and file names are in English; the application interface is in Brazilian Portuguese.

## Run

```powershell
cargo run --release
```

## Main workflow

1. Select **Parede** and click two points on the canvas to create a wall.
2. Select a room type, choose **Cômodo**, and click two opposite corners to create a rectangular room with four connected walls.
3. Drag **Porta** or **Janela** from the left panel onto a wall. You can also select either tool and click a wall.
4. Select **Mover parede** and drag a wall. Connected endpoints follow it. Click a wall and press Delete to remove it.
5. Use **Salvar** to store the editable project as JSON, or **Exportar PNG/PDF** for sharing.

The grid snaps to 0.25 m, with additional snapping to existing endpoints. Scroll to zoom, use the middle mouse button to pan, and press Esc to cancel a two-point action. Ctrl+S, Ctrl+O, Ctrl+Z, and Ctrl+Y are available.

## Structure

- `src/domain/geometry.rs`: points, distances, projections, and grid spacing.
- `src/domain/model.rs`: walls, rooms, openings, and editing operations.
- `src/domain/history.rs`: undo and redo snapshots.
- `src/app/canvas.rs`: drawing the grid, plan, openings, and live previews.
- `src/app/mod.rs`: native interface and interaction handling.
- `src/io/project_file.rs`: editable project persistence.
- `src/io/export.rs`: SVG based PNG and vector PDF export.

This is an initial drafting tool. Rooms are rectangular and wall thickness, opening dimensions, and door swing are fixed defaults. Wall length and room dimensions are shown in meters.
