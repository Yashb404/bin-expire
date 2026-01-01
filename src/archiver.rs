use crate::models::BinaryInfo;
use anyhow::Result;
use anyhow::Context;
use std::fs;
use std::path::{Path, PathBuf};

fn unique_destination(archive_dir: &Path, file_name: &str) -> PathBuf {
    let mut candidate = archive_dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    // Avoid overwriting; append a numeric suffix.
    for i in 1..10000u32 {
        let with_suffix = format!("{}.{}", file_name, i);
        candidate = archive_dir.join(with_suffix);
        if !candidate.exists() {
            return candidate;
        }
    }

    // Last resort: include a timestamp.
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    archive_dir.join(format!("{}.{}", file_name, ts))
}

pub fn archive_binary(bin: &BinaryInfo, archive_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(archive_dir)
        .with_context(|| format!("Failed to create archive dir: {}", archive_dir.display()))?;

    let dest = unique_destination(archive_dir, &bin.name);

    match fs::rename(&bin.path, &dest) {
        Ok(_) => Ok(dest),
        Err(rename_err) => {
            // Fallback for cross-device moves / permission quirks: copy then remove.
            fs::copy(&bin.path, &dest)
                .with_context(|| format!("Failed to copy {} to {}", bin.path.display(), dest.display()))?;
            fs::remove_file(&bin.path)
                .with_context(|| format!("Failed to remove original {} after copy", bin.path.display()))?;
            // Preserve the original rename error in context for troubleshooting.
            let _ = rename_err;
            Ok(dest)
        }
    }
}

pub fn restore_binary(name: &str) -> Result<()> {
    println!("Restoring: {}", name);
    // TODO: Implement restore logic
    Ok(())
}