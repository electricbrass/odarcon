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
use cursive::align::{HAlign, VAlign};
use cursive::event::{Event, Key};
use cursive::theme::{ColorStyle, ColorType, Effect, Effects, PaletteColor, PaletteStyle, Style};
use cursive::utils::markup::StyledString;
use cursive::view::*;
use cursive::views::*;
use cursive::views::{EditView, LinearLayout, TextView};
use cursive::{Cursive, CursiveExt};
use futures_util::{SinkExt, StreamExt};
use tokio::runtime::Runtime;
use tokio::time::error;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
// use url::Url;

mod config;
mod protocol;
mod socket;
use crate::config::Config;
use crate::protocol::{ClientMessage, ClientMessageType, ServerMessage, ServerMessageType};

// TODO: use directories to get XDG_STATE_HOME location and write stderr logs there
// TODO: add mode for client commands, like alt c to switch modes or prefixing with ! or : or something

struct AppState {
    config: Config,
}

#[tokio::main]
async fn main() {
    cursive::logger::init();
    cursive::logger::set_internal_filter_level(log::LevelFilter::Off);

    let mut siv = Cursive::default();

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

    #[cfg(debug_assertions)]
    siv.add_global_callback(Event::CtrlChar('d'), |s| {
        s.toggle_debug_console();
    });

    main_menu(&mut siv);

    let config = Config::load().unwrap_or_else(|e| {
        error_popup("Config file could not be loaded", &mut siv);
        log::error!("Config file could not be loaded: {e}");
        Config::new()
    });

    siv.run();
}

fn error_popup(message: &str, s: &mut Cursive) {
    s.add_layer(
        Dialog::around(
            LinearLayout::vertical()
                .child(TextView::new(StyledString::styled(
                    "Error:",
                    Style {
                        color: ColorStyle::front(ColorType::Palette(PaletteColor::TitlePrimary)),
                        effects: Effects::only(Effect::Bold),
                    },
                )))
                .child(DummyView.fixed_height(1))
                .child(PaddedView::new(
                    Margins {
                        left: 2,
                        right: 2,
                        top: 0,
                        bottom: 0,
                    },
                    TextView::new(message).h_align(HAlign::Center),
                )),
        )
        .dismiss_button("Ok"),
    );
}

fn main_menu(siv: &mut Cursive) {
    let mut quick_connect = ListView::new();
    quick_connect.add_child("Hostname:", EditView::new().with_name("hostname"));
    quick_connect.add_child("Port (optional):", EditView::new().with_name("port"));
    quick_connect.add_child("Password:", EditView::new().secret().with_name("password"));

    let quick_connect = Panel::new(PaddedView::new(
        Margins {
            left: 3,
            right: 3,
            top: 1,
            bottom: 1,
        },
        LinearLayout::vertical()
            .child(quick_connect)
            .child(DummyView.fixed_height(1))
            .child(Button::new("Connect", |s| {
                let hostname = s.call_on_name("hostname", |v: &mut EditView| v.get_content());
                let port = s.call_on_name("port", |v: &mut EditView| v.get_content());
                let password = s.call_on_name("password", |v: &mut EditView| v.get_content());
                s.pop_layer();
                rcon_layer(
                    s,
                    hostname.unwrap().as_str(),
                    port.unwrap().as_str(),
                    password.unwrap().as_str(),
                );
            })),
    ))
    .title("Quick Connect");

    let mut welcome = StyledString::new();
    welcome.append_plain("Welcome to\n");
    welcome.append_plain("Oda");
    welcome.append_styled(
        "RCON",
        Style {
            color: ColorStyle {
                front: ColorType::Palette(PaletteColor::TitlePrimary),
                back: ColorType::InheritParent,
            },
            effects: Effects::only(Effect::Bold),
        },
    );
    welcome.append_plain("!");

    let welcome = Panel::new(
        TextView::new(welcome)
            .h_align(HAlign::Center)
            .v_align(VAlign::Center),
    );

    let servers = Panel::new(ListView::new()).title("Servers");

    siv.add_fullscreen_layer(
        LinearLayout::vertical()
            .child(
                LinearLayout::horizontal()
                    // TODO: make these widths look better
                    .child(quick_connect.full_width())
                    // TODO: i want min_width to be 20 and be used for smaller screens
                    // but right now the full_width on quick_connect
                    // just makes this always use it's min width
                    .child(welcome.min_width(32).max_width(32)),
            )
            .child(servers.full_height()),
    );
}

fn rcon_layer(siv: &mut Cursive, hostname: &str, port: &str, password: &str) {
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
        .with_name("input");

    let input_row = LinearLayout::horizontal()
        .child(TextView::new("> "))
        .child(input.full_width());

    let input_panel = Panel::new(input_row).title("Command");

    let left_pane = LinearLayout::vertical()
        .child(output_panel.full_height())
        .child(input_panel)
        .with_name("left");

    let right_pane = LinearLayout::vertical()
        .child(Button::new("Maplist", |_| {}).with_name("button1"))
        .child(Button::new("Button 2", |_| {}))
        .child(Button::new("Button 3", |_| {}))
        .child(DummyView.fixed_height(1))
        .child(Button::new("Disconnect", |s| {
            s.pop_layer();
            main_menu(s);
        }))
        .child(Button::new("Quit", |s| s.quit()));

    let right_panel = Panel::new(right_pane).title("Actions").fixed_width(18);

    let main_layout = LinearLayout::horizontal()
        .child(left_pane)
        .child(right_panel);

    fn update_left_max_width(s: &mut Cursive) {
        let term_width = s.screen_size().x;
        let right_width = 18;
        if term_width > right_width {
            let max_left = term_width - right_width;
            s.call_on_name("left", |v: &mut ResizedView<LinearLayout>| {
                v.set_width(SizeConstraint::AtMost(max_left));
            });
            s.call_on_name("output", |v: &mut TextView| {
                v.append(format!("> new width: {}\n", max_left));
            });
        }
    }

    // siv.add_global_callback(cursive::event::Event::WindowResize, |s| {
    //     update_left_max_width(s);
    // });

    // siv.add_global_callback(cursive::event::Event::Refresh, |s| {
    //     // This runs after the first frame
    //     let term_width = s.screen_size().x;
    //     let right_width = 18;
    //     let max_left = term_width.saturating_sub(right_width);
    //     s.call_on_name("left", |v: &mut ResizedView<LinearLayout>| {
    //         v.set_width(SizeConstraint::AtMost(max_left));
    //     });
    //     s.call_on_name("output", |v: &mut TextView| {
    //         v.append(format!("> new width: {}\n", max_left));
    //     });

    //     // remove this callback after first run
    //     s.clear_global_callbacks(cursive::event::Event::Refresh);
    // });

    siv.add_fullscreen_layer(main_layout);

    // TODO: todo, make callbacks layer specific
    siv.add_global_callback('/', |s| match s.focus_name("input") {
        Ok(cb) => cb.process(s),
        Err(_) => error_popup("Console input could not be focused", s),
    });

    // TODO: todo, make callbacks layer specific
    siv.add_global_callback(Key::Esc, |s| match s.focus_name("button1") {
        Ok(cb) => cb.process(s),
        Err(_) => error_popup("Button 1 could not be focused", s),
    });

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    siv.set_user_data(tx);

    let cb_sink = siv.cb_sink().clone();
    // TODO: make a visual distinction between prints from the client and from the server
    // probably keep the > for the printing of commands, and for server logs nothing and for client logs some other character
    let print_to_console = move |text: String| {
        cb_sink
            .send(Box::new(move |s: &mut Cursive| {
                s.call_on_name("output", |v: &mut TextView| {
                    v.append(format!("> {}", text));
                });
            }))
            .unwrap();
    };

    // print_to_console("this is something really really long wow look how long this is its so long wahoo wow woahhhhhhhhhhhhhhhh what is this why is this so long".to_string());

    tokio::spawn(async move {
        print_to_console("Starting connection...\n".to_string());
        // let url = Url::parse("ws://127.0.0.1:11666").unwrap();
        let mut req = "ws://127.0.0.1:10666".into_client_request().unwrap();
        req.headers_mut()
            .append("Sec-WebSocket-Protocol", "odamex-rcon".parse().unwrap()); // unwrap is safe with only ascii
        let (ws_stream, _) = connect_async(req).await.expect("Failed to connect");
        print_to_console("Connected to odamex server!\n".to_string());

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
                        ServerMessageType::Print { printlevel, text } => print_to_console(text),
                        _ => print_to_console(format!("Received: {}\n", message)),
                    },
                    Err(e) => {
                        print_to_console(format!("Received invalid message: {}\n{}\n", txt, e))
                    }
                },
                Ok(Message::Binary(_)) => {}
                Ok(Message::Close(_)) => {
                    print_to_console("Connection to server has been closed\n".to_string());
                    break;
                }
                _ => {}
            }
        }
    });
}
