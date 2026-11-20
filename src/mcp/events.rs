//! Server-to-client messages: the window publishes "the plan changed" and
//! every assistant listening on an event stream (`GET /mcp`) receives it at
//! once, so it notices the person's edits without asking.

use std::sync::{Arc, Mutex, mpsc};

use serde_json::{Value, json};

/// The address assistants read and subscribe to for the open plan.
pub const PLAN_URI: &str = "trazo://planta";

/// The notification sent when the open plan changes.
pub fn plan_updated() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/resources/updated",
        "params": { "uri": PLAN_URI },
    })
}

/// The open event streams, each fed through its own channel.
#[derive(Clone, Debug, Default)]
pub struct EventHub {
    listeners: Arc<Mutex<Vec<mpsc::Sender<String>>>>,
}

impl EventHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// A new stream: it receives every message published from now on.
    pub fn listen(&self) -> mpsc::Receiver<String> {
        let (sender, receiver) = mpsc::channel();
        if let Ok(mut listeners) = self.listeners.lock() {
            listeners.push(sender);
        }
        receiver
    }

    /// Sends `message` to every open stream; closed ones are dropped.
    pub fn publish(&self, message: &Value) {
        let text = message.to_string();
        if let Ok(mut listeners) = self.listeners.lock() {
            listeners.retain(|listener| listener.send(text.clone()).is_ok());
        }
    }

    /// How many streams are open.
    pub fn listeners(&self) -> usize {
        self.listeners
            .lock()
            .map(|listeners| listeners.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_open_stream_hears_each_message() {
        let hub = EventHub::new();
        let first = hub.listen();
        let second = hub.listen();
        assert_eq!(hub.listeners(), 2);
        hub.publish(&plan_updated());
        for stream in [&first, &second] {
            let message: Value = serde_json::from_str(&stream.recv().unwrap()).unwrap();
            assert_eq!(message["params"]["uri"], PLAN_URI);
        }
        // A closed stream is forgotten at the next message.
        drop(second);
        hub.publish(&plan_updated());
        assert_eq!(hub.listeners(), 1);
        assert!(first.try_recv().is_ok());
    }
}
