use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use toml;

#[derive(Debug, Deserialize, Serialize)]
pub enum ProtocolVersion {
    Latest,
    Custom(u8),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub protoversion: ProtocolVersion,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "".to_string(),
            port: 10666,
            password: "".to_string(),
            protoversion: ProtocolVersion::Latest,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub servers: Vec<ServerConfig>,
}

impl Config {
    fn config_dir() -> Option<std::path::PathBuf> {
        ProjectDirs::from("net", "odamex", "odarcon").map(|dirs| dirs.config_dir().to_path_buf())
    }

    pub fn load() -> Result<Self, ()> {
        let config_dir = Self::config_dir().unwrap(); // need to actually return error here
        let config_path = config_dir.join("config.toml");

        if !config_path.exists() {
            return Ok(Self::new());
        }

        let config_str = std::fs::read_to_string(config_path).expect("sdfsdf"); // need to actually return error here
        let config: Self = toml::from_str::<Self>(&config_str).expect("sdfds"); // need to actually return error here

        Ok(config)
    }

    pub fn save(&self) -> Result<(), ()> {
        let config_dir = Self::config_dir().unwrap(); // need to actually return error here
        let config_path = config_dir.join("config.toml");

        let config_str = toml::to_string_pretty(self).expect("sdfds"); // need to actually return error here
        std::fs::write(config_path, config_str).expect("sdfds"); // need to actually return error here

        Ok(())
    }

    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
        }
    }

    pub fn add_server(&mut self, server: ServerConfig) {
        self.servers.push(server);
    }
}
