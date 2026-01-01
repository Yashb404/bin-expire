use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::manifest_file_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub name: String,
    pub original_path: PathBuf,
    pub archived_path: PathBuf,
    pub moved_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub entries: Vec<ArchiveEntry>,
}

fn load_manifest(path: &Path) -> Result<ArchiveManifest> {
    if !path.exists() {
        return Ok(ArchiveManifest::default());
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Failed to read manifest: {}", path.display()))?;
    let manifest = serde_json::from_str::<ArchiveManifest>(&raw)
        .with_context(|| format!("Failed to parse manifest JSON: {}", path.display()))?;
    Ok(manifest)
}

fn save_manifest_atomic(path: &Path, manifest: &ArchiveManifest) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create manifest directory: {}", parent.display()))?;
    }

    let tmp = path.with_extension("json.tmp");
    let raw = serde_json::to_string_pretty(manifest).context("Failed to serialize manifest")?;
    fs::write(&tmp, raw).with_context(|| format!("Failed to write temp manifest: {}", tmp.display()))?;

    // Windows doesn't allow rename over an existing file.
    if path.exists() {
        let _ = fs::remove_file(path);
    }

    fs::rename(&tmp, path)
        .with_context(|| format!("Failed to replace manifest {}", path.display()))?;

    Ok(())
}

pub fn record_archive(name: &str, original_path: &Path, archived_path: &Path) -> Result<()> {
    let path = manifest_file_path();
    let mut manifest = load_manifest(&path)?;

    manifest.entries.push(ArchiveEntry {
        name: name.to_string(),
        original_path: original_path.to_path_buf(),
        archived_path: archived_path.to_path_buf(),
        moved_at: humantime::format_rfc3339_seconds(std::time::SystemTime::now()).to_string(),
    });

    save_manifest_atomic(&path, &manifest)
}

pub fn take_latest_entry_by_name(name: &str) -> Result<ArchiveEntry> {
    let path = manifest_file_path();
    let mut manifest = load_manifest(&path)?;

    let idx = manifest
        .entries
        .iter()
        .rposition(|e| e.name == name)
        .ok_or_else(|| anyhow!("No archived entry found for '{}'", name))?;

    let entry = manifest.entries.remove(idx);
    save_manifest_atomic(&path, &manifest)?;

    Ok(entry)
}
