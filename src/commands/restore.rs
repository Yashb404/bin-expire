use anyhow::{bail, Result};
use colored::Colorize;

use crate::archive_manifest::{latest_entry_by_name, take_latest_entry_by_name};
use crate::archiver::move_file_with_fallback;
use crate::models::Config;

pub fn run(name: &str, _config: &Config) -> Result<()> {
    println!("{}", "─".repeat(60).dimmed());
    println!("{}", "Restoring binary".cyan().bold());
    println!("{}", "─".repeat(60).dimmed());

    // Do not mutate the manifest until we've validated and completed the restore.
    let entry = latest_entry_by_name(name)?;

    if !entry.archived_path.exists() {
        bail!(
            "Archived file does not exist: {}",
            entry.archived_path.display()
        );
    }
    if entry.original_path.exists() {
        bail!(
            "Destination already exists: {}",
            entry.original_path.display()
        );
    }
    if let Some(parent) = entry.original_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    move_file_with_fallback(&entry.archived_path, &entry.original_path)?;

    // Now that we've restored the file, remove the manifest entry.
    // If this fails, warn but don't fail the restore itself.
    if let Err(err) = take_latest_entry_by_name(name) {
        eprintln!(
            "{} Restored but failed to update manifest for '{}': {:#}",
            "[WARN]".yellow(),
            name,
            err
        );
    }

    println!(
        "{} Restored '{}' -> {}",
        "✓".green(),
        entry.name,
        entry.original_path.display()
    );

    Ok(())
}
