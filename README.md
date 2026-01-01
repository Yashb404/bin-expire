# bin-expire

A small Rust CLI to find and safely archive “stale” binaries from common global bin folders (like `~/.cargo/bin` and `~/go/bin`).

## What it does (MVP)

- Scans one or more directories for files older than a threshold (in days)
- Prints a simple table: Status / Name / Size / Last Used / Path
- Archives stale binaries by moving them into an archive directory (no permanent deletion)
- Uses a persistent config file (`config.toml`) for defaults

> Note on “Last Used”: file access times (`atime`) can be unreliable on some systems (especially Windows). This project chooses a cross-platform “last used” timestamp with sensible fallbacks.

On Windows, the default prefers access time (`atime`) when available for better accuracy with frequently-run tools (like `cargo`). Note that NTFS can defer updating `atime` (commonly up to ~1 hour).

## Install / Run

### Run from source

```bash
cargo run -- scan --days 30
```

### Build

```bash
cargo build --release
```

Run the built binary:

```bash
./target/release/bin-expire scan --days 30
```

## Commands

### `scan`

Scan directories and list stale binaries.

- If you omit `--dir`, it scans both `~/.cargo/bin` and `~/go/bin` (skipping any that don’t exist).

Examples:

```bash
# scan defaults (~/.cargo/bin and ~/go/bin)
bin-expire scan

# scan a specific directory
bin-expire scan --dir ~/.cargo/bin --days 30

# short flag for dir
bin-expire scan -p ~/.cargo/bin --days 30
```

### `archive`

Move stale binaries into the archive directory.

Examples:

```bash
# archive from defaults (~/.cargo/bin and ~/go/bin)
bin-expire archive --days 30

# archive from a specific directory
bin-expire archive --dir ~/.cargo/bin --days 30
```

### `restore`

Restore a previously archived binary back to its original path (based on the archive manifest).

```bash
bin-expire restore old_tool.exe
```

## Configuration

On first run, `bin-expire` creates a config file in your platform config directory.

Typical locations:

- **Windows**: `%APPDATA%/bin-expire/config.toml`
- **Linux**: `~/.config/bin-expire/config.toml`
- **macOS**: `~/Library/Application Support/bin-expire/config.toml`

Example `config.toml`:

```toml
ignored_bins = ["cargo", "rustc"]
default_threshold_days = 90
archive_path = "C:\\Users\\me\\.bin-expire\\archive"
```

### Config keys

- `ignored_bins`: list of binary file names to ignore during scan/archive
- `default_threshold_days`: used when `--days` is not provided
- `archive_path`: where archived binaries are moved
- `windows_use_access_time`: **Windows only**. If `true`, prefers `atime` over `mtime` when deciding “last used”.

#### Windows note: NTFS last access updates

Windows/NTFS may defer last access time updates (commonly up to ~1 hour) and the feature can be disabled for performance.

To check the setting (run in an elevated terminal):

```bat
fsutil behavior query disablelastaccess
```

To enable last access updates (may require a restart):

```bat
fsutil behavior set disablelastaccess 0
```

## Archive manifest

When you run `archive`, `bin-expire` records moves in an `archive.json` manifest alongside the config file. `restore` uses this manifest to put a binary back where it came from.

## Notes / Safety

- `archive` avoids overwriting by choosing a non-colliding filename in the archive directory.
- If a direct rename/move fails (e.g., cross-device move), it falls back to copy + remove.

## Development

Run tests:

```bash
cargo test
```

## Roadmap ideas (not implemented yet)

- Archive manifest + `restore <name>`
- `clean` command to delete archived binaries
- Dry-run mode
- Color-coded output
