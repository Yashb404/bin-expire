
mod models;
mod analyzer;
mod config;
mod fs_scanner;


use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::analyzer::is_dormant;
use crate::config::load_config;
use crate::fs_scanner::scan_directory;

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
        #[arg(short, long)]
        dir: Option<String>,
        /// Threshold in days for stale files
        #[arg(short, long, default_value_t = 90)]
        days: i64,
    },
    /// Move stale binaries to archive (Not implemented in skeleton)
    Archive {
        #[arg(short, long, default_value_t = 90)]
        days: i64,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Load configuration (uses 'dirs' crate internally)
    let _config = load_config()?;

    match &cli.command {
        Commands::Scan { dir, days } => {
            let path_str = dir.clone().unwrap_or_else(|| String::from("~/.cargo/bin"));
            let path = expand_tilde(&path_str);
            
            println!("Scanning: {:?} for files > {} days old", path, days);
            
            if !path.exists() {
                eprintln!("Error: Directory {:?} does not exist.", path);
                return Ok(());
            }

            let binaries = scan_directory(&path);
            let mut stale_count = 0;

            for bin in binaries {
                if is_dormant(bin.last_accessed, *days) {
                    // Format the date for display
                    let date_str = humantime::format_rfc3339_seconds(bin.last_accessed);
                    
                    println!("[STALE] {} (Last: {}) - {} bytes", 
                             bin.name, date_str, bin.size);
                    stale_count += 1;
                }
            }
            
            println!("Found {} stale binaries.", stale_count);
        }
        Commands::Archive { days: _ } => {
            println!("Archive command is not yet implemented in this skeleton.");
        }
    }

    Ok(())
}

/// Helper to convert "~" to the actual home directory
fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}