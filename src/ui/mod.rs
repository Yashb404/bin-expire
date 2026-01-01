use chrono::{DateTime, Utc};
use colored::Colorize;
use std::path::{Path, PathBuf};
use tabled::Tabled;

use crate::analyzer::{select_last_used_time, FileTimes};

// Default View: Compare Access vs Mod dates
#[derive(Tabled)]
pub struct DefaultRow {
    #[tabled(rename = "ST")]
    pub st: &'static str,

    #[tabled(rename = "NAME")]
    pub name: String,

    #[tabled(rename = "SIZE")]
    pub size: String,

    #[tabled(rename = "ACCESSED")]
    pub accessed: String,

    #[tabled(rename = "MODIFIED")]
    pub modified: String,
}

// Verbose View: Adds Path
#[derive(Tabled)]
pub struct VerboseRow {
    #[tabled(rename = "ST")]
    pub st: &'static str,

    #[tabled(rename = "NAME")]
    pub name: String,

    #[tabled(rename = "SIZE")]
    pub size: String,

    #[tabled(rename = "ACCESSED")]
    pub accessed: String,

    #[tabled(rename = "MODIFIED")]
    pub modified: String,

    #[tabled(rename = "PATH")]
    pub path: String,
}

/// Helper to convert "~" to the actual home directory
pub fn expand_tilde(path: &str) -> PathBuf {
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

pub fn format_bytes(bytes: u64) -> String {
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

// Formats to YYYY-MM-DD only
pub fn format_date_short(value: Option<std::time::SystemTime>) -> String {
    match value {
        Some(t) => {
            let dt = DateTime::<Utc>::from(t);
            dt.format("%Y-%m-%d").to_string()
        }
        None => "-".to_string(),
    }
}

pub fn print_mount_option_warning(path: &Path) {
    // Best-effort only; Windows doesn't have /proc mount options.
    #[cfg(unix)]
    {
        use std::fs;

        let mounts = fs::read_to_string("/proc/mounts").ok();
        let Some(mounts) = mounts else {
            return;
        };

        // Find the most specific mountpoint that prefixes the target path.
        let target = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
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

    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

#[cfg(windows)]
pub fn maybe_fallback_from_atime_contamination(
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

#[cfg(windows)]
pub fn print_windows_notice(windows_use_access_time: bool) {
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

pub fn print_scan_status_info(
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
