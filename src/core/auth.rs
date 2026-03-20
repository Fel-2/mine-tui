use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MinecraftLoginResponse {
    pub access_token: String,
    pub username: Option<String>, // Sometimes not in response, need profile fetch
}

#[derive(Debug, Deserialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
}

// Ely.by / Yggdrasil
#[derive(Debug, Serialize)]
struct YggdrasilAuthRequest {
    agent: Agent,
    username: String,
    password: String,
    #[serde(rename = "clientToken")]
    client_token: String,
    #[serde(rename = "requestUser")]
    request_user: bool,
}

#[derive(Debug, Serialize)]
struct Agent {
    name: String,
    version: u32,
}

#[derive(Debug, Deserialize)]
struct YggdrasilAuthResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "clientToken")]
    client_token: String,
    #[serde(rename = "selectedProfile")]
    selected_profile: Profile,
}

#[derive(Debug, Deserialize)]
struct Profile {
    id: String,
    name: String,
}

pub async fn start_microsoft_auth_flow(client: &Client) -> Result<DeviceCodeResponse, String> {
    let params = [
        ("client_id", "00000000-402b-9631-3959-f52ef9304b6d"), // Standard Minecraft Launcher Client ID
        ("scope", "XboxLive.Signin offline_access"),
    ];

    let res = client.post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("Failed to get device code: {}", res.status()));
    }

    res.json::<DeviceCodeResponse>().await.map_err(|e| e.to_string())
}

pub async fn poll_microsoft_token(client: &Client, device_code: &str) -> Result<TokenResponse, String> {
    let params = [
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ("client_id", "00000000-402b-9631-3959-f52ef9304b6d"),
        ("device_code", device_code),
    ];

    let res = client.post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    
    if let Some(err) = body.get("error") {
        return Ok(TokenResponse {
            access_token: "".to_string(),
            refresh_token: None,
            error: Some(err.as_str().unwrap_or("unknown_error").to_string()),
        });
    }

    let access_token = body.get("access_token")
        .ok_or("No access token in response")?
        .as_str().unwrap().to_string();
        
    let refresh_token = body.get("refresh_token")
        .map(|v| v.as_str().unwrap().to_string());

    Ok(TokenResponse {
        access_token,
        refresh_token,
        error: None,
    })
}

pub async fn authenticate_minecraft_xbox(client: &Client, ms_access_token: &str) -> Result<(String, String, String), String> {
    // 1. XBL Auth
    let xbl_body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={}", ms_access_token)
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    
    let xbl_res = client.post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&xbl_body)
        .send().await.map_err(|e| e.to_string())?;
    let xbl_data: serde_json::Value = xbl_res.json().await.map_err(|e| e.to_string())?;
    let xbl_token = xbl_data["Token"].as_str().ok_or("Failed to get XBL token")?;
    let uhs = xbl_data["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or("Failed to get UHS")?;

    // 2. XSTS Auth
    let xsts_body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    
    let xsts_res = client.post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&xsts_body)
        .send().await.map_err(|e| e.to_string())?;
        
    if xsts_res.status() == 401 {
        return Err("Xbox account does not exist or is not verified.".to_string());
    }
    
    let xsts_data: serde_json::Value = xsts_res.json().await.map_err(|e| e.to_string())?;
    let xsts_token = xsts_data["Token"].as_str().ok_or("Failed to get XSTS token")?;

    // 3. Minecraft Auth
    let mc_body = serde_json::json!({
        "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token)
    });
    
    let mc_res = client.post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&mc_body)
        .send().await.map_err(|e| e.to_string())?;
    let mc_data: serde_json::Value = mc_res.json().await.map_err(|e| e.to_string())?;
    let mc_token = mc_data["access_token"].as_str().ok_or("Failed to get MC token")?;

    // 4. Get Profile
    let profile_res = client.get("https://api.minecraftservices.com/minecraft/profile")
        .header("Authorization", format!("Bearer {}", mc_token))
        .send().await.map_err(|e| e.to_string())?;
    let profile: MinecraftProfile = profile_res.json().await.map_err(|e| e.to_string())?;

    Ok((mc_token.to_string(), profile.name, profile.id))
}

// Overloaded return type for simplicity in implementation plan, but actually need struct
pub async fn authenticate_ely_by(client: &Client, username: &str, password: &str) -> Result<(String, String, String), String> {
    let body = YggdrasilAuthRequest {
        agent: Agent { name: "Minecraft".to_string(), version: 1 },
        username: username.to_string(),
        password: password.to_string(),
        client_token: "minetui-client".to_string(),
        request_user: true,
    };

    let res = client.post("https://authserver.ely.by/auth/authenticate")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("Ely.by Login Failed: {}", res.status()));
    }

    let data: YggdrasilAuthResponse = res.json().await.map_err(|e| e.to_string())?;
    Ok((data.access_token, data.selected_profile.name, data.selected_profile.id))
}

pub fn generate_offline_uuid(username: &str) -> String {
    let hash = md5::compute(format!("OfflinePlayer:{}", username));
    let mut bytes = hash.0;
    // Set version to 3
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    // Set variant to IETF
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    
    let uuid = uuid::Builder::from_bytes(bytes).into_uuid();
    uuid.to_string()
}
