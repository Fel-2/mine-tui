use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub name: String, // Display Name
    pub id: String,   // Folder Name
    pub version: String,
    pub loader: String, 
    pub max_memory: u32, 
    pub java_path: String, 
    pub played_last: Option<String>,
}
