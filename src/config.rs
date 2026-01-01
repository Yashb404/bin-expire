use anyhow::Result;
use anyhow::Context;
use std::fs;
use std::path::PathBuf;

use crate::models::Config;

fn config_file_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("bin-expire").join("config.toml")
}

fn default_archive_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".bin-expire").join("archive")
}

pub fn load_config() -> Result<Config> {
    let path = config_file_path();

    if path.exists() {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("Failed to parse config TOML: {}", path.display()))?;
        return Ok(cfg);
    }

    let cfg = Config {
        ignored_bins: vec![],
        default_threshold_days: 90,
        archive_path: default_archive_path(),
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }

    let raw = toml::to_string_pretty(&cfg).context("Failed to serialize default config")?;
    fs::write(&path, raw)
        .with_context(|| format!("Failed to write default config: {}", path.display()))?;

    Ok(cfg)
}