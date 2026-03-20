use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModrinthIndex {
    pub formatVersion: u32,
    pub game: String, // "minecraft"
    pub versionId: String,
    pub name: String,
    pub summary: Option<String>,
    pub files: Vec<ModpackFile>,
    pub dependencies: HashMap<String, String>, // e.g. "minecraft": "1.20.1", "fabric-loader": "0.14.21"
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModpackFile {
    pub path: String, // "mods/fabric-api.jar"
    pub hashes: HashMap<String, String>, // "sha1", "sha512"
    pub env: Option<EnvSupport>,
    pub downloads: Vec<String>, // URLs
    pub fileSize: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EnvSupport {
    pub client: String, // "required", "optional", "unsupported"
    pub server: String,
}
