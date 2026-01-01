use clap::{Parser, Subcommand};

mod help;

#[derive(Parser)]
#[command(name = "bin-expire")]
#[command(
    about = "A CLI tool to manage old binaries",
    long_about = help::TOP_LONG_ABOUT,
    after_help = help::TOP_AFTER_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scan directories for stale binaries
    #[command(long_about = help::SCAN_LONG_ABOUT, after_help = help::SCAN_AFTER_HELP)]
    Scan {
        /// Directory to scan (e.g., ~/.cargo/bin)
        #[arg(short = 'p', long)]
        dir: Option<String>,
        /// Threshold in days for stale files
        #[arg(short, long)]
        days: Option<i64>,

        /// Show a more detailed table (includes PATH, SRC) and also shows OK rows
        #[arg(short, long)]
        verbose: bool,

        /// Show only stale binaries (hides OK and stub rows)
        #[arg(long)]
        only_stale: bool,
        /// Hide OK rows from the scan output table (mainly useful with --verbose)
        #[arg(long)]
        hide_ok: bool,
        /// Hide stub rows (0-byte .exe App Execution Alias stubs) from the scan output table
        #[arg(long)]
        hide_stub: bool,
    },

    /// Move stale binaries to the archive folder
    #[command(after_help = help::ARCHIVE_AFTER_HELP)]
    Archive {
        /// Directory to scan (e.g., ~/.cargo/bin)
        #[arg(short = 'p', long)]
        dir: Option<String>,
        #[arg(short, long)]
        days: Option<i64>,
    },

    /// Restore a previously archived binary back to its original path
    #[command(after_help = help::RESTORE_AFTER_HELP)]
    Restore {
        /// The archived file name to restore (e.g., "ripgrep" or "old_tool.exe")
        name: String,
    },
}
