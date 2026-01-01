use anyhow::Result;
use anyhow::Context;
use std::fs;
use std::path::PathBuf;
use std::env;

use crate::models::Config;

fn base_config_dir() -> PathBuf {
    if let Some(dir) = env::var_os("BIN_EXPIRE_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn config_file_path() -> PathBuf {
    base_config_dir().join("bin-expire").join("config.toml")
}

pub fn manifest_file_path() -> PathBuf {
    base_config_dir().join("bin-expire").join("archive.json")
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
        let mut cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("Failed to parse config TOML: {}", path.display()))?;

        // Preserve the existing behavior: if archive_path wasn't set (older config), use the default.
        // With serde defaults, this will already be set, but keep it explicit/defensive.
        if cfg.archive_path.as_os_str().is_empty() {
            cfg.archive_path = default_archive_path();
        }

        // If this is an older config without newer keys, write it back with defaults filled in.
        // This makes the effective behavior explicit to the user (especially on Windows).
        let missing_windows_key = !raw.contains("windows_use_access_time");
        let missing_threshold_key = !raw.contains("default_threshold_days");
        let missing_archive_key = !raw.contains("archive_path");
        let missing_ignored_key = !raw.contains("ignored_bins");
        if missing_windows_key || missing_threshold_key || missing_archive_key || missing_ignored_key {
            let updated = toml::to_string_pretty(&cfg).context("Failed to serialize updated config")?;
            fs::write(&path, updated)
                .with_context(|| format!("Failed to update config file: {}", path.display()))?;
        }
        return Ok(cfg);
    }

    let mut cfg = Config::default();
    cfg.archive_path = default_archive_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }

    let raw = toml::to_string_pretty(&cfg).context("Failed to serialize default config")?;
    fs::write(&path, raw)
        .with_context(|| format!("Failed to write default config: {}", path.display()))?;

    Ok(cfg)
}