# User guide

This guide walks through Trazo's interface and workflow. The application interface is in **Brazilian Portuguese**; every label below is quoted as it appears on screen, followed by its English meaning the first time it shows up.

## Before you start

Trazo is built for people who will build or extend their own home without an engineer or an architect, often with little money (see [the proposal](proposta.md), in Portuguese). Keep three things in mind:

1. **You do not need to learn the tools to start.** The easiest way in is to connect an AI assistant ([MCP guide](mcp.md)) and describe the house you want: the assistant draws, checks and explains, and this guide is there for when you want to adjust something by hand.
2. **Read the traffic light** (*Segurança*, see [Safety and limits](#safety-and-limits)). Red means stop: never build an element that is red, and never accept a green that depends on a soil, concrete or load value nobody can back with data.
3. **Trazo is a protection, not an engineer.** It does not see the ground, the neighbours' walls or the quality of the work. When it refuses, when something stays red, or when the lot is on a slope, near a stream or on landfill, stop and look for help: low-income families are entitled to free technical assistance (ATHIS), and the Defesa Civil answers on 199. Take the report (*memorial*) with you.

- [Before you start](#before-you-start)
- [The workspace](#the-workspace)
- [Drawing a plan](#drawing-a-plan)
- [Selecting and editing](#selecting-and-editing)
- [CAD modify tools](#cad-modify-tools)
- [Architecture tools](#architecture-tools)
- [Interiors](#interiors)
- [Review marks](#review-marks)
- [Electrical design](#electrical-design)
- [Structure](#structure)
- [Water, sewage and rain](#water-sewage-and-rain)
- [Discipline views](#discipline-views)
- [Documentation and output](#documentation-and-output)
- [Files and data safety](#files-and-data-safety)
- [Keyboard and mouse](#keyboard-and-mouse)

Conventions: lengths are in metres, Portuguese decimal commas are accepted in every field (`3,47`), and coordinates use **Y pointing up**, as in the status bar.

## The workspace

![The Trazo workspace](images/workspace.png)

| Area | What it holds |
| --- | --- |
| **Menu bar** | *Arquivo* (File), *Editar* (Edit), *Exibir* (View), *Desenhar* (Draw) and *Ajuda* (Help), each entry with its icon and shortcut; quick buttons for new, open, save, undo and redo; the project name with a dot that marks unsaved changes; and **Exportar** (Export). |
| **Ribbon** | Ten tabs: *Arquitetura* (architecture), *Editar* (edit), *Interiores* (interiors), *Elétrica* (electrical), *Hidráulica* (plumbing), *Estrutura* (structure), *Desenho* (2D drafting), *Marcações* (review marks), *Vista* (view) and *Saída* (output). Click the active tab, or the arrow at the right, to fold the ribbon and gain drawing space. |
| **Options bar** | The active tool and its settings: the lot's exact width and length, the wall material and block size, the modulation switch, the default door width, the selection filter, and so on. |
| **Project browser** (left) | Views, lot, rooms, walls, doors, windows, open spans, fixtures, drawings, groups and constraints, with areas and lengths. A text filter, foldable groups, click to select, double-click to zoom to an element. Empty categories stay as quiet one-line rows. |
| **Properties** (right) | Foldable sections for the selected element (dimensions, masonry, frame type, finishes, location, relations). With nothing selected: the area schedule, the door and window schedule, the masonry estimate, the occupancy rate and the view settings. |
| **Plan view** | Hover highlight, a CAD crosshair while drawing, snap markers, live dimensions, dimension chains, door and window tags, graphic scale, north arrow and a floating zoom bar. Right-click opens a context menu. |
| **Status bar** | Messages, cursor coordinates, the *COTAS* (dimensions), *GRADE* (grid), *ORTO* (ortho) and *ENCAIXE* (snap) switches, the *IA* (AI) server status, zoom and element count. |

Grid, snapping, ortho mode, modulation, dimension chains, open panels and the folded ribbon are remembered between sessions.

**Modo leve (computador simples)**, in *Exibir*, is for modest computers: it stops the continuous animations (the AI highlight pulse and the animated load path) and keeps less memory for undo (32 MiB instead of 128 MiB). It turns itself on when the computer has about 4 GB of RAM or less; your own choice is remembered.

## Drawing a plan

### 1. The lot

Choose **Terreno** (lot, `T`), type the exact width and length (for example 12 m × 25 m) and click **Criar terreno** (create lot), or drag it on the canvas, or click two opposite corners. Lots can also be irregular polygons with sloped boundaries, and they can be subdivided. In Properties, **Recuos e limite de implantação** (setbacks and buildable limit) sets front, side and rear setbacks; Trazo then shows the buildable area and refuses edits that leave it.

### 2. Walls

Choose **Parede** (wall, `P`) and drag, or click the two ends. Snapping to the grid (0.25 m), wall ends, midpoints, intersections, perpendiculars, tangents, centres and extensions (**OSNAP**) helps precision; ortho mode (`F8`) keeps walls horizontal or vertical. While drawing you can type a length, an angle or coordinates (`@dX;dY`, `3<45`).

- **Masonry.** In the options bar pick the brick or block: ceramic bricks 9×19, 11.5×19, 14×19 and solid 10×20; structural ceramic block 14×29; concrete blocks 9, 14 and 19 × 39. Or type the two plan dimensions, joint and plaster in cm. Walls are drawn with their real finished thickness (block plus plaster on both faces).
- **Multilayer walls.** A wall can be built from layers (masonry, render, insulation, finish), each with its own thickness and material.
- **Reference line.** Walls can be placed by their axis, inner face or outer face.
- **Modulation** (*Modulação*). When on, wall lengths and room sides snap to half-block modules (length + joint), so walls close with whole and half blocks. Properties show each wall's first course and, for the project, the wall area and a block estimate for the chosen ceiling height, net of openings.
- **Junctions.** L, T and X junctions are mitred automatically, at any angle and with different thicknesses.

### 3. Rooms

- **Cômodo** (room, `C`): drag or click two corners, then name the room. Its four walls are created and joined to the walls already there; walls in line with existing ones are shared, never duplicated.
- **Cômodo livre** (free-shape room): click the corners of an L, U or any polygon.
- **Detectar** (detect): finds rooms automatically from walls that already enclose a space.

Moving a wall between two rooms updates both rooms, their doors and windows and the dimensions; nothing has to be redrawn. Vertices of free-shape rooms can be dragged directly.

### 4. Doors and windows

Drag **Porta** (door, `R`) or **Janela** (window, `J`) from the ribbon onto a wall, or pick the tool and click a wall; the wall that will receive it is highlighted. A door swings towards the side you clicked.

- **Door types:** hinged, double, sliding, pocket, pivot, bifold and revolving; **Vai-e-vem** adds double-acting saloon doors.
- **Window types:** sliding, top-hung (*basculante*), awning (*maxim-ar*), casement, tilt-and-turn, fixed and bay, each with its technical plan symbol.
- **Vão livre** (open span) leaves an opening with no frame.
- **Portão** (gate) goes in a boundary or front wall: 3.00 m sliding by default, from 0.80 m (pedestrian gate) to 8 m (vehicle gate). In Properties choose sliding, swing, double, pivot or folding. Gates are tagged "PT1 300/220".
- Openings keep a minimum distance from wall ends and never overlap. In Properties set width, height and, for windows, the sill; turn a door to the other side of the wall (`I`) or move its hinge to the other jamb (`Shift+I`).

### 5. Levels, stairs and floors

- **Escadas** (stairs): straight, L and U stairs with a Blondel comfort check, accessible ramps checked against the ABNT NBR 9050 slope limits, landings, railings and level changes.
- **Pavimentos** (floors): basement, ground and upper floors, each with its level, ceiling height and slab thickness. The floor below can be shown as a faded underlay while you draw the one above, and a new floor can start as a copy of the walls below. The area schedule totals all floors.

## Selecting and editing

Choose **Selecionar** (select, `V`) to inspect and edit elements from the plan or the project browser.

- **One element.** Walls take a numeric length; rooms a name, width and length (connected openings and shared walls are kept); openings can be dragged along the wall; fixtures turn 90° with `G` and resize in Properties.
- **Several at once.** `Shift`+click adds or removes an element. Dragging from empty space draws a selection box: **left to right** selects by window (elements wholly inside, solid blue box), **right to left** by crossing (anything it touches, dashed green box). `Ctrl+A` selects everything.
- **By kind.** The *Editar* tab selects similar elements (doors of the same size, the same fixture, walls of the same thickness) or the inverse, and the **selection filter** (*Filtro*) limits which kinds boxes and select-all pick. In the project browser, `Shift`/`Ctrl`+click adds rows, and **Selecionar os encontrados** selects everything the text filter shows.
- **Moving.** Dragging any selected element moves them all; the arrow keys move the selection 0.25 m (1 m with `Shift`). **Excluir** (delete) or `Delete` removes it.

## CAD modify tools

All of these live on the *Editar* tab, the *Editar* menu and the context menu. Each operation is a single undo step, and doors and windows keep their place on site.

| Tool | What it does |
| --- | --- |
| Copy, cut, paste, duplicate | `Ctrl+C`, `Ctrl+X`, `Ctrl+V` (pastes at the pointer, keeping the grid), `Ctrl+D`. Rooms are copied with their walls, doors and windows. |
| **Mover**, **Copiar** | Move or copy from a base point, with typed distances (`@dX;dY`, `3<45`). |
| **Girar** | Rotate about a centre by clicking the angle or typing it. `G` / `Shift+G` turn the selection 90° clockwise / counter-clockwise. |
| **Espelhar** | Mirror across any axis, optionally keeping the original. `H` / `Shift+H` mirror left-right / upside down in place. |
| **Deslocar** (offset, `O`) | Parallel copy of a wall (same material) or of a line, polyline, rectangle, arc, circle or spline, on the side clicked. |
| **Aparar** (trim, `A`) | Removes the clicked piece of a wall or drawing between its nearest crossings. |
| **Estender** (extend, `E`) | Runs the nearest open end up to the next wall, drawing or lot edge. |
| **Dividir** (divide, `D`) | Cuts a wall or drawing where you click, or into 2–100 equal parts. Cuts never go through an opening; a circle divides into arcs. |
| **Unir** (join, `U`) | Joins collinear walls end to end, or lines, open polylines and arcs into one drawing. |
| **Alinhar**, **Distribuir** | Align the selection on an edge or centre; space three or more elements evenly. Rooms that share walls move as one block. |
| **Agrupar** (`Ctrl+G`) | Groups elements: one click picks the whole group, `Ctrl`+click picks a single member, `Ctrl+Shift+G` ungroups. Groups are saved and listed in the browser. |
| **Repetir** | Copies in rows and columns or around a centre, up to 400 at once. |

### Constraints

**Paralelas** (parallel), **Perpendiculares** (perpendicular), **Coincidentes** (coincident points) and **Distância fixa** (fixed distance, between two points, two parallel lines, or the length of a line clicked twice). The first pick is the reference; the second element moves to satisfy the constraint, turning about the corner it shares with the reference so joined walls stay joined.

From then on every edit (dragging, typed lengths, transforms and the AI tools) keeps the constraints, and an edit that would break them is refused with the reason. Constraints are saved in the file, copied with the elements they tie, listed in the browser and shown on the plan (∥, ⊥, ●, ↔).

## Architecture tools

- **Areas.** Usable, built, projection and lot areas are computed separately, along with the occupancy rate, live in Properties and in the schedules.
- **Finishes.** Per room: floor, wall, ceiling and baseboard finishes, wet-area waterproofing and specification notes, with quantities (floor + 10 % waste, wall area, baseboard length). Finish packages apply a full set at once.
- **Circulation check** (*Circulação*, on the *Interiores* tab): checks passage widths, furniture collisions, door swings blocked by furniture and the functional clearance in front of each fixture (ABNT NBR 9050 and residential ergonomics), and highlights each problem on the plan.

## Interiors

The *Interiores* tab places counters, sinks, stoves, a wood stove, a barbecue grill, a fridge, washbasins, toilets, showers, sofas, beds, wardrobes, tables, desks, a washing machine and shelves. **Biblioteca** (library) opens the full catalogue of furniture, equipment and technical symbols, which can be customised.

Furniture uses dimension-aware vector symbols, with cushions and upholstery seams,
bedding, basins and taps, appliance controls, cabinet divisions and detailed chairs.
Fine details appear as you zoom in and are preserved in SVG, PDF, PNG and DXF exports.
Small ribbon icons retain the main outlines for readability. Hover over an object
or its library preview for a larger view.

The library shows model previews, dimensions and seating counts. Search accepts
unaccented words such as `sofa` and `lavatorio`. **Posicionar com mouse** preserves
the selected model's dimensions, including Queen/King beds and larger sofas; **G**
rotates the next object. Chairs outside a table and car mirrors are included in
selection and display extents. Width and depth still describe the furniture body.

Near a straight wall, wall-backed furniture turns its back to the wall and stands against the face of the bricks. Dining tables, cars and poolside objects remain freestanding where you click. Fixtures drag in 5 cm steps, nudge with the arrows, turn with `G` and resize in Properties. A washbasin 0.90 m or wider becomes a double sink.

Room names and sizes find the free spot nearest the middle of each room, clear of furniture and door swings, both on screen and in exported drawings.

## Review marks

Use **Marcações** for reminders, questions and design reviews. These marks are saved in the `.trazo` project, belong to their floor and do not change construction geometry, loads or quantities.

- Choose **Retângulo**, **Círculo**, **Seta** or **Traço livre**, then a colour. Rectangles use opposite corners; circles use a centre and a point on the circumference; arrows use a start and a tip. Click twice or drag. Freehand marks are drawn by dragging; Esc cancels the draft.
- Every mark receives a stable **M** identifier. **Selecionar** lets you drag a mark, drag its blue control points to resize it, or press Delete to remove it. Hover to read its comment.
- Select elements before opening the tab, then choose **Marcar seleção** to enclose them and keep explicit references to those elements.
- **Lista e comentários** searches identifiers/comments, locates marks and edits colour, comment, coordinates and resolved state. **Salvar comentário** applies an undoable change. You can also set a comment for subsequent marks.
- **Mostrar marcações** hides the overlay; **Mostrar resolvidas** includes completed marks. **Incluir na exportação** is off by default and adds the visible marks to plan/sheet SVG, PDF, PNG and DXF exports when enabled. DXF uses its supported indexed colour palette.

Connected clients can read the same identifiers, colours, comments, floor and real coordinates through MCP. A request such as “Move the cabinet in the red mark M12” therefore refers to the same part of the drawing for both people and connected tools. See [MCP review marks and resources](mcp.md#review-marks-and-project-resources).

## Electrical design

The *Elétrica* (electrical) tab designs the installation on the floor plan, following ABNT NBR 5410. Devices are drawn in red on their own layer, *Instalações elétricas*, which can be hidden.

| Group | Tools |
| --- | --- |
| **Tomadas** (outlets) | **Baixa** (0.30 m), **Média** (1.20 m), **Alta** (2.20 m) and **TUE** (specific use, such as an electric shower or oven) |
| **Iluminação e comando** | **Luz teto** (ceiling light), **Arandela** (wall light), **Interruptor** (switch) and **Paralelo** (two-way switch) |
| **Quadro e dados** | **Quadro** (distribution board) and **Dados/TV** |
| **Projeto (NBR 5410)** | **Gerar pontos**, **Circuitos**, **Quadro de cargas** and **Unifilar** |
| **Eletrodutos** (conduits) | **Gerar**, **Ligar pontos**, **Piso / teto** and **Remover** |
| **Exibição** | The circuit voltage (127 or 220 V), **Só elétrica** (the electrical view: see [Discipline views](#discipline-views)) and **Ocultar elétrica** |

Wall devices stand on the face of the nearest wall, like furniture; ceiling lights go where you click. In Properties, each device has its circuit, power in VA and mounting height. Tags such as "C2 · 600 VA" show the circuit and power once the plan is zoomed in, and always on paper.

- **Gerar pontos** (generate points) adds what the norm asks for and each room still lacks, classifying rooms by name (*Cozinha*, *Banheiro*, *Quarto*…):
  - a ceiling light with the minimum lighting load (100 VA for the first 6 m², plus 60 VA per 4 m² beyond);
  - general outlets spread along the walls, clear of doors and windows: one per 3.5 m of perimeter in kitchens and service areas, one per 5 m in other rooms, at least one in bathrooms, with 600 VA for the first three in wet rooms;
  - a switch beside the door, and a 5,500 VA specific-use point at each shower.
- **Circuitos** numbers the circuits again: lighting apart from outlets (up to 10 A each), kitchens and service areas on circuits of their own, and one circuit per specific-use outlet. Switches follow the lights of their room.
- **Quadro de cargas** (load schedule) shows the forecast of each room next to what is drawn, and each circuit's power, voltage, current, wire section, breaker and the need for a 30 mA RCD (wet and outdoor areas), with warnings for anything the norm asks to change.

Wire sections assume copper with PVC insulation, method B1, two loaded conductors at 30 °C (NBR 5410 table 36), with minimums of 1.5 mm² for lighting and 2.5 mm² for outlets. The design is a starting point: have it checked by a licensed professional.

### Conduits and wiring

Conduits (*eletrodutos*) join the devices to the distribution board. They are drawn as in Brazilian practice: a gentle arc between two points, solid when it runs in the ceiling or down a wall and dashed when it runs in the floor. Each run shows one tick per conductor and the circuit number beside them:

| Tick | Conductor |
| --- | --- |
| Tick with a flag | Phase (*fase*) |
| Plain tick | Neutral (*neutro*) |
| Tick with a ring | Switch return (*retorno*), and the travellers between two-way switches |
| Tick with a bar (T) | Protective earth (*terra*) |

- Insert the **Quadro** (board) first. **Gerar** then links every device not yet reached: ceiling runs from the board to the ceiling lights, spread in several branches instead of one long chain, and a run from the light of each room to its outlets and switches. Existing conduits stay as they are.
- **Ligar pontos** joins the selected devices (Shift+click), in the order they were picked. **Piso / teto** moves the runs between the selected devices to the floor or back to the ceiling, and **Remover** deletes them. With a single device selected, both act on all its runs. Deleting a device removes its conduits.
- The wires of each run come from the circuits beyond it: phase, neutral and earth to outlets and lights, phase down to a switch and the return back up, two travellers between a pair of two-way switches, and two phases with no neutral for a 220 V circuit on a 127 V network.
- Each conduit is sized for its wires with the NBR 5410 fill limits (53 % for one conductor, 31 % for two, 40 % for three or more). Runs above the usual 20 mm show their diameter, such as "Ø25". When a run has too many wires to tick, it shows the circuits, the number of wires and the size instead ("1–4, 7 · 15 fios · Ø32").

Sheets with the electrical layer visible add a legend of the ticks, and the conduits go to PDF, SVG, PNG and DXF like the devices.

### Distribution board and single-line diagram

The *Quadro e diagrama unifilar* tab of the *Projeto elétrico* window (or **Unifilar** on the ribbon) puts together what the board needs:

- **Supply** (*alimentação*): automatic or forced to single, two or three phases. Automatic picks single-phase up to 8 kVA of demand, two-phase up to 15 kVA (or whenever there are 220 V circuits between phases), and three-phase above that, warning above 75 kVA.
- **Phase balancing**: circuits go on phases A, B and C from the largest down, each on the least loaded phase, and two-pole circuits on the least loaded pair. A warning appears when the phases differ by more than 10 %.
- **Voltage drop**: with a board on the plan, each circuit's cable length runs along its conduits (or square to the axes, without them), plus the drops from the ceiling to the board and to each device. The drop uses ρ = 1/56 Ω·mm²/m, and a wire is raised to the next section until the drop is 4 % or less (NBR 5410 6.2.7), marked with an asterisk.
- **Demand**: lighting and general outlets use the usual utility factor for their installed load (0.86 up to 1 kVA, down to 0.24 above 10 kVA), and specific-use circuits use the factor for how many there are (1.00 for one or two, 0.76 for four…).
- **Service entrance** (*padrão de entrada*): the demand current on the most loaded phase gives the main breaker, the feeder section (at least 6 mm²), the earth conductor (NBR 5410 table 58) and the entrance conduit.

The single-line diagram shows the meter, the main breaker, the surge protector (DPS), the busbar and one branch per circuit with its breaker, RCD, wire, phases and description, next to the panel schedule with the load on each phase, the demand and the entrance. **Exportar PDF** and **Exportar SVG** save it as an A3 sheet. On drawing sheets that show the electrical layer, a compact *Quadro de cargas (QD)* table joins the other schedules.

Each utility publishes its own entrance tables: check them before you order the meter box.

### Electrical shopping list

The *Lista de materiais* tab of *Projeto elétrico* consolidates everything to buy from the conduits and load schedule: copper PVC 750 V wires grouped by section and NBR 5410 colour (phase, light-blue neutral, return, green/yellow earth) with 100 m rolls, corrugated/rigid conduits by nominal diameter with 50 m coils, 4×2″ wall boxes, 4×4″ octagonal ceiling boxes, DIN distribution boards with 30 % spare space, main/feeder/terminal breakers, 30 mA RCDs, Class II SPDs and finishing devices. Adjust the waste percentage (default 10 %) and export to CSV, PDF or SVG.

### Lighting calculation (lux per room)

The *Iluminação (lux)* tab of *Projeto elétrico* computes each room's maintained illuminance by the Lumen Method against the ABNT NBR ISO/CIE 8995-1 targets (100 lx corridors/garages, 150 lx living/bedrooms, 200 lx bathrooms/dining, 300 lx kitchens/laundries, 500 lx offices). You can set the default ceiling and wall luminaire lumens (or override lumens on any individual light point in *Propriedades*), surface reflectances and maintenance factor `Fd`. Each room shows its calculated lux directly on the electrical plan and exported sheets, feeds the *Segurança* traffic light, and exports to PDF, SVG and CSV.

### Photovoltaic pre-sizing

The *Solar (Fotovoltaico)* tab of *Projeto elétrico* pre-sizes a rooftop grid-tied PV array for the project's city (27 Brazilian cities from INPE/CRESESB, or a conservative 4.20 kWh/m²·day fallback when no city is set): enter your monthly bill in kWh/month (or use the automatic estimate), module wattage (default 550 Wp) and Performance Ratio (default 78 %). Trazo packs the modules onto the best-oriented roof planes first (North → East/West → South), sizes the inverter and AC protection, checks the 75 kW microgeneration limit and roof dead load (`0.15 kN/m²`), displays the 12-month generation profile, draws the modules on the plan and exports to PDF, SVG and CSV.

### Multiple boards, short-circuit and breaker selectivity

Place more than one **Quadro** on the plan to model a main board (`QDG`) feeding secondary sub-boards (`QDC1`, `QDC2`, …). The *Curto e seletividade* tab of *Projeto elétrico* estimates the prospective three-phase and single-phase short-circuit currents (`Icc`) at each board from the utility transformer, service drop and feeders, verifies the breakers' breaking capacity (`Icn`), checks cumulative feeder + terminal voltage drop (≤ 4 %) and cable thermal withstand, and evaluates upstream-to-downstream breaker selectivity (`DG → QDC → Ck`), exporting to PDF, SVG and CSV.

### Earthing layout and simplified lightning-protection risk (SPDA)

The *Aterramento e SPDA* tab of *Projeto elétrico* lays out the earthing electrode array (BEP + Copperweld grounding rods spaced 3 m outside the building inside the lot, drawn on the electrical plan and exported sheets), sizes the rod count and earthing/bonding conductors by Dwight's equation and NBR 5410 §6.4, and runs a simplified ABNT NBR 5419-2:2015 risk screening (`R1` vs. `RT = 10⁻⁵/year`) for houses up to 15 m tall, naming the required protection level (`NP IV`–`NP I`) or refusing buildings above 15 m for specialist design. Export to PDF or SVG.

## Structure

The *Estrutura* (structure) tab lays out a first reinforced-concrete structure over the floor plan and pre-sizes it. Columns, beams, slabs and footings are drawn as a formwork plan (*planta de forma*) on their own layer, *Estrutura*, in dark teal.

| Group | Tools |
| --- | --- |
| **Elementos** | **Pilar** (column, 30 × 14 cm) and **Sapata** (footing, 80 × 80 cm). Both snap onto the nearest wall junction, or onto the wall's centre line, turned along the wall |
| **Cargas** | **Caixa-d'água** (10.81 kN by default, including 1,000 L of water plus tank/fittings) and **Equipamento** (an illustrative 15 kN default that must be replaced with manufacturer data). Place them over a slab or beam and edit their footprint and characteristic load in Properties |
| **Vigas** (beams) | **Ligar pilares** joins the selected columns (Shift+click) in order; **Remover vigas** removes the beams between them, or all the beams of one column |
| **Projeto (NBR 6118)** | **Gerar estrutura**, **Pré-dimensionar**, **Esforços**, **Ajustar sapatas**, **Vigas alavanca** and **Baldrames** |
| **Exibição** | **Só estrutura** (the formwork view) and **Ocultar estrutura** |

Columns and footings are objects like furniture: they select, move, copy, rotate and undo, and their section is their width and depth in Properties. Beams name the two columns they join, so they follow them, and deleting a column removes its beams. Labels are numbered in reading order, top to bottom and left to right: P1, P2… for columns, V1, V2… for beams (horizontal first, then vertical, a continuous beam keeping one name), L1, L2… for slabs and S1, S2… for footings, each footing taking the number of its column.

- **Gerar estrutura** (generate the structure) adds what is missing: a column at every wall junction and free end, more along walls longer than 4.5 m (clear of doors and windows), a beam along every wall between consecutive columns (no beam on spans under 60 cm, where two columns act together), a lattice slab (*treliçada*) over every room and a footing under every column, sized by its load. Existing elements stay.
- **Pré-dimensionar** (pre-size) opens the *Estrutura* window. Its tabs show element sizing (*Pré-dimensionamento*), continuous beam forces (*Esforços nas vigas*), preliminary reinforcement (*Armaduras*), soil and foundations (*Solo e fundações*), timber roofs (*Telhado*), coupled 2D plane frames (*Pórticos 2D*) and load cases & combinations (*Cargas e combinações*). The materials and loads are set there (concrete fck, allowable soil pressure, floors carried, occupancy class, live load and finishes), and each element is checked:
  - **Slabs**: lattice or solid, with the usual minimum thickness for the smaller span (lattice slabs from 12 cm up to 3.5 m; solid slabs L/40, at least 8 cm). The load is the self weight (a lattice slab weighs about 55 % of a solid one), the finishes and the live load (1.5 kN/m² for dwellings, NBR 6120). A solid slab shows every rectangular panel (including the pieces of an L-shaped room), the governing Czerny/Marcus moments in both directions and its long-term flecha against L/250. Those moments feed the two reinforcement directions. A lattice slab instead keeps the explicit manufacturer check.
  - **Beams**: each slab's load is shared by the supported edges with 45° yield lines. With the beam's self weight and the walls of the floors above, each continuous beam is analysed over its columns. The checks are the usual minimum height (L/10 at an end span, L/12 inside a continuous beam) and the 12 cm minimum width (NBR 6118 13.2.2).
  - **Columns**: the beam reactions, times the floors, plus the self weight. The concrete area is 1.4 · α · N / (0.85 fcd + 1.5 % · 420 MPa), with α = 1.3 for interior columns, 1.5 at an edge and 1.8 at a corner, and at least 360 cm² and 14 cm a side.
  - **Footings**: area = 1.05 · N / allowable soil pressure, with the same overhang on every side of the column and a height for a rigid footing. **Ajustar sapatas** resizes them all.
- **Esforços** (forces) draws every continuous beam with its supports, the bending moment diagram, the moments at the supports and in the spans, the shear, the reactions and the deflection. The analysis uses the stiffness method on pinned supports. Deflection uses the quasi-permanent load (g + 0.3q), NBR 6118 Branson cracked-section equivalent inertia (`Mr`, Stage I/II, `I_eq/Ic`, `As`) and a creep factor of 2.32, checked against L/250 (hover the Flecha column in *Pré-dimensionamento* to inspect Stage I/II, `Mr` and `I_eq/Ic`). **Exportar PDF** and **Exportar SVG** save it as an A3 sheet.
- **Armaduras** estimates longitudinal bars, stirrups, NBR 6118:2023 hooked anchorage lengths (`lb,nec`, distinguishing good and poor bond zones) for beams, longitudinal bars with starter lap splices (`l0`) and ties for columns, principal/distribution meshes for slabs and bottom meshes with 90° hooks for footings. It also gives the bar schedule with shapes, a CA-50 summary by diameter (commercial 12 m bars and +10% cutting/bending waste) and reinforcement traffic lights. **Exportar tabela CSV** saves the schedule and diameter summary; **Detalhamento PDF** / **Detalhamento SVG** export an A3 schematic detailing sheet; **Exportar memorial PDF** includes the full proposal. These outputs remain preliminary review aids traced to NBR 6118:2023 and require a licensed engineer's executive detailing (ART/RRT).
- **Concentrated loads** receive a `CC` label on the plan. If the footprint crosses a beam, the full load enters that beam. Otherwise, it enters the slab, is shared by the nearest supported edges and continues through beams, columns and footings. Solid slabs get a preliminary local bending and punching check; a lattice slab asks for a supporting beam or a solid strip/joist check from its manufacturer instead of inventing a resistance.
- **Stairs, balconies and cantilevers** (*Escadas, sacadas e balanços*) appear automatically in *Pré-dimensionamento* and *Segurança* whenever stairs, balcony rooms or 1-edge-supported slabs exist (or explicit cantilevers are declared): concrete stairs get waist thickness, bottom/top steel and `L/250` deflection; balconies and cantilever slabs enforce solid slabs, the **NBR 6118 Table 13.2 `γ_n`** safety factor for thin cantilevers, guardrail loads, top negative steel, **`L/125`** tip deflection and **`1.5 × L_bal`** back-span anchorage/overturning checks; cantilever beams check `L/5` depth, top steel and `L/125` deflection.
- Columns placed on diagonal walls keep the wall's exact angle, shown in Properties. In **Pré-dimensionar → Viga secundária apoiada em vigas**, choose two primary girders and the position along each span: the new beam ends there and its reactions enter the girders as concentrated loads. Assistants can make the same connection with `projeto_estrutural` → `apoiar_em_viga`.
- Every opening with masonry above it receives a **VA** lintel; each window also receives a **CV** sill. They appear on the structural plan, in the pre-sizing window, the professional-review PDF and `pre_dimensionamento`, with span, side bearings, section, preliminary bars and a traffic light. The same window checks each **AL** masonry panel for height/thickness and lateral bracing; a material whose name contains *estrutural* also gets the conservative compression check. Red means the opening/panel must change or be designed outside this simplified method.

A wall can have its own height. In Properties, **Altura** takes the height of the top at the start and at the end of the wall: equal values give a straight top, different ones a sloped top, such as the gable under a mono-pitch roof or a boundary wall raised to leave a ventilation gap. **Seguir o pé-direito** puts the wall back on the storey height. The real top appears in sections and facade elevations and counts in the masonry takeoff, in the weight of the wall on beams and foundations, and in the height of the columns standing in the wall. A tall column is checked for slenderness (λ): up to 35 it is green, up to 90 yellow (the final design must add the second-order effects), and above 90 red, with the side it needs.

**Baldrames** gives every ground-floor wall its foundation: a ground beam (*viga baldrame*) where the wall reaches two columns or connects to column-backed baldrames, and a strip footing (*sapata corrida*) elsewhere, such as the inner walls of a bathroom. A secondary baldrame passes its reaction as a point load to the supporting baldrame; the wall and beam weights then reach the footings. Unsupported beam cycles stay on strip footings, with a red check if a ground beam was requested manually. Run **Ajustar sapatas** again to size the footings with these loads.

For a wall taller than the storey, **Pré-dimensionamento** and **Vento** show the out-of-plane masonry check and the proposed intermediate ties and crown beam. The panel needs columns near both ends and a continuous foundation. The result includes tie spacing, section across the wall and steel; openings turn the local wind check red until their load path is designed, while also proposing sized grouted vertical concrete jambs (*montantes grauteados*) beside each opening. The plan, review PDF and `pre_dimensionamento` carry the same proposal.

The *Estrutura* window's **Solo e fundações** tab holds the site's SPT soil survey (supporting one or several boreholes `SP-01`, `SP-02`, …): one reading per metre with the blow count N and soil type, water table per borehole and footing base depth. **Usar na tensão do solo** takes the governing (weakest-borehole) allowable pressure for footings (`20 · N̄` kPa in the pressure bulb). Below it, the comparative foundation study evaluates **footings, a raft (*radier*), hand-dug bored piles (*brocas*, 3–6 m above the water table) and deep mechanized piles (*hélice contínua*, 6–20 m, allowed below the water table)** for the real loads, including immediate elastic and differential settlement estimates, and recommends the viable option with the least concrete. Both the SPT boreholes and the foundation study also appear in the review PDF memorial and MCP output.

The **Telhado** tab lays **mono-pitch (*meia-água*)**, **gable (*duas águas*)** and **hip (*quatro águas*)** timber roofs with configurable **overhangs (*beirais*, up to 1.50 m)**. Pick **Nova meia-água**, **Novo duas águas** or **Novo quatro águas** over the walls; set the roof style, high side, ridge/eave heights, overhang, tile, timber class, truss option and spacings of beams, purlins and rafters, and tick **sem forro** for an exposed roof. Trazo sizes battens, rafters, purlins, **wall plates (*frechais*)**, main beams, 4-panel **Howe timber trusses (*tesouras*)** and 4 diagonal **hip rafters (*espigões*)** (NBR 7190), checks overhang cantilever bending and **NBR 6123 wind uplift** (including overhang edge suction and anchor demand), counts tiles and timber, draws ridges/hips/overhangs on the plan, and puts each support reaction on the column, post or beam below. **Ajustar as paredes ao telhado** raises walls and slopes gables to meet the roof planes.

In **Telhado → Contraventamento**, enter the verified design resistance of the complete lateral path from the roof to the foundations. Trazo shows the required lateral force along with a preliminary CA-50 X-bracing tie bar specification and requires at least two roof supports on founded columns before treating a tall-wall column as braced at the top. Without that evidence it checks the column as a cantilever and also verifies its footing eccentricity (`e ≤ B/3`) and peak soil bearing pressure (`≤ 1.3·σadm`) under wind. **Vento** also shows NBR 8800 anchor-bolt proposals for roof uplift + wind shear and the global ±X/±Y screening: wind force, α instability indicator, lateral drift, overturning balance and NBR 6122 soil sliding (`FS ≥ 1.50`). All preliminary tie, bolt and footing proposals remain subject to final structural and geotechnical detailing.

**Mourão** (on the Estrutura ribbon) places a timber post; round when its width and depth are equal. A roof beam end with no column under it bears on the post next to it, or on a timber beam that Trazo lays along the edge from post to post. In the Telhado tab, choose a certified strength class or a catalogued species and confirm the exterior preservative treatment. Trazo lists height, vertical load, wind force/moment, slenderness, combined compression–buckling–bending use and concrete base for every post, plus the section and treatment of each eave beam. The steel anchor must keep the wood off the soil.

**Vento** at the bottom of the Telhado tab takes the city's basic wind speed V0 (from the NBR 6123 map) and the terrain category. Trazo then checks each column of a wall taller than the storey against the wind on its share of the wall (propped at the top where the roof bears on it, otherwise as a cantilever) and says how deep across the wall it must be, and checks whether the wind lifts each roof, giving the anchorage each beam end needs.

The **Cargas e combinações** tab consolidates the building's fundamental **NBR 6120:2019** load cases (`G1` structural self-weight, `G2` non-structural permanent loads and `Q` live loads, with room-by-room Table 10 checks and occupancy-specific `ψ0, ψ1, ψ2`), the whole-building **NBR 6123** wind analysis across `+X`, `−X`, `+Y` and `−Y` (`V0`, terrain category I–V, `S1`, `S3` Group 1–5 presets, Table 4 `Cpe`/`Ca`, storey-by-storey forces and `ELU-EQU` overturning), and all 17 **NBR 8681 / NBR 6118** ultimate (ELU) and serviceability (ELS) combinations. Every 2D portal frame in the **Pórticos 2D** tab solves those load cases and builds the governing ELU/ELS envelopes, checking both `ELS-F` lateral drift (`H/850`) and `ELU-6/7` column uplift/tension. The same tables and verdicts appear in the professional-review PDF memorial and MCP output.

In the *Estrutura* ribbon's **Simulação na planta** group, you can overlay live simulation results directly on the floor plan and in exported SVG/PDF sheets (screen equals paper): **Cores semáforo** (green/yellow/red utilisation colours on slabs, beams, columns and footings), **Momento (M) / Cortante (V)** diagrams drawn along each beam, **Deformada** (exaggerated beam deflected shape with `L/xxx`), and **Caminho cargas** (animated slab → beam → column → footing load flow on screen, directional arrows on paper), all with overlap-free labels. Click **E se?** to open the interactive **What-if** simulator (sliders for span, beam/column/slab sections, loads and concrete/soil materials updating live in under 100 ms with green/yellow/red safe-range bars and one-click *Aplicar*), or **Grelha** to inspect the coupled slab + beam grillage analysis (**Analogia de Grelha**, including L-shaped and irregular slabs, deflection heatmap, `mx/my/mxy` moments, beam torsion and PDF/SVG export). Structural and electrical analyses run in the background with cached results so large projects stay responsive.

On a wall along the lot line, columns and footings stand flush inside the lot, so the footing is off centre from its column. **Vigas alavanca** ties each such footing to the nearest interior column with a balance beam: the footing is then centred under the lever's reaction, the interior column is relieved, and the beam is sized with its top steel. If the interior column is too light, the traffic light shows that the beam would lift it.

Warnings name each element that needs attention, including a support that a short span next to a long one would lift. Sheets that show the structure add a *Quadro de pilares e sapatas* table. This is pre-sizing for a first layout: the structural design belongs to a licensed engineer. If the family cannot pay for one, free technical assistance (ATHIS, Lei 11.888/2008) is available through the municipality, CAU or CREA; see [where to find free help](proposta.md#onde-encontrar-ajuda-gratuita).

### Safety and limits

**Segurança** (on the *Estrutura* and *Elétrica* tabs) shows every engineering check as a traffic light:

- **green** passes with a comfortable margin, using up to 85 % of the limit;
- **yellow** passes with a small margin;
- **red** fails.

Each element is listed worst first, with its margin and one sentence on what to change. A sentence for the family sums the whole light up, and hovering a check's name explains the technical words it uses (pilar, sapata, fck, punção…). **Detalhes técnicos** adds the formula and the standard, and the assumptions close the list.

Trazo refuses to size what its methods do not cover, and says why:

- more than three storeys;
- a storey taller than 4 m;
- a live load above 5 kN/m²;
- a new floor over an existing house whose foundation and columns nobody knows;
- a beam span over 7 m;
- a smaller slab span over 6.5 m;
- footings on soil weaker than 100 kPa;
- an electrical demand above 75 kVA.

A project-wide limit blocks *Gerar estrutura* and *Ajustar sapatas*; an element beyond its limit shows no numbers, only the reason and where to find help.

**Perguntas sobre o terreno** in *Segurança* asks what Trazo cannot see: a slope, a nearby stream or canal, landfill, water in the ground, cracks in the neighbouring houses, digging next to a neighbour's foundation. Answer **Sim**, **Não** or **Não sei**. While a question has no answer, the *Riscos do terreno* check is yellow; a **Sim** turns every footing, ground beam, strip footing, pile cap and balance beam red with "stop and get help" instead of sizing it. An assistant can record a risk, but its "no" counts only after you confirm it.

Two more questions cover building on top of a house that already exists. If you will, and nobody knows how its foundation and columns were made, Trazo refuses to size the structure and explains what to find out first: the kind and size of the foundation (dig carefully beside a column without removing soil from under it), the size and steel of the columns and beams, and any cracks, rust or sagging slab.

**Construção em etapas** in *Segurança* asks how many floors you mean to build later (0 to 2): from then on the columns carry them, so the ground floor's footings, columns and slab are sized for the whole house (run **Ajustar sapatas** again). Once the foundation is built, click **A fundação já foi feita**: any later extension beyond the storeys it was built for turns red. With more than one floor already drawn, draw the future floor as a floor instead.

**Onde buscar ajuda** in *Segurança* opens by itself whenever something is red: free technical assistance (ATHIS, Lei 11.888/2008) through the municipality, CAU or CREA, university model offices and the Defesa Civil (199). Type local contacts there (one per line) and **Salvar contatos**: they go into the project and the report. The methods and limits are in [Engineering methods](engineering.md).

**Check the hypotheses behind a green.** Every result rests on the values listed at the end of *Segurança* and in the report: concrete strength, allowable soil pressure, loads and wind. When data is missing, Trazo takes the safe side: 100 kPa (1 kgf/cm²) for the soil without a survey, C20 concrete without a specification and V0 = 45 m/s while the city is unknown (set it in *Normas › Local e clima*). The **Hipóteses de cálculo** panel at the top of *Segurança* shows each value with who set it (Trazo's default, you or the AI) and its evidence. A value more favourable than Trazo's safe value without evidence, or one below the standard, is contested: it turns red and no structural result that depends on it stays green. Type the evidence (the soil survey report, the concrete invoice) and click **Informar prova**, or click **Usar o valor seguro**. When an AI assistant tries to make a hypothesis more favourable, Trazo does not apply it: the proposal waits in the same panel until you **Confirmar** it with the evidence, or **Recusar** it.

**Caderno da família (PDF)**, in the same window, saves a few A4 pages in plain language to take to the hardware store, the builder and the reviewer: how the house stands in one sentence, what must not be built as it is, the shopping list, the building guide, when to stop and where to get help, and the plan.

**Guia de obra (texto)**, in the same window, saves the building guide: the checked house as instructions in the order of the work (site and setting out, foundation, columns and beams, walls, slab, roof, electrical), each stage with what to buy, the checklist to tick before pouring and where a professional should look.

**Exportar memorial (PDF)**, in the same window, saves the pre-sizing report (*memorial de pré-dimensionamento*) for a licensed professional to review. It contains:

- a cover with the overall traffic light and a review box (name, CREA or CAU registration, ART or RRT number, date, signature and opinion);
- the inputs, the assumptions and the applicability limits;
- the slab, beam, column, footing and circuit tables with their lights;
- the preliminary reinforcement of beams, columns, slabs and footings, its bar schedule and theoretical steel mass;
- every check with its margin and what to change, and the warnings;
- before each pour (footings, columns, beams, slabs): each element's bars and a checklist to tick beside the form;
- where to find help: free technical assistance, university model offices, the Defesa Civil and the project's local contacts;
- the drawings: the formwork plan, the beam forces, the electrical plan and the single-line diagram, as available.

The report pages are A4 and numbered; the drawings follow at their own sheet size.

## Water, sewage and rain

The plumbing design starts from the fixtures you place with the *Interiores* or *Hidráulica* tabs: toilet, washbasin, shower, kitchen sink, washing machine, bathtub and stove. Nothing else needs drawing. The *Hidráulica* tab also chooses how the water is heated (**Chuveiro elétrico**, **Aquecedor a gás** or **Aquecedor solar**), turns on the plumbing view (**Só hidráulica**) and hides or shows the plumbing layer, *Instalações hidrossanitárias*.

- **Points** (*Exportar* › **Projeto hidrossanitário (PDF)**): every fixture gets its points at the usual heights from the finished floor: cold water (**AF**), hot water (**AQ**, only when the water is heated by gas or the sun; with electric showers there are no hot-water pipes), the drain (**ES**, with its DN: 100 for the toilet, 50 for the kitchen sink and the washing machine, 40 for the others) and gas for the stove. The sheet has the legend, the count of each kind of point and the fixture schedule (*quadro de aparelhos*) with the rooms, the heights, the weight each fixture adds to the water flow (NBR 5626) and its drainage units (UHC, NBR 8160). The heights are the usual ones: the fixture's manual has the last word.
- **Cold water** (NBR 5626, the *Água fria* sheet): the family's water for a day (two people per bedroom and 150 L each, unless you say otherwise), the commercial water tank that keeps it (one day by default; two where the street water often fails) and how much it weighs full, a riser (*coluna* AF-1, AF-2…) for each wet room, the pipes sized by the fixtures' weights with their diameters, and the pressure left at every fixture: green when good, amber when a shower runs but weakly, red below the standard's least. When the tank sits right on the slab, a shower head at 2.10 m usually gets too little: Trazo says how high the tank's base must go (or to use a pressurizer) before the base is built. The *Hidráulica* view shows the same on the plan, and *Segurança* lists it: pressure problems are yellow (the fixture works badly), while a full tank heavier than the load the slab was checked for is red.

## Discipline views

Architecture, electrical, plumbing and structure are views of the same model. The tabs choose what the plan shows:

- **Só elétrica** (on the *Elétrica* tab) puts the architecture behind a veil and draws the electrical design on top, without the structure.
- **Só hidráulica** (on the *Hidráulica* tab) does the same for the fixtures that take water and the water (blue), hot water (red), drain (brown) and gas (amber) points, with their heights when you zoom in.
- **Só estrutura** (on the *Estrutura* tab) does the same for the formwork plan, without furniture or electrical devices.

Each tab of the window keeps its own view. Every sheet in a sheet set has a discipline too: **+ Planta de Instalações Elétricas**, **+ Planta Hidrossanitária** and **+ Planta de Forma** add sheets that show only their layers (with their schedules), and the *Disciplina* list changes it for any sheet. An architecture sheet leaves out the electrical design, the plumbing and the structure.

## Documentation and output

### Annotation

- **Dimension chains** (*Cotas*, status bar or *Vista*): every wall thickness and clear span in centimetres along the top and left of the building, plus totals.
- **Associative dimensions & opening chains:** internal room dimensions, external, aligned and angular dimensions, plus one-click automatic opening dimension chains (*Cadeias de aberturas* / *Cotas completas*) locating every door, window, gate and gap along its wall with outward offset and live updates as openings move or resize, with editable override text.
- **Door and window tags:** openings of the same kind and size share a code numbered from the smallest (P1, P2… for doors, PV for saloon doors, J for windows, V for open spans), tagged as "P1 80/210" or "J1 120/100" (width/height in cm).
- **Texts and symbols:** notes, leaders, callouts, north arrow, level marks, slope and entry arrows.
- **Hatches** by material, with exact clipping on screen and in vector export.
- **Layers** (*Camadas*) with colour, line weight, line type, visibility and lock.
- **Medir** (measure, `M`): click two points to keep a measurement on screen; it never changes the project. `Esc` or **Limpar medidas** clears them.

### Sheets and views

![A plan exported by Trazo](images/plan-export.png)

- **Prancha** (sheet): ISO A4 to A0 or a custom size, portrait or landscape, ABNT margins, and exact print scales of 1:25, 1:50, 1:75, 1:100 and 1:200 (**Melhor escala** picks the largest that fits). The title block holds the drawing, client, author, registration (CAU/CREA), address, date, sheet number and revision.
- **Quadros** (schedules): areas, doors and windows, rooms, materials and construction quantities, configurable and placed on sheets.
- **Conjunto para obra A3** (*Exportar* menu, PDF): the whole construction set in one click, numbered `01/NN`: the dimensioned plan at 1:50 when it fits with all its dimension chains, otherwise the largest scale that does (with the opening dimension chains, added only to the copy that is printed, the door and window schedule and the legend; when the lot alone would force a smaller scale, the plan leaves it out, since the setting-out plan shows it), the setting-out plan, the first-course plan, the wall elevations, the roof plan when there is a roof, the plumbing sheets when there are fixtures that take water, and the quantities takeoff.
- **Primeira fiada e paginação** (*Exportar* menu, PDF): an A3 first-course plan with every block of each wall's first course (whole, half and cut, in colours), the doors that interrupt it, the lintels (VA) and sills (CV), and the count per wall; then the elevation of every wall, course by course in running bond, with its doors, windows and concrete bands and the count of whole, half and cut blocks. 1:50 whenever it fits.
- **Planta de locação** (*Exportar* menu, PDF): the building on the lot with the origin at the lot's lower-left corner, the string lines (*gabarito*) through every wall axis and column centre, the cumulative distances along X and Y, and a table of every corner (A, B, C…) and column (P1, P2…) with its X and Y in metres.
- **Planta de cobertura** (*Exportar* menu, PDF): each roof's water planes with an arrow showing where the water runs and the slope against the tile's minimum (red when below it), the ridge, hips, eaves, verges and high edge, a gutter on every eave with its length, drained area and, when the city is set, its NBR 10844 design flow, and the downpipes (Ø 100 mm, one for every 78 m² of roof, at least one per gutter). Final rainwater sizing follows NBR 10844.
- **Quantitativos** (construction quantities): active floor or consolidated across all floors, with adjustable block waste %, per-masonry totals (whole blocks with waste, half blocks, bond-beam and lintel U-blocks with waste, laying mortar, plaster and U-block grout fill), per-wall breakdown, lintels and sills per opening with schedule codes (`P1`, `J1`…) and 30 cm bearings, per-room finishes (floor +10 %, skirting, net walls +10 % using real wall heights, ceiling, waterproofing and specifications), optional placement on ABNT sheets and export to CSV, PDF and SVG.
- **Vistas** (views): sections, elevations and detail callouts with cross-references between drawings.
- **Pranchas** (sheet sets): multi-sheet sets with automatic numbering (`01/03`), revisions (`R00`, `R01`…), issue status and a master document list. To select **Liberado para obra** while the *Segurança* window has red checks, enter the reviewing professional's name, numbered CREA/CAU registration and review date. The PDF/SVG sheet records the reviewer. If a red check changes or a new sheet revision is issued, the effective status returns to **Para aprovação** until reviewed again. This is a recorded professional acknowledgement, not a digital signature or a substitute for ART/RRT.
- **Modelos** (templates and sample projects): ready-made floor plan, site plan and wet-area detail setups, plus four full sample projects (`examples/casa-terrea.json`, `examples/sobrado.json`, `examples/apartamento.json` and the structural reference `examples/area-gourmet.json`).
- Documentation styles follow ABNT NBR 6492:2021 and NBR 16752, with visual and monochrome site presets. Checking the requirements that apply to a given project remains the designer's responsibility.

### Export

| Format | Use |
| --- | --- |
| **PDF** | Vector drawing, or a sheet at exact scale. |
| **PNG** | High-resolution image. |
| **SVG** | Editable vector drawing for other drawing programs. |
| **DXF** | AutoCAD, QCAD and LibreCAD exchange (import and export) with layers, colours, units, walls, openings, fixtures, drawings, dimensions, texts and hatches. |
| **CSV** | Area, door and window and quantity schedules for spreadsheets. |

## Files and data safety

- Projects are JSON internally and use the `.trazo` extension, saved atomically with a `.trazo.bak` backup of the previous version. Older `.trazo.json` and `.json` files remain supported and are migrated on open.
- A crash-recovery draft is written in the background and offered at the next start.
- The draft and the interface preferences live in `%LOCALAPPDATA%\Trazo\` (`recovery.json`, `settings.json`), or in the system temporary folder when `LOCALAPPDATA` is not set.
- Opening or creating a project never drops unsaved changes without asking.
- **Several projects in tabs:** **Nova planta** (`Ctrl+N` or `Ctrl+T`) and **Abrir** (`Ctrl+O`) open in a new tab of the same window, above the plan; the current project stays open in its own tab. Each tab keeps its project, undo history, selection and zoom, and the clipboard is shared, so you can copy in one tab and paste in another. `Ctrl+Tab` / `Ctrl+Shift+Tab` switch tabs and `Ctrl+W` closes one, asking to save when needed.
  - **Duplicar aba** copies the project into a new tab: the base for another design of the same building, such as the electrical or the structural one.
  - **Lado a lado** shows a second tab beside the active one, with the same view, to compare designs of the same building. Click the right-hand plan to edit it; the two trade places.
  - Unsaved tabs are protected by crash-recovery drafts, and closing the window asks about each one. *Arquivo* › **Nova janela** (`Ctrl+Shift+N`) still opens a separate window when you want one.
- **Crash logs and bug reports:** unexpected crashes are written to a rotating log under `%LOCALAPPDATA%\Trazo\logs\crash.log`. Open *Ajuda* › **Relatar problema...** to inspect the log, copy a diagnostic bundle, open the logs folder, or open a pre-filled GitHub issue.
- **Opening by double-click and updates:** new projects use the dedicated `.trazo` extension; existing `.trazo.json` and `.json` projects remain compatible. The Windows installer associates `.trazo` with Trazo and adds Trazo to the non-destructive `.json` *Open with* menu. Check for newer releases anytime via *Ajuda* › **Verificar atualizações...**.

## Keyboard and mouse

| Keys | Action |
| --- | --- |
| `V` `T` `P` `C` `R` `J` | Select, Lot, Wall, Room, Door, Window |
| `M` | Measure |
| `I` / `Shift+I` | Turn the selected door to the other side / move its hinge |
| `Ctrl+N` / `Ctrl+O` / `Ctrl+S` | New, open, save |
| `Ctrl+T` / `Ctrl+W` | New tab, close tab |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab |
| `Ctrl+Shift+N` | New window |
| `Ctrl+Z` / `Ctrl+Y` (or `Ctrl+Shift+Z`) | Undo, redo |
| `Delete` | Delete the selection |
| `Ctrl+A` | Select all (following the selection filter) |
| `Shift`+click / drag → / drag ← | Add to or remove from the selection / window / crossing selection |
| Arrows / `Shift`+arrows | Move the selection 0.25 m / 1 m |
| `G` / `Shift+G` | Turn the selection (or the next fixture) 90° clockwise / counter-clockwise |
| `H` / `Shift+H` | Mirror the selection left-right / upside down |
| `Ctrl+C` / `Ctrl+X` / `Ctrl+V` / `Ctrl+D` | Copy, cut, paste at the pointer, duplicate |
| `O` / `A` / `E` | Offset, trim, extend |
| `D` / `U` | Divide, join |
| `Ctrl+G` / `Ctrl+Shift+G` / `Ctrl`+click | Group, ungroup, pick one member of a group |
| `Esc` | Cancel the current action, or clear the selection |
| `F` | Zoom to fit the whole plan |
| `F3` / `F7` / `F8` | Snapping, grid, ortho mode |
| `F1` | Shortcuts and quick guide |
| Mouse wheel / middle button / right button | Zoom at the cursor / pan / context menu |

Any drawing action can be cancelled with `Esc`, a right click, or by switching tools.
