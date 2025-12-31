use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BinaryInfo {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub last_accessed: std::time::SystemTime,
    pub is_symlink: bool,
}

// Placeholder for Configuration
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub ignored_bins: Vec<String>,
    pub archive_path: PathBuf,
}