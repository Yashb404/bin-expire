use filetime::{set_file_times, FileTime};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

#[derive(Debug, Deserialize)]
struct TestArchiveEntry {
    name: String,
}

#[derive(Debug, Deserialize)]
struct TestArchiveManifest {
    entries: Vec<TestArchiveEntry>,
}

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
    std::env::temp_dir().join(format!("{}_{}_{}", prefix, pid, ts))
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

fn read_manifest_names(manifest_path: &Path) -> Vec<String> {
    if !manifest_path.exists() {
        return vec![];
    }
    let raw = fs::read_to_string(manifest_path).expect("Failed to read archive.json");
    let manifest: TestArchiveManifest =
        serde_json::from_str(&raw).expect("Failed to parse archive.json");
    manifest.entries.into_iter().map(|e| e.name).collect()
}

/// Detects a deliberately backdated binary as stale.
#[test]
fn test_detects_stale_binary() {
    let test_dir = unique_dir("test_integration_dir");
    let config_root = unique_dir("test_integration_config");
    let archive_dir = unique_dir("test_integration_archive");
    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    fs::create_dir_all(&archive_dir).expect("Failed to create archive dir");

    let cfg_dir = config_root.join("bin-expire");
    fs::create_dir_all(&cfg_dir).expect("Failed to create config dir");

    // Keep the test deterministic by using mtime (not atime).
    let archive_str = archive_dir.to_string_lossy().replace('\\', "\\\\");
    let config_toml = format!(
        "ignored_bins = []\ndefault_threshold_days = 90\narchive_path = \"{}\"\nwindows_use_access_time = false\n",
        archive_str
    );
    fs::write(cfg_dir.join("config.toml"), config_toml).expect("Failed to write config.toml");
    let file_path = test_dir.join("old_tool.exe");

    fs::write(&file_path, "content").expect("Failed to write test file");

    let old_time = SystemTime::now() - Duration::from_secs(86400 * 100);
    let ft = FileTime::from_system_time(old_time);
    set_file_times(&file_path, ft, ft).expect("Failed to backdate file");

    let output = run_cli(
        &["scan", "-p", test_dir.to_str().unwrap(), "--days", "30"],
        &config_root,
    );

    // Save artifacts for inspection (use BIN_EXPIRE_TEST_KEEP=1 to keep dirs).
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

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

    // Scan tables use glyphs: ✗ stale, ✓ ok, · stub.
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

    let _ = fs::remove_file(&file_path);
    cleanup_dir(&test_dir);
    cleanup_dir(&archive_dir);
    cleanup_dir(&config_root);
}

/// This test verifies that `archive` records a manifest entry and `restore` puts it back.
#[test]
fn test_archive_and_restore_roundtrip() {
    let test_dir = unique_dir("test_integration_dir_roundtrip");
    let config_root = unique_dir("test_integration_config_roundtrip");
    let archive_dir = unique_dir("test_integration_archive_roundtrip");

    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    fs::create_dir_all(&archive_dir).expect("Failed to create archive dir");

    let cfg_dir = config_root.join("bin-expire");
    fs::create_dir_all(&cfg_dir).expect("Failed to create config dir");

    // Escape backslashes for TOML on Windows.
    let archive_str = archive_dir.to_string_lossy().replace('\\', "\\\\");
    let config_toml = format!(
        "ignored_bins = []\ndefault_threshold_days = 90\narchive_path = \"{}\"\nwindows_use_access_time = false\n",
        archive_str
    );
    fs::write(cfg_dir.join("config.toml"), config_toml).expect("Failed to write config.toml");

    // Create a stale file.
    let file_name = "old_tool.exe";
    let file_path = test_dir.join(file_name);
    fs::write(&file_path, "content").expect("Failed to write test file");

    let old_time = SystemTime::now() - Duration::from_secs(86400 * 100);
    let ft = FileTime::from_system_time(old_time);
    set_file_times(&file_path, ft, ft).expect("Failed to backdate file");

    let artifacts_dir = test_dir.join("_test_artifacts");

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

    let archived_path = archive_dir.join(file_name);
    assert!(
        !file_path.exists(),
        "Original file still exists after archive"
    );
    assert!(archived_path.exists(), "Archived file was not found");

    let manifest_path = cfg_dir.join("archive.json");
    assert!(
        manifest_path.exists(),
        "Manifest archive.json was not created"
    );

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

    let _ = fs::remove_file(&file_path);
    let _ = fs::remove_file(&archived_path);
    let _ = fs::remove_file(&manifest_path);
    let _ = fs::remove_file(cfg_dir.join("config.toml"));
    cleanup_dir(&test_dir);
    cleanup_dir(&archive_dir);
    cleanup_dir(&config_root);
}

/// This test verifies restore safety behavior:
/// - If destination exists, restore should fail and NOT remove manifest entry.
/// - If archived file is missing, restore should fail and NOT remove manifest entry.
///   Also verifies a second archive run still records entries.
#[test]
fn test_restore_safety_and_archive_again() {
    let test_dir = unique_dir("test_integration_dir_restore_safety");
    let config_root = unique_dir("test_integration_config_restore_safety");
    let archive_dir = unique_dir("test_integration_archive_restore_safety");

    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    fs::create_dir_all(&archive_dir).expect("Failed to create archive dir");

    let cfg_dir = config_root.join("bin-expire");
    fs::create_dir_all(&cfg_dir).expect("Failed to create config dir");

    // Deterministic: use mtime, not atime.
    let archive_str = archive_dir.to_string_lossy().replace('\\', "\\\\");
    let config_toml = format!(
        "ignored_bins = []\ndefault_threshold_days = 90\narchive_path = \"{}\"\nwindows_use_access_time = false\n",
        archive_str
    );
    fs::write(cfg_dir.join("config.toml"), config_toml).expect("Failed to write config.toml");

    let artifacts_dir = test_dir.join("_test_artifacts");
    let manifest_path = cfg_dir.join("archive.json");

    // Create a stale file so archive will pick it up.
    let file_name = "old_tool.exe";
    let file_path = test_dir.join(file_name);
    fs::write(&file_path, "content").expect("Failed to write test file");
    let old_time = SystemTime::now() - Duration::from_secs(86400 * 100);
    let ft = FileTime::from_system_time(old_time);
    set_file_times(&file_path, ft, ft).expect("Failed to backdate file");

    let output = run_cli(
        &["archive", "-p", test_dir.to_str().unwrap(), "--days", "30"],
        &config_root,
    );
    write_artifact(&artifacts_dir, "archive1.stdout.txt", &output.stdout);
    write_artifact(&artifacts_dir, "archive1.stderr.txt", &output.stderr);
    assert!(
        output.status.success(),
        "Archive(1) failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let archived_path = archive_dir.join(file_name);
    assert!(
        !file_path.exists(),
        "Original file still exists after archive"
    );
    assert!(
        archived_path.exists(),
        "Archived file not found after archive"
    );

    let names = read_manifest_names(&manifest_path);
    assert!(
        names.iter().any(|n| n == file_name),
        "Manifest did not include {}. names={:?}",
        file_name,
        names
    );

    // Case: destination exists -> restore must fail and keep the manifest entry.
    fs::write(&file_path, "collision").expect("Failed to create destination collision file");
    let output = run_cli(&["restore", file_name], &config_root);
    write_artifact(
        &artifacts_dir,
        "restore_destination_exists.stdout.txt",
        &output.stdout,
    );
    write_artifact(
        &artifacts_dir,
        "restore_destination_exists.stderr.txt",
        &output.stderr,
    );
    assert!(
        !output.status.success(),
        "Restore unexpectedly succeeded when destination existed. stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        archived_path.exists(),
        "Archived file should still exist after failed restore"
    );
    let names = read_manifest_names(&manifest_path);
    assert!(
        names.iter().any(|n| n == file_name),
        "Manifest entry was removed on failed restore (destination exists). names={:?}",
        names
    );

    let _ = fs::remove_file(&file_path);

    // Case: archived file missing -> restore must fail and keep the manifest entry.
    fs::remove_file(&archived_path).expect("Failed to remove archived file for negative test");
    let output = run_cli(&["restore", file_name], &config_root);
    write_artifact(
        &artifacts_dir,
        "restore_archived_missing.stdout.txt",
        &output.stdout,
    );
    write_artifact(
        &artifacts_dir,
        "restore_archived_missing.stderr.txt",
        &output.stderr,
    );
    assert!(
        !output.status.success(),
        "Restore unexpectedly succeeded when archived file missing. stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = read_manifest_names(&manifest_path);
    assert!(
        names.iter().any(|n| n == file_name),
        "Manifest entry was removed on failed restore (archived missing). names={:?}",
        names
    );

    // Archive again to ensure archive still works after restore failures.
    let file2 = "old_tool2.exe";
    let file2_path = test_dir.join(file2);
    fs::write(&file2_path, "content2").expect("Failed to write test file 2");
    set_file_times(&file2_path, ft, ft).expect("Failed to backdate file 2");
    let output = run_cli(
        &["archive", "-p", test_dir.to_str().unwrap(), "--days", "30"],
        &config_root,
    );
    write_artifact(&artifacts_dir, "archive2.stdout.txt", &output.stdout);
    write_artifact(&artifacts_dir, "archive2.stderr.txt", &output.stderr);
    assert!(
        output.status.success(),
        "Archive(2) failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        archive_dir.join(file2).exists(),
        "Second archived file was not found"
    );
    let names = read_manifest_names(&manifest_path);
    assert!(
        names.iter().any(|n| n == file2),
        "Manifest did not include {} after second archive. names={:?}",
        file2,
        names
    );

    println!("[bin-expire integration] test_restore_safety_and_archive_again: PASS");
    println!("[bin-expire integration] test_dir={}", test_dir.display());
    println!(
        "[bin-expire integration] artifacts_dir={} (set BIN_EXPIRE_TEST_KEEP=1 to keep dirs)",
        artifacts_dir.display()
    );
    if test_verbose() {
        eprintln!(
            "[bin-expire integration] final manifest:\n{}",
            fs::read_to_string(&manifest_path).unwrap_or_default()
        );
    }

    cleanup_dir(&test_dir);
    cleanup_dir(&archive_dir);
    cleanup_dir(&config_root);
}
