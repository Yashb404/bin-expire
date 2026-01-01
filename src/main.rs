
mod models;
mod analyzer;
mod config;
mod fs_scanner;
mod archiver;


use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tabled::{Table, Tabled};

use crate::analyzer::is_dormant;
use crate::config::load_config;
use crate::fs_scanner::scan_directory;
use crate::archiver::archive_binary;

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
    /// Move stale binaries to archive (Not implemented in skeleton)
    Archive {
        /// Directory to scan (e.g., ~/.cargo/bin)
        #[arg(short = 'p', long)]
        dir: Option<String>,
        #[arg(short, long)]
        days: Option<i64>,
    },
}

#[derive(Tabled)]
struct StaleRow {
    status: String,
    name: String,
    size: String,
    last_used: String,
    path: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Load configuration (uses 'dirs' crate internally)
    let config = load_config()?;

    match &cli.command {
        Commands::Scan { dir, days } => {
            let days = days.unwrap_or(config.default_threshold_days);
            let dirs: Vec<PathBuf> = match dir.clone() {
                Some(path_str) => vec![expand_tilde(&path_str)],
                None => vec![expand_tilde("~/.cargo/bin"), expand_tilde("~/go/bin")],
            };

            let mut binaries = Vec::new();
            let mut any_dir = false;

            for path in dirs {
                if !path.exists() {
                    eprintln!("Warning: Directory {} does not exist. Skipping.", path.display());
                    continue;
                }
                any_dir = true;
                print_mount_option_warning(&path);
                println!("Scanning: {} for files > {} days old", path.display(), days);
                binaries.extend(scan_directory(&path));
            }

            if !any_dir {
                eprintln!("Error: No default directories exist to scan.");
                return Ok(());
            }

            let mut stale_rows: Vec<StaleRow> = Vec::new();
            let mut stale_total_bytes: u64 = 0;

            for bin in binaries {
                if config.ignored_bins.iter().any(|b| b == &bin.name) {
                    continue;
                }
                if is_dormant(bin.last_used, days) {
                    stale_total_bytes = stale_total_bytes.saturating_add(bin.size);
                    stale_rows.push(StaleRow {
                        status: "STALE".to_string(),
                        name: bin.name,
                        size: format_bytes(bin.size),
                        last_used: humantime::format_rfc3339_seconds(bin.last_used).to_string(),
                        path: bin.path.display().to_string(),
                    });
                }
            }

            println!(
                "Found {} stale binaries (older than {} days):",
                stale_rows.len(),
                days
            );

            if !stale_rows.is_empty() {
                println!("{}", Table::new(stale_rows));
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

            for path in dirs {
                if !path.exists() {
                    eprintln!("Warning: Directory {} does not exist. Skipping.", path.display());
                    continue;
                }
                any_dir = true;
                print_mount_option_warning(&path);
                println!("Scanning: {} for files > {} days old", path.display(), days);
                binaries.extend(scan_directory(&path));
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
                    Ok(dest) => println!("[OK] Moved '{}' -> {}", bin.name, dest.display()),
                    Err(err) => eprintln!("[ERR] Failed to move '{}': {:#}", bin.name, err),
                }
            }

            println!("Done.");
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