//! Model Context Protocol (MCP) support: lets an AI assistant such as
//! ChatGPT or Claude read and edit the plan open in Trazo through tools.
//!
//! The protocol layer here knows nothing about the interface. The tools
//! themselves run inside the app (`crate::app::mcp_tools`), on the open
//! project, so every change shows live and can be undone.

use base64::Engine as _;
use serde_json::{Value, json};

pub mod bridge;
pub mod protocol;
pub mod server;

/// The local port of the MCP server, unless `TRAZO_MCP_PORT` says otherwise.
pub const DEFAULT_PORT: u16 = 7799;

pub fn port() -> u16 {
    std::env::var("TRAZO_MCP_PORT")
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|port| *port != 0)
        .unwrap_or(DEFAULT_PORT)
}

/// The address clients connect to on this computer.
pub fn local_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/mcp")
}

/// What a tool returns: text for the assistant, optionally structured data
/// and a picture of the plan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolOutput {
    pub text: String,
    pub structured: Option<Value>,
    pub image_png: Option<Vec<u8>>,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
            ..Default::default()
        }
    }

    pub fn with_structured(mut self, value: Value) -> Self {
        self.structured = Some(value);
        self
    }

    pub fn with_image(mut self, png: Vec<u8>) -> Self {
        self.image_png = Some(png);
        self
    }

    /// The MCP `CallToolResult` for this output.
    pub fn to_result(&self) -> Value {
        let mut content = Vec::new();
        if let Some(png) = &self.image_png {
            content.push(json!({
                "type": "image",
                "data": base64::engine::general_purpose::STANDARD.encode(png),
                "mimeType": "image/png",
            }));
        }
        content.push(json!({ "type": "text", "text": self.text }));
        let mut result = json!({ "content": content, "isError": self.is_error });
        if let Some(structured) = self.structured.as_ref().filter(|value| value.is_object()) {
            result["structuredContent"] = structured.clone();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_output_becomes_an_mcp_result() {
        let result = ToolOutput::ok("feito")
            .with_structured(json!({ "id": 3 }))
            .with_image(vec![1, 2, 3])
            .to_result();
        assert_eq!(result["isError"], false);
        assert_eq!(result["content"][0]["type"], "image");
        assert_eq!(result["content"][0]["data"], "AQID");
        assert_eq!(result["content"][1]["text"], "feito");
        assert_eq!(result["structuredContent"]["id"], 3);

        let error = ToolOutput::error("falhou").to_result();
        assert_eq!(error["isError"], true);
        assert!(error.get("structuredContent").is_none());
    }

    #[test]
    fn local_url_points_at_the_mcp_endpoint() {
        assert_eq!(local_url(7799), "http://127.0.0.1:7799/mcp");
    }
}
