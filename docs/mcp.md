# Working with AI assistants (MCP)

Trazo includes a [Model Context Protocol](https://modelcontextprotocol.io) server. Claude, ChatGPT and any other MCP client can read and edit the plan that is open on screen from plain instructions, such as *"create a 12 × 20 m lot with a 6 × 5 m living room and a door on the left wall"*.

Everything the assistant does appears live in the window. Each tool call is one undo step, so `Ctrl+Z` takes it back.

![An assistant adding a porch: the new room flashes purple and a badge names the tool at work](images/ai-live-edit.png)

- [How it works](#how-it-works)
- [Connecting a client](#connecting-a-client)
- [Tools](#tools)
- [Working together in real time](#working-together-in-real-time)
- [Conventions](#conventions)
- [Security](#security)
- [Troubleshooting](#troubleshooting)

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

Trazo exposes 43 tools. Their names and messages are in Portuguese, like the interface.

| Group | Tools |
| --- | --- |
| **Reading** | `obter_planta` (the plan: ids, coordinates, sizes, areas, wall thickness and material, opening codes, the door and window schedule, groups, constraints, the person's selection and the current revision), `imagem_da_planta` (a PNG preview), `mudancas_desde` (what changed since a revision) |
| **Lot and walls** | `definir_terreno`, `criar_parede`, `editar_parede` (length, and the wall's own height: `altura_m`, `altura_fim_m` for a sloped top, `altura_padrao`), `definir_alvenaria` (brick or block, joint, plaster, ceiling height) |
| **Rooms** | `criar_comodo` (rectangle or any polygon), `editar_comodo` |
| **Openings** | `inserir_abertura` (door, window, saloon door, open span or gate), `editar_abertura` (width, position, height, sill, side and hinge) |
| **Electrical (NBR 5410)** | `inserir_ponto_eletrico` (outlets, switches, lights, panel, data), `editar_ponto_eletrico` (circuit, power, height), `projeto_eletrico` (generate the minimum points, distribute circuits, set the voltage, lay conduits with `gerar_eletrodutos`, join two points with `ligar_pontos`, move a run to the floor with `definir_trecho`, delete one with `remover_eletroduto`, choose the supply with `definir_alimentacao`), `quadro_de_cargas` (forecast, circuits with phases, voltage drop, wire and breaker, demand, service entrance, the wires of each conduit, warnings) |
| **Structure (pre-sizing)** | `projeto_estrutural` (lay out the structure with `gerar_estrutura`, place columns/footings, add/edit/remove water-tank or equipment point loads with `inserir_carga`, `editar_carga` and `remover_carga`, join columns or support secondary beams, edit beams/slabs, set materials, resize footings and select a discipline; the soil survey with `definir_sondagem` and `remover_sondagem`, the foundation study with `simular_fundacoes`, and mono-pitch timber roofs with `definir_telhado` and `remover_telhado`), `pre_dimensionamento` (slabs and concentrated loads, continuous beams, columns, footings, preliminary reinforcement/bar schedule, safety lights and warnings) |
| **Fixtures and drawing** | `inserir_objeto` and `editar_objeto` (optionally placed against the nearest wall), `desenhar` (lines, polylines, rectangles, arcs, circles, splines) |
| **CAD editing** | `selecionar` (shows the person which elements the assistant means), `mover_elemento`, `transformar` (move, rotate, mirror or copy several elements), `alinhar`, `distribuir`, `agrupar`, `desagrupar`, `repetir` (grid or polar), `editar_geometria` (trim, extend, offset, divide), `unir`, `restringir`, `remover_restricao`, `excluir_elemento` |
| **Batch** | `executar_lote`: up to 200 tool calls in one request, all or nothing, as a single undo step |
| **History and view** | `desfazer`, `refazer`, `centralizar_planta` |
| **Tabs** | `abas`: list, switch, create or duplicate tabs; every other tool works on the active tab |
| **Files** | `novo_projeto`, `abrir_projeto`, `salvar_projeto`, `exportar_planta` (PDF, PNG or DXF), `importar_dxf` |

Tools that read are marked read-only and tools that delete are marked destructive, so clients can ask before running them. `executar_lote` cannot contain file or history tools.

## Working together in real time

You and the assistant can edit the same plan at the same time without overwriting each other.

- **Live highlight.** Every element the assistant adds or changes flashes with a purple outline, and a badge names the tool at work. With *Acompanhar a IA* (follow the AI) on, the view slides to edits that happen off screen, keeping your zoom. *Conectar uma IA* lists the assistant's recent actions.
- **Revisions.** Every change to the plan gets a revision number: yours or the assistant's, including undo and redo. `obter_planta` returns the current `revisao`. `mudancas_desde` tells the assistant what changed after the revision it knows: who did it (`usuario` or `ia`, with the tool) and which element ids were added, removed or altered.
- **Notifications.** The open plan is the MCP resource `trazo://planta` (`resources/read`). A client that subscribes and opens the event stream (`GET /mcp` with `Accept: text/event-stream`, also relayed by `--mcp-stdio`) receives `notifications/resources/updated` the moment the plan changes. *Conectar uma IA* shows how many assistants are following the plan live.

## Conventions

- Coordinates are in metres with **Y pointing up**, as in the status bar.
- Files saved or exported without a full path go to `Documents\Trazo`.
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
