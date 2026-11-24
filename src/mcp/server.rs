//! The MCP endpoint over HTTP (the "streamable HTTP" transport), running
//! inside the Trazo window on 127.0.0.1. Claude Code, ChatGPT (through a
//! tunnel) and other clients connect to `http://127.0.0.1:<port>/mcp`.
//!
//! Requests are answered one at a time on a background thread. Tool calls
//! are handed to the app's UI thread, which runs them on the open project.

use std::io::{Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde_json::Value;
use tiny_http::{Header, Method, Request, Response, ResponseBox, Server};

use super::events::EventHub;
use super::{ToolOutput, protocol};

/// Shared secret token required on requests arriving through a public tunnel.
/// Local requests (`127.0.0.1` / `localhost`, without proxy headers) continue
/// to work on `/mcp` with no token.
#[derive(Clone, Debug, Default)]
pub struct TunnelToken(Arc<Mutex<Option<String>>>);

impl TunnelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, token: Option<String>) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = token;
        }
    }

    pub fn clear(&self) {
        self.set(None);
    }

    pub fn current(&self) -> Option<String> {
        self.0.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn matches(&self, candidate: &str) -> bool {
        let Ok(guard) = self.0.lock() else {
            return false;
        };
        guard
            .as_deref()
            .is_some_and(|expected| constant_time_eq(expected.as_bytes(), candidate.as_bytes()))
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&a, &b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

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
#[cfg(test)]
pub fn start(
    port: u16,
    calls: mpsc::Sender<ToolCall>,
    wake: impl Fn() + Send + 'static,
) -> Result<u16, String> {
    start_with_token(port, calls, TunnelToken::new(), EventHub::new(), wake)
}

/// Starts the server with a shared `tunnel_token` protecting tunneled
/// traffic; `events` feeds the event streams assistants open.
pub fn start_with_token(
    port: u16,
    calls: mpsc::Sender<ToolCall>,
    tunnel_token: TunnelToken,
    events: EventHub,
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
                // An event stream stays open: it gets a thread of its own.
                if matches!(request.method(), Method::Get)
                    && request_header(&request, "Accept").contains("text/event-stream")
                    && is_authorized(&request, &tunnel_token)
                {
                    let stream = events.listen();
                    let _ = std::thread::Builder::new()
                        .name("trazo-mcp-eventos".to_owned())
                        .spawn(move || stream_events(request, stream));
                    continue;
                }
                // One bad request must never stop the server.
                let _ = catch_unwind(AssertUnwindSafe(|| {
                    respond(request, &calls, &tunnel_token, &wake)
                }));
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(port)
}

/// How often an idle event stream sends a comment, so proxies and clients
/// keep it open.
const KEEPALIVE: Duration = Duration::from_secs(15);

/// Serves one event stream until the client goes away: each message the
/// window publishes is written and flushed at once.
fn stream_events(request: Request, messages: mpsc::Receiver<String>) {
    let mut writer = request.into_writer();
    let head = "HTTP/1.1 200 OK\r\n\
        Content-Type: text/event-stream\r\n\
        Cache-Control: no-store\r\n\
        Connection: keep-alive\r\n\
        Access-Control-Allow-Origin: *\r\n\r\n";
    let mut send = |text: &str| {
        writer
            .write_all(text.as_bytes())
            .and_then(|()| writer.flush())
            .is_ok()
    };
    if !send(head) {
        return;
    }
    loop {
        let chunk = match messages.recv_timeout(KEEPALIVE) {
            Ok(message) => format!("event: message\ndata: {message}\n\n"),
            Err(mpsc::RecvTimeoutError::Timeout) => ": ativo\n\n".to_owned(),
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        // A write error means the client closed the stream.
        if !send(&chunk) {
            return;
        }
    }
}

fn with_header<R: std::io::Read>(
    response: Response<R>,
    name: &'static str,
    value: &str,
) -> Response<R> {
    match Header::from_bytes(name.as_bytes(), value.as_bytes()) {
        Ok(h) => response.with_header(h),
        Err(()) => response,
    }
}

fn with_header_box(mut response: ResponseBox, name: &'static str, value: &str) -> ResponseBox {
    if let Ok(h) = Header::from_bytes(name.as_bytes(), value.as_bytes()) {
        response.add_header(h);
    }
    response
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
    with_header(
        Response::from_string(body).with_status_code(status),
        "Content-Type",
        content_type,
    )
    .boxed()
}

fn is_local_host(host: &str) -> bool {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        return true;
    }
    let name = if let Some(rest) = trimmed.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        trimmed.split(':').next().unwrap_or("")
    };
    name.eq_ignore_ascii_case("localhost") || name == "127.0.0.1" || name == "::1"
}

/// True when the request came from a native client on this computer, not
/// from a web page, Cloudflare tunnel or another reverse proxy.
fn is_local_request(request: &Request) -> bool {
    const PROXY_HEADERS: [&str; 5] = [
        "Cf-Connecting-Ip",
        "Cf-Ray",
        "X-Forwarded-For",
        "X-Forwarded-Host",
        "Forwarded",
    ];
    if PROXY_HEADERS
        .iter()
        .any(|name| !request_header(request, name).trim().is_empty())
    {
        return false;
    }
    // Browsers add Origin to cross-origin requests. Treat those as remote so
    // an arbitrary web page cannot use the unauthenticated loopback endpoint
    // to read or edit the open project. Browser-based MCP clients use the
    // tunnel URL and its token instead.
    if !request_header(request, "Origin").trim().is_empty() {
        return false;
    }
    is_local_host(&request_header(request, "Host"))
}

/// Extracts `<token>` from `/mcp/<token>` (ignoring query string and trailing `/`).
fn extract_path_token(url: &str) -> Option<&str> {
    let path = url.split('?').next().unwrap_or(url).trim_end_matches('/');
    let rest = path.strip_prefix("/mcp/")?;
    (!rest.is_empty() && !rest.contains('/')).then_some(rest)
}

fn is_authorized(request: &Request, tunnel_token: &TunnelToken) -> bool {
    if is_local_request(request) {
        return true;
    }
    extract_path_token(request.url()).is_some_and(|token| tunnel_token.matches(token))
}

fn respond(
    mut request: Request,
    calls: &mpsc::Sender<ToolCall>,
    tunnel_token: &TunnelToken,
    wake: &dyn Fn(),
) {
    let accept = request_header(&request, "Accept");
    let response = if matches!(request.method(), Method::Options) {
        Response::empty(204).boxed()
    } else if !is_authorized(&request, tunnel_token) {
        match request.method() {
            Method::Post => text_response(
                403,
                "application/json",
                protocol::error_response(
                    Value::Null,
                    -32001,
                    "Token do túnel inválido ou ausente. Copie o endereço completo em Conectar uma IA.",
                )
                .to_string(),
            ),
            _ => text_response(
                403,
                "text/plain; charset=UTF-8",
                "Acesso negado: o túnel do Trazo exige o endereço completo com token.\n".to_owned(),
            ),
        }
    } else {
        match request.method() {
            Method::Post => {
                let mut bytes = Vec::new();
                let read = request.as_reader().take(MAX_BODY).read_to_end(&mut bytes);
                if read.is_err() {
                    text_response(
                        400,
                        "application/json",
                        protocol::error_response(
                            Value::Null,
                            -32700,
                            "Corpo da requisição ilegível.",
                        )
                        .to_string(),
                    )
                } else {
                    let body = decode_body(bytes);
                    let mut run =
                        |name: &str, arguments: &Value| run_in_app(calls, wake, name, arguments);
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
            // An event stream without the tunnel token ends up here.
            Method::Get if accept.contains("text/event-stream") => {
                with_header(Response::empty(405), "Allow", "POST, OPTIONS").boxed()
            }
            Method::Get => text_response(
                200,
                "text/plain; charset=UTF-8",
                "Servidor MCP do Trazo ativo.\n\
                 Conecte o assistente a este endereço terminado em /mcp (requisições POST).\n"
                    .to_owned(),
            ),
            _ => with_header(Response::empty(405), "Allow", "POST, GET, OPTIONS").boxed(),
        }
    };
    let response = with_header_box(response, "Access-Control-Allow-Origin", "*");
    let response = with_header_box(
        response,
        "Access-Control-Allow-Methods",
        "GET, POST, DELETE, OPTIONS",
    );
    let response = with_header_box(response, "Access-Control-Allow-Headers", "*");
    let response = with_header_box(response, "Access-Control-Expose-Headers", "Mcp-Session-Id");
    let response = with_header_box(response, "Cache-Control", "no-store");
    let _ = request.respond(response);
}

/// Bodies should be UTF-8, but command-line tools on Windows often send
/// accented text as Latin-1; that is read as such instead of failing.
fn decode_body(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap_or_else(|error| {
        error
            .into_bytes()
            .iter()
            .map(|&byte| byte as char)
            .collect()
    })
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
        let (status, content_type, body) = http(
            port,
            "POST",
            "application/json, text/event-stream",
            &initialize,
        );
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

        // GET with text/event-stream now opens a live stream: see
        // `event_stream_tests`.
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
        let call =
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"obter_planta"}}"#;
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

    #[test]
    fn tunneled_requests_require_matching_secret_token() {
        let token = TunnelToken::new();
        token.set(Some("segredo-123".to_owned()));
        let (sender, receiver) = mpsc::channel::<ToolCall>();
        std::thread::spawn(move || {
            for call in receiver {
                let _ = call
                    .reply
                    .send(ToolOutput::ok(format!("ran {}", call.name)).to_result());
            }
        });
        let port = start_with_token(0, sender, token.clone(), EventHub::new(), || {}).unwrap();

        let send_raw = |path: &str, extra_headers: &str| -> u16 {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
            write!(
                stream,
                "POST {path} HTTP/1.1\r\n{extra_headers}Accept: application/json\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            let mut raw = String::new();
            stream.read_to_string(&mut raw).unwrap();
            raw.split(' ').nth(1).unwrap().parse().unwrap()
        };

        // Local requests need no token.
        assert_eq!(send_raw("/mcp", "Host: 127.0.0.1\r\n"), 200);
        // A web page is not a trusted local client, even though it connects to
        // the loopback address. Without this check any site could edit a plan.
        assert_eq!(
            send_raw(
                "/mcp",
                "Host: 127.0.0.1\r\nOrigin: https://malicious.example\r\n"
            ),
            403
        );
        // Tunneled requests without a token or with a wrong token are rejected.
        let tunnel_headers = "Host: demo.trycloudflare.com\r\nCf-Connecting-Ip: 203.0.113.5\r\n";
        assert_eq!(send_raw("/mcp", tunnel_headers), 403);
        assert_eq!(send_raw("/mcp/errado", tunnel_headers), 403);
        // With the active token they succeed.
        assert_eq!(send_raw("/mcp/segredo-123", tunnel_headers), 200);
        // Closing the tunnel immediately invalidates the token.
        token.clear();
        assert_eq!(send_raw("/mcp/segredo-123", tunnel_headers), 403);
    }

    #[test]
    fn extracts_tokens_only_from_mcp_subpaths() {
        assert_eq!(extract_path_token("/mcp/abc123"), Some("abc123"));
        assert_eq!(extract_path_token("/mcp/abc123/"), Some("abc123"));
        assert_eq!(extract_path_token("/mcp/abc123?x=1"), Some("abc123"));
        assert_eq!(extract_path_token("/mcp"), None);
        assert_eq!(extract_path_token("/mcp/"), None);
        assert_eq!(extract_path_token("/mcp/a/b"), None);
        assert_eq!(extract_path_token("/other/abc"), None);
    }

    #[test]
    fn tunnel_token_handles_proxy_headers_options_get_and_local_hosts() {
        assert!(is_local_host("127.0.0.1"));
        assert!(is_local_host("127.0.0.1:7799"));
        assert!(is_local_host("localhost:7799"));
        assert!(is_local_host("LOCALHOST"));
        assert!(is_local_host("[::1]:7799"));
        assert!(is_local_host(""));
        assert!(!is_local_host("demo.trycloudflare.com"));
        assert!(!is_local_host("192.168.0.10:7799"));

        assert!(constant_time_eq(b"abc123", b"abc123"));
        assert!(!constant_time_eq(b"abc123", b"abc124"));
        assert!(!constant_time_eq(b"abc", b"abcd"));

        let token = TunnelToken::new();
        assert_eq!(token.current(), None);
        assert!(!token.matches("anything"));
        token.set(Some("chave-xyz".to_owned()));
        assert_eq!(token.current().as_deref(), Some("chave-xyz"));
        assert!(token.matches("chave-xyz"));
        assert!(!token.matches("chave-xyZ"));

        let (sender, receiver) = mpsc::channel::<ToolCall>();
        std::thread::spawn(move || {
            for call in receiver {
                let _ = call.reply.send(ToolOutput::ok("ok").to_result());
            }
        });
        let port = start_with_token(0, sender, token.clone(), EventHub::new(), || {}).unwrap();

        let send = |method: &str, path: &str, headers: &str, body: &str| -> u16 {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            write!(
                stream,
                "{method} {path} HTTP/1.1\r\n{headers}Accept: application/json\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            let mut raw = String::new();
            stream.read_to_string(&mut raw).unwrap();
            raw.split(' ').nth(1).unwrap().parse().unwrap()
        };

        let ping = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        // CORS preflight passes through tunnel without requiring a token.
        assert_eq!(
            send(
                "OPTIONS",
                "/mcp",
                "Host: demo.trycloudflare.com\r\nCf-Connecting-Ip: 1.2.3.4\r\n",
                ""
            ),
            204
        );
        // Proxy headers force token check even if Host says 127.0.0.1.
        assert_eq!(
            send(
                "POST",
                "/mcp",
                "Host: 127.0.0.1\r\nX-Forwarded-For: 203.0.113.9\r\n",
                ping
            ),
            403
        );
        assert_eq!(
            send(
                "POST",
                "/mcp/chave-xyz",
                "Host: 127.0.0.1\r\nX-Forwarded-For: 203.0.113.9\r\n",
                ping
            ),
            200
        );
        // GET over tunnel also requires the token.
        assert_eq!(
            send("GET", "/mcp", "Host: demo.trycloudflare.com\r\n", ""),
            403
        );
        assert_eq!(
            send(
                "GET",
                "/mcp/chave-xyz",
                "Host: demo.trycloudflare.com\r\n",
                ""
            ),
            200
        );
    }
}

#[cfg(test)]
mod event_stream_tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    /// Publishes until `received` says the stream got it (a stream only
    /// hears what is published after it opens).
    fn publish_until(events: &EventHub, received: impl Fn() -> bool) {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            events.publish(&super::super::events::plan_updated());
            if received() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the event never arrived");
    }

    #[test]
    fn an_open_event_stream_hears_each_change_at_once() {
        let (sender, _calls) = mpsc::channel();
        let events = EventHub::new();
        let port = start_with_token(0, sender, TunnelToken::new(), events.clone(), || {}).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .write_all(
                format!("GET /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/event-stream\r\n\r\n")
                    .as_bytes(),
            )
            .unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut status = String::new();
        reader.read_line(&mut status).unwrap();
        assert!(status.starts_with("HTTP/1.1 200"), "{status}");
        let (found, seen) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|read| read > 0) {
                if let Some(data) = line.trim_end().strip_prefix("data: ") {
                    let _ = found.send(data.to_owned());
                    return;
                }
                line.clear();
            }
        });
        let message = std::sync::Mutex::new(None);
        publish_until(&events, || {
            if let Ok(data) = seen.try_recv() {
                *message.lock().unwrap() = Some(data);
            }
            message.lock().unwrap().is_some()
        });
        let data: Value = serde_json::from_str(&message.lock().unwrap().take().unwrap()).unwrap();
        assert_eq!(data["method"], "notifications/resources/updated");
        assert_eq!(data["params"]["uri"], "trazo://planta");
    }
}
