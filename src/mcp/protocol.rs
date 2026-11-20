//! JSON-RPC 2.0 messages of the Model Context Protocol and the catalogue of
//! Trazo's tools. Transport-agnostic: the HTTP server and the stdio bridge
//! both hand their text here.

use serde_json::{Value, json};

pub const SERVER_NAME: &str = "trazo";

/// Protocol revisions this server speaks; it only uses tools, which every
/// revision shares, so it can answer in whichever the client asks for.
const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const FALLBACK_VERSION: &str = "2025-06-18";

const INSTRUCTIONS: &str = "\
O Trazo é um editor de plantas baixas aberto na tela do usuário. Tudo o que você fizer \
aparece ao vivo e pode ser desfeito (ferramenta desfazer ou Ctrl+Z no Trazo).
Unidades em metros. Coordenadas: X para a direita e Y para cima, as mesmas da barra de \
status do Trazo.
Comece com obter_planta para conhecer os elementos e seus ids. Para criar uma planta: \
definir_terreno, depois criar_comodo para cada cômodo (cômodos vizinhos que encostam \
lado a lado compartilham a parede automaticamente), inserir_abertura com o id da parede \
(posicao de 0 a 1 ao longo da parede) e inserir_objeto para bancadas, pias, fogões, \
churrasqueira, louças e mesas. As paredes têm espessura (espessura_parede_m): as \
coordenadas de cômodos são o eixo das paredes, e a face interna fica meia espessura para \
dentro. Paredes e cômodos precisam ficar dentro \
do terreno. Confira o resultado com imagem_da_planta. Salve com salvar_projeto quando o \
usuário pedir.
Para editar como num CAD: transformar (mover, girar, espelhar, copiar vários elementos), \
alinhar, distribuir, agrupar, repetir, editar_geometria (aparar, estender, deslocar, \
dividir), unir e restringir (paralelas, perpendiculares, distâncias fixas que o Trazo \
mantém nas edições seguintes). Use selecionar para mostrar ao usuário os elementos de que \
está falando; 'selecao' em obter_planta diz o que ele selecionou.";

/// Runs a tool by name with its arguments and returns the `CallToolResult`.
pub type ToolRunner<'a> = dyn FnMut(&str, &Value) -> Value + 'a;

/// Answers one transport message: a single JSON-RPC message or a batch.
/// Returns `None` when there is nothing to send back (only notifications).
pub fn handle_text(text: &str, run: &mut ToolRunner) -> Option<String> {
    let parsed: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => {
            return Some(
                error_response(Value::Null, -32700, &format!("JSON inválido: {error}")).to_string(),
            );
        }
    };
    match parsed {
        Value::Array(messages) if messages.is_empty() => {
            Some(error_response(Value::Null, -32600, "Lote vazio").to_string())
        }
        Value::Array(messages) => {
            let responses: Vec<Value> = messages
                .iter()
                .filter_map(|message| handle_message(message, run))
                .collect();
            (!responses.is_empty()).then(|| Value::Array(responses).to_string())
        }
        message => handle_message(&message, run).map(|response| response.to_string()),
    }
}

/// Answers one JSON-RPC message; `None` for notifications and responses.
pub fn handle_message(message: &Value, run: &mut ToolRunner) -> Option<Value> {
    let Some(object) = message.as_object() else {
        return Some(error_response(
            Value::Null,
            -32600,
            "Mensagem JSON-RPC inválida",
        ));
    };
    let id = object.get("id").cloned().filter(|id| !id.is_null());
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        // A response to a request we never send, or a malformed message.
        let is_response = object.contains_key("result") || object.contains_key("error");
        return id
            .filter(|_| !is_response)
            .map(|id| error_response(id, -32600, "Falta o método"));
    };
    // Notifications (no id) such as notifications/initialized need no answer.
    let id = id?;
    let params = object.get("params").cloned().unwrap_or_else(|| json!({}));
    let outcome = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" | "logging/setLevel" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool(&params, run),
        // Not offered, but answered so that clients probing for them carry on.
        "resources/list" => Ok(json!({ "resources": [] })),
        "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        _ => Err((-32601, format!("Método desconhecido: {method}"))),
    };
    Some(match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => error_response(id, code, &message),
    })
}

pub fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

fn initialize(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(FALLBACK_VERSION);
    let version = if PROTOCOL_VERSIONS.contains(&requested) {
        requested
    } else {
        FALLBACK_VERSION
    };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": SERVER_NAME,
            "title": "Trazo — plantas baixas",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": INSTRUCTIONS,
    })
}

fn call_tool(params: &Value, run: &mut ToolRunner) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "Falta o nome da ferramenta".to_owned()))?;
    if !tool_names().contains(&name) {
        return Err((-32602, format!("Ferramenta desconhecida: {name}")));
    }
    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => json!({}),
        Some(arguments) if arguments.is_object() => arguments.clone(),
        Some(_) => return Err((-32602, "Os argumentos devem ser um objeto JSON".to_owned())),
    };
    Ok(run(name, &arguments))
}

fn number(description: &str) -> Value {
    json!({ "type": "number", "description": description })
}

fn integer(description: &str) -> Value {
    json!({ "type": "integer", "description": description })
}

fn string(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn boolean(description: &str) -> Value {
    json!({ "type": "boolean", "description": description })
}

fn ids() -> Value {
    json!({
        "type": "array",
        "items": { "type": "integer" },
        "description": "Ids dos elementos (veja obter_planta).",
    })
}

struct Tool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    properties: Value,
    required: &'static [&'static str],
    read_only: bool,
    destructive: bool,
}

impl Tool {
    fn definition(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": {
                "type": "object",
                "properties": self.properties,
                "required": self.required,
            },
            "annotations": {
                "title": self.title,
                "readOnlyHint": self.read_only,
                "destructiveHint": self.destructive,
                "openWorldHint": false,
            },
        })
    }
}

fn catalogue() -> Vec<Tool> {
    let id_of = |what: &str| integer(&format!("Id {what} (veja obter_planta)."));
    vec![
        Tool {
            name: "obter_planta",
            title: "Obter planta",
            description: "Estado atual da planta: projeto, terreno, cômodos, paredes e aberturas, com ids, coordenadas (metros, Y para cima), medidas e áreas. Use antes de editar.",
            properties: json!({}),
            required: &[],
            read_only: true,
            destructive: false,
        },
        Tool {
            name: "imagem_da_planta",
            title: "Imagem da planta",
            description: "Imagem PNG da planta como na exportação, para conferir visualmente o resultado.",
            properties: json!({
                "tamanho_max": integer("Lado maior da imagem em pixels, de 256 a 2048. Padrão 1024."),
            }),
            required: &[],
            read_only: true,
            destructive: false,
        },
        Tool {
            name: "definir_terreno",
            title: "Definir terreno",
            description: "Cria o terreno ou altera suas medidas. Sem x e y: um terreno existente mantém o canto inferior esquerdo; um novo fica na origem (ou centrado no que já foi desenhado).",
            properties: json!({
                "largura": number("Largura em metros (eixo X), de 0,25 a 10000."),
                "comprimento": number("Comprimento em metros (eixo Y), de 0,25 a 10000."),
                "x": number("Opcional: X do canto inferior esquerdo."),
                "y": number("Opcional: Y do canto inferior esquerdo."),
            }),
            required: &["largura", "comprimento"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "criar_comodo",
            title: "Criar cômodo",
            description: "Cria um cômodo com suas paredes: retangular (x, y, largura, comprimento) ou de qualquer forma, como em L ou U (pontos: os cantos em ordem). Paredes que coincidem com as de cômodos vizinhos são compartilhadas. Devolve o id do cômodo e de suas paredes.",
            properties: json!({
                "nome": string("Nome do cômodo, por exemplo Sala, Cozinha, Suíte."),
                "x": number("Retângulo: X do canto inferior esquerdo, em metros."),
                "y": number("Retângulo: Y do canto inferior esquerdo, em metros."),
                "largura": number("Retângulo: largura em metros (eixo X), mínimo 0,5."),
                "comprimento": number("Retângulo: comprimento em metros (eixo Y), mínimo 0,5."),
                "pontos": {
                    "type": "array",
                    "description": "Forma livre: os cantos em ordem, como [{\"x\": 0, \"y\": 0}, {\"x\": 4, \"y\": 0}, ...]; pelo menos 3, sem lados cruzados. Substitui x, y, largura e comprimento.",
                    "items": {
                        "type": "object",
                        "properties": { "x": { "type": "number" }, "y": { "type": "number" } },
                        "required": ["x", "y"]
                    }
                },
            }),
            required: &["nome"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "criar_parede",
            title: "Criar parede",
            description: "Cria uma parede avulsa (12 cm de espessura) entre dois pontos, com pelo menos 0,25 m.",
            properties: json!({
                "x1": number("X do início."),
                "y1": number("Y do início."),
                "x2": number("X do fim."),
                "y2": number("Y do fim."),
            }),
            required: &["x1", "y1", "x2", "y2"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "inserir_abertura",
            title: "Inserir porta ou janela",
            description: "Insere uma porta (largura padrão 0,90 m, de 0,60 a 2,00), janela (padrão 1,20 m, de 0,40 a 4,00) porta vai-e-vem (padrão 0,80 m, de 0,60 a 1,80) ou vão livre (sem porta, de 0,50 a 30 m, para lados abertos) numa parede. Não pode sobrepor outra abertura nem encostar nas pontas da parede.",
            properties: json!({
                "parede_id": id_of("da parede"),
                "tipo": { "type": "string", "enum": ["porta", "janela", "porta_vai_e_vem", "vao_livre"], "description": "porta, janela, porta_vai_e_vem (tipo faroeste, duas folhas que abrem para os dois lados) ou vao_livre (trecho aberto, sem porta)." },
                "posicao": number("Centro da abertura ao longo da parede, de 0 (início) a 1 (fim). Padrão 0,5 (meio)."),
                "largura": number("Opcional: largura em metros."),
            }),
            required: &["parede_id", "tipo"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "inserir_objeto",
            title: "Inserir objeto",
            description: "Insere um objeto ou equipamento: bancada, pia, fogao, fogao_lenha, churrasqueira, geladeira, lavatorio (com largura >= 0,90 m vira pia dupla), vaso, chuveiro, mesa (com cadeiras) ou prateleira. x e y são o centro. A rotação diz para onde fica o fundo (o lado encostado na parede): 0 = fundo para +Y, 90 = +X, 180 = -Y, 270 = -X. Com encostar_na_parede = true o Trazo vira o fundo para a parede reta mais próxima (até 0,8 m) e encosta na face dos tijolos, descontando meia espessura de parede (veja espessura_parede_m em obter_planta).",
            properties: json!({
                "tipo": {
                    "type": "string",
                    "enum": ["bancada", "pia", "fogao", "fogao_lenha", "churrasqueira", "geladeira", "lavatorio", "vaso", "chuveiro", "mesa", "prateleira"],
                    "description": "Tipo do objeto.",
                },
                "x": number("X do centro, em metros."),
                "y": number("Y do centro, em metros."),
                "rotacao": integer("0, 90, 180 ou 270 graus (para onde fica o fundo). Padrão 0."),
                "largura": number("Opcional: largura ao longo do fundo, em metros (padrão do tipo)."),
                "profundidade": number("Opcional: profundidade, em metros (padrão do tipo)."),
                "encostar_na_parede": boolean("Encosta na face da parede mais próxima, virando o fundo para ela."),
            }),
            required: &["tipo", "x", "y"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "definir_alvenaria",
            title: "Definir alvenaria",
            description: "Escolhe o tijolo ou bloco das paredes: material (um dos prontos, como \"Bloco de concreto 14\" ou \"Tijolo cerâmico 6 furos (em pé)\"), e/ou largura e comprimento do bloco, junta e reboco em cm. A espessura da parede acabada é bloco + 2 × reboco, e o módulo (comprimento + junta) guia os comprimentos. Vale para as novas paredes, ou para todas com aplicar_em_todas. pe_direito (m) serve à estimativa de blocos.",
            properties: json!({
                "material": string("Opcional: nome de um material pronto, ou um nome para o personalizado."),
                "largura_bloco_cm": number("Opcional: largura do bloco em cm (espessura bruta)."),
                "comprimento_bloco_cm": number("Opcional: comprimento do bloco em cm."),
                "junta_cm": number("Opcional: junta de argamassa em cm (padrão 1)."),
                "reboco_cm": number("Opcional: reboco em cada face em cm (padrão 1,5)."),
                "pe_direito": number("Opcional: pé-direito em metros, para a estimativa."),
                "aplicar_em_todas": boolean("Aplica também às paredes já desenhadas."),
            }),
            required: &[],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "editar_objeto",
            title: "Editar objeto",
            description: "Muda posição (centro), rotação, largura ou profundidade de um objeto.",
            properties: json!({
                "id": id_of("do objeto"),
                "x": number("Opcional: novo X do centro."),
                "y": number("Opcional: novo Y do centro."),
                "rotacao": integer("Opcional: 0, 90, 180 ou 270 graus."),
                "largura": number("Opcional: nova largura em metros."),
                "profundidade": number("Opcional: nova profundidade em metros."),
            }),
            required: &["id"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "desenhar",
            title: "Desenhar",
            description: "Desenha uma entidade 2D sem significado arquitetônico (não vira parede nem entra nas áreas): linha (2 pontos), polilinha ou spline (2 ou mais pontos; fechado opcional), retangulo (2 cantos opostos), arco (início, um ponto do arco e fim) ou circulo (centro e raio). Devolve o id.",
            properties: json!({
                "tipo": string("linha, polilinha, retangulo, arco, circulo ou spline."),
                "pontos": {
                    "type": "array",
                    "description": "Os pontos em ordem, como [{\"x\": 0, \"y\": 0}, {\"x\": 3, \"y\": 0}].",
                    "items": {
                        "type": "object",
                        "properties": { "x": { "type": "number" }, "y": { "type": "number" } },
                        "required": ["x", "y"]
                    }
                },
                "centro": {
                    "type": "object",
                    "description": "Círculo: o centro {x, y}.",
                    "properties": { "x": { "type": "number" }, "y": { "type": "number" } }
                },
                "raio": number("Círculo: raio em metros."),
                "fechado": boolean("Polilinha ou spline: liga o último ponto ao primeiro."),
            }),
            required: &["tipo"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "editar_comodo",
            title: "Editar cômodo",
            description: "Renomeia um cômodo e/ou muda sua largura e comprimento, movendo as paredes e mantendo portas e janelas.",
            properties: json!({
                "id": id_of("do cômodo"),
                "nome": string("Opcional: novo nome."),
                "largura": number("Opcional: nova largura em metros."),
                "comprimento": number("Opcional: novo comprimento em metros."),
            }),
            required: &["id"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "editar_parede",
            title: "Editar parede",
            description: "Muda o comprimento de uma parede, na direção dela.",
            properties: json!({
                "id": id_of("da parede"),
                "comprimento": number("Novo comprimento em metros."),
            }),
            required: &["id", "comprimento"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "editar_abertura",
            title: "Editar porta ou janela",
            description: "Muda a largura e/ou a posição de uma porta ou janela ao longo da sua parede, ou inverte o lado para onde a porta abre e o lado da dobradiça.",
            properties: json!({
                "id": id_of("da porta ou janela"),
                "largura": number("Opcional: nova largura em metros."),
                "posicao": number("Opcional: novo centro ao longo da parede, de 0 a 1."),
                "inverter_lado": boolean("Opcional: a porta passa a abrir para o outro lado da parede."),
                "inverter_dobradica": boolean("Opcional: a dobradiça vai para o outro batente."),
                "altura": number("Opcional: altura em metros (porta padrão 2,10; janela 1,00)."),
                "peitoril": number("Opcional: altura do peitoril da janela em metros (padrão 1,10)."),
            }),
            required: &["id"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "mover_elemento",
            title: "Mover elemento",
            description: "Desloca um elemento (parede, cômodo, terreno, porta ou janela). Portas e janelas só deslizam ao longo da parede; cômodos com paredes compartilhadas não se movem sozinhos; nada sai do terreno.",
            properties: json!({
                "id": id_of("do elemento"),
                "dx": number("Deslocamento em X, em metros."),
                "dy": number("Deslocamento em Y (positivo para cima), em metros."),
            }),
            required: &["id", "dx", "dy"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "selecionar",
            title: "Selecionar na tela",
            description: "Seleciona elementos na tela do usuário para mostrar do que você está falando (não altera a planta). Use ids, tipo ou tudo; sem nada, limpa a seleção. obter_planta devolve em 'selecao' o que o usuário selecionou.",
            properties: json!({
                "ids": ids(),
                "tipo": string("comodos, paredes, aberturas, objetos, desenhos ou terreno."),
                "tudo": boolean("Seleciona todos os elementos visíveis."),
                "aproximar": boolean("Aproxima a vista da seleção."),
            }),
            required: &[],
            read_only: true,
            destructive: false,
        },
        Tool {
            name: "transformar",
            title: "Mover, girar, espelhar ou copiar",
            description: "Move, gira ou espelha vários elementos juntos (cômodos levam paredes, portas e janelas), ou cria cópias assim com copiar=true. Girar e espelhar usam o centro dos elementos, a menos que centro_x/centro_y ou um eixo x1,y1–x2,y2 sejam dados. Objetos só giram em múltiplos de 90°.",
            properties: json!({
                "ids": ids(),
                "operacao": string("mover, girar ou espelhar."),
                "dx": number("Mover: deslocamento em X (metros)."),
                "dy": number("Mover: deslocamento em Y, positivo para cima (metros)."),
                "angulo": number("Girar: graus, positivo no sentido anti-horário."),
                "centro_x": number("Centro do giro ou do espelhamento (X)."),
                "centro_y": number("Centro do giro ou do espelhamento (Y)."),
                "eixo": string("Espelhar: vertical (esquerda↔direita) ou horizontal (cima↔baixo)."),
                "x1": number("Espelhar por um eixo qualquer: X do 1º ponto."),
                "y1": number("Y do 1º ponto do eixo."),
                "x2": number("X do 2º ponto do eixo."),
                "y2": number("Y do 2º ponto do eixo."),
                "copiar": boolean("Mantém os originais e cria cópias transformadas."),
            }),
            required: &["ids", "operacao"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "alinhar",
            title: "Alinhar elementos",
            description: "Alinha dois ou mais elementos pela borda ou pelo centro do conjunto. Cômodos com paredes em comum se movem como um bloco.",
            properties: json!({
                "ids": ids(),
                "alinhamento": string("esquerda, direita, centro_horizontal, topo, base ou centro_vertical."),
            }),
            required: &["ids", "alinhamento"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "distribuir",
            title: "Distribuir elementos",
            description: "Deixa espaços iguais entre três ou mais elementos, mantendo o primeiro e o último.",
            properties: json!({
                "ids": ids(),
                "direcao": string("horizontal (padrão) ou vertical."),
            }),
            required: &["ids"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "agrupar",
            title: "Agrupar elementos",
            description: "Agrupa cômodos, paredes, objetos e desenhos: no Trazo um clique passa a selecionar o grupo todo, e cópias mantêm o grupo.",
            properties: json!({
                "ids": ids(),
                "nome": string("Nome do grupo (opcional)."),
            }),
            required: &["ids"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "desagrupar",
            title: "Desagrupar",
            description: "Desfaz os grupos dos elementos indicados (os elementos continuam na planta).",
            properties: json!({ "ids": ids() }),
            required: &["ids"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "repetir",
            title: "Repetir elementos",
            description: "Cria cópias em grade (colunas × linhas, o original é a primeira célula) ou em volta de um centro (modo polar). Até 400 cópias por vez.",
            properties: json!({
                "ids": ids(),
                "modo": string("grade (padrão) ou polar."),
                "colunas": integer("Grade: colunas, para a direita."),
                "linhas": integer("Grade: linhas, para cima."),
                "espacamento_x": number("Grade: distância entre colunas (metros)."),
                "espacamento_y": number("Grade: distância entre linhas (metros)."),
                "quantidade": integer("Polar: itens no total, contando o original."),
                "angulo_total": number("Polar: graus (360 espalha por igual em volta)."),
                "centro_x": number("Polar: X do centro (padrão: centro dos elementos)."),
                "centro_y": number("Polar: Y do centro."),
            }),
            required: &["ids"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "editar_geometria",
            title: "Aparar, estender, deslocar ou dividir",
            description: "Edita uma parede ou um desenho: aparar remove o trecho em x,y até os cruzamentos mais próximos; estender leva a ponta mais próxima de x,y até a próxima parede, desenho ou divisa; deslocar cria uma cópia paralela a 'distancia' metros do lado de x,y; dividir corta em x,y ou em 'partes' iguais. Portas e janelas ficam no lugar.",
            properties: json!({
                "id": id_of("da parede ou do desenho"),
                "operacao": string("aparar, estender, deslocar ou dividir."),
                "x": number("Ponto de referência (X)."),
                "y": number("Ponto de referência (Y)."),
                "distancia": number("Deslocar: distância em metros (paredes: de eixo a eixo)."),
                "partes": integer("Dividir em partes iguais (2 a 100)."),
            }),
            required: &["id", "operacao"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "unir",
            title: "Unir",
            description: "Une paredes alinhadas que se tocam pelas pontas (mesmo material) em uma só, ou linhas, polilinhas abertas e arcos do mesmo círculo em um só desenho.",
            properties: json!({ "ids": ids() }),
            required: &["ids"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "restringir",
            title: "Criar restrição",
            description: "Cria uma restrição que o Trazo mantém em todas as edições seguintes (suas e do usuário): paralelas ou perpendiculares (a e b: paredes ou linhas), coincidentes (ponto_a de a vai até ponto_b de b), distancia entre dois pontos, afastamento entre duas linhas paralelas, ou comprimento fixo de a. O segundo elemento se ajusta ao primeiro. Pontos: 0 = início, 1 = fim.",
            properties: json!({
                "tipo": string("paralelas, perpendiculares, coincidentes, distancia, afastamento ou comprimento."),
                "a": id_of("da parede ou linha de referência"),
                "b": id_of("da parede ou linha que se ajusta"),
                "ponto_a": integer("Ponto de a: 0 início, 1 fim."),
                "ponto_b": integer("Ponto de b: 0 início, 1 fim."),
                "distancia": number("Distância a manter, em metros (padrão: a atual)."),
            }),
            required: &["tipo", "a"],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "remover_restricao",
            title: "Remover restrição",
            description: "Remove uma restrição pelo id (veja 'restricoes' em obter_planta); a geometria fica como está.",
            properties: json!({ "id": id_of("da restrição") }),
            required: &["id"],
            read_only: false,
            destructive: true,
        },
        Tool {
            name: "excluir_elemento",
            title: "Excluir elemento",
            description: "Exclui uma parede, cômodo, porta, janela ou o terreno. Excluir um cômodo remove as paredes que só ele usa.",
            properties: json!({ "id": id_of("do elemento") }),
            required: &["id"],
            read_only: false,
            destructive: true,
        },
        Tool {
            name: "desfazer",
            title: "Desfazer",
            description: "Desfaz as últimas ações (do usuário ou suas).",
            properties: json!({ "passos": integer("Quantas ações desfazer, de 1 a 50. Padrão 1.") }),
            required: &[],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "refazer",
            title: "Refazer",
            description: "Refaz ações desfeitas.",
            properties: json!({ "passos": integer("Quantas ações refazer, de 1 a 50. Padrão 1.") }),
            required: &[],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "centralizar_planta",
            title: "Centralizar planta",
            description: "Centraliza paredes e cômodos dentro do terreno.",
            properties: json!({}),
            required: &[],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "novo_projeto",
            title: "Novo projeto",
            description: "Começa uma planta vazia. Se houver alterações não salvas, falha a menos que descartar_alteracoes seja true.",
            properties: json!({
                "nome": string("Opcional: nome do novo projeto."),
                "descartar_alteracoes": boolean("Descarta alterações não salvas da planta atual."),
            }),
            required: &[],
            read_only: false,
            destructive: true,
        },
        Tool {
            name: "abrir_projeto",
            title: "Abrir projeto",
            description: "Abre um projeto .json do Trazo. Caminho relativo é procurado em Documentos\\Trazo. Se houver alterações não salvas, falha a menos que descartar_alteracoes seja true.",
            properties: json!({
                "caminho": string("Caminho do arquivo .json."),
                "descartar_alteracoes": boolean("Descarta alterações não salvas da planta atual."),
            }),
            required: &["caminho"],
            read_only: false,
            destructive: true,
        },
        Tool {
            name: "salvar_projeto",
            title: "Salvar projeto",
            description: "Salva o projeto. Sem caminho: no arquivo atual ou, se ainda não foi salvo, em Documentos\\Trazo com o nome do projeto. Devolve o caminho usado.",
            properties: json!({
                "caminho": string("Opcional: caminho do arquivo .json (relativo a Documentos\\Trazo)."),
            }),
            required: &[],
            read_only: false,
            destructive: false,
        },
        Tool {
            name: "exportar_planta",
            title: "Exportar planta",
            description: "Exporta a planta em PDF vetorial ou imagem PNG. Sem caminho: em Documentos\\Trazo com o nome do projeto. Devolve o caminho usado.",
            properties: json!({
                "formato": { "type": "string", "enum": ["pdf", "png"], "description": "pdf ou png." },
                "caminho": string("Opcional: caminho do arquivo (relativo a Documentos\\Trazo)."),
            }),
            required: &["formato"],
            read_only: false,
            destructive: false,
        },
    ]
}

/// The tool definitions for `tools/list`.
pub fn tools() -> Vec<Value> {
    catalogue().iter().map(Tool::definition).collect()
}

pub fn tool_names() -> Vec<&'static str> {
    catalogue().iter().map(|tool| tool.name).collect()
}

/// Tools that only read the plan.
pub fn is_read_only(name: &str) -> bool {
    catalogue()
        .iter()
        .any(|tool| tool.name == name && tool.read_only)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_tools(_: &str, _: &Value) -> Value {
        panic!("no tool should run");
    }

    #[test]
    fn initialize_answers_in_the_clients_revision_when_known() {
        let mut run = no_tools;
        for (asked, answered) in [
            ("2025-06-18", "2025-06-18"),
            ("2024-11-05", "2024-11-05"),
            ("2099-01-01", FALLBACK_VERSION),
        ] {
            let request = json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": asked, "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } }
            });
            let response = handle_message(&request, &mut run).unwrap();
            assert_eq!(response["result"]["protocolVersion"], answered);
            assert_eq!(response["result"]["serverInfo"]["name"], "trazo");
            assert!(response["result"]["capabilities"]["tools"].is_object());
        }
    }

    #[test]
    fn tool_catalogue_is_well_formed() {
        let tools = tools();
        let names = tool_names();
        assert_eq!(tools.len(), 22);
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
        for tool in &tools {
            let name = tool["name"].as_str().unwrap();
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{name}"
            );
            assert_eq!(tool["inputSchema"]["type"], "object");
            for required in tool["inputSchema"]["required"].as_array().unwrap() {
                let key = required.as_str().unwrap();
                assert!(
                    tool["inputSchema"]["properties"].get(key).is_some(),
                    "{name}.{key}"
                );
            }
        }
        assert!(is_read_only("obter_planta") && !is_read_only("criar_comodo"));
    }

    #[test]
    fn tool_calls_reach_the_runner_with_their_arguments() {
        let mut seen = Vec::new();
        let mut run = |name: &str, arguments: &Value| {
            seen.push((name.to_owned(), arguments.clone()));
            json!({ "content": [], "isError": false })
        };
        let request = json!({
            "jsonrpc": "2.0", "id": "a", "method": "tools/call",
            "params": { "name": "criar_comodo", "arguments": { "nome": "Sala" } }
        });
        let response = handle_message(&request, &mut run).unwrap();
        assert_eq!(response["id"], "a");
        assert_eq!(response["result"]["isError"], false);

        let unknown = json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "apagar_disco" }
        });
        assert_eq!(
            handle_message(&unknown, &mut run).unwrap()["error"]["code"],
            -32602
        );
        assert_eq!(
            seen,
            vec![("criar_comodo".to_owned(), json!({ "nome": "Sala" }))]
        );
    }

    #[test]
    fn notifications_batches_and_bad_json_are_handled() {
        let mut run = no_tools;
        assert_eq!(
            handle_text(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                &mut run
            ),
            None
        );
        let batch = handle_text(
            r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"}]"#,
            &mut run,
        )
        .unwrap();
        let batch: Value = serde_json::from_str(&batch).unwrap();
        assert_eq!(batch.as_array().unwrap().len(), 1);
        let bad: Value = serde_json::from_str(&handle_text("{oops", &mut run).unwrap()).unwrap();
        assert_eq!(bad["error"]["code"], -32700);
        let unknown = handle_message(
            &json!({ "jsonrpc": "2.0", "id": 9, "method": "sampling/createMessage" }),
            &mut run,
        )
        .unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
        let listed = handle_message(
            &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }),
            &mut run,
        )
        .unwrap();
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 22);
    }
}
