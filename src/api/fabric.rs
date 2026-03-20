use serde::{Deserialize, Serialize};
use crate::core::versions::{Library, VersionDownloads};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FabricProfile {
    pub id: String,
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub libraries: Vec<Library>,
    // Fabric doesn't usually provide "downloads" for the jar in this JSON, 
    // the jar comes from the libraries list (fabric-loader).
}

pub async fn fetch_fabric_profile(game_version: &str, loader_version: &str) -> Result<FabricProfile, reqwest::Error> {
    let client = reqwest::Client::new();
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json", game_version, loader_version);
    
    let resp = client.get(&url)
        .send()
        .await?
        .json::<FabricProfile>()
        .await?;
        
    Ok(resp)
}
