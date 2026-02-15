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

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use toml;

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

// TODO: implement into for config::protocolversion to protocol::protocolversion
// custom is easy, latest needs a constant to represent the latest

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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub colorize_logs: bool,
    pub servers: Vec<ServerConfig>,
}

impl Config {
    fn config_dir() -> Option<std::path::PathBuf> {
        ProjectDirs::from("net", "odamex", "odarcon").map(|dirs| dirs.config_dir().to_path_buf())
    }

    pub fn load() -> Result<Self, ConfigError> {
        let config_dir = Self::config_dir().ok_or(ConfigError::NoConfigDir)?;
        let config_path = config_dir.join("config.toml");

        if !config_path.exists() {
            return Ok(Self::new());
        }

        let config_str = std::fs::read_to_string(config_path)?;
        let config: Self = toml::from_str::<Self>(&config_str)?;

        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let config_dir = Self::config_dir().ok_or(ConfigError::NoConfigDir)?;
        let config_path = config_dir.join("config.toml");

        std::fs::create_dir_all(&config_dir)?;

        let config_str = toml::to_string_pretty(self)?; // need to actually return error here
        std::fs::write(config_path, config_str)?;

        Ok(())
    }

    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            colorize_logs: false,
        }
    }

    pub fn add_server(&mut self, server: ServerConfig) {
        self.servers.push(server);
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
}
