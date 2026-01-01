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
    pub _is_symlink: bool,
}

// Placeholder for Configuration
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub ignored_bins: Vec<String>,
    pub default_threshold_days: i64,
    pub archive_path: PathBuf,
    /// On Windows, prefer `atime` (last access time) over `mtime` when selecting `last_used`.
    /// This can reduce false positives for frequently-run tools, but depends on NTFS last access updates.
    pub windows_use_access_time: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ignored_bins: vec![],
            default_threshold_days: 90,
            archive_path: PathBuf::from(".bin-expire/archive"),
            windows_use_access_time: true,
        }
    }
}