use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;
use tabled::settings::style::Style;
use tabled::Table;

use crate::analyzer::is_dormant;
use crate::fs_scanner::scan_directory;
use crate::models::Config;
use crate::ui;

pub fn run(
    dir: Option<String>,
    days: Option<i64>,
    verbose: bool,
    only_stale: bool,
    hide_ok: bool,
    hide_shim: bool,
    config: &Config,
) -> Result<()> {
    let days = days.unwrap_or(config.default_threshold_days);
    let hide_ok = only_stale || hide_ok;
    let hide_shim = only_stale || hide_shim;

    let dirs: Vec<PathBuf> = match dir {
        Some(path_str) => vec![ui::expand_tilde(&path_str)],
        None => vec![
            ui::expand_tilde("~/.cargo/bin"),
            ui::expand_tilde("~/go/bin"),
        ],
    };

    let mut binaries = Vec::new();
    let mut any_dir = false;

    #[cfg(windows)]
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
        ui::print_mount_option_warning(&path);
        println!(
            "{} {} for files > {} days old",
            "[*]".blue(),
            path.display(),
            days
        );
        binaries.extend(scan_directory(&path, config.windows_use_access_time));
    }

    #[cfg(windows)]
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
        eprintln!("{} No valid directories found to scan.", "[ERROR]".red());
        return Ok(());
    }

    println!();

    let mut default_rows: Vec<ui::DefaultRow> = Vec::new();
    let mut verbose_rows: Vec<ui::VerboseRow> = Vec::new();
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
        let accessed_str = ui::format_date_short(bin.accessed);
        let modified_str = ui::format_date_short(bin.modified);

        if verbose {
            let src = match bin.last_used_source {
                crate::models::LastUsedSource::Accessed => "A",
                crate::models::LastUsedSource::Modified => "M",
                crate::models::LastUsedSource::Unknown => "?",
            };
            verbose_rows.push(ui::VerboseRow {
                st: status,
                src,
                name: bin.name,
                size: ui::format_bytes(bin.size),
                accessed: accessed_str,
                modified: modified_str,
                path: bin.path.display().to_string(),
            });
        } else {
            default_rows.push(ui::DefaultRow {
                st: status,
                name: bin.name,
                size: ui::format_bytes(bin.size),
                accessed: accessed_str,
                modified: modified_str,
            });
        }
    }

    // Clean, minimal explanation (short + visible where it matters)
    println!(
        "{}",
        "accessed(atime)=last read/execute (best-effort on Windows), modified(mtime)=last content change".dimmed()
    );
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
            ui::format_bytes(stale_total_bytes).bold()
        );
        println!();
        println!(
            "Run {} to move these to {}.",
            format!("bin-expire archive --days {}", days)
                .cyan()
                .underline(),
            config.archive_path.display().to_string().cyan()
        );

        ui::print_scan_status_info(days, ok_count, shim_count, stale_count, hide_ok, hide_shim);
    } else {
        println!(
            "{} No stale binaries found. Your system is clean!",
            "✓".green().bold()
        );

        ui::print_scan_status_info(days, ok_count, shim_count, stale_count, hide_ok, hide_shim);
    }

    Ok(())
}
