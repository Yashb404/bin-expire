pub const TOP_LONG_ABOUT: &str = "bin-expire scans your bin directories, identifies stale binaries, and can archive/restore them.";

pub const TOP_AFTER_HELP: &str = "EXAMPLES:\n  bin-expire scan\n  bin-expire scan --days 30\n  bin-expire scan --verbose\n  bin-expire scan --only-stale\n  bin-expire scan --verbose --hide-ok\n  bin-expire archive --days 30\n  bin-expire restore <name>\n\nSCAN OUTPUT:\n  Default scan shows only stale (✗) and shim (·) rows.\n  Use --verbose to include PATH and OK (✓) rows.\n\nSTATUS GLYPHS:\n  ✗  stale (older than threshold)\n  ✓  ok (shown in --verbose)\n  ·  shim (0-byte .exe, never archived)\n\nWINDOWS NOTE:\n  On Windows, access times (atime) are best-effort and can be updated by scanning/listing. If results look suspicious, set windows_use_access_time=false in config.toml to use mtime.";

pub const SCAN_LONG_ABOUT: &str = "Scan directories for binaries older than the given threshold.\n\nDates:\n  ACCESSED (atime): last read/execute (best-effort on Windows)\n  MODIFIED (mtime): last content change\n\nDefault view:\n  Shows only stale (✗) and shim (·) rows with short dates (YYYY-MM-DD).\n\nVerbose view (--verbose):\n  Adds PATH column and also shows OK (✓) rows.";

pub const SCAN_AFTER_HELP: &str = "FILTERS:\n  --only-stale   Show only stale rows (hides OK and SHIM)\n  --hide-ok      Hide OK rows (mainly useful with --verbose)\n  --hide-shim    Hide shim rows\n\nEXAMPLES:\n  bin-expire scan --days 30\n  bin-expire scan --only-stale\n  bin-expire scan --verbose --hide-ok\n  bin-expire scan --verbose --hide-shim";

pub const ARCHIVE_AFTER_HELP: &str = "NOTES:\n  - SHIM entries (0-byte .exe) are never archived.\n  - Archiving records entries in archive.json so restore can put files back.";

pub const RESTORE_AFTER_HELP: &str = "EXAMPLE:\n  bin-expire restore old_tool.exe\n\nRestores the most recent archived entry for that name using archive.json.";
