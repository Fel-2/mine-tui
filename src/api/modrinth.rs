use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SearchResponse {
    pub hits: Vec<SearchResult>,
    pub total_hits: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub slug: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub project_type: String,
    pub project_id: String, 
}

#[derive(Deserialize, Debug, Clone)]
pub struct ProjectVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<VersionFile>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
}

pub async fn search_modpacks(query: &str) -> Result<Vec<SearchResult>, reqwest::Error> {
    let client = reqwest::Client::new();
    // Modrinth requires a User-Agent: AppName/Version (Contact)
    let url = format!("https://api.modrinth.com/v2/search?query={}&facets=[[\"project_type:modpack\"]]&limit=20", query);
    
    let resp = client.get(&url)
        .header("User-Agent", "mine-tui/0.1.0 (opencode-generated)")
        .send()
        .await?
        .json::<SearchResponse>()
        .await?;
        
    Ok(resp.hits)
}

pub async fn fetch_project_versions(slug_or_id: &str) -> Result<Vec<ProjectVersion>, reqwest::Error> {
    let client = reqwest::Client::new();
    let url = format!("https://api.modrinth.com/v2/project/{}/version", slug_or_id);
    
    let resp = client.get(&url)
        .header("User-Agent", "mine-tui/0.1.0 (opencode-generated)")
        .send()
        .await?
        .json::<Vec<ProjectVersion>>()
        .await?;
        
    Ok(resp)
}
