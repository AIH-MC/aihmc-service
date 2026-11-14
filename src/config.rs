use std::{error::Error, path::Path};

use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    #[serde(default)]
    pub servers: Vec<Source>,
    #[serde(default)]
    pub extra_skin_domains: Vec<String>,
    #[serde(default)]
    pub server: ServerConfig,
    pub rsakey: KeyConfig,
    #[serde(default)]
    pub allow_offline: bool,
    #[serde(default = "default_check_auth_source_names")]
    pub check_auth_source_names: bool,
    #[serde(default)]
    pub figura_session: String,
    #[serde(default)]
    pub access_token: String,
}

fn default_check_auth_source_names() -> bool {
    true
}

#[derive(Clone, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub db_name: String,
}

#[derive(Clone, Deserialize)]
pub struct Source {
    pub name: String,
    pub root_url: String,
    pub api_type: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

impl Source {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub debug: bool,
    pub timeout: f64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 8000,
            debug: true,
            timeout: 5.0,
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct KeyConfig {
    pub public_keyfile: String,
    pub private_keyfile: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_preserves_configuration_values() {
        let config: Config = toml::from_str(include_str!("../config.example.toml")).unwrap();
        assert_eq!(config.server.port, 8002);
        assert_eq!(config.servers.len(), 4);
        assert_eq!(config.figura_session, "2d");
        assert!(config.allow_offline);
        assert!(config.check_auth_source_names);
        let example = include_str!("../config.example.toml");
        let legacy: Config =
            toml::from_str(&example.replace("check_auth_source_names = true", "")).unwrap();
        assert!(legacy.check_auth_source_names);
        let local_only: Config = toml::from_str(&example.replace(
            "check_auth_source_names = true",
            "check_auth_source_names = false",
        ))
        .unwrap();
        assert!(!local_only.check_auth_source_names);
        assert!(config.access_token.is_empty());
    }
}
