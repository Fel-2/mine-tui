use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionInfo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String, // "release", "snapshot", "old_beta", etc.
    pub url: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}

// Structures for the specific version details (client.json)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionMetadata {
    pub id: String,
    pub libraries: Vec<Library>,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub downloads: VersionDownloads,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndex,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    #[serde(rename = "totalSize")]
    pub total_size: u64,
    pub url: String,
}

// The actual content of the asset index file (indexes/1.20.json)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AssetsManifest {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionDownloads {
    pub client: DownloadArtifact,
    // server, windows_server, etc.
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DownloadArtifact {
    pub sha1: String,
    pub size: u64,
    pub url: String,
    pub path: Option<String>, // Added path field
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>, 
    // rules, natives, etc. need to be handled for full implementation
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LibraryDownloads {
    pub artifact: Option<DownloadArtifact>,
    pub classifiers: Option<HashMap<String, DownloadArtifact>>,
}

pub async fn fetch_manifest() -> Result<VersionManifest, reqwest::Error> {
    let client = reqwest::Client::new();
    let resp = client.get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send()
        .await?
        .json::<VersionManifest>()
        .await?;
    Ok(resp)
}

pub async fn fetch_version_metadata(url: &str) -> Result<VersionMetadata, reqwest::Error> {
    let client = reqwest::Client::new();
    let resp = client.get(url)
        .send()
        .await?
        .json::<VersionMetadata>()
        .await?;
    Ok(resp)
}
