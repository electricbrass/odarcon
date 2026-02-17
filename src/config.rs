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

use crate::protocol;
use crate::protocol::PrintLevel;
use cursive::theme::BaseColor;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;
use thiserror::Error;

type CursiveColor = cursive::theme::Color;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("No config directory found")]
    NoConfigDir,
    #[error("Config file io error: {0}")]
    FileError(#[from] std::io::Error),
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Failed to serialize config file: {0}")]
    SerializeError(#[from] toml::ser::Error),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProtocolVersion {
    Latest,
    Custom { major: u8, minor: u8, revision: u8 },
}

impl Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolVersion::Latest => write!(
                f,
                "latest ({}.{}.{})",
                protocol::LATEST_PROTOCOL_VERSION.major,
                protocol::LATEST_PROTOCOL_VERSION.minor,
                protocol::LATEST_PROTOCOL_VERSION.revision
            ),
            // TODO: make this display like "1.0.0 (Odamex 12.2.0)"
            // wait until an actual odamex release supports it tho lol
            // need a map somewhere to get that info from
            ProtocolVersion::Custom {
                major,
                minor,
                revision,
            } => write!(f, "{}.{}.{}", major, minor, revision),
        }
    }
}

impl From<ProtocolVersion> for protocol::ProtocolVersion {
    fn from(version: ProtocolVersion) -> protocol::ProtocolVersion {
        match version {
            ProtocolVersion::Latest => protocol::LATEST_PROTOCOL_VERSION,
            ProtocolVersion::Custom {
                major,
                minor,
                revision,
            } => protocol::ProtocolVersion {
                major,
                minor,
                revision,
            },
        }
    }
}

impl Serialize for ProtocolVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ProtocolVersion::Latest => serializer.serialize_str("latest"),
            ProtocolVersion::Custom {
                major,
                minor,
                revision,
            } => serializer.serialize_str(&format!("{}.{}.{}", major, minor, revision)),
        }
    }
}

impl<'a> Deserialize<'a> for ProtocolVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let s = String::deserialize(deserializer)?;
        if s == "latest" {
            Ok(ProtocolVersion::Latest)
        } else {
            let parts: Vec<&str> = s.split('.').collect();
            if parts.len() != 3 {
                return Err(serde::de::Error::custom(
                    "Expected format 'major.minor.revision'",
                ));
            }

            let major = parts[0]
                .parse::<u8>()
                .map_err(|_| serde::de::Error::custom("Invalid major version"))?;
            let minor = parts[1]
                .parse::<u8>()
                .map_err(|_| serde::de::Error::custom("Invalid minor version"))?;
            let revision = parts[2]
                .parse::<u8>()
                .map_err(|_| serde::de::Error::custom("Invalid revision version"))?;

            Ok(ProtocolVersion::Custom {
                major,
                minor,
                revision,
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub password: String,
    pub protoversion: ProtocolVersion,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "".to_string(),
            host: "".to_string(),
            port: 10666,
            password: "".to_string(),
            protoversion: ProtocolVersion::Latest,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Color(pub CursiveColor);

impl From<CursiveColor> for Color {
    fn from(color: CursiveColor) -> Self {
        Color(color)
    }
}

impl From<Color> for CursiveColor {
    fn from(color: Color) -> Self {
        color.0
    }
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let basecolor_str = |c: BaseColor| match c {
            BaseColor::Black => "black",
            BaseColor::Red => "red",
            BaseColor::Green => "green",
            BaseColor::Yellow => "yellow",
            BaseColor::Blue => "blue",
            BaseColor::Magenta => "magenta",
            BaseColor::Cyan => "cyan",
            BaseColor::White => "white",
        };

        match self.0 {
            CursiveColor::TerminalDefault => serializer.serialize_str("default"),
            CursiveColor::Rgb(r, g, b) => {
                serializer.serialize_str(&format!("#{:02X}{:02X}{:02X}", r, g, b))
            }
            CursiveColor::RgbLowRes(r, g, b) => {
                serializer.serialize_str(&format!("#{:X}{:X}{:X}", r, g, b))
            }
            CursiveColor::Light(basecolor) => {
                serializer.serialize_str(&format!("light {}", basecolor_str(basecolor)))
            }
            CursiveColor::Dark(basecolor) => serializer.serialize_str(basecolor_str(basecolor)),
        }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let color = CursiveColor::from_str(&s)
            .map_err(|e| serde::de::Error::custom(format!("Invalid color: {e}")))?;
        Ok(Color(color))
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub colorize_logs: bool,
    pub servers: Vec<ServerConfig>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub logcolors: HashMap<PrintLevel, Color>,
}

impl Config {
    pub fn config_dir() -> Option<std::path::PathBuf> {
        ProjectDirs::from("net", "odamex", "odarcon").map(|dirs| dirs.config_dir().to_path_buf())
    }

    pub fn load() -> Result<Self, ConfigError> {
        let config_dir = Self::config_dir().ok_or(ConfigError::NoConfigDir)?;
        let config_path = config_dir.join("config.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let config_str = std::fs::read_to_string(config_path)?;
        let config: Self = toml::from_str::<Self>(&config_str)?;

        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let config_dir = Self::config_dir().ok_or(ConfigError::NoConfigDir)?;
        let config_path = config_dir.join("config.toml");

        std::fs::create_dir_all(&config_dir)?;

        let config_str = toml::to_string_pretty(self)?;
        std::fs::write(config_path, config_str)?;

        Ok(())
    }

    pub fn new() -> Self {
        Self {
            colorize_logs: false,
            servers: Vec::new(),
            // TODO: maybe do something different so that if a user doesnt change the colors
            // an old config doesnt leave them with old colors if they change in an update
            logcolors: toml::from_str(include_str!("../res/logcolors.toml")).unwrap(),
        }
    }

    pub fn add_server(&mut self, server: ServerConfig) {
        self.servers.push(server);
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_config() {
        let toml_config = toml::toml! {
            colorize_logs = false
            servers = []
        };
        let empty_config = Config::new();
        let parsed_config =
            toml::from_str::<Config>(&toml_config.to_string()).expect("Failed to parse config");
        assert_eq!(parsed_config, empty_config);
    }

    #[test]
    fn parse_config() {
        let toml_config = toml::toml! {
            colorize_logs = true
            [[servers]]
            name = "A cool server"
            host = "1.2.3.4"
            port = 10666
            password = "verysecure"
            protoversion = "latest"

            [[servers]]
            name = "Another cool server"
            host = "1.2.3.4"
            port = 10667
            password = "password"
            protoversion = "1.0.0"

            [logcolors]
            error = "#FF0000"
        };
        let config = Config {
            colorize_logs: true,
            servers: vec![
                ServerConfig {
                    name: "A cool server".to_string(),
                    host: "1.2.3.4".to_string(),
                    port: 10666,
                    password: "verysecure".to_string(),
                    protoversion: ProtocolVersion::Latest,
                },
                ServerConfig {
                    name: "Another cool server".to_string(),
                    host: "1.2.3.4".to_string(),
                    port: 10667,
                    password: "password".to_string(),
                    protoversion: ProtocolVersion::Custom {
                        major: 1,
                        minor: 0,
                        revision: 0,
                    },
                },
            ],
            logcolors: HashMap::from([(PrintLevel::Error, Color(CursiveColor::Rgb(255, 0, 0)))]),
        };
        let parsed_config =
            toml::from_str::<Config>(&toml_config.to_string()).expect("Failed to parse config");
        assert_eq!(parsed_config, config);
    }

    #[test]
    fn parse_config_missing_name() {
        let toml_config = toml::toml! {
            colorize_logs = false
            [[servers]]
                host = "1.2.3.4"
                port = 10667
                password = "password"
                protoversion = "1.0.0"
        };
        let parsed_config = toml::from_str::<Config>(&toml_config.to_string());
        assert!(parsed_config.is_err());
    }

    #[test]
    fn parse_config_missing_host() {
        let toml_config = toml::toml! {
            colorize_logs = false
            [[servers]]
                name = "Another cool server"
                port = 10667
                password = "password"
                protoversion = "1.0.0"
        };
        let parsed_config = toml::from_str::<Config>(&toml_config.to_string());
        assert!(parsed_config.is_err());
    }

    #[test]
    fn parse_config_missing_port() {
        let toml_config = toml::toml! {
            colorize_logs = false
            [[servers]]
                name = "Another cool server"
                host = "1.2.3.4"
                password = "password"
                protoversion = "1.0.0"
        };
        let parsed_config = toml::from_str::<Config>(&toml_config.to_string());
        assert!(parsed_config.is_err());
    }

    #[test]
    fn parse_config_missing_password() {
        let toml_config = toml::toml! {
            colorize_logs = false
            [[servers]]
                name = "Another cool server"
                host = "1.2.3.4"
                port = 10667
                protoversion = "1.0.0"
        };
        let parsed_config = toml::from_str::<Config>(&toml_config.to_string());
        assert!(parsed_config.is_err());
    }

    #[test]
    fn parse_config_missing_protoversion() {
        let toml_config = toml::toml! {
            colorize_logs = false
            [[servers]]
                name = "Another cool server"
                host = "1.2.3.4"
                port = 10667
                password = "password"
        };
        let parsed_config = toml::from_str::<Config>(&toml_config.to_string());
        assert!(parsed_config.is_err());
    }

    #[test]
    fn parse_config_bad_colorn() {
        let toml_config = toml::toml! {
            colorize_logs = false
            servers = []
            [logcolors]
            what = "red"
        };
        let parsed_config = toml::from_str::<Config>(&toml_config.to_string());
        assert!(parsed_config.is_err());
        let toml_config = toml::toml! {
            colorize_logs = false
            servers = []
            [logcolors]
            error = "1234567"
        };
        let parsed_config = toml::from_str::<Config>(&toml_config.to_string());
        assert!(parsed_config.is_err());
    }

    #[test]
    fn color_conversion() {
        let curcolor = CursiveColor::Dark(BaseColor::Red);
        let mycolor = Color(curcolor.clone());
        assert_eq!(curcolor, mycolor.clone().into());
        assert_eq!(Color::from(curcolor), mycolor.clone());
        assert_eq!(mycolor, curcolor.into());
        assert_eq!(CursiveColor::from(mycolor), curcolor);
    }
}
