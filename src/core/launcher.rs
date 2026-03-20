
use std::path::PathBuf;
use crate::core::{fs, versions};
use zip::ZipArchive;
use std::fs::File;
use std::io::Write;
use std::collections::HashSet;

use crate::core::config::{AuthData, AuthType};
use crate::core::installer;

pub async fn launch_instance(instance_name: &str, version_id: &str, memory_mb: u32, java_path: &str, auth_data: &AuthData) -> Result<tokio::process::Child, String> {
    // 1. Paths
    let instance_dir = fs::create_instance_dir(instance_name)
         .map_err(|e| format!("Failed to get instance dir: {}", e))?;
    let metadata_path = instance_dir.join("client.json");
    let libraries_dir = fs::get_data_dir().join("libraries");
    let assets_dir = fs::get_data_dir().join("assets");
    let natives_dir = instance_dir.join("natives");

    // 2. Load Metadata
    let file = std::fs::File::open(&metadata_path)
        .map_err(|e| format!("Failed to open metadata: {}", e))?;
    let metadata: versions::VersionMetadata = serde_json::from_reader(file)
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;

    // 3. Extract Natives & Build Classpath
    if !natives_dir.exists() {
        std::fs::create_dir_all(&natives_dir)
            .map_err(|e| format!("Failed to create natives dir: {}", e))?;
    }

    let mut classpath_entries = Vec::new();
    
    for lib in &metadata.libraries {
        // Check for Natives (Vanilla)
        if let Some(downloads) = &lib.downloads {
            // Add main artifact to classpath
            if let Some(artifact) = &downloads.artifact {
                if let Some(path) = &artifact.path {
                    let lib_path = libraries_dir.join(path);
                    if lib_path.exists() {
                         classpath_entries.push(lib_path.to_string_lossy().to_string());
                    } else {
                        println!("WARNING: Skipping missing library: {}", lib_path.to_string_lossy());
                    }
                }
            }
            
            // Handle Natives Extraction
            if let Some(classifiers) = &downloads.classifiers {
                // We assume Linux for this environment
                if let Some(native_artifact) = classifiers.get("natives-linux") {
                     if let Some(path) = &native_artifact.path {
                         let lib_path = libraries_dir.join(path);
                         if lib_path.exists() {
                             extract_native(&lib_path, &natives_dir)?;
                         }
                     }
                }
            }
        } 
        // Fabric / Maven Libraries (3-part: group:artifact:version, or 4-part with classifier)
        else {
            let parts: Vec<&str> = lib.name.split(':').collect();
            if parts.len() >= 3 {
                let group = parts[0].replace('.', "/");
                let artifact = parts[1];
                let version = parts[2];

                // 4-part names with classifier (e.g. :natives-linux) — natives only, skip classpath
                if parts.len() > 3 {
                    let classifier = parts[3];
                    let filename = format!("{}-{}-{}.jar", artifact, version, classifier);
                    let rel_path = format!("{}/{}/{}/{}", group, artifact, version, filename);
                    let native_path = libraries_dir.join(&rel_path);
                    if native_path.exists() {
                        extract_native(&native_path, &natives_dir)?;
                    }
                } else {
                    // 3-part — regular library jar
                    let filename = format!("{}-{}.jar", artifact, version);
                    let rel_path = format!("{}/{}/{}/{}", group, artifact, version, filename);
                    let lib_path = libraries_dir.join(&rel_path);
                    if lib_path.exists() {
                        classpath_entries.push(lib_path.to_string_lossy().to_string());
                    }

                    // Also check for a natives-linux variant
                    let native_filename = format!("{}-{}-natives-linux.jar", artifact, version);
                    let native_rel_path = format!("{}/{}/{}/{}", group, artifact, version, native_filename);
                    let native_path = libraries_dir.join(&native_rel_path);
                    if native_path.exists() {
                        extract_native(&native_path, &natives_dir)?;
                    }
                }
            }
        }
    }
    
    // Client JAR
    let client_jar = instance_dir.join("client.jar");
    if client_jar.exists() {
        classpath_entries.push(client_jar.to_string_lossy().to_string());
    }

    // Deduplicate Classpath
    let mut unique_classpath = Vec::new();
    let mut seen = HashSet::new();
    for entry in classpath_entries {
        if seen.insert(entry.clone()) {
            unique_classpath.push(entry);
        }
    }
    
    let classpath = unique_classpath.join(":"); 

    // 4. Construct Arguments
    let mut args = Vec::new();
    
    // JVM Arguments
    args.push(format!("-Xmx{}M", memory_mb));
    args.push(format!("-Djava.library.path={}", natives_dir.to_string_lossy())); 
    // Wayland/Linux Fix: Prefer system GLFW if possible, or at least hint it
    // args.push("-Dorg.lwjgl.glfw.libname=glfw".to_string()); 
    
    // Authlib Injector for Ely.by
    if auth_data.auth_type == AuthType::ElyBy {
        let authlib_path = installer::ensure_authlib_injector().await?;
        args.push(format!("-javaagent:{}=https://authlib.ely.by", authlib_path.to_string_lossy()));
        // Required for Java 16+ (authlib-injector needs deep reflection access)
        args.push("--add-opens".to_string());
        args.push("java.base/java.net=ALL-UNNAMED".to_string());
        args.push("--add-exports".to_string());
        args.push("java.base/sun.security.util=ALL-UNNAMED".to_string());
    }

    args.push("-cp".to_string());
    args.push(classpath);
    args.push(metadata.main_class);
    
    // Game Arguments
    args.push("--version".to_string());
    args.push(version_id.to_string());
    args.push("--gameDir".to_string());
    args.push(instance_dir.to_string_lossy().to_string());
    args.push("--assetsDir".to_string());
    args.push(assets_dir.to_string_lossy().to_string());
    args.push("--assetIndex".to_string());
    args.push(metadata.asset_index.id.clone());
    args.push("--uuid".to_string());
    args.push(auth_data.uuid.clone());
    args.push("--accessToken".to_string());
    args.push(auth_data.access_token.clone());
    args.push("--userType".to_string());
    args.push("msa".to_string()); // or "mojang" or "legacy"
    args.push("--versionType".to_string());
    args.push("release".to_string());
    args.push("--username".to_string());
    args.push(auth_data.username.clone());

    // DEBUG: Write run command to file
    let debug_path = instance_dir.join("debug_launch.txt");
    if let Ok(mut f) = File::create(&debug_path) {
        let _ = writeln!(f, "Command: {} {}", java_path, args.join(" "));
    }

    // 5. Spawn Process with Log Redirection
    let log_path = instance_dir.join("latest.log");
    let log_file = File::create(&log_path).map_err(|e| format!("Failed to create log file: {}", e))?;
    let log_file_err = log_file.try_clone().map_err(|e| format!("Failed to clone log file handle: {}", e))?;

    let child = tokio::process::Command::new(java_path) // Use custom java path
        .args(&args)
        .current_dir(&instance_dir)
        .stdout(std::process::Stdio::from(log_file))
        .stderr(std::process::Stdio::from(log_file_err))
        .spawn()
        .map_err(|e| format!("Failed to start Java: {}", e))?;

    Ok(child)
}

fn extract_native(zip_path: &PathBuf, target_dir: &PathBuf) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("Failed to open native jar: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read native jar: {}", e))?;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("Zip error: {}", e))?;
        let path = file.mangled_name();
        
        if file.is_dir() || path.to_string_lossy().contains("META-INF") {
            continue;
        }
        
        let out_path = target_dir.join(path);
        if let Some(p) = out_path.parent() {
            if !p.exists() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
        }
        
        let mut outfile = File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
    }
    Ok(())
}
