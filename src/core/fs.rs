use std::path::PathBuf;
use directories::ProjectDirs;
use std::fs;
use std::io;
use crate::core::instance::Instance;

pub fn get_data_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "minetui", "mine-tui") {
        proj_dirs.data_dir().to_path_buf()
    } else {
        PathBuf::from("mine-tui-data")
    }
}

pub fn get_instances_dir() -> PathBuf {
    let mut dir = get_data_dir();
    dir.push("instances");
    dir
}

pub fn create_instance_dir(name: &str) -> io::Result<PathBuf> {
    let mut dir = get_instances_dir();
    dir.push(name);
    
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    
    Ok(dir)
}

pub fn save_instances(instances: &[Instance]) -> io::Result<()> {
    let dir = get_data_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let path = dir.join("instances.json");
    let json = serde_json::to_string_pretty(instances)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_instances() -> io::Result<Vec<Instance>> {
    let path = get_data_dir().join("instances.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    let instances: Vec<Instance> = serde_json::from_str(&content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(instances)
}
