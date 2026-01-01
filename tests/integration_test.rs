use filetime::{set_file_times, FileTime};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

fn env_flag_is_truthy(key: &str) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no")
        }
        Err(_) => false,
    }
}

fn test_verbose() -> bool {
    env_flag_is_truthy("BIN_EXPIRE_TEST_VERBOSE")
}

fn test_keep_artifacts() -> bool {
    env_flag_is_truthy("BIN_EXPIRE_TEST_KEEP")
}

fn unique_dir(prefix: &str) -> PathBuf {
    let pid = std::process::id();
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    PathBuf::from(format!("{}_{}_{}", prefix, pid, ts))
}

fn run_cli(args: &[&str], config_root: &Path) -> Output {
    let mut cmd = Command::new("cargo");
    cmd.env("BIN_EXPIRE_CONFIG_DIR", config_root);
    cmd.args(["run", "--"]);
    cmd.args(args);
    if test_verbose() {
        eprintln!(
            "[bin-expire tests] running: cargo run -- {}",
            args.join(" ")
        );
        eprintln!(
            "[bin-expire tests] BIN_EXPIRE_CONFIG_DIR={}",
            config_root.display()
        );
    }
    cmd.output().expect("Failed to execute command")
}

fn cleanup_dir(path: &Path) {
    if test_keep_artifacts() {
        eprintln!(
            "[bin-expire tests] keeping artifacts under {} (BIN_EXPIRE_TEST_KEEP=1)",
            path.display()
        );
        return;
    }
    let _ = fs::remove_dir_all(path);
}

fn write_artifact(dir: &Path, filename: &str, bytes: &[u8]) {
    let _ = fs::create_dir_all(dir);
    let _ = fs::write(dir.join(filename), bytes);
}

/// This test verifies that the tool correctly identifies a fake old binary.
#[test]
fn test_detects_stale_binary() {
    // 1. Setup: Create a temporary directory
    let test_dir = unique_dir("test_integration_dir");
    let config_root = unique_dir("test_integration_config");
    let archive_dir = unique_dir("test_integration_archive");
    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    fs::create_dir_all(&archive_dir).expect("Failed to create archive dir");

    // Write a config.toml into BIN_EXPIRE_CONFIG_DIR/bin-expire/config.toml
    let cfg_dir = config_root.join("bin-expire");
    fs::create_dir_all(&cfg_dir).expect("Failed to create config dir");

    // Force deterministic behavior for this test: use mtime rather than atime.
    let archive_str = archive_dir.to_string_lossy().replace('\\', "\\\\");
    let config_toml = format!(
        "ignored_bins = []\ndefault_threshold_days = 90\narchive_path = \"{}\"\nwindows_use_access_time = false\n",
        archive_str
    );
    fs::write(cfg_dir.join("config.toml"), config_toml).expect("Failed to write config.toml");
    let file_path = test_dir.join("old_tool.exe");

    // 2. Create the file
    fs::write(&file_path, "content").expect("Failed to write test file");

    // 3. Backdate the file to 100 days ago
    let old_time = SystemTime::now() - Duration::from_secs(86400 * 100);
    let ft = FileTime::from_system_time(old_time);
    set_file_times(&file_path, ft, ft).expect("Failed to backdate file");

    // 4. Execute the binary (This runs 'cargo run' logic)
    let output = run_cli(
        &["scan", "-p", test_dir.to_str().unwrap(), "--days", "30"],
        &config_root,
    );

    // Save artifacts for inspection (especially useful with BIN_EXPIRE_TEST_KEEP=1)
    let artifacts_dir = test_dir.join("_test_artifacts");
    write_artifact(&artifacts_dir, "scan.stdout.txt", &output.stdout);
    write_artifact(&artifacts_dir, "scan.stderr.txt", &output.stderr);
    write_artifact(
        &artifacts_dir,
        "paths.txt",
        format!(
            "test_dir={}\nconfig_root={}\narchive_dir={}\nfile_path={}\n",
            test_dir.display(),
            config_root.display(),
            archive_dir.display(),
            file_path.display(),
        )
        .as_bytes(),
    );

    // 6. Assertions
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // We assert that the output contains "STALE"
    assert!(
        output.status.success(),
        "scan command failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout,
        stderr
    );

    let row = stdout
        .lines()
        .find(|l| l.contains("old_tool.exe"))
        .unwrap_or_else(|| {
            panic!(
                "Output did not contain filename row.\nstdout:\n{}\nstderr:\n{}",
                stdout, stderr
            )
        });

    // New scan table uses glyphs:
    // ✗ = stale, ✓ = ok, · = shim
    assert!(
        row.contains("✗"),
        "Expected old_tool.exe to be marked stale (✗).\nrow:\n{}\nstdout:\n{}\nstderr:\n{}",
        row,
        stdout,
        stderr
    );

    println!("[bin-expire integration] test_detects_stale_binary: PASS");
    println!("[bin-expire integration] test_dir={}", test_dir.display());
    println!("[bin-expire integration] file_path={}", file_path.display());
    println!(
        "[bin-expire integration] artifacts_dir={}",
        artifacts_dir.display()
    );
    println!("[bin-expire integration] tips: run `cargo test -- --show-output` to see this on pass; set BIN_EXPIRE_TEST_KEEP=1 to keep dirs");
    if test_verbose() {
        println!("[bin-expire integration] scan stdout:\n{}", stdout);
        eprintln!("[bin-expire integration] scan stderr:\n{}", stderr);
    }

    // 5. Cleanup (unless BIN_EXPIRE_TEST_KEEP=1)
    let _ = fs::remove_file(&file_path);
    cleanup_dir(&test_dir);
    cleanup_dir(&archive_dir);
    cleanup_dir(&config_root);
}

/// This test verifies that `archive` records a manifest entry and `restore` puts it back.
#[test]
fn test_archive_and_restore_roundtrip() {
    // Setup dirs
    let test_dir = unique_dir("test_integration_dir_roundtrip");
    let config_root = unique_dir("test_integration_config_roundtrip");
    let archive_dir = unique_dir("test_integration_archive_roundtrip");

    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    fs::create_dir_all(&archive_dir).expect("Failed to create archive dir");

    // Write a config.toml into BIN_EXPIRE_CONFIG_DIR/bin-expire/config.toml
    let cfg_dir = config_root.join("bin-expire");
    fs::create_dir_all(&cfg_dir).expect("Failed to create config dir");

    // Escape backslashes for TOML on Windows
    let archive_str = archive_dir.to_string_lossy().replace('\\', "\\\\");
    let config_toml = format!(
        "ignored_bins = []\ndefault_threshold_days = 90\narchive_path = \"{}\"\nwindows_use_access_time = false\n",
        archive_str
    );
    fs::write(cfg_dir.join("config.toml"), config_toml).expect("Failed to write config.toml");

    // Create a file and backdate it
    let file_name = "old_tool.exe";
    let file_path = test_dir.join(file_name);
    fs::write(&file_path, "content").expect("Failed to write test file");

    let old_time = SystemTime::now() - Duration::from_secs(86400 * 100);
    let ft = FileTime::from_system_time(old_time);
    set_file_times(&file_path, ft, ft).expect("Failed to backdate file");

    let artifacts_dir = test_dir.join("_test_artifacts");

    // Run archive
    let output = run_cli(
        &["archive", "-p", test_dir.to_str().unwrap(), "--days", "30"],
        &config_root,
    );
    write_artifact(&artifacts_dir, "archive.stdout.txt", &output.stdout);
    write_artifact(&artifacts_dir, "archive.stderr.txt", &output.stderr);
    assert!(
        output.status.success(),
        "Archive command failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // It should have moved into archive
    let archived_path = archive_dir.join(file_name);
    assert!(
        !file_path.exists(),
        "Original file still exists after archive"
    );
    assert!(archived_path.exists(), "Archived file was not found");

    // Manifest should exist
    let manifest_path = cfg_dir.join("archive.json");
    assert!(
        manifest_path.exists(),
        "Manifest archive.json was not created"
    );

    // Run restore
    let output = run_cli(&["restore", file_name], &config_root);
    write_artifact(&artifacts_dir, "restore.stdout.txt", &output.stdout);
    write_artifact(&artifacts_dir, "restore.stderr.txt", &output.stderr);
    assert!(
        output.status.success(),
        "Restore command failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        file_path.exists(),
        "File was not restored to original location"
    );

    println!("[bin-expire integration] test_archive_and_restore_roundtrip: PASS");
    println!("[bin-expire integration] test_dir={}", test_dir.display());
    println!(
        "[bin-expire integration] archive_dir={}",
        archive_dir.display()
    );
    println!(
        "[bin-expire integration] config_root={}",
        config_root.display()
    );
    println!(
        "[bin-expire integration] artifacts_dir={}",
        artifacts_dir.display()
    );
    println!(
        "[bin-expire integration] manifest_path={}",
        manifest_path.display()
    );

    // Cleanup (unless BIN_EXPIRE_TEST_KEEP=1)
    let _ = fs::remove_file(&file_path);
    let _ = fs::remove_file(&archived_path);
    let _ = fs::remove_file(&manifest_path);
    let _ = fs::remove_file(cfg_dir.join("config.toml"));
    cleanup_dir(&test_dir);
    cleanup_dir(&archive_dir);
    cleanup_dir(&config_root);
}
