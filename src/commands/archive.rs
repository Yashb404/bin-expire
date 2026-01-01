use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::analyzer::is_dormant;
use crate::archive_manifest::record_archive;
use crate::archiver::archive_binary;
use crate::fs_scanner::scan_directory;
use crate::models::Config;
use crate::ui;

pub fn run(dir: Option<String>, days: Option<i64>, config: &Config) -> Result<()> {
    let days = days.unwrap_or(config.default_threshold_days);

    let dirs: Vec<PathBuf> = match dir {
        Some(path_str) => vec![ui::expand_tilde(&path_str)],
        None => vec![
            ui::expand_tilde("~/.cargo/bin"),
            ui::expand_tilde("~/go/bin"),
        ],
    };

    let mut binaries = Vec::new();
    let mut any_dir = false;

    let scan_start = std::time::SystemTime::now();

    println!("{}", "─".repeat(60).dimmed());
    println!("{}", "Archiving stale binaries".cyan().bold());
    println!("{}", "─".repeat(60).dimmed());

    for path in dirs {
        if !path.exists() {
            eprintln!(
                "{} Directory {} does not exist. Skipping.",
                "[!]".yellow(),
                path.display()
            );
            continue;
        }
        any_dir = true;
        ui::print_mount_option_warning(&path);
        println!(
            "{} {} for files > {} days old",
            "[*]".blue(),
            path.display(),
            days
        );
        binaries.extend(scan_directory(&path, config.windows_use_access_time));
    }

    let scan_end = std::time::SystemTime::now();

    #[cfg(windows)]
    {
        ui::maybe_fallback_from_atime_contamination(
            &mut binaries,
            config.windows_use_access_time,
            scan_start,
            scan_end,
        );
    }

    if !any_dir {
        eprintln!("{} No valid directories found to archive.", "[ERROR]".red());
        return Ok(());
    }

    let mut stale: Vec<crate::models::BinaryInfo> = Vec::new();
    let mut success_count = 0u64;
    let mut fail_count = 0u64;

    for bin in binaries {
        if config.ignored_bins.iter().any(|b| b == &bin.name) {
            continue;
        }
        let is_probable_shim = bin.size == 0
            && bin
                .path
                .extension()
                .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("exe"));

        if is_probable_shim {
            continue;
        }

        if is_dormant(bin.last_used, days) {
            stale.push(bin);
        }
    }

    if stale.is_empty() {
        println!();
        println!("{} Nothing to archive.", "✓".green().bold());
        return Ok(());
    }

    println!();
    println!("Moving {} binaries to archive...", stale.len());
    println!("{}", "─".repeat(60).dimmed());
    for bin in &stale {
        match archive_binary(bin, &config.archive_path) {
            Ok(dest) => {
                if let Err(err) = record_archive(&bin.name, &bin.path, &dest) {
                    eprintln!(
                        "{} Archived but failed to record manifest for '{}': {:#}",
                        "[WARN]".yellow(),
                        bin.name,
                        err
                    );
                }
                println!("{} Moved '{}' -> {}", "✓".green(), bin.name, dest.display());
                success_count += 1;
            }
            Err(err) => {
                eprintln!("{} Failed to move '{}': {:#}", "✗".red(), bin.name, err);
                fail_count += 1;
            }
        }
    }

    println!("{}", "─".repeat(60).dimmed());
    println!("{} Archive operation completed.", "✓".green().bold());
    println!(
        "{} Success: {} | Failed: {}",
        "   ".dimmed(),
        success_count.to_string().green(),
        fail_count.to_string().red()
    );

    Ok(())
}
