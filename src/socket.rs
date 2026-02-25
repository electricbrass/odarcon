/*
 * Copyright (C) 2026  Mia McMahill
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 */

use crate::protocol::{ClientMessage, PrintLevel, ServerMessage, ServerMessageType};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[derive(Debug, Error)]
pub enum RCONError {
    #[error("Websocket error: {0}")]
    WebsocketError(#[from] tungstenite::Error),
}

pub struct RCONSocket {
    tx: UnboundedSender<String>,
    on_log: Arc<dyn Fn(String, Option<PrintLevel>) + Send + Sync>,
}

impl RCONSocket {
    pub fn connect<F>(host: &str, port: u16, password: &str, on_log: F) -> Result<Self, RCONError>
    where
        F: Fn(String, Option<PrintLevel>) + Send + Sync + 'static,
    {
        let url_str = format!("ws://{}:{}", host, port);
        // TODO: better error handling here, this likely wont result in a good error
        let mut req = url_str.into_client_request()?;
        let on_log = Arc::new(on_log);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        tokio::spawn({
            let on_log = on_log.clone();
            async move {
                req.headers_mut()
                    .append("Sec-WebSocket-Protocol", "odamex-rcon".parse().unwrap()); // unwrap is safe with only ascii
                let (ws_stream, _) = connect_async(req).await.expect("Failed to connect");
                on_log("Connected to odamex server!\n".to_string(), None);

                let (mut write, mut read) = ws_stream.split();

                tokio::spawn({
                    let on_log = on_log.clone();
                    async move {
                        while let Some(msg) = rx.recv().await {
                            if let Err(e) = write.send(Message::Text(msg.into())).await {
                                on_log(format!("Failed to send message: {}", e), None);
                            }
                        }
                    }
                });

                // read messages from websocket
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(txt)) => match txt.parse::<ServerMessage>() {
                            Ok(message) => match message.content {
                                ServerMessageType::Print { printlevel, text } => {
                                    on_log(text, Some(printlevel))
                                }
                                _ => on_log(format!("Received: {}\n", message), None),
                            },
                            Err(e) => {
                                on_log(format!("Received invalid message: {}\n{}\n", txt, e), None)
                            }
                        },
                        Ok(Message::Binary(_)) => {}
                        Ok(Message::Close(_)) => {
                            on_log("Connection to server has been closed\n".to_string(), None);
                            break;
                        }
                        _ => {}
                    }
                }
            }
        });
        Ok(Self { tx, on_log })
    }

    pub fn send(&self, message: ClientMessage) {
        if let Err(e) = self.tx.send(message.serialize()) {
            (self.on_log)(format!("Failed to send message: {}", e), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn on_log(_s: String, _p: Option<PrintLevel>) {}

    #[test]
    fn connect_invalid_hostname() {
        let s = RCONSocket::connect("example com", 11666, "", on_log);
        assert!(s.is_err())
    }
}
