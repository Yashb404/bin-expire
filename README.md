# bin-expire
![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg) 
![Language: Rust](https://img.shields.io/badge/Language-Rust-orange) 
![Platform: Linux | macOS | Windows](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey)

bin-expire scans common "bin" folders, identifies stale binaries, and can archive/restore them safely.

By default it scans:

- `~/.cargo/bin`
- `~/go/bin`

It also detects Windows “App Execution Alias stubs” (0-byte `.exe` placeholder files) and treats them specially so you don’t accidentally archive them.

## Install
### Option A: Install as a cargo crate
1. Install Rust (from rustup)
2. Run
   
   ```
   cargo install bin-expire
   ```

### Option B: Download from GitHub Releases

1. Download the asset for your OS from the GitHub Releases page.
2. Put the binary in a permanent folder (example):

  - Windows: `C:\Users\<you>\AppData\Local\rust-apps\bin-expire.exe`

3. Add that folder (not the `.exe`) to your `PATH`.
4. Verify:

```bat
where bin-expire
bin-expire --help
```

### Option C: Build from source

Build an optimized binary:

```bash
cargo build --release
```

Run it:

```bash
./target/release/bin-expire scan --days 30
```

Or install it into your Cargo bin directory:

```bash
cargo install --path .
```

## Quick start

```bash
# Scan default locations (~/.cargo/bin and ~/go/bin)
bin-expire scan

# Scan a specific directory
bin-expire scan -p ~/.cargo/bin --days 30

# Archive stale binaries (moves them into your configured archive_path)
bin-expire archive --days 30

# Restore a previously archived binary by name
bin-expire restore old_tool.exe
```

## Commands

### scan

- Default output shows only:
 \- STALE rows (`✗`)
  - stub rows (`·`) (Windows App Execution Alias stubs)
- `--verbose` also shows OK rows (`✓`) and adds:
  - `PATH` column
  - `SRC` column indicating where `last_used` came from: `A`=atime, `M`=mtime, `?`=unknown

Useful filters:

- `--only-stale` (hides OK + stubs)
- `--hide-ok` (mainly useful with `--verbose`)
- `--hide-stub` (hides stub rows)

### archive

Moves stale binaries into `archive_path` and records each move in a manifest so it can be restored later.

Notes:

- App Execution Alias stubs (0-byte `.exe`) are never archived.
- Archiving avoids overwriting by choosing a non-colliding filename in the archive directory.
- If a direct rename/move fails, it falls back to copy + remove.

### restore

Restores the most recent archived entry for the given name (from the manifest).

Safety behavior:

- Fails if the archived file is missing.
- Fails if the destination already exists (it will not overwrite your existing file).

## Configuration

On first run, bin-expire creates a config file under your platform config directory:

- Windows: `%APPDATA%\bin-expire\config.toml`
- Linux: `~/.config/bin-expire/config.toml`
- macOS: `~/Library/Application Support/bin-expire/config.toml`

You can override the config root with an environment variable:

- `BIN_EXPIRE_CONFIG_DIR=/some/path`

When set, bin-expire reads/writes:

- `BIN_EXPIRE_CONFIG_DIR/bin-expire/config.toml`
- `BIN_EXPIRE_CONFIG_DIR/bin-expire/archive.json`

Example `config.toml`:

```toml
ignored_bins = ["cargo", "rustc"]
default_threshold_days = 90
archive_path = "C:/Users/me/.bin-expire/archive"
windows_use_access_time = true
```

Config keys:

- `ignored_bins`: file names to ignore during scan/archive
- `default_threshold_days`: used when `--days` is not provided
- `archive_path`: where archived binaries are moved
- `windows_use_access_time`: Windows-only preference for selecting `last_used`

## Windows note (atime)

On Windows, access times (atime) are best-effort and can be disabled, delayed, or updated by scanning/listing.

- If results look suspicious, set `windows_use_access_time=false` to use modified time (mtime).
- bin-expire may also detect atime “contamination” during a scan and fall back to mtime.

## Development

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --release --all-targets
```
