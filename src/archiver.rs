use crate::models::BinaryInfo;
use anyhow::Result;
use anyhow::Context;
use std::fs;
use std::path::{Path, PathBuf};

pub fn move_file_with_fallback(src: &Path, dest: &Path) -> Result<()> {
    match fs::rename(src, dest) {
        Ok(_) => Ok(()),
        Err(rename_err) => {
            fs::copy(src, dest)
                .with_context(|| format!("Failed to copy {} to {}", src.display(), dest.display()))?;
            fs::remove_file(src)
                .with_context(|| format!("Failed to remove original {} after copy", src.display()))?;
            let _ = rename_err;
            Ok(())
        }
    }
}

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

    move_file_with_fallback(&bin.path, &dest)?;
    Ok(dest)
}
