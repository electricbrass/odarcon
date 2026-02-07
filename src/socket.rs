use crate::protocol::ClientMessage;
use futures_util::{SinkExt, StreamExt};
use tokio::runtime::Runtime;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

struct RCONSocket {
    on_log: fn(),
}

impl RCONSocket {
    fn new() -> Self {
        Self { on_log: || {} }
    }

    fn send(&self, message: ClientMessage) {}
}
