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
usuário pedir.";

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
        return Some(error_response(Value::Null, -32600, "Mensagem JSON-RPC inválida"));
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
            description: "Cria um cômodo retangular com suas quatro paredes. Paredes que coincidem com as de cômodos vizinhos são compartilhadas. Devolve o id do cômodo e de suas paredes.",
            properties: json!({
                "nome": string("Nome do cômodo, por exemplo Sala, Cozinha, Suíte."),
                "x": number("X do canto inferior esquerdo, em metros."),
                "y": number("Y do canto inferior esquerdo, em metros."),
                "largura": number("Largura em metros (eixo X), mínimo 0,5."),
                "comprimento": number("Comprimento em metros (eixo Y), mínimo 0,5."),
            }),
            required: &["nome", "x", "y", "largura", "comprimento"],
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
            description: "Muda a largura e/ou a posição de uma porta ou janela ao longo da sua parede.",
            properties: json!({
                "id": id_of("da porta ou janela"),
                "largura": number("Opcional: nova largura em metros."),
                "posicao": number("Opcional: novo centro ao longo da parede, de 0 a 1."),
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
        assert_eq!(tools.len(), 20);
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
        for tool in &tools {
            let name = tool["name"].as_str().unwrap();
            assert!(name.chars().all(|c| c.is_ascii_lowercase() || c == '_'), "{name}");
            assert_eq!(tool["inputSchema"]["type"], "object");
            for required in tool["inputSchema"]["required"].as_array().unwrap() {
                let key = required.as_str().unwrap();
                assert!(tool["inputSchema"]["properties"].get(key).is_some(), "{name}.{key}");
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
        assert_eq!(handle_message(&unknown, &mut run).unwrap()["error"]["code"], -32602);
        assert_eq!(seen, vec![("criar_comodo".to_owned(), json!({ "nome": "Sala" }))]);
    }

    #[test]
    fn notifications_batches_and_bad_json_are_handled() {
        let mut run = no_tools;
        assert_eq!(
            handle_text(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, &mut run),
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
        let listed = handle_message(&json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }), &mut run).unwrap();
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 20);
    }
}
