mod analyzer;
mod archive_manifest;
mod archiver;
mod cli;
mod cli_help;
mod config;
mod fs_scanner;
mod models;

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Parser;
use std::path::PathBuf;
use tabled::settings::style::Style;
use tabled::{Table, Tabled};

use colored::Colorize;

use crate::analyzer::is_dormant;
use crate::analyzer::{select_last_used_time, FileTimes};
use crate::archive_manifest::{record_archive, take_latest_entry_by_name};
use crate::archiver::archive_binary;
use crate::archiver::move_file_with_fallback;
use crate::config::load_config;
use crate::fs_scanner::scan_directory;

use crate::cli::{Cli, Commands};

// Default View: Compare Access vs Mod dates
#[derive(Tabled)]
struct DefaultRow {
    #[tabled(rename = "ST")]
    st: &'static str,

    #[tabled(rename = "NAME")]
    name: String,

    #[tabled(rename = "SIZE")]
    size: String,

    #[tabled(rename = "ACCESSED")]
    accessed: String,

    #[tabled(rename = "MODIFIED")]
    modified: String,
}

// Verbose View: Adds Path
#[derive(Tabled)]
struct VerboseRow {
    #[tabled(rename = "ST")]
    st: &'static str,

    #[tabled(rename = "NAME")]
    name: String,

    #[tabled(rename = "SIZE")]
    size: String,

    #[tabled(rename = "ACCESSED")]
    accessed: String,

    #[tabled(rename = "MODIFIED")]
    modified: String,

    #[tabled(rename = "PATH")]
    path: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load configuration (uses 'dirs' crate internally)
    let config = load_config()?;

    #[cfg(windows)]
    {
        print_windows_notice(config.windows_use_access_time);
    }

    match &cli.command {
        Commands::Scan {
            dir,
            days,
            verbose,
            only_stale,
            hide_ok,
            hide_shim,
        } => {
            let days = days.unwrap_or(config.default_threshold_days);
            let verbose = *verbose;
            let hide_ok = *only_stale || *hide_ok;
            let hide_shim = *only_stale || *hide_shim;

            let dirs: Vec<PathBuf> = match dir.clone() {
                Some(path_str) => vec![expand_tilde(&path_str)],
                None => vec![expand_tilde("~/.cargo/bin"), expand_tilde("~/go/bin")],
            };

            let mut binaries = Vec::new();
            let mut any_dir = false;

            let scan_start = std::time::SystemTime::now();

            // Visual Header
            println!("{}", "─".repeat(60).dimmed());
            println!("{}", "Scanning for stale binaries".cyan().bold());
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
                print_mount_option_warning(&path);
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
                maybe_fallback_from_atime_contamination(
                    &mut binaries,
                    config.windows_use_access_time,
                    scan_start,
                    scan_end,
                );
            }

            if !any_dir {
                eprintln!("{} No valid directories found to scan.", "[ERROR]".red());
                return Ok(());
            }

            println!();

            let mut default_rows: Vec<DefaultRow> = Vec::new();
            let mut verbose_rows: Vec<VerboseRow> = Vec::new();
            let mut stale_count: u64 = 0;
            let mut stale_total_bytes: u64 = 0;
            let mut ok_count: u64 = 0;
            let mut shim_count: u64 = 0;

            // Stable display order is nicer to read.
            binaries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            for bin in binaries {
                if config.ignored_bins.iter().any(|b| b == &bin.name) {
                    continue;
                }
                let is_probable_shim = bin.size == 0
                    && bin
                        .path
                        .extension()
                        .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("exe"));

                let is_stale = !is_probable_shim && is_dormant(bin.last_used, days);
                if is_stale {
                    stale_count += 1;
                    stale_total_bytes = stale_total_bytes.saturating_add(bin.size);
                }

                if is_probable_shim {
                    shim_count += 1;
                } else if !is_stale {
                    ok_count += 1;
                }

                // Visibility rules (QoL):
                // - Default: show only STALE + SHIM
                // - Verbose: also show OK
                // - User filters can hide OK/SHIM regardless of verbose
                let mut is_visible = is_stale || is_probable_shim || verbose;
                if hide_shim && is_probable_shim {
                    is_visible = false;
                }
                if hide_ok && !is_probable_shim && !is_stale {
                    is_visible = false;
                }
                if !is_visible {
                    continue;
                }

                // Prepare status glyphs (keep cells ASCII/short for stable alignment)
                let status = if is_probable_shim {
                    "·" // Shim
                } else if is_stale {
                    "✗" // Stale
                } else {
                    "✓" // OK (only visible in verbose)
                };

                // Format Dates (Short YYYY-MM-DD)
                let accessed_str = format_date_short(bin.accessed);
                let modified_str = format_date_short(bin.modified);

                if verbose {
                    verbose_rows.push(VerboseRow {
                        st: status,
                        name: bin.name,
                        size: format_bytes(bin.size),
                        accessed: accessed_str,
                        modified: modified_str,
                        path: bin.path.display().to_string(),
                    });
                } else {
                    default_rows.push(DefaultRow {
                        st: status,
                        name: bin.name,
                        size: format_bytes(bin.size),
                        accessed: accessed_str,
                        modified: modified_str,
                    });
                }
            }

            // Clean, minimal explanation (short + visible where it matters)
            println!("{}", "accessed(atime)=last read/execute (best-effort on Windows), modified(mtime)=last content change".dimmed());
            println!("{}", "│".cyan());

            if verbose {
                if !verbose_rows.is_empty() {
                    let mut table = Table::new(verbose_rows);
                    table.with(Style::modern());
                    println!("{}", table);
                } else {
                    println!("│ ✓ No matching binaries found.");
                }
            } else if !default_rows.is_empty() {
                let mut table = Table::new(default_rows);
                table.with(Style::markdown());
                println!("{}", table);
            } else {
                println!("│ ✓ No stale binaries found.");
            }

            println!("{}", "╰────".cyan());

            // Summary Section
            println!();
            if stale_count > 0 {
                println!(
                    "{} Summary: {} stale items | {} total wastage",
                    ">>>".bold(),
                    stale_count.to_string().red().bold(),
                    format_bytes(stale_total_bytes).bold()
                );
                println!();
                println!(
                    "Run {} to move these to {}.",
                    format!("bin-expire archive --days {}", days)
                        .cyan()
                        .underline(),
                    config.archive_path.display().to_string().cyan()
                );

                print_scan_status_info(days, ok_count, shim_count, stale_count, hide_ok, hide_shim);
            } else {
                println!(
                    "{} No stale binaries found. Your system is clean!",
                    "✓".green().bold()
                );

                print_scan_status_info(days, ok_count, shim_count, stale_count, hide_ok, hide_shim);
            }
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
                print_mount_option_warning(&path);
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
                maybe_fallback_from_atime_contamination(
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
        }

        Commands::Restore { name } => {
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
        let Some(mounts) = mounts else {
            return;
        };

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

        let Some(opts) = best_opts else {
            return;
        };
        if opts.contains("noatime") || opts.contains("relatime") {
            println!(
                "{} Warning: Filesystem is mounted with 'noatime' or 'relatime'. 'Last Accessed' dates may be inaccurate.",
                "[!]".yellow()
            );
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
        let Some(atime) = bin.accessed else {
            continue;
        };
        eligible += 1;
        if atime >= scan_start && atime <= scan_end {
            in_window += 1;
        }
    }

    // Only trigger when we have enough samples, and the vast majority are "recent".
    if eligible >= 10 && (in_window as f64 / eligible as f64) >= 0.80 {
        eprintln!(
            "{} Access times appear to have been updated during this scan ({} of {} within scan window).",
            "[!]".yellow(),
            in_window,
            eligible
        );
        eprintln!("    Falling back to modified time (mtime) for this run.");
        eprintln!("    Tip: Set windows_use_access_time=false in config.toml to avoid this check.");
        println!();

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

// Formats to YYYY-MM-DD only
fn format_date_short(value: Option<std::time::SystemTime>) -> String {
    match value {
        Some(t) => {
            let dt = DateTime::<Utc>::from(t);
            dt.format("%Y-%m-%d").to_string()
        }
        None => "-".to_string(),
    }
}

#[cfg(windows)]
fn print_windows_notice(windows_use_access_time: bool) {
    // Keep this on stderr so it doesn't pollute table output.
    let red_notice = "NOTICE".red().bold();

    eprintln!(
        "{} (Windows): access times (atime) are best-effort on Windows.",
        red_notice
    );
    eprintln!("- atime can be disabled, delayed, or not updated consistently by the filesystem.");
    eprintln!("- listing directories / scanning files can itself update atime, making files look 'recent'.");
    eprintln!("Access time example:");
    eprintln!("  You haven't run tool.exe in 90 days (atime=old),");
    eprintln!("  but a scan/listing updates atime to now -> it may appear recently used.");

    if windows_use_access_time {
        eprintln!(
            "This run will prefer atime to compute 'last used' (windows_use_access_time=true)."
        );
        eprintln!(
            "If results look suspicious, set windows_use_access_time=false to use mtime instead."
        );
    } else {
        eprintln!("This run will use modified time (mtime) for 'last used' (windows_use_access_time=false)." );
    }

    eprintln!();
}

fn print_scan_status_info(
    days: i64,
    ok_count: u64,
    shim_count: u64,
    stale_count: u64,
    hide_ok: bool,
    hide_shim: bool,
) {
    println!(
        "{} (info) STALE={} OK={} SHIM={} (filters: hide_ok={}, hide_shim={}).",
        "[i]".blue(),
        stale_count,
        ok_count,
        shim_count,
        hide_ok,
        hide_shim
    );
    println!(
        "{} OK: non-shim binaries with last_used within {} days.",
        "[i]".blue(),
        days
    );
    println!(
        "{} STALE: non-shim binaries with last_used older than {} days.",
        "[i]".blue(),
        days
    );
    println!(
        "{} SHIM: a 0-byte .exe placeholder (often App Execution Alias); treated specially and never archived.",
        "[i]".blue()
    );
}
