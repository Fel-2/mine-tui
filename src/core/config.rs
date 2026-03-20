use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::core::fs::get_data_dir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuthType {
    Offline,
    Microsoft,
    ElyBy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthData {
    pub auth_type: AuthType,
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub refresh_token: Option<String>, // For Microsoft/Ely.by
}

impl Default for AuthData {
    fn default() -> Self {
        Self {
            auth_type: AuthType::Offline,
            username: "Steve".to_string(),
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            access_token: "0".to_string(),
            refresh_token: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub auth: AuthData,
    pub java_path: String,
    pub max_memory: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auth: AuthData::default(),
            java_path: "java".to_string(),
            max_memory: 4096,
        }
    }
}

pub fn get_config_path() -> PathBuf {
    get_data_dir().join("config.json")
}

pub fn load_config() -> Config {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }
    }
    Config::default()
}

pub fn save_config(config: &Config) -> Result<(), String> {
    let path = get_config_path();
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(path, content)
        .map_err(|e| format!("Failed to write config: {}", e))?;
    Ok(())
}
