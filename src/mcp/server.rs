//! The MCP endpoint over HTTP (the "streamable HTTP" transport), running
//! inside the Trazo window on 127.0.0.1. Claude Code, ChatGPT (through a
//! tunnel) and other clients connect to `http://127.0.0.1:<port>/mcp`.
//!
//! Requests are answered one at a time on a background thread. Tool calls
//! are handed to the app's UI thread, which runs them on the open project.

use std::io::Read;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tiny_http::{Header, Method, Request, Response, ResponseBox, Server};

use super::{ToolOutput, protocol};

/// A tool call for the app, with where to send its `CallToolResult`.
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
    pub reply: mpsc::Sender<Value>,
    /// After this the caller has given up: the call must not run, or an
    /// assistant that retries would make the change twice.
    pub deadline: Instant,
}

/// How long a tool may wait for the window (a file dialog open in Trazo
/// blocks it, for instance).
const REPLY_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_BODY: u64 = 8 * 1024 * 1024;

/// Starts the server on 127.0.0.1:`port` (0 picks a free port) and returns
/// the port in use. `wake` makes the app look at its queue of calls.
pub fn start(
    port: u16,
    calls: mpsc::Sender<ToolCall>,
    wake: impl Fn() + Send + 'static,
) -> Result<u16, String> {
    let server = Server::http(("127.0.0.1", port))
        .map_err(|error| format!("a porta {port} não está disponível ({error})"))?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|address| address.port())
        .unwrap_or(port);
    std::thread::Builder::new()
        .name("trazo-mcp".to_owned())
        .spawn(move || {
            for request in server.incoming_requests() {
                // One bad request must never stop the server.
                let _ = catch_unwind(AssertUnwindSafe(|| respond(request, &calls, &wake)));
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(port)
}

fn header(name: &'static str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes())
        .unwrap_or_else(|_| Header::from_bytes(name.as_bytes(), &b""[..]).expect("ASCII header"))
}

fn request_header(request: &Request, name: &'static str) -> String {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(name))
        .map(|header| header.value.as_str().to_owned())
        .unwrap_or_default()
}

fn text_response(status: u16, content_type: &str, body: String) -> ResponseBox {
    Response::from_string(body)
        .with_status_code(status)
        .with_header(header("Content-Type", content_type))
        .boxed()
}

fn respond(mut request: Request, calls: &mpsc::Sender<ToolCall>, wake: &dyn Fn()) {
    let accept = request_header(&request, "Accept");
    let response = match request.method() {
        Method::Options => Response::empty(204).boxed(),
        Method::Post => {
            let mut bytes = Vec::new();
            let read = request.as_reader().take(MAX_BODY).read_to_end(&mut bytes);
            if read.is_err() {
                text_response(
                    400,
                    "application/json",
                    protocol::error_response(Value::Null, -32700, "Corpo da requisição ilegível.")
                        .to_string(),
                )
            } else {
                let body = decode_body(bytes);
                let mut run = |name: &str, arguments: &Value| {
                    run_in_app(calls, wake, name, arguments)
                };
                match protocol::handle_text(&body, &mut run) {
                    Some(answer) if wants_event_stream(&accept) => text_response(
                        200,
                        "text/event-stream",
                        format!("event: message\ndata: {answer}\n\n"),
                    ),
                    Some(answer) => text_response(200, "application/json", answer),
                    // Only notifications: accepted, nothing to say.
                    None => Response::empty(202).boxed(),
                }
            }
        }
        // This server sends nothing unprompted, so it opens no event stream.
        Method::Get if accept.contains("text/event-stream") => Response::empty(405)
            .with_header(header("Allow", "POST, OPTIONS"))
            .boxed(),
        Method::Get => text_response(
            200,
            "text/plain; charset=UTF-8",
            "Servidor MCP do Trazo ativo.\n\
             Conecte o assistente a este endereço terminado em /mcp (requisições POST).\n"
                .to_owned(),
        ),
        _ => Response::empty(405)
            .with_header(header("Allow", "POST, GET, OPTIONS"))
            .boxed(),
    };
    let response = response
        .with_header(header("Access-Control-Allow-Origin", "*"))
        .with_header(header(
            "Access-Control-Allow-Methods",
            "GET, POST, DELETE, OPTIONS",
        ))
        .with_header(header("Access-Control-Allow-Headers", "*"))
        .with_header(header("Access-Control-Expose-Headers", "Mcp-Session-Id"))
        .with_header(header("Cache-Control", "no-store"));
    let _ = request.respond(response);
}

/// Bodies should be UTF-8, but command-line tools on Windows often send
/// accented text as Latin-1; that is read as such instead of failing.
fn decode_body(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|error| error.into_bytes().iter().map(|&byte| byte as char).collect())
}

/// Clients send `Accept: application/json, text/event-stream`; one that
/// only takes a stream gets the answer as a single event.
fn wants_event_stream(accept: &str) -> bool {
    accept.contains("text/event-stream")
        && !accept.contains("application/json")
        && !accept.contains("*/*")
}

fn run_in_app(
    calls: &mpsc::Sender<ToolCall>,
    wake: &dyn Fn(),
    name: &str,
    arguments: &Value,
) -> Value {
    let (reply, answer) = mpsc::channel();
    let call = ToolCall {
        name: name.to_owned(),
        arguments: arguments.clone(),
        reply,
        deadline: Instant::now() + REPLY_TIMEOUT,
    };
    if calls.send(call).is_err() {
        return ToolOutput::error("O Trazo está sendo fechado.").to_result();
    }
    wake();
    answer.recv_timeout(REPLY_TIMEOUT).unwrap_or_else(|_| {
        ToolOutput::error(
            "O Trazo não respondeu a tempo. Verifique se não há uma janela de arquivo ou um \
             diálogo aberto no Trazo e tente de novo.",
        )
        .to_result()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use std::net::TcpStream;

    /// Sends a raw HTTP request and returns (status, content type, body).
    fn http(port: u16, method: &str, accept: &str, body: &str) -> (u16, String, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        write!(
            stream,
            "{method} /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: {accept}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").unwrap();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        let content_type = head
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-type:")
                    .map(|value| value.trim().to_owned())
            })
            .unwrap_or_default();
        (status, content_type, body.to_owned())
    }

    /// A server whose "app" answers every tool with the tool's name.
    fn echo_server() -> u16 {
        let (sender, receiver) = mpsc::channel::<ToolCall>();
        std::thread::spawn(move || {
            for call in receiver {
                let _ = call
                    .reply
                    .send(ToolOutput::ok(format!("ran {}", call.name)).to_result());
            }
        });
        start(0, sender, || {}).unwrap()
    }

    #[test]
    fn answers_json_rpc_over_http() {
        let port = echo_server();
        let initialize = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } }
        })
        .to_string();
        let (status, content_type, body) =
            http(port, "POST", "application/json, text/event-stream", &initialize);
        assert_eq!(status, 200);
        assert_eq!(content_type, "application/json");
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["result"]["serverInfo"]["name"], "trazo");

        let call = json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "obter_planta", "arguments": {} }
        })
        .to_string();
        let (_, _, body) = http(port, "POST", "application/json", &call);
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["result"]["content"][0]["text"], "ran obter_planta");
    }

    #[test]
    fn handles_notifications_streams_and_browsers() {
        let port = echo_server();
        let notification = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        assert_eq!(http(port, "POST", "application/json", notification).0, 202);

        let ping = r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#;
        let (status, content_type, body) = http(port, "POST", "text/event-stream", ping);
        assert_eq!(status, 200);
        assert_eq!(content_type, "text/event-stream");
        assert!(body.starts_with("event: message\ndata: {"));

        assert_eq!(http(port, "GET", "text/event-stream", "").0, 405);
        let (status, _, body) = http(port, "GET", "text/html", "");
        assert_eq!(status, 200);
        assert!(body.contains("Servidor MCP do Trazo ativo"));
        assert_eq!(http(port, "OPTIONS", "*/*", "").0, 204);
    }

    #[test]
    fn latin1_bodies_from_windows_tools_are_understood() {
        let utf8 = r#"{"nome":"Suíte"}"#;
        let latin1: Vec<u8> = utf8.chars().map(|c| c as u8).collect();
        assert_ne!(latin1, utf8.as_bytes());
        assert_eq!(decode_body(latin1), utf8);
        assert_eq!(decode_body(utf8.as_bytes().to_vec()), utf8);
    }

    #[test]
    fn a_closed_app_is_reported_instead_of_hanging() {
        let (sender, receiver) = mpsc::channel::<ToolCall>();
        drop(receiver);
        let port = start(0, sender, || {}).unwrap();
        let call = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"obter_planta"}}"#;
        let (_, _, body) = http(port, "POST", "application/json", call);
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["result"]["isError"], true);
    }

    #[test]
    fn a_busy_port_is_reported() {
        let (sender, _receiver) = mpsc::channel::<ToolCall>();
        let port = echo_server();
        assert!(start(port, sender, || {}).is_err());
    }
}
