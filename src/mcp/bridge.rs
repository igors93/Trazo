//! `trazo --mcp-stdio`: MCP over stdin/stdout, for clients that start a
//! program instead of connecting to an address (Claude Desktop, for one).
//!
//! The handshake and the tool list are answered here, so the client starts
//! without opening any window. Tool calls go to the Trazo window's HTTP
//! server; if no window is running, the first call opens one.

use std::io::{BufRead, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{ToolOutput, protocol};

/// How long to wait for a freshly opened window to start listening.
const WINDOW_START_TIMEOUT: Duration = Duration::from_secs(40);
/// Longer than the server's own wait for the window, so its answer arrives.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(60);

/// Writes one JSON message as a line on stdout, whole: answers and
/// notifications come from two threads and must not interleave.
fn write_line(message: &str) -> bool {
    let mut output = std::io::stdout().lock();
    output
        .write_all(format!("{message}\n").as_bytes())
        .and_then(|()| output.flush())
        .is_ok()
}

/// Serves MCP on stdin/stdout until the client closes stdin.
pub fn run(port: u16) {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut line = Vec::new();
    let mut relaying = false;
    loop {
        line.clear();
        match input.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let text = String::from_utf8_lossy(&line);
        // Once the client subscribes to the plan, the window's change
        // notices are passed on to it.
        if !relaying && text.contains("resources/subscribe") {
            relaying = true;
            let _ = std::thread::Builder::new()
                .name("trazo-mcp-avisos".to_owned())
                .spawn(move || relay_events(port, &mut |message| write_line(message)));
        }
        let mut forward =
            |name: &str, arguments: &Value| forward_tool(port, name, arguments, &launch_window);
        if let Some(answer) = handle_line(&text, &mut forward) {
            // One JSON message per line; serde_json writes no raw newlines.
            if !write_line(&answer) {
                break;
            }
        }
    }
}

/// Listens to the window's event stream and hands each message to
/// `deliver`, reconnecting while the window is closed; returns when
/// `deliver` fails (the client went away).
fn relay_events(port: u16, deliver: &mut dyn FnMut(&str) -> bool) {
    loop {
        if let Ok(mut stream) = TcpStream::connect_timeout(&address(port), Duration::from_secs(2)) {
            let request = format!(
                "GET /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/event-stream\r\n\r\n"
            );
            if stream.write_all(request.as_bytes()).is_ok() {
                let mut reader = std::io::BufReader::new(stream);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                    if let Some(data) = line.trim_end().strip_prefix("data: ")
                        && !deliver(data)
                    {
                        return;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_secs(3));
    }
}

/// Answers one line from the client, or nothing for blank lines and
/// notifications.
fn handle_line(line: &str, forward: &mut protocol::ToolRunner) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    protocol::handle_text(line, forward)
}

/// Runs a tool in the Trazo window, opening the window first if needed.
fn forward_tool(
    port: u16,
    name: &str,
    arguments: &Value,
    launch: &dyn Fn() -> Result<(), String>,
) -> Value {
    if let Err(message) = ensure_window(port, launch) {
        return ToolOutput::error(message).to_result();
    }
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    });
    let answer = post(port, &request.to_string()).and_then(|body| {
        serde_json::from_str::<Value>(&body).map_err(|error| format!("resposta inválida ({error})"))
    });
    match answer {
        Ok(mut response) => match response.get_mut("result") {
            Some(result) => result.take(),
            None => {
                let message = response["error"]["message"]
                    .as_str()
                    .unwrap_or("erro desconhecido")
                    .to_owned();
                ToolOutput::error(message).to_result()
            }
        },
        Err(error) => {
            ToolOutput::error(format!("Não foi possível falar com o Trazo: {error}")).to_result()
        }
    }
}

fn address(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn reachable(port: u16) -> bool {
    TcpStream::connect_timeout(&address(port), Duration::from_millis(500)).is_ok()
}

fn ensure_window(port: u16, launch: &dyn Fn() -> Result<(), String>) -> Result<(), String> {
    if reachable(port) {
        return Ok(());
    }
    launch()?;
    let started = Instant::now();
    while started.elapsed() < WINDOW_START_TIMEOUT {
        std::thread::sleep(Duration::from_millis(300));
        if reachable(port) {
            return Ok(());
        }
    }
    Err(
        "O Trazo foi aberto, mas o servidor MCP não respondeu a tempo. Veja se outra \
         janela do Trazo ou outro programa está usando a porta."
            .to_owned(),
    )
}

/// Opens a Trazo window that outlives this bridge: when the client stops
/// the bridge, the person's drawing stays open.
fn launch_window() -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Não foi possível localizar o Trazo ({error})."))?;
    let command = || {
        let mut command = Command::new(&executable);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    };
    #[cfg(windows)]
    let spawned = {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        let flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        // Leaving the client's job keeps the window alive when the client
        // ends the bridge; not every job allows it, so try both.
        command()
            .creation_flags(flags | CREATE_BREAKAWAY_FROM_JOB)
            .spawn()
            .or_else(|_| command().creation_flags(flags).spawn())
    };
    #[cfg(not(windows))]
    let spawned = command().spawn();
    spawned
        .map(|_| ())
        .map_err(|error| format!("Não foi possível abrir o Trazo ({error})."))
}

/// A minimal HTTP/1.1 POST to the window's MCP endpoint.
fn post(port: u16, body: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect_timeout(&address(port), Duration::from_secs(3))
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(ANSWER_TIMEOUT))
        .and_then(|_| stream.set_write_timeout(Some(Duration::from_secs(10))))
        .map_err(|error| error.to_string())?;
    let head = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|_| stream.write_all(body.as_bytes()))
        .map_err(|error| error.to_string())?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|error| error.to_string())?;
    parse_http_response(&raw)
}

fn parse_http_response(raw: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or("resposta HTTP incompleta")?;
    let status: u16 = head
        .split_whitespace()
        .nth(1)
        .and_then(|status| status.parse().ok())
        .ok_or("resposta HTTP sem status")?;
    if !(200..300).contains(&status) {
        return Err(format!("o servidor respondeu HTTP {status}"));
    }
    let chunked = head
        .to_ascii_lowercase()
        .lines()
        .any(|line| line.starts_with("transfer-encoding:") && line.contains("chunked"));
    Ok(if chunked {
        dechunk(body)
    } else {
        body.to_owned()
    })
}

fn dechunk(mut body: &str) -> String {
    let mut out = String::new();
    while let Some((size, rest)) = body.split_once("\r\n") {
        let Ok(size) = usize::from_str_radix(size.trim(), 16) else {
            break;
        };
        if size == 0 || rest.len() < size {
            break;
        }
        out.push_str(&rest[..size]);
        body = rest[size..].trim_start_matches("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::server::{self, ToolCall};
    use std::sync::mpsc;

    fn no_launch() -> Result<(), String> {
        Err("sem janela nos testes".to_owned())
    }

    #[test]
    fn handshake_and_tool_list_need_no_window() {
        let mut forward = |_: &str, _: &Value| -> Value { panic!("must not forward") };
        assert_eq!(handle_line("   \n", &mut forward), None);
        let answer = handle_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"claude-ai","version":"1"}}}"#,
            &mut forward,
        )
        .unwrap();
        assert!(!answer.contains('\n'));
        let answer: Value = serde_json::from_str(&answer).unwrap();
        assert_eq!(answer["result"]["serverInfo"]["name"], "trazo");
        let tools: Value = serde_json::from_str(
            &handle_line(
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
                &mut forward,
            )
            .unwrap(),
        )
        .unwrap();
        // HTTP and stdio expose the same complete catalogue before opening a window.
        assert_eq!(
            tools["result"]["tools"].as_array().unwrap(),
            &crate::mcp::protocol::tools()
        );
    }

    #[test]
    fn tool_calls_reach_the_window_over_http() {
        let (sender, receiver) = mpsc::channel::<ToolCall>();
        std::thread::spawn(move || {
            for call in receiver {
                let text = format!("{} {}", call.name, call.arguments["nome"]);
                let _ = call.reply.send(ToolOutput::ok(text).to_result());
            }
        });
        let port = server::start(0, sender, || {}).unwrap();
        let result = forward_tool(
            port,
            "criar_comodo",
            &json!({ "nome": "Suíte" }),
            &no_launch,
        );
        assert_eq!(result["isError"], false);
        assert_eq!(result["content"][0]["text"], "criar_comodo \"Suíte\"");
    }

    #[test]
    fn a_missing_window_that_cannot_open_is_an_error_result() {
        // Port 1 is never a Trazo window.
        let result = forward_tool(1, "obter_planta", &json!({}), &no_launch);
        assert_eq!(result["isError"], true);
        assert_eq!(result["content"][0]["text"], "sem janela nos testes");
    }

    #[test]
    fn http_responses_are_parsed_plain_or_chunked() {
        let plain = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(parse_http_response(plain).unwrap(), "{}");
        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n";
        assert_eq!(parse_http_response(chunked).unwrap(), "{\"a\":1}");
        assert!(parse_http_response(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n").is_err());
        assert!(parse_http_response(b"garbage").is_err());
    }
}

#[cfg(test)]
mod relay_tests {
    use super::*;
    use crate::mcp::events::{EventHub, plan_updated};
    use crate::mcp::server::{TunnelToken, start_with_token};
    use std::sync::mpsc;

    #[test]
    fn the_bridge_passes_the_windows_notices_on() {
        let (sender, _calls) = mpsc::channel();
        let events = EventHub::new();
        let port = start_with_token(0, sender, TunnelToken::new(), events.clone(), || {}).unwrap();
        let (delivered, received) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            relay_events(port, &mut |message| {
                delivered.send(message.to_owned()).is_ok()
            })
        });
        let started = Instant::now();
        let message = loop {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "no notice arrived"
            );
            events.publish(&plan_updated());
            if let Ok(message) = received.recv_timeout(Duration::from_millis(100)) {
                break message;
            }
        };
        let message: Value = serde_json::from_str(&message).unwrap();
        assert_eq!(message["method"], "notifications/resources/updated");
    }
}
