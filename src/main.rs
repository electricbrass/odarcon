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

use cursive;
use cursive::event::{Event, Key};
use cursive::theme::{ColorStyle, ColorType, PaletteColor, Style};
use cursive::utils::markup::StyledString;
use cursive::view::*;
use cursive::views::*;
use cursive::views::{EditView, LinearLayout, TextView};
use cursive::{Cursive, CursiveExt};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::protocol::{ClientMessage, ClientMessageType};
mod config;
mod protocol;
mod socket;
// use url::Url;

fn main() {
    let mut siv = Cursive::default();
    siv.add_layer(
        Dialog::around(TextView::new("Hello Dialog!"))
            .title("Cursive")
            .button("Quit", |s| s.quit()),
    );

    let output = TextView::new("")
        .with_name("output")
        .scrollable()
        .scroll_strategy(ScrollStrategy::StickToBottom);
    let output_panel = Panel::new(output).title("Console");

    let input = EditView::new()
        .on_submit(|s, text| {
            s.call_on_name("output", |v: &mut TextView| {
                v.append(format!("> {}\n", text));
            });

            s.call_on_name("input", |v: &mut EditView| {
                v.set_content("");
            });

            let json_msg =
                ClientMessage::new(ClientMessageType::Command(text.to_string())).serialize();
            if let Some(tx) = s.user_data::<tokio::sync::mpsc::UnboundedSender<String>>() {
                let _ = tx.send(json_msg);
            }
        })
        .filler(" ")
        .style(Style {
            color: ColorStyle {
                front: ColorType::Palette(PaletteColor::Primary),
                back: ColorType::Palette(PaletteColor::Secondary),
            },
            ..Style::default()
        })
        .with_name("input");

    let input_row = LinearLayout::horizontal()
        .child(TextView::new("> "))
        .child(input.full_width());

    let input_panel = Panel::new(input_row).title("Command");

    let left_pane = LinearLayout::vertical()
        .child(output_panel.full_height())
        .child(input_panel);

    let right_pane = LinearLayout::vertical()
        .child(Button::new("Button 1", |_| {}).with_name("button1"))
        .child(Button::new("Button 2", |_| {}))
        .child(Button::new("Button 3", |_| {}))
        .child(DummyView.fixed_height(1))
        .child(Button::new("Quit", |s| s.quit()));

    let right_panel = Panel::new(right_pane).title("Actions").fixed_width(18);

    let main_layout = LinearLayout::horizontal()
        .child(left_pane.full_width())
        .child(right_panel);

    siv.add_fullscreen_layer(main_layout);

    siv.load_toml(include_str!("../res/theme.toml")).unwrap();

    #[cfg(debug_assertions)]
    siv.add_global_callback(Event::CtrlChar('r'), |s| {
        match s.load_theme_file("./res/theme.toml") {
            Ok(_) => (),
            Err(_e) => error_popup("theme.toml not found", s),
        }
    });

    #[cfg(debug_assertions)]
    siv.add_global_callback(Event::CtrlChar('e'), |s| {
        error_popup("This is a test error popup", s)
    });

    siv.add_global_callback('/', |s| match s.focus_name("input") {
        Ok(cb) => cb.process(s),
        Err(_) => error_popup("Console input could not be focused", s),
    });

    siv.add_global_callback(Key::Esc, |s| match s.focus_name("button1") {
        Ok(cb) => cb.process(s),
        Err(_) => error_popup("Button 1 could not be focused", s),
    });

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    siv.set_user_data(tx);

    let rt = Runtime::new().unwrap();

    let cb_sink = siv.cb_sink().clone();
    let print_to_console = move |text: String| {
        cb_sink
            .send(Box::new(move |s: &mut Cursive| {
                s.call_on_name("output", |v: &mut TextView| {
                    v.append(format!("> {}\n", text));
                });
            }))
            .unwrap();
    };

    rt.spawn(async move {
        print_to_console("Starting connection...".to_string());
        // let url = Url::parse("ws://127.0.0.1:11666").unwrap();
        let mut req = "ws://127.0.0.1:11666".into_client_request().unwrap();
        req.headers_mut()
            .append("Sec-WebSocket-Protocol", "odamex-rcon".parse().unwrap()); // unwrap is safe with only ascii
        let (ws_stream, _) = connect_async(req).await.expect("Failed to connect");
        print_to_console("Connected to websocket server!".to_string());

        let (mut write, mut read) = ws_stream.split();

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let _ = write.send(Message::Text(msg.into())).await;
            }
        });

        // read messages from websocket
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(txt)) => match serde_json::from_str::<Value>(&txt) {
                    Ok(json) => print_to_console(format!("Received: {:?}", json)),
                    Err(_) => print_to_console(format!("Received text: {}", txt)),
                },
                Ok(Message::Binary(_)) => {}
                Ok(Message::Close(_)) => break,
                _ => {}
            }
        }
    });

    siv.run();
}

fn error_popup(message: &str, s: &mut Cursive) {
    let mut text = StyledString::styled(
        "Error:\n\n",
        Style {
            color: ColorStyle {
                back: ColorType::InheritParent,
                front: ColorType::Palette(PaletteColor::TitlePrimary),
            },
            ..Style::default()
        },
    );
    text.append(StyledString::plain(message));
    s.add_layer(Dialog::info(text).padding_left(3).padding_right(3)); // TODO: make this a little nicer
    // only want padding applied to the message but not button or Error:
}
