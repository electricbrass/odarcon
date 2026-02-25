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

use cursive::align::{HAlign, VAlign};
use cursive::event::{Event, Key};
use cursive::theme;
use cursive::theme::{ColorStyle, ColorType, Effect, Effects, PaletteColor, PaletteStyle, Style};
use cursive::utils::markup::StyledString;
use cursive::view::*;
use cursive::views::*;
use cursive::views::{EditView, LinearLayout, TextView};
use cursive::{Cursive, CursiveExt};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
// use url::Url;

mod config;
mod protocol;
mod socket;
use crate::config::{Config, ServerConfig};
use crate::protocol::{ClientMessage, ClientMessageType, ServerMessage, ServerMessageType};

// TODO: use directories to get XDG_STATE_HOME location and write stderr logs there
// TODO: add mode for client commands, like alt c to switch modes or prefixing with ! or : or something
// TODO: leave main menu layer at the bottom instead of popping it
// just make sure that the quick connect input fields get cleared
// this will make it so that the other layers dont need to worry
// about passing arguments to main_menu

struct AppState {
    config: Config,
}

#[tokio::main]
async fn main() {
    cursive::logger::init();
    cursive::logger::set_internal_filter_level(log::LevelFilter::Off);

    let mut siv = Cursive::default();

    #[cfg(debug_assertions)]
    // hot reload default theme for testing
    siv.add_global_callback(Event::CtrlChar('r'), |s| {
        if let Err(e) = s.load_theme_file("./res/theme.toml") {
            match e {
                theme::Error::Io(io_error) => {
                    error_popup("theme.toml could not be loaded", s);
                    log::error!("theme.toml could not be loaded: {io_error}");
                }
                theme::Error::Parse(parse_error) => {
                    error_popup("theme.toml could not be parsed", s);
                    log::error!("theme.toml could not be parsed: {parse_error}");
                }
            }
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

    let config = Config::load().unwrap_or_else(|e| {
        // TODO: make the popup more informative
        error_popup("Config file could not be loaded", &mut siv);
        log::error!("Config file could not be loaded: {e}");
        Config::default()
    });

    siv.set_user_data(AppState { config });

    main_menu(&mut siv);

    if let Some(themefile) = Config::config_dir().map(|dir| dir.join("theme.toml"))
        && themefile.exists()
    {
        if let Err(e) = siv.load_theme_file(themefile) {
            // TODO: make the popup more informative
            error_popup("Theme file could not be loaded", &mut siv);
            match e {
                theme::Error::Io(io_error) => {
                    log::error!("Theme file could not be loaded: {io_error}");
                }
                theme::Error::Parse(parse_error) => {
                    log::error!("Theme file could not be parsed: {parse_error}");
                }
            }
        };
    } else {
        siv.load_toml(include_str!("../res/theme.toml")).unwrap();
    }

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

fn filter_port(name: &str, siv: &mut Cursive, content: &str) {
    let filtered: String = content.chars().filter(|c| c.is_ascii_digit()).collect();

    let filtered = if filtered.len() > 5 {
        &filtered[..5]
    } else {
        &filtered
    };

    if filtered != content {
        siv.call_on_name(name, |v: &mut EditView| {
            v.set_content(filtered);
        });
    }
}

fn verify_port(port: &str, siv: &mut Cursive) -> Option<u16> {
    if port.is_empty() {
        return Some(10666);
    }

    match port.parse::<u16>() {
        Ok(port) => Some(port),
        Err(_) => {
            error_popup("Port must be in the range 0-65535", siv);
            None
        }
    }
}

fn main_menu(siv: &mut Cursive) {
    let mut quick_connect = ListView::new();
    quick_connect.add_child("Hostname:", EditView::new().with_name("hostname"));
    quick_connect.add_child(
        "Port (optional):",
        EditView::new()
            .on_edit(|s, content, _| filter_port("port", s, content))
            .with_name("port"),
    );
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
                if let Some(port) = verify_port(&port.unwrap(), s) {
                    s.pop_layer();
                    rcon_layer(s, &hostname.unwrap(), port, &password.unwrap());
                }
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
        LinearLayout::vertical()
            .child(DummyView.fixed_height(1))
            .child(TextView::new(welcome).h_align(HAlign::Center))
            .child(DummyView.fixed_height(1))
            .child(Button::new("Settings", |s| {
                settings(s);
            }))
            .child(Button::new("About", |s| {
                s.add_layer(
                    Dialog::info(format!(
                        "\nOdaRCON {}\nCopyright © 2026 smth idk\nLicensed under the GPLv2+",
                        env!("CARGO_PKG_VERSION")
                    ))
                    .title("About")
                    .h_align(HAlign::Center),
                )
            })),
    );

    let servers = Panel::new(server_list(siv)).title("Servers");

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

fn rebuild_server_list(siv: &mut Cursive) {
    let server_names: Vec<String> = siv
        .user_data::<AppState>()
        .unwrap()
        .config
        .servers
        .iter()
        .map(|s| s.name.clone())
        .collect();

    siv.call_on_name("server_list", |list: &mut SelectView<usize>| {
        list.clear();
        for (i, name) in server_names.iter().enumerate() {
            list.add_item(name, i);
        }
    });
}

fn server_list(siv: &mut Cursive) -> impl cursive::View {
    let mut servers = SelectView::new();
    siv.with_user_data(|state: &mut AppState| {
        for (i, server) in state.config.servers.iter().enumerate() {
            servers.add_item(&server.name, i);
        }
    });
    servers.set_on_submit(|s, server_id| {
        let server = s
            // TODO: can we do this without cloning?
            .with_user_data(|state: &mut AppState| state.config.servers[*server_id].clone())
            .unwrap();
        let server_id = *server_id;
        let modes = Dialog::around(
            LinearLayout::vertical()
                .child(Button::new("Connect", move |s| {
                    s.pop_layer();
                    rcon_layer(s, &server.host, server.port, &server.password);
                }))
                .child(Button::new("Edit", move |s| {
                    s.pop_layer(); // todo: maybe only pop this after choosing save in the edit dialog?
                    edit_server(s, "Edit Server", Some(server_id));
                }))
                .child(Button::new("Delete", move |s| {
                    let areyousure = Dialog::text("Are you sure you want to delete this server?")
                        .title("Delete Server")
                        .dismiss_button("No")
                        .button("Yes", move |s| {
                            // TODO: figure out difference between this and with_user_data
                            let config = &mut s.user_data::<AppState>().unwrap().config;
                            config.servers.remove(server_id);
                            if let Err(e) = config.save() {
                                // TODO: make the popup more informative
                                error_popup("Config file could not be saved", s);
                                log::error!("Config file could not be saved: {e}");
                            }
                            s.pop_layer();
                            s.pop_layer();
                            s.call_on_name("server_list", |list: &mut SelectView| {
                                // todo dont just unwrap
                                list.remove_item(server_id);
                            });
                            rebuild_server_list(s);
                        })
                        .padding_top(1);
                    s.add_layer(areyousure);
                })),
        )
        .dismiss_button("Cancel")
        .title(&server.name)
        .padding_top(1);
        s.add_layer(modes);
    });
    let servers = Panel::new(servers.with_name("server_list").scrollable());
    LinearLayout::vertical()
        .child(
            LinearLayout::horizontal()
                .child(DummyView.fixed_width(1))
                .child(Button::new("New Server", |s| {
                    edit_server(s, "New Server", None);
                })),
        )
        .child(servers)
}

fn settings(siv: &mut Cursive) {
    let config = &siv.user_data::<AppState>().unwrap().config; // TODO: no unwrap pls, maybe move to its own get_config function
    let mut settings = ListView::new();
    settings.add_child(
        "Colorize server log",
        Checkbox::new()
            .with_checked(config.colorize_logs)
            .with_name("colorize_logs"),
    );
    siv.add_layer(
        Dialog::around(settings)
            .padding_top(1)
            .title("Settings")
            .dismiss_button("Cancel")
            .button("Save", |s| {
                let colorize = s
                    .call_on_name("colorize_logs", |v: &mut Checkbox| v.is_checked())
                    .unwrap();
                if let Some(Err(e)) = s.with_user_data(|state: &mut AppState| {
                    state.config.colorize_logs = colorize;
                    state.config.save()
                }) {
                    // TODO: make the popup more informative
                    error_popup("Config file could not be saved", s);
                    log::error!("Config file could not be saved: {e}");
                } else {
                    s.pop_layer();
                }
            }),
    );
}

fn edit_server(siv: &mut Cursive, title: &str, server_index: Option<usize>) {
    // if let Some(server_index) = server_index {
    //     let server = siv
    //         .with_user_data(|state: &mut AppState| state.config.servers[server_index].clone())
    //         .unwrap();
    // }
    let (init_name, init_host, init_port, init_pass, init_proto) = if let Some(index) = server_index
    {
        let state = siv.user_data::<AppState>().unwrap();
        let server = &state.config.servers[index];
        (
            server.name.clone(),
            server.host.clone(),
            server.port.to_string(),
            server.password.clone(),
            server.protoversion,
        )
    } else {
        (
            // TODO: impl Default for ServerConfig?
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            config::ProtocolVersion::Latest,
        )
    };

    let mut server_settings = ListView::new();
    server_settings.add_child(
        "Name:",
        EditView::new().content(init_name).with_name("server_name"),
    );
    server_settings.add_child(
        "Hostname:",
        EditView::new()
            .content(init_host)
            .with_name("server_hostname"),
    );
    server_settings.add_child(
        "Port (optional):",
        EditView::new()
            .content(init_port)
            .on_edit(|s, content, _| filter_port("server_port", s, content))
            .with_name("server_port"),
    );
    server_settings.add_child(
        "Password:",
        EditView::new().secret().with_name("server_password"),
    );
    // TODO: get the labels from to_string or something on the versions
    let mut protocol_versions = SelectView::new().popup().with_all(vec![
        ("Latest (1.0.0)", config::ProtocolVersion::Latest),
        (
            "1.0.0",
            config::ProtocolVersion::Custom {
                major: 1,
                minor: 0,
                revision: 0,
            },
        ),
    ]);
    let proto_index = protocol_versions
        .iter()
        .position(|(_, proto)| *proto == init_proto);
    if let Some(proto_index) = proto_index {
        protocol_versions.set_selection(proto_index);
    }
    server_settings.add_child(
        "Protocol Version:",
        protocol_versions.with_name("protocol_version"),
    );

    let edit_dialog = Dialog::around(server_settings)
        .title(title)
        .dismiss_button("Cancel")
        .button("Save", move |s| {
            let name = s.call_on_name("server_name", |v: &mut EditView| v.get_content());
            let hostname = s.call_on_name("server_hostname", |v: &mut EditView| v.get_content());
            let port = s.call_on_name("server_port", |v: &mut EditView| v.get_content());
            let password = s
                .call_on_name("server_password", |v: &mut EditView| v.get_content())
                // TODO: don't just unwrap here
                .unwrap();
            let password = if password.is_empty() {
                init_pass.clone()
            } else {
                password.to_string()
            };
            let protocol = s.call_on_name(
                "protocol_version",
                |v: &mut SelectView<config::ProtocolVersion>| {
                    let id = v.selected_id();
                    match id {
                        Some(id) => *v.get_item(id).unwrap().1,
                        None => config::ProtocolVersion::Latest,
                    }
                },
            );
            if let Some(port) = verify_port(&port.unwrap(), s) {
                let server = ServerConfig {
                    // TODO: dont just do unwraps
                    name: name.unwrap().to_string(),
                    host: hostname.unwrap().to_string(),
                    port,
                    password,
                    protoversion: protocol.unwrap(),
                };
                if let Some(Err(e)) = s.with_user_data(|state: &mut AppState| {
                    // TODO: make sure the main server list gets updated
                    match server_index {
                        Some(index) => state.config.servers[index] = server,
                        None => state.config.add_server(server),
                    }
                    state.config.save()
                }) {
                    // TODO: make the popup more informative
                    error_popup("Config file could not be saved", s);
                    log::error!("Config file could not be saved: {e}");
                } else {
                    s.pop_layer();
                }
                rebuild_server_list(s);
            }
        })
        .min_width(56);
    siv.add_layer(edit_dialog);
}

fn rcon_layer(siv: &mut Cursive, hostname: &str, port: u16, password: &str) {
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

    let console_view = LinearLayout::horizontal()
        .child(left_pane)
        .child(right_panel);

    // fn update_left_max_width(s: &mut Cursive) {
    //     let term_width = s.screen_size().x;
    //     let right_width = 18;
    //     if term_width > right_width {
    //         let max_left = term_width - right_width;
    //         s.call_on_name("left", |v: &mut ResizedView<LinearLayout>| {
    //             v.set_width(SizeConstraint::AtMost(max_left));
    //         });
    //         s.call_on_name("output", |v: &mut TextView| {
    //             v.append(format!("> new width: {}\n", max_left));
    //         });
    //     }
    // }

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
    //
    let layer = OnEventView::new(console_view)
        .on_event('/', |s| match s.focus_name("input") {
            Ok(cb) => cb.process(s),
            Err(_) => error_popup("Console input could not be focused", s),
        })
        .on_event(Key::Esc, |s| match s.focus_name("button1") {
            Ok(cb) => cb.process(s),
            Err(_) => error_popup("Button 1 could not be focused", s),
        });

    siv.add_fullscreen_layer(layer);

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
