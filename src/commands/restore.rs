use anyhow::Result;
use colored::Colorize;

use crate::archive_manifest::take_latest_entry_by_name;
use crate::archiver::move_file_with_fallback;
use crate::models::Config;

pub fn run(name: &str, _config: &Config) -> Result<()> {
    println!("{}", "─".repeat(60).dimmed());
    println!("{}", "Restoring binary".cyan().bold());
    println!("{}", "─".repeat(60).dimmed());

    let entry = take_latest_entry_by_name(name)?;

    if !entry.archived_path.exists() {
        eprintln!(
            "{} Archived file does not exist: {}",
            "[ERROR]".red(),
            entry.archived_path.display()
        );
        return Ok(());
    }
    if entry.original_path.exists() {
        eprintln!(
            "{} Destination already exists: {}",
            "[ERROR]".red(),
            entry.original_path.display()
        );
        return Ok(());
    }
    if let Some(parent) = entry.original_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    move_file_with_fallback(&entry.archived_path, &entry.original_path)?;
    println!(
        "{} Restored '{}' -> {}",
        "✓".green(),
        entry.name,
        entry.original_path.display()
    );

    Ok(())
}
