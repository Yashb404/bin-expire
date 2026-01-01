mod analyzer;
mod archive_manifest;
mod archiver;
mod cli;
mod commands;
mod config;
mod fs_scanner;
mod models;
mod ui;

use crate::config::load_config;
use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load configuration (uses 'dirs' crate internally)
    let config = load_config()?;

    #[cfg(windows)]
    {
        ui::print_windows_notice(config.windows_use_access_time);
    }

    match &cli.command {
        Commands::Scan {
            dir,
            days,
            verbose,
            only_stale,
            hide_ok,
            hide_stub,
        } => {
            commands::scan::run(
                dir.clone(),
                *days,
                *verbose,
                *only_stale,
                *hide_ok,
                *hide_stub,
                &config,
            )?;
        }

        Commands::Archive { dir, days } => {
            commands::archive::run(dir.clone(), *days, &config)?;
        }

        Commands::Restore { name } => {
            commands::restore::run(name, &config)?;
        }
    }

    Ok(())
}
