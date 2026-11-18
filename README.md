# Trazo

Trazo is a native Rust floor plan editor designed for quick, simple drafting. The source code and file names are in English; the application interface is in Brazilian Portuguese.

## Run

```powershell
cargo run --release
```

## Main workflow

1. Choose **Terreno** and enter exact width and length in meters. For a 50 m² plot, use 5 m × 10 m and click **Criar terreno**. You can also click two opposite corners on the canvas. A new numeric terrain is centered on existing walls and rooms when they fit.
2. Choose **Parede** and click two points on the canvas to create a wall.
3. Choose **Cômodo**, click two opposite corners, and type any name in the dialog. A rectangular room with four connected walls is created.
4. Drag **Porta** or **Janela** from the ribbon onto a wall. You can also select either tool and click a wall.
5. Choose **Selecionar**. Click or drag a wall, room, or terrain. Use **Excluir** or Delete to remove it. Terrain dimensions can be changed in the contextual inspector. If an older plan is outside the terrain, use **Centralizar planta no terreno** in the inspector.
6. Use the **Arquivo** menu to save the editable project as JSON or export PNG/PDF for sharing.

The grid adapts to zoom (0.25 m, 1 m, or 5 m), with additional snapping to existing endpoints. Walls and rooms stay within the terrain when one exists, and wall strokes become thinner when zoomed out. Wall length, room dimensions, and terrain area update in the live drawing preview. Use **Enquadrar** to fit the whole project in the window. The bottom slider and mouse wheel control zoom; the middle mouse button pans. Press Esc to cancel drawing. Ctrl+S, Ctrl+O, Ctrl+Z, and Ctrl+Y are available. Older project files still open.

## Structure

- `src/domain/geometry.rs`: points, distances, projections, and grid spacing.
- `src/domain/model.rs`: terrain, walls, rooms, openings, and editing operations.
- `src/domain/history.rs`: undo and redo snapshots.
- `src/app/canvas.rs`: drawing the grid, plan, openings, and live previews.
- `src/app/ui/`: menu bar, ribbon, contextual inspector, room naming dialog, status bar, and lightweight vector icons.
- `src/app/interaction.rs`: canvas input, selection, dragging, and snapping.
- `src/app/theme.rs`: egui visual theme.
- `src/app/mod.rs`: application state, project commands, and viewport fitting.
- `src/io/project_file.rs`: editable project persistence.
- `src/io/export.rs`: SVG based PNG and vector PDF export.

This is an initial drafting tool. Rooms are rectangular and wall thickness, opening dimensions, and door swing are fixed defaults. Wall length and room dimensions are shown in meters.
