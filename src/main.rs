
mod models;
mod analyzer;
mod config;
mod fs_scanner;
mod archiver;
mod archive_manifest;


use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tabled::{Table, Tabled};

use crate::analyzer::is_dormant;
use crate::analyzer::{select_last_used_time, FileTimes};
use crate::config::load_config;
use crate::fs_scanner::scan_directory;
use crate::archiver::archive_binary;
use crate::archiver::move_file_with_fallback;
use crate::archive_manifest::{record_archive, take_latest_entry_by_name};

#[derive(Parser)]
#[command(name = "bin-expire")]
#[command(about = "A CLI tool to manage old binaries", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan directories for stale binaries
    Scan {
        /// Directory to scan (e.g., ~/.cargo/bin)
        #[arg(short = 'p', long)]
        dir: Option<String>,
        /// Threshold in days for stale files
        #[arg(short, long)]
        days: Option<i64>,
    },
    /// Move stale binaries to the archive folder
    Archive {
        /// Directory to scan (e.g., ~/.cargo/bin)
        #[arg(short = 'p', long)]
        dir: Option<String>,
        #[arg(short, long)]
        days: Option<i64>,
    },
    /// Restore a previously archived binary back to its original path
    Restore {
        /// The archived file name to restore (e.g., "ripgrep" or "old_tool.exe")
        name: String,
    },
}

#[derive(Tabled)]
struct ScanRow {
    status: String,
    name: String,
    size: String,
    accessed: String,
    modified: String,
    last_used: String,
    path: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Load configuration (uses 'dirs' crate internally)
    let config = load_config()?;

    #[cfg(windows)]
    {
        eprintln!("Warning (Windows): file access times (atime) may be updated by directory listing/scanning. Treat atime as best-effort.");
        if config.windows_use_access_time {
            println!("Using access time (atime) on Windows to determine 'last used'.");
        }
    }

    match &cli.command {
        Commands::Scan { dir, days } => {
            let days = days.unwrap_or(config.default_threshold_days);
            let dirs: Vec<PathBuf> = match dir.clone() {
                Some(path_str) => vec![expand_tilde(&path_str)],
                None => vec![expand_tilde("~/.cargo/bin"), expand_tilde("~/go/bin")],
            };

            let mut binaries = Vec::new();
            let mut any_dir = false;

            let scan_start = std::time::SystemTime::now();

            for path in dirs {
                if !path.exists() {
                    eprintln!("Warning: Directory {} does not exist. Skipping.", path.display());
                    continue;
                }
                any_dir = true;
                print_mount_option_warning(&path);
                println!("Scanning: {} for files > {} days old", path.display(), days);
                binaries.extend(scan_directory(&path, config.windows_use_access_time));
            }

            let scan_end = std::time::SystemTime::now();

            #[cfg(windows)]
            {
                maybe_fallback_from_atime_contamination(
                    &mut binaries,
                    config.windows_use_access_time,
                    scan_start,
                    scan_end,
                );
            }

            if !any_dir {
                eprintln!("Error: No default directories exist to scan.");
                return Ok(());
            }

            let mut rows: Vec<ScanRow> = Vec::new();
            let mut stale_count: u64 = 0;
            let mut stale_total_bytes: u64 = 0;

            // Stable display order is nicer to read.
            binaries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            for bin in binaries {
                if config.ignored_bins.iter().any(|b| b == &bin.name) {
                    continue;
                }
                let is_stale = is_dormant(bin.last_used, days);
                if is_stale {
                    stale_count += 1;
                    stale_total_bytes = stale_total_bytes.saturating_add(bin.size);
                }

                rows.push(ScanRow {
                    status: if is_stale { "STALE" } else { "OK" }.to_string(),
                    name: bin.name,
                    size: format_bytes(bin.size),
                    accessed: format_optional_time(bin.accessed),
                    modified: format_optional_time(bin.modified),
                    last_used: humantime::format_rfc3339_seconds(bin.last_used).to_string(),
                    path: bin.path.display().to_string(),
                });
            }

            println!(
                "Found {} stale binaries (older than {} days):",
                stale_count,
                days
            );

            if !rows.is_empty() {
                println!("{}", Table::new(rows));
            }

            println!("Total wastage: {}", format_bytes(stale_total_bytes));
            println!(
                "Run 'bin-expire archive --days {}' to move these to {}.",
                days,
                config.archive_path.display()
            );
        }
        Commands::Archive { dir, days } => {
            let days = days.unwrap_or(config.default_threshold_days);
            let dirs: Vec<PathBuf> = match dir.clone() {
                Some(path_str) => vec![expand_tilde(&path_str)],
                None => vec![expand_tilde("~/.cargo/bin"), expand_tilde("~/go/bin")],
            };

            let mut binaries = Vec::new();
            let mut any_dir = false;

            let scan_start = std::time::SystemTime::now();

            for path in dirs {
                if !path.exists() {
                    eprintln!("Warning: Directory {} does not exist. Skipping.", path.display());
                    continue;
                }
                any_dir = true;
                print_mount_option_warning(&path);
                println!("Scanning: {} for files > {} days old", path.display(), days);
                binaries.extend(scan_directory(&path, config.windows_use_access_time));
            }

            let scan_end = std::time::SystemTime::now();

            #[cfg(windows)]
            {
                maybe_fallback_from_atime_contamination(
                    &mut binaries,
                    config.windows_use_access_time,
                    scan_start,
                    scan_end,
                );
            }

            if !any_dir {
                eprintln!("Error: No default directories exist to archive from.");
                return Ok(());
            }

            let mut stale: Vec<crate::models::BinaryInfo> = Vec::new();

            for bin in binaries {
                if config.ignored_bins.iter().any(|b| b == &bin.name) {
                    continue;
                }
                if is_dormant(bin.last_used, days) {
                    stale.push(bin);
                }
            }

            println!("Moving {} binaries to archive...", stale.len());
            for bin in &stale {
                match archive_binary(bin, &config.archive_path) {
                    Ok(dest) => {
                        if let Err(err) = record_archive(&bin.name, &bin.path, &dest) {
                            eprintln!("[WARN] Archived but failed to record manifest for '{}': {:#}", bin.name, err);
                        }
                        println!("[OK] Moved '{}' -> {}", bin.name, dest.display());
                    }
                    Err(err) => eprintln!("[ERR] Failed to move '{}': {:#}", bin.name, err),
                }
            }

            println!("Done.");
        }
        Commands::Restore { name } => {
            let entry = take_latest_entry_by_name(name)?;

            if !entry.archived_path.exists() {
                eprintln!("Error: Archived file does not exist: {}", entry.archived_path.display());
                return Ok(());
            }
            if entry.original_path.exists() {
                eprintln!("Error: Destination already exists: {}", entry.original_path.display());
                return Ok(());
            }
            if let Some(parent) = entry.original_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            move_file_with_fallback(&entry.archived_path, &entry.original_path)?;
            println!("[OK] Restored '{}' -> {}", entry.name, entry.original_path.display());
        }
    }

    Ok(())
}

/// Helper to convert "~" to the actual home directory
fn expand_tilde(path: &str) -> PathBuf {
    if !path.starts_with('~') {
        return PathBuf::from(path);
    }

    let Some(home) = dirs::home_dir() else {
        return PathBuf::from(path);
    };

    if path == "~" {
        return home;
    }

    // Handle common forms: "~/..." and "~\\...".
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        return home.join(rest);
    }

    PathBuf::from(path)
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0usize;

    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.1} {}", size, UNITS[unit])
    }
}

fn print_mount_option_warning(_path: &std::path::Path) {
    // Best-effort only; Windows doesn't have /proc mount options.
    #[cfg(unix)]
    {
        use std::fs;

        let mounts = fs::read_to_string("/proc/mounts").ok();
        let Some(mounts) = mounts else { return; };

        // Find the most specific mountpoint that prefixes the target path.
        let target = _path.canonicalize().unwrap_or_else(|_| _path.to_path_buf());
        let target_str = target.to_string_lossy();
        let mut best_opts: Option<String> = None;
        let mut best_len: usize = 0;

        for line in mounts.lines() {
            // format: <src> <mountpoint> <fstype> <opts> ...
            let mut parts = line.split_whitespace();
            let _src = parts.next();
            let mountpoint = parts.next();
            let _fstype = parts.next();
            let opts = parts.next();
            if mountpoint.is_none() || opts.is_none() {
                continue;
            }
            let mountpoint = mountpoint.unwrap();
            let opts = opts.unwrap();

            if target_str.starts_with(mountpoint) && mountpoint.len() >= best_len {
                best_len = mountpoint.len();
                best_opts = Some(opts.to_string());
            }
        }

        let Some(opts) = best_opts else { return; };
        if opts.contains("noatime") {
            println!("? Checking filesystem mount options... [WARN] noatime detected. Accuracy may vary.");
        } else if opts.contains("relatime") {
            println!("? Checking filesystem mount options... [WARN] relatime detected. Accuracy may vary.");
        }
    }
}

#[cfg(windows)]
fn maybe_fallback_from_atime_contamination(
    binaries: &mut [crate::models::BinaryInfo],
    windows_use_access_time: bool,
    scan_start: std::time::SystemTime,
    scan_end: std::time::SystemTime,
) {
    if !windows_use_access_time {
        return;
    }

    // Heuristic: if most files now show an access time within the scan window, the scan itself likely
    // updated atime and the values are not useful for determining real usage.
    let mut eligible = 0u64;
    let mut in_window = 0u64;

    for bin in binaries.iter() {
        let Some(atime) = bin.accessed else { continue; };
        eligible += 1;
        if atime >= scan_start && atime <= scan_end {
            in_window += 1;
        }
    }

    // Only trigger when we have enough samples, and the vast majority are "recent".
    if eligible >= 10 && (in_window as f64 / eligible as f64) >= 0.80 {
        eprintln!(
            "Warning: access times appear to have been updated during this scan ({} of {} within scan window). Falling back to modified time (mtime) for this run.",
            in_window,
            eligible
        );
        eprintln!(
            "Tip: you can set windows_use_access_time=false in config.toml to always use mtime."
        );

        for bin in binaries.iter_mut() {
            let (last_used, source) = select_last_used_time(
                FileTimes {
                    accessed: bin.accessed,
                    modified: bin.modified,
                },
                false,
            );
            bin.last_used = last_used;
            bin.last_used_source = source;
        }
    }
}

fn format_optional_time(value: Option<std::time::SystemTime>) -> String {
    match value {
        Some(t) => humantime::format_rfc3339_seconds(t).to_string(),
        None => "-".to_string(),
    }
}