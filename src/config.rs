use anyhow::Result;
use std::path::PathBuf;

use crate::models::Config;

pub fn load_config() -> Result<Config> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    
    let archive_path = home
        .join(".bin-expire")
        .join("archive");

    Ok(Config {
        ignored_bins: vec![],
        archive_path,
    })
}