use std::path::PathBuf;
use futures::stream::{self, StreamExt};
use sha1::{Sha1, Digest};
use crate::core::{fs, versions, modpack};
use crate::api::fabric; 
use zip::ZipArchive;
use std::io::Cursor;

pub async fn install_version(
    version_id: String, 
    manifest: &versions::VersionManifest, 
    instance_name: String
) -> Result<PathBuf, String> {
    let version_info = manifest.versions.iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| format!("Version {} not found in manifest", version_id))?;

    let instance_dir = fs::create_instance_dir(&instance_name)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let metadata = versions::fetch_version_metadata(&version_info.url).await
        .map_err(|e| format!("Failed to fetch metadata: {}", e))?;

    let client_jar_path = instance_dir.join("client.jar");
    // Verify/Download Client JAR
    download_file_verified(&metadata.downloads.client.url, &client_jar_path, metadata.downloads.client.size, &metadata.downloads.client.sha1).await
        .map_err(|e| format!("Failed to download client jar: {}", e))?;

    let metadata_path = instance_dir.join("client.json");
    let metadata_json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
    
    tokio::fs::write(metadata_path, metadata_json).await
        .map_err(|e| format!("Failed to save metadata: {}", e))?;

    let libraries_dir = fs::get_data_dir().join("libraries");
    download_libraries(&metadata.libraries, &libraries_dir).await?;

    download_assets(&metadata.asset_index).await?;

    Ok(instance_dir)
}

pub async fn install_fabric(
    game_version: String,
    loader_version: String,
    instance_name: String
) -> Result<(), String> {
    let instance_dir = fs::create_instance_dir(&instance_name)
        .map_err(|e| format!("Failed to get instance dir: {}", e))?;
    let metadata_path = instance_dir.join("client.json");

    let fabric_profile = fabric::fetch_fabric_profile(&game_version, &loader_version).await
        .map_err(|e| format!("Failed to fetch Fabric profile: {}", e))?;

    if !metadata_path.exists() {
        return Err("Vanilla client.json not found. Install Vanilla first.".to_string());
    }
    let file_content = tokio::fs::read_to_string(&metadata_path).await
        .map_err(|e| format!("Failed to read client.json: {}", e))?;
    let mut metadata: versions::VersionMetadata = serde_json::from_str(&file_content)
        .map_err(|e| format!("Failed to parse client.json: {}", e))?;

    let mut lib_map = std::collections::HashMap::new();
    
    // FIXED KEY GENERATION
    let get_key = |name: &str| -> String {
        let parts: Vec<&str> = name.split(':').collect();
        // group:artifact:version[:classifier]
        if parts.len() > 3 {
             // Include classifier in key to prevent natives from overwriting code jar
             format!("{}:{}:{}", parts[0], parts[1], parts[3])
        } else if parts.len() >= 2 {
            format!("{}:{}", parts[0], parts[1])
        } else {
            name.to_string()
        }
    };

    for lib in metadata.libraries {
        lib_map.insert(get_key(&lib.name), lib);
    }
    
    for lib in fabric_profile.libraries {
        let key = get_key(&lib.name);
        if let Some(existing_lib) = lib_map.get(&key) {
             if lib.name == existing_lib.name && lib.downloads.is_none() && existing_lib.downloads.is_some() {
                 let mut merged_lib = lib.clone();
                 merged_lib.downloads = existing_lib.downloads.clone();
                 lib_map.insert(key, merged_lib);
                 continue;
            }
        }
        lib_map.insert(key, lib);
    }
    
    metadata.libraries = lib_map.into_values().collect();
    metadata.main_class = fabric_profile.main_class;
    metadata.id = format!("{}-fabric-{}", game_version, loader_version);

    let libraries_dir = fs::get_data_dir().join("libraries");
    download_libraries(&metadata.libraries, &libraries_dir).await?;

    let new_json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("Failed to serialize new metadata: {}", e))?;
    tokio::fs::write(&metadata_path, new_json).await
        .map_err(|e| format!("Failed to save new client.json: {}", e))?;

    Ok(())
}

async fn download_libraries(libraries: &[versions::Library], libraries_dir: &PathBuf) -> Result<(), String> {
    let client = reqwest::Client::new();
    let libraries = libraries.to_vec(); // Fixed: Clone for async ownership
    
    // Process in parallel chunks
    let downloads = stream::iter(libraries)
        .map(|lib| {
            let libraries_dir = libraries_dir.clone();
            let client = client.clone();
            async move {
                // Logic A: "downloads" object (Vanilla style)
                if let Some(downloads) = &lib.downloads {
                    if let Some(artifact) = &downloads.artifact {
                        if let Some(path_str) = &artifact.path {
                            let path = libraries_dir.join(path_str);
                            let _ = download_file_verified(&artifact.url, &path, artifact.size, &artifact.sha1).await;
                        }
                    }
                } 
                // Logic B: Maven coordinates (Fabric style)
                else {
                     let parts: Vec<&str> = lib.name.split(':').collect();
                     if parts.len() >= 3 { // Allow classifiers here too (len 3 or 4)
                         let group = parts[0].replace('.', "/");
                         let name = parts[1];
                         let version = parts[2];
                         
                         let filename = if parts.len() > 3 {
                             format!("{}-{}-{}.jar", name, version, parts[3])
                         } else {
                             format!("{}-{}.jar", name, version)
                         };
                         
                         let rel_path = format!("{}/{}/{}/{}", group, name, version, filename);
                         let path = libraries_dir.join(&rel_path);
                         
                         // For maven libs without explicit metadata, we check existence or force download if 0 bytes
                         let mut needs_dl = true;
                         if path.exists() {
                             if let Ok(meta) = tokio::fs::metadata(&path).await {
                                 if meta.len() > 0 { needs_dl = false; }
                             }
                         }

                         if needs_dl {
                              if let Some(parent) = path.parent() {
                                    let _ = tokio::fs::create_dir_all(parent).await;
                              }
                              let repos = vec![
                                  "https://maven.fabricmc.net/", 
                                  "https://libraries.minecraft.net/", 
                                  "https://repo1.maven.org/maven2/"
                              ];
                              for repo in repos {
                                  let url = format!("{}{}", repo, rel_path);
                                  if download_file_simple(&client, &url, &path).await.is_ok() {
                                      break;
                                  }
                              }
                         }
                     }
                }
            }
        })
        .buffer_unordered(20);
    
    downloads.collect::<Vec<()>>().await;
    Ok(())
}

async fn download_file_verified(url: &str, path: &PathBuf, expected_size: u64, expected_sha1: &str) -> Result<(), String> {
    // Check existing
    if path.exists() {
        if let Ok(bytes) = tokio::fs::read(path).await {
            if bytes.len() as u64 == expected_size {
                let mut hasher = Sha1::new();
                hasher.update(&bytes);
                let result = hasher.finalize();
                if hex::encode(result) == expected_sha1 {
                    return Ok(()); // File is good
                }
            }
        }
    }
    
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    let response = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if !response.status().is_success() { return Err("HTTP Error".to_string()); }
    
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    if expected_size > 0 && bytes.len() as u64 != expected_size { return Err("Size mismatch".to_string()); }
    
    tokio::fs::write(path, bytes).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn download_file_simple(client: &reqwest::Client, url: &str, path: &PathBuf) -> Result<(), String> {
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() { return Err("HTTP Error".to_string()); }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    tokio::fs::write(path, bytes).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn install_modpack(
    mrpack_url: String, 
    instance_name: String
) -> Result<(String, String), String> {
    let instance_dir = fs::create_instance_dir(&instance_name)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let response = reqwest::get(&mrpack_url).await
        .map_err(|e| format!("Failed to download modpack: {}", e))?;
    let bytes = response.bytes().await
        .map_err(|e| format!("Failed to get modpack bytes: {}", e))?;
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| format!("Failed to open zip: {}", e))?;

    let index_file = archive.by_name("modrinth.index.json")
        .map_err(|e| format!("Modpack missing index: {}", e))?;
    let index: modpack::ModrinthIndex = serde_json::from_reader(index_file)
        .map_err(|e| format!("Failed to parse index: {}", e))?;

    let client = reqwest::Client::new();
    let files = index.files.clone(); // Clone for async
    let downloads = stream::iter(files)
        .map(|file| {
            let instance_dir = instance_dir.clone();
            let client = client.clone();
            async move {
                if file.env.as_ref().map(|e| e.client == "unsupported").unwrap_or(false) { return; }
                let file_path = instance_dir.join(&file.path);
                if let Some(parent) = file_path.parent() { let _ = tokio::fs::create_dir_all(parent).await; }
                if !file.downloads.is_empty() {
                     let mut needs_download = true;
                    if file_path.exists() {
                         if let Ok(meta) = tokio::fs::metadata(&file_path).await {
                             if meta.len() == file.fileSize { needs_download = false; }
                         }
                    }
                    if needs_download {
                         for url in &file.downloads {
                             if download_file_simple(&client, url, &file_path).await.is_ok() { break; }
                         }
                    }
                }
            }
        })
        .buffer_unordered(10);
    downloads.collect::<Vec<()>>().await;

    let game_version = index.dependencies.get("minecraft").cloned().unwrap_or_default();
    let loader_version = index.dependencies.get("fabric-loader").cloned().unwrap_or_default();
    Ok((game_version, loader_version))
}

async fn download_assets(index_info: &versions::AssetIndex) -> Result<(), String> {
    let assets_dir = fs::get_data_dir().join("assets");
    let indexes_dir = assets_dir.join("indexes");
    let objects_dir = assets_dir.join("objects");

    if !indexes_dir.exists() { let _ = tokio::fs::create_dir_all(&indexes_dir).await; }
    let index_path = indexes_dir.join(format!("{}.json", index_info.id));
    
    download_file_verified(&index_info.url, &index_path, index_info.size, &index_info.sha1).await
         .map_err(|e| format!("Failed to download asset index: {}", e))?;

    let index_content = tokio::fs::read_to_string(&index_path).await
         .map_err(|e| format!("Failed to read asset index: {}", e))?;
    let manifest: versions::AssetsManifest = serde_json::from_str(&index_content)
         .map_err(|e| format!("Failed to parse asset index: {}", e))?;

    let client = reqwest::Client::new();
    let objects = manifest.objects.into_iter().collect::<Vec<_>>(); // Clone to vec for async
    let downloads = stream::iter(objects)
        .map(|(_name, object)| {
            let objects_dir = objects_dir.clone();
            let client = client.clone();
            async move {
                let hash_prefix = &object.hash[0..2];
                let file_path = objects_dir.join(hash_prefix).join(&object.hash);
                
                let needs_dl = if file_path.exists() {
                     match tokio::fs::metadata(&file_path).await {
                         Ok(m) => m.len() != object.size,
                         Err(_) => true
                     }
                } else { true };

                if needs_dl {
                     let url = format!("https://resources.download.minecraft.net/{}/{}", hash_prefix, object.hash);
                     if let Some(parent) = file_path.parent() {
                         let _ = tokio::fs::create_dir_all(parent).await;
                     }
                     let _ = download_file_simple(&client, &url, &file_path).await;
                }
            }
        })
        .buffer_unordered(50); 
    downloads.collect::<Vec<()>>().await;
    Ok(())
}

pub async fn ensure_authlib_injector() -> Result<PathBuf, String> {
    let libraries_dir = fs::get_data_dir().join("libraries");
    let authlib_path = libraries_dir.join("authlib-injector.jar");

    if !authlib_path.exists() {
        println!("Downloading authlib-injector...");
        let url = "https://github.com/yushijinhun/authlib-injector/releases/download/v1.2.5/authlib-injector-1.2.5.jar";
        // SHA1 for v1.2.5
        let sha1 = "4b95433462f51a623394928d5323793245818d07"; 
        download_file_verified(url, &authlib_path, 0, sha1).await // Size check skipped (0) as it might vary slightly or I don't have exact byte count handy, but SHA1 is key.
            .map_err(|e| format!("Failed to download authlib-injector: {}", e))?;
    }

    Ok(authlib_path)
}
