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
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

pub struct RCONSocket {
    tx: UnboundedSender<String>,
    on_log: fn(String),
}

impl RCONSocket {
    pub fn connect<F>(host: &str, port: u16, password: &str, on_log: F) -> Result<Self, ()>
    where
        F: Fn(String, Option<PrintLevel>) + Send + Sync + 'static,
    {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            on_log("Starting connection...\n".to_string(), None);
            // let url = Url::parse("ws://127.0.0.1:11666").unwrap();
            let mut req = "ws://127.0.0.1:10666".into_client_request().unwrap();
            req.headers_mut()
                .append("Sec-WebSocket-Protocol", "odamex-rcon".parse().unwrap()); // unwrap is safe with only ascii
            let (ws_stream, _) = connect_async(req).await.expect("Failed to connect");
            on_log("Connected to odamex server!\n".to_string(), None);

            let (mut write, mut read) = ws_stream.split();

            tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    let _ = write.send(Message::Text(msg.into())).await;
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
        });
        Ok(Self { tx, on_log: |_| {} })
    }

    pub fn send(&self, message: ClientMessage) {
        let _ = self.tx.send(message.serialize());
    }
}
