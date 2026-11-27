# Working with AI assistants (MCP)

Trazo includes a [Model Context Protocol](https://modelcontextprotocol.io) server. Claude, ChatGPT and any other MCP client can read and edit the plan that is open on screen from plain instructions, such as *"quero uma casa de dois quartos num terreno de 5 por 25 metros, com laje para subir um andar depois"*.

Everything the assistant does appears live in the window. Each tool call is one undo step, so `Ctrl+Z` takes it back.

![An assistant adding a porch: the new room flashes purple and a badge names the tool at work](images/ai-live-edit.png)

- [MCP at the centre of Trazo](#mcp-at-the-centre-of-trazo)
- [How Trazo contests the assistant](#how-trazo-contests-the-assistant)
- [Free and local assistants](#free-and-local-assistants)
- [How it works](#how-it-works)
- [Connecting a client](#connecting-a-client)
- [Tools](#tools)
- [Working together in real time](#working-together-in-real-time)
- [Conventions](#conventions)
- [Security](#security)
- [Troubleshooting](#troubleshooting)

## MCP at the centre of Trazo

Trazo exists so that a lack of money and technical knowledge does not leave a family unprotected against the risks of building its own home (see [the proposal](proposta.md)). Most of the people it serves have never used a CAD and never studied engineering. For them, **MCP is Trazo's operational centre**: the assistant does the intellectual work of design, and the family receives clear instructions to build.

| The assistant does | Trazo does | The person does |
| --- | --- | --- |
| Turns the family's description into a plan; compares layouts; lays out the structure and the electrical installation; runs every check; fixes what it can; organises the sheets, the report and the shopping list; explains each result in plain Portuguese | Computes every result with deterministic, tested rules; refuses what lies outside its methods; checks and contests every assistant edit and hypothesis; keeps every change visible and undoable | Describes the house and the site; provides real data (a soil survey, the concrete actually bought); confirms hypotheses; decides; takes the report to a professional or to free technical assistance |

Trazo tells every assistant how to behave through the MCP server instructions (`INSTRUCTIONS` in `src/mcp/protocol.rs`), sent when the client connects. In short, an assistant connected to Trazo must:

- assume the person may have no engineering training and may be building with few resources, and speak plain Portuguese, explaining any technical term;
- design, compare and review on the person's behalf, then deliver clear, ordered instructions to build;
- never play down, hide or work around a red result or a refusal; explain the risk in one sentence and propose the change that fixes it;
- never change a hypothesis (soil pressure, concrete strength, loads, room use, wind) to make a result pass; change one only with real data from the person, and say so;
- ask the person about the site with `projeto_estrutural` → `riscos_terreno` (slope, water, landfill, groundwater, neighbours' cracks, digging next to a neighbour, building on top of an existing house and whether its base is known) and record only their answers: a "yes" stops the foundations, and an assistant's "no" counts only after the person confirms it in *Segurança*;
- ask whether the family will build more floors later and record it with `projeto_estrutural` → `etapas` (`andares_futuros`, `fundacao_executada`), so the first stage is sized for them;
- say clearly when a professional is needed (`pre_dimensionamento` lists the free paths and the project's local contacts in `onde_buscar_ajuda`), and remind low-income families of free technical assistance (ATHIS, Lei 11.888/2008, through the municipality, CAU or CREA) and, for slopes, streams, landfill or cracked neighbouring houses, the Defesa Civil (199);
- never present Trazo's pre-sizing as a final design, or green as a guarantee;
- use the plain explanations Trazo returns: every check carries `termos` (the technical words it uses, explained), and `pre_dimensionamento` has `resumo_para_a_familia`, one sentence on the whole traffic light.

### The design prompt

Trazo offers one MCP prompt, `projetar_casa` (`prompts/list`, `prompts/get`, optional argument `descricao`). It walks the assistant from the family's description to a checked house: talk (lot, city, rooms, budget, future floors, the site questions), design, compare at least two layouts in separate tabs, lay out the structure, review with `revisar_projeto` until no red it can fix is left, and deliver `guia_de_obra`, the family booklet and the report, saying clearly what needs a professional.

## How Trazo contests the assistant

Instructions are not enough: an assistant can be wrong, and an assistant that designs alone can call a simulation with **wrong hypotheses** and receive a result that **looks safe**. So the essential condition is that Trazo's own rules can contest the assistant's decisions (rule 10 of the [safety contract](../ROADMAP.md#engineering-in-trazo-the-safety-contract)):

| Rule | Status |
| --- | --- |
| The assistant supplies inputs, never results: every check is computed by Trazo | ✅ Today: tools return Trazo's own results; there is no way to write one |
| Every assistant edit goes through the same validation as a person's, and is logged with its origin (`ia` and the tool) | ✅ Today: transactional edits, constraints, applicability limits, and `mudancas_desde` |
| Every hypothesis records its origin and evidence (#168) | ✅ `hipoteses` in `pre_dimensionamento` and `configurar`; *Segurança* and the report list them |
| A favourable hypothesis without evidence makes the result *contested*, never green; NBR minimums cannot be lowered (#169) | ✅ A red *Hipóteses de cálculo* verdict, green structural checks turn yellow, and `configurar` refuses a live load below NBR 6120 or a V0 below the city's |
| A hypothesis change by the assistant is proposed, and applied only when the person confirms it with its evidence (#171) | ✅ The answer lists `hipoteses_aguardando_pessoa`; pass `evidencia` so the person sees what backs the value |
| Each tool answer reports what the edit made worse: checks that changed colour, structural elements removed, hypotheses changed (#170) | ✅ `seguranca_piorou` in the answer, a sentence for the assistant to explain, and a notice in the status bar |
| Simple independent cross-checks contest a detailed simulation that disagrees (#172) | ✅ A *Conferência independente* verdict in `semaforo`; when it is red, no structural result stays green |

> [!NOTE]
> When an assistant makes a hypothesis more favourable than Trazo's safe value without evidence the project holds (a soil survey, the city's wind), `projeto_estrutural` does not apply it: the change waits in *Segurança* › *Hipóteses de cálculo* until the person confirms it with evidence. Tell the person what is waiting and why, and ask for the soil survey report, the concrete invoice or the source of the value.

## Free and local assistants

A paid subscription must not stand between a family and a safer house (#167). Trazo never needs an AI: every check, refusal, contestation, the building guide and the family booklet work with no assistant connected. When an assistant is used, any MCP client can connect, including clients that run models on the person's own computer.

Small, free or local models get lost in a long tool list. **Modo essencial**, in *Conectar uma IA* (or `TRAZO_MCP_ESSENCIAL=1` before starting Trazo), makes `tools/list` offer only the 16 tools needed to design, have Trazo check and hand over the guide: `obter_planta`, `imagem_da_planta`, `definir_terreno`, `criar_comodo`, `editar_comodo`, `inserir_abertura`, `inserir_objeto`, `executar_lote`, `projeto_estrutural`, `projeto_eletrico`, `pre_dimensionamento`, `revisar_projeto`, `guia_de_obra`, `exportar_planta`, `desfazer` and `salvar_projeto`. The other tools still run when called. Reconnect the assistant after changing the mode.

## How it works

The server starts together with the window at `http://127.0.0.1:7799/mcp`, using the streamable HTTP transport. The **IA** switch in the status bar shows whether it is running; clicking it, or **Conectar IA** on the *Saída* tab, opens the *Conectar uma IA* (connect an AI) panel, which has everything below ready to copy.

![The Conectar uma IA panel with the server address and the assistant's recent actions](images/ai-connect.png)

- Keep Trazo open while the assistant works. Only one Trazo window serves MCP at a time.
- Each Trazo window serves its own project: the first on 7799, the next ones on 7800, 7801 and so on. An assistant can connect to several windows to work on more than one project.
- Set the `TRAZO_MCP_PORT` environment variable to change the first port.
- Every change runs on a copy of the project and is applied only if it succeeds. A failing tool explains why, so the assistant can correct itself.

## Connecting a client

| Client | How to connect |
| --- | --- |
| **Claude Desktop** | Click **Configurar Claude Desktop** in *Conectar uma IA*. Trazo is added to `claude_desktop_config.json`, keeping your other settings and a `.bak` copy. Restart Claude Desktop. |
| **Claude Code** | `claude mcp add --transport http trazo http://127.0.0.1:7799/mcp` |
| **ChatGPT, Claude on the web** | These run in the cloud and need a public HTTPS address; see [Cloud assistants](#cloud-assistants). |
| **Any stdio client** | Run `trazo --mcp-stdio` (see below). |

### Claude Desktop by hand

```json
{
  "mcpServers": {
    "trazo": {
      "command": "C:\\path\\to\\trazo.exe",
      "args": ["--mcp-stdio"]
    }
  }
}
```

In stdio mode Trazo has no window of its own. It relays to the window's server, and opens the window by itself on the first tool call.

### Cloud assistants

**Abrir túnel** (open tunnel) in *Conectar uma IA* starts a free Cloudflare quick tunnel with `cloudflared`, which the same panel can install with winget. Copy the connector address it shows: `https://….trycloudflare.com/mcp/<token>`. The secret token in the path is created anew each time the tunnel opens.

Create a connector with that address and no authentication; in ChatGPT this is done in developer mode. The tunnel closes when you close it or quit Trazo.

## Tools

Trazo exposes 68 tools. Their names and messages are in Portuguese, like the interface.

| Group | Tools |
| --- | --- |
| **Reading** | `obter_planta` (the plan: ids, coordinates, sizes, areas, setbacks, wall thickness/material/heights, opening codes/typologies, finishes, structural & documentation summary, groups, constraints, selection and revision), `imagem_da_planta` (a PNG preview), `mudancas_desde` (what changed since a revision) |
| **Lot and walls** | `definir_terreno` (size, origin and building setbacks `recuo_frente`/`recuo_fundos`/`recuo_lateral`), `criar_parede`, `editar_parede` (length, and the wall's own height: `altura_m`, `altura_fim_m` for a sloped top, `altura_padrao`), `definir_alvenaria` (brick or block, joint, plaster, ceiling height) |
| **Rooms** | `criar_comodo` (rectangle or any polygon), `editar_comodo` (rename, resize, finish presets and per-room floor/wall/ceiling/baseboard/waterproofing specifications) |
| **Openings** | `inserir_abertura` (door, window, saloon door, open span or gate), `editar_abertura` (width, position, height, sill, door/window typology `tipo_porta`/`tipo_janela`, side and hinge) |
| **Electrical (NBR 5410)** | `inserir_ponto_eletrico` (outlets, switches, lights, panel, data), `editar_ponto_eletrico` (circuit, power, height, lumens), `projeto_eletrico` (generate the minimum points, distribute circuits, set the voltage, lay conduits with `gerar_eletrodutos`, join two points with `ligar_pontos`, move a run to the floor with `definir_trecho`, delete one with `remover_eletroduto`, choose the supply with `definir_alimentacao`, utility profile with `definir_concessionaria`, collective demand with `demanda_coletiva`, shopping list with `lista_materiais`, Lumen Method room lighting with `luminotecnico`/`configurar_iluminacao`, photovoltaic pre-sizing with `fotovoltaico`/`configurar_fotovoltaico`, multi-board short-circuit & selectivity with `seletividade`/`configurar_curto_circuito`, and earthing layout & simplified NBR 5419-2 SPDA risk with `aterramento_spda`/`configurar_aterramento`), `quadro_de_cargas` (forecast, circuits with phases, voltage drop, wire and breaker, demand, service entrance, room illuminance in lux, the wires of each conduit, warnings) |
| **Water, sewage and rain** | `instalacoes_hidraulicas` (the plumbing points of every fixture: cold water, hot water, drain with its DN and gas, with position and height; the count of each kind; the fixture schedule with weights and UHC; the cold-water design; the hot water; the sewage `esgoto` (floor traps, grease trap, inspection boxes with depths, vents, branches and main with DN and fall); the septic system `fossa_septica`; the notes), `projeto_hidraulico` (residents, consumption, reserve, tank position and height, heating and heater position, the sewer connection `saida_esgoto_x`/`saida_esgoto_y`; `configurar_fossa` / `remover_fossa`: septic tank NBR 7229 and soakaway NBR 13969 where there is no sewer, with the norm's distances), `exportar_planta` → `hidrossanitario_pdf` (the plumbing sheets) |
| **Structure (pre-sizing)** | `projeto_estrutural` (lay out the structure with `gerar_estrutura`, place columns/footings, add/edit/remove water-tank or equipment point loads with `inserir_carga`/`editar_carga`/`remover_carga`, join columns or support secondary beams, edit beams/slabs, configure materials/masonry weight/per-room loads and 2D frame joint/base fixity `portico_nos_rigidos`/`portico_bases_engastadas`, toggle plan/paper overlays with `configurar_sobreposicao`, run What-if scenarios with `simular_whatif`, resize footings, generate/edit balance beams and wall ground beams/strip footings, define SPT `definir_sondagem`/`remover_sondagem` and `simular_fundacoes`, a presumed soil profile without a survey `definir_solo_provavel`/`remover_solo_provavel` (only ever safer, flags collapsible soil), in-ground pools `configurar_piscina`, timber roofs `definir_telhado` (`estilo`: `meia_agua`/`duas_aguas`/`quatro_aguas`, `beiral_m`, `tesouras`) / `ajustar_paredes_telhado` / `remover_telhado`, timber posts `inserir_mourao` with species/class and treatment, NBR 6123 wind `configurar_vento`, and select a discipline), `pre_dimensionamento` (slabs with rectangular panels, Czerny/Marcus moments and deflection, continuous beams with Branson B12 cracked-section `Mr`/Stage I-II/`I_eq/Ic`, 2D plane frames `porticos_2d` coupling beams and columns with column moments, reactions and lateral drift, explicit solver failures in `falhas_porticos_2d` with frame label and reason, columns with slenderness, footings, wall foundations, lintels/sills and infill/load-bearing masonry panels, balance beams, roofs with wall plates/Howe trusses/hip rafters/overhangs/uplift, timber posts with compression/buckling/wind interaction and durability, NBR 6123 wind checks, soil survey, presumed soil `solo_provavel`, in-ground pools `piscinas` (wall under water and earth pressure, floor, flotation, nearby foundations, boundary, durability and safety care), comparative foundation study, preliminary reinforcement with NBR 6118 anchorage/laps, bar schedule and CA-50 summary by diameter with 12 m bars and +10% waste, safety lights and warnings) |
| **Quantities & ergonomics** | `quantitativos` (consolidated takeoff for active floor or `todos_pavimentos: true` with optional `perda_blocos_pct`: masonry blocks/half-blocks, bond-beam & lintel U-blocks with waste, laying mortar, plaster, grout fill, lintels/sills with opening codes `P1`/`J1`, per-room finishes +10% waste/waterproofing/specs, per-floor summary, roof timber/tiles and preliminary steel mass), `ergonomia` (NBR 9050 & residential collision, door-swing, clearance and narrow-passage checks) |
| **Documentation & circulation** | `camadas` (CAD layers: list, create, edit, delete, assign), `anotacoes` (texts, notes, leader callouts, north/level/slope/entry symbols), `cotas` (associative internal room dimensions, automatic opening dimension chains `gerar_aberturas`/`gerar_completas`, wall-aligned and external dimensions), `circulacao` (stairs with Blondel check, NBR 9050 ramps, landings, railings, level changes), `vistas_tecnicas` (sections, facade elevations, detail callouts and SVG/PDF export), `prancha` (ABNT sheet size, orientation, scale, title block and sheet-set release) |
| **Fixtures and drawing** | `inserir_objeto` and `editar_objeto` (position, 90° or free `angulo_graus`, size, optionally against the nearest wall), `desenhar` (lines, polylines, rectangles, arcs, circles, splines) |
| **CAD editing** | `selecionar` (shows the person which elements the assistant means), `mover_elemento`, `transformar` (move, rotate, mirror or copy several elements), `alinhar`, `distribuir`, `agrupar`, `desagrupar`, `repetir` (grid or polar), `editar_geometria` (trim, extend, offset, divide), `unir`, `restringir`, `remover_restricao`, `excluir_elemento` |
| **Protection for self-builders** | `revisar_projeto` (the project reviewed for the family in plain words: the headline, what must not be built as it is, what is at the limit, contested hypotheses and proposals waiting for the person, unanswered site questions, where to get help), `exportar_planta` → `caderno_familia_pdf` (the family booklet), `guia_de_obra` (the checked house as ordered building instructions: site and setting out, foundation, columns and beams, walls, slab, roof, electrical, each with materials, the before-the-pour checklist and where a professional should look); `projeto_estrutural` → `riscos_terreno` and `etapas` |
| **Batch** | `executar_lote`: up to 200 tool calls in one request, all or nothing, as a single undo step |
| **History and view** | `desfazer`, `refazer`, `centralizar_planta` |
| **Synced views, floors & dependencies** | `abas` (list projects and linked tabs, switch, open linked discipline views `nova_visao`, configure view, split side-by-side, close, duplicate), `pavimentos` (list, add or switch floors), `dependencias` (cross-discipline dependency graph, last auto-sync report and manual-review items) |
| **Files & exports** | `novo_projeto`, `abrir_projeto`, `salvar_projeto`, `exportar_planta` (`pdf`, `pdf_abas`, `png`, `dxf`, `svg`, `prancha_pdf`/`prancha_pdf_abas`/`prancha_svg`, `memorial_pdf`, `unifilar_svg`/`unifilar_pdf`, `esforcos_svg`/`esforcos_pdf`, `porticos_svg`/`porticos_pdf`, `grelha_svg`/`grelha_pdf`, `armaduras_svg`/`armaduras_pdf`, `quantitativos_csv`/`quantitativos_svg`/`quantitativos_pdf` with optional `todos_pavimentos`, `conjunto_obra_pdf` (the one-click A3 construction set), `primeira_fiada_svg`, `paginacao_pdf` (first course and wall elevations with block paging), `locacao_svg`/`locacao_pdf` (setting-out plan), `cobertura_svg`/`cobertura_pdf` (roof plan), `hidrossanitario_pdf` (the plumbing sheets), `tabela_aco_csv`, `csv`), `importar_dxf` |

Tools that only read are marked read-only. Tools that mix reads and edits are marked as mutations. `executar_lote` cannot contain file/history tools, `biblioteca`, `blocos`, `referencias`, `documentacao` or `modelos`, because these groups can also change files or the shared personal library. Project mark edits, `acabamentos` and `terreno` can participate in atomic batches.

### Review marks and project resources

| Tool | Coverage |
| --- | --- |
| `obter_marcacoes` | Read-only mark query, including hidden/resolved marks; filter by numeric id, exact colour, resolved state and floor, or use `todos_pavimentos`. Returns `referencia` (`M12`), colour, comment, geometry in metres with Y up, bounds, floor, explicit references and geometrically related elements. |
| `marcacoes` | `criar`, `editar`, `mover`, `excluir`, `resolver`, `reabrir`, `selecionar`, `configurar`, or `listar`. A rectangle uses two corners, a circle uses centre/radius point, an arrow uses its ends and freehand uses 2–2048 points. Comments allow 4000 characters. Mutations apply to the active floor; optional `pavimento_id` protects against editing another floor. |
| `biblioteca` | Search/list built-in, project and personal models; create/edit/delete custom models, insert with their real dimensions/rotation, or share to the personal library. Always pair `origem` with `id`. |
| `blocos` | List project/personal definitions and placed instances; define a block from selected ids, insert with attribute overrides, edit instance attributes, share or delete a definition. |
| `referencias` | Images/PDF/SVG underlays with import, geometry, opacity, visibility, locking and two-point calibration; external Trazo/DXF references with reload; compare against a file or tab, show/clear the visual overlay and export the comparison SVG. |
| `documentacao` | Read actual schedule tables/style settings; configure pens, font, units, dimension ticks, schedule visibility and waste; add revisions, update/remove/reorder sheets and export the CSV transmittal. Updating a released sheet invalidates its previous release review. |
| `modelos` | List built-in templates, examples and personal `.trazot` templates; apply with an unsaved-changes guard or save a new template without rebinding the current project file. |
| `acabamentos` | List/apply room hatches, spacing, angle and colour; list/apply/clear wall assembly presets or define explicit layers with roles, thicknesses and colours. |
| `terreno` | Read polygon boundaries/lots, define a polygonal site, subdivide by a line, rename/remove lots. Rectangular sites and setbacks remain available through `definir_terreno`. |

The existing `pavimentos` tool also edits/removes floor metadata and controls the floor-below reference. `circulacao` and `vistas_tecnicas` accept `acao: "editar"` to update placed elements without deleting/recreating their ids. All tools are listed by `tools/list` with their arguments.

Example mark creation:

```json
{
  "acao": "criar",
  "tipo": "retangulo",
  "cor": "vermelho",
  "comentario": "Rever a disposição dos armários",
  "pontos": [{"x": 1, "y": 2}, {"x": 4, "y": 5}]
}
```

`obter_planta` includes the active floor's marks, `marcacao_selecionada` and a resource summary. `mudancas_desde` includes `marcacoes_adicionadas`, `marcacoes_removidas` and `marcacoes_alteradas`. `imagem_da_planta` includes visible marks by default; use `incluir_marcacoes: false` for a clean preview. File exports follow the saved mark export setting instead.

Project changes use undo/recovery and linked views. Personal-library and file writes are persistent external operations and do not belong to project undo. Comments are project data; creating a mark does not execute its text or authorize an assistant to change the referenced elements.

`pre_dimensionamento` now includes `alvenaria.paineis[].vento_fora_plano` with wind demand/capacity, proposed intermediate/crown ties and `jambas_aberturas[]` (grouted vertical concrete jambs beside openings in tall walls), and `fundacao_paredes[]` with signed reactions to columns and receiving ground beams. `projeto_estrutural` with `configurar_vento` also returns the tall masonry-panel effects under `paineis_alvenaria`. A missing support path is exposed as `apoios_insuficientes` and a red verdict.

`projeto_estrutural` with `definir_telhado` accepts `contraventamento_kn` as the verified design resistance of the full lateral path (or `null` to remove it). `configurar_vento` and `pre_dimensionamento` return `vento.pilares[]` (including footing wind eccentricity/pressure `sapata_excentricidade_m`, `sapata_pressao_max_kpa`, `sapata_vento_uso`), `vento.telhados[]` (with NBR 8800 anchor bolts `chumbador_mm`, `chumbadores_qtd`, `chumbador_interacao`, `chumbadores_sugeridos`), `vento.contraventamento` (with CA-50 X-bracing tie rod `tirante_mm`, `tirante_as_cm2`, `tirante_sugerido`) and `vento.estabilidade_global` with four directional checks including NBR 6122 soil sliding (`atrito_mu`, `resistencia_deslizamento_kn`, `deslizamento_fs`). Roof bearing alone never counts as top lateral restraint.

`pre_dimensionamento` also returns `cargas_e_combinacoes` (`nbr_6120` with `G1`/`G2`/`Q` totals, per-room Table 10 live loads and occupancy `ψ0/ψ1/ψ2`; `vento_edificacao_nbr_6123` with four orthogonal directions, Table 4 `Cpe`/`Ca`, storey-by-storey forces and `ELU-EQU` overturning; `combinacoes_nbr_8681` with all 17 ELU/ELS combinations and resultants), `sobreposicao_planta` (active on-plan/paper simulation overlays), `grelha_lajes_vigas` (coupled 3-DOF slab + beam grillage analysis including L-shaped/irregular slabs, `mx/my/mxy`, deflections, beam torsion and equilibrium residual), `escadas_e_balancos` (concrete stairs, balconies and cantilever slabs/beams with NBR 6118 `γ_n`, `L/125` deflection, top negative steel and `1.5·L_bal` back-span checks; add/remove explicit cantilevers via `projeto_estrutural` → `definir_balanco`/`remover_balanco`), and enriches every `porticos_2d[]` entry with `vento_caracteristico_kn`, `casos_de_carga`, `combinacoes`, `envoltoria_elu` (including `arrancamento_pilar`) and `envoltoria_els`. Use `projeto_estrutural` → `configurar_sobreposicao` to toggle utilisation colours, beam diagrams, deformed shape and load path on screen and exported sheets, and `projeto_estrutural` → `simular_whatif` to test span/section/load/material variations with safe-range bands (optionally setting `aplicar: true`). Setting `ocupacao` via `projeto_estrutural` → `configurar` updates both the base live load and the NBR 8681 combination factors.

## Working together in real time

You and the assistant can edit the same plan at the same time without overwriting each other.

- **Live highlight.** Every element the assistant adds or changes flashes with a purple outline, and a badge names the tool at work. With *Acompanhar a IA* (follow the AI) on, the view slides to edits that happen off screen, keeping your zoom. *Conectar uma IA* lists the assistant's recent actions.
- **Revisions.** Every change to the plan gets a revision number: yours or the assistant's, including undo and redo. `obter_planta` returns the current `revisao`. `mudancas_desde` tells the assistant what changed after the revision it knows: who did it (`usuario` or `ia`, with the tool) and which element ids were added, removed or altered.
- **Notifications.** The open plan is the MCP resource `trazo://planta` (`resources/read`). A client that subscribes and opens the event stream (`GET /mcp` with `Accept: text/event-stream`, also relayed by `--mcp-stdio`) receives `notifications/resources/updated` the moment the plan changes. *Conectar uma IA* shows how many assistants are following the plan live.

## Conventions

- Coordinates are in metres with **Y pointing up**, as in the status bar.
- Files saved or exported without a full path go to `Documents\Trazo`.
- `prancha` supports `listar_conjunto`, `adicionar_ao_conjunto`, `definir_situacao` and `exportar_conjunto`. To set `situacao: "liberado_obra"` while there are red checks, provide `id`, `revisor`, `registro_revisor` (numbered CREA/CAU) and `data_revisao`. A changed red result or sheet revision invalidates that release in exports.
- Opening or creating a project refuses to drop unsaved changes unless the assistant explicitly asks to discard them.

## Security

For ease of use, the local server has no authentication. It only listens on this computer (`127.0.0.1`).

While a tunnel is open, anyone who knows its full address, including the secret token, can read and edit the open plan and save or export files. Close the tunnel when you are done. See [SECURITY.md](../SECURITY.md) for how to report a vulnerability.

## Troubleshooting

| Problem | Fix |
| --- | --- |
| The **IA** switch is off | Another program is using this window's port (7799 for the first window, 7800 for the second…). Close it, or start Trazo with a different `TRAZO_MCP_PORT`. |
| Claude Desktop does not list Trazo | Restart Claude Desktop after configuring it, and check that the path in `claude_desktop_config.json` points to the current `trazo.exe`. |
| `cargo build` fails with "access denied" on Windows | Claude Desktop is running `trazo.exe --mcp-stdio`, and Windows locks the running executable. Close Claude Desktop, then rebuild. |
| The cloud assistant cannot connect | Check that the tunnel is still open and that you copied the full address, including `/mcp/<token>`. |
