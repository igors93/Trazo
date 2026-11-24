# Plano Mestre de Fortalecimento do Trazo a partir da Análise do BIMCraft

> **Base da análise:** clone completo do [BIMCraft](https://github.com/IPedrax/BIMCraft) (`v0.12.0`, Rust Edition 2024 + `egui`/`eframe`, 1.163 arquivos em `_tmp_bimcraft`) confrontado com o estado atual do **Trazo** (`main`, após a conclusão integral de M3, M4 e M5 — 518 testes).
>
> **Objetivo deste documento:** servir como **fila de implementação item a item** (`T-01` a `T-56`), ordenada rigorosamente por **dependências arquiteturais**, **risco técnico** e **retorno prático** para o usuário brasileiro de arquitetura, engenharia e obra.

---

## 1. Diagnóstico Comparativo: Trazo vs. BIMCraft

Ambos os projetos são escritos em **Rust puro + `egui`/`eframe`** com licença **MIT OR Apache-2.0**, tornando o aproveitamento de padrões arquiteturais e bibliotecas Rust quase direto. A complementaridade é quase perfeita:

| Dimensão | Onde o **Trazo** já é superior | Onde o **BIMCraft** é superior (fonte de melhorias) |
|---|---|---|
| **Engenharia Calculada** | Pórtico plano 2D (DSM), analogia de grelha (FEM), 17 combinações NBR 8681 + NBR 6120 + NBR 6123, sondagem SPT + 4 fundações, baldrames/vigas alavanca, telhados com tesouras Howe, mourões NBR 7190, escadas/sacadas/balanços, semáforo + limites + memorial PDF com gate CREA/CAU + simulador What-if. | Cálculos normativos **orientados a dados (JSON versionado)** em Hidrossanitário (NBR 5626/8160/10844, IT-22), Elétrica avançada (NBR 5410 DR/DPS/volumes banheiro + 9 concessionárias + demanda coletiva), Plano de Corte de Aço (*Cut Plan*), descontos geométricos reais de concreto/fôrma, perfis metálicos e *wood framing*. |
| **Modelagem 2D/3D e BIM** | Planta 2D ágil, paredes compartilhadas, cotas de aberturas, quantitativos reais de alvenaria/acabamentos, abas sincronizadas por disciplina. | 3D nativo (CPU + GPU `wgpu`), `Section Box`, famílias paramétricas + CSG/voids + solver de restrições, `Curtain Walls`, `Toposolid`, `Massing`, `Shape Editing` (Delaunay), `Phasing` (reformas) + `Design Options`, `View Range`, `V/G Overrides`, `View Templates`, `Schedules` com fórmulas, `Classification` (NBR 15965/COBie). |
| **Interoperabilidade** | DXF R12 (import/export), SVG/PDF/PNG/CSV, underlay imagem, Xref. | **IFC4 / IFC4.3 / IFC2x3** (import + export validados com IfcOpenShell), **DWG 2000–2018** em subprocesso isolado, **Revit `.rvt`/`.rfa`** read-only, **Point Clouds** (LAS/E57/PLY/PTS/XYZ), **glTF/OBJ/STL**, **BCF 2.1**, PDF vetorial (`hayro`), PDF/A (`krilla`), XLSX/ODS/ODT, `.PAT`, Manage Links e pacotes `.bima`. |
| **Escala, Plataforma e IA** | MCP HTTP + stdio acoplado à janela ao vivo (53 ferramentas) + túnel Cloudflare. | `#![forbid(unsafe_code)]`, `unwrap/expect/panic = deny`, `MemoryBudget` (`cap`+`sysinfo`), chunks `Arc<Chunk>` + check incremental + canvas GPU retido, **Assistente IA embutido (BYOK)**, **Co-Work LAN criptografado (Noise)**, **Plugins WASM (`wasmi`)**, **PyAddins**, **CLI headless**, **i18n `.po` estático**, auto-update criptográfico e packaging completo. |

> **Regra de ouro ao implementar cada item:** nunca enfraquecer o **contrato de segurança de engenharia do Trazo** (semáforo, margem, explicação simples, recusa fora dos limites e revisão CREA/CAU). As capacidades BIM/3D/MEP/interop vindas do BIMCraft devem **alimentar e potencializar** as simulações e o memorial do Trazo.

---

## 2. Mapa Geral das Fases (Ordem Recomendada de Execução)

1. **Fase 1 (`T-01`–`T-07`) — Alicerces Arquiteturais, Segurança de Runtime e Escala** *(fazer primeiro: GUIDs, memória e armazenamento ficam 10× mais caros se feitos depois)*
2. **Fase 2 (`T-08`–`T-11`) — Motor de Normas em Dados (`codes`) e Códigos de Obra (`#73`, `#140`)** *(unifica normas de arquitetura, elétrica, hidráulica e estrutura sem hardcode)*
3. **Fase 3 (`T-12`–`T-21`) — Produtividade 2D, Documentação Profissional e UX Desktop** *(valor imediato no fluxo principal atual do Trazo)*
4. **Fase 4 (`T-22`–`T-30`) — Modelagem Arquitetônica Avançada, Famílias, Terreno e Georreferenciamento** *(prepara a geometria 3D real para visualização, MEP e IFC)*
5. **Fase 5 (`T-31`–`T-35`) — Aceleração de Canvas 2D, Cortes Escaláveis e Motor 3D Nativo (`wgpu`) (`#85`, `#62`)**
6. **Fase 6 (`T-36`–`T-43`) — Disciplinas de Engenharia & MEP (Hidráulica, Elétrica Avançada, SPDA, Dados, Metálica/Wood Framing, Armaduras 3D + Plano de Corte, Classificação BIM) (`#72`, `#118`–`#129`)**
7. **Fase 7 (`T-44`–`T-48`) — Interoperabilidade Profissional (IFC4/4.3/2x3, DWG, RVT, Nuvem de Pontos, glTF/BCF/PDF-Underlay/Links) (`#86`)**
8. **Fase 8 (`T-49`–`T-53`) — Assistente IA Embutido, CLI/MCP Hardening, Plugins WASM, PyAddins e Co-Work (`#48`, `#75`, `#141`)**
9. **Fase 9 (`T-54`–`T-56`) — Internacionalização Estática (`.po`), Auto-Update Criptográfico e Empacotamento Multiplataforma (`#68`, `#78`)**

---

## 3. Lista Completa de Implementação (Item a Item)

### FASE 1 — Alicerces Arquiteturais, Segurança de Runtime e Escala

- [x] **T-01 · Blindagem de Compilação, Zero-Panic e Perfil `dev` Otimizado** *(Tamanho: S)*
  - **Ref. BIMCraft:** `Cargo.toml` (`[workspace.lints]`, `[profile.dev.package.*]`), `clippy.toml`.
  - **O que implementar:**
    - Ativar `unsafe_code = "forbid"` na raiz do crate e habilitar gradativamente (módulo por módulo) `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`, `clippy::todo`, `clippy::unimplemented`, `clippy::unreachable` em produção (liberados em `#[cfg(test)]` via `clippy.toml`).
    - Configurar `opt-level = 2` no `[profile.dev.package.*]` para dependências pesadas (ex.: rasterizadores, parsers, criptografia) e, quando o Trazo evoluir para workspace, para `trazo-core`, garantindo testes e preview fluidos em debug.
  - **Por que agora:** cria a rede de segurança antes de adicionar parsers binários (IFC/DWG/E57/RVT) e motores 3D.
  - **Critério de pronto:** `cargo clippy --all-targets --all-features -- -D warnings` verde sem `unwrap()`/`expect()` em código de produção.

- [x] **T-02 · Identificadores Globalmente Únicos (`Guid`) Estáveis em Todo o Modelo** *(Tamanho: M · Pré-requisito de IFC, Links, Diff, Parâmetros e Colaboração)*
  - **Ref. BIMCraft:** `crates/core/src/ids.rs`, `ROADMAP.md` (0.5).
  - **O que implementar:**
    - Adicionar um `Guid` (128 bits, gerado via `getrandom`, serializado em Base64/UUID compacto e compatível com IFC `GlobalId` de 22 caracteres) ao lado do `id: u64` em todos os elementos (`Wall`, `Room`, `Opening`, `Fixture`, `Drawing`, `Beam`, `Column`, `Slab`, `Footing`, `Roof`, `Conduit`, `Circuit`, `Note`, `Dimension`, `View`, `Sheet`) e entradas de biblioteca (materiais, tipos, camadas, parâmetros, fases).
    - Migração automática na abertura de projetos antigos gerando GUIDs determinísticos/únicos; *Insert from File* / *Paste* preservam referências internas e atribuem novos GUIDs às cópias.
    - Busca/seleção por ID ou GUID (*Consultar por ID/GUID* + MCP).
  - **Critério de pronto:** projetos antigos migram sem aviso; salvar/reabrir preserva 100% dos GUIDs; cópias recebem GUIDs novos sem quebrar vínculos.

- [x] **T-03 · Orçamento de Memória (`MemoryBudget`) Contra Arquivos Grandes e Hostis** *(Tamanho: S)*
  - **Ref. BIMCraft:** `crates/core/src/memory.rs`, `Cargo.toml` (`cap = "0.1"`, `sysinfo = "0.40"`).
  - **O que implementar:**
    - Instalar alocador global contador seguro (`cap::Cap`) sem `unsafe` combinado com `sysinfo` para ler RAM livre.
    - Criar `MemoryBudget` / `SwapWatch` usado por `io::project_file::load`, `io::dxf`, e futuros importadores (IFC, nuvem de pontos, PDF, DWG) que estima o custo antes de carregar e aborta com erro amigável preservando o projeto aberto caso um arquivo corrompido ou gigantesco tente exaurir a memória.
  - **Critério de pronto:** testes unitários comprovam recusa limpa quando um arquivo excede o teto de memória sem `panic` nem OOM.

- [x] **T-04 · Armazenamento Persistente em Chunks (`Arc<Chunk>`), CoW (`Shared<T>`) e Revisões por Elemento** *(Tamanho: L · Atende `#84` e base de `#62`)*
  - **Ref. BIMCraft:** `crates/core/src/ids.rs`, `ids/revisions.rs`, `shared.rs`.
  - **O que implementar:**
    - Substituir vetores planos clonados integralmente no histórico de undo por armazenamento estruturalmente compartilhado em chunks imutáveis (`BTreeMap<ChunkId, Arc<Chunk>>`, 1024 IDs por chunk) + `Shared<T>` com `Identity<T>` (`Weak<T>`) para detectar mutação de definições/materiais sem reter snapshots antigos.
    - Manter `RevisionSnapshot` (revisão inteira por elemento) para saber exatamente o que mudou entre dois estados.
  - **Por que agora:** corta consumo de RAM do `History` pela metade, derruba tempo de undo/redo para `< 0,1 ms` e habilita invalidação granular em caches e 3D.
  - **Critério de pronto:** undo/redo de projeto com 500+ paredes usa memória limitada (compartilhamento estrutural verificado por teste) e `< 1 ms`.

- [ ] **T-05 · Validação Incremental, Índices Espaciais XY e Cancelamento Cooperativo** *(Tamanho: L · Atende `#62`)*
  - **Ref. BIMCraft:** `crates/core/src/derived.rs`, `walls/index.rs`, `engine.rs`, `ui-egui/src/view_worker.rs`.
  - **O que implementar:**
    - Criar índice espacial em grade XY (`SpatialIndex`) para paredes, aberturas, fixtures, colunas e desenhos — substituindo varreduras lineares em picking, snapping, detecção de cômodos (`auto_rooms`) e acoplamento estrutura/alvenaria.
    - Evoluir `src/domain/background.rs` para suportar token de cancelamento cooperativo (`Cancel(Arc<AtomicBool>)`) em análises estruturais, grelha FEM e geração de desenhos pesados.
    - Registrar *change sets* por edição para revalidar apenas elementos afetados e seus dependentes.
  - **Critério de pronto:** edição em planta sintética de 500 paredes e 5 pavimentos roda em `< 50 ms` no núcleo e picking/snap em `< 2 ms`.

- [ ] **T-06 · Formato Compacto Paralelo (`.trazo` ZIP), Templates de Projeto (`.trazot`) e Página Inicial (*Start Page*)** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/bimc.rs`, `files.rs`, `ui-egui/src/project_templates.rs`, `autosave.md`.
  - **O que implementar:**
    - Suportar salvamento opcional/padrão comprimido por blocos de 1 MiB em paralelo (`rayon` + `flate2` dentro de ZIP com índice `pieces.json`), mantendo leitura transparente do JSON puro atual.
    - Criar extensão `.trazot` (Template de Projeto): abrir um `.trazot` inicia um projeto novo sem vínculo de arquivo (nunca sobrescreve o template); *Salvar como Modelo…* + lista ordenada de templates no `Ctrl+N`.
    - Criar *Start Page* (Página Inicial) com *Novo*, *Abrir*, cartões dos projetos recentes com miniatura PNG embutida e estado de recuperação de autosave (*Recuperar / Descartar / Adiar*).
  - **Critério de pronto:** projeto grande salva/abre > 3× menor e mais rápido; abrir `.trazot` cria documento limpo; Start Page exibe miniaturas de arquivos recentes.

- [ ] **T-07 · Suíte de Regressão de UI (`egui_kittest`), Fuzz Hostil de Comandos e Benchmark CLI (`--bench`)** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/ui-egui/src/tests/`, `bench.rs`, `pacing.rs`, `docs/performance.md`.
  - **O que implementar:**
    - Adicionar `egui_kittest` em `dev-dependencies` para testar diálogos críticos (Estrutura, Elétrica, Prancha, Segurança, What-if) de forma headless no CI.
    - Adicionar teste de varredura hostil (`hostile_params_never_panic_or_break_model`) executando todas as ferramentas MCP com `NaN`, `Infinity`, strings gigantes, IDs inexistentes e polígonos degenerados.
    - Adicionar flag `--bench` e um modelo sintético de stress (ex.: prédio 3 pavimentos com 500 paredes) medindo tempo de edição, cálculo e renderização, além do *frame pacing* suave no Windows (`pacing.rs`).
  - **Critério de pronto:** CI roda testes de UI headless e fuzz MCP com zero panics; baseline de performance registrado em `docs/performance.md`.

---

### FASE 2 — Motor de Normas Orientado a Dados (`codes`) e Códigos de Obra (`#73`, `#140`)

- [ ] **T-08 · Motor de Códigos de Obra em Camadas (`Base → Emendas → Projeto`) com Pinagem e XML** *(Tamanho: M · Atende `#140`)*
  - **Ref. BIMCraft:** `crates/core/src/codes.rs`, `codes/{command, xml, rule_tests}.rs`, `crates/core/data/codes/`.
  - **O que implementar:**
    - Criar `src/domain/codes/` suportando 5 disciplinas (`Arquitetura`, `Estrutura`, `Elétrica`, `Hidrossanitário`, `Conforto/Mecânica`) com resolução em camadas: `Base` (ex.: ABNT NBR) → até 16 `Emendas` ordenadas (ex.: Código de Obras municipal, IT do Corpo de Bombeiros, norma da concessionária) → `Ajustes do projeto`.
    - Guardar a edição pinada no arquivo do projeto (atualizações do Trazo nunca mudam silenciosamente resultados de projetos já salvos).
    - Exigir metadados `document`, `item`, `page`, `kind` (`requirement` / `recommendation` / `practice` / `assumption`) e `checked: bool` em cada regra, com teste automático proibindo números normativos soltos no código Rust nos novos módulos.
    - Exportação/importação de pacotes de normas em JSON/XML para compartilhamento entre escritórios e municípios.
  - **Critério de pronto:** carregar/emendar uma regra altera a verificação instantaneamente, sem recompilar; export/import round-trip testado; projetos antigos preservam sua edição pinada.

- [ ] **T-09 · Parâmetros Climáticos e de Sítio Compartilhados (`Site & Climate`)** *(Tamanho: S · Base de `#122`, `#130`, `#131`, `#140`)*
  - **Ref. BIMCraft:** `crates/core/src/codes.rs` (`Site`, comando `site_climate`).
  - **O que implementar:**
    - Centralizar no projeto os dados do local: cidade/UF, latitude/longitude, zona bioclimática NBR 15220 (1–8), velocidade básica do vento `V0` (NBR 6123), intensidade pluviométrica `I` (mm/h, NBR 10844), índice ceráunico `AQ1/AQ2/AQ3` (dias de trovoada/ano para DPS/SPDA), e temperaturas de projeto.
    - Integrar com presets regionais brasileiros editáveis e conectar automaticamente ao vento estrutural já existente, à futura drenagem pluvial e ao módulo de DPS/SPDA.
  - **Critério de pronto:** escolher uma cidade/preset atualiza `V0`, chuva `I` e nível ceráunico em um único local, com sincronização via UI e MCP.

- [ ] **T-10 · Verificação em Tempo Real de Códigos de Obra: Escadas (26 regras + Solver), Rampas, Portas e Habitabilidade/Iluminação (`#73`, `#132`)** *(Tamanho: L)*
  - **Ref. BIMCraft:** `crates/core/src/codes/{stairs.rs, checks.rs}`, `data/codes/architectural/nbr-9050.json` e `nbr-9077.json`.
  - **O que implementar:**
    - Embarcar regras JSON auditadas para **NBR 9050:2020**, **NBR 9077:2001** e template de **Código de Obras Municipal Brasileiro** (áreas mínimas, menor lado inscrito, pé-direito mínimo, razão de iluminação e ventilação natural `ΣA_janelas / A_piso` com busca espacial nas paredes do cômodo, vãos livres de porta descontando batente).
    - Expandir escadas com **26 verificações normativas** (incluindo *headroom* / pé-direito livre 3D sobre cada degrau consultando lajes/vigas acima) + **Solver Interativo de Escadas (`stair_solve`)** que sugere soluções válidas de menor alteração respeitando travas (*locks* de número de espelhos, largura ou patamar).
    - Exibir achados no *Segurança* (semáforo) e em tempo real ao desenhar/arrastar, sem nunca bloquear o desenho.
  - **Critério de pronto:** cômodos sem iluminação suficiente, portas estreitas, rampas fora da NBR 9050 e escadas com baixo headroom acendem no semáforo e no MCP com explicação + solução sugerida.

- [ ] **T-11 · Expansão das Concessionárias Elétricas Brasileiras + Demanda Coletiva Multifamiliar** *(Tamanho: M · Evolui B21 e `#140`)*
  - **Ref. BIMCraft:** `crates/core/src/demand.rs`, `entrance.rs`, `building_demand.rs`, `data/codes/electrical/{cpfl, enel, celesc, cemig, copel, energisa, equatorial, fecoergs, hidropan}.json`.
  - **O que implementar:**
    - Migrar `src/domain/utility_profiles.rs` para o motor `codes` em JSON versionado e expandir de 4 para **9 concessionárias brasileiras** (CPFL GED-13 completo `D = a..i`, Enel GRI-0017, Celesc N-321.0001, CEMIG ND-5.1/ND-5.2, COPEL NTC 901100 + rural, Energisa NDU-001 por distribuidora, Equatorial NT.00001, Fecoergs + cooperativas, Hidropan).
    - Adicionar cálculo de **Demanda Coletiva de Edifícios Residenciais** (tabelas CODI / CEMIG ND-5.2 / Celesc N-321.0003) para pequenos edifícios de até 3 pavimentos com múltiplas unidades.
  - **Critério de pronto:** testes reproduzem tabelas oficiais das 9 concessionárias e da demanda coletiva; UI/MCP/unifilar exibem norma, edição e tabela exata.

---

### FASE 3 — Produtividade 2D, Documentação Profissional e UX Desktop

- [ ] **T-12 · Cotas Interativas Editáveis (*Listening Dimensions*), Travas `Lock`/`EQ` e Régua/Guias Angulares** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/constraints.rs`, `snap.rs`, `alignment.rs`, `README.md` (Snaps & Annotation).
  - **O que implementar:**
    - Ao selecionar uma parede/abertura/objeto, destacar as cotas ligadas a ele; clicar no valor da cota abre um campo numérico inline e digitar o novo valor move a referência (mantendo a outra fixa).
    - Adicionar cadeado (`Lock`) e igualdade (`EQ`) em cadeias de cotas associativas com solver pós-edição que mantém equidistância e alinha referências automaticamente.
    - Desenhar régua graduada dinâmica junto à linha sendo traçada, com arco de ângulo no vértice e guias magnéticas inteligentes (5°/15°/30°/45°/90°, paralelo e perpendicular a segmentos vizinhos).
  - **Critério de pronto:** editar uma cota de 3,00 m para 3,50 m desloca a parede e preserva junções; cadeia `EQ` redistribui divisórias igualmente após mover a parede externa.

- [ ] **T-13 · Hierarquia Gráfica Profissional: `View Range` (4 planos), `Object Styles`, `Line Weights` (Penas 1–16) e `Linework` (`LW`)** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/view_range.rs`, `styles.rs`, `pen.rs`, `lines.rs`.
  - **O que implementar:**
    - Introduzir `ViewRange` por vista de planta (`Top`, `Cut Plane` 1,20 m padrão, `Bottom`, `View Depth`) dividindo o desenho em projeção acima do corte (tracejada, ex.: telhados/vigas), corte (poché), projeção abaixo e profundidade em halftone.
    - Criar tabela de **Line Weights** (penas 1 a 16 por escala + pena 0 hairline) e **Object Styles** (pena de corte e projeção por categoria/subcategoria), com alternância rápida **Thin Lines (`TL`)** na tela.
    - Adicionar **Line Patterns** editáveis (traços, pontos, ondas, zigue-zague e texto em mm de papel, ex.: `--- AF ---`, `--- ESG ---`) e ferramenta **Linework (`LW`)** para sobrescrever o estilo de uma aresta individual na vista (inclusive `<Linha invisível>`).
  - **Critério de pronto:** mesma planta impressa em 1:50 e 1:100 respeita a tabela de penas por escala; `ViewRange` controla corretamente o que aparece acima/abaixo do plano de corte.

- [ ] **T-14 · Visibility/Graphics Overrides (`VV`), Filtros de Vista, View Templates e Hide/Isolate Temporário** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/graphics.rs`, `ui-egui/src/{graphics.rs, templates.rs, halftone.rs}`.
  - **O que implementar:**
    - Dialog `VV` por vista controlando visibilidade, cor/pena/padrão de corte e projeção, halftone (%) e transparência por categoria, subcategoria e por elemento individual.
    - **View Filters** (até 64 filtros por regras sobre parâmetros/materiais/fases/semáforo — ex.: "destacar paredes estruturais em azul" ou "destacar vigas com uso > 85% em laranja").
    - **View Templates** nomeados com checkboxes `Incluir` por grupo (escala, nível de detalhe `Baixo/Médio/Alto`, V/G, `View Range`, fases), propagando alterações automaticamente a todas as vistas vinculadas.
    - Controles rápidos na barra da vista: **Ocultar/Isolar Temporário (`HH`/`HI`/`HR`)** e **Revelar Elementos Ocultos (`RH`)**.
  - **Critério de pronto:** alterar um View Template atualiza imediatamente a tela e o PDF das pranchas que o usam.

- [ ] **T-15 · Planta de Forro Refletido (RCP), Vistas Duplicadas/Dependentes, Callouts de Detalhe e `Trace & Reference`** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/underlay.rs`, `render.rs` (`DetailView`, RCP, `duplicate_view`), `background_grid.rs`.
  - **O que implementar:**
    - **Planta de Forro Refletido (RCP)** por pavimento (corte a 2,30 m olhando para cima: forro, luminárias, vigas, shafts e sancas).
    - **Duplicar Vista (`DP`)** (com ou sem anotações) e **Vistas Dependentes** (recortes independentes herdando gráficos/escala da vista-mãe, ideal para dividir plantas grandes em várias pranchas).
    - **Callouts / Vistas de Detalhe (`DetailView`)**: retângulo ampliado (1:10, 1:20, 1:25) com marca de chamada clicável e sincronizada na prancha (banheiros, escadas, nós estruturais).
    - **Trace & Reference (`Underlay` entre vistas)**: sobrepor qualquer outra planta/pavimento/corte em cor única com deslocamento, rotação e snapping (sem ser selecionável nem impresso).
    - **Background Grid** configurável (grade principal + subgrade rotacionável com snap).
  - **Critério de pronto:** um projeto gera planta baixa, RCP, planta de detalhes 1:20 e vistas dependentes sobre o mesmo modelo único.

- [ ] **T-16 · Fases de Obra (*Existente / Demolir / Construir*) e Opções de Projeto (*Design Options*)** *(Tamanho: L · Altíssimo valor para reformas no Brasil!)*
  - **Ref. BIMCraft:** `crates/core/src/phasing.rs`, `ui-egui/src/phasing.rs`.
  - **O que implementar:**
    - Fases ordenadas no projeto (padrão: `Existente` → `Construção Nova`), com `fase_criacao` e `fase_demolicao` em cada elemento (aberturas herdam da parede hospedeira).
    - Filtros de fase (*Mostrar Tudo*, *Existente*, *Demolição*, *Construção Nova*, *Final*) com overrides padrão brasileiros (existente em cinza/halftone, a demolir tracejado amarelo/vermelho, a construir cheio/vermelho) + ferramenta **Demolir (`DW`)** com 1 clique.
    - Integrar fases aos quantitativos (separar volume de demolição vs. construção nova!) e ao cálculo estrutural (elementos demolidos não carregam a estrutura nova).
    - **Opções de Projeto (`Design Options`)**: conjuntos de alternativas (ex.: *Layout Cozinha: Ilha vs. Península*) com visualização alternável, comparação de áreas/custos e botão *Aceitar Principal*.
  - **Critério de pronto:** planta de reforma gera automaticamente pranchas de Existente, Demolir/Construir e Final; quantitativos e estrutura respeitam a fase ativa.

- [ ] **T-17 · Parâmetros Customizados por GUID (Instância/Tipo), Dicionários `.trazoparams` e `Project Information`** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/parameters.rs`, `shared_parameters.rs`, `project_info.rs`.
  - **O que implementar:**
    - Permitir criar parâmetros customizados (texto, inteiro, número, comprimento, área, volume, ângulo, sim/não) com escopo `Instância` ou `Tipo`, identificados por GUID (unificando project/shared parameters).
    - Dicionários externos versionados em JSON (`.trazoparams`) compartilháveis entre projetos/Git, operações em lote (mesclar, purgar não usados, importar/exportar CSV/XLSX) e leitura de arquivos `.txt` de *Shared Parameters* do Revit.
    - Singleton `ProjectInformation` (obra, cliente, endereço, nº processo prefeitura, autor, CREA/CAU, data, revisão) ligado automaticamente aos carimbos das pranchas e ao memorial PDF.
  - **Critério de pronto:** criar um parâmetro customizado (ex.: `Código SINAPI` ou `Resistência ao Fogo`) permite editá-lo no inspetor, filtrá-lo em vistas, tabelá-lo e exibi-lo em tags.

- [ ] **T-18 · Schedules Configuráveis com Fórmulas, Filtros e Exportação Nativa `.xlsx` / `.ods` / `.odt`** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/schedule.rs`, `xlsx.rs`, `ods.rs`, `odt.rs`.
  - **O que implementar:**
    - Evoluir os quadros atuais do Trazo para um motor genérico de tabelas com: escolha e ordenação de colunas (incluindo parâmetros customizados), **campos calculados com fórmulas** (`Perimetro * PeDireito`, `Area * PrecoM2`), **14 operadores de filtro**, ordenação múltipla (respeitando ordem cronológica de fases), agrupamento com subtotais e **Multi-Category Schedules**.
    - Adicionar escritor nativo **`.xlsx`** (sobre `flate2`/`zip`, com números reais em vez de texto — sem dependências pesadas) e **`.ods`**, além de preenchimento de templates **LibreOffice `.odt`** (para gerar contratos, memoriais descritivos editáveis e propostas direto do modelo).
  - **Critério de pronto:** criar tabela de ambientes com campo calculado de pintura/custo, exportar `.xlsx` abrível no Excel/LibreOffice com fórmulas/números válidos e colocar na prancha.

- [ ] **T-19 · Padrões `.PAT` (Import/Export + Pattern Maker), Cores RAL/CSS, Hachura Dupla e `Copy/Paste` em 6 Modos** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/pat.rs`, `fills.rs`, `standard_colors.rs`, `ral.rs`, `clipboard.rs`.
  - **O que implementar:**
    - Leitor/escritor de arquivos `.pat` padrão AutoCAD/Revit (drafting e model patterns) + **Pattern Maker** (desenhar linhas na planta e converter em padrão de hachura reutilizável).
    - Hachura em 2 camadas (fundo sólido + padrão sobreposto) e paleta embarcada de cores **RAL Classic** e **CSS** + tabela CSV customizada do usuário.
    - Evoluir `src/app/clipboard.rs` para serializar mini-projeto JSON no clipboard do SO (funciona entre janelas/projetos distintos!) com os **6 modos de colagem**: *Manual*, *Na vista atual*, *No mesmo lugar*, *Alinhado aos pavimentos selecionados*, *Alinhado às vistas selecionadas* e *No pavimento clicado em corte*, re-hospedando aberturas automaticamente.
  - **Critério de pronto:** importar um `.pat` real renderiza na tela/SVG/PDF; copiar um banheiro inteiro de um pavimento ou outra janela e colar *Alinhado ao 2º Pavimento* mantém portas, louças e paredes íntegras.

- [ ] **T-20 · Engine de Exportação Unificada, PDF/A (`krilla`), Print Sets e Publisher em Lote** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/export/`, `pdf.rs`, `printset.rs`, `publish.rs`, `ui-egui/src/export/`.
  - **O que implementar:**
    - Unificar exports em um pipeline (`Job` → `Prepared` → `Format`) rodando em worker thread sobre snapshot do modelo (usuário continua desenhando durante exportação), com escrita atômica e cancelamento.
    - Adotar **`krilla`** para gerar PDFs vetoriais compatíveis com **PDF/A-1b, 2b, 2u, 3b e 4** (arquivamento oficial em prefeituras/cartórios), bookmarks automáticos, marcas de corte em paginação *tiled* e modo Cor / Escala de Cinza / Preto-e-Branco.
    - Criar **Print Sets** (conjuntos salvos de vistas/pranchas) e **Publisher** em lote com nomenclatura por tokens (`{NumeroPrancha}_{NomePrancha}_R{Revisao}_{Data}.pdf`), múltiplos formatos por conjunto e export/import de presets em XML.
    - Permitir recorte por região customizada (`ExportRegion`) inclusive em SVG/DXF.
  - **Critério de pronto:** um clique no Publisher gera o pacote completo da obra (PDF/A + DXF + XLSX de quantitativos + Memorial) com nomes padronizados sem congelar a UI.

- [ ] **T-21 · UX Desktop Avançada: Atalhos de 2 Letras, Paleta de Comandos, Tema Escuro, Docking, Minimap, Pin & Edit Group** *(Tamanho: M · Atende `#69` e `#82`)*
  - **Ref. BIMCraft:** `crates/ui-egui/src/{shortcuts.rs, dock.rs, minimap.rs, groups.rs}`, `core/src/groups.rs`.
  - **O que implementar:**
    - Atalhos editáveis de 2 letras estilo Revit (`WA` parede, `DR` porta, `WN` janela, `DI` cota, `VV` gráficos, `MV` mover, `TR` trim, `SL` dividir, `MA` igualar propriedades, `PN`/`UP` fixar/soltar) + **Paleta de Comandos (`Ctrl+K`)** com busca fuzzy e comandos recentes.
    - **Tema Escuro** completo (mantendo pranchas em papel branco) + escala de interface e acessibilidade de teclado (AccessKit do `egui`).
    - Painéis laterais encaixáveis/flutuantes com layout persistido + **Minimap** interativo para plantas grandes.
    - **`Pin`/`Unpin`** (protege eixos, limites e pilares contra movimentação acidental), **`Match Properties` (`MA`)**, **`Edit Group` (`GE`)** em isolamento acinzentado (com grupos flexíveis entre pavimentos e *Exclude Member*) e **Repeating Details** (detalhes 2D repetidos ao longo de uma linha, ex.: telhas/blocos em corte).
  - **Critério de pronto:** todos os atalhos funcionam e são reconfiguráveis; tema escuro cobre todas as janelas sem perder contraste; elementos fixados recusam movimentação/exclusão.

---

### FASE 4 — Modelagem Arquitetônica Avançada, Famílias, Terreno e Georreferenciamento

- [ ] **T-22 · Paredes Avançadas: Paredes Curvas, Inclinadas (*Slanted*), `Edit Profile` e Junção de Núcleo** *(Tamanho: L)*
  - **Ref. BIMCraft:** `crates/core/src/walls/{mod.rs, compound.rs, profile.rs, slant.rs}`.
  - **O que implementar:**
    - Suporte a **paredes em arco** (centro/raio/ângulo) com aberturas hospedadas, cotas radiais/de arco e junções limpas.
    - **Junção inteligente de núcleo em cantos compostos**: núcleo estrutural encontra em topo (*butt*) enquanto reboco/acabamento fecha em meia-esquadria limpa.
    - **Paredes inclinadas (`slant` ±60°)** e **`Edit Wall Profile`** (editar contorno e recortes/furos em elevação mantendo junções e aberturas), além de **`Disallow Join`** por extremidade.
  - **Critério de pronto:** paredes curvas e recortadas em elevação aparecem corretamente em planta, corte, elevação, quantitativos de alvenaria e (na Fase 5) em 3D.

- [ ] **T-23 · Paredes Cortina (*Curtain Walls*) Procedurais** *(Tamanho: L)*
  - **Ref. BIMCraft:** `crates/core/src/curtain.rs`, `curtain/surface.rs`, `ui-egui/src/curtain.rs`.
  - **O que implementar:**
    - Tipo de parede cortina com grid procedural vertical/horizontal (`Fixo`, `Flexível`, `Uniforme`), montantes (*mullions*) de seção configurável, montantes de canto automáticos, painéis envidraçados ou opacos, junção por profundidade/meia-esquadria, adição/remoção de segmentos (fusão de painéis), seleção individual via `Tab` e substituição de um painel por parede real com porta.
    - Quantitativo automático de área de vidro e metros lineares de perfis/montantes.
  - **Critério de pronto:** fachada envidraçada residencial/comercial com porta embutida é modelada, desenhada em planta/elevação/3D e quantificada sem gerar milhares de elementos soltos.

- [ ] **T-24 · Telhados em Polígono Livre (*Straight Skeleton*), Pátios, Águas-Furtadas e Caimentos Automáticos para Ralos** *(Tamanho: L · Atende `#71`)*
  - **Ref. BIMCraft:** `crates/core/src/skeleton.rs`, `falls.rs`.
  - **O que implementar:**
    - Evoluir `src/domain/roof/` (que hoje cobre retângulos meia/duas/quatro águas) com algoritmo **Straight Skeleton** para gerar automaticamente cumeeiras, espigões e águas-furtadas (*valleys*) sobre **qualquer polígono em L, U ou com pátio interno**, permitindo inclinação individual ou oitão por aresta, além de telhados por extrusão.
    - Conectar a geometria da planta de cobertura (`#71`: setas de caimento, calhas, condutores e beirais) diretamente ao dimensionamento de madeiramento NBR 7190 já existente no Trazo.
    - Implementar **`falls` (caimentos automáticos de lajes/terraços para ralos)**: particionamento por cumeeiras + Voronoi por ralo + caminho mínimo Dijkstra contornando aberturas/shafts, garantindo declividade mínima em todas as faces.
  - **Critério de pronto:** casa em L gera planta de cobertura completa com espigões/rincões corretos e laje impermeabilizada gera planos de caimento válidos para cada ralo.

- [ ] **T-25 · Shape Editing de Lajes/Pisos (Delaunay Restrito Exato), `Shaft Openings`, `Room Separators` e Acabamentos Automáticos** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/triangulation.rs`, `rooms.rs`, `spots.rs`, `ui-egui/src/shape.rs`.
  - **O que implementar:**
    - Triangulador **Delaunay restrito determinístico em grade inteira (0,01 mm)** para editar cotas de pontos e linhas de quebra em pisos, terraços e terrenos, com suporte a **camada variável** (contrapiso/regularização mantém fundo da laje plano!) e anotação **`Spot Slope`** (seta de declividade).
    - **`Shaft Openings`**: abertura vertical que recorta automaticamente todas as lajes, forros e telhados atravessados em vários pavimentos (com X padrão ABNT em planta).
    - **`Room Separation Lines`** (divide ambientes integrados sala/jantar sem parede real), `Room Bounding` opcional por parede/pilar, **Frozen Rooms** (para performance) e geração automática de pisos/forros a partir dos acabamentos do `RoomType`.
  - **Critério de pronto:** shaft de escada/elevador recorta múltiplos pavimentos automaticamente; sala/jantar integrados com linha separadora calculam áreas e acabamentos separadamente.

- [ ] **T-26 · Escadas e Rampas Avançadas (Leque/Balanceadas, Espirais, Sketch, Multistorey e Longarinas)** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/stairs.rs`, `ramps.rs`, `ui-egui/src/stairgrips.rs`.
  - **O que implementar:**
    - Expandir `src/domain/circulation.rs` com degraus em leque (*winders*) e balanceados, escadas espirais, escadas por sketch livre, **escadas multistorey** (adaptando número de espelhos em pavimentos de pés-direitos distintos), modos construtivos (monolítica, longarina lateral, viga central, degraus engastados) e abertura automática da laje superior (`stair_opening`).
    - Expandir rampas com trechos em arco/circular e modo laje inclinada por camadas; guarda-corpos acompanhando automaticamente escadas e rampas.
    - Integrar os novos tipos ao pré-dimensionamento estrutural de escadas (`src/domain/cantilevers.rs`).
  - **Critério de pronto:** escadas em L/U com leque, espirais e multistorey validam na NBR 9050/9077 e alimentam o pré-dimensionamento estrutural.

- [ ] **T-27 · Motor de Famílias Paramétricas 3D/2D (`.trazof`) com CSG/Voids e Solver de Restrições** *(Tamanho: XL)*
  - **Ref. BIMCraft:** `crates/core/src/family.rs`, `family/*`, `csg.rs`, `ui-egui/src/editor.rs`.
  - **O que implementar:**
    - Substituir/estender fixtures estáticos por **famílias paramétricas** compostas de formas 3D (`Extrusion`, `Revolve`, `Blend`, `Sweep`, `SweptBlend`) + **Voids booleanos via árvore BSP iterativa** (`csg.rs`, com orçamento de triângulos contra geometria hostil) + símbolos 2D em planta com visibilidade por nível de detalhe.
    - Avaliador de fórmulas (`Width / 2`, `if(Depth > 600, 4, 2)`), tipos e instâncias, planos de referência com solver linear de restrições (`EQ`/`Lock`), famílias aninhadas, *Place on Face* e conectores MEP/estruturais embutidos.
    - Editor de Famílias integrado com preview 3D girável, catálogo inicial CC0 (portas, janelas, mobiliário, louças, quadros, luminárias, peças hidráulicas/SPDA/dados) e Favoritos globais do usuário.
  - **Critério de pronto:** editar `Largura` ou `Altura` de uma janela paramétrica atualiza folhas, caixilhos, vidros, vão na parede, símbolo em planta, tabela e 3D.

- [ ] **T-28 · Componentes Adaptativos (DAG Paramétrico + Frames 6-DOF)** *(Tamanho: L · Opcional/Avançado)*
  - **Ref. BIMCraft:** `crates/core/src/adaptive/`, `ui-egui/src/adaptive_*.rs`.
  - **O que implementar:**
    - Família adaptativa baseada em grafo acíclico (DAG) de pontos numerados, curvas Bézier/polilinhas e formas `Patch`/`Extrude`/`Loft` com orientação 6-DOF (método de quatérnios de Horn + normal de Newell) para brises, pergolados curvos, painéis de fachada, treliças especiais e coberturas livres.
  - **Critério de pronto:** colocar um painel ou treliça adaptativa clicando em 4 pontos de referência gera geometria limpa e tabelável.

- [ ] **T-29 · Massas Conceituais (*Massing*), Terreno 3D (`Toposolid`) e Limites com Azimute/Rumo** *(Tamanho: L)*
  - **Ref. BIMCraft:** `crates/core/src/massing.rs`, `site.rs`, `docs/areas/site.md`, `property-boundaries.md`.
  - **O que implementar:**
    - **Estudo de Massa (`Mass`)**: extrusões e lofts translúcidos com `Mass Floors` por pavimento (cálculo imediato de área computável/coeficiente de aproveitamento/taxa de ocupação!) e geração associativa de paredes/pisos/telhados pelas faces (`Wall/Floor/Roof by Face` + `Update to Face`).
    - **Terreno 3D (`Toposolid`)**: sólido topográfico moldado por pontos de cota (manual, CSV ou curvas DXF 3D) usando o triangulador Delaunay de `T-25`, gerando curvas de nível automáticas em planta, cortes de terreno em seções e cálculo futuro de corte/aterro (*Graded Region*).
    - **Limites de Propriedade (`PropertyLine`)**: tabela de rumos/azimutes reais (integrada ao georreferenciamento), área/perímetro de matrícula e linha traço-ponto ABNT.
  - **Critério de pronto:** importar levantamento topográfico simples gera terreno 3D com curvas de nível e corte real do lote na seção.

- [ ] **T-30 · Georreferenciamento Real (EPSG/SIRGAS 2000), Norte Verdadeiro, `Spot Coordinates` e Mapa Offline** *(Tamanho: M · Desbloqueia Carta Solar `#130` e IFC georreferenciado)*
  - **Ref. BIMCraft:** `crates/core/src/coordinates.rs`, `crs.rs`, `spots.rs`, `assets/map/`.
  - **O que implementar:**
    - Adotar `proj4rs` + `crs-definitions` (puro Rust) suportando sistemas projetados brasileiros (**SIRGAS 2000 / UTM zonas 18S–25S**, ex.: `EPSG:31983`).
    - Modelo limpo de 3 pontos (`Origem Interna` próxima a 0,0, `Project Base Point` e `Survey Point`) com múltiplos sítios nomeados, rotação para **Norte Verdadeiro** (`Orientation::TrueNorth` em plantas e carimbo/seta norte automática), **`Spot Coordinates`** e conversão `internal ↔ base ↔ shared ↔ WGS84`.
    - Embutir mapa vetorial offline compacto (Natural Earth) para escolher latitude/longitude da obra (alimentando diretamente `Site & Climate` e a Carta Solar / Sombras `#130`).
  - **Critério de pronto:** coordenadas em SIRGAS 2000 convertem com precisão milimétrica para o modelo local e para WGS84, rotacionando corretamente a planta para o Norte Verdadeiro.

---

### FASE 5 — Aceleração de Canvas 2D, Cortes Escaláveis e Motor 3D Nativo (`wgpu`) (`#85`, `#62`)

- [ ] **T-31 · Aceleração do Canvas 2D na GPU (Triângulos Retidos `wgpu` + *Sharp Zoom*)** *(Tamanho: L · Atende `#62`)*
  - **Ref. BIMCraft:** `crates/ui-egui/src/{retained.rs, retained.wgsl, motion.rs, view_worker.rs}`.
  - **O que implementar:**
    - Tesselar a planta uma vez em worker thread (com `rayon`) e manter os triângulos em buffers `wgpu` via `PaintCallback`: frames ociosos caem para `< 0,5 ms`, pan vira apenas deslocamento de uniform (`~0,05 ms`) e hover/seleção reescrevem apenas o intervalo de vértices do elemento afetado.
    - Implementar o shader de ***Sharp Zoom*** (`motion.rs`): durante zoom contínuo, expandir segmentos no vertex shader com espessura constante em pixels e anti-aliasing analítico de 1 px (eliminando o borramento de zoom em plantas densas).
  - **Critério de pronto:** pan e zoom em planta de 500 paredes mantêm 60 fps constantes sem borramento.

- [ ] **T-32 · Motor de Cortes e Elevações Escaláveis (`SectionWork` + Occlusion Culling + Workers)** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/render.rs` (`SectionWork`, `SectionOverlay`), `ui-egui/src/section_worker.rs`.
  - **O que implementar:**
    - Evoluir `src/domain/views.rs` para usar `SectionWork` compartilhável entre vistas/pranchas, volume culling (descarta o que está fora do corte/`far_clip`/crop), occlusion culling em tiles 8×8 px para linhas ocultas atrás de paredes/lajes e `Retint` in-place para hover/seleção em cortes.
    - Mover geração de cortes/elevações para workers canceláveis exibindo o último quadro reprojetado durante pan/zoom.
    - Permitir edição básica (mover aberturas/cotas/linhas de detalhe) diretamente dentro do corte/elevação.
  - **Critério de pronto:** cortes e elevações de sobrados completos mostram corretamente elementos além do corte em linha oculta, atualizam em background e renderizam em `< 200 ms`.

- [ ] **T-33 · Motor 3D Nativo (Rasterizador CPU + Pipeline GPU `wgpu` em 5 Passes + Instancing + BVH Picking)** *(Tamanho: XL · Atende `#85`)*
  - **Ref. BIMCraft:** `crates/core/src/view3d.rs`, `view3d/{gpu.rs, update.rs, fragments.rs}`, `ui-egui/src/{gpu.rs, gpu.wgsl}`.
  - **O que implementar:**
    - Criar abstração `Scene` no domínio (headless, sem UI): gera malhas 3D de paredes (com recortes de aberturas), pisos/lajes (com shafts), telhados/madeiramento/tesouras, pilares/vigas/sapatas/baldrames/mourões, escadas/rampas, famílias e instalações.
    - **Rasterizador CPU de referência** (usado em exports PNG/PDF/memorial, CLI e fallback sem GPU) + **Pipeline GPU `wgpu` em 5 passes** (opacos + nuvem de pontos → translúcidos → pós-processamento com detecção de arestas por `plane_id`, silhuetas e sombras ambientais → linhas de nível/wireframe → work plane).
    - **Instancing GPU** (componentes, tubos/eletrodutos, montantes de parede cortina), **BVH Picking** em espaço 3D, **Frustum Culling**, **Frame Budget Progressivo** (caixas *stand-in* ao orbitar projetos grandes), **Atualização Incremental (`Scene::update`)** após edições e degradação graciosa sob pouca VRAM.
    - Permitir exibir no 3D os mesmos **filtros por disciplina** (Só Arquitetura, Só Estrutura com cores do semáforo, Só Elétrica, Só Hidráulica!).
  - **Critério de pronto:** qualquer projeto do Trazo (incluindo *Casa Térrea*, *Sobrado* e *Área Gourmet*) abre em vista 3D fluida a 60 fps com seleção por clique, sincronizada em tempo real com a planta 2D.

- [ ] **T-34 · Recursos Visuais 3D: Câmeras Perspectiva, `Section Box` com Caps, Estilos Visuais, Texturas e `Paint`** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/core/src/view3d/caps.rs`, `texture.rs`, `ui-egui/src/{camera.rs, section_box.rs, display.rs}`.
  - **O que implementar:**
    - Alternância **Axonométrica / Perspectiva** (distância focal + altura do observador) e ferramenta **Câmera** em planta criando vistas 3D nomeadas (com ícone clicável na planta e inclusão em pranchas!).
    - **`Section Box` (`BX`)** com 6 faces arrastáveis em 3D cortando modelo e nuvens de pontos, desenhando **Caps hachurados** nas faces cortadas (e opção de sincronizar planta ao Section Box).
    - Estilos visuais (*Shaded*, *Hidden Line*, *Wireframe*, *Consistent Colors*), silhuetas, **Ambient Shadows (SSAO)** e **Texturas reais** (procedurais em código — tijolo, bloco, madeira, telha, concreto, piso — ou PNG/JPEG com tamanho real em metros) + ferramenta **`Paint` (`PT`)** para revestir faces individuais.
  - **Critério de pronto:** o usuário consegue cortar a casa ao meio com o Section Box mostrando estrutura + alvenaria + telhado, salvar câmeras internas/externas e inseri-las na prancha PDF.

- [ ] **T-35 · Navegação 3D Profissional: Órbita Inteligente, Fly Mode, Tríade Interativa, Linked 3D e SpaceMouse** *(Tamanho: S)*
  - **Ref. BIMCraft:** `crates/ui-egui/src/{orbit.rs, fly.rs, triad.rs, spacemouse.rs}`.
  - **O que implementar:**
    - Órbita centrada no ponto sob o cursor ou na seleção, seleção por caixa (window/crossing) em 3D, **Fly Mode (`FY`)** estilo FPS (`WASD`/`QE` + roda controla velocidade), **Tríade de Eixos X/Y/Z** clicável no canto (com `Alt+clique` definindo plano de trabalho), **Vista 3D Vinculada** ao recorte da planta (lado a lado) e suporte opcional a **3Dconnexion SpaceMouse** (`hidapi`, Windows).
  - **Critério de pronto:** navegar internamente pelos cômodos em Fly Mode e trabalhar com planta 2D + 3D sincronizados lado a lado.

---

### FASE 6 — Disciplinas de Engenharia & MEP (Completando M5, M6 e M7)

- [ ] **T-36 · Abstração Unificada `Run` (Condutos, Tubos, Vigas Inclinadas, Eletrocalhas) e Conectores em Famílias** *(Tamanho: M · Base de toda a Fase 6)*
  - **Ref. BIMCraft:** `crates/core/src/runs.rs`, `glue.rs`, `fixtures.rs` (`ConnectorKind`).
  - **O que implementar:**
    - Criar primitiva linear 3D `Run` (`start`, `end`, `elevation_start`, `elevation_end`, inclinação até 75° ou prumada vertical, seção redonda/retangular/perfil, justificativa topo/centro/base, `system` nomeado e mitragem automática em encontros).
    - Definir `Connector` tipado nas famílias (`Power`, `Switch`, `Pipe { domain, flow, dn }`, `Data`, `Earthing`) com coordenadas locais 3D e fórmulas de carga/bitola.
  - **Por que agora:** com 1 único tipo geométrico + conector, o Trazo destrava de uma vez Hidráulica, Esgoto, Pluvial, Incêndio, Elétrica 3D, SPDA, Dados e Estruturas Metálicas!
  - **Critério de pronto:** prumadas verticais, ramais inclinados e trechos horizontais renderizam em planta, corte (como anel/seção), 3D e exportam como `IfcSystem`.

- [ ] **T-37 · Estrutura Metálica (Perfis I/H/U/L/T/RHS/CHS) e *Wood/Steel Framing* com Lista de Corte Otimizada** *(Tamanho: L)*
  - **Ref. BIMCraft:** `crates/core/src/profiles.rs`, `framing.rs`, `framing/{floor.rs, roof.rs}`, `timber.rs`.
  - **O que implementar:**
    - Biblioteca paramétrica de perfis metálicos (`I/H`, `U`, `L`, `T`/T-invertido, `RHS`/`SHS`, `CHS`) com catálogos EN (`IPE/HEA/HEB`) e **Gerdau/ABNT (`W`, `HP`, `U`, `L`)**, área analítica, inércias `Ix/Iy`, `Wx/Wy`, massa linear (kg/m) e integração ao pórtico plano 2D do Trazo (verificação preliminar NBR 8800).
    - Geração procedural (derivada, nunca duplicada) de **Wood Frame / Light Steel Frame** em paredes, pisos e telhados (guias, montantes, vergas, ombreiras, barrotes, caibros) com otimização de emendas por programação dinâmica $O(n^2)$ contra comprimentos comerciais de barra e geração de lista de corte.
  - **Critério de pronto:** selecionar um perfil `W 200×19,3` ou ativar Framing numa parede calcula peso/lista de corte, exibe em 3D/cortes e entra nos quantitativos.

- [ ] **T-38 · Armaduras 3D, Plano de Corte de Aço (*Cut Plan*), Desconto Real de Concreto/Fôrma e Prancha Individual (`RebarDetail`)** *(Tamanho: L · Evolui `#110` para nível executivo)*
  - **Ref. BIMCraft:** `crates/core/src/{rebar.rs, reinforce.rs, steel.rs, quantities.rs, rebar_detail.rs, sheet/detail.rs}`.
  - **O que implementar:**
    - Converter a armadura preliminar já calculada pelo Trazo (`src/domain/reinforcement/`) em barras 3D reais (`Element::Rebar` com `path: Vec<P3>`, dobras, ganchos, barras dobradas e desvios de espera em pilares), visíveis em 3D (transparência do concreto), cortes (pontos sobre o poché) e plantas.
    - **Reconhecimento canônico de forma 3D (`Shape::key`) + Numeração `N1, N2…`** por elemento e **Plano de Corte Otimizado (`Cut Plan`, Best-Fit Decreasing)** para barras comerciais de 12 m (considerando perda de corte `kerf`, reaproveitamento de pontas `min_offcut` e exclusão de rolos até Ø8 mm), exportado em CSV e `.xlsx`.
    - **Quantitativo geométrico exato de Concreto e Fôrma (`quantities::Index`)**: desconta automaticamente sobreposição viga×pilar, viga×laje e pilares×laje, além da taxa de aço real (kg/m³) por elemento e pavimento.
    - **Vista `RebarDetail`**: gera automaticamente prancha individual de detalhamento de cada viga/pilar/sapata/laje (elevação + cortes transversais em 2× escala + tabela de aço com croqui de dobra), pronta para o engenheiro revisar e assinar.
  - **Critério de pronto:** plano de corte reduz sobras vs. cálculo bruto +10%, fecha massa 100% com a tabela de aço e gera prancha vetorial individual da viga/pilar.

- [ ] **T-39 · Hidrossanitário e Incêndio Completos: Rede 3D + Cálculos NBR 5626, NBR 8160, NBR 10844 e IT-22** *(Tamanho: XL · Atende `#72`, `#96`, `#118`, `#119`, `#120`, `#122`, `#123`)*
  - **Ref. BIMCraft:** `crates/core/src/{network.rs, plumbing.rs, water.rs, reservoir.rs, drainage.rs, storm.rs, hydrant.rs}`, `data/codes/plumbing/{nbr.json, cbpmesp-it22.json}`.
  - **O que implementar:**
    - Ativar a aba e disciplina **Hidrossanitário** (já preparada em `Discipline::Plumbing`) com tubos `Run` (Água Fria, Água Quente, Esgoto, Ventilação, Pluvial, Gás, Incêndio), conexões (joelho, tê, redução, registro, caixa sifonada, caixa de gordura/inspeção) e aparelhos com conectores de DN/pesos/UHC.
    - **Análise Topológica da Rede (`network.rs`, Union-Find + spatial hash)**: detecta pontas abertas (*Show Disconnects*), cruzamento indevido de sistemas/domínios, ligações no meio do tubo e **redução ilegal de diâmetro a jusante em esgoto**.
    - **Motores normativos conectados ao modelo + semáforo de segurança + memorial PDF + MCP**:
      1. **Água Fria (NBR 5626:2020 + Tigre)**: método dos pesos $Q = 0{,}3\sqrt{\sum P}$, perda de carga universal **Darcy-Weisbach + Colebrook-White**, tabela PVC soldável DE 20–110 mm, comprimentos equivalentes, $v \le 3{,}0\text{ m/s}$ e pressão dinâmica mínima $\ge 10\text{ kPa}$ no chuveiro/peça!
      2. **Reservação (NBR 5626:2020)**: volume potável (1–3 dias), RTI incêndio, tempo de enchimento, compartimentos, bombas e integração direta com carga concentrada da caixa d'água na estrutura (`#113`)!
      3. **Esgoto e Ventilação (NBR 8160:1999)**: Tabelas 1–8 literais em JSON (ramais, tubos de queda, subcoletores por declividade 0,5%–4%, ramais/colunas de ventilação e distância máxima desconector–ventilação).
      4. **Águas Pluviais (NBR 10844:1989)**: $Q = I \cdot A / 60$ integrado à área real do telhado (`#114`) + `Site & Climate`, ábacos Fig. 3a/3b (condutores verticais), Manning a 2/3 da seção (calhas e horizontais) e **alerta automático de telhado drenando sobre piscina ou vizinho (`#122`)**!
      5. **Hidrantes (CBPMESP IT-22)**: Hazen-Williams, Tabela 2/3, 2 jatos, RTI e velocidade máxima.
    - **Diagrama Isométrico Hidráulico (`#123`)**: projeção isométrica automática da rede 3D de banheiros/cozinhas para prancha PDF/SVG.
  - **Critério de pronto:** traçar a rede de um banheiro e telhado no Trazo valida pressões, diâmetros, declividades, ventilação e calhas no semáforo, gera isométrico e lista de tubos/conexões.

- [ ] **T-40 · Elétrica Avançada (NBR 5410 Completa + Eletrocalhas 3D + Quadro Editável)** *(Tamanho: M · Evolui M7 `#124`–`#127`)*
  - **Ref. BIMCraft:** `crates/core/src/{nbr5410.rs, bath.rs, dr.rs, dps.rs, carrier_fill.rs, panel_schedule.rs}`.
  - **O que implementar:**
    - Integrar ao módulo elétrico atual do Trazo os motores orientados a dados da NBR 5410:
      - **Volumes de Banheiro (§9.1 — Volumes 0/1/2/3)**: verificação espacial automática das tomadas/interruptores/luminárias em relação ao box/banheira/chuveiro (grau IP, SELV, distância mínima)!
      - **Auditoria Completa de DR ($\le 30\text{ mA}$, §5.1.3.2.2)** e **Dimensionamento de DPS (§5.4.2 / §6.3.5.2, Fig. 13, Tab. 31 & 49)** usando o índice ceráunico de `Site & Climate`.
      - **Tabelas 36, 37, 40, 41, 42, 45, 47, 48 e 58 completas** (fatores de correção de temperatura, solo e agrupamento em feixe/camada + seção de neutro e PE + catálogos reais de eletrodutos/cabos).
      - **Eletrocalhas 3D (`cabletray`) + `carrier_fill`** (taxa de ocupação em eletrocalhas/leitos) e **Quadro de Cargas Editável estilo Revit** (alocação de slots/reservas/espaços, colocável diretamente na prancha).
  - **Critério de pronto:** colocar uma tomada dentro do Volume 1 ou 2 do banheiro acende alerta vermelho NBR 5410 §9.1 no semáforo; DPS e fatores de agrupamento entram automaticamente no dimensionamento e memorial.

- [ ] **T-41 · Disciplina SPDA — Proteção contra Descargas Atmosféricas (NBR 5419)** *(Tamanho: M · Atende `#129`)*
  - **Ref. BIMCraft:** `crates/core/src/runs.rs` (`downconductor`, `groundconductor`), `README.md` (SPDA).
  - **O que implementar:**
    - Nova seção/aba **SPDA (NBR 5419)** usando `Run` + famílias paramétricas: captores Franklin (`Air Terminal`), malha de captação no telhado, condutores de descida verticais (`Down Conductor`), anel/malha de aterramento enterrado (`Ground Conductor`), hastes de aterramento (`Earthing Electrode`) e caixas de inspeção (`Inspection Box`).
    - Adicionar o que falta no próprio BIMCraft: **cálculo preliminar NBR 5419** (triagem de gerenciamento de risco NBR 5419-2 usando `Site & Climate`, nível de proteção NP I–IV, raio da esfera rolante / ângulo Franklin / espaçamento da malha de Faraday e distância de segurança `s`), tudo com semáforo, memorial PDF, quantitativos e MCP.
  - **Critério de pronto:** cobertura do projeto exibe zona de proteção Franklin/Faraday, descidas espaçadas conforme NP e anel de aterramento no 2D/3D e no IFC.

- [ ] **T-42 · Disciplina Dados, Voz e Cabeamento Estruturado (NBR 14565)** *(Tamanho: S)*
  - **Ref. BIMCraft:** `crates/core/src/runs.rs` (`cable`, `cabletray`), `README.md` (Data).
  - **O que implementar:**
    - Ferramentas de **Dados/Lógica (NBR 14565)**: tomadas RJ45 dados/voz, racks 19", patch panels, eletrocalhas compartilhadas e cabeamento Cat6/Cat6A/fibra óptica sobre `Run`.
    - Verificação automática de **comprimento máximo do enlace permanente (90 m) e canal (100 m)**, contagem de portas por patch panel e lista de compras de cabos/tomadas.
  - **Critério de pronto:** pontos de dados roteados até o rack calculam comprimento real do cabo, alertam acima de 90 m e entram no quadro/quantitativos.

- [ ] **T-43 · Classificação BIM (ABNT NBR 15965, Uniclass, OmniClass, MasterFormat, COBie 2.4) + IA Assistida** *(Tamanho: M · Ponte direta para Orçamento SINAPI `#135`/`#136`)*
  - **Ref. BIMCraft:** `crates/core/src/classification.rs`.
  - **O que implementar:**
    - Gerenciador de Classificação suportando **ABNT NBR 15965** (todas as 13 tabelas `0M..5I`), Uniclass 2015, OmniClass, UniFormat, MasterFormat, COBie 2.4 e tabelas **SINAPI / TCPO** importadas pelo usuário via `.csv`/`.xlsx` (sem embutir tabelas proprietárias de terceiros).
    - Parâmetros determinísticos por GUID, validação de completude (`classification_check`), **proposta assistida por IA** (via assistente embutido ou MCP com revisão humana obrigatória item a item) e exportação como `IfcClassificationReference`.
  - **Critério de pronto:** importar uma tabela CSV da NBR 15965 ou SINAPI permite classificar todos os tipos do projeto (manual ou via IA) e cruzar direto com quantitativos/orçamento.

---

### FASE 7 — Interoperabilidade Profissional BIM/CAD (`#86`)

- [ ] **T-44 · Importação e Exportação IFC Completa (IFC4, IFC4.3, IFC2x3) + Validação IfcOpenShell no CI** *(Tamanho: XL · Atende `#86`)*
  - **Ref. BIMCraft:** `crates/core/src/{step.rs, ifc.rs, ifc/classes.rs, ifc/v2x3.rs, import.rs, survey.rs}`, `ci/validate_ifc.py`.
  - **O que implementar:**
    - Parser STEP ISO 10303-21 puro em Rust com limite de profundidade e `MemoryBudget`.
    - **Exportador IFC4 nativo** + adaptador IFC4.3 + **downgrader automático para IFC2x3** (cobrindo paredes compostas, aberturas, lajes, telhados, escadas, rampas, espaços `Qto_SpaceBaseQuantities`, estrutura com `LoadBearing` e `IfcReinforcingBar`, `IfcSystem` de elétrica/hidráulica/SPDA/dados, `IfcClassificationReference`, `Pset_*Common.Status` de fases e `IfcMapConversion` de georreferenciamento).
    - **Importador IFC2x3/IFC4/IFC4.3** baseado na geometria real (extrusões, varreduras, revoluções, Brep avançado, `IfcMappedItem`, estilos/cores, Psets de Eberick/TQS/Revit preservados para round-trip).
    - Comando diagnóstico anônimo **`ifc_survey`** (estatísticas sem vazar nomes/coordenadas) e **job no CI com IfcOpenShell** validando `express_rules=True` e criação de geometria de 100% dos produtos exportados!
  - **Critério de pronto:** exportar *Casa Térrea*, *Sobrado* e *Área Gourmet* para IFC4/IFC2x3 passa validação rigorosa no IfcOpenShell no CI e reimporta no Trazo sem perda dos elementos suportados.

- [ ] **T-45 · Leitura e Escrita Nativa de DWG (2000–2018) em Subprocesso Isolado** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/dwg/src/lib.rs` (`acadrust`, MPL-2.0 isolada atrás de feature flag).
  - **O que implementar:**
    - Isolar `acadrust` em um módulo/crate opcional (`trazo-dwg`, feature default `dwg`, mantendo o núcleo 100% MIT/Apache-2.0) executando a conversão DWG ↔ DXF em **subprocesso filho** (`--dwg-to-dxf` / `--dxf-to-dwg`) com timeout e limite de 128 MB — um DWG corrompido nunca derruba o Trazo!
    - Evoluir também o DXF/DWG export atual com **Export Setups** (tabela de camadas AIA/ISO 13567/ABNT, hachuras reais, Paper Space com viewports e `ACAD_TABLE` / `$DWGCODEPAGE` via `encoding_rs`).
  - **Critério de pronto:** abrir e salvar `.dwg` (2000–2018) funciona direto no menu *Arquivo* e no MCP sem depender de conversores externos (ODA/LibreCAD).

- [ ] **T-46 · Importação Nativa Read-Only de Revit (`.rvt` / `.rfa`) atrás de Feature Opcional** *(Tamanho: XL · Opcional/Estratégico)*
  - **Ref. BIMCraft:** `crates/rvt/`, `docs/rvt-provenance.md`.
  - **O que implementar:**
    - Leitor read-only opcional (atrás de feature flag e isolamento de licença conforme `rvt-provenance.md`, usando `cfb` + `flate2` + `memchr`) capaz de inspecionar (`info`) e importar níveis, paredes, pisos, forros, telhados, aberturas, pilares, vigas, fundações, tubos e ambientes de arquivos `.rvt` (2015–2027), rodando em thread separada com **Relatório Honesto de Importação** listando o que entrou e o que ficou de fora por motivo.
  - **Critério de pronto:** abrir um `.rvt` de arquitetura no Trazo gera um novo projeto `.trazo` pronto para lançar estrutura/elétrica/hidráulica e exibe o relatório completo de elementos omitidos.

- [ ] **T-47 · Nuvens de Pontos (PTS, XYZ, PLY, LAS 1.0–1.4, ASTM E57) em Rust Puro** *(Tamanho: L)*
  - **Ref. BIMCraft:** `crates/core/src/cloud.rs`, `cloud/e57.rs`, `ui-egui/src/clouds.rs`.
  - **O que implementar:**
    - Leitores 100% Rust puro (zero deps C++) para PTS, XYZ, PLY, LAS e E57 com leitura em 2 passadas + *voxel thinning* determinístico (até 5M pontos), multi-estação, armazenamento compacto em `f32` relativo à origem, renderização 3D (GPU + CPU com oclusão mútua e Section Box), exibição em plantas/cortes (modo *Alto Contraste*) e **snapping 2D na faixa de corte** em microssegundos (grid espacial 250 mm).
  - **Por que vale ouro no Brasil:** reformas e *as-built* feitos com escaneamento LiDAR (iPhone/iPad/Leica) permitem traçar paredes exatamente sobre a nuvem de pontos!
  - **Critério de pronto:** importar arquivo E57/PLY/LAS exibe os pontos em planta e 3D e permite desenhar paredes com snap nos cantos escaneados.

- [ ] **T-48 · Malhas 3D (`glTF`/`OBJ`/`STL`), Coordenação `BCF 2.1` + Clash Detection, PDF Vetorial (`hayro`) e Manage Links / `.trazoa`** *(Tamanho: L)*
  - **Ref. BIMCraft:** `crates/core/src/{gltf.rs, mesh_in.rs, pdf_in.rs, link/manage.rs, assets.rs, bima.rs}`, `export/formats/coordination.rs`.
  - **O que implementar:**
    - Exportação **glTF 2.0 (`.glb`)**, **OBJ** e **STL** + importação de `.glb`/`.obj` como famílias genéricas recarregáveis.
    - **Underlay Vetorial de PDF (`hayro`)**: importar páginas de PDF (ex.: planta de prefeitura/levantamento) como vetores nítidos escaláveis em plantas (não só bitmap borrado).
    - **Manage Links** unificado (abas Projetos `.trazo`, IFC, CAD, PDF, Imagens, Nuvens, Malhas 3D com status, caminho relativo, *Unload* persistido e *Reload From* atômico) + **Asset Manager & pacote `.trazoa`** (ZIP com fechamento transitivo de dependências de materiais/hachuras/estilos/texturas para compartilhar padrão de escritório).
    - **Detecção de Colisões (*Clash Detection*) entre disciplinas** (ex.: viga × tubo de esgoto, pilar × janela) exportada como **BCF 2.1 (`.bcfzip`)** com câmera 3D, snapshot e GUIDs dos elementos conflitantes.
  - **Critério de pronto:** detectar colisões Estrutura × Hidráulica exporta `.bcfzip` abrível no BIMcollab/Solibri/BIMCraft; Manage Links recarrega referências sem perder posições.

---

### FASE 8 — Assistente IA Embutido, Automação, Extensibilidade e Colaboração (`#48`, `#75`, `#141`)

- [ ] **T-49 · Assistente de IA Embutido no Desktop (BYOK Multi-Provedor + Segurança Criptográfica)** *(Tamanho: L · Atende `#48` e `#142`)*
  - **Ref. BIMCraft:** `crates/assistant/src/{chat, tools, anthropic, openai, gemini, net, seal, attach, saved}.rs`.
  - **O que implementar:**
    - Painel lateral de chat IA nativo no Trazo usando a própria chave do usuário (BYOK) para **Anthropic Claude**, **OpenAI (Responses API)** e **Google Gemini** via `ureq` (`rustls`), sem conta obrigatória nem telemetria.
    - Armazenamento seguro de chaves com **`ring` (AES-256-GCM + PBKDF2-HMAC-SHA256 600.000 iterações)** protegido por passphrase opcional, whitelist estrita dos 3 domínios HTTPS oficiais, bloqueio de ferramentas que escrevem no disco (`touches_files`), defesa contra *prompt injection*, anexos de imagem/texto, bloco recolhível de *Thinking*, checklist visual (`todo_write`) e **1 único passo de Undo por turno da IA**.
    - Ação rápida **"Explicar e corrigir com IA" (`#142`)** diretamente em cada cartão vermelho/amarelo do semáforo *Segurança*!
  - **Critério de pronto:** o usuário clica em um aviso vermelho no *Segurança*, o assistente embutido explica a norma em português claro, propõe e executa a correção em 1 passo desfazível com `Ctrl+Z`.

- [ ] **T-50 · Hardening de MCP (`--read-only`, `dry_run`, `touches_files`), `trazo-cli` Headless e Validação Cruzada** *(Tamanho: M · Atende `#75`)*
  - **Ref. BIMCraft:** `apps/bimcraft-cli/`, `crates/core/src/crosscheck.rs`.
  - **O que implementar:**
    - Adicionar interruptor **Modo Somente Leitura (`#75`)** em *Conectar uma IA* e flag `--read-only` no MCP, mais `dry_run: true` no `executar_lote` (simula todas as alterações e devolve o novo semáforo/quantitativos sem gravar no projeto — base para o Modo Aprovação `#46`!).
    - Classificar todas as ferramentas MCP com metadado `Files::{None, Read, Write, Conditional}` testado contra regressão.
    - Criar binário **`trazo-cli`** headless com subcomandos `run` (scripts com variáveis `@var`), `info`, `convert` (ex.: `.trazo` → `.ifc`/`.pdf`/`.dxf`/`.xlsx`), `check` (roda todo o semáforo estrutural/elétrico/hidráulico e retorna exit code para pipelines de CI!) e `mcp [--read-only | --connect]`.
  - **Critério de pronto:** `trazo-cli check examples/area-gourmet.json` valida toda a engenharia sem abrir janela gráfica; `--read-only` bloqueia 100% das mutações.

- [ ] **T-51 · Sistema de Plugins WASM em Sandbox (`wasmi`, ABI 1 & 2)** *(Tamanho: L · Atende `#141`)*
  - **Ref. BIMCraft:** `crates/plugin/`, `crates/plugin-sdk/`, `ui-egui/src/{plugins, plugin_form, plugin_panels}.rs`.
  - **O que implementar:**
    - Runtime WASM em Rust puro (`wasmi`) com sandbox hermética (importa apenas `trazo.execute`, `trazo.log`, `trazo.info`; zero WASI/rede/filesystem), limites de combustível (`fuel`), RAM (64 MB), tempo (5s) e máximo de comandos por chamada, com rollback transacional automático em falha.
    - Permissões explícitas `files` aprovadas pelo usuário em *Gerenciar Plugins*.
    - **UI Declarativa**: formulários auto-gerados a partir do JSON Schema dos parâmetros (com widgets `x-trazo`: nível, comprimento, seleção, material), relatórios em Markdown GFM seguro e **painéis dockáveis declarativos** (ABI 2: tabelas, botões, campos, hooks `on_open`/`refresh`).
    - Publicar `trazo-plugin-sdk` + plugin de exemplo compilado no CI.
  - **Critério de pronto:** um plugin WASM externo adiciona um botão na faixa, abre formulário/painel, executa comandos no modelo em 1 passo de undo e falha seguramente se tentar loop infinito ou estourar memória.

- [ ] **T-52 · Python Addins (*PyAddins* via AST + JSON-lines + Skill IA)** *(Tamanho: M)*
  - **Ref. BIMCraft:** `crates/pyaddins/`, `skills/bimcraft-pyaddins/SKILL.md`.
  - **O que implementar:**
    - Executor de scripts Python (≥ 3.9 do sistema do usuário, zero embutimento de Python nem PyO3; projetos `.trazo` nunca embutem código!) comunicando via JSON-lines com wrapper `trazo.py` (stdlib pura, `with trazo.batch():` em 1 passo de undo).
    - Inspeção estática via **AST do Python** da assinatura `def main(...)` (sem executar o script!) gerando formulários automáticos no *Automação Python* e botões na faixa *PyAddins* (ideal para engenheiros brasileiros criarem planilhas customizadas com `pandas`/`openpyxl`, rotinas de orçamento SINAPI ou verificações próprias).
    - Ferramentas MCP `pyaddin_*` + instalação de `SKILL.md` em 1 clique.
  - **Critério de pronto:** usuário salva um script `.py` na pasta `pyaddins/`, o Trazo monta o formulário pelos type hints e executa alterações atômicas desfazíveis com `Ctrl+Z`.

- [ ] **T-53 · Co-Work — Edição Colaborativa ao Vivo em Rede Local / IP (Sem Servidor Central)** *(Tamanho: XL)*
  - **Ref. BIMCraft:** `crates/cowork/`, `crates/core/src/cowork.rs`, `COWORK-PLAN.md`.
  - **O que implementar:**
    - Arquitetura estrela **Host-Autoritativo** (sem CRDT: preserva 100% das invariantes de cálculo e rollback do Trazo): até 8 convidados conectam à janela do Host via código Base32 de 80 bits.
    - Transporte TCP criptografado com **Noise `NNpsk0_25519_ChaChaPoly_SHA256`** sobre `ring` (zero deps extras), descoberta LAN via UDP com tag HMAC + *Ask to Join* com verificação SAS de 6 dígitos (anti-MITM).
    - Sincronização por **deltas incrementais**, **Undo seletivo por autor** (`undo_own` recusando conflitos com destaque visual), presença em tempo real a 15 Hz (cursores 2D, câmeras 3D, seleções coloridas, *soft-lock* no inspetor, modo *Seguir*), handover de Host e **chat de IA compartilhado onde cada chave de API nunca sai da máquina do dono**.
  - **Critério de pronto:** duas instâncias do Trazo (ex.: arquiteto + engenheiro estrutural na mesma obra) editam simultaneamente em loopback/LAN, veem cursores/semáforo em tempo real e desfazem apenas suas próprias ações.

---

### FASE 9 — Internacionalização, Auto-Update Criptográfico e Empacotamento Multiplataforma

- [ ] **T-54 · Internacionalização Estática (`.po`) em Dois Domínios (Interface vs. Projeto)** *(Tamanho: M · Atende `#68`)*
  - **Ref. BIMCraft:** `crates/i18n/`, `translations/{ui, project}/*.po`.
  - **O que implementar:**
    - Criar extrator baseado em AST (`cargo i18n extract | update | check | stats`) compilando arquivos gettext `.po` em tabelas ordenadas estáticas no `build.rs` (busca binária $O(\log n)$, zero overhead em runtime, zero `libintl`).
    - Separar **Idioma da Interface** (padrão `pt-BR`, adicionar `en`, `es`...) de **Idioma do Projeto** (controla textos impressos em pranchas, `SOBE`/`UP`, carimbos e títulos de tabelas), com validação automática de placeholders `{}` no CI.
  - **Critério de pronto:** alternar a interface para Inglês mantendo um projeto brasileiro imprimindo em Português funciona instantaneamente, validado por `cargo i18n check` no CI.

- [ ] **T-55 · Auto-Update Criptográfico Multiplataforma** *(Tamanho: M · Evolui `#63`)*
  - **Ref. BIMCraft:** `crates/update/src/{updater, net, install, appimage, macos}.rs`.
  - **O que implementar:**
    - Endurecer `src/io/updates.rs`: nunca confiar em URLs arbitrárias do JSON de release (reconstruir a URL localmente a partir da tag semver), exigir verificação criptográfica dupla (**digest da API GitHub + `SHA256SUMS`** via `ring`) e teto de tamanho.
    - Implementar instalação assistida limpa por plataforma: Windows (MSI/instalador com verificação de caminho seguro anti-escalação e reabertura do projeto), Linux AppImage (validação `--version` + swap atômico) e macOS DMG (`codesign --verify --strict` + swap com rollback).
  - **Critério de pronto:** suíte de testes simulados valida download, verificação SHA-256, rejeição de hash adulterado e rollback em falha.

- [ ] **T-56 · Empacotamento Multiplataforma Completo e Hardening de CI** *(Tamanho: M · Atende `#78` e `#97`)*
  - **Ref. BIMCraft:** `packaging/{windows, linux, macos}/`, `.github/workflows/{ci.yml, release.yml}`, `ci/fast-build.toml`.
  - **O que implementar:**
    - Windows: evoluir instalador (Inno Setup / WiX MSI com teste automatizado de instalação, upgrade e desinstalação no CI via PowerShell + `+crt-static` verificado sem dependência de `VCRUNTIME140` + assinatura de código `#78`) + ZIP portátil.
    - Linux: gerar `.deb`, `.rpm` (`nfpm`), `AppImage`, `Flatpak` (testado em sandbox), pacote Arch e `.tar.gz`, com tipos MIME registrados (`.trazo`, `.trazot`, `.trazof`, `.trazoa`) e opção de repositório APT assinado via GitHub Pages.
    - macOS: `.dmg` Universal (`aarch64` + `x86_64` via `lipo`) assinado e notarizado.
    - CI: adicionar `ci/fast-build.toml` (acelera builds em Windows/macOS removendo debug info de crates do workspace), `shellcheck` + `actionlint` (pinado por SHA-256), job IfcOpenShell (da `T-44`) e validação de inclusão de `NOTICE`/licenças das fontes embarcadas.
  - **Critério de pronto:** uma tag `v*` gera, valida e publica automaticamente todos os instaladores (Win/Linux/macOS) com `SHA256SUMS` e smoke-tests verdes.

---

## 4. Resumo Executivo de Priorização (Para Começar Amanhã)

Se o objetivo for maximizar impacto rápido mantendo a base sólida:

1. **Curto prazo (alavancagem imediata no produto atual):** `T-01` → `T-02` → `T-03` → `T-08` → `T-09` → `T-10` → `T-11` → `T-12` → `T-16` → `T-18` → `T-20`
   *(blinda o código, coloca GUIDs na base, entrega Códigos de Obra NBR 9050/9077/municipais + 9 concessionárias elétricas, cotas editáveis, reformas com Fases Existente/Demolir/Construir, Excel `.xlsx` e PDF/A).*
2. **Médio prazo (salto para plataforma BIM + MEP brasileira completa):** `T-04` → `T-05` → `T-13`–`T-15` → `T-24`–`T-25` → `T-30` → `T-31`–`T-34` → `T-36` → `T-38` → `T-39` → `T-40` → `T-41` → `T-42` → `T-43`
   *(entrega 3D nativo GPU `wgpu` com Section Box, Hidrossanitário NBR 5626/8160/10844/IT-22, Elétrica NBR 5410 completa com volumes de banheiro/DPS, SPDA NBR 5419, Dados NBR 14565, Plano de Corte de Aço + `RebarDetail` e Classificação NBR 15965/SINAPI).*
3. **Longo prazo (interoperabilidade global e ecossistema):** `T-22`–`T-23` → `T-27` → `T-29` → `T-44`–`T-48` → `T-49`–`T-53` → `T-54`–`T-56`
   *(IFC4/4.3/2x3 validado no CI, DWG isolado, `.rvt` read-only, Nuvem de Pontos E57/LAS, Assistente IA embutido, Plugins WASM, PyAddins, Co-Work LAN, i18n e empacotamento completo).*
