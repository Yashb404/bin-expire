use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LastUsedSource {
    Accessed,
    Modified,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct BinaryInfo {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub accessed: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub last_used: SystemTime,
    pub last_used_source: LastUsedSource,
    pub is_symlink: bool,
}

// Placeholder for Configuration
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub ignored_bins: Vec<String>,
    pub default_threshold_days: i64,
    pub archive_path: PathBuf,
}